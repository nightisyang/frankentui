#![forbid(unsafe_code)]

//! WASM terrain viewer using FrankenTUI braille rendering.
//!
//! Renders 3D terrain as colored braille dot patterns with curvature-adaptive
//! interpolation — more dots at ridges/valleys, fewer on flat areas.

mod terrain_model;

#[cfg(target_arch = "wasm32")]
mod wasm;
