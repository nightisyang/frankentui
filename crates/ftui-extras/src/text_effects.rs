#![forbid(unsafe_code)]

//! Animated text effects for terminal UI.
//!
//! This module provides a rich set of text animation and styling effects:
//!
//! - **Fade effects**: Smooth fade-in, fade-out, pulse
//! - **Gradient fills**: Horizontal, vertical, diagonal, radial gradients
//! - **Animated gradients**: Moving gradient patterns
//! - **Color cycling**: Rainbow, breathing, wave effects
//! - **Style animations**: Blinking, bold/dim toggle, underline wave
//! - **Character effects**: Typing, scramble, glitch
//! - **Transition overlays**: Full-screen announcement effects
//!
//! # Example
//!
//! ```rust,ignore
//! use ftui_extras::text_effects::{StyledText, TextEffect, TransitionOverlay};
//!
//! // Rainbow gradient text
//! let rainbow = StyledText::new("Hello World")
//!     .effect(TextEffect::RainbowGradient { speed: 0.1 })
//!     .time(current_time);
//!
//! // Fade-in text
//! let fading = StyledText::new("Appearing...")
//!     .effect(TextEffect::FadeIn { progress: 0.5 });
//!
//! // Pulsing glow
//! let pulse = StyledText::new("IMPORTANT")
//!     .effect(TextEffect::Pulse { speed: 2.0, min_alpha: 0.3 })
//!     .base_color(PackedRgba::rgb(255, 100, 100))
//!     .time(current_time);
//! ```

use std::f64::consts::{PI, TAU};

use ftui_core::geometry::Rect;
use ftui_render::cell::{CellAttrs, CellContent, PackedRgba, StyleFlags as CellStyleFlags};
use ftui_render::frame::Frame;
use ftui_widgets::Widget;

// =============================================================================
// Color Utilities
// =============================================================================

/// Interpolate between two colors.
pub fn lerp_color(a: PackedRgba, b: PackedRgba, t: f64) -> PackedRgba {
    let t = t.clamp(0.0, 1.0);
    let r = (a.r() as f64 + (b.r() as f64 - a.r() as f64) * t) as u8;
    let g = (a.g() as f64 + (b.g() as f64 - a.g() as f64) * t) as u8;
    let b_val = (a.b() as f64 + (b.b() as f64 - a.b() as f64) * t) as u8;
    PackedRgba::rgb(r, g, b_val)
}

/// Apply alpha/brightness to a color.
pub fn apply_alpha(color: PackedRgba, alpha: f64) -> PackedRgba {
    let alpha = alpha.clamp(0.0, 1.0);
    PackedRgba::rgb(
        (color.r() as f64 * alpha) as u8,
        (color.g() as f64 * alpha) as u8,
        (color.b() as f64 * alpha) as u8,
    )
}

/// Convert HSV to RGB.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> PackedRgba {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    PackedRgba::rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Multi-stop color gradient.
#[derive(Debug, Clone)]
pub struct ColorGradient {
    stops: Vec<(f64, PackedRgba)>,
}

impl ColorGradient {
    /// Create a new gradient with color stops.
    /// Stops should be tuples of (position, color) where position is 0.0 to 1.0.
    pub fn new(stops: Vec<(f64, PackedRgba)>) -> Self {
        let mut stops = stops;
        stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Self { stops }
    }

    /// Create a rainbow gradient.
    pub fn rainbow() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(255, 0, 0)),    // Red
            (0.17, PackedRgba::rgb(255, 127, 0)), // Orange
            (0.33, PackedRgba::rgb(255, 255, 0)), // Yellow
            (0.5, PackedRgba::rgb(0, 255, 0)),    // Green
            (0.67, PackedRgba::rgb(0, 127, 255)), // Blue
            (0.83, PackedRgba::rgb(127, 0, 255)), // Indigo
            (1.0, PackedRgba::rgb(255, 0, 255)),  // Violet
        ])
    }

    /// Create a sunset gradient (purple -> pink -> orange -> yellow).
    pub fn sunset() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(80, 20, 120)),
            (0.33, PackedRgba::rgb(255, 50, 120)),
            (0.66, PackedRgba::rgb(255, 150, 50)),
            (1.0, PackedRgba::rgb(255, 255, 150)),
        ])
    }

    /// Create an ocean gradient (deep blue -> cyan -> seafoam).
    pub fn ocean() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(10, 30, 100)),
            (0.5, PackedRgba::rgb(30, 180, 220)),
            (1.0, PackedRgba::rgb(150, 255, 200)),
        ])
    }

    /// Create a cyberpunk gradient (hot pink -> purple -> cyan).
    pub fn cyberpunk() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(255, 20, 150)),
            (0.5, PackedRgba::rgb(150, 50, 200)),
            (1.0, PackedRgba::rgb(50, 220, 255)),
        ])
    }

    /// Create a fire gradient (black -> red -> orange -> yellow -> white).
    pub fn fire() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(0, 0, 0)),
            (0.2, PackedRgba::rgb(80, 10, 0)),
            (0.4, PackedRgba::rgb(200, 50, 0)),
            (0.6, PackedRgba::rgb(255, 150, 20)),
            (0.8, PackedRgba::rgb(255, 230, 100)),
            (1.0, PackedRgba::rgb(255, 255, 220)),
        ])
    }

    /// Sample the gradient at position t (0.0 to 1.0).
    pub fn sample(&self, t: f64) -> PackedRgba {
        let t = t.clamp(0.0, 1.0);

        if self.stops.is_empty() {
            return PackedRgba::rgb(255, 255, 255);
        }
        if self.stops.len() == 1 {
            return self.stops[0].1;
        }

        // Find the two stops we're between
        let mut prev = &self.stops[0];
        for stop in &self.stops {
            if stop.0 >= t {
                if stop.0 == prev.0 {
                    return stop.1;
                }
                let local_t = (t - prev.0) / (stop.0 - prev.0);
                return lerp_color(prev.1, stop.1, local_t);
            }
            prev = stop;
        }

        self.stops
            .last()
            .map(|s| s.1)
            .unwrap_or(PackedRgba::rgb(255, 255, 255))
    }
}

// =============================================================================
// Easing Functions - Animation curve system
// =============================================================================

/// Easing functions for smooth, professional animations.
///
/// Most curves output values in the 0.0-1.0 range, but some (Elastic, Back)
/// can overshoot outside this range for spring/bounce effects. Code using
/// easing should handle values outside 0-1 gracefully (clamp colors, etc.).
///
/// # Performance
/// All `apply()` calls are < 100ns (no allocations, pure math).
///
/// # Example
/// ```ignore
/// let progress = 0.5;
/// let eased = Easing::EaseInOut.apply(progress);
/// // eased ≈ 0.5 but with smooth acceleration/deceleration
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Easing {
    /// Linear interpolation: `t` (no easing).
    #[default]
    Linear,

    // --- Cubic curves (smooth, professional) ---
    /// Slow start, accelerating: `t³`
    EaseIn,
    /// Slow end, decelerating: `1 - (1-t)³`
    EaseOut,
    /// Smooth S-curve: slow start and end.
    EaseInOut,

    // --- Quadratic curves (subtler than cubic) ---
    /// Subtle slow start: `t²`
    EaseInQuad,
    /// Subtle slow end: `1 - (1-t)²`
    EaseOutQuad,
    /// Subtle S-curve.
    EaseInOutQuad,

    // --- Playful/dynamic curves ---
    /// Ball bounce effect at end.
    Bounce,
    /// Spring with overshoot. **WARNING: Can exceed 1.0!**
    Elastic,
    /// Slight overshoot then settle. **WARNING: Can go < 0 and > 1!**
    Back,

    // --- Discrete ---
    /// Discrete steps. `Step(4)` outputs {0, 0.25, 0.5, 0.75, 1.0}.
    Step(u8),
}

impl Easing {
    /// Apply the easing function to a progress value.
    ///
    /// # Arguments
    /// * `t` - Progress value (clamped to 0.0-1.0 internally)
    ///
    /// # Returns
    /// The eased value. Most curves return 0.0-1.0, but `Elastic` and `Back`
    /// can briefly exceed these bounds for spring/overshoot effects.
    ///
    /// # Performance
    /// < 100ns per call (pure math, no allocations).
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Self::Linear => t,

            // Cubic curves
            Self::EaseIn => t * t * t,
            Self::EaseOut => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let inv = -2.0 * t + 2.0;
                    1.0 - inv * inv * inv / 2.0
                }
            }

            // Quadratic curves
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => {
                let inv = 1.0 - t;
                1.0 - inv * inv
            }
            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    let inv = -2.0 * t + 2.0;
                    1.0 - inv * inv / 2.0
                }
            }

            // Bounce - ball bouncing at end
            Self::Bounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                let mut t = t;

                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    t -= 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    t -= 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    t -= 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }

            // Elastic - spring with overshoot (CAN EXCEED 1.0!)
            Self::Elastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = TAU / 3.0;
                    2.0_f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }

            // Back - overshoot then settle (CAN GO < 0 AND > 1!)
            // Uses easeOutBack formula: 1 + c3 * (t-1)^3 + c1 * (t-1)^2
            Self::Back => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                let t_minus_1 = t - 1.0;
                1.0 + c3 * t_minus_1 * t_minus_1 * t_minus_1 + c1 * t_minus_1 * t_minus_1
            }

            // Step - discrete steps. Step(n) outputs n+1 values: {0, 1/n, 2/n, ..., 1}
            Self::Step(steps) => {
                if *steps == 0 {
                    t
                } else {
                    let s = *steps as f64;
                    (t * s).round() / s
                }
            }
        }
    }

    /// Check if this easing can produce values outside 0.0-1.0.
    pub fn can_overshoot(&self) -> bool {
        matches!(self, Self::Elastic | Self::Back)
    }

    /// Get a human-readable name for the easing function.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::EaseIn => "Ease In (Cubic)",
            Self::EaseOut => "Ease Out (Cubic)",
            Self::EaseInOut => "Ease In-Out (Cubic)",
            Self::EaseInQuad => "Ease In (Quad)",
            Self::EaseOutQuad => "Ease Out (Quad)",
            Self::EaseInOutQuad => "Ease In-Out (Quad)",
            Self::Bounce => "Bounce",
            Self::Elastic => "Elastic",
            Self::Back => "Back",
            Self::Step(_) => "Step",
        }
    }
}

// =============================================================================
// Animation Timing - Frame-rate independent animation clock
// =============================================================================

/// Animation clock for time-based effects.
///
/// Provides a unified timing system with:
/// - Frame-rate independence via delta-time calculation
/// - Global speed control (pause/resume/slow-motion)
/// - Consistent time units (seconds)
///
/// # Speed Convention
/// All effects use **cycles per second**:
/// - `speed: 1.0` = one full cycle per second
/// - `speed: 0.5` = one cycle every 2 seconds
/// - `speed: 2.0` = two cycles per second
///
/// # Example
/// ```ignore
/// let mut clock = AnimationClock::new();
/// loop {
///     clock.tick(); // Call once per frame
///     let t = clock.time();
///     let styled = StyledText::new("Hello").time(t);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AnimationClock {
    /// Current animation time in seconds.
    time: f64,
    /// Time multiplier (1.0 = normal, 0.0 = paused, 0.5 = half-speed).
    speed: f64,
    /// Last tick instant for delta calculation.
    last_tick: std::time::Instant,
}

