use std::process::Command;
use serde_json::{json, Value};

const HIVE_BIN: &str = "/home/drakestapleton/aien-hive/target/release/aien-hive";

pub fn hive_roster() -> String {
    let out = Command::new(HIVE_BIN).arg("roster").output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Failed to run aien-hive roster: {}", e),
    }
}

pub fn hive_spawn(role: &str, task: &str) -> String {
    let out = Command::new(HIVE_BIN)
        .args(["spawn", role, task])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Failed to spawn cell: {}", e),
    }
}

pub fn hive_swarm(task: &str, n: usize) -> String {
    let out = Command::new(HIVE_BIN)
        .args(["swarm", "--task", task, "--n", &n.to_string()])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Failed to run swarm: {}", e),
    }
}

pub fn hive_read(name: &str) -> String {
    let out = Command::new(HIVE_BIN)
        .args(["read", name])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Failed to read cell {}: {}", name, e),
    }
}

pub fn hive_kill(name: &str) -> String {
    let out = Command::new(HIVE_BIN)
        .args(["kill", name])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Failed to kill cell {}: {}", name, e),
    }
}

pub fn hive_sense(text: &str) -> String {
    let out = Command::new(HIVE_BIN)
        .args(["sense", text])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Failed to sense: {}", e),
    }
}

pub fn hive_dispatch_tool(args: &Value) -> Value {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("roster");
    match action {
        "roster" => json!({"roster": hive_roster()}),
        "spawn" => {
            let role = args.get("role").and_then(Value::as_str).unwrap_or("researcher");
            let task = args.get("task").and_then(Value::as_str).unwrap_or("");
            json!({"result": hive_spawn(role, task)})
        },
        "swarm" => {
            let task = args.get("task").and_then(Value::as_str).unwrap_or("");
            let n = args.get("n").and_then(Value::as_u64).unwrap_or(2) as usize;
            json!({"result": hive_swarm(task, n)})
        },
        "read" => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or("");
            json!({"output": hive_read(name)})
        },
        "kill" => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or("");
            json!({"result": hive_kill(name)})
        },
        "sense" => {
            let text = args.get("text").and_then(Value::as_str).unwrap_or("");
            json!({"salience": hive_sense(text)})
        },
        other => json!({"error": format!("Unknown hive action: {}", other)}),
    }
}
