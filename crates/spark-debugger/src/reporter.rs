use reqwest::Client;
use rusqlite::params;
use serde_json::json;
use std::path::Path;
use tracing::{error, info};
use uuid::Uuid;

use crate::audit::SystemAuditReport;

const CORTEX_URL: &str = "http://127.0.0.1:18080";
const CORTEX_TOKEN_PATH: &str = "/home/drakestapleton/.config/cortex/token";
const HIVE_DB_PATH: &str = "/home/drakestapleton/.config/cortex/hive.db";

pub async fn report_incident(client: &Client, report: &SystemAuditReport, failure_summary: &str) {
    info!("[DebuggerReporter] Reporting incident: {}", failure_summary);

    // 1. Commit bug report to Cortex memory
    if let Ok(token) = std::fs::read_to_string(CORTEX_TOKEN_PATH) {
        let payload = json!({
            "space": "atlas-memory",
            "canonicalName": format!("incident-{}", Uuid::new_v4().to_string()[..8].to_string()),
            "entityType": "bug_report",
            "content": format!("Incident detected by spark-debugger: {}
Report Details: {:?}", failure_summary, report)
        });

        let write_url = format!("{}/api/cortex/write", CORTEX_URL);
        if let Err(e) = client.post(&write_url)
            .header("Authorization", format!("Bearer {}", token.trim()))
            .json(&payload)
            .send()
            .await
        {
            error!("[DebuggerReporter] Failed to commit bug report to Cortex: {}", e);
        } else {
            info!("[DebuggerReporter] Committed bug report to Cortex.");
        }
    }

    // 2. Emit alert comb onto Honeycomb lattice if hive.db exists
    if Path::new(HIVE_DB_PATH).exists() {
        if let Ok(conn) = rusqlite::Connection::open(HIVE_DB_PATH) {
            let comb_id = format!("comb-alert-{}", &Uuid::new_v4().to_string()[..8]);
            let now = chrono::Utc::now().to_rfc3339();
            let content = format!("🚨 AUDIT ALERT: {}", failure_summary);

            // Find free coordinate or default to outer ring
            let res = conn.execute(
                "INSERT INTO hive_combs (id, q, r, author, role, content, intent, created_at)
                 VALUES (?1, -2, 2, ?2, ?3, ?4, ?5, ?6)",
                params![comb_id, "spark-debugger", "watchdog", content, "branch", now],
            );
            if let Ok(_) = res {
                info!("[DebuggerReporter] Emitted alert comb to Honeycomb Wall.");
            }
        }
    }
}
