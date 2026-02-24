//! Markov-chain prediction API for screen transitions.
//!
//! [`MarkovPredictor<S>`] wraps a [`TransitionCounter<S>`] and provides a clean
//! prediction interface with confidence-weighted probabilities. The Predictive
//! tick strategy (C.5) queries this to determine tick rates for each screen.
//!
//! # Confidence model
//!
//! Predictions blend between uniform (cold start) and observed (warm) based on
//! the number of transitions recorded from the current screen:
//!
//! ```text
//! confidence = min(1.0, observations / min_observations)
//! effective  = confidence * predicted + (1 - confidence) * uniform
//! ```
//!
//! This ensures smooth warm-up from equal-probability to learned behavior.

use std::hash::Hash;

use super::TransitionCounter;

/// Default minimum observations before predictions are fully trusted.
const DEFAULT_MIN_OBSERVATIONS: u64 = 20;

/// A single screen prediction with probability and confidence.
#[derive(Debug, Clone)]
pub struct ScreenPrediction<S> {
    /// The predicted next screen.
    pub screen: S,
    /// Probability of transitioning to this screen (0.0..1.0).
    pub probability: f64,
    /// Confidence in this prediction (0.0..1.0), based on observation count.
    pub confidence: f64,
}

/// Markov-chain predictor wrapping a [`TransitionCounter`].
///
/// Provides prediction queries with confidence-aware blending between
/// observed transition probabilities and a uniform fallback.
#[derive(Debug, Clone)]
pub struct MarkovPredictor<S: Eq + Hash + Clone> {
    counter: TransitionCounter<S>,
    min_observations: u64,
}

