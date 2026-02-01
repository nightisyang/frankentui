#![forbid(unsafe_code)]

//! Core: terminal lifecycle, capability detection, events, and input parsing.

pub mod cursor;
pub mod event;
pub mod inline_mode;
pub mod input_parser;
pub mod terminal_capabilities;
pub mod terminal_session;
