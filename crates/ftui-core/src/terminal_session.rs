#![forbid(unsafe_code)]

//! Terminal session lifecycle guard.
//!
//! Owns raw-mode entry/exit and ensures cleanup on drop (including panic).
//!
//! # Backend Decision Spike (bd-10i.1.3)
//!
//! This module validates Crossterm as the terminal backend. Key requirements:
//! - Raw mode enter/exit must be reliable
//! - Cleanup must happen on normal exit AND panic
//! - Resize events must be delivered accurately
//!
//! See ADR-003 for the backend decision rationale.

use std::io::{self, Write};

/// Terminal session options.
#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    /// Enable alternate screen buffer.
    pub alternate_screen: bool,
    /// Enable mouse capture (SGR mode).
    pub mouse_capture: bool,
    /// Enable bracketed paste mode.
    pub bracketed_paste: bool,
    /// Enable focus change events.
    pub focus_events: bool,
}

/// A terminal session that manages raw mode and cleanup.
///
/// The terminal is restored to its original state when this struct is dropped,
/// whether through normal control flow or panic unwinding.
#[derive(Debug)]
pub struct TerminalSession {
    options: SessionOptions,
    /// Track what was enabled so we can disable on drop.
    alternate_screen_enabled: bool,
    mouse_enabled: bool,
    bracketed_paste_enabled: bool,
    focus_events_enabled: bool,
}

impl TerminalSession {
    /// Enter raw mode and optionally enable additional features.
    ///
    /// # Errors
    ///
    /// Returns an error if raw mode cannot be enabled.
    pub fn new(options: SessionOptions) -> io::Result<Self> {
        // Enter raw mode first
        crossterm::terminal::enable_raw_mode()?;

        let mut session = Self {
            options: options.clone(),
            alternate_screen_enabled: false,
            mouse_enabled: false,
            bracketed_paste_enabled: false,
            focus_events_enabled: false,
        };

        // Enable optional features
        let mut stdout = io::stdout();

        if options.alternate_screen {
            crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
            session.alternate_screen_enabled = true;
        }

        if options.mouse_capture {
            crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
            session.mouse_enabled = true;
        }

        if options.bracketed_paste {
            crossterm::execute!(stdout, crossterm::event::EnableBracketedPaste)?;
            session.bracketed_paste_enabled = true;
        }

        if options.focus_events {
            crossterm::execute!(stdout, crossterm::event::EnableFocusChange)?;
            session.focus_events_enabled = true;
        }

        Ok(session)
    }

    /// Create a minimal session (raw mode only).
    pub fn minimal() -> io::Result<Self> {
        Self::new(SessionOptions::default())
    }

    /// Get the current terminal size (columns, rows).
    pub fn size(&self) -> io::Result<(u16, u16)> {
        crossterm::terminal::size()
    }

    /// Poll for an event with a timeout.
    ///
    /// Returns `Ok(true)` if an event is available, `Ok(false)` if timeout.
    pub fn poll_event(&self, timeout: std::time::Duration) -> io::Result<bool> {
        crossterm::event::poll(timeout)
    }

    /// Read the next event (blocking until available).
    pub fn read_event(&self) -> io::Result<crossterm::event::Event> {
        crossterm::event::read()
    }

    /// Show the cursor.
    pub fn show_cursor(&self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), crossterm::cursor::Show)
    }

    /// Hide the cursor.
    pub fn hide_cursor(&self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), crossterm::cursor::Hide)
    }

    /// Get the session options.
    pub fn options(&self) -> &SessionOptions {
        &self.options
    }

    /// Cleanup helper (shared between drop and explicit cleanup).
    fn cleanup(&mut self) {
        let mut stdout = io::stdout();

        // Disable features in reverse order of enabling
        if self.focus_events_enabled {
            let _ = crossterm::execute!(stdout, crossterm::event::DisableFocusChange);
            self.focus_events_enabled = false;
        }

        if self.bracketed_paste_enabled {
            let _ = crossterm::execute!(stdout, crossterm::event::DisableBracketedPaste);
            self.bracketed_paste_enabled = false;
        }

        if self.mouse_enabled {
            let _ = crossterm::execute!(stdout, crossterm::event::DisableMouseCapture);
            self.mouse_enabled = false;
        }

        // Always show cursor before leaving
        let _ = crossterm::execute!(stdout, crossterm::cursor::Show);

        if self.alternate_screen_enabled {
            let _ = crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen);
            self.alternate_screen_enabled = false;
        }

        // Exit raw mode last
        let _ = crossterm::terminal::disable_raw_mode();

        // Flush to ensure cleanup bytes are sent
        let _ = stdout.flush();
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Spike validation notes (for ADR-003).
///
/// ## Crossterm Evaluation Results
///
/// ### Functionality (all verified)
/// - ✅ raw mode: `enable_raw_mode()` / `disable_raw_mode()`
/// - ✅ alternate screen: `EnterAlternateScreen` / `LeaveAlternateScreen`
/// - ✅ cursor show/hide: `Show` / `Hide`
/// - ✅ mouse mode (SGR): `EnableMouseCapture` / `DisableMouseCapture`
/// - ✅ bracketed paste: `EnableBracketedPaste` / `DisableBracketedPaste`
/// - ✅ focus events: `EnableFocusChange` / `DisableFocusChange`
/// - ✅ resize events: `Event::Resize(cols, rows)`
///
/// ### Robustness
/// - ✅ bounded-time reads via `poll()` with timeout
/// - ✅ handles partial sequences (internal buffer management)
/// - ⚠️ adversarial input: not fuzz-tested in this spike
///
/// ### Cleanup Discipline
/// - ✅ Drop impl guarantees cleanup on normal exit
/// - ✅ Drop impl guarantees cleanup on panic (via unwinding)
/// - ✅ cursor shown before exit
/// - ✅ raw mode disabled last
///
/// ### Platform Coverage
/// - ✅ Linux: fully supported
/// - ✅ macOS: fully supported
/// - ⚠️ Windows: supported with some feature limitations (see ADR-004)
///
/// ## Decision
/// **Crossterm is approved as the v1 terminal backend.**
///
/// Rationale: It provides all required functionality, handles cleanup via
/// standard Rust drop semantics, and has broad platform support.
///
/// Limitations documented in ADR-004 (Windows scope).
#[doc(hidden)]
pub const _SPIKE_NOTES: () = ();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_options_default_is_minimal() {
        let opts = SessionOptions::default();
        assert!(!opts.alternate_screen);
        assert!(!opts.mouse_capture);
        assert!(!opts.bracketed_paste);
        assert!(!opts.focus_events);
    }

    // Note: Interactive tests that actually enter raw mode should be run
    // via the spike example binary, not as unit tests, since they would
    // interfere with the test runner's terminal state.
    //
    // PTY-based tests can safely test cleanup behavior without affecting
    // the controlling terminal.
}
