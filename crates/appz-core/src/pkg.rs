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

/// Whether `name` appears in package.json's dependencies or devDependencies.
pub fn has_dependency(content: &str, name: &str) -> bool {
    parse_deps(content).contains_key(name)
}
