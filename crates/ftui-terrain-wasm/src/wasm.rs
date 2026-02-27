//! `wasm-bindgen` exports for the TerrainRunner.

use js_sys::{Float32Array, Object, Reflect, Uint32Array};
use wasm_bindgen::prelude::*;
use web_time::Duration;

use crate::terrain_model::{TerrainDataset, TerrainModel};
use ftui_web::step_program::StepProgram;
use ftui_web::WebFlatPatchBatch;

fn set_js(obj: &Object, key: &str, value: JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), &value);
}

fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let global = js_sys::global();
            if let Ok(console) = Reflect::get(&global, &"console".into()) {
                if let Ok(error) = Reflect::get(&console, &"error".into()) {
                    if let Ok(f) = error.dyn_into::<js_sys::Function>() {
                        let _ = f.call1(&console, &JsValue::from_str(&format!("{info}")));
                    }
                }
            }
        }));
    });
}

/// WASM terrain viewer runner.
///
/// Host-driven: JavaScript controls the event loop via `requestAnimationFrame`,
/// pushing input events and advancing time each frame.
#[wasm_bindgen]
pub struct TerrainRunner {
    inner: StepProgram<TerrainModel>,
}

#[wasm_bindgen]
impl TerrainRunner {
    /// Create a new terrain runner with initial terminal dimensions.
    #[wasm_bindgen(constructor)]
    pub fn new(cols: u16, rows: u16) -> Self {
        install_panic_hook();
        let cols = cols.max(10).min(500);
        let rows = rows.max(5).min(200);
        let model = TerrainModel::default();
        Self {
            inner: StepProgram::new(model, cols, rows),
        }
    }

    /// Load terrain data from a JSON string.
    ///
    /// Expected format:
    /// ```json
    /// [
    ///   { "name": "...", "grid": [[...], ...], "rows": N, "cols": N,
    ///     "min_elev": F, "max_elev": F }
    /// ]
    /// ```
    #[wasm_bindgen(js_name = loadTerrainData)]
    pub fn load_terrain_data(&mut self, json: &str) -> bool {
        #[derive(serde::Deserialize)]
        struct DatasetJson {
            name: String,
            grid: Vec<Vec<f64>>,
            rows: usize,
            cols: usize,
            min_elev: f64,
            max_elev: f64,
            #[serde(default = "default_cell_size")]
            cell_size: f64,
        }

        fn default_cell_size() -> f64 {
            30.0
        }

        match serde_json::from_str::<Vec<DatasetJson>>(json) {
            Ok(datasets) => {
                let ds: Vec<TerrainDataset> = datasets
                    .into_iter()
                    .map(|d| TerrainDataset {
                        name: d.name,
                        grid: d.grid,
                        rows: d.rows,
                        cols: d.cols,
                        min_elev: d.min_elev,
                        max_elev: d.max_elev,
                        cell_size: if d.cell_size > 0.0 { d.cell_size } else { 30.0 },
                    })
                    .collect();
                self.inner.model_mut().set_datasets(ds);
                true
            }
            Err(_) => false,
        }
    }

    /// Initialize the model and render the first frame.
    pub fn init(&mut self) {
        if !self.inner.is_initialized() {
            let _ = self.inner.init();
        }
    }

    /// Advance deterministic clock by `dt_ms` milliseconds.
    #[wasm_bindgen(js_name = advanceTime)]
    pub fn advance_time(&mut self, dt_ms: f64) {
        if !dt_ms.is_finite() || dt_ms <= 0.0 {
            return;
        }
        let secs = (dt_ms / 1000.0).min(Duration::MAX.as_secs_f64());
        let duration = Duration::try_from_secs_f64(secs).unwrap_or(Duration::MAX);
        self.inner.advance_time(duration);
    }

    /// Push a JSON-encoded input event.
    #[wasm_bindgen(js_name = pushEncodedInput)]
    pub fn push_encoded_input(&mut self, json: &str) -> bool {
        match ftui_web::input_parser::parse_encoded_input_to_event(json) {
            Ok(Some(event)) => {
                self.inner.push_event(event);
                true
            }
            _ => false,
        }
    }

    /// Process pending events and render if dirty.
    pub fn step(&mut self) -> JsValue {
        if !self.inner.is_initialized() {
            self.init();
        }
        let result = match self.inner.step() {
            Ok(r) => r,
            Err(_) => {
                let obj = Object::new();
                set_js(&obj, "running", JsValue::from(false));
                set_js(&obj, "rendered", JsValue::from(false));
                set_js(&obj, "events_processed", JsValue::from(0u32));
                set_js(&obj, "frame_idx", JsValue::from(0u32));
                return obj.into();
            }
        };

        let obj = Object::new();
        set_js(&obj, "running", JsValue::from(result.running));
        set_js(&obj, "rendered", JsValue::from(result.rendered));
        set_js(
            &obj,
            "events_processed",
            JsValue::from(result.events_processed),
        );
        set_js(
            &obj,
            "frame_idx",
            JsValue::from(self.inner.frame_idx() as u32),
        );
        obj.into()
    }

    /// Take flat patch batch for rendering.
    #[wasm_bindgen(js_name = takeFlatPatches)]
    pub fn take_flat_patches(&mut self) -> JsValue {
        let batch: WebFlatPatchBatch = self.inner.take_outputs().flatten_patches_u32();

        let cells_arr = Uint32Array::from(&batch.cells[..]);
        let spans_arr = Uint32Array::from(&batch.spans[..]);

        let obj = Object::new();
        set_js(&obj, "cells", cells_arr.into());
        set_js(&obj, "spans", spans_arr.into());
        obj.into()
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(10).min(500);
        let rows = rows.max(5).min(200);
        self.inner.resize(cols, rows);
    }

