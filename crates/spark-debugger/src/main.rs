use clap::{Parser, Subcommand};
use reqwest::Client;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod audit;
mod daemon;
mod mcp;
mod reporter;

use audit::run_full_audit;
use daemon::run_watchdog_daemon;
use mcp::run_mcp_server;

#[derive(Parser, Debug)]
#[command(name = "spark-debugger", about = "Autonomous Continuous Watchdog and MCP Debugger for SparkOS")]
struct Cli {
    #[arg(long, default_value = "/home/drakestapleton/workspace/aien-sovereign-core")]
    workspace: String,

    #[arg(long)]
    daemon: bool,

    #[arg(long)]
    mcp: bool,

    #[arg(long, default_value_t = 30)]
    interval: u64,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Check,
    Daemon,
    Mcp,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let client = Client::builder()
        .tcp_nodelay(true)
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    if cli.mcp || matches!(cli.command, Some(Commands::Mcp)) {
        // Run MCP server over stdio without standard logging interfering with stdout
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
        return run_mcp_server(client, cli.workspace).await;
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if cli.daemon || matches!(cli.command, Some(Commands::Daemon)) {
        return run_watchdog_daemon(client, cli.workspace, cli.interval).await;
    }

    // Default: one-shot check
    println!("");
    println!("=== SparkOS Sovereign Watchdog: System Diagnostic Audit ===");
    let report = run_full_audit(&client, &cli.workspace).await;

    println!("{:<22} {:<38} {:<10} {:<10}", "SERVICE", "ENDPOINT", "STATUS", "LATENCY");
    println!("{}", "-".repeat(82));
    for s in &report.services {
        let status_str = if s.reachable { "ONLINE" } else { "OFFLINE" };
        let code_str = s.status_code.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
        println!("{:<22} {:<38} {:<10} {:<10}",
            s.name, s.endpoint, format!("{} ({})", status_str, code_str), format!("{}ms", s.latency_ms));
    }
    println!("{}", "-".repeat(82));

    println!("
--- Invariant Checks ---");
    println!("TPM Vault Clean (Zero .env files): {}", if report.invariants.plaintext_env_files_found.is_empty() { "PASSED" } else { "FAILED" });
    if !report.invariants.plaintext_env_files_found.is_empty() {
        for f in &report.invariants.plaintext_env_files_found {
            println!("  Leaked file: {}", f);
        }
    }
    println!("Unslop Standard (Zero em/en dashes): {}", if report.invariants.unslop_violations.is_empty() { "PASSED" } else { "FAILED" });

    println!("
--- Workspace Compile Check ---");
    println!("Cargo Check: {}", if report.build.passed { "PASSED" } else { "FAILED" });
    if !report.build.errors.is_empty() {
        for e in &report.build.errors {
            println!("  {}", e);
        }
    }

    println!("
Overall Diagnostic Status: {}", if report.overall_healthy { "ALL SYSTEMS GREEN" } else { "ATTENTION REQUIRED" });
    println!("");

    Ok(())
}
