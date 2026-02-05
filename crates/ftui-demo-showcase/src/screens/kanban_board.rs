#![forbid(unsafe_code)]

//! Kanban Board screen — drag-and-drop task management (bd-iuvb.12).
//!
//! Demonstrates a minimal Kanban board with three columns (Todo, In Progress,
//! Done). Cards can be moved between columns using keyboard shortcuts.
//! Keeps a deterministic data set for stable snapshots.

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ftui_core::geometry::Rect;
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

/// Column identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Todo,
    InProgress,
    Done,
}

impl Column {
    fn title(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::Done => "Done",
        }
    }

    fn all() -> [Self; 3] {
        [Self::Todo, Self::InProgress, Self::Done]
    }

    fn index(self) -> usize {
        match self {
            Self::Todo => 0,
            Self::InProgress => 1,
            Self::Done => 2,
        }
    }

    fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Todo,
            1 => Self::InProgress,
            _ => Self::Done,
        }
    }
}

/// A single card on the board.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: u32,
    pub title: String,
    pub tag: &'static str,
}

/// Kanban Board screen state.
pub struct KanbanBoard {
    /// Cards in each column: [Todo, InProgress, Done].
    columns: [Vec<Card>; 3],
    /// Currently focused column index.
    focus_col: usize,
    /// Currently focused card index within the focused column.
    focus_row: usize,
    /// Move history for undo: (card_id, from_col, to_col, from_row, to_row).
    history: Vec<(u32, usize, usize, usize, usize)>,
    /// Redo stack.
    redo_stack: Vec<(u32, usize, usize, usize, usize)>,
    /// Tick counter.
    tick_count: u64,
}

impl Default for KanbanBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl KanbanBoard {
    /// Create a new kanban board with deterministic seed data.
    pub fn new() -> Self {
        let todo = vec![
            Card {
                id: 1,
                title: "Design login page".into(),
                tag: "UI",
            },
            Card {
                id: 2,
                title: "Add input validation".into(),
                tag: "Logic",
            },
            Card {
                id: 3,
                title: "Write unit tests".into(),
                tag: "QA",
            },
            Card {
                id: 4,
                title: "Set up CI pipeline".into(),
                tag: "Ops",
            },
        ];
        let in_progress = vec![
            Card {
                id: 5,
                title: "Build nav component".into(),
                tag: "UI",
            },
            Card {
                id: 6,
                title: "Implement auth flow".into(),
                tag: "Logic",
            },
        ];
        let done = vec![Card {
            id: 7,
            title: "Project scaffolding".into(),
            tag: "Ops",
        }];

        Self {
            columns: [todo, in_progress, done],
            focus_col: 0,
            focus_row: 0,
            history: Vec::new(),
            redo_stack: Vec::new(),
            tick_count: 0,
        }
    }

    /// Number of cards in the focused column.
    fn focused_col_len(&self) -> usize {
        self.columns[self.focus_col].len()
    }

    /// Clamp focus_row to valid range.
    fn clamp_row(&mut self) {
        let len = self.focused_col_len();
        if len == 0 {
            self.focus_row = 0;
        } else if self.focus_row >= len {
            self.focus_row = len - 1;
        }
    }

    /// Move focus to the next column (right).
    fn focus_right(&mut self) {
        if self.focus_col < 2 {
            self.focus_col += 1;
            self.clamp_row();
        }
    }

    /// Move focus to the previous column (left).
    fn focus_left(&mut self) {
        if self.focus_col > 0 {
            self.focus_col -= 1;
            self.clamp_row();
        }
    }

    /// Move focus to the next card (down).
    fn focus_down(&mut self) {
        let len = self.focused_col_len();
        if len > 0 && self.focus_row < len - 1 {
            self.focus_row += 1;
        }
    }

    /// Move focus to the previous card (up).
    fn focus_up(&mut self) {
        if self.focus_row > 0 {
            self.focus_row -= 1;
        }
    }

    /// Move the focused card one column to the right.
    fn move_card_right(&mut self) {
        if self.focus_col >= 2 || self.columns[self.focus_col].is_empty() {
            return;
        }
        let from = self.focus_col;
        let to = self.focus_col + 1;
        let from_row = self.focus_row;
        let card = self.columns[from].remove(from_row);
        let card_id = card.id;
        let to_row = self.columns[to].len();
        self.columns[to].push(card);
        self.history.push((card_id, from, to, from_row, to_row));
        self.redo_stack.clear();
        // Follow the card to the new column
        self.focus_col = to;
        self.focus_row = to_row;
    }