impl Default for AnimationClock {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationClock {
    /// Create a new animation clock starting at time 0.
    #[inline]
    pub fn new() -> Self {
        Self {
            time: 0.0,
            speed: 1.0,
            last_tick: std::time::Instant::now(),
        }
    }

    /// Create a clock with a specific start time.
    #[inline]
    pub fn with_time(time: f64) -> Self {
        Self {
            time,
            speed: 1.0,
            last_tick: std::time::Instant::now(),
        }
    }

    /// Advance the clock by elapsed real time since last tick.
    ///
    /// Call this once per frame. The time advancement respects the current
    /// speed multiplier, enabling pause/slow-motion effects.
    #[inline]
    pub fn tick(&mut self) {
        let now = std::time::Instant::now();
        let delta = now.duration_since(self.last_tick).as_secs_f64();
        self.time += delta * self.speed;
        self.last_tick = now;
    }

    /// Advance the clock by a specific delta time.
    ///
    /// Use this for deterministic testing or when you control the time step.
    #[inline]
    pub fn tick_delta(&mut self, delta_seconds: f64) {
        self.time += delta_seconds * self.speed;
        self.last_tick = std::time::Instant::now();
    }

    /// Get the current animation time in seconds.
    #[inline]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Set the current time directly.
    #[inline]
    pub fn set_time(&mut self, time: f64) {
        self.time = time;
    }

    /// Get the current speed multiplier.
    #[inline]
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Set the speed multiplier.
    ///
    /// - `1.0` = normal speed
    /// - `0.0` = paused
    /// - `0.5` = half speed
    /// - `2.0` = double speed
    #[inline]
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.max(0.0);
    }

    /// Pause the animation (equivalent to `set_speed(0.0)`).
    #[inline]
    pub fn pause(&mut self) {
        self.speed = 0.0;
    }

    /// Resume the animation at normal speed (equivalent to `set_speed(1.0)`).
    #[inline]
    pub fn resume(&mut self) {
        self.speed = 1.0;
    }

    /// Check if the clock is paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.speed == 0.0
    }

    /// Reset the clock to time 0.
    #[inline]
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.last_tick = std::time::Instant::now();
    }

    /// Get elapsed time since a given start time (useful for relative animations).
    #[inline]
    pub fn elapsed_since(&self, start_time: f64) -> f64 {
        (self.time - start_time).max(0.0)
    }

    /// Calculate a cyclic phase for periodic animations.
    ///
    /// Returns a value in `0.0..1.0` that cycles at the given frequency.
    ///
    /// # Arguments
    /// * `cycles_per_second` - How many full cycles per second
    ///
    /// # Example
    /// ```ignore
    /// // Pulse that completes 2 cycles per second
    /// let phase = clock.phase(2.0);
    /// let brightness = 0.5 + 0.5 * (phase * TAU).sin();
    /// ```
    #[inline]
    pub fn phase(&self, cycles_per_second: f64) -> f64 {
        if cycles_per_second <= 0.0 {
            return 0.0;
        }
        (self.time * cycles_per_second).fract()
    }
}

// =============================================================================
// Position Animation Types
// =============================================================================

/// Direction for wave/cascade/position effects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    /// Characters move/wave vertically downward.
    #[default]
    Down,
    /// Characters move/wave vertically upward.
    Up,
    /// Characters move/wave horizontally leftward.
    Left,
    /// Characters move/wave horizontally rightward.
    Right,
}

impl Direction {
    /// Returns true if this direction affects vertical position.
    #[inline]
    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    /// Returns true if this direction affects horizontal position.
    #[inline]
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Character position offset for position-based effects.
///
/// This is internal and used to calculate how much each character
/// should be offset from its base position.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharacterOffset {
    /// Horizontal offset in cells (positive = right, negative = left).
    pub dx: i16,
    /// Vertical offset in rows (positive = down, negative = up).
    pub dy: i16,
}

impl CharacterOffset {
    /// Create a new offset.
    #[inline]
    pub const fn new(dx: i16, dy: i16) -> Self {
        Self { dx, dy }
    }

    /// Zero offset (no movement).
    pub const ZERO: Self = Self { dx: 0, dy: 0 };

    /// Add another offset (for combining effects).
    #[inline]
    pub fn add(self, other: Self) -> Self {
        Self {
            dx: self.dx.saturating_add(other.dx),
            dy: self.dy.saturating_add(other.dy),
        }
    }

    /// Clamp offset to terminal bounds.
    ///
    /// Ensures that when applied to position (x, y), the result stays within bounds.
    #[inline]
    pub fn clamp_for_position(self, x: u16, y: u16, width: u16, height: u16) -> Self {
        let min_dx = -(x as i16);
        let max_dx = (width.saturating_sub(1).saturating_sub(x)) as i16;
        let min_dy = -(y as i16);
        let max_dy = (height.saturating_sub(1).saturating_sub(y)) as i16;

        Self {
            dx: self.dx.clamp(min_dx, max_dx),
            dy: self.dy.clamp(min_dy, max_dy),
        }
    }
}

// =============================================================================
// Text Effects
// =============================================================================

/// Available text effects.
#[derive(Debug, Clone, Default)]
pub enum TextEffect {
    /// No effect, plain text.
    #[default]
    None,

    // --- Fade Effects ---
    /// Fade in from transparent to opaque.
    FadeIn {
        /// Progress from 0.0 (invisible) to 1.0 (visible).
        progress: f64,
    },
    /// Fade out from opaque to transparent.
    FadeOut {
        /// Progress from 0.0 (visible) to 1.0 (invisible).
        progress: f64,
    },
    /// Pulsing fade (breathing effect).
    Pulse {
        /// Oscillation speed (cycles per second).
        speed: f64,
        /// Minimum alpha (0.0 to 1.0).
        min_alpha: f64,
    },

    // --- Gradient Effects ---
    /// Horizontal gradient across text.
    HorizontalGradient {
        /// Gradient to use.
        gradient: ColorGradient,
    },
    /// Animated horizontal gradient.
    AnimatedGradient {
        /// Gradient to use.
        gradient: ColorGradient,
        /// Animation speed.
        speed: f64,
    },
    /// Rainbow colors cycling through text.
    RainbowGradient {
        /// Animation speed.
        speed: f64,
    },

    // --- Color Cycling ---
    /// Cycle through colors (all characters same color).
    ColorCycle {
        /// Colors to cycle through.
        colors: Vec<PackedRgba>,
        /// Cycle speed.
        speed: f64,
    },
    /// Wave effect - color moves through text like a wave.
    ColorWave {
        /// Primary color.
        color1: PackedRgba,
        /// Secondary color.
        color2: PackedRgba,
        /// Wave speed.
        speed: f64,
        /// Wave length (characters per cycle).
        wavelength: f64,
    },

    // --- Glow Effects ---
    /// Static glow around text.
    Glow {
        /// Glow color (usually a brighter version of base).
        color: PackedRgba,
        /// Intensity (0.0 to 1.0).
        intensity: f64,
    },
    /// Animated glow that pulses.
    PulsingGlow {
        /// Glow color.
        color: PackedRgba,
        /// Pulse speed.
        speed: f64,
    },

    // --- Character Effects ---
    /// Typewriter effect - characters appear one by one.
    Typewriter {
        /// Number of characters visible (can be fractional for smooth animation).
        visible_chars: f64,
    },
    /// Scramble effect - random characters that resolve to final text.
    Scramble {
        /// Progress from 0.0 (scrambled) to 1.0 (resolved).
        progress: f64,
    },
    /// Glitch effect - occasional character corruption.
    Glitch {
        /// Glitch intensity (0.0 to 1.0).
        intensity: f64,
    },

    // --- Position/Wave Effects ---
    /// Sinusoidal wave motion - characters oscillate up/down or left/right.
    ///
    /// Creates a smooth wave pattern across the text. The wave travels through
    /// the text at the specified speed, with each character's phase determined
    /// by its position and the wavelength.
    Wave {
        /// Maximum offset in cells (typically 1-3).
        amplitude: f64,
        /// Characters per wave cycle (typically 5-15).
        wavelength: f64,
        /// Wave cycles per second.
        speed: f64,
        /// Wave travel direction.
        direction: Direction,
    },

    /// Bouncing motion - characters bounce as if dropped.
    ///
    /// Characters start high (at `height` offset) and bounce toward rest,
    /// with optional damping for a settling effect. The stagger parameter
    /// creates a cascade where each character starts its bounce slightly later.
    Bounce {
        /// Initial/max bounce height in cells.
        height: f64,
        /// Bounces per second.
        speed: f64,
        /// Delay between adjacent characters (0.0-1.0 of total cycle).
        stagger: f64,
        /// Damping factor (0.8-0.99). Higher = slower settling.
        damping: f64,
    },

    /// Random shake/jitter motion - characters vibrate randomly.
    ///
    /// Creates a shaking effect using deterministic pseudo-random offsets.
    /// The same seed and time always produce the same offsets.
    Shake {
        /// Maximum offset magnitude (typically 0.5-2).
        intensity: f64,
        /// Shake frequency (updates per second).
        speed: f64,
        /// Seed for deterministic randomness.
        seed: u64,
    },

    /// Cascade reveal - characters appear in sequence from a direction.
    ///
    /// Similar to typewriter but with directional control and positional offset.
    /// Characters slide in from the specified direction as they're revealed.
    Cascade {
        /// Characters revealed per second.
        speed: f64,
        /// Direction characters slide in from.
        direction: Direction,
        /// Delay between characters (0.0-1.0).
        stagger: f64,
    },
}

// =============================================================================
// StyledText - Text with effects
// =============================================================================

/// Maximum number of effects that can be chained on a single StyledText.
/// This limit prevents performance issues from excessive effect stacking.
pub const MAX_EFFECTS: usize = 8;

/// Text widget with animated effects.
///
/// StyledText supports composable effect chains - multiple effects can be
/// applied simultaneously. Effects are categorized and combined as follows:
///
/// | Category | Effects | Combination Rule |
/// |----------|---------|------------------|
/// | ColorModifier | Gradient, ColorCycle, ColorWave, Glow | BLEND: colors multiply |
/// | AlphaModifier | FadeIn, FadeOut, Pulse | MULTIPLY: alpha values multiply |
/// | PositionModifier | Wave, Bounce, Shake | ADD: offsets sum |
/// | CharModifier | Typewriter, Scramble, Glitch | PRIORITY: first wins |
///
/// # Example
///
/// ```rust,ignore
/// let styled = StyledText::new("Hello")
///     .effect(TextEffect::RainbowGradient { speed: 0.1 })
///     .effect(TextEffect::Pulse { speed: 2.0, min_alpha: 0.3 })
///     .time(current_time);
/// ```
#[derive(Debug, Clone)]
pub struct StyledText {
    text: String,
    /// Effects to apply, in order. Maximum of MAX_EFFECTS.
    effects: Vec<TextEffect>,
    base_color: PackedRgba,
    bg_color: Option<PackedRgba>,
    bold: bool,
    italic: bool,
    underline: bool,
    time: f64,
    seed: u64,
    /// Easing function for time-based effects.
    easing: Easing,
}

