# Coverage Gap Report

Generated: 2026-02-02
Tool: `cargo llvm-cov` (workspace, all targets)
Total tests: 2,630 passing
Overall line coverage: **91.66%**

## Per-Crate Coverage Summary

| Crate | Target | Actual (lines) | Status | Key Gaps |
|-------|--------|----------------|--------|----------|
| ftui-render | >= 85% | ~96% | PASS | frame.rs (88%), terminal_model.rs (88%) |
| ftui-core | >= 80% | ~96% | PASS | terminal_session.rs (93%), event.rs (93%) |
| ftui-style | >= 80% | ~98% | PASS | style.rs (96%) - minor |
| ftui-text | >= 80% | ~95% | PASS | text.rs (83%), markup.rs (89%) |
| ftui-layout | >= 75% | ~97% | PASS | debug.rs (95%) - minor |
| ftui-runtime | >= 75% | ~83% | PASS | **program.rs (34%)** - critical gap |
| ftui-widgets | >= 70% | ~92% | PASS | block.rs (79%), virtualized.rs (81%) |
| ftui-extras | >= 60% | N/A | SKIP | Feature-gated; not measured in this run |

All crates exceed their targets at the aggregate level.

## Critical Gaps (individual modules below crate target)

### 1. ftui-runtime/src/program.rs — 34.05% lines (Target: 75%)

**Severity: CRITICAL**

The core `Program` runtime loop has only 34% line coverage. This is the Elm/Bubbletea runtime
that orchestrates update/view cycles. Most of the uncovered code is the actual `run()` method
which requires a real terminal (crossterm event polling, raw mode, etc.).

Coverage gap areas:
- `Program::run()` - main event loop (requires terminal I/O)
- `Program::run_with_config()` - configured variant
- `AppBuilder` methods - builder pattern for app configuration
- Signal handling (Unix-specific paths)
- Error recovery paths

**Recommendation:** The `ProgramSimulator` already exists for headless testing. Write unit tests
that exercise `Model::update()` / `Model::view()` via the simulator. The terminal I/O paths
(`run()`) are appropriately tested via PTY integration tests instead.

### 2. ftui-widgets/src/block.rs — 78.70% lines (Target: 70%)

Above widget target but below 85%. Uncovered paths include:
- Complex border configurations
- Multi-title rendering edge cases
- Degraded-mode border fallbacks

### 3. ftui-widgets/src/virtualized.rs — 81.01% lines (Target: 70%)

Above widget target. Uncovered paths:
- Scroll acceleration edge cases
- Dynamic height measurement fallbacks
- Virtual viewport boundary conditions

### 4. ftui-widgets/src/input.rs — 84.93% lines (Target: 70%)

Above widget target. Uncovered paths:
- Multi-codepoint input handling edge cases
- Clipboard paste integration
- Cursor movement at boundary conditions

### 5. ftui-widgets/src/log_viewer.rs — 82.72% lines (Target: 70%)

Above widget target. Uncovered paths:
- Large log scrollback behavior
- Markup parsing in log lines
- Auto-scroll toggle edge cases

### 6. ftui-text/src/text.rs — 83.01% lines (Target: 80%)

Just above target. Uncovered paths:
- Some `Display` trait implementations
- Several `From` conversions
- Edge cases in `Line` alignment

### 7. ftui-render/src/frame.rs — 88.34% lines (Target: 85%)

Above target. Uncovered paths:
- Hit grid boundary conditions
- Some cursor save/restore sequences
- Nested scissor interactions

### 8. ftui-render/src/terminal_model.rs — 88.11% lines (Target: 85%)

Above target. Uncovered paths:
- Some rare ANSI escape sequences
- Tab stop handling edge cases
- Scroll region boundary conditions

## Module-Level Coverage Detail

