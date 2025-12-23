#![deny(clippy::use_self)]

mod analyzer;
mod ast_transformer;
mod generator;
mod transformer;
mod types;

pub use analyzer::analyze_project;
pub use ast_transformer::transform_with_ast;
pub use generator::generate_astro_project;
pub use transformer::{transform_component_to_astro, transform_route_to_astro_page, transform_to_astro_simple};
pub use types::{ComponentInfo, MigrationConfig, ProjectAnalysis, RouteInfo};
