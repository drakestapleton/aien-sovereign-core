use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::audit::run_full_audit;
use crate::reporter::report_incident;

pub async fn run_watchdog_daemon(
    client: Client,
    workspace_dir: String,
    interval_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("[Watchdog] Starting spark-debugger watchdog daemon (interval: {}s)...", interval_secs);

    let mut consecutive_failures = 0;

    loop {
        let report = run_full_audit(&client, &workspace_dir).await;

        if !report.overall_healthy {
            consecutive_failures += 1;
            let mut issues = Vec::new();
            for s in &report.services {
                if !s.reachable {
                    issues.push(format!("Service {} unreachable at {}", s.name, s.endpoint));
                }
            }
            if !report.invariants.passed {
                issues.push("Vault or unslop invariants violated".to_string());
            }
            if !report.build.passed {
                issues.push(format!("Workspace build failed with {} errors", report.build.errors.len()));
            }

            let summary = issues.join("; ");
            warn!("[Watchdog] Audit failed (attempt {}): {}", consecutive_failures, summary);

            if consecutive_failures == 1 || consecutive_failures % 5 == 0 {
                report_incident(&client, &report, &summary).await;
            }
        } else {
            if consecutive_failures > 0 {
                info!("[Watchdog] All systems recovered to healthy state.");
            }
            consecutive_failures = 0;
            info!("[Watchdog] All 5 services and workspace invariants OK.");
        }

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("[Watchdog] Received termination signal. Halting daemon.");
                break;
            }
            _ = sleep(Duration::from_secs(interval_secs)) => {}
        }
    }

    Ok(())
}
