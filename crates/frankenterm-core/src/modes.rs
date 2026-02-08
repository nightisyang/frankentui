//! Terminal modes (ANSI + DEC private).
//!
//! This module models the mode bits that influence how the terminal engine
//! mutates the grid (origin mode, autowrap, insert mode, etc.).
//!
//! The intent is to keep this as pure state with small helpers so that the
//! VT/ANSI parser can toggle modes deterministically.

use bitflags::bitflags;

bitflags! {
    /// Mode flags for the terminal engine.
    ///
    /// This is intentionally incomplete in the API skeleton; we will expand as
    /// the support matrix is implemented.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ModeFlags: u32 {
        /// DEC origin mode (DECOM): cursor addressing is relative to the scroll region.
        const ORIGIN = 1 << 0;
        /// Automatic line wrap at the right margin (DECAWM).
        const AUTOWRAP = 1 << 1;
        /// Insert mode (IRM): printed characters shift existing content to the right.
        const INSERT = 1 << 2;
        /// Application cursor keys (DECCKM).
        const APPLICATION_CURSOR = 1 << 3;
    }
}

/// Mode state for the terminal engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modes {
    flags: ModeFlags,
}

impl Modes {
    /// Construct default modes (typical xterm defaults).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Raw access to the underlying flags.
    #[must_use]
    pub fn flags(&self) -> ModeFlags {
        self.flags
    }

    /// Overwrite mode flags.
    pub fn set_flags(&mut self, flags: ModeFlags) {
        self.flags = flags;
    }

    /// Whether origin mode is enabled.
    #[must_use]
    pub fn origin_mode(&self) -> bool {
        self.flags.contains(ModeFlags::ORIGIN)
    }

    /// Enable/disable origin mode.
    pub fn set_origin_mode(&mut self, enabled: bool) {
        self.flags.set(ModeFlags::ORIGIN, enabled);
    }

    /// Whether autowrap is enabled.
    #[must_use]
    pub fn autowrap(&self) -> bool {
        self.flags.contains(ModeFlags::AUTOWRAP)
    }

    /// Enable/disable autowrap.
    pub fn set_autowrap(&mut self, enabled: bool) {
        self.flags.set(ModeFlags::AUTOWRAP, enabled);
    }

    /// Whether insert mode is enabled.
    #[must_use]
    pub fn insert_mode(&self) -> bool {
        self.flags.contains(ModeFlags::INSERT)
    }

    /// Enable/disable insert mode.
    pub fn set_insert_mode(&mut self, enabled: bool) {
        self.flags.set(ModeFlags::INSERT, enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_default_is_stable() {
        let m = Modes::new();
        assert!(!m.origin_mode());
        assert!(!m.insert_mode());
        // Autowrap default is intentionally left as false in the skeleton; we
        // can revisit once the support matrix is implemented.
        assert!(!m.autowrap());
    }

    #[test]
    fn toggle_origin_mode() {
        let mut m = Modes::new();
        m.set_origin_mode(true);
        assert!(m.origin_mode());
        m.set_origin_mode(false);
        assert!(!m.origin_mode());
    }
}
