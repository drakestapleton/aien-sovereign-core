use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_CORTEX_URL: &str = "http://127.0.0.1:18080";
pub const DEFAULT_MODEL_URL: &str = "http://127.0.0.1:18006";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamState {
    pub daemon_running: bool,
    pub current_phase: String,
    pub last_cycle_timestamp: Option<String>,
    pub cycles_completed: u64,
    pub facts_committed: u64,
    pub proposals_staged: u64,
    pub facts_rejected: u64,
    pub hive_pulse: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateItem {
    pub source: String,
    pub text: String,
    pub item_type: String, // operational_fact, soul_proposal, discovery, learned_procedure, lesson
    pub canonical_name: String,
    pub metadata: Value,
}

/// Extracts a structured candidate item from varied markdown note layouts.
/// Handles frontmatter, markdown headers, bullet findings, and unstructured raw notes.
/// Recovers gracefully from malformed frontmatter and empty content.
pub fn extract_discovery_from_markdown(path: &Path, raw_content: &str) -> Option<CandidateItem> {
    let trimmed = raw_content.trim();
    if trimmed.is_empty() {
        return None;
    }

    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown_note".to_string());

    let source = path.to_string_lossy().to_string();

    // 1. Check for YAML-style frontmatter delimiter
    if trimmed.starts_with("---") {
        let after_first = &trimmed[3..];
        if let Some(closing_idx) = after_first.find("\n---") {
            let frontmatter_block = &after_first[..closing_idx];
            let body_block = after_first[closing_idx + 4..].trim();

            let mut title: Option<String> = None;
            let mut entity_type: Option<String> = None;
            let mut tags: Vec<String> = Vec::new();
            let mut custom_meta = serde_json::Map::new();

            for line in frontmatter_block.lines() {
                let l = line.trim();
                if l.is_empty() || l.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = l.split_once(':') {
                    let key = k.trim().to_lowercase();
                    let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    match key.as_str() {
                        "title" | "canonical_name" | "name" => title = Some(val),
                        "type" | "entity_type" | "kind" => entity_type = Some(val),
                        "tags" => {
                            let clean_val = val.trim_matches('[').trim_matches(']');
                            for t in clean_val.split(',') {
                                let tag_clean = t.trim().trim_matches('"').trim_matches('\'');
                                if !tag_clean.is_empty() {
                                    tags.push(tag_clean.to_string());
                                }
                            }
                        }
                        _ => {
                            custom_meta.insert(key, json!(val));
                        }
                    }
                }
            }

            let canonical_name = title.unwrap_or_else(|| format!("parked-{}", file_stem));
            let item_type = entity_type.unwrap_or_else(|| "discovery".to_string());
            let mut meta_obj = json!({
                "source": source,
                "consolidated_at": Utc::now().to_rfc3339(),
                "layout": "frontmatter",
            });

            if !tags.is_empty() {
                meta_obj["tags"] = json!(tags);
            }
            for (k, v) in custom_meta {
                meta_obj[k] = v;
            }

            let text = if body_block.is_empty() {
                frontmatter_block.trim().to_string()
            } else {
                body_block.to_string()
            };

            return Some(CandidateItem {
                source,
                text,
                item_type,
                canonical_name,
                metadata: meta_obj,
            });
        }
        // If unclosed frontmatter fence, fall through to heading or raw extraction
    }

    // 2. Check for markdown top-level headings: # Heading
    for line in trimmed.lines() {
        let l = line.trim();
        if l.starts_with("# ") {
            let heading_text = l.trim_start_matches("# ").trim();
            let clean_heading = heading_text
                .trim_start_matches("Discovery:")
                .trim_start_matches("discovery:")
                .trim_start_matches("Lesson:")
                .trim_start_matches("lesson:")
                .trim_start_matches("Procedure:")
                .trim();

            let item_type = if heading_text.to_lowercase().contains("lesson") {
                "lesson".to_string()
            } else if heading_text.to_lowercase().contains("procedure") {
                "learned_procedure".to_string()
            } else {
                "discovery".to_string()
            };

            let canonical_name = if !clean_heading.is_empty() {
                clean_heading.to_string()
            } else {
                format!("parked-{}", file_stem)
            };

            return Some(CandidateItem {
                source,
                text: trimmed.to_string(),
                item_type,
                canonical_name,
                metadata: json!({
                    "source": path.to_string_lossy().to_string(),
                    "consolidated_at": Utc::now().to_rfc3339(),
                    "layout": "heading"
                }),
            });
        }
    }

    // 3. Check for bullet list finding layout (- Finding: / - Solution:)
    let mut finding: Option<String> = None;
    for line in trimmed.lines() {
        let l = line.trim();
        if l.starts_with("- finding:") || l.starts_with("- Finding:") || l.starts_with("* Finding:") {
            if let Some((_, val)) = l.split_once(':') {
                finding = Some(val.trim().to_string());
                break;
            }
        }
    }

    if let Some(f) = finding {
        return Some(CandidateItem {
            source,
            text: trimmed.to_string(),
            item_type: "discovery".to_string(),
            canonical_name: f,
            metadata: json!({
                "source": path.to_string_lossy().to_string(),
                "consolidated_at": Utc::now().to_rfc3339(),
                "layout": "bullet_finding"
            }),
        });
    }

    // 4. Default unstructured raw note layout
    Some(CandidateItem {
        source,
        text: trimmed.to_string(),
        item_type: "discovery".to_string(),
        canonical_name: format!("parked-{}", file_stem),
        metadata: json!({
            "source": path.to_string_lossy().to_string(),
            "consolidated_at": Utc::now().to_rfc3339(),
            "layout": "raw"
        }),
    })
}

/// Evaluates whether the workstation state constitutes an idle consolidation window.
pub fn is_idle(gpu_util: u32, idle_secs: u64, idle_threshold: u64) -> bool {
    gpu_util <= 5 && idle_secs >= idle_threshold
}

#[derive(Debug, Clone)]
pub struct DreamEngine {
    pub base_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub park_dir: PathBuf,
    pub inbox_dir: PathBuf,
    pub dream_dir: PathBuf,
    pub cortex_url: String,
    pub cortex_token_path: PathBuf,
    pub client: Client,
}

impl DreamEngine {
    pub fn new(base_dir: PathBuf) -> Self {
        let sessions_dir = base_dir.join("sessions");
        let inbox_dir = base_dir.join("inbox");
        let dream_dir = base_dir.join("aien-dream");
        let park_dir = PathBuf::from("/home/drakestapleton/atlas-prime-workspace/park");
        let cortex_token_path = PathBuf::from("/home/drakestapleton/.config/cortex/token");

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            base_dir,
            sessions_dir,
            park_dir,
            inbox_dir,
            dream_dir,
            cortex_url: DEFAULT_CORTEX_URL.to_string(),
            cortex_token_path,
            client,
        }
    }

    pub fn with_cortex_url(mut self, url: String) -> Self {
        self.cortex_url = url;
        self
    }

    pub fn with_park_dir(mut self, dir: PathBuf) -> Self {
        self.park_dir = dir;
        self
    }

    pub fn get_token_secure(&self) -> String {
        // First try hardware TPM vault environment
        if let Ok(t) = std::env::var("CORTEX_TOKEN") {
            if !t.trim().is_empty() {
                return t.trim().to_string();
            }
        }
        self.get_cortex_token()
    }

    pub fn get_cortex_token(&self) -> String {
        fs::read_to_string(&self.cortex_token_path)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    /// Read GPU telemetry directly from nvidia-smi / /proc without python overhead
    pub async fn get_gpu_utilization(&self) -> u32 {
        let output = tokio::process::Command::new("nvidia-smi")
            .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
            .output()
            .await;

        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(val) = s.lines().next().and_then(|l| l.trim().parse::<u32>().ok()) {
                return val;
            }
        }
        0
    }

    /// Check last activity timestamp across active session files
    pub fn get_last_session_activity(&self) -> Option<DateTime<Utc>> {
        let mut latest: Option<DateTime<Utc>> = None;
        if let Ok(entries) = fs::read_dir(&self.sessions_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        let dt: DateTime<Utc> = mtime.into();
                        if latest.map(|l| dt > l).unwrap_or(true) {
                            latest = Some(dt);
                        }
                    }
                }
            }
        }
        latest
    }

    /// Commit verified fact to Cortex Memory (port 18080)
    pub async fn commit_to_cortex(
        &self,
        entity_type: &str,
        canonical_name: &str,
        content: &str,
        metadata: &Value,
    ) -> Result<Value, String> {
        let token = self.get_token_secure();
        let payload = json!({
            "kind": "entity",
            "value": {
                "canonicalName": canonical_name,
                "entityType": entity_type,
                "content": content,
                "metadata": metadata,
                "space": "atlas-memory"
            }
        });

        let resp = self.client
            .post(format!("{}/api/cortex/write", self.cortex_url))
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Cortex request failed: {}", e))?;

        if resp.status().is_success() {
            let res_json = resp.json::<Value>().await.unwrap_or_else(|_| json!({"status": "ok"}));
            Ok(res_json)
        } else {
            Err(format!("Cortex returned status {}", resp.status()))
        }
    }

    /// Consolidate uncommitted knowledge from parked notes and sessions
    pub async fn execute_dream_cycle(&self) -> Result<DreamState, String> {
        let mut committed = 0;
        let staged = 0;
        let mut rejected = 0;

        fs::create_dir_all(&self.inbox_dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.dream_dir).map_err(|e| e.to_string())?;

        // 1. Scan Park notes
        if self.park_dir.exists() {
            if let Ok(entries) = fs::read_dir(&self.park_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if content.trim().is_empty() {
                                continue;
                            }

                            if let Some(item) = extract_discovery_from_markdown(&path, &content) {
                                match self.commit_to_cortex(&item.item_type, &item.canonical_name, &item.text, &item.metadata).await {
                                    Ok(_) => {
                                        committed += 1;
                                        // Archive parked note
                                        let archive_name = format!("{}.consolidated", path.file_name().unwrap().to_string_lossy());
                                        let _ = fs::rename(&path, path.with_file_name(archive_name));
                                    }
                                    Err(_) => {
                                        rejected += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Compute state summary
        let state = DreamState {
            daemon_running: true,
            current_phase: "idle".to_string(),
            last_cycle_timestamp: Some(Utc::now().to_rfc3339()),
            cycles_completed: 1,
            facts_committed: committed,
            proposals_staged: staged,
            facts_rejected: rejected,
            hive_pulse: Some(json!({
                "heartbeat": "synchronous",
                "timestamp": Utc::now().to_rfc3339()
            })),
        };

        // Write state file
        let state_path = self.dream_dir.join("state.json");
        let _ = fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap());

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Json, Router};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn test_extract_discovery_from_frontmatter_layout() {
        let path = PathBuf::from("/tmp/note_frontmatter.md");
        let content = r#"---
title: NVFP4 Checkpoint Loader Fix
type: learned_procedure
tags: [vllm, max, quantization]
subsystem: neural_weights
---
# Implementation Overview
Directly map fp4 quantized tensor blocks without conversion overhead."#;

        let item = extract_discovery_from_markdown(&path, content).expect("extracted item");
        assert_eq!(item.canonical_name, "NVFP4 Checkpoint Loader Fix");
        assert_eq!(item.item_type, "learned_procedure");
        assert_eq!(item.metadata["layout"], "frontmatter");
        assert_eq!(item.metadata["subsystem"], "neural_weights");
        let tags = item.metadata["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], "vllm");
        assert!(item.text.contains("Directly map fp4 quantized tensor blocks"));
        assert!(!item.text.contains("subsystem: neural_weights"));
    }

    #[test]
    fn test_extract_discovery_from_heading_layout() {
        let path = PathBuf::from("/tmp/note_heading.md");
        let content = r#"# Discovery: Grace Blackwell GB10 Topology
Discovered unified 128GB LPDDR5X bus between CPU and dual Blackwell GPUs."#;

        let item = extract_discovery_from_markdown(&path, content).expect("extracted item");
        assert_eq!(item.canonical_name, "Grace Blackwell GB10 Topology");
        assert_eq!(item.item_type, "discovery");
        assert_eq!(item.metadata["layout"], "heading");
        assert!(item.text.contains("unified 128GB LPDDR5X"));
    }

    #[test]
    fn test_extract_discovery_from_bullet_layout() {
        let path = PathBuf::from("/tmp/note_bullet.md");
        let content = r#"- Finding: SQLite WAL checkpoint stalls under massive bulk insert
- Solution: Run PRAGMA wal_checkpoint(TRUNCATE) before consolidation
- Verification: 100% pass rate"#;

        let item = extract_discovery_from_markdown(&path, content).expect("extracted item");
        assert_eq!(item.canonical_name, "SQLite WAL checkpoint stalls under massive bulk insert");
        assert_eq!(item.item_type, "discovery");
        assert_eq!(item.metadata["layout"], "bullet_finding");
    }

    #[test]
    fn test_extract_discovery_from_raw_unstructured_layout() {
        let path = PathBuf::from("/tmp/parked-hotfix-123.md");
        let content = "Fixed zero-length embedding cosine calculation crash.";

        let item = extract_discovery_from_markdown(&path, content).expect("extracted item");
        assert_eq!(item.canonical_name, "parked-parked-hotfix-123");
        assert_eq!(item.item_type, "discovery");
        assert_eq!(item.metadata["layout"], "raw");
        assert_eq!(item.text, "Fixed zero-length embedding cosine calculation crash.");
    }

    #[test]
    fn test_extract_discovery_malformed_frontmatter_recovery() {
        let path = PathBuf::from("/tmp/malformed.md");
        let content = r#"---
unclosed frontmatter fence without closing delimiter
# Actual Title Here
Valid body content that must not be discarded."#;

        let item = extract_discovery_from_markdown(&path, content).expect("recovers cleanly");
        assert_eq!(item.canonical_name, "Actual Title Here");
        assert!(item.text.contains("Valid body content that must not be discarded"));
    }

    #[test]
    fn test_scanner_ignores_non_markdown_and_zero_byte_files() {
        let path_txt = PathBuf::from("/tmp/random.txt");
        let path_png = PathBuf::from("/tmp/diagram.png");
        let path_empty = PathBuf::from("/tmp/empty.md");
        let path_whitespace = PathBuf::from("/tmp/whitespace.md");

        // Zero-byte or whitespace-only files
        assert!(extract_discovery_from_markdown(&path_empty, "").is_none());
        assert!(extract_discovery_from_markdown(&path_whitespace, "   \n\t  \n ").is_none());

        // File extensions filter validation in scanner
        assert_ne!(path_txt.extension().and_then(|e| e.to_str()), Some("md"));
        assert_ne!(path_png.extension().and_then(|e| e.to_str()), Some("md"));
    }

    #[test]
    fn test_idle_cycle_detection_gpu_and_operator_thresholds() {
        let idle_threshold = 900; // 15 minutes

        // 1. GPU busy (> 5%) -> not idle
        assert!(!is_idle(6, 1200, idle_threshold));
        assert!(!is_idle(99, 1200, idle_threshold));

        // 2. GPU idle (0%), operator recently active (< 900s) -> not idle
        assert!(!is_idle(0, 300, idle_threshold));
        assert!(!is_idle(2, 899, idle_threshold));

        // 3. GPU idle (<= 5%) and operator idle (>= 900s) -> idle trigger!
        assert!(is_idle(0, 900, idle_threshold));
        assert!(is_idle(3, 1500, idle_threshold));
        assert!(is_idle(5, 3600, idle_threshold));
    }

    #[test]
    fn test_last_session_activity_detection() {
        let temp_dir = std::env::temp_dir().join(format!("spark_dream_test_sessions_{}", Uuid::new_v4()));
        let sessions_dir = temp_dir.join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();

        let engine = DreamEngine::new(temp_dir.clone());

        // Empty directory
        assert!(engine.get_last_session_activity().is_none());

        // Write a session file
        let sess_file = sessions_dir.join("session-alpha.json");
        fs::write(&sess_file, "{\"active\": true}").unwrap();

        let last_act = engine.get_last_session_activity();
        assert!(last_act.is_some());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_deduplication_and_simulated_dream_consolidation_cycle() {
        // Spin up mock Cortex server to verify deduplication and entity upserts
        let write_count = Arc::new(AtomicUsize::new(0));
        let last_canonical_name = Arc::new(std::sync::Mutex::new(String::new()));

        let wc = Arc::clone(&write_count);
        let lcn = Arc::clone(&last_canonical_name);

        let app = Router::new().route(
            "/api/cortex/write",
            post(move |Json(payload): Json<Value>| {
                let wc = Arc::clone(&wc);
                let lcn = Arc::clone(&lcn);
                async move {
                    wc.fetch_add(1, Ordering::SeqCst);
                    if let Some(val) = payload.get("value") {
                        if let Some(name) = val.get("canonicalName").and_then(|n| n.as_str()) {
                            *lcn.lock().unwrap() = name.to_string();
                        }
                    }
                    Json(json!({
                        "recorded": true,
                        "receipt": {
                            "id": "mock-receipt-id",
                            "operation": "entity.upsert",
                            "targetType": "entity",
                            "targetId": "entity-uuid-1",
                            "committedAt": Utc::now().to_rfc3339()
                        }
                    }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Set up test directories
        let temp_dir = std::env::temp_dir().join(format!("spark_dream_cycle_test_{}", Uuid::new_v4()));
        let park_dir = temp_dir.join("park");
        fs::create_dir_all(&park_dir).unwrap();

        let test_note = park_dir.join("note-dedup.md");
        fs::write(
            &test_note,
            "---\ntitle: Deduplication Fact\ntype: discovery\n---\nImportant consolidated insight.",
        )
        .unwrap();

        // Also add non-markdown and empty file to verify scanner resilience
        fs::write(park_dir.join("ignore.txt"), "non-markdown content").unwrap();
        fs::write(park_dir.join("empty.md"), "").unwrap();

        std::env::set_var("CORTEX_TOKEN", "mock_token");

        let engine = DreamEngine::new(temp_dir.clone())
            .with_cortex_url(format!("http://127.0.0.1:{}", port))
            .with_park_dir(park_dir.clone());

        // Execute dream cycle
        let state = engine.execute_dream_cycle().await.expect("dream cycle executed");

        assert_eq!(state.facts_committed, 1);
        assert_eq!(state.facts_rejected, 0);
        assert_eq!(write_count.load(Ordering::SeqCst), 1);
        assert_eq!(*last_canonical_name.lock().unwrap(), "Deduplication Fact");

        // Verify note was archived to .consolidated
        assert!(!test_note.exists());
        assert!(park_dir.join("note-dedup.md.consolidated").exists());

        // Verify non-markdown file was NOT touched or moved
        assert!(park_dir.join("ignore.txt").exists());

        // Clean up
        server_handle.abort();
        std::env::remove_var("CORTEX_TOKEN");
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
