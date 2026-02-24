//! [`Predictive`] strategy: Markov-chain-driven tick allocation.
//!
//! The crown jewel of the tick strategy system. Uses a [`MarkovPredictor`] to
//! learn screen transition patterns and a [`TickAllocation`] to convert
//! predicted probabilities into tick divisors. Screens the user is likely to
//! switch to get ticked more frequently; unlikely screens get throttled.
//!
//! Gracefully degrades to uniform behavior during cold start (insufficient
//! data) via confidence-weighted blending in [`MarkovPredictor`].

use std::collections::HashMap;

use super::{MarkovPredictor, TickAllocation, TickDecision, TickStrategy, TransitionCounter};

/// Configuration for the [`Predictive`] tick strategy.
#[derive(Debug, Clone)]
pub struct PredictiveStrategyConfig {
    /// How probabilities map to tick divisors.
    pub allocation: TickAllocation,
    /// Divisor used for unknown screens (not in the predictor's vocabulary).
    pub fallback_divisor: u64,
    /// Minimum observations before predictions are fully trusted.
    pub min_observations: u64,
    /// Temporal decay factor applied during maintenance (0.0..1.0).
    pub decay_factor: f64,
    /// How many ticks between decay cycles.
    pub decay_interval: u64,
    /// How many ticks between auto-save cycles (0 = disabled).
    pub auto_save_interval: u64,
}

impl Default for PredictiveStrategyConfig {
    fn default() -> Self {
        Self {
            allocation: TickAllocation::default(),
            fallback_divisor: 5,
            min_observations: 20,
            decay_factor: 0.85,
            decay_interval: 500,
            auto_save_interval: 3000,
        }
    }
}

/// Markov-chain-driven tick strategy.
///
/// Learns screen transition patterns and allocates tick budget proportionally
/// to transition probability. High-probability next screens tick at near-full
/// rate; low-probability screens are aggressively throttled.
///
/// See module-level docs for architecture details.
#[derive(Debug, Clone)]
pub struct Predictive {
    predictor: MarkovPredictor<String>,
    allocation: TickAllocation,
    fallback_divisor: u64,
    decay_factor: f64,
    decay_interval: u64,
    auto_save_interval: u64,
    /// Cache: recomputed only when active screen changes.
    cached_divisors: HashMap<String, u64>,
    /// The active screen the cache was computed for.
    cached_for_screen: Option<String>,
    /// Tick counter for maintenance scheduling.
    ticks_since_decay: u64,
    /// Tick counter for auto-save scheduling.
    ticks_since_save: u64,
    /// Whether there's unsaved data (transitions recorded since last save).
    dirty: bool,
}

impl Predictive {
    /// Create a new predictive strategy with the given config.
    #[must_use]
    pub fn new(config: PredictiveStrategyConfig) -> Self {
        Self {
            predictor: MarkovPredictor::with_min_observations(config.min_observations),
            allocation: config.allocation,
            fallback_divisor: config.fallback_divisor.max(1),
            decay_factor: config.decay_factor,
            decay_interval: config.decay_interval.max(1),
            auto_save_interval: config.auto_save_interval,
            cached_divisors: HashMap::new(),
            cached_for_screen: None,
            ticks_since_decay: 0,
            ticks_since_save: 0,
            dirty: false,
        }
    }

    /// Create with pre-loaded transition data (e.g., from persistence).
    #[must_use]
    pub fn with_counter(
        config: PredictiveStrategyConfig,
        counter: TransitionCounter<String>,
    ) -> Self {
        Self {
            predictor: MarkovPredictor::with_counter(counter, config.min_observations),
            allocation: config.allocation,
            fallback_divisor: config.fallback_divisor.max(1),
            decay_factor: config.decay_factor,
            decay_interval: config.decay_interval.max(1),
            auto_save_interval: config.auto_save_interval,
            cached_divisors: HashMap::new(),
            cached_for_screen: None,
            ticks_since_decay: 0,
            ticks_since_save: 0,
            dirty: false,
        }
    }

    /// Access the underlying predictor.
    #[must_use]
    pub fn predictor(&self) -> &MarkovPredictor<String> {
        &self.predictor
    }

    /// Access the underlying transition counter.
    #[must_use]
    pub fn counter(&self) -> &TransitionCounter<String> {
        self.predictor.counter()
    }