impl StyledText {
    /// Create new styled text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            effects: Vec::new(),
            base_color: PackedRgba::rgb(255, 255, 255),
            bg_color: None,
            bold: false,
            italic: false,
            underline: false,
            time: 0.0,
            seed: 12345,
            easing: Easing::default(),
        }
    }

    /// Add a text effect to the chain.
    ///
    /// Effects are applied in the order they are added. A maximum of
    /// [`MAX_EFFECTS`] can be chained; additional effects are ignored.
    ///
    /// # Effect Composition
    ///
    /// - **Color effects** (Gradient, ColorCycle, etc.): Colors are blended/modulated
    /// - **Alpha effects** (FadeIn, Pulse, etc.): Alpha values multiply together
    /// - **Character effects** (Typewriter, Scramble): First visible char wins
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// StyledText::new("Hello")
    ///     .effect(TextEffect::RainbowGradient { speed: 0.1 })
    ///     .effect(TextEffect::Pulse { speed: 2.0, min_alpha: 0.3 })
    /// ```
    pub fn effect(mut self, effect: TextEffect) -> Self {
        if !matches!(effect, TextEffect::None) && self.effects.len() < MAX_EFFECTS {
            self.effects.push(effect);
        }
        self
    }

    /// Add multiple effects at once.
    ///
    /// Convenience method for chaining several effects. Only adds up to
    /// [`MAX_EFFECTS`] total effects.
    pub fn effects(mut self, effects: impl IntoIterator<Item = TextEffect>) -> Self {
        for effect in effects {
            if matches!(effect, TextEffect::None) {
                continue;
            }
            if self.effects.len() >= MAX_EFFECTS {
                break;
            }
            self.effects.push(effect);
        }
        self
    }

    /// Clear all effects, returning to plain text rendering.
    pub fn clear_effects(mut self) -> Self {
        self.effects.clear();
        self
    }

    /// Get the current number of effects.
    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    /// Check if any effects are applied.
    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Set the easing function for time-based effects.
    ///
    /// The easing function affects animations like Pulse, ColorWave,
    /// AnimatedGradient, and PulsingGlow. It does not affect static
    /// effects or progress-based effects (FadeIn, FadeOut, Typewriter).
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set the base text color.
    pub fn base_color(mut self, color: PackedRgba) -> Self {
        self.base_color = color;
        self
    }

    /// Set the background color.
    pub fn bg_color(mut self, color: PackedRgba) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// Make text bold.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Make text italic.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Make text underlined.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Set the animation time (for time-based effects).
    pub fn time(mut self, time: f64) -> Self {
        self.time = time;
        self
    }

    /// Set random seed for scramble/glitch effects.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Get the length of the text.
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Check if text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Calculate the color for a single effect at position `idx`.
    fn effect_color(
        &self,
        effect: &TextEffect,
        idx: usize,
        total: usize,
        base: PackedRgba,
    ) -> PackedRgba {
        let t = if total > 1 {
            idx as f64 / (total - 1) as f64
        } else {
            0.5
        };

        match effect {
            TextEffect::None => base,

            TextEffect::FadeIn { progress } => apply_alpha(base, *progress),

            TextEffect::FadeOut { progress } => apply_alpha(base, 1.0 - progress),

            TextEffect::Pulse { speed, min_alpha } => {
                let alpha =
                    min_alpha + (1.0 - min_alpha) * (0.5 + 0.5 * (self.time * speed * TAU).sin());
                apply_alpha(base, alpha)
            }

            TextEffect::HorizontalGradient { gradient } => gradient.sample(t),

            TextEffect::AnimatedGradient { gradient, speed } => {
                let animated_t = (t + self.time * speed).rem_euclid(1.0);
                gradient.sample(animated_t)
            }

            TextEffect::RainbowGradient { speed } => {
                let hue = ((t + self.time * speed) * 360.0).rem_euclid(360.0);
                hsv_to_rgb(hue, 1.0, 1.0)
            }

            TextEffect::ColorCycle { colors, speed } => {
                if colors.is_empty() {
                    return base;
                }
                let cycle_pos = (self.time * speed).rem_euclid(colors.len() as f64);
                let idx1 = cycle_pos as usize % colors.len();
                let idx2 = (idx1 + 1) % colors.len();
                let local_t = cycle_pos.fract();
                lerp_color(colors[idx1], colors[idx2], local_t)
            }

            TextEffect::ColorWave {
                color1,
                color2,
                speed,
                wavelength,
            } => {
                let phase = t * TAU * (total as f64 / wavelength) - self.time * speed;
                let wave = 0.5 + 0.5 * phase.sin();
                lerp_color(*color1, *color2, wave)
            }

            TextEffect::Glow { color, intensity } => lerp_color(base, *color, *intensity),

            TextEffect::PulsingGlow { color, speed } => {
                let intensity = 0.5 + 0.5 * (self.time * speed * TAU).sin();
                lerp_color(base, *color, intensity)
            }

            TextEffect::Typewriter { visible_chars } => {
                if (idx as f64) < *visible_chars {
                    base
                } else {
                    PackedRgba::TRANSPARENT
                }
            }

            TextEffect::Scramble { progress: _ } | TextEffect::Glitch { intensity: _ } => base,
        }
    }

    /// Calculate the color for a character at position `idx`.
    ///
    /// Applies all effects in order. Color effects blend/modulate,
    /// alpha effects multiply together.
    fn char_color(&self, idx: usize, total: usize) -> PackedRgba {
        if self.effects.is_empty() {
            return self.base_color;
        }

        let mut color = self.base_color;
        let mut alpha_multiplier = 1.0;

        for effect in &self.effects {
            match effect {
                // Alpha-modifying effects: accumulate alpha multipliers
                TextEffect::FadeIn { progress } => {
                    alpha_multiplier *= progress;
                }
                TextEffect::FadeOut { progress } => {
                    alpha_multiplier *= 1.0 - progress;
                }
                TextEffect::Pulse { speed, min_alpha } => {
                    let alpha = min_alpha
                        + (1.0 - min_alpha) * (0.5 + 0.5 * (self.time * speed * TAU).sin());
                    alpha_multiplier *= alpha;
                }
                TextEffect::Typewriter { visible_chars } => {
                    if (idx as f64) >= *visible_chars {
                        return PackedRgba::TRANSPARENT;
                    }
                }

                // Color-modifying effects: blend with current color
                TextEffect::HorizontalGradient { .. }
                | TextEffect::AnimatedGradient { .. }
                | TextEffect::RainbowGradient { .. }
                | TextEffect::ColorCycle { .. }
                | TextEffect::ColorWave { .. } => {
                    // Get the color from this effect and blend with current
                    let effect_color = self.effect_color(effect, idx, total, color);
                    color = effect_color;
                }

                // Glow effects: blend current with glow color
                TextEffect::Glow {
                    color: glow_color,
                    intensity,
                } => {
                    color = lerp_color(color, *glow_color, *intensity);
                }
                TextEffect::PulsingGlow {
                    color: glow_color,
                    speed,
                } => {
                    let intensity = 0.5 + 0.5 * (self.time * speed * TAU).sin();
                    color = lerp_color(color, *glow_color, intensity);
                }

                // Non-color effects don't change color
                TextEffect::None | TextEffect::Scramble { .. } | TextEffect::Glitch { .. } => {}
            }
        }

        // Apply accumulated alpha
        if alpha_multiplier < 1.0 {
            color = apply_alpha(color, alpha_multiplier);
        }

        color
    }

    /// Get the character to display at position `idx`.
    ///
    /// Character-modifying effects have priority - the first effect that
    /// would change the character wins.
    fn char_at(&self, idx: usize, original: char) -> char {
        if self.effects.is_empty() {
            return original;
        }

        let total = self.text.chars().count();

        for effect in &self.effects {
            match effect {
                TextEffect::Scramble { progress } => {
                    if *progress >= 1.0 {
                        continue;
                    }
                    let resolve_threshold = idx as f64 / total as f64;
                    if *progress > resolve_threshold {
                        continue;
                    }
                    // Random character based on time and position
                    let hash = self
                        .seed
                        .wrapping_mul(idx as u64 + 1)
                        .wrapping_add((self.time * 10.0) as u64);
                    let ascii = 33 + (hash % 94) as u8;
                    return ascii as char;
                }

                TextEffect::Glitch { intensity } => {
                    if *intensity <= 0.0 {
                        continue;
                    }
                    // Random glitch based on time
                    let hash = self
                        .seed
                        .wrapping_mul(idx as u64 + 1)
                        .wrapping_add((self.time * 30.0) as u64);
                    let glitch_chance = (hash % 1000) as f64 / 1000.0;
                    if glitch_chance < *intensity * 0.3 {
                        let ascii = 33 + (hash % 94) as u8;
                        return ascii as char;
                    }
                }

                TextEffect::Typewriter { visible_chars } => {
                    if (idx as f64) >= *visible_chars {
                        return ' ';
                    }
                }

                _ => {}
            }
        }

        original
    }

    /// Render at a specific position.
    pub fn render_at(&self, x: u16, y: u16, frame: &mut Frame) {
        let total = self.text.chars().count();
        if total == 0 {
            return;
        }
        let has_fade_effect = self.effects.iter().any(|effect| {
            matches!(
                effect,
                TextEffect::FadeIn { .. } | TextEffect::FadeOut { .. }
            )
        });

        for (i, ch) in self.text.chars().enumerate() {
            let px = x.saturating_add(i as u16);
            let color = self.char_color(i, total);
            let display_char = self.char_at(i, ch);

            // Skip fully transparent
            if color.r() == 0 && color.g() == 0 && color.b() == 0 && has_fade_effect {
                continue;
            }

            if let Some(cell) = frame.buffer.get_mut(px, y) {
                cell.content = CellContent::from_char(display_char);
                cell.fg = color;

                if let Some(bg) = self.bg_color {
                    cell.bg = bg;
                }

                let mut flags = CellStyleFlags::empty();
                if self.bold {
                    flags = flags.union(CellStyleFlags::BOLD);
                }
                if self.italic {
                    flags = flags.union(CellStyleFlags::ITALIC);
                }
                if self.underline {
                    flags = flags.union(CellStyleFlags::UNDERLINE);
                }
                cell.attrs = CellAttrs::new(flags, 0);
            }
        }
    }
}

impl Widget for StyledText {
    fn render(&self, area: Rect, frame: &mut Frame) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.render_at(area.x, area.y, frame);
    }
}

// =============================================================================
// TransitionOverlay - Full-screen announcement effect
// =============================================================================

/// A centered overlay for displaying transition text with fade effects.
///
/// Progress goes from 0.0 (invisible) to 0.5 (peak visibility) to 1.0 (invisible).
/// This creates a smooth fade-in then fade-out animation.
#[derive(Debug, Clone)]
pub struct TransitionOverlay {
    title: String,
    subtitle: String,
    progress: f64,
    primary_color: PackedRgba,
    secondary_color: PackedRgba,
    gradient: Option<ColorGradient>,
    time: f64,
}

