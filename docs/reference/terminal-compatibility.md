# Terminal Compatibility Matrix

This document summarizes FrankenTUI feature support across common terminals and
multiplexers. It is a user-facing guide to what works where and what ftui will
enable by default.

## Compatibility Matrix

Legend: Yes = supported, No = not supported, Partial = limited/quirks.

| Feature | Kitty | WezTerm | Alacritty | Ghostty | iTerm2 | GNOME Term | Windows Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- |
| True color (24-bit) | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Sync output (DEC 2026) | Yes | Yes | Yes | Yes | No | No | No |
| OSC 8 hyperlinks | Yes | Yes | Yes | Yes | Yes | Yes | No |
| Kitty keyboard protocol | Yes | Yes | No | Yes | No | No | No |
| Kitty graphics | Yes | Yes | No | Yes | No | No | No |
| Sixel | No | Yes | No | No | Yes | No | No |
| Focus events | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Bracketed paste | Yes | Yes | Yes | Yes | Yes | Yes | Yes |

Notes:
- Multiplexers (tmux/screen/zellij) can reduce effective support even when the
  underlying terminal supports a feature.
- ftui enables sync output, OSC 8, and kitty protocol features only when
  `TerminalCapabilities` indicates they are safe.

## Feature Details

### True color (24-bit RGB)
- Detection: `COLORTERM=truecolor` or `COLORTERM=24bit`.
- Fallback: 256-color or 16-color palette.

### Sync output (DEC 2026)
- Purpose: atomically update frames to reduce flicker.
- Detection: known terminal capability or allowlist.
- Default: enabled only when safe; disabled in multiplexers.

### OSC 8 hyperlinks
- Purpose: clickable links for help, logs, and UI metadata.
- Format: `ESC ] 8 ; ; URL ESC \` ... `ESC ] 8 ; ; ESC \`.
- Windows Terminal: not supported.

### Kitty keyboard protocol
- Purpose: richer key data (press/release/repeat, modifiers).
- Limited adoption; only enabled when explicitly detected.

### Sixel / Kitty graphics
- Purpose: inline graphics.
- Feature-gated; not required for kernel correctness.

## Glyph Policy Overrides

FrankenTUI centralizes glyph decisions (Unicode vs ASCII, emoji, line drawing,
arrows, and double-width handling) in `GlyphPolicy`. You can force deterministic
overrides via environment variables:

- `FTUI_GLYPH_MODE=unicode|ascii` — force overall glyph mode.
- `FTUI_GLYPH_EMOJI=1|0` — enable/disable emoji (ignored in ASCII mode).
- `FTUI_NO_EMOJI=1|0` — legacy alias (`1` disables emoji).
- `FTUI_GLYPH_LINE_DRAWING=1|0` — enable/disable Unicode box drawing glyphs.
- `FTUI_GLYPH_ARROWS=1|0` — enable/disable Unicode arrows/symbols.
- `FTUI_GLYPH_DOUBLE_WIDTH=1|0` — override double-width glyph support.

Notes:
- Overrides are deterministic and applied before rendering.
- `FTUI_GLYPH_MODE=ascii` forces line drawing/arrows/emoji off regardless of
  other flags.
- If double-width is disabled, emoji defaults off unless explicitly overridden.

## Multiplexer Notes

### tmux
- Passthrough required for OSC and sync output.
- Sync output is disabled by default in tmux.
- OSC 8 may work on newer tmux, but ftui is conservative by default.
- Detection: `TMUX` environment variable.

### screen
- Limited support for modern features.
- Sync output and OSC 8 passthrough are unreliable.
- Detection: `STY` environment variable.

### zellij
- Better passthrough than tmux/screen, but still conservative.
- Detection: `ZELLIJ` environment variable.

## Windows Considerations

- Windows Terminal supports ANSI and true color.
- OSC 8 hyperlinks and sync output are not supported.
- Raw mode and input handling rely on crossterm on Windows.
- conhost / legacy consoles are out of scope for v1.

## Inline Mode Notes

- Inline mode is designed to work everywhere.
- Scroll region optimization is optional and disabled in multiplexers.
- Cursor save/restore is universally supported.

## Quirk Catalog (Simulator)

The PTY simulator (`ftui-pty` `VirtualTerminal`) can apply explicit quirk
profiles via `QuirkSet` to reproduce known terminal oddities in tests.

- **tmux nested cursor save/restore**: In alt-screen, DEC save/restore (`ESC 7/8`)
  is ignored to model nested tmux cursor quirks.
- **GNU screen immediate wrap**: Writing the last column immediately wraps to
  the next line (cursor moves to column 0 after the write).
- **Windows console no alt-screen**: DEC 1049/1047 alternate screen sequences
  are ignored; output stays on the main buffer.

## Recommended Test Matrix

For CI or manual verification, test against:
1. Kitty (max feature surface)
2. WezTerm (cross-platform)
3. Alacritty (minimal, popular)
4. Windows Terminal (Windows v1 scope)

## Profile Snapshot Matrix

For simulated capability testing, set `FTUI_TEST_PROFILE` to a predefined
profile name (for example: `dumb`, `screen`, `tmux`, `windows-console`).
Snapshot filenames are suffixed with `__<profile>` so each profile keeps
its own baseline outputs. This is how we document and track profile
differences over time.

When comparing profiles in a single run, use the harness helper
`profile_matrix_text` with `FTUI_TEST_PROFILE_COMPARE=report` or `strict`.

## Known Limitations

- Multiplexers can disable or break passthrough for advanced sequences.
- Feature detection is intentionally conservative to preserve correctness.
- Some features are feature-gated and not part of the kernel contract.
