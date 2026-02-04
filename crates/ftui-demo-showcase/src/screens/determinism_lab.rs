#![forbid(unsafe_code)]

//! Determinism Lab — checksum equivalence across diff strategies.
//!
//! Demonstrates:
//! - Full vs DirtyRows vs FullRedraw equivalence
//! - Per-frame change counts
//! - Mismatch detection (first coordinate + delta count)
//! - Deterministic checksum timeline
//! - JSONL export of verification reports

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;

use ftui_core::event::{Event, KeyCode, KeyEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::buffer::Buffer;
use ftui_render::cell::Cell;
use ftui_render::diff::BufferDiff;
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::WrapMode;
use ftui_text::text::{Line, Span, Text};
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;

use super::{HelpEntry, Screen};
use crate::determinism;
use crate::theme;

const SCENE_WIDTH: u16 = 60;
const SCENE_HEIGHT: u16 = 18;
const HISTORY_LEN: usize = 12;
const DEFAULT_SEED: u64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrategyKind {
    Full,
    DirtyRows,
    FullRedraw,
}

impl StrategyKind {
    const ALL: [StrategyKind; 3] = [Self::Full, Self::DirtyRows, Self::FullRedraw];

    fn label(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::DirtyRows => "DirtyRows",
            Self::FullRedraw => "FullRedraw",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::DirtyRows => "Dirty",
            Self::FullRedraw => "Redraw",
        }
    }

    fn key_hint(self) -> &'static str {
        match self {
            Self::Full => "1",
            Self::DirtyRows => "2",
            Self::FullRedraw => "3",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MismatchInfo {
    x: u16,
    y: u16,
    count: usize,
}

#[derive(Debug, Clone, Copy)]
struct StrategyResult {
    checksum: u64,
    change_count: usize,
    mismatch: Option<MismatchInfo>,
}

impl StrategyResult {
    fn status_label(self) -> &'static str {
        if self.mismatch.is_some() {
            "MISMATCH"
        } else {
            "OK"
        }
    }
}

#[derive(Debug, Clone)]
struct StrategyResults {
    full: StrategyResult,
    dirty: StrategyResult,
    redraw: StrategyResult,
}

impl StrategyResults {
    fn get(&self, strategy: StrategyKind) -> StrategyResult {
        match strategy {
            StrategyKind::Full => self.full,
            StrategyKind::DirtyRows => self.dirty,
            StrategyKind::FullRedraw => self.redraw,
        }
    }

    fn first_mismatch(&self) -> Option<(StrategyKind, MismatchInfo)> {
        if let Some(info) = self.full.mismatch {
            return Some((StrategyKind::Full, info));
        }
        if let Some(info) = self.dirty.mismatch {
            return Some((StrategyKind::DirtyRows, info));
        }
        if let Some(info) = self.redraw.mismatch {
            return Some((StrategyKind::FullRedraw, info));
        }
        None
    }
}

#[derive(Debug, Clone)]
struct ExportStatus {
    path: String,
    ok: bool,
    message: String,
}

pub struct DeterminismLab {
    seed: u64,
    frame_index: u64,
    paused: bool,
    active_strategy: StrategyKind,
    inject_fault: bool,
    prev_buffer: Buffer,
    current_buffer: Buffer,
    results: StrategyResults,
    history_full: VecDeque<u64>,
    history_dirty: VecDeque<u64>,
    history_redraw: VecDeque<u64>,
    last_export: Option<ExportStatus>,
    export_path: String,
}

impl Default for DeterminismLab {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterminismLab {
    pub fn new() -> Self {
        let base = Buffer::new(SCENE_WIDTH, SCENE_HEIGHT);
        let mut lab = Self {
            seed: determinism::demo_seed(DEFAULT_SEED),
            frame_index: 0,
            paused: false,
            active_strategy: StrategyKind::DirtyRows,
            inject_fault: false,
            prev_buffer: base.clone(),
            current_buffer: base,
            results: StrategyResults {
                full: StrategyResult {
                    checksum: 0,
                    change_count: 0,
                    mismatch: None,
                },
                dirty: StrategyResult {
                    checksum: 0,
                    change_count: 0,
                    mismatch: None,
                },
                redraw: StrategyResult {
                    checksum: 0,
                    change_count: 0,
                    mismatch: None,
                },
            },
            history_full: VecDeque::with_capacity(HISTORY_LEN),
            history_dirty: VecDeque::with_capacity(HISTORY_LEN),
            history_redraw: VecDeque::with_capacity(HISTORY_LEN),
            last_export: None,
            export_path: std::env::var("FTUI_DETERMINISM_LAB_REPORT")
                .unwrap_or_else(|_| "determinism_lab_report.jsonl".to_string()),
        };

        lab.reset_scene();
        lab
    }

    fn reset_scene(&mut self) {
        let base = Buffer::new(SCENE_WIDTH, SCENE_HEIGHT);
        let next = self.generate_next_buffer(&base, 0);
        self.prev_buffer = base;
        self.current_buffer = next;
        self.frame_index = 0;
        self.history_full.clear();
        self.history_dirty.clear();
        self.history_redraw.clear();
        let results = self.compute_results(&self.prev_buffer, &self.current_buffer);
        self.push_history(&results);
        self.results = results;
    }

    fn generate_next_buffer(&self, base: &Buffer, frame_index: u64) -> Buffer {
        let mut next = base.clone();
        next.clear_dirty();

        let mut state = self.seed ^ frame_index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let width = next.width();
        let height = next.height();

        let row_count = 3 + (state % 2) as usize;
        let mut rows = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            state = lcg_next(state);
            rows.push((state as u16) % height);
        }

        let cells_per_row = (width as usize / 3).clamp(6, 20);
        for (row_idx, &y) in rows.iter().enumerate() {
            for _ in 0..cells_per_row {
                state = lcg_next(state);
                let x = (state as u16) % width;
                let ch = char::from_u32(('A' as u32) + (state as u32 % 26)).unwrap_or('X');
                let fg = match (row_idx + (state as usize)) % 4 {
                    0 => theme::accent::INFO,
                    1 => theme::accent::SUCCESS,
                    2 => theme::accent::WARNING,
                    _ => theme::accent::ERROR,
                };
                let cell = Cell::from_char(ch).with_fg(fg.into());
                next.set_raw(x, y, cell);
            }
        }

        let cursor_x = ((frame_index * 5) % (width as u64)) as u16;
        let cursor_y = ((frame_index * 3) % (height as u64)) as u16;
        let cursor = Cell::from_char('O').with_fg(theme::accent::ACCENT_1.into());
        next.set_raw(cursor_x, cursor_y, cursor);

        next
    }

    fn advance_frame(&mut self) {
        if self.paused {
            return;
        }
        let next = self.generate_next_buffer(&self.current_buffer, self.frame_index + 1);
        let results = self.compute_results(&self.current_buffer, &next);
        self.prev_buffer = self.current_buffer.clone();
        self.current_buffer = next;
        self.frame_index = self.frame_index.wrapping_add(1);
        self.push_history(&results);
        self.results = results;
    }

    fn compute_results(&self, prev: &Buffer, next: &Buffer) -> StrategyResults {
        let full_diff = BufferDiff::compute(prev, next);
        let dirty_diff = BufferDiff::compute_dirty(prev, next);

        let full_applied = apply_diff(prev, next, &full_diff);
        let mut dirty_applied = apply_diff(prev, next, &dirty_diff);
        let redraw_applied = next.clone();

        if self.inject_fault {
            let fault_cell = Cell::from_char('!').with_fg(theme::accent::ERROR.into());
            dirty_applied.set_raw(0, 0, fault_cell);
        }

        let full_mismatch = compare_buffers(next, &full_applied);
        let dirty_mismatch = compare_buffers(next, &dirty_applied);
        let redraw_mismatch = compare_buffers(next, &redraw_applied);

        StrategyResults {
            full: StrategyResult {
                checksum: checksum_buffer(&full_applied),
                change_count: full_diff.len(),
                mismatch: full_mismatch,
            },
            dirty: StrategyResult {
                checksum: checksum_buffer(&dirty_applied),
                change_count: dirty_diff.len(),
                mismatch: dirty_mismatch,
            },
            redraw: StrategyResult {
                checksum: checksum_buffer(&redraw_applied),
                change_count: (next.width() as usize) * (next.height() as usize),
                mismatch: redraw_mismatch,
            },
        }
    }

    fn push_history(&mut self, results: &StrategyResults) {
        push_history(&mut self.history_full, results.full.checksum);
        push_history(&mut self.history_dirty, results.dirty.checksum);
        push_history(&mut self.history_redraw, results.redraw.checksum);
    }

    fn history_for(&self, strategy: StrategyKind) -> &VecDeque<u64> {
        match strategy {
            StrategyKind::Full => &self.history_full,
            StrategyKind::DirtyRows => &self.history_dirty,
            StrategyKind::FullRedraw => &self.history_redraw,
        }
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    fn bump_seed(&mut self, delta: i64) {
        if delta.is_negative() {
            self.seed = self.seed.saturating_sub(1);
        } else {
            self.seed = self.seed.saturating_add(1);
        }
        self.seed = self.seed.max(1);
        self.reset_scene();
    }

    fn toggle_fault(&mut self) {
        self.inject_fault = !self.inject_fault;
        self.results = self.compute_results(&self.prev_buffer, &self.current_buffer);
    }

    fn json_escape(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn export_report(&mut self) {
        let path = self.export_path.clone();
        let mut message = String::new();
        let mut ok = true;

        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut file) => {
                let env_json = determinism::demo_env_json();
                let hash_key = determinism::demo_hash_key(
                    self.current_buffer.width(),
                    self.current_buffer.height(),
                );
                let run_id = determinism::demo_run_id();
                let run_id_json = run_id
                    .as_ref()
                    .map(|value| format!("\"{}\"", Self::json_escape(value)))
                    .unwrap_or_else(|| "null".to_string());
                let env_line = format!(
                    "{{\"event\":\"determinism_env\",\"timestamp\":\"{}\",\"run_id\":{},\"hash_key\":\"{}\",\"seed\":{},\"width\":{},\"height\":{},\"env\":{}}}\n",
                    determinism::chrono_like_timestamp(),
                    run_id_json,
                    Self::json_escape(&hash_key),
                    self.seed,
                    self.current_buffer.width(),
                    self.current_buffer.height(),
                    env_json
                );
                if let Err(err) = file.write_all(env_line.as_bytes()) {
                    ok = false;
                    message = format!("env write failed: {err}");
                }
                if !ok {
                    self.last_export = Some(ExportStatus { path, ok, message });
                    return;
                }
                for strategy in StrategyKind::ALL {
                    let result = self.results.get(strategy);
                    let (mismatch_x, mismatch_y, mismatch_count) = match result.mismatch {
                        Some(info) => (info.x as i64, info.y as i64, info.count as i64),
                        None => (-1, -1, 0),
                    };
                    let timestamp = determinism::chrono_like_timestamp();
                    let line = format!(
                        "{{\"event\":\"determinism_report\",\"timestamp\":\"{}\",\"run_id\":{},\"hash_key\":\"{}\",\"frame\":{},\"seed\":{},\"width\":{},\"height\":{},\"strategy\":\"{}\",\"checksum\":\"0x{:016x}\",\"changes\":{},\"mismatch_count\":{},\"mismatch_x\":{},\"mismatch_y\":{}}}\n",
                        timestamp,
                        run_id_json,
                        Self::json_escape(&hash_key),
                        self.frame_index,
                        self.seed,
                        self.current_buffer.width(),
                        self.current_buffer.height(),
                        strategy.label(),
                        result.checksum,
                        result.change_count,
                        mismatch_count,
                        mismatch_x,
                        mismatch_y
                    );
                    if let Err(err) = file.write_all(line.as_bytes()) {
                        ok = false;
                        message = format!("write failed: {err}");
                        break;
                    }
                }
                if ok {
                    message = "report appended".to_string();
                }
            }
            Err(err) => {
                ok = false;
                message = format!("open failed: {err}");
            }
        }

        self.last_export = Some(ExportStatus { path, ok, message });
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let accent = Style::new().fg(theme::screen_accent::PERFORMANCE).bold();
        let muted = theme::muted();
        let status = if self.paused { "Paused" } else { "Live" };
        let fault = if self.inject_fault { "ON" } else { "off" };

        let line = Line::from_spans(vec![
            Span::styled("Determinism Lab", accent),
            Span::raw("  "),
            Span::styled("Seed: ", muted),
            Span::raw(format!("{}", self.seed)),
            Span::raw("  "),
            Span::styled("Frame: ", muted),
            Span::raw(format!("{}", self.frame_index)),
            Span::raw("  "),
            Span::styled("Status: ", muted),
            Span::raw(status),
            Span::raw("  "),
            Span::styled("Fault: ", muted),
            Span::raw(fault),
        ]);

        Paragraph::new(Text::from(line))
            .wrap(WrapMode::None)
            .render(area, frame);
    }

    fn render_results(&self, frame: &mut Frame, area: Rect) {
        let mut lines = Vec::new();
        let muted = theme::muted();
        let accent = Style::new().fg(theme::screen_accent::PERFORMANCE).bold();
        let ok_style = Style::new().fg(theme::accent::SUCCESS);
        let bad_style = Style::new().fg(theme::accent::ERROR).bold();

        lines.push(Line::from_spans(vec![
            Span::styled("Strategy", accent),
            Span::raw("  "),
            Span::styled("Changes", accent),
            Span::raw("   "),
            Span::styled("Checksum", accent),
            Span::raw("             "),
            Span::styled("Status", accent),
        ]));

        for strategy in StrategyKind::ALL {
            let result = self.results.get(strategy);
            let active = strategy == self.active_strategy;
            let row_style = if active {
                Style::new().fg(theme::accent::INFO).bold()
            } else {
                Style::new().fg(theme::fg::PRIMARY)
            };
            let status_style = if result.mismatch.is_some() {
                bad_style
            } else {
                ok_style
            };
            lines.push(Line::from_spans(vec![
                Span::styled(format!("{: <10}", strategy.label()), row_style),
                Span::raw("  "),
                Span::styled(format!("{: >6}", result.change_count), row_style),
                Span::raw("   "),
                Span::styled(format!("0x{:016x}", result.checksum), row_style),
                Span::raw("   "),
                Span::styled(result.status_label(), status_style),
            ]));
        }

        if let Some((strategy, info)) = self.results.first_mismatch() {
            lines.push(Line::from(""));
            lines.push(Line::from_spans(vec![
                Span::styled("Mismatch:", bad_style),
                Span::raw(" "),
                Span::styled(strategy.label(), bad_style),
                Span::raw(" "),
                Span::styled(format!("first at ({}, {})", info.x, info.y), bad_style),
                Span::raw(" "),
                Span::styled(format!("delta {}", info.count), bad_style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from_spans(vec![
            Span::styled("Timeline:", muted),
            Span::raw(" "),
            Span::styled(
                format_history(self.history_for(self.active_strategy)),
                Style::new().fg(theme::accent::ACCENT_2),
            ),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from_spans(vec![
            Span::styled("Controls:", muted),
            Span::raw(" "),
            Span::raw("1/2/3 "),
            Span::styled("strategy", muted),
            Span::raw("  "),
            Span::raw("[/] "),
            Span::styled("seed", muted),
            Span::raw("  "),
            Span::raw("Space "),
            Span::styled("pause", muted),
            Span::raw("  "),
            Span::raw("F "),
            Span::styled("fault", muted),
            Span::raw("  "),
            Span::raw("E "),
            Span::styled("export", muted),
        ]));

        if let Some(export) = &self.last_export {
            let export_style = if export.ok {
                Style::new().fg(theme::accent::SUCCESS)
            } else {
                Style::new().fg(theme::accent::ERROR)
            };
            lines.push(Line::from_spans(vec![
                Span::styled("Last export:", muted),
                Span::raw(" "),
                Span::styled(&export.path, export_style),
                Span::raw(" "),
                Span::styled(&export.message, muted),
            ]));
        }

        Paragraph::new(Text::from_lines(lines))
            .wrap(WrapMode::None)
            .render(area, frame);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let max_w = area.width.min(self.current_buffer.width());
        let max_h = area.height.min(self.current_buffer.height());
        let mut lines = Vec::new();

        for y in 0..max_h {
            let mut row = String::with_capacity(max_w as usize);
            for x in 0..max_w {
                let ch = self
                    .current_buffer
                    .get(x, y)
                    .and_then(|cell| cell.content.as_char())
                    .unwrap_or('.');
                row.push(ch);
            }
            lines.push(Line::from(row));
        }

        Paragraph::new(Text::from_lines(lines))
            .wrap(WrapMode::None)
            .render(area, frame);
    }
}

impl Screen for DeterminismLab {
    type Message = ();

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('1') => self.active_strategy = StrategyKind::Full,
                KeyCode::Char('2') => self.active_strategy = StrategyKind::DirtyRows,
                KeyCode::Char('3') => self.active_strategy = StrategyKind::FullRedraw,
                KeyCode::Char(' ') => self.toggle_pause(),
                KeyCode::Char('[') => self.bump_seed(-1),
                KeyCode::Char(']') => self.bump_seed(1),
                KeyCode::Char('f') | KeyCode::Char('F') => self.toggle_fault(),
                KeyCode::Char('e') | KeyCode::Char('E') => self.export_report(),
                KeyCode::Char('r') | KeyCode::Char('R') => self.reset_scene(),
                _ => {}
            }
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let border = Block::new()
            .title("Determinism Lab")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(theme::content_border());
        let inner = border.inner(area);
        border.render(area, frame);

        let rows = Flex::vertical()
            .constraints([Constraint::Fixed(2), Constraint::Fill])
            .split(inner);

        self.render_header(frame, rows[0]);

        let cols = Flex::horizontal()
            .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
            .split(rows[1]);

        let left = Block::new()
            .title("Equivalence")
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Square)
            .style(theme::panel_border_style(
                false,
                theme::screen_accent::PERFORMANCE,
            ));
        let left_inner = left.inner(cols[0]);
        left.render(cols[0], frame);
        self.render_results(frame, left_inner);

        let right = Block::new()
            .title("Scene Preview")
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Square)
            .style(theme::panel_border_style(
                false,
                theme::screen_accent::PERFORMANCE,
            ));
        let right_inner = right.inner(cols[1]);
        right.render(cols[1], frame);
        self.render_preview(frame, right_inner);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "1/2/3",
                action: "Select Full / DirtyRows / FullRedraw strategy",
            },
            HelpEntry {
                key: "[ / ]",
                action: "Decrease / increase seed",
            },
            HelpEntry {
                key: "Space",
                action: "Pause / resume updates",
            },
            HelpEntry {
                key: "F",
                action: "Toggle fault injection",
            },
            HelpEntry {
                key: "E",
                action: "Export JSONL verification report",
            },
            HelpEntry {
                key: "R",
                action: "Reset scene + timeline",
            },
        ]
    }

    fn tick(&mut self, _tick_count: u64) {
        self.advance_frame();
    }

    fn title(&self) -> &'static str {
        "Determinism Lab"
    }

    fn tab_label(&self) -> &'static str {
        "Determinism"
    }
}

