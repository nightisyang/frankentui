# Fixes Summary - Session 2026-02-01

## 1. Cursor Tracking in Presenter
**File:** `crates/ftui-render/src/presenter.rs`
**Issue:** The presenter used `cell.width_hint()` (which returns 1 for wide characters) instead of `cell.width()` (which calculates correct width) to update its internal cursor state.
**Fix:** Changed to use `cell.width()`. This ensures the presenter correctly tracks the terminal cursor position when rendering wide characters (e.g., CJK, Emoji), preventing rendering artifacts and redundant cursor move sequences.

## 2. Input Parser UTF-8 Recovery
**File:** `crates/ftui-core/src/input_parser.rs`
**Issue:** The UTF-8 state machine swallowed the invalid byte when a sequence was broken (e.g., unexpected start byte inside a sequence).
**Fix:** Modified `process_utf8` to transition to `Ground` state and immediately re-process the unexpected byte. This prevents data loss when input streams are slightly malformed or interleaved.

## 3. Layout Division by Zero Protection
**File:** `crates/ftui-layout/src/lib.rs`
**Issue:** The `Constraint::Ratio(n, d)` solver could panic if `d` was 0.
**Fix:** Added `.max(1)` to the denominator in `solve_constraints`. This ensures the layout solver is robust against invalid user input.

## 4. Text Wrapping Newline Handling
**File:** `crates/ftui-text/src/wrap.rs`
**Issue:** `wrap_words` logic incorrectly swallowed explicit newlines when paragraphs were empty (e.g., `"\n"` input resulted in one empty line instead of two).
**Fix:** Rewrote `wrap_words` to process paragraphs independently and ensure that every paragraph (even empty ones) produces at least one line in the output. This guarantees that `text.split('\n')` structure is preserved in the wrapped output.
