#![forbid(unsafe_code)]

//! JSONL evidence sink for deterministic diagnostics.
//!
//! This provides a shared, line-oriented sink that can be wired into runtime
//! policies (diff/resize/budget) to emit JSONL evidence to a single destination.
//! Ordering is deterministic with respect to call order because writes are
//! serialized behind a mutex, and flush behavior is explicit and configurable.

use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Schema version for JSONL evidence lines.
pub const EVIDENCE_SCHEMA_VERSION: &str = "ftui-evidence-v1";

/// Destination for evidence JSONL output.
#[derive(Debug, Clone)]
pub enum EvidenceSinkDestination {
    /// Write to stdout.
    Stdout,
    /// Append to a file at the given path.
    File(PathBuf),
}

impl EvidenceSinkDestination {
    /// Convenience helper for file destinations.
    #[must_use]
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }
}

/// Configuration for evidence logging.
#[derive(Debug, Clone)]
pub struct EvidenceSinkConfig {
    /// Whether evidence logging is enabled.
    pub enabled: bool,
    /// Output destination for JSONL lines.
    pub destination: EvidenceSinkDestination,
    /// Flush after every line (recommended for tests/e2e capture).
    pub flush_on_write: bool,
}

impl Default for EvidenceSinkConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            destination: EvidenceSinkDestination::Stdout,
            flush_on_write: true,
        }
    }
}

impl EvidenceSinkConfig {
    /// Create a disabled sink config.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Enable logging to stdout with flush-on-write.
    #[must_use]
    pub fn enabled_stdout() -> Self {
        Self {
            enabled: true,
            destination: EvidenceSinkDestination::Stdout,
            flush_on_write: true,
        }
    }

    /// Enable logging to a file with flush-on-write.
    #[must_use]
    pub fn enabled_file(path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            destination: EvidenceSinkDestination::file(path),
            flush_on_write: true,
        }
    }

    /// Set whether logging is enabled.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the destination for evidence output.
    #[must_use]
    pub fn with_destination(mut self, destination: EvidenceSinkDestination) -> Self {
        self.destination = destination;
        self
    }

    /// Set flush-on-write behavior.
    #[must_use]
    pub fn with_flush_on_write(mut self, enabled: bool) -> Self {
        self.flush_on_write = enabled;
        self
    }
}

struct EvidenceSinkInner {
    writer: BufWriter<Box<dyn Write + Send>>,
    flush_on_write: bool,
}

/// Shared, line-oriented JSONL sink for evidence logging.
#[derive(Clone)]
pub struct EvidenceSink {
    inner: Arc<Mutex<EvidenceSinkInner>>,
}

impl std::fmt::Debug for EvidenceSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceSink").finish()
    }
}

impl EvidenceSink {
    /// Build an evidence sink from config. Returns `Ok(None)` when disabled.
    pub fn from_config(config: &EvidenceSinkConfig) -> io::Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }

        let writer: Box<dyn Write + Send> = match &config.destination {
            EvidenceSinkDestination::Stdout => Box::new(io::stdout()),
            EvidenceSinkDestination::File(path) => {
                let file = OpenOptions::new().create(true).append(true).open(path)?;
                Box::new(file)
            }
        };

        let inner = EvidenceSinkInner {
            writer: BufWriter::new(writer),
            flush_on_write: config.flush_on_write,
        };

        Ok(Some(Self {
            inner: Arc::new(Mutex::new(inner)),
        }))
    }

    /// Write a single JSONL line with newline and optional flush.
    pub fn write_jsonl(&self, line: &str) -> io::Result<()> {
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.writer.write_all(line.as_bytes())?;
        inner.writer.write_all(b"\n")?;
        if inner.flush_on_write {
            inner.writer.flush()?;
        }
        Ok(())
    }

    /// Flush any buffered output.
    pub fn flush(&self) -> io::Result<()> {
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_stable() {
        assert_eq!(EVIDENCE_SCHEMA_VERSION, "ftui-evidence-v1");
    }

    #[test]
    fn config_default_is_disabled() {
        let config = EvidenceSinkConfig::default();
        assert!(!config.enabled);
        assert!(config.flush_on_write);
        assert!(matches!(
            config.destination,
            EvidenceSinkDestination::Stdout
        ));
    }

    #[test]
    fn config_disabled_matches_default() {
        let config = EvidenceSinkConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn config_enabled_stdout() {
        let config = EvidenceSinkConfig::enabled_stdout();
        assert!(config.enabled);
        assert!(config.flush_on_write);
        assert!(matches!(
            config.destination,
            EvidenceSinkDestination::Stdout
        ));
    }

    #[test]
    fn config_enabled_file() {
        let config = EvidenceSinkConfig::enabled_file("/tmp/test.jsonl");
        assert!(config.enabled);
        assert!(config.flush_on_write);
        assert!(matches!(
            config.destination,
            EvidenceSinkDestination::File(_)
        ));
    }

    #[test]
    fn config_builder_chain() {
        let config = EvidenceSinkConfig::default()
            .with_enabled(true)
            .with_destination(EvidenceSinkDestination::Stdout)
            .with_flush_on_write(false);
        assert!(config.enabled);
        assert!(!config.flush_on_write);
    }

    #[test]
    fn destination_file_helper() {
        let dest = EvidenceSinkDestination::file("/tmp/evidence.jsonl");
        assert!(
            matches!(dest, EvidenceSinkDestination::File(p) if p.to_str() == Some("/tmp/evidence.jsonl"))
        );
    }

    #[test]
    fn disabled_config_returns_none() {
        let config = EvidenceSinkConfig::disabled();
        let sink = EvidenceSink::from_config(&config).unwrap();
        assert!(sink.is_none());
    }

    #[test]
    fn enabled_file_sink_writes_jsonl() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let config = EvidenceSinkConfig::enabled_file(&path);
        let sink = EvidenceSink::from_config(&config).unwrap().unwrap();

        sink.write_jsonl(r#"{"event":"test","value":1}"#).unwrap();
        sink.write_jsonl(r#"{"event":"test","value":2}"#).unwrap();
        sink.flush().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"event":"test","value":1}"#);
        assert_eq!(lines[1], r#"{"event":"test","value":2}"#);
    }

    #[test]
    fn sink_is_clone_and_shared() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let config = EvidenceSinkConfig::enabled_file(&path);
        let sink = EvidenceSink::from_config(&config).unwrap().unwrap();
        let sink2 = sink.clone();

        sink.write_jsonl(r#"{"from":"sink1"}"#).unwrap();
        sink2.write_jsonl(r#"{"from":"sink2"}"#).unwrap();
        sink.flush().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn sink_debug_impl() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let config = EvidenceSinkConfig::enabled_file(tmp.path());
        let sink = EvidenceSink::from_config(&config).unwrap().unwrap();
        let debug = format!("{:?}", sink);
        assert!(debug.contains("EvidenceSink"));
    }
}
