#![forbid(unsafe_code)]

//! Platform-independent runner core wrapping `StepProgram<AppModel>`.
//!
//! This module contains the logic shared between the wasm-bindgen exports
//! and the native test harness. No JS/WASM types here.

use core::time::Duration;

use ftui_demo_showcase::app::AppModel;
use ftui_layout::{
    PaneModifierSnapshot, PanePointerButton, PanePointerPosition, PaneResizeTarget, SplitAxis,
};
use ftui_web::pane_pointer_capture::{
    PanePointerCaptureAdapter, PanePointerCaptureCommand, PanePointerCaptureConfig,
    PanePointerDispatch, PanePointerIgnoredReason, PanePointerLifecyclePhase, PanePointerLogEntry,
    PanePointerLogOutcome,
};
use ftui_web::step_program::{StepProgram, StepResult};
use ftui_web::{WebFlatPatchBatch, WebPatchStats};

/// Platform-independent showcase runner wrapping `StepProgram<AppModel>`.
pub struct RunnerCore {
    inner: StepProgram<AppModel>,
    /// Cached patch hash from the last `take_flat_patches()` call.
    cached_patch_hash: Option<String>,
    /// Cached patch stats from the last `take_flat_patches()` call.
    cached_patch_stats: Option<WebPatchStats>,
    /// Cached logs from the last `take_flat_patches()` call.
    cached_logs: Vec<String>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    /// Reusable cell buffer for flat patch output (avoids per-frame allocation).
    flat_cells_buf: Vec<u32>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    /// Reusable span buffer for flat patch output (avoids per-frame allocation).
    flat_spans_buf: Vec<u32>,
    /// Deterministic pane pointer lifecycle adapter for wasm-hosted pane interactions.
    pane_adapter: PanePointerCaptureAdapter,
    /// Structured pane interaction logs (kept separate from presenter output logs).
    pane_logs: Vec<String>,
}

const PATCH_HASH_ALGO: &str = "fnv1a64";
const FNV64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV64_PRIME: u64 = 0x100000001b3;

/// Host-facing outcome category for one pane lifecycle dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDispatchOutcome {
    SemanticForwarded,
    CaptureStateUpdated,
    Ignored(PanePointerIgnoredReason),
}

/// Host-facing summary of one pane lifecycle dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneDispatchSummary {
    pub phase: PanePointerLifecyclePhase,
    pub sequence: Option<u64>,
    pub pointer_id: Option<u32>,
    pub target: Option<PaneResizeTarget>,
    pub capture_command: Option<PanePointerCaptureCommand>,
    pub outcome: PaneDispatchOutcome,
}

impl PaneDispatchSummary {
    #[must_use]
    pub const fn accepted(self) -> bool {
        !matches!(self.outcome, PaneDispatchOutcome::Ignored(_))
    }
}

impl RunnerCore {
    /// Create a new runner with the given initial terminal dimensions.
    pub fn new(cols: u16, rows: u16) -> Self {
        let model = AppModel::default();
        Self {
            inner: StepProgram::new(model, cols, rows),
            cached_patch_hash: None,
            cached_patch_stats: None,
            cached_logs: Vec::new(),
            flat_cells_buf: Vec::new(),
            flat_spans_buf: Vec::new(),
            pane_adapter: PanePointerCaptureAdapter::new(PanePointerCaptureConfig::default())
                .expect("default pane pointer adapter config should be valid"),
            pane_logs: Vec::new(),
        }
    }

    /// Initialize the model and render the first frame. Call exactly once.
    pub fn init(&mut self) {
        self.inner
            .init()
            .expect("StepProgram init should not fail on WebBackend");
        self.refresh_cached_patch_meta_from_live_outputs();
    }

    /// Advance the deterministic clock by `dt_ms` milliseconds.
    pub fn advance_time_ms(&mut self, dt_ms: f64) {
        let duration = Duration::from_secs_f64(dt_ms / 1000.0);
        self.inner.advance_time(duration);
    }

    /// Set the deterministic clock to absolute nanoseconds.
    pub fn set_time_ns(&mut self, ts_ns: f64) {
        let duration = Duration::from_nanos(ts_ns as u64);
        self.inner.set_time(duration);
    }

