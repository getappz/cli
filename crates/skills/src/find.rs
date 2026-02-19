//! Search for skills. Supports skills.sh API search, interactive select-then-install, and local search.

use crate::add;
use crate::context::SkillsContext;
use std::io::IsTerminal;
use serde::Deserialize;
use starbase::AppResult;
use starbase_utils::fs;

const SEARCH_API_BASE: &str = "https://skills.sh";

#[derive(Deserialize)]
struct SearchApiSkill {
    id: Option<String>,
    name: Option<String>,
    installs: Option<u64>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct SearchApiResponse {
    skills: Option<Vec<SearchApiSkill>>,
}

/// Search skills.sh API.
pub async fn search_skills_api(query: &str, limit: u32) -> Vec<(String, String, u64)> {
    let url = format!(
        "{}/api/search?q={}&limit={}",
        SEARCH_API_BASE,
        urlencoding::encode(query),
        limit
    );
    let client = reqwest::Client::new();
    let Ok(res) = client.get(&url).send().await else {
        return Vec::new();
    };
    if !res.status().is_success() {
        return Vec::new();
    }
    let Ok(data) = res.json::<SearchApiResponse>().await else {
        return Vec::new();
    };
    let Some(skills) = data.skills else {
        return Vec::new();
    };
    skills
        .into_iter()
        .filter_map(|s| {
            let name = s.name.or(s.id)?.to_string();
            let source = s.source.unwrap_or_default();
            let installs = s.installs.unwrap_or(0);
            Some((name, source, installs))
        })
        .collect()
}

fn format_installs(installs: u64) -> String {
    if installs == 0 {
        return String::new();
    }
    if installs >= 1_000_000 {
        return format!("{}M installs", (installs as f64) / 1_000_000.0);
    }
    if installs >= 1_000 {
        return format!("{}K installs", (installs as f64) / 1_000.0);
    }
    format!("{} install{}", installs, if installs == 1 { "" } else { "s" })
}

/// Search for skills. When query is provided: tries skills.sh API first, then local.
/// When no query and TTY: interactive search + select-then-install.
/// When no query and non-TTY: lists installed skills.
pub async fn find(ctx: &SkillsContext, query: Option<String>) -> AppResult {
    if query.is_none() && std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        let search_q = ui::prompt::prompt("Search query (min 2 chars):", None)
            .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
        let search_q = search_q.trim();
        if search_q.len() >= 2 {
            let _search_spinner = if !ctx.verbose {
                Some(ui::progress::spinner("Searching skills.sh..."))
            } else {
                None
            };
            let api_results = search_skills_api(search_q, 10).await;
            if !api_results.is_empty() {
                let options: Vec<String> = api_results
                    .iter()
                    .map(|(name, source, installs)| {
                        let i = format_installs(*installs);
                        if i.is_empty() {
                            format!("{} ({})", source, name)
                        } else {
                            format!("{} ({}) - {}", source, name, i)
                        }
                    })
                    .collect();
                if let Ok(Some(selected)) = ui::interactive::select_interactive("Select a skill to install:", &options) {
                    if let Some(idx) = options.iter().position(|o| o == &selected) {
                        if let Some((name, source, _)) = api_results.get(idx) {
                            let install_source = format!("{}@{}", source, name);
                            let _ = ui::status::info(&format!("Installing {}...", install_source));
                            return add::add(
                                ctx,
                                install_source,
                                false,
                                true,
                                true,
                                Some(name.clone()),
                                false,
                                &[],
                                false,
                                false,
                                None,
                                false,
                                false, // code
                                None,  // workdir
                                None,  // name
                            )
                            .await;
                        }
                    }
                }
            }
        }
    }

