//! `appz agents {list,doctor,install,uninstall,update,refresh}` —
//! registry-driven AI CLI agent manager backed by the `skill` crate.

use std::path::{Path, PathBuf};

use serde::Serialize;
use skill::manager::SkillManager;
use skill::types::{
    AgentId, InstallMode, InstallOptions, InstallScope, ListOptions, RemoveOptions,
};

pub struct AgentCommand {
    pub kind: AgentSubcommand,
    pub dir: PathBuf,
    pub agent: Option<String>,
    pub skill_name: Option<String>,
    pub yes: bool,
    pub json: bool,
}

pub enum AgentSubcommand {
    List,
    Doctor,
    Install,
    Uninstall,
    Update,
    Refresh,
}

fn manager(root: &Path) -> SkillManager {
    SkillManager::builder().cwd(root.to_path_buf()).build()
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

pub fn run(cmd: &AgentCommand) {
    let root = super::resolve_root(&cmd.dir);
    match cmd.kind {
        AgentSubcommand::List => run_list(&root, cmd.json),
        AgentSubcommand::Doctor => run_doctor(&root, cmd.json),
        AgentSubcommand::Install => run_install(&root, cmd),
        AgentSubcommand::Uninstall => run_uninstall(&root, cmd),
        AgentSubcommand::Update => refresh_impl(&root, cmd.json, "update"),
        AgentSubcommand::Refresh => run_refresh(&root, cmd.json),
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

    let installed = if json {
        block_on(mgr.detect_installed_agents())
    } else {
        super::intro("appz agents list");
        super::with_spinner("detecting agents...", "detection complete", || {
            block_on(mgr.detect_installed_agents())
        })
    };
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
    let installed = if json {
        block_on(mgr.detect_installed_agents())
    } else {
        super::intro("appz agents doctor");
        super::with_spinner("probing agents...", "probing done", || {
            block_on(mgr.detect_installed_agents())
        })
    };

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

#[derive(Serialize)]
struct InstallReport {
    agent: String,
    skill: String,
    path: String,
}

fn run_install(root: &Path, cmd: &AgentCommand) {
    let json = cmd.json;
    if !json {
        super::intro("appz agents install");
    }
    let mgr = manager(root);

    let agent_id = match &cmd.agent {
        Some(a) => AgentId::new(a.clone()),
        None => {
            let installed = if json {
                block_on(mgr.detect_installed_agents())
            } else {
                super::with_spinner("detecting agents...", "", || {
                    block_on(mgr.detect_installed_agents())
                })
            };
            if installed.is_empty() {
                super::error("no agents detected — use --agent <name> to target a specific agent");
            }
            if !json {
                super::info(&format!(
                    "installing for {} detected agent(s)",
                    installed.len()
                ));
            }
            if installed.len() == 1 {
                installed[0].clone()
            } else if json {
                super::error("multiple agents detected — specify --agent");
            } else {
                super::step("use --agent <name> to pick one");
                super::outro("cancelled");
                return;
            }
        }
    };

    let display = mgr
        .agents()
        .get(&agent_id)
        .map(|c| c.display_name.clone())
        .unwrap_or_else(|| agent_id.as_str().to_string());
    if !json {
        super::step(&format!("target agent: {display} ({agent_id})"));
    }

    let skill_name = cmd.skill_name.as_deref().unwrap_or("");
    if skill_name.is_empty() {
        super::error("specify a skill name to install");
    }

    let skills = if json {
        block_on(mgr.discover_skills(
            root,
            &skill::types::DiscoverOptions {
                ..Default::default()
            },
        ))
    } else {
        super::with_spinner(&format!("discovering skill '{skill_name}'..."), "", || {
            block_on(mgr.discover_skills(
                root,
                &skill::types::DiscoverOptions {
                    ..Default::default()
                },
            ))
        })
    };

    let skills = match skills {
        Ok(s) => s,
        Err(e) => super::error(&format!("discovery failed: {e}")),
    };
    let mut matches: Vec<_> = skills
        .into_iter()
        .filter(|s| s.name.contains(skill_name) || s.name == skill_name)
        .collect();

    if matches.is_empty() {
        super::error(&format!("no skill found matching '{skill_name}'"));
    }

    if matches.len() > 1 && !cmd.yes {
        if json {
            super::error(&format!(
                "{} skills match '{skill_name}' — specify a more specific name or pass --yes",
                matches.len()
            ));
        }
        super::info(&format!("{} skills match '{skill_name}'", matches.len()));
        super::step("use a more specific name or pass --yes to install the first match");
        super::outro("cancelled");
        return;
    }

    let skill = matches.swap_remove(0);

    let result = if json {
        block_on(mgr.install_skill(
            &skill,
            &agent_id,
            &InstallOptions {
                scope: InstallScope::Project,
                mode: InstallMode::Symlink,
                cwd: None,
            },
        ))
    } else {
        super::with_spinner(
            &format!("installing '{}' for {display}...", skill.name),
            "installation complete",
            || {
                block_on(mgr.install_skill(
                    &skill,
                    &agent_id,
                    &InstallOptions {
                        scope: InstallScope::Project,
                        mode: InstallMode::Symlink,
                        cwd: None,
                    },
                ))
            },
        )
    };

    match result {
        Ok(r) => {
            if json {
                let report = InstallReport {
                    agent: agent_id.as_str().to_string(),
                    skill: skill.name.clone(),
                    path: r.path.display().to_string(),
                };
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                super::success(&format!("installed → {}", r.path.display()));
                super::outro("done — review before use");
            }
        }
        Err(e) => super::error(&format!("installation failed: {e}")),
    }
}

#[derive(Serialize)]
struct UninstallReport {
    skill: String,
    status: &'static str,
}

fn run_uninstall(root: &Path, cmd: &AgentCommand) {
    let json = cmd.json;
    if !json {
        super::intro("appz agents uninstall");
    }
    let mgr = manager(root);

    let skill_name = cmd.skill_name.as_deref().unwrap_or("");
    if skill_name.is_empty() {
        super::error("specify a skill name to uninstall");
    }

    let remove = || {
        block_on(mgr.remove_skills(
            &[skill_name.to_string()],
            &RemoveOptions {
                scope: InstallScope::Project,
                agents: Vec::new(),
                cwd: None,
            },
        ))
    };
    let result = if json {
        remove()
    } else {
        super::with_spinner(
            &format!("removing '{skill_name}'..."),
            "removal complete",
            remove,
        )
    };
    if let Err(e) = result {
        super::error(&format!("removal failed: {e}"));
    }

    if json {
        let report = UninstallReport {
            skill: skill_name.to_string(),
            status: "removed",
        };
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        super::success(&format!("removed '{skill_name}'"));
        super::outro("uninstall complete");
    }
}

fn run_refresh(root: &Path, json: bool) {
    refresh_impl(root, json, "refresh");
}

#[derive(Serialize)]
struct SkillEntry {
    name: String,
    agents: Vec<String>,
}

#[derive(Serialize)]
struct RefreshReport {
    installed_agents: Vec<String>,
    skills: Vec<SkillEntry>,
}

fn refresh_impl(root: &Path, json: bool, verb: &str) {
    let mgr = manager(root);

    let installed = if json {
        block_on(mgr.detect_installed_agents())
    } else {
        super::intro(&format!("appz agents {verb}"));
        super::with_spinner("re-detecting agents...", "detection complete", || {
            block_on(mgr.detect_installed_agents())
        })
    };

    if !json {
        if installed.is_empty() {
            super::info("no agents detected");
        } else {
            super::step("detected agents:");
            for id in &installed {
                let display = mgr
                    .agents()
                    .get(id)
                    .map(|c| c.display_name.as_str())
                    .unwrap_or(id.as_str());
                super::info(&format!("  {display} ({id})"));
            }
        }
    }

    let skills = if json {
        block_on(mgr.list_installed(&ListOptions::default()))
    } else {
        super::with_spinner("listing installed skills...", "", || {
            block_on(mgr.list_installed(&ListOptions::default()))
        })
    };

    match skills {
        Ok(list) => {
            if json {
                let report = RefreshReport {
                    installed_agents: installed.iter().map(|a| a.as_str().to_string()).collect(),
                    skills: list
                        .iter()
                        .map(|s| SkillEntry {
                            name: s.name.clone(),
                            agents: s.agents.iter().map(|a| a.as_str().to_string()).collect(),
                        })
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                return;
            }
            if list.is_empty() {
                super::info("no skills installed");
            } else {
                super::step("installed skills:");
                for s in &list {
                    let agents: Vec<&str> = s.agents.iter().map(|a| a.as_str()).collect();
                    super::info(&format!("  {} — for {}", s.name, agents.join(", ")));
                }
            }
            super::outro(&format!(
                "{} agent(s), {} skill(s)",
                installed.len(),
                list.len()
            ));
        }
        Err(e) => super::error(&format!("failed to list skills: {e}")),
    }
}
