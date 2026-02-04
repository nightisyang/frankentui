# Unit Test Coverage Matrix

This document encodes the project's expectations for unit test coverage by crate and module.
It prevents "test later" drift, keeps kernel invariants continuously verified, and makes CI
decisions explicit.

See Bead: bd-2nu8.17.

## How to Use
- When adding a new module, add it here.
- When adding a new public API, add explicit unit tests here.
- CI enforces these thresholds via the coverage gate.

## Coverage Targets (v1)
- Overall workspace: >= 70% (CI gate)
- ftui-render: >= 80% (kernel)
- ftui-core: >= 80% (terminal/session + input)
- ftui-style: >= 80%
- ftui-text: >= 80%
- ftui-layout: >= 75%
- ftui-runtime: >= 75%
- ftui-widgets: >= 70%
- ftui-extras: >= 60% (feature-gated)

Non-gated crates (report-only, no threshold): ftui, ftui-harness, ftui-demo-showcase, ftui-pty, ftui-simd.

Note: Integration-heavy PTY tests are enforced separately; do not "unit test" around reality.

## Last Measured: 2026-02-02 (cargo llvm-cov, 2630 tests)
Note: 2026-02-04 attempt hung in `ftui-demo-showcase` a11y snapshot tests; no new metrics produced.

| Crate | Target | Actual | Status |
|-------|--------|--------|--------|
| ftui-render | >= 80% | 95.06% | PASS |
| ftui-core | >= 80% | 94.86% | PASS |
| ftui-style | >= 80% | 98.53% | PASS |
| ftui-text | >= 80% | 94.44% | PASS |
| ftui-layout | >= 75% | 97.39% | PASS |
| ftui-runtime | >= 75% | 84.03% | PASS (program.rs at 34% drags average) |
| ftui-widgets | >= 70% | 90.04% | PASS |
| ftui-extras | >= 60% | 89.66% | PASS |

## ftui-render (>= 85%) — Actual: ~95%
Kernel correctness lives here.

### Cell / CellContent / CellAttrs — 93% lines
- [x] CellContent creation from char vs grapheme-id
- [x] Width semantics (ASCII, wide, combining, emoji)
- [x] Continuation-cell sentinel semantics for wide glyphs
- [x] PackedRgba: construction + Porter-Duff alpha blending
- [x] CellAttrs: bitflags operations + merge/override
- [x] 16-byte Cell layout invariants (size/alignment) + bits_eq correctness

### Buffer — 96% lines
- [x] Create/resize buffer with dimensions
- [x] get/set bounds checking + deterministic defaults
- [x] Clear semantics (full vs region)
- [x] Scissor stack push/pop semantics (intersection monotonicity)
- [x] Opacity stack push/pop semantics (product in [0,1])
- [x] Wide glyph placement + continuation cells
- [x] Iteration order and row-major storage assumptions

### Diff — 100% lines
- [x] Empty diff (no changes)
- [x] Single cell change
- [x] Row changes
- [x] Run grouping behavior
- [x] Scratch buffer reuse (no unbounded allocations)

### Presenter — 93% lines
- [x] Cursor tracking correctness
- [x] Style tracking correctness
- [x] Link tracking correctness (OSC 8 open/close)
- [x] Single-write-per-frame behavior
- [x] Synchronized output behavior where supported (fallback correctness)

### Other Modules
- ansi.rs: 97% lines
- budget.rs: 98% lines
- counting_writer.rs: 98% lines
- drawing.rs: 99% lines
- frame.rs: 88% lines
- grapheme_pool.rs: 99% lines
- headless.rs: 99% lines (test infrastructure)
- link_registry.rs: 99% lines
- sanitize.rs: 98% lines
- terminal_model.rs: 88% lines (test infrastructure)

## ftui-core (>= 80%) — Actual: ~95%

### Event types — 93% lines
- [x] Canonical key/mouse/resize/paste/focus event types are stable

### InputParser — 94% lines
- [x] Bounded CSI/OSC/DCS parsing (DoS limits)
- [x] Bracketed paste decoding + max size
- [x] Mouse SGR decoding
- [x] Focus/resize event decoding

### TerminalCapabilities — 100% lines
- [x] Env heuristic detection (TERM/COLORTERM)
- [x] Mux flags (tmux/screen/zellij) correctness

### TerminalSession lifecycle — 93% lines
- [x] RAII enter/exit discipline
- [ ] Panic cleanup paths are idempotent — partial coverage via PTY tests; needs dedicated unit test

