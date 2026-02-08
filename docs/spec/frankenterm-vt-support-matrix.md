# FrankenTerm VT/ANSI Support Matrix

> bd-lff4p.1.1 — Defines exactly what FrankenTerm implements (and intentionally does not)
> for VT/ANSI output streams.

Status: DRAFT
Authors: ChartreuseStream (claude-code / opus-4.6)
Date: 2026-02-08
References: ECMA-48, VT100/VT220 Programmer Reference, xterm control sequences

---

## 1. Scope

FrankenTerm is a terminal **engine** (parser + grid + state) that processes VT/ANSI byte
streams and produces a cell grid. It does NOT emit escape sequences — that's ftui-render's
Presenter. This matrix defines what the engine **consumes and interprets**.

### Design Principles

1. **Modern-first**: Prioritize what xterm, VTE, Kitty, Alacritty, WezTerm emit today.
2. **Correct subset**: Better to correctly implement 80% than incorrectly implement 100%.
3. **Explicit boundaries**: Every sequence is categorized as Must/Should/Won't.
4. **Testable**: Every Must/Should entry has a conformance fixture.

### Priority Levels

| Level | Meaning |
|-------|---------|
| **Must** | Required for correct rendering of modern terminal applications |
| **Should** | Useful for compatibility; implement if cost is low |
| **Won't** | Legacy or niche; explicitly not supported |

---

## 2. C0 Control Characters (0x00–0x1F)

| Byte | Name | Priority | Behavior |
|------|------|----------|----------|
| 0x00 | NUL | Must | Ignore (no-op) |
| 0x07 | BEL | Must | Trigger bell callback; also terminates OSC sequences |
| 0x08 | BS | Must | Move cursor left one column (stop at column 0) |
| 0x09 | HT | Must | Advance cursor to next tab stop (default every 8 columns) |
| 0x0A | LF | Must | Line feed: move cursor down, scroll if at bottom |
| 0x0B | VT | Must | Treated as LF (vertical tab = line feed) |
| 0x0C | FF | Must | Treated as LF (form feed = line feed) |
| 0x0D | CR | Must | Move cursor to column 0 |
| 0x0E | SO | Should | Invoke G1 character set into GL |
| 0x0F | SI | Should | Invoke G0 character set into GL |
| 0x1B | ESC | Must | Start escape sequence |
| 0x01–0x06, 0x10–0x1A, 0x1C–0x1F | — | Must | Ignore (no-op) |

### Notes
- LF behavior depends on LNM (Line Feed/New Line Mode): in LNM mode, LF implies CR+LF.
  Default: LF only (no implicit CR). FrankenTerm defaults to LF-only.
- Tab stops: default every 8 columns; HTS/TBC can set/clear custom stops.

---

## 3. C1 Control Characters

FrankenTerm accepts C1 controls in their **7-bit ESC encoding** only (ESC followed by
0x40–0x5F). The 8-bit single-byte C1 range (0x80–0x9F) is treated as printable under
UTF-8 and is NOT interpreted as control characters.

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| ESC [ | CSI | Must | Control Sequence Introducer |
| ESC ] | OSC | Must | Operating System Command |
| ESC P | DCS | Should | Device Control String (pass-through for now) |
| ESC \ | ST | Must | String Terminator (ends OSC, DCS, APC) |
| ESC ^ | PM | Won't | Privacy Message (ignore content until ST) |
| ESC _ | APC | Should | Application Program Command (ignore content until ST) |
| ESC D | IND | Must | Index (move cursor down, scroll if at bottom) |
| ESC M | RI | Must | Reverse Index (move cursor up, scroll if at top) |
| ESC E | NEL | Must | Next Line (CR + LF) |
| ESC H | HTS | Should | Horizontal Tab Set (set tab stop at current column) |
| ESC 7 | DECSC | Must | Save cursor position + attributes |
| ESC 8 | DECRC | Must | Restore cursor position + attributes |
| ESC c | RIS | Must | Full reset (all state to defaults) |
| ESC = | DECKPAM | Should | Keypad Application Mode |
| ESC > | DECKPNM | Should | Keypad Normal Mode |
| ESC # 8 | DECALN | Should | Screen Alignment Pattern (fill screen with 'E') |
| ESC ( B | SCS G0 | Should | Designate ASCII to G0 |
| ESC ( 0 | SCS G0 | Should | Designate DEC Special Graphics to G0 |
| ESC ) B | SCS G1 | Should | Designate ASCII to G1 |
| ESC ) 0 | SCS G1 | Should | Designate DEC Special Graphics to G1 |

