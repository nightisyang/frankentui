#![forbid(unsafe_code)]

//! Forms and Input screen — interactive form widgets and text editing.
//!
//! Demonstrates:
//! - `Form` with Text, Checkbox, Radio, Select, Number fields
//! - `TextInput` (single-line, with password mask)
//! - `TextArea` (multi-line editor with line numbers)
//! - Panel-based focus management

use std::cell::RefCell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_extras::forms::{Form, FormField, FormState};
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::input::TextInput;
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::textarea::TextArea;
use ftui_widgets::{StatefulWidget, Widget};

use super::{HelpEntry, Screen};
use crate::theme;

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPanel {
    Form,
    SearchInput,
    PasswordInput,
    TextEditor,
}

impl FocusPanel {
    fn next(self) -> Self {
        match self {
            Self::Form => Self::SearchInput,
            Self::SearchInput => Self::PasswordInput,
            Self::PasswordInput => Self::TextEditor,
            Self::TextEditor => Self::Form,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Form => Self::TextEditor,
            Self::SearchInput => Self::Form,
            Self::PasswordInput => Self::SearchInput,
            Self::TextEditor => Self::PasswordInput,
        }
    }
}

pub struct FormsInput {
    focus: FocusPanel,
    form: Form,
    /// `RefCell` because `StatefulWidget::render` needs `&mut FormState`
    /// but `Screen::view` only has `&self`.
    form_state: RefCell<FormState>,
    search_input: TextInput,
    password_input: TextInput,
    textarea: TextArea,
    status_text: String,
}

impl Default for FormsInput {
    fn default() -> Self {
        Self::new()
    }
}

impl FormsInput {
    pub fn new() -> Self {
        let form = Form::new(vec![
            FormField::text_with_placeholder("Name", "Enter your name..."),
            FormField::text_with_placeholder("Email", "user@example.com"),
            FormField::select(
                "Role",
                vec![
                    "Developer".into(),
                    "Designer".into(),
                    "Manager".into(),
                    "QA Engineer".into(),
                ],
            ),
            FormField::radio(
                "Theme",
                vec!["Light".into(), "Dark".into(), "System".into()],
            ),
            FormField::number_bounded("Age", 25, 0, 120),
            FormField::checkbox("Accept Terms", false),
        ]);

        let search_input = TextInput::new()
            .with_placeholder("Search...")
            .with_style(Style::new().fg(theme::fg::PRIMARY))
            .with_focused(false);

        let password_input = TextInput::new()
            .with_placeholder("Password")
            .with_mask('*')
            .with_style(Style::new().fg(theme::fg::PRIMARY))
            .with_focused(false);

        let textarea = TextArea::new()
            .with_text(
                "Hello, world!\n\
                 \n\
                 This is a multi-line text editor.\n\
                 You can type, select, undo/redo, and more.\n\
                 \n\
                 Try Shift+Arrow to select text.\n\
                 Ctrl+A selects all.\n\
                 Ctrl+Z to undo, Ctrl+Y to redo.",
            )
            .with_placeholder("Type something...")
            .with_line_numbers(true)
            .with_style(Style::new().fg(theme::fg::PRIMARY))
            .with_focus(false);

        let mut state = Self {
            focus: FocusPanel::Form,
            form,
            form_state: RefCell::new(FormState::default()),
            search_input,
            password_input,
            textarea,
            status_text: "Ctrl+\u{2190}/\u{2192}: switch panels | Form: Tab/\u{2191}/\u{2193} navigate, Space toggle, Enter submit".into(),
        };
        state.apply_theme();
        state
    }

    pub fn apply_theme(&mut self) {
        self.form.set_style(Style::new().fg(theme::fg::PRIMARY));
        self.form
            .set_label_style(Style::new().fg(theme::fg::SECONDARY));
        self.form
            .set_focused_style(Style::new().fg(theme::accent::PRIMARY));
        self.form
            .set_error_style(Style::new().fg(theme::accent::ERROR));

        let input_style = Style::new()
            .fg(theme::fg::PRIMARY)
            .bg(theme::alpha::SURFACE);
        let placeholder_style = Style::new().fg(theme::fg::MUTED);
        self.search_input = self
            .search_input
            .clone()
            .with_style(input_style)
            .with_placeholder_style(placeholder_style);
        self.password_input = self
            .password_input
            .clone()
            .with_style(input_style)
            .with_placeholder_style(placeholder_style);
        self.textarea = self
            .textarea
            .clone()
            .with_style(
                Style::new()
                    .fg(theme::fg::PRIMARY)
                    .bg(theme::alpha::SURFACE),
            )
            .with_cursor_line_style(Style::new().bg(theme::alpha::HIGHLIGHT))
            .with_selection_style(
                Style::new()
                    .bg(theme::alpha::HIGHLIGHT)
                    .fg(theme::fg::PRIMARY),
            );
    }

