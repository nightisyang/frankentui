#![forbid(unsafe_code)]
#![allow(dead_code)] // Screen stubs and theme constants will be used as downstream beads are implemented

//! FrankenTUI Demo Showcase
//!
//! A comprehensive demo application that demonstrates every feature, widget,
//! layout mode, style, event type, and capability of the FrankenTUI framework.
//!
//! # Running
//!
//! ```sh
//! cargo run -p ftui-demo-showcase
//! ```
//!
//! # Controls
//!
//! - Tab / 1-9: Switch between screens
//! - ?: Toggle help overlay
//! - q / Ctrl+C: Quit

mod data;
mod screens;
mod theme;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::{Cmd, Model, Program, ProgramConfig, ScreenMode};
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::status_line::{StatusItem, StatusLine};

#[allow(unused_imports)]
use screens::Screen as _;

/// Which screen is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Dashboard,
}

impl Screen {
    fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
        }
    }
}

/// Top-level application message.
enum Msg {
    Event(Event),
}

impl From<Event> for Msg {
    fn from(event: Event) -> Self {
        Self::Event(event)
    }
}

/// Top-level application state.
struct DemoShowcase {
    current_screen: Screen,
    show_help: bool,
}

impl DemoShowcase {
    fn new() -> Self {
        Self {
            current_screen: Screen::Dashboard,
            show_help: false,
        }
    }
}

impl Model for DemoShowcase {
    type Message = Msg;

    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
        match msg {
            Msg::Event(Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: Modifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            })) => Cmd::Quit,

            Msg::Event(Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: Modifiers::CTRL,
                kind: KeyEventKind::Press,
                ..
            })) => Cmd::Quit,

            Msg::Event(Event::Key(KeyEvent {
                code: KeyCode::Char('?'),
                kind: KeyEventKind::Press,
                ..
            })) => {
                self.show_help = !self.show_help;
                Cmd::None
            }

            _ => Cmd::None,
        }
    }

    fn view(&self, frame: &mut Frame) {
        let area = Rect::from_size(frame.buffer.width(), frame.buffer.height());

        // Top-level layout: tab bar (1 row) + content + status bar (1 row)
        let chunks = Flex::vertical()
            .constraints([
                Constraint::Fixed(1),
                Constraint::Min(1),
                Constraint::Fixed(1),
            ])
            .split(area);

        // Tab bar
        let tab_text = format!(" [1] {} ", self.current_screen.title());
        let tab_bar = Paragraph::new(tab_text).style(theme::tab_bar());
        tab_bar.render(chunks[0], frame);

        // Content area
        let content_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(self.current_screen.title())
            .title_alignment(Alignment::Center)
            .style(theme::content_border());

        let inner = content_block.inner(chunks[1]);
        content_block.render(chunks[1], frame);

        // Placeholder content
        let welcome = Paragraph::new(
            "Welcome to the FrankenTUI Demo Showcase!\n\n\
             This application will demonstrate every feature of the framework.\n\n\
             Press ? for help, q to quit.",
        )
        .style(theme::body());
        welcome.render(inner, frame);

        // Help overlay
        if self.show_help {
            self.render_help_overlay(frame, area);
        }

        // Status bar
        let status = StatusLine::new()
            .left(StatusItem::text("FrankenTUI Demo"))
            .right(StatusItem::key_hint("?", "Help"))
            .right(StatusItem::key_hint("q", "Quit"))
            .style(theme::status_bar());
        status.render(chunks[2], frame);
    }
}

impl DemoShowcase {
    fn render_help_overlay(&self, frame: &mut Frame, area: Rect) {
        let overlay_width = 50u16.min(area.width.saturating_sub(4));
        let overlay_height = 12u16.min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
        let y = area.y + (area.height.saturating_sub(overlay_height)) / 2;
        let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

        let help_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .title("Help")
            .title_alignment(Alignment::Center)
            .style(theme::help_overlay());

        let help_inner = help_block.inner(overlay_area);
        help_block.render(overlay_area, frame);

        let help_text = Paragraph::new(
            "Keybindings:\n\n\
             Tab / 1-9  Switch screens\n\
             ?          Toggle help\n\
             q          Quit\n\
             Ctrl+C     Quit",
        )
        .style(theme::body());
        help_text.render(help_inner, frame);
    }
}

fn main() {
    let model = DemoShowcase::new();
    let config = ProgramConfig {
        screen_mode: ScreenMode::AltScreen,
        ..ProgramConfig::default()
    };
    match Program::with_config(model, config) {
        Ok(mut program) => {
            if let Err(e) = program.run() {
                eprintln!("Runtime error: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            std::process::exit(1);
        }
    }
}
