use blueprint::types::*;

#[test]
fn parse_full_blueprint() {
    let json = include_str!("fixtures/full_blueprint.json");
    let bp: Blueprint = serde_json::from_str(json).expect("Failed to parse blueprint");

    // Top-level fields
    assert_eq!(bp.schema.as_deref(), Some("https://playground.wordpress.net/blueprint-schema.json"));
    assert_eq!(bp.meta.as_ref().unwrap().title.as_deref(), Some("Test Blueprint"));
    assert_eq!(bp.landing_page.as_deref(), Some("/wp-admin/"));

    // Preferred versions
    let versions = bp.preferred_versions.as_ref().unwrap();
    assert_eq!(versions.php.as_deref(), Some("8.3"));
    assert_eq!(versions.wp.as_deref(), Some("latest"));

    // Features
    assert_eq!(bp.features.as_ref().unwrap().networking, Some(true));

    // Extra libraries
    assert_eq!(bp.extra_libraries, vec!["wp-cli"]);

    // Constants
    let consts = bp.constants.as_ref().unwrap();
    assert_eq!(consts.get("WP_DEBUG").unwrap(), &serde_json::Value::Bool(true));

    // Plugins shorthand
    assert_eq!(bp.plugins.len(), 1);

    // Site options shorthand
    let opts = bp.site_options.as_ref().unwrap();
    assert_eq!(opts.get("blogname").unwrap(), "Test Site");

    // Login shorthand
    assert!(matches!(bp.login, Some(LoginShorthand::Bool(true))));

    // Steps: all 29 types covered (some appear multiple times)
    assert_eq!(bp.steps.len(), 31); // 31 entries in the fixture

    // Verify each step type parses correctly
    let steps: Vec<&Step> = bp.steps.iter().filter_map(|e| {
        if let StepEntry::Step(s) = e { Some(s) } else { None }
    }).collect();

    // Check that we got all step types
    let type_names: Vec<&str> = steps.iter().map(|s| s.type_name()).collect();
    assert!(type_names.contains(&"installPlugin"));
    assert!(type_names.contains(&"installTheme"));
    assert!(type_names.contains(&"activatePlugin"));
    assert!(type_names.contains(&"activateTheme"));
    assert!(type_names.contains(&"setSiteOptions"));
    assert!(type_names.contains(&"defineWpConfigConsts"));
    assert!(type_names.contains(&"defineSiteUrl"));
    assert!(type_names.contains(&"setSiteLanguage"));
    assert!(type_names.contains(&"login"));
    assert!(type_names.contains(&"runPHP"));
    assert!(type_names.contains(&"runPHPWithOptions"));
    assert!(type_names.contains(&"wp-cli"));
    assert!(type_names.contains(&"mkdir"));
    assert!(type_names.contains(&"writeFile"));
    assert!(type_names.contains(&"writeFiles"));
    assert!(type_names.contains(&"cp"));
    assert!(type_names.contains(&"mv"));
    assert!(type_names.contains(&"rm"));
    assert!(type_names.contains(&"rmdir"));
    assert!(type_names.contains(&"resetData"));
    assert!(type_names.contains(&"enableMultisite"));
    assert!(type_names.contains(&"updateUserMeta"));
    assert!(type_names.contains(&"runSql"));
    assert!(type_names.contains(&"importWxr"));
    assert!(type_names.contains(&"importThemeStarterContent"));
    assert!(type_names.contains(&"unzip"));
    assert!(type_names.contains(&"request"));
    assert!(type_names.contains(&"runWpInstallationWizard"));
    assert!(type_names.contains(&"importWordPressFiles"));
}

#[test]
fn parse_minimal_blueprint() {
    let json = r#"{
        "steps": [
            { "step": "installPlugin", "pluginData": { "resource": "wordpress.org/plugins", "slug": "woocommerce" } }
        ]
    }"#;
    let bp: Blueprint = serde_json::from_str(json).expect("Failed to parse minimal blueprint");
    assert_eq!(bp.steps.len(), 1);
}

#[test]
fn parse_login_variants() {
    // Boolean login
    let json = r#"{ "login": true, "steps": [] }"#;
    let bp: Blueprint = serde_json::from_str(json).unwrap();
    assert!(matches!(bp.login, Some(LoginShorthand::Bool(true))));

    // Credentials login
    let json = r#"{ "login": { "username": "admin", "password": "pass" }, "steps": [] }"#;
    let bp: Blueprint = serde_json::from_str(json).unwrap();
    match bp.login {
        Some(LoginShorthand::Credentials { username, password }) => {
            assert_eq!(username.as_deref(), Some("admin"));
            assert_eq!(password.as_deref(), Some("pass"));
        }
        _ => panic!("Expected Credentials"),
    }
}

#[test]
fn parse_step_entries_with_skips() {
    let json = r#"{
        "steps": [
            { "step": "login" },
            false,
            null,
            "hello-dolly"
        ]
    }"#;
    let bp: Blueprint = serde_json::from_str(json).unwrap();
    assert_eq!(bp.steps.len(), 4);
    assert!(matches!(bp.steps[0], StepEntry::Step(Step::Login(_))));
    assert!(matches!(bp.steps[1], StepEntry::Bool(false)));
    // null becomes StepEntry::Null or may fail; let's verify parsing doesn't crash
}

#[test]
fn parse_wp_cli_string_and_array() {
    let json = r#"{ "steps": [
        { "step": "wp-cli", "command": "post list" },
        { "step": "wp-cli", "command": ["option", "get", "blogname"] }
    ] }"#;
    let bp: Blueprint = serde_json::from_str(json).unwrap();
    assert_eq!(bp.steps.len(), 2);
}

#[test]
fn parse_plugin_resources() {
    let json = r#"{ "steps": [
        { "step": "installPlugin", "pluginData": { "resource": "wordpress.org/plugins", "slug": "coblocks" } },
        { "step": "installPlugin", "pluginData": { "resource": "url", "url": "https://example.com/plugin.zip" } }
    ] }"#;
    let bp: Blueprint = serde_json::from_str(json).unwrap();
    assert_eq!(bp.steps.len(), 2);
}
