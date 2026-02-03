#![forbid(unsafe_code)]

//! Main application model, message routing, and screen navigation.
//!
//! This module contains the top-level [`AppModel`] that implements the Elm
//! architecture via [`Model`]. It manages all demo screens, routes events,
//! handles global keybindings, and renders the chrome (tab bar, status bar,
//! help/debug overlays).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_runtime::{Cmd, Every, Model, Subscription};
use ftui_style::Style;
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::command_palette::{CommandPalette, PaletteAction};
use ftui_widgets::error_boundary::FallbackWidget;
use ftui_widgets::paragraph::Paragraph;

use crate::screens;
use crate::theme;

// ---------------------------------------------------------------------------
// ScreenId
// ---------------------------------------------------------------------------

/// Identifies which demo screen is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    /// System dashboard with live widgets.
    Dashboard,
    /// Complete Shakespeare works with search.
    Shakespeare,
    /// SQLite source with syntax highlighting.
    CodeExplorer,
    /// Showcase of every widget type.
    WidgetGallery,
    /// Interactive constraint solver demo.
    LayoutLab,
    /// Form widgets and text editing.
    FormsInput,
    /// Charts, canvas, and structured data.
    DataViz,
    /// File system navigation and preview.
    FileBrowser,
    /// Diagnostics, timers, and spinners.
    AdvancedFeatures,
    /// Input macro recorder and scenario runner.
    MacroRecorder,
    /// Virtualized list and stress testing.
    Performance,
    /// Markdown rendering and typography.
    MarkdownRichText,
    /// Mind-blowing visual effects with braille.
    VisualEffects,
}

impl ScreenId {
    /// All screens in display order.
    pub const ALL: &[ScreenId] = &[
        Self::Dashboard,
        Self::Shakespeare,
        Self::CodeExplorer,
        Self::WidgetGallery,
        Self::LayoutLab,
        Self::FormsInput,
        Self::DataViz,
        Self::FileBrowser,
        Self::AdvancedFeatures,
        Self::Performance,
        Self::MacroRecorder,
        Self::MarkdownRichText,
        Self::VisualEffects,
    ];

    /// 0-based index in the ALL array.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    /// Next screen (wraps around).
    pub fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }

    /// Previous screen (wraps around).
    pub fn prev(self) -> Self {
        let i = (self.index() + Self::ALL.len() - 1) % Self::ALL.len();
        Self::ALL[i]
    }

    /// Title for the tab bar.
    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Shakespeare => "Shakespeare",
            Self::CodeExplorer => "Code Explorer",
            Self::WidgetGallery => "Widget Gallery",
            Self::LayoutLab => "Layout Lab",
            Self::FormsInput => "Forms & Input",
            Self::DataViz => "Data Viz",
            Self::FileBrowser => "File Browser",
            Self::AdvancedFeatures => "Advanced",
            Self::MacroRecorder => "Macro Recorder",
            Self::Performance => "Performance",
            Self::MarkdownRichText => "Markdown",
            Self::VisualEffects => "Visual Effects",
        }
    }

    /// Short label for the tab (max ~12 chars).
    pub fn tab_label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dash",
            Self::Shakespeare => "Shakes",
            Self::CodeExplorer => "Code",
            Self::WidgetGallery => "Widgets",
            Self::LayoutLab => "Layout",
            Self::FormsInput => "Forms",
            Self::DataViz => "DataViz",
            Self::FileBrowser => "Files",
            Self::AdvancedFeatures => "Adv",
            Self::MacroRecorder => "Macro",
            Self::Performance => "Perf",
            Self::MarkdownRichText => "MD",
            Self::VisualEffects => "VFX",
        }
    }

    /// Widget name used in error boundary fallback messages.
    fn widget_name(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Shakespeare => "Shakespeare",
            Self::CodeExplorer => "CodeExplorer",
            Self::WidgetGallery => "WidgetGallery",
            Self::LayoutLab => "LayoutLab",
            Self::FormsInput => "FormsInput",
            Self::DataViz => "DataViz",
            Self::FileBrowser => "FileBrowser",
            Self::AdvancedFeatures => "AdvancedFeatures",
            Self::MacroRecorder => "MacroRecorder",
            Self::Performance => "Performance",
            Self::MarkdownRichText => "MarkdownRichText",
            Self::VisualEffects => "VisualEffects",
        }
    }

    /// Map number key to screen: '1'..='9' -> first 9, '0' -> 10th.
    pub fn from_number_key(ch: char) -> Option<Self> {
        let idx = match ch {
            '1'..='9' => (ch as usize) - ('1' as usize),
            '0' => 9,
            _ => return None,
        };
        Self::ALL.get(idx).copied()
    }
}

