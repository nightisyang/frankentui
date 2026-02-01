# ADR-003: Terminal Backend Selection

Status: ACCEPTED

## Context

FrankenTUI needs a terminal backend that provides:
- Raw mode lifecycle management (enter/exit)
- Event reading (keyboard, mouse, resize, paste, focus)
- Terminal capability detection
- Cross-platform support (Linux, macOS, Windows best-effort)

The backend choice is foundational and extremely hard to change later. It affects:
- Correctness of terminal cleanup (on normal exit AND panic)
- Platform reach and feature availability
- Maintenance burden and dependency complexity

## Decision

**Crossterm is the v1 terminal backend.**

## Evaluation Matrix

### Candidates Considered

| Backend | Unix | Windows | Async | Maintenance | Notes |
|---------|------|---------|-------|-------------|-------|
| **Crossterm** | ✅ | ✅ | Optional | Active | Selected |
| termwiz | ✅ | ⚠️ | No | Active | More complex API |
| termion | ✅ | ❌ | No | Low | Unix-only, simpler |
| custom termios | ✅ | ❌ | N/A | High | Maximum control, maximum burden |

### Crossterm Functionality Validated (Spike bd-10i.1.3)

| Feature | API | Status |
|---------|-----|--------|
| Raw mode | `enable_raw_mode()` / `disable_raw_mode()` | ✅ Works |
| Alternate screen | `EnterAlternateScreen` / `LeaveAlternateScreen` | ✅ Works |
| Cursor show/hide | `Show` / `Hide` | ✅ Works |
| Mouse (SGR) | `EnableMouseCapture` / `DisableMouseCapture` | ✅ Works |
| Bracketed paste | `EnableBracketedPaste` / `DisableBracketedPaste` | ✅ Works |
| Focus events | `EnableFocusChange` / `DisableFocusChange` | ✅ Works |
| Resize events | `Event::Resize(cols, rows)` | ✅ Works |
| Bounded reads | `poll(timeout)` | ✅ Works |

### Cleanup Discipline

Crossterm's stateless design requires explicit cleanup, which we handle via `Drop`:

```rust
impl Drop for TerminalSession {
    fn drop(&mut self) {
        // Disable features in reverse order
        // Show cursor
        // Exit raw mode
        // Flush stdout
    }
}
```

This guarantees cleanup on:
- Normal function return
- Early return via `?` operator
- Panic unwinding (default in debug, opt-in in release)

### Platform Coverage

| Platform | Raw Mode | Events | Mouse | Paste | Focus |
|----------|----------|--------|-------|-------|-------|
| Linux | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS | ✅ | ✅ | ✅ | ✅ | ✅ |
| Windows | ✅ | ✅ | ⚠️ | ⚠️ | ⚠️ |

Windows limitations documented in ADR-004.

## Alternatives Considered

### termwiz
- More comprehensive API but more complex
- Stronger opinions about rendering model
- Would constrain our architecture choices

### termion
- Simpler, lighter weight
- Unix-only (no Windows support)
- Less active maintenance

### Custom termios
- Maximum control
- Massive maintenance burden
- Would duplicate existing battle-tested code

## Consequences

### Positive
- Cross-platform support out of the box
- Active maintenance and community
- Familiar API for Rust TUI ecosystem
- Handles edge cases we'd otherwise miss

### Negative
- External dependency (can't inline fix bugs)
- Some Windows features are best-effort
- May need to work around crossterm opinions

### Mitigation
- Abstract crossterm behind our own types (`TerminalSession`, `Event`)
- Document Windows limitations clearly (ADR-004)
- If critical bugs found, can vendor fork as last resort

## Test Plan

- PTY tests validate cleanup on normal exit and panic
- CI includes Linux, macOS, and Windows builds
- Integration tests exercise resize and input events
- Manual testing on common terminals (see compatibility matrix)

## Implementation

See `crates/ftui-core/src/terminal_session.rs` for the `TerminalSession` wrapper
that provides the one-writer-rule-compliant API with guaranteed cleanup.
