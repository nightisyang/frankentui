//! Scrollback buffer (API skeleton).
//!
//! Scrollback stores lines that have scrolled off the visible viewport.
//! For the crate skeleton, this is a simple capacity-bounded ring.

/// Scrollback line storage (UTF-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scrollback {
    capacity: usize,
    lines: Vec<String>,
}

impl Scrollback {
    /// Create a new scrollback with a line capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            lines: Vec::new(),
        }
    }

    /// Capacity in lines.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current number of stored lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the scrollback is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Push a line into scrollback, evicting the oldest if over capacity.
    pub fn push(&mut self, line: String) {
        if self.capacity == 0 {
            return;
        }
        if self.lines.len() == self.capacity {
            // Skeleton ring: O(n) eviction. Replace with a real ring buffer in
            // the full implementation.
            self.lines.remove(0);
        }
        self.lines.push(line);
    }

    /// Iterate over stored lines (oldest to newest).
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_zero_drops_lines() {
        let mut sb = Scrollback::new(0);
        sb.push("x".to_string());
        assert_eq!(sb.len(), 0);
    }

    #[test]
    fn bounded_capacity_evicts_oldest() {
        let mut sb = Scrollback::new(2);
        sb.push("a".to_string());
        sb.push("b".to_string());
        sb.push("c".to_string());
        let collected = sb.iter().collect::<Vec<_>>();
        assert_eq!(collected, vec!["b", "c"]);
    }
}
