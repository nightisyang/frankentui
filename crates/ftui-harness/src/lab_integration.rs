#![forbid(unsafe_code)]

//! FrankenLab integration: deterministic model simulation with LabScenario.
//!
//! Bridges [`LabScenario`](crate::determinism::LabScenario) (seed-controlled
//! scheduling, JSONL logging, tracing spans) with
//! [`ProgramSimulator`](ftui_runtime::simulator::ProgramSimulator) (headless
//! model execution, frame capture).
//!
//! # Design
//!
//! [`Lab`] is the entry point. It creates a [`LabSession`] that wraps a
//! `ProgramSimulator<M>` with deterministic time, structured logging, and
//! frame-checksum recording for replay verification.
//!
//! # Example
//!
//! ```ignore
//! use ftui_harness::lab_integration::{Lab, LabConfig};
//!
//! let config = LabConfig::new("my_test", "theme_switch", 42)
//!     .viewport(80, 24)
//!     .time_step_ms(16);
//!
//! let run = Lab::run_scenario(config, MyModel::new(), |session| {
//!     session.init();
//!     session.send(MyMsg::SwitchTheme);
//!     session.tick();
//!     session.capture_frame();
//!     session.send(MyMsg::SwitchTheme);
//!     session.tick();
//!     session.capture_frame();
//! });
//!
//! assert!(run.result.deterministic);
//! assert_eq!(run.output.frame_count, 2);
//! // Replay: same seed → identical checksums
//! ```

use crate::determinism::{JsonValue, LabScenario, LabScenarioRun, TestJsonlLogger};
use ftui_core::event::Event;
use ftui_render::buffer::Buffer;
use ftui_runtime::program::Model;
use ftui_runtime::simulator::ProgramSimulator;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for a FrankenLab scenario run.
#[derive(Debug, Clone)]
pub struct LabConfig {
    /// Prefix for JSONL logger and run IDs.
    pub prefix: String,
    /// Scenario name (used in tracing spans and JSONL).
    pub scenario_name: String,
    /// Deterministic seed.
    pub seed: u64,
    /// Viewport width for frame captures.
    pub viewport_width: u16,
    /// Viewport height for frame captures.
    pub viewport_height: u16,
    /// Time step in milliseconds for the deterministic clock.
    pub time_step_ms: u64,
    /// Whether to log each captured frame's checksum to JSONL.
    pub log_frame_checksums: bool,
}

impl LabConfig {
    /// Create a new configuration with defaults.
    ///
    /// Defaults: 80x24 viewport, 16ms time step, frame checksum logging on.
    pub fn new(prefix: &str, scenario_name: &str, seed: u64) -> Self {
        Self {
            prefix: prefix.to_string(),
            scenario_name: scenario_name.to_string(),
            seed,
            viewport_width: 80,
            viewport_height: 24,
            time_step_ms: 16,
            log_frame_checksums: true,
        }
    }

    /// Set the viewport dimensions for frame captures.
    #[must_use]
    pub fn viewport(mut self, width: u16, height: u16) -> Self {
        self.viewport_width = width;
        self.viewport_height = height;
        self
    }

    /// Set the deterministic time step in milliseconds.
    #[must_use]
    pub fn time_step_ms(mut self, ms: u64) -> Self {
        self.time_step_ms = ms;
        self
    }

    /// Enable or disable JSONL logging of frame checksums.
    #[must_use]
    pub fn log_frame_checksums(mut self, enabled: bool) -> Self {
        self.log_frame_checksums = enabled;
        self
    }
}

// ============================================================================
// Session (mutable handle passed to scenario closures)
// ============================================================================

/// A frame checksum record for replay verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRecord {
    /// Frame index (0-based).
    pub index: usize,
    /// Deterministic timestamp when the frame was captured.
    pub timestamp_ms: u64,
    /// FNV-1a checksum of the frame buffer cells.
    pub checksum: u64,
}

/// Record of an injected event for ordering verification.
#[derive(Debug, Clone)]
pub struct EventRecord {
    /// Deterministic timestamp when the event was injected.
    pub timestamp_ms: u64,
    /// Sequential index of this event.
    pub sequence: u64,
    /// Human-readable label for the event kind.
    pub kind: String,
}

