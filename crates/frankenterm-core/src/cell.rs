//! Terminal cell: the fundamental unit of the grid.
//!
//! Each cell stores a character (or grapheme cluster) and its SGR attributes.
//! This is intentionally simpler than `ftui-render::Cell` — it models the
//! terminal's internal state rather than the rendering pipeline.

use bitflags::bitflags;

bitflags! {
    /// SGR text attribute flags.
    ///
    /// Maps directly to the ECMA-48 / VT100 SGR parameter values.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct SgrFlags: u16 {
        const BOLD          = 1 << 0;
        const DIM           = 1 << 1;
        const ITALIC        = 1 << 2;
        const UNDERLINE     = 1 << 3;
        const BLINK         = 1 << 4;
        const INVERSE       = 1 << 5;
        const HIDDEN        = 1 << 6;
        const STRIKETHROUGH = 1 << 7;
        const DOUBLE_UNDERLINE = 1 << 8;
        const CURLY_UNDERLINE  = 1 << 9;
        const OVERLINE      = 1 << 10;
    }
}

/// Color representation for terminal cells.
///
/// Supports the standard terminal color model hierarchy:
/// default → 16 named → 256 indexed → 24-bit RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Color {
    /// Terminal default (SGR 39 / SGR 49).
    #[default]
    Default,
    /// Named color index (0-15): standard 8 + bright 8.
    Named(u8),
    /// 256-color palette index (0-255).
    Indexed(u8),
    /// 24-bit true color.
    Rgb(u8, u8, u8),
}

/// SGR attributes for a cell: flags + foreground/background colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SgrAttrs {
    pub flags: SgrFlags,
    pub fg: Color,
    pub bg: Color,
    /// Underline color (SGR 58). `None` means use foreground.
    pub underline_color: Option<Color>,
}

impl SgrAttrs {
    /// Reset all attributes to default (SGR 0).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// A single cell in the terminal grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The character content. A space for empty/erased cells.
    /// Multi-codepoint grapheme clusters are stored as full strings.
    content: char,
    /// Display width of the content in terminal columns (1 or 2 for wide chars).
    width: u8,
    /// SGR text attributes.
    pub attrs: SgrAttrs,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: ' ',
            width: 1,
            attrs: SgrAttrs::default(),
        }
    }
}

impl Cell {
    /// Create a new cell with the given character and default attributes.
    pub fn new(ch: char) -> Self {
        Self {
            content: ch,
            width: 1,
            attrs: SgrAttrs::default(),
        }
    }

    /// Create a new cell with the given character, width, and attributes.
    pub fn with_attrs(ch: char, width: u8, attrs: SgrAttrs) -> Self {
        Self {
            content: ch,
            width,
            attrs,
        }
    }

    /// The character content of this cell.
    pub fn content(&self) -> char {
        self.content
    }

    /// The display width in terminal columns.
    pub fn width(&self) -> u8 {
        self.width
    }

    /// Set the character content and display width.
    pub fn set_content(&mut self, ch: char, width: u8) {
        self.content = ch;
        self.width = width;
    }

    /// Reset this cell to a blank space with the given background attributes.
    ///
    /// Used by erase operations (ED, EL, ECH) which fill with the current
    /// background color but reset all other attributes.
    pub fn erase(&mut self, bg: Color) {
        self.content = ' ';
        self.width = 1;
        self.attrs = SgrAttrs {
            bg,
            ..SgrAttrs::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cell_is_space() {
        let cell = Cell::default();
        assert_eq!(cell.content(), ' ');
        assert_eq!(cell.width(), 1);
        assert_eq!(cell.attrs, SgrAttrs::default());
    }

    #[test]
    fn cell_new_has_default_attrs() {
        let cell = Cell::new('A');
        assert_eq!(cell.content(), 'A');
        assert_eq!(cell.attrs.flags, SgrFlags::empty());
        assert_eq!(cell.attrs.fg, Color::Default);
        assert_eq!(cell.attrs.bg, Color::Default);
    }

    #[test]
    fn cell_erase_clears_content_and_attrs() {
        let mut cell = Cell::with_attrs(
            'X',
            1,
            SgrAttrs {
                flags: SgrFlags::BOLD | SgrFlags::ITALIC,
                fg: Color::Named(1),
                bg: Color::Named(4),
                underline_color: None,
            },
        );
        cell.erase(Color::Named(2));
        assert_eq!(cell.content(), ' ');
        assert_eq!(cell.attrs.flags, SgrFlags::empty());
        assert_eq!(cell.attrs.fg, Color::Default);
        assert_eq!(cell.attrs.bg, Color::Named(2));
    }

    #[test]
    fn sgr_attrs_reset() {
        let mut attrs = SgrAttrs {
            flags: SgrFlags::BOLD,
            fg: Color::Rgb(255, 0, 0),
            bg: Color::Indexed(42),
            underline_color: Some(Color::Named(3)),
        };
        attrs.reset();
        assert_eq!(attrs, SgrAttrs::default());
    }

    #[test]
    fn color_default() {
        assert_eq!(Color::default(), Color::Default);
    }
}
