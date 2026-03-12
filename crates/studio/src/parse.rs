//! Parse AI-generated response (open-lovable format: file blocks, packages, commands).

use miette::{miette, Result};
use regex::Regex;
use std::collections::HashMap;

/// Parsed result of an AI generation response.
#[derive(Debug, Default, Clone)]
pub struct ParsedResponse {
    pub files: Vec<FileEntry>,
    pub packages: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub content: String,
}

/// Parse the raw AI response into files, packages, and commands.
pub fn parse(response: &str) -> Result<ParsedResponse> {
    let mut files: HashMap<String, String> = HashMap::new();
    let mut packages: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();

    // <file path="...">content</file> or <file path="...">content (no closing, take rest)
    let file_re = Regex::new(r#"<file\s+path="([^"]+)">([\s\S]*?)(?:</file>|$)"#)
        .map_err(|e| miette!("Invalid file regex: {}", e))?;
    for cap in file_re.captures_iter(response) {
        let path = cap[1].trim().to_string();
        let content = cap[2].trim().to_string();
        // Prefer longer / later content for same path (simplified: last wins)
        if path.is_empty() {
            continue;
        }
        let is_complete = response[cap.get(0).unwrap().range()].contains("</file>");
        let existing = files.get(&path);
        let replace = match existing {
            None => true,
            Some(_) => is_complete || content.len() > existing.map(|s| s.len()).unwrap_or(0),
        };
        if replace {
            files.insert(path, content);
        }
    }

    // <package>name</package>
    let pkg_re = Regex::new(r"<package>([^<]+)</package>").map_err(|e| miette!("Invalid package regex: {}", e))?;
    for cap in pkg_re.captures_iter(response) {
        let p = cap[1].trim().to_string();
        if !p.is_empty() && !packages.contains(&p) {
            packages.push(p);
        }
    }

    // <packages>...lines...</packages>
    let packages_block_re =
        Regex::new(r"<packages>([\s\S]*?)</packages>").map_err(|e| miette!("Invalid packages regex: {}", e))?;
    if let Some(cap) = packages_block_re.captures(response) {
        for line in cap[1].split(&[',', '\n'][..]) {
            let p = line.trim().to_string();
            if !p.is_empty() && !packages.contains(&p) {
                packages.push(p);
            }
        }
    }

    // <command>...</command>
    let cmd_re = Regex::new(r"<command>([^<]*)</command>").map_err(|e| miette!("Invalid command regex: {}", e))?;
    for cap in cmd_re.captures_iter(response) {
        let c = cap[1].trim().to_string();
        if !c.is_empty() {
            commands.push(c);
        }
    }

    let files = files
        .into_iter()
        .map(|(path, content)| FileEntry { path, content })
        .collect();

    Ok(ParsedResponse {
        files,
        packages,
        commands,
    })
}
