//! `appz skills [query]` — discover agent skills relevant to a project and
//! install them, porting `vercel skills` (https://vercel.com/docs/cli/skills).
//!
//! Search against the skills.sh registry is done the same way `vercel
//! skills` and `skills-cli` (https://github.com/qntx/skill) do it — a plain
//! GET against `skills.sh/api/search`. Installation, however, is native:
//! it drives the published `skill` crate directly instead of shelling out
//! to `npx skills add`, so this command has no Node.js dependency.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use skill::SkillManager;
use skill::types::{
    AgentId, DiscoverOptions, InstallMode, InstallOptions, InstallScope, ParsedSource, SourceType,
};

const SKILLS_API: &str = "https://skills.sh/api/search";
const MIN_INSTALLS: u64 = 100;
const MAX_RESULTS: usize = 8;
const MAX_FRAMEWORK_RESULTS: usize = 4;
/// Skip sources whose repo has more files than this. The registry only gives
/// us `owner/repo`, not the skill's actual subdirectory (see the note on
/// `install_from_source`), so installing from a source means downloading it
/// file-by-file in full — fine for a typical skills repo, painfully slow
/// (minutes) for a large monorepo like `wshobson/agents` (~2000 files).
const MAX_SOURCE_FILES: usize = 300;

#[derive(Deserialize, Debug, Clone)]
struct ApiSkill {
    #[serde(rename = "skillId")]
    skill_id: String,
    name: String,
    #[serde(default)]
    installs: u64,
    #[serde(default)]
    source: String,
}

#[derive(Deserialize, Default)]
struct ApiSearchResponse {
    #[serde(default)]
    skills: Vec<ApiSkill>,
}

#[derive(Serialize, Clone)]
pub struct SkillHit {
    pub skill_id: String,
    pub name: String,
    pub installs: u64,
    pub source: String,
    pub installed: bool,
    /// Verified subdirectory within `source` (e.g. `skills/frameworks/foo`),
    /// found by scanning the repo's file tree for a `SKILL.md` under a
    /// directory named `foo` — set only for sources too large to install in
    /// full (see `MAX_SOURCE_FILES`). `None` means "clone/blob-install the
    /// whole repo", which is what small sources do anyway.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_subpath: Option<String>,
}

pub struct SkillsRequest {
    pub query: Option<String>,
    /// Skip the large-source-repo filter and offer everything the registry
    /// returned, even sources that would be slow to install from.
    pub include_large: bool,
}

#[derive(Serialize)]
pub struct SkillsReport {
    pub context: String,
    pub hits: Vec<SkillHit>,
    /// `owner/repo` sources dropped for having more than `MAX_SOURCE_FILES`
    /// files (empty when `include_large` was set).
    pub excluded_large_sources: Vec<String>,
}

pub struct InstallSummary {
    pub installed: Vec<String>,
    pub failed: Vec<String>,
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn search_api(query: &str) -> Vec<ApiSkill> {
    if query.trim().chars().count() < 2 {
        return Vec::new();
    }
    let url = format!("{SKILLS_API}?q={}&limit=10", percent_encode(query));
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(10))
        .build();
    agent
        .get(&url)
        .set("User-Agent", "appz")
        .call()
        .ok()
        .and_then(|resp| resp.into_json::<ApiSearchResponse>().ok())
        .map(|r| r.skills)
        .unwrap_or_default()
}

/// `owner/repo`, no more or less — the shape the registry's `source` field
/// takes for GitHub sources (the only ones we can size-check this way).
fn is_owner_repo(s: &str) -> bool {
    let mut parts = s.splitn(3, '/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(o), Some(r), None) if !o.is_empty() && !r.is_empty()
    )
}

#[derive(Deserialize, Clone)]
struct TreeEntry {
    path: String,
}

#[derive(Deserialize, Default)]
struct TreeResponse {
    #[serde(default)]
    tree: Vec<TreeEntry>,
    #[serde(default)]
    truncated: bool,
}

/// Fetch `owner/repo`'s full default-branch file tree. `None` on any
/// network or parse error.
fn fetch_tree(owner_repo: &str) -> Option<TreeResponse> {
    let url = format!("https://api.github.com/repos/{owner_repo}/git/trees/HEAD?recursive=1");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build();
    let mut req = agent
        .get(&url)
        .set("User-Agent", "appz")
        .set("Accept", "application/vnd.github+json");
    if let Some(token) = skill::github::discover_token() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    req.call().ok()?.into_json::<TreeResponse>().ok()
}