    if let Some(ref q) = query {
        let _search_spinner = if !ctx.verbose {
            Some(ui::progress::spinner("Searching skills.sh..."))
        } else {
            None
        };
        let api_results = search_skills_api(q, 10).await;
        if !api_results.is_empty() {
            let _ = ui::layout::blank_line();
            let _ = ui::layout::section_title("Skills from skills.sh");
            let _ = ui::layout::indented("Install with: appz skills add <owner/repo@skill>", 1);
            let _ = ui::layout::blank_line();
            for (name, source, installs) in api_results.iter().take(6) {
                let inst_str = format_installs(*installs);
                let _ = ui::status::info(&format!(
                    "{}@{} {}",
                    source,
                    name,
                    if inst_str.is_empty() {
                        String::new()
                    } else {
                        format!("({})", inst_str)
                    }
                ));
                let _ = ui::layout::indented(&format!("https://skills.sh/{}", name.to_lowercase().replace(' ', "-")), 1);
                let _ = ui::layout::blank_line();
            }
            let _ = ui::layout::indented("Discover more at: https://skills.sh", 1);
            return Ok(None);
        }
    }

    let skills = collect_installed_skills(ctx);

    if skills.is_empty() {
        let _ = ui::status::info("No installed skills to search.");
        let _ = ui::layout::indented(
            "Install skills with: appz skills add <owner/repo> or appz skills add <url>",
            1,
        );
        let _ = ui::layout::indented("Discover skills at: https://skills.sh", 1);
        return Ok(None);
    }

    let filtered: Vec<_> = if let Some(ref q) = query {
        let q_lower = q.to_lowercase();
        skills
            .into_iter()
            .filter(|(name, desc, _)| {
                name.to_lowercase().contains(&q_lower) || desc.to_lowercase().contains(&q_lower)
            })
            .collect()
    } else {
        skills
    };

    if filtered.is_empty() {
        let _ = ui::status::info(&format!("No skills matching '{}' in installed skills.", query.as_deref().unwrap_or("")));
        let _ = ui::layout::indented("Discover more skills at: https://skills.sh", 1);
        return Ok(None);
    }

    let _ = ui::layout::blank_line();
    let _ = ui::layout::section_title(if query.is_some() {
        "Matching skills"
    } else {
        "Installed skills"
    });
    let _ = ui::layout::blank_line();

    for (name, description, path) in &filtered {
        let _ = ui::status::info(name);
        let _ = ui::layout::indented(description, 1);
        let _ = ui::layout::indented(&format!("path: {}", common::user_config::path_for_display(path)), 1);
        let _ = ui::layout::indented(&format!("install: appz skills add <source> -s {}", name), 1);
        let _ = ui::layout::blank_line();
    }

    let _ = ui::layout::indented("Discover more skills at: https://skills.sh", 1);

    Ok(None)
}

#[derive(Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

fn collect_installed_skills(ctx: &SkillsContext) -> Vec<(String, String, std::path::PathBuf)> {
    let mut skills = Vec::new();

    let project_dir = ctx.working_dir.join(".agents").join("skills");
    if project_dir.exists() {
        collect_from_dir(&project_dir, &mut skills);
    }
    if let Some(ref appz_dir) = ctx.user_appz_dir {
        let user_dir = appz_dir.join("skills");
        if user_dir.exists() {
            collect_from_dir(&user_dir, &mut skills);
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for (name, desc, path) in skills {
        let key = name.to_lowercase();
        if !seen.contains(&key) {
            seen.insert(key);
            result.push((name, desc, path));
        }
    }
    result
}

fn collect_from_dir(
    dir: &std::path::Path,
    out: &mut Vec<(String, String, std::path::PathBuf)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Ok(content) = fs::read_file(&skill_file) {
                    if let Ok((name, desc)) = parse_frontmatter(&content) {
                        out.push((name, desc, path));
                    }
                }
            } else {
                collect_from_dir(&path, out);
            }
        }
    }
}

fn parse_frontmatter(content: &str) -> Result<(String, String), miette::Report> {
    let content = content.trim_start();
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| miette::miette!("No YAML frontmatter"))?;
    let rest = rest.trim_start_matches(['\n', '\r']);
    let end = rest
        .find("\n---")
        .or_else(|| rest.find("\r\n---"))
        .ok_or_else(|| miette::miette!("No closing ---"))?;
    let yaml = rest[..end].trim();
    let parsed: SkillFrontmatter = serde_yaml::from_str(yaml)
        .map_err(|e| miette::miette!("Invalid frontmatter: {}", e))?;
    Ok((parsed.name, parsed.description))
}