    fn update_focus_states(&mut self) {
        self.search_input
            .set_focused(self.focus == FocusPanel::SearchInput);
        self.password_input
            .set_focused(self.focus == FocusPanel::PasswordInput);
        self.textarea
            .set_focused(self.focus == FocusPanel::TextEditor);
    }

    fn update_status(&mut self) {
        let form_state = self.form_state.borrow();
        self.status_text = match self.focus {
            FocusPanel::Form => {
                if form_state.submitted {
                    let data = self.form.data();
                    format!(
                        "Form submitted! Name={}",
                        data.get("Name")
                            .map_or_else(|| "(empty)".into(), |v| format!("{v:?}"))
                    )
                } else if form_state.cancelled {
                    "Form cancelled.".into()
                } else if let Some(field) = self.form.field(form_state.focused) {
                    format!(
                        "Editing: {} (field {}/{})",
                        field.label(),
                        form_state.focused + 1,
                        self.form.field_count()
                    )
                } else {
                    "Form panel active".into()
                }
            }
            FocusPanel::SearchInput => {
                format!(
                    "Search: \"{}\" ({} chars)",
                    self.search_input.value(),
                    self.search_input.value().len()
                )
            }
            FocusPanel::PasswordInput => {
                format!(
                    "Password: {} chars entered",
                    self.password_input.value().len()
                )
            }
            FocusPanel::TextEditor => {
                let cursor = self.textarea.cursor();
                format!(
                    "Editor: line {}, col {} | {} lines",
                    cursor.line + 1,
                    cursor.grapheme + 1,
                    self.textarea.line_count()
                )
            }
        };
    }

    fn render_form_panel(&self, frame: &mut Frame, area: Rect) {
        let border_style = theme::panel_border_style(
            self.focus == FocusPanel::Form,
            theme::screen_accent::FORMS_INPUT,
        );

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Registration Form")
            .title_alignment(Alignment::Center)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let chunks = Flex::vertical()
            .constraints([Constraint::Min(1), Constraint::Fixed(2)])
            .split(inner);

        let mut state = self.form_state.borrow_mut();
        StatefulWidget::render(&self.form, chunks[0], frame, &mut state);

        let hint = if state.submitted {
            "Form submitted successfully!"
        } else if state.cancelled {
            "Form cancelled"
        } else {
            "Tab: next | Enter: submit | Esc: cancel"
        };
        let hint_style = if state.submitted {
            Style::new().fg(theme::accent::SUCCESS)
        } else if state.cancelled {
            Style::new().fg(theme::accent::WARNING)
        } else {
            theme::muted()
        };
        Paragraph::new(hint)
            .style(hint_style)
            .render(chunks[1], frame);
    }

    fn render_input_panel(&self, frame: &mut Frame, area: Rect) {
        let input_focused =
            self.focus == FocusPanel::SearchInput || self.focus == FocusPanel::PasswordInput;
        let border_style =
            theme::panel_border_style(input_focused, theme::screen_accent::FORMS_INPUT);

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Text Inputs")
            .title_alignment(Alignment::Center)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let rows = Flex::vertical()
            .constraints([Constraint::Fixed(1), Constraint::Fixed(1)])
            .split(inner);

        // Search row
        if !rows[0].is_empty() {
            let cols = Flex::horizontal()
                .constraints([Constraint::Fixed(10), Constraint::Min(1)])
                .split(rows[0]);
            Paragraph::new("Search:")
                .style(Style::new().fg(theme::fg::SECONDARY))
                .render(cols[0], frame);
            Widget::render(&self.search_input, cols[1], frame);
        }

        // Password row
        if rows.len() > 1 && !rows[1].is_empty() {
            let cols = Flex::horizontal()
                .constraints([Constraint::Fixed(10), Constraint::Min(1)])
                .split(rows[1]);
            Paragraph::new("Password:")
                .style(Style::new().fg(theme::fg::SECONDARY))
                .render(cols[0], frame);
            Widget::render(&self.password_input, cols[1], frame);
        }
    }

    fn render_editor_panel(&self, frame: &mut Frame, area: Rect) {
        let border_style = theme::panel_border_style(
            self.focus == FocusPanel::TextEditor,
            theme::screen_accent::FORMS_INPUT,
        );

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Text Editor")
            .title_alignment(Alignment::Center)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        Widget::render(&self.textarea, inner, frame);
    }
}

impl Screen for FormsInput {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event
            && modifiers.contains(Modifiers::CTRL)
        {
            self.focus = self.focus.next();
            self.update_focus_states();
            self.update_status();
            return Cmd::None;
        }
        if let Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event
            && modifiers.contains(Modifiers::CTRL)
        {
            self.focus = self.focus.prev();
            self.update_focus_states();
            self.update_status();
            return Cmd::None;
        }

