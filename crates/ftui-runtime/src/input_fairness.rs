//! Input fairness guard - stub module for compilation.
//!
//! TODO(bd-???): Implement full input fairness scheduling.
//!
//! This module provides placeholder types for input fairness scheduling
//! to allow compilation while the full implementation is pending.

#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

/// Event type for fairness classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// User input events (keyboard, mouse).
    Input,
    /// Terminal resize events.
    Resize,
    /// Timer tick events.
    Tick,
}

/// Type alias for compatibility with program.rs
pub type FairnessEventType = EventType;

/// Configuration for input fairness.
#[derive(Debug, Clone)]
pub struct FairnessConfig {
    /// Maximum latency for input events before they get priority.
    pub input_priority_threshold: Duration,
    /// Enable fairness scheduling.
    pub enabled: bool,
}

impl Default for FairnessConfig {
    fn default() -> Self {
        Self {
            input_priority_threshold: Duration::from_millis(50),
            enabled: false, // Disabled by default until fully implemented
        }
    }
}

/// Fairness decision returned by the guard.
#[derive(Debug, Clone)]
pub struct FairnessDecision {
    /// Whether to proceed with the event.
    pub should_process: bool,
    /// Pending input latency if any.
    pub pending_input_latency: Option<Duration>,
    /// Reason for the decision.
    pub reason: &'static str,
    /// Whether to yield to input processing.
    pub yield_to_input: bool,
    /// Jain fairness index (0.0-1.0).
    pub jain_index: f64,
}

impl Default for FairnessDecision {
    fn default() -> Self {
        Self {
            should_process: true,
            pending_input_latency: None,
            reason: "fairness_disabled",
            yield_to_input: false,
            jain_index: 1.0, // Perfect fairness when disabled
        }
    }
}

/// Fairness log entry for telemetry.
#[derive(Debug, Clone)]
pub struct FairnessLogEntry {
    /// Timestamp of the entry.
    pub timestamp: Instant,
    /// Event type processed.
    pub event_type: EventType,
    /// Duration of processing.
    pub duration: Duration,
}

/// Statistics about fairness scheduling.
#[derive(Debug, Clone, Default)]
pub struct FairnessStats {
    /// Total events processed.
    pub events_processed: u64,
    /// Input events processed.
    pub input_events: u64,
    /// Resize events processed.
    pub resize_events: u64,
    /// Tick events processed.
    pub tick_events: u64,
}

/// Intervention reason for fairness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterventionReason {
    /// Input latency exceeded threshold.
    InputLatency,
    /// No intervention needed.
    None,
}

/// Counts of interventions by type.
#[derive(Debug, Clone, Default)]
pub struct InterventionCounts {
    /// Input latency interventions.
    pub input_latency: u64,
}

/// Guard for input fairness scheduling.
///
/// Stub implementation - returns default decisions.
#[derive(Debug)]
pub struct InputFairnessGuard {
    config: FairnessConfig,
    stats: FairnessStats,
}

impl InputFairnessGuard {
    /// Create a new fairness guard with default configuration.
    pub fn new() -> Self {
        Self::with_config(FairnessConfig::default())
    }

    /// Create a new fairness guard with the given configuration.
    pub fn with_config(config: FairnessConfig) -> Self {
        Self {
            config,
            stats: FairnessStats::default(),
        }
    }

    /// Check fairness and return a decision.
    ///
    /// Stub implementation - always returns default decision.
    pub fn check_fairness(&self, _now: Instant) -> FairnessDecision {
        FairnessDecision::default()
    }

    /// Record that an event was processed.
    pub fn event_processed(&mut self, event_type: EventType, _duration: Duration, _now: Instant) {
        self.stats.events_processed += 1;
        match event_type {
            EventType::Input => self.stats.input_events += 1,
            EventType::Resize => self.stats.resize_events += 1,
            EventType::Tick => self.stats.tick_events += 1,
        }
    }

    /// Get current statistics.
    pub fn stats(&self) -> &FairnessStats {
        &self.stats
    }

    /// Check if fairness is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl Default for InputFairnessGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = FairnessConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn default_decision_allows_processing() {
        let guard = InputFairnessGuard::default();
        let decision = guard.check_fairness(Instant::now());
        assert!(decision.should_process);
    }

    #[test]
    fn event_processing_updates_stats() {
        let mut guard = InputFairnessGuard::default();
        let now = Instant::now();

        guard.event_processed(EventType::Input, Duration::from_millis(10), now);
        guard.event_processed(EventType::Resize, Duration::from_millis(5), now);
        guard.event_processed(EventType::Tick, Duration::from_millis(1), now);

        let stats = guard.stats();
        assert_eq!(stats.events_processed, 3);
        assert_eq!(stats.input_events, 1);
        assert_eq!(stats.resize_events, 1);
        assert_eq!(stats.tick_events, 1);
    }
}