    /// Parse a JSON-encoded input event and push to the event queue.
    ///
    /// Returns `true` if the event was accepted, `false` if it was
    /// unsupported, malformed, or had no `Event` mapping.
    pub fn push_encoded_input(&mut self, json: &str) -> bool {
        match ftui_web::input_parser::parse_encoded_input_to_event(json) {
            Ok(Some(event)) => {
                self.inner.push_event(event);
                true
            }
            _ => false,
        }
    }

    /// Resize the terminal. Pushes a `Resize` event processed on the next step.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.inner.resize(cols, rows);
    }

    /// Process pending events and render if dirty.
    pub fn step(&mut self) -> StepResult {
        let result = self
            .inner
            .step()
            .expect("StepProgram step should not fail on WebBackend");
        if result.rendered {
            self.refresh_cached_patch_meta_from_live_outputs();
        }
        result
    }

    /// Take the flat patch batch for GPU upload.
    ///
    /// Also caches patch hash, stats, and logs so they can be read
    /// via `patch_hash()`, `patch_stats()`, and `take_logs()` after
    /// the outputs have been drained.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn take_flat_patches(&mut self) -> WebFlatPatchBatch {
        let mut outputs = self.inner.take_outputs();
        self.cached_patch_hash = outputs.compute_patch_hash().map(str::to_owned);
        self.cached_patch_stats = outputs.last_patch_stats;
        let flat = outputs.flatten_patches_u32();
        self.cached_logs = outputs.logs;
        flat
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    /// Prepare flat patch data into reusable internal buffers.
    ///
    /// Call this instead of [`take_flat_patches`](Self::take_flat_patches)
    /// when you want to avoid per-frame Vec allocation. Access the results
    /// via [`flat_cells`](Self::flat_cells) and [`flat_spans`](Self::flat_spans).
    pub fn prepare_flat_patches(&mut self) {
        // Flatten into reusable buffers before draining outputs.
        self.inner
            .backend_mut()
            .presenter_mut()
            .flatten_patches_into(&mut self.flat_cells_buf, &mut self.flat_spans_buf);

        // Cache metadata, then drain outputs.
        let outputs = self.inner.take_outputs();
        // Hash stays lazy: compute on-demand from `flat_*_buf` only if asked.
        self.cached_patch_hash = None;
        self.cached_patch_stats = outputs.last_patch_stats;
        self.cached_logs = outputs.logs;
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    /// Flat cell payload from the last [`prepare_flat_patches`](Self::prepare_flat_patches) call.
    pub fn flat_cells(&self) -> &[u32] {
        &self.flat_cells_buf
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    /// Flat span payload from the last [`prepare_flat_patches`](Self::prepare_flat_patches) call.
    pub fn flat_spans(&self) -> &[u32] {
        &self.flat_spans_buf
    }

    /// Take accumulated log lines (from the last `take_flat_patches` call).
    pub fn take_logs(&mut self) -> Vec<String> {
        let mut logs = std::mem::take(&mut self.cached_logs);
        logs.append(&mut self.pane_logs);
        logs
    }

    /// FNV-1a hash of the last patch batch.
    pub fn patch_hash(&mut self) -> Option<String> {
        if self.cached_patch_hash.is_none() {
            if !self.flat_spans_buf.is_empty() {
                self.cached_patch_hash =
                    hash_flat_patch_batch(&self.flat_spans_buf, &self.flat_cells_buf);
            } else {
                let outputs = self.inner.backend_mut().presenter_mut().outputs_mut();
                self.cached_patch_hash = outputs.compute_patch_hash().map(str::to_owned);
            }
        }
        self.cached_patch_hash.clone()
    }

    /// Patch upload stats.
    pub fn patch_stats(&self) -> Option<WebPatchStats> {
        self.cached_patch_stats
            .or(self.inner.outputs().last_patch_stats)
    }

    /// Current frame index (monotonic, 0-based).
    pub fn frame_idx(&self) -> u64 {
        self.inner.frame_idx()
    }

    /// Whether the program is still running.
    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    /// Active pane pointer ID currently tracked by the adapter.
    pub fn pane_active_pointer_id(&self) -> Option<u32> {
        self.pane_adapter.active_pointer_id()
    }

    /// Handle pane pointer-down and emit capture command if needed.
    pub fn pane_pointer_down(
        &mut self,
        target: PaneResizeTarget,
        pointer_id: u32,
        button: PanePointerButton,
        x: i32,
        y: i32,
        modifiers: PaneModifierSnapshot,
    ) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.pointer_down(
            target,
            pointer_id,
            button,
            PanePointerPosition::new(x, y),
            modifiers,
        );
        self.record_pane_dispatch(dispatch)
    }

    /// Mark pane pointer capture as acquired by the host/browser.
    pub fn pane_capture_acquired(&mut self, pointer_id: u32) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.capture_acquired(pointer_id);
        self.record_pane_dispatch(dispatch)
    }

    /// Handle pane pointer-move updates.
    pub fn pane_pointer_move(
        &mut self,
        pointer_id: u32,
        x: i32,
        y: i32,
        modifiers: PaneModifierSnapshot,
    ) -> PaneDispatchSummary {
        let dispatch =
            self.pane_adapter
                .pointer_move(pointer_id, PanePointerPosition::new(x, y), modifiers);
        self.record_pane_dispatch(dispatch)
    }

    /// Handle pane pointer-up and capture release if needed.
    pub fn pane_pointer_up(
        &mut self,
        pointer_id: u32,
        button: PanePointerButton,
        x: i32,
        y: i32,
        modifiers: PaneModifierSnapshot,
    ) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.pointer_up(
            pointer_id,
            button,
            PanePointerPosition::new(x, y),
            modifiers,
        );
        self.record_pane_dispatch(dispatch)
    }

    /// Handle pane pointer-cancel lifecycle.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn pane_pointer_cancel(&mut self, pointer_id: Option<u32>) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.pointer_cancel(pointer_id);
        self.record_pane_dispatch(dispatch)
    }

    /// Handle pane pointer-leave lifecycle.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn pane_pointer_leave(&mut self, pointer_id: u32) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.pointer_leave(pointer_id);
        self.record_pane_dispatch(dispatch)
    }

    /// Handle browser blur for pane interaction lifecycle.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn pane_blur(&mut self) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.blur();
        self.record_pane_dispatch(dispatch)
    }

    /// Handle hidden-tab visibility transition for pane interaction lifecycle.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn pane_visibility_hidden(&mut self) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.visibility_hidden();
        self.record_pane_dispatch(dispatch)
    }

    /// Handle lost-pointer-capture lifecycle signal.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn pane_lost_pointer_capture(&mut self, pointer_id: u32) -> PaneDispatchSummary {
        let dispatch = self.pane_adapter.lost_pointer_capture(pointer_id);
        self.record_pane_dispatch(dispatch)
    }

    fn refresh_cached_patch_meta_from_live_outputs(&mut self) {
        let outputs = self.inner.outputs();
        // Invalidate heavy hash cache on each newly rendered frame. Compute only
        // when explicitly requested by the host.
        self.cached_patch_hash = None;
        self.cached_patch_stats = outputs.last_patch_stats;
        self.flat_cells_buf.clear();
        self.flat_spans_buf.clear();
    }

    fn record_pane_dispatch(&mut self, dispatch: PanePointerDispatch) -> PaneDispatchSummary {
        let summary = PaneDispatchSummary {
            phase: dispatch.log.phase,
            sequence: dispatch.log.sequence,
            pointer_id: dispatch.log.pointer_id,
            target: dispatch.log.target,
            capture_command: dispatch.capture_command,
            outcome: match dispatch.log.outcome {
                PanePointerLogOutcome::SemanticForwarded => PaneDispatchOutcome::SemanticForwarded,
                PanePointerLogOutcome::CaptureStateUpdated => {
                    PaneDispatchOutcome::CaptureStateUpdated
                }
                PanePointerLogOutcome::Ignored(reason) => PaneDispatchOutcome::Ignored(reason),
            },
        };
        self.pane_logs.push(format_pane_log_entry(dispatch.log));
        summary
    }
}

