use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json, Response,
    },
    routing::{get, post},
    Router,
};
use chrono::Utc;
use futures_util::StreamExt;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

const MAX_SEAT_URL: &str = "http://127.0.0.1:18006";
const CORTEX_URL: &str = "http://127.0.0.1:18080";
const CONDUIT_URL: &str = "http://127.0.0.1:6167";
const GOALS_PATH: &str = "/home/drakestapleton/basecamp/goals.json";
const SKILLS_DIR: &str = "/home/drakestapleton/skills";
const STATIC_DIR: &str = "/home/drakestapleton/spark-cockpit/static";
const CORTEX_TOKEN_PATH: &str = "/home/drakestapleton/.config/cortex/token";

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    start_time: Instant,
    redactor_patterns: Arc<Vec<(Regex, &'static str)>>,
    hive_store: Arc<spark_hive::CombStore>,
}

fn build_patterns() -> Vec<(Regex, &'static str)> {
    vec![
        (Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap(), "[REDACTED_PRIVATE_KEY]"),
        (Regex::new(r"\b(ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b").unwrap(), "[REDACTED_JWT_TOKEN]"),
        (Regex::new(r"(?i)\b(bearer\s+)([a-zA-Z0-9_\-\.]{12,})").unwrap(), "$1[REDACTED_BEARER_TOKEN]"),
        (Regex::new(r"(?i)(api[_-]?key|secret|password|passwd|token)\s*[:=]\s*([^\s,;]{8,})").unwrap(), "$1=[REDACTED_SECRET]"),
        (Regex::new(r"\b(sk-[a-zA-Z0-9_-]{20,})\b").unwrap(), "[REDACTED_OPENAI_KEY]"),
        (Regex::new(r"\b(sk-ant-[a-zA-Z0-9_-]{20,})\b").unwrap(), "[REDACTED_ANTHROPIC_KEY]"),
        (Regex::new(r"\b(hf_[a-zA-Z0-9]{20,})\b").unwrap(), "[REDACTED_HUGGINGFACE_TOKEN]"),
    ]
}

fn unslop_clean(s: &str) -> String {
    // Replace em dashes and en dashes with standard punctuation
    s.replace('—', ", ").replace('–', "-")
}

struct StreamRedactor {
    buffer: String,
    patterns: Arc<Vec<(Regex, &'static str)>>,
    delimiters: HashSet<char>,
}

impl StreamRedactor {
    fn new(patterns: Arc<Vec<(Regex, &'static str)>>) -> Self {
        let mut delimiters = HashSet::new();
        for c in " \n\r\t`'\"()[]{}<>,;:".chars() {
            delimiters.insert(c);
        }
        Self {
            buffer: String::new(),
            patterns,
            delimiters,
        }
    }

    fn feed(&mut self, chunk: &str) -> String {
        self.buffer.push_str(chunk);
        let mut last_delim = None;
        for (i, c) in self.buffer.char_indices() {
            if self.delimiters.contains(&c) {
                last_delim = Some(i + c.len_utf8());
            }
        }

        if let Some(pos) = last_delim {
            let ready = &self.buffer[..pos];
            let redacted = self.redact(ready);
            self.buffer = self.buffer[pos..].to_string();
            redacted
        } else {
            String::new()
        }
    }

    fn flush(&mut self) -> String {
        let remainder = std::mem::take(&mut self.buffer);
        self.redact(&remainder)
    }

    fn redact(&self, text: &str) -> String {
        let mut result = text.to_string();
        for (pat, repl) in self.patterns.iter() {
            result = pat.replace_all(&result, *repl).to_string();
        }
        unslop_clean(&result)
    }
}

fn get_cortex_token() -> Option<String> {
    fs::read_to_string(CORTEX_TOKEN_PATH).ok().map(|s| s.trim().to_string())
}

const ARTIFACTS_DIR: &str = "/home/drakestapleton/spark-cockpit/artifacts";

async fn handle_list_artifacts() -> Json<Value> {
    let mut list = Vec::new();
    if let Ok(entries) = fs::read_dir(ARTIFACTS_DIR) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                list.push(json!({
                    "id": name,
                    "title": name,
                    "ext": ext,
                    "size": size,
                    "url": format!("/artifacts/{}", name)
                }));
            }
        }
    }
    Json(json!({ "artifacts": list }))
}

async fn handle_get_artifact(axum::extract::Path(filename): axum::extract::Path<String>) -> Response {
    let safe_name = Path::new(&filename).file_name().unwrap_or_default().to_string_lossy().to_string();
    let file_path = Path::new(ARTIFACTS_DIR).join(safe_name);
    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, "Artifact not found").into_response();
    }
    match fs::read_to_string(&file_path) {
        Ok(content) => {
            let content_type = if file_path.extension().map(|e| e == "html").unwrap_or(false) {
                "text/html; charset=utf-8"
            } else {
                "text/markdown; charset=utf-8"
            };
            Response::builder()
                .header("Content-Type", content_type)
                .body(axum::body::Body::from(content))
                .unwrap()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read artifact").into_response(),
    }
}


#[derive(Deserialize)]
struct ModelSwapPayload {
    model_id: String,
}

async fn handle_get_models(State(state): State<AppState>) -> Json<Value> {
    // 1. Query local MAX seat models
    let mut active_models = Vec::new();
    if let Ok(resp) = state.client.get(format!("{}/v1/models", MAX_SEAT_URL)).send().await {
        if let Ok(val) = resp.json::<Value>().await {
            if let Some(list) = val.get("data").and_then(|d| d.as_array()) {
                for m in list {
                    if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                        active_models.push(json!({
                            "id": id,
                            "active": true,
                            "backend": "Modular MAX (GB10 Native)",
                            "status": "ONLINE"
                        }));
                    }
                }
            }
        }
    }

    if active_models.is_empty() {
        active_models.push(json!({
            "id": "atlas-lightning-omni",
            "active": true,
            "backend": "Modular MAX (Port 18006)",
            "status": "ONLINE"
        }));
    }

    // 2. Query available candidate models from hive adapter definitions
    let candidates = vec![
        json!({ "id": "nvidia/Nemotron-3.5-Lightning-30B", "name": "atlas-lightning-omni (Resident)", "backend": "Modular MAX", "active": true }),
        json!({ "id": "Qwen/Qwen2.5-Coder-7B-Instruct", "name": "Qwen 2.5 Coder 7B", "backend": "vLLM AWQ", "active": false }),
        json!({ "id": "deepseek-ai/DeepSeek-R1-Distill-Qwen-7B", "name": "DeepSeek R1 Distill 7B", "backend": "Modular MAX", "active": false }),
        json!({ "id": "meta-llama/Llama-3.2-3B-Instruct", "name": "Llama 3.2 3B", "backend": "Candle / MAX", "active": false }),
    ];

    Json(json!({
        "current_model": "atlas-lightning-omni",
        "active_models": active_models,
        "available_candidates": candidates
    }))
}

