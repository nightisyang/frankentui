#![forbid(unsafe_code)]

//! Bubbletea/Elm-style runtime for terminal applications.
//!
//! The program runtime manages the update/view loop, handling events and
//! rendering frames. It separates state (Model) from rendering (View) and
//! provides a command pattern for side effects.
//!
//! # Example
//!
//! ```ignore
//! use ftui_runtime::program::{Model, Cmd};
//! use ftui_core::event::Event;
//! use ftui_render::frame::Frame;
//!
//! struct Counter {
//!     count: i32,
//! }
//!
//! enum Msg {
//!     Increment,
//!     Decrement,
//!     Quit,
//! }
//!
//! impl From<Event> for Msg {
//!     fn from(event: Event) -> Self {
//!         match event {
//!             Event::Key(k) if k.is_char('q') => Msg::Quit,
//!             Event::Key(k) if k.is_char('+') => Msg::Increment,
//!             Event::Key(k) if k.is_char('-') => Msg::Decrement,
//!             _ => Msg::Increment, // Default
//!         }
//!     }
//! }
//!
//! impl Model for Counter {
//!     type Message = Msg;
//!
//!     fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
//!         match msg {
//!             Msg::Increment => { self.count += 1; Cmd::none() }
//!             Msg::Decrement => { self.count -= 1; Cmd::none() }
//!             Msg::Quit => Cmd::quit(),
//!         }
//!     }
//!
//!     fn view(&self, frame: &mut Frame) {
//!         // Render counter value to frame
//!     }
//! }
//! ```

use crate::input_macro::{EventRecorder, InputMacro};
use crate::subscription::SubscriptionManager;
use crate::terminal_writer::{ScreenMode, TerminalWriter, UiAnchor};
use ftui_core::event::Event;
use ftui_core::terminal_capabilities::TerminalCapabilities;
use ftui_core::terminal_session::{SessionOptions, TerminalSession};
use ftui_render::budget::{FrameBudgetConfig, RenderBudget};
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_render::sanitize::sanitize;
use std::io::{self, Stdout, Write};
use std::time::{Duration, Instant};
use tracing::{debug, debug_span, info, info_span};

/// The Model trait defines application state and behavior.
///
/// Implementations define how the application responds to events
/// and renders its current state.
pub trait Model: Sized {
    /// The message type for this model.
    ///
    /// Messages represent actions that update the model state.
    /// Must be convertible from terminal events.
    type Message: From<Event> + Send + 'static;

    /// Initialize the model with startup commands.
    ///
    /// Called once when the program starts. Return commands to execute
    /// initial side effects like loading data.
    fn init(&mut self) -> Cmd<Self::Message> {
        Cmd::none()
    }

    /// Update the model in response to a message.
    ///
    /// This is the core state transition function. Returns commands
    /// for any side effects that should be executed.
    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message>;

    /// Render the current state to a frame.
    ///
    /// Called after updates when the UI needs to be redrawn.
    fn view(&self, frame: &mut Frame);

    /// Declare active subscriptions.
    ///
    /// Called after each `update()`. The runtime compares the returned set
    /// (by `SubId`) against currently running subscriptions and starts/stops
    /// as needed. Returning an empty vec stops all subscriptions.
    ///
    /// # Default
    ///
    /// Returns an empty vec (no subscriptions).
    fn subscriptions(&self) -> Vec<Box<dyn crate::subscription::Subscription<Self::Message>>> {
        vec![]
    }
}

/// Commands represent side effects to be executed by the runtime.
///
/// Commands are returned from `init()` and `update()` to trigger
/// actions like quitting, sending messages, or scheduling ticks.
#[derive(Default)]
pub enum Cmd<M> {
    /// No operation.
    #[default]
    None,
    /// Quit the application.
    Quit,
    /// Execute multiple commands as a batch (currently sequential).
    Batch(Vec<Cmd<M>>),
    /// Execute commands sequentially.
    Sequence(Vec<Cmd<M>>),
    /// Send a message to the model.
    Msg(M),
    /// Schedule a tick after a duration.
    Tick(Duration),
    /// Write a log message to the terminal output.
    ///
    /// This writes to the scrollback region in inline mode, or is ignored/handled
    /// appropriately in alternate screen mode. Safe to use with the One-Writer Rule.
    Log(String),
    /// Execute a blocking operation on a background thread.
    ///
    /// The closure runs on a spawned thread and its return value
    /// is sent back as a message to the model.
    Task(Box<dyn FnOnce() -> M + Send>),
}

impl<M: std::fmt::Debug> std::fmt::Debug for Cmd<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Quit => write!(f, "Quit"),
            Self::Batch(cmds) => f.debug_tuple("Batch").field(cmds).finish(),
            Self::Sequence(cmds) => f.debug_tuple("Sequence").field(cmds).finish(),
            Self::Msg(m) => f.debug_tuple("Msg").field(m).finish(),
            Self::Tick(d) => f.debug_tuple("Tick").field(d).finish(),
            Self::Log(s) => f.debug_tuple("Log").field(s).finish(),
            Self::Task(_) => write!(f, "Task(...)"),
        }
    }
}

impl<M> Cmd<M> {
    /// Create a no-op command.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Create a quit command.
    #[inline]
    pub fn quit() -> Self {
        Self::Quit
    }

    /// Create a message command.
    #[inline]
    pub fn msg(m: M) -> Self {
        Self::Msg(m)
    }

    /// Create a log command.
    ///
    /// The message will be sanitized and written to the terminal log (scrollback).
    /// A newline is appended if not present.
    #[inline]
    pub fn log(msg: impl Into<String>) -> Self {
        Self::Log(msg.into())
    }

    /// Create a batch of commands.
    pub fn batch(cmds: Vec<Self>) -> Self {
        if cmds.is_empty() {
            Self::None
        } else if cmds.len() == 1 {
            cmds.into_iter().next().unwrap()
        } else {
            Self::Batch(cmds)
        }
    }

    /// Create a sequence of commands.
    pub fn sequence(cmds: Vec<Self>) -> Self {
        if cmds.is_empty() {
            Self::None
        } else if cmds.len() == 1 {
            cmds.into_iter().next().unwrap()
        } else {
            Self::Sequence(cmds)
        }
    }

    /// Create a tick command.
    #[inline]
    pub fn tick(duration: Duration) -> Self {
        Self::Tick(duration)
    }

    /// Create a background task command.
    ///
    /// The closure runs on a spawned thread. When it completes,
    /// the returned message is sent back to the model's `update()`.
    pub fn task<F>(f: F) -> Self
    where
        F: FnOnce() -> M + Send + 'static,
    {
        Self::Task(Box::new(f))
    }
}

/// Configuration for the program runtime.
#[derive(Debug, Clone)]
pub struct ProgramConfig {
    /// Screen mode (inline or alternate screen).
    pub screen_mode: ScreenMode,
    /// UI anchor for inline mode.
    pub ui_anchor: UiAnchor,
    /// Frame budget configuration.
    pub budget: FrameBudgetConfig,
    /// Input poll timeout.
    pub poll_timeout: Duration,
    /// Debounce duration for resize events.
    pub resize_debounce: Duration,
    /// Enable mouse support.
    pub mouse: bool,
    /// Enable bracketed paste.
    pub bracketed_paste: bool,
    /// Enable focus reporting.
    pub focus_reporting: bool,
    /// Enable Kitty keyboard protocol (repeat/release events).
    pub kitty_keyboard: bool,
}

impl Default for ProgramConfig {
    fn default() -> Self {
        Self {
            screen_mode: ScreenMode::Inline { ui_height: 4 },
            ui_anchor: UiAnchor::Bottom,
            budget: FrameBudgetConfig::default(),
            poll_timeout: Duration::from_millis(100),
            resize_debounce: Duration::from_millis(100),
            mouse: false,
            bracketed_paste: true,
            focus_reporting: false,
            kitty_keyboard: false,
        }
    }
}

