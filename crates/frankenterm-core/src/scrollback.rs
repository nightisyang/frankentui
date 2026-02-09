//! Scrollback buffer: lines that have scrolled off the visible viewport.
//!
//! Stores rows as `Vec<Cell>` so that SGR attributes, hyperlinks, and wide-char
//! flags are preserved through scrollback. Uses a `VecDeque` ring for O(1)
//! push/pop at both ends.

use std::collections::VecDeque;
use std::ops::Range;

use crate::cell::Cell;

/// A single line in the scrollback buffer.
///
/// Stores the cells that made up the row when it was evicted from the viewport.
/// The `wrapped` flag records whether the line was a soft-wrap continuation of
/// the previous line (used by reflow on resize).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbackLine {
    /// The cells of this line (may be shorter than the viewport width if
    /// trailing blanks were trimmed).
    pub cells: Vec<Cell>,
    /// Whether this line was a soft-wrap continuation (as opposed to a hard
    /// newline / CR+LF). Used by reflow policies.
    pub wrapped: bool,
}

/// Computed visible/render window over scrollback for virtualized rendering.
///
/// Indexes are in scrollback space (`0 = oldest`, `total_lines = one past newest`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbackWindow {
    /// Total lines currently stored in scrollback.
    pub total_lines: usize,
    /// Maximum legal scroll offset from the newest viewport position.
    pub max_scroll_offset: usize,
    /// Clamped scroll offset from the newest viewport position.
    pub scroll_offset_from_bottom: usize,
    /// Visible viewport start (inclusive).
    pub viewport_start: usize,
    /// Visible viewport end (exclusive).
    pub viewport_end: usize,
    /// Render start including overscan (inclusive).
    pub render_start: usize,
    /// Render end including overscan (exclusive).
    pub render_end: usize,
}

impl ScrollbackWindow {
    /// Visible viewport range.
    #[inline]
    #[must_use]
    pub fn viewport_range(self) -> Range<usize> {
        self.viewport_start..self.viewport_end
    }

    /// Render range including overscan.
    #[inline]
    #[must_use]
    pub fn render_range(self) -> Range<usize> {
        self.render_start..self.render_end
    }

    /// Number of visible viewport lines.
    #[inline]
    #[must_use]
    pub fn viewport_len(self) -> usize {
        self.viewport_end.saturating_sub(self.viewport_start)
    }

    /// Number of lines in the render range (viewport + overscan).
    #[inline]
    #[must_use]
    pub fn render_len(self) -> usize {
        self.render_end.saturating_sub(self.render_start)
    }
}

impl ScrollbackLine {
    /// Create a new scrollback line from a cell slice.
    pub fn new(cells: &[Cell], wrapped: bool) -> Self {
        Self {
            cells: cells.to_vec(),
            wrapped,
        }
    }

