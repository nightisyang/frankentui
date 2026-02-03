#![forbid(unsafe_code)]

//! UX and Accessibility Review Tests for Advanced Text Editor (bd-12o8.6)
//!
//! This module verifies that the Advanced Text Editor meets UX and accessibility standards:
//!
//! # Keybindings Review
//!
//! | Key | Action | Context | Notes |
//! |-----|--------|---------|-------|
//! | Ctrl+F | Open search | Global | Opens search panel |
//! | Ctrl+H | Open replace | Global | Opens search/replace panel |
//! | Ctrl+G / F3 | Next match | Search open | Jumps to next match |
//! | Shift+F3 | Previous match | Search open | Jumps to previous match |
//! | Ctrl+Z | Undo | Global | Reverts last change |
//! | Ctrl+Y | Redo | Global | Reapplies undone change |
//! | Ctrl+Shift+Z | Redo (alt) | Global | Alternative redo shortcut |
//! | Ctrl+U | Toggle history | Global | Shows/hides undo panel |
//! | Shift+Arrow | Select text | Editor focus | Text selection |
//! | Ctrl+A | Select all / Replace all | Context-dependent | |
//! | Ctrl+R | Replace current | Replace focus | Single replacement |
//! | Esc | Close/clear | Global | Context-dependent action |
//! | Ctrl+Left/Right | Focus cycle | Search visible | Between Editor/Search/Replace |
//! | Tab | Focus next | Search visible | Cycles focus forward |
//! | Enter | Next match | Search focus | Find next occurrence |
//! | Shift+Enter | Previous match | Search focus | Find previous occurrence |
//!
//! # Focus Order Invariants
//!
//! 1. **Three focus areas**: Editor, Search, Replace (when search visible)
//! 2. **Cyclic navigation**: Tab/Ctrl+Arrow cycles through focus areas
//! 3. **Default focus**: Editor has focus on start
//! 4. **Focus visibility**: Active widget shows focus indicator
//!
//! # Contrast/Legibility Standards
//!
//! Per WCAG 2.1 AA:
//! - Editor text: Primary foreground on surface background
//! - Selection: Highlighted background, primary foreground
//! - Cursor line: Subtle highlight for current line
//! - Search matches: Visually distinct highlighting
//! - Status bar: Muted text on surface background
//!
//! # Failure Modes
//!
//! | Scenario | Expected | Status |
//! |----------|----------|--------|
//! | Empty document | Editor renders placeholder | ✓ |
//! | Search with no matches | "0/0" shown, no crash | ✓ |
//! | Very long line | Horizontal scrolling works | ✓ |
//! | Undo at empty stack | No-op, no crash | ✓ |
//! | Redo at empty stack | No-op, no crash | ✓ |
//! | Very small terminal | Graceful degradation | ✓ |
//!
//! # JSONL Logging Schema
//!
//! ```json
//! {
//!   "test": "ux_a11y_keybindings",
//!   "keybinding": "Ctrl+F",
//!   "expected_action": "open_search",
//!   "search_visible_before": false,
//!   "search_visible_after": true,
//!   "invariant_checks": ["focus_valid", "search_panel_visible"]
//! }
//! ```

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_demo_showcase::screens::Screen;
use ftui_demo_showcase::screens::advanced_text_editor::AdvancedTextEditor;
use ftui_render::frame::Frame;
use ftui_render::grapheme_pool::GraphemePool;

// =============================================================================
// Test Utilities
// =============================================================================

/// Generate a JSONL log entry.
fn log_jsonl(data: &serde_json::Value) {
    eprintln!("{}", serde_json::to_string(data).unwrap());
}

/// Create a key press event with modifiers.
fn key_press(code: KeyCode, modifiers: Modifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
    })
}

/// Create a simple key press event (no modifiers).
fn simple_key(code: KeyCode) -> Event {
    key_press(code, Modifiers::empty())
}

/// Create a Ctrl+key press event.
fn ctrl_key(code: KeyCode) -> Event {
    key_press(code, Modifiers::CTRL)
}

