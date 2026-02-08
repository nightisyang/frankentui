#![forbid(unsafe_code)]

//! Deterministic, JSON-friendly input schema for `frankenterm-web`.
//!
//! The web host (JS/TS) is expected to provide:
//! - cell coordinates for pointer/touch events, and
//! - quantized (`i16`) wheel deltas (already normalized for determinism).
//!
//! This module focuses on:
//! - stable key-code normalization (DOM `key`/`code` → [`KeyCode`]),
//! - a compact modifier bitset (`mods: u8`) for logs/traces, and
//! - JSON encoding suitable for record/replay.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// Modifier keys held during an input event.
    ///
    /// These flags are encoded as a compact `u8` bitset in JSON (`mods`).
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const SHIFT = 0b0001;
        const ALT   = 0b0010;
        const CTRL  = 0b0100;
        const SUPER = 0b1000;
    }
}

impl Modifiers {
    #[must_use]
    pub const fn from_bits_truncate_u8(bits: u8) -> Self {
        Self::from_bits_truncate(bits)
    }
}

/// Phase for key events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyPhase {
    Down,
    Up,
}

/// Phase for mouse events in cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MousePhase {
    Down,
    Up,
    Move,
    Drag,
}

/// Phase for IME composition events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionPhase {
    Start,
    Update,
    End,
    Cancel,
}

/// Phase for touch events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

/// Normalized key code for deterministic record/replay.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
    BackTab,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Unidentified { key: Box<str>, code: Box<str> },
}

impl KeyCode {
    #[must_use]
    pub fn to_code_string(&self) -> String {
        match self {
            Self::Char(c) => c.to_string(),
            Self::Enter => "Enter".to_string(),
            Self::Escape => "Escape".to_string(),
            Self::Backspace => "Backspace".to_string(),
            Self::Tab => "Tab".to_string(),
            Self::BackTab => "BackTab".to_string(),
            Self::Delete => "Delete".to_string(),
            Self::Insert => "Insert".to_string(),
            Self::Home => "Home".to_string(),
            Self::End => "End".to_string(),
            Self::PageUp => "PageUp".to_string(),
            Self::PageDown => "PageDown".to_string(),
            Self::Up => "Up".to_string(),
            Self::Down => "Down".to_string(),
            Self::Left => "Left".to_string(),
            Self::Right => "Right".to_string(),
            Self::F(n) => format!("F{n}"),
            Self::Unidentified { .. } => "Unidentified".to_string(),
        }
    }