---

## 4. CSI Sequences (ESC [ ...)

### 4.1 Cursor Movement

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI n A | CUU | Must | Cursor Up n rows (default 1) |
| CSI n B | CUD | Must | Cursor Down n rows (default 1) |
| CSI n C | CUF | Must | Cursor Forward n columns (default 1) |
| CSI n D | CUB | Must | Cursor Back n columns (default 1) |
| CSI n ; m H | CUP | Must | Cursor Position (row n, column m; 1-indexed) |
| CSI n ; m f | HVP | Must | Same as CUP |
| CSI n G | CHA | Must | Cursor Character Absolute (column n; 1-indexed) |
| CSI n d | VPA | Must | Vertical Position Absolute (row n; 1-indexed) |
| CSI n E | CNL | Should | Cursor Next Line (move down n, then to column 1) |
| CSI n F | CPL | Should | Cursor Previous Line (move up n, then to column 1) |
| CSI s | SCP | Must | Save Cursor Position (ANSI.SYS; separate from DECSC) |
| CSI u | RCP | Must | Restore Cursor Position |
| CSI n S | SU | Must | Scroll Up n lines (within scroll region) |
| CSI n T | SD | Must | Scroll Down n lines (within scroll region) |

### 4.2 Erase Operations

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI n J | ED | Must | Erase in Display: 0=below, 1=above, 2=all, 3=all+scrollback |
| CSI n K | EL | Must | Erase in Line: 0=right, 1=left, 2=whole line |
| CSI n X | ECH | Should | Erase n Characters at cursor (fill with spaces, keep style) |

### 4.3 Insert/Delete

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI n @ | ICH | Must | Insert n blank Characters at cursor (shift right) |
| CSI n P | DCH | Must | Delete n Characters at cursor (shift left) |
| CSI n L | IL | Must | Insert n blank Lines at cursor row (shift down) |
| CSI n M | DL | Must | Delete n Lines at cursor row (shift up) |

### 4.4 Scroll Region

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI top ; bottom r | DECSTBM | Must | Set Scrolling Region (1-indexed; default=full screen) |

### 4.5 Tab Control

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI n g | TBC | Should | Tab Clear: 0=current column, 3=all tab stops |
| CSI n I | CHT | Should | Cursor Forward Tabulation (n tab stops) |
| CSI n Z | CBT | Should | Cursor Backward Tabulation (n tab stops) |

### 4.6 Mode Setting

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI n h | SM | Should | Set Mode (n = mode number) |
| CSI n l | RM | Should | Reset Mode |
| CSI 4 h | IRM | Must | Insert Mode (ICH behavior for printable chars) |
| CSI 4 l | — | Must | Replace Mode (default; overwrite at cursor) |
| CSI 20 h | LNM | Should | Line Feed/New Line Mode (LF implies CR) |

### 4.7 Device Status / Reports

| Sequence | Name | Priority | Behavior |
|----------|------|----------|----------|
| CSI 5 n | DSR | Must | Device Status Report → respond CSI 0 n (OK) |
| CSI 6 n | CPR | Must | Cursor Position Report → respond CSI row ; col R |
| CSI c | DA1 | Must | Primary Device Attributes → respond with VT220 ID |
| CSI > c | DA2 | Should | Secondary Device Attributes → respond with version |
| CSI ? 6 n | DECXCPR | Should | Extended CPR → respond CSI ? row ; col R |