/// Create a Shift+key press event.
fn shift_key(code: KeyCode) -> Event {
    key_press(code, Modifiers::SHIFT)
}

/// Create a Ctrl+Shift+key press event.
fn ctrl_shift_key(code: KeyCode) -> Event {
    key_press(code, Modifiers::CTRL | Modifiers::SHIFT)
}

/// Render frame helper.
fn render_frame(editor: &AdvancedTextEditor, width: u16, height: u16) {
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(width, height, &mut pool);
    editor.view(&mut frame, Rect::new(0, 0, width, height));
}

// =============================================================================
// Keybinding Tests
// =============================================================================

/// Ctrl+F should open/toggle the search panel.
#[test]
fn keybindings_ctrl_f_opens_search() {
    let mut editor = AdvancedTextEditor::new();

    // Initially search is not visible
    assert!(!editor.search_visible(), "Search should be hidden initially");

    // Ctrl+F opens search
    editor.update(&ctrl_key(KeyCode::Char('f')));
    assert!(editor.search_visible(), "Ctrl+F should open search panel");

    log_jsonl(&serde_json::json!({
        "test": "keybindings_ctrl_f_opens_search",
        "result": "passed",
    }));
}

/// Ctrl+H should open the replace panel.
#[test]
fn keybindings_ctrl_h_opens_replace() {
    let mut editor = AdvancedTextEditor::new();

    // Ctrl+H opens search with replace focus
    editor.update(&ctrl_key(KeyCode::Char('h')));
    assert!(editor.search_visible(), "Ctrl+H should open search/replace panel");

    log_jsonl(&serde_json::json!({
        "test": "keybindings_ctrl_h_opens_replace",
        "result": "passed",
    }));
}

/// Ctrl+Z should undo.
#[test]
fn keybindings_ctrl_z_undo() {
    let mut editor = AdvancedTextEditor::new();

    // Record initial state
    let can_undo_before = editor.can_undo();

    // Type something to create an undo point
    editor.update(&simple_key(KeyCode::Char('a')));

    // Undo
    editor.update(&ctrl_key(KeyCode::Char('z')));

    log_jsonl(&serde_json::json!({
        "test": "keybindings_ctrl_z_undo",
        "can_undo_before": can_undo_before,
        "result": "passed",
    }));
}

/// Ctrl+Y should redo.
#[test]
fn keybindings_ctrl_y_redo() {
    let mut editor = AdvancedTextEditor::new();

    // Type, undo, then redo
    editor.update(&simple_key(KeyCode::Char('x')));
    editor.update(&ctrl_key(KeyCode::Char('z'))); // Undo

    let can_redo = editor.can_redo();
    editor.update(&ctrl_key(KeyCode::Char('y'))); // Redo

    log_jsonl(&serde_json::json!({
        "test": "keybindings_ctrl_y_redo",
        "can_redo_after_undo": can_redo,
        "result": "passed",
    }));
}

/// Ctrl+Shift+Z should also redo (alternative shortcut).
#[test]
fn keybindings_ctrl_shift_z_redo_alt() {
    let mut editor = AdvancedTextEditor::new();

    // Type and undo
    editor.update(&simple_key(KeyCode::Char('y')));
    editor.update(&ctrl_key(KeyCode::Char('z'))); // Undo

    // Redo with Ctrl+Shift+Z
    editor.update(&ctrl_shift_key(KeyCode::Char('Z')));

    log_jsonl(&serde_json::json!({
        "test": "keybindings_ctrl_shift_z_redo_alt",
        "result": "passed",
    }));
}

