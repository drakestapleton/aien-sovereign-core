use clap::Parser;
use spark_adapters::models::{DistillTask, TaskType, VerificationStrategy};
use spark_adapters::DistillationEngine;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "spark-distill")]
#[command(author = "AIEN <aien.atlas@proton.me>")]
#[command(version = "0.1.0")]
#[command(about = "Sovereign Teacher Distillation and Training Pair Generator for Open-Weight Models")]
struct Args {
    #[arg(short, long, help = "Task prompt or reasoning challenge to evaluate")]
    prompt: String,

    #[arg(short, long, default_value = "anthropic/claude-3-7-sonnet", help = "Teacher model identifier")]
    teacher: String,

    #[arg(short, long, default_value = "atlas-lightning-omni", help = "Student open-weight model seat")]
    student: String,

    #[arg(long, default_value = "code", help = "Task type: code, arch, reasoning, refactor, harness")]
    task_type: String,

    #[arg(long, default_value = "compiler", help = "Verification strategy: compiler, unslop, json, consensus")]
    verify: String,

    #[arg(long, help = "Commit high-scoring verified solution to Cortex memory")]
    commit_cortex: bool,

    #[arg(long, help = "Custom dataset directory for SFT/DPO output")]
    dataset_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let task_type = match args.task_type.to_lowercase().as_str() {
        "arch" => TaskType::SystemArchitecture,
        "reasoning" => TaskType::ReasoningTrace,
        "refactor" => TaskType::RefactorLogic,
        "harness" => TaskType::VerificationHarness,
        _ => TaskType::CodeSynthesis,
    };

    let strategy = match args.verify.to_lowercase().as_str() {
        "unslop" => VerificationStrategy::UnslopStrict,
        "json" => VerificationStrategy::JsonSchema,
        "consensus" => VerificationStrategy::DualConsensus,
        _ => VerificationStrategy::CompilerCheck,
    };

    let task = DistillTask {
        id: Uuid::new_v4().to_string(),
        task_type,
        prompt: args.prompt.clone(),
        system_prompt: Some("You are a verified sovereign systems specialist. Output clear, compilable native code with zero unslop.".to_string()),
        teacher_model: args.teacher.clone(),
        student_model: Some(args.student.clone()),
        verification_strategy: strategy,
        commit_to_cortex: args.commit_cortex,
    };

    println!("=== Sovereign Knowledge Distillation ===");
    println!("Task ID:  {}", task.id);
    println!("Teacher:  {}", task.teacher_model);
    println!("Student:  {}", task.student_model.as_deref().unwrap_or("<none>"));
    println!("Prompt:   {}", task.prompt);

    let mut engine = DistillationEngine::new();
    if let Some(dir) = args.dataset_dir {
        engine = engine.with_dataset_dir(dir);
    }

    match engine.distill(task).await {
        Ok(record) => {
            println!("\n=== Distillation Complete ===");
            println!("Teacher Verification: Passed: {}, Score: {:.2}", 
                record.teacher_rollout.verification.passed,
                record.teacher_rollout.verification.score
            );
            if let Some(ref s_roll) = record.student_rollout {
                println!("Student Verification: Passed: {}, Score: {:.2}", 
                    s_roll.verification.passed,
                    s_roll.verification.score
                );
                println!("Preference Delta:     {:.2}", record.preference_delta);
            }
            if record.durable_memory_committed {
                println!("Cortex Entity:        {}", record.cortex_entity_id.as_deref().unwrap_or("<committed>"));
            }
            println!("Datasets Appended:    {}/sft.jsonl, {}/dpo.jsonl", 
                engine.dataset_dir.display(),
                engine.dataset_dir.display()
            );
        }
        Err(e) => {
            eprintln!("Distillation error: {}", e);
            std::process::exit(1);
        }
    }
}
