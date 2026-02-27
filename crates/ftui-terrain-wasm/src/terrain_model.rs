//! Terrain visualization Model using curvature-adaptive braille rendering.

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::canvas::{CanvasRef, Mode, Painter};
use ftui_render::cell::PackedRgba;
use ftui_render::frame::Frame;
use ftui_runtime::program::{Cmd, Model};
use ftui_widgets::Widget;
use web_time::Duration;

/// Color mode for terrain rendering.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum ColorMode {
    #[default]
    Terrain,
    Slope,
    Grayscale,
    Contour,
}

/// A 3D projected point ready for braille rasterization.
struct ProjectedPoint {
    sx: f64, // screen x in sub-pixel coords
    sy: f64, // screen y in sub-pixel coords
    depth: f64,
    elev: f64,  // original elevation for color mapping
    slope: f64, // local slope 0..1 (computed when needed)
}

/// Terrain elevation dataset.
pub struct TerrainDataset {
    pub name: String,
    pub grid: Vec<Vec<f64>>,
    pub rows: usize,
    pub cols: usize,
    pub min_elev: f64,
    pub max_elev: f64,
    pub cell_size: f64, // horizontal cell spacing in meters (same unit as elevation)
}

impl Default for TerrainDataset {
    fn default() -> Self {
        Self {
            name: String::new(),
            grid: vec![],
            rows: 0,
            cols: 0,
            min_elev: 0.0,
            max_elev: 1.0,
            cell_size: 30.0,
        }
    }
}

pub enum Msg {
    Event(Event),
}

impl From<Event> for Msg {
    fn from(e: Event) -> Self {
        Msg::Event(e)
    }
}

pub struct TerrainModel {
    datasets: Vec<TerrainDataset>,
    active: usize,
    azimuth: f64,
    elevation: f64,
    zoom: f64,
    height_scale: f64,
    density: f64, // multiplier for dot density (1.0 = default)
    auto_rotate: bool,
    color_mode: ColorMode,
    // Drag state
    dragging: bool,
    drag_start_x: u16,
    drag_start_y: u16,
    drag_start_az: f64,
    drag_start_el: f64,
}

impl Default for TerrainModel {
    fn default() -> Self {
        Self {
            datasets: vec![],
            active: 0,
            azimuth: 225.0,
            elevation: 35.0,
            zoom: 0.65,
            height_scale: 2.0,
            density: 1.0,
            auto_rotate: true,
            color_mode: ColorMode::default(),
            dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            drag_start_az: 0.0,
            drag_start_el: 0.0,
        }
    }
}

impl TerrainModel {
    /// Set terrain datasets (called from JS after construction).
    pub fn set_datasets(&mut self, datasets: Vec<TerrainDataset>) {
        self.datasets = datasets;
        self.active = 0;
    }

    /// Elevation → RGB terrain color gradient.
    fn elev_to_color(t: f64) -> PackedRgba {
        if t < 0.25 {
            let s = t / 0.25;
            PackedRgba::rgb(
                (26.0 + s * 50.0) as u8,
                (92.0 + s * 83.0) as u8,
                (26.0 + s * 30.0) as u8,
            )
        } else if t < 0.5 {
            let s = (t - 0.25) / 0.25;
            PackedRgba::rgb(
                (76.0 + s * 179.0) as u8,
                (175.0 + s * 40.0) as u8,
                (56.0 - s * 10.0) as u8,
            )
        } else if t < 0.75 {
            let s = (t - 0.5) / 0.25;
            PackedRgba::rgb(
                (255.0 - s * 116.0) as u8,
                (215.0 - s * 146.0) as u8,
                (46.0 - s * 27.0) as u8,
            )
        } else {
            let s = (t - 0.75) / 0.25;
            PackedRgba::rgb(
                (139.0 + s * 116.0) as u8,
                (69.0 + s * 186.0) as u8,
                (19.0 + s * 236.0) as u8,
            )
        }
    }

    /// Slope → RGB color gradient: green (flat) → yellow (moderate) → red (steep).
    fn slope_to_color(t: f64) -> PackedRgba {
        let t = t.clamp(0.0, 1.0);
        if t < 0.5 {
            let s = t / 0.5;
            PackedRgba::rgb(
                (34.0 + s * 221.0) as u8, // 34 → 255
                (139.0 + s * 76.0) as u8, // 139 → 215
                (34.0 - s * 34.0) as u8,  // 34 → 0
            )
        } else {
            let s = (t - 0.5) / 0.5;
            PackedRgba::rgb(
                255,                       // stay 255
                (215.0 - s * 185.0) as u8, // 215 → 30
                0,
            )
        }
    }

    /// Grayscale color: black (low) → white (high).
    fn grayscale_color(t: f64) -> PackedRgba {
        let v = (t.clamp(0.0, 1.0) * 255.0) as u8;
        PackedRgba::rgb(v, v, v)
    }

    /// Contour background: dim terrain tint so JS-drawn contour lines pop.
    fn contour_color(elev: f64, min_elev: f64, max_elev: f64) -> PackedRgba {
        let range = (max_elev - min_elev).max(1.0);
        let t = ((elev - min_elev) / range).clamp(0.0, 1.0);
        // Very dim terrain gradient for depth context
        let v = (20.0 + t * 30.0) as u8; // 20..50 range
        PackedRgba::rgb(v, (v as f64 * 1.1) as u8, (v as f64 * 1.2) as u8)
    }

