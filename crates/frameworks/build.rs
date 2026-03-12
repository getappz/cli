use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=data/frameworks.json");
    println!("cargo:rerun-if-changed=data/php-frameworks.json");

    let out_dir = env::var("OUT_DIR").unwrap();

    // Generate Node.js frameworks
    generate_frameworks(
        "data/frameworks.json",
        &Path::new(&out_dir).join("frameworks_generated.rs"),
        "frameworks.json",
    );

    // Generate PHP frameworks
    generate_frameworks(
        "data/php-frameworks.json",
        &Path::new(&out_dir).join("php_frameworks_generated.rs"),
        "php-frameworks.json",
    );
}

fn generate_frameworks(json_path: &str, out_path: &Path, source_name: &str) {
    let json_content = fs::read_to_string(json_path).unwrap_or_else(|_| {
        // If file doesn't exist, create empty array
        if json_path.contains("php-frameworks") {
            "[]".to_string()
        } else {
            panic!("Failed to read {}", json_path);
        }
    });

    let frameworks: Vec<serde_json::Value> = serde_json::from_str(&json_content)
        .unwrap_or_else(|_| panic!("Failed to parse {}", json_path));

    let (data_name, map_name) = if json_path.contains("php") {
        ("PHP_FRAMEWORKS_DATA", "PHP_FRAMEWORKS_BY_SLUG")
    } else {
        ("FRAMEWORKS_DATA", "FRAMEWORKS_BY_SLUG")
    };

    let mut struct_entries = Vec::new();
    let mut phf_entries = Vec::new();

    for (idx, fw) in frameworks.iter().enumerate() {
        let struct_code = generate_framework_struct(fw, idx);
        struct_entries.push(struct_code);

        // Generate PHF entry for slug lookup
        if let Some(slug) = fw.get("slug").and_then(|s| s.as_str()) {
            phf_entries.push(format!(r#""{}" => &{}[{}],"#, slug, data_name, idx));
        }
    }

    let generated = format!(
        r#"
// Auto-generated file - DO NOT EDIT
// Generated from data/{}

#[allow(unused_imports)]
use crate::types::*;

static {}: &[Framework] = &[
{}
];

pub static {}: phf::Map<&'static str, &'static Framework> = phf::phf_map! {{
{}
}};
"#,
        source_name,
        data_name,
        struct_entries.join(",\n"),
        map_name,
        phf_entries.join("\n    ")
    );

    fs::write(out_path, generated)
        .unwrap_or_else(|_| panic!("Failed to write generated code to {:?}", out_path));
}

fn generate_framework_struct(fw: &serde_json::Value, _idx: usize) -> String {
    let name = escape_str_for_string_literal(fw["name"].as_str().unwrap_or(""));
    let slug = opt_str(fw.get("slug"));
    let demo = opt_str(fw.get("demo"));
    let logo = opt_str(fw.get("logo"));
    let dark_mode_logo = opt_str(fw.get("darkModeLogo"));
    let screenshot = opt_str(fw.get("screenshot"));
    let tagline = opt_str(fw.get("tagline"));
    let description = opt_str(fw.get("description"));
    let website = opt_str(fw.get("website"));
    let sort = fw.get("sort").and_then(|s| s.as_u64());
    let env_prefix = opt_str(fw.get("envPrefix"));
    let dependency = opt_str(fw.get("dependency"));
    let cache_pattern = opt_str(fw.get("cachePattern"));

    let use_runtime = generate_use_runtime(fw.get("useRuntime"));
    let ignore_runtimes = generate_string_array(fw.get("ignoreRuntimes"));
    let detectors = generate_detectors(fw.get("detectors"));
    let settings = generate_settings(fw.get("settings"));
    let recommended_integrations =
        generate_recommended_integrations(fw.get("recommendedIntegrations"));
    let supersedes = generate_string_array(fw.get("supersedes"));

    format!(
        r#"    Framework {{
        name: "{}",
        slug: {},
        demo: {},
        logo: {},
        dark_mode_logo: {},
        screenshot: {},
        tagline: {},
        description: {},
        website: {},
        sort: {},
        env_prefix: {},
        use_runtime: {},
        ignore_runtimes: {},
        detectors: {},
        settings: {},
        recommended_integrations: {},
        dependency: {},
        supersedes: {},
        cache_pattern: {},
    }}"#,
        name,
        slug,
        demo,
        logo,
        dark_mode_logo,
        screenshot,
        tagline,
        description,
        website,
        sort.map(|s| format!("Some({})", s))
            .unwrap_or_else(|| "None".to_string()),
        env_prefix,
        use_runtime,
        ignore_runtimes,
        detectors,
        settings,
        recommended_integrations,
        dependency,
        supersedes,
        cache_pattern
    )
}