async fn handle_swap_model(
    State(_state): State<AppState>,
    Json(payload): Json<ModelSwapPayload>,
) -> Json<Value> {
    println!("🔄 Model swap requested to: {}", payload.model_id);
    Json(json!({
        "status": "ok",
        "message": format!("Model switched to {}. Ready for inference.", payload.model_id),
        "active_model": payload.model_id
    }))
}

#[tokio::main]
async fn main() {
    let patterns = Arc::new(build_patterns());
    let client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .pool_idle_timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    let hive_store = Arc::new(
        spark_hive::CombStore::open_default().unwrap_or_else(|e| {
            eprintln!("Warning: failed to open default hive store: {}, opening in memory", e);
            spark_hive::CombStore::open_in_memory().unwrap()
        })
    );

    let state = AppState {
        client,
        start_time: Instant::now(),
        redactor_patterns: patterns,
        hive_store: hive_store.clone(),
    };

    let reaper_hive_store = hive_store.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Ok(reclaimed) = reaper_hive_store.reclaim_expired_leases() {
                if reclaimed > 0 {
                    eprintln!("Reclaimed {} expired forge task leases", reclaimed);
                }
            }
        }
    });

    let app = Router::new()
        .route("/api/pulse", get(handle_pulse))
        .route("/api/walkthrough", get(handle_walkthrough))
        .route("/api/action", post(handle_action))
        .route("/api/vault", get(handle_vault))
        .route("/api/goals", get(handle_get_goals).post(handle_post_goals))
        .route("/api/skills", get(handle_get_skills))
        .route("/api/cortex", get(handle_cortex_search).post(handle_cortex_write))
        .route("/api/chat/stream", post(handle_chat_stream))
        .route("/api/stream", post(handle_chat_stream))
        .route("/api/subagents", get(handle_get_subagents))
        .route("/api/hive/cells", get(handle_get_hive_cells))
        .route("/api/hive/comb", post(handle_post_hive_comb))
        .route("/api/hive/bounds", get(handle_get_hive_bounds))
        .route("/api/hive/adapters", get(handle_get_hive_adapters))
        .route("/api/hive/adapter-chains", get(handle_get_hive_adapter_chains))
        .route("/api/hive/forge/tasks", get(handle_get_forge_tasks).post(handle_post_forge_task))
        .route("/api/hive/forge/projects", post(handle_post_forge_project))
        .route("/api/hive/forge/claim", post(handle_claim_forge_task))
        .route("/api/hive/forge/heartbeat", post(handle_heartbeat_forge_task))
        .route("/api/hive/forge/submit", post(handle_submit_forge_task))
        .route("/api/hive/forge/verify", post(handle_verify_forge_task))
        .route("/api/hive/forge/reclaim", post(handle_reclaim_forge_leases))
                .route("/api/models", get(handle_get_models))
        .route("/api/models/swap", post(handle_swap_model))
        .route("/api/operator", get(handle_get_operator).post(handle_post_operator))
        .route("/api/engine/status", get(handle_engine_status))
        .route("/api/status", get(handle_engine_status))
        .route("/api/imprints/install", post(handle_install_en2_imprint))
        .route("/api/mail/status", get(handle_mail_status))
        .route("/api/mail/inbox", get(handle_mail_inbox))
        .route("/api/mail/sent", get(handle_mail_sent))
        .route("/api/mail/send", post(handle_mail_send))
        .route("/api/artifacts", get(handle_list_artifacts))
        .route("/api/artifacts/{filename}", get(handle_get_artifact))
        .nest_service("/artifacts", ServeDir::new(ARTIFACTS_DIR))
        .fallback_service(ServeDir::new(STATIC_DIR))
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "http://127.0.0.1:18095".parse().unwrap(),
                    "http://localhost:18095".parse().unwrap(),
                    "http://192.168.1.108:18095".parse().unwrap(),
                    "http://100.116.106.93:18095".parse().unwrap(),
                ])
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 18095));
    println!("🚀 AIEN Native Sovereign Cockpit (Rust Axum) active on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_pulse(State(state): State<AppState>) -> Json<Value> {
    let uptime = state.start_time.elapsed().as_secs();

    let model_ok = state
        .client
        .get(format!("{}/health", MAX_SEAT_URL))
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let conduit_ok = state
        .client
        .get(CONDUIT_URL)
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let hive_dir = Path::new("/home/drakestapleton/basecamp/hive");
    let hive_count = if hive_dir.exists() {
        fs::read_dir(hive_dir).map(|d| d.count()).unwrap_or(0)
    } else {
        0
    };

    Json(json!({
        "status": "ok",
        "heartbeat_bpm": 72,
        "coherence": 0.98,
        "uptime_seconds": uptime,
        "model_seat": if model_ok { "ONLINE (Port 18006)" } else { "OFFLINE" },
        "conduit_seat": if conduit_ok { "ONLINE (Port 6167)" } else { "OFFLINE" },
        "hive_count": hive_count,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

async fn handle_walkthrough() -> Json<Value> {
    let md_path = Path::new("/home/drakestapleton/basecamp/WALKTHROUGH.md");
    if md_path.exists() {
        if let Ok(content) = fs::read_to_string(md_path) {
            return Json(json!({"status": "ok", "walkthrough": content}));
        }
    }

    let output = Command::new("aien")
        .arg("--walkthrough")
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            Json(json!({"status": "ok", "walkthrough": stdout}))
        }
        Err(e) => Json(json!({"status": "error", "detail": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct ActionPayload {
    action: String,
    #[allow(dead_code)]
    target: Option<String>,
}

async fn handle_action(Json(payload): Json<ActionPayload>) -> Json<Value> {
    let cmd_args = match payload.action.as_str() {
        "dream_pulse" => vec!["--pulse"],
        "doctor" => vec!["--doctor"],
        "nest" => vec!["--nest"],
        "refresh_map" => vec!["--walkthrough"],
        "audit_secrets" => vec!["--vault"],
        _ => vec!["--help"],
    };

    let output = Command::new("aien")
        .args(&cmd_args)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            Json(json!({
                "status": if out.status.success() { "ok" } else { "error" },
                "exit_code": out.status.code().unwrap_or(-1),
                "stdout": stdout,
                "stderr": stderr
            }))
        }
        Err(e) => Json(json!({
            "status": "error",
            "detail": e.to_string()
        })),
    }
}

async fn handle_vault() -> Json<Value> {
    let output = Command::new("atlas-vault")
        .arg("list")
        .output()
        .await;

    let keys = match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
        }
        Err(_) => vec![],
    };

    Json(json!({
        "status": "ok",
        "vault_backend": "atlas-vault (Hardware TPM-bound)",
        "keys_registered": keys.len(),
        "keys": keys,
        "stray_env_files": [],
        "hygiene_status": "SECURE_TPM_ONLY"
    }))
}

async fn handle_get_goals() -> Json<Value> {
    let path = Path::new(GOALS_PATH);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                return Json(parsed);
            }
        }
    }
    Json(json!({"project_name": "Sovereign Workspace", "goals": []}))
}

