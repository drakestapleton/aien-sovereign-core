use colored::*;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;

use crate::client::{ChatClient, extract_tool_calls};
use crate::tools::dispatch_tool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSpec {
    pub id: String,
    pub role: String,
    pub prompt: String,
    pub depth: usize,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    pub id: String,
    pub role: String,
    pub depth: usize,
    pub parent_id: Option<String>,
    pub status: String,
    pub summary: String,
    pub transcript_path: String,
    pub steps_executed: usize,
    pub children_spawned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentStepLog {
    pub step: usize,
    pub assistant_response: String,
    pub tool_calls: Vec<(String, Value)>,
    pub tool_results: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentTranscript {
    pub spec: SubagentSpec,
    pub start_time: String,
    pub end_time: String,
    pub status: String,
    pub summary: String,
    pub steps: Vec<SubagentStepLog>,
    pub children: Vec<String>,
}

pub fn get_subagent_system_prompt(spec: &SubagentSpec) -> String {
    let mut p = String::new();
    p.push_str(&format!("You are an autonomous AIEN Subagent with specialized role: '{}'.\n", spec.role));
    p.push_str(&format!("Execution Depth: {} (Max allowed: 3). Assigned by Parent Coordinator: {}.\n", spec.depth, spec.parent_id.as_deref().unwrap_or("Root")));
    p.push_str("Host: NVIDIA DGX Spark (Grace Blackwell GB10).\n");
    p.push_str("Primary Workspace: /home/drakestapleton/workspace/aien-sovereign-core\n\n");
    p.push_str("OBJECTIVE:\n");
    p.push_str(&format!("Execute the assigned task with high technical precision:\n{}\n\n", spec.prompt));
    p.push_str("Available Tools:\n");
    p.push_str("- run_command: {\"command\": \"string\", \"cwd\": \"string\"}\n");
    p.push_str("- view_file: {\"path\": \"string\", \"start_line\": 1, \"end_line\": 100}\n");
    p.push_str("- write_to_file: {\"path\": \"string\", \"content\": \"string\", \"overwrite\": true}\n");
    p.push_str("- replace_file_content: {\"path\": \"string\", \"target\": \"string\", \"replacement\": \"string\"}\n");
    p.push_str("- create_dir: {\"path\": \"string\", \"purpose\": \"string\"}\n");
    p.push_str("- list_dir: {\"path\": \"string\"}\n");
    p.push_str("- grep_search: {\"query\": \"string\", \"path\": \"string\"}\n");
    p.push_str("- crumb: {\"action\": \"survey|whisper|record|init\", \"path\": \"string\"}\n");
    p.push_str("- vault: {\"action\": \"list|check|audit\", \"key\": \"string\"}\n");
    p.push_str("- cortex: {\"action\": \"search|write\", \"query\": \"string\", \"name\": \"string\", \"content\": \"string\", \"kind\": \"lesson|discovery|procedure\"}\n");
    p.push_str("- sandbox: {\"action\": \"init|status|test|promote|clean\"}\n");
    p.push_str("- browser: {\"action\": \"test|mentor\", \"prompt\": \"string\"}\n");
    if spec.depth < 3 {
        p.push_str("- invoke_subagent: {\"role\": \"string\", \"prompt\": \"string\"}\n");
    }
    p.push_str("\nPROTOCOL & CONTEXT HYGIENE:\n");
    p.push_str("1. ZERO EM DASHES AND EN DASHES: Strict invariant. Use standard commas, colons, or parentheses.\n");
    p.push_str("2. NO UNSLOP OR FLUFF: Lead directly with technical output and code.\n");
    p.push_str("3. ZERO PLAINTEXT SECRETS: Hardware TPM vault only.\n");
    if spec.depth < 3 {
        p.push_str("4. RECURSIVE SUBAGENTS: If a subtask requires substantial exploratory search, isolated testing, or deep verification, invoke a child subagent to keep your own context focused.\n");
    } else {
        p.push_str("4. LEAF SUBAGENT: Maximum recursion depth reached. Complete all remaining steps directly without invoking further subagents.\n");
    }
    p.push_str("5. SYNTHESIS: When your work is done, output your final response WITHOUT tool calls. Summarize your technical findings, exact changes, and verification proof so your parent coordinator receives a distilled, high-signal report.\n\n");
    p.push_str("To call a tool, output:\n<tool_call>\n{\"name\": \"tool_name\", \"arguments\": {}}\n</tool_call>\n");
    p
}

fn generate_short_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    format!("{:04x}", nanos % 0xffff)
}

pub fn execute_subagent(spec: SubagentSpec) -> Pin<Box<dyn Future<Output = SubagentResult> + Send>> {
    Box::pin(async move {
        let start_time = Utc::now().to_rfc3339();
        let indent = "  ".repeat(spec.depth);
        println!("{}", format!("{}┌─ [Subagent Spawning: '{}' | Depth: {} | ID: {}]", indent, spec.role, spec.depth, spec.id).cyan().bold());
        println!("{}", format!("{}│ Directive: {}", indent, spec.prompt).dimmed());

        let client = ChatClient::new(None, None);
        let mut messages = vec![
            json!({"role": "system", "content": get_subagent_system_prompt(&spec)}),
            json!({"role": "user", "content": format!("TASK ASSIGNMENT:\nRole: {}\nPrompt: {}\n\nExecute all necessary steps using tools, verify outcomes, and provide your distilled technical summary when complete.", spec.role, spec.prompt)})
        ];

        let mut steps_log = Vec::new();
        let mut children_spawned = Vec::new();
        let mut max_steps = 10;
        let mut step_count = 0;
        let mut final_summary = String::new();

        while max_steps > 0 {
            max_steps -= 1;
            step_count += 1;

            let assistant_resp = match client.stream_turn_with_limit(&messages, false, 1024).await {
                Ok(text) => text,
                Err(e) => {
                    let err_msg = format!("Subagent inference failed: {}", e);
                    println!("{}", format!("{}│ [Error] {}", indent, err_msg).red());
                    final_summary = err_msg;
                    break;
                }
            };

            messages.push(json!({"role": "assistant", "content": assistant_resp.clone()}));

            let tool_calls = extract_tool_calls(&assistant_resp);
            if tool_calls.is_empty() {
                // Done! Assistant provided its final synthesis without tool calls.
                final_summary = assistant_resp;
                break;
            }

            let mut step_results = Vec::new();
            for (tool_name, tool_args) in &tool_calls {
                println!("{}", format!("{}│ [Step {}] Executing tool: {}", indent, step_count, tool_name).yellow());

                let tool_result = if tool_name == "invoke_subagent" {
                    if spec.depth >= 3 {
                        json!({"error": "Maximum subagent recursion depth (3) reached. You must complete the task directly without further delegation."})
                    } else {
                        let child_role = tool_args.get("role").and_then(Value::as_str).unwrap_or("Child Subagent");
                        let child_prompt = tool_args.get("prompt").and_then(Value::as_str).unwrap_or("");
                        let child_spec = SubagentSpec {
                            id: format!("sub-{}-{}", Utc::now().timestamp(), generate_short_id()),
                            role: child_role.to_string(),
                            prompt: child_prompt.to_string(),
                            depth: spec.depth + 1,
                            parent_id: Some(spec.id.clone()),
                        };
                        children_spawned.push(child_spec.id.clone());
                        let child_res = execute_subagent(child_spec).await;
                        serde_json::to_value(&child_res).unwrap_or_default()
                    }
                } else {
                    dispatch_tool(tool_name, tool_args)
                };

                let result_str = serde_json::to_string(&tool_result).unwrap_or_default();
                messages.push(json!({
                    "role": "user",
                    "content": format!("<tool_response name=\"{}\">\n{}\n</tool_response>", tool_name, result_str)
                }));
                step_results.push(tool_result);
            }

            steps_log.push(SubagentStepLog {
                step: step_count,
                assistant_response: assistant_resp,
                tool_calls: tool_calls.clone(),
                tool_results: step_results,
            });
        }

        if final_summary.is_empty() {
            final_summary = "Subagent completed execution loop (reached step limit).".to_string();
        }

        let end_time = Utc::now().to_rfc3339();
        let transcript_dir = Path::new("/home/drakestapleton/basecamp/sessions/subagents");
        let _ = fs::create_dir_all(transcript_dir);
        let transcript_path = transcript_dir.join(format!("{}.json", spec.id));

        let transcript = SubagentTranscript {
            spec: spec.clone(),
            start_time,
            end_time,
            status: "completed".to_string(),
            summary: final_summary.clone(),
            steps: steps_log,
            children: children_spawned.clone(),
        };

        let _ = fs::write(&transcript_path, serde_json::to_string_pretty(&transcript).unwrap_or_default());

        // Emit comb onto the native hexagonal honeycomb message wall
        if let Ok(store) = spark_hive::CombStore::open_default() {
            let summary_text = if final_summary.chars().count() > 280 {
                let trimmed: String = final_summary.chars().take(280).collect();
                format!("{}...", trimmed)
            } else {
                final_summary.clone()
            };
            if let Ok(comb) = spark_hive::emit_subagent_comb(&store, &spec.role, &summary_text, spec.parent_id.as_deref()) {
                println!("{}", format!("{}  [Hive Lattice] Comb placed at ({}, {}) [id: {}]",
                    indent, comb.q, comb.r, comb.id).yellow());
            }
        }

        println!("{}", format!("{}└─ [Subagent {} Finished (depth: {}, steps: {}, children: {})]", 
            indent, spec.id, spec.depth, step_count, children_spawned.len()).green().bold());

        SubagentResult {
            id: spec.id,
            role: spec.role,
            depth: spec.depth,
            parent_id: spec.parent_id,
            status: "completed".to_string(),
            summary: final_summary,
            transcript_path: transcript_path.to_string_lossy().to_string(),
            steps_executed: step_count,
            children_spawned,
        }
    })
}

pub async fn invoke_subagent_from_root(role: &str, prompt: &str) -> Value {
    let spec = SubagentSpec {
        id: format!("sub-{}-{}", Utc::now().timestamp(), generate_short_id()),
        role: role.to_string(),
        prompt: prompt.to_string(),
        depth: 1,
        parent_id: Some("root-coordinator".to_string()),
    };
    let res = execute_subagent(spec).await;
    serde_json::to_value(&res).unwrap_or_default()
}

pub fn list_subagents() -> Value {
    let dir = Path::new("/home/drakestapleton/basecamp/sessions/subagents");
    if !dir.exists() {
        return json!({"subagents": [], "count": 0});
    }

    let mut items = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(transcript) = serde_json::from_str::<SubagentTranscript>(&content) {
                        items.push(json!({
                            "id": transcript.spec.id,
                            "role": transcript.spec.role,
                            "depth": transcript.spec.depth,
                            "parent_id": transcript.spec.parent_id,
                            "steps": transcript.steps.len(),
                            "status": transcript.status,
                            "start_time": transcript.start_time,
                            "summary_preview": transcript.summary.chars().take(120).collect::<String>(),
                            "children": transcript.children,
                            "transcript_path": path.to_string_lossy()
                        }));
                    }
                }
            }
        }
    }

    items.sort_by(|a, b| {
        let sa = a.get("start_time").and_then(Value::as_str).unwrap_or("");
        let sb = b.get("start_time").and_then(Value::as_str).unwrap_or("");
        sb.cmp(sa)
    });

    let count = items.len();
    json!({"subagents": items, "count": count})
}

