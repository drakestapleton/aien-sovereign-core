use std::path::Path;
use std::process::Command;
use std::sync::{LazyLock, Mutex, RwLock};
use regex::Regex;
use serde_json::{json, Value};

const VAULT_BIN: &str = "/home/drakestapleton/.local/bin/atlas-vault";

static VAULT_SECRET_CACHE: LazyLock<RwLock<Option<Vec<(String, String)>>>> = LazyLock::new(|| RwLock::new(None));
static VAULT_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub fn is_vault_available() -> bool {
    Path::new(VAULT_BIN).is_file()
}

pub fn list_keys() -> Vec<String> {
    if !is_vault_available() {
        return Vec::new();
    }
    let output = Command::new(VAULT_BIN)
        .arg("list")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }
        _ => Vec::new(),
    }
}

pub fn has_key(key: &str) -> bool {
    let keys = list_keys();
    keys.iter().any(|k| k == key)
}

pub fn get_secret(key: &str) -> Result<String, String> {
    if !is_vault_available() {
        return Err("atlas-vault binary not found at /home/drakestapleton/.local/bin/atlas-vault".to_string());
    }
    let output = Command::new(VAULT_BIN)
        .arg("get")
        .arg(key)
        .output()
        .map_err(|e| format!("Failed to execute atlas-vault: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("atlas-vault get failed: {}", stderr.trim()));
    }

    let secret = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if secret.is_empty() {
        return Err(format!("Key {} returned empty value", key));
    }
    Ok(secret)
}

pub fn get_cached_vault_secrets() -> Vec<(String, String)> {
    if cfg!(test) {
        return Vec::new();
    }

    if let Ok(read) = VAULT_SECRET_CACHE.read() {
        if let Some(ref cache) = *read {
            return cache.clone();
        }
    }

    let _guard = match VAULT_MUTEX.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Ok(read) = VAULT_SECRET_CACHE.read() {
        if let Some(ref cache) = *read {
            return cache.clone();
        }
    }

    let keys = list_keys();
    let mut entries = Vec::new();
    for key in keys {
        if let Ok(val) = get_secret(&key) {
            if val.len() >= 6 {
                entries.push((key, val));
            }
        }
    }

    if let Ok(mut write) = VAULT_SECRET_CACHE.write() {
        *write = Some(entries.clone());
    }
    entries
}

/// Scrub text to ensure zero secret values or raw token patterns are printed.
pub fn redact_secrets(input: &str) -> String {
    let mut result = input.to_string();

    // 1. Scrub actual values of known vault keys from memory cache
    let secrets = get_cached_vault_secrets();
    for (key, val) in &secrets {
        result = result.replace(val, &format!("[REDACTED_BY_ATLAS_VAULT:{}]", key));
    }

    // 2. Scrub standard API key patterns
    let patterns = [
        (r"\b(sk-ant-[a-zA-Z0-9_-]{20,})\b", "[REDACTED_ANTHROPIC_KEY]"),
        (r"\b(ghp_[a-zA-Z0-9]{20,})\b", "[REDACTED_GITHUB_TOKEN]"),
        (r"\b(AIzaSy[a-zA-Z0-9_-]{33})\b", "[REDACTED_GOOGLE_KEY]"),
        (r"\b(sk-[a-zA-Z0-9_-]{20,})\b", "[REDACTED_OPENAI_KEY]"),
    ];

    for (pat, repl) in patterns {
        if let Ok(re) = Regex::new(pat) {
            result = re.replace_all(&result, repl).to_string();
        }
    }

    result
}

/// Audit workspace directories for stray plaintext secret files (.env, etc.)
pub fn audit_workspace_secrets() -> Vec<String> {
    let scan_dirs = [
        "/home/drakestapleton/workspace",
        "/home/drakestapleton/basecamp",
        "/home/drakestapleton/spark-neural-os",
        "/home/drakestapleton/aien-cli",
    ];

    let mut findings = Vec::new();
    for d in scan_dirs {
        if !Path::new(d).exists() {
            continue;
        }
        let output = Command::new("find")
            .args([d, "-maxdepth", "4", "-name", ".env*", "-not", "-path", "*/.git/*", "-not", "-path", "*/target/*"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    findings.push(trimmed.to_string());
                }
            }
        }
    }
    findings
}

pub fn vault_dispatch_tool(args: &Value) -> Value {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("list");
    match action {
        "list" => {
            let keys = list_keys();
            json!({
                "status": "ok",
                "vault": "TPM-bound atlas-vault",
                "registered_keys": keys,
                "note": "Secret values reside in hardware TPM. Values are never written to disk or echoed in transcripts."
            })
        },
        "check" => {
            let key = args.get("key").and_then(Value::as_str).unwrap_or("");
            if key.is_empty() {
                return json!({"error": "Missing key argument"});
            }
            let exists = has_key(key);
            json!({
                "status": "ok",
                "key": key,
                "exists": exists,
                "storage": "TPM-bound atlas-vault"
            })
        },
        "audit" => {
            let findings = audit_workspace_secrets();
            json!({
                "status": "ok",
                "stray_env_files_detected": findings.len(),
                "findings": findings,
                "policy": "Zero .env files on disk. Plaintext secrets strictly forbidden."
            })
        },
        other => json!({
            "error": format!("Unknown vault action {}. Supported actions: list, check, audit. Direct value retrieval is restricted to in-memory runtime execution.", other)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_redaction_patterns() {
        let sample = "Error with key sk-proj-1234567890abcdef123456 and token ghp_abcdefghijklmnopqrstuvwxyz12";
        let redacted = redact_secrets(sample);
        assert!(!redacted.contains("sk-proj-1234567890abcdef123456"));
        assert!(!redacted.contains("ghp_abcdefghijklmnopqrstuvwxyz12"));
        assert!(redacted.contains("[REDACTED_OPENAI_KEY]"));
        assert!(redacted.contains("[REDACTED_GITHUB_TOKEN]"));
    }
}
