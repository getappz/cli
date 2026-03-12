#![deny(clippy::use_self)]

// Core types are always available
mod types;
pub use types::{
    ComponentInfo, MigrationConfig, PreMigrationReport, ProjectAnalysis, RouteInfo, SsgSeverity,
    SsgWarning,
};

// Virtual filesystem abstraction (always available)
pub mod vfs;
pub use vfs::{FsEntry, Vfs};

// Native Vfs implementation (std::fs + walkdir + git2)
#[cfg(feature = "native")]
pub mod vfs_native;
#[cfg(feature = "native")]
pub use vfs_native::NativeFs;

// Modules that use the Vfs trait — always compiled.
mod analyzer;
mod ast_transformer;
mod common;
mod generator;
mod nextjs;
mod sync;
mod transformer;

pub use analyzer::{analyze_project, run_pre_migration_scan};
pub use ast_transformer::transform_with_ast;
pub use common::{copy_public_assets, copy_tailwind_config, filter_deps, filter_lovable_deps};
pub use generator::generate_astro_project;
pub use nextjs::{convert_to_nextjs, generate_nextjs_project, parse_transforms, NextJsTransform};
pub use transformer::{
    convert_to_astro, transform_component_to_astro, transform_route_to_astro_page,
    transform_to_astro_simple, AstroConvertOptions,
};
pub use sync::{
    changed_files, collect_copy_only_mappings, is_git_repo, read_manifest,
    staged_files, sync_backward, sync_forward, write_manifest, SyncManifest,
    SyncResult,
};
