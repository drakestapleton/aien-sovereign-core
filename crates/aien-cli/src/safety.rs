use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum SafetyDecision {
    Allow,
    AskUser(String),
    Deny(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyPriority {
    SpecificDeny = 1,
    SpecificAsk = 2,
    SpecificAllow = 3,
    PrefixWildcardDeny = 4,
    PrefixWildcardAsk = 5,
    PrefixWildcardAllow = 6,
    GlobalWildcardDeny = 7,
    GlobalWildcardAsk = 8,
    GlobalWildcardAllow = 9,
}

pub type PredicateFn = Arc<dyn Fn(&Value) -> Result<bool, String> + Send + Sync>;

#[derive(Clone)]
pub struct PolicyRule {
    pub name: String,
    pub tool_pattern: String,
    pub priority: PolicyPriority,
    pub predicate: Option<PredicateFn>,
    pub action: SafetyDecision,
}

pub struct SafetyEngine {
    rules: Vec<PolicyRule>,
    workspaces: Vec<PathBuf>,
}

impl SafetyEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            workspaces: vec![
                PathBuf::from("/home/drakestapleton/workspace"),
                PathBuf::from("/home/drakestapleton/atlas-prime-workspace"),
                PathBuf::from("/home/drakestapleton/basecamp"),
                PathBuf::from("/home/drakestapleton/spark-neural-os"),
                PathBuf::from("/home/drakestapleton/skills"),
                PathBuf::from("/home/drakestapleton/aien-cli"),
                PathBuf::from("/home/drakestapleton/spark-cockpit"),
                PathBuf::from("/home/drakestapleton/spark-cockpit-rs"),
                PathBuf::from("/tmp"),
            ],
        }
    }

    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.priority);
    }

    pub fn set_workspaces(&mut self, workspaces: Vec<PathBuf>) {
        self.workspaces = workspaces;
    }

    pub fn evaluate(&self, tool: &str, args: &Value) -> SafetyDecision {
        for rule in &self.rules {
            if !self.matches_tool_pattern(&rule.tool_pattern, tool) {
                continue;
            }

            if let Some(pred) = &rule.predicate {
                match pred(args) {
                    Ok(true) => return rule.action.clone(),
                    Ok(false) => continue,
                    Err(err) => {
                        // Fail-closed guarantee from Antigravity SDK:
                        // Predicate evaluation failure treats the rule as matched for Deny/Ask
                        return match &rule.action {
                            SafetyDecision::Deny(msg) => SafetyDecision::Deny(format!("{}: [Predicate Evaluation Error: {}]", msg, err)),
                            SafetyDecision::AskUser(msg) => SafetyDecision::AskUser(format!("{}: [Predicate Evaluation Error: {}]", msg, err)),
                            SafetyDecision::Allow => SafetyDecision::Deny(format!("Predicate failed closed on allow rule: {}", err)),
                        };
                    }
                }
            } else {
                return rule.action.clone();
            }
        }

        // Default conservative fallback
        SafetyDecision::AskUser(format!("No matching safety rule for tool '{}'", tool))
    }

    fn matches_tool_pattern(&self, pattern: &str, tool: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern.ends_with("/*") {
            let prefix = &pattern[..pattern.len() - 2];
            return tool.starts_with(prefix);
        }
        pattern == tool
    }

    pub fn default_sovereign_engine() -> Self {
        let mut engine = Self::new();

        // 1. SPECIFIC DENY: Block writing plaintext secrets to .env or credential files
        engine.add_rule(PolicyRule {
            name: "deny_env_secrets".to_string(),
            tool_pattern: "write_to_file".to_string(),
            priority: PolicyPriority::SpecificDeny,
            predicate: Some(Arc::new(|args| {
                let path = args.get("path").and_then(Value::as_str).unwrap_or("");
                Ok(is_secret_leak_path(path))
            })),
            action: SafetyDecision::Deny("REJECTED BY SOVEREIGN VAULT POLICY: Writing plaintext secrets into .env or credential files is forbidden. Use 'atlas-vault add <KEY>'.".to_string()),
        });

        engine.add_rule(PolicyRule {
            name: "deny_env_secrets_replace".to_string(),
            tool_pattern: "replace_file_content".to_string(),
            priority: PolicyPriority::SpecificDeny,
            predicate: Some(Arc::new(|args| {
                let path = args.get("path").and_then(Value::as_str).unwrap_or("");
                Ok(is_secret_leak_path(path))
            })),
            action: SafetyDecision::Deny("REJECTED BY SOVEREIGN VAULT POLICY: Writing plaintext secrets into .env or credential files is forbidden. Use 'atlas-vault add <KEY>'.".to_string()),
        });

        // 2. SPECIFIC DENY: Block writing unmasked API keys or private keys into any file
        engine.add_rule(PolicyRule {
            name: "deny_unmasked_secret_content".to_string(),
            tool_pattern: "write_to_file".to_string(),
            priority: PolicyPriority::SpecificDeny,
            predicate: Some(Arc::new(|args| {
                let content = args.get("content").and_then(Value::as_str).unwrap_or("");
                Ok(contains_unmasked_secret(content).is_some())
            })),
            action: SafetyDecision::Deny("REJECTED BY SOVEREIGN VAULT POLICY: Plaintext API key or credential pattern detected. Use atlas-vault.".to_string()),
        });

        // 3. SPECIFIC ASK: Destructive commands require operator confirmation
        engine.add_rule(PolicyRule {
            name: "ask_destructive_commands".to_string(),
            tool_pattern: "run_command".to_string(),
            priority: PolicyPriority::SpecificAsk,
            predicate: Some(Arc::new(|args| {
                let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
                Ok(is_high_risk_command(cmd))
            })),
            action: SafetyDecision::AskUser("Potentially destructive command detected.".to_string()),
        });

        // 4. SPECIFIC ASK: Writing to OS system paths (/etc, /boot, /root, /usr)
        engine.add_rule(PolicyRule {
            name: "ask_system_path_writes".to_string(),
            tool_pattern: "write_to_file".to_string(),
            priority: PolicyPriority::SpecificAsk,
            predicate: Some(Arc::new(|args| {
                let path = args.get("path").and_then(Value::as_str).unwrap_or("");
                Ok(is_high_risk_path(path))
            })),
            action: SafetyDecision::AskUser("Writing to system operating system path requires operator confirmation.".to_string()),
        });

        // 5. SPECIFIC ALLOW: Safe standard tool execution
        let safe_tools = [
            "view_file", "list_dir", "grep_search", "crumb",
            "vault", "goal", "skill", "cortex", "walkthrough", "hive",
            "sandbox", "browser", "invoke_subagent", "subagent", "subagents",
            "adapter", "model_adapter", "model_adapters", "socratic", "socratic_inquiry"
        ];
        for st in safe_tools {
            engine.add_rule(PolicyRule {
                name: format!("allow_{}", st),
                tool_pattern: st.to_string(),
                priority: PolicyPriority::SpecificAllow,
                predicate: None,
                action: SafetyDecision::Allow,
            });
        }

        // 6. GLOBAL WILDCARD ALLOW: Permissive default for verified standard commands
        engine.add_rule(PolicyRule {
            name: "allow_standard_commands".to_string(),
            tool_pattern: "run_command".to_string(),
            priority: PolicyPriority::GlobalWildcardAllow,
            predicate: None,
            action: SafetyDecision::Allow,
        });

        engine.add_rule(PolicyRule {
            name: "allow_standard_writes".to_string(),
            tool_pattern: "write_to_file".to_string(),
            priority: PolicyPriority::GlobalWildcardAllow,
            predicate: None,
            action: SafetyDecision::Allow,
        });

        engine.add_rule(PolicyRule {
            name: "allow_standard_replaces".to_string(),
            tool_pattern: "replace_file_content".to_string(),
            priority: PolicyPriority::GlobalWildcardAllow,
            predicate: None,
            action: SafetyDecision::Allow,
        });

        engine.add_rule(PolicyRule {
            name: "allow_standard_create_dir".to_string(),
            tool_pattern: "create_dir".to_string(),
            priority: PolicyPriority::GlobalWildcardAllow,
            predicate: None,
            action: SafetyDecision::Allow,
        });

        engine
    }
}