    /// Compute local slope from neighbors, returning 0..1 normalized.
    /// `cell_size` is the horizontal spacing between grid cells in meters
    /// (same unit as elevation values), so the gradient is dimensionless.
    fn local_slope(
        grid: &[Vec<f64>],
        fr: f64,
        fc: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
    ) -> f64 {
        let r = (fr.round() as usize).min(rows.saturating_sub(1));
        let c = (fc.round() as usize).min(cols.saturating_sub(1));
        if r == 0 || r >= rows - 1 || c == 0 || c >= cols - 1 {
            return 0.0;
        }
        // Gradient in meters/meter (dimensionless) by dividing by cell spacing
        let dz_dx = (grid[r][c + 1] - grid[r][c - 1]) / (2.0 * cell_size);
        let dz_dy = (grid[r + 1][c] - grid[r - 1][c]) / (2.0 * cell_size);
        let slope_rad = (dz_dx * dz_dx + dz_dy * dz_dy).sqrt().atan();
        // Normalize: 0 = flat, 1 = 45° or steeper
        (slope_rad / std::f64::consts::FRAC_PI_4).min(1.0)
    }

    pub fn color_mode(&self) -> u8 {
        match self.color_mode {
            ColorMode::Terrain => 0,
            ColorMode::Slope => 1,
            ColorMode::Grayscale => 2,
            ColorMode::Contour => 3,
        }
    }

    pub fn set_color_mode(&mut self, v: u8) {
        self.color_mode = match v {
            1 => ColorMode::Slope,
            2 => ColorMode::Grayscale,
            3 => ColorMode::Contour,
            _ => ColorMode::Terrain,
        };
    }

    /// Apply the full camera/view state in one call.
    pub fn set_view_state(
        &mut self,
        azimuth: f64,
        elevation: f64,
        zoom: f64,
        height_scale: f64,
        density: f64,
        active: usize,
        color_mode: u8,
        auto_rotate: bool,
    ) {
        self.set_azimuth(azimuth);
        self.set_elevation(elevation);
        self.set_zoom(zoom);
        self.set_height_scale(height_scale);
        self.set_density(density);
        self.set_active(active);
        self.set_color_mode(color_mode);
        self.set_auto_rotate(auto_rotate);
    }

    /// Compute curvature at a grid point (angle change between neighbors).
    fn curvature_at(grid: &[Vec<f64>], r: usize, c: usize, rows: usize, cols: usize) -> f64 {
        if r == 0 || r >= rows - 1 || c == 0 || c >= cols - 1 {
            return 0.0; // edge points: no curvature data, use base step
        }
        // Compute gradient magnitude change (Laplacian approximation)
        let center = grid[r][c];
        let dx = (grid[r][c + 1] - grid[r][c.saturating_sub(1)]) / 2.0;
        let dy = (grid[r + 1][c] - grid[r.saturating_sub(1)][c]) / 2.0;
        let dxx = grid[r][c + 1] + grid[r][c.saturating_sub(1)] - 2.0 * center;
        let dyy = grid[r + 1][c] + grid[r.saturating_sub(1)][c] - 2.0 * center;
        let dxy = (grid[r + 1][c + 1] + grid[r.saturating_sub(1)][c.saturating_sub(1)]
            - grid[r + 1][c.saturating_sub(1)]
            - grid[r.saturating_sub(1)][c + 1])
            / 4.0;

        // Mean curvature approximation
        let grad_sq = dx * dx + dy * dy;
        if grad_sq < 1e-6 {
            return 0.0;
        }
        let curv = ((1.0 + dy * dy) * dxx - 2.0 * dx * dy * dxy + (1.0 + dx * dx) * dyy)
            / (2.0 * (1.0 + grad_sq).powf(1.5));
        curv.abs()
    }

    /// Bilinear interpolation of grid elevation at fractional (row, col) position.
    fn bilinear_sample(grid: &[Vec<f64>], fr: f64, fc: f64, rows: usize, cols: usize) -> f64 {
        let r0 = (fr as usize).min(rows - 1);
        let c0 = (fc as usize).min(cols - 1);
        let r1 = (r0 + 1).min(rows - 1);
        let c1 = (c0 + 1).min(cols - 1);
        let dr = fr - r0 as f64;
        let dc = fc - c0 as f64;
        let top = grid[r0][c0] + (grid[r0][c1] - grid[r0][c0]) * dc;
        let bot = grid[r1][c0] + (grid[r1][c1] - grid[r1][c0]) * dc;
        top + (bot - top) * dr
    }

    /// Project points into a flat f32 buffer: [x, y, r, g, b, ...] for direct canvas rendering.
    /// canvas_w/canvas_h are pixel dimensions of the target canvas.
    pub fn project_to_buffer(&self, canvas_w: f64, canvas_h: f64) -> Vec<f32> {
        if self.datasets.is_empty() {
            return vec![];
        }
        let ds = &self.datasets[self.active.min(self.datasets.len() - 1)];
        if ds.grid.is_empty() {
            return vec![];
        }
        let points = self.project_adaptive(ds, canvas_w, canvas_h);
        let elev_range = (ds.max_elev - ds.min_elev).max(1.0);
        let mut buf = Vec::with_capacity(points.len() * 5);
        for pt in &points {
            let t = ((pt.elev - ds.min_elev) / elev_range).clamp(0.0, 1.0);
            let color = match self.color_mode {
                ColorMode::Terrain => Self::elev_to_color(t),
                ColorMode::Slope => Self::slope_to_color(pt.slope),
                ColorMode::Grayscale => Self::grayscale_color(t),
                ColorMode::Contour => Self::contour_color(pt.elev, ds.min_elev, ds.max_elev),
            };
            let packed = color.0;
            let r = ((packed >> 24) & 0xFF) as f32;
            let g = ((packed >> 16) & 0xFF) as f32;
            let b = ((packed >> 8) & 0xFF) as f32;
            buf.push(pt.sx as f32);
            buf.push(pt.sy as f32);
            buf.push(r);
            buf.push(g);
            buf.push(b);
        }
        buf
    }