/// Find the one directory in `tree` containing a `SKILL.md` whose own name
/// matches `skill_name` — e.g. `skills/frameworks/clerk-nextjs-patterns/SKILL.md`
/// resolves `clerk-nextjs-patterns` to `Some("skills/frameworks/clerk-nextjs-patterns")`.
/// `None` when there's no match or more than one (ambiguous) — this is a
/// verified lookup against the repo's real tree, never a guess.
fn resolve_skill_subpath(tree: &TreeResponse, skill_name: &str) -> Option<String> {
    let mut matches = tree.tree.iter().filter_map(|e| {
        let dir = e.path.strip_suffix("/SKILL.md")?;
        let leaf = dir.rsplit('/').next().unwrap_or(dir);
        (!dir.is_empty() && leaf.eq_ignore_ascii_case(skill_name)).then(|| dir.to_string())
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// For sources whose tree exceeds `MAX_SOURCE_FILES` (fetched once per
/// unique source, cached), try to resolve each of their skills to a
/// verified subpath instead of downloading the whole repo. Skills that
/// can't be confidently resolved are dropped and reported in the second
/// return value; everything else (small sources, non-GitHub sources) is
/// kept unscoped, same as before.
fn resolve_and_filter_sources(
    skills: Vec<ApiSkill>,
) -> (Vec<(ApiSkill, Option<String>)>, Vec<String>) {
    let mut tree_cache: HashMap<String, Option<TreeResponse>> = HashMap::new();
    let mut excluded = Vec::new();
    let mut kept = Vec::new();

    for skill in skills {
        if !is_owner_repo(&skill.source) {
            kept.push((skill, None));
            continue;
        }

        let tree = tree_cache
            .entry(skill.source.clone())
            .or_insert_with(|| fetch_tree(&skill.source));

        let Some(tree) = tree else {
            // Couldn't check size at all — fail open, install whole repo.
            kept.push((skill, None));
            continue;
        };

        if !tree.truncated && tree.tree.len() <= MAX_SOURCE_FILES {
            kept.push((skill, None));
            continue;
        }

        match resolve_skill_subpath(tree, &skill.name) {
            Some(subpath) => kept.push((skill, Some(subpath))),
            None => {
                if !excluded.contains(&skill.source) {
                    excluded.push(skill.source.clone());
                }
            }
        }
    }

    (kept, excluded)
}

fn dedup(terms: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    terms
        .into_iter()
        .filter(|t| seen.insert(t.clone()))
        .collect()
}

fn merge_best(map: &mut HashMap<String, ApiSkill>, results: Vec<ApiSkill>) {
    for s in results {
        map.entry(s.skill_id.clone())
            .and_modify(|existing| {
                if s.installs > existing.installs {
                    *existing = s.clone();
                }
            })
            .or_insert(s);
    }
}

pub fn format_installs(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// Search skills.sh, either directly (query given) or by auto-detecting the
/// project's toolchains/frameworks via `appz-core`, then mark which of the
/// results are already installed for this project.
pub fn search(root: &Path, req: &SkillsRequest) -> Result<SkillsReport, String> {
    let (context, ranked) = match req
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        Some(q) => {
            let mut results: Vec<ApiSkill> = search_api(q)
                .into_iter()
                .filter(|s| s.installs >= MIN_INSTALLS)
                .collect();
            results.sort_by_key(|s| std::cmp::Reverse(s.installs));
            results.truncate(MAX_RESULTS);
            (format!("Search: \"{q}\""), results)
        }
        None => {
            let toolchains = appz_core::detect_toolchains(root)?;
            if toolchains.is_empty() {
                return Ok(SkillsReport {
                    context: "no toolchains detected in this directory".to_string(),
                    hits: Vec::new(),
                    excluded_large_sources: Vec::new(),
                });
            }

            let mut framework_terms = Vec::new();
            let mut toolchain_terms = Vec::new();
            let mut detected_parts = Vec::new();
            for tc in &toolchains {
                toolchain_terms.push(tc.name.to_lowercase());
                detected_parts.push(tc.name.to_string());
                for fw in &tc.frameworks {
                    framework_terms.push(fw.name.to_lowercase());
                    detected_parts.push(fw.name.to_string());
                }
            }

            let mut framework_hits: HashMap<String, ApiSkill> = HashMap::new();
            for term in dedup(framework_terms) {
                merge_best(&mut framework_hits, search_api(&term));
            }
            let mut dep_hits: HashMap<String, ApiSkill> = HashMap::new();
            for term in dedup(toolchain_terms) {
                merge_best(&mut dep_hits, search_api(&term));
            }

            let mut top_framework: Vec<ApiSkill> = framework_hits
                .into_values()
                .filter(|s| s.installs >= MIN_INSTALLS)
                .collect();
            top_framework.sort_by_key(|s| std::cmp::Reverse(s.installs));
            top_framework.truncate(MAX_FRAMEWORK_RESULTS);
            let framework_ids: HashSet<String> =
                top_framework.iter().map(|s| s.skill_id.clone()).collect();

            let mut top_dep: Vec<ApiSkill> = dep_hits
                .into_values()
                .filter(|s| s.installs >= MIN_INSTALLS && !framework_ids.contains(&s.skill_id))
                .collect();
            top_dep.sort_by_key(|s| std::cmp::Reverse(s.installs));
            top_dep.truncate(MAX_RESULTS.saturating_sub(top_framework.len()));

            let mut ranked = top_framework;
            ranked.extend(top_dep);
            (format!("Detected: {}", detected_parts.join(" + ")), ranked)
        }
    };

    let (kept, excluded_large_sources) = if req.include_large {
        (ranked.into_iter().map(|s| (s, None)).collect(), Vec::new())
    } else {
        resolve_and_filter_sources(ranked)
    };

    Ok(SkillsReport {
        context,
        hits: mark_installed(root, kept),
        excluded_large_sources,
    })
}

fn mark_installed(root: &Path, skills: Vec<(ApiSkill, Option<String>)>) -> Vec<SkillHit> {
    if skills.is_empty() {
        return Vec::new();
    }

    let installed_names: HashSet<String> = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()
        .map(|rt| {
            rt.block_on(async {
                let manager = SkillManager::builder().cwd(root.to_path_buf()).build();
                manager
                    .list_installed(&skill::types::ListOptions::default())
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| s.name)
                    .collect()
            })
        })
        .unwrap_or_default();

    skills
        .into_iter()
        .map(|(s, resolved_subpath)| SkillHit {
            installed: installed_names.contains(&s.name),
            skill_id: s.skill_id,
            name: s.name,
            installs: s.installs,
            source: s.source,
            resolved_subpath,
        })
        .collect()
}

/// Install the given skills natively, grouped by their `owner/repo` source so
/// each repository is only cloned/downloaded once, with independent sources
/// installed concurrently.
pub fn install(root: &Path, hits: &[SkillHit]) -> Result<InstallSummary, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(install_async(root, hits))
}

async fn install_async(root: &Path, hits: &[SkillHit]) -> Result<InstallSummary, String> {
    let manager = std::sync::Arc::new(SkillManager::builder().cwd(root.to_path_buf()).build());

    // Group by effective source: plain `owner/repo` for most hits (cloned/
    // blob-installed once in full, then filtered by discovery), or
    // `owner/repo/<verified subpath>` for hits from a large source where
    // `resolve_skill_subpath` found the skill's real directory — that
    // narrows the blob-install to just that folder instead of the whole
    // repo. Never a guessed path (see `resolve_and_filter_sources`).
    let mut by_source: HashMap<String, Vec<String>> = HashMap::new();
    for hit in hits {
        let key = match &hit.resolved_subpath {
            Some(subpath) => format!("{}/{subpath}", hit.source),
            None => hit.source.clone(),
        };
        by_source.entry(key).or_default().push(hit.name.clone());
    }

    let mut target_agents = manager.detect_installed_agents().await;
    if target_agents.is_empty() {
        target_agents = manager.agents().universal_agents();
    }
    let target_agents = std::sync::Arc::new(target_agents);

    // Each source is an independent clone/blob-download + install, so run
    // them concurrently — sequentially they add up fast (a handful of
    // sources easily takes minutes one at a time over real network).
    let mut set = tokio::task::JoinSet::new();
    // Tracked outside the spawned task so a panicking task's skills are
    // still attributable to `failed` below — `names` itself is moved into
    // the task and unrecoverable from a `JoinError`.
    let mut pending: HashMap<tokio::task::Id, Vec<String>> = HashMap::new();
    for (source, names) in by_source {
        let manager = std::sync::Arc::clone(&manager);
        let target_agents = std::sync::Arc::clone(&target_agents);
        let root = root.to_path_buf();
        let owned = names.clone();
        let handle = set.spawn(async move {
            let result =
                install_from_source(&manager, &source, &names, &target_agents, &root).await;
            (names, result)
        });
        pending.insert(handle.id(), owned);
    }

    let mut installed = Vec::new();
    let mut failed = Vec::new();
    while let Some(joined) = set.join_next_with_id().await {
        let (names, result) = match joined {
            Ok((id, (names, result))) => {
                pending.remove(&id);
                (names, result)
            }
            // A panicking install task is counted as a failure for its
            // skills rather than aborting the whole batch: best-effort,
            // matching how `detect_installed_agents` treats per-agent panics.
            Err(e) => {
                if let Some(names) = pending.remove(&e.id()) {
                    failed.extend(names);
                }
                continue;
            }
        };
        match result {
            Ok((ok_names, causes)) => {
                let ok: HashSet<&str> = ok_names.iter().map(String::as_str).collect();
                for name in &names {
                    if ok.contains(name.as_str()) {
                        installed.push(name.clone());
                    } else if let Some(cause) = causes.get(name) {
                        failed.push(format!("{name} ({cause})"));
                    } else {
                        failed.push(name.clone());
                    }
                }
            }
            Err(_) => failed.extend(names),
        }
    }

    Ok(InstallSummary { installed, failed })
}

/// Installs every skill in `names` found at `source`, for every target agent.
/// Returns the names that installed on at least one agent, plus — for every
/// name that failed on *all* agents — the last agent's error, so a total
/// failure surfaces an actionable cause instead of a bare name.
async fn install_from_source(
    manager: &SkillManager,
    source: &str,
    names: &[String],
    target_agents: &[AgentId],
    root: &Path,
) -> Result<(Vec<String>, HashMap<String, String>), String> {
    let parsed = manager.parse_source(source);
    let (dir, _temp) = resolve_source(&parsed).await?;

    let discovered = skill::skills::discover_skills(
        &dir,
        parsed.subpath.as_deref(),
        &DiscoverOptions::default(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let selected: Vec<_> = discovered
        .into_iter()
        .filter(|s| names.contains(&s.name))
        .collect();
    if selected.is_empty() {
        return Err(format!("no matching skills found in {source}"));
    }

    let install_opts = InstallOptions {
        scope: InstallScope::Project,
        mode: InstallMode::Symlink,
        cwd: Some(root.to_path_buf()),
    };

    let mut ok_names = Vec::new();
    let mut causes = HashMap::new();
    for skill_item in &selected {
        let mut any_ok = false;
        let mut last_err = None;
        for agent_id in target_agents {
            match manager
                .install_skill(skill_item, agent_id, &install_opts)
                .await
            {
                Ok(_) => any_ok = true,
                Err(e) => last_err = Some(e.to_string()),
            }
        }
        if any_ok {
            ok_names.push(skill_item.name.clone());
        } else if let Some(err) = last_err {
            causes.insert(skill_item.name.clone(), err);
        }
    }
    Ok((ok_names, causes))
}

/// Resolve a parsed source to a local directory: GitHub blob fast-path (only
/// downloads the target skill folder), falling back to a shallow git clone.
/// Mirrors `skills add`'s own resolver (`skills-cli/src/commands/add/install.rs`),
/// built only from the `skill` crate's public API.
async fn resolve_source(
    parsed: &ParsedSource,
) -> Result<(PathBuf, Option<tempfile::TempDir>), String> {
    if parsed.source_type == SourceType::Local {
        let path = parsed.local_path.clone().ok_or("local path not resolved")?;
        return Ok((path, None));
    }

    if parsed.source_type == SourceType::Github
        && let Some(owner_repo) = skill::source::owner_repo(parsed)
    {
        let token = skill::github::discover_token();
        if let Ok(Some(td)) = skill::blob::try_blob_install(
            &owner_repo,
            parsed.subpath.as_deref(),
            parsed.git_ref.as_deref(),
            token.as_deref(),
        )
        .await
        {
            let path = td.path().to_path_buf();
            return Ok((path, Some(td)));
        }
    }

    let td = skill::git::clone_repo(&parsed.url, parsed.git_ref.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    let path = td.path().to_path_buf();
    Ok((path, Some(td)))
}
