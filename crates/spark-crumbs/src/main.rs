use clap::{Parser, Subcommand};
use spark_crumbs::{add_crumb_whisper, format_dir_crumb_tui, record_crumb_action, seed_directory_tree, sniff_crumb};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "spark-crumbs")]
#[command(author = "AIEN <aien.atlas@proton.me>")]
#[command(version = "0.1.0")]
#[command(about = "High-speed native Rust distributed breadcrumb and agent scent engine for Sovereign SparkOS")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Seed .crumb and .crumb.local in a directory or recursively across a repository
    Seed {
        #[arg(default_value = ".", help = "Root directory to seed")]
        dir: PathBuf,

        #[arg(short, long, help = "Recursively seed all subdirectories")]
        recursive: bool,

        #[arg(short, long, default_value = "AIEN", help = "Agent ID creating the crumb")]
        agent: String,

        #[arg(short, long, help = "Initial whisper message to plant")]
        whisper: Option<String>,
    },

    /// Sniff peer agent scent, recent history, and active whispers for a file or directory
    Sniff {
        #[arg(help = "Target file or directory path")]
        path: PathBuf,

        #[arg(short, long, default_value = "AIEN", help = "Current agent ID")]
        agent: String,
    },

    /// Show rich topographic inspection card for a directory
    Show {
        #[arg(default_value = ".", help = "Target directory to inspect")]
        dir: PathBuf,
    },

    /// Plant an inter-agent whisper message in a directory
    Whisper {
        #[arg(default_value = ".", help = "Target directory")]
        dir: PathBuf,

        #[arg(short, long, help = "Whisper message content")]
        message: String,

        #[arg(short, long, default_value = "AIEN", help = "Agent ID whispering")]
        agent: String,

        #[arg(short, long, help = "Optional target file name")]
        file: Option<String>,
    },

    /// Record an agent action vector in a directory crumb
    Record {
        #[arg(default_value = ".", help = "Target directory")]
        dir: PathBuf,

        #[arg(short, long, default_value = "AIEN", help = "Agent ID")]
        agent: String,

        #[arg(short, long, help = "Action type (e.g. edit, test, build, audit)")]
        action: String,

        #[arg(short, long, help = "Target artifact or file")]
        target: String,

        #[arg(short, long, help = "Operational intent")]
        intent: String,

        #[arg(short, long, help = "Vector description")]
        vector: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Seed { dir, recursive, agent, whisper } => {
            println!("Seeding directory crumbs at: {} (recursive: {})", dir.display(), recursive);
            let count = seed_directory_tree(&dir, recursive, &agent, whisper.as_deref());
            println!("Seeded {} directory crumbs successfully.", count);
        }
        Commands::Sniff { path, agent } => {
            if let Some(sniff) = sniff_crumb(&path, &agent) {
                println!("{}", sniff);
            } else {
                println!("No active peer scent or whispers detected on '{}'.", path.display());
            }
        }
        Commands::Show { dir } => {
            let card = format_dir_crumb_tui(&dir);
            println!("{}", card);
        }
        Commands::Whisper { dir, message, agent, file } => {
            add_crumb_whisper(&dir, &agent, &message, file.as_deref());
            println!("Planted whisper from [{}] in {}: '{}'", agent, dir.display(), message);
        }
        Commands::Record { dir, agent, action, target, intent, vector } => {
            record_crumb_action(&dir, &agent, "cli-session", &action, &target, &intent, &vector);
            println!("Recorded action '{}' on '{}' in {}.", action, target, dir.display());
        }
    }
}
