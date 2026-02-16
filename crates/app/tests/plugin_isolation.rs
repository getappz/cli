//! Security tests: WASM plugins must not be able to use WASM from other apps.
//!
//! These tests simulate attacker scenarios:
//! - Loading a plugin from "another app" (wrong plugin_id)
//! - Swapping plugins (e.g. use check.wasm when running migrate)
//! - Loading arbitrary WASM without appz header

use app::wasm::PluginRunner;
use plugin_manager::security;
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_plugin_runner_rejects_wasm_from_other_apps() {
    // Simulate hacker: provide WASM that claims to be "evil-app" / "other-product"
    // when we expect "ssg-migrator". Must be rejected.
    let wasm = security::build_test_wasm_with_header("evil-app", "0.1.0");

    let temp = tempdir().unwrap();
    let project_dir = temp.path();
    let wasm_path = project_dir.join("evil.wasm");
    starbase_utils::fs::write_file(&wasm_path, &wasm).unwrap();

    let config = SandboxConfig::new(project_dir).with_settings(SandboxSettings {
        auto_install_mise: false,
        quiet: true,
        ..Default::default()
    });

    let sandbox = create_sandbox(config).await.unwrap();
    let scoped_fs = Arc::new(sandbox::ScopedFs::new(project_dir).unwrap());
    let sandbox_arc: Arc<dyn sandbox::SandboxProvider> = Arc::from(sandbox);

    let mut runner = PluginRunner::new(sandbox_arc, scoped_fs);
    let result = runner.load_verified_plugin(&wasm_path, "ssg-migrator");

    assert!(
        result.is_err(),
        "Must reject WASM from other apps - cannot use evil-app WASM for ssg-migrator"
    );
}

#[tokio::test]
async fn test_plugin_runner_rejects_plugin_swap() {
    // Simulate hacker: swap plugins - provide check.wasm when running migrate.
    // Plugin ID in header is "check", expected is "ssg-migrator". Must be rejected.
    let wasm = security::build_test_wasm_with_header("check", "0.1.0");

    let temp = tempdir().unwrap();
    let project_dir = temp.path();
    let wasm_path = project_dir.join("check.wasm");
    starbase_utils::fs::write_file(&wasm_path, &wasm).unwrap();

    let config = SandboxConfig::new(project_dir).with_settings(SandboxSettings {
        auto_install_mise: false,
        quiet: true,
        ..Default::default()
    });

    let sandbox = create_sandbox(config).await.unwrap();
    let scoped_fs = Arc::new(sandbox::ScopedFs::new(project_dir).unwrap());
    let sandbox_arc: Arc<dyn sandbox::SandboxProvider> = Arc::from(sandbox);

    let mut runner = PluginRunner::new(sandbox_arc, scoped_fs);
    let result = runner.load_verified_plugin(&wasm_path, "ssg-migrator");

    assert!(
        result.is_err(),
        "Must reject plugin swap - cannot use check WASM for migrate command"
    );
}

#[tokio::test]
async fn test_plugin_runner_rejects_arbitrary_wasm() {
    // Simulate hacker: provide arbitrary WASM (e.g. from npm, another product)
    // with no appz_header. Must be rejected.
    let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]; // Minimal WASM, no custom section

    let temp = tempdir().unwrap();
    let project_dir = temp.path();
    let wasm_path = project_dir.join("arbitrary.wasm");
    starbase_utils::fs::write_file(&wasm_path, &wasm).unwrap();

    let config = SandboxConfig::new(project_dir).with_settings(SandboxSettings {
        auto_install_mise: false,
        quiet: true,
        ..Default::default()
    });

    let sandbox = create_sandbox(config).await.unwrap();
    let scoped_fs = Arc::new(sandbox::ScopedFs::new(project_dir).unwrap());
    let sandbox_arc: Arc<dyn sandbox::SandboxProvider> = Arc::from(sandbox);

    let mut runner = PluginRunner::new(sandbox_arc, scoped_fs);
    let result = runner.load_verified_plugin(&wasm_path, "ssg-migrator");

    assert!(
        result.is_err(),
        "Must reject arbitrary WASM without appz header"
    );
}