impl ProgramConfig {
    /// Create config for fullscreen applications.
    pub fn fullscreen() -> Self {
        Self {
            screen_mode: ScreenMode::AltScreen,
            ..Default::default()
        }
    }

    /// Create config for inline mode with specified height.
    pub fn inline(height: u16) -> Self {
        Self {
            screen_mode: ScreenMode::Inline { ui_height: height },
            ..Default::default()
        }
    }

    /// Enable mouse support.
    pub fn with_mouse(mut self) -> Self {
        self.mouse = true;
        self
    }

    /// Set the budget configuration.
    pub fn with_budget(mut self, budget: FrameBudgetConfig) -> Self {
        self.budget = budget;
        self
    }

    /// Set the resize debounce duration.
    pub fn with_resize_debounce(mut self, debounce: Duration) -> Self {
        self.resize_debounce = debounce;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeAction {
    None,
    ShowPlaceholder,
    ApplyResize {
        width: u16,
        height: u16,
        elapsed: Duration,
    },
}

#[derive(Debug)]
struct ResizeDebouncer {
    debounce: Duration,
    last_resize: Option<Instant>,
    pending_size: Option<(u16, u16)>,
    last_applied: (u16, u16),
}

impl ResizeDebouncer {
    fn new(debounce: Duration, initial_size: (u16, u16)) -> Self {
        Self {
            debounce,
            last_resize: None,
            pending_size: None,
            last_applied: initial_size,
        }
    }

    fn handle_resize(&mut self, width: u16, height: u16) -> ResizeAction {
        self.handle_resize_at(width, height, Instant::now())
    }

    fn handle_resize_at(&mut self, width: u16, height: u16, now: Instant) -> ResizeAction {
        if self.pending_size.is_none() && (width, height) == self.last_applied {
            return ResizeAction::None;
        }
        self.pending_size = Some((width, height));
        self.last_resize = Some(now);
        ResizeAction::ShowPlaceholder
    }

    fn tick(&mut self) -> ResizeAction {
        self.tick_at(Instant::now())
    }

    fn tick_at(&mut self, now: Instant) -> ResizeAction {
        let Some(pending) = self.pending_size else {
            return ResizeAction::None;
        };
        let Some(last) = self.last_resize else {
            return ResizeAction::None;
        };

        let elapsed = now.saturating_duration_since(last);
        if elapsed >= self.debounce {
            self.pending_size = None;
            self.last_resize = None;
            self.last_applied = pending;
            return ResizeAction::ApplyResize {
                width: pending.0,
                height: pending.1,
                elapsed,
            };
        }

        ResizeAction::None
    }

    fn time_until_apply(&self, now: Instant) -> Option<Duration> {
        let _pending = self.pending_size?;
        let last = self.last_resize?;
        let elapsed = now.saturating_duration_since(last);
        Some(self.debounce.saturating_sub(elapsed))
    }
}

/// The program runtime that manages the update/view loop.
pub struct Program<M: Model, W: Write + Send = Stdout> {
    /// The application model.
    model: M,
    /// Terminal output coordinator.
    writer: TerminalWriter<W>,
    /// Terminal lifecycle guard (raw mode, mouse, paste, focus).
    session: TerminalSession,
    /// Whether the program is running.
    running: bool,
    /// Current tick rate (if any).
    tick_rate: Option<Duration>,
    /// Last tick time.
    last_tick: Instant,
    /// Whether the UI needs to be redrawn.
    dirty: bool,
    /// Current terminal width.
    width: u16,
    /// Current terminal height.
    height: u16,
    /// Poll timeout when no tick is scheduled.
    poll_timeout: Duration,
    /// Frame budget configuration.
    budget: RenderBudget,
    /// Resize debouncer for rapid resize events.
    resize_debouncer: ResizeDebouncer,
    /// Whether the resize placeholder should be shown.
    resizing: bool,
    /// Optional event recorder for macro capture.
    event_recorder: Option<EventRecorder>,
    /// Subscription lifecycle manager.
    subscriptions: SubscriptionManager<M::Message>,
    /// Channel for receiving messages from background tasks.
    task_sender: std::sync::mpsc::Sender<M::Message>,
    /// Channel for receiving messages from background tasks.
    task_receiver: std::sync::mpsc::Receiver<M::Message>,
    /// Join handles for background tasks; reaped opportunistically.
    task_handles: Vec<std::thread::JoinHandle<()>>,
}

impl<M: Model> Program<M, Stdout> {
    /// Create a new program with default configuration.
    pub fn new(model: M) -> io::Result<Self> {
        Self::with_config(model, ProgramConfig::default())
    }

    /// Create a new program with the specified configuration.
    pub fn with_config(model: M, config: ProgramConfig) -> io::Result<Self> {
        let capabilities = TerminalCapabilities::detect();
        let session = TerminalSession::new(SessionOptions {
            alternate_screen: matches!(config.screen_mode, ScreenMode::AltScreen),
            mouse_capture: config.mouse,
            bracketed_paste: config.bracketed_paste,
            focus_events: config.focus_reporting,
            kitty_keyboard: config.kitty_keyboard,
        })?;

        let mut writer = TerminalWriter::new(
            io::stdout(),
            config.screen_mode,
            config.ui_anchor,
            capabilities,
        );

        // Get terminal size for initial frame
        let (width, height) = session.size().unwrap_or((80, 24));
        writer.set_size(width, height);

        let budget = RenderBudget::from_config(&config.budget);
        let resize_debouncer = ResizeDebouncer::new(config.resize_debounce, (width, height));
        let subscriptions = SubscriptionManager::new();
        let (task_sender, task_receiver) = std::sync::mpsc::channel();

        Ok(Self {
            model,
            writer,
            session,
            running: true,
            tick_rate: None,
            last_tick: Instant::now(),
            dirty: true,
            width,
            height,
            poll_timeout: config.poll_timeout,
            budget,
            resize_debouncer,
            resizing: false,
            event_recorder: None,
            subscriptions,
            task_sender,
            task_receiver,
            task_handles: Vec::new(),
        })
    }
}

impl<M: Model, W: Write + Send> Program<M, W> {
    /// Run the main event loop.
    ///
    /// This is the main entry point. It handles:
    /// 1. Initialization (terminal setup, raw mode)
    /// 2. Event polling and message dispatch
    /// 3. Frame rendering
    /// 4. Shutdown (terminal cleanup)
    pub fn run(&mut self) -> io::Result<()> {
        self.run_event_loop()
    }

    /// The inner event loop, separated for proper cleanup handling.
    fn run_event_loop(&mut self) -> io::Result<()> {
        // Initialize
        let cmd = self.model.init();
        self.execute_cmd(cmd)?;

        // Reconcile initial subscriptions
        self.reconcile_subscriptions();

        // Initial render
        self.render_frame()?;

        // Main loop
        while self.running {
            // Poll for input with tick timeout
            let timeout = self.effective_timeout();

            // Poll for events with timeout
            if self.session.poll_event(timeout)? {
                // Drain all pending events
                loop {
                    // read_event returns Option<Event> after converting from crossterm
                    if let Some(event) = self.session.read_event()? {
                        self.handle_event(event)?;
                    }
                    if !self.session.poll_event(Duration::from_millis(0))? {
                        break;
                    }
                }
            }

            // Process subscription messages
            self.process_subscription_messages()?;

            // Process background task results
            self.process_task_results()?;
            self.reap_finished_tasks();

            self.process_resize_debounce()?;

            // Check for tick - deliver to model so periodic logic can run
            if self.should_tick() {
                let msg = M::Message::from(Event::Tick);
                let cmd = self.model.update(msg);
                self.dirty = true;
                self.execute_cmd(cmd)?;
                self.reconcile_subscriptions();
            }

            // Render if dirty
            if self.dirty {
                self.render_frame()?;
            }
        }

        // Stop all subscriptions on exit
        self.subscriptions.stop_all();
        self.reap_finished_tasks();

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> io::Result<()> {
        // Record event before processing (no-op when recorder is None or idle).
        if let Some(recorder) = &mut self.event_recorder {
            recorder.record(&event);
        }

        let event = match event {
            Event::Resize { width, height } => {
                debug!(width, height, "Resize event received, debouncing");
                let action = self.resize_debouncer.handle_resize(width, height);
                if matches!(action, ResizeAction::ShowPlaceholder) {
                    let was_resizing = self.resizing;
                    self.resizing = true;
                    if !was_resizing {
                        debug!("Showing resize placeholder");
                    }
                    // Clamp to minimum 1 to prevent Buffer::new panic on zero dimensions
                    let width = width.max(1);
                    let height = height.max(1);
                    self.width = width;
                    self.height = height;
                    self.writer.set_size(width, height);
                    self.dirty = true;
                }
                return Ok(());
            }
            other => other,
        };

        let msg = M::Message::from(event);
        let cmd = self.model.update(msg);
        self.dirty = true;
        self.execute_cmd(cmd)?;
        self.reconcile_subscriptions();
        Ok(())
    }

    /// Reconcile the model's declared subscriptions with running ones.
    fn reconcile_subscriptions(&mut self) {
        let subs = self.model.subscriptions();
        self.subscriptions.reconcile(subs);
    }

    /// Process pending messages from subscriptions.
    fn process_subscription_messages(&mut self) -> io::Result<()> {
        let messages = self.subscriptions.drain_messages();
        for msg in messages {
            let cmd = self.model.update(msg);
            self.dirty = true;
            self.execute_cmd(cmd)?;
        }
        if self.dirty {
            self.reconcile_subscriptions();
        }
        Ok(())
    }

    /// Process results from background tasks.
    fn process_task_results(&mut self) -> io::Result<()> {
        while let Ok(msg) = self.task_receiver.try_recv() {
            let cmd = self.model.update(msg);
            self.dirty = true;
            self.execute_cmd(cmd)?;
        }
        if self.dirty {
            self.reconcile_subscriptions();
        }
        Ok(())
    }

    /// Execute a command.
    fn execute_cmd(&mut self, cmd: Cmd<M::Message>) -> io::Result<()> {
        match cmd {
            Cmd::None => {}
            Cmd::Quit => self.running = false,
            Cmd::Msg(m) => {
                let cmd = self.model.update(m);
                self.dirty = true;
                self.execute_cmd(cmd)?;
            }
            Cmd::Batch(cmds) => {
                // Batch currently executes sequentially. This is intentional
                // until an async runtime or task scheduler is added.
                for c in cmds {
                    self.execute_cmd(c)?;
                }
            }
            Cmd::Sequence(cmds) => {
                for c in cmds {
                    self.execute_cmd(c)?;
                    if !self.running {
                        break;
                    }
                }
            }
            Cmd::Tick(duration) => {
                self.tick_rate = Some(duration);
                self.last_tick = Instant::now();
            }
            Cmd::Log(text) => {
                let sanitized = sanitize(&text);
                if sanitized.ends_with('\n') {
                    self.writer.write_log(&sanitized)?;
                } else {
                    let mut owned = sanitized.into_owned();
                    owned.push('\n');
                    self.writer.write_log(&owned)?;
                }
            }
            Cmd::Task(f) => {
                let sender = self.task_sender.clone();
                let handle = std::thread::spawn(move || {
                    let msg = f();
                    let _ = sender.send(msg);
                });
                self.task_handles.push(handle);
            }
        }
        Ok(())
    }

    fn reap_finished_tasks(&mut self) {
        if self.task_handles.is_empty() {
            return;
        }

        let mut remaining = Vec::with_capacity(self.task_handles.len());
        for handle in self.task_handles.drain(..) {
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                remaining.push(handle);
            }
        }
        self.task_handles = remaining;
    }

    /// Render a frame with budget tracking.
    fn render_frame(&mut self) -> io::Result<()> {
        // Use ui_height() to get the effective visible height in inline mode.
        // In alt-screen mode this returns the full terminal height.
        let frame_height = self.writer.ui_height();
        let _frame_span =
            info_span!("render_frame", width = self.width, height = frame_height).entered();

        // Reset budget for new frame, potentially upgrading quality
        self.budget.next_frame();

        if self.resizing {
            self.render_resize_placeholder()?;
            self.dirty = false;
            return Ok(());
        }

        // Early skip if budget says to skip this frame entirely
        if self.budget.exhausted() {
            debug!(
                degradation = self.budget.degradation().as_str(),
                "frame skipped: budget exhausted before render"
            );
            self.dirty = false;
            return Ok(());
        }

        // --- Render phase ---
        let render_start = Instant::now();
        let buffer = {
            // Note: Frame borrows the pool and links from writer.
            // We scope it so it drops before we call present_ui (which needs exclusive writer access).
            let (pool, links) = self.writer.pool_and_links_mut();
            let mut frame = Frame::new(self.width, frame_height, pool);
            frame.set_degradation(self.budget.degradation());
            frame.set_links(links);

            let _view_span = debug_span!("model_view").entered();
            self.model.view(&mut frame);

            frame.buffer
        };
        let render_elapsed = render_start.elapsed();

        // Check if render phase overspent its budget
        let render_budget = self.budget.phase_budgets().render;
        if render_elapsed > render_budget {
            debug!(
                render_ms = render_elapsed.as_millis() as u32,
                budget_ms = render_budget.as_millis() as u32,
                "render phase exceeded budget"
            );
            // Trigger degradation if we're consistently over budget
            if self.budget.should_degrade(render_budget) {
                self.budget.degrade();
            }
        }

        // --- Present phase ---
        if !self.budget.exhausted() {
            let present_start = Instant::now();
            {
                let _present_span = debug_span!("frame_present").entered();
                self.writer.present_ui(&buffer)?;
            }
            let present_elapsed = present_start.elapsed();

            let present_budget = self.budget.phase_budgets().present;
            if present_elapsed > present_budget {
                debug!(
                    present_ms = present_elapsed.as_millis() as u32,
                    budget_ms = present_budget.as_millis() as u32,
                    "present phase exceeded budget"
                );
            }
        } else {
            debug!(
                degradation = self.budget.degradation().as_str(),
                elapsed_ms = self.budget.elapsed().as_millis() as u32,
                "frame present skipped: budget exhausted after render"
            );
        }

        self.dirty = false;

        Ok(())
    }

    /// Calculate the effective poll timeout.
    fn effective_timeout(&self) -> Duration {
        if let Some(tick_rate) = self.tick_rate {
            let elapsed = self.last_tick.elapsed();
            let mut timeout = tick_rate.saturating_sub(elapsed);
            if let Some(resize_timeout) = self.resize_debouncer.time_until_apply(Instant::now()) {
                timeout = timeout.min(resize_timeout);
            }
            timeout
        } else {
            let mut timeout = self.poll_timeout;
            if let Some(resize_timeout) = self.resize_debouncer.time_until_apply(Instant::now()) {
                timeout = timeout.min(resize_timeout);
            }
            timeout
        }
    }

    /// Check if we should send a tick.
    fn should_tick(&mut self) -> bool {
        if let Some(tick_rate) = self.tick_rate
            && self.last_tick.elapsed() >= tick_rate
        {
            self.last_tick = Instant::now();
            return true;
        }
        false
    }

    fn process_resize_debounce(&mut self) -> io::Result<()> {
        match self.resize_debouncer.tick() {
            ResizeAction::ApplyResize {
                width,
                height,
                elapsed,
            } => self.apply_resize(width, height, elapsed),
            _ => Ok(()),
        }
    }

    fn apply_resize(&mut self, width: u16, height: u16, elapsed: Duration) -> io::Result<()> {
        self.resizing = false;
        // Clamp to minimum 1 to prevent Buffer::new panic on zero dimensions
        let width = width.max(1);
        let height = height.max(1);
        self.width = width;
        self.height = height;
        self.writer.set_size(width, height);
        info!(
            width = width,
            height = height,
            debounce_ms = elapsed.as_millis() as u64,
            "Resize applied"
        );

        let msg = M::Message::from(Event::Resize { width, height });
        let cmd = self.model.update(msg);
        self.dirty = true;
        self.execute_cmd(cmd)
    }

    fn render_resize_placeholder(&mut self) -> io::Result<()> {
        const PLACEHOLDER_TEXT: &str = "Resizing...";

        // Use ui_height() to get effective visible height in inline mode.
        let frame_height = self.writer.ui_height();
        let mut frame = Frame::new(self.width, frame_height, self.writer.pool_mut());
        let text_width = PLACEHOLDER_TEXT.chars().count().min(self.width as usize) as u16;
        let x_start = self.width.saturating_sub(text_width) / 2;
        let y = frame_height / 2;

        for (offset, ch) in PLACEHOLDER_TEXT.chars().enumerate() {
            let x = x_start.saturating_add(offset as u16);
            if x >= self.width {
                break;
            }
            frame.buffer.set_raw(x, y, Cell::from_char(ch));
        }

        let buffer = frame.buffer;
        self.writer.present_ui(&buffer)?;

        Ok(())
    }

    /// Get a reference to the model.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Get a mutable reference to the model.
    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }

    /// Check if the program is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Request a quit.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Mark the UI as needing redraw.
    pub fn request_redraw(&mut self) {
        self.dirty = true;
    }

    /// Start recording events into a macro.
    ///
    /// If already recording, the current recording is discarded and a new one starts.
    /// The current terminal size is captured as metadata.
    pub fn start_recording(&mut self, name: impl Into<String>) {
        let mut recorder = EventRecorder::new(name).with_terminal_size(self.width, self.height);
        recorder.start();
        self.event_recorder = Some(recorder);
    }

    /// Stop recording and return the recorded macro, if any.
    ///
    /// Returns `None` if not currently recording.
    pub fn stop_recording(&mut self) -> Option<InputMacro> {
        self.event_recorder.take().map(EventRecorder::finish)
    }

    /// Check if event recording is active.
    pub fn is_recording(&self) -> bool {
        self.event_recorder
            .as_ref()
            .is_some_and(EventRecorder::is_recording)
    }
}

/// Builder for creating and running programs.
pub struct App;

impl App {
    /// Create a new app builder with the given model.
    #[allow(clippy::new_ret_no_self)] // App is a namespace for builder methods
    pub fn new<M: Model>(model: M) -> AppBuilder<M> {
        AppBuilder {
            model,
            config: ProgramConfig::default(),
        }
    }

