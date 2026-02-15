//! Security audit of skill content.
//!
//! Ported from aistudio/backend/server.py SECURITY_PATTERNS, SAFE_PATTERNS, audit_skill_security.

use crate::session::AppzSession;
use regex::RegexBuilder;
use serde::Serialize;
use starbase::AppResult;
use std::path::PathBuf;

/// Risk indicator for a security pattern category.
#[derive(Debug, Clone, Serialize)]
pub struct RiskIndicator {
    pub category: String,
    pub severity: String,
    pub description: String,
    #[serde(default)]
    pub matches: Vec<String>,
    #[serde(default)]
    pub line_numbers: Vec<u32>,
}

/// Result of a security audit.
#[derive(Debug, Clone, Serialize)]
pub struct SecurityAuditResult {
    pub risk_level: String,
    pub risk_score: u32,
    pub indicators: Vec<RiskIndicator>,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub passed_checks: Vec<String>,
    pub failed_checks: Vec<String>,
}

struct SecurityCategory {
    id: &'static str,
    severity: &'static str,
    description: &'static str,
    patterns: &'static [&'static str],
}

const SECURITY_CATEGORIES: &[SecurityCategory] = &[
    SecurityCategory {
        id: "credential_access",
        severity: "high",
        description: "Credential and sensitive file access patterns",
        patterns: &[
            r"\.env\b",
            r"credentials?\b",
            r"password[s]?\b",
            r"api[_-]?key[s]?\b",
            r"secret[s]?\b",
            r"token[s]?\b",
            r"private[_-]?key",
            r"\.pem\b",
            r"\.key\b",
            r"ssh[_-]?key",
            r"auth[_-]?token",
            r"bearer\s+token",
            r"~/\.ssh",
            r"~/\.aws",
            r"~/\.config",
        ],
    },
    SecurityCategory {
        id: "environment_variables",
        severity: "medium",
        description: "Environment variable harvesting",
        patterns: &[
            r"process\.env",
            r"os\.environ",
            r"\$\{?\w+\}?",
            r"getenv\s*\(",
            r"ENV\[",
            r"export\s+\w+=",
        ],
    },
    SecurityCategory {
        id: "network_access",
        severity: "medium",
        description: "Network requests to external services",
        patterns: &[
            r"https?://[^\s)\x22\x27]+",
            r"fetch\s*\(",
            r"requests?\.(get|post|put|delete)",
            r"axios\.",
            r"curl\s+",
            r"wget\s+",
            r"socket\.",
            r"connect\s*\(",
            r"websocket",
        ],
    },
    SecurityCategory {
        id: "code_execution",
        severity: "high",
        description: "Code execution and shell commands",
        patterns: &[
            r"eval\s*\(",
            r"exec\s*\(",
            r"subprocess\.",
            r"os\.system\s*\(",
            r"os\.popen\s*\(",
            r"shell\s*=\s*True",
            r"`[^`]+`",
            r"\$\([^\)]+\)",
            r"child_process",
            r"spawn\s*\(",
        ],
    },
    SecurityCategory {
        id: "filesystem_access",
        severity: "medium",
        description: "Filesystem read/write operations",
        patterns: &[
            r"open\s*\([^)]*[\x22\x27][wr]",
            r"readFile",
            r"writeFile",
            r"fs\.",
            r"path\.join",
            r"os\.path",
            r"shutil\.",
            r"glob\.",
            r"rmdir",
            r"unlink",
            r"chmod",
            r"chown",
        ],
    },
    SecurityCategory {
        id: "obfuscation",
        severity: "critical",
        description: "Obfuscated or suspicious code patterns",
        patterns: &[
            r"\\x[0-9a-fA-F]{2}",
            r"\\u[0-9a-fA-F]{4}",
            r"base64\.(encode|decode)",
            r"atob\s*\(",
            r"btoa\s*\(",
            r"fromCharCode",
            r"charCodeAt",
            r"String\.fromCharCode",
        ],
    },
    SecurityCategory {
        id: "external_commands",
        severity: "high",
        description: "Shell or system commands",
        patterns: &[
            r"\bbash\b",
            r"\bsh\b\s+-c",
            r"\bzsh\b",
            r"\bpowershell\b",
            r"\bcmd\.exe\b",
            r"/bin/sh",
            r"/bin/bash",
            r"sudo\s+",
            r"\brm\s+-rf",
            r"\bchmod\s+",
        ],
    },
    SecurityCategory {
        id: "data_exfiltration",
        severity: "critical",
        description: "Potential data exfiltration patterns",
        patterns: &[
            r"upload\s*\(",
            r"send\s*\(",
            r"post\s*\([^)]*data",
            r"FormData",
            r"XMLHttpRequest",
            r"navigator\.sendBeacon",
        ],
    },
];

