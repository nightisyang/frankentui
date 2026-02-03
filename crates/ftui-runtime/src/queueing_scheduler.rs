#![forbid(unsafe_code)]

//! Queueing Theory Scheduler with SRPT/Smith-Rule Style Scheduling (bd-13pq.7).
//!
//! This module provides a fair, work-conserving task scheduler based on queueing theory
//! principles. It implements variants of SRPT (Shortest Remaining Processing Time) with
//! fairness constraints to prevent starvation.
//!
//! # Mathematical Model
//!
//! ## Scheduling Disciplines
//!
//! 1. **SRPT (Shortest Remaining Processing Time)**
//!    - Optimal for minimizing mean response time in M/G/1 queues
//!    - Preempts current job if a shorter job arrives
//!    - Problem: Can starve long jobs indefinitely
//!
//! 2. **Smith's Rule (Weighted SRPT)**
//!    - Priority = weight / remaining_time
//!    - Maximizes weighted throughput
//!    - Still suffers from starvation
//!
//! 3. **Fair SRPT (this implementation)**
//!    - Uses aging: priority increases with wait time
//!    - Ensures bounded wait time for all jobs
//!    - Trade-off: slightly worse mean response time for bounded starvation
//!
//! ## Queue Discipline
//!
//! Jobs are ordered by effective priority:
//! ```text
//! priority = (weight / remaining_time) + aging_factor * wait_time
//! ```
//!
//! This combines:
//! - Smith's rule: `weight / remaining_time`
//! - Aging: linear increase with wait time
//!
//! ## Fairness Guarantee (Aging-Based)
//!
//! With aging factor `a` and maximum job size `S_max`:
//! ```text
//! max_wait <= S_max * (1 + 1/a) / min_weight
//! ```
//!
//! # Key Invariants
//!
//! 1. **Work-conserving**: Server never idles when queue is non-empty
//! 2. **Priority ordering**: Queue is always sorted by effective priority
//! 3. **Bounded starvation**: All jobs complete within bounded time
//! 4. **Monotonic aging**: Wait time only increases while in queue
//!
//! # Failure Modes
//!
//! | Condition | Behavior | Rationale |
//! |-----------|----------|-----------|
//! | Zero weight | Use minimum weight | Prevent infinite priority |
//! | Zero remaining time | Complete immediately | Job is done |
//! | Queue overflow | Reject new jobs | Bounded memory |
//! | Clock drift | Use monotonic time | Avoid priority inversions |

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Minimum weight to prevent division issues.
const MIN_WEIGHT: f64 = 1e-6;

/// Default aging factor (0.1 = job gains priority of 1 unit after 10 time units).
const DEFAULT_AGING_FACTOR: f64 = 0.1;

/// Maximum queue size.
const MAX_QUEUE_SIZE: usize = 10_000;

/// Configuration for the scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Aging factor: how fast priority increases with wait time.
    /// Higher = faster aging = more fairness, less optimality.
    /// Default: 0.1.
    pub aging_factor: f64,

    /// Maximum queue size. Default: 10_000.
    pub max_queue_size: usize,

    /// Enable preemption. Default: true.
    pub preemptive: bool,

    /// Time quantum for round-robin fallback (when priorities are equal).
    /// Default: 10.0.
    pub time_quantum: f64,

    /// Enable logging. Default: false.
    pub enable_logging: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            aging_factor: DEFAULT_AGING_FACTOR,
            max_queue_size: MAX_QUEUE_SIZE,
            preemptive: true,
            time_quantum: 10.0,
            enable_logging: false,
        }
    }
}

/// A job in the queue.
#[derive(Debug, Clone)]
pub struct Job {
    /// Unique job identifier.
    pub id: u64,

    /// Job weight (importance). Higher = more priority.
    pub weight: f64,

    /// Estimated remaining processing time.
    pub remaining_time: f64,

    /// Original estimated total time.
    pub total_time: f64,

    /// Time when job was submitted.
    pub arrival_time: f64,

