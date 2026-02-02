#![forbid(unsafe_code)]

//! Shared theme, color palette, and style constants for the demo showcase.
//!
//! Inspired by Dracula/Tokyo Night color schemes but with a unique identity.

use ftui_render::cell::PackedRgba;
use ftui_style::{Style, StyleFlags};

// ---------------------------------------------------------------------------
// Color palette
// ---------------------------------------------------------------------------

/// Background colors.
pub mod bg {
    use super::*;

    pub const DEEP: PackedRgba = PackedRgba::rgb(15, 15, 30);
    pub const BASE: PackedRgba = PackedRgba::rgb(25, 25, 45);
    pub const SURFACE: PackedRgba = PackedRgba::rgb(35, 35, 60);
    pub const OVERLAY: PackedRgba = PackedRgba::rgb(45, 45, 75);
    pub const HIGHLIGHT: PackedRgba = PackedRgba::rgb(55, 55, 90);
}

/// Foreground / text colors.
pub mod fg {
    use super::*;

    pub const PRIMARY: PackedRgba = PackedRgba::rgb(220, 220, 240);
    pub const SECONDARY: PackedRgba = PackedRgba::rgb(180, 180, 210);
    pub const MUTED: PackedRgba = PackedRgba::rgb(120, 120, 150);
    pub const DISABLED: PackedRgba = PackedRgba::rgb(80, 80, 100);
}

/// Accent / semantic colors.
pub mod accent {
    use super::*;

    pub const PRIMARY: PackedRgba = PackedRgba::rgb(130, 170, 255);
    pub const SECONDARY: PackedRgba = PackedRgba::rgb(180, 130, 255);
    pub const SUCCESS: PackedRgba = PackedRgba::rgb(80, 220, 140);
    pub const WARNING: PackedRgba = PackedRgba::rgb(255, 200, 80);
    pub const ERROR: PackedRgba = PackedRgba::rgb(255, 100, 100);
    pub const INFO: PackedRgba = PackedRgba::rgb(100, 200, 255);
    pub const LINK: PackedRgba = PackedRgba::rgb(100, 180, 255);
}

/// Per-screen accent colors for visual distinction.
pub mod screen_accent {
    use super::*;

    pub const DASHBOARD: PackedRgba = PackedRgba::rgb(100, 180, 255);
    pub const SHAKESPEARE: PackedRgba = PackedRgba::rgb(255, 180, 100);
    pub const CODE_EXPLORER: PackedRgba = PackedRgba::rgb(130, 220, 130);
    pub const WIDGET_GALLERY: PackedRgba = PackedRgba::rgb(200, 130, 255);
    pub const LAYOUT_LAB: PackedRgba = PackedRgba::rgb(255, 130, 180);
    pub const FORMS_INPUT: PackedRgba = PackedRgba::rgb(130, 230, 230);
    pub const DATA_VIZ: PackedRgba = PackedRgba::rgb(255, 200, 100);
    pub const FILE_BROWSER: PackedRgba = PackedRgba::rgb(180, 200, 130);
    pub const ADVANCED: PackedRgba = PackedRgba::rgb(255, 130, 130);
    pub const PERFORMANCE: PackedRgba = PackedRgba::rgb(200, 200, 100);
    pub const MARKDOWN: PackedRgba = PackedRgba::rgb(180, 160, 255);
}

// ---------------------------------------------------------------------------
// Syntax highlighting colors.
// ---------------------------------------------------------------------------

pub mod syntax {
    use super::*;

    pub const KEYWORD: PackedRgba = PackedRgba::rgb(200, 120, 255);
    pub const STRING: PackedRgba = PackedRgba::rgb(130, 220, 130);
    pub const NUMBER: PackedRgba = PackedRgba::rgb(255, 180, 100);
    pub const COMMENT: PackedRgba = PackedRgba::rgb(100, 100, 140);
    pub const FUNCTION: PackedRgba = PackedRgba::rgb(100, 180, 255);
    pub const TYPE: PackedRgba = PackedRgba::rgb(130, 220, 230);
    pub const OPERATOR: PackedRgba = PackedRgba::rgb(200, 200, 220);
    pub const PUNCTUATION: PackedRgba = PackedRgba::rgb(160, 160, 190);
}

// ---------------------------------------------------------------------------
// Named styles
// ---------------------------------------------------------------------------

/// Semantic text styles.
pub fn title() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::BOLD)
}

pub fn subtitle() -> Style {
    Style::new().fg(fg::SECONDARY).attrs(StyleFlags::ITALIC)
}

pub fn body() -> Style {
    Style::new().fg(fg::PRIMARY)
}

pub fn muted() -> Style {
    Style::new().fg(fg::MUTED)
}

pub fn link() -> Style {
    Style::new().fg(accent::LINK).attrs(StyleFlags::UNDERLINE)
}

pub fn code() -> Style {
    Style::new().fg(accent::INFO).bg(bg::SURFACE)
}

pub fn error_style() -> Style {
    Style::new().fg(accent::ERROR).attrs(StyleFlags::BOLD)
}

pub fn success() -> Style {
    Style::new().fg(accent::SUCCESS).attrs(StyleFlags::BOLD)
}

pub fn warning() -> Style {
    Style::new().fg(accent::WARNING).attrs(StyleFlags::BOLD)
}

// ---------------------------------------------------------------------------
// Attribute showcase styles (exercises every StyleFlags variant)
// ---------------------------------------------------------------------------

pub fn bold() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::BOLD)
}

pub fn dim() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::DIM)
}

pub fn italic() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::ITALIC)
}

pub fn underline() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::UNDERLINE)
}

pub fn double_underline() -> Style {
    Style::new()
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::DOUBLE_UNDERLINE)
}

pub fn curly_underline() -> Style {
    Style::new()
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::CURLY_UNDERLINE)
}

pub fn blink_style() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::BLINK)
}

pub fn reverse() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::REVERSE)
}

pub fn hidden() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::HIDDEN)
}

pub fn strikethrough() -> Style {
    Style::new()
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::STRIKETHROUGH)
}

// ---------------------------------------------------------------------------
// Component styles
// ---------------------------------------------------------------------------

/// Tab bar background.
pub fn tab_bar() -> Style {
    Style::new().bg(bg::SURFACE).fg(fg::SECONDARY)
}

/// Active tab.
pub fn tab_active() -> Style {
    Style::new()
        .bg(bg::HIGHLIGHT)
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::BOLD)
}

/// Status bar background.
pub fn status_bar() -> Style {
    Style::new().bg(bg::SURFACE).fg(fg::MUTED)
}

/// Content area border.
pub fn content_border() -> Style {
    Style::new().fg(PackedRgba::rgb(60, 60, 100))
}

/// Help overlay background.
pub fn help_overlay() -> Style {
    Style::new().bg(bg::OVERLAY).fg(fg::PRIMARY)
}