fn lcg_next(state: u64) -> u64 {
    state.wrapping_mul(6364136223846793005).wrapping_add(1)
}

fn apply_diff(base: &Buffer, target: &Buffer, diff: &BufferDiff) -> Buffer {
    let mut result = base.clone();
    for (x, y) in diff.iter() {
        let cell = *target.get_unchecked(x, y);
        result.set_raw(x, y, cell);
    }
    result
}

fn compare_buffers(expected: &Buffer, actual: &Buffer) -> Option<MismatchInfo> {
    let width = expected.width().min(actual.width());
    let height = expected.height().min(actual.height());
    let mut count = 0usize;
    let mut first = None;

    for y in 0..height {
        for x in 0..width {
            let a = expected.get_unchecked(x, y);
            let b = actual.get_unchecked(x, y);
            if !a.bits_eq(b) {
                count += 1;
                if first.is_none() {
                    first = Some((x, y));
                }
            }
        }
    }

    first.map(|(x, y)| MismatchInfo { x, y, count })
}

fn checksum_buffer(buf: &Buffer) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            if let Some(cell) = buf.get(x, y) {
                hash = fnv1a64_u32(hash, cell.content.raw());
                hash = fnv1a64_u32(hash, cell.fg.0);
                hash = fnv1a64_u32(hash, cell.bg.0);
                hash = fnv1a64_u32(hash, pack_attrs(cell.attrs));
            }
        }
    }
    hash
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv1a64_u32(hash: u64, v: u32) -> u64 {
    fnv1a64_bytes(hash, &v.to_le_bytes())
}

fn fnv1a64_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn pack_attrs(attrs: ftui_render::cell::CellAttrs) -> u32 {
    let flags = attrs.flags().bits() as u32;
    let link = attrs.link_id() & 0x00FF_FFFF;
    (flags << 24) | link
}

fn push_history(history: &mut VecDeque<u64>, checksum: u64) {
    if history.len() >= HISTORY_LEN {
        history.pop_front();
    }
    history.push_back(checksum);
}

fn format_history(history: &VecDeque<u64>) -> String {
    if history.is_empty() {
        return "n/a".to_string();
    }
    let mut parts = Vec::with_capacity(history.len());
    for checksum in history.iter() {
        parts.push(format!("{:06x}", checksum & 0x00FF_FFFF));
    }
    parts.join(" ")
}