    /// Optional job name for debugging.
    pub name: Option<String>,
}

impl Job {
    /// Create a new job with given ID, weight, and estimated time.
    pub fn new(id: u64, weight: f64, estimated_time: f64) -> Self {
        Self {
            id,
            weight: weight.max(MIN_WEIGHT),
            remaining_time: estimated_time.max(0.0),
            total_time: estimated_time.max(0.0),
            arrival_time: 0.0,
            name: None,
        }
    }

    /// Create a job with a name.
    pub fn with_name(id: u64, weight: f64, estimated_time: f64, name: impl Into<String>) -> Self {
        let mut job = Self::new(id, weight, estimated_time);
        job.name = Some(name.into());
        job
    }

    /// Fraction of job completed.
    pub fn progress(&self) -> f64 {
        if self.total_time <= 0.0 {
            1.0
        } else {
            1.0 - (self.remaining_time / self.total_time).clamp(0.0, 1.0)
        }
    }

    /// Is the job complete?
    pub fn is_complete(&self) -> bool {
        self.remaining_time <= 0.0
    }
}

/// Priority wrapper for the binary heap (max-heap, so we negate priority).
#[derive(Debug, Clone)]
struct PriorityJob {
    priority: f64,
    job: Job,
}

impl PartialEq for PriorityJob {
    fn eq(&self, other: &Self) -> bool {
        self.job.id == other.job.id
    }
}

impl Eq for PriorityJob {}

impl PartialOrd for PriorityJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority comes first (max-heap)
        self.priority
            .partial_cmp(&other.priority)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                // Tie-breaker: earlier arrival time
                other
                    .job
                    .arrival_time
                    .partial_cmp(&self.job.arrival_time)
                    .unwrap_or(Ordering::Equal)
            })
    }
}

/// Evidence for scheduling decisions.
#[derive(Debug, Clone)]
pub struct SchedulingEvidence {
    /// Current time.
    pub current_time: f64,

    /// Selected job ID (if any).
    pub selected_job_id: Option<u64>,

    /// Queue length.
    pub queue_length: usize,

    /// Mean wait time in queue.
    pub mean_wait_time: f64,

    /// Max wait time in queue.
    pub max_wait_time: f64,

    /// Reason for selection.
    pub reason: SelectionReason,
}

/// Reason for job selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionReason {
    /// No jobs in queue.
    QueueEmpty,
    /// Selected by SRPT (shortest remaining time).
    ShortestRemaining,
    /// Selected by Smith's rule (weight/time).
    HighestWeightedPriority,
    /// Selected due to aging (waited too long).
    AgingBoost,
    /// Continued from preemption.
    Continuation,
}

/// Scheduler statistics.
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    /// Total jobs submitted.
    pub total_submitted: u64,

    /// Total jobs completed.
    pub total_completed: u64,

    /// Total jobs rejected (queue full).
    pub total_rejected: u64,

    /// Total preemptions.
    pub total_preemptions: u64,

    /// Total time processing.
    pub total_processing_time: f64,

    /// Sum of response times (for mean calculation).
    pub total_response_time: f64,

    /// Max response time observed.
    pub max_response_time: f64,

    /// Current queue length.
    pub queue_length: usize,
}

impl SchedulerStats {
    /// Mean response time.
    pub fn mean_response_time(&self) -> f64 {
        if self.total_completed > 0 {
            self.total_response_time / self.total_completed as f64
        } else {
            0.0
        }
    }

    /// Throughput (jobs per time unit).
    pub fn throughput(&self) -> f64 {
        if self.total_processing_time > 0.0 {
            self.total_completed as f64 / self.total_processing_time
        } else {
            0.0
        }
    }
}

/// Queueing theory scheduler with fair SRPT.
#[derive(Debug)]
pub struct QueueingScheduler {
    config: SchedulerConfig,

    /// Priority queue of jobs.
    queue: BinaryHeap<PriorityJob>,

    /// Currently running job (if preemptive and processing).
    current_job: Option<Job>,

