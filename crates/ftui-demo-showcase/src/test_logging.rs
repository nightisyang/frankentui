#![forbid(unsafe_code)]

//! Shared JSONL logging helpers for tests.

use std::sync::atomic::{AtomicU64, Ordering};

/// Schema version for test JSONL logs.
pub const TEST_JSONL_SCHEMA: &str = "test-jsonl-v1";

/// Returns true if JSONL logging should be emitted.
#[must_use]
pub fn jsonl_enabled() -> bool {
    std::env::var("E2E_JSONL").is_ok() || std::env::var("CI").is_ok()
}

/// Escape a string for JSON output (minimal string escaping).
#[must_use]
pub fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

/// JSONL logger with stable run context + per-entry sequence numbering.
pub struct JsonlLogger {
    run_id: String,
    seed: Option<u64>,
    context: Vec<(String, String)>,
    seq: AtomicU64,
}

impl JsonlLogger {
    /// Create a new JSONL logger with a run identifier.
    #[must_use]
    pub fn new(run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            seed: None,
            context: Vec::new(),
            seq: AtomicU64::new(0),
        }
    }

    /// Attach a deterministic seed field to all log entries.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Add a context field to all log entries.
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    /// Emit a JSONL line if logging is enabled.
    pub fn log(&self, event: &str, fields: &[(&str, &str)]) {
        if !jsonl_enabled() {
            return;
        }

        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let mut parts = Vec::with_capacity(6 + self.context.len() + fields.len());
        parts.push(format!("\"schema_version\":\"{}\"", TEST_JSONL_SCHEMA));
        parts.push(format!("\"run_id\":\"{}\"", escape_json(&self.run_id)));
        parts.push(format!("\"seq\":{seq}"));
        parts.push(format!("\"event\":\"{}\"", escape_json(event)));
        if let Some(seed) = self.seed {
            parts.push(format!("\"seed\":{seed}"));
        }
        for (key, value) in &self.context {
            parts.push(format!("\"{}\":\"{}\"", key, escape_json(value)));
        }
        for (key, value) in fields {
            parts.push(format!("\"{}\":\"{}\"", key, escape_json(value)));
        }

        eprintln!("{{{}}}", parts.join(","));
    }
}
