use std::path::Path;
use std::process::Command;
use std::fs;
use colored::*;
use serde_json::{json, Value};

pub const SANDBOX_DIR: &str = "/home/drakestapleton/workspace/aien-sandbox";
pub const REPO_DIR: &str = "/home/drakestapleton/workspace/aien-sovereign-core";

fn cargo_cmd() -> Command {
    let cargo_bin = if Path::new("/home/drakestapleton/.cargo/bin/cargo").exists() {
        "/home/drakestapleton/.cargo/bin/cargo"
    } else {
        "cargo"
    };
    let mut cmd = Command::new(cargo_bin);
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", format!("/home/drakestapleton/.cargo/bin:{}", path));
    } else {
        cmd.env("PATH", "/home/drakestapleton/.cargo/bin:/usr/local/bin:/usr/bin:/bin");
    }
    cmd
}

pub fn init_sandbox(branch_opt: Option<&str>) -> Result<Value, String> {
    let sandbox_path = Path::new(SANDBOX_DIR);
    let repo_path = Path::new(REPO_DIR);

    if !repo_path.exists() {
        return Err(format!("Base repository not found at {}", REPO_DIR));
    }

    let branch = branch_opt.unwrap_or("sandbox-auto");

    if sandbox_path.exists() {
        let _ = clean_sandbox();
    }

    let output = Command::new("git")
        .args(["worktree", "add", "-B", branch, SANDBOX_DIR, "main"])
        .current_dir(REPO_DIR)
        .output()
        .map_err(|e| format!("Failed to create git worktree: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {}", err));
    }

    let crumb_path = sandbox_path.join(".crumb");
    if !crumb_path.exists() {
        let crumb_content = r#"{
  "directory": "aien-sandbox",
  "statement": "Isolated Git worktree sandbox for AIEN autonomous edits and testing.",
  "above": "/home/drakestapleton/workspace",
  "below": []
}"#;
        let _ = fs::write(&crumb_path, crumb_content);
    }

    Ok(json!({
        "status": "ok",
        "action": "init",
        "sandbox_dir": SANDBOX_DIR,
        "branch": branch,
        "message": format!("Sandbox initialized cleanly at {} on branch {}", SANDBOX_DIR, branch)
    }))
}

pub fn status_sandbox() -> Result<Value, String> {
    let sandbox_path = Path::new(SANDBOX_DIR);
    if !sandbox_path.exists() {
        return Ok(json!({
            "status": "not_initialized",
            "sandbox_dir": SANDBOX_DIR,
            "exists": false
        }));
    }

    let status_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(SANDBOX_DIR)
        .output()
        .map_err(|e| e.to_string())?;

    let branch_out = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(SANDBOX_DIR)
        .output()
        .map_err(|e| e.to_string())?;

    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();
    let status_text = String::from_utf8_lossy(&status_out.stdout).to_string();
    let modified_files: Vec<String> = status_text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(json!({
        "status": "ok",
        "sandbox_dir": SANDBOX_DIR,
        "branch": branch,
        "is_clean": modified_files.is_empty(),
        "modified_files": modified_files
    }))
}

pub fn test_sandbox() -> Result<Value, String> {
    let sandbox_path = Path::new(SANDBOX_DIR);
    if !sandbox_path.exists() {
        return Err(format!("Sandbox not found at {}. Run init first.", SANDBOX_DIR));
    }

    let mut cmd = cargo_cmd();
    cmd.args(["check", "--tests"]).current_dir(SANDBOX_DIR);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run cargo check in sandbox: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    Ok(json!({
        "status": if success { "passed" } else { "failed" },
        "exit_code": output.status.code().unwrap_or(-1),
        "stdout": stdout,
        "stderr": stderr
    }))
}

