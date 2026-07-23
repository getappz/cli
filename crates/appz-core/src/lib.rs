mod claude;
mod detect;
mod mise;
mod doctor;
mod frameworks;
mod fs;
mod generator;
mod pkg;
mod storage;
mod toolchains;

pub use claude::generate_claude_md;
pub use mise::{ensure_mise, mise_on_path};
pub use detect::{detect_monorepo, detect_toolchains, pm_install_cmd, DetectedToolchain, MonorepoConfig};
pub use doctor::{run_doctor, DoctorReport};
pub use fs::DetectorFilesystem;
pub use generator::{generate, generate_merged};
pub use storage::{compute_input_hash, home_dir, inputs_unchanged, mise_config_path, print_location, read_latest_snapshot, read_latest_state, state_path, write_state, StateSnapshot, StoredToolchain};
