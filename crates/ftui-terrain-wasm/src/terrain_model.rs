//! Terrain visualization Model using curvature-adaptive braille rendering.

use ftui_extras::canvas::{CanvasRef, Mode, Painter};
use ftui_render::cell::PackedRgba;
use ftui_render::frame::Frame;
use ftui_runtime::program::{Cmd, Model};
use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind};
use ftui_widgets::Widget;
use ftui_core::geometry::Rect;
use web_time::Duration;

/// A 3D projected point ready for braille rasterization.
struct ProjectedPoint {
    sx: f64,   // screen x in sub-pixel coords
    sy: f64,   // screen y in sub-pixel coords
    depth: f64,
    elev: f64, // original elevation for color mapping
}

/// Terrain elevation dataset.
pub struct TerrainDataset {
    pub name: String,
    pub grid: Vec<Vec<f64>>,
    pub rows: usize,
    pub cols: usize,
    pub min_elev: f64,
    pub max_elev: f64,
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

    /// Compute curvature at a grid point (angle change between neighbors).
    fn curvature_at(grid: &[Vec<f64>], r: usize, c: usize, rows: usize, cols: usize) -> f64 {
        if r == 0 || r >= rows - 1 || c == 0 || c >= cols - 1 {
            return 1.0; // edge points: treat as moderate curvature
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
            let color = Self::elev_to_color(t);
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
        format!(
            "{} | {:.0}-{:.0}m | az:{:.0} el:{:.0} | h:{:.1}x | d:{:.0}%",
            ds.name, ds.min_elev, ds.max_elev, self.azimuth, self.elevation,
            self.height_scale, self.density * 100.0
        )
    }

    /// Project terrain with curvature-adaptive interpolation.
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

        let base_step = (ds.rows.max(ds.cols) as f64 / (60.0 * self.density)).round().max(1.0) as usize;
        let elev_range = (ds.max_elev - ds.min_elev).max(1.0);

        let mut points = Vec::with_capacity(ds.rows * ds.cols / (base_step * base_step));

        let mut r = 0;
        while r < ds.rows {
            let mut c = 0;
            while c < ds.cols {
                // Compute curvature to determine local step size
                let curv = Self::curvature_at(&ds.grid, r, c, ds.rows, ds.cols);
                // Normalize curvature: high curvature → step=1, low → step=base_step
                let norm_curv = (curv / elev_range * 500.0).min(1.0);
                let local_step = ((1.0 - norm_curv) * base_step as f64)
                    .round()
                    .max(1.0) as usize;

                let x = c as f64 - ds.cols as f64 / 2.0;
                let y = r as f64 - ds.rows as f64 / 2.0;
                // Subtract min_elev so height_scale amplifies differences, not absolute elevation
                let z = (ds.grid[r][c] - ds.min_elev) * self.height_scale * 0.015;

                // Rotate azimuth
                let rx = x * cos_az - y * sin_az;
                let ry = x * sin_az + y * cos_az;

                // Tilt elevation
                let depth = ry * cos_el + z * sin_el;
                let screen_y = ry * sin_el - z * cos_el;

                let sx = rx * self.zoom + cx;
                let sy = screen_y * self.zoom + cy;

                if sx >= 0.0 && sx < canvas_w && sy >= 0.0 && sy < canvas_h {
                    // For high-curvature areas, add interpolated sub-points
                    let norm_curv_threshold = norm_curv;
                    if norm_curv_threshold > 0.3 && local_step == 1 {
                        // Add extra dots between this and neighbors
                        points.push(ProjectedPoint { sx, sy, depth, elev: ds.grid[r][c] });

                        // Interpolate half-steps in both directions
                        if c + 1 < ds.cols && r + 1 < ds.rows {
                            let mid_elev = (ds.grid[r][c] + ds.grid[r][c + 1]
                                + ds.grid[r + 1][c] + ds.grid[r + 1][c + 1]) / 4.0;
                            let mx = (c as f64 + 0.5) - ds.cols as f64 / 2.0;
                            let my = (r as f64 + 0.5) - ds.rows as f64 / 2.0;
                            let mz = (mid_elev - ds.min_elev) * self.height_scale * 0.015;
                            let mrx = mx * cos_az - my * sin_az;
                            let mry = mx * sin_az + my * cos_az;
                            let md = mry * cos_el + mz * sin_el;
                            let msy = mry * sin_el - mz * cos_el;
                            let msx = mrx * self.zoom + cx;
                            let msyy = msy * self.zoom + cy;
                            if msx >= 0.0 && msx < canvas_w && msyy >= 0.0 && msyy < canvas_h {
                                points.push(ProjectedPoint {
                                    sx: msx, sy: msyy, depth: md, elev: mid_elev,
                                });
                            }
                        }
                    } else {
                        points.push(ProjectedPoint { sx, sy, depth, elev: ds.grid[r][c] });
                    }
                }

                c += local_step;
            }
            // Adaptive row step based on average curvature of this row
            r += base_step.max(1);
        }

        // Painter's algorithm: sort by depth (far first)
        points.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(std::cmp::Ordering::Equal));
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
            Msg::Event(Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. })) => {
                match code {
                    KeyCode::Char('q') | KeyCode::Escape => return Cmd::Quit,
                    KeyCode::Left => self.azimuth = (self.azimuth - 5.0).rem_euclid(360.0),
                    KeyCode::Right => self.azimuth = (self.azimuth + 5.0) % 360.0,
                    KeyCode::Up => self.elevation = (self.elevation + 3.0).min(85.0),
                    KeyCode::Down => self.elevation = (self.elevation - 3.0).max(5.0),
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        self.zoom = (self.zoom * 1.2).min(8.0);
                    }
                    KeyCode::Char('-') => {
                        self.zoom = (self.zoom / 1.2).max(0.1);
                    }
                    KeyCode::Char('h') => {
                        self.height_scale = (self.height_scale + 0.5).min(5.0);
                    }
                    KeyCode::Char('l') => {
                        self.height_scale = (self.height_scale - 0.5).max(0.5);
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
                }
            }
            Msg::Event(Event::Mouse(MouseEvent { kind, x, y, .. })) => {
                match kind {
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
                        self.zoom = (self.zoom * 1.1).min(8.0);
                    }
                    MouseEventKind::ScrollDown => {
                        self.zoom = (self.zoom / 1.1).max(0.1);
                    }
                    _ => {}
                }
            }
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
            let t = (pt.elev - ds.min_elev) / elev_range;
            let color = Self::elev_to_color(t.clamp(0.0, 1.0));
            painter.point_colored(pt.sx.round() as i32, pt.sy.round() as i32, color);
        }

        // Render braille canvas to frame
        let canvas = CanvasRef::from_painter(&painter);
        canvas.render(area, frame);

        // Header
        let header = format!(
            " {} · {:.0}-{:.0}m · az:{:.0}° el:{:.0}° · d:{:.0}% · {} pts",
            ds.name, ds.min_elev, ds.max_elev, self.azimuth, self.elevation,
            self.density * 100.0, points.len()
        );
        for (i, ch) in header.chars().enumerate() {
            if i >= w as usize { break; }
            let cell = ftui_render::cell::Cell::from_char(ch)
                .with_fg(PackedRgba::rgb(150, 150, 150))
                .with_bg(PackedRgba::rgb(20, 20, 20));
            frame.buffer.set_raw(i as u16, 0, cell);
        }
        // Fill rest of header row
        for i in header.len()..(w as usize) {
            let cell = ftui_render::cell::Cell::from_char(' ')
                .with_bg(PackedRgba::rgb(20, 20, 20));
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
            if i >= w as usize { break; }
            let cell = ftui_render::cell::Cell::from_char(ch)
                .with_fg(PackedRgba::rgb(100, 100, 100))
                .with_bg(PackedRgba::rgb(15, 15, 15));
            frame.buffer.set_raw(i as u16, footer_y, cell);
        }
        for i in footer.len()..(w as usize) {
            let cell = ftui_render::cell::Cell::from_char(' ')
                .with_bg(PackedRgba::rgb(15, 15, 15));
            frame.buffer.set_raw(i as u16, footer_y, cell);
        }
    }
}