fn format_split_axis(axis: SplitAxis) -> &'static str {
    match axis {
        SplitAxis::Horizontal => "horizontal",
        SplitAxis::Vertical => "vertical",
    }
}

fn format_capture_command(command: Option<PanePointerCaptureCommand>) -> &'static str {
    match command {
        Some(PanePointerCaptureCommand::Acquire { .. }) => "acquire",
        Some(PanePointerCaptureCommand::Release { .. }) => "release",
        None => "none",
    }
}

fn format_ignored_reason(reason: PanePointerIgnoredReason) -> &'static str {
    match reason {
        PanePointerIgnoredReason::InvalidPointerId => "invalid_pointer_id",
        PanePointerIgnoredReason::ButtonNotAllowed => "button_not_allowed",
        PanePointerIgnoredReason::ButtonMismatch => "button_mismatch",
        PanePointerIgnoredReason::ActivePointerAlreadyInProgress => {
            "active_pointer_already_in_progress"
        }
        PanePointerIgnoredReason::NoActivePointer => "no_active_pointer",
        PanePointerIgnoredReason::PointerMismatch => "pointer_mismatch",
        PanePointerIgnoredReason::LeaveWhileCaptured => "leave_while_captured",
        PanePointerIgnoredReason::MachineRejectedEvent => "machine_rejected_event",
    }
}