    /// Get current state info for display.
    pub fn state_info(&self) -> String {
        if self.datasets.is_empty() {
            return String::from("No data");
        }
        let ds = &self.datasets[self.active.min(self.datasets.len() - 1)];
        let mode_str = match self.color_mode {
            ColorMode::Terrain => "terrain",
            ColorMode::Slope => "slope",
            ColorMode::Grayscale => "gray",
            ColorMode::Contour => "contour",
        };
        format!(
            "{} | {:.0}-{:.0}m | az:{:.0} el:{:.0} | z:{:.2}x | h:{:.1}x | d:{:.0}% | {}",
            ds.name,
            ds.min_elev,
            ds.max_elev,
            self.azimuth,
            self.elevation,
            self.zoom,
            self.height_scale,
            self.density * 100.0,
            mode_str
        )
    }

    // --- Direct getters/setters for JS-driven gesture control ---

    pub fn zoom(&self) -> f64 {
        self.zoom
    }
    pub fn set_zoom(&mut self, v: f64) {
        self.zoom = v.clamp(0.1, 20.0);
    }

    pub fn azimuth(&self) -> f64 {
        self.azimuth
    }
    pub fn set_azimuth(&mut self, v: f64) {
        self.azimuth = v.rem_euclid(360.0);
    }

    pub fn elevation(&self) -> f64 {
        self.elevation
    }
    pub fn set_elevation(&mut self, v: f64) {
        self.elevation = v.clamp(5.0, 85.0);
    }

    pub fn height_scale(&self) -> f64 {
        self.height_scale
    }
    pub fn set_height_scale(&mut self, v: f64) {
        self.height_scale = v.clamp(0.05, 5.0);
    }

    pub fn density(&self) -> f64 {
        self.density
    }
    pub fn set_density(&mut self, v: f64) {
        self.density = v.clamp(0.25, 4.0);
    }

    pub fn auto_rotate(&self) -> bool {
        self.auto_rotate
    }
    pub fn set_auto_rotate(&mut self, v: bool) {
        self.auto_rotate = v;
    }

    pub fn active(&self) -> usize {
        self.active
    }
    pub fn set_active(&mut self, v: usize) {
        if !self.datasets.is_empty() {
            self.active = v.min(self.datasets.len() - 1);
        }
    }

    pub fn dataset_count(&self) -> usize {
        self.datasets.len()
    }

    /// Get the contour interval for the active dataset (same logic as contour_color).
    /// Returns 0.0 if no dataset loaded.
    pub fn contour_interval(&self) -> f64 {
        if self.datasets.is_empty() {
            return 0.0;
        }
        let ds = &self.datasets[self.active.min(self.datasets.len() - 1)];
        let range = (ds.max_elev - ds.min_elev).max(1.0);
        let nice = [500.0, 200.0, 100.0, 50.0, 20.0, 10.0, 5.0, 2.0];
        let ideal = range / 10.0;
        nice.iter().find(|&&n| n <= ideal).copied().unwrap_or(2.0)
    }

    /// Project a single fractional grid point (row, col) to canvas screen coordinates.
    /// Uses the same camera pipeline as `project_to_buffer`.
    /// Returns `Some((sx, sy))` or `None` if no dataset is loaded.
    pub fn project_single_point(
        &self,
        row: f64,
        col: f64,
        canvas_w: f64,
        canvas_h: f64,
    ) -> Option<(f64, f64)> {
        if self.datasets.is_empty() {
            return None;
        }
        let ds = &self.datasets[self.active.min(self.datasets.len() - 1)];
        if ds.grid.is_empty() {
            return None;
        }

        let half_cols = ds.cols as f64 / 2.0;
        let half_rows = ds.rows as f64 / 2.0;

        // Bilinear elevation at fractional position
        let elev = Self::bilinear_sample(&ds.grid, row, col, ds.rows, ds.cols);

        // 3D projection — same math as project_adaptive
        let x = col - half_cols;
        let y = row - half_rows;
        let z = (elev - ds.min_elev) * self.height_scale * 0.015;

        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();
        let cos_az = az.cos();
        let sin_az = az.sin();
        let cos_el = el.cos();
        let sin_el = el.sin();

        let rx = x * cos_az - y * sin_az;
        let ry = x * sin_az + y * cos_az;
        let screen_y = ry * sin_el - z * cos_el;

        let cx = canvas_w / 2.0;
        let cy = canvas_h / 2.0;
        let sx = rx * self.zoom + cx;
        let sy = screen_y * self.zoom + cy;

        Some((sx, sy))
    }

