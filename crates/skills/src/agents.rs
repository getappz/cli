//! Agent definitions: project path, global path, and detection.
//!
//! Matches the skills.sh/Codex Supported Agents table.

use std::path::{Path, PathBuf};

/// Agent identifier (--agent value).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum AgentType {
    AdaL,
    Amp,
    Antigravity,
    Augment,
    ClaudeCode,
    Cline,
    CodeBuddy,
    Codex,
    CommandCode,
    Continue,
    Crush,
    Cursor,
    Droid,
    GeminiCli,
    GitHubCopilot,
    Goose,
    IFlowCli,
    Junie,
    Kilo,
    KimiCli,
    KiroCli,
    Kode,
    Mcpjam,
    MistralVibe,
    Mux,
    Neovate,
    OpenClaw,
    OpenCode,
    OpenHands,
    Pi,
    Pochi,
    Qoder,
    QwenCode,
    Replit,
    Roo,
    Trae,
    TraeCn,
    Windsurf,
    Zencoder,
}

/// Agent configuration: paths and display name.
#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub id: &'static str,
    pub display_name: &'static str,
    pub project_path: &'static str,
    pub global_path: &'static str,
}

fn home_dir() -> PathBuf {
    starbase_utils::dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn config_home() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".config"))
}

fn codex_home() -> PathBuf {
    std::env::var("CODEX_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".codex"))
}

fn claude_home() -> PathBuf {
    std::env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".claude"))
}

fn openclaw_global() -> PathBuf {
    let home = home_dir();
    if home.join(".openclaw").exists() {
        home.join(".openclaw/skills")
    } else if home.join(".clawdbot").exists() {
        home.join(".clawdbot/skills")
    } else if home.join(".moltbot").exists() {
        home.join(".moltbot/skills")
    } else {
        home.join(".openclaw/skills")
    }
}