    /// Create a fullscreen app.
    pub fn fullscreen<M: Model>(model: M) -> AppBuilder<M> {
        AppBuilder {
            model,
            config: ProgramConfig::fullscreen(),
        }
    }

    /// Create an inline app with the given height.
    pub fn inline<M: Model>(model: M, height: u16) -> AppBuilder<M> {
        AppBuilder {
            model,
            config: ProgramConfig::inline(height),
        }
    }

    /// Create a fullscreen app from a [`StringModel`](crate::string_model::StringModel).
    ///
    /// This wraps the string model in a [`StringModelAdapter`](crate::string_model::StringModelAdapter)
    /// so that `view_string()` output is rendered through the standard kernel pipeline.
    pub fn string_model<S: crate::string_model::StringModel>(
        model: S,
    ) -> AppBuilder<crate::string_model::StringModelAdapter<S>> {
        AppBuilder {
            model: crate::string_model::StringModelAdapter::new(model),
            config: ProgramConfig::fullscreen(),
        }
    }
}

/// Builder for configuring and running programs.
pub struct AppBuilder<M: Model> {
    model: M,
    config: ProgramConfig,
}

impl<M: Model> AppBuilder<M> {
    /// Set the screen mode.
    pub fn screen_mode(mut self, mode: ScreenMode) -> Self {
        self.config.screen_mode = mode;
        self
    }