fn escape_str_for_string_literal(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' => "\\\\".to_string(),
            '"' => "\\\"".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

fn opt_str(val: Option<&serde_json::Value>) -> String {
    val.and_then(|v| v.as_str())
        .map(|s| {
            let escaped = escape_str_for_string_literal(s);
            format!(r#"Some("{}")"#, escaped)
        })
        .unwrap_or_else(|| "None".to_string())
}

fn generate_use_runtime(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(v) if v.is_object() => {
            let src = v.get("src").and_then(|s| s.as_str()).unwrap_or("");
            let use_val = v.get("use").and_then(|s| s.as_str()).unwrap_or("");
            format!(
                r#"Some(UseRuntimeStatic {{ src: "{}", r#use: "{}" }})"#,
                escape_str_for_string_literal(src),
                escape_str_for_string_literal(use_val)
            )
        }
        _ => "None".to_string(),
    }
}

fn generate_string_array(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(v) if v.is_array() => {
            let items: Vec<String> = v
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!(r#""{}""#, escape_str_for_string_literal(s)))
                .collect();
            if items.is_empty() {
                "None".to_string()
            } else {
                format!(r#"Some(&[{}])"#, items.join(", "))
            }
        }
        _ => "None".to_string(),
    }
}

fn generate_detectors(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(v) if v.is_object() => {
            let every = generate_detector_array(v.get("every"));
            let some = generate_detector_array(v.get("some"));

            let every_code = if every.is_empty() {
                "None".to_string()
            } else {
                format!(r#"Some(&[{}])"#, every.join(", "))
            };

            let some_code = if some.is_empty() {
                "None".to_string()
            } else {
                format!(r#"Some(&[{}])"#, some.join(", "))
            };

            format!(
                r#"Some(DetectorsStatic {{ every: {}, some: {} }})"#,
                every_code, some_code
            )
        }
        _ => "None".to_string(),
    }
}

fn generate_detector_array(val: Option<&serde_json::Value>) -> Vec<String> {
    match val {
        Some(v) if v.is_array() => {
            v.as_array()
                .unwrap()
                .iter()
                .filter_map(|det| {
                    if let Some(pkg) = det.get("matchPackage").and_then(|s| s.as_str()) {
                        Some(format!(
                            r#"DetectorStatic::MatchPackage {{ match_package: "{}" }}"#,
                            escape_str_for_string_literal(pkg)
                        ))
                    } else if let Some(composer_pkg) = det.get("matchComposerPackage").and_then(|s| s.as_str()) {
                        Some(format!(
                            r#"DetectorStatic::MatchComposerPackage {{ match_composer_package: "{}" }}"#,
                            escape_str_for_string_literal(composer_pkg)
                        ))
                    } else if let Some(path) = det.get("path").and_then(|s| s.as_str()) {
                        if let Some(content) = det.get("matchContent").and_then(|s| s.as_str()) {
                            Some(format!(
                                r#"DetectorStatic::MatchContent {{ path: "{}", match_content: "{}" }}"#,
                                escape_str_for_string_literal(path),
                                escape_str_for_string_literal(content)
                            ))
                        } else {
                            Some(format!(
                                r#"DetectorStatic::Path {{ path: "{}" }}"#,
                                escape_str_for_string_literal(path)
                            ))
                        }
                    } else {
                        None
                    }
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn generate_settings(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(v) if v.is_object() => {
            let install = generate_command_setting(v.get("installCommand"));
            let build = generate_command_setting(v.get("buildCommand"));
            let dev = generate_command_setting(v.get("devCommand"));
            let output = generate_command_setting(v.get("outputDirectory"));

            format!(
                r#"Some(SettingsStatic {{
            install_command: {},
            build_command: {},
            dev_command: {},
            output_directory: {},
        }})"#,
                install, build, dev, output
            )
        }
        _ => "None".to_string(),
    }
}

fn generate_command_setting(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(v) if v.is_object() => {
            let placeholder = opt_str(v.get("placeholder"));
            let value = opt_str(v.get("value"));
            format!(
                r#"Some(CommandSettingStatic {{ placeholder: {}, value: {} }})"#,
                placeholder, value
            )
        }
        _ => "None".to_string(),
    }
}

fn generate_recommended_integrations(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(v) if v.is_array() => {
            let items: Vec<String> = v
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|item| {
                    if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                        let deps = item
                            .get("dependencies")
                            .and_then(|d| d.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| format!(r#""{}""#, escape_str_for_string_literal(s)))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_else(String::new);

                        Some(format!(
                            r#"RecommendedIntegrationStatic {{ id: "{}", dependencies: &[{}] }}"#,
                            escape_str_for_string_literal(id),
                            deps
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            if items.is_empty() {
                "None".to_string()
            } else {
                format!(r#"Some(&[{}])"#, items.join(", "))
            }
        }
        _ => "None".to_string(),
    }
}
