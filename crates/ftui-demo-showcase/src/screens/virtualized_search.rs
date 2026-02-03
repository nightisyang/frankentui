#![forbid(unsafe_code)]

//! Virtualized List + Fuzzy Search demo screen.
//!
//! Demonstrates:
//! - `VirtualizedList` with large datasets (10k+ items)
//! - Fuzzy search with incremental filtering
//! - Match highlighting and scoring
//! - Keyboard navigation (J/K, /, Esc, G/Shift+G)
//!
//! Part of bd-2zbk: Demo Showcase: Virtualized List + Fuzzy Search

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::PackedRgba;
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::input::TextInput;
use ftui_widgets::paragraph::Paragraph;

use super::{HelpEntry, Screen};
use crate::theme;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TOTAL_ITEMS: usize = 10_000;

/// Match highlight color (bright yellow/gold).
const MATCH_HIGHLIGHT: PackedRgba = PackedRgba::rgb(255, 215, 0);

// ---------------------------------------------------------------------------
// Fuzzy Matching
// ---------------------------------------------------------------------------

/// A fuzzy match result with score and match positions.
#[derive(Debug, Clone)]
struct FuzzyMatch {
    /// Index into the original items list.
    index: usize,
    /// Match score (higher = better match).
    score: i32,
    /// Positions of matched characters in the item text.
    positions: Vec<usize>,
}

