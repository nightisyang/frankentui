//! Terminal grid: 2D cell matrix representing the visible viewport.
//!
//! The grid is the primary data model for the terminal. It owns a flat vector
//! of cells indexed by `(row, col)` and provides methods for the operations
//! that the VT parser dispatches (print, erase, scroll, resize).

use crate::cell::Cell;

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

    /// Fill a region of cells with defaults (erase).
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

    /// Convert (row, col) to flat index.
    #[inline]
    fn index(&self, row: u16, col: u16) -> usize {
        (row as usize) * (self.cols as usize) + (col as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Rows: A A A / B B B / C C C / D D D
        g.scroll_up(0, 4, 1);
        // Rows: B B B / C C C / D D D / _ _ _
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
        // Rows: A A A / B B B / C C C / D D D
        g.scroll_down(0, 4, 1);
        // Rows: _ _ _ / A A A / B B B / C C C
        assert_eq!(g.cell(0, 0).unwrap().content(), ' ');
        assert_eq!(g.cell(1, 0).unwrap().content(), 'A');
        assert_eq!(g.cell(2, 0).unwrap().content(), 'B');
        assert_eq!(g.cell(3, 0).unwrap().content(), 'C');
    }
}