    /// Whether the program is still running.
    #[wasm_bindgen(js_name = isRunning)]
    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    /// Project terrain points for direct canvas rendering.
    /// Returns Float32Array of [x, y, r, g, b, x, y, r, g, b, ...].
    /// canvas_w/canvas_h are the pixel dimensions of the target canvas.
    #[wasm_bindgen(js_name = projectPoints)]
    pub fn project_points(&mut self, canvas_w: f64, canvas_h: f64) -> Float32Array {
        let buf = self.inner.model().project_to_buffer(canvas_w, canvas_h);
        Float32Array::from(&buf[..])
    }

    /// Project packed `[row, col, ...]` pairs to `[x, y, ...]` in one call.
    #[wasm_bindgen(js_name = projectPointsBatch)]
    pub fn project_points_batch(
        &mut self,
        row_col_pairs: &Float32Array,
        canvas_w: f64,
        canvas_h: f64,
    ) -> Float32Array {
        let coords = row_col_pairs.to_vec();
        let out = self
            .inner
            .model()
            .project_row_col_pairs_to_buffer(&coords, canvas_w, canvas_h);
        Float32Array::from(&out[..])
    }

    /// Get current state info string for display.
    #[wasm_bindgen(js_name = stateInfo)]
    pub fn state_info(&mut self) -> String {
        self.inner.model().state_info()
    }

    /// Set the full camera/view state in one call.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = setView)]
    pub fn set_view(
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
        self.inner.model_mut().set_view_state(
            azimuth,
            elevation,
            zoom,
            height_scale,
            density,
            active,
            color_mode,
            auto_rotate,
        );
    }

    // --- Direct getters/setters for JS-driven gesture control ---

    #[wasm_bindgen(js_name = getZoom)]
    pub fn get_zoom(&self) -> f64 {
        self.inner.model().zoom()
    }
    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, v: f64) {
        self.inner.model_mut().set_zoom(v);
    }

    #[wasm_bindgen(js_name = getAzimuth)]
    pub fn get_azimuth(&self) -> f64 {
        self.inner.model().azimuth()
    }
    #[wasm_bindgen(js_name = setAzimuth)]
    pub fn set_azimuth(&mut self, v: f64) {
        self.inner.model_mut().set_azimuth(v);
    }

    #[wasm_bindgen(js_name = getElevation)]
    pub fn get_elevation(&self) -> f64 {
        self.inner.model().elevation()
    }
    #[wasm_bindgen(js_name = setElevation)]
    pub fn set_elevation(&mut self, v: f64) {
        self.inner.model_mut().set_elevation(v);
    }

    #[wasm_bindgen(js_name = getHeightScale)]
    pub fn get_height_scale(&self) -> f64 {
        self.inner.model().height_scale()
    }
    #[wasm_bindgen(js_name = setHeightScale)]
    pub fn set_height_scale(&mut self, v: f64) {
        self.inner.model_mut().set_height_scale(v);
    }

    #[wasm_bindgen(js_name = getDensity)]
    pub fn get_density(&self) -> f64 {
        self.inner.model().density()
    }
    #[wasm_bindgen(js_name = setDensity)]
    pub fn set_density(&mut self, v: f64) {
        self.inner.model_mut().set_density(v);
    }

    #[wasm_bindgen(js_name = getAutoRotate)]
    pub fn get_auto_rotate(&self) -> bool {
        self.inner.model().auto_rotate()
    }
    #[wasm_bindgen(js_name = setAutoRotate)]
    pub fn set_auto_rotate(&mut self, v: bool) {
        self.inner.model_mut().set_auto_rotate(v);
    }

    #[wasm_bindgen(js_name = getActive)]
    pub fn get_active(&self) -> usize {
        self.inner.model().active()
    }
    #[wasm_bindgen(js_name = setActive)]
    pub fn set_active(&mut self, v: usize) {
        self.inner.model_mut().set_active(v);
    }

    #[wasm_bindgen(js_name = getDatasetCount)]
    pub fn get_dataset_count(&self) -> usize {
        self.inner.model().dataset_count()
    }

    #[wasm_bindgen(js_name = getColorMode)]
    pub fn get_color_mode(&self) -> u8 {
        self.inner.model().color_mode()
    }
    #[wasm_bindgen(js_name = setColorMode)]
    pub fn set_color_mode(&mut self, v: u8) {
        self.inner.model_mut().set_color_mode(v);
    }

    #[wasm_bindgen(js_name = getContourInterval)]
    pub fn get_contour_interval(&self) -> f64 {
        self.inner.model().contour_interval()
    }

    /// Project a single fractional grid point to canvas screen coordinates.
    /// Returns Float32Array [x, y] or empty array if no dataset loaded.
    #[wasm_bindgen(js_name = projectPoint)]
    pub fn project_point(
        &mut self,
        row: f64,
        col: f64,
        canvas_w: f64,
        canvas_h: f64,
    ) -> Float32Array {
        match self
            .inner
            .model()
            .project_single_point(row, col, canvas_w, canvas_h)
        {
            Some((sx, sy)) => Float32Array::from(&[sx as f32, sy as f32][..]),
            None => Float32Array::new_with_length(0),
        }
    }

    /// Release resources.
    pub fn destroy(&mut self) {
        // No-op for now
    }
}
