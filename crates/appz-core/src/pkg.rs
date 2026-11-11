use std::collections::HashMap;

use serde_json::Value;

/// Parse package.json `dependencies` + `devDependencies` into name -> version.
pub fn parse_deps(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(val) = serde_json::from_str::<Value>(content) else {
        return map;
    };
    for key in ["dependencies", "devDependencies"] {
        if let Some(Value::Object(deps)) = val.get(key) {
            for (k, v) in deps {
                if let Some(v) = v.as_str() {
                    map.entry(k.clone()).or_insert_with(|| v.to_string());
                }
            }
        }
    }
    map
}

/// Split a detector's package spec into the package name and an optional
/// version prefix: `@sveltejs/kit@1.0.0-next` -> (`@sveltejs/kit`,
/// `Some("1.0.0-next")`). The `@` that starts a scoped name is not a separator.
pub fn split_package_spec(spec: &str) -> (&str, Option<&str>) {
    match spec[1.min(spec.len())..].rfind('@') {
        Some(i) => (&spec[..i + 1], Some(&spec[i + 2..])),
        None => (spec, None),
    }
}

/// Whether `deps` (from [`parse_deps`]) satisfies the detector spec `spec`:
/// the package is listed and, for a version-qualified spec, its declared
/// version (ignoring a leading `^`, `~`, `=` or `v`) starts with that prefix.
pub fn deps_match(deps: &HashMap<String, String>, spec: &str) -> bool {
    let (name, prefix) = split_package_spec(spec);
    match (deps.get(name), prefix) {
        (Some(_), None) => true,
        (Some(version), Some(prefix)) => version
            .trim_start_matches(|c: char| matches!(c, '^' | '~' | '=' | 'v') || c.is_whitespace())
            .starts_with(prefix),
        (None, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_dependency(content: &str, spec: &str) -> bool {
        deps_match(&parse_deps(content), spec)
    }

    #[test]
    fn version_qualified_specs_match_on_the_declared_version() {
        let pre = r#"{"devDependencies":{"@sveltejs/kit":"^1.0.0-next.589"}}"#;
        let stable = r#"{"devDependencies":{"@sveltejs/kit":"^2.5.0"}}"#;
        assert!(has_dependency(pre, "@sveltejs/kit@1.0.0-next"));
        assert!(!has_dependency(stable, "@sveltejs/kit@1.0.0-next"));
        assert!(has_dependency(stable, "@sveltejs/kit"));
        assert!(has_dependency(r#"{"dependencies":{"next":"14"}}"#, "next"));
        assert_eq!(split_package_spec("@scope/pkg"), ("@scope/pkg", None));
        assert_eq!(split_package_spec("pkg@2"), ("pkg", Some("2")));
    }
}
