use init::registry::{RegistryIndex, resolve_blueprint_url};

#[test]
fn parse_registry_index() {
    let json = r#"{"version": 1, "frameworks": {"nextjs": {"name": "Next.js", "blueprints": {"default": {"description": "Base Next.js setup"}, "ecommerce": {"description": "E-commerce starter"}}}}}"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert_eq!(index.version, 1);
    assert!(index.frameworks.contains_key("nextjs"));
    let nextjs = &index.frameworks["nextjs"];
    assert_eq!(nextjs.name, "Next.js");
    assert!(nextjs.blueprints.contains_key("default"));
    assert!(nextjs.blueprints.contains_key("ecommerce"));
}

#[test]
fn resolve_blueprint_url_from_registry() {
    let url = resolve_blueprint_url("nextjs", "ecommerce");
    assert!(url.contains("nextjs/ecommerce/blueprint.yaml"));
}

#[test]
fn resolve_default_blueprint_url() {
    let url = resolve_blueprint_url("nextjs", "default");
    assert!(url.contains("nextjs/default/blueprint.yaml"));
}

#[test]
fn registry_has_blueprint_check() {
    let json = r#"{"version": 1, "frameworks": {"nextjs": {"name": "Next.js", "blueprints": {"default": {"description": "Base"}}}}}"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert!(index.has_blueprint("nextjs", "default"));
    assert!(!index.has_blueprint("nextjs", "ecommerce"));
    assert!(!index.has_blueprint("rails", "default"));
}