/// Active FrankenLab session wrapping a `ProgramSimulator`.
///
/// Provides deterministic time injection, structured event logging, and
/// frame-checksum recording. All operations are logged to JSONL via the
/// underlying [`TestJsonlLogger`].
pub struct LabSession<M: Model> {
    sim: ProgramSimulator<M>,
    logger: TestJsonlLogger,
    viewport_width: u16,
    viewport_height: u16,
    log_frame_checksums: bool,
    frame_records: Vec<FrameRecord>,
    event_log: Vec<EventRecord>,
    tick_count: u64,
    anomaly_count: u64,
    last_event_ms: u64,
}

impl<M: Model> LabSession<M> {
    /// Initialize the model (calls `Model::init()`).
    ///
    /// Should be called once before injecting events or capturing frames.
    pub fn init(&mut self) {
        self.sim.init();
        self.logger.log(
            "lab.session.init",
            &[(
                "viewport",
                JsonValue::raw(format!(
                    "[{},{}]",
                    self.viewport_width, self.viewport_height
                )),
            )],
        );
    }

    /// Send a message directly to the model.
    pub fn send(&mut self, msg: M::Message) {
        let now_ms = self.logger.fixture().now_ms();
        self.check_time_ordering(now_ms, "send");
        let seq = self.event_log.len() as u64;
        self.event_log.push(EventRecord {
            timestamp_ms: now_ms,
            sequence: seq,
            kind: "message".to_string(),
        });
        self.sim.send(msg);
    }

    /// Inject a terminal event into the model.
    pub fn inject_event(&mut self, event: Event) {
        let now_ms = self.logger.fixture().now_ms();
        self.check_time_ordering(now_ms, "inject_event");
        let seq = self.event_log.len() as u64;
        let kind = event_kind_label(&event);
        self.event_log.push(EventRecord {
            timestamp_ms: now_ms,
            sequence: seq,
            kind,
        });
        self.sim.inject_event(event);
    }

    /// Inject multiple terminal events in order.
    pub fn inject_events(&mut self, events: &[Event]) {
        for event in events {
            self.inject_event(event.clone());
        }
    }

    /// Simulate a tick event (deterministic time advance).
    ///
    /// Injects `Event::Tick` into the model. The deterministic clock advances
    /// by `time_step_ms` for each call to `now_ms()`.
    pub fn tick(&mut self) {
        let now_ms = self.logger.fixture().now_ms();
        self.check_time_ordering(now_ms, "tick");
        self.tick_count += 1;
        let seq = self.event_log.len() as u64;
        self.event_log.push(EventRecord {
            timestamp_ms: now_ms,
            sequence: seq,
            kind: "tick".to_string(),
        });
        self.sim.inject_event(Event::Tick);
    }

    /// Capture the current frame at the configured viewport dimensions.
    ///
    /// Records a checksum for replay verification and optionally logs it
    /// to JSONL.
    pub fn capture_frame(&mut self) -> &Buffer {
        let w = self.viewport_width;
        let h = self.viewport_height;
        self.capture_frame_inner(w, h)
    }

    /// Capture a frame at custom dimensions (overriding the configured viewport).
    pub fn capture_frame_at(&mut self, width: u16, height: u16) -> &Buffer {
        self.capture_frame_inner(width, height)
    }

    fn capture_frame_inner(&mut self, width: u16, height: u16) -> &Buffer {
        let now_ms = self.logger.fixture().now_ms();
        let buf = self.sim.capture_frame(width, height);
        let checksum = fnv1a_buffer(buf);
        let index = self.frame_records.len();

        self.frame_records.push(FrameRecord {
            index,
            timestamp_ms: now_ms,
            checksum,
        });

        if self.log_frame_checksums {
            self.logger.log(
                "lab.frame",
                &[
                    ("frame_idx", JsonValue::u64(index as u64)),
                    ("checksum", JsonValue::str(format!("{checksum:016x}"))),
                    ("timestamp_ms", JsonValue::u64(now_ms)),
                    ("width", JsonValue::u64(width as u64)),
                    ("height", JsonValue::u64(height as u64)),
                ],
            );
        }

        self.sim.last_frame().expect("frame just captured")
    }

