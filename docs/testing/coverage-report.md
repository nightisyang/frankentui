# Coverage Report (llvm-cov)

- Generated: 2026-02-02 03:15:52Z (baseline)
- Command: `cargo llvm-cov --workspace --all-targets --all-features --summary-only --json --output-path /tmp/ftui_coverage.json`
- Notes: All tests passed; coverage run completed without `--ignore-run-fail`.
  - 2026-02-04: attempted a fresh full run (command below); coverage run failed in `ftui-demo-showcase` a11y snapshot tests and then hung. Baseline below remains authoritative.
    - Command: `cargo llvm-cov --workspace --all-targets --all-features --summary-only --json --output-path /tmp/ftui_coverage_pre.json`
    - Outcome: a11y snapshot failures + many tests stalled >60s; run aborted (cargo-llvm-cov/test processes terminated).

## Failing Tests During Coverage Run
- `crates/ftui-demo-showcase/tests/a11y_snapshots.rs` failures:
  - `a11y_accessibility_panel_all_modes_120x40`
  - `a11y_accessibility_panel_high_contrast_80x24`
  - `a11y_accessibility_panel_reduced_motion_80x24`
  - `a11y_accessibility_panel_large_text_80x24`
- Additional `a11y_snapshots` tests reported running >60s (large batch across dashboard/data_viz/forms/widget_gallery and multiple size modes); run was terminated to prevent indefinite hang.

## Coverage Summary (Lines)
Target policy: overall >= 70%; per-crate targets per `coverage-matrix.md`.

| Crate | Covered / Total | % | Target | Delta | Status |
| --- | ---: | ---: | ---: | ---: | :---: |
| `ftui` | 0/8 | 0.00% | n/a | n/a | n/a |
| `ftui-core` | 4612/4862 | 94.86% | 80% | +14.86 | PASS |
| `ftui-demo-showcase` | 745/1442 | 51.66% | n/a | n/a | n/a |
| `ftui-extras` | 8679/9680 | 89.66% | 60% | +29.66 | PASS |
| `ftui-harness` | 2030/2652 | 76.55% | n/a | n/a | n/a |
| `ftui-layout` | 1308/1343 | 97.39% | 75% | +22.39 | PASS |
| `ftui-pty` | 603/667 | 90.40% | n/a | n/a | n/a |
| `ftui-render` | 6180/6501 | 95.06% | 80% | +15.06 | PASS |
| `ftui-runtime` | 3341/3976 | 84.03% | 75% | +9.03 | PASS |
| `ftui-style` | 1607/1631 | 98.53% | 80% | +18.53 | PASS |
| `ftui-text` | 5014/5309 | 94.44% | 80% | +14.44 | PASS |
| `ftui-widgets` | 11613/12897 | 90.04% | 70% | +20.04 | PASS |

## Lowest-Covered Files (Top 5 per Target Crate)
### `ftui-core`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-core/src/caps_probe.rs` | 734/804 | 91.29% |
| `/data/projects/frankentui/crates/ftui-core/src/inline_mode.rs` | 247/268 | 92.16% |
| `/data/projects/frankentui/crates/ftui-core/src/terminal_session.rs` | 514/555 | 92.61% |
| `/data/projects/frankentui/crates/ftui-core/src/event.rs` | 395/426 | 92.72% |
| `/data/projects/frankentui/crates/ftui-core/src/input_parser.rs` | 836/885 | 94.46% |

### `ftui-extras`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-extras/src/image.rs` | 118/254 | 46.46% |
| `/data/projects/frankentui/crates/ftui-extras/src/pty_capture.rs` | 127/170 | 74.71% |
| `/data/projects/frankentui/crates/ftui-extras/src/clipboard.rs` | 661/854 | 77.40% |
| `/data/projects/frankentui/crates/ftui-extras/src/forms.rs` | 1020/1199 | 85.07% |
| `/data/projects/frankentui/crates/ftui-extras/src/console.rs` | 424/485 | 87.42% |

### `ftui-layout`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-layout/src/debug.rs` | 450/476 | 94.54% |
| `/data/projects/frankentui/crates/ftui-layout/src/grid.rs` | 435/441 | 98.64% |
| `/data/projects/frankentui/crates/ftui-layout/src/lib.rs` | 423/426 | 99.30% |

### `ftui-render`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-render/src/terminal_model.rs` | 767/877 | 87.46% |
| `/data/projects/frankentui/crates/ftui-render/src/frame.rs` | 463/524 | 88.36% |
| `/data/projects/frankentui/crates/ftui-render/src/presenter.rs` | 403/435 | 92.64% |
| `/data/projects/frankentui/crates/ftui-render/src/cell.rs` | 472/506 | 93.28% |
| `/data/projects/frankentui/crates/ftui-render/src/buffer.rs` | 465/485 | 95.88% |

### `ftui-runtime`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-runtime/src/program.rs` | 205/602 | 34.05% |
| `/data/projects/frankentui/crates/ftui-runtime/src/string_model.rs` | 176/209 | 84.21% |
| `/data/projects/frankentui/crates/ftui-runtime/src/render_thread.rs` | 157/178 | 88.20% |
| `/data/projects/frankentui/crates/ftui-runtime/src/terminal_writer.rs` | 854/960 | 88.96% |
| `/data/projects/frankentui/crates/ftui-runtime/src/asciicast.rs` | 248/267 | 92.88% |

### `ftui-style`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-style/src/style.rs` | 345/357 | 96.64% |
| `/data/projects/frankentui/crates/ftui-style/src/stylesheet.rs` | 274/278 | 98.56% |
| `/data/projects/frankentui/crates/ftui-style/src/theme.rs` | 605/611 | 99.02% |
| `/data/projects/frankentui/crates/ftui-style/src/color.rs` | 383/385 | 99.48% |

### `ftui-text`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-text/src/text.rs` | 469/559 | 83.90% |
| `/data/projects/frankentui/crates/ftui-text/src/markup.rs` | 431/485 | 88.87% |
| `/data/projects/frankentui/crates/ftui-text/src/segment.rs` | 491/545 | 90.09% |
| `/data/projects/frankentui/crates/ftui-text/src/bidi.rs` | 160/173 | 92.49% |
| `/data/projects/frankentui/crates/ftui-text/src/search.rs` | 290/306 | 94.77% |

### `ftui-widgets`
| File | Covered / Total | % |
| --- | ---: | ---: |
| `/data/projects/frankentui/crates/ftui-widgets/src/log_viewer.rs` | 678/907 | 74.75% |
| `/data/projects/frankentui/crates/ftui-widgets/src/block.rs` | 364/462 | 78.79% |
| `/data/projects/frankentui/crates/ftui-widgets/src/file_picker.rs` | 302/374 | 80.75% |
| `/data/projects/frankentui/crates/ftui-widgets/src/virtualized.rs` | 648/799 | 81.10% |
| `/data/projects/frankentui/crates/ftui-widgets/src/textarea.rs` | 654/791 | 82.68% |

## Follow-ups
- `ftui-runtime/src/program.rs` remains the single largest unit-test gap (34.05% coverage).
- Demo/showcase binaries are still lightly covered; add targeted integration scenarios if we want higher executable coverage.
