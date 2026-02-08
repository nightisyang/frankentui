//! Patch format (API skeleton).
//!
//! A Patch represents a minimal set of updates to apply to a terminal grid.
//! In the full system, patches are produced by the terminal engine and can be
//! consumed by renderers (native presenter, WebGPU renderer, trace replayer).

use crate::cell::Cell;

/// A single cell update at (row, col).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellUpdate {
    pub row: u16,
    pub col: u16,
    pub cell: Cell,
}

/// A minimal patch of updates for a viewport-sized grid.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Patch {
    pub cols: u16,
    pub rows: u16,
    pub updates: Vec<CellUpdate>,
}

impl Patch {
    /// Create an empty patch for a given grid shape.
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            updates: Vec::new(),
        }
    }

    /// Whether the patch has no updates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    /// Append a single cell update.
    pub fn push(&mut self, row: u16, col: u16, cell: Cell) {
        self.updates.push(CellUpdate { row, col, cell });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_patch_is_empty() {
        let p = Patch::new(80, 24);
        assert!(p.is_empty());
        assert_eq!(p.cols, 80);
        assert_eq!(p.rows, 24);
    }

    #[test]
    fn push_adds_update() {
        let mut p = Patch::new(2, 2);
        p.push(0, 1, Cell::new('X'));
        assert_eq!(p.updates.len(), 1);
        assert_eq!(p.updates[0].row, 0);
        assert_eq!(p.updates[0].col, 1);
        assert_eq!(p.updates[0].cell.content(), 'X');
    }
}