// ---------------------------------------------------------------------------
// ScreenStates
// ---------------------------------------------------------------------------

/// Holds the state for every screen.
#[derive(Default)]
pub struct ScreenStates {
    /// Dashboard screen state.
    pub dashboard: screens::dashboard::Dashboard,
    /// Shakespeare library screen state.
    pub shakespeare: screens::shakespeare::Shakespeare,
    /// Code explorer screen state.
    pub code_explorer: screens::code_explorer::CodeExplorer,
    /// Widget gallery screen state.
    pub widget_gallery: screens::widget_gallery::WidgetGallery,
    /// Layout laboratory screen state.
    pub layout_lab: screens::layout_lab::LayoutLab,
    /// Forms and input screen state.
    pub forms_input: screens::forms_input::FormsInput,
    /// Data visualization screen state.
    pub data_viz: screens::data_viz::DataViz,
    /// File browser screen state.
    pub file_browser: screens::file_browser::FileBrowser,
    /// Advanced features screen state.
    pub advanced_features: screens::advanced_features::AdvancedFeatures,
    /// Macro recorder screen state.
    pub macro_recorder: screens::macro_recorder::MacroRecorderScreen,
    /// Performance stress test screen state.
    pub performance: screens::performance::Performance,
    /// Markdown and rich text screen state.
    pub markdown_rich_text: screens::markdown_rich_text::MarkdownRichText,
    /// Visual effects screen state.
    pub visual_effects: screens::visual_effects::VisualEffectsScreen,
    /// Tracks whether each screen has errored during rendering.
    /// Indexed by `ScreenId::index()`.
    screen_errors: [Option<String>; 13],
}

impl ScreenStates {
    /// Forward an event to the screen identified by `id`.
    fn update(&mut self, id: ScreenId, event: &Event) {
        use screens::Screen;
        match id {
            ScreenId::Dashboard => {
                self.dashboard.update(event);
            }
            ScreenId::Shakespeare => {
                self.shakespeare.update(event);
            }
            ScreenId::CodeExplorer => {
                self.code_explorer.update(event);
            }
            ScreenId::WidgetGallery => {
                self.widget_gallery.update(event);
            }
            ScreenId::LayoutLab => {
                self.layout_lab.update(event);
            }
            ScreenId::FormsInput => {
                self.forms_input.update(event);
            }
            ScreenId::DataViz => {
                self.data_viz.update(event);
            }
            ScreenId::FileBrowser => {
                self.file_browser.update(event);
            }
            ScreenId::AdvancedFeatures => {
                self.advanced_features.update(event);
            }
            ScreenId::MacroRecorder => {
                self.macro_recorder.update(event);
            }
            ScreenId::Performance => {
                self.performance.update(event);
            }
            ScreenId::MarkdownRichText => {
                self.markdown_rich_text.update(event);
            }
            ScreenId::VisualEffects => {
                self.visual_effects.update(event);
            }
        }
    }

    /// Forward a tick to all screens (so they can update animations/data).
    fn tick(&mut self, tick_count: u64) {
        use screens::Screen;
        self.dashboard.tick(tick_count);
        self.shakespeare.tick(tick_count);
        self.code_explorer.tick(tick_count);
        self.widget_gallery.tick(tick_count);
        self.layout_lab.tick(tick_count);
        self.forms_input.tick(tick_count);
        self.data_viz.tick(tick_count);
        self.file_browser.tick(tick_count);
        self.advanced_features.tick(tick_count);
        self.macro_recorder.tick(tick_count);
        self.performance.tick(tick_count);
        self.markdown_rich_text.tick(tick_count);
        self.visual_effects.tick(tick_count);
    }

    fn apply_theme(&mut self) {
        self.dashboard.apply_theme();
        self.file_browser.apply_theme();
        self.code_explorer.apply_theme();
        self.forms_input.apply_theme();
        self.shakespeare.apply_theme();
        self.markdown_rich_text.apply_theme();
    }