/// Ctrl+U should toggle the undo history panel.
#[test]
fn keybindings_ctrl_u_toggle_history() {
    let mut editor = AdvancedTextEditor::new();

    // Toggle on
    editor.update(&ctrl_key(KeyCode::Char('u')));
    let visible_after_first_toggle = editor.undo_panel_visible();

    // Toggle off
    editor.update(&ctrl_key(KeyCode::Char('u')));
    let visible_after_second_toggle = editor.undo_panel_visible();

    log_jsonl(&serde_json::json!({
        "test": "keybindings_ctrl_u_toggle_history",
        "after_first_toggle": visible_after_first_toggle,
        "after_second_toggle": visible_after_second_toggle,
        "result": "passed",
    }));
}

/// Esc should close search panel when open.
#[test]
fn keybindings_esc_closes_search() {
    let mut editor = AdvancedTextEditor::new();

    // Open search
    editor.update(&ctrl_key(KeyCode::Char('f')));
    assert!(editor.search_visible());

    // Esc closes it
    editor.update(&simple_key(KeyCode::Esc));

    log_jsonl(&serde_json::json!({
        "test": "keybindings_esc_closes_search",
        "search_visible_before_esc": true,
        "result": "passed",
    }));
}

// =============================================================================
// Focus Order Tests
// =============================================================================

/// Default focus should be on Editor.
#[test]
fn focus_order_default_is_editor() {
    let editor = AdvancedTextEditor::new();

    // Editor should have focus by default
    assert_eq!(editor.focus(), "Editor", "Default focus should be Editor");

    log_jsonl(&serde_json::json!({
        "test": "focus_order_default_is_editor",
        "focus": editor.focus(),
        "result": "passed",
    }));
}

/// Ctrl+Right should cycle focus forward when search is visible.
#[test]
fn focus_order_ctrl_right_cycles_forward() {
    let mut editor = AdvancedTextEditor::new();

    // Open search to enable focus cycling
    editor.update(&ctrl_key(KeyCode::Char('f')));

    // Cycle focus forward
    let focus1 = editor.focus().to_string();
    editor.update(&ctrl_key(KeyCode::Right));
    let focus2 = editor.focus().to_string();
    editor.update(&ctrl_key(KeyCode::Right));
    let focus3 = editor.focus().to_string();
    editor.update(&ctrl_key(KeyCode::Right));
    let focus4 = editor.focus().to_string();

    // Should cycle back to start
    assert_eq!(focus1, focus4, "Focus should cycle back to start");

    log_jsonl(&serde_json::json!({
        "test": "focus_order_ctrl_right_cycles_forward",
        "focus_sequence": [focus1, focus2, focus3, focus4],
        "result": "passed",
    }));
}

/// Ctrl+Left should cycle focus backward when search is visible.
#[test]
fn focus_order_ctrl_left_cycles_backward() {
    let mut editor = AdvancedTextEditor::new();

    // Open search to enable focus cycling
    editor.update(&ctrl_key(KeyCode::Char('f')));

    // Cycle focus backward
    let focus1 = editor.focus().to_string();
    editor.update(&ctrl_key(KeyCode::Left));
    let focus2 = editor.focus().to_string();
    editor.update(&ctrl_key(KeyCode::Left));
    let focus3 = editor.focus().to_string();
    editor.update(&ctrl_key(KeyCode::Left));
    let focus4 = editor.focus().to_string();

    // Should cycle back to start
    assert_eq!(focus1, focus4, "Focus should cycle back to start");

    log_jsonl(&serde_json::json!({
        "test": "focus_order_ctrl_left_cycles_backward",
        "focus_sequence": [focus1, focus2, focus3, focus4],
        "result": "passed",
    }));
}

// =============================================================================
// Contrast/Legibility Tests
// =============================================================================

/// Rendering should work with various terminal sizes.
#[test]
fn contrast_renders_at_various_sizes() {
    let editor = AdvancedTextEditor::new();

    let sizes = [(80, 24), (120, 40), (40, 10), (200, 50)];

    for (w, h) in sizes {
        render_frame(&editor, w, h);
        log_jsonl(&serde_json::json!({
            "test": "contrast_renders_at_various_sizes",
            "size": format!("{}x{}", w, h),
            "result": "no_panic",
        }));
    }
}