    /// Access the underlying model.
    pub fn model(&self) -> &M {
        self.sim.model()
    }

    /// Access the underlying model mutably.
    pub fn model_mut(&mut self) -> &mut M {
        self.sim.model_mut()
    }

    /// Check if the simulated program is still running.
    pub fn is_running(&self) -> bool {
        self.sim.is_running()
    }

    /// Get all frame checksum records.
    pub fn frame_records(&self) -> &[FrameRecord] {
        &self.frame_records
    }

    /// Get all event records (for ordering verification).
    pub fn event_log(&self) -> &[EventRecord] {
        &self.event_log
    }

    /// Number of ticks injected.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Number of scheduling anomalies detected.
    pub fn anomaly_count(&self) -> u64 {
        self.anomaly_count
    }

    /// All captured frame buffers.
    pub fn frames(&self) -> &[Buffer] {
        self.sim.frames()
    }

    /// Most recently captured frame.
    pub fn last_frame(&self) -> Option<&Buffer> {
        self.sim.last_frame()
    }

    /// Logs emitted via `Cmd::Log`.
    pub fn logs(&self) -> &[String] {
        self.sim.logs()
    }

    /// Underlying simulator command log.
    pub fn command_log(&self) -> &[ftui_runtime::simulator::CmdRecord] {
        self.sim.command_log()
    }

    /// Access the grapheme pool for text extraction.
    pub fn pool(&self) -> &ftui_render::grapheme_pool::GraphemePool {
        self.sim.pool()
    }

    /// Deterministic monotonic time from the fixture.
    pub fn now_ms(&self) -> u64 {
        self.logger.fixture().now_ms()
    }

    /// Log a custom info event via the JSONL logger.
    pub fn log_info(&self, event: &str, fields: &[(&str, JsonValue)]) {
        self.logger.log(event, fields);
    }

    /// Log a warning (scheduling anomaly or custom) event.
    pub fn log_warn(&self, anomaly: &str, detail: &str) {
        self.logger.log(
            "lab.session.warn",
            &[
                ("anomaly", JsonValue::str(anomaly)),
                ("detail", JsonValue::str(detail)),
            ],
        );
    }

    // ── Internal ─────────────────────────────────────────────────────

    fn check_time_ordering(&mut self, now_ms: u64, operation: &str) {
        if now_ms < self.last_event_ms {
            self.anomaly_count += 1;
            self.logger.log(
                "lab.session.warn",
                &[
                    ("anomaly", JsonValue::str("time_ordering")),
                    (
                        "detail",
                        JsonValue::str(format!(
                            "{operation}: time went backwards ({now_ms} < {})",
                            self.last_event_ms
                        )),
                    ),
                ],
            );
        }
        self.last_event_ms = now_ms;
    }

    fn into_output(self) -> LabOutput {
        LabOutput {
            frame_count: self.frame_records.len(),
            frame_records: self.frame_records,
            event_count: self.event_log.len(),
            event_log: self.event_log,
            tick_count: self.tick_count,
            anomaly_count: self.anomaly_count,
        }
    }
}

// ============================================================================
// Output
// ============================================================================

/// Summary output from a FrankenLab scenario run.
#[derive(Debug, Clone)]
pub struct LabOutput {
    /// Number of frames captured.
    pub frame_count: usize,
    /// Frame checksum records for replay verification.
    pub frame_records: Vec<FrameRecord>,
    /// Number of events injected.
    pub event_count: usize,
    /// Event log for ordering verification.
    pub event_log: Vec<EventRecord>,
    /// Number of ticks injected.
    pub tick_count: u64,
    /// Number of scheduling anomalies detected.
    pub anomaly_count: u64,
}

// ============================================================================
// Lab entry point
// ============================================================================

/// FrankenLab — deterministic model testing harness.
///
/// Combines [`LabScenario`] (seed, logging, tracing spans, metrics) with
/// [`ProgramSimulator`] (headless model execution) into a single API.
pub struct Lab;