---

## 5. DEC Private Modes (CSI ? n h/l)

| Mode | Name | Priority | Set (h) | Reset (l) |
|------|------|----------|---------|-----------|
| 1 | DECCKM | Must | Application cursor keys | Normal cursor keys |
| 6 | DECOM | Should | Origin Mode (cursor relative to scroll region) | Absolute cursor addressing |
| 7 | DECAWM | Must | Auto-wrap at right margin | No auto-wrap |
| 12 | Cursor blink | Should | Start blinking | Stop blinking |
| 25 | DECTCEM | Must | Show cursor | Hide cursor |
| 47 | Alt buffer (old) | Should | Switch to alternate buffer | Switch to normal buffer |
| 1000 | Mouse tracking (X10/normal) | Must | Enable normal mouse tracking | Disable |
| 1002 | Mouse button-event | Must | Report button + motion while pressed | Disable |
| 1003 | Mouse any-event | Must | Report all motion events | Disable |
| 1004 | Focus events | Must | Enable focus in/out reporting | Disable |
| 1006 | SGR mouse mode | Must | Use SGR-style mouse coordinates | Use legacy encoding |
| 1007 | Mouse alternate scroll | Should | Enable alternate scroll mode | Disable |
| 1049 | Alt screen + save cursor | Must | Enter alt screen, save cursor | Leave alt screen, restore cursor |
| 2004 | Bracketed paste | Must | Enable bracketed paste mode | Disable |
| 2026 | Synchronized output | Must | Begin sync update | End sync update |
| 2027 | Grapheme clustering | Should | Enable grapheme cluster mode | Disable (per-codepoint) |
| 80 | DECSDM (Sixel) | Won't | — | — |

### Mode Save/Restore (XTPUSHSGR/XTPOPSGR)

| Sequence | Priority | Behavior |
|----------|----------|----------|
| CSI ? n s | Should | Save private mode n |
| CSI ? n r | Should | Restore private mode n |

---

## 6. SGR — Select Graphic Rendition (CSI ... m)

### 6.1 Text Attributes

| Code | Attribute | Priority |
|------|-----------|----------|
| 0 | Reset all attributes | Must |
| 1 | Bold (bright) | Must |
| 2 | Dim (faint) | Must |
| 3 | Italic | Must |
| 4 | Underline | Must |
| 4:0 | No underline | Must |
| 4:1 | Single underline | Must |
| 4:2 | Double underline | Should |
| 4:3 | Curly underline | Should |
| 4:4 | Dotted underline | Should |
| 4:5 | Dashed underline | Should |
| 5 | Slow blink | Should |
| 7 | Reverse video (swap fg/bg) | Must |
| 8 | Hidden (invisible) | Should |
| 9 | Strikethrough (crossed-out) | Must |
| 21 | Double underline (alt) | Should |
| 22 | Normal intensity (reset bold/dim) | Must |
| 23 | Not italic | Must |
| 24 | Not underlined | Must |
| 25 | Not blinking | Should |
| 27 | Not reversed | Must |
| 28 | Not hidden | Should |
| 29 | Not strikethrough | Must |
| 53 | Overline | Should |
| 55 | Not overline | Should |

### 6.2 Foreground Colors

| Code(s) | Color | Priority |
|---------|-------|----------|
| 30–37 | Standard foreground (8 colors) | Must |
| 38;5;n | 256-color foreground | Must |
| 38;2;r;g;b | RGB foreground (24-bit) | Must |
| 39 | Default foreground | Must |
| 90–97 | Bright foreground (8 colors) | Must |

### 6.3 Background Colors

| Code(s) | Color | Priority |
|---------|-------|----------|
| 40–47 | Standard background (8 colors) | Must |
| 48;5;n | 256-color background | Must |
| 48;2;r;g;b | RGB background (24-bit) | Must |
| 49 | Default background | Must |
| 100–107 | Bright background (8 colors) | Must |