    /// Whether there is unsaved transition data.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Recompute the cached divisor map for predictions from `active`.
    fn refresh_cache(&mut self, active: &str) {
        if self.cached_for_screen.as_deref() == Some(active) {
            return;
        }

        self.cached_divisors.clear();
        let predictions = self.predictor.predict(&active.to_owned());

        for p in &predictions {
            let divisor = self.allocation.divisor_for(p.probability);
            self.cached_divisors.insert(p.screen.clone(), divisor);
        }

        self.cached_for_screen = Some(active.to_owned());
    }
}

impl TickStrategy for Predictive {
    fn should_tick(
        &mut self,
        screen_id: &str,
        tick_count: u64,
        active_screen: &str,
    ) -> TickDecision {
        // Ensure cache is fresh for current active screen.
        self.refresh_cache(active_screen);

        let divisor = self
            .cached_divisors
            .get(screen_id)
            .copied()
            .unwrap_or(self.fallback_divisor);

        if tick_count.is_multiple_of(divisor) {
            TickDecision::Tick
        } else {
            TickDecision::Skip
        }
    }

    fn on_screen_transition(&mut self, from: &str, to: &str) {
        self.predictor
            .record_transition(from.to_owned(), to.to_owned());
        self.dirty = true;
        // Force cache refresh for the new active screen.
        self.cached_for_screen = None;
        self.refresh_cache(to);
    }

    fn maintenance_tick(&mut self, _tick_count: u64) {
        self.ticks_since_decay += 1;
        self.ticks_since_save += 1;

        // Periodic decay.
        if self.ticks_since_decay >= self.decay_interval {
            self.predictor.counter_mut().decay(self.decay_factor);
            self.ticks_since_decay = 0;
            // Invalidate cache since probabilities changed.
            self.cached_for_screen = None;
        }

        // Auto-save tracking (actual I/O is handled by the persistence layer).
        // We just mark save-ready state here.
        if self.auto_save_interval > 0 && self.ticks_since_save >= self.auto_save_interval {
            self.ticks_since_save = 0;
            // Persistence layer (E.1/E.3) will check is_dirty() and counter().
        }
    }

    fn shutdown(&mut self) {
        // Persistence layer handles actual saving.
        // The strategy just ensures its state is consistent for reading.
    }

    fn name(&self) -> &str {
        "Predictive"
    }

