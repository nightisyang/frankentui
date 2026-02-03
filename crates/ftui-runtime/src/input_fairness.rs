//! Input Fairness Guard (stub implementation)
//!
//! Prevents render/resize events from starving keyboard/mouse input.
//! This is a placeholder implementation - full implementation pending.

use std::time::{Duration, Instant};

/// Event type classification for fairness tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FairnessEventType {
    /// Keyboard or mouse input
    Input,
    /// Terminal resize event
    Resize,
    /// Timer tick event
    Tick,
}

/// Fairness decision returned by the guard.
#[derive(Debug, Clone)]
pub struct FairnessDecision {
    /// Whether to proceed with processing
    pub should_process: bool,
    /// Time input has been pending (if any)
    pub pending_input_latency: Option<Duration>,
    /// Suggested yield duration if not processing
    pub suggested_yield: Option<Duration>,
}

impl Default for FairnessDecision {
    fn default() -> Self {
        Self {
            should_process: true,
            pending_input_latency: None,
            suggested_yield: None,
        }
    }
}

/// Guard that ensures input events are not starved by render/resize.
#[derive(Debug, Default)]
pub struct InputFairnessGuard {
    /// When input last arrived
    pending_input_arrival: Option<Instant>,
}

impl InputFairnessGuard {
    /// Create a new fairness guard.
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal that input has arrived.
    pub fn input_arrived(&mut self, _when: Instant) {
        self.pending_input_arrival = Some(Instant::now());
    }

    /// Signal that an event has been processed.
    pub fn event_processed(
        &mut self,
        event_type: FairnessEventType,
        _processing_time: Duration,
        _completed_at: Instant,
    ) {
        if event_type == FairnessEventType::Input {
            self.pending_input_arrival = None;
        }
    }

    /// Check if we should proceed or yield to input.
    pub fn check_fairness(&self, _now: Instant) -> FairnessDecision {
        FairnessDecision::default()
    }

    /// Get pending input latency if any.
    pub fn pending_input_latency(&self) -> Option<Duration> {
        self.pending_input_arrival.map(|t| t.elapsed())
    }
}
