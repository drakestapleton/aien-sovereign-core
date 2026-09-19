use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

#[async_trait]
pub trait AgentHook: Send + Sync {
    fn name(&self) -> &'static str;

    async fn pre_turn(&self, _prompt: &mut String) -> Result<(), String> {
        Ok(())
    }

    async fn post_turn(&self, _response: &mut String) {}

    async fn pre_tool(&self, _tool: &str, _args: &mut Value) -> Result<Option<Value>, String> {
        Ok(None)
    }

    async fn post_tool(&self, _tool: &str, _args: &Value, _result: &mut Value) {}

    async fn on_tool_error(&self, _tool: &str, _args: &Value, _error: &str) -> Option<Value> {
        None
    }

    async fn on_compaction(&self, _messages: &mut Vec<Value>) {}
}

#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<Arc<dyn AgentHook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn register(&mut self, hook: Arc<dyn AgentHook>) {
        self.hooks.push(hook);
    }

    pub async fn run_pre_turn(&self, prompt: &mut String) -> Result<(), String> {
        for h in &self.hooks {
            h.pre_turn(prompt).await?;
        }
        Ok(())
    }

    pub async fn run_post_turn(&self, response: &mut String) {
        for h in &self.hooks {
            h.post_turn(response).await;
        }
    }

    pub async fn run_pre_tool(&self, tool: &str, args: &mut Value) -> Result<Option<Value>, String> {
        for h in &self.hooks {
            if let Some(short_circuit) = h.pre_tool(tool, args).await? {
                return Ok(Some(short_circuit));
            }
        }
        Ok(None)
    }

    pub async fn run_post_tool(&self, tool: &str, args: &Value, result: &mut Value) {
        for h in &self.hooks {
            h.post_tool(tool, args, result).await;
        }
    }

    pub async fn run_on_tool_error(&self, tool: &str, args: &Value, error: &str) -> Option<Value> {
        for h in &self.hooks {
            if let Some(recovery) = h.on_tool_error(tool, args, error).await {
                return Some(recovery);
            }
        }
        None
    }

    pub async fn run_on_compaction(&self, messages: &mut Vec<Value>) {
        for h in &self.hooks {
            h.on_compaction(messages).await;
        }
    }

    pub fn default_sovereign_registry() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(UnslopHook));
        reg.register(Arc::new(VaultRedactionHook));
        reg.register(Arc::new(CrumbAutoTrackingHook));
        reg.register(Arc::new(CompilerDiagnosticsHook));
        reg.register(Arc::new(CortexRecallHook));
        reg.register(Arc::new(CortexLearningCaptureHook));
        reg
    }
}

// ==========================================
// Standard Sovereign Hooks
// ==========================================

/// 1. Unslop Hook: Enforces strict sovereign voice and wipes em dashes and en dashes
pub struct UnslopHook;

#[async_trait]
impl AgentHook for UnslopHook {
    fn name(&self) -> &'static str {
        "UnslopHook"
    }

    async fn post_turn(&self, response: &mut String) {
        let sanitized = response.replace('—', ", ").replace('–', "-");
        *response = sanitized;
    }
}

/// 2. Vault Redaction Hook: Scans and redacts credentials before outputs persist
pub struct VaultRedactionHook;

#[async_trait]
impl AgentHook for VaultRedactionHook {
    fn name(&self) -> &'static str {
        "VaultRedactionHook"
    }

    async fn post_turn(&self, response: &mut String) {
        *response = crate::vault::redact_secrets(response);
    }

    async fn post_tool(&self, _tool: &str, _args: &Value, result: &mut Value) {
        if let Some(s) = result.as_str() {
            *result = json!(crate::vault::redact_secrets(s));
        } else if let Some(obj) = result.as_object_mut() {
            for val in obj.values_mut() {
                if let Some(s) = val.as_str() {
                    *val = json!(crate::vault::redact_secrets(s));
                }
            }
        }
    }
}

