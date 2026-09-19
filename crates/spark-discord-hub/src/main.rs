use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod atlas_bot;
mod gateway;
mod gideon_bot;
mod models;
mod vault;

use gateway::DiscordGatewayHub;
use vault::get_vault_secret;

#[derive(Parser, Debug)]
#[command(name = "spark-discord-hub", about = "Sovereign Native Multi-Bot Discord Hub in Rust")]
struct Cli {
    #[arg(long)]
    token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    tracing::info!("Initializing SparkOS Discord Multi-Bot Hub...");

    // Dynamic resolution from TPM vault
    let token = cli.token
        .or_else(|| get_vault_secret("DISCORD_BOT_TOKEN"))
        .or_else(|| get_vault_secret("DISCORD_ATLAS_TOKEN"));

    let token = match token {
        Some(t) if !t.is_empty() => t,
        _ => {
            tracing::error!("No Discord bot token found in atlas-vault (DISCORD_BOT_TOKEN). Exiting.");
            std::process::exit(1);
        }
    };

    tracing::info!("Discord bot token dynamically resolved from TPM vault. Starting gateway hub...");

    let hub = DiscordGatewayHub::new(token);
    hub.run().await?;

    Ok(())
}
