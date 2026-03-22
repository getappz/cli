use std::path::PathBuf;
use blueprint::converter::{convert_playground_to_generic, is_playground_blueprint};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

#[test]
fn detect_playground_blueprint() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    assert!(is_playground_blueprint(&raw));
}

#[test]
fn detect_non_playground_json() {
    let raw = r#"{"version": 1, "meta": {"framework": "nextjs"}}"#;
    assert!(!is_playground_blueprint(raw));
}

#[test]
fn convert_plugins_shorthand() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let plugin_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp plugin install")).unwrap_or(false))
        .collect();
    assert_eq!(plugin_steps.len(), 2);
    assert!(plugin_steps[0].run_locally.as_ref().unwrap().contains("woocommerce"));
    assert!(plugin_steps[1].run_locally.as_ref().unwrap().contains("jetpack"));
}

#[test]
fn convert_site_options() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let opt_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp option update")).unwrap_or(false))
        .collect();
    assert!(opt_steps.len() >= 1);
    assert!(opt_steps[0].run_locally.as_ref().unwrap().contains("blogname"));
}

#[test]
fn convert_install_theme_step() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let theme_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp theme install")).unwrap_or(false))
        .collect();
    assert_eq!(theme_steps.len(), 1);
    assert!(theme_steps[0].run_locally.as_ref().unwrap().contains("astra"));
}

#[test]
fn convert_write_file_step() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let wf_steps: Vec<_> = setup.iter().filter(|s| s.write_file.is_some()).collect();
    assert_eq!(wf_steps.len(), 1);
    assert_eq!(wf_steps[0].write_file.as_ref().unwrap().path, "test.txt");
}

#[test]
fn convert_wp_cli_step() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let cli_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp cache flush")).unwrap_or(false))
        .collect();
    assert_eq!(cli_steps.len(), 1);
}

#[test]
fn convert_sets_wordpress_framework() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    assert_eq!(result.meta.as_ref().unwrap().framework.as_deref(), Some("wordpress"));
}