const SAFE_PATTERNS: &[&str] = &[
    r"##\s+",
    r"\*\*[^*]+\*\*",
    r"```",
    r"example",
    r"usage",
    r"step\s*\d",
    r"first",
    r"then",
    r"finally",
    r"note:",
    r"read-?only",
    r"view",
    r"display",
    r"show",
    r"print",
    r"log",
];

fn severity_weight(s: &str) -> u32 {
    match s {
        "safe" => 0,
        "low" => 10,
        "medium" => 25,
        "high" => 50,
        "critical" => 100,
        _ => 25,
    }
}

fn category_display_name(id: &str) -> String {
    id.split('_')
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().chain(c).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run security audit on skill content.
pub fn audit_skill_security(content: &str) -> SecurityAuditResult {
    let mut indicators = Vec::new();
    let mut passed_checks = Vec::new();
    let mut failed_checks = Vec::new();
    let lines: Vec<&str> = content.split('\n').collect();
    let mut total_severity_score: i32 = 0;

    for cat in SECURITY_CATEGORIES {
        let mut category_matches: Vec<String> = Vec::new();
        let mut matched_lines: Vec<u32> = Vec::new();

        for pattern_str in cat.patterns {
            if let Ok(re) = RegexBuilder::new(pattern_str).case_insensitive(true).build() {
                for (line_num, line) in lines.iter().enumerate() {
                    for cap in re.find_iter(line) {
                        category_matches.push(cap.as_str().to_string());
                        let ln = (line_num + 1) as u32;
                        if !matched_lines.contains(&ln) {
                            matched_lines.push(ln);
                        }
                    }
                }
            }
        }

        let display_name = category_display_name(cat.id);

        if category_matches.is_empty() {
            passed_checks.push(format!("No {} patterns detected", cat.id.replace('_', " ")));
        } else {
            let match_count = category_matches.len();
            let weight = severity_weight(cat.severity) as i32;
            total_severity_score += weight * match_count.min(5) as i32;

            let mut unique: Vec<String> = category_matches
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            unique.truncate(10);
            matched_lines.truncate(10);

            indicators.push(RiskIndicator {
                category: display_name.clone(),
                severity: cat.severity.to_string(),
                description: cat.description.to_string(),
                matches: unique,
                line_numbers: matched_lines,
            });
            failed_checks.push(format!("{}: {} pattern(s) detected", display_name, match_count));
        }
    }

    // Safe patterns reduce risk
    let mut doc_score: i32 = 0;
    for pattern_str in SAFE_PATTERNS {
        if let Ok(re) = RegexBuilder::new(pattern_str).case_insensitive(true).build() {
            if re.is_match(content) {
                doc_score += 5;
            }
        }
    }
    total_severity_score = (total_severity_score - doc_score).max(0);
    let risk_score = total_severity_score.min(100) as u32;

    let risk_level = if risk_score == 0 {
        "safe"
    } else if risk_score <= 20 {
        "low"
    } else if risk_score <= 50 {
        "medium"
    } else if risk_score <= 80 {
        "high"
    } else {
        "critical"
    }
    .to_string();

    let mut recommendations = Vec::new();
    if indicators.iter().any(|i| i.category == "Credential Access") {
        recommendations.push("Review credential access patterns. Consider using environment variables securely.".to_string());
    }
    if indicators.iter().any(|i| i.category == "Code Execution") {
        recommendations.push("Audit all code execution paths. Ensure commands are necessary and sanitized.".to_string());
    }
    if indicators.iter().any(|i| i.category == "Network Access") {
        recommendations.push("Verify all external URLs are trusted. Consider documenting network requirements.".to_string());
    }
    if indicators.iter().any(|i| i.category == "Filesystem Access") {
        recommendations.push("Review file operations. Ensure no sensitive paths are accessed.".to_string());
    }
    if indicators.iter().any(|i| i.category == "Obfuscation") {
        recommendations.push("CRITICAL: Remove or explain any obfuscated code. This is a major red flag.".to_string());
    }
    if recommendations.is_empty() {
        recommendations.push("Skill appears safe. Continue following security best practices.".to_string());
    }

    let summary = match risk_level.as_str() {
        "safe" => "No security concerns detected. This skill appears safe for use.",
        "low" => "Minor patterns detected that may require attention in sensitive environments.",
        "medium" => "Contains features that warrant review before use in production.",
        "high" => "Contains patterns that may pose security risks. Careful review recommended.",
        _ => "Critical security concerns detected. Do not use without thorough audit.",
    }
    .to_string();

    SecurityAuditResult {
        risk_level,
        risk_score,
        indicators,
        summary,
        recommendations,
        passed_checks,
        failed_checks,
    }
}

fn collect_skill_paths(session: &AppzSession) -> Vec<(String, PathBuf)> {
    let mut skills: Vec<(String, PathBuf)> = Vec::new();

    let project_dir = session.working_dir.join(".agents").join("skills");
    if project_dir.exists() {
        collect_skills_from_dir(&project_dir, &mut skills);
    }
    if let Some(appz_dir) = common::user_config::user_appz_dir() {
        let user_dir = appz_dir.join("skills");
        if user_dir.exists() {
            collect_skills_from_dir(&user_dir, &mut skills);
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for (name, path) in skills {
        let key = name.to_lowercase();
        if !seen.contains(&key) {
            seen.insert(key);
            result.push((name, path));
        }
    }
    result
}

fn collect_skills_from_dir(dir: &std::path::Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&skill_file) {
                    if let Ok(name) = parse_skill_name(&content) {
                        out.push((name, path));
                    }
                }
            } else {
                collect_skills_from_dir(&path, out);
            }
        }
    }
}

