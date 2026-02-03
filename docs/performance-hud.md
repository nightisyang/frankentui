# Performance HUD User Guide

The Performance HUD is an on-screen overlay that surfaces frame time, diff size, and render budget state in real time. It is designed to be lightweight, deterministic, and safe to keep on during normal usage.

## Quick Start

Run the demo showcase and jump directly to the Performance screen:

```bash
cargo run -p ftui-demo-showcase -- --screen=10
```

Toggle the HUD overlay on/off with `Ctrl+P`.

## Keybindings

- `Ctrl+P` Toggle Performance HUD
- `?` Toggle help overlay
- `F12` Toggle debug overlay
- `Ctrl+K` Open command palette
- `Tab` / `Shift+Tab` Cycle screens
- `q` or `Ctrl+C` Quit

## Demo Script

A convenience script is provided to launch the HUD demo with a predictable setup:

```bash
./scripts/perf_hud_demo.sh
```

Useful options:

- `--inline` Run in inline mode
- `--ui-height 12` Set inline UI height
- `--auto-exit 1500` Auto-quit after N milliseconds
- `--pty` Run under a PTY (useful in CI)

## What You Are Seeing

The HUD surfaces these signals:

- Frame time (ms) and tick rate
- Render budget state (remaining time and degradation level)
- Diff size (changed cells and run count)
- Output volume (bytes emitted and bytes per cell)

See the full spec for layout details, invariants, and failure modes: `docs/spec/performance-hud.md`.

## Determinism + Safety Guarantees

- Output is deterministic given identical inputs.
- Missing data renders as `n/a` and never panics.
- Tiny terminals fall back to a minimal single-line HUD.

## Troubleshooting

- If the HUD does not appear, press `Ctrl+P` or use the command palette action `cmd:toggle_perf_hud`.
- If the UI is too small, increase terminal size or use alt-screen mode.

## Tests

```bash
cargo test -p ftui-demo-showcase perf_hud
cargo test -p ftui-demo-showcase --test perf_hud_e2e
```