impl AgentType {
    pub fn config(&self) -> AgentConfig {
        match self {
            AgentType::AdaL => AgentConfig {
                id: "adal",
                display_name: "AdaL",
                project_path: ".adal/skills",
                global_path: "~/.adal/skills",
            },
            AgentType::Amp => AgentConfig {
                id: "amp",
                display_name: "Amp",
                project_path: ".agents/skills",
                global_path: "~/.config/agents/skills",
            },
            AgentType::Antigravity => AgentConfig {
                id: "antigravity",
                display_name: "Antigravity",
                project_path: ".agent/skills",
                global_path: "~/.gemini/antigravity/skills",
            },
            AgentType::Augment => AgentConfig {
                id: "augment",
                display_name: "Augment",
                project_path: ".augment/skills",
                global_path: "~/.augment/skills",
            },
            AgentType::ClaudeCode => AgentConfig {
                id: "claude-code",
                display_name: "Claude Code",
                project_path: ".claude/skills",
                global_path: "~/.claude/skills",
            },
            AgentType::Cline => AgentConfig {
                id: "cline",
                display_name: "Cline",
                project_path: ".cline/skills",
                global_path: "~/.cline/skills",
            },
            AgentType::CodeBuddy => AgentConfig {
                id: "codebuddy",
                display_name: "CodeBuddy",
                project_path: ".codebuddy/skills",
                global_path: "~/.codebuddy/skills",
            },
            AgentType::Codex => AgentConfig {
                id: "codex",
                display_name: "Codex",
                project_path: ".agents/skills",
                global_path: "~/.codex/skills",
            },
            AgentType::CommandCode => AgentConfig {
                id: "command-code",
                display_name: "Command Code",
                project_path: ".commandcode/skills",
                global_path: "~/.commandcode/skills",
            },
            AgentType::Continue => AgentConfig {
                id: "continue",
                display_name: "Continue",
                project_path: ".continue/skills",
                global_path: "~/.continue/skills",
            },
            AgentType::Crush => AgentConfig {
                id: "crush",
                display_name: "Crush",
                project_path: ".crush/skills",
                global_path: "~/.config/crush/skills",
            },
            AgentType::Cursor => AgentConfig {
                id: "cursor",
                display_name: "Cursor",
                project_path: ".cursor/skills",
                global_path: "~/.cursor/skills",
            },
            AgentType::Droid => AgentConfig {
                id: "droid",
                display_name: "Droid",
                project_path: ".factory/skills",
                global_path: "~/.factory/skills",
            },
            AgentType::GeminiCli => AgentConfig {
                id: "gemini-cli",
                display_name: "Gemini CLI",
                project_path: ".agents/skills",
                global_path: "~/.gemini/skills",
            },
            AgentType::GitHubCopilot => AgentConfig {
                id: "github-copilot",
                display_name: "GitHub Copilot",
                project_path: ".agents/skills",
                global_path: "~/.copilot/skills",
            },
            AgentType::Goose => AgentConfig {
                id: "goose",
                display_name: "Goose",
                project_path: ".goose/skills",
                global_path: "~/.config/goose/skills",
            },
            AgentType::IFlowCli => AgentConfig {
                id: "iflow-cli",
                display_name: "iFlow CLI",
                project_path: ".iflow/skills",
                global_path: "~/.iflow/skills",
            },
            AgentType::Junie => AgentConfig {
                id: "junie",
                display_name: "Junie",
                project_path: ".junie/skills",
                global_path: "~/.junie/skills",
            },
            AgentType::Kilo => AgentConfig {
                id: "kilo",
                display_name: "Kilo Code",
                project_path: ".kilocode/skills",
                global_path: "~/.kilocode/skills",
            },
            AgentType::KimiCli => AgentConfig {
                id: "kimi-cli",
                display_name: "Kimi Code CLI",
                project_path: ".agents/skills",
                global_path: "~/.config/agents/skills",
            },
            AgentType::KiroCli => AgentConfig {
                id: "kiro-cli",
                display_name: "Kiro CLI",
                project_path: ".kiro/skills",
                global_path: "~/.kiro/skills",
            },
            AgentType::Kode => AgentConfig {
                id: "kode",
                display_name: "Kode",
                project_path: ".kode/skills",
                global_path: "~/.kode/skills",
            },
            AgentType::Mcpjam => AgentConfig {
                id: "mcpjam",
                display_name: "MCPJam",
                project_path: ".mcpjam/skills",
                global_path: "~/.mcpjam/skills",
            },
            AgentType::MistralVibe => AgentConfig {
                id: "mistral-vibe",
                display_name: "Mistral Vibe",
                project_path: ".vibe/skills",
                global_path: "~/.vibe/skills",
            },
            AgentType::Mux => AgentConfig {
                id: "mux",
                display_name: "Mux",
                project_path: ".mux/skills",
                global_path: "~/.mux/skills",
            },
            AgentType::Neovate => AgentConfig {
                id: "neovate",
                display_name: "Neovate",
                project_path: ".neovate/skills",
                global_path: "~/.neovate/skills",
            },
            AgentType::OpenClaw => AgentConfig {
                id: "openclaw",
                display_name: "OpenClaw",
                project_path: "skills",
                global_path: "~/.openclaw/skills",
            },
            AgentType::OpenCode => AgentConfig {
                id: "opencode",
                display_name: "OpenCode",
                project_path: ".agents/skills",
                global_path: "~/.config/opencode/skills",
            },
            AgentType::OpenHands => AgentConfig {
                id: "openhands",
                display_name: "OpenHands",
                project_path: ".openhands/skills",
                global_path: "~/.openhands/skills",
            },
            AgentType::Pi => AgentConfig {
                id: "pi",
                display_name: "Pi",
                project_path: ".pi/skills",
                global_path: "~/.pi/agent/skills",
            },
            AgentType::Pochi => AgentConfig {
                id: "pochi",
                display_name: "Pochi",
                project_path: ".pochi/skills",
                global_path: "~/.pochi/skills",
            },
            AgentType::Qoder => AgentConfig {
                id: "qoder",
                display_name: "Qoder",
                project_path: ".qoder/skills",
                global_path: "~/.qoder/skills",
            },
            AgentType::QwenCode => AgentConfig {
                id: "qwen-code",
                display_name: "Qwen Code",
                project_path: ".qwen/skills",
                global_path: "~/.qwen/skills",
            },
            AgentType::Replit => AgentConfig {
                id: "replit",
                display_name: "Replit",
                project_path: ".agents/skills",
                global_path: "~/.config/agents/skills",
            },
            AgentType::Roo => AgentConfig {
                id: "roo",
                display_name: "Roo Code",
                project_path: ".roo/skills",
                global_path: "~/.roo/skills",
            },
            AgentType::Trae => AgentConfig {
                id: "trae",
                display_name: "Trae",
                project_path: ".trae/skills",
                global_path: "~/.trae/skills",
            },
            AgentType::TraeCn => AgentConfig {
                id: "trae-cn",
                display_name: "Trae CN",
                project_path: ".trae/skills",
                global_path: "~/.trae-cn/skills",
            },
            AgentType::Windsurf => AgentConfig {
                id: "windsurf",
                display_name: "Windsurf",
                project_path: ".windsurf/skills",
                global_path: "~/.codeium/windsurf/skills",
            },
            AgentType::Zencoder => AgentConfig {
                id: "zencoder",
                display_name: "Zencoder",
                project_path: ".zencoder/skills",
                global_path: "~/.zencoder/skills",
            },
        }
    }