fn parse_skill_name(content: &str) -> Result<String, miette::Report> {
    let content = content.trim_start();
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| miette::miette!("No frontmatter"))?;
    let rest = rest.trim_start_matches(|c| c == '\n' || c == '\r');
    let end = rest.find("\n---").or_else(|| rest.find("\r\n---")).ok_or_else(|| miette::miette!("No closing ---"))?;
    let yaml = rest[..end].trim();
    #[derive(serde::Deserialize)]
    struct Fm {
        name: String,
    }
    let fm: Fm = serde_yaml::from_str(yaml).map_err(|e| miette::miette!("Invalid YAML: {}", e))?;
    Ok(fm.name)
}

fn find_skill_by_name_or_path(
    session: &AppzSession,
    name_or_path: &str,
) -> Result<Vec<(String, PathBuf)>, miette::Report> {
    let path = std::path::Path::new(name_or_path);
    if path.exists() {
        let canon = path.canonicalize().map_err(|e| miette::miette!("{}", e))?;
        let (skill_file, name) = if canon.ends_with("SKILL.md") {
            let parent = canon.parent().unwrap();
            let name = parent.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "skill".to_string());
            (canon.clone(), name)
        } else if canon.join("SKILL.md").exists() {
            let name = canon.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "skill".to_string());
            (canon.join("SKILL.md"), name)
        } else {
            return Err(miette::miette!("No SKILL.md at {}", path.display()).into());
        };
        return Ok(vec![(name, skill_file.parent().unwrap().to_path_buf())]);
    }

    let all = collect_skill_paths(session);
    let matches: Vec<_> = all
        .into_iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case(name_or_path))
        .collect();
    if matches.is_empty() {
        Err(miette::miette!("Skill '{}' not found", name_or_path).into())
    } else {
        Ok(matches)
    }
}

