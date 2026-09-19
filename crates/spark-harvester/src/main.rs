use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use spark_harvester::{DatasetExporter, ExtractedPair, ReasoningExtractor};

#[derive(Parser)]
#[command(name = "spark-harvester")]
#[command(about = "Native Rust pipeline for harvesting commercial AI reasoning pairs for open weight training")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract reasoning pairs from a raw text file or query
    Extract {
        #[arg(short, long)]
        prompt: String,
        #[arg(short, long)]
        input: String,
        #[arg(short, long, default_value = "commercial-lab")]
        provider: String,
        #[arg(short, long, default_value = "frontier-model")]
        model: String,
    },
    /// Export extracted training pairs into JSONL
    Distill {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Display pipeline status
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();
    match cli.command {
        Commands::Extract { prompt, input, provider, model } => {
            let extractor = ReasoningExtractor::new();
            let pair = extractor.extract(&prompt, &input, &provider, &model);
            info!("Extracted pair: tokens={}, quality={:.2}", pair.token_count, pair.quality_score);
            println!("{}", serde_json::to_string_pretty(&pair)?);
        }
        Commands::Distill { output } => {
            info!("Writing distillation manifest to {:?}", output);
            let sample_pair = ExtractedPair {
                prompt: "Sovereign AI foundational premise".to_string(),
                reasoning: Some("Verify local computation vs centralized extraction".to_string()),
                completion: "Human freedom requires local ownership of intelligence.".to_string(),
                provider: "community".to_string(),
                model: "atlas-sovereign".to_string(),
                token_count: 42,
                quality_score: 1.0,
            };
            let count = DatasetExporter::append_to_jsonl(&output, &[sample_pair])?;
            info!("Appended {} pairs to {:?}", count, output);
        }
        Commands::Status => {
            println!("AIEN Sovereign Harvester Pipeline Active");
            println!("Zero Disk Secrets: Enforced via hardware TPM vault (atlas-vault)");
            println!("License: PolyForm Noncommercial 1.0.0 with Sovereign Defense Covenant");
        }
    }
    Ok(())
}