impl<S: Eq + Hash + Clone> MarkovPredictor<S> {
    /// Create a predictor with default settings (min_observations=20, alpha=1.0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            counter: TransitionCounter::new(),
            min_observations: DEFAULT_MIN_OBSERVATIONS,
        }
    }

    /// Create a predictor with a custom observation threshold.
    #[must_use]
    pub fn with_min_observations(n: u64) -> Self {
        Self {
            counter: TransitionCounter::new(),
            min_observations: n.max(1),
        }
    }

    /// Create a predictor with a pre-loaded counter (e.g., from persistence).
    #[must_use]
    pub fn with_counter(counter: TransitionCounter<S>, min_observations: u64) -> Self {
        Self {
            counter,
            min_observations: min_observations.max(1),
        }
    }

    /// Record a screen transition.
    pub fn record_transition(&mut self, from: S, to: S) {
        self.counter.record(from, to);
    }

    /// Predict likely next screens from `current_screen`.
    ///
    /// Returns predictions sorted by effective probability (descending).
    /// Each prediction blends observed and uniform probabilities based on
    /// confidence.
    #[must_use]
    pub fn predict(&self, current_screen: &S) -> Vec<ScreenPrediction<S>> {
        let confidence = self.confidence(current_screen);
        let ranked = self.counter.all_targets_ranked(current_screen);

        if ranked.is_empty() {
            return Vec::new();
        }

        let n_targets = ranked.len() as f64;
        let uniform_prob = 1.0 / n_targets;

        let mut predictions: Vec<ScreenPrediction<S>> = ranked
            .into_iter()
            .map(|(screen, raw_prob)| {
                let effective = confidence * raw_prob + (1.0 - confidence) * uniform_prob;
                ScreenPrediction {
                    screen,
                    probability: effective,
                    confidence,
                }
            })
            .collect();

        // Re-sort by effective probability (blending can change order during warm-up)
        predictions.sort_by(|a, b| {
            b.probability
                .partial_cmp(&a.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        predictions
    }

    /// Check whether predictions from `screen` have insufficient data.
    ///
    /// Returns `true` if fewer than `min_observations` transitions have
    /// been recorded from this screen.
    #[must_use]
    pub fn is_cold_start(&self, screen: &S) -> bool {
        (self.counter.total_from(screen) as u64) < self.min_observations
    }

    /// Confidence level for predictions from `screen` (0.0..1.0).
    ///
    /// Grows linearly from 0 to 1 as observations approach `min_observations`.
    #[must_use]
    pub fn confidence(&self, screen: &S) -> f64 {
        let observations = self.counter.total_from(screen);
        (observations / self.min_observations as f64).min(1.0)
    }

    /// Access the underlying transition counter.
    #[must_use]
    pub fn counter(&self) -> &TransitionCounter<S> {
        &self.counter
    }

    /// Mutable access to the transition counter (for merging, decay, etc.).
    pub fn counter_mut(&mut self) -> &mut TransitionCounter<S> {
        &mut self.counter
    }

    /// Return the minimum observations threshold.
    #[must_use]
    pub fn min_observations(&self) -> u64 {
        self.min_observations
    }
}

impl<S: Eq + Hash + Clone> Default for MarkovPredictor<S> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_start_returns_uniform_distribution() {
        let mut mp = MarkovPredictor::with_min_observations(20);
        // Record just 1 transition (well below threshold of 20)
        mp.record_transition("a", "b");
        mp.record_transition("a", "c");

        let preds = mp.predict(&"a");
        assert_eq!(preds.len(), 2);

        // With only 2 observations out of 20 needed, confidence = 2/20 = 0.1
        // Effective probs should be close to uniform (0.5 each)
        let diff = (preds[0].probability - preds[1].probability).abs();
        assert!(
            diff < 0.15,
            "cold start should be near-uniform, diff={diff}"
        );
    }

    #[test]
    fn warm_predictions_match_observed() {
        let mut mp = MarkovPredictor::with_min_observations(10);

        // Record 30 total transitions (well above threshold)
        for _ in 0..20 {
            mp.record_transition("a", "b");
        }
        for _ in 0..10 {
            mp.record_transition("a", "c");
        }

        let preds = mp.predict(&"a");
        assert_eq!(preds.len(), 2);

        // Confidence should be 1.0 (30 >= 10)
        assert!((preds[0].confidence - 1.0).abs() < 1e-10);

        // "b" should have higher probability than "c"
        assert_eq!(preds[0].screen, "b");
        assert!(preds[0].probability > preds[1].probability);

        // With full confidence, effective = raw probability
        // P(b|a) with smoothing: (20+1)/(30+2) ≈ 0.656
        // P(c|a) with smoothing: (10+1)/(30+2) ≈ 0.344
        assert!((preds[0].probability - 21.0 / 32.0).abs() < 1e-10);
        assert!((preds[1].probability - 11.0 / 32.0).abs() < 1e-10);
    }

    #[test]
    fn confidence_increases_with_observations() {
        let mut mp = MarkovPredictor::with_min_observations(10);

        assert_eq!(mp.confidence(&"x"), 0.0); // no data

        mp.record_transition("x", "y");
        assert!((mp.confidence(&"x") - 0.1).abs() < 1e-10); // 1/10

        for _ in 0..4 {
            mp.record_transition("x", "y");
        }
        assert!((mp.confidence(&"x") - 0.5).abs() < 1e-10); // 5/10

        for _ in 0..5 {
            mp.record_transition("x", "y");
        }
        assert!((mp.confidence(&"x") - 1.0).abs() < 1e-10); // 10/10
    }

    #[test]
    fn confidence_caps_at_one() {
        let mut mp = MarkovPredictor::with_min_observations(5);
        for _ in 0..100 {
            mp.record_transition("a", "b");
        }
        assert!((mp.confidence(&"a") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn is_cold_start_reflects_threshold() {
        let mut mp = MarkovPredictor::with_min_observations(5);
        assert!(mp.is_cold_start(&"x"));

        for _ in 0..4 {
            mp.record_transition("x", "y");
        }
        assert!(mp.is_cold_start(&"x")); // 4 < 5

        mp.record_transition("x", "y");
        assert!(!mp.is_cold_start(&"x")); // 5 >= 5
    }

    #[test]
    fn empty_predictor_returns_no_predictions() {
        let mp: MarkovPredictor<&str> = MarkovPredictor::new();
        let preds = mp.predict(&"x");
        assert!(preds.is_empty());
    }

    #[test]
    fn predictions_sorted_by_probability() {
        let mut mp = MarkovPredictor::with_min_observations(5);
        for _ in 0..10 {
            mp.record_transition("a", "x");
        }
        for _ in 0..5 {
            mp.record_transition("a", "y");
        }
        for _ in 0..1 {
            mp.record_transition("a", "z");
        }

        let preds = mp.predict(&"a");
        assert_eq!(preds.len(), 3);
        assert!(preds[0].probability >= preds[1].probability);
        assert!(preds[1].probability >= preds[2].probability);
    }

    #[test]
    fn probabilities_sum_to_approximately_one() {
        let mut mp = MarkovPredictor::with_min_observations(10);
        for _ in 0..15 {
            mp.record_transition("a", "b");
        }
        for _ in 0..8 {
            mp.record_transition("a", "c");
        }
        for _ in 0..3 {
            mp.record_transition("a", "d");
        }

        let preds = mp.predict(&"a");
        let sum: f64 = preds.iter().map(|p| p.probability).sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "probabilities should sum to 1.0, got {sum}"
        );
    }

    #[test]
    fn counter_access() {
        let mut mp = MarkovPredictor::<&str>::new();
        mp.record_transition("a", "b");

        assert_eq!(mp.counter().total(), 1.0);
        assert_eq!(mp.counter().count(&"a", &"b"), 1.0);
    }

    #[test]
    fn counter_mut_access() {
        let mut mp = MarkovPredictor::<&str>::new();
        mp.record_transition("a", "b");

        // Merge via mutable access
        let mut other = TransitionCounter::new();
        other.record("a", "c");
        mp.counter_mut().merge(&other);

        assert_eq!(mp.counter().total(), 2.0);
    }

    #[test]
    fn with_counter_constructor() {
        let mut counter = TransitionCounter::new();
        for _ in 0..50 {
            counter.record("a", "b");
        }

        let mp = MarkovPredictor::with_counter(counter, 10);
        assert!(!mp.is_cold_start(&"a"));
        assert_eq!(mp.min_observations(), 10);
    }

    #[test]
    fn default_impl() {
        let mp: MarkovPredictor<String> = MarkovPredictor::default();
        assert_eq!(mp.min_observations(), DEFAULT_MIN_OBSERVATIONS);
        assert_eq!(mp.counter().total(), 0.0);
    }

    #[test]
    fn blending_transitions_smoothly() {
        // With min_observations=10, after 5 transitions we're at 50% confidence.
        // The predictions should blend 50% observed + 50% uniform.
        let mut mp = MarkovPredictor::with_min_observations(10);

        // 4 transitions A→B, 1 transition A→C (total from A = 5)
        for _ in 0..4 {
            mp.record_transition("a", "b");
        }
        mp.record_transition("a", "c");

        let preds = mp.predict(&"a");
        assert_eq!(preds.len(), 2);

        // confidence = 5/10 = 0.5
        let conf = mp.confidence(&"a");
        assert!((conf - 0.5).abs() < 1e-10);

        // Raw P(b|a) with smoothing: (4+1)/(5+2) = 5/7 ≈ 0.714
        // Raw P(c|a) with smoothing: (1+1)/(5+2) = 2/7 ≈ 0.286
        // Uniform: 0.5 each
        // Effective P(b) = 0.5 * 5/7 + 0.5 * 0.5 = 5/14 + 1/4 = 10/28 + 7/28 = 17/28 ≈ 0.607
        // Effective P(c) = 0.5 * 2/7 + 0.5 * 0.5 = 2/14 + 1/4 = 4/28 + 7/28 = 11/28 ≈ 0.393
        let expected_b = 0.5 * (5.0 / 7.0) + 0.5 * 0.5;
        let expected_c = 0.5 * (2.0 / 7.0) + 0.5 * 0.5;

        assert_eq!(preds[0].screen, "b");
        assert!(
            (preds[0].probability - expected_b).abs() < 1e-10,
            "expected {expected_b}, got {}",
            preds[0].probability
        );
        assert!(
            (preds[1].probability - expected_c).abs() < 1e-10,
            "expected {expected_c}, got {}",
            preds[1].probability
        );

        // Sum should still be ~1.0
        let sum: f64 = preds.iter().map(|p| p.probability).sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }
}