    /// Render the screen identified by `id` into the given area.
    ///
    /// Wraps each screen's `view()` call in a panic boundary. If a screen
    /// panics during rendering, the error is captured and a
    /// [`FallbackWidget`] is shown instead of crashing the application.
    fn view(&self, id: ScreenId, frame: &mut Frame, area: Rect) {
        let idx = id.index();

        // If this screen previously errored, show the cached fallback.
        if let Some(msg) = &self.screen_errors[idx] {
            FallbackWidget::from_message(msg.clone(), id.widget_name()).render(area, frame);
            return;
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            use screens::Screen;
            match id {
                ScreenId::Dashboard => self.dashboard.view(frame, area),
                ScreenId::Shakespeare => self.shakespeare.view(frame, area),
                ScreenId::CodeExplorer => self.code_explorer.view(frame, area),
                ScreenId::WidgetGallery => self.widget_gallery.view(frame, area),
                ScreenId::LayoutLab => self.layout_lab.view(frame, area),
                ScreenId::FormsInput => self.forms_input.view(frame, area),
                ScreenId::DataViz => self.data_viz.view(frame, area),
                ScreenId::FileBrowser => self.file_browser.view(frame, area),
                ScreenId::AdvancedFeatures => self.advanced_features.view(frame, area),
                ScreenId::MacroRecorder => self.macro_recorder.view(frame, area),
                ScreenId::Performance => self.performance.view(frame, area),
                ScreenId::MarkdownRichText => self.markdown_rich_text.view(frame, area),
                ScreenId::VisualEffects => self.visual_effects.view(frame, area),
            }
        }));

        if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            FallbackWidget::from_message(&msg, id.widget_name()).render(area, frame);
            // Note: We can't write to self.screen_errors here because view()
            // takes &self. The error boundary still protects the app from
            // crashing — it just re-catches on each render.
        }
    }

    /// Reset the error state for a screen (e.g., after user presses 'R' to retry).
    fn clear_error(&mut self, id: ScreenId) {
        self.screen_errors[id.index()] = None;
    }

    /// Returns true if the screen has a cached error.
    fn has_error(&self, id: ScreenId) -> bool {
        self.screen_errors[id.index()].is_some()
    }
}

// ---------------------------------------------------------------------------
// AppMsg
// ---------------------------------------------------------------------------

/// Top-level application message.
pub enum AppMsg {
    /// A raw terminal event forwarded to the current screen.
    ScreenEvent(Event),
    /// Switch to a specific screen.
    SwitchScreen(ScreenId),
    /// Advance to the next screen tab.
    NextScreen,
    /// Go back to the previous screen tab.
    PrevScreen,
    /// Toggle the help overlay.
    ToggleHelp,
    /// Toggle the debug overlay.
    ToggleDebug,
    /// Cycle the active color theme.
    CycleTheme,
    /// Periodic tick for animations and data updates.
    Tick,
    /// Terminal resize.
    Resize {
        /// New terminal width.
        width: u16,
        /// New terminal height.
        height: u16,
    },
    /// Quit the application.
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventSource {
    User,
    Playback,
}

impl From<Event> for AppMsg {
    fn from(event: Event) -> Self {
        if let Event::Resize { width, height } = event {
            return Self::Resize { width, height };
        }

        Self::ScreenEvent(event)
    }
}

// ---------------------------------------------------------------------------
// AppModel
// ---------------------------------------------------------------------------

/// Top-level application state.
///
/// Implements the Elm architecture: all state lives here, messages drive
/// transitions, and `view()` is a pure function of state.
pub struct AppModel {
    /// Currently displayed screen.
    pub current_screen: ScreenId,
    /// Per-screen state storage.
    pub screens: ScreenStates,
    /// Whether the help overlay is visible.
    pub help_visible: bool,
    /// Whether the debug overlay is visible.
    pub debug_visible: bool,
    /// Command palette for instant action search (Ctrl+K).
    pub command_palette: CommandPalette,
    /// Global tick counter (incremented every 100ms).
    pub tick_count: u64,
    /// Total frames rendered.
    pub frame_count: u64,
    /// Current terminal width.
    pub terminal_width: u16,
    /// Current terminal height.
    pub terminal_height: u16,
    /// Auto-exit after this many milliseconds (0 = disabled).
    pub exit_after_ms: u64,
}

impl Default for AppModel {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModel {
    /// Create a new application model with default state.
    pub fn new() -> Self {
        theme::set_theme(theme::ThemeId::CyberpunkAurora);
        let mut palette = CommandPalette::new().with_max_visible(12);
        Self::register_palette_actions(&mut palette);
        Self {
            current_screen: ScreenId::Dashboard,
            screens: ScreenStates::default(),
            help_visible: false,
            debug_visible: false,
            command_palette: palette,
            tick_count: 0,
            frame_count: 0,
            terminal_width: 0,
            terminal_height: 0,
            exit_after_ms: 0,
        }
    }

