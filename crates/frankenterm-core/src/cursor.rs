//! Terminal cursor: position, visibility, and saved state.
//!
//! The cursor tracks the current writing position in the grid and manages
//! saved/restored state for DECSC/DECRC sequences.

use crate::cell::SgrAttrs;

/// Terminal cursor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Current row (0-indexed from top of viewport).
    pub row: u16,
    /// Current column (0-indexed from left).
    pub col: u16,
    /// Whether the cursor is visible (DECTCEM).
    pub visible: bool,
    /// Pending wrap: the cursor is at the right margin and the next printable
    /// character should trigger a line wrap. This avoids the xterm off-by-one
    /// behavior where the cursor sits *past* the last column.
    pub pending_wrap: bool,
    /// Current SGR attributes applied to newly written characters.
    pub attrs: SgrAttrs,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            pending_wrap: false,
            attrs: SgrAttrs::default(),
        }
    }
}

impl Cursor {
    /// Create a cursor at the given position with default attributes.
    pub fn at(row: u16, col: u16) -> Self {
        Self {
            row,
            col,
            ..Self::default()
        }
    }

    /// Clamp the cursor position to the given grid bounds.
    pub fn clamp(&mut self, rows: u16, cols: u16) {
        if rows > 0 {
            self.row = self.row.min(rows - 1);
        }
        if cols > 0 {
            self.col = self.col.min(cols - 1);
        }
        self.pending_wrap = false;
    }
}

/// Saved cursor state for DECSC / DECRC.
///
/// Captures the full cursor state so it can be restored exactly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavedCursor {
    pub row: u16,
    pub col: u16,
    pub attrs: SgrAttrs,
    pub origin_mode: bool,
    pub pending_wrap: bool,
}

impl SavedCursor {
    /// Capture the current cursor state.
    pub fn save(cursor: &Cursor, origin_mode: bool) -> Self {
        Self {
            row: cursor.row,
            col: cursor.col,
            attrs: cursor.attrs,
            origin_mode,
            pending_wrap: cursor.pending_wrap,
        }
    }

    /// Restore the saved state into the cursor.
    pub fn restore(&self, cursor: &mut Cursor) {
        cursor.row = self.row;
        cursor.col = self.col;
        cursor.attrs = self.attrs;
        cursor.pending_wrap = self.pending_wrap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor_at_origin() {
        let c = Cursor::default();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
        assert!(c.visible);
        assert!(!c.pending_wrap);
    }

    #[test]
    fn cursor_at_position() {
        let c = Cursor::at(5, 10);
        assert_eq!(c.row, 5);
        assert_eq!(c.col, 10);
    }

    #[test]
    fn cursor_clamp_to_bounds() {
        let mut c = Cursor::at(100, 200);
        c.clamp(24, 80);
        assert_eq!(c.row, 23);
        assert_eq!(c.col, 79);
        assert!(!c.pending_wrap);
    }

    #[test]
    fn save_restore_roundtrip() {
        let mut cursor = Cursor::at(5, 10);
        cursor.attrs.flags = crate::cell::SgrFlags::BOLD;
        cursor.pending_wrap = true;

        let saved = SavedCursor::save(&cursor, true);
        assert_eq!(saved.row, 5);
        assert_eq!(saved.col, 10);
        assert!(saved.origin_mode);

        let mut new_cursor = Cursor::default();
        saved.restore(&mut new_cursor);
        assert_eq!(new_cursor.row, 5);
        assert_eq!(new_cursor.col, 10);
        assert!(new_cursor.pending_wrap);
        assert_eq!(new_cursor.attrs.flags, crate::cell::SgrFlags::BOLD);
    }
}
