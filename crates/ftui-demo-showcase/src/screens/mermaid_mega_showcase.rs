//! Mermaid Mega Showcase Screen — interactive layout lab.
//!
//! A comprehensive, over-the-top Mermaid diagram demo with:
//! - Full sample library with metadata and filtering
//! - Interactive node navigation and edge highlighting
//! - Split-panel layout with diagram, controls, metrics, and detail panels
//! - All configuration knobs exposed as keybindings
//! - Help overlay driven by the canonical keymap spec

use std::cell::RefCell;
use std::time::Instant;

use ftui_core::geometry::Rect;
use ftui_extras::mermaid;
use ftui_extras::mermaid::{
    DiagramPalettePreset, MermaidCompatibilityMatrix, MermaidConfig, MermaidDiagramIr,
    MermaidError, MermaidFallbackPolicy, MermaidGlyphMode, MermaidRenderMode, MermaidTier,
    MermaidWrapMode, ShowcaseMode,
};
use ftui_extras::mermaid_layout;
use ftui_extras::mermaid_render;
use ftui_extras::mermaid_render::DiagramPalette;
use ftui_render::buffer::Buffer;
use ftui_render::cell::{Cell, PackedRgba};
use ftui_render::drawing::{BorderChars, Draw};

use crate::screens::{Cmd, Event, Frame, HelpEntry, Screen};

// ── Layout constants ────────────────────────────────────────────────

/// Minimum terminal width for full layout (below this, panels collapse).
const MIN_FULL_WIDTH: u16 = 100;
/// Minimum terminal height for full layout.
const MIN_FULL_HEIGHT: u16 = 20;
/// Side panel width when visible.
const SIDE_PANEL_WIDTH: u16 = 28;
/// Footer height (status + hints).
const FOOTER_HEIGHT: u16 = 2;
/// Controls panel height when visible.
const CONTROLS_PANEL_HEIGHT: u16 = 6;
/// Pan step in cells per keypress.
const PAN_STEP: i16 = 4;
/// Minimum milliseconds between layout recomputations (debounce window).
const LAYOUT_DEBOUNCE_MS: u128 = 50;
/// Layout computation budget in milliseconds.  Exceeding this triggers a warning.
const LAYOUT_BUDGET_MS: f32 = 16.0;
/// Default viewport override columns.
const VIEWPORT_OVERRIDE_DEFAULT_COLS: u16 = 80;
/// Default viewport override rows.
const VIEWPORT_OVERRIDE_DEFAULT_ROWS: u16 = 24;
/// Minimum viewport override columns.
const VIEWPORT_OVERRIDE_MIN_COLS: u16 = 10;
/// Minimum viewport override rows.
const VIEWPORT_OVERRIDE_MIN_ROWS: u16 = 4;
/// Viewport override step for width adjustments.
const VIEWPORT_OVERRIDE_STEP_COLS: i16 = 4;
/// Viewport override step for height adjustments.
const VIEWPORT_OVERRIDE_STEP_ROWS: i16 = 2;

// ── Sample library ─────────────────────────────────────────────────

/// A curated Mermaid sample for the mega showcase.
struct MegaSample {
    name: &'static str,
    source: &'static str,
}

const MEGA_SAMPLES: &[MegaSample] = &[
    MegaSample {
        name: "Flow Basic",
        source: "graph TD\n    A[Start] --> B{Decision}\n    B -->|Yes| C[OK]\n    B -->|No| D[Error]\n    C --> E[End]\n    D --> E",
    },
    MegaSample {
        name: "Flow Subgraphs",
        source: "graph LR\n    subgraph Frontend\n        A[React] --> B[Redux]\n    end\n    subgraph Backend\n        C[API] --> D[DB]\n    end\n    B --> C",
    },
    MegaSample {
        name: "Flow Dense",
        source: "graph TD\n    A --> B --> C --> D\n    A --> C\n    B --> D\n    E --> F --> G\n    D --> G\n    A --> E",
    },
    MegaSample {
        name: "Sequence",
        source: "sequenceDiagram\n    Alice->>Bob: Hello Bob\n    Bob-->>Alice: Hi Alice\n    Alice->>Bob: How are you?\n    Bob-->>Alice: Great!",
    },
    MegaSample {
        name: "Class",
        source: "classDiagram\n    class Animal {\n        +String name\n        +makeSound()\n    }\n    class Dog {\n        +fetch()\n    }\n    Animal <|-- Dog",
    },
    MegaSample {
        name: "State",
        source: "stateDiagram-v2\n    [*] --> Idle\n    Idle --> Processing : start\n    Processing --> Done : finish\n    Processing --> Error : fail\n    Error --> Idle : retry\n    Done --> [*]",
    },
    MegaSample {
        name: "ER",
        source: "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER ||--|{ LINE-ITEM : contains\n    PRODUCT ||--o{ LINE-ITEM : \"ordered in\"",
    },
    MegaSample {
        name: "Pie",
        source: "pie title Browser Share\n    \"Chrome\" : 65\n    \"Firefox\" : 15\n    \"Safari\" : 12\n    \"Edge\" : 8",
    },
    MegaSample {
        name: "Gantt",
        source: "gantt\n    title Project Plan\n    section Design\n    Wireframes :a1, 2024-01-01, 7d\n    Mockups :a2, after a1, 5d\n    section Dev\n    Frontend :b1, after a2, 14d\n    Backend :b2, after a2, 14d",
    },
    MegaSample {
        name: "Mindmap",
        source: "mindmap\n  root((Project))\n    Planning\n      Goals\n      Timeline\n    Development\n      Frontend\n      Backend\n    Testing",
    },
];

// ── Panel visibility ────────────────────────────────────────────────

/// Which panels are currently visible.
#[derive(Debug, Clone, Copy)]
struct PanelVisibility {
    controls: bool,
    metrics: bool,
    detail: bool,
    status_log: bool,
    help_overlay: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            controls: true,
            metrics: true,
            detail: false,
            status_log: false,
            help_overlay: false,
        }
    }
}

// ── Layout mode ─────────────────────────────────────────────────────

/// Layout density preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Dense,
    Normal,
    Spacious,
    Auto,
}

impl LayoutMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Dense => "dense",
            Self::Normal => "normal",
            Self::Spacious => "spacious",
            Self::Auto => "auto",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Dense => Self::Normal,
            Self::Normal => Self::Spacious,
            Self::Spacious => Self::Auto,
            Self::Auto => Self::Dense,
        }
    }
}

// ── Computed layout regions ─────────────────────────────────────────

/// Regions computed from the terminal area and panel visibility.
#[derive(Debug, Clone, Copy, Default)]
struct LayoutRegions {
    /// Main diagram rendering area.
    diagram: Rect,
    /// Right-side panel area (metrics + detail).
    side_panel: Rect,
    /// Top controls strip.
    controls: Rect,
    /// Bottom footer (status line + key hints).
    footer: Rect,
}

impl LayoutRegions {
    /// Compute layout regions from available area and panel state.
    fn compute(area: Rect, panels: &PanelVisibility) -> Self {
        if area.width < 10 || area.height < 5 {
            return Self {
                diagram: area,
                ..Default::default()
            };
        }

        let x = area.x;
        let mut y = area.y;
        let mut w = area.width;
        let mut h = area.height;

        // Footer always present.
        let footer_h = FOOTER_HEIGHT.min(h.saturating_sub(3));
        h = h.saturating_sub(footer_h);
        let footer = Rect::new(x, y + h, w, footer_h);

        // Controls strip at top.
        let controls_h = if panels.controls && h > 8 {
            CONTROLS_PANEL_HEIGHT.min(h / 3)
        } else {
            0
        };
        let controls = Rect::new(x, y, w, controls_h);
        y += controls_h;
        h = h.saturating_sub(controls_h);

        // Side panel on right.
        let side_w = if (panels.metrics || panels.detail) && w >= MIN_FULL_WIDTH {
            SIDE_PANEL_WIDTH.min(w / 3)
        } else {
            0
        };
        let side_panel = if side_w > 0 {
            w -= side_w;
            Rect::new(x + w, y, side_w, h)
        } else {
            Rect::default()
        };

        let diagram = Rect::new(x, y, w, h);

        Self {
            diagram,
            side_panel,
            controls,
            footer,
        }
    }
}

// ── Render cache ───────────────────────────────────────────────────

/// Cached render pipeline output for the mega showcase.
#[derive(Debug)]
struct MegaRenderCache {
    analysis_epoch: u64,
    layout_epoch: u64,
    render_epoch: u64,
    viewport: (u16, u16),
    zoom: f32,
    ir: Option<MermaidDiagramIr>,
    layout: Option<mermaid_layout::DiagramLayout>,
    buffer: Buffer,
    errors: Vec<MermaidError>,
    cache_hits: u64,
    cache_misses: u64,
    last_cache_hit: bool,
    parse_ms: Option<f32>,
    layout_ms: Option<f32>,
    render_ms: Option<f32>,
    /// Timestamp of the most recent layout computation (for debounce).
    last_layout_instant: Option<Instant>,
    /// Whether the last layout exceeded the budget threshold.
    layout_budget_exceeded: bool,
    /// Whether the budget warning has been logged (prevents duplicate logs).
    budget_warning_logged: bool,
    /// Number of layout passes deferred due to debounce.
    debounce_skips: u64,
}

impl MegaRenderCache {
    fn empty() -> Self {
        Self {
            analysis_epoch: u64::MAX,
            layout_epoch: u64::MAX,
            render_epoch: u64::MAX,
            viewport: (0, 0),
            zoom: 1.0,
            ir: None,
            layout: None,
            buffer: Buffer::new(1, 1),
            errors: Vec::new(),
            cache_hits: 0,
            cache_misses: 0,
            last_cache_hit: false,
            parse_ms: None,
            layout_ms: None,
            render_ms: None,
            last_layout_instant: None,
            layout_budget_exceeded: false,
            budget_warning_logged: true,
            debounce_skips: 0,
        }
    }
}

// ── State ───────────────────────────────────────────────────────────

/// Maximum status log entries before oldest are evicted.
const STATUS_LOG_CAP: usize = 64;

/// A single entry in the status log.
#[derive(Debug, Clone)]
struct StatusLogEntry {
    action: &'static str,
    detail: String,
}

/// State for the Mermaid Mega Showcase screen.
#[derive(Debug)]
pub struct MermaidMegaState {
    /// Current interaction mode.
    mode: ShowcaseMode,
    /// Panel visibility flags.
    panels: PanelVisibility,
    /// Layout density mode.
    layout_mode: LayoutMode,
    /// Fidelity tier.
    tier: MermaidTier,
    /// Glyph mode (Unicode / ASCII).
    glyph_mode: MermaidGlyphMode,
    /// Render mode (Cell / Braille / Block / etc).
    render_mode: MermaidRenderMode,
    /// Wrap mode for labels.
    wrap_mode: MermaidWrapMode,
    /// Color palette preset.
    palette: DiagramPalettePreset,
    /// Whether classDef/style rendering is enabled.
    styles_enabled: bool,
    /// Viewport zoom level (1.0 = 100%).
    viewport_zoom: f32,
    /// Viewport pan offset (x, y) in cells.
    viewport_pan: (i16, i16),
    /// Optional explicit viewport size override (cols, rows).
    viewport_size_override: Option<(u16, u16)>,
    /// Selected sample index.
    selected_sample: usize,
    /// Selected node index for inspect mode.
    selected_node: Option<usize>,
    /// Search query (when in search mode).
    search_query: Option<String>,
    /// Epoch counters for cache invalidation.
    analysis_epoch: u64,
    layout_epoch: u64,
    render_epoch: u64,
    /// Status log for debugging state changes.
    status_log: Vec<StatusLogEntry>,
}

impl Default for MermaidMegaState {
    fn default() -> Self {
        Self {
            mode: ShowcaseMode::Normal,
            panels: PanelVisibility::default(),
            layout_mode: LayoutMode::Auto,
            tier: MermaidTier::Auto,
            glyph_mode: MermaidGlyphMode::Unicode,
            render_mode: MermaidRenderMode::Auto,
            wrap_mode: MermaidWrapMode::WordChar,
            palette: DiagramPalettePreset::Default,
            styles_enabled: true,
            viewport_zoom: 1.0,
            viewport_pan: (0, 0),
            viewport_size_override: None,
            selected_sample: 0,
            selected_node: None,
            search_query: None,
            analysis_epoch: 0,
            layout_epoch: 0,
            render_epoch: 0,
            status_log: Vec::new(),
        }
    }
}