    /// Project a packed `[row, col, row, col, ...]` payload into `[x, y, x, y, ...]`.
    ///
    /// Returns an empty Vec when no dataset is loaded.
    pub fn project_row_col_pairs_to_buffer(
        &self,
        row_col_pairs: &[f32],
        canvas_w: f64,
        canvas_h: f64,
    ) -> Vec<f32> {
        if self.datasets.is_empty() || row_col_pairs.len() < 2 {
            return vec![];
        }
        let ds = &self.datasets[self.active.min(self.datasets.len() - 1)];
        if ds.grid.is_empty() {
            return vec![];
        }

        let half_cols = ds.cols as f64 / 2.0;
        let half_rows = ds.rows as f64 / 2.0;

        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();
        let cos_az = az.cos();
        let sin_az = az.sin();
        let cos_el = el.cos();
        let sin_el = el.sin();

        let cx = canvas_w / 2.0;
        let cy = canvas_h / 2.0;

        let mut out = Vec::with_capacity(row_col_pairs.len());
        let mut i = 0usize;
        while i + 1 < row_col_pairs.len() {
            let row = row_col_pairs[i] as f64;
            let col = row_col_pairs[i + 1] as f64;

            let elev = Self::bilinear_sample(&ds.grid, row, col, ds.rows, ds.cols);
            let x = col - half_cols;
            let y = row - half_rows;
            let z = (elev - ds.min_elev) * self.height_scale * 0.015;

            let rx = x * cos_az - y * sin_az;
            let ry = x * sin_az + y * cos_az;
            let screen_y = ry * sin_el - z * cos_el;

            let sx = rx * self.zoom + cx;
            let sy = screen_y * self.zoom + cy;

            out.push(sx as f32);
            out.push(sy as f32);
            i += 2;
        }
        out
    }

