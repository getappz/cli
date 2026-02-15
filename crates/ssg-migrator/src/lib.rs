#![deny(clippy::use_self)]

// Core types are always available
mod types;
pub use types::{ComponentInfo, MigrationConfig, ProjectAnalysis, RouteInfo, SsgSeverity, SsgWarning};

// -- Native-only modules (direct filesystem, Biome, git2, sandbox) --
#[cfg(feature = "native")]
mod analyzer;
#[cfg(feature = "native")]
mod ast_transformer;
#[cfg(feature = "native")]
mod common;
#[cfg(feature = "native")]
mod generator;
#[cfg(feature = "native")]
mod nextjs;
#[cfg(feature = "native")]
mod sync;
#[cfg(feature = "native")]
mod transformer;

#[cfg(feature = "native")]
pub use analyzer::analyze_project;
#[cfg(feature = "native")]
pub use ast_transformer::transform_with_ast;
#[cfg(feature = "native")]
pub use common::{copy_from_external, copy_public_assets, copy_tailwind_config, filter_deps, filter_lovable_deps};
#[cfg(feature = "native")]
pub use generator::generate_astro_project;
#[cfg(feature = "native")]
pub use nextjs::generate_nextjs_project;
#[cfg(feature = "native")]
pub use transformer::{transform_component_to_astro, transform_route_to_astro_page, transform_to_astro_simple};
#[cfg(feature = "native")]
pub use sync::{
    changed_files, collect_copy_only_mappings, is_git_repo, read_manifest,
    staged_files, sync_backward, sync_forward, write_manifest, SyncManifest,
    SyncResult,
};
