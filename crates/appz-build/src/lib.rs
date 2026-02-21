//! # appz-build
//!
//! Framework detection and build pipeline for the Appz platform.
//!
//! Ports Vercel's fs-detectors, frameworks, and static-build logic for local
//! static site builds. Produces Build Output v3–style `.appz/output/` for
//! deployment.
//!
//! ## Modules
//!
//! - [`detect`] — framework detection via detectors + frameworks
//! - [`build`] — run install and build via sandbox, validate output
//! - [`output`] — produce standardized `.appz/output/` from build artifacts

pub mod build;
pub mod detect;
pub mod output;

pub use build::{run_build, run_install, validate_output_dir};
pub use output::{resolve_build_output_dir, APPZ_OUTPUT_DIR};
pub use detect::detect_framework;
pub use output::produce_standardized_output;