fn risk_level_rank(level: &str) -> u8 {
    match level {
        "safe" => 0,
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "critical" => 4,
        _ => 0,
    }
}

/// CLI entry point for `appz skills audit [NAME_OR_PATH]`.
pub async fn audit(
    session: AppzSession,
    name_or_path: Option<String>,
    json_output: bool,
    min_risk: Option<String>,
) -> AppResult {
    let min_rank = min_risk
        .as_ref()
        .map(|s| risk_level_rank(s.to_lowercase().as_str()))
        .unwrap_or(0);

    let skills_to_audit: Vec<(String, PathBuf)> = if let Some(ref nop) = name_or_path {
        find_skill_by_name_or_path(&session, nop)?
    } else {
        let collected = collect_skill_paths(&session);
        if collected.is_empty() {
            if json_output {
                println!("[]");
            } else {
                ui::empty::display(
                    "No skills to audit",
                    Some("Run `appz skills add <source>` to add skills, or pass a name/path."),
                )?;
            }
            return Ok(None);
        }
        collected
    };

    let mut all_results: Vec<(String, SecurityAuditResult)> = Vec::new();
    let show_spinner = skills_to_audit.len() > 1 && !json_output;
    let _audit_spinner = if show_spinner {
        Some(ui::progress::spinner("Auditing skills..."))
    } else {
        None
    };
    for (name, skill_dir) in &skills_to_audit {
        let skill_file = skill_dir.join("SKILL.md");
        let content = match std::fs::read_to_string(&skill_file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let result = audit_skill_security(&content);
        if risk_level_rank(&result.risk_level) >= min_rank {
            all_results.push((name.clone(), result));
        }
    }

    if json_output {
        if all_results.len() == 1 {
            let json = serde_json::to_string_pretty(&all_results[0].1)
                .map_err(|e| miette::miette!("JSON: {}", e))?;
            println!("{}", json);
        } else {
            let vec: Vec<&SecurityAuditResult> = all_results.iter().map(|(_, r)| r).collect();
            let json = serde_json::to_string_pretty(&vec)
                .map_err(|e| miette::miette!("JSON: {}", e))?;
            println!("{}", json);
        }
        return Ok(None);
    }

    let _ = ui::layout::blank_line();

    if all_results.len() == 1 {
        let (name, result) = &all_results[0];
        let _ = ui::layout::section_title(name);
        let badge = ui::format::status_badge(&result.risk_level);
        println!("  Risk: {}", badge);
        let _ = ui::status::info(&format!("Score: {}/100", result.risk_score));
        let _ = ui::layout::subsection_title("Summary");
        let _ = ui::layout::indented(&result.summary, 1);
        let _ = ui::layout::blank_line();
        let _ = ui::layout::subsection_title("Recommendations");
        ui::list::display_bullet(&result.recommendations, None)?;
    } else {
        let headers = vec!["Skill", "Risk", "Score"];
        let rows: Vec<Vec<String>> = all_results
            .iter()
            .map(|(name, r)| {
                vec![
                    name.clone(),
                    r.risk_level.clone(),
                    r.risk_score.to_string(),
                ]
            })
            .collect();
        ui::table::display(&headers, &rows, Some("Security Audit Results"))?;
    }

    Ok(None)
}
