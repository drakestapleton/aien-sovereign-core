use std::process::Command;

pub fn get_vault_secret(key: &str) -> Option<String> {
    if let Ok(val) = std::env::var(key) {
        if !val.trim().is_empty() {
            return Some(val.trim().to_string());
        }
    }

    let output = Command::new("/home/drakestapleton/.local/bin/atlas-vault")
        .arg("get")
        .arg(key)
        .output()
        .ok()?;

    if output.status.success() {
        let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}
