//! Terminal modes (ANSI + DEC private).
//!
//! This module models the mode bits that influence how the terminal engine
//! mutates the grid (origin mode, autowrap, insert mode, etc.).
//!
//! The intent is to keep this as pure state with small helpers so that the
//! VT/ANSI parser can toggle modes deterministically.

use bitflags::bitflags;

bitflags! {
    /// DEC private mode flags (DECSET/DECRST, `CSI ? Pm h` / `CSI ? Pm l`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DecModes: u32 {
        /// DECCKM (mode 1): Application cursor keys.
        const APPLICATION_CURSOR = 1 << 0;
        /// DECOM (mode 6): Origin mode — cursor addressing relative to scroll region.
        const ORIGIN = 1 << 1;
        /// DECAWM (mode 7): Auto-wrap at right margin.
        const AUTOWRAP = 1 << 2;
        /// DECTCEM (mode 25): Text cursor enable (visible).
        const CURSOR_VISIBLE = 1 << 3;
        /// Mode 1000: Mouse button event tracking.
        const MOUSE_BUTTON = 1 << 4;
        /// Mode 1002: Mouse cell motion tracking.
        const MOUSE_CELL_MOTION = 1 << 5;
        /// Mode 1003: Mouse all motion tracking.
        const MOUSE_ALL_MOTION = 1 << 6;
        /// Mode 1004: Focus event reporting.
        const FOCUS_EVENTS = 1 << 7;
        /// Mode 1006: SGR extended mouse coordinates.
        const MOUSE_SGR = 1 << 8;
        /// Mode 1049: Alternate screen buffer (save cursor + switch + clear).
        const ALT_SCREEN = 1 << 9;
        /// Mode 2004: Bracketed paste.
        const BRACKETED_PASTE = 1 << 10;
        /// Mode 2026: Synchronized output.
        const SYNC_OUTPUT = 1 << 11;
    }
}

bitflags! {
    /// ANSI standard mode flags (SM/RM, `CSI Pm h` / `CSI Pm l`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct AnsiModes: u8 {
        /// IRM (mode 4): Insert/Replace mode.
        const INSERT = 1 << 0;
        /// LNM (mode 20): Linefeed / Newline mode.
        const LINEFEED_NEWLINE = 1 << 1;
    }
}

/// Combined mode state for the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modes {
    pub dec: DecModes,
    pub ansi: AnsiModes,
}