    /// Move the focused card one column to the left.
    fn move_card_left(&mut self) {
        if self.focus_col == 0 || self.columns[self.focus_col].is_empty() {
            return;
        }
        let from = self.focus_col;
        let to = self.focus_col - 1;
        let from_row = self.focus_row;
        let card = self.columns[from].remove(from_row);
        let card_id = card.id;
        let to_row = self.columns[to].len();
        self.columns[to].push(card);
        self.history.push((card_id, from, to, from_row, to_row));
        self.redo_stack.clear();
        // Follow the card to the new column
        self.focus_col = to;
        self.focus_row = to_row;
    }

    /// Render a single column.
    fn render_column(&self, frame: &mut Frame, area: Rect, col_idx: usize) {
        if area.is_empty() {
            return;
        }

        let col = Column::from_index(col_idx);
        let is_focused_col = col_idx == self.focus_col;

        let border_style = if is_focused_col {
            Style::new().fg(theme::accent::INFO)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(if is_focused_col {
                BorderType::Heavy
            } else {
                BorderType::Rounded
            })
            .title(col.title())
            .title_alignment(Alignment::Center)
            .style(border_style.bg(theme::bg::DEEP));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let cards = &self.columns[col_idx];
        if cards.is_empty() {
            let empty_style = Style::new().fg(theme::fg::MUTED);
            Paragraph::new("(empty)")
                .style(empty_style)
                .render(Rect::new(inner.x, inner.y, inner.width, 1), frame);
            return;
        }

        // Each card takes 3 rows (1 title + 1 tag + 1 separator)
        let card_height: u16 = 3;
        for (i, card) in cards.iter().enumerate() {
            let y_offset = i as u16 * card_height;
            if y_offset + 2 > inner.height {
                break;
            }

            let is_selected = is_focused_col && i == self.focus_row;
            let card_area = Rect::new(inner.x, inner.y + y_offset, inner.width, card_height.min(inner.height - y_offset));

            self.render_card(frame, card_area, card, is_selected);
        }
    }

    /// Render a single card within a column.
    fn render_card(&self, frame: &mut Frame, area: Rect, card: &Card, selected: bool) {
        if area.is_empty() {
            return;
        }

        let (title_style, tag_style) = if selected {
            (
                Style::new()
                    .fg(theme::bg::DEEP)
                    .bg(theme::accent::INFO)
                    .bold(),
                Style::new()
                    .fg(theme::bg::DEEP)
                    .bg(theme::accent::INFO),
            )
        } else {
            (
                Style::new().fg(theme::fg::PRIMARY),
                Style::new().fg(theme::fg::MUTED),
            )
        };

        // Title row
        let title_text = if area.width as usize > card.title.len() + 2 {
            if selected {
                format!("> {}", card.title)
            } else {
                format!("  {}", card.title)
            }
        } else {
            card.title.clone()
        };
        Paragraph::new(title_text.as_str())
            .style(title_style)
            .render(Rect::new(area.x, area.y, area.width, 1), frame);

        // Tag row
        if area.height > 1 {
            let tag_text = format!("  [{}]", card.tag);
            Paragraph::new(tag_text.as_str())
                .style(tag_style)
                .render(Rect::new(area.x, area.y + 1, area.width, 1), frame);
        }
    }

    /// Render the instruction footer.
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let move_count = self.history.len();
        let info = format!(
            " h/l: column | j/k: card | H/L: move card | u: undo | r: redo | moves: {}",
            move_count,
        );

        Paragraph::new(info.as_str())
            .style(Style::new().fg(theme::fg::MUTED).bg(theme::bg::DEEP))
            .render(area, frame);
    }
}