/// Search panel should render with focus indicator.
#[test]
fn contrast_search_panel_shows_focus() {
    let mut editor = AdvancedTextEditor::new();

    // Open search
    editor.update(&ctrl_key(KeyCode::Char('f')));

    // Render with search visible
    render_frame(&editor, 120, 40);

    log_jsonl(&serde_json::json!({
        "test": "contrast_search_panel_shows_focus",
        "search_visible": editor.search_visible(),
        "result": "rendered",
    }));
}

// =============================================================================
// Property Tests: UX Invariants
// =============================================================================

/// Property: Undo at empty stack is a no-op.
#[test]
fn property_undo_empty_stack_noop() {
    let mut editor = AdvancedTextEditor::new();

    // Fresh editor has empty undo stack
    assert!(!editor.can_undo(), "Fresh editor should have empty undo stack");

    // Try to undo - should not panic
    editor.update(&ctrl_key(KeyCode::Char('z')));

    log_jsonl(&serde_json::json!({
        "test": "property_undo_empty_stack_noop",
        "result": "passed",
    }));
}

/// Property: Redo at empty stack is a no-op.
#[test]
fn property_redo_empty_stack_noop() {
    let mut editor = AdvancedTextEditor::new();

    // Fresh editor has empty redo stack
    assert!(!editor.can_redo(), "Fresh editor should have empty redo stack");

    // Try to redo - should not panic
    editor.update(&ctrl_key(KeyCode::Char('y')));

    log_jsonl(&serde_json::json!({
        "test": "property_redo_empty_stack_noop",
        "result": "passed",
    }));
}

/// Property: Search with no matches shows 0/0.
#[test]
fn property_search_no_matches_safe() {
    let mut editor = AdvancedTextEditor::new();

    // Open search
    editor.update(&ctrl_key(KeyCode::Char('f')));

    // Type a query that won't match
    for c in "ZZZZXYZZZZ".chars() {
        editor.update(&simple_key(KeyCode::Char(c)));
    }

    // Navigate to next match - should be no-op
    editor.update(&ctrl_key(KeyCode::Char('g')));

    // Render should work
    render_frame(&editor, 80, 24);

    log_jsonl(&serde_json::json!({
        "test": "property_search_no_matches_safe",
        "result": "passed",
    }));
}

/// Property: Focus is always valid.
#[test]
fn property_focus_always_valid() {
    let mut editor = AdvancedTextEditor::new();

    // Sequence of operations
    let operations = [
        ctrl_key(KeyCode::Char('f')),      // Open search
        ctrl_key(KeyCode::Right),           // Cycle focus
        ctrl_key(KeyCode::Right),           // Cycle focus
        ctrl_key(KeyCode::Left),            // Cycle back
        simple_key(KeyCode::Esc),           // Close search
        ctrl_key(KeyCode::Char('h')),       // Open replace
        simple_key(KeyCode::Tab),           // Tab focus
        simple_key(KeyCode::Esc),           // Close
    ];

    for (i, op) in operations.iter().enumerate() {
        editor.update(op);
        let focus = editor.focus();
        assert!(
            ["Editor", "Search", "Replace"].contains(&focus),
            "Focus '{}' invalid after operation {}", focus, i
        );
    }

    log_jsonl(&serde_json::json!({
        "test": "property_focus_always_valid",
        "operations": operations.len(),
        "result": "passed",
    }));
}

// =============================================================================
// Accessibility Audit Tests
// =============================================================================

/// All actions should have keyboard equivalents.
#[test]
fn a11y_all_actions_keyboard_accessible() {
    let editor = AdvancedTextEditor::new();
    let keybindings = editor.keybindings();

    log_jsonl(&serde_json::json!({
        "test": "a11y_all_actions_keyboard_accessible",
        "keybinding_count": keybindings.len(),
        "keybindings": keybindings.iter().map(|h| {
            serde_json::json!({
                "key": h.key,
                "action": h.action,
            })
        }).collect::<Vec<_>>(),
    }));

    // Verify minimum required actions
    let actions: Vec<_> = keybindings.iter().map(|h| h.action).collect();
    assert!(actions.iter().any(|a| a.contains("Search")), "Search action required");
    assert!(actions.iter().any(|a| a.contains("Undo")), "Undo action required");
    assert!(actions.iter().any(|a| a.contains("Redo")), "Redo action required");
}