    /// Project skills directory (relative to cwd).
    pub fn project_dir(&self, cwd: &Path) -> PathBuf {
        cwd.join(self.config().project_path)
    }

    /// Resolve global path (expand ~ to home).
    pub fn global_dir(&self) -> PathBuf {
        let home = home_dir();
        let config = config_home();
        
        match self {
            AgentType::Amp | AgentType::KimiCli | AgentType::Replit => {
                config.join("agents/skills")
            }
            AgentType::Antigravity => home.join(".gemini/antigravity/skills"),
            AgentType::Augment => home.join(".augment/skills"),
            AgentType::ClaudeCode => claude_home().join("skills"),
            AgentType::Cline => home.join(".cline/skills"),
            AgentType::CodeBuddy => home.join(".codebuddy/skills"),
            AgentType::Codex => codex_home().join("skills"),
            AgentType::CommandCode => home.join(".commandcode/skills"),
            AgentType::Continue => home.join(".continue/skills"),
            AgentType::Crush => config.join("crush/skills"),
            AgentType::Cursor => home.join(".cursor/skills"),
            AgentType::Droid => home.join(".factory/skills"),
            AgentType::GeminiCli => home.join(".gemini/skills"),
            AgentType::GitHubCopilot => home.join(".copilot/skills"),
            AgentType::Goose => config.join("goose/skills"),
            AgentType::IFlowCli => home.join(".iflow/skills"),
            AgentType::Junie => home.join(".junie/skills"),
            AgentType::Kilo => home.join(".kilocode/skills"),
            AgentType::KiroCli => home.join(".kiro/skills"),
            AgentType::Kode => home.join(".kode/skills"),
            AgentType::Mcpjam => home.join(".mcpjam/skills"),
            AgentType::MistralVibe => home.join(".vibe/skills"),
            AgentType::Mux => home.join(".mux/skills"),
            AgentType::Neovate => home.join(".neovate/skills"),
            AgentType::OpenClaw => openclaw_global(),
            AgentType::OpenCode => config.join("opencode/skills"),
            AgentType::OpenHands => home.join(".openhands/skills"),
            AgentType::Pi => home.join(".pi/agent/skills"),
            AgentType::Pochi => home.join(".pochi/skills"),
            AgentType::Qoder => home.join(".qoder/skills"),
            AgentType::QwenCode => home.join(".qwen/skills"),
            AgentType::Roo => home.join(".roo/skills"),
            AgentType::Trae => home.join(".trae/skills"),
            AgentType::TraeCn => home.join(".trae-cn/skills"),
            AgentType::Windsurf => home.join(".codeium/windsurf/skills"),
            AgentType::Zencoder => home.join(".zencoder/skills"),
            AgentType::AdaL => home.join(".adal/skills"),
        }
    }