impl Screen for KanbanBoard {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match code {
                // Navigation
                KeyCode::Char('h') | KeyCode::Left => self.focus_left(),
                KeyCode::Char('l') | KeyCode::Right => self.focus_right(),
                KeyCode::Char('j') | KeyCode::Down => self.focus_down(),
                KeyCode::Char('k') | KeyCode::Up => self.focus_up(),
                // Move card
                KeyCode::Char('H') => self.move_card_left(),
                KeyCode::Char('L') => self.move_card_right(),
                // Undo
                KeyCode::Char('u') => {
                    self.undo();
                }
                // Redo
                KeyCode::Char('r') => {
                    self.redo();
                }
                _ => {}
            }
        }
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        // Split: main board area + 1-row footer
        let footer_height = 1u16;
        let board_height = area.height.saturating_sub(footer_height);
        if board_height == 0 {
            return;
        }

        let board_area = Rect::new(area.x, area.y, area.width, board_height);
        let footer_area = Rect::new(area.x, area.y + board_height, area.width, footer_height);

        // Split board into 3 equal columns
        let col_chunks = Flex::horizontal()
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(board_area);

        for (i, _col) in Column::all().iter().enumerate() {
            self.render_column(frame, col_chunks[i], i);
        }

        self.render_footer(frame, footer_area);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "h/l",
                action: "Move between columns",
            },
            HelpEntry {
                key: "j/k",
                action: "Move between cards",
            },
            HelpEntry {
                key: "H/L",
                action: "Move card left/right",
            },
            HelpEntry {
                key: "u",
                action: "Undo last move",
            },
            HelpEntry {
                key: "r",
                action: "Redo last undo",
            },
        ]
    }

    fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn undo(&mut self) -> bool {
        if let Some((card_id, from_col, to_col, from_row, _to_row)) = self.history.pop() {
            // Find the card in to_col and move it back to from_col at from_row
            if let Some(pos) = self.columns[to_col].iter().position(|c| c.id == card_id) {
                let card = self.columns[to_col].remove(pos);
                let insert_at = from_row.min(self.columns[from_col].len());
                self.columns[from_col].insert(insert_at, card);
                self.redo_stack
                    .push((card_id, to_col, from_col, pos, insert_at));
                self.focus_col = from_col;
                self.focus_row = insert_at;
                return true;
            }
        }
        false
    }

    fn redo(&mut self) -> bool {
        if let Some((card_id, from_col, to_col, _from_row, _to_row)) = self.redo_stack.pop() {
            if let Some(pos) = self.columns[from_col].iter().position(|c| c.id == card_id) {
                let card = self.columns[from_col].remove(pos);
                let insert_at = self.columns[to_col].len();
                self.columns[to_col].push(card);
                self.history
                    .push((card_id, from_col, to_col, pos, insert_at));
                self.focus_col = to_col;
                self.focus_row = insert_at;
                return true;
            }
        }
        false
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    fn title(&self) -> &'static str {
        "Kanban Board"
    }

    fn tab_label(&self) -> &'static str {
        "Kanban"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_render::grapheme_pool::GraphemePool;

    #[test]
    fn default_board_has_seed_data() {
        let board = KanbanBoard::new();
        assert_eq!(board.columns[0].len(), 4, "Todo should have 4 cards");
        assert_eq!(board.columns[1].len(), 2, "In Progress should have 2 cards");
        assert_eq!(board.columns[2].len(), 1, "Done should have 1 card");
    }

    #[test]
    fn focus_navigation() {
        let mut board = KanbanBoard::new();
        assert_eq!(board.focus_col, 0);
        assert_eq!(board.focus_row, 0);

        board.focus_down();
        assert_eq!(board.focus_row, 1);

        board.focus_down();
        assert_eq!(board.focus_row, 2);

        board.focus_up();
        assert_eq!(board.focus_row, 1);

        board.focus_right();
        assert_eq!(board.focus_col, 1);
        // Row clamps to in-progress column length (2 cards, max row 1)
        assert_eq!(board.focus_row, 1);

        board.focus_left();
        assert_eq!(board.focus_col, 0);
    }

    #[test]
    fn focus_clamping_at_boundaries() {
        let mut board = KanbanBoard::new();

        // Can't go left from column 0
        board.focus_left();
        assert_eq!(board.focus_col, 0);

        // Can't go up from row 0
        board.focus_up();
        assert_eq!(board.focus_row, 0);

        // Go to last column
        board.focus_right();
        board.focus_right();
        assert_eq!(board.focus_col, 2);

        // Can't go right from last column
        board.focus_right();
        assert_eq!(board.focus_col, 2);
    }

    #[test]
    fn move_card_right() {
        let mut board = KanbanBoard::new();
        let card_id = board.columns[0][0].id;

        board.move_card_right();

        // Card moved from Todo to In Progress
        assert_eq!(board.columns[0].len(), 3);
        assert_eq!(board.columns[1].len(), 3);
        assert_eq!(board.columns[1].last().unwrap().id, card_id);
        // Focus follows the card
        assert_eq!(board.focus_col, 1);
        assert_eq!(board.focus_row, 2);
    }

    #[test]
    fn move_card_left() {
        let mut board = KanbanBoard::new();
        // Focus on In Progress column
        board.focus_col = 1;
        board.focus_row = 0;
        let card_id = board.columns[1][0].id;

        board.move_card_left();

        // Card moved from In Progress to Todo
        assert_eq!(board.columns[0].len(), 5);
        assert_eq!(board.columns[1].len(), 1);
        assert_eq!(board.columns[0].last().unwrap().id, card_id);
        assert_eq!(board.focus_col, 0);
    }

    #[test]
    fn move_card_right_from_done_is_noop() {
        let mut board = KanbanBoard::new();
        board.focus_col = 2;
        board.focus_row = 0;

        board.move_card_right();

        // Nothing changes
        assert_eq!(board.columns[2].len(), 1);
        assert_eq!(board.focus_col, 2);
    }

    #[test]
    fn move_card_left_from_todo_is_noop() {
        let mut board = KanbanBoard::new();
        board.focus_col = 0;
        board.focus_row = 0;

        board.move_card_left();

        // Nothing changes
        assert_eq!(board.columns[0].len(), 4);
        assert_eq!(board.focus_col, 0);
    }

    #[test]
    fn undo_redo_cycle() {
        let mut board = KanbanBoard::new();
        let card_id = board.columns[0][0].id;

        // Move card right
        board.move_card_right();
        assert_eq!(board.columns[0].len(), 3);
        assert_eq!(board.columns[1].len(), 3);

        // Undo
        assert!(board.can_undo());
        board.undo();
        assert_eq!(board.columns[0].len(), 4);
        assert_eq!(board.columns[1].len(), 2);
        assert_eq!(board.columns[0][0].id, card_id);

        // Redo
        assert!(board.can_redo());
        board.redo();
        assert_eq!(board.columns[0].len(), 3);
        assert_eq!(board.columns[1].len(), 3);
    }

    #[test]
    fn move_clears_redo_stack() {
        let mut board = KanbanBoard::new();

        board.move_card_right();
        board.undo();
        assert!(board.can_redo());

        // New move should clear redo
        board.move_card_right();
        assert!(!board.can_redo());
    }

    #[test]
    fn key_event_navigation() {
        use super::Screen;
        let mut board = KanbanBoard::new();

        let down = Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        board.update(&down);
        assert_eq!(board.focus_row, 1);

        let right = Event::Key(KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        board.update(&right);
        assert_eq!(board.focus_col, 1);
    }

    #[test]
    fn key_event_move_card() {
        use super::Screen;
        let mut board = KanbanBoard::new();

        let move_right = Event::Key(KeyEvent {
            code: KeyCode::Char('L'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        board.update(&move_right);
        assert_eq!(board.columns[0].len(), 3);
        assert_eq!(board.columns[1].len(), 3);
    }

    #[test]
    fn render_does_not_panic() {
        use super::Screen;
        let board = KanbanBoard::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        let area = Rect::new(0, 0, 80, 24);
        board.view(&mut frame, area);
    }

    #[test]
    fn render_zero_area_does_not_panic() {
        use super::Screen;
        let board = KanbanBoard::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(1, 1, &mut pool);
        let area = Rect::new(0, 0, 0, 0);
        board.view(&mut frame, area);
    }

    #[test]
    fn render_tiny_area_does_not_panic() {
        use super::Screen;
        let board = KanbanBoard::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(10, 5, &mut pool);
        let area = Rect::new(0, 0, 10, 5);
        board.view(&mut frame, area);
    }

    #[test]
    fn keybindings_returns_entries() {
        use super::Screen;
        let board = KanbanBoard::new();
        let bindings = board.keybindings();
        assert_eq!(bindings.len(), 5);
    }

    #[test]
    fn title_and_label() {
        use super::Screen;
        let board = KanbanBoard::new();
        assert_eq!(board.title(), "Kanban Board");
        assert_eq!(board.tab_label(), "Kanban");
    }

    #[test]
    fn move_from_empty_column_is_noop() {
        let mut board = KanbanBoard::new();
        // Clear the done column
        board.columns[2].clear();
        board.focus_col = 2;
        board.focus_row = 0;

        board.move_card_right();
        assert_eq!(board.columns[2].len(), 0);

        board.move_card_left();
        assert_eq!(board.columns[2].len(), 0);
    }

    #[test]
    fn focus_clamps_when_moving_to_shorter_column() {
        let mut board = KanbanBoard::new();
        // Focus on last card in Todo (index 3)
        board.focus_row = 3;
        assert_eq!(board.focus_col, 0);

        // Move to In Progress (only 2 cards, max row 1)
        board.focus_right();
        assert_eq!(board.focus_col, 1);
        assert_eq!(board.focus_row, 1);
    }
}