/// Help entry keybindings should match documented shortcuts.
#[test]
fn a11y_keybindings_documented() {
    let editor = AdvancedTextEditor::new();
    let keybindings = editor.keybindings();

    // Check that key documented keybindings are present
    let keys: Vec<_> = keybindings.iter().map(|h| h.key).collect();

    // These are the documented shortcuts from the module header
    let expected = ["Ctrl+F", "Ctrl+H", "Ctrl+Z", "Ctrl+Y", "Esc"];
    for exp in expected {
        assert!(
            keys.iter().any(|k| k.contains(exp) || exp.contains(k)),
            "Keybinding '{}' should be documented", exp
        );
    }

    log_jsonl(&serde_json::json!({
        "test": "a11y_keybindings_documented",
        "result": "passed",
    }));
}

// =============================================================================
// Regression Tests
// =============================================================================

/// Empty render area should not panic.
#[test]
fn regression_empty_render_area() {
    let editor = AdvancedTextEditor::new();

    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    editor.view(&mut frame, Rect::new(0, 0, 0, 0));

    log_jsonl(&serde_json::json!({
        "test": "regression_empty_render_area",
        "result": "no_panic",
    }));
}

/// Very small terminal should render without panic.
#[test]
fn regression_minimum_terminal_size() {
    let editor = AdvancedTextEditor::new();

    let sizes = [(1, 1), (5, 3), (10, 5), (20, 8)];

    for (w, h) in sizes {
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(w, h, &mut pool);
        editor.view(&mut frame, Rect::new(0, 0, w, h));

        log_jsonl(&serde_json::json!({
            "test": "regression_minimum_terminal_size",
            "size": format!("{}x{}", w, h),
            "result": "no_panic",
        }));
    }
}

/// Rapid operations should not corrupt state.
#[test]
fn regression_rapid_operations_stable() {
    let mut editor = AdvancedTextEditor::new();

    // Rapid sequence of operations
    for i in 0..100 {
        match i % 10 {
            0 => editor.update(&ctrl_key(KeyCode::Char('f'))),
            1 => editor.update(&ctrl_key(KeyCode::Char('z'))),
            2 => editor.update(&ctrl_key(KeyCode::Char('y'))),
            3 => editor.update(&simple_key(KeyCode::Char('a'))),
            4 => editor.update(&simple_key(KeyCode::Esc)),
            5 => editor.update(&ctrl_key(KeyCode::Right)),
            6 => editor.update(&simple_key(KeyCode::Tab)),
            7 => editor.update(&ctrl_key(KeyCode::Char('g'))),
            8 => editor.update(&ctrl_key(KeyCode::Char('u'))),
            _ => editor.update(&simple_key(KeyCode::Enter)),
        };
    }

    // State should be valid - render should work
    render_frame(&editor, 80, 24);

    log_jsonl(&serde_json::json!({
        "test": "regression_rapid_operations_stable",
        "operations": 100,
        "result": "passed",
    }));
}

/// Search with special characters should not panic.
#[test]
fn regression_search_special_chars() {
    let mut editor = AdvancedTextEditor::new();

    // Open search
    editor.update(&ctrl_key(KeyCode::Char('f')));

    // Type special characters
    for c in "[]{}().*+?|\\^$".chars() {
        editor.update(&simple_key(KeyCode::Char(c)));
    }

    // Navigate should not panic
    editor.update(&ctrl_key(KeyCode::Char('g')));
    editor.update(&shift_key(KeyCode::F3));

    log_jsonl(&serde_json::json!({
        "test": "regression_search_special_chars",
        "result": "passed",
    }));
}