impl Modes {
    /// Construct default modes (typical xterm defaults).
    /// DECAWM and DECTCEM are ON by default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dec: DecModes::AUTOWRAP | DecModes::CURSOR_VISIBLE,
            ansi: AnsiModes::empty(),
        }
    }

    /// Reset all modes to power-on defaults.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // ── DEC mode accessors ──────────────────────────────────────────

    /// Raw access to DEC mode flags.
    #[must_use]
    pub fn dec_flags(&self) -> DecModes {
        self.dec
    }

    /// Whether origin mode (DECOM) is enabled.
    #[must_use]
    pub fn origin_mode(&self) -> bool {
        self.dec.contains(DecModes::ORIGIN)
    }

    /// Enable/disable origin mode.
    pub fn set_origin_mode(&mut self, enabled: bool) {
        self.dec.set(DecModes::ORIGIN, enabled);
    }

    /// Whether autowrap (DECAWM) is enabled.
    #[must_use]
    pub fn autowrap(&self) -> bool {
        self.dec.contains(DecModes::AUTOWRAP)
    }

    /// Enable/disable autowrap.
    pub fn set_autowrap(&mut self, enabled: bool) {
        self.dec.set(DecModes::AUTOWRAP, enabled);
    }

    /// Whether the cursor is visible (DECTCEM).
    #[must_use]
    pub fn cursor_visible(&self) -> bool {
        self.dec.contains(DecModes::CURSOR_VISIBLE)
    }

    /// Enable/disable cursor visibility.
    pub fn set_cursor_visible(&mut self, enabled: bool) {
        self.dec.set(DecModes::CURSOR_VISIBLE, enabled);
    }

    /// Whether insert mode (IRM) is enabled.
    #[must_use]
    pub fn insert_mode(&self) -> bool {
        self.ansi.contains(AnsiModes::INSERT)
    }

    /// Enable/disable insert mode.
    pub fn set_insert_mode(&mut self, enabled: bool) {
        self.ansi.set(AnsiModes::INSERT, enabled);
    }

    /// Whether alt screen buffer is active.
    #[must_use]
    pub fn alt_screen(&self) -> bool {
        self.dec.contains(DecModes::ALT_SCREEN)
    }

    /// Enable/disable alt screen.
    pub fn set_alt_screen(&mut self, enabled: bool) {
        self.dec.set(DecModes::ALT_SCREEN, enabled);
    }

    /// Whether bracketed paste is enabled.
    #[must_use]
    pub fn bracketed_paste(&self) -> bool {
        self.dec.contains(DecModes::BRACKETED_PASTE)
    }

    /// Enable/disable bracketed paste.
    pub fn set_bracketed_paste(&mut self, enabled: bool) {
        self.dec.set(DecModes::BRACKETED_PASTE, enabled);
    }

    /// Whether focus event reporting is enabled.
    #[must_use]
    pub fn focus_events(&self) -> bool {
        self.dec.contains(DecModes::FOCUS_EVENTS)
    }

    /// Enable/disable focus events.
    pub fn set_focus_events(&mut self, enabled: bool) {
        self.dec.set(DecModes::FOCUS_EVENTS, enabled);
    }

    /// Whether synchronized output is enabled.
    #[must_use]
    pub fn sync_output(&self) -> bool {
        self.dec.contains(DecModes::SYNC_OUTPUT)
    }

    /// Enable/disable synchronized output.
    pub fn set_sync_output(&mut self, enabled: bool) {
        self.dec.set(DecModes::SYNC_OUTPUT, enabled);
    }

    // ── DEC mode by number ──────────────────────────────────────────

    /// Set a DEC private mode by its ECMA-48 number.
    /// Returns `true` if the mode is recognized.
    pub fn set_dec_mode(&mut self, mode: u16, enabled: bool) -> bool {
        let Some(flag) = Self::dec_flag_for_mode(mode) else {
            return false;
        };
        self.dec.set(flag, enabled);
        true
    }

    /// Query a DEC private mode by number.
    ///
    /// Returns:
    /// - `Some(true)` if the mode is recognized and set,
    /// - `Some(false)` if the mode is recognized and reset,
    /// - `None` if the mode number is unknown.
    #[must_use]
    pub fn dec_mode(&self, mode: u16) -> Option<bool> {
        let flag = Self::dec_flag_for_mode(mode)?;
        Some(self.dec.contains(flag))
    }

    /// Set an ANSI standard mode by its number.
    /// Returns `true` if the mode is recognized.
    pub fn set_ansi_mode(&mut self, mode: u16, enabled: bool) -> bool {
        let Some(flag) = Self::ansi_flag_for_mode(mode) else {
            return false;
        };
        self.ansi.set(flag, enabled);
        true
    }

    fn dec_flag_for_mode(mode: u16) -> Option<DecModes> {
        let flag = match mode {
            1 => DecModes::APPLICATION_CURSOR,
            6 => DecModes::ORIGIN,
            7 => DecModes::AUTOWRAP,
            25 => DecModes::CURSOR_VISIBLE,
            1000 => DecModes::MOUSE_BUTTON,
            1002 => DecModes::MOUSE_CELL_MOTION,
            1003 => DecModes::MOUSE_ALL_MOTION,
            1004 => DecModes::FOCUS_EVENTS,
            1006 => DecModes::MOUSE_SGR,
            1049 => DecModes::ALT_SCREEN,
            2004 => DecModes::BRACKETED_PASTE,
            2026 => DecModes::SYNC_OUTPUT,
            _ => return None,
        };
        Some(flag)
    }

    fn ansi_flag_for_mode(mode: u16) -> Option<AnsiModes> {
        let flag = match mode {
            4 => AnsiModes::INSERT,
            20 => AnsiModes::LINEFEED_NEWLINE,
            _ => return None,
        };
        Some(flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_autowrap_and_cursor_visible() {
        let m = Modes::new();
        assert!(m.autowrap());
        assert!(m.cursor_visible());
        assert!(!m.origin_mode());
        assert!(!m.insert_mode());
        assert!(!m.alt_screen());
        assert!(!m.bracketed_paste());
    }

    #[test]
    fn reset_restores_defaults() {
        let mut m = Modes::new();
        m.set_alt_screen(true);
        m.set_insert_mode(true);
        m.set_origin_mode(true);
        m.reset();
        assert!(m.autowrap());
        assert!(m.cursor_visible());
        assert!(!m.alt_screen());
        assert!(!m.insert_mode());
        assert!(!m.origin_mode());
    }

    #[test]
    fn toggle_origin_mode() {
        let mut m = Modes::new();
        m.set_origin_mode(true);
        assert!(m.origin_mode());
        m.set_origin_mode(false);
        assert!(!m.origin_mode());
    }

    #[test]
    fn set_dec_mode_by_number() {
        let mut m = Modes::new();
        assert!(m.set_dec_mode(25, false));
        assert!(!m.cursor_visible());
        assert!(m.set_dec_mode(25, true));
        assert!(m.cursor_visible());
    }

    #[test]
    fn set_dec_mode_unknown_returns_false() {
        let mut m = Modes::new();
        assert!(!m.set_dec_mode(9999, true));
    }

    #[test]
    fn dec_mode_by_number_reports_state() {
        let mut m = Modes::new();
        assert_eq!(m.dec_mode(7), Some(true));
        assert_eq!(m.dec_mode(1004), Some(false));
        assert_eq!(m.dec_mode(9999), None);
        assert!(m.set_dec_mode(1004, true));
        assert_eq!(m.dec_mode(1004), Some(true));
    }

    #[test]
    fn set_ansi_mode_by_number() {
        let mut m = Modes::new();
        assert!(m.set_ansi_mode(4, true));
        assert!(m.insert_mode());
        assert!(m.set_ansi_mode(4, false));
        assert!(!m.insert_mode());
    }

    #[test]
    fn mouse_modes() {
        let mut m = Modes::new();
        m.set_dec_mode(1000, true);
        assert!(m.dec.contains(DecModes::MOUSE_BUTTON));
        m.set_dec_mode(1006, true);
        assert!(m.dec.contains(DecModes::MOUSE_SGR));
    }

    #[test]
    fn alt_screen_and_bracketed_paste() {
        let mut m = Modes::new();
        m.set_alt_screen(true);
        m.set_bracketed_paste(true);
        assert!(m.alt_screen());
        assert!(m.bracketed_paste());
    }

    #[test]
    fn sync_output() {
        let mut m = Modes::new();
        assert!(!m.sync_output());
        m.set_sync_output(true);
        assert!(m.sync_output());
    }
}