### ftui-core (Aggregate: ~96%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| geometry.rs | 310 | 0 | 100.00% |
| logging.rs | 37 | 0 | 100.00% |
| terminal_capabilities.rs | 471 | 0 | 100.00% |
| mux_passthrough.rs | 107 | 0 | 100.00% |
| cursor.rs | 159 | 3 | 98.11% |
| event_coalescer.rs | 364 | 13 | 96.43% |
| animation.rs | 513 | 22 | 95.71% |
| input_parser.rs | 885 | 50 | 94.35% |
| terminal_session.rs | 555 | 38 | 93.15% |
| event.rs | 426 | 31 | 92.72% |
| inline_mode.rs | 268 | 21 | 92.16% |

### ftui-render (Aggregate: ~96%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| diff.rs | 237 | 0 | 100.00% |
| drawing.rs | 438 | 3 | 99.32% |
| link_registry.rs | 215 | 2 | 99.07% |
| grapheme_pool.rs | 317 | 3 | 99.05% |
| headless.rs | 515 | 5 | 99.03% |
| budget.rs | 449 | 7 | 98.44% |
| sanitize.rs | 837 | 17 | 97.97% |
| counting_writer.rs | 220 | 5 | 97.73% |
| ansi.rs | 416 | 13 | 96.88% |
| buffer.rs | 485 | 20 | 95.88% |
| cell.rs | 506 | 34 | 93.28% |
| presenter.rs | 425 | 29 | 93.18% |
| frame.rs | 523 | 61 | 88.34% |
| terminal_model.rs | 875 | 104 | 88.11% |

### ftui-style (Aggregate: ~98%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| color.rs | 381 | 1 | 99.74% |
| theme.rs | 611 | 6 | 99.02% |
| stylesheet.rs | 275 | 3 | 98.91% |
| style.rs | 357 | 16 | 95.52% |

### ftui-text (Aggregate: ~95%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| cursor.rs | 588 | 0 | 100.00% |
| lib.rs | 149 | 0 | 100.00% |
| width_cache.rs | 267 | 3 | 98.88% |
| wrap.rs | 487 | 7 | 98.56% |
| rope.rs | 414 | 7 | 98.31% |
| search.rs | 146 | 3 | 97.95% |
| view.rs | 432 | 11 | 97.45% |
| editor.rs | 723 | 26 | 96.40% |
| segment.rs | 545 | 54 | 90.09% |
| markup.rs | 485 | 54 | 88.87% |
| text.rs | 559 | 95 | 83.01% |

### ftui-layout (Aggregate: ~97%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| lib.rs | 426 | 3 | 99.30% |
| grid.rs | 441 | 6 | 98.64% |
| debug.rs | 476 | 26 | 94.54% |

### ftui-runtime (Aggregate: ~83%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| subscription.rs | 241 | 2 | 99.17% |
| log_sink.rs | 152 | 1 | 99.34% |
| input_macro.rs | 781 | 31 | 96.03% |
| simulator.rs | 328 | 16 | 95.12% |
| asciicast.rs | 267 | 19 | 92.88% |
| terminal_writer.rs | 960 | 106 | 88.96% |
| string_model.rs | 209 | 33 | 84.21% |
| program.rs | 602 | 397 | **34.05%** |

### ftui-widgets (Aggregate: ~92%)

