#![deny(clippy::use_self)]

mod analyzer;
mod ast_transformer;
mod common;
mod generator;
mod nextjs_generator;
mod transformer;
mod types;

pub use analyzer::analyze_project;
pub use ast_transformer::transform_with_ast;
pub use common::{copy_from_external, copy_public_assets, copy_tailwind_config, filter_deps, filter_lovable_deps};
pub use generator::generate_astro_project;
pub use nextjs_generator::generate_nextjs_project;
pub use transformer::{transform_component_to_astro, transform_route_to_astro_page, transform_to_astro_simple};
pub use types::{ComponentInfo, MigrationConfig, ProjectAnalysis, RouteInfo, SsgSeverity, SsgWarning};