fn format_pane_log_entry(log: PanePointerLogEntry) -> String {
    let phase = match log.phase {
        PanePointerLifecyclePhase::PointerDown => "pointer_down",
        PanePointerLifecyclePhase::PointerMove => "pointer_move",
        PanePointerLifecyclePhase::PointerUp => "pointer_up",
        PanePointerLifecyclePhase::PointerCancel => "pointer_cancel",
        PanePointerLifecyclePhase::PointerLeave => "pointer_leave",
        PanePointerLifecyclePhase::Blur => "blur",
        PanePointerLifecyclePhase::VisibilityHidden => "visibility_hidden",
        PanePointerLifecyclePhase::LostPointerCapture => "lost_pointer_capture",
        PanePointerLifecyclePhase::CaptureAcquired => "capture_acquired",
    };
    let pointer_id = log
        .pointer_id
        .map_or_else(|| "-".to_owned(), |id| id.to_string());
    let sequence = log
        .sequence
        .map_or_else(|| "-".to_owned(), |seq| seq.to_string());
    let (split_id, axis) = match log.target {
        Some(target) => (
            target.split_id.get().to_string(),
            format_split_axis(target.axis).to_owned(),
        ),
        None => ("-".to_owned(), "-".to_owned()),
    };
    let (x, y) = match log.position {
        Some(pos) => (pos.x.to_string(), pos.y.to_string()),
        None => ("-".to_owned(), "-".to_owned()),
    };
    let outcome = match log.outcome {
        PanePointerLogOutcome::SemanticForwarded => "semantic_forwarded".to_owned(),
        PanePointerLogOutcome::CaptureStateUpdated => "capture_state_updated".to_owned(),
        PanePointerLogOutcome::Ignored(reason) => {
            format!("ignored:{}", format_ignored_reason(reason))
        }
    };
    let command = format_capture_command(log.capture_command);
    format!(
        "pane_pointer phase={phase} seq={sequence} pointer={pointer_id} split={split_id} axis={axis} x={x} y={y} command={command} outcome={outcome}"
    )
}

#[must_use]
fn fnv1a64_extend(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV64_PRIME);
    }
    hash
}

#[must_use]
fn hash_flat_patch_batch(spans: &[u32], cells: &[u32]) -> Option<String> {
    if spans.is_empty() {
        return None;
    }
    if !spans.len().is_multiple_of(2) {
        return None;
    }

    let mut hash = FNV64_OFFSET_BASIS;
    let patch_count = u64::try_from(spans.len() / 2).unwrap_or(u64::MAX);
    hash = fnv1a64_extend(hash, &patch_count.to_le_bytes());

    let mut word_idx = 0usize;
    let mut cell_bytes = [0u8; 16];
    for span in spans.chunks_exact(2) {
        let offset = span[0];
        let len = span[1] as usize;
        let cell_count = u64::try_from(len).unwrap_or(u64::MAX);
        hash = fnv1a64_extend(hash, &offset.to_le_bytes());
        hash = fnv1a64_extend(hash, &cell_count.to_le_bytes());

        let words_needed = len.saturating_mul(4);
        if word_idx.saturating_add(words_needed) > cells.len() {
            return None;
        }

        for _ in 0..len {
            let bg = cells[word_idx];
            let fg = cells[word_idx + 1];
            let glyph = cells[word_idx + 2];
            let attrs = cells[word_idx + 3];
            word_idx += 4;

            cell_bytes[0..4].copy_from_slice(&bg.to_le_bytes());
            cell_bytes[4..8].copy_from_slice(&fg.to_le_bytes());
            cell_bytes[8..12].copy_from_slice(&glyph.to_le_bytes());
            cell_bytes[12..16].copy_from_slice(&attrs.to_le_bytes());
            hash = fnv1a64_extend(hash, &cell_bytes);
        }
    }

    if word_idx != cells.len() {
        return None;
    }

    Some(format!("{PATCH_HASH_ALGO}:{hash:016x}"))
}