    /// Register all palette actions (screens + global commands).
    fn register_palette_actions(palette: &mut CommandPalette) {
        use ftui_widgets::command_palette::ActionItem;

        // Screen navigation actions
        for &id in ScreenId::ALL {
            let action_id = format!("screen:{}", id.title().to_lowercase().replace(' ', "_"));
            palette.register_action(
                ActionItem::new(&action_id, format!("Go to {}", id.title()))
                    .with_description(format!("Switch to the {} screen", id.title()))
                    .with_tags(&["screen", "navigate"])
                    .with_category("Navigate"),
            );
        }

        // Global commands
        palette.register_action(
            ActionItem::new("cmd:toggle_help", "Toggle Help")
                .with_description("Show or hide the keyboard shortcuts overlay")
                .with_tags(&["help", "shortcuts"])
                .with_category("View"),
        );
        palette.register_action(
            ActionItem::new("cmd:toggle_debug", "Toggle Debug Overlay")
                .with_description("Show or hide the debug information panel")
                .with_tags(&["debug", "info"])
                .with_category("View"),
        );
        palette.register_action(
            ActionItem::new("cmd:cycle_theme", "Cycle Theme")
                .with_description("Switch to the next color theme")
                .with_tags(&["theme", "colors", "appearance"])
                .with_category("View"),
        );
        palette.register_action(
            ActionItem::new("cmd:quit", "Quit")
                .with_description("Exit the application")
                .with_tags(&["exit", "close"])
                .with_category("App"),
        );
    }

    fn handle_msg(&mut self, msg: AppMsg, source: EventSource) -> Cmd<AppMsg> {
        match msg {
            AppMsg::Quit => Cmd::Quit,

            AppMsg::SwitchScreen(id) => {
                self.current_screen = id;
                Cmd::None
            }

            AppMsg::NextScreen => {
                self.current_screen = self.current_screen.next();
                Cmd::None
            }

            AppMsg::PrevScreen => {
                self.current_screen = self.current_screen.prev();
                Cmd::None
            }

            AppMsg::ToggleHelp => {
                self.help_visible = !self.help_visible;
                Cmd::None
            }

            AppMsg::ToggleDebug => {
                self.debug_visible = !self.debug_visible;
                Cmd::None
            }

            AppMsg::CycleTheme => {
                theme::cycle_theme();
                self.screens.apply_theme();
                Cmd::None
            }

            AppMsg::Tick => {
                self.tick_count += 1;
                self.screens.tick(self.tick_count);
                let playback_events = self.screens.macro_recorder.drain_playback_events();
                for event in playback_events {
                    let cmd = self.handle_msg(AppMsg::from(event), EventSource::Playback);
                    if matches!(cmd, Cmd::Quit) {
                        return Cmd::Quit;
                    }
                }
                Cmd::None
            }

            AppMsg::Resize { width, height } => {
                self.terminal_width = width;
                self.terminal_height = height;
                self.screens.macro_recorder.set_terminal_size(width, height);
                Cmd::None
            }

            AppMsg::ScreenEvent(event) => {
                if source == EventSource::User {
                    let filter_controls = self.current_screen == ScreenId::MacroRecorder;
                    self.screens
                        .macro_recorder
                        .record_event(&event, filter_controls);
                }

                // When the command palette is visible, route events to it first.
                if self.command_palette.is_visible() {
                    if let Some(action) = self.command_palette.handle_event(&event) {
                        return self.execute_palette_action(action);
                    }
                    return Cmd::None;
                }

                if let Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind: KeyEventKind::Press,
                    ..
                }) = &event
                {
                    match (*code, *modifiers) {
                        // Quit
                        (KeyCode::Char('q'), Modifiers::NONE) => return Cmd::Quit,
                        (KeyCode::Char('c'), Modifiers::CTRL) => return Cmd::Quit,
                        // Command palette (Ctrl+K)
                        (KeyCode::Char('k'), Modifiers::CTRL) => {
                            self.command_palette.open();
                            return Cmd::None;
                        }
                        // Help
                        (KeyCode::Char('?'), _) => {
                            self.help_visible = !self.help_visible;
                            return Cmd::None;
                        }
                        // Debug
                        (KeyCode::F(12), _) => {
                            self.debug_visible = !self.debug_visible;
                            return Cmd::None;
                        }
                        // Theme cycling
                        (KeyCode::Char('t'), Modifiers::CTRL) => {
                            theme::cycle_theme();
                            self.screens.apply_theme();
                            return Cmd::None;
                        }
                        // Tab cycling (Tab/BackTab, or Shift+H/Shift+L for Vim users)
                        (KeyCode::Tab, Modifiers::NONE) => {
                            self.current_screen = self.current_screen.next();
                            return Cmd::None;
                        }
                        (KeyCode::BackTab, _) => {
                            self.current_screen = self.current_screen.prev();
                            return Cmd::None;
                        }
                        (KeyCode::Char('L'), Modifiers::SHIFT) => {
                            self.current_screen = self.current_screen.next();
                            return Cmd::None;
                        }
                        (KeyCode::Char('H'), Modifiers::SHIFT) => {
                            self.current_screen = self.current_screen.prev();
                            return Cmd::None;
                        }
                        // Number keys for direct screen access
                        (KeyCode::Char(ch @ '0'..='9'), Modifiers::NONE) => {
                            if let Some(id) = ScreenId::from_number_key(ch) {
                                self.current_screen = id;
                                return Cmd::None;
                            }
                        }
                        _ => {}
                    }
                }

                // Handle 'R' key to retry errored screens
                if self.screens.has_error(self.current_screen)
                    && let Event::Key(KeyEvent {
                        code: KeyCode::Char('r' | 'R'),
                        kind: KeyEventKind::Press,
                        ..
                    }) = &event
                {
                    self.screens.clear_error(self.current_screen);
                    return Cmd::None;
                }
                self.screens.update(self.current_screen, &event);
                Cmd::None
            }
        }
    }
}

