#![forbid(unsafe_code)]

//! Integration tests for global keyboard shortcuts (bd-iuvb.17.5).
//!
//! Validates that all global shortcuts trigger the expected state changes
//! and that Esc correctly dismisses overlays in priority order.

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_demo_showcase::app::{AppModel, AppMsg, ScreenId};
use ftui_runtime::Model;

fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Press,
    })
}

fn press_mod(code: KeyCode, modifiers: Modifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
    })
}

#[test]
fn m_key_toggles_mouse_capture() {
    let mut app = AppModel::new();
    assert!(app.mouse_capture_enabled);

    app.update(AppMsg::from(press(KeyCode::Char('m'))));
    assert!(!app.mouse_capture_enabled, "m should disable mouse capture");

    app.update(AppMsg::from(press(KeyCode::Char('m'))));
    assert!(app.mouse_capture_enabled, "m again should re-enable");
}

#[test]
fn f6_toggles_mouse_capture() {
    let mut app = AppModel::new();
    assert!(app.mouse_capture_enabled);

    app.update(AppMsg::from(press(KeyCode::F(6))));
    assert!(
        !app.mouse_capture_enabled,
        "F6 should disable mouse capture"
    );
}

#[test]
fn shift_a_toggles_a11y_panel() {
    let mut app = AppModel::new();
    assert!(!app.a11y_panel_visible);

    app.update(AppMsg::from(press_mod(
        KeyCode::Char('A'),
        Modifiers::SHIFT,
    )));
    assert!(app.a11y_panel_visible, "Shift+A should open a11y panel");

    app.update(AppMsg::from(press_mod(
        KeyCode::Char('A'),
        Modifiers::SHIFT,
    )));
    assert!(!app.a11y_panel_visible, "Shift+A again should close it");
}

#[test]
fn f12_toggles_debug_overlay() {
    let mut app = AppModel::new();
    assert!(!app.debug_visible);

    app.update(AppMsg::from(press(KeyCode::F(12))));
    assert!(app.debug_visible, "F12 should open debug overlay");

    app.update(AppMsg::from(press(KeyCode::F(12))));
    assert!(!app.debug_visible, "F12 again should close it");
}

#[test]
fn ctrl_p_toggles_perf_hud() {
    let mut app = AppModel::new();
    assert!(!app.perf_hud_visible);

    app.update(AppMsg::from(press_mod(KeyCode::Char('p'), Modifiers::CTRL)));
    assert!(app.perf_hud_visible, "Ctrl+P should open perf HUD");

    app.update(AppMsg::from(press_mod(KeyCode::Char('p'), Modifiers::CTRL)));
    assert!(!app.perf_hud_visible, "Ctrl+P again should close it");
}

#[test]
fn shift_l_advances_screen() {
    let mut app = AppModel::new();
    assert_eq!(app.current_screen, ScreenId::Dashboard);

    app.update(AppMsg::from(press_mod(
        KeyCode::Char('L'),
        Modifiers::SHIFT,
    )));
    assert_eq!(
        app.current_screen,
        ScreenId::Shakespeare,
        "Shift+L should advance to next screen"
    );
}

#[test]
fn shift_h_goes_previous_screen() {
    let mut app = AppModel::new();
    app.current_screen = ScreenId::Shakespeare;

    app.update(AppMsg::from(press_mod(
        KeyCode::Char('H'),
        Modifiers::SHIFT,
    )));
    assert_eq!(
        app.current_screen,
        ScreenId::Dashboard,
        "Shift+H should go to previous screen"
    );
}

#[test]
fn esc_closes_a11y_panel() {
    let mut app = AppModel::new();
    app.a11y_panel_visible = true;

    app.update(AppMsg::from(press(KeyCode::Escape)));
    assert!(!app.a11y_panel_visible, "Esc should close the a11y panel");
}

#[test]
fn esc_closes_command_palette() {
    let mut app = AppModel::new();
    app.command_palette.open();
    assert!(app.command_palette.is_visible());

    app.update(AppMsg::from(press(KeyCode::Escape)));
    assert!(
        !app.command_palette.is_visible(),
        "Esc should close the command palette"
    );
}

#[test]
fn ctrl_k_opens_command_palette() {
    let mut app = AppModel::new();
    assert!(!app.command_palette.is_visible());

    app.update(AppMsg::from(press_mod(KeyCode::Char('k'), Modifiers::CTRL)));
    assert!(
        app.command_palette.is_visible(),
        "Ctrl+K should open command palette"
    );
}

#[test]
fn question_mark_toggles_help() {
    let mut app = AppModel::new();
    assert!(!app.help_visible);

    app.update(AppMsg::from(press(KeyCode::Char('?'))));
    assert!(app.help_visible, "? should show help");

    app.update(AppMsg::from(press(KeyCode::Char('?'))));
    assert!(!app.help_visible, "? again should hide help");
}

#[test]
fn all_global_shortcuts_are_distinct() {
    // Verify no shortcut accidentally triggers two actions
    let mut app = AppModel::new();

    // Press Ctrl+P - only perf_hud should change
    app.update(AppMsg::from(press_mod(KeyCode::Char('p'), Modifiers::CTRL)));
    assert!(app.perf_hud_visible);
    assert!(!app.help_visible);
    assert!(!app.debug_visible);
    assert!(!app.a11y_panel_visible);
    assert!(!app.command_palette.is_visible());

    // Reset and press F12 - only debug should change
    let mut app = AppModel::new();
    app.update(AppMsg::from(press(KeyCode::F(12))));
    assert!(app.debug_visible);
    assert!(!app.perf_hud_visible);
    assert!(!app.help_visible);
    assert!(!app.a11y_panel_visible);
}

#[test]
fn help_visible_after_question_mark_survives_toggle_cycle() {
    // Verify that toggling help on/off/on results in help visible
    let mut app = AppModel::new();

    app.update(AppMsg::from(press(KeyCode::Char('?'))));
    assert!(app.help_visible);

    app.update(AppMsg::from(press(KeyCode::Char('?'))));
    assert!(!app.help_visible);

    app.update(AppMsg::from(press(KeyCode::Char('?'))));
    assert!(app.help_visible, "third toggle should show help again");
}