impl Lab {
    /// Run a deterministic scenario with a model.
    ///
    /// Creates a [`LabScenario`] for the outer tracing span (including
    /// `lab.scenario` with `scenario_name`, `seed`, `event_count`,
    /// `duration_us` fields) and `lab_scenarios_run_total` metric counter.
    ///
    /// The closure receives a [`LabSession`] that wraps a `ProgramSimulator`
    /// with deterministic time injection, JSONL logging, and frame checksums.
    pub fn run_scenario<M: Model>(
        config: LabConfig,
        model: M,
        run: impl FnOnce(&mut LabSession<M>),
    ) -> LabScenarioRun<LabOutput> {
        let scenario = LabScenario::new_with(
            &config.prefix,
            &config.scenario_name,
            config.seed,
            true, // always deterministic
            config.time_step_ms,
        );

        // LabScenario::run() handles the outer span + start/end JSONL +
        // lab_scenarios_run_total counter. Inside, we create a session-level
        // logger with the same determinism settings for frame/event logging.
        scenario.run(|_ctx| {
            let mut session_logger = TestJsonlLogger::new_with(
                &format!("{}_session", config.prefix),
                config.seed,
                true,
                config.time_step_ms,
            );
            session_logger.add_context_str("scenario_name", &config.scenario_name);

            let mut session = LabSession {
                sim: ProgramSimulator::new(model),
                logger: session_logger,
                viewport_width: config.viewport_width,
                viewport_height: config.viewport_height,
                log_frame_checksums: config.log_frame_checksums,
                frame_records: Vec::new(),
                event_log: Vec::new(),
                tick_count: 0,
                anomaly_count: 0,
                last_event_ms: 0,
            };

            run(&mut session);
            session.into_output()
        })
    }

    /// Verify determinism with a custom scenario closure.
    ///
    /// Runs `scenario_fn` twice with the same seed and model, asserting
    /// frame checksum equality.
    ///
    /// # Panics
    ///
    /// Panics if frame counts differ or any checksum mismatches.
    pub fn assert_deterministic_with<M, MF, SF>(
        config: LabConfig,
        model_factory: MF,
        scenario_fn: SF,
    ) -> LabOutput
    where
        M: Model,
        MF: Fn() -> M,
        SF: Fn(&mut LabSession<M>),
    {
        let run1 = Self::run_scenario(config.clone(), model_factory(), |s| scenario_fn(s));
        let run2 = Self::run_scenario(config, model_factory(), |s| scenario_fn(s));

        assert_eq!(
            run1.output.frame_count, run2.output.frame_count,
            "frame count mismatch between identical-seed runs"
        );

        for (i, (a, b)) in run1
            .output
            .frame_records
            .iter()
            .zip(run2.output.frame_records.iter())
            .enumerate()
        {
            assert_eq!(
                a.checksum, b.checksum,
                "frame {i} checksum mismatch: run1={:016x}, run2={:016x}",
                a.checksum, b.checksum
            );
        }

        run1.output
    }
}

// ============================================================================
// FNV-1a checksum (matches trace_replay.rs)
// ============================================================================

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// FNV-1a checksum over buffer cell data.
fn fnv1a_buffer(buf: &Buffer) -> u64 {
    let mut hash = FNV_OFFSET;
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            if let Some(cell) = buf.get(x, y) {
                if cell.is_continuation() {
                    hash ^= 0x01u64;
                    hash = hash.wrapping_mul(FNV_PRIME);
                    continue;
                }
                if cell.is_empty() {
                    hash ^= 0x00u64;
                    hash = hash.wrapping_mul(FNV_PRIME);
                } else if let Some(c) = cell.content.as_char() {
                    hash ^= 0x02u64;
                    hash = hash.wrapping_mul(FNV_PRIME);
                    for b in (c as u32).to_le_bytes() {
                        hash ^= b as u64;
                        hash = hash.wrapping_mul(FNV_PRIME);
                    }
                } else {
                    // grapheme reference
                    hash ^= 0x03u64;
                    hash = hash.wrapping_mul(FNV_PRIME);
                }
                // fg (PackedRgba inner u32)
                hash ^= cell.fg.0 as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
                // bg
                hash ^= cell.bg.0 as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
                // attrs: combine flags (u8) and link_id (u32)
                hash ^= cell.attrs.flags().bits() as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
                hash ^= cell.attrs.link_id() as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
    }
    hash
}