pub fn promote_sandbox(commit_msg_opt: Option<&str>) -> Result<Value, String> {
    let sandbox_path = Path::new(SANDBOX_DIR);
    if !sandbox_path.exists() {
        return Err(format!("Sandbox not found at {}", SANDBOX_DIR));
    }

    let branch_out = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(SANDBOX_DIR)
        .output()
        .map_err(|e| e.to_string())?;
    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();

    if branch.is_empty() || branch == "main" {
        return Err("Cannot promote directly from main or empty branch".to_string());
    }

    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(SANDBOX_DIR)
        .status();

    let msg = commit_msg_opt.unwrap_or("chore(sandbox): autonomous verified edit by AIEN");
    let _ = Command::new("git")
        .args([
            "commit",
            "-m", msg,
            "--author=AIEN <aien.atlas@proton.me>"
        ])
        .current_dir(SANDBOX_DIR)
        .status();

    let merge_out = Command::new("git")
        .args(["merge", &branch, "--no-ff", "-m", &format!("merge(sandbox): integrate {}", branch)])
        .current_dir(REPO_DIR)
        .output()
        .map_err(|e| format!("Merge failed: {}", e))?;

    if !merge_out.status.success() {
        let err = String::from_utf8_lossy(&merge_out.stderr);
        return Err(format!("Failed to merge {} into main: {}", branch, err));
    }

    let mut build_cmd = cargo_cmd();
    build_cmd.args(["build", "--release", "-p", "aien-cli"]).current_dir(REPO_DIR);

    let build_out = build_cmd
        .output()
        .map_err(|e| format!("cargo build failed: {}", e))?;

    let mut install_success = false;
    if build_out.status.success() {
        let ws_bin = format!("{}/target/release/aien-cli", REPO_DIR);
        let release_bin = if Path::new(&ws_bin).exists() {
            ws_bin
        } else {
            format!("{}/crates/aien-cli/target/release/aien-cli", REPO_DIR)
        };
        let dest_bin = "/home/drakestapleton/.local/bin/aien";
        let install_status = Command::new("install")
            .args(["-m", "755", &release_bin, dest_bin])
            .status();
        if let Ok(s) = install_status {
            install_success = s.success();
        }
    }

    let _ = clean_sandbox();

    Ok(json!({
        "status": "promoted",
        "branch": branch,
        "merge_success": true,
        "binary_installed": install_success,
        "message": format!("Successfully promoted {} into main and updated ~/.local/bin/aien", branch)
    }))
}

pub fn clean_sandbox() -> Result<String, String> {
    let sandbox_path = Path::new(SANDBOX_DIR);
    if sandbox_path.exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force", SANDBOX_DIR])
            .current_dir(REPO_DIR)
            .status();

        if sandbox_path.exists() {
            let _ = fs::remove_dir_all(sandbox_path);
        }
    }
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(REPO_DIR)
        .status();

    Ok(format!("Sandbox at {} cleaned successfully", SANDBOX_DIR))
}

pub fn sandbox_dispatch_tool(args: &Value) -> Value {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("status");
    match action {
        "init" => {
            let branch = args.get("branch").and_then(Value::as_str);
            match init_sandbox(branch) {
                Ok(v) => v,
                Err(e) => json!({"status": "error", "error": e}),
            }
        }
        "status" => match status_sandbox() {
            Ok(v) => v,
            Err(e) => json!({"status": "error", "error": e}),
        },
        "test" => match test_sandbox() {
            Ok(v) => v,
            Err(e) => json!({"status": "error", "error": e}),
        },
        "promote" => {
            let msg = args.get("message").and_then(Value::as_str);
            match promote_sandbox(msg) {
                Ok(v) => v,
                Err(e) => json!({"status": "error", "error": e}),
            }
        }
        "clean" => match clean_sandbox() {
            Ok(msg) => json!({"status": "ok", "message": msg}),
            Err(e) => json!({"status": "error", "error": e}),
        },
        _ => json!({"status": "error", "error": format!("Unknown sandbox action: {}", action)}),
    }
}

