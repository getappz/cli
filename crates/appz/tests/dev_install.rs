use std::{fs, process::Command};

#[test]
fn dev_generates_the_project_install_command() {
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join(".appz");
    fs::create_dir(&cache).unwrap();
    let registry = include_str!("../../../registry/toolchains.toml");
    let payload = appz_core::CachedPayload {
        fetched_at: 0,
        data: appz_core::parse_registry_toml(registry).unwrap(),
    };
    fs::write(
        cache.join("toolchains-registry.json"),
        serde_json::to_vec(&payload).unwrap(),
    )
    .unwrap();

    for (lockfile, overrides, expected) in [
        ("pnpm-lock.yaml", "{}", "pnpm install"),
        ("bun.lock", "{}", "bun install"),
        (
            "bun.lock",
            r#"{"installCommand":"custom install"}"#,
            "custom install",
        ),
    ] {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"next":"1"}}"#,
        )
        .unwrap();
        fs::write(root.join("turbo.json"), "{}").unwrap();
        fs::write(root.join(lockfile), "").unwrap();
        fs::write(root.join("appz.jsonc"), overrides).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_appz"))
            .env("HOME", home.path())
            .current_dir(root)
            .args(["--json", "dev", "--skip-install"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let toolchains = appz_core::read_latest_state(root).unwrap();
        for slug in ["turbo", "nextjs"] {
            let toolchain = toolchains.iter().find(|tc| tc.slug == slug).unwrap();
            assert_eq!(
                toolchain.install_command.as_deref(),
                Some(expected),
                "{slug}"
            );
        }
        let config = fs::read_to_string(appz_core::mise_config_path(root)).unwrap();
        assert!(
            config.contains(&format!("[tasks.install]\nrun = \"{expected}\"")),
            "{config}"
        );
    }
}