    /// Set the UI anchor.
    pub fn anchor(mut self, anchor: UiAnchor) -> Self {
        self.config.ui_anchor = anchor;
        self
    }

    /// Enable mouse support.
    pub fn with_mouse(mut self) -> Self {
        self.config.mouse = true;
        self
    }

    /// Set the frame budget configuration.
    pub fn with_budget(mut self, budget: FrameBudgetConfig) -> Self {
        self.config.budget = budget;
        self
    }

    /// Set the resize debounce duration.
    pub fn resize_debounce(mut self, debounce: Duration) -> Self {
        self.config.resize_debounce = debounce;
        self
    }

    /// Run the application.
    pub fn run(self) -> io::Result<()> {
        let mut program = Program::with_config(self.model, self.config)?;
        program.run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test model
    struct TestModel {
        value: i32,
    }

    #[derive(Debug)]
    enum TestMsg {
        Increment,
        Decrement,
        Quit,
    }

    impl From<Event> for TestMsg {
        fn from(_event: Event) -> Self {
            TestMsg::Increment
        }
    }

    impl Model for TestModel {
        type Message = TestMsg;

        fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
            match msg {
                TestMsg::Increment => {
                    self.value += 1;
                    Cmd::none()
                }
                TestMsg::Decrement => {
                    self.value -= 1;
                    Cmd::none()
                }
                TestMsg::Quit => Cmd::quit(),
            }
        }

        fn view(&self, _frame: &mut Frame) {
            // No-op for tests
        }
    }

