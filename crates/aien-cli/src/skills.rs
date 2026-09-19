use std::fs;
use std::path::{Path, PathBuf};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: String,
    pub has_scripts: bool,
}

pub fn get_skills_dir() -> PathBuf {
    PathBuf::from("/home/drakestapleton/skills")
}

fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let mut in_frontmatter = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_frontmatter {
                break;
            } else {
                in_frontmatter = true;
                continue;
            }
        }
        if in_frontmatter {
            if let Some(rest) = trimmed.strip_prefix(&format!("{}:", field)) {
                return Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    None
}

pub fn discover_skills() -> Vec<Skill> {
    let dir = get_skills_dir();
    let mut skills = Vec::new();
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
        return skills;
    }

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    let content = fs::read_to_string(&skill_md).unwrap_or_default();
                    let name = extract_frontmatter_field(&content, "name")
                        .unwrap_or_else(|| path.file_name().unwrap_or_default().to_string_lossy().to_string());
                    let desc = extract_frontmatter_field(&content, "description")
                        .unwrap_or_else(|| "No description provided.".to_string());
                    let has_scripts = path.join("scripts").exists();
                    skills.push(Skill {
                        name,
                        description: desc,
                        path: skill_md.display().to_string(),
                        has_scripts,
                    });
                }
            }
        }
    }
    skills
}

pub fn format_skills_tui() -> String {
    let skills = discover_skills();
    let mut out = String::new();
    out.push_str(&format!("{}\n", "=== AIEN Sovereign Skill Registry ===".cyan().bold()));
    out.push_str(&format!("Directory: {}\n\n", get_skills_dir().display().to_string().dimmed()));

    if skills.is_empty() {
        out.push_str(&format!("{}\n", "No skills currently installed in ~/skills/. Create a folder with SKILL.md to define one.".dimmed()));
        return out;
    }

    for (i, s) in skills.iter().enumerate() {
        let scripts_badge = if s.has_scripts { "[scripts]".yellow() } else { "".normal() };
        out.push_str(&format!("{}. {} {} ({})\n", i + 1, s.name.bold().green(), scripts_badge, s.path.dimmed()));
        out.push_str(&format!("   {}\n\n", s.description.dimmed()));
    }

    out
}

pub fn read_skill_content(name: &str) -> Result<String, String> {
    let dir = get_skills_dir();
    let skill_path = dir.join(name).join("SKILL.md");
    if skill_path.exists() {
        fs::read_to_string(&skill_path).map_err(|e| e.to_string())
    } else {
        // Try searching by name match
        for s in discover_skills() {
            if s.name.to_lowercase() == name.to_lowercase() {
                return fs::read_to_string(&s.path).map_err(|e| e.to_string());
            }
        }
        Err(format!("Skill '{}' not found in ~/skills/", name))
    }
}

pub fn skills_dispatch_tool(args: &Value) -> Value {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("list");

    match action {
        "list" => {
            let skills = discover_skills();
            json!({
                "status": "ok",
                "total": skills.len(),
                "skills": skills
            })
        },
        "read" | "load" => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or("");
            match read_skill_content(name) {
                Ok(content) => json!({"status": "ok", "name": name, "content": content}),
                Err(e) => json!({"status": "error", "error": e}),
            }
        },
        "optimize" => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or("atlas-skillopt");
            optimize_skill(name)
        },
        _ => json!({"error": format!("Unknown skill action '{}'", action)})
    }
}

pub fn optimize_skill(name: &str) -> Value {
    let script = "/home/drakestapleton/atlas-skillopt-stage.sh";
    if !Path::new(script).exists() {
        return json!({"status": "error", "error": "atlas-skillopt-stage.sh not found"});
    }
    let output = std::process::Command::new("bash")
        .arg(script)
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            json!({
                "status": if out.status.success() { "ok" } else { "failed" },
                "target_skill": name,
                "stdout": stdout,
                "stderr": stderr
            })
        },
        Err(e) => json!({"status": "error", "error": e.to_string()})
    }
}

pub fn run_optimize_cli(name: &str) {
    println!("{}", format!("Starting Microsoft SkillOpt self-improvement loop for '{}'...", name).cyan().bold());
    let res = optimize_skill(name);
    println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
}