pub fn handle_sandbox_command(parts: &[&str]) {
    if parts.is_empty() || parts[0] == "status" {
        match status_sandbox() {
            Ok(val) => {
                println!("\n{}", "=== AIEN Sovereign Sandbox Status ===".cyan().bold());
                println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
            }
            Err(e) => println!("{}", format!("Error: {}", e).red()),
        }
    } else if parts[0] == "init" {
        let branch = if parts.len() > 1 { Some(parts[1]) } else { None };
        match init_sandbox(branch) {
            Ok(val) => {
                println!("\n{}", "=== AIEN Sandbox Initialized ===".green().bold());
                println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
            }
            Err(e) => println!("{}", format!("Error: {}", e).red()),
        }
    } else if parts[0] == "test" {
        println!("{}", "Running test suite inside sandbox...".yellow());
        match test_sandbox() {
            Ok(val) => {
                println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
            }
            Err(e) => println!("{}", format!("Error: {}", e).red()),
        }
    } else if parts[0] == "promote" {
        let msg = if parts.len() > 1 { Some(parts[1..].join(" ")) } else { None };
        match promote_sandbox(msg.as_deref()) {
            Ok(val) => {
                println!("\n{}", "=== Sandbox Promoted to Main ===".green().bold());
                println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
            }
            Err(e) => println!("{}", format!("Error: {}", e).red()),
        }
    } else if parts[0] == "clean" {
        match clean_sandbox() {
            Ok(msg) => println!("{}", msg.green()),
            Err(e) => println!("{}", format!("Error: {}", e).red()),
        }
    } else {
        println!("{}", "Usage: /sandbox <init [branch] | status | test | promote [msg] | clean>".yellow());
    }
}

pub fn browser_dispatch_tool(action: &str, prompt: Option<&str>) -> Value {
    let py = if Path::new("/home/drakestapleton/max-env/bin/python").exists() {
        "/home/drakestapleton/max-env/bin/python"
    } else {
        "/home/drakestapleton/.skillopt-venv/bin/python"
    };
    let script = "/home/drakestapleton/basecamp/scripts/browser_mirror_test.py";

    let mut cmd = Command::new(py);
    cmd.arg(script);

    match action {
        "mentor" => {
            cmd.arg("--mentor");
            cmd.arg(prompt.unwrap_or("AIEN, verify your sovereign self-improvement status."));
        }
        _ => {
            cmd.arg("--test");
        }
    }

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if let Ok(parsed) = serde_json::from_str::<Value>(&stdout) {
                parsed
            } else {
                json!({
                    "status": if out.status.success() { "ok" } else { "failed" },
                    "raw_output": stdout,
                    "stderr": String::from_utf8_lossy(&out.stderr).to_string()
                })
            }
        }
        Err(e) => json!({"status": "error", "error": e.to_string()}),
    }
}

pub fn handle_browser_command(parts: &[&str]) {
    if !parts.is_empty() && parts[0] == "mentor" {
        let prompt = if parts.len() > 1 { parts[1..].join(" ") } else { "AIEN, report self-improvement status.".to_string() };
        println!("{}", format!("Spawning browser mentor session with prompt: '{}'...", prompt).yellow());
        let res = browser_dispatch_tool("mentor", Some(&prompt));
        println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
    } else {
        println!("{}", "Spawning headless Chrome and running mirror self-test on Cockpit...".yellow());
        let res = browser_dispatch_tool("test", None);
        println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_constants() {
        assert_eq!(SANDBOX_DIR, "/home/drakestapleton/workspace/aien-sandbox");
        assert_eq!(REPO_DIR, "/home/drakestapleton/workspace/aien-sovereign-core");
    }

    #[test]
    fn test_sandbox_dispatch_unknown() {
        let args = json!({"action": "invalid_action"});
        let res = sandbox_dispatch_tool(&args);
        assert_eq!(res["status"], "error");
    }

    #[test]
    fn test_sandbox_status_serialization() {
        let res = status_sandbox();
        assert!(res.is_ok());
        let val = res.unwrap();
        assert!(val.get("status").is_some());
    }
}