        match self.focus {
            FocusPanel::Form => {
                let mut state = self.form_state.borrow_mut();
                state.handle_event(&mut self.form, event);
            }
            FocusPanel::SearchInput => {
                self.search_input.handle_event(event);
            }
            FocusPanel::PasswordInput => {
                self.password_input.handle_event(event);
            }
            FocusPanel::TextEditor => {
                self.textarea.handle_event(event);
            }
        }

        self.update_status();
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let main_chunks = Flex::vertical()
            .constraints([Constraint::Min(1), Constraint::Fixed(1)])
            .split(area);

        let content_chunks = Flex::horizontal()
            .constraints([Constraint::Percentage(50.0), Constraint::Percentage(50.0)])
            .split(main_chunks[0]);

        self.render_form_panel(frame, content_chunks[0]);

        let right_chunks = Flex::vertical()
            .constraints([Constraint::Fixed(5), Constraint::Min(5)])
            .split(content_chunks[1]);

        self.render_input_panel(frame, right_chunks[0]);
        self.render_editor_panel(frame, right_chunks[1]);

        Paragraph::new(&*self.status_text)
            .style(Style::new().fg(theme::fg::MUTED).bg(theme::alpha::SURFACE))
            .render(main_chunks[1], frame);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "Ctrl+\u{2190}/\u{2192}",
                action: "Switch panel",
            },
            HelpEntry {
                key: "Tab/S-Tab",
                action: "Navigate form fields",
            },
            HelpEntry {
                key: "Space",
                action: "Toggle checkbox",
            },
            HelpEntry {
                key: "\u{2191}/\u{2193}",
                action: "Radio/select/number",
            },
            HelpEntry {
                key: "Enter",
                action: "Submit form",
            },
            HelpEntry {
                key: "Esc",
                action: "Cancel form",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Forms and Input"
    }

    fn tab_label(&self) -> &'static str {
        "Forms"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: Modifiers::empty(),
            kind: KeyEventKind::Press,
        })
    }

    fn ctrl_press(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: Modifiers::CTRL,
            kind: KeyEventKind::Press,
        })
    }

    #[test]
    fn initial_state() {
        let screen = FormsInput::new();
        assert_eq!(screen.focus, FocusPanel::Form);
        assert_eq!(screen.title(), "Forms and Input");
        assert_eq!(screen.tab_label(), "Forms");
    }

    #[test]
    fn focus_panel_cycles_forward() {
        assert_eq!(FocusPanel::Form.next(), FocusPanel::SearchInput);
        assert_eq!(FocusPanel::SearchInput.next(), FocusPanel::PasswordInput);
        assert_eq!(FocusPanel::PasswordInput.next(), FocusPanel::TextEditor);
        assert_eq!(FocusPanel::TextEditor.next(), FocusPanel::Form);
    }

    #[test]
    fn focus_panel_cycles_backward() {
        assert_eq!(FocusPanel::Form.prev(), FocusPanel::TextEditor);
        assert_eq!(FocusPanel::TextEditor.prev(), FocusPanel::PasswordInput);
        assert_eq!(FocusPanel::PasswordInput.prev(), FocusPanel::SearchInput);
        assert_eq!(FocusPanel::SearchInput.prev(), FocusPanel::Form);
    }

    #[test]
    fn ctrl_right_switches_panel() {
        let mut screen = FormsInput::new();
        screen.update(&ctrl_press(KeyCode::Right));
        assert_eq!(screen.focus, FocusPanel::SearchInput);
        screen.update(&ctrl_press(KeyCode::Right));
        assert_eq!(screen.focus, FocusPanel::PasswordInput);
    }

    #[test]
    fn ctrl_left_switches_panel_back() {
        let mut screen = FormsInput::new();
        screen.update(&ctrl_press(KeyCode::Left));
        assert_eq!(screen.focus, FocusPanel::TextEditor);
    }

    #[test]
    fn form_has_six_fields() {
        let screen = FormsInput::new();
        assert_eq!(screen.form.field_count(), 6);
    }

    #[test]
    fn form_tab_navigates_fields() {
        let mut screen = FormsInput::new();
        assert_eq!(screen.form_state.borrow().focused, 0);
        screen.update(&press(KeyCode::Tab));
        assert_eq!(screen.form_state.borrow().focused, 1);
    }

    #[test]
    fn search_input_receives_chars() {
        let mut screen = FormsInput::new();
        // Switch to search input
        screen.update(&ctrl_press(KeyCode::Right));
        assert_eq!(screen.focus, FocusPanel::SearchInput);
        // Type a character
        screen.update(&press(KeyCode::Char('h')));
        assert_eq!(screen.search_input.value(), "h");
    }

    #[test]
    fn textarea_has_content() {
        let screen = FormsInput::new();
        assert!(!screen.textarea.is_empty());
        assert!(screen.textarea.line_count() > 1);
    }

    #[test]
    fn keybindings_non_empty() {
        let screen = FormsInput::new();
        assert!(!screen.keybindings().is_empty());
    }
}
