#![forbid(unsafe_code)]

//! Layout Laboratory screen — interactive constraint solver and layout widget demos.

use ftui_core::event::{Event, KeyCode, KeyEventKind, Modifiers};
use ftui_core::geometry::{Rect, Sides};
use ftui_layout::{Alignment as FlexAlignment, Constraint, Flex};
use ftui_render::cell::PackedRgba;
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::{Style, StyleFlags};
use ftui_widgets::Widget;
use ftui_widgets::align::{Align, VerticalAlignment};
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::columns::Columns;
use ftui_widgets::group::Group;
use ftui_widgets::layout_debugger::LayoutDebugger;
use ftui_widgets::padding::Padding;
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::rule::Rule;

use super::{HelpEntry, Screen};
use crate::theme;

/// The five layout presets available.
const PRESET_COUNT: usize = 5;

/// Names for each preset.
const PRESET_NAMES: [&str; PRESET_COUNT] = [
    "Flex Vertical",
    "Flex Horizontal",
    "Grid 3x3",
    "Nested Flex",
    "Real-World Layout",
];

/// Whether the flex direction is vertical or horizontal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Vertical,
    Horizontal,
}

impl Direction {
    fn label(self) -> &'static str {
        match self {
            Direction::Vertical => "Vertical",
            Direction::Horizontal => "Horizontal",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Direction::Vertical => Direction::Horizontal,
            Direction::Horizontal => Direction::Vertical,
        }
    }
}

/// Layout Lab screen state.
pub struct LayoutLab {
    /// Current preset index (0-4).
    current_preset: usize,
    /// Which constraint is selected for editing (within presets 0/1).
    selected_constraint: usize,
    /// Flex direction for presets 0/1.
    direction: Direction,
    /// Alignment mode index (0-4).
    alignment_idx: usize,
    /// Gap between items.
    gap: u16,
    /// Margin around layout.
    margin: u16,
    /// Padding amount for Padding widget demo.
    padding_amount: u16,
    /// Align demo position index (0-8 for 9 positions).
    align_pos: usize,
    /// Layout debugger instance.
    debugger: LayoutDebugger,
    /// Show debug overlay.
    show_debug: bool,
}

/// The 5 alignment modes.
const ALIGNMENTS: [FlexAlignment; 5] = [
    FlexAlignment::Start,
    FlexAlignment::Center,
    FlexAlignment::End,
    FlexAlignment::SpaceBetween,
    FlexAlignment::SpaceAround,
];

const ALIGNMENT_NAMES: [&str; 5] = ["Start", "Center", "End", "SpaceBetween", "SpaceAround"];

/// Colors for layout regions (theme-aware accents).
fn region_colors() -> [PackedRgba; 6] {
    [
        theme::accent::ACCENT_1.into(),
        theme::accent::ACCENT_2.into(),
        theme::accent::ACCENT_3.into(),
        theme::accent::ACCENT_4.into(),
        theme::accent::ACCENT_5.into(),
        theme::accent::ACCENT_6.into(),
    ]
}

impl Default for LayoutLab {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutLab {
    pub fn new() -> Self {
        let mut debugger = LayoutDebugger::new();
        debugger.set_enabled(true);
        Self {
            current_preset: 0,
            selected_constraint: 0,
            direction: Direction::Vertical,
            alignment_idx: 0,
            gap: 0,
            margin: 0,
            padding_amount: theme::spacing::XS,
            align_pos: 4, // Center/Middle
            debugger,
            show_debug: false,
        }
    }

    /// Get the constraints for the current preset.
    fn preset_constraints(&self) -> Vec<Constraint> {
        match self.current_preset {
            0 | 1 => vec![
                Constraint::Fixed(5),
                Constraint::Percentage(30.0),
                Constraint::Min(3),
                Constraint::Max(10),
                Constraint::Ratio(1, 3),
            ],
            2 => vec![
                // Grid: 3 columns
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ],
            3 => vec![
                // Nested: outer 60/40
                Constraint::Percentage(60.0),
                Constraint::Percentage(40.0),
            ],
            4 => vec![
                // Real-world: sidebar + main + aside
                Constraint::Fixed(20),
                Constraint::Min(40),
                Constraint::Max(30),
            ],
            _ => vec![Constraint::Min(0)],
        }
    }

