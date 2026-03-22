use std::path::PathBuf;
use init::blueprint_schema::{parse_blueprint};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn parse_yaml_blueprint() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    assert_eq!(bp.version, Some(1));
    assert_eq!(bp.meta.as_ref().unwrap().name.as_deref(), Some("Test Blueprint"));
    assert_eq!(bp.meta.as_ref().unwrap().framework.as_deref(), Some("nextjs"));
    assert_eq!(bp.meta.as_ref().unwrap().create_command.as_deref(), Some("npx create-next-app@latest"));
    assert_eq!(bp.meta.as_ref().unwrap().package_manager.as_deref(), Some("npm"));
    assert!(bp.setup.as_ref().unwrap().len() == 4);
    assert!(bp.tasks.is_some());
}

#[test]
fn parse_json_blueprint() {
    let bp = parse_blueprint(&fixture("simple_blueprint.json")).unwrap();
    assert_eq!(bp.version, Some(1));
    assert_eq!(bp.meta.as_ref().unwrap().framework.as_deref(), Some("nextjs"));
    assert!(bp.setup.as_ref().unwrap().len() == 1);
}

#[test]
fn parse_jsonc_blueprint() {
    let bp = parse_blueprint(&fixture("simple_blueprint.jsonc")).unwrap();
    assert_eq!(bp.version, Some(1));
    assert_eq!(bp.meta.as_ref().unwrap().framework.as_deref(), Some("vite"));
}

#[test]
fn parse_setup_only_blueprint_is_valid() {
    let bp = parse_blueprint(&fixture("setup_only_blueprint.yaml")).unwrap();
    // setup_only fixture has no tasks key, so it should be None or an empty object/null
    assert!(
        bp.tasks.is_none()
            || bp.tasks.as_ref().unwrap().is_null()
            || bp.tasks.as_ref().unwrap().as_object().map_or(true, |m| m.is_empty())
    );
    assert!(bp.setup.as_ref().unwrap().len() == 1);
}

#[test]
fn parse_add_dependency_step() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    let steps = bp.setup.as_ref().unwrap();
    let step = &steps[0];
    assert_eq!(step.add_dependency.as_ref().unwrap(), &vec!["tailwindcss".to_string(), "postcss".to_string()]);
    assert_eq!(step.dev, Some(true));
}

#[test]
fn parse_write_file_step() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    let steps = bp.setup.as_ref().unwrap();
    let step = &steps[1];
    let wf = step.write_file.as_ref().unwrap();
    assert_eq!(wf.path, "tailwind.config.js");
    assert!(wf.content.as_ref().unwrap().contains("module.exports"));
}

#[test]
fn parse_set_env_step() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    let steps = bp.setup.as_ref().unwrap();
    let step = &steps[2];
    let env = step.set_env.as_ref().unwrap();
    assert_eq!(env.get("APP_NAME").unwrap(), "{{app_name}}");
}