    /// Check if this agent is installed (has config dir).
    pub fn is_installed(&self, cwd: &Path) -> bool {
        let home = home_dir();
        let config = config_home();
        match self {
            AgentType::Amp => config.join("amp").exists(),
            AgentType::Antigravity => {
                cwd.join(".agent").exists() || home.join(".gemini/antigravity").exists()
            }
            AgentType::Augment => home.join(".augment").exists(),
            AgentType::ClaudeCode => claude_home().exists(),
            AgentType::Cline => home.join(".cline").exists(),
            AgentType::CodeBuddy => cwd.join(".codebuddy").exists() || home.join(".codebuddy").exists(),
            AgentType::Codex => codex_home().exists() || Path::new("/etc/codex").exists(),
            AgentType::CommandCode => home.join(".commandcode").exists(),
            AgentType::Continue => cwd.join(".continue").exists() || home.join(".continue").exists(),
            AgentType::Crush => config.join("crush").exists(),
            AgentType::Cursor => home.join(".cursor").exists(),
            AgentType::Droid => home.join(".factory").exists(),
            AgentType::GeminiCli => home.join(".gemini").exists(),
            AgentType::GitHubCopilot => cwd.join(".github").exists() || home.join(".copilot").exists(),
            AgentType::Goose => config.join("goose").exists(),
            AgentType::IFlowCli => home.join(".iflow").exists(),
            AgentType::Junie => home.join(".junie").exists(),
            AgentType::Kilo => home.join(".kilocode").exists(),
            AgentType::KimiCli => home.join(".kimi").exists(),
            AgentType::KiroCli => home.join(".kiro").exists(),
            AgentType::Kode => home.join(".kode").exists(),
            AgentType::Mcpjam => home.join(".mcpjam").exists(),
            AgentType::MistralVibe => home.join(".vibe").exists(),
            AgentType::Mux => home.join(".mux").exists(),
            AgentType::Neovate => home.join(".neovate").exists(),
            AgentType::OpenClaw => {
                home.join(".openclaw").exists()
                    || home.join(".clawdbot").exists()
                    || home.join(".moltbot").exists()
            }
            AgentType::OpenCode => config.join("opencode").exists() || claude_home().join("skills").exists(),
            AgentType::OpenHands => home.join(".openhands").exists(),
            AgentType::Pi => home.join(".pi/agent").exists(),
            AgentType::Pochi => home.join(".pochi").exists(),
            AgentType::Qoder => home.join(".qoder").exists(),
            AgentType::QwenCode => home.join(".qwen").exists(),
            AgentType::Replit => cwd.join(".agents").exists(),
            AgentType::Roo => home.join(".roo").exists(),
            AgentType::Trae => home.join(".trae").exists(),
            AgentType::TraeCn => home.join(".trae-cn").exists(),
            AgentType::Windsurf => home.join(".codeium/windsurf").exists(),
            AgentType::Zencoder => home.join(".zencoder").exists(),
            AgentType::AdaL => home.join(".adal").exists(),
        }
    }

    /// Agents that share .agents/skills (universal).
    pub fn is_universal(&self) -> bool {
        matches!(
            self,
            AgentType::Amp
                | AgentType::Codex
                | AgentType::GeminiCli
                | AgentType::GitHubCopilot
                | AgentType::KimiCli
                | AgentType::OpenCode
                | AgentType::Replit
        )
    }
}

/// All agent types in display order.
pub fn all_agents() -> &'static [AgentType] {
    &[
        AgentType::Amp,
        AgentType::Antigravity,
        AgentType::Augment,
        AgentType::ClaudeCode,
        AgentType::OpenClaw,
        AgentType::Cline,
        AgentType::CodeBuddy,
        AgentType::Codex,
        AgentType::CommandCode,
        AgentType::Continue,
        AgentType::Crush,
        AgentType::Cursor,
        AgentType::Droid,
        AgentType::GeminiCli,
        AgentType::GitHubCopilot,
        AgentType::Goose,
        AgentType::Junie,
        AgentType::IFlowCli,
        AgentType::Kilo,
        AgentType::KimiCli,
        AgentType::KiroCli,
        AgentType::Kode,
        AgentType::Mcpjam,
        AgentType::MistralVibe,
        AgentType::Mux,
        AgentType::OpenCode,
        AgentType::OpenHands,
        AgentType::Pi,
        AgentType::Qoder,
        AgentType::QwenCode,
        AgentType::Replit,
        AgentType::Roo,
        AgentType::Trae,
        AgentType::TraeCn,
        AgentType::Windsurf,
        AgentType::Zencoder,
        AgentType::Neovate,
        AgentType::Pochi,
        AgentType::AdaL,
    ]
}

/// Parse agent id string to AgentType. Supports aliases (e.g. "claude" -> claude-code).
pub fn parse_agent(id: &str) -> Option<AgentType> {
    let id = id.trim().to_lowercase();
    let id = match id.as_str() {
        "claude" => "claude-code",
        "copilot" | "github" => "github-copilot",
        "gemini" => "gemini-cli",
        "kimi" => "kimi-cli",
        "kiro" => "kiro-cli",
        "qwen" => "qwen-code",
        "roo" => "roo",
        "trae-cn" | "traecn" => "trae-cn",
        "vibe" => "mistral-vibe",
        other => other,
    };
    all_agents().iter().find(|&&a| a.config().id == id).copied()
}

