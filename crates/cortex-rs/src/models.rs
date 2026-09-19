#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CortexEntity {
    pub id: String,
    pub space_id: String,
    pub space_slug: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub content: String,
    pub aliases: Vec<String>,
    pub metadata: Value,
    pub confidence: f64,
    pub revision: i64,
    pub retracted: bool,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CortexClaim {
    pub id: String,
    pub space_id: String,
    pub space_slug: String,
    pub subject_entity_id: String,
    pub predicate: String,
    pub object_entity_id: Option<String>,
    pub literal_value: Option<Value>,
    pub confidence: f64,
    pub metadata: Value,
    pub retracted: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptDetails {
    pub id: String,
    pub operation: String,
    pub target_type: String,
    pub target_id: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CortexReceipt {
    pub recorded: bool,
    pub receipt: ReceiptDetails,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityWriteInput {
    pub id: Option<String>,
    #[serde(default = "default_space")]
    pub space: String,
    #[serde(default = "default_entity_type")]
    pub entity_type: String,
    pub canonical_name: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "default_json_object")]
    pub metadata: Value,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub external_id: Option<String>,
}

fn default_space() -> String { "atlas-memory".to_string() }
fn default_entity_type() -> String { "discovery".to_string() }
fn default_json_object() -> Value { serde_json::json!({}) }
fn default_confidence() -> f64 { 1.0 }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimWriteInput {
    pub id: Option<String>,
    #[serde(default = "default_space")]
    pub space: String,
    pub subject_entity_id: String,
    pub predicate: String,
    pub object_entity_id: Option<String>,
    pub literal_value: Option<Value>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_json_object")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetractInput {
    pub target_type: String,
    pub target_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub enum WritePayload {
    #[serde(rename = "entity")]
    Entity { value: EntityWriteInput },
    #[serde(rename = "claim")]
    Claim { value: ClaimWriteInput },
    #[serde(rename = "retract")]
    Retract { value: RetractInput },
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    pub space: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: Option<usize>,
    #[serde(default)]
    pub include_retracted: Option<bool>,
}

fn default_search_limit() -> Option<usize> { Some(12) }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    #[serde(flatten)]
    pub entity: CortexEntity,
    pub lexical_score: f64,
    pub graph_score: f64,
    pub semantic_score: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub degraded: Vec<String>,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallPayload {
    pub query: String,
    pub space: Option<String>,
    #[serde(default = "default_recall_limit")]
    pub limit: usize,
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,
}

fn default_recall_limit() -> usize { 5 }
fn default_token_budget() -> usize { 1200 }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallComponentScores {
    pub lexical: f64,
    pub semantic: f64,
    pub confidence_recency: f64,
    pub graph: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallItem {
    pub entity: CortexEntity,
    pub final_score: f64,
    pub component_scores: RecallComponentScores,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecallResponse {
    pub results: Vec<RecallItem>,
    pub degraded: Vec<String>,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraversePayload {
    pub subject_id: String,
    pub space: Option<String>,
}
