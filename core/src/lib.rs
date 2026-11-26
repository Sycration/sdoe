//! sdoe: design-of-experiments mathematics and project engine.
//!
//! The crate contains all calculations and project data handling. Frontends
//! use [`api::Engine`] directly (Rust) or through the C ABI in [`ffi`]; both
//! exchange JSON. File system access is left to the frontend: the engine
//! produces and consumes text (project files, CSV, HTML reports).

pub mod analysis;
pub mod api;
pub mod condition;
pub mod design;
pub mod examples;
pub mod ffi;
pub mod format;
pub mod io;
pub mod linalg;
pub mod model;
pub mod optimize;
pub mod plot;
pub mod project;
pub mod report;
pub mod rscript;
pub mod results;
pub mod rng;
pub mod sheet;
pub mod stats;
pub mod table;

pub use api::Engine;