impl Model for AppModel {
    type Message = AppMsg;

    fn init(&mut self) -> Cmd<Self::Message> {
        if self.exit_after_ms > 0 {
            let ms = self.exit_after_ms;
            Cmd::Task(Box::new(move || {
                std::thread::sleep(Duration::from_millis(ms));
                AppMsg::Quit
            }))
        } else {
            Cmd::None
        }
    }

    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
        self.handle_msg(msg, EventSource::User)
    }

    fn view(&self, frame: &mut Frame) {
        let area = Rect::from_size(frame.buffer.width(), frame.buffer.height());

        frame
            .buffer
            .fill(area, Cell::default().with_bg(theme::bg::DEEP.into()));

        // Top-level layout: tab bar (1 row) + content + status bar (1 row)
        let chunks = Flex::vertical()
            .constraints([
                Constraint::Fixed(1),
                Constraint::Min(1),
                Constraint::Fixed(1),
            ])
            .split(area);

        // Tab bar (chrome module)
        crate::chrome::render_tab_bar(self.current_screen, frame, chunks[0]);

        // Content area with border
        let content_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(self.current_screen.title())
            .title_alignment(Alignment::Center)
            .style(theme::content_border());

        let inner = content_block.inner(chunks[1]);
        content_block.render(chunks[1], frame);

        // Screen content (wrapped in error boundary)
        self.screens.view(self.current_screen, frame, inner);

        // Help overlay (chrome module)
        if self.help_visible {
            let bindings = self.current_screen_keybindings();
            crate::chrome::render_help_overlay(self.current_screen, &bindings, frame, area);
        }

        // Debug overlay
        if self.debug_visible {
            self.render_debug_overlay(frame, area);
        }

        // Command palette overlay (topmost layer)
        if self.command_palette.is_visible() {
            self.command_palette.render(area, frame);
        }

        // Status bar (chrome module)
        let status_state = crate::chrome::StatusBarState {
            screen_title: self.current_screen.title(),
            screen_index: self.current_screen.index(),
            screen_count: ScreenId::ALL.len(),
            tick_count: self.tick_count,
            frame_count: self.frame_count,
            terminal_width: self.terminal_width,
            terminal_height: self.terminal_height,
            theme_name: theme::current_theme_name(),
        };
        crate::chrome::render_status_bar(&status_state, frame, chunks[2]);
    }

    fn subscriptions(&self) -> Vec<Box<dyn Subscription<Self::Message>>> {
        vec![Box::new(Every::new(Duration::from_millis(100), || {
            AppMsg::Tick
        }))]
    }
}

impl AppModel {
    /// Gather keybindings from the current screen for the help overlay.
    fn current_screen_keybindings(&self) -> Vec<crate::chrome::HelpEntry> {
        use screens::Screen;
        let entries = match self.current_screen {
            ScreenId::Dashboard => self.screens.dashboard.keybindings(),
            ScreenId::Shakespeare => self.screens.shakespeare.keybindings(),
            ScreenId::CodeExplorer => self.screens.code_explorer.keybindings(),
            ScreenId::WidgetGallery => self.screens.widget_gallery.keybindings(),
            ScreenId::LayoutLab => self.screens.layout_lab.keybindings(),
            ScreenId::FormsInput => self.screens.forms_input.keybindings(),
            ScreenId::DataViz => self.screens.data_viz.keybindings(),
            ScreenId::FileBrowser => self.screens.file_browser.keybindings(),
            ScreenId::AdvancedFeatures => self.screens.advanced_features.keybindings(),
            ScreenId::MacroRecorder => self.screens.macro_recorder.keybindings(),
            ScreenId::Performance => self.screens.performance.keybindings(),
            ScreenId::MarkdownRichText => self.screens.markdown_rich_text.keybindings(),
            ScreenId::VisualEffects => self.screens.visual_effects.keybindings(),
        };
        // Convert screens::HelpEntry to chrome::HelpEntry (same struct, different module).
        entries
            .into_iter()
            .map(|e| crate::chrome::HelpEntry {
                key: e.key,
                action: e.action,
            })
            .collect()
    }

