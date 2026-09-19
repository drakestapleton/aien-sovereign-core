mod client;
pub mod cortex;
pub mod goals;
pub mod skills;
pub mod sandbox;
pub mod hooks;
pub mod compaction;
pub mod subagents;
mod commands;
mod crumbs;
mod hive;
mod nesting;
mod safety;
mod telemetry;
mod tools;
mod ui;
pub mod vault;
mod walkthrough;

use colored::*;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::Path;

use client::{extract_tool_calls, get_system_prompt, ChatClient};
use commands::{handle_slash_command, run_doctor};
use nesting::perform_nesting_ritual;
use tools::dispatch_tool;
use ui::print_banner;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        if args[1] == "--adapter" || args[1] == "adapter" || args[1] == "--adapters" {
            let cmd = if args.len() > 2 { format!("/adapter {}", args[2..].join(" ")) } else { "/adapter".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if args[1] == "--skill" || args[1] == "skill" || args[1] == "--skills" {
            let cmd = if args.len() > 2 { format!("/skill {}", args[2..].join(" ")) } else { "/skill".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if (args[1] == "--auto" || args[1] == "auto") && args.len() > 2 {
            run_autonomous_goal(&args[2]).await;
            return;
        }
        if (args[1] == "--goal" || args[1] == "goal") && args.len() > 3 && args[2] == "auto" {
            run_autonomous_goal(&args[3]).await;
            return;
        }
        if args[1] == "--goal" || args[1] == "goal" || args[1] == "--goals" {
            let cmd = if args.len() > 2 { format!("/goal {}", args[2..].join(" ")) } else { "/goal".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if args[1] == "--cortex" || args[1] == "cortex" {
            let cmd = if args.len() > 2 { format!("/cortex {}", args[2..].join(" ")) } else { "/cortex".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if args[1] == "--crumb" || args[1] == "crumb" {
            let target = if args.len() > 2 { std::path::Path::new(&args[2]) } else { std::path::Path::new(".") };
            println!("{}", crumbs::format_dir_crumb_tui(target));
            return;
        }
        if args[1] == "--doctor" || args[1] == "doctor" {
            run_doctor().await;
            return;
        }
        if args[1] == "--vault" || args[1] == "vault" {
            let _ = handle_slash_command("/vault").await;
            return;
        }
        if args[1] == "--nest" || args[1] == "nest" {
            perform_nesting_ritual("manual-nest", true).await;
            return;
        }
        if args[1] == "--help" || args[1] == "-h" || args[1] == "help" {
            let _ = handle_slash_command("/help").await;
            return;
        }
        if args[1] == "--version" || args[1] == "-v" {
            println!("AIEN CLI v0.1.0 (NVIDIA DGX Spark / Grace Blackwell GB10)");
            return;
        }
        if args[1] == "--walkthrough" || args[1] == "-w" || args[1] == "--roadmap" || args[1] == "walkthrough" {
            let card = walkthrough::render_walkthrough_tui();
            println!("{}", card);
            let dest = Path::new("/home/drakestapleton/basecamp/WALKTHROUGH.md");
            let _ = walkthrough::generate_and_save_walkthrough_md(dest);
            return;
        }
        if args[1] == "--sandbox" || args[1] == "sandbox" {
            let cmd = if args.len() > 2 { format!("/sandbox {}", args[2..].join(" ")) } else { "/sandbox".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if args[1] == "--browser" || args[1] == "browser" || args[1] == "--mirror" {
            let cmd = if args.len() > 2 { format!("/browser {}", args[2..].join(" ")) } else { "/browser".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if args[1] == "--subagents" || args[1] == "subagents" || args[1] == "--subagent" || args[1] == "subagent" {
            let cmd = if args.len() > 2 { format!("/subagents {}", args[2..].join(" ")) } else { "/subagents".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if args[1] == "--skill" || args[1] == "skill" {
            let cmd = if args.len() > 2 { format!("/skill {}", args[2..].join(" ")) } else { "/skill".to_string() };
            let _ = handle_slash_command(&cmd).await;
            return;
        }
        if (args[1] == "-p" || args[1] == "--prompt") && args.len() > 2 {
            let prompt = args[2..].join(" ");
            if prompt.starts_with("/") {
                let _ = handle_slash_command(&prompt).await;
                return;
            }
            run_single_prompt(&prompt).await;
            return;
        }
    }

    // Interactive REPL Mode
    print_banner();

    let session_id = format!("session-{}", chrono::Utc::now().timestamp());
    let sessions_dir = Path::new("/home/drakestapleton/basecamp/sessions");
    let _ = fs::create_dir_all(sessions_dir);

    // Mandatory Nesting Ritual: Circle, Check Wind, Leave Scent, Flatten Terrain
    let nesting = perform_nesting_ritual(&session_id, true).await;

    let client = ChatClient::new(None, None);
    let mut messages: Vec<Value> = vec![
        json!({"role": "system", "content": get_system_prompt()}),
        json!({"role": "user", "content": format!("<system_grounding>\n{}\n</system_grounding>\nPerform your mandatory nesting initiation. Confirm your verified terrain, wind, scent mark, and operational readiness before taking commands.", nesting.grounding_context)}),
    ];

    print!("{}", "AIEN Initiation: ".magenta().bold());
    let init_resp = match client.stream_turn(&messages, true).await {
        Ok(text) => text,
        Err(e) => {
            eprintln!("\n{}", format!("Failed during model nesting initiation: {}", e).red().bold());
            return;
        }
    };
    messages.push(json!({"role": "assistant", "content": init_resp}));

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize readline: {}", e);
            return;
        }
    };

    let history_file = "/home/drakestapleton/basecamp/.aien_history";
    let _ = rl.load_history(history_file);

    println!("{}", "Type your request, or /help for slash commands. Press Ctrl+C or /exit to exit.\n".dimmed());

    loop {
        let prompt_str = format!("{}{}", "AIEN".magenta().bold(), " ❯ ".cyan().bold());
        match rl.readline(&prompt_str) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(trimmed);

                if trimmed.starts_with("/") {
                    if handle_slash_command(trimmed).await {
                        continue;
                    }
                }

                let mut user_turn = trimmed.to_string();
                let hook_registry = crate::hooks::HookRegistry::default_sovereign_registry();
                let _ = hook_registry.run_pre_turn(&mut user_turn).await;

                messages.push(json!({"role": "user", "content": user_turn}));

                let compactor = crate::compaction::ContextCompactor::default();
                if let Some(stats) = compactor.compact_if_needed(&mut messages) {
                    println!("{}", format!("⚡ ContextCompactor: Pruned {} bytes of tool output (tokens: {} -> {})",
                        stats.pruned_tool_bytes, stats.initial_tokens, stats.compacted_tokens).cyan().dimmed());
                }

                // Multi-turn tool execution loop
                let mut max_tool_steps = 10;
                while max_tool_steps > 0 {
                    max_tool_steps -= 1;

                    print!("\n{}", "AIEN: ".magenta().bold());
                    let mut assistant_resp = match client.stream_turn(&messages, true).await {
                        Ok(text) => text,
                        Err(e) => {
                            println!("\n{}", format!("Error during inference: {}", e).red().bold());
                            break;
                        }
                    };

                    hook_registry.run_post_turn(&mut assistant_resp).await;
                    messages.push(json!({"role": "assistant", "content": assistant_resp.clone()}));

                    let compactor = crate::compaction::ContextCompactor::default();
                    if let Some(stats) = compactor.compact_if_needed(&mut messages) {
                        println!("{}", format!("  ⚡ ContextCompactor: Pruned {} bytes across {} turns (tokens: {} -> {})",
                            stats.pruned_tool_bytes, stats.pruned_messages_count, stats.initial_tokens, stats.compacted_tokens).cyan().dimmed());
                    }

                    let tool_calls = extract_tool_calls(&assistant_resp);
                    if !tool_calls.is_empty() {
                        for (tool_name, tool_args) in tool_calls {
                            let tool_result = dispatch_tool(&tool_name, &tool_args);
                            let result_str = serde_json::to_string(&tool_result).unwrap_or_default();
                            messages.push(json!({
                                "role": "user",
                                "content": format!("<tool_response name=\"{}\">\n{}\n</tool_response>", tool_name, result_str)
                            }));
                        }
                    } else {
                        // Completed turn
                        break;
                    }
                }

                // Save session turn to sessions/
                let session_path = sessions_dir.join(format!("{}.json", session_id));
                let _ = fs::write(&session_path, serde_json::to_string_pretty(&messages).unwrap_or_default());
                println!();
            },
            Err(ReadlineError::Interrupted) => {
                println!("\n{}", "^C (type /exit to quit)".dimmed());
            },
            Err(ReadlineError::Eof) => {
                println!("\n{}", "Exiting AIEN.".cyan());
                break;
            },
            Err(err) => {
                eprintln!("Readline error: {}", err);
                break;
            }
        }
    }

    let _ = rl.save_history(history_file);
}

async fn run_single_prompt(prompt: &str) {
    let session_id = format!("session-single-{}", chrono::Utc::now().timestamp());
    let sessions_dir = Path::new("/home/drakestapleton/basecamp/sessions");
    let _ = fs::create_dir_all(sessions_dir);

    // Mandatory Nesting Ritual: Required at startup across all modes
    let nesting = perform_nesting_ritual(&session_id, true).await;

    let mut turn_prompt = prompt.to_string();
    let hook_registry = crate::hooks::HookRegistry::default_sovereign_registry();
    let _ = hook_registry.run_pre_turn(&mut turn_prompt).await;

    let client = ChatClient::new(None, None);
    let mut messages = vec![
        json!({"role": "system", "content": get_system_prompt()}),
        json!({"role": "user", "content": format!("<system_grounding>\n{}\n</system_grounding>\n{}", nesting.grounding_context, turn_prompt)})
    ];

    let mut max_tool_steps = 10;
    while max_tool_steps > 0 {
        max_tool_steps -= 1;
        let mut assistant_resp = match client.stream_turn(&messages, true).await {
            Ok(text) => text,
            Err(e) => {
                eprintln!("Error: {}", e);
                return;
            }
        };

        hook_registry.run_post_turn(&mut assistant_resp).await;
        messages.push(json!({"role": "assistant", "content": assistant_resp.clone()}));

        let tool_calls = extract_tool_calls(&assistant_resp);
        if !tool_calls.is_empty() {
            for (tool_name, tool_args) in tool_calls {
                let tool_result = dispatch_tool(&tool_name, &tool_args);
                let result_str = serde_json::to_string(&tool_result).unwrap_or_default();
                messages.push(json!({
                    "role": "user",
                    "content": format!("<tool_response name=\"{}\">\n{}\n</tool_response>", tool_name, result_str)
                }));
            }
        } else {
            break;
        }
    }

    let session_path = sessions_dir.join(format!("{}.json", session_id));
    let _ = fs::write(&session_path, serde_json::to_string_pretty(&messages).unwrap_or_default());
}

pub async fn run_autonomous_goal(goal_id: &str) {
    let manifest = goals::load_goals();
    let goal = match manifest.goals.iter().find(|g| g.id == goal_id || g.title.to_lowercase().contains(&goal_id.to_lowercase())) {
        Some(g) => g.clone(),
        None => {
            eprintln!("{}", format!("Goal '{}' not found in goals.json", goal_id).red());
            return;
        }
    };

    println!("{}", format!("\n🚀 Launching Autonomous Execution Loop for Goal: {}", goal.title).cyan().bold());
    println!("{}", format!("ID: {} | Total Milestones: {}", goal.id, goal.milestones.len()).dimmed());

    let session_id = format!("auto-goal-{}", chrono::Utc::now().timestamp());
    let nesting = perform_nesting_ritual(&session_id, true).await;
    let client = ChatClient::new(None, None);

    for m in &goal.milestones {
        if m.completed {
            println!("{}", format!("  ✓ Milestone {} already completed: {}", m.id, m.description).green());
            continue;
        }

        println!("{}", format!("\n▶ Commencing Milestone {}: {}", m.id, m.description).yellow().bold());

        let mut task_directive = format!(
            "<system_grounding>\n{}\n</system_grounding>\nAUTONOMOUS TASK DIRECTIVE:\nGoal: '{}' (ID: {})\nActive Milestone {}: {}\n\nExecute the necessary commands, write/edit files, run verifications, and accomplish this milestone.\nWhen verified, use the 'goal' tool with action 'milestone_done' (id: \"{}\", milestone_id: {}) and write any durable lesson learned with the 'cortex' tool.\nBegin now.",
            nesting.grounding_context,
            goal.title,
            goal.id,
            m.id,
            m.description,
            goal.id,
            m.id
        );
        let hook_registry = crate::hooks::HookRegistry::default_sovereign_registry();
        let _ = hook_registry.run_pre_turn(&mut task_directive).await;

        let mut messages = vec![
            json!({"role": "system", "content": get_system_prompt()}),
            json!({
                "role": "user",
                "content": task_directive
            })
        ];

        let mut max_tool_steps = 15;
        let mut milestone_done = false;

        while max_tool_steps > 0 {
            max_tool_steps -= 1;
            print!("\n{}", format!("AIEN [Milestone {}]: ", m.id).magenta().bold());
            let mut assistant_resp = match client.stream_turn(&messages, true).await {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("Inference error: {}", e);
                    break;
                }
            };

            hook_registry.run_post_turn(&mut assistant_resp).await;
            messages.push(json!({"role": "assistant", "content": assistant_resp.clone()}));

            let tool_calls = extract_tool_calls(&assistant_resp);
            if !tool_calls.is_empty() {
                for (tool_name, tool_args) in tool_calls {
                    if tool_name == "goal" {
                        let action = tool_args.get("action").and_then(Value::as_str).unwrap_or("");
                        if action == "milestone_done" || action == "done" {
                            milestone_done = true;
                        }
                    }
                    let tool_result = dispatch_tool(&tool_name, &tool_args);
                    let result_str = serde_json::to_string(&tool_result).unwrap_or_default();
                    messages.push(json!({
                        "role": "user",
                        "content": format!("<tool_response name=\"{}\">\n{}\n</tool_response>", tool_name, result_str)
                    }));
                }
            } else {
                break;
            }
            if milestone_done {
                break;
            }
        }

        let _ = goals::complete_milestone(&goal.id, m.id);
        println!("{}", format!("  ✓ Milestone {} accomplished: {}", m.id, m.description).green().bold());
    }

    println!("{}", format!("\n🏆 All milestones for goal '{}' completed successfully!", goal.title).green().bold());
}
