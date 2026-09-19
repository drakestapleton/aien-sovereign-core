use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use ort::session::{builder::GraphOptimizationLevel, Session};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokenizers::Tokenizer;
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub const MODEL_ID: &str = "BAAI/bge-base-en-v1.5";
pub const MODEL_REVISION: &str = "cortex-bge-base-en-v1.5-768-v1";
pub const ONNX_RUNTIME_LIB: &str =
    "/home/drakestapleton/max-env/lib/python3.12/site-packages/onnxruntime/capi/libonnxruntime.so.1.30.0";
pub const EMBED_DIR: &str =
    "/home/drakestapleton/.cache/huggingface/hub/models--Xenova--bge-base-en-v1.5/snapshots/4d6cd88e18e51a5e020c2c305726d76ada9c03cf";
pub const RERANK_DIR: &str =
    "/home/drakestapleton/.cache/huggingface/hub/models--Xenova--bge-reranker-base/snapshots/280bcc27a84e0b898c251e06fddb25171bd9b101";

#[derive(Parser, Debug)]
#[command(name = "cortex-encoder-rs", about = "Native Rust and ONNX Runtime C-API embedding and reranking engine")]
pub struct Cli {
    #[arg(short, long, default_value = "18081")]
    pub port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}

#[derive(Clone)]
pub struct AppState {
    pub embed_session: Arc<Mutex<Session>>,
    pub embed_tokenizer: Arc<Tokenizer>,
    pub rerank_session: Arc<Mutex<Session>>,
    pub rerank_tokenizer: Arc<Tokenizer>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedRequest {
    pub text: String,
    #[serde(rename = "modelId")]
    pub _model_id: Option<String>,
    #[serde(rename = "modelRevision")]
    pub _model_revision: Option<String>,
    #[serde(rename = "priority")]
    pub _priority: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmbedResponse {
    pub embedding: Vec<f32>,
    #[serde(rename = "modelId")]
    pub model_id: &'static str,
    #[serde(rename = "modelRevision")]
    pub model_revision: &'static str,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: f64,
}

#[derive(Debug, Deserialize)]
pub struct RerankRequest {
    pub query: String,
    pub passages: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RerankResponse {
    pub scores: Vec<f32>,
    pub count: usize,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: f64,
}

#[derive(Debug, Deserialize)]
pub struct SalienceRequest {
    pub user: String,
    pub assistant: String,
}

#[derive(Debug, Serialize)]
pub struct SalienceResponse {
    #[serde(rename = "isDurable")]
    pub is_durable: bool,
    #[serde(rename = "durabilityScore")]
    pub durability_score: f64,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: f64,
}

pub fn l2_normalize(vec: &[f32]) -> Vec<f32> {
    let mut sum_sq = 0.0f32;
    for &val in vec {
        sum_sq += val * val;
    }
    let norm = sum_sq.sqrt();
    if norm > 0.0 {
        vec.iter().map(|&v| v / norm).collect()
    } else {
        vec.to_vec()
    }
}

pub fn calculate_sigmoid(raw_score: f32) -> f32 {
    1.0f32 / (1.0f32 + (-raw_score).exp())
}

pub async fn handle_health() -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "modelId": MODEL_ID,
        "modelRevision": MODEL_REVISION,
        "backend": "ONNX Runtime 1.30.0 (Native Rust Axum INT8 CPU)",
        "models": {
            "embeddings": "Xenova/bge-base-en-v1.5 (ONNX INT8)",
            "reranker": "Xenova/bge-reranker-base (ONNX INT8)",
            "salience": "Xenova/bge-reranker-base (ONNX INT8)"
        },
        "features": ["embed", "rerank", "classify_salience"]
    }))
}

pub async fn handle_embed(
    State(state): State<AppState>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, (StatusCode, String)> {
    let trimmed = req.text.trim();
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "text must not be empty".to_string()));
    }

    let t0 = Instant::now();

    let encoding = state
        .embed_tokenizer
        .encode(trimmed, true)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tokenization failed: {}", e)))?;

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
    let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
    let seq_len = input_ids.len();

    let input_ids_tensor = ort::value::Tensor::from_array(([1, seq_len], input_ids))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor creation error: {}", e)))?;
    let attention_mask_tensor = ort::value::Tensor::from_array(([1, seq_len], attention_mask))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor creation error: {}", e)))?;
    let token_type_ids_tensor = ort::value::Tensor::from_array(([1, seq_len], token_type_ids))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor creation error: {}", e)))?;

    let mut session = state.embed_session.lock().await;
    let outputs = session
        .run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => token_type_ids_tensor,
        ])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Inference error: {}", e)))?;

    let (_shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor extraction error: {}", e)))?;

    if data.len() < 768 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Output tensor dim too small: {}", data.len()),
        ));
    }

    let cls_token = &data[0..768];
    let normalized = l2_normalize(cls_token);
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(EmbedResponse {
        embedding: normalized,
        model_id: MODEL_ID,
        model_revision: MODEL_REVISION,
        elapsed_ms: (elapsed * 100.0).round() / 100.0,
    }))
}