### Other Modules
- animation.rs: 96% lines
- cursor.rs: 98% lines
- event_coalescer.rs: 96% lines
- geometry.rs: 100% lines
- inline_mode.rs: 92% lines
- logging.rs: 100% lines
- mux_passthrough.rs: 100% lines

## ftui-style (>= 80%) — Actual: ~99%
- [x] Style defaults + builder ergonomics — 96% (style.rs)
- [x] Deterministic style merge (explicit masks) — 96% (style.rs)
- [x] Color downgrade (truecolor -> 256 -> 16 -> mono) — 100% (color.rs)
- [x] Theme presets + semantic slots — 99% (theme.rs)
- [x] StyleSheet registry + named style composition — 99% (stylesheet.rs)

## ftui-text (>= 80%) — Actual: ~94%
- [x] Segment system correctness (Cow<str>) — 90% (segment.rs)
- [x] Width measurement correctness + LRU cache behavior — 99% (width_cache.rs)
- [x] Grapheme segmentation helpers for wrap/truncate correctness — 99% (wrap.rs)
- [x] Wrap/truncate semantics for ZWJ/emoji/combining — 99% (wrap.rs + unicode corpus)
- [x] Markup parser correctness (feature-gated) — 89% (markup.rs)

### Other Modules
- cursor.rs: 100% lines
- editor.rs: 96% lines
- lib.rs: 100% lines
- rope.rs: 98% lines
- search.rs: 98% lines
- text.rs: 83% lines (gap: Display impls, From conversions)
- view.rs: 97% lines

## ftui-layout (>= 75%) — Actual: ~97%
- [x] Rect operations (intersection/contains) — 100% (geometry.rs in ftui-core)
- [x] Flex constraint solving + gaps — 99% (lib.rs)
- [x] Grid placement + spanning + named areas — 99% (grid.rs)
- [x] Min/max sizing invariants — 99% (lib.rs)

## ftui-runtime (>= 75%) — Actual: ~84%
- [x] Deterministic scheduling (update/view loop) — 95% (simulator.rs)
- [ ] Cmd sequencing + cancellation — **34% (program.rs) CRITICAL GAP**
- [x] Subscription polling correctness — 99% (subscription.rs)
- [x] Simulator determinism (headless) — 95% (simulator.rs)

### Other Modules
- asciicast.rs: 93% lines
- input_macro.rs: 96% lines
- log_sink.rs: 99% lines
- string_model.rs: 84% lines
- terminal_writer.rs: 89% lines

### Critical Gap: program.rs (34% coverage)
The core `Program` runtime loop has only 34% line coverage. The `run()` method requires
a real terminal for event polling. Coverage should be improved via `ProgramSimulator` tests
for Cmd handling, AppBuilder, and model lifecycle. Terminal I/O paths belong in PTY tests.

## ftui-widgets (>= 70%) — Actual: ~90%
- [x] Harness-essential widgets have snapshot tests (renderable_snapshots.rs: 59+ tests)
- [x] Widgets: key unit tests (render + layout invariants) (frame_integration.rs + per-module)

### Per-Widget Coverage
| Widget | Coverage | Notes |
|--------|----------|-------|
| borders.rs | 100% | |
| rule.rs | 100% | |
| padding.rs | 99% | |
| cached.rs | 98% | |
| log_ring.rs | 98% | |
| group.rs | 98% | |
| spinner.rs | 98% | |
| paginator.rs | 98% | |
| progress.rs | 97% | |
| table.rs | 97% | |
| status_line.rs | 96% | |
| panel.rs | 96% | |
| pretty.rs | 96% | |
| columns.rs | 94% | |
| timer.rs | 95% | |
| error_boundary.rs | 94% | |
| paragraph.rs | 93% | |
| stopwatch.rs | 93% | |
| layout.rs | 93% | |
| textarea.rs | 93% | |
| list.rs | 93% | |
| tree.rs | 92% | |
| help.rs | 92% | |
| align.rs | 91% | |
| emoji.rs | 90% | |
| json_view.rs | 86% | |
| input.rs | 85% | Gap: multi-codepoint, clipboard, cursor boundaries |
| log_viewer.rs | 83% | Gap: large scrollback, markup in logs |
| virtualized.rs | 81% | Gap: scroll acceleration, dynamic height |
| file_picker.rs | 81% | Gap: filesystem edge cases |
| block.rs | 79% | Gap: complex borders, multi-title, degraded mode |

## ftui-extras (>= 60%) — Actual: ~90%
- [x] Feature-gated modules include correctness tests (measured with `--all-features`)
- [ ] `image.rs` remains low coverage (46.46%) — add decode/format/error-path tests
- [ ] `pty_capture.rs` at 74.71% — add PTY integration scenarios for partial reads