    #[must_use]
    pub fn from_code_string(code: &str, raw_key: Option<&str>, raw_code: Option<&str>) -> Self {
        match code {
            "Enter" => Self::Enter,
            "Escape" => Self::Escape,
            "Backspace" => Self::Backspace,
            "Tab" => Self::Tab,
            "BackTab" => Self::BackTab,
            "Delete" => Self::Delete,
            "Insert" => Self::Insert,
            "Home" => Self::Home,
            "End" => Self::End,
            "PageUp" => Self::PageUp,
            "PageDown" => Self::PageDown,
            "Up" => Self::Up,
            "Down" => Self::Down,
            "Left" => Self::Left,
            "Right" => Self::Right,
            "Unidentified" => Self::Unidentified {
                key: raw_key.unwrap_or("").into(),
                code: raw_code.unwrap_or("").into(),
            },
            _ => {
                if let Some(n) = parse_function_key(code) {
                    return Self::F(n);
                }

                let mut chars = code.chars();
                let Some(first) = chars.next() else {
                    return Self::Unidentified {
                        key: raw_key.unwrap_or("").into(),
                        code: raw_code.unwrap_or("").into(),
                    };
                };
                if chars.next().is_none() {
                    Self::Char(first)
                } else {
                    Self::Unidentified {
                        key: raw_key.unwrap_or(code).into(),
                        code: raw_code.unwrap_or("").into(),
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Other(u8),
}

impl MouseButton {
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
            Self::Other(n) => n,
        }
    }

    #[must_use]
    pub const fn from_u8(n: u8) -> Self {
        match n {
            0 => Self::Left,
            1 => Self::Middle,
            2 => Self::Right,
            other => Self::Other(other),
        }
    }
}

/// Normalized key input event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyInput {
    pub phase: KeyPhase,
    pub code: KeyCode,
    pub mods: Modifiers,
    pub repeat: bool,
}

/// Normalized mouse input event in cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseInput {
    pub phase: MousePhase,
    pub button: Option<MouseButton>,
    pub x: u16,
    pub y: u16,
    pub mods: Modifiers,
}

/// Normalized wheel input event (deterministic integer deltas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WheelInput {
    pub x: u16,
    pub y: u16,
    pub dx: i16,
    pub dy: i16,
    pub mods: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TouchPoint {
    pub id: u32,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TouchInput {
    pub phase: TouchPhase,
    pub touches: Vec<TouchPoint>,
    pub mods: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompositionInput {
    pub phase: CompositionPhase,
    pub data: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FocusInput {
    pub focused: bool,
}

/// Normalized, deterministic web input event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputEvent {
    Key(KeyInput),
    Mouse(MouseInput),
    Wheel(WheelInput),
    Touch(TouchInput),
    Composition(CompositionInput),
    Focus(FocusInput),
}

/// Minimal modifier tracker used to guarantee "no stuck modifiers" after focus loss.
#[derive(Debug, Default, Clone)]
pub struct ModifierTracker {
    current: Modifiers,
}

impl ModifierTracker {
    #[must_use]
    pub const fn current(&self) -> Modifiers {
        self.current
    }

    pub fn handle_focus(&mut self, focused: bool) {
        if !focused {
            self.current = Modifiers::empty();
        }
    }

    pub fn reconcile(&mut self, seen: Modifiers) {
        self.current = seen;
    }
}

/// Deterministic normalization of DOM key/code strings into a [`KeyCode`].
#[must_use]
pub fn normalize_dom_key_code(dom_key: &str, dom_code: &str, mods: Modifiers) -> KeyCode {
    // Shift+Tab should be represented explicitly.
    if dom_key == "Tab" && mods.contains(Modifiers::SHIFT) {
        return KeyCode::BackTab;
    }

    // Prefer the logical `key` for printable characters (already includes shift).
    let mut chars = dom_key.chars();
    if let Some(first) = chars.next()
        && chars.next().is_none()
    {
        return KeyCode::Char(first);
    }

    match dom_key {
        "Enter" => KeyCode::Enter,
        "Escape" | "Esc" => KeyCode::Escape,
        "Backspace" => KeyCode::Backspace,
        "Tab" => KeyCode::Tab,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        "Spacebar" => KeyCode::Char(' '),
        _ => {
            if let Some(n) = parse_function_key(dom_key) {
                return KeyCode::F(n);
            }

            // Fallback to DOM `code` for non-printable keys.
            if let Some(code) = key_code_from_dom_code(dom_code, mods) {
                return code;
            }

            KeyCode::Unidentified {
                key: dom_key.into(),
                code: dom_code.into(),
            }
        }
    }
}

fn parse_function_key(s: &str) -> Option<u8> {
    let rest = s.strip_prefix('F')?;
    rest.parse::<u8>().ok().filter(|n| (1..=24).contains(n))
}

fn key_code_from_dom_code(dom_code: &str, mods: Modifiers) -> Option<KeyCode> {
    // Support the `code` form for BackTab as well (some wrappers may pass it).
    if dom_code == "Tab" && mods.contains(Modifiers::SHIFT) {
        return Some(KeyCode::BackTab);
    }

    Some(match dom_code {
        "Enter" | "NumpadEnter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Backspace" => KeyCode::Backspace,
        "Tab" => KeyCode::Tab,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        _ => {
            return None;
        }
    })
}

/// JSON encoding used by `ftui-web` and golden traces.
///
/// This is intentionally small and stable: a `kind` tag plus the minimum
/// semantic fields needed for replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputEventJson {
    Key {
        phase: KeyPhase,
        code: String,
        mods: u8,
        repeat: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw_code: Option<String>,
    },
    Mouse {
        phase: MousePhase,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        button: Option<u8>,
        x: u16,
        y: u16,
        mods: u8,
    },
    Wheel {
        x: u16,
        y: u16,
        dx: i16,
        dy: i16,
        mods: u8,
    },
    Touch {
        phase: TouchPhase,
        touches: Vec<TouchPoint>,
        mods: u8,
    },
    Composition {
        phase: CompositionPhase,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<String>,
    },
    Focus {
        focused: bool,
    },
}

impl InputEvent {
    /// Encode this event as a stable JSON string.
    ///
    /// Errors can occur only if serialization fails (for example, due to an
    /// internal `serde_json` formatting error).
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&InputEventJson::from(self))
    }

    /// Decode a previously encoded event JSON string.
    ///
    /// Errors occur if the JSON does not match the expected schema.
    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        let json: InputEventJson = serde_json::from_str(s)?;
        Ok(Self::from(json))
    }
}