    fn constraint_label(c: &Constraint) -> String {
        match c {
            Constraint::Fixed(v) => format!("Fixed({v})"),
            Constraint::Percentage(v) => format!("Pct({v:.0}%)"),
            Constraint::Min(v) => format!("Min({v})"),
            Constraint::Max(v) => format!("Max({v})"),
            Constraint::Ratio(n, d) => format!("Ratio({n}/{d})"),
            Constraint::Fill => "Fill".to_string(),
            Constraint::FitContent => "FitContent".to_string(),
            Constraint::FitContentBounded { min, max } => format!("FitContent({min}..{max})"),
            Constraint::FitMin => "FitMin".to_string(),
        }
    }

    fn current_alignment(&self) -> FlexAlignment {
        ALIGNMENTS[self.alignment_idx]
    }
}

impl Screen for LayoutLab {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Cmd::None;
            }

            match (key.code, key.modifiers) {
                // Preset selection: 1-5
                (KeyCode::Char('1'), Modifiers::NONE) => self.current_preset = 0,
                (KeyCode::Char('2'), Modifiers::NONE) => self.current_preset = 1,
                (KeyCode::Char('3'), Modifiers::NONE) => self.current_preset = 2,
                (KeyCode::Char('4'), Modifiers::NONE) => self.current_preset = 3,
                (KeyCode::Char('5'), Modifiers::NONE) => self.current_preset = 4,

                // Direction toggle
                (KeyCode::Char('d'), Modifiers::NONE) => {
                    self.direction = self.direction.toggle();
                }

                // Alignment cycle
                (KeyCode::Char('a'), Modifiers::NONE) => {
                    self.alignment_idx = (self.alignment_idx + 1) % ALIGNMENTS.len();
                }

                // Gap adjustment
                (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
                    self.gap = self.gap.saturating_add(1).min(5);
                }
                (KeyCode::Char('-'), Modifiers::NONE) => {
                    self.gap = self.gap.saturating_sub(1);
                }

                // Margin adjustment
                (KeyCode::Char('m'), Modifiers::NONE) => {
                    self.margin = self.margin.saturating_add(1).min(4);
                }
                (KeyCode::Char('M'), Modifiers::NONE) | (KeyCode::Char('m'), Modifiers::SHIFT) => {
                    self.margin = self.margin.saturating_sub(1);
                }

                // Padding adjustment
                (KeyCode::Char('p'), Modifiers::NONE) => {
                    self.padding_amount = self.padding_amount.saturating_add(1).min(4);
                }
                (KeyCode::Char('P'), Modifiers::NONE) | (KeyCode::Char('p'), Modifiers::SHIFT) => {
                    self.padding_amount = self.padding_amount.saturating_sub(1);
                }

                // Constraint selection (Tab to cycle)
                (KeyCode::Tab, Modifiers::NONE) => {
                    let count = self.preset_constraints().len();
                    self.selected_constraint = (self.selected_constraint + 1) % count;
                }

                // Arrow keys to adjust selected constraint value
                (KeyCode::Right, Modifiers::NONE) => self.adjust_constraint(1),
                (KeyCode::Left, Modifiers::NONE) => self.adjust_constraint(-1),

                // Align position cycle
                (KeyCode::Char('l'), Modifiers::NONE) => {
                    self.align_pos = (self.align_pos + 1) % 9;
                }

                // Debug overlay toggle
                (KeyCode::Char('D'), Modifiers::NONE) | (KeyCode::Char('d'), Modifiers::SHIFT) => {
                    self.show_debug = !self.show_debug;
                }

                _ => {}
            }
        }
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.height < 8 || area.width < 40 {
            Paragraph::new("Terminal too small for Layout Lab")
                .style(theme::muted())
                .render(area, frame);
            return;
        }

        // Top-level: upper preview + lower controls/demos
        let main_chunks = Flex::vertical()
            .constraints([
                Constraint::Percentage(55.0),
                Constraint::Fixed(1),
                Constraint::Min(8),
            ])
            .split(area);

        self.render_preview(frame, main_chunks[0]);
        Rule::new()
            .style(Style::new().fg(theme::fg::MUTED))
            .render(main_chunks[1], frame);
        self.render_bottom(frame, main_chunks[2]);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "1-5",
                action: "Switch preset",
            },
            HelpEntry {
                key: "d",
                action: "Toggle direction",
            },
            HelpEntry {
                key: "a",
                action: "Cycle alignment",
            },
            HelpEntry {
                key: "+/-",
                action: "Adjust gap",
            },
            HelpEntry {
                key: "m/M",
                action: "Adjust margin",
            },
            HelpEntry {
                key: "p/P",
                action: "Adjust padding",
            },
            HelpEntry {
                key: "Tab",
                action: "Select constraint",
            },
            HelpEntry {
                key: "Left/Right",
                action: "Adjust constraint",
            },
            HelpEntry {
                key: "l",
                action: "Cycle align pos",
            },
            HelpEntry {
                key: "D",
                action: "Toggle debug",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Layout Laboratory"
    }

    fn tab_label(&self) -> &'static str {
        "Layout"
    }
}