    fn debug_stats(&self) -> Vec<(String, String)> {
        let confidence = self
            .cached_for_screen
            .as_ref()
            .map(|s| self.predictor.confidence(s))
            .unwrap_or(0.0);

        vec![
            ("strategy".into(), "Predictive".into()),
            (
                "total_transitions".into(),
                format!("{:.0}", self.predictor.counter().total()),
            ),
            (
                "known_screens".into(),
                self.predictor.counter().state_ids().len().to_string(),
            ),
            (
                "cached_divisors".into(),
                self.cached_divisors.len().to_string(),
            ),
            (
                "active_screen".into(),
                self.cached_for_screen
                    .as_deref()
                    .unwrap_or("(none)")
                    .to_owned(),
            ),
            ("confidence".into(), format!("{confidence:.2}")),
            ("fallback_divisor".into(), self.fallback_divisor.to_string()),
            ("dirty".into(), self.dirty.to_string()),
        ]
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> PredictiveStrategyConfig {
        PredictiveStrategyConfig {
            min_observations: 5, // low threshold for test speed
            fallback_divisor: 10,
            decay_interval: 100,
            ..PredictiveStrategyConfig::default()
        }
    }

    #[test]
    fn cold_start_uses_fallback_divisor() {
        let mut s = Predictive::new(default_config());
        // No transition data → unknown screen → fallback divisor
        assert_eq!(s.should_tick("x", 0, "a"), TickDecision::Tick); // 0 % 10 == 0
        assert_eq!(s.should_tick("x", 1, "a"), TickDecision::Skip);
        assert_eq!(s.should_tick("x", 9, "a"), TickDecision::Skip);
        assert_eq!(s.should_tick("x", 10, "a"), TickDecision::Tick); // 10 % 10 == 0
    }

    #[test]
    fn learns_and_adjusts_divisors() {
        let config = PredictiveStrategyConfig {
            min_observations: 5,
            fallback_divisor: 20,
            ..PredictiveStrategyConfig::default()
        };
        let mut s = Predictive::new(config);

        // Record many a→b transitions
        for _ in 0..20 {
            s.on_screen_transition("a", "b");
        }
        // Record few a→c transitions
        for _ in 0..2 {
            s.on_screen_transition("a", "c");
        }

        // Now when active is "a", "b" should have a lower divisor than "c"
        s.refresh_cache("a");
        let b_div = s.cached_divisors.get("b").copied().unwrap_or(99);
        let c_div = s.cached_divisors.get("c").copied().unwrap_or(99);
        assert!(
            b_div < c_div,
            "b should tick more: b_div={b_div}, c_div={c_div}"
        );
    }

    #[test]
    fn cache_refreshes_on_screen_transition() {
        let mut s = Predictive::new(default_config());
        s.on_screen_transition("a", "b");
        assert_eq!(s.cached_for_screen.as_deref(), Some("b"));

        s.on_screen_transition("b", "c");
        assert_eq!(s.cached_for_screen.as_deref(), Some("c"));
    }

    #[test]
    fn cache_reused_for_same_screen() {
        let mut s = Predictive::new(default_config());
        s.on_screen_transition("a", "b");

        // First call refreshes cache
        s.should_tick("x", 1, "b");
        let cached = s.cached_for_screen.clone();

        // Second call reuses cache (same active screen)
        s.should_tick("x", 2, "b");
        assert_eq!(s.cached_for_screen, cached);
    }

    #[test]
    fn unknown_screen_uses_fallback() {
        let mut s = Predictive::new(default_config());
        s.on_screen_transition("a", "b");

        // "unknown" not in any prediction → fallback_divisor
        let div = s
            .cached_divisors
            .get("unknown")
            .copied()
            .unwrap_or(s.fallback_divisor);
        assert_eq!(div, 10); // default_config fallback
    }

    #[test]
    fn decay_triggers_at_interval() {
        let config = PredictiveStrategyConfig {
            decay_interval: 10,
            decay_factor: 0.5,
            min_observations: 5,
            ..PredictiveStrategyConfig::default()
        };
        let mut s = Predictive::new(config);
        s.on_screen_transition("a", "b");
        let before = s.predictor.counter().total();

        // Simulate maintenance ticks
        for _ in 0..10 {
            s.maintenance_tick(0);
        }

        let after = s.predictor.counter().total();
        assert!(
            after < before,
            "decay should reduce total: {after} < {before}"
        );
    }

    #[test]
    fn dirty_flag_set_on_transition() {
        let mut s = Predictive::new(default_config());
        assert!(!s.is_dirty());

        s.on_screen_transition("a", "b");
        assert!(s.is_dirty());
    }

    #[test]
    fn name_is_stable() {
        let s = Predictive::new(default_config());
        assert_eq!(s.name(), "Predictive");
    }

    #[test]
    fn debug_stats_populated() {
        let mut s = Predictive::new(default_config());
        s.on_screen_transition("a", "b");

        let stats = s.debug_stats();
        assert!(!stats.is_empty());
        assert!(stats.iter().any(|(k, _)| k == "strategy"));
        assert!(stats.iter().any(|(k, _)| k == "total_transitions"));
        assert!(stats.iter().any(|(k, _)| k == "confidence"));
    }

    #[test]
    fn with_counter_preloads_data() {
        let mut counter = TransitionCounter::new();
        for _ in 0..50 {
            counter.record("a".to_owned(), "b".to_owned());
        }

        let s = Predictive::with_counter(default_config(), counter);
        assert!(!s.predictor().is_cold_start(&"a".to_owned()));
    }

    #[test]
    fn high_probability_screen_ticks_more() {
        let config = PredictiveStrategyConfig {
            min_observations: 5,
            fallback_divisor: 20,
            ..PredictiveStrategyConfig::default()
        };
        let mut s = Predictive::new(config);

        // Build strong signal: a→b is very likely
        for _ in 0..30 {
            s.on_screen_transition("a", "b");
        }
        s.on_screen_transition("a", "c");

        // Count ticks over 100 frames for each screen
        let mut b_ticks = 0u64;
        let mut c_ticks = 0u64;
        for tick in 0..100 {
            if s.should_tick("b", tick, "a") == TickDecision::Tick {
                b_ticks += 1;
            }
            if s.should_tick("c", tick, "a") == TickDecision::Tick {
                c_ticks += 1;
            }
        }

        assert!(
            b_ticks > c_ticks,
            "b should tick more than c: b={b_ticks}, c={c_ticks}"
        );
    }
}