impl From<&InputEvent> for InputEventJson {
    fn from(value: &InputEvent) -> Self {
        match value {
            InputEvent::Key(key) => {
                let (code, raw_key, raw_code) = match &key.code {
                    KeyCode::Unidentified { key, code } => (
                        "Unidentified".to_string(),
                        Some(key.to_string()),
                        Some(code.to_string()),
                    ),
                    other => (other.to_code_string(), None, None),
                };
                Self::Key {
                    phase: key.phase,
                    code,
                    mods: key.mods.bits(),
                    repeat: key.repeat,
                    raw_key,
                    raw_code,
                }
            }
            InputEvent::Mouse(mouse) => Self::Mouse {
                phase: mouse.phase,
                button: mouse.button.map(MouseButton::to_u8),
                x: mouse.x,
                y: mouse.y,
                mods: mouse.mods.bits(),
            },
            InputEvent::Wheel(wheel) => Self::Wheel {
                x: wheel.x,
                y: wheel.y,
                dx: wheel.dx,
                dy: wheel.dy,
                mods: wheel.mods.bits(),
            },
            InputEvent::Touch(touch) => Self::Touch {
                phase: touch.phase,
                touches: touch.touches.clone(),
                mods: touch.mods.bits(),
            },
            InputEvent::Composition(comp) => Self::Composition {
                phase: comp.phase,
                data: comp.data.as_deref().map(str::to_string),
            },
            InputEvent::Focus(f) => Self::Focus { focused: f.focused },
        }
    }
}

impl From<InputEventJson> for InputEvent {
    fn from(value: InputEventJson) -> Self {
        match value {
            InputEventJson::Key {
                phase,
                code,
                mods,
                repeat,
                raw_key,
                raw_code,
            } => Self::Key(KeyInput {
                phase,
                code: KeyCode::from_code_string(&code, raw_key.as_deref(), raw_code.as_deref()),
                mods: Modifiers::from_bits_truncate_u8(mods),
                repeat,
            }),
            InputEventJson::Mouse {
                phase,
                button,
                x,
                y,
                mods,
            } => Self::Mouse(MouseInput {
                phase,
                button: button.map(MouseButton::from_u8),
                x,
                y,
                mods: Modifiers::from_bits_truncate_u8(mods),
            }),
            InputEventJson::Wheel { x, y, dx, dy, mods } => Self::Wheel(WheelInput {
                x,
                y,
                dx,
                dy,
                mods: Modifiers::from_bits_truncate_u8(mods),
            }),
            InputEventJson::Touch {
                phase,
                touches,
                mods,
            } => Self::Touch(TouchInput {
                phase,
                touches,
                mods: Modifiers::from_bits_truncate_u8(mods),
            }),
            InputEventJson::Composition { phase, data } => Self::Composition(CompositionInput {
                phase,
                data: data.map(Into::into),
            }),
            InputEventJson::Focus { focused } => Self::Focus(FocusInput { focused }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn map_dom_key_specials() {
        let mods = Modifiers::empty();
        assert_eq!(
            normalize_dom_key_code("Enter", "Enter", mods),
            KeyCode::Enter
        );
        assert_eq!(
            normalize_dom_key_code("ArrowLeft", "ArrowLeft", mods),
            KeyCode::Left
        );
        assert_eq!(normalize_dom_key_code("F12", "F12", mods), KeyCode::F(12));
    }

    #[test]
    fn shift_tab_is_backtab() {
        let mods = Modifiers::SHIFT;
        assert_eq!(normalize_dom_key_code("Tab", "Tab", mods), KeyCode::BackTab);
    }

    #[test]
    fn key_event_json_roundtrip_is_stable() {
        let ev = InputEvent::Key(KeyInput {
            phase: KeyPhase::Down,
            code: KeyCode::Char('a'),
            mods: Modifiers::empty(),
            repeat: false,
        });
        let j1 = ev.to_json_string().expect("serialize");
        let j2 = ev.to_json_string().expect("serialize");
        assert_eq!(j1, j2);
        let back = InputEvent::from_json_str(&j1).expect("deserialize");
        assert_eq!(ev, back);
    }

    proptest! {
        #[test]
        fn modifier_tracker_focus_loss_is_idempotent(events in prop::collection::vec(any::<u8>(), 1..200)) {
            let mut tracker = ModifierTracker::default();
            for mods in events {
                tracker.reconcile(Modifiers::from_bits_truncate_u8(mods));
            }
            tracker.handle_focus(false);
            tracker.handle_focus(false);
            prop_assert_eq!(tracker.current(), Modifiers::empty());
        }
    }
}
