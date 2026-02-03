#![forbid(unsafe_code)]

pub mod metaballs;
pub mod plasma;
pub mod sampling;

#[cfg(feature = "canvas")]
pub mod canvas_adapters;

pub use metaballs::{Metaball, MetaballsFx, MetaballsPalette, MetaballsParams};
pub use plasma::{PlasmaFx, PlasmaPalette, plasma_wave, plasma_wave_low};
pub use sampling::{
    BallState, CoordCache, FnSampler, MetaballFieldSampler, PlasmaSampler, Sampler,
    cell_to_normalized, fill_normalized_coords,
};

#[cfg(feature = "canvas")]
pub use canvas_adapters::{MetaballsCanvasAdapter, PlasmaCanvasAdapter};