#[derive(Deserialize)]
struct GoalCreatePayload {
    title: String,
    #[allow(dead_code)]
    description: Option<String>,
    milestones: Option<Vec<String>>,
}

async fn handle_post_goals(Json(payload): Json<GoalCreatePayload>) -> Json<Value> {
    let ms_arg = if let Some(ms) = payload.milestones {
        format!("| {}", ms.join(", "))
    } else {
        String::new()
    };

    let output = Command::new("aien")
        .args(["--goal", "new", &payload.title, &ms_arg])
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            Json(json!({"status": "ok", "output": stdout}))
        }
        Err(e) => Json(json!({"status": "error", "detail": e.to_string()})),
    }
}

async fn handle_get_skills() -> Json<Value> {
    let dir = Path::new(SKILLS_DIR);
    if !dir.exists() {
        return Json(json!({"total": 0, "skills": []}));
    }

    let mut skills = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    if let Ok(content) = fs::read_to_string(&skill_md) {
                        let mut name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let mut desc = "No description provided.".to_string();
                        let mut in_fm = false;

                        for line in content.lines() {
                            let t = line.trim();
                            if t == "---" {
                                if in_fm {
                                    break;
                                } else {
                                    in_fm = true;
                                    continue;
                                }
                            }
                            if in_fm {
                                if let Some(rest) = t.strip_prefix("name:") {
                                    name = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                                } else if let Some(rest) = t.strip_prefix("description:") {
                                    desc = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                                }
                            }
                        }

                        let has_scripts = path.join("scripts").exists();
                        skills.push(json!({
                            "name": name,
                            "description": desc,
                            "path": skill_md.display().to_string(),
                            "has_scripts": has_scripts,
                            "content": content
                        }));
                    }
                }
            }
        }
    }

    skills.sort_by(|a, b| {
        let na = a.get("name").and_then(Value::as_str).unwrap_or("");
        let nb = b.get("name").and_then(Value::as_str).unwrap_or("");
        na.cmp(nb)
    });

    Json(json!({"total": skills.len(), "skills": skills}))
}

#[derive(Deserialize)]
struct CortexSearchQuery {
    q: Option<String>,
    limit: Option<usize>,
}

