//! Migration command — now handled by the downloadable `ssg-migrator` plugin.
//!
//! This module is kept as a stub for backward compatibility. The actual
//! migration functionality has been extracted to the `ssg-migrator-plugin`
//! WASM crate, which is downloaded on demand when the user runs `appz migrate`.
//!
//! The `Migrate` command variant has been removed from the `Commands` enum
//! and is now caught by the `External(Vec<String>)` catch-all variant,
//! which triggers the plugin system.

// This module intentionally left minimal. The migrate command is now a plugin.
