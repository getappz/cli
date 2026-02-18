//! Detect project characteristics and recommend skills via skills.sh API.

use crate::context::SkillsContext;
use crate::find;
use crate::monorepo;
use detectors::{detect_for_skills, StdFilesystem};
use std::collections::{HashMap, BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use starbase::AppResult;
use starbase_utils::fs;

const SKILLS_JSON_FILE: &str = "skills.json";
const SKILLS_SCHEMA: &str = "https://unpkg.com/skillman/skills_schema.json";

/// Curated/official skill recommendations based on detected frameworks.
const CURATED_SKILLS: &[(&str, &[&str])] = &[
    ("nextjs", &[
        "vercel-labs/next-skills@next-best-practices",
        "vercel-labs/next-skills@next-upgrade",
        "vercel-labs/agent-skills@vercel-react-best-practices",
    ]),
    ("react", &["vercel-labs/agent-skills@vercel-react-best-practices"]),
    ("turborepo", &["vercel/turborepo@turborepo"]),
];

/// Web frameworks that should get design guidelines skill.
const WEB_FRAMEWORKS: &[&str] = &[
    "nextjs", "react", "vue", "svelte", "nuxt", "remix", "astro",
    "gatsby", "angular", "solid", "qwik",
];

/// Ecosystem markers: skills with these in the ref are for that ecosystem.
/// If project doesn't use the ecosystem, we filter the skill out.
const ECOSYSTEM_MARKERS: &[(&str, &[&str])] = &[
    ("expo", &["expo", "react-native", "mobile"]),
    ("react-native", &["expo", "react-native", "mobile"]),
    ("flutter", &["flutter", "dart"]),
    ("android", &["android", "kotlin", "gradle"]),
    ("ios", &["ios", "swift", "xcode", "cocoapods"]),
    ("unity", &["unity", "gamedev"]),
];

fn load_all_dependencies(path: &Path) -> HashMap<String, String> {
    let mut all = HashMap::new();
    let pkg_path = path.join("package.json");
    if !pkg_path.exists() {
        return all;
    }
    let Ok(content) = fs::read_file(&pkg_path) else {
        return all;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return all;
    };
    for key in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            for (name, val) in obj {
                let ver = val.as_str().unwrap_or("*").to_string();
                all.insert(name.clone(), ver);
            }
        }
    }
    all
}

/// Merge dependencies from multiple packages. First-seen version wins.
fn load_all_dependencies_multi(paths: &[std::path::PathBuf]) -> HashMap<String, String> {
    let mut all = HashMap::new();
    for path in paths {
        for (name, ver) in load_all_dependencies(path) {
            all.entry(name).or_insert(ver);
        }
    }
    all
}

fn parse_skill_ref(ref_str: &str) -> (String, String) {
    if let Some(at) = ref_str.find('@') {
        (
            ref_str[..at].to_string(),
            ref_str[at + 1..].to_string(),
        )
    } else {
        (ref_str.to_string(), String::new())
    }
}

fn group_skills_by_source(refs: &[String]) -> Vec<serde_json::Value> {
    let mut by_source: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in refs {
        let (source, skill) = parse_skill_ref(r);
        if !skill.is_empty() {
            by_source
                .entry(source)
                .or_default()
                .push(skill);
        }
    }
    for skills in by_source.values_mut() {
        skills.sort();
        skills.dedup();
    }
    by_source
        .into_iter()
        .map(|(source, skills)| {
            serde_json::json!({
                "source": source,
                "skills": skills
            })
        })
        .collect()
}

fn is_relevant_skill(skill_ref: &str, term: &str, detected_frameworks: &[String]) -> bool {
    let lower_ref = skill_ref.to_lowercase();
    let lower_term = term.to_lowercase();

    // Word-boundary style match: term should appear as whole word
    let has_term = {
        let pattern = format!(r"(^|[^a-z0-9]){}([^a-z0-9]|$)", regex::escape(&lower_term));
        regex::Regex::new(&pattern).ok().map_or(false, |re| re.is_match(&lower_ref))
            || (lower_term.contains('-') && lower_ref.contains(&lower_term))
    };
    if !has_term {
        return false;
    }

    // Filter by ecosystem: if skill targets an ecosystem the project doesn't use, skip
    for (ecosystem, markers) in ECOSYSTEM_MARKERS {
        let has_marker = markers.iter().any(|m| lower_ref.contains(&m.to_lowercase()));
        if has_marker {
            let project_uses = detected_frameworks.iter().any(|fw| {
                fw.to_lowercase() == *ecosystem
                    || markers.iter().any(|m| fw.to_lowercase() == m.to_lowercase())
            });
            if !project_uses {
                return false;
            }
        }
    }
    true
}

