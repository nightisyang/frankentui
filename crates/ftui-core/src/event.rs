#![forbid(unsafe_code)]

//! Canonical input/event types.
//!
//! This module defines the standard event types used throughout ftui for
//! input handling. All events derive `Clone`, `PartialEq`, and `Eq` for
//! use in tests and pattern matching.
//!
//! # Design Notes
//!
//! - Mouse coordinates are 0-indexed (terminal is 1-indexed internally)
//! - `KeyEventKind` defaults to `Press` when not available from the terminal
//! - `Modifiers` use bitflags for easy combination
//! - Clipboard events are optional and feature-gated in the future

use bitflags::bitflags;

/// Canonical input event.
///
/// This enum represents all possible input events that ftui can receive
/// from the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A keyboard event.
    Key(KeyEvent),

    /// A mouse event.
    Mouse(MouseEvent),

    /// Terminal was resized.
    Resize {
        /// New terminal width in columns.
        width: u16,
        /// New terminal height in rows.
        height: u16,
    },

    /// Paste event (from bracketed paste mode).
    Paste(PasteEvent),

    /// Focus gained or lost.
    ///
    /// `true` = focus gained, `false` = focus lost.
    Focus(bool),

    /// Clipboard content received (optional, from OSC 52 response).
    Clipboard(ClipboardEvent),
}

/// A keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key code that was pressed.
    pub code: KeyCode,

    /// Modifier keys held during the event.
    pub modifiers: Modifiers,

    /// The type of key event (press, repeat, or release).
    pub kind: KeyEventKind,
}

impl KeyEvent {
    /// Create a new key event with default modifiers and Press kind.
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a key event with modifiers.
    #[must_use]
    pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Create a key event with a specific kind.
    #[must_use]
    pub const fn with_kind(mut self, kind: KeyEventKind) -> Self {
        self.kind = kind;
        self
    }

    /// Check if this is a specific character key.
    #[must_use]
    pub fn is_char(&self, c: char) -> bool {
        matches!(self.code, KeyCode::Char(ch) if ch == c)
    }

    /// Check if Ctrl modifier is held.
    #[must_use]
    pub const fn ctrl(&self) -> bool {
        self.modifiers.contains(Modifiers::CTRL)
    }

    /// Check if Alt modifier is held.
    #[must_use]
    pub const fn alt(&self) -> bool {
        self.modifiers.contains(Modifiers::ALT)
    }

    /// Check if Shift modifier is held.
    #[must_use]
    pub const fn shift(&self) -> bool {
        self.modifiers.contains(Modifiers::SHIFT)
    }

    /// Check if Super/Meta/Cmd modifier is held.
    #[must_use]
    pub const fn super_key(&self) -> bool {
        self.modifiers.contains(Modifiers::SUPER)
    }
}

/// Key codes for keyboard events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A regular character key.
    Char(char),

    /// Enter/Return key.
    Enter,

    /// Escape key.
    Escape,

    /// Backspace key.
    Backspace,

    /// Tab key.
    Tab,

    /// Shift+Tab (back-tab).
    BackTab,

    /// Delete key.
    Delete,

    /// Insert key.
    Insert,

    /// Home key.
    Home,

    /// End key.
    End,

    /// Page Up key.
    PageUp,

    /// Page Down key.
    PageDown,

    /// Up arrow key.
    Up,

    /// Down arrow key.
    Down,

    /// Left arrow key.
    Left,

    /// Right arrow key.
    Right,

    /// Function key (F1-F24).
    F(u8),

    /// Null character (Ctrl+Space or Ctrl+@).
    Null,

    /// Media key: Play/Pause.
    MediaPlayPause,

    /// Media key: Stop.
    MediaStop,

    /// Media key: Next track.
    MediaNextTrack,

    /// Media key: Previous track.
    MediaPrevTrack,
}

/// The type of key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyEventKind {
    /// Key was pressed (default when not distinguishable).
    #[default]
    Press,

    /// Key is being held (repeat event).
    Repeat,

    /// Key was released.
    Release,
}

bitflags! {
    /// Modifier keys that can be held during a key event.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        /// No modifiers.
        const NONE  = 0b0000;
        /// Shift key.
        const SHIFT = 0b0001;
        /// Alt/Option key.
        const ALT   = 0b0010;
        /// Control key.
        const CTRL  = 0b0100;
        /// Super/Meta/Command key.
        const SUPER = 0b1000;
    }
}

impl Default for Modifiers {
    fn default() -> Self {
        Self::NONE
    }
}

/// A mouse event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// The type of mouse event.
    pub kind: MouseEventKind,

    /// X coordinate (0-indexed, leftmost column is 0).
    pub x: u16,

    /// Y coordinate (0-indexed, topmost row is 0).
    pub y: u16,

    /// Modifier keys held during the event.
    pub modifiers: Modifiers,
}