impl MermaidMegaState {
    /// Record an action in the status log.
    fn log_action(&mut self, action: &'static str, detail: String) {
        if self.status_log.len() >= STATUS_LOG_CAP {
            self.status_log.remove(0);
        }
        self.status_log.push(StatusLogEntry { action, detail });
    }

    /// Build a MermaidConfig from the current state.
    fn to_config(&self) -> MermaidConfig {
        let mut config = MermaidConfig {
            glyph_mode: self.glyph_mode,
            render_mode: self.render_mode,
            tier_override: self.tier,
            wrap_mode: self.wrap_mode,
            enable_styles: self.styles_enabled,
            palette: self.palette,
            ..Default::default()
        };
        match self.layout_mode {
            LayoutMode::Dense => {
                config.layout_iteration_budget = 400;
                config.route_budget = 8_000;
            }
            LayoutMode::Spacious => {
                config.layout_iteration_budget = 140;
                config.route_budget = 3_000;
            }
            LayoutMode::Normal | LayoutMode::Auto => {}
        }
        config
    }

    /// Get layout spacing based on the current layout mode.
    fn layout_spacing(&self) -> mermaid_layout::LayoutSpacing {
        match self.layout_mode {
            LayoutMode::Dense => mermaid_layout::LayoutSpacing {
                rank_gap: 2.0,
                node_gap: 2.0,
                ..mermaid_layout::LayoutSpacing::default()
            },
            LayoutMode::Spacious => mermaid_layout::LayoutSpacing {
                rank_gap: 6.0,
                node_gap: 5.0,
                ..mermaid_layout::LayoutSpacing::default()
            },
            LayoutMode::Normal | LayoutMode::Auto => mermaid_layout::LayoutSpacing::default(),
        }
    }

    /// Get the currently selected sample source, wrapping around.
    fn selected_source(&self) -> Option<&'static str> {
        if MEGA_SAMPLES.is_empty() {
            return None;
        }
        let idx = self.selected_sample % MEGA_SAMPLES.len();
        Some(MEGA_SAMPLES[idx].source)
    }

    /// Get the currently selected sample name, wrapping around.
    fn selected_name(&self) -> &'static str {
        if MEGA_SAMPLES.is_empty() {
            return "(none)";
        }
        let idx = self.selected_sample % MEGA_SAMPLES.len();
        MEGA_SAMPLES[idx].name
    }

    /// Adjust the viewport size override by a delta, creating one if needed.
    fn adjust_viewport_override(&mut self, delta_cols: i16, delta_rows: i16) {
        let (cols, rows) = self.viewport_size_override.unwrap_or((
            VIEWPORT_OVERRIDE_DEFAULT_COLS,
            VIEWPORT_OVERRIDE_DEFAULT_ROWS,
        ));
        let cols = (cols as i32 + delta_cols as i32)
            .clamp(VIEWPORT_OVERRIDE_MIN_COLS as i32, u16::MAX as i32) as u16;
        let rows = (rows as i32 + delta_rows as i32)
            .clamp(VIEWPORT_OVERRIDE_MIN_ROWS as i32, u16::MAX as i32) as u16;
        let next = Some((cols, rows));
        if self.viewport_size_override != next {
            self.viewport_size_override = next;
            self.bump_render();
        }
    }

    /// Bump render epoch (triggers re-render without re-layout).
    fn bump_render(&mut self) {
        self.render_epoch = self.render_epoch.wrapping_add(1);
    }

    /// Bump layout epoch (triggers re-layout + re-render).
    fn bump_layout(&mut self) {
        self.layout_epoch = self.layout_epoch.wrapping_add(1);
        self.bump_render();
    }

    /// Bump analysis epoch (triggers full re-parse + re-layout + re-render).
    fn bump_analysis(&mut self) {
        self.analysis_epoch = self.analysis_epoch.wrapping_add(1);
        self.bump_layout();
    }
}

// ── Actions ─────────────────────────────────────────────────────────

/// Actions the mega showcase screen can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MegaAction {
    NextSample,
    PrevSample,
    CycleTier,
    ToggleGlyphMode,
    CycleRenderMode,
    CycleWrapMode,
    ToggleStyles,
    CycleLayoutMode,
    ForceRelayout,
    CyclePalette,
    PrevPalette,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitToView,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    IncreaseViewportWidth,
    DecreaseViewportWidth,
    IncreaseViewportHeight,
    DecreaseViewportHeight,
    ResetViewportOverride,
    ToggleMetrics,
    ToggleControls,
    ToggleDetail,
    ToggleStatusLog,
    ToggleHelp,
    SelectNextNode,
    SelectPrevNode,
    DeselectNode,
    EnterSearch,
    ExitSearch,
    CollapsePanels,
}

impl MermaidMegaState {
    /// Apply an action to the state.
    fn apply(&mut self, action: MegaAction) {
        match action {
            MegaAction::NextSample => {
                self.selected_sample = self.selected_sample.wrapping_add(1);
                self.selected_node = None;
                self.bump_analysis();
            }
            MegaAction::PrevSample => {
                self.selected_sample = self.selected_sample.wrapping_sub(1);
                self.selected_node = None;
                self.bump_analysis();
            }
            MegaAction::CycleTier => {
                self.tier = match self.tier {
                    MermaidTier::Auto => MermaidTier::Compact,
                    MermaidTier::Compact => MermaidTier::Normal,
                    MermaidTier::Normal => MermaidTier::Rich,
                    MermaidTier::Rich => MermaidTier::Auto,
                };
                self.bump_layout();
            }
            MegaAction::ToggleGlyphMode => {
                self.glyph_mode = match self.glyph_mode {
                    MermaidGlyphMode::Unicode => MermaidGlyphMode::Ascii,
                    MermaidGlyphMode::Ascii => MermaidGlyphMode::Unicode,
                };
                self.bump_render();
            }
            MegaAction::CycleRenderMode => {
                self.render_mode = match self.render_mode {
                    MermaidRenderMode::Auto => MermaidRenderMode::CellOnly,
                    MermaidRenderMode::CellOnly => MermaidRenderMode::Braille,
                    MermaidRenderMode::Braille => MermaidRenderMode::Block,
                    MermaidRenderMode::Block => MermaidRenderMode::HalfBlock,
                    MermaidRenderMode::HalfBlock => MermaidRenderMode::Auto,
                };
                self.bump_render();
            }
            MegaAction::CycleWrapMode => {
                self.wrap_mode = match self.wrap_mode {
                    MermaidWrapMode::None => MermaidWrapMode::Word,
                    MermaidWrapMode::Word => MermaidWrapMode::Char,
                    MermaidWrapMode::Char => MermaidWrapMode::WordChar,
                    MermaidWrapMode::WordChar => MermaidWrapMode::None,
                };
                self.bump_layout();
            }
            MegaAction::ToggleStyles => {
                self.styles_enabled = !self.styles_enabled;
                self.bump_render();
            }
            MegaAction::CycleLayoutMode => {
                self.layout_mode = self.layout_mode.next();
                self.bump_layout();
            }
            MegaAction::ForceRelayout => {
                self.bump_layout();
            }
            MegaAction::CyclePalette => {
                self.palette = self.palette.next();
                self.bump_render();
            }
            MegaAction::PrevPalette => {
                self.palette = self.palette.prev();
                self.bump_render();
            }
            MegaAction::ZoomIn => {
                self.viewport_zoom = (self.viewport_zoom * 1.25).min(4.0);
                self.bump_render();
            }
            MegaAction::ZoomOut => {
                self.viewport_zoom = (self.viewport_zoom / 1.25).max(0.25);
                self.bump_render();
            }
            MegaAction::ZoomReset => {
                self.viewport_zoom = 1.0;
                self.viewport_pan = (0, 0);
                self.bump_render();
            }
            MegaAction::FitToView => {
                self.viewport_zoom = 1.0;
                self.viewport_pan = (0, 0);
                self.bump_render();
            }
            MegaAction::PanLeft => {
                self.viewport_pan.0 = self.viewport_pan.0.saturating_sub(PAN_STEP);
            }
            MegaAction::PanRight => {
                self.viewport_pan.0 = self.viewport_pan.0.saturating_add(PAN_STEP);
            }
            MegaAction::PanUp => {
                self.viewport_pan.1 = self.viewport_pan.1.saturating_sub(PAN_STEP);
            }
            MegaAction::PanDown => {
                self.viewport_pan.1 = self.viewport_pan.1.saturating_add(PAN_STEP);
            }
            MegaAction::IncreaseViewportWidth => {
                self.adjust_viewport_override(VIEWPORT_OVERRIDE_STEP_COLS, 0);
            }
            MegaAction::DecreaseViewportWidth => {
                self.adjust_viewport_override(-VIEWPORT_OVERRIDE_STEP_COLS, 0);
            }
            MegaAction::IncreaseViewportHeight => {
                self.adjust_viewport_override(0, VIEWPORT_OVERRIDE_STEP_ROWS);
            }
            MegaAction::DecreaseViewportHeight => {
                self.adjust_viewport_override(0, -VIEWPORT_OVERRIDE_STEP_ROWS);
            }
            MegaAction::ResetViewportOverride => {
                if self.viewport_size_override.is_some() {
                    self.viewport_size_override = None;
                    self.bump_render();
                }
            }
            MegaAction::ToggleMetrics => {
                self.panels.metrics = !self.panels.metrics;
            }
            MegaAction::ToggleControls => {
                self.panels.controls = !self.panels.controls;
            }
            MegaAction::ToggleDetail => {
                self.panels.detail = !self.panels.detail;
            }
            MegaAction::ToggleStatusLog => {
                self.panels.status_log = !self.panels.status_log;
            }
            MegaAction::ToggleHelp => {
                self.panels.help_overlay = !self.panels.help_overlay;
            }
            MegaAction::SelectNextNode => {
                self.selected_node = Some(self.selected_node.map_or(0, |n| n + 1));
                self.mode = ShowcaseMode::Inspect;
                self.bump_render();
            }
            MegaAction::SelectPrevNode => {
                self.selected_node = Some(self.selected_node.map_or(0, |n| n.saturating_sub(1)));
                self.mode = ShowcaseMode::Inspect;
                self.bump_render();
            }
            MegaAction::DeselectNode => {
                self.selected_node = None;
                self.mode = ShowcaseMode::Normal;
                self.bump_render();
            }
            MegaAction::EnterSearch => {
                self.mode = ShowcaseMode::Search;
                self.search_query = Some(String::new());
            }
            MegaAction::ExitSearch => {
                self.mode = ShowcaseMode::Normal;
                self.search_query = None;
                self.bump_render();
            }
            MegaAction::CollapsePanels => {
                if self.mode == ShowcaseMode::Inspect {
                    self.selected_node = None;
                    self.mode = ShowcaseMode::Normal;
                    self.bump_render();
                } else if self.mode == ShowcaseMode::Search {
                    self.search_query = None;
                    self.mode = ShowcaseMode::Normal;
                    self.bump_render();
                } else {
                    self.panels.controls = false;
                    self.panels.metrics = false;
                    self.panels.detail = false;
                    self.panels.status_log = false;
                    self.panels.help_overlay = false;
                }
            }
        }
        // Log every action for debugging.
        self.log_action("action", format!("{action:?}"));
    }
}

// ── Screen ──────────────────────────────────────────────────────────

/// Mermaid Mega Showcase — the over-the-top interactive diagram lab.
pub struct MermaidMegaShowcaseScreen {
    state: MermaidMegaState,
    cache: RefCell<MegaRenderCache>,
}

impl Default for MermaidMegaShowcaseScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidMegaShowcaseScreen {
    /// Create a new mega showcase screen.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: MermaidMegaState::default(),
            cache: RefCell::new(MegaRenderCache::empty()),
        }
    }

    /// Map a key event to an action.
    fn handle_key(&self, event: &ftui_core::event::KeyEvent) -> Option<MegaAction> {
        use ftui_core::event::KeyCode;
        match event.code {
            // Sample navigation
            KeyCode::Down | KeyCode::Char('j') => Some(MegaAction::NextSample),
            KeyCode::Up | KeyCode::Char('k') => Some(MegaAction::PrevSample),
            // Render config
            KeyCode::Char('t') => Some(MegaAction::CycleTier),
            KeyCode::Char('g') => Some(MegaAction::ToggleGlyphMode),
            KeyCode::Char('b') => Some(MegaAction::CycleRenderMode),
            KeyCode::Char('s') => Some(MegaAction::ToggleStyles),
            KeyCode::Char('w') => Some(MegaAction::CycleWrapMode),
            KeyCode::Char('l') => Some(MegaAction::CycleLayoutMode),
            KeyCode::Char('r') => Some(MegaAction::ForceRelayout),
            // Theme
            KeyCode::Char('p') => Some(MegaAction::CyclePalette),
            KeyCode::Char('P') => Some(MegaAction::PrevPalette),
            // Viewport zoom
            KeyCode::Char('+') | KeyCode::Char('=') => Some(MegaAction::ZoomIn),
            KeyCode::Char('-') => Some(MegaAction::ZoomOut),
            KeyCode::Char('0') => Some(MegaAction::ZoomReset),
            KeyCode::Char('f') => Some(MegaAction::FitToView),
            // Viewport pan (Shift+H/J/K/L)
            KeyCode::Char('H') => Some(MegaAction::PanLeft),
            KeyCode::Char('J') => Some(MegaAction::PanDown),
            KeyCode::Char('K') => Some(MegaAction::PanUp),
            KeyCode::Char('L') => Some(MegaAction::PanRight),
            // Viewport size override
            KeyCode::Char(']') => Some(MegaAction::IncreaseViewportWidth),
            KeyCode::Char('[') => Some(MegaAction::DecreaseViewportWidth),
            KeyCode::Char('}') => Some(MegaAction::IncreaseViewportHeight),
            KeyCode::Char('{') => Some(MegaAction::DecreaseViewportHeight),
            KeyCode::Char('o') => Some(MegaAction::ResetViewportOverride),
            // Panels
            KeyCode::Char('m') => Some(MegaAction::ToggleMetrics),
            KeyCode::Char('c') => Some(MegaAction::ToggleControls),
            KeyCode::Char('d') => Some(MegaAction::ToggleDetail),
            KeyCode::Char('i') => Some(MegaAction::ToggleStatusLog),
            KeyCode::Char('?') => Some(MegaAction::ToggleHelp),
            // Node inspection
            KeyCode::Tab => Some(MegaAction::SelectNextNode),
            KeyCode::BackTab => Some(MegaAction::SelectPrevNode),
            // Search
            KeyCode::Char('/') => Some(MegaAction::EnterSearch),
            // Escape is context-dependent
            KeyCode::Escape => Some(MegaAction::CollapsePanels),
            _ => None,
        }
    }

    // ── Render pipeline ────────────────────────────────────────────

    /// Compute the target viewport size, respecting overrides.
    fn target_viewport_size(&self, inner: Rect) -> (u16, u16) {
        if let Some((cols, rows)) = self.state.viewport_size_override {
            (cols.max(1), rows.max(1))
        } else {
            (inner.width.max(1), inner.height.max(1))
        }
    }

    /// Ensure the render cache is up-to-date for the given diagram area.
    fn ensure_render_cache(&self, inner: Rect) {
        let (width, height) = self.target_viewport_size(inner);
        let zoom = self.state.viewport_zoom;
        let render_width = (f32::from(width) * zoom)
            .round()
            .clamp(1.0, f32::from(u16::MAX)) as u16;
        let render_height = (f32::from(height) * zoom)
            .round()
            .clamp(1.0, f32::from(u16::MAX)) as u16;

        let mut cache = self.cache.borrow_mut();
        let zoom_matches = (cache.zoom - zoom).abs() <= f32::EPSILON;
        let mut analysis_needed = cache.analysis_epoch != self.state.analysis_epoch;
        let mut layout_needed = cache.layout_epoch != self.state.layout_epoch;
        let mut render_needed = cache.render_epoch != self.state.render_epoch
            || cache.viewport != (width, height)
            || !zoom_matches;

        if cache.ir.is_none() {
            analysis_needed = true;
        }
        if cache.layout.is_none() {
            layout_needed = true;
        }

        if !analysis_needed && !layout_needed && !render_needed {
            cache.cache_hits = cache.cache_hits.saturating_add(1);
            cache.last_cache_hit = true;
            return;
        }

        // Debounce: when the previous layout was expensive (exceeded the budget)
        // and was computed very recently, defer this pass to coalesce rapid
        // input and prevent CPU spikes on complex diagrams.
        if (analysis_needed || layout_needed)
            && cache.layout.is_some()
            && cache.layout_budget_exceeded
            && cache
                .last_layout_instant
                .is_some_and(|last| last.elapsed().as_millis() < LAYOUT_DEBOUNCE_MS)
        {
            cache.debounce_skips = cache.debounce_skips.saturating_add(1);
            cache.cache_hits = cache.cache_hits.saturating_add(1);
            cache.last_cache_hit = true;
            return;
        }

        cache.cache_misses = cache.cache_misses.saturating_add(1);
        cache.last_cache_hit = false;

        let source = match self.state.selected_source() {
            Some(s) => s,
            None => {
                cache.analysis_epoch = self.state.analysis_epoch;
                cache.layout_epoch = self.state.layout_epoch;
                cache.render_epoch = self.state.render_epoch;
                cache.ir = None;
                cache.layout = None;
                cache.errors.clear();
                cache.parse_ms = None;
                cache.layout_ms = None;
                cache.render_ms = None;
                return;
            }
        };

        let config = self.state.to_config();
        let matrix = MermaidCompatibilityMatrix::default();
        let policy = MermaidFallbackPolicy::default();

        if analysis_needed {
            let parse_start = Instant::now();
            let parsed = mermaid::parse_with_diagnostics(source);
            cache.parse_ms = Some(parse_start.elapsed().as_secs_f32() * 1000.0);

            let ir_parse = mermaid::normalize_ast_to_ir(&parsed.ast, &config, &matrix, &policy);
            let mut errors = Vec::new();
            errors.extend(parsed.errors);
            errors.extend(ir_parse.errors);
            cache.errors = errors;
            cache.ir = Some(ir_parse.ir);
            cache.analysis_epoch = self.state.analysis_epoch;
            layout_needed = true;
            render_needed = true;
        }

        if layout_needed {
            if let Some(ir) = cache.ir.as_ref() {
                let spacing = self.state.layout_spacing();
                let layout_start = Instant::now();
                let layout = mermaid_layout::layout_diagram_with_spacing(ir, &config, &spacing);
                let elapsed_ms = layout_start.elapsed().as_secs_f32() * 1000.0;
                cache.layout_ms = Some(elapsed_ms);
                cache.layout = Some(layout);

                // Record timestamp for debounce and check budget.
                cache.last_layout_instant = Some(Instant::now());
                if elapsed_ms > LAYOUT_BUDGET_MS {
                    cache.layout_budget_exceeded = true;
                    cache.budget_warning_logged = false;
                } else {
                    cache.layout_budget_exceeded = false;
                }
            }
            cache.layout_epoch = self.state.layout_epoch;
            render_needed = true;
        }

        if render_needed {
            if let (Some(ir), Some(layout)) = (cache.ir.as_ref(), cache.layout.as_ref()) {
                let mut buffer = Buffer::new(render_width, render_height);
                let area = Rect::new(0, 0, render_width, render_height);
                let render_start = Instant::now();
                let _plan =
                    mermaid_render::render_diagram_adaptive(layout, ir, &config, area, &mut buffer);
                cache.render_ms = Some(render_start.elapsed().as_secs_f32() * 1000.0);
                cache.buffer = buffer;
            }
            cache.viewport = (width, height);
            cache.zoom = zoom;
            cache.render_epoch = self.state.render_epoch;
        }
    }

    /// Blit a cached buffer onto the frame with centering and pan offset.
    fn blit_buffer(&self, frame: &mut Frame, area: Rect, buf: &Buffer, pan: (i16, i16)) {
        let view_w = area.width;
        let view_h = area.height;
        let buf_w = buf.width();
        let buf_h = buf.height();
        if view_w == 0 || view_h == 0 || buf_w == 0 || buf_h == 0 {
            return;
        }

        let pan_x = i32::from(pan.0);
        let pan_y = i32::from(pan.1);

        let (src_x, dst_x, copy_w) = if buf_w >= view_w {
            let center = ((buf_w - view_w) / 2) as i32;
            let max_src = (buf_w - view_w) as i32;
            let src = (center + pan_x).clamp(0, max_src);
            (src as u16, area.x, view_w)
        } else {
            let center = ((view_w - buf_w) / 2) as i32;
            let min_dst = i32::from(area.x);
            let max_dst = min_dst + (view_w - buf_w) as i32;
            let dst = (min_dst + center + pan_x).clamp(min_dst, max_dst);
            (0, dst as u16, buf_w)
        };

        let (src_y, dst_y, copy_h) = if buf_h >= view_h {
            let center = ((buf_h - view_h) / 2) as i32;
            let max_src = (buf_h - view_h) as i32;
            let src = (center + pan_y).clamp(0, max_src);
            (src as u16, area.y, view_h)
        } else {
            let center = ((view_h - buf_h) / 2) as i32;
            let min_dst = i32::from(area.y);
            let max_dst = min_dst + (view_h - buf_h) as i32;
            let dst = (min_dst + center + pan_y).clamp(min_dst, max_dst);
            (0, dst as u16, buf_h)
        };

        if copy_w == 0 || copy_h == 0 {
            return;
        }

        frame
            .buffer
            .copy_from(buf, Rect::new(src_x, src_y, copy_w, copy_h), dst_x, dst_y);
    }

    // ── Panel renderers ────────────────────────────────────────────

    /// Render the controls strip at the top.
    fn render_controls(&self, area: Rect, frame: &mut Frame) {
        if area.is_empty() {
            return;
        }
        let border = Cell::from_char(' ').with_fg(PackedRgba::rgb(80, 80, 100));
        frame.draw_border(area, BorderChars::SQUARE, border);

        let s = &self.state;
        let viewport_info = if let Some((cols, rows)) = s.viewport_size_override {
            format!("VP:{cols}x{rows}")
        } else {
            "VP:auto".to_string()
        };
        let pan_info = if s.viewport_pan != (0, 0) {
            format!(" Pan:{},{}", s.viewport_pan.0, s.viewport_pan.1)
        } else {
            String::new()
        };
        let status = format!(
            " Tier:{} Glyph:{} Render:{} Wrap:{} Layout:{} Palette:{} Zoom:{:.0}% {}{} ",
            s.tier,
            s.glyph_mode,
            s.render_mode,
            s.wrap_mode,
            s.layout_mode.as_str(),
            s.palette,
            (s.viewport_zoom * 100.0),
            viewport_info,
            pan_info,
        );
        let text_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(180, 200, 220));
        frame.print_text_clipped(
            area.x + 1,
            area.y + 1,
            &status,
            text_cell,
            area.x + area.width - 1,
        );
    }

    /// Render the side panel (metrics / detail) with cache stats.
    fn render_side_panel(&self, area: Rect, frame: &mut Frame) {
        if area.is_empty() {
            return;
        }
        let border = Cell::from_char(' ').with_fg(PackedRgba::rgb(80, 80, 100));
        frame.draw_border(area, BorderChars::SQUARE, border);

        let title = if self.state.panels.detail {
            " Detail "
        } else {
            " Metrics "
        };
        let title_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(140, 180, 220));
        frame.print_text_clipped(
            area.x + 1,
            area.y,
            title,
            title_cell,
            area.x + area.width - 1,
        );

        let cache = self.cache.borrow();
        let mut lines: Vec<String> = Vec::new();

        lines.push(format!("Mode: {}", self.state.mode.as_str()));
        lines.push(format!("Sample: {}", self.state.selected_name()));
        lines.push(format!("Palette: {}", self.state.palette));
        lines.push(format!(
            "Node: {}",
            self.state
                .selected_node
                .map_or("-".to_string(), |n| format!("#{n}"))
        ));
        lines.push(format!(
            "Epoch: a{}/l{}/r{}",
            self.state.analysis_epoch, self.state.layout_epoch, self.state.render_epoch
        ));
        if let Some((cols, rows)) = self.state.viewport_size_override {
            lines.push(format!("VP: {cols}x{rows} (override)"));
        } else {
            lines.push("VP: auto".to_string());
        }
        if self.state.viewport_pan != (0, 0) {
            lines.push(format!(
                "Pan: {},{}",
                self.state.viewport_pan.0, self.state.viewport_pan.1
            ));
        }
        lines.push(String::new());

        // Cache performance metrics
        lines.push(format!(
            "Parse: {}",
            cache
                .parse_ms
                .map_or("-".to_string(), |ms| format!("{ms:.1}ms"))
        ));
        lines.push(format!(
            "Layout: {}",
            cache
                .layout_ms
                .map_or("-".to_string(), |ms| format!("{ms:.1}ms"))
        ));
        lines.push(format!(
            "Render: {}",
            cache
                .render_ms
                .map_or("-".to_string(), |ms| format!("{ms:.1}ms"))
        ));
        lines.push(format!(
            "Cache: {}/{}",
            cache.cache_hits, cache.cache_misses
        ));
        lines.push(format!(
            "Last: {}",
            if cache.last_cache_hit { "HIT" } else { "MISS" }
        ));

        if cache.debounce_skips > 0 {
            lines.push(format!("Debounce: {}", cache.debounce_skips));
        }
        if cache.layout_budget_exceeded {
            lines.push(format!("Budget: OVER ({LAYOUT_BUDGET_MS:.0}ms)"));
        }
        if !cache.errors.is_empty() {
            lines.push(format!("Errors: {}", cache.errors.len()));
        }

        let info_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(160, 160, 180));
        let max_x = area.x + area.width - 1;
        for (row, line) in lines.iter().enumerate() {
            let y = area.y + 2 + row as u16;
            if y >= area.y + area.height - 1 {
                break;
            }
            frame.print_text_clipped(area.x + 1, y, line, info_cell, max_x);
        }
    }

    /// Render the footer with mode indicator and key hints.
    fn render_footer(&self, area: Rect, frame: &mut Frame) {
        if area.is_empty() {
            return;
        }
        let mode_str = match self.state.mode {
            ShowcaseMode::Normal => "NORMAL",
            ShowcaseMode::Inspect => "INSPECT",
            ShowcaseMode::Search => "SEARCH",
        };
        let mode_color = match self.state.mode {
            ShowcaseMode::Normal => PackedRgba::rgb(80, 200, 120),
            ShowcaseMode::Inspect => PackedRgba::rgb(80, 180, 255),
            ShowcaseMode::Search => PackedRgba::rgb(255, 200, 80),
        };
        let mode_cell = Cell::from_char(' ').with_fg(mode_color);
        let end =
            frame.print_text_clipped(area.x, area.y, mode_str, mode_cell, area.x + area.width);

        let hints = " j/k:sample t:tier p:palette H/J/K/L:pan []/{}:size o:reset ?:help";
        let hint_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(100, 100, 120));
        frame.print_text_clipped(end + 1, area.y, hints, hint_cell, area.x + area.width);
    }

    /// Render the help/legend overlay centered on the given area.
    fn render_help_overlay(&self, area: Rect, frame: &mut Frame) {
        if area.is_empty() {
            return;
        }

        // Build help content: category → [(key, description)]
        let sections: &[(&str, &[(&str, &str)])] = &[
            (
                "Navigation",
                &[
                    ("j / Down", "Next sample"),
                    ("k / Up", "Previous sample"),
                    ("Tab", "Select next node"),
                    ("S-Tab", "Select previous node"),
                    ("/", "Enter search mode"),
                ],
            ),
            (
                "Render Config",
                &[
                    ("t", "Cycle tier (Auto/Compact/Normal/Rich)"),
                    ("g", "Toggle glyph mode (Unicode/ASCII)"),
                    ("b", "Cycle render mode (Auto/Cell/Braille/...)"),
                    ("s", "Toggle styles (classDef/style)"),
                    ("w", "Cycle wrap mode (None/Word/Char/WordChar)"),
                    ("l", "Cycle layout mode (Dense/Normal/Spacious/Auto)"),
                    ("r", "Force relayout"),
                    ("p / P", "Cycle palette forward / backward"),
                ],
            ),
            (
                "Viewport",
                &[
                    ("+ / =", "Zoom in"),
                    ("-", "Zoom out"),
                    ("0", "Reset zoom (+ clear pan)"),
                    ("f", "Fit to view (+ clear pan)"),
                    ("H / J / K / L", "Pan left / down / up / right"),
                    ("] / [", "Increase / decrease viewport width"),
                    ("} / {", "Increase / decrease viewport height"),
                    ("o", "Reset viewport override"),
                ],
            ),
            (
                "Panels",
                &[
                    ("m", "Toggle metrics panel"),
                    ("c", "Toggle controls strip"),
                    ("d", "Toggle detail panel"),
                    ("i", "Toggle status log"),
                    ("?", "Toggle this help overlay"),
                    ("Esc", "Deselect node / exit search / collapse"),
                ],
            ),
        ];

        // Compute overlay dimensions.
        let content_width: u16 = 50;
        let mut content_lines: u16 = 2; // title + blank
        for (name, entries) in sections {
            content_lines += 1; // section header
            content_lines += entries.len() as u16;
            content_lines += 1; // blank separator
            let _ = name;
        }
        let overlay_w = (content_width + 4).min(area.width);
        let overlay_h = (content_lines + 3).min(area.height);

        // Center the overlay.
        let ox = area.x + area.width.saturating_sub(overlay_w) / 2;
        let oy = area.y + area.height.saturating_sub(overlay_h) / 2;
        let overlay = Rect::new(ox, oy, overlay_w, overlay_h);

        // Clear the overlay area with a dark background.
        let bg = Cell::from_char(' ').with_fg(PackedRgba::rgb(40, 40, 60));
        for row in overlay.y..overlay.y + overlay.height {
            for col in overlay.x..overlay.x + overlay.width {
                frame.buffer.set(col, row, bg);
            }
        }

        // Draw border.
        let border_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(100, 140, 200));
        frame.draw_border(overlay, BorderChars::ROUNDED, border_cell);

        // Title.
        let title = " Help (? to close) ";
        let title_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(200, 220, 255));
        frame.print_text_clipped(
            overlay.x + 2,
            overlay.y,
            title,
            title_cell,
            overlay.x + overlay.width - 1,
        );

        let max_x = overlay.x + overlay.width - 2;
        let mut y = overlay.y + 2;
        let max_y = overlay.y + overlay.height - 1;

        let section_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(140, 200, 255));
        let key_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(255, 220, 140));
        let desc_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(180, 180, 200));

        for (section_name, entries) in sections {
            if y >= max_y {
                break;
            }
            // Section header.
            frame.print_text_clipped(overlay.x + 2, y, section_name, section_cell, max_x);
            y += 1;

            for (k, desc) in *entries {
                if y >= max_y {
                    break;
                }
                let end = frame.print_text_clipped(overlay.x + 3, y, k, key_cell, max_x);
                // Pad key column to fixed width for alignment.
                let desc_x = (overlay.x + 19).max(end + 1);
                frame.print_text_clipped(desc_x, y, desc, desc_cell, max_x);
                y += 1;
            }
            y += 1; // blank line between sections
        }

        // Current state legend at bottom if space allows.
        if y + 2 < max_y {
            let s = &self.state;
            let legend = format!(
                "Mode:{} Tier:{} Palette:{} Zoom:{:.0}%",
                s.mode.as_str(),
                s.tier,
                s.palette,
                s.viewport_zoom * 100.0,
            );
            let legend_cell = Cell::from_char(' ').with_fg(PackedRgba::rgb(120, 160, 140));
            frame.print_text_clipped(overlay.x + 2, y, &legend, legend_cell, max_x);
        }
    }

    /// Render the diagram area using the full Mermaid pipeline with caching.
    fn render_diagram(&self, area: Rect, frame: &mut Frame) {
        if area.is_empty() {
            return;
        }
        let border = Cell::from_char(' ').with_fg(PackedRgba::rgb(60, 60, 80));
        frame.draw_border(area, BorderChars::SQUARE, border);

        let palette = DiagramPalette::from_preset(self.state.palette);
        let title = format!(" {} [{}] ", self.state.selected_name(), self.state.palette);
        let title_cell = Cell::from_char(' ').with_fg(palette.node_border);
        frame.print_text_clipped(
            area.x + 1,
            area.y,
            &title,
            title_cell,
            area.x + area.width - 1,
        );

        // Inner area (inside the border).
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        if inner.is_empty() {
            return;
        }

        // Run the pipeline and blit.
        self.ensure_render_cache(inner);
        let cache = self.cache.borrow();
        self.blit_buffer(frame, inner, &cache.buffer, self.state.viewport_pan);
    }
}