/// Simple fzy-style fuzzy matching.
///
/// Algorithm: Sequential character matching with gap penalties.
/// - Consecutive matches: +10 bonus
/// - Word boundary matches: +5 bonus
/// - Gap penalty: -1 per skipped character
fn fuzzy_match(query: &str, target: &str) -> Option<FuzzyMatch> {
    if query.is_empty() {
        return None;
    }

    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let target_lower: Vec<char> = target.to_lowercase().chars().collect();

    let mut positions = Vec::with_capacity(query_lower.len());
    let mut score: i32 = 0;
    let mut query_idx = 0;
    let mut prev_match_pos: Option<usize> = None;

    for (i, c) in target_lower.iter().enumerate() {
        if query_idx < query_lower.len() && *c == query_lower[query_idx] {
            positions.push(i);

            // Consecutive match bonus
            if let Some(prev) = prev_match_pos {
                if i == prev + 1 {
                    score += 10;
                } else {
                    // Gap penalty
                    score -= (i - prev - 1).min(10) as i32;
                }
            }

            // Word boundary bonus (start of word)
            if i == 0 || target_lower.get(i.saturating_sub(1)).map_or(true, |c| !c.is_alphanumeric()) {
                score += 5;
            }

            // Base score for matching
            score += 1;
            prev_match_pos = Some(i);
            query_idx += 1;
        }
    }

    // All query chars must match
    if query_idx == query_lower.len() {
        Some(FuzzyMatch {
            index: 0, // Will be set by caller
            score,
            positions,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Focus State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    List,
    Search,
}

// ---------------------------------------------------------------------------
// VirtualizedSearch Screen
// ---------------------------------------------------------------------------

pub struct VirtualizedSearch {
    /// All items in the list.
    items: Vec<String>,
    /// Filtered matches (indices into items + scores).
    filtered: Vec<FuzzyMatch>,
    /// Current selection index (into filtered list).
    selected: usize,
    /// Scroll offset.
    scroll_offset: usize,
    /// Viewport height (cached from render).
    viewport_height: usize,
    /// Search input widget.
    search_input: TextInput,
    /// Current search query.
    query: String,
    /// Which element has focus.
    focus: Focus,
    /// Tick counter for animation.
    tick_count: u64,
}

impl Default for VirtualizedSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualizedSearch {
    pub fn new() -> Self {
        // Generate diverse test data
        let items: Vec<String> = (0..TOTAL_ITEMS)
            .map(|i| {
                let category = match i % 8 {
                    0 => "Configuration",
                    1 => "Authentication",
                    2 => "Database",
                    3 => "Network",
                    4 => "FileSystem",
                    5 => "Logging",
                    6 => "Security",
                    _ => "Performance",
                };
                let action = match (i / 8) % 6 {
                    0 => "initialized",
                    1 => "updated",
                    2 => "validated",
                    3 => "processed",
                    4 => "cached",
                    _ => "completed",
                };
                let component = match (i / 48) % 5 {
                    0 => "CoreService",
                    1 => "ApiGateway",
                    2 => "WorkerPool",
                    3 => "CacheManager",
                    _ => "EventBus",
                };
                format!(
                    "[{:05}] {} :: {} {} — payload_{}",
                    i, category, component, action, i % 1000
                )
            })
            .collect();

        let search_input = TextInput::new()
            .with_placeholder("Type to search...")
            .with_style(Style::new().fg(theme::fg::PRIMARY))
            .with_focused(false);

        let mut screen = Self {
            items,
            filtered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            viewport_height: 20,
            search_input,
            query: String::new(),
            focus: Focus::List,
            tick_count: 0,
        };

        // Initialize with all items (no filter)
        screen.update_filter();
        screen
    }

    /// Update the filtered list based on current query.
    fn update_filter(&mut self) {
        self.filtered.clear();

        if self.query.is_empty() {
            // No query: show all items with default ordering
            self.filtered = (0..self.items.len())
                .map(|i| FuzzyMatch {
                    index: i,
                    score: 0,
                    positions: Vec::new(),
                })
                .collect();
        } else {
            // Fuzzy match against all items
            for (idx, item) in self.items.iter().enumerate() {
                if let Some(mut m) = fuzzy_match(&self.query, item) {
                    m.index = idx;
                    self.filtered.push(m);
                }
            }

            // Sort by score (descending), then by index for stable tie-breaking
            self.filtered.sort_by(|a, b| {
                b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index))
            });
        }

        // Reset selection and scroll
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn ensure_visible(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        if self.selected >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.selected.saturating_sub(self.viewport_height - 1);
        }
    }

    fn select_previous(&mut self) {
        if !self.filtered.is_empty() && self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    fn select_next(&mut self) {
        if !self.filtered.is_empty() && self.selected < self.filtered.len() - 1 {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn select_last(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
            self.ensure_visible();
        }
    }

    fn page_up(&mut self) {
        if self.viewport_height > 0 {
            self.selected = self.selected.saturating_sub(self.viewport_height);
            self.ensure_visible();
        }
    }

    fn page_down(&mut self) {
        if !self.filtered.is_empty() && self.viewport_height > 0 {
            self.selected = (self.selected + self.viewport_height).min(self.filtered.len() - 1);
            self.ensure_visible();
        }
    }

    fn render_search_bar(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.focus == Focus::Search;
        let border_style = if is_focused {
            Style::new().fg(theme::accent::PRIMARY)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(if is_focused { BorderType::Double } else { BorderType::Rounded })
            .title("Search (/ to focus, Esc to clear)")
            .title_alignment(Alignment::Left)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if !inner.is_empty() {
            // Create a focused version of the input for rendering
            let input = TextInput::new()
                .with_value(&self.query)
                .with_placeholder("Type to search...")
                .with_style(Style::new().fg(theme::fg::PRIMARY))
                .with_focused(is_focused);
            input.render(inner, frame);

            // Set cursor if focused
            if is_focused && !inner.is_empty() {
                let cursor_x = inner.x + self.query.len().min(inner.width as usize - 1) as u16;
                frame.set_cursor(Some((cursor_x, inner.y)));
            }
        }
    }

    fn render_list_panel(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.focus == Focus::List;
        let border_style = if is_focused {
            Style::new().fg(theme::screen_accent::PERFORMANCE)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };

        let title = if self.query.is_empty() {
            format!("Items ({} total)", self.items.len())
        } else {
            format!(
                "Results ({} of {} match)",
                self.filtered.len(),
                self.items.len()
            )
        };

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(&title)
            .title_alignment(Alignment::Center)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        // Store viewport height for navigation
        let viewport = inner.height as usize;

        if self.filtered.is_empty() {
            let msg = if self.query.is_empty() {
                "No items"
            } else {
                "No matches found"
            };
            Paragraph::new(msg)
                .style(Style::new().fg(theme::fg::MUTED))
                .render(inner, frame);
            return;
        }

        let end = (self.scroll_offset + viewport).min(self.filtered.len());

        for (row, filter_idx) in (self.scroll_offset..end).enumerate() {
            let y = inner.y + row as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let m = &self.filtered[filter_idx];
            let item_text = &self.items[m.index];
            let is_selected = filter_idx == self.selected;

            let row_area = Rect::new(inner.x, y, inner.width, 1);

            // Render with match highlighting
            self.render_highlighted_row(frame, row_area, item_text, &m.positions, is_selected, m.score);
        }
    }

    fn render_highlighted_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        text: &str,
        positions: &[usize],
        is_selected: bool,
        score: i32,
    ) {
        let base_style = if is_selected {
            Style::new()
                .fg(theme::fg::PRIMARY)
                .bg(theme::alpha::HIGHLIGHT)
        } else {
            Style::new().fg(theme::fg::SECONDARY)
        };

        // If no matches to highlight, just render plain
        if positions.is_empty() {
            Paragraph::new(text)
                .style(base_style)
                .render(area, frame);
            return;
        }

        // Render character by character with highlighting
        let chars: Vec<char> = text.chars().collect();
        let max_x = area.x + area.width;

        for (i, ch) in chars.iter().enumerate() {
            let x = area.x + i as u16;
            if x >= max_x {
                break;
            }

            let style = if positions.contains(&i) {
                // Highlighted match character
                if is_selected {
                    Style::new()
                        .fg(MATCH_HIGHLIGHT)
                        .bg(theme::alpha::HIGHLIGHT)
                        .bold()
                } else {
                    Style::new().fg(MATCH_HIGHLIGHT).bold()
                }
            } else {
                base_style
            };

            frame.buffer.set_char(x, area.y, *ch, style);
        }

        // Show score at end if there's room and we have matches
        if !positions.is_empty() && score != 0 {
            let score_text = format!(" [{}]", score);
            let score_start = area.x + chars.len() as u16;
            if score_start + score_text.len() as u16 <= max_x {
                for (i, ch) in score_text.chars().enumerate() {
                    let x = score_start + i as u16;
                    if x >= max_x {
                        break;
                    }
                    frame.buffer.set_char(
                        x,
                        area.y,
                        ch,
                        Style::new().fg(theme::fg::MUTED),
                    );
                }
            }
        }
    }

    fn render_stats_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Stats")
            .title_alignment(Alignment::Center)
            .style(theme::content_border());

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let top_score = self.filtered.first().map_or(0, |m| m.score);

        let stats = [
            format!("Total:    {} items", self.items.len()),
            format!("Matches:  {}", self.filtered.len()),
            format!("Selected: {}", if self.filtered.is_empty() { 0 } else { self.selected + 1 }),
            format!("Query:    \"{}\"", self.query),
            format!("Top score: {}", top_score),
            String::new(),
            "Keybindings:".into(),
            "  /      Focus search".into(),
            "  Esc    Clear search".into(),
            "  j/k    Navigate".into(),
            "  g/G    Top/Bottom".into(),
            "  PgUp/Dn  Page scroll".into(),
        ];

        for (i, line) in stats.iter().enumerate() {
            if i >= inner.height as usize {
                break;
            }
            let style = if line.is_empty() || line.starts_with(' ') {
                Style::new().fg(theme::fg::MUTED)
            } else if line.starts_with("Keybindings") {
                Style::new().fg(theme::accent::PRIMARY)
            } else {
                Style::new().fg(theme::fg::SECONDARY)
            };
            let row_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
            Paragraph::new(line.as_str())
                .style(style)
                .render(row_area, frame);
        }
    }
}

impl Screen for VirtualizedSearch {
    type Message = ();

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        match event {
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => {
                let shift = modifiers.contains(Modifiers::SHIFT);

                match self.focus {
                    Focus::Search => {
                        match code {
                            KeyCode::Esc => {
                                // Clear search and return to list
                                self.query.clear();
                                self.search_input = TextInput::new()
                                    .with_placeholder("Type to search...")
                                    .with_style(Style::new().fg(theme::fg::PRIMARY))
                                    .with_focused(false);
                                self.focus = Focus::List;
                                self.update_filter();
                            }
                            KeyCode::Enter => {
                                // Return to list with current filter
                                self.focus = Focus::List;
                            }
                            KeyCode::Backspace => {
                                self.query.pop();
                                self.update_filter();
                            }
                            KeyCode::Char(c) => {
                                self.query.push(*c);
                                self.update_filter();
                            }
                            _ => {}
                        }
                    }
                    Focus::List => {
                        match code {
                            KeyCode::Char('/') => {
                                self.focus = Focus::Search;
                            }
                            KeyCode::Esc => {
                                if !self.query.is_empty() {
                                    self.query.clear();
                                    self.update_filter();
                                }
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                self.select_next();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                self.select_previous();
                            }
                            KeyCode::Char('g') if !shift => {
                                self.select_first();
                            }
                            KeyCode::Char('G') | KeyCode::Char('g') if shift => {
                                self.select_last();
                            }
                            KeyCode::Home => {
                                self.select_first();
                            }
                            KeyCode::End => {
                                self.select_last();
                            }
                            KeyCode::PageUp => {
                                self.page_up();
                            }
                            KeyCode::PageDown => {
                                self.page_down();
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        // Layout: search bar (3 rows) + main content
        let [search_area, content_area] = Flex::column()
            .constraints([Constraint::Fixed(3), Constraint::Fill])
            .split(area);

        self.render_search_bar(frame, search_area);

        // Main content: list (70%) + stats (30%)
        let [list_area, stats_area] = Flex::row()
            .constraints([Constraint::Percentage(70.0), Constraint::Percentage(30.0)])
            .split(content_area);

        // Update viewport height for navigation (mutable through interior mutability would be ideal)
        // For now we just use the value from the area
        let vp = list_area.height.saturating_sub(2) as usize; // -2 for borders

        // Note: We can't mutate self.viewport_height here since view takes &self
        // The navigation uses the last known value, which is fine for this demo

        self.render_list_panel(frame, list_area);
        self.render_stats_panel(frame, stats_area);
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    fn title(&self) -> &'static str {
        "Virtualized Search"
    }

    fn tab_label(&self) -> &'static str {
        "VirtSearch"
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "/",
                action: "Focus search input",
            },
            HelpEntry {
                key: "Esc",
                action: "Clear search / unfocus",
            },
            HelpEntry {
                key: "j/↓",
                action: "Next item",
            },
            HelpEntry {
                key: "k/↑",
                action: "Previous item",
            },
            HelpEntry {
                key: "g/G",
                action: "First/Last item",
            },
            HelpEntry {
                key: "PgUp/Dn",
                action: "Page scroll",
            },
        ]
    }
}
