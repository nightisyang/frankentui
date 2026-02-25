#![forbid(unsafe_code)]

pub mod capture;
pub mod cli;
pub mod composition_semantics;
pub mod doctor;
pub mod error;
pub mod harness;
pub mod import;
pub mod keyseq;
pub mod migration_ir;
pub mod module_graph;
pub mod profile;
pub mod redact;
pub mod report;
pub mod runmeta;
pub mod sandbox;
pub mod seed;
pub mod semantic_contract;
pub mod state_effects;
pub mod style_semantics;
pub mod suite;
pub mod tape;
pub mod trace;
pub mod tsx_parser;
pub mod util;

pub use cli::run_from_env;
pub use error::{DoctorError, Result};