/// Resolve agent filter: "*" or list of ids.
pub fn resolve_agents(agent: &[String], cwd: &Path) -> Vec<AgentType> {
    if agent.is_empty() {
        return detect_installed_agents(cwd);
    }
    if agent.iter().any(|a| a == "*") {
        return all_agents().to_vec();
    }
    agent
        .iter()
        .filter_map(|a| parse_agent(a))
        .collect()
}

/// Detect which agents are installed.
pub fn detect_installed_agents(cwd: &Path) -> Vec<AgentType> {
    all_agents()
        .iter()
        .filter(|a| a.is_installed(cwd))
        .copied()
        .collect()
}

/// Dir entry for list/remove: (path, scope_label e.g. "cursor" or "cursor (global)").
#[derive(Clone, Debug)]
pub struct SkillDir {
    pub path: PathBuf,
    pub scope: String,
}

/// Collect skill dirs to search for list/remove. When agent filter is empty, uses legacy
/// (.agents/skills, .cursor/skills, .claude/skills, ~/.appz/skills).
pub fn skill_dirs_for_list_remove(
    agent: &[String],
    cwd: &Path,
    user_appz_dir: Option<&Path>,
    project_only: bool,
    global_only: bool,
) -> Vec<SkillDir> {
    let mut dirs = Vec::new();
    let agent_all = agent.is_empty() || agent.iter().any(|a| a == "*");
    let agents: Vec<AgentType> = if agent_all {
        all_agents().to_vec()
    } else {
        agent.iter().filter_map(|a| parse_agent(a)).collect()
    };

    if agents.is_empty() {
        // Legacy: backward-compat paths
        if !global_only {
            for subdir in &[
                ".agents/skills",
                ".cursor/skills",
                ".claude/skills",
            ] {
                let p = cwd.join(subdir);
                if p.exists() {
                    dirs.push(SkillDir {
                        path: p,
                        scope: subdir.split('/').next().unwrap_or("project").to_string(),
                    });
                }
            }
        }
        if !project_only {
            if let Some(appz) = user_appz_dir {
                let p = appz.join("skills");
                if p.exists() {
                    dirs.push(SkillDir {
                        path: p,
                        scope: "global".to_string(),
                    });
                }
            }
        }
        return dirs;
    }

    if !global_only {
        let mut seen_project = std::collections::HashSet::new();
        for a in &agents {
            let p = a.project_dir(cwd);
            if p.exists() && seen_project.insert(p.clone()) {
                dirs.push(SkillDir {
                    path: p,
                    scope: a.config().id.to_string(),
                });
            }
        }
    }
    if !project_only {
        for a in &agents {
            let p = a.global_dir();
            if p.exists() {
                dirs.push(SkillDir {
                    path: p,
                    scope: format!("{} (global)", a.config().id),
                });
            }
        }
        // Include ~/.appz/skills for backward compat when any agent selected
        if let Some(appz) = user_appz_dir {
            let p = appz.join("skills");
            if p.exists() {
                dirs.push(SkillDir {
                    path: p,
                    scope: "appz (global)".to_string(),
                });
            }
        }
    }
    dirs
}

/// Target dirs for add. Returns (path, label). When agent empty: legacy .agents/skills or ~/.appz/skills.
pub fn target_dirs_for_add(
    agent: &[String],
    cwd: &Path,
    user_appz_dir: Option<&Path>,
    project: bool,
    global: bool,
) -> Vec<(PathBuf, String)> {
    let mut targets = Vec::new();
    let agents: Vec<AgentType> = if agent.is_empty() {
        vec![]
    } else if agent.iter().any(|a| a == "*") {
        all_agents().to_vec()
    } else {
        agent.iter().filter_map(|a| parse_agent(a)).collect()
    };

    if agents.is_empty() {
        // Legacy
        if project {
            targets.push((cwd.join(".agents/skills"), "project".to_string()));
        }
        if global {
            if let Some(appz) = user_appz_dir {
                targets.push((appz.join("skills"), "global".to_string()));
            }
        }
        return targets;
    }

    if project {
        let mut seen = std::collections::HashSet::new();
        for a in &agents {
            let p = a.project_dir(cwd);
            if seen.insert(p.clone()) {
                targets.push((p, format!("{} (project)", a.config().id)));
            }
        }
    }
    if global {
        for a in &agents {
            let p = a.global_dir();
            targets.push((p, format!("{} (global)", a.config().id)));
        }
    }
    targets
}