### 6.4 Underline Color (SGR 58/59)

| Code(s) | Behavior | Priority |
|---------|----------|----------|
| 58;5;n | 256-color underline | Should |
| 58;2;r;g;b | RGB underline | Should |
| 59 | Default underline color | Should |

---

## 7. OSC — Operating System Commands (ESC ] n ; ... ST)

| OSC # | Name | Priority | Behavior |
|-------|------|----------|----------|
| 0 | Set icon name and window title | Must | Store title; surface via callback |
| 1 | Set icon name | Should | Store; may alias to OSC 0 |
| 2 | Set window title | Must | Store title |
| 4 | Set/query color palette | Should | Change indexed color entries |
| 7 | Set working directory | Should | Store; surface via callback |
| 8 | Hyperlinks | Must | OSC 8 ; params ; uri ST → open/close link |
| 9 | Desktop notification (iTerm2) | Won't | — |
| 10 | Set foreground color | Should | Change default fg |
| 11 | Set background color | Should | Change default bg |
| 12 | Set cursor color | Should | Change cursor color |
| 52 | Clipboard (set/get) | Must | OSC 52 ; c ; base64-data ST |
| 104 | Reset color palette | Should | Restore default palette entries |
| 110 | Reset foreground color | Should | Restore default fg |
| 111 | Reset background color | Should | Restore default bg |
| 112 | Reset cursor color | Should | Restore default cursor color |
| 133 | Shell integration (prompt marks) | Should | Mark prompt/command/output regions |
| 1337 | iTerm2 proprietary | Won't | — |

### OSC 8 Hyperlinks (Detail)

Format: `ESC ] 8 ; params ; uri ST`

- Opening: `ESC ] 8 ; id=myid ; https://example.com ST`
- Closing: `ESC ] 8 ; ; ST` (empty URI)
- `id` parameter groups cells into a single link (for wrapping across lines)
- Nested links: outer link is paused, inner link active; closing inner resumes outer

Priority: **Must** — already emitted by ftui-render Presenter.

### OSC 52 Clipboard

Format: `ESC ] 52 ; c ; base64data ST`

- Clipboard selection: `c` = clipboard, `p` = primary, `s` = select
- Read request: `ESC ] 52 ; c ; ? ST` → response contains base64 content
- Security: only honor clipboard operations when explicitly enabled

Priority: **Must** — modern terminals widely support this.

---

## 8. Kitty Keyboard Protocol

| Sequence | Priority | Behavior |
|----------|----------|----------|
| CSI > n u | Must | Enable progressive enhancement (flags n) |
| CSI < u | Must | Disable/pop enhancement level |
| CSI ? u | Should | Query current enhancement level |

Enhancement flags (bitfield):
- 1: Disambiguate escape codes
- 2: Report event types (press/repeat/release)
- 4: Report alternate keys
- 8: Report all keys as escape codes
- 16: Report associated text

FrankenTerm should support flags 1+2 (disambiguate + event types) at minimum.
Flag 8 (all keys as escapes) is needed for full TUI keyboard handling.

---

## 9. Character Sets

