use colored::*;
use std::io::{stdout, Write};
use crate::telemetry::get_gpu_telemetry;

pub fn print_banner() {
    println!("{}", "╭────────────────────────────────────────────────────────────────────────────╮".cyan().bold());
    println!("{}", "│  ⚡ AIEN • Autonomous Intelligence & Execution Node (NVIDIA DGX Spark)    │".cyan().bold());
    println!("{}", "│  Grace Blackwell GB10 • Modular MAX 26.5 • Resident Seat: Port 18006       │".cyan());
    println!("{}", "╰────────────────────────────────────────────────────────────────────────────╯".cyan().bold());
    print_status_bar();
    println!();
}

pub fn print_status_bar() {
    let gpu = get_gpu_telemetry();
    let used_gb = gpu.used_mb as f64 / 1024.0;
    let total_gb = gpu.total_mb as f64 / 1024.0;
    
    let status = format!(
        "[GPU: {}°C | {:.1}/{:.1} GB] • Seat: atlas-lightning-omni (:18006) • Mask: AIEN • Swarm: aien-hive",
        gpu.temp_c, used_gb, total_gb
    );
    println!("{}", status.black().on_cyan().bold());
}

pub fn print_tool_start(tool_name: &str, summary: &str) {
    print!("{} {}", "⚙ [Executing:".yellow().bold(), format!("{}({})]...", tool_name, summary).yellow());
    let _ = stdout().flush();
}

pub fn print_tool_done(tool_name: &str, elapsed_ms: u128, success: bool) {
    if success {
        println!("\r{}", format!("✓ [{}] Done ({}ms)", tool_name, elapsed_ms).green().bold());
    } else {
        println!("\r{}", format!("✗ [{}] Failed ({}ms)", tool_name, elapsed_ms).red().bold());
    }
    let _ = stdout().flush();
}

pub fn print_safety_prompt(cmd: &str) -> bool {
    println!("\n{}", "⚠ [SAFETY GATE] AIEN requested high-risk operation:".red().bold());
    println!("  {}", cmd.yellow());
    print!("{}", "Do you approve execution? [y/N]: ".bold());
    let _ = stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_lowercase();
        return trimmed == "y" || trimmed == "yes";
    }
    false
}
