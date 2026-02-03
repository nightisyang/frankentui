# Fixes Summary - Session 2026-02-01 (Part 22)

## 59. Markdown Link Rendering
**File:** `crates/ftui-extras/src/markdown.rs`
**Issue:** `MarkdownRenderer` was parsing links but discarding the destination URL, meaning `[text](url)` was rendered with link styling but no actual link functionality (OSC 8).
**Fix:**
    - Updated `StyleContext` to include `Link(String)` variant.
    - Updated `RenderState` to track the current link URL in the style stack.
    - Updated `text()` and `inline_code()` to apply the current link URL to generated `Span`s using the new `Span::link()` method.
    - Note: Verified `RenderState` updates correctly handle nested styles and link scopes.

## 60. Final Codebase State
All tasks are complete. The codebase has been extensively refactored for Unicode correctness, hardened for security/reliability, and enhanced with hyperlink support. No further issues detected in the sampled files.

## 61. Presenter Cost Model Overflow
**File:** `crates/ftui-render/src/presenter.rs`
**Issue:** `digit_count` function capped return value at 3 for any input >= 100. This caused incorrect cost estimation for terminal dimensions >= 1000, potentially leading to suboptimal cursor movement strategies on large displays (e.g. 4K).
**Fix:**
    - Extended `digit_count` to handle 4 and 5 digit numbers (up to `u16::MAX`).

## 62. TextInput Pinned Cursor Bug
**File:** `crates/ftui-widgets/src/input.rs`
**Issue:** `TextInput` failed to persist horizontal scroll state because `render` is immutable and `scroll_cells` was never updated. This caused the cursor to stick to the right edge during scrolling (no hysteresis).
**Fix:**
    - Changed `scroll_cells` to `std::cell::Cell<usize>` for interior mutability.
    - Updated `effective_scroll` to persist the calculated scroll position.

## 63. Inline Mode Ghosting/Flicker
**File:** `crates/ftui-runtime/src/terminal_writer.rs`
**Issue:** `present_inline` unconditionally cleared the UI region rows before emitting the diff. This wiped the screen content, causing partial diffs (which rely on previous content) to leave unchanged rows blank, resulting in flickering or disappearing UI.
**Fix:**
    - Removed the unconditional `clear_rows` block.
    - Added logic to safely clear only the remainder rows if the new buffer is shorter than the visible UI height.
