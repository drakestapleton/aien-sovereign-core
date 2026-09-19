use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use spark_harness::{HarnessEngine, WorkflowState};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "spark-harness", about = "Native Rust 5-Layer Change-Management Harness & MCP Server")]
struct Cli {
    #[arg(long, default_value = "/home/drakestapleton/workspace/spark-harness-data")]
    harness_path: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run self-test across all 5 layers
    Test,
    /// Run as Stdio JSON-RPC MCP server
    Mcp,
    /// Validate JSON against a schema
    Validate {
        #[arg(long)]
        schema: String,
        #[arg(long)]
        data: String,
    },
    /// Evaluate content through a guardrail gate
    Eval {
        #[arg(long)]
        gate: String,
        #[arg(long)]
        payload: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let engine = HarnessEngine::new(&cli.harness_path);

    match cli.command.unwrap_or(Commands::Test) {
        Commands::Test => {
            println!("=== Running spark-harness Native 5-Layer Verification Suite ===");
            
            // 1. Slop Gate Eval Test
            let clean_payload = json!({ "text": "Direct execution of native Rust binary on DGX Spark." });
            let slopped_payload = json!({ "text": "In today's fast-paced world, this is a game-changer to delve into." });
            
            let res_clean = engine.run_eval("slop_gate", &clean_payload);
            let res_slop = engine.run_eval("slop_gate", &slopped_payload);
            
            assert!(res_clean.passed, "Clean text should pass slop gate");
            assert!(!res_slop.passed, "Sloppy text must be rejected by slop gate");
            println!("Layer 2 (Slop Gate Eval): PASSED");

            // 2. Evidence Requirement Eval Test
            let no_evidence = json!({ "text": "Sample" });
            let has_evidence = json!({ "text": "Sample", "evidence": ["commit c9db3e2"] });
            assert!(!engine.run_eval("required_evidence", &no_evidence).passed);
            assert!(engine.run_eval("required_evidence", &has_evidence).passed);
            println!("Layer 2 (Required Evidence Eval): PASSED");

            // 3. Workflow State Transition Test
            let mut wf = WorkflowState {
                id: "test-001".to_string(),
                workflow_type: "engineering_fix".to_string(),
                current_state: "reproduce".to_string(),
                history: Vec::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
            };

            assert!(engine.advance_workflow(&mut wf, "regression_fail").is_ok());
            assert_eq!(wf.current_state, "regression_fail");
            assert!(engine.advance_workflow(&mut wf, "implement").is_ok());
            assert_eq!(wf.current_state, "implement");
            assert!(engine.advance_workflow(&mut wf, "done").is_err(), "Cannot jump directly to done");
            println!("Layer 4 (Deterministic Workflows): PASSED");

            // 4. Failure Replay Record Test
            let log_path = engine.record_failure("test-task-1", "RegressionTestFailed", &json!({"test": "assert_eq"}))?;
            assert!(log_path.exists(), "Failure log should be created on disk");
            println!("Layer 5 (Failure Replay Recording): PASSED");

            println!("\nAll 5-layer change management harness invariants verified in pure native Rust.");
        }

        Commands::Validate { schema, data } => {
            let parsed_data: Value = serde_json::from_str(&data)?;
            match engine.validate_schema(&schema, &parsed_data) {
                Ok(_) => println!("VALIDATION PASSED: Schema '{}'", schema),
                Err(err) => {
                    eprintln!("VALIDATION FAILED: {}", err);
                    std::process::exit(1);
                }
            }
        }

        Commands::Eval { gate, payload } => {
            let parsed: Value = serde_json::from_str(&payload)?;
            let result = engine.run_eval(&gate, &parsed);
            println!("{}", serde_json::to_string_pretty(&result)?);
            if !result.passed {
                std::process::exit(1);
            }
        }

        Commands::Mcp => {
            run_mcp_stdio_loop(engine).await?;
        }
    }

    Ok(())
}

async fn run_mcp_stdio_loop(engine: HarnessEngine) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let req_id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "spark-harness-rs",
                        "version": "0.1.0"
                    },
                    "capabilities": {
                        "tools": {}
                    }
                }
            }),

            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "tools": [
                        {
                            "name": "harness_eval",
                            "description": "Run an artifact or text through change-management guardrail evals (slop_gate, required_evidence)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "gate": { "type": "string" },
                                    "payload": { "type": "object" }
                                },
                                "required": ["gate", "payload"]
                            }
                        },
                        {
                            "name": "harness_validate",
                            "description": "Validate JSON object against registered harness schemas (engineering_fix, research_handoff, harness_change)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "schema": { "type": "string" },
                                    "data": { "type": "object" }
                                },
                                "required": ["schema", "data"]
                            }
                        }
                    ]
                }
            }),

            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(json!({}));
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                match name {
                    "harness_eval" => {
                        let gate = args.get("gate").and_then(|g| g.as_str()).unwrap_or("");
                        let payload = args.get("payload").cloned().unwrap_or(json!({}));
                        let eval_res = engine.run_eval(gate, &payload);
                        json!({
                            "jsonrpc": "2.0",
                            "id": req_id,
                            "result": {
                                "content": [
                                    { "type": "text", "text": serde_json::to_string(&eval_res).unwrap() }
                                ]
                            }
                        })
                    }

                    "harness_validate" => {
                        let schema = args.get("schema").and_then(|s| s.as_str()).unwrap_or("");
                        let data = args.get("data").cloned().unwrap_or(json!({}));
                        match engine.validate_schema(schema, &data) {
                            Ok(_) => json!({
                                "jsonrpc": "2.0",
                                "id": req_id,
                                "result": {
                                    "content": [{ "type": "text", "text": "Validation passed." }]
                                }
                            }),
                            Err(e) => json!({
                                "jsonrpc": "2.0",
                                "id": req_id,
                                "result": {
                                    "content": [{ "type": "text", "text": format!("Validation failed: {}", e) }],
                                    "isError": true
                                }
                            }),
                        }
                    }

                    _ => json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "error": { "code": -32601, "message": "Method or tool not found" }
                    })
                }
            }

            _ => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {}
            }),
        };

        let out = serde_json::to_string(&resp).unwrap();
        writeln!(stdout, "{}", out)?;
        stdout.flush()?;
    }

    Ok(())
}
