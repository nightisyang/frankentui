#![forbid(unsafe_code)]

//! Style types for FrankenTUI with CSS-like cascading semantics.
//!
//! This crate provides:
//! - [`Style`] for unified text styling with CSS-like inheritance
//! - [`StyleSheet`] for named style registration (CSS-like classes)
//! - [`ColorDowngrader`] for color profile conversion (TrueColor → 256 → 16 → mono)

pub mod color;
pub mod style;
pub mod stylesheet;

pub use color::{Ansi16Color, ColorDowngrader, ColorProfile, MonoColor, TerminalColor};
pub use style::{Style, StyleFlags};
pub use stylesheet::{StyleId, StyleSheet};