/// Run skills init: analyze project and recommend skills (writes skills.json).
pub async fn run_detect(
    ctx: &SkillsContext,
    path: Option<std::path::PathBuf>,
    json: bool,
    skip_search: bool,
    output: Option<std::path::PathBuf>,
) -> AppResult {
    let cwd = path
        .unwrap_or_else(|| ctx.working_dir.clone())
        .canonicalize()
        .unwrap_or_else(|_| ctx.working_dir.clone());

    let packages = monorepo::discover_packages(&cwd);

    if packages.len() > 1 && !json {
        let _ = ui::layout::blank_line();
        let _ = ui::status::info(&format!("Detected {} packages in monorepo", packages.len()));
    }

    let result = if packages.len() <= 1 {
        let pkg_path = packages.first().unwrap_or(&cwd);
        let deps = load_all_dependencies(pkg_path);
        let fs: Arc<dyn detectors::DetectorFilesystem> =
            Arc::new(StdFilesystem::new(Some(pkg_path.clone())));
        detect_for_skills(&fs, &deps)
            .await
            .map_err(|e| miette::miette!("Detection failed: {}", e))?
    } else {
        let mut frameworks: HashSet<String> = HashSet::new();
        let mut languages: HashSet<String> = HashSet::new();
        let mut tools: HashSet<String> = HashSet::new();
        let mut testing: HashSet<String> = HashSet::new();
        let mut search_terms: HashSet<String> = HashSet::new();
        let mut package_manager: Option<String> = None;

        for pkg_path in &packages {
            let pkg_deps = load_all_dependencies(pkg_path);
            let fs: Arc<dyn detectors::DetectorFilesystem> =
                Arc::new(StdFilesystem::new(Some(pkg_path.clone())));
            let res = detect_for_skills(&fs, &pkg_deps)
                .await
                .map_err(|e| miette::miette!("Detection failed for {}: {}", pkg_path.display(), e))?;
            frameworks.extend(res.frameworks);
            languages.extend(res.languages);
            tools.extend(res.tools);
            testing.extend(res.testing);
            search_terms.extend(res.search_terms);
            if package_manager.is_none() {
                package_manager = res.package_manager;
            }
        }

        if package_manager.is_none() {
            let root_fs: Arc<dyn detectors::DetectorFilesystem> =
                Arc::new(StdFilesystem::new(Some(cwd.clone())));
            package_manager = detectors::detect_package_manager(&root_fs)
                .await
                .ok()
                .flatten()
                .map(|pm| pm.manager);
        }

        let mut search_terms_vec: Vec<String> = search_terms.into_iter().collect();
        search_terms_vec.sort();
        search_terms_vec.dedup_by_key(|s| s.to_lowercase());

        detectors::SkillsDetectionResult {
            package_manager,
            frameworks: frameworks.into_iter().collect::<Vec<_>>(),
            languages: languages.into_iter().collect::<Vec<_>>(),
            tools: tools.into_iter().collect::<Vec<_>>(),
            testing: testing.into_iter().collect::<Vec<_>>(),
            search_terms: search_terms_vec,
        }
    };

    let timestamp = chrono::Utc::now().to_rfc3339();
    let detected = serde_json::json!({
        "packageManager": result.package_manager,
        "frameworks": result.frameworks,
        "languages": result.languages,
        "tools": result.tools,
        "testing": result.testing,
        "searchTerms": result.search_terms,
        "timestamp": timestamp,
    });

    if !json {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title("Project Analysis");
        let _ = ui::layout::blank_line();
        if let Some(ref pm) = result.package_manager {
            let _ = ui::status::info(&format!("Pkg Manager: {}", pm));
        }
        if !result.frameworks.is_empty() {
            let _ = ui::status::info(&format!("Frameworks:  {}", result.frameworks.join(", ")));
        }
        if !result.languages.is_empty() {
            let _ = ui::status::info(&format!("Languages:   {}", result.languages.join(", ")));
        }
        if !result.tools.is_empty() {
            let _ = ui::status::info(&format!("Tools:       {}", result.tools.join(", ")));
        }
        if !result.testing.is_empty() {
            let _ = ui::status::info(&format!("Testing:     {}", result.testing.join(", ")));
        }
        if result.search_terms.is_empty() {
            let _ = ui::status::info("No project characteristics detected.");
            return Ok(None);
        }
        let _ = ui::layout::blank_line();
        let _ = ui::status::info(&format!("Search terms: {}", result.search_terms.join(", ")));
    }

    if skip_search {
        if json {
            let out = serde_json::json!({
                "$schema": SKILLS_SCHEMA,
                "detected": detected,
                "skills": []
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        }
        return Ok(None);
    }

    let mut all_skill_refs: Vec<String> = Vec::new();
    let curated_keys: std::collections::HashSet<_> =
        CURATED_SKILLS.iter().map(|(k, _)| *k).collect();

    for (key, refs) in CURATED_SKILLS {
        if result.frameworks.contains(&key.to_string()) || result.tools.contains(&key.to_string()) {
            all_skill_refs.extend(refs.iter().map(|s| (*s).to_string()));
        }
    }

    let is_web_app = result.frameworks.iter().any(|fw| WEB_FRAMEWORKS.contains(&fw.as_str()));
    if is_web_app {
        all_skill_refs.push("vercel-labs/web-design-guidelines@web-design-guidelines".to_string());
    }

    if !json && !all_skill_refs.is_empty() {
        let _ = ui::layout::blank_line();
        let _ = ui::status::info("Curated skills:");
        for r in &all_skill_refs {
            let _ = ui::layout::indented(r, 1);
        }
    }

    let mut search_terms: Vec<String> = result
        .search_terms
        .iter()
        .filter(|t| !curated_keys.contains(t.as_str()))
        .cloned()
        .collect();

    if !result.frameworks.is_empty() {
        search_terms.retain(|t| t != "javascript" && t != "typescript");
    } else if search_terms.contains(&"typescript".to_string()) {
        search_terms.retain(|t| t != "javascript");
    }

    if !json && !search_terms.is_empty() {
        let _ = ui::layout::blank_line();
        let _ = ui::status::info("Searching for skills (top result per term)...");
    }

    for term in &search_terms {
        if !json {
            use std::io::Write;
            let _ = std::io::stdout().write_fmt(format_args!("  {}...", term));
            let _ = std::io::stdout().flush();
        }
        let api_results = find::search_skills_api(term, 1).await;
        let top = api_results.first().filter(|(name, source, _)| {
            let ref_str = format!("{}@{}", source, name);
            is_relevant_skill(&ref_str, term, &result.frameworks)
        });
        if let Some((name, source, _)) = top {
            let ref_str = format!("{}@{}", source, name);
            all_skill_refs.push(ref_str.clone());
            if !json {
                let _ = ui::status::info(&format!(" {}", ref_str));
            }
        } else if !json {
            let _ = ui::status::info(" (none)");
        }
    }

    let unique_refs: Vec<String> = all_skill_refs
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let skills = group_skills_by_source(&unique_refs);

    let skills_json = serde_json::json!({
        "$schema": SKILLS_SCHEMA,
        "detected": detected,
        "skills": skills
    });

    let output_path = output
        .unwrap_or_else(|| cwd.join(SKILLS_JSON_FILE));

    let content = serde_json::to_string_pretty(&skills_json).unwrap_or_default();
    starbase_utils::fs::write_file(&output_path, format!("{}\n", content))
        .map_err(|e| miette::miette!("Failed to write {}: {}", output_path.display(), e))?;

    if json {
        println!("{}", content);
    } else {
        let _ = ui::layout::blank_line();
        let _ = ui::status::info(&format!(
            "Found {} skills from {} sources",
            unique_refs.len(),
            skills.len()
        ));
        let _ = ui::status::info(&format!("Wrote {}", common::user_config::path_for_display(&output_path)));
        let _ = ui::layout::blank_line();
        let _ = ui::layout::indented("Install with: appz skills install", 1);
    }

    Ok(None)
}
