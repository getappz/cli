//! loadSkill tool for the agent to fetch full skill instructions on demand.

use aisdk::core::tools::{Tool, ToolExecute};
use sandbox::scoped_fs::ScopedFs;
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::skills::{strip_frontmatter, SkillMetadata};

/// Input for the loadSkill tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct LoadSkillInput {
    /// The skill name to load (case-insensitive).
    name: String,
}

/// Create the loadSkill tool that reads SKILL.md content via the sandbox.
pub fn create_load_skill_tool(skills: Vec<SkillMetadata>, fs: &ScopedFs) -> Tool {
    let fs = fs.clone();
    let execute = ToolExecute::new(Box::new(move |inp: Value| {
        let name = inp
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            return Err("Skill name is required".to_string());
        }

        let skill = skills
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| format!("Skill '{}' not found", name))?;

        let skill_path = skill.path.join("SKILL.md");
        let content = if skill.is_allowed {
            fs.read_allowed(&skill_path)
        } else {
            let root = fs.root();
            let rel = skill_path
                .strip_prefix(root)
                .unwrap_or(&skill_path);
            fs.read_to_string(rel)
        };

        let content = content.map_err(|e| format!("Failed to read skill: {}", e))?;
        let body = strip_frontmatter(&content);

        let skill_dir = skill.path.display().to_string();
        let result = serde_json::json!({
            "skill_directory": skill_dir,
            "content": body
        });
        Ok(result.to_string())
    }));

    Tool {
        name: "loadSkill".to_string(),
        description: "Load a skill to get specialized instructions. Call this when the user's request would benefit from a skill's expertise.".to_string(),
        input_schema: schema_for!(LoadSkillInput),
        execute,
    }
}