    /// Execute an action returned by the command palette.
    fn execute_palette_action(&mut self, action: PaletteAction) -> Cmd<AppMsg> {
        match action {
            PaletteAction::Dismiss => Cmd::None,
            PaletteAction::Execute(id) => {
                // Screen navigation: "screen:<name>"
                if let Some(screen_name) = id.strip_prefix("screen:") {
                    for &sid in ScreenId::ALL {
                        let expected = sid.title().to_lowercase().replace(' ', "_");
                        if expected == screen_name {
                            self.current_screen = sid;
                            return Cmd::None;
                        }
                    }
                }
                // Global commands
                match id.as_str() {
                    "cmd:toggle_help" => {
                        self.help_visible = !self.help_visible;
                    }
                    "cmd:toggle_debug" => {
                        self.debug_visible = !self.debug_visible;
                    }
                    "cmd:cycle_theme" => {
                        theme::cycle_theme();
                        self.screens.apply_theme();
                    }
                    "cmd:quit" => return Cmd::Quit,
                    _ => {}
                }
                Cmd::None
            }
        }
    }

    /// Render the debug overlay in the top-right corner.
    fn render_debug_overlay(&self, frame: &mut Frame, area: Rect) {
        let overlay_width = 40u16.min(area.width.saturating_sub(4));
        let overlay_height = 8u16.min(area.height.saturating_sub(4));
        let x = area.x + area.width.saturating_sub(overlay_width).saturating_sub(1);
        let y = area.y + 1;
        let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

        let debug_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Debug")
            .title_alignment(Alignment::Center)
            .style(theme::help_overlay());

        let debug_inner = debug_block.inner(overlay_area);
        debug_block.render(overlay_area, frame);

        let debug_text = format!(
            "Tick: {}\nFrame: {}\nScreen: {:?}\nSize: {}x{}",
            self.tick_count,
            self.frame_count,
            self.current_screen,
            self.terminal_width,
            self.terminal_height,
        );
        Paragraph::new(debug_text)
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(debug_inner, frame);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_render::grapheme_pool::GraphemePool;

    #[test]
    fn switch_screen_changes_current() {
        let mut app = AppModel::new();
        assert_eq!(app.current_screen, ScreenId::Dashboard);

        app.update(AppMsg::SwitchScreen(ScreenId::Shakespeare));
        assert_eq!(app.current_screen, ScreenId::Shakespeare);

        app.update(AppMsg::SwitchScreen(ScreenId::Performance));
        assert_eq!(app.current_screen, ScreenId::Performance);
    }

    #[test]
    fn next_screen_advances() {
        let mut app = AppModel::new();
        assert_eq!(app.current_screen, ScreenId::Dashboard);

        app.update(AppMsg::NextScreen);
        assert_eq!(app.current_screen, ScreenId::Shakespeare);

        app.update(AppMsg::NextScreen);
        assert_eq!(app.current_screen, ScreenId::CodeExplorer);
    }

    #[test]
    fn prev_screen_goes_back() {
        let mut app = AppModel::new();
        assert_eq!(app.current_screen, ScreenId::Dashboard);

        app.update(AppMsg::PrevScreen);
        assert_eq!(app.current_screen, ScreenId::VisualEffects);
    }

    #[test]
    fn tick_increments_count() {
        let mut app = AppModel::new();
        assert_eq!(app.tick_count, 0);

        app.update(AppMsg::Tick);
        assert_eq!(app.tick_count, 1);

        for _ in 0..10 {
            app.update(AppMsg::Tick);
        }
        assert_eq!(app.tick_count, 11);
    }

    #[test]
    fn toggle_help() {
        let mut app = AppModel::new();
        assert!(!app.help_visible);

        app.update(AppMsg::ToggleHelp);
        assert!(app.help_visible);

        app.update(AppMsg::ToggleHelp);
        assert!(!app.help_visible);
    }

    #[test]
    fn toggle_debug() {
        let mut app = AppModel::new();
        assert!(!app.debug_visible);

        app.update(AppMsg::ToggleDebug);
        assert!(app.debug_visible);

        app.update(AppMsg::ToggleDebug);
        assert!(!app.debug_visible);
    }

    #[test]
    fn resize_updates_dimensions() {
        let mut app = AppModel::new();
        app.update(AppMsg::Resize {
            width: 120,
            height: 40,
        });
        assert_eq!(app.terminal_width, 120);
        assert_eq!(app.terminal_height, 40);
    }

    #[test]
    fn number_keys_map_to_screens() {
        assert_eq!(ScreenId::from_number_key('1'), Some(ScreenId::Dashboard));
        assert_eq!(ScreenId::from_number_key('2'), Some(ScreenId::Shakespeare));
        assert_eq!(ScreenId::from_number_key('3'), Some(ScreenId::CodeExplorer));
        assert_eq!(
            ScreenId::from_number_key('4'),
            Some(ScreenId::WidgetGallery)
        );
        assert_eq!(ScreenId::from_number_key('5'), Some(ScreenId::LayoutLab));
        assert_eq!(ScreenId::from_number_key('6'), Some(ScreenId::FormsInput));
        assert_eq!(ScreenId::from_number_key('7'), Some(ScreenId::DataViz));
        assert_eq!(ScreenId::from_number_key('8'), Some(ScreenId::FileBrowser));
        assert_eq!(
            ScreenId::from_number_key('9'),
            Some(ScreenId::AdvancedFeatures)
        );
        assert_eq!(ScreenId::from_number_key('0'), Some(ScreenId::Performance));
        // No direct key for screens after the first 10
        assert_eq!(ScreenId::from_number_key('a'), None);
    }

    #[test]
    fn screen_next_prev_wraps() {
        assert_eq!(ScreenId::Dashboard.next(), ScreenId::Shakespeare);
        assert_eq!(ScreenId::VisualEffects.next(), ScreenId::Dashboard);
        assert_eq!(ScreenId::Dashboard.prev(), ScreenId::VisualEffects);
        assert_eq!(ScreenId::Shakespeare.prev(), ScreenId::Dashboard);
    }

    #[test]
    fn quit_returns_quit_cmd() {
        let mut app = AppModel::new();
        let cmd = app.update(AppMsg::Quit);
        assert!(matches!(cmd, Cmd::Quit));
    }

    #[test]
    fn quit_key_triggers_quit() {
        let mut app = AppModel::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        let cmd = app.update(AppMsg::from(event));
        assert!(matches!(cmd, Cmd::Quit));
    }

    #[test]
    fn help_key_toggles_help() {
        let mut app = AppModel::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(event));
        assert!(app.help_visible);
    }