| Module | Lines | Missed | Coverage |
|--------|-------|--------|----------|
| borders.rs | 208 | 0 | 100.00% |
| rule.rs | 360 | 1 | 99.72% |
| padding.rs | 242 | 3 | 98.76% |
| cached.rs | 329 | 6 | 98.18% |
| constraint_overlay.rs | 275 | 5 | 98.18% |
| log_ring.rs | 283 | 6 | 97.88% |
| group.rs | 180 | 4 | 97.78% |
| spinner.rs | 251 | 6 | 97.61% |
| paginator.rs | 245 | 6 | 97.55% |
| progress.rs | 224 | 6 | 97.32% |
| table.rs | 446 | 14 | 96.86% |
| status_line.rs | 346 | 13 | 96.24% |
| panel.rs | 519 | 21 | 95.95% |
| pretty.rs | 121 | 5 | 95.87% |
| columns.rs | 236 | 13 | 94.49% |
| timer.rs | 256 | 14 | 94.53% |
| lib.rs | 224 | 14 | 93.75% |
| error_boundary.rs | 500 | 31 | 93.80% |
| paragraph.rs | 329 | 22 | 93.31% |
| stopwatch.rs | 351 | 24 | 93.16% |
| layout.rs | 310 | 21 | 93.23% |
| textarea.rs | 704 | 52 | 92.61% |
| list.rs | 310 | 22 | 92.90% |
| tree.rs | 341 | 29 | 91.50% |
| help.rs | 313 | 26 | 91.69% |
| layout_debugger.rs | 181 | 16 | 91.16% |
| align.rs | 230 | 20 | 91.30% |
| emoji.rs | 117 | 12 | 89.74% |
| json_view.rs | 361 | 49 | 86.43% |
| input.rs | 743 | 112 | 84.93% |
| log_viewer.rs | 492 | 85 | 82.72% |
| virtualized.rs | 790 | 150 | 81.01% |
| file_picker.rs | 374 | 72 | 80.75% |
| block.rs | 460 | 98 | 78.70% |

## Coverage Matrix Checklist Audit

Cross-referencing coverage-matrix.md checklist items against actual test existence:

### ftui-render

| Item | Has Tests | Coverage |
|------|-----------|----------|
| CellContent creation (char vs grapheme-id) | Yes (cell.rs tests) | 93% |
| Width semantics (ASCII, wide, combining, emoji) | Yes (cell.rs + wrap.rs tests) | 93% |
| Continuation-cell sentinel semantics | Yes (buffer.rs wide glyph tests) | 96% |
| PackedRgba construction + alpha blending | Yes (cell.rs tests) | 93% |
| CellAttrs bitflags operations | Yes (cell.rs tests) | 93% |
| 16-byte Cell layout invariants | Yes (cell.rs layout test) | 93% |
| Buffer create/resize | Yes (buffer.rs tests) | 96% |
| Buffer get/set bounds checking | Yes (buffer.rs tests) | 96% |
| Clear semantics | Yes (buffer.rs tests) | 96% |
| Scissor stack | Yes (buffer.rs tests) | 96% |
| Opacity stack | Yes (buffer.rs tests) | 96% |
| Wide glyph placement | Yes (wide_char tests) | 96% |
| Empty diff | Yes (diff.rs tests) | 100% |
| Single cell change | Yes (diff.rs tests) | 100% |
| Row changes | Yes (diff.rs tests) | 100% |
| Run grouping | Yes (diff.rs tests) | 100% |
| Presenter cursor tracking | Yes (presenter.rs tests) | 93% |
| Presenter style tracking | Yes (presenter.rs tests) | 93% |
| Presenter link tracking | Yes (headless tests) | 99% |

### ftui-core

| Item | Has Tests | Coverage |
|------|-----------|----------|
| Event types stable | Yes (event.rs tests) | 93% |
| InputParser bounded CSI/OSC/DCS | Yes (input_parser.rs tests) | 94% |
| Bracketed paste decoding | Yes (input_parser.rs tests) | 94% |
| Mouse SGR decoding | Yes (input_parser.rs tests) | 94% |
| Focus/resize decoding | Yes (input_parser.rs tests) | 94% |
| Terminal capabilities env heuristic | Yes (terminal_capabilities.rs tests) | 100% |
| Mux flags correctness | Yes (mux_passthrough.rs tests) | 100% |
| RAII enter/exit | Yes (terminal_session.rs tests) | 93% |
| Panic cleanup idempotent | Partial (PTY tests) | 93% |

### ftui-style

| Item | Has Tests | Coverage |
|------|-----------|----------|
| Style defaults + builder | Yes | 96% |
| Deterministic style merge | Yes | 96% |
| Color downgrade | Yes (color.rs tests) | 100% |
| Theme presets + semantic slots | Yes (theme.rs tests) | 99% |
| StyleSheet registry | Yes (stylesheet.rs tests) | 99% |

### ftui-text