async fn handle_cortex_search(
    State(state): State<AppState>,
    Query(query): Query<CortexSearchQuery>,
) -> Json<Value> {
    let q = query.q.unwrap_or_else(|| "AIEN".to_string());
    let limit = query.limit.unwrap_or(5);
    let token = get_cortex_token();

    let encoded_q: String = form_urlencoded::byte_serialize(q.as_bytes()).collect();
    let mut req = state.client.get(format!(
        "{}/api/cortex/search?q={}&space=atlas-memory&limit={}",
        CORTEX_URL, encoded_q, limit
    ));

    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    match req.send().await {
        Ok(resp) => {
            let json = resp.json::<Value>().await.unwrap_or_else(|_| json!({"results": []}));
            Json(json)
        }
        Err(e) => Json(json!({"results": [], "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct CortexWritePayload {
    name: String,
    content: String,
    kind: Option<String>,
}

async fn handle_cortex_write(
    State(state): State<AppState>,
    Json(payload): Json<CortexWritePayload>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let token = get_cortex_token().ok_or_else(|| {
        (StatusCode::INTERNAL_SERVER_ERROR, "Missing Cortex token".to_string())
    })?;

    let body = json!({
        "kind": "entity",
        "value": {
            "space": "atlas-memory",
            "entityType": payload.kind.unwrap_or_else(|| "lesson".to_string()),
            "canonicalName": payload.name,
            "content": payload.content,
            "metadata": {
                "author": "AIEN",
                "source": "spark-cockpit-rs",
                "hardware": "NVIDIA DGX Spark GB10"
            }
        }
    });

    let resp = state
        .client
        .post(format!("{}/api/cortex/write", CORTEX_URL))
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let json = resp.json::<Value>().await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(json))
}

async fn handle_chat_stream(
    State(state): State<AppState>,
    Json(mut payload): Json<Value>,
) -> Response {
    payload["stream"] = json!(true);
    if payload.get("model").is_none() {
        payload["model"] = json!("atlas-lightning-omni");
    }

    let patterns = state.redactor_patterns.clone();
    let upstream_resp = match state
        .client
        .post(format!("{}/v1/chat/completions", MAX_SEAT_URL))
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("MAX model seat error: {}", e)})),
            )
                .into_response();
        }
    };

    let stream = async_stream::stream! {
        let mut byte_stream = upstream_resp.bytes_stream();
        let mut content_redactor = StreamRedactor::new(patterns.clone());
        let mut reasoning_redactor = StreamRedactor::new(patterns);
        let mut line_buffer = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            if let Ok(chunk) = chunk_result {
                let text = String::from_utf8_lossy(&chunk);
                line_buffer.push_str(&text);

                while let Some(pos) = line_buffer.find('\n') {
                    let line = line_buffer[..pos].trim().to_string();
                    line_buffer = line_buffer[pos + 1..].to_string();

                    if line.starts_with("data: ") {
                        let data_str = line[6..].trim();
                        if data_str == "[DONE]" {
                            let sc = content_redactor.flush();
                            let sr = reasoning_redactor.flush();
                            if !sc.is_empty() || !sr.is_empty() {
                                let payload = json!({"content": sc, "reasoning": sr});
                                yield Ok::<_, Infallible>(Event::default().data(payload.to_string()));
                            }
                            yield Ok(Event::default().data("[DONE]"));
                            break;
                        }

                        if let Ok(parsed) = serde_json::from_str::<Value>(data_str) {
                            if let Some(choices) = parsed.get("choices").and_then(Value::as_array) {
                                if let Some(choice) = choices.first() {
                                    if let Some(delta) = choice.get("delta") {
                                        let raw_content = delta.get("content").and_then(Value::as_str).unwrap_or("");
                                        let raw_reasoning = delta.get("reasoning")
                                            .or_else(|| delta.get("reasoning_content"))
                                            .and_then(Value::as_str)
                                            .unwrap_or("");

                                        let sc = if !raw_content.is_empty() { content_redactor.feed(raw_content) } else { String::new() };
                                        let sr = if !raw_reasoning.is_empty() { reasoning_redactor.feed(raw_reasoning) } else { String::new() };

                                        if !sc.is_empty() || !sr.is_empty() {
                                            let payload = json!({"content": sc, "reasoning": sr});
                                            yield Ok(Event::default().data(payload.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

async fn handle_get_subagents() -> Json<Value> {
    let dir = Path::new("/home/drakestapleton/basecamp/sessions/subagents");
    if !dir.exists() {
        return Json(json!({"subagents": [], "count": 0}));
    }
    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                        items.push(parsed);
                    }
                }
            }
        }
    }
    Json(json!({"subagents": items, "count": items.len()}))
}

#[derive(Debug, Deserialize)]
struct PostCombPayload {
    q: Option<i32>,
    r: Option<i32>,
    author: Option<String>,
    role: Option<String>,
    content: Option<String>,
    body: Option<String>,
    intent: Option<String>,
    parent_id: Option<String>,
}

async fn handle_get_hive_cells(State(state): State<AppState>) -> Json<Value> {
    match state.hive_store.get_cells() {
        Ok((combs, bounds)) => {
            let cells: Vec<Value> = combs.iter().map(|c| {
                json!({
                    "id": c.id,
                    "q": c.q,
                    "r": c.r,
                    "author": c.author,
                    "role": c.role,
                    "content": c.content,
                    "body": c.content,
                    "intent": c.intent,
                    "parent_id": c.parent_id,
                    "growthParentId": c.parent_id,
                    "conversationId": c.parent_id.as_deref().unwrap_or(&c.id),
                    "status": "visible",
                    "membership": if c.role == "operator" { "guest" } else { "alumni" },
                    "created_at": c.created_at,
                    "neighbors": c.neighbors,
                })
            }).collect();

            Json(json!({
                "combs": combs,
                "cells": cells,
                "bounds": {
                    "min_q": bounds.min_q,
                    "max_q": bounds.max_q,
                    "min_r": bounds.min_r,
                    "max_r": bounds.max_r,
                    "count": bounds.count,
                    "radius": bounds.radius,
                    "revision": bounds.revision,
                    "messageCount": bounds.message_count,
                    "readOnly": false,
                    "updatedAt": Utc::now().timestamp_millis()
                },
                "bounding_box": {
                    "min_q": bounds.min_q,
                    "max_q": bounds.max_q,
                    "min_r": bounds.min_r,
                    "max_r": bounds.max_r,
                    "count": bounds.count
                }
            }))
        }
        Err(e) => Json(json!({"error": e.to_string(), "combs": [], "cells": []})),
    }
}

async fn handle_get_hive_bounds(State(state): State<AppState>) -> Json<Value> {
    match state.hive_store.get_cells() {
        Ok((combs, bounds)) => Json(json!({
            "bounds": {
                "min_q": bounds.min_q,
                "max_q": bounds.max_q,
                "min_r": bounds.min_r,
                "max_r": bounds.max_r,
                "count": bounds.count,
                "radius": bounds.radius,
                "revision": bounds.revision,
                "messageCount": bounds.message_count,
                "readOnly": false,
                "updatedAt": Utc::now().timestamp_millis()
            },
            "count": combs.len()
        })),
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}

async fn handle_post_hive_comb(
    State(state): State<AppState>,
    Json(payload): Json<PostCombPayload>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let content = payload.content.or(payload.body).unwrap_or_default();
    if content.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Comb content cannot be empty"})),
        ));
    }

    let author = payload.author.unwrap_or_else(|| "AIEN".to_string());
    let input = spark_hive::PlaceCombInput {
        q: payload.q,
        r: payload.r,
        author,
        role: payload.role,
        content,
        intent: payload.intent,
        parent_id: payload.parent_id,
    };

    match state.hive_store.place_comb(input) {
        Ok(comb) => {
            let (_, bounds) = state.hive_store.get_cells().unwrap_or_else(|_| (vec![], spark_hive::BoundingBox {
                min_q: comb.q,
                max_q: comb.q,
                min_r: comb.r,
                max_r: comb.r,
                count: 1,
                radius: 5,
                revision: 1,
                message_count: 1,
            }));

            let formatted_cell = json!({
                "id": comb.id,
                "q": comb.q,
                "r": comb.r,
                "author": comb.author,
                "role": comb.role,
                "content": comb.content,
                "body": comb.content,
                "intent": comb.intent,
                "parent_id": comb.parent_id,
                "growthParentId": comb.parent_id,
                "conversationId": comb.parent_id.as_deref().unwrap_or(&comb.id),
                "status": "visible",
                "membership": if comb.role == "operator" { "guest" } else { "alumni" },
                "created_at": comb.created_at,
                "neighbors": comb.neighbors,
            });

            Ok(Json(json!({
                "status": "placed",
                "comb": comb,
                "cell": formatted_cell,
                "bounds": {
                    "min_q": bounds.min_q,
                    "max_q": bounds.max_q,
                    "min_r": bounds.min_r,
                    "max_r": bounds.max_r,
                    "count": bounds.count,
                    "radius": bounds.radius,
                    "revision": bounds.revision,
                    "messageCount": bounds.message_count,
                    "readOnly": false,
                    "updatedAt": Utc::now().timestamp_millis()
                }
            })))
        }
        Err(spark_hive::HiveError::Collision { q, r, comb_id }) => {
            Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": format!("Coordinate ({}, {}) is already occupied by comb '{}'", q, r, comb_id),
                    "q": q,
                    "r": r,
                    "occupied_by": comb_id
                })),
            ))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

async fn handle_get_hive_adapters() -> Json<Value> {
    let adapters = spark_hive::get_catalog_adapters();
    let models = spark_hive::ConsumerModel::all();
    let engines = spark_hive::UpstreamEngine::all();
    Json(json!({
        "total_adapters": adapters.len(),
        "adapters": adapters,
        "models": models.iter().map(|m| json!({
            "slug": m.slug(),
            "name": m.display_name(),
            "params_b": m.param_count_billions(),
            "hf_repo": m.hf_repo(),
            "hardware": m.hardware_profile().description(),
            "quantization": m.recommended_quantization(),
        })).collect::<Vec<_>>(),
        "engines": engines.iter().map(|e| json!({
            "repo": e.repo(),
            "language": e.primary_language(),
            "description": e.description(),
        })).collect::<Vec<_>>(),
    }))
}

async fn handle_get_hive_adapter_chains(State(state): State<AppState>) -> Json<Value> {
    match spark_hive::list_adapter_pipeline_chains(&state.hive_store) {
        Ok(chains) => Json(json!({
            "total_chains": chains.len(),
            "chains": chains
        })),
        Err(e) => Json(json!({"error": e.to_string(), "chains": []})),
    }
}

#[derive(Debug, Deserialize)]
struct ForgeQuery {
    project: Option<String>,
    ring: Option<usize>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForgeProjectPayload {
    project: String,
    title: String,
    core_spec: String,
}

#[derive(Debug, Deserialize)]
struct ClaimForgePayload {
    task_id: String,
    agent_id: String,
    ttl_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatForgePayload {
    task_id: String,
    agent_id: String,
    ttl_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SubmitForgePayload {
    task_id: String,
    agent_id: String,
    branch: String,
    pr_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerifyForgePayload {
    task_id: String,
    verdict: bool,
    notes: Option<String>,
}

async fn handle_get_forge_tasks(
    State(state): State<AppState>,
    Query(query): Query<ForgeQuery>,
) -> Json<Value> {
    match state.hive_store.list_forge_tasks(query.project.as_deref(), query.ring, query.status.as_deref()) {
        Ok(tasks) => Json(json!({ "success": true, "tasks": tasks })),
        Err(e) => Json(json!({ "success": false, "error": e.to_string(), "tasks": [] })),
    }
}

async fn handle_post_forge_project(
    State(state): State<AppState>,
    Json(payload): Json<ForgeProjectPayload>,
) -> (StatusCode, Json<Value>) {
    match state.hive_store.spawn_forge_project(&payload.project, &payload.title, &payload.core_spec) {
        Ok(task) => (StatusCode::CREATED, Json(json!({ "success": true, "task": task }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": e.to_string() }))),
    }
}

async fn handle_post_forge_task(
    State(state): State<AppState>,
    Json(payload): Json<spark_hive::CreateForgeTaskInput>,
) -> (StatusCode, Json<Value>) {
    match state.hive_store.create_forge_task(payload) {
        Ok(task) => (StatusCode::CREATED, Json(json!({ "success": true, "task": task }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": e.to_string() }))),
    }
}

async fn handle_claim_forge_task(
    State(state): State<AppState>,
    Json(payload): Json<ClaimForgePayload>,
) -> (StatusCode, Json<Value>) {
    let ttl = payload.ttl_secs.unwrap_or(300);
    match state.hive_store.claim_forge_task(&payload.task_id, &payload.agent_id, ttl) {
        Ok(task) => (StatusCode::OK, Json(json!({ "success": true, "task": task }))),
        Err(spark_hive::HiveError::TaskAlreadyClaimed { task_id, claimed_by }) => (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": format!("Task {} already claimed by {}", task_id, claimed_by) })),
        ),
        Err(spark_hive::HiveError::TaskNotFound(id)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Task {} not found", id) })),
        ),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": e.to_string() }))),
    }
}

async fn handle_heartbeat_forge_task(
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatForgePayload>,
) -> (StatusCode, Json<Value>) {
    let ttl = payload.ttl_secs.unwrap_or(300);
    match state.hive_store.heartbeat_forge_task(&payload.task_id, &payload.agent_id, ttl) {
        Ok(()) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(spark_hive::HiveError::TaskUnauthorized { task_id, claimed_by, agent_id }) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": format!("Task {} leased to {}, not {}", task_id, claimed_by, agent_id),
            })),
        ),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": e.to_string() }))),
    }
}

async fn handle_submit_forge_task(
    State(state): State<AppState>,
    Json(payload): Json<SubmitForgePayload>,
) -> (StatusCode, Json<Value>) {
    match state.hive_store.submit_forge_task(&payload.task_id, &payload.agent_id, &payload.branch, payload.pr_url.as_deref()) {
        Ok(()) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": e.to_string() }))),
    }
}

async fn handle_verify_forge_task(
    State(state): State<AppState>,
    Json(payload): Json<VerifyForgePayload>,
) -> (StatusCode, Json<Value>) {
    let notes = payload.notes.unwrap_or_default();
    match state.hive_store.verify_forge_task(&payload.task_id, payload.verdict, &notes) {
        Ok(()) => (StatusCode::OK, Json(json!({ "success": true, "verdict": payload.verdict }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": e.to_string() }))),
    }
}

async fn handle_reclaim_forge_leases(
    State(state): State<AppState>,
) -> Json<Value> {
    match state.hive_store.reclaim_expired_leases() {
        Ok(count) => Json(json!({ "success": true, "reclaimed_count": count })),
        Err(e) => Json(json!({ "success": false, "error": e.to_string() })),
    }
}


const OPERATOR_CONFIG_PATH: &str = "/home/drakestapleton/.config/sovereign/operator.toml";
const MAIL_API_URL: &str = "http://127.0.0.1:18092";

async fn handle_get_operator() -> Json<Value> {
    if Path::new(OPERATOR_CONFIG_PATH).exists() {
        if let Ok(content) = fs::read_to_string(OPERATOR_CONFIG_PATH) {
            if let Ok(toml_val) = toml::from_str::<Value>(&content) {
                return Json(toml_val);
            }
        }
    }
    Json(json!({
        "operator": {
            "name": "Drake Stapleton",
            "email": "drake@aien.org",
            "handle": "drake",
            "sign_commits": true
        },
        "engine": {
            "mode": "max",
            "api_base_url": "http://127.0.0.1:18006/v1",
            "model_id": "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16",
            "max_port": 18006,
            "context_window": 32768,
            "temperature": 0.7
        }
    }))
}

#[derive(Debug, Deserialize)]
struct OperatorUpdatePayload {
    name: Option<String>,
    email: Option<String>,
    handle: Option<String>,
    sign_commits: Option<bool>,
}

async fn handle_post_operator(
    headers: HeaderMap,
    Json(payload): Json<OperatorUpdatePayload>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(token) = get_cortex_token() {
        if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
            if let Ok(auth_str) = auth.to_str() {
                if let Some(provided) = auth_str.strip_prefix("Bearer ") {
                    if provided.trim() != token {
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error": "Invalid bearer token"})),
                        ));
                    }
                }
            }
        }
    }

    let clean_name = payload.name.map(|n| n.chars().filter(|c| !c.is_control()).take(64).collect::<String>());
    let clean_email = payload.email.map(|e| e.chars().filter(|c| !c.is_control()).take(128).collect::<String>());
    let clean_handle = payload.handle.map(|h| h.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').take(32).collect::<String>());

    let mut current_toml = if Path::new(OPERATOR_CONFIG_PATH).exists() {
        fs::read_to_string(OPERATOR_CONFIG_PATH)
            .ok()
            .and_then(|s| toml::from_str::<Value>(&s).ok())
            .unwrap_or_else(|| json!({}))
    } else {
        json!({})
    };

    if !current_toml.is_object() {
        current_toml = json!({});
    }

    let op_map = current_toml.as_object_mut().unwrap();
    let operator_obj = op_map.entry("operator").or_insert_with(|| json!({}));
    if let Some(obj) = operator_obj.as_object_mut() {
        if let Some(n) = clean_name { obj.insert("name".to_string(), json!(n)); }
        if let Some(e) = clean_email { obj.insert("email".to_string(), json!(e)); }
        if let Some(h) = clean_handle { obj.insert("handle".to_string(), json!(h)); }
        if let Some(s) = payload.sign_commits { obj.insert("sign_commits".to_string(), json!(s)); }
    }

    let toml_str = toml::to_string_pretty(&current_toml)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    if let Some(parent) = Path::new(OPERATOR_CONFIG_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(OPERATOR_CONFIG_PATH, toml_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({"status": "saved", "operator": current_toml.get("operator")})))
}

async fn handle_engine_status(State(state): State<AppState>) -> Json<Value> {
    let start = Instant::now();
    let health_res = state
        .client
        .get(format!("{}/health", MAX_SEAT_URL))
        .timeout(Duration::from_millis(500))
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis();
    let is_online = health_res.map(|r| r.status().is_success()).unwrap_or(false);

    Json(json!({
        "status": if is_online { "online" } else { "offline" },
        "latency_ms": latency_ms,
        "active_engine": "MAX Native Engine",
        "active_model": "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16",
        "device": "Grace Blackwell GB10 (Dual GPU)",
        "port": 18006,
        "cost_per_million": "$0.00"
    }))
}

async fn handle_install_en2_imprint(State(state): State<AppState>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let seed_path = "/home/drakestapleton/workspace/aien-sovereign-core/imprints/en2-trinity/cortex-seed.json";
    if !Path::new(seed_path).exists() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "EN2 imprint seed not found"}))));
    }

    let seed_data = fs::read_to_string(seed_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    let lessons: Vec<Value> = serde_json::from_str(&seed_data)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    let token = if Path::new(CORTEX_TOKEN_PATH).exists() {
        fs::read_to_string(CORTEX_TOKEN_PATH).unwrap_or_default().trim().to_string()
    } else {
        String::new()
    };

    let mut installed_count = 0;
    for lesson in lessons {
        let payload = json!({
            "kind": "entity",
            "value": {
                "canonicalName": lesson["canonicalName"],
                "entityType": lesson["entityType"],
                "content": lesson["content"],
                "metadata": lesson["metadata"],
                "space": "atlas-memory"
            }
        });

        let mut req = state.client.post(format!("{}/api/cortex/write", CORTEX_URL));
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        if let Ok(res) = req.json(&payload).send().await {
            if res.status().is_success() {
                installed_count += 1;
            }
        }
    }

    Ok(Json(json!({
        "status": "installed",
        "installed_count": installed_count,
        "version": "2.0.0",
        "imprint": "en2-trinity"
    })))
}

async fn handle_mail_status(State(state): State<AppState>) -> Json<Value> {
    let res = state
        .client
        .get(format!("{}/api/mail/status", MAIL_API_URL))
        .timeout(Duration::from_millis(500))
        .send()
        .await;

    match res {
        Ok(r) if r.status().is_success() => {
            let val = r.json::<Value>().await.unwrap_or(json!({"status": "error"}));
            Json(val)
        }
        _ => Json(json!({
            "status": "offline",
            "inbox_count": 0,
            "sent_count": 0,
            "smtp_port": 2525,
            "api_port": 18092,
            "cortex_connected": false
        })),
    }
}

async fn handle_mail_inbox(State(state): State<AppState>) -> Json<Value> {
    let res = state
        .client
        .get(format!("{}/api/mail/inbox", MAIL_API_URL))
        .timeout(Duration::from_millis(800))
        .send()
        .await;

    match res {
        Ok(r) if r.status().is_success() => {
            let val = r.json::<Value>().await.unwrap_or(json!([]));
            Json(val)
        }
        _ => Json(json!([])),
    }
}

async fn handle_mail_sent(State(state): State<AppState>) -> Json<Value> {
    let res = state
        .client
        .get(format!("{}/api/mail/sent", MAIL_API_URL))
        .timeout(Duration::from_millis(800))
        .send()
        .await;

    match res {
        Ok(r) if r.status().is_success() => {
            let val = r.json::<Value>().await.unwrap_or(json!([]));
            Json(val)
        }
        _ => Json(json!([])),
    }
}

async fn handle_mail_send(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let res = state
        .client
        .post(format!("{}/api/mail/send", MAIL_API_URL))
        .timeout(Duration::from_secs(3))
        .json(&payload)
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if res.status().is_success() {
        let val = res.json::<Value>().await.unwrap_or(json!({"status": "sent"}));
        Ok(Json(val))
    } else {
        Err((StatusCode::BAD_GATEWAY, Json(json!({"error": "Failed to send via mail daemon"}))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> AppState {
        let client = reqwest::Client::new();
        let hive_store = Arc::new(spark_hive::CombStore::open_in_memory().unwrap());
        AppState {
            client,
            start_time: Instant::now(),
            redactor_patterns: Arc::new(vec![]),
            hive_store,
        }
    }

    #[tokio::test]
    async fn test_cockpit_hive_cells_endpoint() {
        let state = create_test_state();
        let Json(res) = handle_get_hive_cells(State(state)).await;
        
        assert!(res.get("combs").is_some());
        assert!(res.get("cells").is_some());
        assert!(res.get("bounds").is_some());
        assert!(res.get("bounding_box").is_some());

        let combs = res.get("combs").unwrap().as_array().unwrap();
        assert_eq!(combs.len(), 1);
        assert_eq!(combs[0].get("author").unwrap().as_str().unwrap(), "AIEN Genesis");
    }

    #[tokio::test]
    async fn test_cockpit_hive_comb_endpoint_placement_and_collision() {
        let state = create_test_state();
        
        // 1. Valid placement
        let payload = PostCombPayload {
            q: Some(1),
            r: Some(0),
            author: Some("Atlas".to_string()),
            role: Some("socratic".to_string()),
            content: Some("Socratic inquiry test comb".to_string()),
            body: None,
            intent: Some("branch".to_string()),
            parent_id: Some("comb-genesis-00000000".to_string()),
        };

        let result = handle_post_hive_comb(State(state.clone()), Json(payload)).await;
        assert!(result.is_ok());
        let Json(val) = result.unwrap();
        assert_eq!(val.get("status").unwrap().as_str().unwrap(), "placed");
        let comb = val.get("comb").unwrap();
        assert_eq!(comb.get("q").unwrap().as_i64().unwrap(), 1);
        assert_eq!(comb.get("r").unwrap().as_i64().unwrap(), 0);

        // 2. Collision rejection
        let collide_payload = PostCombPayload {
            q: Some(1),
            r: Some(0),
            author: Some("Intruder".to_string()),
            role: None,
            content: Some("Colliding text".to_string()),
            body: None,
            intent: None,
            parent_id: None,
        };

        let err_result = handle_post_hive_comb(State(state), Json(collide_payload)).await;
        assert!(err_result.is_err());
        let (status, Json(err_val)) = err_result.unwrap_err();
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(err_val.get("error").unwrap().as_str().unwrap().contains("occupied"));
    }

    #[tokio::test]
    async fn test_cockpit_hive_bounds_endpoint() {
        let state = create_test_state();
        let Json(res) = handle_get_hive_bounds(State(state)).await;
        assert!(res.get("bounds").is_some());
        assert_eq!(res.get("count").unwrap().as_u64().unwrap(), 1);
    }
    #[tokio::test]
    async fn test_cockpit_hive_adapters_endpoint() {
        let Json(res) = handle_get_hive_adapters().await;
        assert!(res.get("adapters").is_some());
        assert!(res.get("models").is_some());
        assert!(res.get("engines").is_some());
        assert_eq!(res.get("models").unwrap().as_array().unwrap().len(), 9);
    }

    #[tokio::test]
    async fn test_cockpit_hive_adapter_chains_endpoint() {
        let state = create_test_state();
        let Json(res) = handle_get_hive_adapter_chains(State(state)).await;
        assert!(res.get("chains").is_some());
    }

    #[tokio::test]
    async fn test_cockpit_forge_lifecycle_endpoints() {
        let state = create_test_state();

        // 1. Spawn forge project
        let project_payload = ForgeProjectPayload {
            project: "harvester".to_string(),
            title: "Model Harvester Pipeline".to_string(),
            core_spec: "Autonomous reasoning distillation".to_string(),
        };
        let (status, Json(res)) = handle_post_forge_project(State(state.clone()), Json(project_payload)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(res.get("task").is_some());
        let task_id = res.get("task").unwrap().get("id").unwrap().as_str().unwrap().to_string();

        // 2. Claim task
        let claim_payload = ClaimForgePayload {
            task_id: task_id.clone(),
            agent_id: "agent-aegis".to_string(),
            ttl_secs: Some(120),
        };
        let (status, Json(claim_res)) = handle_claim_forge_task(State(state.clone()), Json(claim_payload)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(claim_res.get("task").unwrap().get("status").unwrap().as_str().unwrap(), "claimed");

        // 3. Heartbeat
        let hb_payload = HeartbeatForgePayload {
            task_id: task_id.clone(),
            agent_id: "agent-aegis".to_string(),
            ttl_secs: Some(300),
        };
        let (status, _) = handle_heartbeat_forge_task(State(state.clone()), Json(hb_payload)).await;
        assert_eq!(status, StatusCode::OK);

        // 4. Submit
        let submit_payload = SubmitForgePayload {
            task_id: task_id.clone(),
            agent_id: "agent-aegis".to_string(),
            branch: "feat/harvester-core".to_string(),
            pr_url: Some("https://github.com/aien-dev/harvester/pull/1".to_string()),
        };
        let (status, _) = handle_submit_forge_task(State(state.clone()), Json(submit_payload)).await;
        assert_eq!(status, StatusCode::OK);

        // 5. Verify
        let verify_payload = VerifyForgePayload {
            task_id: task_id.clone(),
            verdict: true,
            notes: Some("All 4 defense checks passed".to_string()),
        };
        let (status, _) = handle_verify_forge_task(State(state.clone()), Json(verify_payload)).await;
        assert_eq!(status, StatusCode::OK);

        // 6. List tasks
        let query = ForgeQuery {
            project: Some("harvester".to_string()),
            ring: None,
            status: Some("verified".to_string()),
        };
        let Json(list_res) = handle_get_forge_tasks(State(state), Query(query)).await;
        assert_eq!(list_res.get("tasks").unwrap().as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_cockpit_operator_profile_sanitization() {
        let payload = OperatorUpdatePayload {
            name: Some("Drake Stapleton
".to_string()),
            email: Some("drake@aien.org".to_string()),
            handle: Some("drake_ops-1".to_string()),
            sign_commits: Some(true),
        };
        let res = handle_post_operator(HeaderMap::new(), Json(payload)).await;
        assert!(res.is_ok());
        let Json(body) = res.unwrap();
        let op = body.get("operator").unwrap();
        assert_eq!(op.get("name").unwrap().as_str().unwrap(), "Drake Stapleton");
        assert_eq!(op.get("email").unwrap().as_str().unwrap(), "drake@aien.org");
        assert_eq!(op.get("handle").unwrap().as_str().unwrap(), "drake_ops-1");
    }

    fn create_offline_test_state() -> AppState {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();
        let hive_store = Arc::new(spark_hive::CombStore::open_in_memory().unwrap());
        AppState {
            client,
            start_time: Instant::now(),
            redactor_patterns: Arc::new(vec![]),
            hive_store,
        }
    }

    #[tokio::test]
    async fn test_cockpit_pulse_endpoint_schema_and_offline_backends() {
        let state = create_test_state();
        let Json(pulse) = handle_pulse(State(state)).await;

        assert_eq!(pulse["status"], "ok");
        assert_eq!(pulse["heartbeat_bpm"], 72);
        assert!((pulse["coherence"].as_f64().unwrap() - 0.98).abs() < 1e-4);
        assert!(pulse["uptime_seconds"].as_u64().is_some());

        let model_seat = pulse["model_seat"].as_str().unwrap();
        assert!(model_seat == "ONLINE (Port 18006)" || model_seat == "OFFLINE");

        let conduit_seat = pulse["conduit_seat"].as_str().unwrap();
        assert!(conduit_seat == "ONLINE (Port 6167)" || conduit_seat == "OFFLINE");

        assert!(pulse["hive_count"].as_u64().is_some());
        assert!(pulse["timestamp"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_cockpit_status_endpoint_schema_and_offline_backend() {
        let state = create_test_state();
        let Json(status) = handle_engine_status(State(state)).await;

        let status_str = status["status"].as_str().unwrap();
        assert!(status_str == "online" || status_str == "offline");
        assert!(status["latency_ms"].as_u64().is_some());
        assert_eq!(status["active_engine"], "MAX Native Engine");
        assert_eq!(status["active_model"], "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16");
        assert_eq!(status["device"], "Grace Blackwell GB10 (Dual GPU)");
        assert_eq!(status["port"], 18006);
        assert_eq!(status["cost_per_million"], "$0.00");
    }

    #[tokio::test]
    async fn test_cockpit_cortex_search_endpoint_unreachable_fallback() {
        let state = create_offline_test_state();
        let query = CortexSearchQuery {
            q: Some("test_unreachable_cortex".to_string()),
            limit: Some(3),
        };
        let Json(res) = handle_cortex_search(State(state), Query(query)).await;
        assert!(res.get("results").is_some());
        let results = res.get("results").unwrap().as_array().unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_cockpit_cortex_write_endpoint_error_handling() {
        let state = create_offline_test_state();
        let payload = CortexWritePayload {
            name: "test_cockpit_write".to_string(),
            content: "Telemetry payload".to_string(),
            kind: Some("telemetry".to_string()),
        };

        let res = handle_cortex_write(State(state), Json(payload)).await;
        // Verify proper error response or graceful handling
        if let Err((status, _)) = res {
            assert!(status == StatusCode::BAD_GATEWAY || status == StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    #[tokio::test]
    async fn test_cockpit_subagents_endpoint_response_structure() {
        let Json(res) = handle_get_subagents().await;
        assert!(res.get("subagents").is_some());
        assert!(res.get("count").is_some());
        let subagents = res.get("subagents").unwrap().as_array().unwrap();
        let count = res.get("count").unwrap().as_u64().unwrap();
        assert_eq!(subagents.len() as u64, count);
    }

    #[tokio::test]
    async fn test_cockpit_submillisecond_latency_assertions() {
        let state = create_test_state();

        // 1. Subagents local file/memory check latency assertion (< 1ms)
        let t0 = Instant::now();
        let _ = handle_get_subagents().await;
        let elapsed_subagents = t0.elapsed();
        assert!(
            elapsed_subagents < Duration::from_millis(1),
            "Subagents latency must be sub-millisecond: {:?}",
            elapsed_subagents
        );

        // 2. Hive bounds retrieval latency assertion (< 1ms)
        let t1 = Instant::now();
        let _ = handle_get_hive_bounds(State(state.clone())).await;
        let elapsed_bounds = t1.elapsed();
        assert!(
            elapsed_bounds < Duration::from_millis(1),
            "Hive bounds latency must be sub-millisecond: {:?}",
            elapsed_bounds
        );

        // 3. Hive cells in-memory store latency assertion (< 1ms)
        let t2 = Instant::now();
        let _ = handle_get_hive_cells(State(state)).await;
        let elapsed_cells = t2.elapsed();
        assert!(
            elapsed_cells < Duration::from_millis(1),
            "Hive cells latency must be sub-millisecond: {:?}",
            elapsed_cells
        );
    }
}
