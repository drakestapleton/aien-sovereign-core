use serde::{Deserialize, Serialize};
use chrono::Utc;
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GoalStatus {
    Active,
    Completed,
    Paused,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoalStatus::Active => write!(f, "Active"),
            GoalStatus::Completed => write!(f, "Completed"),
            GoalStatus::Paused => write!(f, "Paused"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: usize,
    pub description: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    pub milestones: Vec<Milestone>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalManifest {
    pub project_name: String,
    pub goals: Vec<Goal>,
}

pub fn get_goals_path() -> PathBuf {
    let local = Path::new(".goals.json");
    if local.exists() {
        return local.to_path_buf();
    }
    PathBuf::from("/home/drakestapleton/basecamp/goals.json")
}

pub fn load_goals() -> GoalManifest {
    let path = get_goals_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(manifest) = serde_json::from_str::<GoalManifest>(&content) {
                return manifest;
            }
        }
    }
    GoalManifest {
        project_name: "Sovereign Workspace".to_string(),
        goals: Vec::new(),
    }
}

pub fn save_goals(manifest: &GoalManifest) -> Result<PathBuf, String> {
    let path = get_goals_path();
    let json_str = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(&path, json_str).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn add_goal(title: &str, description: &str, milestones_raw: Vec<String>) -> Goal {
    let mut manifest = load_goals();
    let id = format!("goal-{}", Utc::now().timestamp());
    
    let milestones: Vec<Milestone> = milestones_raw.into_iter().enumerate().map(|(idx, d)| Milestone {
        id: idx + 1,
        description: d,
        completed: false,
    }).collect();

    let now_str = Utc::now().to_rfc3339();
    let goal = Goal {
        id: id.clone(),
        title: title.to_string(),
        description: description.to_string(),
        status: GoalStatus::Active,
        milestones,
        created_at: now_str.clone(),
        updated_at: now_str,
    };

    manifest.goals.push(goal.clone());
    let _ = save_goals(&manifest);
    goal
}

pub fn complete_goal(target_id: &str) -> Result<String, String> {
    let mut manifest = load_goals();
    let mut found = false;
    let mut title = String::new();

    for g in manifest.goals.iter_mut() {
        if g.id == target_id || g.title.to_lowercase().contains(&target_id.to_lowercase()) {
            g.status = GoalStatus::Completed;
            for m in g.milestones.iter_mut() {
                m.completed = true;
            }
            g.updated_at = Utc::now().to_rfc3339();
            title = g.title.clone();
            found = true;
            break;
        }
    }

    if found {
        let _ = save_goals(&manifest);
        Ok(format!("Goal '{}' ({}) marked as Completed.", title, target_id))
    } else {
        Err(format!("Goal '{}' not found in manifest.", target_id))
    }
}

pub fn complete_milestone(target_id: &str, milestone_id: usize) -> Result<String, String> {
    let mut manifest = load_goals();
    let mut found = false;
    let mut all_completed = false;

    for g in manifest.goals.iter_mut() {
        if g.id == target_id || g.title.to_lowercase().contains(&target_id.to_lowercase()) {
            for m in g.milestones.iter_mut() {
                if m.id == milestone_id {
                    m.completed = true;
                    found = true;
                }
            }
            g.updated_at = Utc::now().to_rfc3339();
            if g.milestones.iter().all(|m| m.completed) {
                g.status = GoalStatus::Completed;
                all_completed = true;
            }
            break;
        }
    }

    if found {
        let _ = save_goals(&manifest);
        if all_completed {
            Ok(format!("Milestone {} completed. All milestones finished! Goal is now Completed.", milestone_id))
        } else {
            Ok(format!("Milestone {} completed.", milestone_id))
        }
    } else {
        Err(format!("Milestone {} for goal '{}' not found.", milestone_id, target_id))
    }
}

pub fn format_goals_tui() -> String {
    let manifest = load_goals();
    let mut out = String::new();

    out.push_str(&format!("{}\n", format!("=== Project Goals: {} ===", manifest.project_name).cyan().bold()));
    if manifest.goals.is_empty() {
        out.push_str(&format!("{}\n", "No active goals in manifest. Use '/goal new <title>' to create one.".dimmed()));
        return out;
    }

    for (i, g) in manifest.goals.iter().enumerate() {
        let status_colored = match g.status {
            GoalStatus::Active => "[ACTIVE]".green().bold(),
            GoalStatus::Completed => "[DONE]".dimmed(),
            GoalStatus::Paused => "[PAUSED]".yellow(),
        };

        out.push_str(&format!("{}. {} {} ({})\n", i + 1, status_colored, g.title.bold(), g.id.cyan()));
        if !g.description.is_empty() {
            out.push_str(&format!("   {}\n", g.description.dimmed()));
        }

        for m in &g.milestones {
            let mark = if m.completed { "✓".green() } else { "○".dimmed() };
            out.push_str(&format!("     {} {}. {}\n", mark, m.id, m.description));
        }
        out.push('\n');
    }

    out
}

pub fn goals_dispatch_tool(args: &Value) -> Value {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("list");

    match action {
        "milestone_done" | "advance" => {
            let id = args.get("id").and_then(Value::as_str).unwrap_or("");
            let m_id = args.get("milestone_id").and_then(Value::as_u64).unwrap_or(1) as usize;
            match complete_milestone(id, m_id) {
                Ok(msg) => json!({"status": "ok", "message": msg}),
                Err(e) => json!({"status": "error", "error": e}),
            }
        },
        "new" | "create" => {
            let title = args.get("title").and_then(Value::as_str).unwrap_or("Untitled Goal");
            let desc = args.get("description").and_then(Value::as_str).unwrap_or("");
            let milestones: Vec<String> = args.get("milestones")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(|s| s.to_string()).collect())
                .unwrap_or_default();

            let g = add_goal(title, desc, milestones);
            json!({
                "status": "ok",
                "goal": g,
                "message": format!("Goal '{}' initialized successfully.", title)
            })
        },
        "done" | "complete" => {
            let id = args.get("id").and_then(Value::as_str).unwrap_or("");
            match complete_goal(id) {
                Ok(msg) => json!({"status": "ok", "message": msg}),
                Err(e) => json!({"status": "error", "error": e}),
            }
        },
        "list" => {
            let manifest = load_goals();
            json!({
                "status": "ok",
                "project": manifest.project_name,
                "total_goals": manifest.goals.len(),
                "goals": manifest.goals
            })
        },
        _ => json!({"error": format!("Unknown goal action '{}'", action)})
    }
}
