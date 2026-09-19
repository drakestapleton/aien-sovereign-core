use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod proc_telemetry;
mod supervisor;

use config::SupervisorConfig;
use supervisor::Supervisor;

#[derive(Parser, Debug)]
#[command(name = "spark-supervisor", about = "Sovereign Native Process Supervisor for SparkOS")]
struct Cli {
    #[arg(long, default_value = "/home/drakestapleton/.config/aien/supervisor.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Run,
    Status,
    Check,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if !cli.config.exists() {
        eprintln!("Config file not found at {:?}. Generating default supervisor.toml...", cli.config);
        if let Some(p) = cli.config.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let default_toml = "[services.cortex]\ncommand = \"/home/drakestapleton/.local/bin/cortex-rs\"\nargs = [\"--port\", \"18080\"]\nrestart = \"always\"\nhealth_url = \"http://127.0.0.1:18080/health\"\n\n[services.cockpit]\ncommand = \"/home/drakestapleton/.local/bin/spark-cockpit-rs\"\nrestart = \"always\"\nhealth_url = \"http://127.0.0.1:18095/health\"\n";
        std::fs::write(&cli.config, default_toml)?;
    }

    let config = SupervisorConfig::load_from_file(&cli.config)?;
    let supervisor = Supervisor::new(config);

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            supervisor.run_daemon().await?;
        }
        Commands::Status => {
            supervisor.print_status().await;
        }
        Commands::Check => {
            println!("Configuration {:?} is valid.", cli.config);
        }
    }

    Ok(())
}
