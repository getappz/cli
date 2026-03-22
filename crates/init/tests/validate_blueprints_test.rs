//! Validate all blueprints in the appz-blueprints repo parse correctly.

use std::path::PathBuf;
use init::blueprint_schema::{parse_blueprint, BlueprintSchema};

fn blueprints_dir() -> PathBuf {
    PathBuf::from(env!("HOME"))
        .join("workspace")
        .join("appz-blueprints")
}

const FRAMEWORKS: &[&str] = &[
    "nextjs", "astro", "vite", "sveltekit", "nuxt", "remix",
    "docusaurus", "vitepress", "gatsby", "eleventy", "wordpress", "laravel",
];

#[test]
fn all_default_blueprints_parse() {
    let dir = blueprints_dir();
    if !dir.exists() {
        eprintln!("Skipping: appz-blueprints repo not found at {}", dir.display());
        return;
    }

    for fw in FRAMEWORKS {
        let path = dir.join(fw).join("default").join("blueprint.yaml");
        assert!(path.exists(), "Missing blueprint: {}", path.display());

        let bp = parse_blueprint(&path)
            .unwrap_or_else(|e| panic!("{}/default/blueprint.yaml failed to parse: {}", fw, e));

        // version must be 1
        assert_eq!(bp.version, Some(1), "{}: version should be 1", fw);

        // meta.framework must match directory name
        let meta = bp.meta.as_ref().unwrap_or_else(|| panic!("{}: missing meta", fw));
        assert_eq!(
            meta.framework.as_deref(),
            Some(*fw),
            "{}: meta.framework mismatch",
            fw
        );

        // must have either setup or tasks
        let has_setup = bp.setup.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        let has_tasks = bp.tasks.is_some();
        assert!(
            has_setup || has_tasks,
            "{}: must have setup or tasks",
            fw
        );

        eprintln!("  {} - OK (name: {:?})", fw, meta.name);
    }
}

#[test]
fn all_default_blueprints_have_dev_task() {
    let dir = blueprints_dir();
    if !dir.exists() {
        return;
    }

    for fw in FRAMEWORKS {
        let path = dir.join(fw).join("default").join("blueprint.yaml");
        let bp = parse_blueprint(&path).unwrap();

        if let Some(tasks) = &bp.tasks {
            let tasks_obj = tasks.as_object()
                .unwrap_or_else(|| panic!("{}: tasks should be an object", fw));
            assert!(
                tasks_obj.contains_key("dev"),
                "{}: default blueprint should have a 'dev' task",
                fw
            );
        }
    }
}

#[test]
fn registry_json_is_valid() {
    let dir = blueprints_dir();
    if !dir.exists() {
        return;
    }

    let path = dir.join("registry.json");
    let raw = std::fs::read_to_string(&path).unwrap();
    let index: init::registry::RegistryIndex = serde_json::from_str(&raw).unwrap();

    assert_eq!(index.version, 1);

    // Every framework dir should be in the registry
    for fw in FRAMEWORKS {
        assert!(
            index.has_framework(fw),
            "{} directory exists but not in registry.json",
            fw
        );
        assert!(
            index.has_blueprint(fw, "default"),
            "{} missing 'default' blueprint in registry.json",
            fw
        );
    }
}

#[test]
fn blueprint_create_commands_match_framework_table() {
    let dir = blueprints_dir();
    if !dir.exists() {
        return;
    }

    // Frameworks that should have create_command in their blueprint
    let expected_commands: &[(&str, &str)] = &[
        ("nextjs", "npx create-next-app@latest"),
        ("astro", "npm create astro@latest"),
        ("vite", "npm create vite@latest"),
        ("sveltekit", "npm create svelte@latest"),
        ("nuxt", "npx nuxi@latest init"),
        ("remix", "npx create-remix@latest"),
        ("docusaurus", "npx create-docusaurus@latest"),
        ("vitepress", "npx vitepress@latest init"),
        ("gatsby", "npm create gatsby@latest"),
        ("eleventy", "npm create @11ty/eleventy@latest"),
    ];

    for (fw, expected_cmd) in expected_commands {
        let path = dir.join(fw).join("default").join("blueprint.yaml");
        let bp = parse_blueprint(&path).unwrap();
        let meta = bp.meta.as_ref().unwrap();

        assert_eq!(
            meta.create_command.as_deref(),
            Some(*expected_cmd),
            "{}: create_command mismatch",
            fw
        );
    }
}