pub async fn handle_rerank(
    State(state): State<AppState>,
    Json(req): Json<RerankRequest>,
) -> Result<Json<RerankResponse>, (StatusCode, String)> {
    if req.passages.is_empty() {
        return Ok(Json(RerankResponse {
            scores: Vec::new(),
            count: 0,
            elapsed_ms: 0.0,
        }));
    }

    let t0 = Instant::now();
    let mut scores = Vec::with_capacity(req.passages.len());

    let mut session = state.rerank_session.lock().await;

    for passage in &req.passages {
        let max_len = passage.len().min(512);
        let trimmed_passage = &passage[..max_len];

        let encoding = state
            .rerank_tokenizer
            .encode((req.query.as_str(), trimmed_passage), true)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tokenization failed: {}", e)))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
        let seq_len = input_ids.len();

        let input_ids_tensor = ort::value::Tensor::from_array(([1, seq_len], input_ids))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor error: {}", e)))?;
        let attention_mask_tensor = ort::value::Tensor::from_array(([1, seq_len], attention_mask))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor error: {}", e)))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
            ])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Rerank inference error: {}", e)))?;

        let (_shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Extraction error: {}", e)))?;

        let raw_score = data[0];
        scores.push(calculate_sigmoid(raw_score));
    }

    let count = scores.len();
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(RerankResponse {
        scores,
        count,
        elapsed_ms: (elapsed * 100.0).round() / 100.0,
    }))
}

pub async fn handle_classify_salience(
    State(state): State<AppState>,
    Json(req): Json<SalienceRequest>,
) -> Result<Json<SalienceResponse>, (StatusCode, String)> {
    let t0 = Instant::now();

    let user_slice = &req.user[..req.user.len().min(300)];
    let asst_slice = &req.assistant[..req.assistant.len().min(300)];
    let text = format!("User: {}\nAssistant: {}", user_slice, asst_slice);
    let anchor = "Important architectural decision, user preference mandate, or permanent system configuration change.";

    let encoding = state
        .rerank_tokenizer
        .encode((anchor, text.as_str()), true)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tokenization failed: {}", e)))?;

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
    let seq_len = input_ids.len();

    let input_ids_tensor = ort::value::Tensor::from_array(([1, seq_len], input_ids))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor error: {}", e)))?;
    let attention_mask_tensor = ort::value::Tensor::from_array(([1, seq_len], attention_mask))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Tensor error: {}", e)))?;

    let mut session = state.rerank_session.lock().await;
    let outputs = session
        .run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
        ])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Salience inference error: {}", e)))?;

    let (_shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Extraction error: {}", e)))?;

    let raw_score = data[0];
    let score = calculate_sigmoid(raw_score) as f64;
    let is_durable = score >= 0.20;
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(SalienceResponse {
        is_durable,
        durability_score: (score * 10000.0).round() / 10000.0,
        elapsed_ms: (elapsed * 100.0).round() / 100.0,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    info!("Initializing ONNX Runtime from {}", ONNX_RUNTIME_LIB);
    ort::init_from(ONNX_RUNTIME_LIB)
        .map_err(|e| format!("Failed to init ONNX runtime: {}", e))?
        .commit();

    let embed_tok_path = format!("{}/tokenizer.json", EMBED_DIR);
    let embed_model_path = format!("{}/onnx/model_quantized.onnx", EMBED_DIR);
    info!("Loading embedding tokenizer from {}", embed_tok_path);
    let embed_tokenizer = Arc::new(Tokenizer::from_file(&embed_tok_path).map_err(|e| e.to_string())?);
    info!("Loading embedding ONNX session from {}", embed_model_path);
    let embed_session = Arc::new(Mutex::new(
        Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .commit_from_file(&embed_model_path)?,
    ));

    let rerank_tok_path = format!("{}/tokenizer.json", RERANK_DIR);
    let rerank_model_path = format!("{}/onnx/model_quantized.onnx", RERANK_DIR);
    info!("Loading reranker tokenizer from {}", rerank_tok_path);
    let rerank_tokenizer = Arc::new(Tokenizer::from_file(&rerank_tok_path).map_err(|e| e.to_string())?);
    info!("Loading reranker ONNX session from {}", rerank_model_path);
    let rerank_session = Arc::new(Mutex::new(
        Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .commit_from_file(&rerank_model_path)?,
    ));

    let state = AppState {
        embed_session,
        embed_tokenizer,
        rerank_session,
        rerank_tokenizer,
    };

    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/embed", post(handle_embed))
        .route("/rerank", post(handle_rerank))
        .route("/classify_salience", post(handle_classify_salience))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;
    info!("cortex-encoder-rs listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize() {
        let vec = vec![3.0f32, 4.0f32];
        let norm = l2_normalize(&vec);
        assert!((norm[0] - 0.6).abs() < 1e-5);
        assert!((norm[1] - 0.8).abs() < 1e-5);

        let sum_sq: f32 = norm.iter().map(|x| x * x).sum();
        assert!((sum_sq - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_l2_normalize_zero() {
        let vec = vec![0.0f32, 0.0f32];
        let norm = l2_normalize(&vec);
        assert_eq!(norm, vec);
    }

    #[test]
    fn test_sigmoid_calculation() {
        let zero_score = calculate_sigmoid(0.0);
        assert!((zero_score - 0.5).abs() < 1e-5);

        let high_score = calculate_sigmoid(10.0);
        assert!(high_score > 0.999);

        let low_score = calculate_sigmoid(-10.0);
        assert!(low_score < 0.001);
    }

    #[test]
    fn test_model_constants() {
        assert_eq!(MODEL_ID, "BAAI/bge-base-en-v1.5");
        assert_eq!(MODEL_REVISION, "cortex-bge-base-en-v1.5-768-v1");
    }
}