    /// Number of cells in this line.
    #[inline]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether this line has zero cells.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Scrollback buffer with configurable line capacity.
///
/// Uses a `VecDeque` for O(1) push/pop. When over capacity, the oldest line
/// (front of the deque) is evicted.
#[derive(Debug, Clone)]
pub struct Scrollback {
    lines: VecDeque<ScrollbackLine>,
    capacity: usize,
}

impl Scrollback {
    /// Create a new scrollback with the given line capacity.
    ///
    /// A capacity of `0` means scrollback is disabled (all pushes are dropped).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity.min(4096)),
            capacity,
        }
    }

    /// Maximum number of lines this scrollback can hold.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Change the scrollback capacity.
    ///
    /// If the new capacity is smaller than the current line count, the oldest
    /// lines are evicted.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        while self.lines.len() > capacity {
            self.lines.pop_front();
        }
    }

    /// Current number of stored lines.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the scrollback is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Push a row (as a cell slice) into scrollback.
    ///
    /// `wrapped` indicates whether the row was a soft-wrap continuation.
    /// If over capacity, the oldest line is evicted.
    pub fn push_row(&mut self, cells: &[Cell], wrapped: bool) -> Option<ScrollbackLine> {
        if self.capacity == 0 {
            return None;
        }
        let evicted = if self.lines.len() == self.capacity {
            self.lines.pop_front()
        } else {
            None
        };
        self.lines.push_back(ScrollbackLine::new(cells, wrapped));
        evicted
    }

    /// Pop the most recent (newest) line from scrollback.
    ///
    /// Used when scrolling down to pull lines back into the viewport, or
    /// when the viewport grows taller and lines are reclaimed.
    pub fn pop_newest(&mut self) -> Option<ScrollbackLine> {
        self.lines.pop_back()
    }

    /// Peek at the most recent (newest) line without removing it.
    #[inline]
    #[must_use]
    pub fn peek_newest(&self) -> Option<&ScrollbackLine> {
        self.lines.back()
    }

    /// Get a line by index (0 = oldest).
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&ScrollbackLine> {
        self.lines.get(index)
    }

    /// Iterate over stored lines from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &ScrollbackLine> {
        self.lines.iter()
    }

    /// Iterate over a specific line range (`0 = oldest`).
    ///
    /// The range is clamped to valid bounds. This enables viewport
    /// virtualization without scanning the full history each frame.
    pub fn iter_range(&self, range: Range<usize>) -> impl Iterator<Item = &ScrollbackLine> {
        let end = range.end.min(self.lines.len());
        let start = range.start.min(end);
        self.lines.range(start..end)
    }

    /// Iterate over stored lines from newest to oldest.
    pub fn iter_rev(&self) -> impl Iterator<Item = &ScrollbackLine> {
        self.lines.iter().rev()
    }

    /// Clear all stored lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Compute a virtualized scrollback window for viewport rendering.
    ///
    /// - `scroll_offset_from_bottom=0` anchors viewport at the newest lines.
    /// - Larger offsets move viewport toward older lines.
    /// - `overscan_lines` expands the render range around the viewport.
    #[must_use]
    pub fn virtualized_window(
        &self,
        scroll_offset_from_bottom: usize,
        viewport_lines: usize,
        overscan_lines: usize,
    ) -> ScrollbackWindow {
        let total_lines = self.lines.len();
        let viewport_len = viewport_lines.min(total_lines);
        let max_scroll_offset = total_lines.saturating_sub(viewport_len);
        let scroll_offset_from_bottom = scroll_offset_from_bottom.min(max_scroll_offset);

        if viewport_len == 0 {
            return ScrollbackWindow {
                total_lines,
                max_scroll_offset,
                scroll_offset_from_bottom,
                viewport_start: total_lines,
                viewport_end: total_lines,
                render_start: total_lines,
                render_end: total_lines,
            };
        }

        let newest_viewport_start = total_lines.saturating_sub(viewport_len);
        let viewport_start = newest_viewport_start.saturating_sub(scroll_offset_from_bottom);
        let viewport_end = viewport_start.saturating_add(viewport_len);
        let render_start = viewport_start.saturating_sub(overscan_lines);
        let render_end = viewport_end.saturating_add(overscan_lines).min(total_lines);

        ScrollbackWindow {
            total_lines,
            max_scroll_offset,
            scroll_offset_from_bottom,
            viewport_start,
            viewport_end,
            render_start,
            render_end,
        }
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{Color, SgrAttrs, SgrFlags};

    fn make_row(text: &str) -> Vec<Cell> {
        text.chars().map(Cell::new).collect()
    }

    fn row_text(cells: &[Cell]) -> String {
        cells.iter().map(|c| c.content()).collect()
    }

    #[test]
    fn capacity_zero_drops_lines() {
        let mut sb = Scrollback::new(0);
        let _ = sb.push_row(&make_row("hello"), false);
        assert!(sb.is_empty());
    }

    #[test]
    fn push_and_retrieve() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("first"), false);
        let _ = sb.push_row(&make_row("second"), true);
        assert_eq!(sb.len(), 2);

        let line0 = sb.get(0).unwrap();
        assert_eq!(row_text(&line0.cells), "first");
        assert!(!line0.wrapped);

        let line1 = sb.get(1).unwrap();
        assert_eq!(row_text(&line1.cells), "second");
        assert!(line1.wrapped);
    }

    #[test]
    fn bounded_capacity_evicts_oldest() {
        let mut sb = Scrollback::new(2);
        let _ = sb.push_row(&make_row("a"), false);
        let _ = sb.push_row(&make_row("b"), false);
        let _ = sb.push_row(&make_row("c"), false);
        assert_eq!(sb.len(), 2);
        assert_eq!(row_text(&sb.get(0).unwrap().cells), "b");
        assert_eq!(row_text(&sb.get(1).unwrap().cells), "c");
    }

    #[test]
    fn pop_newest_returns_most_recent() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("old"), false);
        let _ = sb.push_row(&make_row("new"), false);
        let popped = sb.pop_newest().unwrap();
        assert_eq!(row_text(&popped.cells), "new");
        assert_eq!(sb.len(), 1);
    }

    #[test]
    fn pop_newest_empty_returns_none() {
        let mut sb = Scrollback::new(10);
        assert!(sb.pop_newest().is_none());
    }

    #[test]
    fn peek_newest() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("line"), false);
        assert_eq!(row_text(&sb.peek_newest().unwrap().cells), "line");
        assert_eq!(sb.len(), 1); // not consumed
    }

    #[test]
    fn set_capacity_evicts_excess() {
        let mut sb = Scrollback::new(10);
        for i in 0..5 {
            let _ = sb.push_row(&make_row(&format!("line{i}")), false);
        }
        sb.set_capacity(2);
        assert_eq!(sb.len(), 2);
        assert_eq!(row_text(&sb.get(0).unwrap().cells), "line3");
        assert_eq!(row_text(&sb.get(1).unwrap().cells), "line4");
    }

    #[test]
    fn iter_oldest_to_newest() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("a"), false);
        let _ = sb.push_row(&make_row("b"), false);
        let _ = sb.push_row(&make_row("c"), false);
        let texts: Vec<String> = sb.iter().map(|l| row_text(&l.cells)).collect();
        assert_eq!(texts, vec!["a", "b", "c"]);
    }

    #[test]
    fn iter_rev_newest_to_oldest() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("a"), false);
        let _ = sb.push_row(&make_row("b"), false);
        let texts: Vec<String> = sb.iter_rev().map(|l| row_text(&l.cells)).collect();
        assert_eq!(texts, vec!["b", "a"]);
    }

    #[test]
    fn iter_range_is_clamped_and_ordered() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("a"), false);
        let _ = sb.push_row(&make_row("b"), false);
        let _ = sb.push_row(&make_row("c"), false);
        let _ = sb.push_row(&make_row("d"), false);

        let texts: Vec<String> = sb.iter_range(1..3).map(|l| row_text(&l.cells)).collect();
        assert_eq!(texts, vec!["b", "c"]);

        let clamped: Vec<String> = sb.iter_range(3..99).map(|l| row_text(&l.cells)).collect();
        assert_eq!(clamped, vec!["d"]);
    }

    #[test]
    fn virtualized_window_from_bottom_with_overscan() {
        let mut sb = Scrollback::new(32);
        for i in 0..10 {
            let _ = sb.push_row(&make_row(&format!("{i}")), false);
        }

        let window = sb.virtualized_window(0, 4, 1);
        assert_eq!(window.total_lines, 10);
        assert_eq!(window.max_scroll_offset, 6);
        assert_eq!(window.viewport_range(), 6..10);
        assert_eq!(window.render_range(), 5..10);
        assert_eq!(window.viewport_len(), 4);
        assert_eq!(window.render_len(), 5);
    }

    #[test]
    fn virtualized_window_clamps_large_scroll_offset() {
        let mut sb = Scrollback::new(32);
        for i in 0..10 {
            let _ = sb.push_row(&make_row(&format!("{i}")), false);
        }

        let window = sb.virtualized_window(999, 4, 2);
        assert_eq!(window.scroll_offset_from_bottom, 6);
        assert_eq!(window.viewport_range(), 0..4);
        assert_eq!(window.render_range(), 0..6);
    }

    #[test]
    fn virtualized_window_handles_small_history() {
        let mut sb = Scrollback::new(8);
        let _ = sb.push_row(&make_row("x"), false);
        let _ = sb.push_row(&make_row("y"), false);

        let window = sb.virtualized_window(3, 10, 5);
        assert_eq!(window.max_scroll_offset, 0);
        assert_eq!(window.viewport_range(), 0..2);
        assert_eq!(window.render_range(), 0..2);
    }

    #[test]
    fn clear_empties_buffer() {
        let mut sb = Scrollback::new(10);
        let _ = sb.push_row(&make_row("x"), false);
        sb.clear();
        assert!(sb.is_empty());
    }

    #[test]
    fn preserves_cell_attributes() {
        let mut sb = Scrollback::new(10);
        let mut cells = make_row("AB");
        cells[0].attrs = SgrAttrs {
            flags: SgrFlags::BOLD,
            fg: Color::Rgb(255, 0, 0),
            bg: Color::Default,
            underline_color: None,
        };
        cells[1].hyperlink = 42;
        let _ = sb.push_row(&cells, false);

        let stored = sb.get(0).unwrap();
        assert!(stored.cells[0].attrs.flags.contains(SgrFlags::BOLD));
        assert_eq!(stored.cells[0].attrs.fg, Color::Rgb(255, 0, 0));
        assert_eq!(stored.cells[1].hyperlink, 42);
    }

    #[test]
    fn scrollback_line_len_and_empty() {
        let line = ScrollbackLine::new(&make_row("abc"), false);
        assert_eq!(line.len(), 3);
        assert!(!line.is_empty());

        let empty = ScrollbackLine::new(&[], false);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}
