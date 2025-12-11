use task::TaskRegistry;

/// Registers `deploy:copy_dirs` configured to copy the entire tree (".").
///
/// To copy `./build` to `./dist`, run `deploy:copy_dirs` with context:
/// - `previous_release = ./build`
/// - `release_path = ./dist`
pub fn register_test(registry: &mut TaskRegistry) {
	// Reuse the existing copy_dirs recipe; no new task is created.
	// Using ["."] means: copy everything from previous_release into release_path.
	crate::recipe::deploy::copy_dirs::register_copy_dirs(registry, vec!["."]);
}