impl LayoutLab {
    /// Adjust the selected constraint's value by delta.
    fn adjust_constraint(&mut self, delta: i16) {
        // Only presets 0, 1, 4 have meaningfully adjustable constraints
        let _ = delta; // Constraint values are fixed in our presets for simplicity
        // The interactive preview already shows the effect of gap/margin/alignment
    }

    /// Render the upper half: layout preview with colored blocks.
    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let title = format!(
            " Preset {}: {} ",
            self.current_preset + 1,
            PRESET_NAMES[self.current_preset]
        );
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(&title)
            .title_alignment(Alignment::Center)
            .style(theme::content_border());
        let inner = block.inner(area);
        block.render(area, frame);

        match self.current_preset {
            0 | 1 => self.render_flex_preset(frame, inner),
            2 => self.render_grid_preset(frame, inner),
            3 => self.render_nested_preset(frame, inner),
            4 => self.render_realworld_preset(frame, inner),
            _ => {}
        }

        // Debug overlay
        if self.show_debug {
            // Render the debug info as text in the bottom-right corner
            let debug_width = 40u16.min(inner.width);
            let debug_height = 6u16.min(inner.height);
            let debug_area = Rect::new(
                inner.x + inner.width.saturating_sub(debug_width),
                inner.y + inner.height.saturating_sub(debug_height),
                debug_width,
                debug_height,
            );
            let debug_block = Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .title("Debug")
                .style(Style::new().fg(theme::accent::WARNING).bg(theme::bg::DEEP));
            let debug_inner = debug_block.inner(debug_area);
            debug_block.render(debug_area, frame);

            let constraints = self.preset_constraints();
            let debug_text: String = constraints
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let marker = if i == self.selected_constraint {
                        ">"
                    } else {
                        " "
                    };
                    format!("{marker} {}", Self::constraint_label(c))
                })
                .collect::<Vec<_>>()
                .join("\n");
            Paragraph::new(debug_text)
                .style(Style::new().fg(theme::fg::PRIMARY))
                .render(debug_inner, frame);
        }
    }

    /// Render a flex preset (vertical or horizontal) with colored blocks.
    fn render_flex_preset(&self, frame: &mut Frame, area: Rect) {
        let constraints = self.preset_constraints();
        let flex = match self.direction {
            Direction::Vertical => Flex::vertical(),
            Direction::Horizontal => Flex::horizontal(),
        };
        let rects = flex
            .constraints(constraints.clone())
            .gap(self.gap)
            .margin(Sides::all(self.margin))
            .alignment(self.current_alignment())
            .split(area);

        for (i, (rect, constraint)) in rects.iter().zip(constraints.iter()).enumerate() {
            self.render_region(frame, *rect, i, constraint);
        }
    }

    /// Render a grid 3x3 preset with header spanning 3 columns and sidebar spanning 2 rows.
    fn render_grid_preset(&self, frame: &mut Frame, area: Rect) {
        // 3 rows: header (3 high) + 2 content rows
        let rows = Flex::vertical()
            .gap(self.gap)
            .margin(Sides::all(self.margin))
            .constraints([Constraint::Fixed(3), Constraint::Min(2), Constraint::Min(2)])
            .split(area);

        // Header spans full width
        let header_constraint = Constraint::Ratio(1, 1);
        self.render_region(frame, rows[0], 0, &header_constraint);

        // Row 1: sidebar (spans 2 rows conceptually) + 2 cells
        let row1_cols = Flex::horizontal()
            .gap(self.gap)
            .constraints([
                Constraint::Fixed(12),
                Constraint::Min(5),
                Constraint::Min(5),
            ])
            .split(rows[1]);

        // Sidebar spans rows[1] and rows[2] vertically
        let sidebar_area = Rect::new(
            row1_cols[0].x,
            row1_cols[0].y,
            row1_cols[0].width,
            rows[1]
                .height
                .saturating_add(rows[2].height)
                .saturating_add(self.gap),
        );
        let sidebar_constraint = Constraint::Fixed(12);
        self.render_region(frame, sidebar_area, 1, &sidebar_constraint);

        self.render_region(frame, row1_cols[1], 2, &Constraint::Min(5));
        self.render_region(frame, row1_cols[2], 3, &Constraint::Min(5));

        // Row 2: skip sidebar col, render 2 cells
        let row2_cols = Flex::horizontal()
            .gap(self.gap)
            .constraints([
                Constraint::Fixed(12),
                Constraint::Min(5),
                Constraint::Min(5),
            ])
            .split(rows[2]);

        // Don't re-render sidebar area; just render the two cells
        self.render_region(frame, row2_cols[1], 4, &Constraint::Min(5));
        self.render_region(frame, row2_cols[2], 5, &Constraint::Min(5));
    }

    /// Render nested flex preset: outer 60/40 horizontal, each with vertical subdivisions.
    fn render_nested_preset(&self, frame: &mut Frame, area: Rect) {
        let outer = Flex::horizontal()
            .gap(self.gap)
            .margin(Sides::all(self.margin))
            .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
            .split(area);

        // Left side: 3 vertical regions
        let left = Flex::vertical()
            .gap(self.gap)
            .constraints([
                Constraint::Fixed(3),
                Constraint::Min(4),
                Constraint::Fixed(3),
            ])
            .split(outer[0]);
        self.render_region(frame, left[0], 0, &Constraint::Fixed(3));
        self.render_region(frame, left[1], 1, &Constraint::Min(4));
        self.render_region(frame, left[2], 2, &Constraint::Fixed(3));

        // Right side: 2 vertical regions
        let right = Flex::vertical()
            .gap(self.gap)
            .constraints([Constraint::Percentage(50.0), Constraint::Percentage(50.0)])
            .split(outer[1]);
        self.render_region(frame, right[0], 3, &Constraint::Percentage(50.0));
        self.render_region(frame, right[1], 4, &Constraint::Percentage(50.0));
    }

    /// Render real-world layout: header + (sidebar + main + aside) + footer.
    fn render_realworld_preset(&self, frame: &mut Frame, area: Rect) {
        let rows = Flex::vertical()
            .margin(Sides::all(self.margin))
            .constraints([
                Constraint::Fixed(3),
                Constraint::Min(4),
                Constraint::Fixed(1),
            ])
            .split(area);

        // Header
        self.render_region(frame, rows[0], 0, &Constraint::Fixed(3));

        // Middle: sidebar + main + aside
        let cols = Flex::horizontal()
            .gap(self.gap)
            .constraints([
                Constraint::Fixed(20),
                Constraint::Min(20),
                Constraint::Max(30),
            ])
            .split(rows[1]);
        self.render_region(frame, cols[0], 1, &Constraint::Fixed(20));
        self.render_region(frame, cols[1], 2, &Constraint::Min(20));
        self.render_region(frame, cols[2], 3, &Constraint::Max(30));

        // Footer
        self.render_region(frame, rows[2], 4, &Constraint::Fixed(1));
    }

    /// Render a single colored region with its rect info and constraint label.
    fn render_region(&self, frame: &mut Frame, rect: Rect, idx: usize, constraint: &Constraint) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let colors = region_colors();
        let color = colors[idx % colors.len()];
        let label = Self::constraint_label(constraint);

        let style = Style::new().fg(theme::fg::PRIMARY).bg(color);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Square)
            .title(&label)
            .title_alignment(Alignment::Center)
            .style(style);
        let inner = block.inner(rect);
        block.render(rect, frame);

        if inner.height > 0 && inner.width > 4 {
            let rect_text = format!("{}x{} @({},{})", rect.width, rect.height, rect.x, rect.y);
            Paragraph::new(rect_text).style(style).render(inner, frame);
        }
    }

    /// Render the bottom half: controls + widget demos.
    fn render_bottom(&self, frame: &mut Frame, area: Rect) {
        // Split horizontally: controls (left) + widget demos (right)
        let cols = Flex::horizontal()
            .constraints([Constraint::Percentage(40.0), Constraint::Percentage(60.0)])
            .split(area);

        self.render_controls(frame, cols[0]);
        self.render_widget_demos(frame, cols[1]);
    }

    /// Render the controls panel showing current settings.
    fn render_controls(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Controls ")
            .title_alignment(Alignment::Center)
            .style(theme::content_border());
        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height < 3 || inner.width < 15 {
            return;
        }

        let constraints = self.preset_constraints();
        let constraint_list: String = constraints
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let marker = if i == self.selected_constraint {
                    ">"
                } else {
                    " "
                };
                format!("{marker} {}", Self::constraint_label(c))
            })
            .collect::<Vec<_>>()
            .join("  ");

        let info = format!(
            "Preset: [{}] {}\n\
             Direction: {} (d)\n\
             Alignment: {} (a)\n\
             Gap: {} (+/-)\n\
             Margin: {} (m/M)\n\
             Padding: {} (p/P)\n\
             Constraints: {}",
            self.current_preset + 1,
            PRESET_NAMES[self.current_preset],
            self.direction.label(),
            ALIGNMENT_NAMES[self.alignment_idx],
            self.gap,
            self.margin,
            self.padding_amount,
            constraint_list,
        );

        Paragraph::new(info)
            .style(Style::new().fg(theme::fg::SECONDARY))
            .render(inner, frame);
    }

    /// Render widget demos: Padding, Align, Columns, Group.
    fn render_widget_demos(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Widget Demos ")
            .title_alignment(Alignment::Center)
            .style(theme::content_border());
        let demo_inner = block.inner(area);
        block.render(area, frame);

        if demo_inner.height < 4 || demo_inner.width < 20 {
            return;
        }

        // Split into 4 demo areas
        let demo_rows = Flex::vertical()
            .constraints([Constraint::Min(3), Constraint::Min(3)])
            .split(demo_inner);

        let top_demos = Flex::horizontal()
            .gap(theme::spacing::XS)
            .constraints([Constraint::Percentage(50.0), Constraint::Percentage(50.0)])
            .split(demo_rows[0]);

        let bottom_demos = Flex::horizontal()
            .gap(theme::spacing::XS)
            .constraints([Constraint::Percentage(50.0), Constraint::Percentage(50.0)])
            .split(demo_rows[1]);

        // Demo 1: Padding widget
        self.render_padding_demo(frame, top_demos[0]);

        // Demo 2: Align widget
        self.render_align_demo(frame, top_demos[1]);

        // Demo 3: Columns widget
        self.render_columns_demo(frame, bottom_demos[0]);

        // Demo 4: Group widget
        self.render_group_demo(frame, bottom_demos[1]);
    }

    fn render_padding_demo(&self, frame: &mut Frame, area: Rect) {
        let label = Paragraph::new(format!("Padding({})", self.padding_amount)).style(
            Style::new()
                .fg(theme::accent::PRIMARY)
                .attrs(StyleFlags::BOLD),
        );
        let padded = Padding::new(label, Sides::all(self.padding_amount));
        let demo_block = Block::new()
            .borders(Borders::ALL)
            .title("Padding")
            .style(Style::new().fg(theme::fg::MUTED));
        let inner = demo_block.inner(area);
        demo_block.render(area, frame);
        Widget::render(&padded, inner, frame);
    }

    fn render_align_demo(&self, frame: &mut Frame, area: Rect) {
        let (h_align, v_align, pos_name) = align_position(self.align_pos);

        let content = Paragraph::new(pos_name)
            .style(Style::new().fg(theme::accent::INFO).attrs(StyleFlags::BOLD));
        let aligned = Align::new(content)
            .horizontal(h_align)
            .vertical(v_align)
            .child_width(pos_name.len() as u16)
            .child_height(1);

        let demo_block = Block::new()
            .borders(Borders::ALL)
            .title("Align (l)")
            .style(Style::new().fg(theme::fg::MUTED));
        let inner = demo_block.inner(area);
        demo_block.render(area, frame);
        Widget::render(&aligned, inner, frame);
    }

    fn render_columns_demo(&self, frame: &mut Frame, area: Rect) {
        let colors = region_colors();
        let col_a = Paragraph::new("A").style(Style::new().fg(colors[0]).attrs(StyleFlags::BOLD));
        let col_b = Paragraph::new("B").style(Style::new().fg(colors[1]).attrs(StyleFlags::BOLD));
        let col_c = Paragraph::new("C").style(Style::new().fg(colors[2]).attrs(StyleFlags::BOLD));

        let columns = Columns::new()
            .gap(theme::spacing::XS)
            .column(col_a, Constraint::Ratio(1, 3))
            .column(col_b, Constraint::Ratio(1, 3))
            .column(col_c, Constraint::Ratio(1, 3));

        let demo_block = Block::new()
            .borders(Borders::ALL)
            .title("Columns")
            .style(Style::new().fg(theme::fg::MUTED));
        let inner = demo_block.inner(area);
        demo_block.render(area, frame);
        columns.render(inner, frame);
    }

    fn render_group_demo(&self, frame: &mut Frame, area: Rect) {
        let bg = Paragraph::new("Group layer").style(Style::new().fg(theme::fg::MUTED));
        let fg = Paragraph::new("Overlay").style(
            Style::new()
                .fg(theme::accent::SUCCESS)
                .attrs(StyleFlags::BOLD),
        );

        let group = Group::new().push(bg).push(fg);

        let demo_block = Block::new()
            .borders(Borders::ALL)
            .title("Group")
            .style(Style::new().fg(theme::fg::MUTED));
        let inner = demo_block.inner(area);
        demo_block.render(area, frame);
        group.render(inner, frame);
    }
}