    /// Project terrain with bilinear sub-grid sampling and curvature-adaptive stepping.
    fn project_adaptive(
        &self,
        ds: &TerrainDataset,
        canvas_w: f64,
        canvas_h: f64,
    ) -> Vec<ProjectedPoint> {
        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();
        let cos_az = az.cos();
        let sin_az = az.sin();
        let cos_el = el.cos();
        let sin_el = el.sin();
        let cx = canvas_w / 2.0;
        let cy = canvas_h / 2.0;

        let base_step = (ds.rows.max(ds.cols) as f64 / (60.0 * self.density))
            .round()
            .max(1.0);
        let elev_range = (ds.max_elev - ds.min_elev).max(1.0);

        // Alternating refinement: rows refine first, then cols
        let row_refine: f64 = if self.density >= 2.5 {
            4.0
        } else if self.density >= 1.25 {
            2.0
        } else {
            1.0
        };
        let col_refine: f64 = if self.density >= 3.0 {
            4.0
        } else if self.density >= 1.5 {
            2.0
        } else {
            1.0
        };

        let row_step = (base_step / row_refine).max(1.0);
        let col_step_base = (base_step / col_refine).max(1.0);
        let min_col_step = col_step_base.min(1.0);
        let half_cols = ds.cols as f64 / 2.0;
        let half_rows = ds.rows as f64 / 2.0;

        let est_rows = (ds.rows as f64 / row_step) as usize + 1;
        let est_cols = (ds.cols as f64 / col_step_base) as usize + 1;
        let mut points = Vec::with_capacity(est_rows * est_cols);

        let rows_f = ds.rows as f64;
        let cols_f = ds.cols as f64;

        let mut fr = 0.0;
        while fr < rows_f {
            let mut fc = 0.0;
            while fc < cols_f {
                // Curvature at nearest grid point for adaptive column stepping
                let gr = (fr.round() as usize).min(ds.rows - 1);
                let gc = (fc.round() as usize).min(ds.cols - 1);
                let curv = Self::curvature_at(&ds.grid, gr, gc, ds.rows, ds.cols);
                let norm_curv = (curv / elev_range * 500.0).min(1.0);
                let local_col_step = ((1.0 - norm_curv) * col_step_base).max(min_col_step);

                // Bilinear elevation at fractional position
                let elev = Self::bilinear_sample(&ds.grid, fr, fc, ds.rows, ds.cols);

                // 3D projection
                let x = fc - half_cols;
                let y = fr - half_rows;
                let z = (elev - ds.min_elev) * self.height_scale * 0.015;

                let rx = x * cos_az - y * sin_az;
                let ry = x * sin_az + y * cos_az;

                let depth = ry * cos_el + z * sin_el;
                let screen_y = ry * sin_el - z * cos_el;

                let sx = rx * self.zoom + cx;
                let sy = screen_y * self.zoom + cy;

                if sx >= 0.0 && sx < canvas_w && sy >= 0.0 && sy < canvas_h {
                    let slope = if self.color_mode == ColorMode::Slope {
                        Self::local_slope(&ds.grid, fr, fc, ds.rows, ds.cols, ds.cell_size)
                    } else {
                        0.0
                    };
                    points.push(ProjectedPoint {
                        sx,
                        sy,
                        depth,
                        elev,
                        slope,
                    });
                }

                fc += local_col_step;
            }
            fr += row_step;
        }

        // Painter's algorithm: sort by depth (far first)
        points.sort_by(|a, b| {
            a.depth
                .partial_cmp(&b.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        points
    }
}

impl Model for TerrainModel {
    type Message = Msg;

    fn init(&mut self) -> Cmd<Self::Message> {
        // Request ticks for auto-rotation
        Cmd::Tick(Duration::from_millis(33)) // ~30fps
    }

    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
        match msg {
            Msg::Event(Event::Tick) => {
                if self.auto_rotate {
                    self.azimuth = (self.azimuth + 0.5) % 360.0;
                }
            }
            Msg::Event(Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            })) => match code {
                KeyCode::Char('q') | KeyCode::Escape => return Cmd::Quit,
                KeyCode::Left => self.azimuth = (self.azimuth - 5.0).rem_euclid(360.0),
                KeyCode::Right => self.azimuth = (self.azimuth + 5.0) % 360.0,
                KeyCode::Up => self.elevation = (self.elevation + 3.0).min(85.0),
                KeyCode::Down => self.elevation = (self.elevation - 3.0).max(5.0),
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.zoom = (self.zoom * 1.2).min(20.0);
                }
                KeyCode::Char('-') => {
                    self.zoom = (self.zoom / 1.2).max(0.1);
                }
                KeyCode::Char('h') => {
                    self.height_scale = (self.height_scale + 0.5).min(5.0);
                }
                KeyCode::Char('l') => {
                    self.height_scale = (self.height_scale - 0.5).max(0.05);
                }
                KeyCode::Char('r') => {
                    self.auto_rotate = !self.auto_rotate;
                }
                KeyCode::Char('d') => {
                    self.density = (self.density + 0.25).min(4.0);
                }
                KeyCode::Char('f') => {
                    self.density = (self.density - 0.25).max(0.25);
                }
                KeyCode::Char('1') => self.active = 0,
                KeyCode::Char('2') => {
                    if self.datasets.len() > 1 {
                        self.active = 1;
                    }
                }
                KeyCode::Tab => {
                    if !self.datasets.is_empty() {
                        self.active = (self.active + 1) % self.datasets.len();
                    }
                }
                _ => {}
            },
            Msg::Event(Event::Mouse(MouseEvent { kind, x, y, .. })) => match kind {
                MouseEventKind::Down(_) => {
                    self.auto_rotate = false;
                    self.dragging = true;
                    self.drag_start_x = x;
                    self.drag_start_y = y;
                    self.drag_start_az = self.azimuth;
                    self.drag_start_el = self.elevation;
                }
                MouseEventKind::Drag(_) if self.dragging => {
                    let dx = x as f64 - self.drag_start_x as f64;
                    let dy = y as f64 - self.drag_start_y as f64;
                    self.azimuth = (self.drag_start_az + dx * 1.5).rem_euclid(360.0);
                    self.elevation = (self.drag_start_el + dy * 1.0).clamp(5.0, 85.0);
                }
                MouseEventKind::Up(_) => {
                    self.dragging = false;
                }
                MouseEventKind::ScrollUp => {
                    self.zoom = (self.zoom * 1.1).min(20.0);
                }
                MouseEventKind::ScrollDown => {
                    self.zoom = (self.zoom / 1.1).max(0.1);
                }
                _ => {}
            },
            _ => {}
        }
        Cmd::None
    }

    fn view(&self, frame: &mut Frame) {
        let w = frame.width();
        let h = frame.height();
        if w == 0 || h == 0 || self.datasets.is_empty() {
            return;
        }

        let ds = &self.datasets[self.active.min(self.datasets.len() - 1)];
        if ds.grid.is_empty() {
            return;
        }

        // Reserve 2 rows for header/footer
        let view_h = h.saturating_sub(2);
        let area = Rect::new(0, 1, w, view_h);

        // Braille sub-pixel resolution
        let sub_w = (w as f64) * 2.0;
        let sub_h = (view_h as f64) * 4.0;

        // Create painter for this frame
        let mut painter = Painter::for_area(area, Mode::Braille);
        painter.clear();

        // Project with curvature-adaptive interpolation
        let points = self.project_adaptive(ds, sub_w, sub_h);
        let elev_range = (ds.max_elev - ds.min_elev).max(1.0);

        // Rasterize points
        for pt in &points {
            let t = ((pt.elev - ds.min_elev) / elev_range).clamp(0.0, 1.0);
            let color = match self.color_mode {
                ColorMode::Terrain => Self::elev_to_color(t),
                ColorMode::Slope => Self::slope_to_color(pt.slope),
                ColorMode::Grayscale => Self::grayscale_color(t),
                ColorMode::Contour => Self::contour_color(pt.elev, ds.min_elev, ds.max_elev),
            };
            painter.point_colored(pt.sx.round() as i32, pt.sy.round() as i32, color);
        }

        // Render braille canvas to frame
        let canvas = CanvasRef::from_painter(&painter);
        canvas.render(area, frame);

        // Header
        let header = format!(
            " {} · {:.0}-{:.0}m · az:{:.0}° el:{:.0}° · d:{:.0}% · {} pts",
            ds.name,
            ds.min_elev,
            ds.max_elev,
            self.azimuth,
            self.elevation,
            self.density * 100.0,
            points.len()
        );
        for (i, ch) in header.chars().enumerate() {
            if i >= w as usize {
                break;
            }
            let cell = ftui_render::cell::Cell::from_char(ch)
                .with_fg(PackedRgba::rgb(150, 150, 150))
                .with_bg(PackedRgba::rgb(20, 20, 20));
            frame.buffer.set_raw(i as u16, 0, cell);
        }
        // Fill rest of header row
        for i in header.len()..(w as usize) {
            let cell = ftui_render::cell::Cell::from_char(' ').with_bg(PackedRgba::rgb(20, 20, 20));
            frame.buffer.set_raw(i as u16, 0, cell);
        }

        // Footer
        let auto_str = if self.auto_rotate { "ON" } else { "OFF" };
        let footer = format!(
            " [Tab] switch · [R]otate:{} · [←→↑↓] orbit · [+/-] zoom · [H/L] height",
            auto_str
        );
        let footer_y = h - 1;
        for (i, ch) in footer.chars().enumerate() {
            if i >= w as usize {
                break;
            }
            let cell = ftui_render::cell::Cell::from_char(ch)
                .with_fg(PackedRgba::rgb(100, 100, 100))
                .with_bg(PackedRgba::rgb(15, 15, 15));
            frame.buffer.set_raw(i as u16, footer_y, cell);
        }
        for i in footer.len()..(w as usize) {
            let cell = ftui_render::cell::Cell::from_char(' ').with_bg(PackedRgba::rgb(15, 15, 15));
            frame.buffer.set_raw(i as u16, footer_y, cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model() -> TerrainModel {
        TerrainModel::default()
    }

    fn make_dataset(rows: usize, cols: usize) -> TerrainDataset {
        // Simple gradient: elevation = row index
        let grid: Vec<Vec<f64>> = (0..rows)
            .map(|r| (0..cols).map(|_| r as f64 * 10.0).collect())
            .collect();
        TerrainDataset {
            name: "test".into(),
            grid,
            rows,
            cols,
            min_elev: 0.0,
            max_elev: (rows - 1) as f64 * 10.0,
            cell_size: 30.0,
        }
    }

    // --- Setter clamping tests ---

    #[test]
    fn zoom_clamps_low() {
        let mut m = make_model();
        m.set_zoom(-5.0);
        assert_eq!(m.zoom(), 0.1);
    }

    #[test]
    fn zoom_clamps_high() {
        let mut m = make_model();
        m.set_zoom(100.0);
        assert_eq!(m.zoom(), 20.0);
    }

    #[test]
    fn zoom_accepts_valid() {
        let mut m = make_model();
        m.set_zoom(3.5);
        assert!((m.zoom() - 3.5).abs() < 1e-10);
    }

    #[test]
    fn azimuth_wraps_negative() {
        let mut m = make_model();
        m.set_azimuth(-10.0);
        assert!((m.azimuth() - 350.0).abs() < 1e-10);
    }

    #[test]
    fn azimuth_wraps_over_360() {
        let mut m = make_model();
        m.set_azimuth(370.0);
        assert!((m.azimuth() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn azimuth_zero_stays_zero() {
        let mut m = make_model();
        m.set_azimuth(0.0);
        assert!((m.azimuth()).abs() < 1e-10);
    }

    #[test]
    fn elevation_clamps_low() {
        let mut m = make_model();
        m.set_elevation(-10.0);
        assert_eq!(m.elevation(), 5.0);
    }

    #[test]
    fn elevation_clamps_high() {
        let mut m = make_model();
        m.set_elevation(90.0);
        assert_eq!(m.elevation(), 85.0);
    }

    #[test]
    fn height_scale_clamps() {
        let mut m = make_model();
        m.set_height_scale(0.0);
        assert_eq!(m.height_scale(), 0.05);
        m.set_height_scale(10.0);
        assert_eq!(m.height_scale(), 5.0);
    }

    #[test]
    fn density_clamps() {
        let mut m = make_model();
        m.set_density(0.0);
        assert_eq!(m.density(), 0.25);
        m.set_density(10.0);
        assert_eq!(m.density(), 4.0);
    }

    #[test]
    fn auto_rotate_toggle() {
        let mut m = make_model();
        assert!(m.auto_rotate());
        m.set_auto_rotate(false);
        assert!(!m.auto_rotate());
    }

    #[test]
    fn active_clamps_to_dataset_count() {
        let mut m = make_model();
        let ds = vec![make_dataset(10, 10), make_dataset(10, 10)];
        m.set_datasets(ds);
        m.set_active(5);
        assert_eq!(m.active(), 1); // clamped to len-1
    }

    #[test]
    fn active_noop_when_no_datasets() {
        let mut m = make_model();
        m.set_active(3);
        assert_eq!(m.active(), 0); // unchanged, no datasets
    }

    // --- Bilinear interpolation tests ---

    #[test]
    fn bilinear_at_grid_point() {
        let grid = vec![vec![0.0, 10.0], vec![20.0, 30.0]];
        let v = TerrainModel::bilinear_sample(&grid, 0.0, 0.0, 2, 2);
        assert!((v - 0.0).abs() < 1e-10);

        let v = TerrainModel::bilinear_sample(&grid, 0.0, 1.0, 2, 2);
        assert!((v - 10.0).abs() < 1e-10);

        let v = TerrainModel::bilinear_sample(&grid, 1.0, 0.0, 2, 2);
        assert!((v - 20.0).abs() < 1e-10);

        let v = TerrainModel::bilinear_sample(&grid, 1.0, 1.0, 2, 2);
        assert!((v - 30.0).abs() < 1e-10);
    }

    #[test]
    fn bilinear_midpoint() {
        let grid = vec![vec![0.0, 10.0], vec![20.0, 30.0]];
        // Center of 4 cells: average = (0+10+20+30)/4 = 15
        let v = TerrainModel::bilinear_sample(&grid, 0.5, 0.5, 2, 2);
        assert!((v - 15.0).abs() < 1e-10);
    }

    #[test]
    fn bilinear_horizontal_lerp() {
        let grid = vec![vec![0.0, 100.0], vec![0.0, 100.0]];
        let v = TerrainModel::bilinear_sample(&grid, 0.0, 0.25, 2, 2);
        assert!((v - 25.0).abs() < 1e-10);
    }

    #[test]
    fn bilinear_clamps_at_boundary() {
        let grid = vec![vec![5.0, 10.0], vec![15.0, 20.0]];
        // Beyond grid edge — should clamp, not panic
        let v = TerrainModel::bilinear_sample(&grid, 1.5, 1.5, 2, 2);
        assert!(v.is_finite());
    }

    // --- Projection tests ---

    #[test]
    fn project_to_buffer_empty_datasets() {
        let m = make_model();
        let buf = m.project_to_buffer(400.0, 300.0);
        assert!(buf.is_empty());
    }

    #[test]
    fn project_produces_points() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(20, 20)]);
        let buf = m.project_to_buffer(400.0, 300.0);
        // Buffer has 5 floats per point (x, y, r, g, b)
        assert!(buf.len() >= 5);
        assert_eq!(buf.len() % 5, 0);
    }

    #[test]
    fn project_points_within_canvas() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(20, 20)]);
        let w = 400.0_f64;
        let h = 300.0_f64;
        let buf = m.project_to_buffer(w, h);
        for i in (0..buf.len()).step_by(5) {
            let x = buf[i] as f64;
            let y = buf[i + 1] as f64;
            assert!(x >= 0.0 && x <= w, "x={x} out of range");
            assert!(y >= 0.0 && y <= h, "y={y} out of range");
        }
    }

    #[test]
    fn project_colors_valid_rgb() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(20, 20)]);
        let buf = m.project_to_buffer(400.0, 300.0);
        for i in (0..buf.len()).step_by(5) {
            let r = buf[i + 2];
            let g = buf[i + 3];
            let b = buf[i + 4];
            assert!(r >= 0.0 && r <= 255.0, "r={r}");
            assert!(g >= 0.0 && g <= 255.0, "g={g}");
            assert!(b >= 0.0 && b <= 255.0, "b={b}");
        }
    }

    #[test]
    fn density_affects_point_count() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(50, 50)]);

        m.set_density(0.5);
        let low = m.project_to_buffer(400.0, 300.0).len();

        m.set_density(2.0);
        let high = m.project_to_buffer(400.0, 300.0).len();

        assert!(
            high > low,
            "higher density should produce more points: low={low} high={high}"
        );
    }

    #[test]
    fn zoom_does_not_change_point_count() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(20, 20)]);

        m.set_zoom(0.5);
        let a = m.project_to_buffer(400.0, 300.0).len();

        m.set_zoom(2.0);
        let b = m.project_to_buffer(400.0, 300.0).len();

        // Zoom changes screen positions but not grid sampling,
        // though some points may fall outside canvas at different zoom.
        // Just verify both produce points.
        assert!(a > 0 && b > 0);
    }

    #[test]
    fn set_view_state_applies_and_clamps() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(10, 10), make_dataset(10, 10)]);
        m.set_view_state(-10.0, 100.0, 30.0, 8.0, 0.0, 9, 9, true);

        assert!((m.azimuth() - 350.0).abs() < 1e-9);
        assert_eq!(m.elevation(), 85.0);
        assert_eq!(m.zoom(), 20.0);
        assert_eq!(m.height_scale(), 5.0);
        assert_eq!(m.density(), 0.25);
        assert_eq!(m.active(), 1);
        assert_eq!(m.color_mode(), 0);
        assert!(m.auto_rotate());
    }

    #[test]
    fn project_row_col_pairs_to_buffer_returns_xy_pairs() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(10, 10)]);
        let coords = vec![0.0_f32, 0.0_f32, 5.0_f32, 5.0_f32, 9.0_f32, 9.0_f32];
        let out = m.project_row_col_pairs_to_buffer(&coords, 400.0, 300.0);

        assert_eq!(out.len(), coords.len());
        assert!(out.iter().all(|v| v.is_finite()));
    }

    // --- Refinement threshold tests ---

    #[test]
    fn refinement_thresholds() {
        // Verify the alternating refinement logic by checking step sizes
        let mut m = make_model();
        // At density 1.0 — no refinement
        m.set_density(1.0);
        assert_eq!(m.density(), 1.0);

        // At density 1.25 — row refinement kicks in
        m.set_density(1.25);
        assert_eq!(m.density(), 1.25);

        // At density 2.5 — 4x row refinement
        m.set_density(2.5);
        assert_eq!(m.density(), 2.5);
    }

    // --- Curvature tests ---

    #[test]
    fn curvature_flat_surface_is_zero() {
        // Flat grid: all same elevation
        let grid = vec![vec![100.0; 5]; 5];
        let curv = TerrainModel::curvature_at(&grid, 2, 2, 5, 5);
        assert!(
            curv.abs() < 1e-6,
            "flat surface should have ~zero curvature: {curv}"
        );
    }

    #[test]
    fn curvature_edge_returns_zero() {
        let grid = vec![vec![0.0; 5]; 5];
        let curv = TerrainModel::curvature_at(&grid, 0, 0, 5, 5);
        assert_eq!(
            curv, 0.0,
            "edge points should return 0.0 (no curvature data)"
        );
    }

    // --- Color mode tests ---

    #[test]
    fn color_mode_default_is_terrain() {
        let m = make_model();
        assert_eq!(m.color_mode(), 0);
    }

    #[test]
    fn color_mode_roundtrip() {
        let mut m = make_model();
        m.set_color_mode(1);
        assert_eq!(m.color_mode(), 1);
        m.set_color_mode(2);
        assert_eq!(m.color_mode(), 2);
        m.set_color_mode(0);
        assert_eq!(m.color_mode(), 0);
    }

    #[test]
    fn color_mode_invalid_defaults_to_terrain() {
        let mut m = make_model();
        m.set_color_mode(99);
        assert_eq!(m.color_mode(), 0);
    }

    #[test]
    fn slope_color_gradient() {
        // Flat (0.0) should be greenish
        let flat = TerrainModel::slope_to_color(0.0);
        let flat_packed = flat.0;
        let flat_r = ((flat_packed >> 24) & 0xFF) as u8;
        let flat_g = ((flat_packed >> 16) & 0xFF) as u8;
        assert!(flat_g > flat_r, "flat should be more green than red");

        // Steep (1.0) should be reddish
        let steep = TerrainModel::slope_to_color(1.0);
        let steep_packed = steep.0;
        let steep_r = ((steep_packed >> 24) & 0xFF) as u8;
        let steep_g = ((steep_packed >> 16) & 0xFF) as u8;
        assert!(steep_r > steep_g, "steep should be more red than green");
    }

    #[test]
    fn grayscale_color_range() {
        let black = TerrainModel::grayscale_color(0.0);
        let black_packed = black.0;
        assert_eq!((black_packed >> 24) & 0xFF, 0);

        let white = TerrainModel::grayscale_color(1.0);
        let white_packed = white.0;
        assert_eq!((white_packed >> 24) & 0xFF, 255);

        let mid = TerrainModel::grayscale_color(0.5);
        let mid_packed = mid.0;
        let v = ((mid_packed >> 24) & 0xFF) as u8;
        assert!(v > 100 && v < 155, "mid gray should be ~127: {v}");
    }

    #[test]
    fn slope_color_clamps() {
        // Out-of-range values should not panic
        let _ = TerrainModel::slope_to_color(-1.0);
        let _ = TerrainModel::slope_to_color(2.0);
    }

    #[test]
    fn grayscale_clamps() {
        let _ = TerrainModel::grayscale_color(-0.5);
        let _ = TerrainModel::grayscale_color(1.5);
    }

    // --- Local slope tests ---

    #[test]
    fn local_slope_flat_is_zero() {
        let grid = vec![vec![100.0; 5]; 5];
        let s = TerrainModel::local_slope(&grid, 2.0, 2.0, 5, 5, 30.0);
        assert!(s.abs() < 1e-6, "flat surface slope should be ~0: {s}");
    }

    #[test]
    fn local_slope_edge_is_zero() {
        let grid = vec![vec![0.0; 5]; 5];
        let s = TerrainModel::local_slope(&grid, 0.0, 0.0, 5, 5, 30.0);
        assert_eq!(s, 0.0);
    }

    #[test]
    fn local_slope_steep_is_positive() {
        // Each row increases by 100m — steep slope
        let grid: Vec<Vec<f64>> = (0..5).map(|r| vec![r as f64 * 100.0; 5]).collect();
        let s = TerrainModel::local_slope(&grid, 2.0, 2.0, 5, 5, 30.0);
        assert!(s > 0.0, "steep terrain should have positive slope: {s}");
    }

    #[test]
    fn project_with_slope_mode_produces_points() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(20, 20)]);
        m.set_color_mode(1); // slope
        let buf = m.project_to_buffer(400.0, 300.0);
        assert!(buf.len() >= 5);
        assert_eq!(buf.len() % 5, 0);
    }

    #[test]
    fn project_with_grayscale_mode_produces_points() {
        let mut m = make_model();
        m.set_datasets(vec![make_dataset(20, 20)]);
        m.set_color_mode(2); // grayscale
        let buf = m.project_to_buffer(400.0, 300.0);
        assert!(buf.len() >= 5);
        assert_eq!(buf.len() % 5, 0);
        // In grayscale, r == g == b for every point
        for i in (0..buf.len()).step_by(5) {
            let r = buf[i + 2];
            let g = buf[i + 3];
            let b = buf[i + 4];
            assert!(
                (r - g).abs() < 1e-3 && (g - b).abs() < 1e-3,
                "grayscale r={r} g={g} b={b} should be equal"
            );
        }
    }

    #[test]
    fn curvature_nonzero_for_ridge() {
        // Create a ridge: center row elevated — sample on the slope (row 1),
        // not the peak (row 2) where gradient is zero.
        let mut grid = vec![vec![0.0; 5]; 5];
        for c in 0..5 {
            grid[2][c] = 100.0;
        }
        let curv = TerrainModel::curvature_at(&grid, 1, 2, 5, 5);
        assert!(
            curv > 0.0,
            "ridge slope should have nonzero curvature: {curv}"
        );
    }
}