| Designation | Character Set | Priority |
|-------------|---------------|----------|
| ESC ( B | ASCII (default G0) | Should |
| ESC ( 0 | DEC Special Graphics (line-drawing) | Should |
| ESC ( A | UK ASCII | Won't |
| ESC ) B | ASCII (default G1) | Should |
| ESC ) 0 | DEC Special Graphics (G1) | Should |

DEC Special Graphics maps 0x60–0x7E to line-drawing characters (┌ ┐ └ ┘ ─ │ etc.).
This is used by legacy applications (less, vim, tmux borders).

FrankenTerm stores the active character set and translates on output. The grid
stores Unicode codepoints, not charset-mapped bytes.

---

## 10. Explicit Non-Support (Won't)

The following are explicitly out of scope:

| Feature | Reason |
|---------|--------|
| Sixel graphics (DCS q ...) | Complex image protocol; prefer iTerm2/Kitty image protocols |
| ReGIS graphics | Obsolete DEC graphics |
| Tektronix 4014 emulation | Obsolete vector graphics |
| VT52 compatibility mode | No modern use |
| ISO 2022 multi-byte charsets | Superseded by UTF-8 |
| DRCS (user-defined characters) | Rarely used |
| 8-bit C1 controls (0x80–0x9F) | Conflicts with UTF-8; use 7-bit ESC equivalents |
| VT320/VT420/VT525 extensions | Most are obscure; add individual sequences on demand |
| DECSED / DECSEL (selective erase) | Rarely used; adds significant complexity |

---

## 11. Conformance Fixture Format

Each fixture is a JSON file with input bytes and expected grid state:

```json
{
  "name": "cursor_movement_basic",
  "description": "CUP, CUU, CUD, CUF, CUB with bounds clamping",
  "initial_size": [80, 24],
  "input_bytes_hex": "1b5b313b314841 1b5b32423132 1b5b313b384841",
  "expected": {
    "cursor": { "row": 0, "col": 7 },
    "cells": [
      { "row": 0, "col": 0, "char": "A" },
      { "row": 2, "col": 0, "char": "1" },
      { "row": 2, "col": 1, "char": "2" },
      { "row": 0, "col": 7, "char": "A" }
    ],
    "title": null,
    "modes": { "autowrap": true, "cursor_visible": true }
  }
}
```

### Fixture Categories

| Category | Count (target) | Coverage |
|----------|---------------|----------|
| C0 controls | 8 | NUL, BEL, BS, HT, LF, VT/FF, CR |
| Cursor movement | 12 | CUP, CUU/D/F/B, CHA, VPA, bounds clamping |
| Erase operations | 8 | ED 0/1/2/3, EL 0/1/2, ECH |
| Insert/Delete | 6 | ICH, DCH, IL, DL at various positions |
| Scroll region | 6 | DECSTBM, scroll up/down, region boundaries |
| SGR attributes | 15 | Each attribute, reset, stacking, 256/RGB colors |
| OSC sequences | 8 | Title, hyperlink open/close, clipboard, palette |
| DEC private modes | 10 | Alt screen, cursor show/hide, mouse modes, sync |
| Character sets | 4 | G0/G1 switching, DEC Special Graphics |
| Auto-wrap | 6 | Wrap at margin, deferred wrap, wide chars at margin |
| Kitty keyboard | 4 | Enable/disable/query, flag combinations |
| Stress / edge cases | 8 | Malformed sequences, parameter overflow, rapid mode switching |

**Total target: ~95 fixtures**

### Fixture Directory Structure

```
tests/fixtures/vt-conformance/
├── c0_controls/
│   ├── null_ignored.json
│   ├── bell_callback.json
│   ├── backspace_basic.json
│   └── ...
├── cursor/
│   ├── cup_basic.json
│   ├── cup_bounds_clamp.json
│   └── ...
├── erase/
├── insert_delete/
├── scroll_region/
├── sgr/
├── osc/
├── dec_modes/
├── charsets/
├── autowrap/
├── kitty_keyboard/
└── stress/
```

---

## 12. Compatibility Boundaries

### De-Facto Standard (must match)

These behaviors must match what xterm, VTE, and Kitty do in practice, even if
the specification is ambiguous:

1. **Deferred line wrap**: When the cursor reaches the right margin, the wrap is
   deferred until the next printable character. Cursor stays at the last column.
2. **Wide character at margin**: If a wide (2-cell) character would start at the
   last column, wrap to the next line first.
3. **Scroll region + origin mode**: Cursor movement is relative to the scroll region
   when origin mode (DECOM) is active. CUP coordinates are offset by the region top.
4. **SGR 0 resets everything**: Including underline color, overline, and hyperlink state.
5. **OSC 8 closing**: Empty URI closes the current hyperlink regardless of `id` parameter.
6. **Tab stops survive resize**: When the terminal widens, new columns get default tab stops.

### FrankenTerm-Specific Decisions

1. **UTF-8 only**: No ISO 2022 encoding negotiation. All input is assumed UTF-8.
2. **No 8-bit C1**: Bytes 0x80–0x9F are printable under UTF-8, not control characters.
3. **Scrollback is ring buffer**: Fixed max depth, FIFO eviction, no infinite growth.
4. **No passthrough for unrecognized DCS/APC**: Consume and discard silently.
5. **Cell type = ftui-render Cell**: 16 bytes, no conversion layer.

---

## 13. Parser State Machine

FrankenTerm's parser follows the **Paul Flo Williams VT parser state machine**
(the de-facto standard used by Alacritty, WezTerm, and others).

### States

| State | Description |
|-------|-------------|
| Ground | Default; printable characters go to grid |
| Escape | After ESC; waiting for next byte |
| EscapeIntermediate | ESC + intermediate byte (0x20–0x2F) |
| CsiEntry | After CSI; collecting parameters |
| CsiParam | Collecting numeric parameters |
| CsiIntermediate | After CSI param + intermediate byte |
| CsiIgnore | Invalid CSI; consuming until final byte |
| OscString | Collecting OSC payload |
| DcsEntry | After DCS; similar to CSI |
| DcsParam | DCS parameter collection |
| DcsIntermediate | DCS intermediate bytes |
| DcsPassthrough | DCS data passthrough |
| DcsIgnore | Invalid DCS |

### Transition Rules

- Ground + 0x20–0x7E → print character
- Ground + 0x1B → Escape
- Escape + `[` → CsiEntry
- Escape + `]` → OscString
- Escape + `P` → DcsEntry
- CsiParam + 0x30–0x39 → accumulate digit
- CsiParam + 0x3B → next parameter
- CsiParam + 0x40–0x7E → dispatch CSI command
- OscString + BEL or ST → dispatch OSC command
- Any state + 0x18 (CAN) or 0x1A (SUB) → Ground (abort sequence)
- Any state + ESC → Escape (abort current, start new)

### Parameter Handling

- Default parameter value: 0 (or 1 depending on command semantics)
- Maximum parameter count: 32 (excess parameters silently ignored)
- Maximum parameter value: 65535 (values > 65535 clamped)
- Subparameters (colon-separated): supported for SGR 4:n and 58:2:r:g:b

---

## 14. Validation and CI

### Conformance Test Runner

```bash
# Run all VT conformance fixtures
cargo test -p frankenterm-core --test vt_conformance

# Run specific category
cargo test -p frankenterm-core --test vt_conformance -- cursor

# Bless fixtures (update expected output)
BLESS=1 cargo test -p frankenterm-core --test vt_conformance
```

### CI Gates

- All fixtures must pass on every PR
- New sequences added to the matrix must have corresponding fixtures
- Parser fuzz target runs for 30s in CI (time-bounded)
- No panics allowed in the parser (any panic = CI failure)

---

## 15. References

- [ECMA-48 (5th ed.)](https://ecma-international.org/publications-and-standards/standards/ecma-48/) — Control Functions for Coded Character Sets
- [xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) — Thomas Dickey's definitive reference
- [VT100 User Guide](https://vt100.net/docs/vt100-ug/) — DEC original documentation
- [Paul Flo Williams VT parser](https://vt100.net/emu/dec_ansi_parser) — State machine reference
- [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) — Progressive enhancement spec
- [OSC 8 hyperlinks](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda) — Terminal hyperlink spec
- docs/spec/frankenterm-architecture.md — North Star architecture (bd-lff4p.6)
- docs/spec/frankenterm-correctness.md — Correctness strategy (bd-lff4p.8)
- docs/spec/state-machines.md — Terminal + rendering pipeline state machines
