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
        set_js(&obj, "events_processed", JsValue::from(result.events_processed));
        set_js(&obj, "frame_idx", JsValue::from(self.inner.frame_idx() as u32));
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
        let buf = self.inner.model_mut().project_to_buffer(canvas_w, canvas_h);
        Float32Array::from(&buf[..])
    }

    /// Get current state info string for display.
    #[wasm_bindgen(js_name = stateInfo)]
    pub fn state_info(&mut self) -> String {
        self.inner.model_mut().state_info()
    }

    /// Get current zoom level.
    #[wasm_bindgen(js_name = getZoom)]
    pub fn get_zoom(&self) -> f64 {
        self.inner.model().zoom()
    }

    /// Set zoom level directly (clamped to 0.1..8.0).
    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f64) {
        self.inner.model_mut().set_zoom(zoom);
    }

    /// Release resources.
    pub fn destroy(&mut self) {
        // No-op for now
    }
}