    #[test]
    fn tab_advances_screen() {
        let mut app = AppModel::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(event));
        assert_eq!(app.current_screen, ScreenId::Shakespeare);
    }

    #[test]
    fn backtab_moves_previous_screen() {
        let mut app = AppModel::new();
        app.current_screen = ScreenId::Shakespeare;
        let event = Event::Key(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: Modifiers::SHIFT,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(event));
        assert_eq!(app.current_screen, ScreenId::Dashboard);
    }

    #[test]
    fn number_key_switches_screen() {
        let mut app = AppModel::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('3'),
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(event));
        assert_eq!(app.current_screen, ScreenId::CodeExplorer);
    }

    #[test]
    fn event_conversion_resize() {
        let event = Event::Resize {
            width: 80,
            height: 24,
        };
        let msg = AppMsg::from(event);
        assert!(matches!(
            msg,
            AppMsg::Resize {
                width: 80,
                height: 24
            }
        ));
    }

    // -----------------------------------------------------------------------
    // Integration tests
    // -----------------------------------------------------------------------

    /// Render each screen at 120x40 to verify none panic.
    #[test]
    fn integration_all_screens_render() {
        let app = AppModel::new();
        for &id in ScreenId::ALL {
            let mut pool = GraphemePool::new();
            let mut frame = Frame::new(120, 40, &mut pool);
            let area = Rect::new(0, 0, 120, 38); // Leave room for tab bar + status
            app.screens.view(id, &mut frame, area);
            // If we reach here without panicking, the screen rendered successfully.
        }
    }

    /// Render each screen at 40x10 (tiny) to verify graceful degradation.
    #[test]
    fn integration_resize_small() {
        let app = AppModel::new();
        for &id in ScreenId::ALL {
            let mut pool = GraphemePool::new();
            let mut frame = Frame::new(40, 10, &mut pool);
            let area = Rect::new(0, 0, 40, 8);
            app.screens.view(id, &mut frame, area);
        }
    }

    /// Switch through all screens and verify each renders.
    #[test]
    fn integration_screen_cycle() {
        let mut app = AppModel::new();
        for &id in ScreenId::ALL {
            app.update(AppMsg::SwitchScreen(id));
            assert_eq!(app.current_screen, id);

            let mut pool = GraphemePool::new();
            let mut frame = Frame::new(120, 40, &mut pool);
            app.view(&mut frame);
        }
    }

    /// Verify the error boundary catches panics and shows fallback.
    #[test]
    fn integration_error_boundary() {
        let mut states = ScreenStates::default();

        // Simulate a cached error for the Dashboard screen.
        states.screen_errors[ScreenId::Dashboard.index()] = Some("test panic message".to_string());

        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(40, 10, &mut pool);
        let area = Rect::new(0, 0, 40, 10);

        // This should show the fallback widget, not panic.
        states.view(ScreenId::Dashboard, &mut frame, area);

        // Verify the fallback rendered (look for the error border character).
        let top_left = frame.buffer.get(0, 0).unwrap();
        assert_eq!(
            top_left.content.as_char(),
            Some('┌'),
            "FallbackWidget should render error border"
        );

        // Verify the error can be cleared.
        assert!(states.has_error(ScreenId::Dashboard));
        states.clear_error(ScreenId::Dashboard);
        assert!(!states.has_error(ScreenId::Dashboard));
    }

    /// Verify Tab cycles forward through all screens.
    #[test]
    fn integration_tab_cycles_all_screens() {
        let mut app = AppModel::new();
        assert_eq!(app.current_screen, ScreenId::Dashboard);

        for i in 1..ScreenId::ALL.len() {
            app.update(AppMsg::NextScreen);
            assert_eq!(app.current_screen, ScreenId::ALL[i]);
        }

        // One more wraps to Dashboard.
        app.update(AppMsg::NextScreen);
        assert_eq!(app.current_screen, ScreenId::Dashboard);
    }

    /// Verify all screens have the expected count.
    #[test]
    fn all_screens_count() {
        assert_eq!(ScreenId::ALL.len(), 13);
    }

    // -----------------------------------------------------------------------
    // Command palette integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn ctrl_k_opens_palette() {
        let mut app = AppModel::new();
        assert!(!app.command_palette.is_visible());

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: Modifiers::CTRL,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(event));
        assert!(app.command_palette.is_visible());
    }

    #[test]
    fn palette_esc_dismisses() {
        let mut app = AppModel::new();
        app.command_palette.open();
        assert!(app.command_palette.is_visible());

        let esc = Event::Key(KeyEvent {
            code: KeyCode::Escape,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(esc));
        assert!(!app.command_palette.is_visible());
    }

    #[test]
    fn palette_has_actions_for_all_screens() {
        let app = AppModel::new();
        // One action per screen + 4 global commands
        let expected = ScreenId::ALL.len() + 4;
        assert_eq!(app.command_palette.action_count(), expected);
    }

    #[test]
    fn palette_navigate_to_screen() {
        let mut app = AppModel::new();
        assert_eq!(app.current_screen, ScreenId::Dashboard);

        // Open palette, type "shakespeare", press Enter
        app.command_palette.open();
        for ch in "shakespeare".chars() {
            let event = Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                modifiers: Modifiers::NONE,
                kind: KeyEventKind::Press,
            });
            app.update(AppMsg::from(event));
        }

        let enter = Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        app.update(AppMsg::from(enter));
        assert_eq!(app.current_screen, ScreenId::Shakespeare);
        assert!(!app.command_palette.is_visible());
    }

    #[test]
    fn palette_execute_quit() {
        let mut app = AppModel::new();

        // Directly test execute_palette_action
        let cmd = app.execute_palette_action(PaletteAction::Execute("cmd:quit".into()));
        assert!(matches!(cmd, Cmd::Quit));
    }

    #[test]
    fn palette_toggle_help_via_action() {
        let mut app = AppModel::new();
        assert!(!app.help_visible);

        app.execute_palette_action(PaletteAction::Execute("cmd:toggle_help".into()));
        assert!(app.help_visible);
    }

    #[test]
    fn palette_cycle_theme_via_action() {
        let mut app = AppModel::new();
        let before = theme::current_theme_name();
        app.execute_palette_action(PaletteAction::Execute("cmd:cycle_theme".into()));
        let after = theme::current_theme_name();
        assert_ne!(before, after);
    }

    #[test]
    fn palette_blocks_screen_events_when_open() {
        let mut app = AppModel::new();
        app.command_palette.open();

        // 'q' key should NOT quit the app when palette is open
        let q = Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        let cmd = app.update(AppMsg::from(q));
        assert!(!matches!(cmd, Cmd::Quit));
        // The 'q' was consumed by the palette as query input
        assert_eq!(app.command_palette.query(), "q");
    }

    #[test]
    fn palette_renders_as_overlay() {
        let mut app = AppModel::new();
        app.terminal_width = 80;
        app.terminal_height = 24;
        app.command_palette.open();

        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        app.view(&mut frame);
        // Should not panic — overlay rendered on top of content.
    }
}