impl MouseEvent {
    /// Create a new mouse event.
    #[must_use]
    pub const fn new(kind: MouseEventKind, x: u16, y: u16) -> Self {
        Self {
            kind,
            x,
            y,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a mouse event with modifiers.
    #[must_use]
    pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Get the position as a tuple.
    #[must_use]
    pub const fn position(&self) -> (u16, u16) {
        (self.x, self.y)
    }
}

/// The type of mouse event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    /// Mouse button pressed down.
    Down(MouseButton),

    /// Mouse button released.
    Up(MouseButton),

    /// Mouse dragged while button held.
    Drag(MouseButton),

    /// Mouse moved (no button pressed).
    Moved,

    /// Mouse wheel scrolled up.
    ScrollUp,

    /// Mouse wheel scrolled down.
    ScrollDown,

    /// Mouse wheel scrolled left (horizontal scroll).
    ScrollLeft,

    /// Mouse wheel scrolled right (horizontal scroll).
    ScrollRight,
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,

    /// Right mouse button.
    Right,

    /// Middle mouse button (scroll wheel click).
    Middle,
}

/// A paste event from bracketed paste mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteEvent {
    /// The pasted text content.
    pub text: String,

    /// True if this came from bracketed paste mode.
    ///
    /// When true, the text was received atomically and should be
    /// treated as a single paste operation rather than individual
    /// key presses.
    pub bracketed: bool,
}

impl PasteEvent {
    /// Create a new paste event.
    #[must_use]
    pub fn new(text: impl Into<String>, bracketed: bool) -> Self {
        Self {
            text: text.into(),
            bracketed,
        }
    }

    /// Create a bracketed paste event (the common case).
    #[must_use]
    pub fn bracketed(text: impl Into<String>) -> Self {
        Self::new(text, true)
    }
}

/// A clipboard event from OSC 52 response.
///
/// This is optional and may not be supported by all terminals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEvent {
    /// The clipboard content (decoded from base64).
    pub content: String,

    /// The source of the clipboard content.
    pub source: ClipboardSource,
}

impl ClipboardEvent {
    /// Create a new clipboard event.
    #[must_use]
    pub fn new(content: impl Into<String>, source: ClipboardSource) -> Self {
        Self {
            content: content.into(),
            source,
        }
    }
}

/// The source of clipboard content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClipboardSource {
    /// Clipboard content from OSC 52 protocol.
    Osc52,

    /// Unknown or unspecified source.
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_event_is_char() {
        let event = KeyEvent::new(KeyCode::Char('q'));
        assert!(event.is_char('q'));
        assert!(!event.is_char('x'));
    }

    #[test]
    fn key_event_modifiers() {
        let event = KeyEvent::new(KeyCode::Char('c')).with_modifiers(Modifiers::CTRL);
        assert!(event.ctrl());
        assert!(!event.alt());
        assert!(!event.shift());
        assert!(!event.super_key());
    }

    #[test]
    fn key_event_combined_modifiers() {
        let event =
            KeyEvent::new(KeyCode::Char('s')).with_modifiers(Modifiers::CTRL | Modifiers::SHIFT);
        assert!(event.ctrl());
        assert!(event.shift());
        assert!(!event.alt());
    }

    #[test]
    fn key_event_kind() {
        let press = KeyEvent::new(KeyCode::Enter);
        assert_eq!(press.kind, KeyEventKind::Press);

        let release = press.with_kind(KeyEventKind::Release);
        assert_eq!(release.kind, KeyEventKind::Release);
    }

    #[test]
    fn mouse_event_position() {
        let event = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 10, 20);
        assert_eq!(event.position(), (10, 20));
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 20);
    }

    #[test]
    fn mouse_event_with_modifiers() {
        let event = MouseEvent::new(MouseEventKind::Moved, 0, 0).with_modifiers(Modifiers::ALT);
        assert_eq!(event.modifiers, Modifiers::ALT);
    }

    #[test]
    fn paste_event_creation() {
        let paste = PasteEvent::bracketed("hello world");
        assert_eq!(paste.text, "hello world");
        assert!(paste.bracketed);
    }

    #[test]
    fn clipboard_event_creation() {
        let clip = ClipboardEvent::new("copied text", ClipboardSource::Osc52);
        assert_eq!(clip.content, "copied text");
        assert_eq!(clip.source, ClipboardSource::Osc52);
    }

    #[test]
    fn event_variants() {
        // Test that all event variants can be created
        let _key = Event::Key(KeyEvent::new(KeyCode::Char('a')));
        let _mouse = Event::Mouse(MouseEvent::new(
            MouseEventKind::Down(MouseButton::Left),
            0,
            0,
        ));
        let _resize = Event::Resize {
            width: 80,
            height: 24,
        };
        let _paste = Event::Paste(PasteEvent::bracketed("test"));
        let _focus = Event::Focus(true);
        let _clipboard = Event::Clipboard(ClipboardEvent::new("test", ClipboardSource::Unknown));
    }

    #[test]
    fn modifiers_default() {
        assert_eq!(Modifiers::default(), Modifiers::NONE);
    }

    #[test]
    fn key_event_kind_default() {
        assert_eq!(KeyEventKind::default(), KeyEventKind::Press);
    }

    #[test]
    fn clipboard_source_default() {
        assert_eq!(ClipboardSource::default(), ClipboardSource::Unknown);
    }

    #[test]
    fn function_keys() {
        let f1 = KeyEvent::new(KeyCode::F(1));
        let f12 = KeyEvent::new(KeyCode::F(12));
        assert_eq!(f1.code, KeyCode::F(1));
        assert_eq!(f12.code, KeyCode::F(12));
    }

    #[test]
    fn event_is_clone_and_eq() {
        let event = Event::Key(KeyEvent::new(KeyCode::Char('x')));
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