impl TransitionOverlay {
    /// Create a new transition overlay.
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            progress: 0.0,
            primary_color: PackedRgba::rgb(255, 100, 200),
            secondary_color: PackedRgba::rgb(180, 180, 220),
            gradient: None,
            time: 0.0,
        }
    }

    /// Set progress (0.0 = invisible, 0.5 = peak, 1.0 = invisible).
    pub fn progress(mut self, progress: f64) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    /// Set the primary (title) color.
    pub fn primary_color(mut self, color: PackedRgba) -> Self {
        self.primary_color = color;
        self
    }

    /// Set the secondary (subtitle) color.
    pub fn secondary_color(mut self, color: PackedRgba) -> Self {
        self.secondary_color = color;
        self
    }

    /// Use an animated gradient for the title.
    pub fn gradient(mut self, gradient: ColorGradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Set animation time.
    pub fn time(mut self, time: f64) -> Self {
        self.time = time;
        self
    }

    /// Calculate opacity from progress.
    fn opacity(&self) -> f64 {
        (self.progress * PI).sin()
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.opacity() > 0.01
    }
}

impl Widget for TransitionOverlay {
    fn render(&self, area: Rect, frame: &mut Frame) {
        let opacity = self.opacity();
        if opacity < 0.01 || area.width < 10 || area.height < 3 {
            return;
        }

        // Center the title
        let title_len = self.title.chars().count() as u16;
        let title_x = area.x + area.width.saturating_sub(title_len) / 2;
        let title_y = area.y + area.height / 2;

        // Render title with gradient or fade
        let title_effect = if let Some(gradient) = &self.gradient {
            TextEffect::AnimatedGradient {
                gradient: gradient.clone(),
                speed: 0.3,
            }
        } else {
            TextEffect::FadeIn { progress: opacity }
        };

        let title_text = StyledText::new(&self.title)
            .effect(title_effect)
            .base_color(apply_alpha(self.primary_color, opacity))
            .bold()
            .time(self.time);
        title_text.render_at(title_x, title_y, frame);

        // Render subtitle
        if !self.subtitle.is_empty() && title_y + 1 < area.y + area.height {
            let subtitle_len = self.subtitle.chars().count() as u16;
            let subtitle_x = area.x + area.width.saturating_sub(subtitle_len) / 2;
            let subtitle_y = title_y + 1;

            let subtitle_text = StyledText::new(&self.subtitle)
                .effect(TextEffect::FadeIn {
                    progress: opacity * 0.85,
                })
                .base_color(self.secondary_color)
                .italic()
                .time(self.time);
            subtitle_text.render_at(subtitle_x, subtitle_y, frame);
        }
    }
}

// =============================================================================
// TransitionState - Animation state manager
// =============================================================================

/// Helper for managing transition animations.
#[derive(Debug, Clone)]
pub struct TransitionState {
    progress: f64,
    active: bool,
    speed: f64,
    title: String,
    subtitle: String,
    color: PackedRgba,
    gradient: Option<ColorGradient>,
    time: f64,
    /// Easing function for transition animations.
    easing: Easing,
}

impl Default for TransitionState {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionState {
    /// Create new transition state.
    pub fn new() -> Self {
        Self {
            progress: 0.0,
            active: false,
            speed: 0.05,
            title: String::new(),
            subtitle: String::new(),
            color: PackedRgba::rgb(255, 100, 200),
            gradient: None,
            time: 0.0,
            easing: Easing::default(),
        }
    }

    /// Set the easing function for the transition animation.
    pub fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
    }

    /// Get the current easing function.
    pub fn easing(&self) -> Easing {
        self.easing
    }

    /// Get the eased progress value.
    pub fn eased_progress(&self) -> f64 {
        self.easing.apply(self.progress)
    }

    /// Start a transition.
    pub fn start(
        &mut self,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        color: PackedRgba,
    ) {
        self.title = title.into();
        self.subtitle = subtitle.into();
        self.color = color;
        self.gradient = None;
        self.progress = 0.0;
        self.active = true;
    }

    /// Start a transition with gradient.
    pub fn start_with_gradient(
        &mut self,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        gradient: ColorGradient,
    ) {
        self.title = title.into();
        self.subtitle = subtitle.into();
        self.gradient = Some(gradient);
        self.progress = 0.0;
        self.active = true;
    }

