#![forbid(unsafe_code)]

//! Mermaid showcase screen — state + command handling scaffold.

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::mermaid::{MermaidGlyphMode, MermaidTier, MermaidWrapMode};
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;

use super::{HelpEntry, Screen};
use crate::theme;

const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN: f32 = 0.2;
const ZOOM_MAX: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Auto,
    Dense,
    Spacious,
}

impl LayoutMode {
    const fn next(self) -> Self {
        match self {
            Self::Auto => Self::Dense,
            Self::Dense => Self::Spacious,
            Self::Spacious => Self::Auto,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Dense => "Dense",
            Self::Spacious => "Spacious",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MermaidSample {
    name: &'static str,
    source: &'static str,
    tags: &'static [&'static str],
}

const DEFAULT_SAMPLES: &[MermaidSample] = &[
    MermaidSample {
        name: "Flow Basic",
        source: "graph LR\nA[Start] --> B{Check}\nB -->|Yes| C[OK]\nB -->|No| D[Fix]",
        tags: &["flow", "basic"],
    },
    MermaidSample {
        name: "Sequence Mini",
        source: "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: Hi!",
        tags: &["sequence", "compact"],
    },
    MermaidSample {
        name: "Mindmap Seed",
        source: "mindmap\n  root\n    alpha\n    beta\n      beta-1\n      beta-2",
        tags: &["mindmap", "tree"],
    },
];

#[derive(Debug, Clone, Copy, Default)]
struct MermaidMetricsSnapshot {
    parse_ms: Option<f32>,
    layout_ms: Option<f32>,
    render_ms: Option<f32>,
    layout_iterations: Option<u32>,
    objective_score: Option<f32>,
    constraint_violations: Option<u32>,
    fallback_tier: Option<MermaidTier>,
    fallback_reason: Option<&'static str>,
}

#[derive(Debug)]
struct MermaidShowcaseState {
    samples: Vec<MermaidSample>,
    selected_index: usize,
    layout_mode: LayoutMode,
    tier: MermaidTier,
    glyph_mode: MermaidGlyphMode,
    wrap_mode: MermaidWrapMode,
    styles_enabled: bool,
    metrics_visible: bool,
    controls_visible: bool,
    viewport_zoom: f32,
    viewport_pan: (i16, i16),
    render_epoch: u64,
    metrics: MermaidMetricsSnapshot,
}

impl MermaidShowcaseState {
    fn new() -> Self {
        Self {
            samples: DEFAULT_SAMPLES.to_vec(),
            selected_index: 0,
            layout_mode: LayoutMode::Auto,
            tier: MermaidTier::Auto,
            glyph_mode: MermaidGlyphMode::Unicode,
            wrap_mode: MermaidWrapMode::WordChar,
            styles_enabled: true,
            metrics_visible: true,
            controls_visible: true,
            viewport_zoom: 1.0,
            viewport_pan: (0, 0),
            render_epoch: 0,
            metrics: MermaidMetricsSnapshot::default(),
        }
    }

    fn selected_sample(&self) -> Option<MermaidSample> {
        self.samples.get(self.selected_index).copied()
    }

    fn clamp_zoom(&mut self) {
        self.viewport_zoom = self.viewport_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    fn apply_action(&mut self, action: MermaidShowcaseAction) {
        match action {
            MermaidShowcaseAction::NextSample => {
                if !self.samples.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.samples.len();
                    self.render_epoch = self.render_epoch.saturating_add(1);
                }
            }
            MermaidShowcaseAction::PrevSample => {
                if !self.samples.is_empty() {
                    self.selected_index =
                        (self.selected_index + self.samples.len() - 1) % self.samples.len();
                    self.render_epoch = self.render_epoch.saturating_add(1);
                }
            }
            MermaidShowcaseAction::FirstSample => {
                if !self.samples.is_empty() {
                    self.selected_index = 0;
                    self.render_epoch = self.render_epoch.saturating_add(1);
                }
            }
            MermaidShowcaseAction::LastSample => {
                if !self.samples.is_empty() {
                    self.selected_index = self.samples.len() - 1;
                    self.render_epoch = self.render_epoch.saturating_add(1);
                }
            }
            MermaidShowcaseAction::Refresh => {
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
            MermaidShowcaseAction::ZoomIn => {
                self.viewport_zoom += ZOOM_STEP;
                self.clamp_zoom();
            }
            MermaidShowcaseAction::ZoomOut => {
                self.viewport_zoom -= ZOOM_STEP;
                self.clamp_zoom();
            }
            MermaidShowcaseAction::ZoomReset => {
                self.viewport_zoom = 1.0;
                self.viewport_pan = (0, 0);
            }
            MermaidShowcaseAction::FitToView => {
                self.viewport_zoom = 1.0;
                self.viewport_pan = (0, 0);
            }
            MermaidShowcaseAction::ToggleLayoutMode => {
                self.layout_mode = self.layout_mode.next();
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
            MermaidShowcaseAction::ForceRelayout => {
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
            MermaidShowcaseAction::ToggleMetrics => {
                self.metrics_visible = !self.metrics_visible;
            }
            MermaidShowcaseAction::ToggleControls => {
                self.controls_visible = !self.controls_visible;
            }
            MermaidShowcaseAction::CycleTier => {
                self.tier = match self.tier {
                    MermaidTier::Auto => MermaidTier::Rich,
                    MermaidTier::Rich => MermaidTier::Normal,
                    MermaidTier::Normal => MermaidTier::Compact,
                    MermaidTier::Compact => MermaidTier::Auto,
                };
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
            MermaidShowcaseAction::ToggleGlyphMode => {
                self.glyph_mode = match self.glyph_mode {
                    MermaidGlyphMode::Unicode => MermaidGlyphMode::Ascii,
                    MermaidGlyphMode::Ascii => MermaidGlyphMode::Unicode,
                };
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
            MermaidShowcaseAction::ToggleStyles => {
                self.styles_enabled = !self.styles_enabled;
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
            MermaidShowcaseAction::CycleWrapMode => {
                self.wrap_mode = match self.wrap_mode {
                    MermaidWrapMode::None => MermaidWrapMode::Word,
                    MermaidWrapMode::Word => MermaidWrapMode::Char,
                    MermaidWrapMode::Char => MermaidWrapMode::WordChar,
                    MermaidWrapMode::WordChar => MermaidWrapMode::None,
                };
                self.render_epoch = self.render_epoch.saturating_add(1);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MermaidShowcaseAction {
    NextSample,
    PrevSample,
    FirstSample,
    LastSample,
    Refresh,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitToView,
    ToggleLayoutMode,
    ForceRelayout,
    ToggleMetrics,
    ToggleControls,
    CycleTier,
    ToggleGlyphMode,
    ToggleStyles,
    CycleWrapMode,
}

/// Mermaid showcase screen scaffold (state + key handling).
pub struct MermaidShowcaseScreen {
    state: MermaidShowcaseState,
}

impl Default for MermaidShowcaseScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidShowcaseScreen {
    pub fn new() -> Self {
        Self {
            state: MermaidShowcaseState::new(),
        }
    }

    fn handle_key(&self, event: &KeyEvent) -> Option<MermaidShowcaseAction> {
        if event.kind != KeyEventKind::Press {
            return None;
        }

        match event.code {
            KeyCode::Down | KeyCode::Char('j') => Some(MermaidShowcaseAction::NextSample),
            KeyCode::Up | KeyCode::Char('k') => Some(MermaidShowcaseAction::PrevSample),
            KeyCode::Home => Some(MermaidShowcaseAction::FirstSample),
            KeyCode::End => Some(MermaidShowcaseAction::LastSample),
            KeyCode::Enter => Some(MermaidShowcaseAction::Refresh),
            KeyCode::Char('+') | KeyCode::Char('=') => Some(MermaidShowcaseAction::ZoomIn),
            KeyCode::Char('-') => Some(MermaidShowcaseAction::ZoomOut),
            KeyCode::Char('0') => Some(MermaidShowcaseAction::ZoomReset),
            KeyCode::Char('f') => Some(MermaidShowcaseAction::FitToView),
            KeyCode::Char('l') => Some(MermaidShowcaseAction::ToggleLayoutMode),
            KeyCode::Char('r') => Some(MermaidShowcaseAction::ForceRelayout),
            KeyCode::Char('m') => Some(MermaidShowcaseAction::ToggleMetrics),
            KeyCode::Char('c') => Some(MermaidShowcaseAction::ToggleControls),
            KeyCode::Char('t') => Some(MermaidShowcaseAction::CycleTier),
            KeyCode::Char('g') => Some(MermaidShowcaseAction::ToggleGlyphMode),
            KeyCode::Char('s') => Some(MermaidShowcaseAction::ToggleStyles),
            KeyCode::Char('w') => Some(MermaidShowcaseAction::CycleWrapMode),
            _ => None,
        }
    }

    fn split_header_body_footer(&self, area: Rect) -> (Rect, Rect, Rect) {
        if area.height >= 3 {
            let rows = Flex::vertical()
                .constraints([
                    Constraint::Fixed(1),
                    Constraint::Min(1),
                    Constraint::Fixed(1),
                ])
                .split(area);
            return (rows[0], rows[1], rows[2]);
        }

        let empty = Rect::new(area.x, area.y, area.width, 0);
        (empty, area, empty)
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let sample = self
            .state
            .selected_sample()
            .map(|s| s.name)
            .unwrap_or("None");
        let total = self.state.samples.len();
        let index = self.state.selected_index.saturating_add(1).min(total);
        let status = if self.state.metrics.fallback_tier.is_some() {
            "WARN"
        } else {
            "OK"
        };
        let text = format!(
            "Mermaid Showcase | Sample: {} ({}/{}) | Layout: {} | Tier: {} | Glyphs: {} | {}",
            sample,
            index,
            total,
            self.state.layout_mode.as_str(),
            self.state.tier,
            self.state.glyph_mode,
            status
        );
        Paragraph::new(text)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::DEEP))
            .render(area, frame);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let hint = "j/k sample  l layout  r relayout  +/- zoom  m metrics  t tier";
        let metrics = if self.state.metrics_visible {
            format!(
                "parse {}ms | layout {}ms | render {}ms",
                self.state.metrics.parse_ms.unwrap_or(0.0),
                self.state.metrics.layout_ms.unwrap_or(0.0),
                self.state.metrics.render_ms.unwrap_or(0.0)
            )
        } else {
            "metrics hidden (m)".to_string()
        };
        let text = format!("{hint} | {metrics}");
        Paragraph::new(text)
            .style(Style::new().fg(theme::fg::MUTED).bg(theme::bg::BASE))
            .render(area, frame);
    }

    fn render_samples(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Samples")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::DEEP));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let mut lines = Vec::new();
        for (idx, sample) in self.state.samples.iter().enumerate() {
            let prefix = if idx == self.state.selected_index {
                "> "
            } else {
                "  "
            };
            let tag_str = if sample.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", sample.tags.join(", "))
            };
            lines.push(format!("{prefix}{}{}", sample.name, tag_str));
        }

        Paragraph::new(lines.join("\n"))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(inner, frame);
    }

    fn render_viewport(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Viewport")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::BASE));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let sample = self.state.selected_sample();
        let mut lines = Vec::new();
        if let Some(sample) = sample {
            lines.push(format!("Sample: {}", sample.name));
            lines.push(String::new());
            for line in sample.source.lines() {
                lines.push(line.to_string());
            }
        } else {
            lines.push("No samples loaded.".to_string());
        }

        Paragraph::new(lines.join("\n"))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(inner, frame);
    }

    fn render_controls_panel(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Controls")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::DEEP));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let lines = [
            format!("Layout: {} (l)", self.state.layout_mode.as_str()),
            format!("Tier: {} (t)", self.state.tier),
            format!("Glyphs: {} (g)", self.state.glyph_mode),
            format!("Wrap: {} (w)", self.state.wrap_mode),
            format!(
                "Styles: {} (s)",
                if self.state.styles_enabled {
                    "on"
                } else {
                    "off"
                }
            ),
            format!("Zoom: {:.0}% (+/-)", self.state.viewport_zoom * 100.0),
            "Fit: f".to_string(),
            format!(
                "Metrics: {} (m)",
                if self.state.metrics_visible {
                    "on"
                } else {
                    "off"
                }
            ),
        ];

        Paragraph::new(lines.join("\n"))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(inner, frame);
    }

    fn render_metrics_panel(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Metrics")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::DEEP));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let metrics = &self.state.metrics;
        let mut lines = Vec::new();
        if self.state.metrics_visible {
            lines.push(format!("Parse: {} ms", metrics.parse_ms.unwrap_or(0.0)));
            lines.push(format!("Layout: {} ms", metrics.layout_ms.unwrap_or(0.0)));
            lines.push(format!("Render: {} ms", metrics.render_ms.unwrap_or(0.0)));
            lines.push(format!(
                "Iterations: {}",
                metrics.layout_iterations.unwrap_or(0)
            ));
            lines.push(format!(
                "Objective: {}",
                metrics.objective_score.unwrap_or(0.0)
            ));
            lines.push(format!(
                "Violations: {}",
                metrics.constraint_violations.unwrap_or(0)
            ));
            if let Some(tier) = metrics.fallback_tier {
                lines.push(format!("Fallback: {}", tier));
            }
            if let Some(reason) = metrics.fallback_reason {
                lines.push(format!("Reason: {}", reason));
            }
        } else {
            lines.push("Metrics hidden (press m)".to_string());
        }

        Paragraph::new(lines.join("\n"))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(inner, frame);
    }
}

impl Screen for MermaidShowcaseScreen {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(key) = event
            && let Some(action) = self.handle_key(key)
        {
            self.state.apply_action(action);
        }
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let (header, body, footer) = self.split_header_body_footer(area);
        self.render_header(frame, header);
        self.render_footer(frame, footer);

        if body.is_empty() {
            return;
        }

        if body.width >= 120 {
            let columns = Flex::horizontal()
                .constraints([
                    Constraint::Percentage(26.0),
                    Constraint::Percentage(52.0),
                    Constraint::Percentage(22.0),
                ])
                .split(body);
            self.render_samples(frame, columns[0]);
            self.render_viewport(frame, columns[1]);
            let right = columns[2];
            if right.is_empty() {
                return;
            }
            if self.state.controls_visible && self.state.metrics_visible && right.height >= 12 {
                let rows = Flex::vertical()
                    .constraints([Constraint::Percentage(55.0), Constraint::Min(6)])
                    .split(right);
                self.render_controls_panel(frame, rows[0]);
                self.render_metrics_panel(frame, rows[1]);
            } else if self.state.controls_visible {
                self.render_controls_panel(frame, right);
            } else if self.state.metrics_visible {
                self.render_metrics_panel(frame, right);
            }
            return;
        }

        if body.width >= 80 {
            let columns = Flex::horizontal()
                .constraints([Constraint::Percentage(30.0), Constraint::Percentage(70.0)])
                .split(body);
            self.render_samples(frame, columns[0]);
            let right = columns[1];
            if self.state.metrics_visible && right.height >= 10 {
                let rows = Flex::vertical()
                    .constraints([Constraint::Min(1), Constraint::Fixed(8)])
                    .split(right);
                self.render_viewport(frame, rows[0]);
                self.render_metrics_panel(frame, rows[1]);
            } else {
                self.render_viewport(frame, right);
            }
            return;
        }

        let rows = Flex::vertical()
            .constraints([Constraint::Fixed(6), Constraint::Min(1)])
            .split(body);
        self.render_samples(frame, rows[0]);
        self.render_viewport(frame, rows[1]);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "j / Down",
                action: "Next sample",
            },
            HelpEntry {
                key: "k / Up",
                action: "Previous sample",
            },
            HelpEntry {
                key: "Enter",
                action: "Re-render sample",
            },
            HelpEntry {
                key: "l",
                action: "Toggle layout mode",
            },
            HelpEntry {
                key: "r",
                action: "Force re-layout",
            },
            HelpEntry {
                key: "+ / -",
                action: "Zoom in/out",
            },
            HelpEntry {
                key: "f",
                action: "Fit to viewport",
            },
            HelpEntry {
                key: "m",
                action: "Toggle metrics",
            },
            HelpEntry {
                key: "c",
                action: "Toggle controls",
            },
            HelpEntry {
                key: "t",
                action: "Cycle fidelity tier",
            },
            HelpEntry {
                key: "g",
                action: "Toggle glyph mode",
            },
            HelpEntry {
                key: "s",
                action: "Toggle styles",
            },
            HelpEntry {
                key: "w",
                action: "Cycle wrap mode",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Mermaid Showcase"
    }

    fn tab_label(&self) -> &'static str {
        "Mermaid"
    }
}
