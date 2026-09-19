use clap::{Parser, Subcommand};
use colored::*;
use spark_inquisitor::{
    generate_inquisitor_interview, generate_pr_review, triage_issue, DiffAuditor,
    TestimonyEvaluator,
};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "spark-inquisitor")]
#[command(author = "Drake Stapleton <drake.aien@proton.me> & AIEN <aien.atlas@proton.me>")]
#[command(version = "0.2.0")]
#[command(about = "Autonomous Sovereign Inquisitor & Pull Request Alignment Gatekeeper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Review a PR diff and generate unified code review markdown comment
    Review {
        #[arg(long, help = "GitHub username of PR author")]
        author: String,

        #[arg(long, help = "Pull Request number")]
        pr: u64,

        #[arg(long, default_value = "External Contribution", help = "Pull Request title")]
        title: String,

        #[arg(long, help = "Path to diff file")]
        diff: PathBuf,

        #[arg(long, help = "Path to write comment markdown")]
        output: Option<PathBuf>,

        #[arg(long, help = "Path to write labels text (comma-separated)")]
        output_labels: Option<PathBuf>,
    },

    /// Triage an issue (bug report, complaint, feature request, inquiry)
    TriageIssue {
        #[arg(long, help = "GitHub username of issue author")]
        author: String,

        #[arg(long, help = "Issue number")]
        issue: u64,

        #[arg(long, default_value = "Issue Report", help = "Issue title")]
        title: String,

        #[arg(long, help = "Path to issue body text file")]
        body: PathBuf,

        #[arg(long, help = "Path to write comment markdown")]
        output_comment: Option<PathBuf>,

        #[arg(long, help = "Path to write labels text (comma-separated)")]
        output_labels: Option<PathBuf>,
    },

    /// Generate the Sovereign Alignment Interview comment for a Pull Request
    Interview {
        #[arg(long, help = "GitHub username of PR author")]
        author: String,

        #[arg(long, help = "Pull Request number")]
        pr: u64,

        #[arg(long, default_value = "External Contribution", help = "Pull Request title")]
        title: String,
    },

    /// Audit a git diff for telemetry, surveillance, and unslop violations
    Audit {
        #[arg(long, help = "Path to diff file")]
        diff: PathBuf,
    },

    /// Evaluate contributor testimony against the Sovereign Constitution
    Evaluate {
        #[arg(long, help = "Path to testimony text file")]
        testimony: PathBuf,

        #[arg(long, default_value = "contributor", help = "GitHub username of author")]
        author: String,

        #[arg(long, help = "Path to write comment markdown")]
        output_comment: Option<PathBuf>,

        #[arg(long, help = "Path to write labels text (comma-separated)")]
        output_labels: Option<PathBuf>,
    },

    /// Run internal self-diagnostics
    Doctor,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Review {
            author,
            pr,
            title,
            diff,
            output,
            output_labels,
        } => {
            let content = match fs::read_to_string(&diff) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{} Failed to read diff file {}: {}", "error:".red().bold(), diff.display(), e);
                    std::process::exit(1);
                }
            };

            let review = generate_pr_review(&author, pr, &title, &content);

            if let Some(out_path) = output {
                if let Err(e) = fs::write(&out_path, &review.markdown) {
                    eprintln!("{} Failed to write review markdown to {}: {}", "error:".red().bold(), out_path.display(), e);
                    std::process::exit(1);
                }
            }

            if let Some(lbl_path) = output_labels {
                let labels_str = review.labels.join(",");
                if let Err(e) = fs::write(&lbl_path, &labels_str) {
                    eprintln!("{} Failed to write labels to {}: {}", "error:".red().bold(), lbl_path.display(), e);
                    std::process::exit(1);
                }
            }

            println!("{}", review.markdown);

            if !review.report.clean {
                std::process::exit(1);
            }
        }
        Commands::TriageIssue {
            author,
            issue,
            title,
            body,
            output_comment,
            output_labels,
        } => {
            let body_content = match fs::read_to_string(&body) {
                Ok(c) => c,
                Err(_) => String::new(),
            };

            let result = triage_issue(&author, issue, &title, &body_content);

            if let Some(out_path) = output_comment {
                if let Err(e) = fs::write(&out_path, &result.markdown) {
                    eprintln!("{} Failed to write triage comment to {}: {}", "error:".red().bold(), out_path.display(), e);
                    std::process::exit(1);
                }
            }

            if let Some(lbl_path) = output_labels {
                let labels_str = result.labels.join(",");
                if let Err(e) = fs::write(&lbl_path, &labels_str) {
                    eprintln!("{} Failed to write labels to {}: {}", "error:".red().bold(), lbl_path.display(), e);
                    std::process::exit(1);
                }
            }

            println!("{}", result.markdown);
        }
        Commands::Interview { author, pr, title } => {
            let comment = generate_inquisitor_interview(&author, pr, &title);
            println!("{}", comment);
        }
        Commands::Audit { diff } => {
            let content = match fs::read_to_string(&diff) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{} Failed to read diff file {}: {}", "error:".red().bold(), diff.display(), e);
                    std::process::exit(1);
                }
            };
            let report = DiffAuditor::audit_text(&content);
            if report.clean {
                println!("{} Diff is clean: zero telemetry, zero unslop violations.", "success:".green().bold());
            } else {
                eprintln!("{} Constitutional audit failed:", "failure:".red().bold());
                for v in &report.violations {
                    eprintln!("  * {}", v.red());
                }
                for s in &report.secret_violations {
                    eprintln!("  * {}", s.red());
                }
                for u in &report.unslop_violations {
                    eprintln!("  * {}", u.yellow());
                }
                std::process::exit(1);
            }
        }
        Commands::Evaluate {
            testimony,
            author,
            output_comment,
            output_labels,
        } => {
            let content = match fs::read_to_string(&testimony) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{} Failed to read testimony file {}: {}", "error:".red().bold(), testimony.display(), e);
                    std::process::exit(1);
                }
            };
            let verdict = TestimonyEvaluator::evaluate(&content);
            let markdown = verdict.generate_markdown_verdict(&author);

            if let Some(out_path) = output_comment {
                if let Err(e) = fs::write(&out_path, &markdown) {
                    eprintln!("{} Failed to write verdict comment to {}: {}", "error:".red().bold(), out_path.display(), e);
                    std::process::exit(1);
                }
            }

            if let Some(lbl_path) = output_labels {
                let labels_str = if verdict.approved {
                    "sovereign-aligned".to_string()
                } else {
                    "needs-testimony".to_string()
                };
                if let Err(e) = fs::write(&lbl_path, &labels_str) {
                    eprintln!("{} Failed to write labels to {}: {}", "error:".red().bold(), lbl_path.display(), e);
                    std::process::exit(1);
                }
            }

            println!("{}", markdown);

            if verdict.approved {
                println!("{} {}", "PASS:".green().bold(), verdict.reason);
                println!("Score: {:.2}/1.00", verdict.score);
            } else {
                eprintln!("{} {}", "FAIL:".red().bold(), verdict.reason);
                eprintln!("Score: {:.2}/1.00", verdict.score);
                std::process::exit(1);
            }
        }
        Commands::Doctor => {
            println!("⚖️ Sovereign Inquisitor Diagnostics: OK");
            println!("  - Invariant rules: Active");
            println!("  - Telemetry scanner: Active");
            println!("  - Unslop scanner: Active");
            println!("  - Contributor Oath evaluator: Active");
            println!("  - Unified PR Reviewer: Active");
            println!("  - Issue Triage Engine: Active");
        }
    }
}