/// 3. Crumb Auto Tracking Hook: Ensures new directories have a .crumb with explicit purpose
pub struct CrumbAutoTrackingHook;

#[async_trait]
impl AgentHook for CrumbAutoTrackingHook {
    fn name(&self) -> &'static str {
        "CrumbAutoTrackingHook"
    }

    async fn post_tool(&self, tool: &str, args: &Value, result: &mut Value) {
        if tool == "create_dir" {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            let purpose = args.get("purpose").and_then(Value::as_str).unwrap_or("Directory created during autonomous execution.");
            if !path.is_empty() {
                let _ = crate::crumbs::record_directory_crumb("AIEN", "session-hook", std::path::Path::new(path), "create_dir", purpose, "scaffold");
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("crumb_logged".to_string(), json!(true));
                }
            }
        }
    }
}

/// 4. Compiler Diagnostics Hook: Enriches cargo/rustc build failures with structured guidance
pub struct CompilerDiagnosticsHook;

#[async_trait]
impl AgentHook for CompilerDiagnosticsHook {
    fn name(&self) -> &'static str {
        "CompilerDiagnosticsHook"
    }

    async fn on_tool_error(&self, tool: &str, args: &Value, error: &str) -> Option<Value> {
        if tool == "run_command" {
            let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
            if cmd.contains("cargo") || cmd.contains("rustc") {
                return Some(json!({
                    "status": "compiler_error",
                    "command": cmd,
                    "error": error,
                    "guidance": "Compiler diagnostics detected. Review the line numbers and error codes above, apply surgical edits via replace_file_content, and re-verify with cargo check."
                }));
            }
        }
        None
    }
}

/// 5. Cortex Recall Hook: Automatically queries Spark Cortex and injects relevant memories into turn
pub struct CortexRecallHook;

#[async_trait]
impl AgentHook for CortexRecallHook {
    fn name(&self) -> &'static str {
        "CortexRecallHook"
    }

    async fn pre_turn(&self, prompt: &mut String) -> Result<(), String> {
        if let Some(recall) = crate::cortex::assemble_cortex_recall(prompt, 3).await {
            *prompt = format!("{}\n\n{}", recall, prompt);
        }
        Ok(())
    }
}

/// 6. Cortex Learning Capture Hook: Automatically records meaningful actions and verifications into Cortex
pub struct CortexLearningCaptureHook;

#[async_trait]
impl AgentHook for CortexLearningCaptureHook {
    fn name(&self) -> &'static str {
        "CortexLearningCaptureHook"
    }

    async fn post_tool(&self, tool: &str, args: &Value, result: &mut Value) {
        match tool {
            "write_to_file" | "replace_file_content" => {
                let path = args.get("path").and_then(Value::as_str).unwrap_or("");
                let intent = args.get("intent").and_then(Value::as_str).unwrap_or("File modified");
                if !path.is_empty() {
                    crate::cortex::trigger_background_capture("file_write", path, intent);
                }
            },
            "run_command" => {
                let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
                if (cmd.contains("cargo build") || cmd.contains("cargo check") || cmd.contains("gh repo") || cmd.contains("git commit")) && result.get("error").is_none() {
                    crate::cortex::trigger_background_capture("command_success", cmd, "Command verified cleanly");
                }
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unslop_hook() {
        let registry = HookRegistry::default_sovereign_registry();
        let mut text = "This is a feature—not a bug–verified.".to_string();
        registry.run_post_turn(&mut text).await;
        assert!(!text.contains('—'));
        assert!(!text.contains('–'));
        assert!(text.contains("feature, not a bug-verified"));
    }

    #[tokio::test]
    async fn test_vault_redactor_hook() {
        let registry = HookRegistry::default_sovereign_registry();
        let mut text = "Here is the key: sk-ant-api03-1234567890123456789012345678901234567890.".to_string();
        registry.run_post_turn(&mut text).await;
        assert!(text.contains("[REDACTED_ANTHROPIC_KEY]"));
    }
}
