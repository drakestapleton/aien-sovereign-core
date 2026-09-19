use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub name: String,
    pub passed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub id: String,
    pub workflow_type: String,
    pub current_state: String,
    pub history: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct HarnessEngine {
    pub base_path: PathBuf,
    pub evals: HashMap<String, Value>,
    pub workflows: HashMap<String, Value>,
}

impl HarnessEngine {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        let base = base_path.as_ref().to_path_buf();
        let mut engine = Self {
            base_path: base,
            evals: HashMap::new(),
            workflows: HashMap::new(),
        };
        engine.reload();
        engine
    }

    pub fn reload(&mut self) {
        let evals_path = self.base_path.join("evals.json");
        if let Ok(content) = std::fs::read_to_string(&evals_path) {
            if let Ok(val) = serde_json::from_str::<HashMap<String, Value>>(&content) {
                self.evals = val;
            }
        }

        let wf_path = self.base_path.join("workflows.json");
        if let Ok(content) = std::fs::read_to_string(&wf_path) {
            if let Ok(val) = serde_json::from_str::<HashMap<String, Value>>(&content) {
                self.workflows = val;
            }
        }
    }

    /// Layer 1: Schema Validation
    pub fn validate_schema(&self, schema_name: &str, data: &Value) -> Result<(), String> {
        let schema_path = self.base_path.join("schemas").join(format!("{}.json", schema_name));
        if !schema_path.exists() {
            return Err(format!("Schema '{}' not found at {:?}", schema_name, schema_path));
        }

        let content = std::fs::read_to_string(&schema_path)
            .map_err(|e| format!("Failed to read schema {}: {}", schema_name, e))?;
        let schema_json: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON in schema {}: {}", schema_name, e))?;

        let compiled = jsonschema::JSONSchema::compile(&schema_json)
            .map_err(|e| format!("Schema compilation failed: {}", e))?;

        if let Err(errors) = compiled.validate(data) {
            let err_msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
            return Err(format!("Schema validation failed: {}", err_msgs.join("; ")));
        }

        Ok(())
    }

    /// Layer 2: Deterministic Evals & Guardrails
    pub fn run_eval(&self, eval_name: &str, payload: &Value) -> EvalResult {
        match eval_name {
            "slop_gate" => {
                let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let lower = text.to_lowercase();
                
                // Disallowed phrases
                let forbidden = [
                    "this is the part where",
                    "the part everyone's missing",
                    "the part everyone is missing",
                    "in today's fast-paced",
                    "game-changer",
                    "delve into",
                    "unlock the power",
                    "at the end of the day",
                    "it is important to note",
                    "in conclusion,",
                ];

                for p in forbidden {
                    if lower.contains(p) {
                        return EvalResult {
                            name: eval_name.to_string(),
                            passed: false,
                            reason: format!("Forbidden AI trope detected: '{}'", p),
                        };
                    }
                }

                // Check antithesis pattern: "not X, it's Y"
                let antithesis_re = Regex::new(r"(?i)\bnot\s+([a-z0-9_\-\s]+),\s*(it's|it\s+is)\s+").unwrap();
                if antithesis_re.is_match(&lower) {
                    return EvalResult {
                        name: eval_name.to_string(),
                        passed: false,
                        reason: "Formulaic antithesis trope detected ('not X, it's Y')".to_string(),
                    };
                }

                EvalResult {
                    name: eval_name.to_string(),
                    passed: true,
                    reason: "Slop gate passed clean.".to_string(),
                }
            }

            "required_evidence" => {
                let evidence = payload.get("evidence").and_then(|v| v.as_array());
                match evidence {
                    Some(arr) if !arr.is_empty() => EvalResult {
                        name: eval_name.to_string(),
                        passed: true,
                        reason: format!("Evidence present ({} item(s)).", arr.len()),
                    },
                    _ => EvalResult {
                        name: eval_name.to_string(),
                        passed: false,
                        reason: "Required evidence array is missing or empty.".to_string(),
                    },
                }
            }

            "reproduction_present" => {
                let repro = payload.get("reproduction_steps").and_then(|v| v.as_array());
                let failure = payload.get("failure_evidence").and_then(|v| v.as_str());
                if repro.map(|a| !a.is_empty()).unwrap_or(false) && failure.map(|s| !s.trim().is_empty()).unwrap_or(false) {
                    EvalResult {
                        name: eval_name.to_string(),
                        passed: true,
                        reason: "Reproduction steps and failure evidence verified.".to_string(),
                    }
                } else {
                    EvalResult {
                        name: eval_name.to_string(),
                        passed: false,
                        reason: "Reproduction steps and failure evidence must be non-empty.".to_string(),
                    }
                }
            }

            _ => EvalResult {
                name: eval_name.to_string(),
                passed: false,
                reason: format!("Unknown eval rule: '{}'", eval_name),
            },
        }
    }

    /// Layer 4: State Machine Workflow Engine
    pub fn advance_workflow(&self, wf: &mut WorkflowState, next_state: &str) -> Result<(), String> {
        let wf_def = self.workflows.get(&wf.workflow_type)
            .ok_or_else(|| format!("Unknown workflow type: '{}'", wf.workflow_type))?;

        let states = wf_def.get("states").and_then(|v| v.as_object())
            .ok_or_else(|| "Malformed workflow states definition".to_string())?;

        let current_def = states.get(&wf.current_state)
            .ok_or_else(|| format!("Current state '{}' not found in workflow", wf.current_state))?;

        let allowed_next: Vec<&str> = current_def.get("next")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str()).collect())
            .unwrap_or_default();

        if !allowed_next.contains(&next_state) {
            return Err(format!(
                "Invalid transition from '{}' to '{}'. Allowed: {:?}",
                wf.current_state, next_state, allowed_next
            ));
        }

        wf.history.push(wf.current_state.clone());
        wf.current_state = next_state.to_string();
        Ok(())
    }

    /// Layer 5: Failure Replay Verification
    pub fn record_failure(&self, task_id: &str, failure_type: &str, details: &Value) -> Result<PathBuf, String> {
        let failures_dir = self.base_path.join("failures");
        std::fs::create_dir_all(&failures_dir)
            .map_err(|e| format!("Failed to create failures dir: {}", e))?;

        let file_name = format!("failure_{}_{}.json", task_id, chrono::Utc::now().timestamp());
        let file_path = failures_dir.join(file_name);

        let record = json!({
            "task_id": task_id,
            "failure_type": failure_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "details": details,
        });

        std::fs::write(&file_path, serde_json::to_string_pretty(&record).unwrap())
            .map_err(|e| format!("Failed to write failure log: {}", e))?;

        Ok(file_path)
    }
}