/// Human-readable label for an event kind.
fn event_kind_label(event: &Event) -> String {
    match event {
        Event::Key(k) => format!("key:{:?}", k.code),
        Event::Resize { width, height } => format!("resize:{width}x{height}"),
        Event::Mouse(m) => format!("mouse:{:?}", m.kind),
        Event::Tick => "tick".to_string(),
        _ => "other".to_string(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_core::event::{KeyCode, KeyEvent, KeyEventKind, Modifiers};
    use ftui_render::frame::Frame;

    // ── Test model ───────────────────────────────────────────────────

    struct Counter {
        value: i32,
    }

    #[derive(Debug)]
    enum CounterMsg {
        Increment,
        Decrement,
        Tick,
        Quit,
    }

    impl From<Event> for CounterMsg {
        fn from(event: Event) -> Self {
            match event {
                Event::Key(k) if k.code == KeyCode::Char('+') => CounterMsg::Increment,
                Event::Key(k) if k.code == KeyCode::Char('-') => CounterMsg::Decrement,
                Event::Key(k) if k.code == KeyCode::Char('q') => CounterMsg::Quit,
                Event::Tick => CounterMsg::Tick,
                _ => CounterMsg::Tick,
            }
        }
    }

    impl Model for Counter {
        type Message = CounterMsg;

        fn init(&mut self) -> ftui_runtime::program::Cmd<Self::Message> {
            ftui_runtime::program::Cmd::none()
        }

        fn update(&mut self, msg: Self::Message) -> ftui_runtime::program::Cmd<Self::Message> {
            match msg {
                CounterMsg::Increment => {
                    self.value += 1;
                    ftui_runtime::program::Cmd::none()
                }
                CounterMsg::Decrement => {
                    self.value -= 1;
                    ftui_runtime::program::Cmd::none()
                }
                CounterMsg::Tick => ftui_runtime::program::Cmd::none(),
                CounterMsg::Quit => ftui_runtime::program::Cmd::quit(),
            }
        }

        fn view(&self, frame: &mut Frame) {
            let text = format!("Count: {}", self.value);
            for (i, c) in text.chars().enumerate() {
                if (i as u16) < frame.width() {
                    use ftui_render::cell::Cell;
                    frame.buffer.set_raw(i as u16, 0, Cell::from_char(c));
                }
            }
        }
    }

    fn key_event(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: Modifiers::empty(),
            kind: KeyEventKind::Press,
        })
    }

    // ── Tests ────────────────────────────────────────────────────────

    #[test]
    fn run_scenario_basic() {
        let config = LabConfig::new("test", "basic", 42).viewport(20, 5);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.send(CounterMsg::Increment);
            s.send(CounterMsg::Increment);
            s.capture_frame();
        });

        assert_eq!(run.output.frame_count, 1);
        assert!(run.result.deterministic);
        assert_eq!(run.result.seed, 42);
    }

    #[test]
    fn deterministic_checksums_across_runs() {
        let checksums1 = {
            let config = LabConfig::new("test", "det_check", 99).viewport(20, 5);
            let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
                s.init();
                for _ in 0..5 {
                    s.send(CounterMsg::Increment);
                    s.tick();
                    s.capture_frame();
                }
            });
            run.output
                .frame_records
                .iter()
                .map(|f| f.checksum)
                .collect::<Vec<_>>()
        };

        let checksums2 = {
            let config = LabConfig::new("test", "det_check", 99).viewport(20, 5);
            let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
                s.init();
                for _ in 0..5 {
                    s.send(CounterMsg::Increment);
                    s.tick();
                    s.capture_frame();
                }
            });
            run.output
                .frame_records
                .iter()
                .map(|f| f.checksum)
                .collect::<Vec<_>>()
        };

        assert_eq!(checksums1, checksums2);
    }

    #[test]
    fn different_seeds_produce_different_metadata() {
        let run1 = Lab::run_scenario(
            LabConfig::new("test", "seed_diff", 1),
            Counter { value: 0 },
            |s| {
                s.init();
                s.capture_frame();
            },
        );

        let run2 = Lab::run_scenario(
            LabConfig::new("test", "seed_diff", 2),
            Counter { value: 0 },
            |s| {
                s.init();
                s.capture_frame();
            },
        );

        assert_ne!(run1.result.run_id, run2.result.run_id);
        assert_ne!(run1.result.seed, run2.result.seed);
    }

    #[test]
    fn event_ordering_is_tracked() {
        let config = LabConfig::new("test", "event_order", 42).viewport(20, 5);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.send(CounterMsg::Increment);
            s.tick();
            s.inject_event(key_event('+'));
            s.capture_frame();
        });

        assert_eq!(run.output.event_count, 3);
        let log = &run.output.event_log;
        assert_eq!(log[0].kind, "message");
        assert_eq!(log[1].kind, "tick");
        assert_eq!(log[2].kind, "key:Char('+')");
        // Timestamps should be monotonically non-decreasing
        for w in log.windows(2) {
            assert!(w[1].timestamp_ms >= w[0].timestamp_ms);
        }
    }

    #[test]
    fn tick_count_is_tracked() {
        let config = LabConfig::new("test", "tick_count", 42);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.tick();
            s.tick();
            s.tick();
            s.capture_frame();
        });

        assert_eq!(run.output.tick_count, 3);
    }

    #[test]
    fn model_access_works() {
        let config = LabConfig::new("test", "model_access", 42);
        Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.send(CounterMsg::Increment);
            s.send(CounterMsg::Increment);
            assert_eq!(s.model().value, 2);
        });
    }

    #[test]
    fn quit_stops_session() {
        let config = LabConfig::new("test", "quit_test", 42);
        Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.send(CounterMsg::Increment);
            s.send(CounterMsg::Quit);
            assert!(!s.is_running());
        });
    }

    #[test]
    fn custom_viewport_in_frame() {
        let config = LabConfig::new("test", "custom_viewport", 42).viewport(40, 10);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.capture_frame_at(100, 50);
            s.capture_frame(); // uses default 40x10
        });

        assert_eq!(run.output.frame_count, 2);
    }

    #[test]
    fn no_anomalies_in_normal_usage() {
        let config = LabConfig::new("test", "no_anomalies", 42);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            for _ in 0..10 {
                s.send(CounterMsg::Increment);
                s.tick();
                s.capture_frame();
            }
        });

        assert_eq!(run.output.anomaly_count, 0);
    }

    #[test]
    fn frame_checksums_change_with_state() {
        let config = LabConfig::new("test", "checksum_changes", 42).viewport(20, 5);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.capture_frame(); // value=0
            s.send(CounterMsg::Increment);
            s.capture_frame(); // value=1
        });

        assert_eq!(run.output.frame_count, 2);
        let records = &run.output.frame_records;
        assert_ne!(
            records[0].checksum, records[1].checksum,
            "different model states should produce different checksums"
        );
    }

    #[test]
    fn assert_deterministic_with_custom_scenario() {
        let config = LabConfig::new("test", "det_custom", 42).viewport(20, 5);
        let output = Lab::assert_deterministic_with(
            config,
            || Counter { value: 0 },
            |s| {
                s.init();
                for i in 0..5 {
                    if i % 2 == 0 {
                        s.send(CounterMsg::Increment);
                    } else {
                        s.send(CounterMsg::Decrement);
                    }
                    s.capture_frame();
                }
            },
        );
        assert_eq!(output.frame_count, 5);
    }

    #[test]
    fn inject_events_batch() {
        let config = LabConfig::new("test", "inject_batch", 42).viewport(20, 5);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.inject_events(&[key_event('+'), key_event('+'), key_event('-')]);
            s.capture_frame();
        });

        assert_eq!(run.output.event_count, 3);
    }

    #[test]
    fn log_info_and_warn_accessible() {
        let config = LabConfig::new("test", "logging", 42);
        Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.log_info("custom.event", &[("key", JsonValue::str("value"))]);
            s.log_warn("test_anomaly", "simulated warning");
        });
        // Verify these don't panic.
    }

    #[test]
    fn fnv1a_buffer_deterministic() {
        let buf1 = {
            let mut b = Buffer::new(5, 3);
            b.set_raw(0, 0, ftui_render::cell::Cell::from_char('A'));
            b.set_raw(1, 0, ftui_render::cell::Cell::from_char('B'));
            b
        };
        let buf2 = {
            let mut b = Buffer::new(5, 3);
            b.set_raw(0, 0, ftui_render::cell::Cell::from_char('A'));
            b.set_raw(1, 0, ftui_render::cell::Cell::from_char('B'));
            b
        };
        assert_eq!(fnv1a_buffer(&buf1), fnv1a_buffer(&buf2));

        // Different content → different checksum
        let buf3 = {
            let mut b = Buffer::new(5, 3);
            b.set_raw(0, 0, ftui_render::cell::Cell::from_char('X'));
            b
        };
        assert_ne!(fnv1a_buffer(&buf1), fnv1a_buffer(&buf3));
    }

    #[test]
    fn multi_seed_determinism_100_seeds() {
        for seed in 0..100 {
            let checksums1 = {
                let config = LabConfig::new("test", "multi_seed", seed).viewport(20, 5);
                let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
                    s.init();
                    s.send(CounterMsg::Increment);
                    s.tick();
                    s.capture_frame();
                });
                run.output
                    .frame_records
                    .iter()
                    .map(|f| f.checksum)
                    .collect::<Vec<_>>()
            };
            let checksums2 = {
                let config = LabConfig::new("test", "multi_seed", seed).viewport(20, 5);
                let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
                    s.init();
                    s.send(CounterMsg::Increment);
                    s.tick();
                    s.capture_frame();
                });
                run.output
                    .frame_records
                    .iter()
                    .map(|f| f.checksum)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                checksums1, checksums2,
                "seed {seed}: checksums diverged between runs"
            );
        }
    }

    #[test]
    fn scenario_metadata_is_correct() {
        let config = LabConfig::new("meta_test", "my_scenario", 777)
            .viewport(40, 10)
            .time_step_ms(8);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.tick();
            s.capture_frame();
        });

        assert_eq!(run.result.scenario_name, "my_scenario");
        assert_eq!(run.result.seed, 777);
        assert!(run.result.deterministic);
        assert!(run.result.run_total >= 1);
    }

    #[test]
    fn event_timestamps_advance_with_time_step() {
        let config = LabConfig::new("test", "ts_advance", 42).time_step_ms(100);
        let run = Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.send(CounterMsg::Increment); // consumes 1 now_ms() call
            s.send(CounterMsg::Increment); // consumes 1 now_ms() call
            s.send(CounterMsg::Increment); // consumes 1 now_ms() call
        });

        let log = &run.output.event_log;
        assert_eq!(log.len(), 3);
        // Each send() calls now_ms() which advances the clock by time_step_ms
        // DeterminismFixture::now_ms() returns fetch_add(step) + step
        // So first call returns step, second returns 2*step, etc.
        assert!(log[0].timestamp_ms <= log[1].timestamp_ms);
        assert!(log[1].timestamp_ms <= log[2].timestamp_ms);
    }

    #[test]
    fn session_now_ms_advances() {
        let config = LabConfig::new("test", "now_ms", 42).time_step_ms(50);
        Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            let t0 = s.now_ms();
            let t1 = s.now_ms();
            assert!(t1 > t0, "now_ms should advance: t0={t0}, t1={t1}");
        });
    }

    #[test]
    fn command_log_accessible() {
        let config = LabConfig::new("test", "cmd_log", 42);
        Lab::run_scenario(config, Counter { value: 0 }, |s| {
            s.init();
            s.send(CounterMsg::Increment);
            s.send(CounterMsg::Quit);
            let log = s.command_log();
            assert!(!log.is_empty());
        });
    }
}