pub fn is_high_risk_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    let dangerous_patterns = [
        "rm -rf /",
        "rm -rf ~",
        "rm -rf /home",
        "mkfs",
        "dd if=",
        "fdisk",
        "> /dev/sd",
        "> /dev/nvme",
        "shutdown",
        "reboot",
        "drop database",
        "drop table",
        "truncate table",
        "kill -9 1",
    ];

    for pattern in dangerous_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }
    false
}

pub fn is_high_risk_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.starts_with("/etc")
        || lower.starts_with("/boot")
        || lower.starts_with("/root")
        || lower.starts_with("/usr")
}

pub fn is_secret_leak_path(path: &str) -> bool {
    let p = Path::new(path);
    let file_name = p.file_name().and_then(|f| f.to_str()).unwrap_or("").to_lowercase();

    if file_name == ".env" || file_name.starts_with(".env.") || file_name.ends_with(".env") {
        return true;
    }
    if file_name == "secrets.json" || file_name == "credentials.json" || file_name == "id_rsa" {
        return true;
    }
    false
}

pub fn contains_unmasked_secret(content: &str) -> Option<&'static str> {
    let lower = content.to_lowercase();
    if lower.contains("api_key=") || lower.contains("secret_key=") || lower.contains("auth_token=") {
        return Some("Plaintext key assignment pattern detected (e.g. API_KEY=...)");
    }
    if content.contains("sk-proj-") || content.contains("ghp_") || content.contains("AIzaSy") {
        return Some("Live API key signature detected (sk-, ghp-, AIzaSy)");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_secret_leak_paths() {
        assert!(is_secret_leak_path(".env"));
        assert!(is_secret_leak_path("/path/to/.env"));
        assert!(is_secret_leak_path(".env.local"));
        assert!(is_secret_leak_path("production.env"));
        assert!(is_secret_leak_path("secrets.json"));
        assert!(!is_secret_leak_path("environment.rs"));
    }

    #[test]
    fn test_unmasked_secret_patterns() {
        assert!(contains_unmasked_secret("API_KEY=\"abcdefghijklmnop\"").is_some());
        assert!(contains_unmasked_secret("export sk-proj-1234567890abcdef").is_some());
        assert!(contains_unmasked_secret("ghp_1234567890abcdef").is_some());
        assert!(contains_unmasked_secret("let normal_code = true;").is_none());
    }

    #[test]
    fn test_policy_precedence_and_fail_closed() {
        let engine = SafetyEngine::default_sovereign_engine();

        // 1. Specific Deny beats everything: writing to .env
        let env_args = json!({"path": "/home/drakestapleton/workspace/.env", "content": "KEY=123"});
        assert!(matches!(engine.evaluate("write_to_file", &env_args), SafetyDecision::Deny(_)));

        // 2. Specific Ask: high-risk command
        let rm_args = json!({"command": "rm -rf /", "cwd": "."});
        assert!(matches!(engine.evaluate("run_command", &rm_args), SafetyDecision::AskUser(_)));

        // 3. Specific Allow: standard read
        let view_args = json!({"path": "src/main.rs"});
        assert_eq!(engine.evaluate("view_file", &view_args), SafetyDecision::Allow);

        // 4. Global Allow: standard harmless command
        let ls_args = json!({"command": "cargo check", "cwd": "."});
        assert_eq!(engine.evaluate("run_command", &ls_args), SafetyDecision::Allow);
    }
}
