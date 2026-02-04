// Forbid unsafe in production; deny (with targeted allows) in tests for env var helpers.
#![cfg_attr(not(test), forbid(unsafe_code))]
#![cfg_attr(test, deny(unsafe_code))]

//! Core: terminal lifecycle, capability detection, events, and input parsing.
//!
//! # Role in FrankenTUI
//! `ftui-core` is the input layer. It owns terminal session setup/teardown,
//! capability probing, and normalized event types that the runtime consumes.
//!
//! # Primary responsibilities
//! - **TerminalSession**: RAII lifecycle for raw mode, alt-screen, and cleanup.
//! - **Event**: canonical input events (keys, mouse, paste, resize, focus).
//! - **Capability detection**: terminal features and overrides.
//! - **Input parsing**: robust decoding of terminal input streams.
//!
//! # How it fits in the system
//! The runtime (`ftui-runtime`) consumes `ftui-core::Event` values and drives
//! application models. The render kernel (`ftui-render`) is independent of
//! input, so `ftui-core` is the clean bridge between terminal I/O and the
//! deterministic render pipeline.

pub mod animation;
pub mod capability_override;
pub mod cursor;
pub mod event;
pub mod event_coalescer;
pub mod geometry;
pub mod gesture;
pub mod hover_stabilizer;
pub mod inline_mode;
pub mod input_parser;
pub mod key_sequence;
pub mod keybinding;
pub mod logging;
pub mod mux_passthrough;
pub mod semantic_event;
pub mod terminal_capabilities;
pub mod terminal_session;

#[cfg(feature = "caps-probe")]
pub mod caps_probe;

// Re-export tracing macros at crate root for ergonomic use.
#[cfg(feature = "tracing")]
pub use logging::{
    debug, debug_span, error, error_span, info, info_span, trace, trace_span, warn, warn_span,
};
