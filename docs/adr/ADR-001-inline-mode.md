# ADR-001: Inline Mode Strategy

## Status

ACCEPTED

## Context

FrankenTUI's core promise is stable UI + streaming logs without flicker, cursor drift, or scrollback corruption. Inline mode is where most TUIs fail in practice because:

- Logs write while UI is mid-draw (multiple writers, interleaving)
- Cursor position gets lost or restored incorrectly
- Full-screen clears destroy scrollback
- Terminal multiplexers (tmux/screen/zellij) differ in behavior for cursor save/restore, scroll regions, and synchronized output

This ADR documents the decision from spike bd-10i.1.1.

## Strategies Evaluated

### Strategy A: Scroll-Region Anchoring (DECSTBM)

Uses `CSI t;b r` to set top/bottom scroll margins:
- Log region scrolls within margins
- UI region stays pinned below

**Pros:**
- Reduces redraw work and cursor movement
- More "native" scrolling behavior when it works

**Cons:**
- Muxes (tmux/screen) may handle margins inconsistently
- Must be carefully reset on exit, panic, and mode switches
- Cursor position relative to region can be confusing

### Strategy B: Overlay Redraw

Before each log write and UI present:
1. Save cursor position (DEC ESC7)
2. Move cursor to target region
3. Write content (logs to log area, UI to UI area)
4. Restore cursor position (DEC ESC8)

**Pros:**
- Works in the widest set of environments (baseline correctness)
- No terminal-specific quirks to handle

**Cons:**
- More redraw work
- Requires explicit region policies

### Strategy C: Hybrid (Selected Default)

- **Overlay redraw** is always available as the correctness baseline
- **Scroll-region** is an internal optimization only where proven safe:
  - Not in any terminal multiplexer
  - Scroll region capability detected
  - Synchronized output available (reduces flicker)

## Decision

**Adopt Hybrid strategy (Strategy C) as the default.**

The public API exposes "inline mode" as a policy concept, not the mechanism (DECSTBM). This prevents terminal quirks from leaking into user code.

### Strategy Selection Logic

```rust
fn select_strategy(caps: &TerminalCapabilities) -> InlineStrategy {
    if caps.in_any_mux() {
        // Muxes may not handle scroll regions correctly
        InlineStrategy::OverlayRedraw
    } else if caps.scroll_region && caps.sync_output {
        // Modern terminal with full support
        InlineStrategy::ScrollRegion
    } else if caps.scroll_region {
        // Scroll region available but no sync output
        InlineStrategy::Hybrid
    } else {
        // Fallback to most portable option
        InlineStrategy::OverlayRedraw
    }
}
```

### Fallback Triggers

Overlay redraw is forced when:
1. `TMUX`, `STY`, or `ZELLIJ` environment variable is set
2. Terminal does not report scroll region support
3. User explicitly requests overlay mode

## Cursor Save/Restore

**Use DEC sequences (ESC7/ESC8), not CSI s/u.**

Rationale from spike investigation:
- DEC sequences are more portable across terminals
- CSI s/u conflict with scroll region operations in some terminals
- Legacy opentui_rust uses DEC sequences successfully

## Cleanup Invariants

On normal exit AND panic, the following must be restored:
1. Scroll region reset to full screen (`CSI r`)
2. Cursor restored (`ESC 8`)
3. Synchronized output ended (`CSI ? 2026 l`)
4. Raw mode exited (handled by RawModeGuard)

Implementation uses RAII `Drop` trait to guarantee cleanup.

## Synchronized Output

Use DEC mode 2026 (`CSI ? 2026 h/l`) when available to:
- Batch terminal updates atomically
- Reduce flicker during UI redraw
- Detection via `TerminalCapabilities::sync_output`

## Consequences

### Positive
- Correctness guaranteed across all terminals via overlay baseline
- Optimized path for modern terminals without muxes
- No terminal quirks exposed in public API
- Cleanup guaranteed even on panic

### Negative
- Overlay mode has more redraw overhead
- Users cannot force scroll-region in muxes (by design)
- Testing requires PTY harness for full validation

## Test Plan

1. **PTY Integration Tests**
   - Cursor restored after each frame present
   - Terminal modes restored on normal exit
   - Terminal modes restored on panic (simulated)
   - No full-screen clears in inline mode

2. **Terminal Compatibility Matrix**
   - Plain terminal (no mux): scroll-region optimization
   - tmux: overlay redraw only
   - screen: overlay redraw only
   - Kitty/WezTerm/Alacritty/Ghostty: scroll-region optimization

3. **Sustained Output Scenario**
   - Continuous log stream + periodic UI redraw + resize events
   - Verify no cursor drift over extended operation

## References

- Spike implementation: `crates/ftui-core/src/inline_mode.rs`
- Terminal capabilities: `crates/ftui-core/src/terminal_capabilities.rs`
- Legacy patterns: `legacy_reference_library_code/opentui_rust/src/terminal/`