impl Screen for MermaidMegaShowcaseScreen {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(key) = event
            && let Some(action) = self.handle_key(key)
        {
            self.state.apply(action);
        }
        Cmd::None
    }

    fn tick(&mut self, _tick_count: u64) {
        let cache = self.cache.borrow();
        if cache.layout_budget_exceeded && !cache.budget_warning_logged {
            let ms = cache.layout_ms.unwrap_or(0.0);
            let sample = self.state.selected_name();
            let mode = self.state.layout_mode.as_str();
            drop(cache);
            self.state.log_action(
                "budget",
                format!("layout {ms:.1}ms > {LAYOUT_BUDGET_MS:.0}ms budget ({sample}, {mode})"),
            );
            self.cache.borrow_mut().budget_warning_logged = true;
        }
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let regions = LayoutRegions::compute(area, &self.state.panels);

        // Render in layer order: background panels first, then diagram, then overlay.
        if !regions.controls.is_empty() {
            self.render_controls(regions.controls, frame);
        }
        if !regions.side_panel.is_empty() {
            self.render_side_panel(regions.side_panel, frame);
        }
        self.render_diagram(regions.diagram, frame);
        self.render_footer(regions.footer, frame);

        // Help overlay renders last (on top of everything).
        if self.state.panels.help_overlay {
            self.render_help_overlay(area, frame);
        }
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "j/↓",
                action: "Next sample",
            },
            HelpEntry {
                key: "k/↑",
                action: "Previous sample",
            },
            HelpEntry {
                key: "t",
                action: "Cycle tier",
            },
            HelpEntry {
                key: "g",
                action: "Toggle glyph mode",
            },
            HelpEntry {
                key: "b",
                action: "Cycle render mode",
            },
            HelpEntry {
                key: "s",
                action: "Toggle styles",
            },
            HelpEntry {
                key: "w",
                action: "Cycle wrap mode",
            },
            HelpEntry {
                key: "l",
                action: "Cycle layout mode",
            },
            HelpEntry {
                key: "r",
                action: "Force relayout",
            },
            HelpEntry {
                key: "p/P",
                action: "Cycle palette",
            },
            HelpEntry {
                key: "Tab",
                action: "Select next node",
            },
            HelpEntry {
                key: "S-Tab",
                action: "Select previous node",
            },
            HelpEntry {
                key: "/",
                action: "Search",
            },
            HelpEntry {
                key: "+/-",
                action: "Zoom in/out",
            },
            HelpEntry {
                key: "0",
                action: "Reset zoom",
            },
            HelpEntry {
                key: "f",
                action: "Fit to view",
            },
            HelpEntry {
                key: "H/J/K/L",
                action: "Pan viewport",
            },
            HelpEntry {
                key: "]/[",
                action: "Viewport width +/-",
            },
            HelpEntry {
                key: "}/{",
                action: "Viewport height +/-",
            },
            HelpEntry {
                key: "o",
                action: "Reset viewport override",
            },
            HelpEntry {
                key: "m",
                action: "Toggle metrics",
            },
            HelpEntry {
                key: "c",
                action: "Toggle controls",
            },
            HelpEntry {
                key: "d",
                action: "Toggle detail",
            },
            HelpEntry {
                key: "i",
                action: "Toggle status log",
            },
            HelpEntry {
                key: "?",
                action: "Toggle help",
            },
            HelpEntry {
                key: "Esc",
                action: "Deselect / collapse",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Mermaid Mega Showcase"
    }

    fn tab_label(&self) -> &'static str {
        "MermaidMega"
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_regions_full_size() {
        let area = Rect::new(0, 0, 120, 40);
        let panels = PanelVisibility::default();
        let regions = LayoutRegions::compute(area, &panels);

        assert!(regions.diagram.width > 0);
        assert!(regions.diagram.height > 0);
        assert!(regions.footer.height > 0);
        assert!(regions.controls.height > 0);
        assert!(
            regions.side_panel.width > 0,
            "metrics panel should be visible at 120 cols"
        );
    }

    #[test]
    fn layout_regions_narrow_collapses_side() {
        let area = Rect::new(0, 0, 80, 24);
        let panels = PanelVisibility::default();
        let regions = LayoutRegions::compute(area, &panels);

        assert_eq!(
            regions.side_panel.width, 0,
            "side panel should collapse at 80 cols"
        );
        assert!(regions.diagram.width > 60);
    }

    #[test]
    fn layout_regions_tiny_gives_all_to_diagram() {
        let area = Rect::new(0, 0, 8, 4);
        let panels = PanelVisibility::default();
        let regions = LayoutRegions::compute(area, &panels);

        assert_eq!(regions.diagram, area);
    }

    #[test]
    fn layout_regions_no_panels() {
        let area = Rect::new(0, 0, 120, 40);
        let panels = PanelVisibility {
            controls: false,
            metrics: false,
            detail: false,
            status_log: false,
            help_overlay: false,
        };
        let regions = LayoutRegions::compute(area, &panels);

        assert_eq!(regions.controls.height, 0);
        assert_eq!(regions.side_panel.width, 0);
        assert!(regions.diagram.height > 30);
    }

    #[test]
    fn state_default_is_normal_mode() {
        let state = MermaidMegaState::default();
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert_eq!(state.palette, DiagramPalettePreset::Default);
        assert_eq!(state.selected_node, None);
    }

    #[test]
    fn state_apply_cycle_palette() {
        let mut state = MermaidMegaState::default();
        let epoch_before = state.render_epoch;
        state.apply(MegaAction::CyclePalette);
        assert_eq!(state.palette, DiagramPalettePreset::Corporate);
        assert!(state.render_epoch > epoch_before);
    }

    #[test]
    fn state_apply_select_node_enters_inspect() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::SelectNextNode);
        assert_eq!(state.mode, ShowcaseMode::Inspect);
        assert_eq!(state.selected_node, Some(0));
    }

    #[test]
    fn state_apply_deselect_returns_normal() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::SelectNextNode);
        state.apply(MegaAction::DeselectNode);
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert_eq!(state.selected_node, None);
    }

    #[test]
    fn state_apply_enter_search_mode() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::EnterSearch);
        assert_eq!(state.mode, ShowcaseMode::Search);
        assert!(state.search_query.is_some());
    }

    #[test]
    fn state_apply_escape_from_search() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::EnterSearch);
        state.apply(MegaAction::CollapsePanels);
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert!(state.search_query.is_none());
    }

    #[test]
    fn state_to_config_applies_palette() {
        let state = MermaidMegaState {
            palette: DiagramPalettePreset::Neon,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        assert_eq!(config.palette, DiagramPalettePreset::Neon);
    }

    #[test]
    fn screen_new_does_not_panic() {
        let _screen = MermaidMegaShowcaseScreen::new();
    }

    #[test]
    fn layout_mode_cycles_through_all() {
        let mut mode = LayoutMode::Dense;
        let start = mode;
        for _ in 0..4 {
            mode = mode.next();
        }
        assert_eq!(mode, start);
    }

    #[test]
    fn layout_regions_deterministic() {
        let area = Rect::new(0, 0, 120, 40);
        let panels = PanelVisibility::default();
        let r1 = LayoutRegions::compute(area, &panels);
        let r2 = LayoutRegions::compute(area, &panels);
        assert_eq!(r1.diagram, r2.diagram);
        assert_eq!(r1.footer, r2.footer);
        assert_eq!(r1.controls, r2.controls);
        assert_eq!(r1.side_panel, r2.side_panel);
    }

    #[test]
    fn status_log_records_actions() {
        let mut state = MermaidMegaState::default();
        assert!(state.status_log.is_empty());
        state.apply(MegaAction::CycleTier);
        assert_eq!(state.status_log.len(), 1);
        assert_eq!(state.status_log[0].action, "action");
        assert!(state.status_log[0].detail.contains("CycleTier"));
    }

    #[test]
    fn status_log_caps_at_limit() {
        let mut state = MermaidMegaState::default();
        for _ in 0..STATUS_LOG_CAP + 10 {
            state.apply(MegaAction::CycleTier);
        }
        assert_eq!(state.status_log.len(), STATUS_LOG_CAP);
    }

    #[test]
    fn mega_samples_non_empty() {
        assert!(!MEGA_SAMPLES.is_empty());
        for sample in MEGA_SAMPLES {
            assert!(!sample.name.is_empty());
            assert!(!sample.source.is_empty());
        }
    }

    #[test]
    fn selected_source_wraps_around() {
        let state = MermaidMegaState {
            selected_sample: MEGA_SAMPLES.len() + 2,
            ..MermaidMegaState::default()
        };
        assert!(state.selected_source().is_some());
        assert_eq!(state.selected_name(), MEGA_SAMPLES[2].name);
    }

    #[test]
    fn render_cache_starts_empty() {
        let cache = MegaRenderCache::empty();
        assert!(cache.ir.is_none());
        assert!(cache.layout.is_none());
        assert_eq!(cache.cache_hits, 0);
        assert_eq!(cache.cache_misses, 0);
        assert!(cache.parse_ms.is_none());
    }

    #[test]
    fn ensure_cache_populates_on_first_call() {
        let screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);

        let cache = screen.cache.borrow();
        assert!(
            cache.ir.is_some(),
            "IR should be populated after first render"
        );
        assert!(
            cache.layout.is_some(),
            "layout should be populated after first render"
        );
        assert!(cache.parse_ms.is_some(), "parse timing should be recorded");
        assert!(
            cache.layout_ms.is_some(),
            "layout timing should be recorded"
        );
        assert!(
            cache.render_ms.is_some(),
            "render timing should be recorded"
        );
        assert_eq!(cache.cache_misses, 1);
        assert_eq!(cache.cache_hits, 0);
    }

    #[test]
    fn ensure_cache_hits_on_repeat() {
        let screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        screen.ensure_render_cache(area);

        let cache = screen.cache.borrow();
        assert_eq!(cache.cache_misses, 1);
        assert_eq!(cache.cache_hits, 1);
        assert!(cache.last_cache_hit);
    }

    #[test]
    fn config_dense_has_higher_budget() {
        let state = MermaidMegaState {
            layout_mode: LayoutMode::Dense,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        assert_eq!(config.layout_iteration_budget, 400);
    }

    #[test]
    fn layout_spacing_dense_tighter() {
        let state = MermaidMegaState {
            layout_mode: LayoutMode::Dense,
            ..MermaidMegaState::default()
        };
        let spacing = state.layout_spacing();
        assert!(spacing.rank_gap < mermaid_layout::LayoutSpacing::default().rank_gap);
    }

    #[test]
    fn pan_left_decreases_x() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::PanLeft);
        assert!(state.viewport_pan.0 < 0);
        assert_eq!(state.viewport_pan.1, 0);
    }

    #[test]
    fn pan_right_increases_x() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::PanRight);
        assert!(state.viewport_pan.0 > 0);
        assert_eq!(state.viewport_pan.1, 0);
    }

    #[test]
    fn pan_up_decreases_y() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::PanUp);
        assert_eq!(state.viewport_pan.0, 0);
        assert!(state.viewport_pan.1 < 0);
    }

    #[test]
    fn pan_down_increases_y() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::PanDown);
        assert_eq!(state.viewport_pan.0, 0);
        assert!(state.viewport_pan.1 > 0);
    }

    #[test]
    fn zoom_reset_clears_pan() {
        let mut state = MermaidMegaState {
            viewport_pan: (10, 20),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::ZoomReset);
        assert_eq!(state.viewport_pan, (0, 0));
    }

    #[test]
    fn fit_to_view_clears_pan() {
        let mut state = MermaidMegaState {
            viewport_pan: (5, 5),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::FitToView);
        assert_eq!(state.viewport_pan, (0, 0));
    }

    #[test]
    fn viewport_override_increase_sets_default() {
        let mut state = MermaidMegaState::default();
        assert!(state.viewport_size_override.is_none());
        let epoch = state.render_epoch;
        state.apply(MegaAction::IncreaseViewportWidth);
        let expected_cols =
            (VIEWPORT_OVERRIDE_DEFAULT_COLS as i32 + VIEWPORT_OVERRIDE_STEP_COLS as i32) as u16;
        let expected_rows = VIEWPORT_OVERRIDE_DEFAULT_ROWS;
        assert_eq!(
            state.viewport_size_override,
            Some((expected_cols, expected_rows))
        );
        assert!(state.render_epoch > epoch);
    }

    #[test]
    fn viewport_override_reset_clears() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::IncreaseViewportHeight);
        assert!(state.viewport_size_override.is_some());
        let epoch = state.render_epoch;
        state.apply(MegaAction::ResetViewportOverride);
        assert!(state.viewport_size_override.is_none());
        assert!(state.render_epoch > epoch);
    }

    #[test]
    fn viewport_override_min_clamped() {
        let mut state = MermaidMegaState {
            viewport_size_override: Some((VIEWPORT_OVERRIDE_MIN_COLS, VIEWPORT_OVERRIDE_MIN_ROWS)),
            ..MermaidMegaState::default()
        };
        // Try to decrease below minimum — should clamp
        state.adjust_viewport_override(-100, -100);
        let (cols, rows) = state.viewport_size_override.unwrap();
        assert!(cols >= VIEWPORT_OVERRIDE_MIN_COLS);
        assert!(rows >= VIEWPORT_OVERRIDE_MIN_ROWS);
    }

    #[test]
    fn key_shift_h_maps_to_pan_left() {
        use ftui_core::event::{KeyCode, KeyEvent, KeyEventKind, Modifiers};
        let screen = MermaidMegaShowcaseScreen::new();
        let event = KeyEvent {
            code: KeyCode::Char('H'),
            modifiers: Modifiers::SHIFT,
            kind: KeyEventKind::Press,
        };
        assert_eq!(screen.handle_key(&event), Some(MegaAction::PanLeft));
    }

    #[test]
    fn key_bracket_maps_to_viewport_width() {
        use ftui_core::event::{KeyCode, KeyEvent, KeyEventKind, Modifiers};
        let screen = MermaidMegaShowcaseScreen::new();
        let event = KeyEvent {
            code: KeyCode::Char(']'),
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        };
        assert_eq!(
            screen.handle_key(&event),
            Some(MegaAction::IncreaseViewportWidth)
        );
    }

    #[test]
    fn key_o_maps_to_reset_viewport() {
        use ftui_core::event::{KeyCode, KeyEvent, KeyEventKind, Modifiers};
        let screen = MermaidMegaShowcaseScreen::new();
        let event = KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        };
        assert_eq!(
            screen.handle_key(&event),
            Some(MegaAction::ResetViewportOverride)
        );
    }

    // ── Exhaustive keymap coverage ───────────────────────────────────

    /// Helper: create a key press event.
    fn key(code: ftui_core::event::KeyCode) -> ftui_core::event::KeyEvent {
        ftui_core::event::KeyEvent {
            code,
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: ftui_core::event::KeyEventKind::Press,
        }
    }

    fn shift_key(ch: char) -> ftui_core::event::KeyEvent {
        ftui_core::event::KeyEvent {
            code: ftui_core::event::KeyCode::Char(ch),
            modifiers: ftui_core::event::Modifiers::SHIFT,
            kind: ftui_core::event::KeyEventKind::Press,
        }
    }

    #[test]
    fn keymap_sample_navigation() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('j'))),
            Some(MegaAction::NextSample)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Down)),
            Some(MegaAction::NextSample)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('k'))),
            Some(MegaAction::PrevSample)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Up)),
            Some(MegaAction::PrevSample)
        );
    }

    #[test]
    fn keymap_render_config() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('t'))),
            Some(MegaAction::CycleTier)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('g'))),
            Some(MegaAction::ToggleGlyphMode)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('b'))),
            Some(MegaAction::CycleRenderMode)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('s'))),
            Some(MegaAction::ToggleStyles)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('w'))),
            Some(MegaAction::CycleWrapMode)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('l'))),
            Some(MegaAction::CycleLayoutMode)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('r'))),
            Some(MegaAction::ForceRelayout)
        );
    }

    #[test]
    fn keymap_palette() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('p'))),
            Some(MegaAction::CyclePalette)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('P'))),
            Some(MegaAction::PrevPalette)
        );
    }

    #[test]
    fn keymap_zoom() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('+'))),
            Some(MegaAction::ZoomIn)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('='))),
            Some(MegaAction::ZoomIn)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('-'))),
            Some(MegaAction::ZoomOut)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('0'))),
            Some(MegaAction::ZoomReset)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('f'))),
            Some(MegaAction::FitToView)
        );
    }

    #[test]
    fn keymap_pan_shift_hjkl() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('H'))),
            Some(MegaAction::PanLeft)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('J'))),
            Some(MegaAction::PanDown)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('K'))),
            Some(MegaAction::PanUp)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('L'))),
            Some(MegaAction::PanRight)
        );
    }

    #[test]
    fn keymap_viewport_size() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char(']'))),
            Some(MegaAction::IncreaseViewportWidth)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('['))),
            Some(MegaAction::DecreaseViewportWidth)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('}'))),
            Some(MegaAction::IncreaseViewportHeight)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('{'))),
            Some(MegaAction::DecreaseViewportHeight)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('o'))),
            Some(MegaAction::ResetViewportOverride)
        );
    }

    #[test]
    fn keymap_panels() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('m'))),
            Some(MegaAction::ToggleMetrics)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('c'))),
            Some(MegaAction::ToggleControls)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('d'))),
            Some(MegaAction::ToggleDetail)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('i'))),
            Some(MegaAction::ToggleStatusLog)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('?'))),
            Some(MegaAction::ToggleHelp)
        );
    }

    #[test]
    fn keymap_node_search_escape() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(
            s.handle_key(&key(KeyCode::Tab)),
            Some(MegaAction::SelectNextNode)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::BackTab)),
            Some(MegaAction::SelectPrevNode)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Char('/'))),
            Some(MegaAction::EnterSearch)
        );
        assert_eq!(
            s.handle_key(&key(KeyCode::Escape)),
            Some(MegaAction::CollapsePanels)
        );
    }

    #[test]
    fn keymap_unbound_returns_none() {
        use ftui_core::event::KeyCode;
        let s = MermaidMegaShowcaseScreen::new();
        assert_eq!(s.handle_key(&key(KeyCode::Char('x'))), None);
        assert_eq!(s.handle_key(&key(KeyCode::Char('z'))), None);
        assert_eq!(s.handle_key(&key(KeyCode::F(1))), None);
        assert_eq!(s.handle_key(&key(KeyCode::Enter)), None);
    }

    // ── State transition: tier cycling ───────────────────────────────

    #[test]
    fn cycle_tier_full_loop() {
        let mut state = MermaidMegaState::default();
        assert_eq!(state.tier, MermaidTier::Auto);
        state.apply(MegaAction::CycleTier);
        assert_eq!(state.tier, MermaidTier::Compact);
        state.apply(MegaAction::CycleTier);
        assert_eq!(state.tier, MermaidTier::Normal);
        state.apply(MegaAction::CycleTier);
        assert_eq!(state.tier, MermaidTier::Rich);
        state.apply(MegaAction::CycleTier);
        assert_eq!(state.tier, MermaidTier::Auto);
    }

    #[test]
    fn cycle_tier_bumps_layout_and_render_epochs() {
        let mut state = MermaidMegaState::default();
        let le = state.layout_epoch;
        let re = state.render_epoch;
        state.apply(MegaAction::CycleTier);
        assert!(state.layout_epoch > le);
        assert!(state.render_epoch > re);
    }

    // ── State transition: glyph mode ─────────────────────────────────

    #[test]
    fn toggle_glyph_mode_oscillates() {
        let mut state = MermaidMegaState::default();
        assert_eq!(state.glyph_mode, MermaidGlyphMode::Unicode);
        state.apply(MegaAction::ToggleGlyphMode);
        assert_eq!(state.glyph_mode, MermaidGlyphMode::Ascii);
        state.apply(MegaAction::ToggleGlyphMode);
        assert_eq!(state.glyph_mode, MermaidGlyphMode::Unicode);
    }

    #[test]
    fn toggle_glyph_bumps_render_not_layout() {
        let mut state = MermaidMegaState::default();
        let le = state.layout_epoch;
        let re = state.render_epoch;
        state.apply(MegaAction::ToggleGlyphMode);
        assert_eq!(state.layout_epoch, le, "glyph should not bump layout");
        assert!(state.render_epoch > re);
    }

    // ── State transition: render mode cycling ────────────────────────

    #[test]
    fn cycle_render_mode_full_loop() {
        let mut state = MermaidMegaState::default();
        assert_eq!(state.render_mode, MermaidRenderMode::Auto);
        state.apply(MegaAction::CycleRenderMode);
        assert_eq!(state.render_mode, MermaidRenderMode::CellOnly);
        state.apply(MegaAction::CycleRenderMode);
        assert_eq!(state.render_mode, MermaidRenderMode::Braille);
        state.apply(MegaAction::CycleRenderMode);
        assert_eq!(state.render_mode, MermaidRenderMode::Block);
        state.apply(MegaAction::CycleRenderMode);
        assert_eq!(state.render_mode, MermaidRenderMode::HalfBlock);
        state.apply(MegaAction::CycleRenderMode);
        assert_eq!(state.render_mode, MermaidRenderMode::Auto);
    }

    // ── State transition: wrap mode cycling ──────────────────────────

    #[test]
    fn cycle_wrap_mode_full_loop() {
        let mut state = MermaidMegaState::default();
        assert_eq!(state.wrap_mode, MermaidWrapMode::WordChar);
        // WordChar → None → Word → Char → WordChar
        state.apply(MegaAction::CycleWrapMode);
        state.apply(MegaAction::CycleWrapMode);
        state.apply(MegaAction::CycleWrapMode);
        state.apply(MegaAction::CycleWrapMode);
        assert_eq!(state.wrap_mode, MermaidWrapMode::WordChar);
    }

    #[test]
    fn cycle_wrap_bumps_layout() {
        let mut state = MermaidMegaState::default();
        let le = state.layout_epoch;
        state.apply(MegaAction::CycleWrapMode);
        assert!(state.layout_epoch > le);
    }

    // ── State transition: styles toggle ──────────────────────────────

    #[test]
    fn toggle_styles_flips() {
        let mut state = MermaidMegaState::default();
        assert!(state.styles_enabled);
        state.apply(MegaAction::ToggleStyles);
        assert!(!state.styles_enabled);
        state.apply(MegaAction::ToggleStyles);
        assert!(state.styles_enabled);
    }

    #[test]
    fn toggle_styles_bumps_render_not_layout() {
        let mut state = MermaidMegaState::default();
        let le = state.layout_epoch;
        state.apply(MegaAction::ToggleStyles);
        assert_eq!(state.layout_epoch, le);
    }

    // ── State transition: palette ────────────────────────────────────

    #[test]
    fn cycle_palette_changes_preset() {
        let mut state = MermaidMegaState::default();
        let orig = state.palette;
        state.apply(MegaAction::CyclePalette);
        assert_ne!(state.palette, orig);
    }

    #[test]
    fn prev_palette_reverses_cycle() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::CyclePalette);
        let after_next = state.palette;
        state.apply(MegaAction::PrevPalette);
        assert_eq!(state.palette, DiagramPalettePreset::Default);
        let _ = after_next; // suppress unused
    }

    // ── State transition: zoom ───────────────────────────────────────

    #[test]
    fn zoom_in_increases() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::ZoomIn);
        assert!(state.viewport_zoom > 1.0);
    }

    #[test]
    fn zoom_out_decreases() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::ZoomOut);
        assert!(state.viewport_zoom < 1.0);
    }

    #[test]
    fn zoom_in_clamped_at_max() {
        let mut state = MermaidMegaState::default();
        for _ in 0..50 {
            state.apply(MegaAction::ZoomIn);
        }
        assert!(state.viewport_zoom <= 4.0);
    }

    #[test]
    fn zoom_out_clamped_at_min() {
        let mut state = MermaidMegaState::default();
        for _ in 0..50 {
            state.apply(MegaAction::ZoomOut);
        }
        assert!(state.viewport_zoom >= 0.25);
    }

    // ── State transition: force relayout ─────────────────────────────

    #[test]
    fn force_relayout_bumps_layout_epoch() {
        let mut state = MermaidMegaState::default();
        let le = state.layout_epoch;
        state.apply(MegaAction::ForceRelayout);
        assert!(state.layout_epoch > le);
    }

    // ── State transition: sample navigation ──────────────────────────

    #[test]
    fn next_sample_increments_and_bumps_analysis() {
        let mut state = MermaidMegaState::default();
        let ae = state.analysis_epoch;
        state.apply(MegaAction::NextSample);
        assert_eq!(state.selected_sample, 1);
        assert!(state.analysis_epoch > ae);
    }

    #[test]
    fn prev_sample_decrements_and_bumps_analysis() {
        let mut state = MermaidMegaState {
            selected_sample: 3,
            ..MermaidMegaState::default()
        };
        let ae = state.analysis_epoch;
        state.apply(MegaAction::PrevSample);
        assert_eq!(state.selected_sample, 2);
        assert!(state.analysis_epoch > ae);
    }

    #[test]
    fn sample_navigation_clears_selected_node() {
        let mut state = MermaidMegaState {
            selected_node: Some(3),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::NextSample);
        assert_eq!(state.selected_node, None);
    }

    #[test]
    fn next_sample_wraps_via_modulo() {
        let mut state = MermaidMegaState {
            selected_sample: MEGA_SAMPLES.len() - 1,
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::NextSample);
        // wrapping_add — the selected_source() handles modulo
        assert_eq!(state.selected_name(), MEGA_SAMPLES[0].name);
    }

    // ── State transition: node selection ──────────────────────────────

    #[test]
    fn select_next_node_from_none_starts_at_zero() {
        let mut state = MermaidMegaState::default();
        assert_eq!(state.selected_node, None);
        state.apply(MegaAction::SelectNextNode);
        assert_eq!(state.selected_node, Some(0));
        assert_eq!(state.mode, ShowcaseMode::Inspect);
    }

    #[test]
    fn select_next_node_increments() {
        let mut state = MermaidMegaState {
            selected_node: Some(2),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::SelectNextNode);
        assert_eq!(state.selected_node, Some(3));
    }

    #[test]
    fn select_prev_node_from_none_starts_at_zero() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::SelectPrevNode);
        assert_eq!(state.selected_node, Some(0));
        assert_eq!(state.mode, ShowcaseMode::Inspect);
    }

    #[test]
    fn select_prev_node_saturates_at_zero() {
        let mut state = MermaidMegaState {
            selected_node: Some(0),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::SelectPrevNode);
        assert_eq!(state.selected_node, Some(0));
    }

    #[test]
    fn deselect_node_clears_and_returns_normal() {
        let mut state = MermaidMegaState {
            selected_node: Some(5),
            mode: ShowcaseMode::Inspect,
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::DeselectNode);
        assert_eq!(state.selected_node, None);
        assert_eq!(state.mode, ShowcaseMode::Normal);
    }

    // ── State transition: search mode ────────────────────────────────

    #[test]
    fn enter_search_sets_mode_and_query() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::EnterSearch);
        assert_eq!(state.mode, ShowcaseMode::Search);
        assert_eq!(state.search_query, Some(String::new()));
    }

    #[test]
    fn exit_search_clears_and_returns_normal() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::EnterSearch);
        state.apply(MegaAction::ExitSearch);
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert_eq!(state.search_query, None);
    }

    // ── State transition: panel toggles ──────────────────────────────

    #[test]
    fn toggle_metrics_panel() {
        let mut state = MermaidMegaState::default();
        let before = state.panels.metrics;
        state.apply(MegaAction::ToggleMetrics);
        assert_ne!(state.panels.metrics, before);
        state.apply(MegaAction::ToggleMetrics);
        assert_eq!(state.panels.metrics, before);
    }

    #[test]
    fn toggle_controls_panel() {
        let mut state = MermaidMegaState::default();
        let before = state.panels.controls;
        state.apply(MegaAction::ToggleControls);
        assert_ne!(state.panels.controls, before);
    }

    #[test]
    fn toggle_detail_panel() {
        let mut state = MermaidMegaState::default();
        assert!(!state.panels.detail);
        state.apply(MegaAction::ToggleDetail);
        assert!(state.panels.detail);
    }

    #[test]
    fn toggle_status_log_panel() {
        let mut state = MermaidMegaState::default();
        assert!(!state.panels.status_log);
        state.apply(MegaAction::ToggleStatusLog);
        assert!(state.panels.status_log);
    }

    #[test]
    fn toggle_help_overlay() {
        let mut state = MermaidMegaState::default();
        assert!(!state.panels.help_overlay);
        state.apply(MegaAction::ToggleHelp);
        assert!(state.panels.help_overlay);
    }

    // ── State transition: collapse panels (context-dependent) ────────

    #[test]
    fn collapse_from_inspect_deselects_node() {
        let mut state = MermaidMegaState {
            mode: ShowcaseMode::Inspect,
            selected_node: Some(3),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::CollapsePanels);
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert_eq!(state.selected_node, None);
    }

    #[test]
    fn collapse_from_search_clears_query() {
        let mut state = MermaidMegaState {
            mode: ShowcaseMode::Search,
            search_query: Some("test".into()),
            ..MermaidMegaState::default()
        };
        state.apply(MegaAction::CollapsePanels);
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert_eq!(state.search_query, None);
    }

    #[test]
    fn collapse_from_normal_hides_all_panels() {
        let mut state = MermaidMegaState::default();
        state.panels.controls = true;
        state.panels.metrics = true;
        state.panels.detail = true;
        state.panels.status_log = true;
        state.panels.help_overlay = true;
        state.apply(MegaAction::CollapsePanels);
        assert!(!state.panels.controls);
        assert!(!state.panels.metrics);
        assert!(!state.panels.detail);
        assert!(!state.panels.status_log);
        assert!(!state.panels.help_overlay);
    }

    // ── Epoch cascade tests ──────────────────────────────────────────

    #[test]
    fn bump_analysis_cascades_to_layout_and_render() {
        let mut state = MermaidMegaState::default();
        let ae = state.analysis_epoch;
        let le = state.layout_epoch;
        let re = state.render_epoch;
        state.apply(MegaAction::NextSample); // bumps analysis
        assert!(state.analysis_epoch > ae);
        assert!(state.layout_epoch > le);
        assert!(state.render_epoch > re);
    }

    #[test]
    fn bump_layout_cascades_to_render_not_analysis() {
        let mut state = MermaidMegaState::default();
        let ae = state.analysis_epoch;
        let le = state.layout_epoch;
        let re = state.render_epoch;
        state.apply(MegaAction::CycleLayoutMode); // bumps layout
        assert_eq!(state.analysis_epoch, ae);
        assert!(state.layout_epoch > le);
        assert!(state.render_epoch > re);
    }

    #[test]
    fn bump_render_only_touches_render() {
        let mut state = MermaidMegaState::default();
        let ae = state.analysis_epoch;
        let le = state.layout_epoch;
        let re = state.render_epoch;
        state.apply(MegaAction::ToggleGlyphMode); // bumps render only
        assert_eq!(state.analysis_epoch, ae);
        assert_eq!(state.layout_epoch, le);
        assert!(state.render_epoch > re);
    }

    // ── Sample registry tests ────────────────────────────────────────

    #[test]
    fn mega_samples_all_unique_names() {
        let names: Vec<&str> = MEGA_SAMPLES.iter().map(|s| s.name).collect();
        for (i, name) in names.iter().enumerate() {
            for (j, other) in names.iter().enumerate() {
                if i != j {
                    assert_ne!(name, other, "duplicate sample name at indices {i} and {j}");
                }
            }
        }
    }

    #[test]
    fn mega_samples_cover_expected_diagram_types() {
        let names: Vec<&str> = MEGA_SAMPLES.iter().map(|s| s.name).collect();
        // Each of these keywords should appear in at least one sample name
        for keyword in &[
            "Flow", "Sequence", "Class", "State", "ER", "Pie", "Gantt", "Mindmap",
        ] {
            assert!(
                names.iter().any(|n| n.contains(keyword)),
                "no sample name contains {keyword:?}"
            );
        }
    }

    #[test]
    fn selected_source_at_zero_is_first() {
        let state = MermaidMegaState::default();
        assert_eq!(state.selected_source(), Some(MEGA_SAMPLES[0].source));
        assert_eq!(state.selected_name(), MEGA_SAMPLES[0].name);
    }

    #[test]
    fn selected_source_large_index_wraps() {
        let state = MermaidMegaState {
            selected_sample: usize::MAX,
            ..MermaidMegaState::default()
        };
        // Should not panic; modulo handles it
        let _ = state.selected_source();
        let _ = state.selected_name();
    }

    // ── Config generation tests ──────────────────────────────────────

    #[test]
    fn config_reflects_glyph_mode() {
        let state = MermaidMegaState {
            glyph_mode: MermaidGlyphMode::Ascii,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        assert_eq!(config.glyph_mode, MermaidGlyphMode::Ascii);
    }

    #[test]
    fn config_reflects_render_mode() {
        let state = MermaidMegaState {
            render_mode: MermaidRenderMode::Braille,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        assert_eq!(config.render_mode, MermaidRenderMode::Braille);
    }

    #[test]
    fn config_reflects_styles_enabled() {
        let state = MermaidMegaState {
            styles_enabled: false,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        assert!(!config.enable_styles);
    }

    #[test]
    fn config_spacious_has_lower_budget() {
        let state = MermaidMegaState {
            layout_mode: LayoutMode::Spacious,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        assert_eq!(config.layout_iteration_budget, 140);
        assert_eq!(config.route_budget, 3_000);
    }

    #[test]
    fn config_normal_uses_defaults() {
        let state = MermaidMegaState {
            layout_mode: LayoutMode::Normal,
            ..MermaidMegaState::default()
        };
        let config = state.to_config();
        let default_config = MermaidConfig::default();
        assert_eq!(
            config.layout_iteration_budget,
            default_config.layout_iteration_budget
        );
    }

    // ── Layout spacing tests ─────────────────────────────────────────

    #[test]
    fn layout_spacing_spacious_wider() {
        let state = MermaidMegaState {
            layout_mode: LayoutMode::Spacious,
            ..MermaidMegaState::default()
        };
        let spacing = state.layout_spacing();
        let default_spacing = mermaid_layout::LayoutSpacing::default();
        assert!(spacing.rank_gap > default_spacing.rank_gap);
        assert!(spacing.node_gap > default_spacing.node_gap);
    }

    #[test]
    fn layout_spacing_normal_matches_default() {
        let state = MermaidMegaState {
            layout_mode: LayoutMode::Normal,
            ..MermaidMegaState::default()
        };
        let spacing = state.layout_spacing();
        let default_spacing = mermaid_layout::LayoutSpacing::default();
        assert!((spacing.rank_gap - default_spacing.rank_gap).abs() < f64::EPSILON);
    }

    // ── Layout mode cycling ──────────────────────────────────────────

    #[test]
    fn layout_mode_as_str_matches() {
        assert_eq!(LayoutMode::Dense.as_str(), "dense");
        assert_eq!(LayoutMode::Normal.as_str(), "normal");
        assert_eq!(LayoutMode::Spacious.as_str(), "spacious");
        assert_eq!(LayoutMode::Auto.as_str(), "auto");
    }

    // ── Cache invalidation: viewport resize triggers miss ────────────

    #[test]
    fn cache_miss_on_viewport_resize() {
        let screen = MermaidMegaShowcaseScreen::new();
        let area1 = Rect::new(0, 0, 80, 24);
        let area2 = Rect::new(0, 0, 120, 40);
        screen.ensure_render_cache(area1);
        screen.ensure_render_cache(area2);
        let cache = screen.cache.borrow();
        // Two different viewports → two misses
        assert_eq!(cache.cache_misses, 2);
    }

    #[test]
    fn cache_miss_on_sample_change() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        screen.state.apply(MegaAction::NextSample);
        screen.ensure_render_cache(area);
        let cache = screen.cache.borrow();
        assert_eq!(cache.cache_misses, 2);
        assert!(!cache.last_cache_hit);
    }

    #[test]
    fn cache_hit_when_only_pan_changes() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        // Pan does NOT bump any epoch — cache should still hit
        screen.state.apply(MegaAction::PanLeft);
        screen.ensure_render_cache(area);
        let cache = screen.cache.borrow();
        assert_eq!(cache.cache_hits, 1, "pan should not invalidate cache");
    }

    #[test]
    fn cache_miss_on_zoom_change() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        screen.state.apply(MegaAction::ZoomIn);
        screen.ensure_render_cache(area);
        let cache = screen.cache.borrow();
        assert_eq!(cache.cache_misses, 2, "zoom change should invalidate");
    }

    #[test]
    fn cache_miss_on_render_epoch_bump() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        screen.state.apply(MegaAction::ToggleStyles);
        screen.ensure_render_cache(area);
        let cache = screen.cache.borrow();
        assert_eq!(cache.cache_misses, 2);
    }

    #[test]
    fn cache_miss_on_layout_epoch_bump() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        screen.state.apply(MegaAction::CycleLayoutMode);
        screen.ensure_render_cache(area);
        let cache = screen.cache.borrow();
        assert_eq!(cache.cache_misses, 2);
    }

    #[test]
    fn cache_records_timing() {
        let screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);
        let cache = screen.cache.borrow();
        assert!(cache.parse_ms.unwrap() >= 0.0);
        assert!(cache.layout_ms.unwrap() >= 0.0);
        assert!(cache.render_ms.unwrap() >= 0.0);
    }

    // ── Viewport override edge cases ─────────────────────────────────

    #[test]
    fn viewport_override_decrease_creates_with_default() {
        let mut state = MermaidMegaState::default();
        assert!(state.viewport_size_override.is_none());
        state.apply(MegaAction::DecreaseViewportWidth);
        let (cols, rows) = state.viewport_size_override.unwrap();
        assert_eq!(
            cols,
            (VIEWPORT_OVERRIDE_DEFAULT_COLS as i32 - VIEWPORT_OVERRIDE_STEP_COLS as i32) as u16
        );
        assert_eq!(rows, VIEWPORT_OVERRIDE_DEFAULT_ROWS);
    }

    #[test]
    fn viewport_override_height_increase() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::IncreaseViewportHeight);
        let (cols, rows) = state.viewport_size_override.unwrap();
        assert_eq!(cols, VIEWPORT_OVERRIDE_DEFAULT_COLS);
        assert_eq!(
            rows,
            (VIEWPORT_OVERRIDE_DEFAULT_ROWS as i32 + VIEWPORT_OVERRIDE_STEP_ROWS as i32) as u16
        );
    }

    #[test]
    fn viewport_override_reset_when_none_is_noop() {
        let mut state = MermaidMegaState::default();
        let re = state.render_epoch;
        state.apply(MegaAction::ResetViewportOverride);
        // No override existed, so render epoch should NOT be bumped
        assert_eq!(state.render_epoch, re);
    }

    #[test]
    fn target_viewport_uses_override_when_set() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        screen.state.viewport_size_override = Some((50, 20));
        let area = Rect::new(0, 0, 100, 40);
        let (w, h) = screen.target_viewport_size(area);
        assert_eq!(w, 50);
        assert_eq!(h, 20);
    }

    #[test]
    fn target_viewport_uses_area_when_no_override() {
        let screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 100, 40);
        let (w, h) = screen.target_viewport_size(area);
        assert_eq!(w, 100);
        assert_eq!(h, 40);
    }

    // ── Layout region edge cases ─────────────────────────────────────

    #[test]
    fn layout_regions_very_small_terminal() {
        let area = Rect::new(0, 0, 5, 3);
        let panels = PanelVisibility::default();
        let regions = LayoutRegions::compute(area, &panels);
        assert_eq!(regions.diagram, area);
        assert_eq!(regions.side_panel.width, 0);
        assert_eq!(regions.controls.height, 0);
    }

    #[test]
    fn layout_regions_no_side_panel_below_min_width() {
        let area = Rect::new(0, 0, 50, 30);
        let panels = PanelVisibility::default();
        let regions = LayoutRegions::compute(area, &panels);
        // Below MIN_FULL_WIDTH — side panel should be zero-width
        assert_eq!(regions.side_panel.width, 0);
    }

    #[test]
    fn layout_regions_side_panel_at_full_width() {
        let area = Rect::new(0, 0, 120, 40);
        let panels = PanelVisibility {
            metrics: true,
            ..PanelVisibility::default()
        };
        let regions = LayoutRegions::compute(area, &panels);
        assert!(regions.side_panel.width > 0);
    }

    // ── Pan step magnitude ───────────────────────────────────────────

    #[test]
    fn pan_step_magnitude_correct() {
        let mut state = MermaidMegaState::default();
        state.apply(MegaAction::PanRight);
        assert_eq!(state.viewport_pan.0, PAN_STEP);
        state.apply(MegaAction::PanRight);
        assert_eq!(state.viewport_pan.0, PAN_STEP * 2);
    }

    // ── Default state verification ───────────────────────────────────

    #[test]
    fn default_state_all_fields() {
        let state = MermaidMegaState::default();
        assert_eq!(state.mode, ShowcaseMode::Normal);
        assert_eq!(state.layout_mode, LayoutMode::Auto);
        assert_eq!(state.tier, MermaidTier::Auto);
        assert_eq!(state.glyph_mode, MermaidGlyphMode::Unicode);
        assert_eq!(state.render_mode, MermaidRenderMode::Auto);
        assert_eq!(state.wrap_mode, MermaidWrapMode::WordChar);
        assert_eq!(state.palette, DiagramPalettePreset::Default);
        assert!(state.styles_enabled);
        assert!((state.viewport_zoom - 1.0).abs() < f32::EPSILON);
        assert_eq!(state.viewport_pan, (0, 0));
        assert!(state.viewport_size_override.is_none());
        assert_eq!(state.selected_sample, 0);
        assert!(state.selected_node.is_none());
        assert!(state.search_query.is_none());
        assert_eq!(state.analysis_epoch, 0);
        assert_eq!(state.layout_epoch, 0);
        assert_eq!(state.render_epoch, 0);
        assert!(state.status_log.is_empty());
    }

    #[test]
    fn default_panel_visibility() {
        let panels = PanelVisibility::default();
        assert!(panels.controls);
        assert!(panels.metrics);
        assert!(!panels.detail);
        assert!(!panels.status_log);
        assert!(!panels.help_overlay);
    }

    // ── Every action logs to status log ──────────────────────────────

    #[test]
    fn all_actions_log_to_status_log() {
        let actions = [
            MegaAction::NextSample,
            MegaAction::PrevSample,
            MegaAction::CycleTier,
            MegaAction::ToggleGlyphMode,
            MegaAction::CycleRenderMode,
            MegaAction::CycleWrapMode,
            MegaAction::ToggleStyles,
            MegaAction::CycleLayoutMode,
            MegaAction::ForceRelayout,
            MegaAction::CyclePalette,
            MegaAction::PrevPalette,
            MegaAction::ZoomIn,
            MegaAction::ZoomOut,
            MegaAction::ZoomReset,
            MegaAction::FitToView,
            MegaAction::PanLeft,
            MegaAction::PanRight,
            MegaAction::PanUp,
            MegaAction::PanDown,
            MegaAction::IncreaseViewportWidth,
            MegaAction::DecreaseViewportWidth,
            MegaAction::IncreaseViewportHeight,
            MegaAction::DecreaseViewportHeight,
            MegaAction::ResetViewportOverride,
            MegaAction::ToggleMetrics,
            MegaAction::ToggleControls,
            MegaAction::ToggleDetail,
            MegaAction::ToggleStatusLog,
            MegaAction::ToggleHelp,
            MegaAction::SelectNextNode,
            MegaAction::SelectPrevNode,
            MegaAction::DeselectNode,
            MegaAction::EnterSearch,
            MegaAction::ExitSearch,
            MegaAction::CollapsePanels,
        ];
        let mut state = MermaidMegaState::default();
        for action in actions {
            state.apply(action);
        }
        assert_eq!(state.status_log.len(), actions.len());
    }

    // ── Debounce and budget guardrail tests ─────────────────────────

    #[test]
    fn cache_debounce_fields_initialised() {
        let cache = MegaRenderCache::empty();
        assert!(cache.last_layout_instant.is_none());
        assert!(!cache.layout_budget_exceeded);
        // budget_warning_logged starts true so no false-positive log on init
        assert!(cache.budget_warning_logged);
        assert_eq!(cache.debounce_skips, 0);
    }

    #[test]
    fn ensure_cache_records_layout_instant() {
        let screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);
        screen.ensure_render_cache(area);

        let cache = screen.cache.borrow();
        assert!(
            cache.last_layout_instant.is_some(),
            "layout instant should be set after first render"
        );
    }

    #[test]
    fn debounce_skips_when_budget_exceeded_and_called_rapidly() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);

        // First call populates the cache.
        screen.ensure_render_cache(area);

        // Simulate that the layout was expensive (exceeded budget).
        {
            let mut cache = screen.cache.borrow_mut();
            cache.layout_budget_exceeded = true;
            cache.last_layout_instant = Some(Instant::now());
        }

        let first_misses = screen.cache.borrow().cache_misses;

        // Immediately bump layout epoch and call again — should debounce
        // because budget was exceeded and the call is within the window.
        screen.state.bump_layout();
        screen.ensure_render_cache(area);

        let cache = screen.cache.borrow();
        assert_eq!(
            cache.cache_misses, first_misses,
            "layout should be debounced"
        );
        assert!(cache.debounce_skips > 0, "debounce_skips should increment");
    }

    #[test]
    fn no_debounce_when_budget_is_ok() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        let area = Rect::new(0, 0, 80, 24);

        // First call populates the cache.
        screen.ensure_render_cache(area);

        // Budget is NOT exceeded, so debounce should not apply.
        assert!(!screen.cache.borrow().layout_budget_exceeded);

        let first_misses = screen.cache.borrow().cache_misses;

        // Bump layout epoch and call again — should NOT debounce.
        screen.state.bump_layout();
        screen.ensure_render_cache(area);

        let cache = screen.cache.borrow();
        assert!(
            cache.cache_misses > first_misses,
            "layout should run (no debounce)"
        );
        assert_eq!(cache.debounce_skips, 0, "no debounce skips expected");
    }

    #[test]
    fn budget_exceeded_flag_set_for_slow_layout() {
        // We can't easily simulate a slow layout, but we can verify the flag
        // logic by directly manipulating the cache.
        let mut cache = MegaRenderCache::empty();
        cache.layout_ms = Some(25.0);
        cache.layout_budget_exceeded = 25.0 > LAYOUT_BUDGET_MS;
        assert!(
            cache.layout_budget_exceeded,
            "25ms should exceed 16ms budget"
        );
    }

    #[test]
    fn budget_not_exceeded_for_fast_layout() {
        let mut cache = MegaRenderCache::empty();
        cache.layout_ms = Some(5.0);
        cache.layout_budget_exceeded = 5.0 > LAYOUT_BUDGET_MS;
        assert!(
            !cache.layout_budget_exceeded,
            "5ms should not exceed 16ms budget"
        );
    }

    #[test]
    fn tick_logs_budget_warning() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        // Simulate a budget-exceeding layout by directly setting cache fields.
        {
            let mut cache = screen.cache.borrow_mut();
            cache.layout_ms = Some(20.0);
            cache.layout_budget_exceeded = true;
            cache.budget_warning_logged = false;
        }

        assert!(screen.state.status_log.is_empty());
        screen.tick(1);

        assert_eq!(screen.state.status_log.len(), 1);
        assert_eq!(screen.state.status_log[0].action, "budget");
        assert!(screen.state.status_log[0].detail.contains("20.0ms"));
        assert!(screen.state.status_log[0].detail.contains("budget"));

        // Second tick should not log again.
        screen.tick(2);
        assert_eq!(screen.state.status_log.len(), 1, "duplicate log prevented");
    }

    #[test]
    fn tick_does_nothing_when_budget_ok() {
        let mut screen = MermaidMegaShowcaseScreen::new();
        {
            let mut cache = screen.cache.borrow_mut();
            cache.layout_ms = Some(5.0);
            cache.layout_budget_exceeded = false;
        }
        screen.tick(1);
        assert!(
            screen.state.status_log.is_empty(),
            "no log when budget is fine"
        );
    }

    #[test]
    fn layout_debounce_constant_reasonable() {
        const { assert!(LAYOUT_DEBOUNCE_MS >= 20) };
        const { assert!(LAYOUT_DEBOUNCE_MS <= 200) };
    }

    #[test]
    fn layout_budget_constant_reasonable() {
        const { assert!(LAYOUT_BUDGET_MS >= 8.0) };
        const { assert!(LAYOUT_BUDGET_MS <= 33.0) };
    }

    // ── Help overlay tests ───────────────────────────────────────────

    #[test]
    fn help_overlay_renders_without_panic() {
        use ftui_render::grapheme_pool::GraphemePool;
        let screen = MermaidMegaShowcaseScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(120, 40, &mut pool);
        let area = Rect::new(0, 0, 120, 40);
        screen.render_help_overlay(area, &mut frame);
    }

    #[test]
    fn help_overlay_renders_on_small_area() {
        use ftui_render::grapheme_pool::GraphemePool;
        let screen = MermaidMegaShowcaseScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(40, 15, &mut pool);
        let area = Rect::new(0, 0, 40, 15);
        screen.render_help_overlay(area, &mut frame);
    }

    #[test]
    fn help_overlay_noop_on_empty() {
        use ftui_render::grapheme_pool::GraphemePool;
        let screen = MermaidMegaShowcaseScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(1, 1, &mut pool);
        let area = Rect::new(0, 0, 0, 0);
        screen.render_help_overlay(area, &mut frame);
    }

    #[test]
    fn view_renders_help_when_overlay_on() {
        use ftui_render::grapheme_pool::GraphemePool;
        let mut screen = MermaidMegaShowcaseScreen::new();
        screen.state.panels.help_overlay = true;
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(120, 40, &mut pool);
        let area = Rect::new(0, 0, 120, 40);
        screen.view(&mut frame, area);
    }

    #[test]
    fn view_skips_help_when_overlay_off() {
        use ftui_render::grapheme_pool::GraphemePool;
        let screen = MermaidMegaShowcaseScreen::new();
        assert!(!screen.state.panels.help_overlay);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        let area = Rect::new(0, 0, 80, 24);
        screen.view(&mut frame, area);
    }

    #[test]
    fn keybindings_include_all_categories() {
        let screen = MermaidMegaShowcaseScreen::new();
        let bindings = screen.keybindings();
        let keys: Vec<&str> = bindings.iter().map(|h| h.key).collect();
        // Verify key categories are represented
        assert!(keys.contains(&"j/\u{2193}"), "missing sample nav");
        assert!(keys.contains(&"t"), "missing tier");
        assert!(keys.contains(&"g"), "missing glyph");
        assert!(keys.contains(&"b"), "missing render mode");
        assert!(keys.contains(&"s"), "missing styles");
        assert!(keys.contains(&"w"), "missing wrap");
        assert!(keys.contains(&"l"), "missing layout");
        assert!(keys.contains(&"r"), "missing relayout");
        assert!(keys.contains(&"p/P"), "missing palette");
        assert!(keys.contains(&"+/-"), "missing zoom");
        assert!(keys.contains(&"0"), "missing zoom reset");
        assert!(keys.contains(&"f"), "missing fit");
        assert!(keys.contains(&"H/J/K/L"), "missing pan");
        assert!(keys.contains(&"]/["), "missing viewport width");
        assert!(keys.contains(&"}/{"), "missing viewport height");
        assert!(keys.contains(&"o"), "missing viewport reset");
        assert!(keys.contains(&"m"), "missing metrics toggle");
        assert!(keys.contains(&"c"), "missing controls toggle");
        assert!(keys.contains(&"d"), "missing detail toggle");
        assert!(keys.contains(&"i"), "missing status log toggle");
        assert!(keys.contains(&"?"), "missing help toggle");
        assert!(keys.contains(&"Esc"), "missing escape");
    }

    #[test]
    fn keybindings_count_covers_all_actions() {
        let screen = MermaidMegaShowcaseScreen::new();
        let bindings = screen.keybindings();
        // We have at least one entry per bound key category.
        // With merged entries (j/Down, +/-, etc.) we expect ~25 entries.
        assert!(
            bindings.len() >= 22,
            "expected at least 22 keybinding entries, got {}",
            bindings.len()
        );
    }
}
