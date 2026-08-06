mod claude;
mod clean;
mod detect;
mod diagnostics;
mod doctor;
mod format_defaults;
mod frameworks;
mod fs;
mod generator;
mod mise;
mod paths;
mod pkg;
mod registry_cache;
mod source;
mod storage;
mod toolchain_registry;
mod toolchains;

pub use claude::generate_claude_md;
pub use clean::{
    CleanReport, CleanTarget, CleanTargetKind, delete_targets, format_size, plan_clean,
};
pub use detect::{
    DetectedToolchain, MonorepoConfig, detect_monorepo, detect_toolchains, pm_install_cmd,
    strip_jsonc,
};
pub use diagnostics::{Diagnostic, parse_cargo_diagnostics};
pub use doctor::{DoctorReport, run_doctor};
pub use fs::DetectorFilesystem;
pub use generator::{generate, generate_merged};
pub use mise::{ensure_mise, mise_on_path, trust};
pub use paths::{canonicalize, find_node_modules_bin_paths};
pub use registry_cache::{CachedPayload, load_cached};
pub use source::{Host, InitSource, RemoteSource, archive_url, classify, parse_remote};
pub use storage::{
    StateSnapshot, StoredToolchain, compute_input_hash, inputs_unchanged, mise_config_path,
    read_latest_snapshot, read_latest_state, state_path, write_state,
};
pub use toolchain_registry::{
    RegistryDetectionCriteria, RegistryDetector, RegistryFrameworkSpec, RegistryLifecycleCommands,
    cache_path, convert_to_framework, framework_to_spec, get_frameworks, load_frameworks_registry,
    parse_registry_toml, refresh_registry_cache,
};
