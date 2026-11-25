//! `appz agents {list,doctor}` — detects which AI CLI agents are installed
//! on this system and diagnoses their config. Skill install/uninstall lives
//! in `appz skills` (see skills.rs), not here — this module doesn't touch
//! the skill registry at all.

use std::path::{Path, PathBuf};

use serde::Serialize;
use skill::manager::SkillManager;
use skill::types::AgentId;

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

/// Detects installed agents: silent in JSON mode, otherwise under a timed
/// fetch line (`> 1 agent detected [12ms]`, stderr).
fn detect(json: bool, mgr: &SkillManager) -> Vec<AgentId> {
    if json {
        return block_on(mgr.detect_installed_agents());
    }
    let fetch = crate::ui::fetching("agents");
    let found = block_on(mgr.detect_installed_agents());
    fetch.done(&format!(
        "{} detected",
        crate::ui::fmt::plural(found.len(), "agent")
    ));
    found
}

/// One `agents list` table row: Agent, ID, Skills dir, Installed.
fn list_row(display: &str, id: &str, skills_dir: &str, installed: bool) -> Vec<String> {
    let mark = if installed { "✓" } else { "" };
    vec![display.into(), id.into(), skills_dir.into(), mark.into()]
}

/// One `agents doctor` table row: Agent, ID, Detected, Skills.
fn doctor_row(e: &DoctorAgentEntry) -> Vec<String> {
    vec![
        e.display_name.clone(),
        e.id.clone(),
        if e.detected { "✓" } else { "" }.into(),
        e.skills_installed.to_string(),
    ]
}

/// Project-file presence for the doctor key/value block.
fn presence(found: bool) -> String {
    if found { "Found" } else { "Missing" }.into()
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

    let installed = detect(json, &mgr);
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

    let mut table = crate::ui::Table::new(&["Agent", "ID", "Skills dir", "Installed"]);
    for id in &all_ids {
        if let Some(c) = registry.get(id) {
            table.row(list_row(
                &c.display_name,
                id.as_str(),
                &c.skills_dir,
                installed.contains(id),
            ));
        }
    }
    table.print();
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
    let installed = detect(json, &mgr);

    let mut issues: Vec<String> = Vec::new();
    let mut entries: Vec<DoctorAgentEntry> = Vec::new();
    for id in registry.all_ids() {
        let config = registry.get(&id).unwrap();
        let detected = installed.contains(&id);
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
            block_on(count_skills(&agent_skills_dir))
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

    let mut table = crate::ui::Table::new(&["Agent", "ID", "Detected", "Skills"]);
    for e in &entries {
        table.row(doctor_row(e));
    }
    table.print();

    crate::ui::heading("Project files");
    crate::ui::print_kv(&[
        ("AGENTS.md", presence(has_agents_md)),
        ("appz.jsonc", presence(has_appz_jsonc)),
    ]);

    if issues.is_empty() {
        crate::success("No issues found");
    } else {
        for issue in &issues {
            crate::warning(issue);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_row_marks_installed() {
        assert_eq!(
            list_row("Claude Code", "claude-code", ".claude/skills", true),
            vec!["Claude Code", "claude-code", ".claude/skills", "✓"]
        );
        assert_eq!(list_row("Amp", "amp", ".agents/skills", false)[3], "");
    }

    #[test]
    fn doctor_row_and_presence() {
        let e = DoctorAgentEntry {
            id: "amp".into(),
            display_name: "Amp".into(),
            detected: true,
            skills_installed: 3,
        };
        assert_eq!(doctor_row(&e), vec!["Amp", "amp", "✓", "3"]);
        assert_eq!(presence(true), "Found");
        assert_eq!(presence(false), "Missing");
    }
}
