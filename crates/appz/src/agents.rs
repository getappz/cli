//! `appz agents {list,doctor}` — detects which AI CLI agents are installed
//! on this system and diagnoses their config. Skill install/uninstall lives
//! in `appz skills` (see skills.rs), not here — this module doesn't touch
//! the skill registry at all.

use std::path::{Path, PathBuf};

use serde::Serialize;
use skill::manager::SkillManager;

pub struct AgentCommand {
    pub kind: AgentSubcommand,
    pub dir: PathBuf,
    pub json: bool,
}

pub enum AgentSubcommand {
    List,
    Doctor,
}

fn manager(root: &Path) -> SkillManager {
    SkillManager::builder().cwd(root.to_path_buf()).build()
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

/// Runs `f` bare in JSON mode (no interleaved spinner text), or wrapped in a
/// spinner for human output.
fn run_task<T>(json: bool, start: &str, done: &str, f: impl FnOnce() -> T) -> T {
    if json {
        f()
    } else {
        super::with_spinner(start, done, f)
    }
}

pub fn run(cmd: &AgentCommand) {
    let root = super::resolve_root(&cmd.dir);
    match cmd.kind {
        AgentSubcommand::List => run_list(&root, cmd.json),
        AgentSubcommand::Doctor => run_doctor(&root, cmd.json),
    }
}

#[derive(Serialize)]
struct AgentEntry {
    id: String,
    display_name: String,
    skills_dir: String,
    detected: bool,
}

#[derive(Serialize)]
struct ListReport {
    installed: usize,
    known: usize,
    agents: Vec<AgentEntry>,
}

fn run_list(root: &Path, json: bool) {
    let mgr = manager(root);

    if !json {
        super::intro("appz agents list");
    }
    let installed = run_task(json, "detecting agents...", "detection complete", || {
        block_on(mgr.detect_installed_agents())
    });
    let registry = mgr.agents();
    let all_ids = registry.all_ids();

    if json {
        let agents = all_ids
            .iter()
            .filter_map(|id| {
                registry.get(id).map(|c| AgentEntry {
                    id: id.as_str().to_string(),
                    display_name: c.display_name.clone(),
                    skills_dir: c.skills_dir.clone(),
                    detected: installed.contains(id),
                })
            })
            .collect();
        let report = ListReport {
            installed: installed.len(),
            known: registry.len(),
            agents,
        };
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    if installed.is_empty() {
        super::info("no agents detected on this system");
    } else {
        super::step("installed agents:");
        for id in &installed {
            let display = registry
                .get(id)
                .map(|c| c.display_name.as_str())
                .unwrap_or(id.as_str());
            super::info(&format!("  {display} ({id})"));
        }
    }

    super::step("known agents:");
    for id in all_ids {
        let mark = if installed.contains(&id) {
            " (detected)"
        } else {
            ""
        };
        if let Some(config) = registry.get(&id) {
            super::info(&format!(
                "  {} ({}) — {} skills dir{mark}",
                config.display_name, id, config.skills_dir
            ));
        }
    }

    super::outro(&format!(
        "{} installed, {} known",
        installed.len(),
        registry.len()
    ));
}

#[derive(Serialize)]
struct DoctorAgentEntry {
    id: String,
    display_name: String,
    detected: bool,
    skills_installed: usize,
}

#[derive(Serialize)]
struct DoctorReport {
    agents: Vec<DoctorAgentEntry>,
    has_agents_md: bool,
    has_appz_jsonc: bool,
    issues: Vec<String>,
}

fn run_doctor(root: &Path, json: bool) {
    let mgr = manager(root);
    let registry = mgr.agents();
    if !json {
        super::intro("appz agents doctor");
    }
    let installed = run_task(json, "probing agents...", "probing done", || {
        block_on(mgr.detect_installed_agents())
    });

    let mut issues: Vec<String> = Vec::new();
    let mut entries: Vec<DoctorAgentEntry> = Vec::new();
    for id in registry.all_ids() {
        let config = registry.get(&id).unwrap();
        let detected = installed.contains(&id);
        if !json {
            let marker = if detected { "✓" } else { "✗" };
            super::step(&format!("{marker} {} ({})", config.display_name, id));
        }

        for path in &config.detect_paths {
            let exists = path.exists();
            if detected && !exists {
                issues.push(format!(
                    "{}: detection path missing: {}",
                    id,
                    path.display()
                ));
            }
        }

        let agent_skills_dir = root.join(&config.skills_dir);
        let skills_installed = if agent_skills_dir.exists() {
            let count = block_on(count_skills(&agent_skills_dir));
            if !json {
                super::info(&format!(
                    "  skills: {} installed at {}",
                    count,
                    agent_skills_dir.display()
                ));
            }
            count
        } else {
            if detected {
                issues.push(format!(
                    "{}: agent detected but skills dir missing: {}",
                    id,
                    agent_skills_dir.display()
                ));
            }
            0
        };

        entries.push(DoctorAgentEntry {
            id: id.as_str().to_string(),
            display_name: config.display_name.clone(),
            detected,
            skills_installed,
        });
    }

    let has_agents_md = root.join("AGENTS.md").exists();
    let has_appz_jsonc = root.join("appz.jsonc").exists();

    if json {
        let report = DoctorReport {
            agents: entries,
            has_agents_md,
            has_appz_jsonc,
            issues,
        };
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    super::step(&format!(
        "{} AGENTS.md",
        if has_agents_md { "✓" } else { "✗" }
    ));
    super::step(&format!(
        "{} appz.jsonc",
        if has_appz_jsonc { "✓" } else { "✗" }
    ));

    if issues.is_empty() {
        super::outro("all good");
    } else {
        for issue in &issues {
            super::warning(issue);
        }
        super::outro(&format!("{} issue(s) found", issues.len()));
    }
}

async fn count_skills(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(_)) = rd.next_entry().await {
            count += 1;
        }
    }
    count
}