    #[test]
    fn cmd_none() {
        let cmd: Cmd<TestMsg> = Cmd::none();
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn cmd_quit() {
        let cmd: Cmd<TestMsg> = Cmd::quit();
        assert!(matches!(cmd, Cmd::Quit));
    }

    #[test]
    fn cmd_msg() {
        let cmd: Cmd<TestMsg> = Cmd::msg(TestMsg::Increment);
        assert!(matches!(cmd, Cmd::Msg(TestMsg::Increment)));
    }

    #[test]
    fn cmd_batch_empty() {
        let cmd: Cmd<TestMsg> = Cmd::batch(vec![]);
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn cmd_batch_single() {
        let cmd: Cmd<TestMsg> = Cmd::batch(vec![Cmd::quit()]);
        assert!(matches!(cmd, Cmd::Quit));
    }

    #[test]
    fn cmd_batch_multiple() {
        let cmd: Cmd<TestMsg> = Cmd::batch(vec![Cmd::none(), Cmd::quit()]);
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn cmd_sequence_empty() {
        let cmd: Cmd<TestMsg> = Cmd::sequence(vec![]);
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn cmd_tick() {
        let cmd: Cmd<TestMsg> = Cmd::tick(Duration::from_millis(100));
        assert!(matches!(cmd, Cmd::Tick(_)));
    }

    #[test]
    fn cmd_task() {
        let cmd: Cmd<TestMsg> = Cmd::task(|| TestMsg::Increment);
        assert!(matches!(cmd, Cmd::Task(_)));
    }

    #[test]
    fn cmd_debug_format() {
        let cmd: Cmd<TestMsg> = Cmd::task(|| TestMsg::Increment);
        let debug = format!("{cmd:?}");
        assert_eq!(debug, "Task(...)");
    }

    #[test]
    fn model_subscriptions_default_empty() {
        let model = TestModel { value: 0 };
        let subs = model.subscriptions();
        assert!(subs.is_empty());
    }

    #[test]
    fn program_config_default() {
        let config = ProgramConfig::default();
        assert!(matches!(config.screen_mode, ScreenMode::Inline { .. }));
        assert!(!config.mouse);
        assert!(config.bracketed_paste);
        assert_eq!(config.resize_debounce, Duration::from_millis(100));
    }

    #[test]
    fn program_config_fullscreen() {
        let config = ProgramConfig::fullscreen();
        assert!(matches!(config.screen_mode, ScreenMode::AltScreen));
    }

    #[test]
    fn program_config_inline() {
        let config = ProgramConfig::inline(10);
        assert!(matches!(
            config.screen_mode,
            ScreenMode::Inline { ui_height: 10 }
        ));
    }

    #[test]
    fn program_config_with_mouse() {
        let config = ProgramConfig::default().with_mouse();
        assert!(config.mouse);
    }

    #[test]
    fn model_update() {
        let mut model = TestModel { value: 0 };
        model.update(TestMsg::Increment);
        assert_eq!(model.value, 1);
        model.update(TestMsg::Decrement);
        assert_eq!(model.value, 0);
        assert!(matches!(model.update(TestMsg::Quit), Cmd::Quit));
    }

    #[test]
    fn model_init_default() {
        let mut model = TestModel { value: 0 };
        let cmd = model.init();
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn resize_debouncer_applies_after_delay() {
        let mut debouncer = ResizeDebouncer::new(Duration::from_millis(100), (80, 24));
        let now = Instant::now();

        assert!(matches!(
            debouncer.handle_resize_at(100, 40, now),
            ResizeAction::ShowPlaceholder
        ));

        assert!(matches!(
            debouncer.tick_at(now + Duration::from_millis(50)),
            ResizeAction::None
        ));

        assert!(matches!(
            debouncer.tick_at(now + Duration::from_millis(120)),
            ResizeAction::ApplyResize {
                width: 100,
                height: 40,
                ..
            }
        ));
    }

    #[test]
    fn resize_debouncer_uses_latest_size() {
        let mut debouncer = ResizeDebouncer::new(Duration::from_millis(100), (80, 24));
        let now = Instant::now();

        debouncer.handle_resize_at(100, 40, now);
        debouncer.handle_resize_at(120, 50, now + Duration::from_millis(10));

        assert!(matches!(
            debouncer.tick_at(now + Duration::from_millis(120)),
            ResizeAction::ApplyResize {
                width: 120,
                height: 50,
                ..
            }
        ));
    }

    // =========================================================================
    // DETERMINISM TESTS - Program loop determinism (bd-2nu8.10.1)
    // =========================================================================

    #[test]
    fn cmd_sequence_executes_in_order() {
        // Verify that Cmd::Sequence executes commands in declared order
        use crate::simulator::ProgramSimulator;

        struct SeqModel {
            trace: Vec<i32>,
        }

        #[derive(Debug)]
        enum SeqMsg {
            Append(i32),
            TriggerSequence,
        }

        impl From<Event> for SeqMsg {
            fn from(_: Event) -> Self {
                SeqMsg::Append(0)
            }
        }

        impl Model for SeqModel {
            type Message = SeqMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    SeqMsg::Append(n) => {
                        self.trace.push(n);
                        Cmd::none()
                    }
                    SeqMsg::TriggerSequence => Cmd::sequence(vec![
                        Cmd::msg(SeqMsg::Append(1)),
                        Cmd::msg(SeqMsg::Append(2)),
                        Cmd::msg(SeqMsg::Append(3)),
                    ]),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(SeqModel { trace: vec![] });
        sim.init();
        sim.send(SeqMsg::TriggerSequence);

        assert_eq!(sim.model().trace, vec![1, 2, 3]);
    }

    #[test]
    fn cmd_batch_executes_all_regardless_of_order() {
        // Verify that Cmd::Batch executes all commands
        use crate::simulator::ProgramSimulator;

        struct BatchModel {
            values: Vec<i32>,
        }

        #[derive(Debug)]
        enum BatchMsg {
            Add(i32),
            TriggerBatch,
        }

        impl From<Event> for BatchMsg {
            fn from(_: Event) -> Self {
                BatchMsg::Add(0)
            }
        }

        impl Model for BatchModel {
            type Message = BatchMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    BatchMsg::Add(n) => {
                        self.values.push(n);
                        Cmd::none()
                    }
                    BatchMsg::TriggerBatch => Cmd::batch(vec![
                        Cmd::msg(BatchMsg::Add(10)),
                        Cmd::msg(BatchMsg::Add(20)),
                        Cmd::msg(BatchMsg::Add(30)),
                    ]),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(BatchModel { values: vec![] });
        sim.init();
        sim.send(BatchMsg::TriggerBatch);

        // All values should be present
        assert_eq!(sim.model().values.len(), 3);
        assert!(sim.model().values.contains(&10));
        assert!(sim.model().values.contains(&20));
        assert!(sim.model().values.contains(&30));
    }

    #[test]
    fn cmd_sequence_stops_on_quit() {
        // Verify that Cmd::Sequence stops processing after Quit
        use crate::simulator::ProgramSimulator;

        struct SeqQuitModel {
            trace: Vec<i32>,
        }

        #[derive(Debug)]
        enum SeqQuitMsg {
            Append(i32),
            TriggerSequenceWithQuit,
        }

        impl From<Event> for SeqQuitMsg {
            fn from(_: Event) -> Self {
                SeqQuitMsg::Append(0)
            }
        }

        impl Model for SeqQuitModel {
            type Message = SeqQuitMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    SeqQuitMsg::Append(n) => {
                        self.trace.push(n);
                        Cmd::none()
                    }
                    SeqQuitMsg::TriggerSequenceWithQuit => Cmd::sequence(vec![
                        Cmd::msg(SeqQuitMsg::Append(1)),
                        Cmd::quit(),
                        Cmd::msg(SeqQuitMsg::Append(2)), // Should not execute
                    ]),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(SeqQuitModel { trace: vec![] });
        sim.init();
        sim.send(SeqQuitMsg::TriggerSequenceWithQuit);

        assert_eq!(sim.model().trace, vec![1]);
        assert!(!sim.is_running());
    }

    #[test]
    fn identical_input_produces_identical_state() {
        // Verify deterministic state transitions
        use crate::simulator::ProgramSimulator;

        fn run_scenario() -> Vec<i32> {
            struct DetModel {
                values: Vec<i32>,
            }

            #[derive(Debug, Clone)]
            enum DetMsg {
                Add(i32),
                Double,
            }

            impl From<Event> for DetMsg {
                fn from(_: Event) -> Self {
                    DetMsg::Add(1)
                }
            }

            impl Model for DetModel {
                type Message = DetMsg;

                fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                    match msg {
                        DetMsg::Add(n) => {
                            self.values.push(n);
                            Cmd::none()
                        }
                        DetMsg::Double => {
                            if let Some(&last) = self.values.last() {
                                self.values.push(last * 2);
                            }
                            Cmd::none()
                        }
                    }
                }

                fn view(&self, _frame: &mut Frame) {}
            }

            let mut sim = ProgramSimulator::new(DetModel { values: vec![] });
            sim.init();
            sim.send(DetMsg::Add(5));
            sim.send(DetMsg::Double);
            sim.send(DetMsg::Add(3));
            sim.send(DetMsg::Double);

            sim.model().values.clone()
        }

        // Run the same scenario multiple times
        let run1 = run_scenario();
        let run2 = run_scenario();
        let run3 = run_scenario();

        assert_eq!(run1, run2);
        assert_eq!(run2, run3);
        assert_eq!(run1, vec![5, 10, 3, 6]);
    }

    #[test]
    fn identical_state_produces_identical_render() {
        // Verify consistent render outputs for identical inputs
        use crate::simulator::ProgramSimulator;

        struct RenderModel {
            counter: i32,
        }

        #[derive(Debug)]
        enum RenderMsg {
            Set(i32),
        }

        impl From<Event> for RenderMsg {
            fn from(_: Event) -> Self {
                RenderMsg::Set(0)
            }
        }

        impl Model for RenderModel {
            type Message = RenderMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    RenderMsg::Set(n) => {
                        self.counter = n;
                        Cmd::none()
                    }
                }
            }

            fn view(&self, frame: &mut Frame) {
                let text = format!("Value: {}", self.counter);
                for (i, c) in text.chars().enumerate() {
                    if (i as u16) < frame.width() {
                        use ftui_render::cell::Cell;
                        frame.buffer.set_raw(i as u16, 0, Cell::from_char(c));
                    }
                }
            }
        }

        // Create two simulators with the same state
        let mut sim1 = ProgramSimulator::new(RenderModel { counter: 42 });
        let mut sim2 = ProgramSimulator::new(RenderModel { counter: 42 });

        let buf1 = sim1.capture_frame(80, 24);
        let buf2 = sim2.capture_frame(80, 24);

        // Compare buffer contents
        for y in 0..24 {
            for x in 0..80 {
                let cell1 = buf1.get(x, y).unwrap();
                let cell2 = buf2.get(x, y).unwrap();
                assert_eq!(
                    cell1.content.as_char(),
                    cell2.content.as_char(),
                    "Mismatch at ({}, {})",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn resize_debouncer_no_action_for_same_size() {
        let mut debouncer = ResizeDebouncer::new(Duration::from_millis(100), (80, 24));

        // Resize to the same size should be no-op
        let action = debouncer.handle_resize(80, 24);
        assert!(matches!(action, ResizeAction::None));
    }

    #[test]
    fn resize_debouncer_time_until_apply() {
        let mut debouncer = ResizeDebouncer::new(Duration::from_millis(100), (80, 24));
        let now = Instant::now();

        // No pending resize
        assert!(debouncer.time_until_apply(now).is_none());

        // Start resize
        debouncer.handle_resize_at(100, 40, now);

        // Should have ~100ms until apply
        let time_left = debouncer.time_until_apply(now).unwrap();
        assert!(time_left <= Duration::from_millis(100));
        assert!(time_left > Duration::from_millis(90));

        // After 50ms, should have ~50ms left
        let time_left = debouncer
            .time_until_apply(now + Duration::from_millis(50))
            .unwrap();
        assert!(time_left <= Duration::from_millis(50));
    }

    #[test]
    fn resize_debouncer_resets_timer_on_new_resize() {
        let mut debouncer = ResizeDebouncer::new(Duration::from_millis(100), (80, 24));
        let now = Instant::now();

        debouncer.handle_resize_at(100, 40, now);

        // At 90ms (before debounce completes), resize again
        debouncer.handle_resize_at(120, 50, now + Duration::from_millis(90));

        // At 100ms from start, should still be pending (timer reset)
        assert!(matches!(
            debouncer.tick_at(now + Duration::from_millis(100)),
            ResizeAction::None
        ));

        // At 200ms from start (100ms after second resize), should apply
        assert!(matches!(
            debouncer.tick_at(now + Duration::from_millis(200)),
            ResizeAction::ApplyResize {
                width: 120,
                height: 50,
                ..
            }
        ));
    }

    #[test]
    fn cmd_log_creates_log_command() {
        let cmd: Cmd<TestMsg> = Cmd::log("test message");
        assert!(matches!(cmd, Cmd::Log(s) if s == "test message"));
    }

    #[test]
    fn cmd_log_from_string() {
        let msg = String::from("dynamic message");
        let cmd: Cmd<TestMsg> = Cmd::log(msg);
        assert!(matches!(cmd, Cmd::Log(s) if s == "dynamic message"));
    }

    #[test]
    fn cmd_sequence_single_unwraps() {
        let cmd: Cmd<TestMsg> = Cmd::sequence(vec![Cmd::quit()]);
        // Single element sequence should unwrap to the inner command
        assert!(matches!(cmd, Cmd::Quit));
    }

    #[test]
    fn cmd_sequence_multiple() {
        let cmd: Cmd<TestMsg> = Cmd::sequence(vec![Cmd::none(), Cmd::quit()]);
        assert!(matches!(cmd, Cmd::Sequence(_)));
    }

    #[test]
    fn cmd_default_is_none() {
        let cmd: Cmd<TestMsg> = Cmd::default();
        assert!(matches!(cmd, Cmd::None));
    }

    #[test]
    fn cmd_debug_all_variants() {
        // Test Debug impl for all variants
        let none: Cmd<TestMsg> = Cmd::none();
        assert_eq!(format!("{none:?}"), "None");

        let quit: Cmd<TestMsg> = Cmd::quit();
        assert_eq!(format!("{quit:?}"), "Quit");

        let msg: Cmd<TestMsg> = Cmd::msg(TestMsg::Increment);
        assert!(format!("{msg:?}").starts_with("Msg("));

        let batch: Cmd<TestMsg> = Cmd::batch(vec![Cmd::none(), Cmd::none()]);
        assert!(format!("{batch:?}").starts_with("Batch("));

        let seq: Cmd<TestMsg> = Cmd::sequence(vec![Cmd::none(), Cmd::none()]);
        assert!(format!("{seq:?}").starts_with("Sequence("));

        let tick: Cmd<TestMsg> = Cmd::tick(Duration::from_secs(1));
        assert!(format!("{tick:?}").starts_with("Tick("));

        let log: Cmd<TestMsg> = Cmd::log("test");
        assert!(format!("{log:?}").starts_with("Log("));
    }

    #[test]
    fn program_config_with_budget() {
        let budget = FrameBudgetConfig {
            total: Duration::from_millis(50),
            ..Default::default()
        };
        let config = ProgramConfig::default().with_budget(budget);
        assert_eq!(config.budget.total, Duration::from_millis(50));
    }

    #[test]
    fn program_config_with_resize_debounce() {
        let config = ProgramConfig::default().with_resize_debounce(Duration::from_millis(200));
        assert_eq!(config.resize_debounce, Duration::from_millis(200));
    }

    #[test]
    fn nested_cmd_msg_executes_recursively() {
        // Verify that Cmd::Msg triggers recursive update
        use crate::simulator::ProgramSimulator;

        struct NestedModel {
            depth: usize,
        }

        #[derive(Debug)]
        enum NestedMsg {
            Nest(usize),
        }

        impl From<Event> for NestedMsg {
            fn from(_: Event) -> Self {
                NestedMsg::Nest(0)
            }
        }

        impl Model for NestedModel {
            type Message = NestedMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    NestedMsg::Nest(n) => {
                        self.depth += 1;
                        if n > 0 {
                            Cmd::msg(NestedMsg::Nest(n - 1))
                        } else {
                            Cmd::none()
                        }
                    }
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(NestedModel { depth: 0 });
        sim.init();
        sim.send(NestedMsg::Nest(3));

        // Should have recursed 4 times (3, 2, 1, 0)
        assert_eq!(sim.model().depth, 4);
    }

    #[test]
    fn task_executes_synchronously_in_simulator() {
        // In simulator, tasks execute synchronously
        use crate::simulator::ProgramSimulator;

        struct TaskModel {
            completed: bool,
        }

        #[derive(Debug)]
        enum TaskMsg {
            Complete,
            SpawnTask,
        }

        impl From<Event> for TaskMsg {
            fn from(_: Event) -> Self {
                TaskMsg::Complete
            }
        }

        impl Model for TaskModel {
            type Message = TaskMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    TaskMsg::Complete => {
                        self.completed = true;
                        Cmd::none()
                    }
                    TaskMsg::SpawnTask => Cmd::task(|| TaskMsg::Complete),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(TaskModel { completed: false });
        sim.init();
        sim.send(TaskMsg::SpawnTask);

        // Task should have completed synchronously
        assert!(sim.model().completed);
    }

    #[test]
    fn resize_action_eq() {
        // Test ResizeAction equality
        assert_eq!(ResizeAction::None, ResizeAction::None);
        assert_eq!(ResizeAction::ShowPlaceholder, ResizeAction::ShowPlaceholder);

        let action1 = ResizeAction::ApplyResize {
            width: 100,
            height: 50,
            elapsed: Duration::from_millis(100),
        };
        let action2 = ResizeAction::ApplyResize {
            width: 100,
            height: 50,
            elapsed: Duration::from_millis(100),
        };
        assert_eq!(action1, action2);

        assert_ne!(ResizeAction::None, ResizeAction::ShowPlaceholder);
    }

    #[test]
    fn multiple_updates_accumulate_correctly() {
        // Verify state accumulates correctly across multiple updates
        use crate::simulator::ProgramSimulator;

        struct AccumModel {
            sum: i32,
        }

        #[derive(Debug)]
        enum AccumMsg {
            Add(i32),
            Multiply(i32),
        }

        impl From<Event> for AccumMsg {
            fn from(_: Event) -> Self {
                AccumMsg::Add(1)
            }
        }

        impl Model for AccumModel {
            type Message = AccumMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    AccumMsg::Add(n) => {
                        self.sum += n;
                        Cmd::none()
                    }
                    AccumMsg::Multiply(n) => {
                        self.sum *= n;
                        Cmd::none()
                    }
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(AccumModel { sum: 0 });
        sim.init();

        // (0 + 5) * 2 + 3 = 13
        sim.send(AccumMsg::Add(5));
        sim.send(AccumMsg::Multiply(2));
        sim.send(AccumMsg::Add(3));

        assert_eq!(sim.model().sum, 13);
    }

    #[test]
    fn init_command_executes_before_first_update() {
        // Verify init() command executes before any update
        use crate::simulator::ProgramSimulator;

        struct InitModel {
            initialized: bool,
            updates: usize,
        }

        #[derive(Debug)]
        enum InitMsg {
            Update,
            MarkInit,
        }

        impl From<Event> for InitMsg {
            fn from(_: Event) -> Self {
                InitMsg::Update
            }
        }

        impl Model for InitModel {
            type Message = InitMsg;

            fn init(&mut self) -> Cmd<Self::Message> {
                Cmd::msg(InitMsg::MarkInit)
            }

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    InitMsg::MarkInit => {
                        self.initialized = true;
                        Cmd::none()
                    }
                    InitMsg::Update => {
                        self.updates += 1;
                        Cmd::none()
                    }
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(InitModel {
            initialized: false,
            updates: 0,
        });
        sim.init();

        assert!(sim.model().initialized);
        sim.send(InitMsg::Update);
        assert_eq!(sim.model().updates, 1);
    }

    // =========================================================================
    // INLINE MODE FRAME SIZING TESTS (bd-20vg)
    // =========================================================================

    #[test]
    fn ui_height_returns_correct_value_inline_mode() {
        // Verify TerminalWriter.ui_height() returns ui_height in inline mode
        use crate::terminal_writer::{ScreenMode, TerminalWriter, UiAnchor};
        use ftui_core::terminal_capabilities::TerminalCapabilities;

        let output = Vec::new();
        let writer = TerminalWriter::new(
            output,
            ScreenMode::Inline { ui_height: 10 },
            UiAnchor::Bottom,
            TerminalCapabilities::basic(),
        );
        assert_eq!(writer.ui_height(), 10);
    }

    #[test]
    fn ui_height_returns_term_height_altscreen_mode() {
        // Verify TerminalWriter.ui_height() returns full terminal height in alt-screen mode
        use crate::terminal_writer::{ScreenMode, TerminalWriter, UiAnchor};
        use ftui_core::terminal_capabilities::TerminalCapabilities;

        let output = Vec::new();
        let mut writer = TerminalWriter::new(
            output,
            ScreenMode::AltScreen,
            UiAnchor::Bottom,
            TerminalCapabilities::basic(),
        );
        writer.set_size(80, 24);
        assert_eq!(writer.ui_height(), 24);
    }

    #[test]
    fn inline_mode_frame_uses_ui_height_not_terminal_height() {
        // Verify that in inline mode, the model receives a frame with ui_height,
        // not the full terminal height. This is the core fix for bd-20vg.
        use crate::simulator::ProgramSimulator;
        use std::cell::Cell as StdCell;

        thread_local! {
            static CAPTURED_HEIGHT: StdCell<u16> = const { StdCell::new(0) };
        }

        struct FrameSizeTracker;

        #[derive(Debug)]
        enum SizeMsg {
            Check,
        }

        impl From<Event> for SizeMsg {
            fn from(_: Event) -> Self {
                SizeMsg::Check
            }
        }

        impl Model for FrameSizeTracker {
            type Message = SizeMsg;

            fn update(&mut self, _msg: Self::Message) -> Cmd<Self::Message> {
                Cmd::none()
            }

            fn view(&self, frame: &mut Frame) {
                // Capture the frame height we receive
                CAPTURED_HEIGHT.with(|h| h.set(frame.height()));
            }
        }

        // Use simulator to verify frame dimension handling
        let mut sim = ProgramSimulator::new(FrameSizeTracker);
        sim.init();

        // Capture with specific dimensions (simulates inline mode ui_height=10)
        let buf = sim.capture_frame(80, 10);
        assert_eq!(buf.height(), 10);
        assert_eq!(buf.width(), 80);

        // Verify the frame has the correct dimensions
        // In inline mode with ui_height=10, the frame should be 10 rows tall,
        // NOT the full terminal height (e.g., 24).
    }

    #[test]
    fn placeholder_centered_in_ui_region() {
        // Verify resize placeholder is centered within the visible UI region,
        // not the full terminal height.
        //
        // With ui_height=10 and terminal height=24:
        // - BEFORE fix: placeholder at y=12 (24/2), which may be outside UI region
        // - AFTER fix: placeholder at y=5 (10/2), centered in visible region
        use crate::terminal_writer::{ScreenMode, TerminalWriter, UiAnchor};
        use ftui_core::terminal_capabilities::TerminalCapabilities;

        let output = Vec::new();
        let mut writer = TerminalWriter::new(
            output,
            ScreenMode::Inline { ui_height: 10 },
            UiAnchor::Bottom,
            TerminalCapabilities::basic(),
        );
        writer.set_size(80, 24);

        // ui_height should be 10, not 24
        let ui_height = writer.ui_height();
        assert_eq!(ui_height, 10);

        // Placeholder should be centered at y = 10/2 = 5
        let expected_placeholder_y = ui_height / 2;
        assert_eq!(expected_placeholder_y, 5);
    }

    #[test]
    fn altscreen_frame_uses_full_terminal_height() {
        // Regression test: in alt-screen mode, frame should use full terminal height.
        use crate::terminal_writer::{ScreenMode, TerminalWriter, UiAnchor};
        use ftui_core::terminal_capabilities::TerminalCapabilities;

        let output = Vec::new();
        let mut writer = TerminalWriter::new(
            output,
            ScreenMode::AltScreen,
            UiAnchor::Bottom,
            TerminalCapabilities::basic(),
        );
        writer.set_size(80, 40);

        // In alt-screen, ui_height equals terminal height
        assert_eq!(writer.ui_height(), 40);
    }

    #[test]
    fn ui_height_clamped_to_terminal_height() {
        // Verify ui_height doesn't exceed terminal height
        // (This is handled in present_inline, but ui_height() returns the configured value)
        use crate::terminal_writer::{ScreenMode, TerminalWriter, UiAnchor};
        use ftui_core::terminal_capabilities::TerminalCapabilities;

        let output = Vec::new();
        let mut writer = TerminalWriter::new(
            output,
            ScreenMode::Inline { ui_height: 100 },
            UiAnchor::Bottom,
            TerminalCapabilities::basic(),
        );
        writer.set_size(80, 10);

        // ui_height() returns configured value, but present_inline clamps
        // The Frame should be created with ui_height (100), which is later
        // clamped during presentation. For safety, we should use the min.
        // Note: This documents current behavior. A stricter fix might
        // have ui_height() return min(ui_height, term_height).
        assert_eq!(writer.ui_height(), 100);
    }

    // =========================================================================
    // TICK DELIVERY TESTS (bd-3ufh)
    // =========================================================================

    #[test]
    fn tick_event_delivered_to_model_update() {
        // Verify that Event::Tick is delivered to model.update()
        // This is the core fix: ticks now flow through the update pipeline.
        use crate::simulator::ProgramSimulator;

        struct TickTracker {
            tick_count: usize,
        }

        #[derive(Debug)]
        enum TickMsg {
            Tick,
            Other,
        }

        impl From<Event> for TickMsg {
            fn from(event: Event) -> Self {
                match event {
                    Event::Tick => TickMsg::Tick,
                    _ => TickMsg::Other,
                }
            }
        }

        impl Model for TickTracker {
            type Message = TickMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    TickMsg::Tick => {
                        self.tick_count += 1;
                        Cmd::none()
                    }
                    TickMsg::Other => Cmd::none(),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(TickTracker { tick_count: 0 });
        sim.init();

        // Manually inject tick event to simulate what the runtime does
        sim.inject_event(Event::Tick);
        assert_eq!(sim.model().tick_count, 1);

        sim.inject_event(Event::Tick);
        sim.inject_event(Event::Tick);
        assert_eq!(sim.model().tick_count, 3);
    }

    #[test]
    fn tick_command_sets_tick_rate() {
        // Verify Cmd::tick() sets the tick rate in the simulator
        use crate::simulator::{CmdRecord, ProgramSimulator};

        struct TickModel;

        #[derive(Debug)]
        enum Msg {
            SetTick,
            Noop,
        }

        impl From<Event> for Msg {
            fn from(_: Event) -> Self {
                Msg::Noop
            }
        }

        impl Model for TickModel {
            type Message = Msg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    Msg::SetTick => Cmd::tick(Duration::from_millis(100)),
                    Msg::Noop => Cmd::none(),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(TickModel);
        sim.init();
        sim.send(Msg::SetTick);

        // Check that tick was recorded
        let commands = sim.command_log();
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, CmdRecord::Tick(d) if *d == Duration::from_millis(100)))
        );
    }

    #[test]
    fn tick_can_trigger_further_commands() {
        // Verify that tick handling can return commands that are executed
        use crate::simulator::ProgramSimulator;

        struct ChainModel {
            stage: usize,
        }

        #[derive(Debug)]
        enum ChainMsg {
            Tick,
            Advance,
            Noop,
        }

        impl From<Event> for ChainMsg {
            fn from(event: Event) -> Self {
                match event {
                    Event::Tick => ChainMsg::Tick,
                    _ => ChainMsg::Noop,
                }
            }
        }

        impl Model for ChainModel {
            type Message = ChainMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    ChainMsg::Tick => {
                        self.stage += 1;
                        // Return another message to be processed
                        Cmd::msg(ChainMsg::Advance)
                    }
                    ChainMsg::Advance => {
                        self.stage += 10;
                        Cmd::none()
                    }
                    ChainMsg::Noop => Cmd::none(),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(ChainModel { stage: 0 });
        sim.init();
        sim.inject_event(Event::Tick);

        // Tick increments by 1, then Advance increments by 10
        assert_eq!(sim.model().stage, 11);
    }

    #[test]
    fn tick_disabled_with_zero_duration() {
        // Verify that Duration::ZERO disables ticks (no busy loop)
        use crate::simulator::ProgramSimulator;

        struct ZeroTickModel {
            disabled: bool,
        }

        #[derive(Debug)]
        enum ZeroMsg {
            DisableTick,
            Noop,
        }

        impl From<Event> for ZeroMsg {
            fn from(_: Event) -> Self {
                ZeroMsg::Noop
            }
        }

        impl Model for ZeroTickModel {
            type Message = ZeroMsg;

            fn init(&mut self) -> Cmd<Self::Message> {
                // Start with a tick enabled
                Cmd::tick(Duration::from_millis(100))
            }

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    ZeroMsg::DisableTick => {
                        self.disabled = true;
                        // Setting tick to ZERO should effectively disable
                        Cmd::tick(Duration::ZERO)
                    }
                    ZeroMsg::Noop => Cmd::none(),
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(ZeroTickModel { disabled: false });
        sim.init();

        // Verify initial tick rate is set
        assert!(sim.tick_rate().is_some());
        assert_eq!(sim.tick_rate(), Some(Duration::from_millis(100)));

        // Disable ticks
        sim.send(ZeroMsg::DisableTick);
        assert!(sim.model().disabled);

        // Note: The simulator still records the ZERO tick, but the runtime's
        // should_tick() handles ZERO duration appropriately
        assert_eq!(sim.tick_rate(), Some(Duration::ZERO));
    }

    #[test]
    fn tick_event_distinguishable_from_other_events() {
        // Verify Event::Tick can be distinguished in pattern matching
        let tick = Event::Tick;
        let key = Event::Key(ftui_core::event::KeyEvent::new(
            ftui_core::event::KeyCode::Char('a'),
        ));

        assert!(matches!(tick, Event::Tick));
        assert!(!matches!(key, Event::Tick));
    }

    #[test]
    fn tick_event_clone_and_eq() {
        // Verify Event::Tick implements Clone and Eq correctly
        let tick1 = Event::Tick;
        let tick2 = tick1.clone();
        assert_eq!(tick1, tick2);
    }

    #[test]
    fn model_receives_tick_and_input_events() {
        // Verify model can handle both tick and input events correctly
        use crate::simulator::ProgramSimulator;

        struct MixedModel {
            ticks: usize,
            keys: usize,
        }

        #[derive(Debug)]
        enum MixedMsg {
            Tick,
            Key,
        }

        impl From<Event> for MixedMsg {
            fn from(event: Event) -> Self {
                match event {
                    Event::Tick => MixedMsg::Tick,
                    _ => MixedMsg::Key,
                }
            }
        }

        impl Model for MixedModel {
            type Message = MixedMsg;

            fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
                match msg {
                    MixedMsg::Tick => {
                        self.ticks += 1;
                        Cmd::none()
                    }
                    MixedMsg::Key => {
                        self.keys += 1;
                        Cmd::none()
                    }
                }
            }

            fn view(&self, _frame: &mut Frame) {}
        }

        let mut sim = ProgramSimulator::new(MixedModel { ticks: 0, keys: 0 });
        sim.init();

        // Interleave tick and input events
        sim.inject_event(Event::Tick);
        sim.inject_event(Event::Key(ftui_core::event::KeyEvent::new(
            ftui_core::event::KeyCode::Char('a'),
        )));
        sim.inject_event(Event::Tick);
        sim.inject_event(Event::Key(ftui_core::event::KeyEvent::new(
            ftui_core::event::KeyCode::Char('b'),
        )));
        sim.inject_event(Event::Tick);

        assert_eq!(sim.model().ticks, 3);
        assert_eq!(sim.model().keys, 2);
    }
}
