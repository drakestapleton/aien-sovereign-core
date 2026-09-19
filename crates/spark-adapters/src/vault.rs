use std::process::Command;

/// Dynamically resolve a secret token.
/// Checks hardware TPM vault first (`atlas-vault get <KEY>`), then falls back to environment variables.
pub fn resolve_secret(key_name: &str) -> Option<String> {
    // 1. Try hardware TPM-bound vault
    if let Ok(out) = Command::new("atlas-vault").args(["get", key_name]).output() {
        if out.status.success() {
            let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }

    // 2. Fallback to process environment (standard for open-source users running outside Spark DGX)
    if let Ok(env_val) = std::env::var(key_name) {
        let trimmed = env_val.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    None
}

/// Check if a secret key is registered without exposing its value.
pub fn is_secret_present(key_name: &str) -> bool {
    resolve_secret(key_name).is_some()
}
