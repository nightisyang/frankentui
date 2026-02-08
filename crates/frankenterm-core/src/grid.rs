//! Terminal grid: 2D cell matrix representing the visible viewport.
//!
//! The grid is the primary data model for the terminal. It owns a flat vector
//! of cells indexed by `(row, col)` and provides methods for the operations
//! that the VT parser dispatches (print, erase, scroll, resize).

use crate::cell::{Cell, Color};

/// 2D terminal cell grid.
///
/// Cells are stored in row-major order in a flat `Vec<Cell>`.
/// The grid does not own scrollback — see [`Scrollback`](crate::Scrollback).
#[derive(Debug, Clone)]
pub struct Grid {
    cells: Vec<Cell>,
    cols: u16,
    rows: u16,
}

impl Grid {
    /// Create a new grid filled with default (blank) cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        let len = (cols as usize) * (rows as usize);
        Self {
            cells: vec![Cell::default(); len],
            cols,
            rows,
        }
    }

    /// Number of columns.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Number of rows.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Get a reference to the cell at `(row, col)`.
    ///
    /// Returns `None` if out of bounds.
    pub fn cell(&self, row: u16, col: u16) -> Option<&Cell> {
        if row < self.rows && col < self.cols {
            Some(&self.cells[self.index(row, col)])
        } else {
            None
        }
    }

    /// Get a mutable reference to the cell at `(row, col)`.
    ///
    /// Returns `None` if out of bounds.
    pub fn cell_mut(&mut self, row: u16, col: u16) -> Option<&mut Cell> {
        if row < self.rows && col < self.cols {
            let idx = self.index(row, col);
            Some(&mut self.cells[idx])
        } else {
            None
        }
    }

    /// Get a slice of cells for the given row.
    ///
    /// Returns `None` if `row` is out of bounds.
    pub fn row_cells(&self, row: u16) -> Option<&[Cell]> {
        if row < self.rows {
            let start = (row as usize) * (self.cols as usize);
            let end = start + (self.cols as usize);
            Some(&self.cells[start..end])
        } else {
            None
        }
    }

    /// Get a mutable slice of cells for the given row.
    pub fn row_cells_mut(&mut self, row: u16) -> Option<&mut [Cell]> {
        if row < self.rows {
            let start = (row as usize) * (self.cols as usize);
            let end = start + (self.cols as usize);
            Some(&mut self.cells[start..end])
        } else {
            None
        }
    }

    // ── Erase operations ────────────────────────────────────────────

    /// ED 0: Erase from cursor to end of display.
    pub fn erase_below(&mut self, row: u16, col: u16, bg: Color) {
        if row >= self.rows {
            return;
        }
        // Erase from cursor to end of current row.
        self.erase_range(row, col, row, self.cols, bg);
        // Erase all rows below.
        self.erase_range(row + 1, 0, self.rows, 0, bg);
    }

    /// ED 1: Erase from start of display to cursor (inclusive).
    pub fn erase_above(&mut self, row: u16, col: u16, bg: Color) {
        if row >= self.rows {
            return;
        }
        // Erase all rows above.
        if row > 0 {
            self.erase_range(0, 0, row, 0, bg);
        }
        // Erase from start of current row through cursor (inclusive).
        let ec = (col + 1).min(self.cols);
        self.erase_range(row, 0, row, ec, bg);
    }

    /// ED 2: Erase entire display.
    pub fn erase_all(&mut self, bg: Color) {
        for cell in &mut self.cells {
            cell.erase(bg);
        }
    }

    /// EL 0: Erase from cursor to end of line.
    pub fn erase_line_right(&mut self, row: u16, col: u16, bg: Color) {
        self.erase_range(row, col, row, self.cols, bg);
    }

    /// EL 1: Erase from start of line to cursor (inclusive).
    pub fn erase_line_left(&mut self, row: u16, col: u16, bg: Color) {
        let ec = (col + 1).min(self.cols);
        self.erase_range(row, 0, row, ec, bg);
    }

    /// EL 2: Erase entire line.
    pub fn erase_line(&mut self, row: u16, bg: Color) {
        self.erase_range(row, 0, row, self.cols, bg);
    }

    /// ECH: Erase `count` characters starting at `(row, col)`.
    pub fn erase_chars(&mut self, row: u16, col: u16, count: u16, bg: Color) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        let end = (col + count).min(self.cols);
        self.erase_range(row, col, row, end, bg);
    }

    /// Erase a rectangular region. Single row if `end_row == start_row`,
    /// or full rows if `end_col == 0` for row > start_row.
    fn erase_range(
        &mut self,
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
        bg: Color,
    ) {
        let sr = start_row.min(self.rows);
        let er = end_row.min(self.rows);

        if sr == er {
            // Single row partial erase.
            let sc = start_col.min(self.cols);
            let ec = end_col.min(self.cols);
            for c in sc..ec {
                let idx = self.index(sr, c);
                self.cells[idx].erase(bg);
            }
        } else {
            // First row partial.
            let sc = start_col.min(self.cols);
            for c in sc..self.cols {
                let idx = self.index(sr, c);
                self.cells[idx].erase(bg);
            }
            // Full rows in between.
            for r in (sr + 1)..er {
                for c in 0..self.cols {
                    let idx = self.index(r, c);
                    self.cells[idx].erase(bg);
                }
            }
            // Last row partial (if end_col > 0).
            if end_col > 0 && er < self.rows {
                let ec = end_col.min(self.cols);
                for c in 0..ec {
                    let idx = self.index(er, c);
                    self.cells[idx].erase(bg);
                }
            }
        }
    }

    // ── Fill / clear ────────────────────────────────────────────────

    /// Fill a region of cells with defaults (erase with default bg).
    ///
    /// Coordinates are clamped to grid bounds.
    pub fn clear_region(&mut self, start_row: u16, start_col: u16, end_row: u16, end_col: u16) {
        let sr = start_row.min(self.rows);
        let er = end_row.min(self.rows);
        let sc = start_col.min(self.cols);
        let ec = end_col.min(self.cols);

        for r in sr..er {
            for c in sc..ec {
                let idx = self.index(r, c);
                self.cells[idx] = Cell::default();
            }
        }
    }

    /// Clear the entire grid.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
    }

    // ── Insert / delete characters ──────────────────────────────────

    /// ICH: Insert `count` blank cells at `(row, col)`, shifting existing
    /// cells to the right. Cells that shift past the right margin are lost.
    pub fn insert_chars(&mut self, row: u16, col: u16, count: u16, bg: Color) {
        if row >= self.rows || col >= self.cols || count == 0 {
            return;
        }
        let cols = self.cols as usize;
        let c = col as usize;
        let n = (count as usize).min(cols - c);
        let start = self.index(row, 0);
        let row_slice = &mut self.cells[start..start + cols];

        // Shift right: copy from right to left to avoid overlap issues.
        for i in (c + n..cols).rev() {
            row_slice[i] = row_slice[i - n];
        }
        // Blank the inserted positions.
        for cell in &mut row_slice[c..c + n] {
            cell.erase(bg);
        }
    }

    /// DCH: Delete `count` cells at `(row, col)`, shifting remaining cells
    /// left. Blank cells are inserted at the right margin.
    pub fn delete_chars(&mut self, row: u16, col: u16, count: u16, bg: Color) {
        if row >= self.rows || col >= self.cols || count == 0 {
            return;
        }
        let cols = self.cols as usize;
        let c = col as usize;
        let n = (count as usize).min(cols - c);
        let start = self.index(row, 0);
        let row_slice = &mut self.cells[start..start + cols];

        // Shift left.
        for i in c..cols - n {
            row_slice[i] = row_slice[i + n];
        }
        // Blank the vacated positions at the right.
        for cell in &mut row_slice[cols - n..] {
            cell.erase(bg);
        }
    }

    // ── Scroll operations ───────────────────────────────────────────

    /// Scroll lines up: remove `count` rows starting at `top`, shift everything
    /// above `bottom` up, and fill the gap at the bottom with blanks.
    ///
    /// `top` and `bottom` define the scroll region (0-indexed, exclusive bottom).
    pub fn scroll_up(&mut self, top: u16, bottom: u16, count: u16) {
        let top = top.min(self.rows);
        let bottom = bottom.min(self.rows);
        if top >= bottom || count == 0 {
            return;
        }
        let count = count.min(bottom - top);
        let cols = self.cols as usize;

        // Shift rows up.
        let src_start = (top + count) as usize * cols;
        let dst_start = top as usize * cols;
        let move_len = (bottom - top - count) as usize * cols;
        self.cells
            .copy_within(src_start..src_start + move_len, dst_start);

        // Blank the vacated rows at the bottom.
        let blank_start = (bottom - count) as usize * cols;
        let blank_end = bottom as usize * cols;
        for cell in &mut self.cells[blank_start..blank_end] {
            *cell = Cell::default();
        }
    }

    /// Scroll lines down: insert `count` blank rows at `top`, shifting
    /// everything down and discarding rows that fall past `bottom`.
    pub fn scroll_down(&mut self, top: u16, bottom: u16, count: u16) {
        let top = top.min(self.rows);
        let bottom = bottom.min(self.rows);
        if top >= bottom || count == 0 {
            return;
        }
        let count = count.min(bottom - top);
        let cols = self.cols as usize;

        // Shift rows down.
        let src_start = top as usize * cols;
        let src_len = (bottom - top - count) as usize * cols;
        let dst_start = (top + count) as usize * cols;
        self.cells
            .copy_within(src_start..src_start + src_len, dst_start);

        // Blank the vacated rows at the top.
        let blank_end = (top + count) as usize * cols;
        for cell in &mut self.cells[top as usize * cols..blank_end] {
            *cell = Cell::default();
        }
    }

    /// IL: Insert `count` blank lines at `row` within the scroll region
    /// `[top, bottom)`. Lines that fall past `bottom` are discarded.
    pub fn insert_lines(&mut self, row: u16, count: u16, top: u16, bottom: u16) {
        if row < top || row >= bottom {
            return;
        }
        self.scroll_down(row, bottom, count);
    }

    /// DL: Delete `count` lines at `row` within the scroll region
    /// `[top, bottom)`. Blank lines appear at `bottom - count`.
    pub fn delete_lines(&mut self, row: u16, count: u16, top: u16, bottom: u16) {
        if row < top || row >= bottom {
            return;
        }
        self.scroll_up(row, bottom, count);
    }

    // ── Wide character handling ──────────────────────────────────────

    /// Write a wide (2-column) character at `(row, col)`.
    ///
    /// Sets the leading cell at `col` and the continuation cell at `col+1`.
    /// If `col+1` is past the right margin, no write occurs.
    /// Also clears any existing wide char that this write would partially
    /// overwrite (the "wide char fixup").
    pub fn write_wide_char(&mut self, row: u16, col: u16, ch: char, attrs: crate::cell::SgrAttrs) {
        if row >= self.rows || col + 1 >= self.cols {
            return;
        }
        // Fixup: if we're overwriting the continuation of a wide char at col,
        // clear the leading cell at col-1.
        if col > 0 {
            let prev_idx = self.index(row, col - 1);
            if self.cells[prev_idx].is_wide() {
                self.cells[prev_idx].clear();
            }
        }
        // Fixup: if we're overwriting the leading cell of a wide char at col+1,
        // clear the continuation at col+2.
        let next_idx = self.index(row, col + 1);
        if self.cells[next_idx].is_wide() && col + 2 < self.cols {
            let cont_idx = self.index(row, col + 2);
            self.cells[cont_idx].clear();
        }

        let (lead, cont) = Cell::wide(ch, attrs);
        let lead_idx = self.index(row, col);
        self.cells[lead_idx] = lead;
        self.cells[next_idx] = cont;
    }

    // ── Resize ──────────────────────────────────────────────────────

    /// Resize the grid to new dimensions.
    ///
    /// Content is preserved where possible: rows/columns that fit in the
    /// new dimensions are kept, extras are truncated, new space is blanked.
    pub fn resize(&mut self, new_cols: u16, new_rows: u16) {
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }
        let mut new_cells = vec![Cell::default(); new_cols as usize * new_rows as usize];
        let copy_rows = self.rows.min(new_rows);
        let copy_cols = self.cols.min(new_cols);

        for r in 0..copy_rows {
            let old_start = (r as usize) * (self.cols as usize);
            let new_start = (r as usize) * (new_cols as usize);
            new_cells[new_start..new_start + copy_cols as usize]
                .copy_from_slice(&self.cells[old_start..old_start + copy_cols as usize]);
        }

        self.cells = new_cells;
        self.cols = new_cols;
        self.rows = new_rows;
    }

    /// Convert (row, col) to flat index.
    #[inline]
    fn index(&self, row: u16, col: u16) -> usize {
        (row as usize) * (self.cols as usize) + (col as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::SgrAttrs;

    #[test]
    fn new_grid_has_correct_dimensions() {
        let g = Grid::new(80, 24);
        assert_eq!(g.cols(), 80);
        assert_eq!(g.rows(), 24);
    }

    #[test]
    fn cells_default_to_space() {
        let g = Grid::new(10, 5);
        let cell = g.cell(0, 0).unwrap();
        assert_eq!(cell.content(), ' ');
    }

    #[test]
    fn cell_mut_allows_modification() {
        let mut g = Grid::new(10, 5);
        if let Some(cell) = g.cell_mut(2, 3) {
            cell.set_content('X', 1);
        }
        assert_eq!(g.cell(2, 3).unwrap().content(), 'X');
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let g = Grid::new(10, 5);
        assert!(g.cell(5, 0).is_none());
        assert!(g.cell(0, 10).is_none());
    }

    #[test]
    fn row_cells_returns_correct_slice() {
        let mut g = Grid::new(3, 2);
        g.cell_mut(1, 0).unwrap().set_content('A', 1);
        g.cell_mut(1, 1).unwrap().set_content('B', 1);
        g.cell_mut(1, 2).unwrap().set_content('C', 1);
        let row = g.row_cells(1).unwrap();
        assert_eq!(row.len(), 3);
        assert_eq!(row[0].content(), 'A');
        assert_eq!(row[1].content(), 'B');
        assert_eq!(row[2].content(), 'C');
    }

    #[test]
    fn clear_region_erases_cells() {
        let mut g = Grid::new(5, 5);
        g.cell_mut(1, 1).unwrap().set_content('X', 1);
        g.cell_mut(2, 2).unwrap().set_content('Y', 1);
        g.clear_region(1, 1, 3, 3);
        assert_eq!(g.cell(1, 1).unwrap().content(), ' ');
        assert_eq!(g.cell(2, 2).unwrap().content(), ' ');
    }

    #[test]
    fn scroll_up_shifts_and_blanks() {
        let mut g = Grid::new(3, 4);
        for r in 0..4u16 {
            let ch = (b'A' + r as u8) as char;
            for c in 0..3u16 {
                g.cell_mut(r, c).unwrap().set_content(ch, 1);
            }
        }
        g.scroll_up(0, 4, 1);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'B');
        assert_eq!(g.cell(1, 0).unwrap().content(), 'C');
        assert_eq!(g.cell(2, 0).unwrap().content(), 'D');
        assert_eq!(g.cell(3, 0).unwrap().content(), ' ');
    }

    #[test]
    fn scroll_down_shifts_and_blanks() {
        let mut g = Grid::new(3, 4);
        for r in 0..4u16 {
            let ch = (b'A' + r as u8) as char;
            for c in 0..3u16 {
                g.cell_mut(r, c).unwrap().set_content(ch, 1);
            }
        }
        g.scroll_down(0, 4, 1);
        assert_eq!(g.cell(0, 0).unwrap().content(), ' ');
        assert_eq!(g.cell(1, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(2, 0).unwrap().content(), 'B');
        assert_eq!(g.cell(3, 0).unwrap().content(), 'C');
    }

    // ── Erase operations ────────────────────────────────────────────

    #[test]
    fn erase_below_from_mid_row() {
        let mut g = Grid::new(5, 3);
        for r in 0..3u16 {
            for c in 0..5u16 {
                g.cell_mut(r, c).unwrap().set_content('X', 1);
            }
        }
        g.erase_below(1, 2, Color::Default);
        // Row 0 untouched.
        assert_eq!(g.cell(0, 4).unwrap().content(), 'X');
        // Row 1 cols 0-1 untouched, cols 2-4 erased.
        assert_eq!(g.cell(1, 1).unwrap().content(), 'X');
        assert_eq!(g.cell(1, 2).unwrap().content(), ' ');
        assert_eq!(g.cell(1, 4).unwrap().content(), ' ');
        // Row 2 fully erased.
        assert_eq!(g.cell(2, 0).unwrap().content(), ' ');
    }

    #[test]
    fn erase_above_from_mid_row() {
        let mut g = Grid::new(5, 3);
        for r in 0..3u16 {
            for c in 0..5u16 {
                g.cell_mut(r, c).unwrap().set_content('X', 1);
            }
        }
        g.erase_above(1, 2, Color::Default);
        // Row 0 fully erased.
        assert_eq!(g.cell(0, 0).unwrap().content(), ' ');
        // Row 1 cols 0-2 erased, cols 3-4 untouched.
        assert_eq!(g.cell(1, 2).unwrap().content(), ' ');
        assert_eq!(g.cell(1, 3).unwrap().content(), 'X');
        // Row 2 untouched.
        assert_eq!(g.cell(2, 0).unwrap().content(), 'X');
    }

    #[test]
    fn erase_all_clears_grid() {
        let mut g = Grid::new(3, 3);
        g.cell_mut(1, 1).unwrap().set_content('Y', 1);
        g.erase_all(Color::Named(4));
        assert_eq!(g.cell(1, 1).unwrap().content(), ' ');
        assert_eq!(g.cell(1, 1).unwrap().attrs.bg, Color::Named(4));
    }

    #[test]
    fn erase_line_right() {
        let mut g = Grid::new(5, 1);
        for c in 0..5u16 {
            g.cell_mut(0, c)
                .unwrap()
                .set_content((b'A' + c as u8) as char, 1);
        }
        g.erase_line_right(0, 2, Color::Default);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(0, 1).unwrap().content(), 'B');
        assert_eq!(g.cell(0, 2).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 4).unwrap().content(), ' ');
    }

    #[test]
    fn erase_line_left() {
        let mut g = Grid::new(5, 1);
        for c in 0..5u16 {
            g.cell_mut(0, c)
                .unwrap()
                .set_content((b'A' + c as u8) as char, 1);
        }
        g.erase_line_left(0, 2, Color::Default);
        assert_eq!(g.cell(0, 0).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 2).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 3).unwrap().content(), 'D');
    }

    #[test]
    fn erase_chars_within_row() {
        let mut g = Grid::new(5, 1);
        for c in 0..5u16 {
            g.cell_mut(0, c).unwrap().set_content('X', 1);
        }
        g.erase_chars(0, 1, 2, Color::Default);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'X');
        assert_eq!(g.cell(0, 1).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 2).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 3).unwrap().content(), 'X');
    }

    // ── Insert/delete characters ────────────────────────────────────

    #[test]
    fn insert_chars_shifts_right() {
        let mut g = Grid::new(5, 1);
        for c in 0..5u16 {
            g.cell_mut(0, c)
                .unwrap()
                .set_content((b'A' + c as u8) as char, 1);
        }
        // Insert 2 blanks at col 1: A _ _ B C (D and E lost)
        g.insert_chars(0, 1, 2, Color::Default);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(0, 1).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 2).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 3).unwrap().content(), 'B');
        assert_eq!(g.cell(0, 4).unwrap().content(), 'C');
    }

    #[test]
    fn delete_chars_shifts_left() {
        let mut g = Grid::new(5, 1);
        for c in 0..5u16 {
            g.cell_mut(0, c)
                .unwrap()
                .set_content((b'A' + c as u8) as char, 1);
        }
        // Delete 2 at col 1: A D E _ _
        g.delete_chars(0, 1, 2, Color::Default);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(0, 1).unwrap().content(), 'D');
        assert_eq!(g.cell(0, 2).unwrap().content(), 'E');
        assert_eq!(g.cell(0, 3).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 4).unwrap().content(), ' ');
    }

    // ── Insert/delete lines ─────────────────────────────────────────

    #[test]
    fn insert_lines_within_region() {
        let mut g = Grid::new(2, 4);
        for r in 0..4u16 {
            let ch = (b'A' + r as u8) as char;
            for c in 0..2u16 {
                g.cell_mut(r, c).unwrap().set_content(ch, 1);
            }
        }
        // Insert 1 line at row 1 within region [0, 4)
        g.insert_lines(1, 1, 0, 4);
        // Result: A _ B C (D lost)
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(1, 0).unwrap().content(), ' ');
        assert_eq!(g.cell(2, 0).unwrap().content(), 'B');
        assert_eq!(g.cell(3, 0).unwrap().content(), 'C');
    }

    #[test]
    fn delete_lines_within_region() {
        let mut g = Grid::new(2, 4);
        for r in 0..4u16 {
            let ch = (b'A' + r as u8) as char;
            for c in 0..2u16 {
                g.cell_mut(r, c).unwrap().set_content(ch, 1);
            }
        }
        // Delete 1 line at row 1 within region [0, 4)
        g.delete_lines(1, 1, 0, 4);
        // Result: A C D _
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(1, 0).unwrap().content(), 'C');
        assert_eq!(g.cell(2, 0).unwrap().content(), 'D');
        assert_eq!(g.cell(3, 0).unwrap().content(), ' ');
    }

    // ── Wide characters ─────────────────────────────────────────────

    #[test]
    fn write_wide_char_sets_two_cells() {
        let mut g = Grid::new(10, 1);
        g.write_wide_char(0, 3, '中', SgrAttrs::default());
        assert!(g.cell(0, 3).unwrap().is_wide());
        assert_eq!(g.cell(0, 3).unwrap().content(), '中');
        assert!(g.cell(0, 4).unwrap().is_wide_continuation());
    }

    #[test]
    fn write_wide_char_at_right_margin_is_noop() {
        let mut g = Grid::new(5, 1);
        // col + 1 >= cols, so no write.
        g.write_wide_char(0, 4, '中', SgrAttrs::default());
        assert_eq!(g.cell(0, 4).unwrap().content(), ' ');
    }

    #[test]
    fn overwrite_wide_continuation_clears_leading() {
        let mut g = Grid::new(10, 1);
        g.write_wide_char(0, 2, '中', SgrAttrs::default());
        // Now overwrite at col 3 (continuation of '中').
        g.write_wide_char(0, 3, '国', SgrAttrs::default());
        // The old leading cell at col 2 should be cleared.
        assert_eq!(g.cell(0, 2).unwrap().content(), ' ');
        assert!(!g.cell(0, 2).unwrap().is_wide());
        // New wide char at 3-4.
        assert!(g.cell(0, 3).unwrap().is_wide());
        assert!(g.cell(0, 4).unwrap().is_wide_continuation());
    }

    // ── Resize ──────────────────────────────────────────────────────

    #[test]
    fn resize_larger_preserves_content() {
        let mut g = Grid::new(3, 2);
        g.cell_mut(0, 0).unwrap().set_content('A', 1);
        g.cell_mut(1, 2).unwrap().set_content('Z', 1);
        g.resize(5, 4);
        assert_eq!(g.cols(), 5);
        assert_eq!(g.rows(), 4);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(1, 2).unwrap().content(), 'Z');
        assert_eq!(g.cell(3, 4).unwrap().content(), ' ');
    }

    #[test]
    fn resize_smaller_truncates() {
        let mut g = Grid::new(5, 5);
        g.cell_mut(4, 4).unwrap().set_content('X', 1);
        g.resize(3, 3);
        assert_eq!(g.cols(), 3);
        assert_eq!(g.rows(), 3);
        assert!(g.cell(4, 4).is_none());
    }

    #[test]
    fn resize_same_is_noop() {
        let mut g = Grid::new(10, 5);
        g.cell_mut(0, 0).unwrap().set_content('A', 1);
        g.resize(10, 5);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn zero_size_grid() {
        let g = Grid::new(0, 0);
        assert_eq!(g.cols(), 0);
        assert_eq!(g.rows(), 0);
        assert!(g.cell(0, 0).is_none());
    }

    #[test]
    fn one_by_one_grid() {
        let mut g = Grid::new(1, 1);
        g.cell_mut(0, 0).unwrap().set_content('X', 1);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'X');
        g.erase_all(Color::Default);
        assert_eq!(g.cell(0, 0).unwrap().content(), ' ');
    }

    #[test]
    fn scroll_zero_count_is_noop() {
        let mut g = Grid::new(3, 3);
        g.cell_mut(0, 0).unwrap().set_content('A', 1);
        g.scroll_up(0, 3, 0);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
    }

    #[test]
    fn insert_chars_at_last_col() {
        let mut g = Grid::new(3, 1);
        g.cell_mut(0, 0).unwrap().set_content('A', 1);
        g.cell_mut(0, 1).unwrap().set_content('B', 1);
        g.cell_mut(0, 2).unwrap().set_content('C', 1);
        g.insert_chars(0, 2, 5, Color::Default);
        // Only 1 cell can be inserted at col 2 (col 2 is last).
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(0, 1).unwrap().content(), 'B');
        assert_eq!(g.cell(0, 2).unwrap().content(), ' ');
    }

    #[test]
    fn delete_chars_more_than_remaining() {
        let mut g = Grid::new(5, 1);
        for c in 0..5u16 {
            g.cell_mut(0, c).unwrap().set_content('X', 1);
        }
        g.delete_chars(0, 3, 100, Color::Default);
        assert_eq!(g.cell(0, 3).unwrap().content(), ' ');
        assert_eq!(g.cell(0, 4).unwrap().content(), ' ');
    }

    #[test]
    fn erase_out_of_bounds_is_safe() {
        let mut g = Grid::new(5, 3);
        // None of these should panic.
        g.erase_below(99, 99, Color::Default);
        g.erase_above(99, 99, Color::Default);
        g.erase_chars(99, 99, 10, Color::Default);
        g.erase_line_right(99, 99, Color::Default);
    }

    #[test]
    fn insert_lines_outside_region_is_noop() {
        let mut g = Grid::new(2, 4);
        for r in 0..4u16 {
            g.cell_mut(r, 0)
                .unwrap()
                .set_content((b'A' + r as u8) as char, 1);
        }
        // Insert at row 0, but region is [1, 3) — row 0 is outside.
        g.insert_lines(0, 1, 1, 3);
        assert_eq!(g.cell(0, 0).unwrap().content(), 'A');
    }
}
