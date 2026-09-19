use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAuditResult {
    pub name: String,
    pub endpoint: String,
    pub reachable: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantAuditResult {
    pub passed: bool,
    pub plaintext_env_files_found: Vec<String>,
    pub unredacted_secrets_found: usize,
    pub unslop_violations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildAuditResult {
    pub passed: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAuditReport {
    pub timestamp: String,
    pub services: Vec<ServiceAuditResult>,
    pub invariants: InvariantAuditResult,
    pub build: BuildAuditResult,
    pub overall_healthy: bool,
}

pub async fn audit_services(client: &Client) -> Vec<ServiceAuditResult> {
    let endpoints = [
        ("Cortex Memory", "http://127.0.0.1:18080/health"),
        ("Sovereign Cockpit", "http://127.0.0.1:18095/api/pulse"),
        ("Cortex ONNX Encoder", "http://127.0.0.1:18081/health"),
        ("Modular MAX LLM", "http://127.0.0.1:18006/v1/models"),
        ("Matrix Conduit", "http://127.0.0.1:6167"),
    ];

    let mut results = Vec::new();
    for (name, ep) in endpoints {
        let start = std::time::Instant::now();
        match client.get(ep).timeout(Duration::from_millis(1500)).send().await {
            Ok(res) => {
                let status = res.status().as_u16();
                results.push(ServiceAuditResult {
                    name: name.to_string(),
                    endpoint: ep.to_string(),
                    reachable: status < 500,
                    status_code: Some(status),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            }
            Err(_) => {
                results.push(ServiceAuditResult {
                    name: name.to_string(),
                    endpoint: ep.to_string(),
                    reachable: false,
                    status_code: None,
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            }
        }
    }
    results
}

pub fn audit_vault_and_unslop(workspace_paths: &[&str]) -> InvariantAuditResult {
    let mut plaintext_env = Vec::new();
    let unslop_violations = Vec::new();

    for base in workspace_paths {
        let p = Path::new(base);
        if !p.exists() {
            continue;
        }

        // Scan for .env files
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(".env") {
                    plaintext_env.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }

    let passed = plaintext_env.is_empty() && unslop_violations.is_empty();
    InvariantAuditResult {
        passed,
        plaintext_env_files_found: plaintext_env,
        unredacted_secrets_found: 0,
        unslop_violations,
    }
}

pub async fn audit_workspace_build(workspace_path: &str) -> BuildAuditResult {
    let output_res = Command::new("cargo")
        .arg("check")
        .arg("--workspace")
        .current_dir(workspace_path)
        .output()
        .await;

    match output_res {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut errors = Vec::new();
            let mut warnings = Vec::new();

            for line in stderr.lines() {
                if line.starts_with("error") {
                    errors.push(line.to_string());
                } else if line.starts_with("warning") && !line.contains("profiles for the non root") {
                    warnings.push(line.to_string());
                }
            }

            BuildAuditResult {
                passed: out.status.success(),
                errors,
                warnings,
            }
        }
        Err(e) => BuildAuditResult {
            passed: false,
            errors: vec![format!("Failed to run cargo check: {}", e)],
            warnings: Vec::new(),
        },
    }
}

pub async fn run_full_audit(client: &Client, workspace_dir: &str) -> SystemAuditReport {
    let services = audit_services(client).await;
    let invariants = audit_vault_and_unslop(&[workspace_dir, "/home/drakestapleton/spark-cockpit-rs"]);
    let build = audit_workspace_build(workspace_dir).await;

    let services_ok = services.iter().all(|s| s.reachable);
    let overall_healthy = services_ok && invariants.passed && build.passed;

    SystemAuditReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        services,
        invariants,
        build,
        overall_healthy,
    }
}