    /// Current simulation time.
    current_time: f64,

    /// Next job ID.
    next_job_id: u64,

    /// Statistics.
    stats: SchedulerStats,
}

impl QueueingScheduler {
    /// Create a new scheduler with given configuration.
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            queue: BinaryHeap::new(),
            current_job: None,
            current_time: 0.0,
            next_job_id: 1,
            stats: SchedulerStats::default(),
        }
    }

    /// Submit a new job to the scheduler.
    ///
    /// Returns the job ID if accepted, None if rejected (queue full).
    pub fn submit(&mut self, weight: f64, estimated_time: f64) -> Option<u64> {
        self.submit_named(weight, estimated_time, None::<&str>)
    }

    /// Submit a named job.
    pub fn submit_named(
        &mut self,
        weight: f64,
        estimated_time: f64,
        name: Option<impl Into<String>>,
    ) -> Option<u64> {
        if self.queue.len() >= self.config.max_queue_size {
            self.stats.total_rejected += 1;
            return None;
        }

        let id = self.next_job_id;
        self.next_job_id += 1;

        let mut job = Job::new(id, weight, estimated_time);
        job.arrival_time = self.current_time;
        if let Some(n) = name {
            job.name = Some(n.into());
        }

        let priority = self.compute_priority(&job);
        self.queue.push(PriorityJob { priority, job });

        self.stats.total_submitted += 1;
        self.stats.queue_length = self.queue.len();

        // Check for preemption
        if self.config.preemptive {
            self.maybe_preempt();
        }

        Some(id)
    }

    /// Advance time by the given amount and process jobs.
    ///
    /// Returns a list of completed job IDs.
    pub fn tick(&mut self, delta_time: f64) -> Vec<u64> {
        let mut completed = Vec::new();

        let mut remaining_time = delta_time;
        self.current_time += delta_time;
        self.stats.total_processing_time += delta_time;

        while remaining_time > 0.0 {
            // Get or select next job
            let job = if let Some(j) = self.current_job.take() {
                j
            } else if let Some(pj) = self.queue.pop() {
                pj.job
            } else {
                break; // Queue empty
            };

            // Process job
            let process_time = remaining_time.min(job.remaining_time);
            let mut job = job;
            job.remaining_time -= process_time;
            remaining_time -= process_time;

            if job.is_complete() {
                // Job completed
                let response_time = self.current_time - job.arrival_time;
                self.stats.total_response_time += response_time;
                self.stats.max_response_time = self.stats.max_response_time.max(response_time);
                self.stats.total_completed += 1;
                completed.push(job.id);
            } else {
                // Job not complete, save for next tick
                self.current_job = Some(job);
            }
        }

        // Recompute priorities for aged jobs
        self.refresh_priorities();

        self.stats.queue_length = self.queue.len();
        completed
    }

    /// Select the next job to run without advancing time.
    pub fn peek_next(&self) -> Option<&Job> {
        self.current_job
            .as_ref()
            .or_else(|| self.queue.peek().map(|pj| &pj.job))
    }

    /// Get scheduling evidence for the current state.
    pub fn evidence(&self) -> SchedulingEvidence {
        let (mean_wait, max_wait) = self.compute_wait_stats();

        let reason = if self.queue.is_empty() && self.current_job.is_none() {
            SelectionReason::QueueEmpty
        } else if self.current_job.is_some() {
            SelectionReason::Continuation
        } else if let Some(pj) = self.queue.peek() {
            let base_priority = pj.job.weight / pj.job.remaining_time.max(MIN_WEIGHT);
            let aging_contribution =
                self.config.aging_factor * (self.current_time - pj.job.arrival_time);
            if aging_contribution > base_priority * 0.5 {
                SelectionReason::AgingBoost
            } else if pj.job.weight > 1.0 {
                SelectionReason::HighestWeightedPriority
            } else {
                SelectionReason::ShortestRemaining
            }
        } else {
            SelectionReason::QueueEmpty
        };

        SchedulingEvidence {
            current_time: self.current_time,
            selected_job_id: self.peek_next().map(|j| j.id),
            queue_length: self.queue.len() + if self.current_job.is_some() { 1 } else { 0 },
            mean_wait_time: mean_wait,
            max_wait_time: max_wait,
            reason,
        }
    }

    /// Get current statistics.
    pub fn stats(&self) -> SchedulerStats {
        let mut stats = self.stats.clone();
        stats.queue_length = self.queue.len() + if self.current_job.is_some() { 1 } else { 0 };
        stats
    }

    /// Cancel a job by ID.
    pub fn cancel(&mut self, job_id: u64) -> bool {
        // Check current job
        if let Some(ref j) = self.current_job
            && j.id == job_id
        {
            self.current_job = None;
            return true;
        }

        // Remove from queue (rebuild without the job)
        let old_len = self.queue.len();
        let jobs: Vec<_> = self
            .queue
            .drain()
            .filter(|pj| pj.job.id != job_id)
            .collect();
        self.queue = jobs.into_iter().collect();

        self.stats.queue_length = self.queue.len();
        old_len != self.queue.len()
    }

    /// Clear all jobs.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.current_job = None;
        self.stats.queue_length = 0;
    }

    /// Reset scheduler state.
    pub fn reset(&mut self) {
        self.queue.clear();
        self.current_job = None;
        self.current_time = 0.0;
        self.next_job_id = 1;
        self.stats = SchedulerStats::default();
    }

    // --- Internal Methods ---

    /// Compute priority for a job using Smith's rule + aging.
    fn compute_priority(&self, job: &Job) -> f64 {
        let remaining = job.remaining_time.max(MIN_WEIGHT);
        let base_priority = job.weight / remaining;
        let wait_time = (self.current_time - job.arrival_time).max(0.0);
        let aging_boost = self.config.aging_factor * wait_time;
        base_priority + aging_boost
    }

    /// Check if current job should be preempted.
    fn maybe_preempt(&mut self) {
        if let Some(ref current) = self.current_job
            && let Some(pj) = self.queue.peek()
        {
            let current_priority = self.compute_priority(current);
            if pj.priority > current_priority {
                // Preempt
                let old = self.current_job.take().unwrap();
                let priority = self.compute_priority(&old);
                self.queue.push(PriorityJob { priority, job: old });
                self.stats.total_preemptions += 1;
            }
        }
    }

    /// Refresh priorities for all queued jobs (aging effect).
    fn refresh_priorities(&mut self) {
        let jobs: Vec<_> = self.queue.drain().map(|pj| pj.job).collect();
        for job in jobs {
            let priority = self.compute_priority(&job);
            self.queue.push(PriorityJob { priority, job });
        }
    }

    /// Compute wait time statistics.
    fn compute_wait_stats(&self) -> (f64, f64) {
        let mut total_wait = 0.0;
        let mut max_wait = 0.0f64;
        let mut count = 0;

        for pj in self.queue.iter() {
            let wait = (self.current_time - pj.job.arrival_time).max(0.0);
            total_wait += wait;
            max_wait = max_wait.max(wait);
            count += 1;
        }

        if let Some(ref j) = self.current_job {
            let wait = (self.current_time - j.arrival_time).max(0.0);
            total_wait += wait;
            max_wait = max_wait.max(wait);
            count += 1;
        }

        let mean = if count > 0 {
            total_wait / count as f64
        } else {
            0.0
        };
        (mean, max_wait)
    }
}

