mod claude;
mod detect;
mod doctor;
mod frameworks;
mod fs;
mod generator;
mod mise;
mod pkg;
mod storage;
mod toolchains;

pub use claude::generate_claude_md;
pub use detect::{
    DetectedToolchain, MonorepoConfig, detect_monorepo, detect_toolchains, pm_install_cmd,
};
pub use doctor::{DoctorReport, run_doctor};
pub use fs::DetectorFilesystem;
pub use generator::{generate, generate_merged};
pub use mise::{ensure_mise, mise_on_path, trust};
pub use storage::{
    StateSnapshot, StoredToolchain, compute_input_hash, inputs_unchanged, mise_config_path,
    read_latest_snapshot, read_latest_state, state_path, write_state,
};