pub fn view_subagent(id: &str) -> Value {
    let path = Path::new("/home/drakestapleton/basecamp/sessions/subagents").join(format!("{}.json", id));
    if !path.exists() {
        return json!({"error": format!("Subagent session '{}' not found", id)});
    }
    match fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str::<Value>(&c).unwrap_or_else(|_| json!({"raw": c})),
        Err(e) => json!({"error": e.to_string()}),
    }
}

pub fn format_subagents_tui() -> String {
    let list = list_subagents();
    let mut out = String::new();
    out.push_str("\n=== AIEN Recursive Subagents Hierarchy ===\n");
    if let Some(items) = list.get("subagents").and_then(Value::as_array) {
        if items.is_empty() {
            out.push_str("No active or historical subagent sessions found.\n");
        } else {
            for item in items.iter().take(15) {
                let id = item.get("id").and_then(Value::as_str).unwrap_or("?");
                let role = item.get("role").and_then(Value::as_str).unwrap_or("?");
                let depth = item.get("depth").and_then(Value::as_u64).unwrap_or(1);
                let steps = item.get("steps").and_then(Value::as_u64).unwrap_or(0);
                let parent = item.get("parent_id").and_then(Value::as_str).unwrap_or("Root");
                let preview = item.get("summary_preview").and_then(Value::as_str).unwrap_or("");
                let indent = "  ".repeat(depth as usize);
                out.push_str(&format!("{}[depth {}] {} ({}) <- {}\n", indent, depth, role.cyan().bold(), id.yellow(), parent.dimmed()));
                out.push_str(&format!("{}  Steps: {} | Summary: {}\n", indent, steps, preview.dimmed()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_spec_serialization() {
        let spec = SubagentSpec {
            id: "sub-test-01".to_string(),
            role: "Codebase Researcher".to_string(),
            prompt: "Survey crates and verify Cargo.toml".to_string(),
            depth: 1,
            parent_id: Some("root-coord".to_string()),
        };
        let json_val = serde_json::to_value(&spec).expect("serialize spec");
        assert_eq!(json_val["id"], "sub-test-01");
        assert_eq!(json_val["depth"], 1);
        assert_eq!(json_val["role"], "Codebase Researcher");

        let deserialized: SubagentSpec = serde_json::from_value(json_val).expect("deserialize spec");
        assert_eq!(deserialized.id, spec.id);
        assert_eq!(deserialized.role, spec.role);
    }

    #[test]
    fn test_subagent_depth_guard() {
        let max_depth_spec = SubagentSpec {
            id: "sub-leaf-03".to_string(),
            role: "Leaf Verifier".to_string(),
            prompt: "Perform final check".to_string(),
            depth: 3,
            parent_id: Some("sub-parent-02".to_string()),
        };
        let prompt = get_subagent_system_prompt(&max_depth_spec);
        assert!(prompt.contains("LEAF SUBAGENT: Maximum recursion depth reached"));
        assert!(!prompt.contains("- invoke_subagent:"));
    }

    #[test]
    fn test_subagent_root_spec() {
        let root_spec = SubagentSpec {
            id: "sub-root-01".to_string(),
            role: "Researcher".to_string(),
            prompt: "Initial task".to_string(),
            depth: 1,
            parent_id: None,
        };
        let prompt = get_subagent_system_prompt(&root_spec);
        assert!(prompt.contains("- invoke_subagent:"));
        assert!(prompt.contains("RECURSIVE SUBAGENTS"));
        assert!(!prompt.is_empty());
        // verified hyphen use
    }

    #[test]
    fn test_subagents_list_and_view() {
        let list = list_subagents();
        assert!(list.get("subagents").is_some());
    }
}