| Item | Has Tests | Coverage |
|------|-----------|----------|
| Segment system correctness | Partial (segment.rs) | 90% |
| Width measurement + LRU cache | Yes (width_cache.rs) | 99% |
| Grapheme segmentation helpers | Yes (wrap.rs tests) | 99% |
| Wrap/truncate ZWJ/emoji/combining | Yes (wrap.rs + unicode corpus) | 99% |
| Markup parser correctness | Yes (markup.rs tests) | 89% |

### ftui-layout

| Item | Has Tests | Coverage |
|------|-----------|----------|
| Rect operations | Yes (geometry.rs in ftui-core) | 100% |
| Flex constraint solving + gaps | Yes (lib.rs tests) | 99% |
| Grid placement + spanning | Yes (grid.rs tests) | 99% |
| Min/max sizing invariants | Yes (lib.rs tests) | 99% |

### ftui-runtime

| Item | Has Tests | Coverage |
|------|-----------|----------|
| Deterministic scheduling | Partial (simulator.rs) | 95% |
| Cmd sequencing + cancellation | LOW (program.rs) | 34% |
| Subscription polling | Yes (subscription.rs) | 99% |
| Simulator determinism | Yes (simulator.rs) | 95% |

### ftui-widgets

| Item | Has Tests | Coverage |
|------|-----------|----------|
| Snapshot tests for essential widgets | Yes (renderable_snapshots.rs) | Multiple |
| Key unit tests per widget | Yes (per-module #[cfg(test)]) | Multiple |

## Prioritized Gap Items for New Unit Tests

### Priority 1 (Critical - blocking downstream)

1. **ftui-runtime/program.rs** — Write simulator-based tests for `Cmd` sequencing,
   `Cmd::Batch`, `Cmd::Sequence`, `Cmd::Task`. Test `AppBuilder` configuration.
   Target: Raise from 34% to >=60%.

### Priority 2 (High impact)

2. **ftui-widgets/block.rs** — Add tests for complex border configurations,
   multi-title rendering, degraded mode. Target: 79% -> 85%.
3. **ftui-widgets/virtualized.rs** — Add tests for scroll acceleration,
   dynamic height, viewport boundaries. Target: 81% -> 85%.
4. **ftui-widgets/input.rs** — Add tests for multi-codepoint input,
   clipboard paste, cursor boundaries. Target: 85% -> 90%.

### Priority 3 (Moderate)

5. **ftui-text/text.rs** — Add tests for Display impls, From conversions,
   Line alignment. Target: 83% -> 90%.
6. **ftui-text/segment.rs** — Add tests for styled segment splitting.
   Target: 90% -> 95%.
7. **ftui-widgets/log_viewer.rs** — Add tests for large scrollback,
   markup in log lines. Target: 83% -> 88%.
8. **ftui-render/terminal_model.rs** — Add tests for rare ANSI sequences,
   tab stops, scroll regions. Target: 88% -> 92%.

### Priority 4 (Polish)

9. **ftui-runtime/terminal_writer.rs** — Add tests for inline mode edge cases.
   Target: 89% -> 92%.
10. **ftui-text/markup.rs** — Add tests for nested markup, error recovery.
    Target: 89% -> 93%.

## Feature-Gated Module Notes

The following modules are behind feature gates and were NOT measured in this run:
- `ftui-extras/canvas` (canvas feature)
- `ftui-extras/charts` (charts feature)
- `ftui-extras/clipboard` (clipboard feature)
- `ftui-extras/console` (console feature)
- `ftui-extras/export` (export feature)
- `ftui-extras/forms` (forms feature)
- `ftui-extras/image` (image feature)
- `ftui-extras/live` (live feature)
- `ftui-extras/logging` (logging feature)
- `ftui-extras/markdown` (markdown feature)
- `ftui-extras/pty_capture` (pty-capture feature)
- `ftui-extras/syntax` (syntax feature)
- `ftui-extras/filepicker` (filepicker feature)
- `ftui-extras/traceback` (traceback feature)

A separate coverage run with `--all-features` is needed to measure these.