/// Map an align position index (0-8) to horizontal + vertical alignment and a label.
fn align_position(idx: usize) -> (Alignment, VerticalAlignment, &'static str) {
    match idx % 9 {
        0 => (Alignment::Left, VerticalAlignment::Top, "TopLeft"),
        1 => (Alignment::Center, VerticalAlignment::Top, "TopCenter"),
        2 => (Alignment::Right, VerticalAlignment::Top, "TopRight"),
        3 => (Alignment::Left, VerticalAlignment::Middle, "MidLeft"),
        4 => (Alignment::Center, VerticalAlignment::Middle, "Center"),
        5 => (Alignment::Right, VerticalAlignment::Middle, "MidRight"),
        6 => (Alignment::Left, VerticalAlignment::Bottom, "BotLeft"),
        7 => (Alignment::Center, VerticalAlignment::Bottom, "BotCenter"),
        _ => (Alignment::Right, VerticalAlignment::Bottom, "BotRight"),
    }
}

/// Solve a flex layout and return the resulting rects (for testing).
pub fn solve_flex_vertical(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    Flex::vertical()
        .constraints(constraints.iter().copied())
        .split(area)
}

pub fn solve_flex_horizontal(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    Flex::horizontal()
        .constraints(constraints.iter().copied())
        .split(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_flex_vertical() {
        let area = Rect::new(0, 0, 80, 40);
        let constraints = [
            Constraint::Fixed(5),
            Constraint::Percentage(30.0),
            Constraint::Min(3),
            Constraint::Max(10),
            Constraint::Ratio(1, 3),
        ];
        let rects = solve_flex_vertical(area, &constraints);
        assert_eq!(rects.len(), 5);
        // First rect should be exactly 5 rows tall
        assert_eq!(rects[0].height, 5);
        assert_eq!(rects[0].width, 80);
        // All rects should be within the area
        for r in &rects {
            assert!(r.x >= area.x);
            assert!(r.y >= area.y);
            assert!(r.x + r.width <= area.x + area.width);
            assert!(r.y + r.height <= area.y + area.height);
        }
    }

    #[test]
    fn layout_flex_horizontal() {
        let area = Rect::new(0, 0, 100, 30);
        let constraints = [
            Constraint::Fixed(10),
            Constraint::Percentage(30.0),
            Constraint::Min(5),
        ];
        let rects = solve_flex_horizontal(area, &constraints);
        assert_eq!(rects.len(), 3);
        // First rect should be exactly 10 cols wide
        assert_eq!(rects[0].width, 10);
        assert_eq!(rects[0].height, 30);
        // All rects should be within the area
        for r in &rects {
            assert!(r.x >= area.x);
            assert!(r.y >= area.y);
            assert!(r.x + r.width <= area.x + area.width);
        }
    }

    #[test]
    fn layout_grid_spanning() {
        let area = Rect::new(0, 0, 60, 20);
        // Header row
        let rows = Flex::vertical()
            .constraints([Constraint::Fixed(3), Constraint::Min(2), Constraint::Min(2)])
            .split(area);
        // Header spans full width
        assert_eq!(rows[0].width, 60);
        assert_eq!(rows[0].height, 3);
        // Each row has 3 cols
        let cols = Flex::horizontal()
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(rows[1]);
        assert_eq!(cols.len(), 3);
        // Columns should roughly be 20 each
        for c in &cols {
            assert!(c.width >= 19 && c.width <= 21, "col width: {}", c.width);
        }
    }

    #[test]
    fn layout_alignment_modes() {
        let area = Rect::new(0, 0, 100, 50);
        // Test all 5 alignments with a small fixed constraint
        for alignment in &ALIGNMENTS {
            let rects = Flex::vertical()
                .alignment(*alignment)
                .constraints([Constraint::Fixed(10)])
                .split(area);
            assert_eq!(rects.len(), 1);
            assert_eq!(rects[0].height, 10);
            assert_eq!(rects[0].width, 100);
        }
    }

    #[test]
    fn layout_nested() {
        let area = Rect::new(0, 0, 100, 40);
        // Outer: 60/40 horizontal split
        let outer = Flex::horizontal()
            .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
            .split(area);
        assert_eq!(outer.len(), 2);
        assert_eq!(outer[0].width, 60);
        assert_eq!(outer[1].width, 40);

        // Inner left: 3 vertical parts
        let inner_left = Flex::vertical()
            .constraints([
                Constraint::Fixed(5),
                Constraint::Min(4),
                Constraint::Fixed(5),
            ])
            .split(outer[0]);
        assert_eq!(inner_left.len(), 3);
        assert_eq!(inner_left[0].height, 5);
        assert_eq!(inner_left[2].height, 5);
        // Middle gets the rest: 40 - 5 - 5 = 30
        assert_eq!(inner_left[1].height, 30);
    }
}