    /// Set transition speed.
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.01, 0.5);
    }

    /// Update the transition (call every tick).
    pub fn tick(&mut self) {
        self.time += 0.1;
        if self.active {
            self.progress += self.speed;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.active = false;
            }
        }
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.active || (self.progress > 0.0 && self.progress < 1.0)
    }

    /// Check if active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get current progress.
    pub fn progress(&self) -> f64 {
        self.progress
    }

    /// Get the overlay widget.
    pub fn overlay(&self) -> TransitionOverlay {
        let mut overlay = TransitionOverlay::new(&self.title, &self.subtitle)
            .progress(self.progress)
            .primary_color(self.color)
            .time(self.time);

        if let Some(ref gradient) = self.gradient {
            overlay = overlay.gradient(gradient.clone());
        }

        overlay
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lerp_color() {
        let black = PackedRgba::rgb(0, 0, 0);
        let white = PackedRgba::rgb(255, 255, 255);
        let mid = lerp_color(black, white, 0.5);
        assert_eq!(mid.r(), 127);
    }

    #[test]
    fn test_color_gradient() {
        let gradient = ColorGradient::rainbow();
        let red = gradient.sample(0.0);
        assert!(red.r() > 200);

        let mid = gradient.sample(0.5);
        assert!(mid.g() > 200); // Should be greenish
    }

    #[test]
    fn test_styled_text_effects() {
        let text = StyledText::new("Hello")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .time(0.5);

        assert_eq!(text.len(), 5);
        assert!(!text.is_empty());
    }

    #[test]
    fn test_transition_state() {
        let mut state = TransitionState::new();
        assert!(!state.is_active());

        state.start("Title", "Sub", PackedRgba::rgb(255, 0, 0));
        assert!(state.is_active());

        for _ in 0..50 {
            state.tick();
        }
        assert!(!state.is_active());
    }

    #[test]
    fn test_scramble_effect() {
        let text = StyledText::new("TEST")
            .effect(TextEffect::Scramble { progress: 0.0 })
            .seed(42)
            .time(1.0);

        // At progress 0, characters should be scrambled
        let ch = text.char_at(0, 'T');
        // The scrambled char will be random but not necessarily 'T'
        assert!(ch.is_ascii_graphic());
    }

    #[test]
    fn test_ascii_art_basic() {
        let art = AsciiArtText::new("HI", AsciiArtStyle::Block);
        let lines = art.render_lines();
        assert!(!lines.is_empty());
        // Block style produces 5-line characters
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_ascii_art_styles() {
        for style in [
            AsciiArtStyle::Block,
            AsciiArtStyle::Banner,
            AsciiArtStyle::Mini,
            AsciiArtStyle::Slant,
        ] {
            let art = AsciiArtText::new("A", style);
            let lines = art.render_lines();
            assert!(!lines.is_empty());
        }
    }

    // =========================================================================
    // Easing Tests
    // =========================================================================

    #[test]
    fn test_easing_linear_identity() {
        // Linear.apply(t) == t for all t
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let result = Easing::Linear.apply(t);
            assert!(
                (result - t).abs() < 1e-10,
                "Linear({t}) should equal {t}, got {result}"
            );
        }
    }

    #[test]
    fn test_easing_input_clamped() {
        // Inputs outside 0-1 should be clamped
        let easings = [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
            Easing::Bounce,
        ];

        for easing in easings {
            let at_zero = easing.apply(0.0);
            let below_zero = easing.apply(-0.5);
            let above_one = easing.apply(1.5);
            let at_one = easing.apply(1.0);

            assert!(
                (below_zero - at_zero).abs() < 1e-10,
                "{:?}.apply(-0.5) should equal apply(0.0)",
                easing
            );
            assert!(
                (above_one - at_one).abs() < 1e-10,
                "{:?}.apply(1.5) should equal apply(1.0)",
                easing
            );
        }
    }

    #[test]
    fn test_easing_bounds_normal() {
        // Most curves output 0 at t=0, 1 at t=1
        let easings = [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
            Easing::Bounce,
        ];

        for easing in easings {
            let start = easing.apply(0.0);
            let end = easing.apply(1.0);

            assert!(
                start.abs() < 1e-10,
                "{:?}.apply(0.0) should be 0, got {start}",
                easing
            );
            assert!(
                (end - 1.0).abs() < 1e-10,
                "{:?}.apply(1.0) should be 1, got {end}",
                easing
            );
        }
    }

    #[test]
    fn test_easing_elastic_overshoots() {
        // Elastic briefly exceeds 1.0
        assert!(Easing::Elastic.can_overshoot());

        // Find the maximum value in the curve
        let mut max_val = 0.0_f64;
        for i in 0..=1000 {
            let t = i as f64 / 1000.0;
            let val = Easing::Elastic.apply(t);
            max_val = max_val.max(val);
        }

        assert!(
            max_val > 1.0,
            "Elastic should exceed 1.0, max was {max_val}"
        );
    }

    #[test]
    fn test_easing_back_overshoots() {
        // Back goes < 0 at start or > 1 during transition
        assert!(Easing::Back.can_overshoot());

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;

        for i in 0..=1000 {
            let t = i as f64 / 1000.0;
            let val = Easing::Back.apply(t);
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }

        // Back should overshoot in one direction
        assert!(
            min_val < 0.0 || max_val > 1.0,
            "Back should overshoot, got range [{min_val}, {max_val}]"
        );
    }

    #[test]
    fn test_easing_monotonic() {
        // EaseIn/Out should be monotonically increasing
        let monotonic_easings = [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
        ];

        for easing in monotonic_easings {
            let mut prev = easing.apply(0.0);
            for i in 1..=100 {
                let t = i as f64 / 100.0;
                let curr = easing.apply(t);
                assert!(
                    curr >= prev - 1e-10,
                    "{:?} is not monotonic at t={t}: {prev} -> {curr}",
                    easing
                );
                prev = curr;
            }
        }
    }

    #[test]
    fn test_easing_step_discrete() {
        // Step(4) outputs exactly {0, 0.25, 0.5, 0.75, 1.0}
        let step4 = Easing::Step(4);

        let expected = [0.0, 0.25, 0.5, 0.75, 1.0];
        let inputs = [0.0, 0.25, 0.5, 0.75, 1.0];

        for (t, exp) in inputs.iter().zip(expected.iter()) {
            let result = step4.apply(*t);
            assert!(
                (result - exp).abs() < 1e-10,
                "Step(4).apply({t}) should be {exp}, got {result}"
            );
        }

        // Values between steps should snap to lower step
        let mid_result = step4.apply(0.3);
        assert!(
            (mid_result - 0.25).abs() < 1e-10,
            "Step(4).apply(0.3) should be 0.25, got {mid_result}"
        );
    }

    #[test]
    fn test_easing_in_slow_start() {
        // EaseIn should be slow at start (derivative ≈ 0)
        // Compare values at t=0.1: EaseIn should be much smaller than Linear
        let linear = Easing::Linear.apply(0.1);
        let ease_in = Easing::EaseIn.apply(0.1);

        assert!(
            ease_in < linear,
            "EaseIn(0.1) should be less than Linear(0.1)"
        );
        assert!(
            ease_in < linear * 0.5,
            "EaseIn(0.1) should be significantly slower than Linear"
        );
    }

    #[test]
    fn test_easing_out_slow_end() {
        // EaseOut should be slow at end
        // Compare values at t=0.9: EaseOut should be much larger than Linear
        let linear = Easing::Linear.apply(0.9);
        let ease_out = Easing::EaseOut.apply(0.9);

        assert!(
            ease_out > linear,
            "EaseOut(0.9) should be greater than Linear(0.9)"
        );
    }

    #[test]
    fn test_easing_symmetry() {
        // EaseInOut should be symmetric around t=0.5
        let easing = Easing::EaseInOut;

        // At t=0.5, value should be 0.5
        let mid = easing.apply(0.5);
        assert!(
            (mid - 0.5).abs() < 1e-10,
            "EaseInOut(0.5) should be 0.5, got {mid}"
        );

        // Check symmetry: f(t) + f(1-t) = 1
        for i in 0..=50 {
            let t = i as f64 / 100.0;
            let left = easing.apply(t);
            let right = easing.apply(1.0 - t);

            assert!(
                (left + right - 1.0).abs() < 1e-10,
                "EaseInOut should be symmetric: f({t}) + f({}) = {} (expected 1.0)",
                1.0 - t,
                left + right
            );
        }
    }

    #[test]
    fn test_easing_styled_text_integration() {
        // Verify StyledText can use easing
        let text = StyledText::new("Hello")
            .effect(TextEffect::Pulse {
                speed: 1.0,
                min_alpha: 0.3,
            })
            .easing(Easing::EaseInOut)
            .time(0.25);

        assert_eq!(text.len(), 5);
    }

    #[test]
    fn test_easing_transition_state_integration() {
        let mut state = TransitionState::new();
        state.set_easing(Easing::EaseOut);

        assert_eq!(state.easing(), Easing::EaseOut);

        state.start("Test", "Subtitle", PackedRgba::rgb(255, 0, 0));

        // Progress starts at 0
        assert!((state.eased_progress() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_easing_names() {
        // All easings should have names
        let easings = [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
            Easing::Bounce,
            Easing::Elastic,
            Easing::Back,
            Easing::Step(4),
        ];

        for easing in easings {
            let name = easing.name();
            assert!(!name.is_empty(), "{:?} should have a name", easing);
        }
    }

    // =========================================================================
    // AnimationClock Tests
    // =========================================================================

    #[test]
    fn test_clock_new_starts_at_zero() {
        let clock = AnimationClock::new();
        assert!((clock.time() - 0.0).abs() < 1e-10);
        assert!((clock.speed() - 1.0).abs() < 1e-10);
        assert!(!clock.is_paused());
    }

    #[test]
    fn test_clock_with_time() {
        let clock = AnimationClock::with_time(5.0);
        assert!((clock.time() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_tick_delta_advances() {
        let mut clock = AnimationClock::new();
        clock.tick_delta(0.5);
        assert!((clock.time() - 0.5).abs() < 1e-10);

        clock.tick_delta(0.25);
        assert!((clock.time() - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_clock_pause_stops_time() {
        let mut clock = AnimationClock::new();
        clock.pause();
        assert!(clock.is_paused());
        assert!((clock.speed() - 0.0).abs() < 1e-10);

        // Ticking while paused should not advance time
        clock.tick_delta(1.0);
        assert!((clock.time() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_resume_restarts() {
        let mut clock = AnimationClock::new();
        clock.pause();
        assert!(clock.is_paused());

        clock.resume();
        assert!(!clock.is_paused());
        assert!((clock.speed() - 1.0).abs() < 1e-10);

        clock.tick_delta(1.0);
        assert!((clock.time() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_speed_multiplies() {
        let mut clock = AnimationClock::new();
        clock.set_speed(2.0);
        clock.tick_delta(1.0);
        // At 2x speed, 1 second real time = 2 seconds animation time
        assert!((clock.time() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_half_speed() {
        let mut clock = AnimationClock::new();
        clock.set_speed(0.5);
        clock.tick_delta(1.0);
        // At 0.5x speed, 1 second real time = 0.5 seconds animation time
        assert!((clock.time() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_clock_reset_zeros() {
        let mut clock = AnimationClock::new();
        clock.tick_delta(5.0);
        assert!((clock.time() - 5.0).abs() < 1e-10);

        clock.reset();
        assert!((clock.time() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_set_time() {
        let mut clock = AnimationClock::new();
        clock.set_time(10.0);
        assert!((clock.time() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_elapsed_since() {
        let mut clock = AnimationClock::new();
        clock.tick_delta(5.0);

        let elapsed = clock.elapsed_since(2.0);
        assert!((elapsed - 3.0).abs() < 1e-10);

        // Elapsed since future time should be 0
        let elapsed_future = clock.elapsed_since(10.0);
        assert!((elapsed_future - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_phase_cycling() {
        let mut clock = AnimationClock::new();

        // At time 0, phase should be 0
        assert!((clock.phase(1.0) - 0.0).abs() < 1e-10);

        // At time 0.5 with 1 cycle/sec, phase = 0.5
        clock.set_time(0.5);
        assert!((clock.phase(1.0) - 0.5).abs() < 1e-10);

        // At time 1.0, phase should wrap to 0
        clock.set_time(1.0);
        assert!((clock.phase(1.0) - 0.0).abs() < 1e-10);

        // At time 1.25, phase = 0.25
        clock.set_time(1.25);
        assert!((clock.phase(1.0) - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_clock_phase_frequency() {
        let mut clock = AnimationClock::new();
        clock.set_time(0.5);

        // 2 cycles per second: at t=0.5, phase = (0.5 * 2).fract() = 0.0
        assert!((clock.phase(2.0) - 0.0).abs() < 1e-10);

        clock.set_time(0.25);
        // At t=0.25 with 2 cycles/sec: phase = (0.25 * 2).fract() = 0.5
        assert!((clock.phase(2.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_clock_phase_zero_frequency() {
        let clock = AnimationClock::with_time(5.0);
        // Zero or negative frequency should return 0
        assert!((clock.phase(0.0) - 0.0).abs() < 1e-10);
        assert!((clock.phase(-1.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_negative_speed_clamped() {
        let mut clock = AnimationClock::new();
        clock.set_speed(-5.0);
        // Negative speed should be clamped to 0
        assert!((clock.speed() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_clock_default() {
        let clock = AnimationClock::default();
        assert!((clock.time() - 0.0).abs() < 1e-10);
        assert!((clock.speed() - 1.0).abs() < 1e-10);
    }

    // =========================================================================
    // Composable Effect Chain Tests (bd-3aa3)
    // =========================================================================

    #[test]
    fn test_single_effect_backwards_compat() {
        // .effect(e) still works as before
        let text = StyledText::new("Hello")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .time(0.5);

        assert_eq!(text.effect_count(), 1);
        assert!(text.has_effects());
        assert_eq!(text.len(), 5);
    }

    #[test]
    fn test_multiple_color_effects_blend() {
        // Rainbow + Pulse should modulate together
        let text = StyledText::new("Test")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .effect(TextEffect::Pulse {
                speed: 2.0,
                min_alpha: 0.5,
            })
            .time(0.25);

        assert_eq!(text.effect_count(), 2);

        // Get colors - they should be non-zero (rainbow provides color, pulse modulates alpha)
        let color = text.char_color(0, 4);
        // Color should exist (not fully transparent)
        assert!(color.r() > 0 || color.g() > 0 || color.b() > 0);
    }

    #[test]
    fn test_multiple_alpha_effects_multiply() {
        // FadeIn * Pulse should multiply alpha values
        let text = StyledText::new("Test")
            .base_color(PackedRgba::rgb(255, 255, 255))
            .effect(TextEffect::FadeIn { progress: 0.5 }) // 50% alpha
            .effect(TextEffect::Pulse {
                speed: 0.0, // No animation, so sin(0) = 0, alpha = 0.5 + 0.5*0 = 0.5
                min_alpha: 0.5,
            })
            .time(0.0);

        assert_eq!(text.effect_count(), 2);

        // The combined alpha should be 0.5 * ~0.5 = ~0.25
        // This means the color values should be reduced
        let color = text.char_color(0, 4);
        // Color should be dimmed (not full 255)
        assert!(color.r() < 200);
    }

    #[test]
    fn test_effect_order_deterministic() {
        // Same effects applied in same order = same output
        let text1 = StyledText::new("Test")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .effect(TextEffect::FadeIn { progress: 0.8 })
            .time(0.5)
            .seed(42);

        let text2 = StyledText::new("Test")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .effect(TextEffect::FadeIn { progress: 0.8 })
            .time(0.5)
            .seed(42);

        let color1 = text1.char_color(0, 4);
        let color2 = text2.char_color(0, 4);

        assert_eq!(color1.r(), color2.r());
        assert_eq!(color1.g(), color2.g());
        assert_eq!(color1.b(), color2.b());
    }

    #[test]
    fn test_clear_effects() {
        // clear_effects() returns to plain rendering
        let text = StyledText::new("Test")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .effect(TextEffect::Pulse {
                speed: 2.0,
                min_alpha: 0.3,
            })
            .clear_effects();

        assert_eq!(text.effect_count(), 0);
        assert!(!text.has_effects());

        // Color should be base color (white by default)
        let color = text.char_color(0, 4);
        assert_eq!(color.r(), 255);
        assert_eq!(color.g(), 255);
        assert_eq!(color.b(), 255);
    }

    #[test]
    fn test_empty_effects_vec() {
        // No effects = plain text rendering
        let text = StyledText::new("Test").base_color(PackedRgba::rgb(100, 150, 200));

        assert_eq!(text.effect_count(), 0);
        assert!(!text.has_effects());

        // Color should be base color
        let color = text.char_color(0, 4);
        assert_eq!(color.r(), 100);
        assert_eq!(color.g(), 150);
        assert_eq!(color.b(), 200);
    }

    #[test]
    fn test_max_effects_enforced() {
        // Adding >MAX_EFFECTS should be silently ignored (truncated)
        let mut text = StyledText::new("Test");

        // Add more than MAX_EFFECTS (8)
        for i in 0..12 {
            text = text.effect(TextEffect::Pulse {
                speed: i as f64,
                min_alpha: 0.5,
            });
        }

        // Should be capped at MAX_EFFECTS
        assert_eq!(text.effect_count(), MAX_EFFECTS);
        assert_eq!(text.effect_count(), 8);
    }

    #[test]
    fn test_effects_method_batch_add() {
        // .effects() method should add multiple at once
        let effects = vec![
            TextEffect::RainbowGradient { speed: 1.0 },
            TextEffect::FadeIn { progress: 0.5 },
            TextEffect::Pulse {
                speed: 1.0,
                min_alpha: 0.3,
            },
        ];

        let text = StyledText::new("Test").effects(effects);

        assert_eq!(text.effect_count(), 3);
    }

    #[test]
    fn test_none_effect_ignored() {
        // TextEffect::None should not be added
        let text = StyledText::new("Test")
            .effect(TextEffect::None)
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .effect(TextEffect::None);

        assert_eq!(text.effect_count(), 1);
    }
}

// =============================================================================
// ASCII Art Text - Figlet-style large text
// =============================================================================

/// ASCII art font styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiArtStyle {
    /// Large block letters using Unicode block characters.
    Block,
    /// Classic banner-style with slashes and pipes.
    Banner,
    /// Minimal 3-line height for compact display.
    Mini,
    /// Slanted italic-like style.
    Slant,
    /// Doom-style chunky letters.
    Doom,
    /// Small caps using Unicode characters.
    SmallCaps,
}

/// ASCII art text renderer.
#[derive(Debug, Clone)]
pub struct AsciiArtText {
    text: String,
    style: AsciiArtStyle,
    color: Option<PackedRgba>,
    gradient: Option<ColorGradient>,
}

impl AsciiArtText {
    /// Create new ASCII art text.
    pub fn new(text: impl Into<String>, style: AsciiArtStyle) -> Self {
        Self {
            text: text.into().to_uppercase(),
            style,
            color: None,
            gradient: None,
        }
    }

    /// Set text color.
    pub fn color(mut self, color: PackedRgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Use a gradient for coloring.
    pub fn gradient(mut self, gradient: ColorGradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Get the height in lines for this style.
    pub fn height(&self) -> usize {
        match self.style {
            AsciiArtStyle::Block => 5,
            AsciiArtStyle::Banner => 6,
            AsciiArtStyle::Mini => 3,
            AsciiArtStyle::Slant => 5,
            AsciiArtStyle::Doom => 8,
            AsciiArtStyle::SmallCaps => 1,
        }
    }

    /// Get the width for a single character.
    #[allow(dead_code)]
    fn char_width(&self) -> usize {
        match self.style {
            AsciiArtStyle::Block => 6,
            AsciiArtStyle::Banner => 6,
            AsciiArtStyle::Mini => 4,
            AsciiArtStyle::Slant => 6,
            AsciiArtStyle::Doom => 8,
            AsciiArtStyle::SmallCaps => 1,
        }
    }

    /// Render to vector of lines.
    pub fn render_lines(&self) -> Vec<String> {
        let height = self.height();
        let mut lines = vec![String::new(); height];

        for ch in self.text.chars() {
            let char_lines = self.render_char(ch);
            for (i, line) in char_lines.iter().enumerate() {
                if i < lines.len() {
                    lines[i].push_str(line);
                }
            }
        }

        lines
    }

    /// Render a single character to lines.
    fn render_char(&self, ch: char) -> Vec<&'static str> {
        match self.style {
            AsciiArtStyle::Block => self.render_block(ch),
            AsciiArtStyle::Banner => self.render_banner(ch),
            AsciiArtStyle::Mini => self.render_mini(ch),
            AsciiArtStyle::Slant => self.render_slant(ch),
            AsciiArtStyle::Doom => self.render_doom(ch),
            AsciiArtStyle::SmallCaps => self.render_small_caps(ch),
        }
    }

    fn render_block(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec!["  █   ", " █ █  ", "█████ ", "█   █ ", "█   █ "],
            'B' => vec!["████  ", "█   █ ", "████  ", "█   █ ", "████  "],
            'C' => vec![" ████ ", "█     ", "█     ", "█     ", " ████ "],
            'D' => vec!["████  ", "█   █ ", "█   █ ", "█   █ ", "████  "],
            'E' => vec!["█████ ", "█     ", "███   ", "█     ", "█████ "],
            'F' => vec!["█████ ", "█     ", "███   ", "█     ", "█     "],
            'G' => vec![" ████ ", "█     ", "█  ██ ", "█   █ ", " ████ "],
            'H' => vec!["█   █ ", "█   █ ", "█████ ", "█   █ ", "█   █ "],
            'I' => vec!["█████ ", "  █   ", "  █   ", "  █   ", "█████ "],
            'J' => vec!["█████ ", "   █  ", "   █  ", "█  █  ", " ██   "],
            'K' => vec!["█   █ ", "█  █  ", "███   ", "█  █  ", "█   █ "],
            'L' => vec!["█     ", "█     ", "█     ", "█     ", "█████ "],
            'M' => vec!["█   █ ", "██ ██ ", "█ █ █ ", "█   █ ", "█   █ "],
            'N' => vec!["█   █ ", "██  █ ", "█ █ █ ", "█  ██ ", "█   █ "],
            'O' => vec![" ███  ", "█   █ ", "█   █ ", "█   █ ", " ███  "],
            'P' => vec!["████  ", "█   █ ", "████  ", "█     ", "█     "],
            'Q' => vec![" ███  ", "█   █ ", "█   █ ", "█  █  ", " ██ █ "],
            'R' => vec!["████  ", "█   █ ", "████  ", "█  █  ", "█   █ "],
            'S' => vec![" ████ ", "█     ", " ███  ", "    █ ", "████  "],
            'T' => vec!["█████ ", "  █   ", "  █   ", "  █   ", "  █   "],
            'U' => vec!["█   █ ", "█   █ ", "█   █ ", "█   █ ", " ███  "],
            'V' => vec!["█   █ ", "█   █ ", "█   █ ", " █ █  ", "  █   "],
            'W' => vec!["█   █ ", "█   █ ", "█ █ █ ", "██ ██ ", "█   █ "],
            'X' => vec!["█   █ ", " █ █  ", "  █   ", " █ █  ", "█   █ "],
            'Y' => vec!["█   █ ", " █ █  ", "  █   ", "  █   ", "  █   "],
            'Z' => vec!["█████ ", "   █  ", "  █   ", " █    ", "█████ "],
            '0' => vec![" ███  ", "█  ██ ", "█ █ █ ", "██  █ ", " ███  "],
            '1' => vec!["  █   ", " ██   ", "  █   ", "  █   ", " ███  "],
            '2' => vec![" ███  ", "█   █ ", "  ██  ", " █    ", "█████ "],
            '3' => vec!["████  ", "    █ ", " ███  ", "    █ ", "████  "],
            '4' => vec!["█   █ ", "█   █ ", "█████ ", "    █ ", "    █ "],
            '5' => vec!["█████ ", "█     ", "████  ", "    █ ", "████  "],
            '6' => vec![" ███  ", "█     ", "████  ", "█   █ ", " ███  "],
            '7' => vec!["█████ ", "    █ ", "   █  ", "  █   ", "  █   "],
            '8' => vec![" ███  ", "█   █ ", " ███  ", "█   █ ", " ███  "],
            '9' => vec![" ███  ", "█   █ ", " ████ ", "    █ ", " ███  "],
            ' ' => vec!["      ", "      ", "      ", "      ", "      "],
            '!' => vec!["  █   ", "  █   ", "  █   ", "      ", "  █   "],
            '?' => vec![" ███  ", "█   █ ", "  ██  ", "      ", "  █   "],
            '.' => vec!["      ", "      ", "      ", "      ", "  █   "],
            '-' => vec!["      ", "      ", "█████ ", "      ", "      "],
            ':' => vec!["      ", "  █   ", "      ", "  █   ", "      "],
            _ => vec!["█████ ", "█   █ ", "█   █ ", "█   █ ", "█████ "],
        }
    }

    fn render_banner(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec![
                "  /\\  ", " /  \\ ", "/----\\", "/    \\", "/    \\", "      ",
            ],
            'B' => vec![
                "==\\   ", "| /=\\ ", "||__/ ", "| /=\\ ", "==/   ", "      ",
            ],
            'C' => vec![" /===\\", "|     ", "|     ", "|     ", " \\===/", "      "],
            'D' => vec!["==\\   ", "| \\   ", "|  |  ", "| /   ", "==/   ", "      "],
            'E' => vec!["|===| ", "|     ", "|===  ", "|     ", "|===| ", "      "],
            'F' => vec!["|===| ", "|     ", "|===  ", "|     ", "|     ", "      "],
            'G' => vec![" /===\\", "|     ", "| /==|", "|    |", " \\===/", "      "],
            'H' => vec!["|   | ", "|   | ", "|===| ", "|   | ", "|   | ", "      "],
            'I' => vec!["|===| ", "  |   ", "  |   ", "  |   ", "|===| ", "      "],
            'J' => vec!["|===| ", "   |  ", "   |  ", "|  |  ", " \\/   ", "      "],
            'K' => vec!["|  /  ", "| /   ", "|<    ", "| \\   ", "|  \\  ", "      "],
            'L' => vec!["|     ", "|     ", "|     ", "|     ", "|===| ", "      "],
            'M' => vec!["|\\  /|", "| \\/ |", "|    |", "|    |", "|    |", "      "],
            'N' => vec![
                "|\\   |", "| \\  |", "|  \\ |", "|   \\|", "|    |", "      ",
            ],
            'O' => vec![" /==\\ ", "|    |", "|    |", "|    |", " \\==/ ", "      "],
            'P' => vec!["|===\\ ", "|   | ", "|===/ ", "|     ", "|     ", "      "],
            'Q' => vec![
                " /==\\ ", "|    |", "|    |", "|  \\ |", " \\==\\/", "      ",
            ],
            'R' => vec![
                "|===\\ ", "|   | ", "|===/ ", "|  \\  ", "|   \\ ", "      ",
            ],
            'S' => vec![
                " /===\\", "|     ", " \\==\\ ", "     |", "\\===/ ", "      ",
            ],
            'T' => vec!["|===| ", "  |   ", "  |   ", "  |   ", "  |   ", "      "],
            'U' => vec!["|   | ", "|   | ", "|   | ", "|   | ", " \\=/ ", "      "],
            'V' => vec!["|   | ", "|   | ", " \\ /  ", "  |   ", "  |   ", "      "],
            'W' => vec![
                "|    |", "|    |", "| /\\ |", "|/  \\|", "/    \\", "      ",
            ],
            'X' => vec![
                "\\   / ", " \\ /  ", "  X   ", " / \\  ", "/   \\ ", "      ",
            ],
            'Y' => vec!["\\   / ", " \\ /  ", "  |   ", "  |   ", "  |   ", "      "],
            'Z' => vec!["|===| ", "   /  ", "  /   ", " /    ", "|===| ", "      "],
            ' ' => vec!["      ", "      ", "      ", "      ", "      ", "      "],
            _ => vec!["[???] ", "[???] ", "[???] ", "[???] ", "[???] ", "      "],
        }
    }

    fn render_mini(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec![" /\\ ", "/--\\", "    "],
            'B' => vec!["|=\\ ", "|=/ ", "    "],
            'C' => vec!["/== ", "\\== ", "    "],
            'D' => vec!["|=\\ ", "|=/ ", "    "],
            'E' => vec!["|== ", "|== ", "    "],
            'F' => vec!["|== ", "|   ", "    "],
            'G' => vec!["/== ", "\\=| ", "    "],
            'H' => vec!["|-| ", "| | ", "    "],
            'I' => vec!["=|= ", "=|= ", "    "],
            'J' => vec!["==| ", "\\=| ", "    "],
            'K' => vec!["|/ ", "|\\  ", "    "],
            'L' => vec!["|   ", "|== ", "    "],
            'M' => vec!["|v| ", "| | ", "    "],
            'N' => vec!["|\\| ", "| | ", "    "],
            'O' => vec!["/=\\ ", "\\=/ ", "    "],
            'P' => vec!["|=\\ ", "|   ", "    "],
            'Q' => vec!["/=\\ ", "\\=\\|", "    "],
            'R' => vec!["|=\\ ", "| \\ ", "    "],
            'S' => vec!["/=  ", "\\=/ ", "    "],
            'T' => vec!["=|= ", " |  ", "    "],
            'U' => vec!["| | ", "\\=/ ", "    "],
            'V' => vec!["| | ", " V  ", "    "],
            'W' => vec!["| | ", "|^| ", "    "],
            'X' => vec!["\\/  ", "/\\  ", "    "],
            'Y' => vec!["\\/  ", " |  ", "    "],
            'Z' => vec!["==/ ", "/== ", "    "],
            ' ' => vec!["    ", "    ", "    "],
            _ => vec!["[?] ", "[?] ", "    "],
        }
    }

    fn render_slant(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec!["   /| ", "  /_| ", " /  | ", "/   | ", "      "],
            'B' => vec!["|===  ", "| __) ", "|  _) ", "|===  ", "      "],
            'C' => vec!["  ___/", " /    ", "|     ", " \\___\\", "      "],
            'D' => vec!["|===  ", "|   \\ ", "|   / ", "|===  ", "      "],
            'E' => vec!["|==== ", "|___  ", "|     ", "|==== ", "      "],
            'F' => vec!["|==== ", "|___  ", "|     ", "|     ", "      "],
            'G' => vec!["  ____", " /    ", "| /_  ", " \\__/ ", "      "],
            'H' => vec!["|   | ", "|===| ", "|   | ", "|   | ", "      "],
            'I' => vec!["  |   ", "  |   ", "  |   ", "  |   ", "      "],
            'J' => vec!["    | ", "    | ", " \\  | ", "  \\=/ ", "      "],
            'K' => vec!["|  /  ", "|-<   ", "|  \\  ", "|   \\ ", "      "],
            'L' => vec!["|     ", "|     ", "|     ", "|==== ", "      "],
            'M' => vec!["|\\  /|", "| \\/ |", "|    |", "|    |", "      "],
            'N' => vec!["|\\   |", "| \\  |", "|  \\ |", "|   \\|", "      "],
            'O' => vec!["  __  ", " /  \\ ", "|    |", " \\__/ ", "      "],
            'P' => vec!["|===\\ ", "|   | ", "|===/ ", "|     ", "      "],
            'Q' => vec!["  __  ", " /  \\ ", "|  \\ |", " \\__\\/", "      "],
            'R' => vec!["|===\\ ", "|   | ", "|===/ ", "|   \\ ", "      "],
            'S' => vec!["  ____", " (    ", "  === ", " ____)", "      "],
            'T' => vec!["====| ", "   |  ", "   |  ", "   |  ", "      "],
            'U' => vec!["|   | ", "|   | ", "|   | ", " \\=/ ", "      "],
            'V' => vec!["|   | ", " \\ /  ", "  |   ", "  .   ", "      "],
            'W' => vec!["|    |", "|/\\/\\|", "|    |", ".    .", "      "],
            'X' => vec!["\\   / ", " \\ /  ", " / \\  ", "/   \\ ", "      "],
            'Y' => vec!["\\   / ", " \\ /  ", "  |   ", "  |   ", "      "],
            'Z' => vec!["=====|", "    / ", "   /  ", "|=====", "      "],
            ' ' => vec!["      ", "      ", "      ", "      ", "      "],
            _ => vec!["[????]", "[????]", "[????]", "[????]", "      "],
        }
    }

    fn render_doom(&self, ch: char) -> Vec<&'static str> {
        // Doom-style large chunky letters
        match ch {
            'A' => vec![
                "   ██   ",
                "  ████  ",
                " ██  ██ ",
                "██    ██",
                "████████",
                "██    ██",
                "██    ██",
                "        ",
            ],
            'B' => vec![
                "██████  ",
                "██   ██ ",
                "██   ██ ",
                "██████  ",
                "██   ██ ",
                "██   ██ ",
                "██████  ",
                "        ",
            ],
            'C' => vec![
                " ██████ ",
                "██      ",
                "██      ",
                "██      ",
                "██      ",
                "██      ",
                " ██████ ",
                "        ",
            ],
            'D' => vec![
                "██████  ",
                "██   ██ ",
                "██    ██",
                "██    ██",
                "██    ██",
                "██   ██ ",
                "██████  ",
                "        ",
            ],
            'E' => vec![
                "████████",
                "██      ",
                "██      ",
                "██████  ",
                "██      ",
                "██      ",
                "████████",
                "        ",
            ],
            'F' => vec![
                "████████",
                "██      ",
                "██      ",
                "██████  ",
                "██      ",
                "██      ",
                "██      ",
                "        ",
            ],
            ' ' => vec![
                "        ", "        ", "        ", "        ", "        ", "        ", "        ",
                "        ",
            ],
            _ => vec![
                "████████",
                "██    ██",
                "██    ██",
                "██    ██",
                "██    ██",
                "██    ██",
                "████████",
                "        ",
            ],
        }
    }

    fn render_small_caps(&self, ch: char) -> Vec<&'static str> {
        // Unicode small caps
        match ch {
            'A' => vec!["ᴀ"],
            'B' => vec!["ʙ"],
            'C' => vec!["ᴄ"],
            'D' => vec!["ᴅ"],
            'E' => vec!["ᴇ"],
            'F' => vec!["ꜰ"],
            'G' => vec!["ɢ"],
            'H' => vec!["ʜ"],
            'I' => vec!["ɪ"],
            'J' => vec!["ᴊ"],
            'K' => vec!["ᴋ"],
            'L' => vec!["ʟ"],
            'M' => vec!["ᴍ"],
            'N' => vec!["ɴ"],
            'O' => vec!["ᴏ"],
            'P' => vec!["ᴘ"],
            'Q' => vec!["ǫ"],
            'R' => vec!["ʀ"],
            'S' => vec!["ꜱ"],
            'T' => vec!["ᴛ"],
            'U' => vec!["ᴜ"],
            'V' => vec!["ᴠ"],
            'W' => vec!["ᴡ"],
            'X' => vec!["x"],
            'Y' => vec!["ʏ"],
            'Z' => vec!["ᴢ"],
            ' ' => vec![" "],
            _ => vec!["?"],
        }
    }

    /// Render to frame at position with optional effects.
    pub fn render_at(&self, x: u16, y: u16, frame: &mut Frame, time: f64) {
        let lines = self.render_lines();
        let total_width: usize = lines.first().map(|l| l.chars().count()).unwrap_or(0);

        for (row, line) in lines.iter().enumerate() {
            let py = y.saturating_add(row as u16);
            for (col, ch) in line.chars().enumerate() {
                let px = x.saturating_add(col as u16);

                // Determine color
                let color = if let Some(ref gradient) = self.gradient {
                    let t = if total_width > 1 {
                        (col as f64 / (total_width - 1) as f64 + time * 0.2).rem_euclid(1.0)
                    } else {
                        0.5
                    };
                    gradient.sample(t)
                } else {
                    self.color.unwrap_or(PackedRgba::rgb(255, 255, 255))
                };

                if let Some(cell) = frame.buffer.get_mut(px, py) {
                    cell.content = CellContent::from_char(ch);
                    if ch != ' ' {
                        cell.fg = color;
                    }
                }
            }
        }
    }
}

// =============================================================================
// Sparkle Effect - Particles that twinkle
// =============================================================================

/// A single sparkle particle.
#[derive(Debug, Clone)]
pub struct Sparkle {
    pub x: f64,
    pub y: f64,
    pub brightness: f64,
    pub phase: f64,
}

/// Manages a collection of sparkle effects.
#[derive(Debug, Clone, Default)]
pub struct SparkleField {
    sparkles: Vec<Sparkle>,
    density: f64,
}

impl SparkleField {
    /// Create a new sparkle field.
    pub fn new(density: f64) -> Self {
        Self {
            sparkles: Vec::new(),
            density: density.clamp(0.0, 1.0),
        }
    }

    /// Initialize sparkles for an area.
    pub fn init_for_area(&mut self, width: u16, height: u16, seed: u64) {
        self.sparkles.clear();
        let count = ((width as f64 * height as f64) * self.density * 0.05) as usize;

        let mut rng = seed;
        for _ in 0..count {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (rng % width as u64) as f64;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y = (rng % height as u64) as f64;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let phase = (rng % 1000) as f64 / 1000.0 * TAU;

            self.sparkles.push(Sparkle {
                x,
                y,
                brightness: 1.0,
                phase,
            });
        }
    }

    /// Update sparkles for animation.
    pub fn update(&mut self, time: f64) {
        for sparkle in &mut self.sparkles {
            sparkle.brightness = 0.5 + 0.5 * (time * 3.0 + sparkle.phase).sin();
        }
    }

    /// Render sparkles to frame.
    pub fn render(&self, offset_x: u16, offset_y: u16, frame: &mut Frame) {
        for sparkle in &self.sparkles {
            let px = offset_x.saturating_add(sparkle.x as u16);
            let py = offset_y.saturating_add(sparkle.y as u16);

            if let Some(cell) = frame.buffer.get_mut(px, py) {
                let b = (sparkle.brightness * 255.0) as u8;
                // Use star characters for sparkle
                let ch = if sparkle.brightness > 0.8 {
                    '*'
                } else if sparkle.brightness > 0.5 {
                    '+'
                } else {
                    '.'
                };
                cell.content = CellContent::from_char(ch);
                cell.fg = PackedRgba::rgb(b, b, b.saturating_add(50));
            }
        }
    }
}

// =============================================================================
// Matrix/Cyber Characters
// =============================================================================

/// Characters for matrix/cyber style effects.
pub struct CyberChars;

impl CyberChars {
    /// Get a random cyber character based on seed.
    pub fn get(seed: u64) -> char {
        const CYBER_CHARS: &[char] = &[
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'ア', 'イ', 'ウ', 'エ', 'オ', 'カ',
            'キ', 'ク', 'ケ', 'コ', 'サ', 'シ', 'ス', 'セ', 'ソ', 'タ', 'チ', 'ツ', 'テ', 'ト',
            '/', '\\', '|', '-', '+', '*', '#', '@', '=', '>', '<', '[', ']', '{', '}', '(', ')',
            '$', '%', '&',
        ];
        let idx = (seed % CYBER_CHARS.len() as u64) as usize;
        CYBER_CHARS[idx]
    }

    /// Get a random printable ASCII character.
    pub fn ascii(seed: u64) -> char {
        let code = 33 + (seed % 94) as u8;
        code as char
    }

    /// Get half-width katakana characters for authentic Matrix effect.
    pub const HALF_WIDTH_KATAKANA: &'static [char] = &[
        'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ',
        'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ',
        'ﾕ', 'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ',
    ];

    /// Get a matrix character (half-width katakana + digits + symbols).
    pub fn matrix(seed: u64) -> char {
        const MATRIX_CHARS: &[char] = &[
            // Digits
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', // Half-width katakana
            'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ',
            'ﾂ', 'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ',
            'ﾓ', 'ﾔ', 'ﾕ', 'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ', // Latin capitals
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q',
            'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
        ];
        let idx = (seed % MATRIX_CHARS.len() as u64) as usize;
        MATRIX_CHARS[idx]
    }
}

// =============================================================================
// Matrix Rain Effect - Digital rain cascading down the screen
// =============================================================================

/// A single column of falling Matrix characters.
#[derive(Debug, Clone)]
pub struct MatrixColumn {
    /// Column x position.
    pub x: u16,
    /// Current y offset (can be negative for off-screen start).
    pub y_offset: f64,
    /// Falling speed (cells per update).
    pub speed: f64,
    /// Characters in the column with their brightness (0.0-1.0).
    pub chars: Vec<(char, f64)>,
    /// Maximum trail length.
    pub max_length: usize,
    /// RNG state for this column.
    rng_state: u64,
}

impl MatrixColumn {
    /// Create a new matrix column.
    pub fn new(x: u16, seed: u64) -> Self {
        let mut rng = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(x as u64);

        // Variable speed: 0.2 to 0.8 cells per update
        let speed = 0.2 + (rng % 600) as f64 / 1000.0;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);

        // Trail length: 8 to 28 characters
        let max_length = 8 + (rng % 20) as usize;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);

        // Start position: above screen
        let y_offset = -((rng % 30) as f64);

        Self {
            x,
            y_offset,
            speed,
            chars: Vec::with_capacity(max_length),
            max_length,
            rng_state: rng,
        }
    }

    /// Advance the RNG and return the next value.
    fn next_rng(&mut self) -> u64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.rng_state
    }

    /// Update the column state.
    pub fn update(&mut self) {
        // Move down
        self.y_offset += self.speed;

        // Fade existing characters
        for (_, brightness) in &mut self.chars {
            *brightness *= 0.92;
        }

        // Maybe add new character at head
        let rng = self.next_rng();
        if rng % 100 < 40 {
            // 40% chance to add new char
            let ch = CyberChars::matrix(self.next_rng());
            self.chars.insert(0, (ch, 1.0));
        }

        // Random character mutations
        let mutation_rng = self.next_rng();
        if mutation_rng % 100 < 15 && !self.chars.is_empty() {
            // 15% chance to mutate
            let idx = (self.next_rng() % self.chars.len() as u64) as usize;
            let new_char = CyberChars::matrix(self.next_rng());
            self.chars[idx].0 = new_char;
        }

        // Trim old characters that have faded
        self.chars.retain(|(_, b)| *b > 0.03);

        // Limit trail length
        if self.chars.len() > self.max_length {
            self.chars.truncate(self.max_length);
        }
    }

    /// Check if this column has scrolled completely off screen.
    pub fn is_offscreen(&self, height: u16) -> bool {
        let tail_y = self.y_offset as i32 - self.chars.len() as i32;
        tail_y > height as i32 + 5
    }

    /// Reset the column to start from above the screen.
    pub fn reset(&mut self, seed: u64) {
        let rng = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.x as u64);
        self.y_offset = -((rng % 30) as f64) - 5.0;
        self.chars.clear();
        self.rng_state = rng;

        // Randomize speed on reset
        let speed_rng = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.speed = 0.2 + (speed_rng % 600) as f64 / 1000.0;
    }
}

/// State manager for the Matrix rain effect.
#[derive(Debug, Clone)]
pub struct MatrixRainState {
    /// All active columns.
    columns: Vec<MatrixColumn>,
    /// Width of the display area.
    width: u16,
    /// Height of the display area.
    height: u16,
    /// Global seed for determinism.
    seed: u64,
    /// Frame counter for time-based effects.
    frame: u64,
    /// Whether the state has been initialized.
    initialized: bool,
}

impl Default for MatrixRainState {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixRainState {
    /// Create a new uninitialized Matrix rain state.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            width: 0,
            height: 0,
            seed: 42,
            frame: 0,
            initialized: false,
        }
    }

    /// Create with a specific seed for deterministic output.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            seed,
            ..Self::new()
        }
    }

    /// Initialize for a given area size.
    pub fn init(&mut self, width: u16, height: u16) {
        if self.initialized && self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;
        self.columns.clear();

        // Create a column for each x position, with some gaps
        for x in 0..width {
            let col_seed = self.seed.wrapping_add(x as u64 * 7919);
            // 70% chance to have a column at each position
            if col_seed % 100 < 70 {
                self.columns.push(MatrixColumn::new(x, col_seed));
            }
        }

        self.initialized = true;
    }

    /// Update all columns.
    pub fn update(&mut self) {
        if !self.initialized {
            return;
        }

        self.frame = self.frame.wrapping_add(1);

        for col in &mut self.columns {
            col.update();

            // Reset columns that have scrolled off screen
            if col.is_offscreen(self.height) {
                col.reset(
                    self.seed
                        .wrapping_add(self.frame)
                        .wrapping_add(col.x as u64),
                );
            }
        }

        // Occasionally spawn new columns in empty spots
        if self.frame.is_multiple_of(20) {
            for x in 0..self.width {
                let has_column = self.columns.iter().any(|c| c.x == x);
                if !has_column {
                    let spawn_rng = self
                        .seed
                        .wrapping_add(self.frame)
                        .wrapping_add(x as u64 * 31);
                    if spawn_rng % 100 < 3 {
                        // 3% chance per empty slot
                        self.columns.push(MatrixColumn::new(x, spawn_rng));
                    }
                }
            }
        }
    }

    /// Render the Matrix rain to a Frame.
    pub fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.initialized {
            return;
        }

        for col in &self.columns {
            // Check if column is in view
            if col.x < area.x || col.x >= area.x + area.width {
                continue;
            }

            let px = col.x;

            for (i, (ch, brightness)) in col.chars.iter().enumerate() {
                let char_y = col.y_offset as i32 - i as i32;

                // Skip if outside area
                if char_y < area.y as i32 || char_y >= (area.y + area.height) as i32 {
                    continue;
                }

                let py = char_y as u16;

                // Calculate color based on brightness
                // Head (i=0, brightness=1.0) is white-green
                // Tail fades to dark green
                let color = if i == 0 && *brightness > 0.95 {
                    // Bright white-green head
                    PackedRgba::rgb(180, 255, 180)
                } else if i == 0 {
                    // Slightly dimmed head
                    let g = (255.0 * brightness) as u8;
                    PackedRgba::rgb((g / 2).min(200), g, (g / 2).min(200))
                } else {
                    // Green tail with fade
                    let g = (220.0 * brightness) as u8;
                    let r = (g / 8).min(30);
                    let b = (g / 6).min(40);
                    PackedRgba::rgb(r, g, b)
                };

                // Write to frame buffer
                if let Some(cell) = frame.buffer.get_mut(px, py) {
                    cell.content = CellContent::from_char(*ch);
                    cell.fg = color;
                    // Keep background as-is or set to black for proper Matrix look
                    cell.bg = PackedRgba::rgb(0, 0, 0);
                }
            }
        }
    }

    /// Get the current frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame
    }

    /// Check if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get column count.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

// =============================================================================
// Matrix Rain Tests
// =============================================================================

#[cfg(test)]
mod matrix_rain_tests {
    use super::*;

    #[test]
    fn matrix_column_speeds_vary() {
        let col1 = MatrixColumn::new(0, 100);
        let col2 = MatrixColumn::new(1, 200);
        let col3 = MatrixColumn::new(2, 300);

        // Speeds should vary between columns
        assert!(
            col1.speed != col2.speed || col2.speed != col3.speed,
            "Column speeds should vary: {}, {}, {}",
            col1.speed,
            col2.speed,
            col3.speed
        );

        // All speeds should be in valid range
        assert!(col1.speed >= 0.2 && col1.speed <= 0.8);
        assert!(col2.speed >= 0.2 && col2.speed <= 0.8);
        assert!(col3.speed >= 0.2 && col3.speed <= 0.8);
    }

    #[test]
    fn matrix_update_progresses() {
        let mut col = MatrixColumn::new(5, 42);
        let initial_y = col.y_offset;

        col.update();

        assert!(col.y_offset > initial_y, "Update should increase y_offset");
    }

    #[test]
    fn matrix_char_brightness_fades() {
        let mut col = MatrixColumn::new(0, 12345);

        // Force some characters to be added
        for _ in 0..10 {
            let rng = col.next_rng();
            col.chars.insert(0, (CyberChars::matrix(rng), 1.0));
        }

        // Get initial brightness of non-head character
        let initial_brightness = col.chars.get(1).map(|(_, b)| *b).unwrap_or(1.0);

        // Update several times
        for _ in 0..5 {
            col.update();
        }

        // Brightness should have faded
        if let Some((_, brightness)) = col.chars.get(1) {
            assert!(
                *brightness < initial_brightness,
                "Brightness should fade over time"
            );
        }
    }

    #[test]
    fn matrix_katakana_chars_valid() {
        // Test that matrix chars are valid
        for seed in 0..100 {
            let ch = CyberChars::matrix(seed);
            assert!(
                ch.is_alphanumeric() || ch as u32 >= 0xFF61,
                "Character {} (seed {}) should be alphanumeric or katakana",
                ch,
                seed
            );
        }
    }

    #[test]
    fn matrix_state_initialization() {
        let mut state = MatrixRainState::with_seed(42);
        assert!(!state.is_initialized());

        state.init(80, 24);

        assert!(state.is_initialized());
        assert!(state.column_count() > 0);
        assert!(state.column_count() <= 80);
    }

    #[test]
    fn matrix_state_deterministic() {
        let mut state1 = MatrixRainState::with_seed(12345);
        let mut state2 = MatrixRainState::with_seed(12345);

        state1.init(40, 20);
        state2.init(40, 20);

        // Same seed should produce same column count
        assert_eq!(state1.column_count(), state2.column_count());

        // Update both
        for _ in 0..10 {
            state1.update();
            state2.update();
        }

        // Frame counts should match
        assert_eq!(state1.frame_count(), state2.frame_count());
    }

    #[test]
    fn matrix_columns_recycle() {
        let mut state = MatrixRainState::with_seed(99);
        state.init(10, 5); // Small area

        let initial_count = state.column_count();

        // Run many updates to cycle columns
        for _ in 0..200 {
            state.update();
        }

        // Should still have columns (recycled, not removed)
        assert!(state.column_count() > 0);
        // Count might vary slightly due to spawn/despawn logic
        assert!(state.column_count() >= initial_count / 2);
    }
}