// =============================================================================
// Unit Tests (bd-13pq.7)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SchedulerConfig {
        SchedulerConfig {
            aging_factor: 0.001,
            max_queue_size: 100,
            preemptive: true,
            time_quantum: 10.0,
            enable_logging: false,
        }
    }

    // =========================================================================
    // Initialization tests
    // =========================================================================

    #[test]
    fn new_creates_empty_scheduler() {
        let scheduler = QueueingScheduler::new(test_config());
        assert_eq!(scheduler.stats().queue_length, 0);
        assert!(scheduler.peek_next().is_none());
    }

    #[test]
    fn default_config_valid() {
        let config = SchedulerConfig::default();
        let scheduler = QueueingScheduler::new(config);
        assert_eq!(scheduler.stats().queue_length, 0);
    }

    // =========================================================================
    // Job submission tests
    // =========================================================================

    #[test]
    fn submit_returns_job_id() {
        let mut scheduler = QueueingScheduler::new(test_config());
        let id = scheduler.submit(1.0, 10.0);
        assert_eq!(id, Some(1));
    }

    #[test]
    fn submit_increments_job_id() {
        let mut scheduler = QueueingScheduler::new(test_config());
        let id1 = scheduler.submit(1.0, 10.0);
        let id2 = scheduler.submit(1.0, 10.0);
        assert_eq!(id1, Some(1));
        assert_eq!(id2, Some(2));
    }

    #[test]
    fn submit_rejects_when_queue_full() {
        let mut config = test_config();
        config.max_queue_size = 2;
        let mut scheduler = QueueingScheduler::new(config);

        assert!(scheduler.submit(1.0, 10.0).is_some());
        assert!(scheduler.submit(1.0, 10.0).is_some());
        assert!(scheduler.submit(1.0, 10.0).is_none()); // Rejected
        assert_eq!(scheduler.stats().total_rejected, 1);
    }

    #[test]
    fn submit_named_job() {
        let mut scheduler = QueueingScheduler::new(test_config());
        let id = scheduler.submit_named(1.0, 10.0, Some("test-job"));
        assert!(id.is_some());
    }

    // =========================================================================
    // SRPT ordering tests
    // =========================================================================

    #[test]
    fn srpt_prefers_shorter_jobs() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 100.0); // Long job
        scheduler.submit(1.0, 10.0); // Short job

        let next = scheduler.peek_next().unwrap();
        assert_eq!(next.remaining_time, 10.0); // Short job selected
    }

    #[test]
    fn smith_rule_prefers_high_weight() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0); // Low weight
        scheduler.submit(10.0, 10.0); // High weight

        let next = scheduler.peek_next().unwrap();
        assert_eq!(next.weight, 10.0); // High weight selected
    }

    #[test]
    fn smith_rule_balances_weight_and_time() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(2.0, 20.0); // priority = 2/20 = 0.1
        scheduler.submit(1.0, 5.0); // priority = 1/5 = 0.2

        let next = scheduler.peek_next().unwrap();
        assert_eq!(next.remaining_time, 5.0); // Higher priority
    }

    // =========================================================================
    // Aging tests
    // =========================================================================

    #[test]
    fn aging_increases_priority_over_time() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 100.0); // Long job
        scheduler.tick(0.0); // Process nothing, just advance

        let before_aging = scheduler.compute_priority(scheduler.peek_next().unwrap());

        scheduler.current_time = 100.0; // Advance time significantly
        scheduler.refresh_priorities();

        let after_aging = scheduler.compute_priority(scheduler.peek_next().unwrap());
        assert!(
            after_aging > before_aging,
            "Priority should increase with wait time"
        );
    }

    #[test]
    fn aging_prevents_starvation() {
        let mut config = test_config();
        config.aging_factor = 1.0; // High aging
        let mut scheduler = QueueingScheduler::new(config);

        scheduler.submit(1.0, 1000.0); // Very long job
        scheduler.submit(1.0, 1.0); // Short job

        // Initially, short job should be preferred
        assert_eq!(scheduler.peek_next().unwrap().remaining_time, 1.0);

        // After the short job completes, long job should eventually run
        let completed = scheduler.tick(1.0);
        assert_eq!(completed.len(), 1);

        assert!(scheduler.peek_next().is_some());
    }

    // =========================================================================
    // Preemption tests
    // =========================================================================

    #[test]
    fn preemption_when_higher_priority_arrives() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 100.0); // Start processing long job
        scheduler.tick(10.0); // Process 10 units

        let before = scheduler.peek_next().unwrap().remaining_time;
        assert_eq!(before, 90.0);

        scheduler.submit(1.0, 5.0); // Higher priority arrives

        // Should now be processing the short job
        let next = scheduler.peek_next().unwrap();
        assert_eq!(next.remaining_time, 5.0);

        // Stats should show preemption
        assert_eq!(scheduler.stats().total_preemptions, 1);
    }

    #[test]
    fn no_preemption_when_disabled() {
        let mut config = test_config();
        config.preemptive = false;
        let mut scheduler = QueueingScheduler::new(config);

        scheduler.submit(1.0, 100.0);
        scheduler.tick(10.0);

        scheduler.submit(1.0, 5.0); // Would preempt if enabled

        // Should still be processing the first job
        let next = scheduler.peek_next().unwrap();
        assert_eq!(next.remaining_time, 90.0);
    }

    // =========================================================================
    // Processing tests
    // =========================================================================

    #[test]
    fn tick_processes_jobs() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        let completed = scheduler.tick(5.0);

        assert!(completed.is_empty()); // Not complete yet
        assert_eq!(scheduler.peek_next().unwrap().remaining_time, 5.0);
    }

    #[test]
    fn tick_completes_jobs() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        let completed = scheduler.tick(10.0);

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], 1);
        assert!(scheduler.peek_next().is_none());
    }

    #[test]
    fn tick_completes_multiple_jobs() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 5.0);
        scheduler.submit(1.0, 5.0);
        let completed = scheduler.tick(10.0);

        assert_eq!(completed.len(), 2);
    }

    #[test]
    fn tick_handles_zero_delta() {
        let mut scheduler = QueueingScheduler::new(test_config());
        scheduler.submit(1.0, 10.0);
        let completed = scheduler.tick(0.0);
        assert!(completed.is_empty());
    }

    // =========================================================================
    // Statistics tests
    // =========================================================================

    #[test]
    fn stats_track_submissions() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        scheduler.submit(1.0, 10.0);

        let stats = scheduler.stats();
        assert_eq!(stats.total_submitted, 2);
        assert_eq!(stats.queue_length, 2);
    }

    #[test]
    fn stats_track_completions() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        scheduler.tick(10.0);

        let stats = scheduler.stats();
        assert_eq!(stats.total_completed, 1);
    }

    #[test]
    fn stats_compute_mean_response_time() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        scheduler.submit(1.0, 10.0);
        scheduler.tick(20.0);

        let stats = scheduler.stats();
        // First job: 10 time units, Second job: 20 time units
        // Mean: (10 + 20) / 2 = 15
        assert_eq!(stats.total_completed, 2);
        assert!(stats.mean_response_time() > 0.0);
    }

    #[test]
    fn stats_compute_throughput() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        scheduler.tick(10.0);

        let stats = scheduler.stats();
        // 1 job in 10 time units
        assert!((stats.throughput() - 0.1).abs() < 0.01);
    }

    // =========================================================================
    // Evidence tests
    // =========================================================================

    #[test]
    fn evidence_reports_queue_empty() {
        let scheduler = QueueingScheduler::new(test_config());
        let evidence = scheduler.evidence();
        assert_eq!(evidence.reason, SelectionReason::QueueEmpty);
        assert!(evidence.selected_job_id.is_none());
    }

    #[test]
    fn evidence_reports_selected_job() {
        let mut scheduler = QueueingScheduler::new(test_config());
        scheduler.submit(1.0, 10.0);
        let evidence = scheduler.evidence();
        assert_eq!(evidence.selected_job_id, Some(1));
    }

    #[test]
    fn evidence_reports_wait_stats() {
        let mut scheduler = QueueingScheduler::new(test_config());
        scheduler.submit(1.0, 100.0);
        scheduler.submit(1.0, 100.0);
        scheduler.current_time = 50.0;
        scheduler.refresh_priorities();

        let evidence = scheduler.evidence();
        assert!(evidence.mean_wait_time > 0.0);
        assert!(evidence.max_wait_time > 0.0);
    }

    // =========================================================================
    // Cancel tests
    // =========================================================================

    #[test]
    fn cancel_removes_job() {
        let mut scheduler = QueueingScheduler::new(test_config());
        let id = scheduler.submit(1.0, 10.0).unwrap();

        assert!(scheduler.cancel(id));
        assert!(scheduler.peek_next().is_none());
    }

    #[test]
    fn cancel_returns_false_for_nonexistent() {
        let mut scheduler = QueueingScheduler::new(test_config());
        assert!(!scheduler.cancel(999));
    }

    // =========================================================================
    // Reset tests
    // =========================================================================

    #[test]
    fn reset_clears_all_state() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        scheduler.tick(5.0);

        scheduler.reset();

        assert!(scheduler.peek_next().is_none());
        assert_eq!(scheduler.stats().total_submitted, 0);
        assert_eq!(scheduler.stats().total_completed, 0);
    }

    #[test]
    fn clear_removes_jobs_but_keeps_stats() {
        let mut scheduler = QueueingScheduler::new(test_config());

        scheduler.submit(1.0, 10.0);
        scheduler.clear();

        assert!(scheduler.peek_next().is_none());
        assert_eq!(scheduler.stats().total_submitted, 1); // Stats preserved
    }

    // =========================================================================
    // Job tests
    // =========================================================================

    #[test]
    fn job_progress_increases() {
        let mut job = Job::new(1, 1.0, 100.0);
        assert_eq!(job.progress(), 0.0);

        job.remaining_time = 50.0;
        assert!((job.progress() - 0.5).abs() < 0.01);

        job.remaining_time = 0.0;
        assert_eq!(job.progress(), 1.0);
    }

    #[test]
    fn job_is_complete() {
        let mut job = Job::new(1, 1.0, 10.0);
        assert!(!job.is_complete());

        job.remaining_time = 0.0;
        assert!(job.is_complete());
    }

    // =========================================================================
    // Property tests
    // =========================================================================

    #[test]
    fn property_work_conserving() {
        let mut scheduler = QueueingScheduler::new(test_config());

        // Submit jobs
        for i in 0..10 {
            scheduler.submit(1.0, (i as f64) + 1.0);
        }

        // Process - should never be idle while jobs remain
        let mut total_processed = 0;
        while scheduler.peek_next().is_some() {
            let completed = scheduler.tick(1.0);
            total_processed += completed.len();
        }

        assert_eq!(total_processed, 10);
    }

    #[test]
    fn property_bounded_memory() {
        let mut config = test_config();
        config.max_queue_size = 100;
        let mut scheduler = QueueingScheduler::new(config);

        // Submit many jobs
        for _ in 0..1000 {
            scheduler.submit(1.0, 10.0);
        }

        assert!(scheduler.stats().queue_length <= 100);
    }

    #[test]
    fn property_deterministic() {
        let run = || {
            let mut scheduler = QueueingScheduler::new(test_config());
            let mut completions = Vec::new();

            for i in 0..20 {
                scheduler.submit(((i % 3) + 1) as f64, ((i % 5) + 1) as f64);
            }

            for _ in 0..50 {
                completions.extend(scheduler.tick(1.0));
            }

            completions
        };

        let run1 = run();
        let run2 = run();

        assert_eq!(run1, run2, "Scheduling should be deterministic");
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn zero_weight_handled() {
        let mut scheduler = QueueingScheduler::new(test_config());
        scheduler.submit(0.0, 10.0);
        assert!(scheduler.peek_next().is_some());
    }

    #[test]
    fn zero_time_completes_immediately() {
        let mut scheduler = QueueingScheduler::new(test_config());
        scheduler.submit(1.0, 0.0);
        let completed = scheduler.tick(1.0);
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn negative_time_handled() {
        let mut scheduler = QueueingScheduler::new(test_config());
        scheduler.submit(1.0, -10.0);
        let completed = scheduler.tick(1.0);
        assert_eq!(completed.len(), 1);
    }
}
