use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

use crate::db::Database;
use crate::embeddings::fetch_embedding;
use crate::models::*;

pub struct AppState {
    pub db: Arc<Database>,
    pub http_client: Client,
    pub encoder_url: String,
}

pub async fn write_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WritePayload>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    match payload {
        WritePayload::Entity { value } => {
            let mut emb = None;
            let text_to_embed = if !value.content.trim().is_empty() {
                format!("{}: {}", value.canonical_name, value.content)
            } else {
                value.canonical_name.clone()
            };

            match fetch_embedding(&state.http_client, &text_to_embed, &state.encoder_url).await {
                Ok(vec) => emb = Some(vec),
                Err(e) => {
                    tracing::warn!("Embedding encoder unavailable, proceeding with lexical only: {}", e);
                }
            }

            match state.db.upsert_entity(&value, emb.as_deref()) {
                Ok(receipt) => Ok((StatusCode::CREATED, Json(CortexReceipt { recorded: true, receipt })).into_response()),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
            }
        }
        WritePayload::Claim { value } => {
            match state.db.upsert_claim(&value) {
                Ok(receipt) => Ok((StatusCode::CREATED, Json(CortexReceipt { recorded: true, receipt })).into_response()),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
            }
        }
        WritePayload::Retract { value } => {
            match state.db.retract_target(&value.target_type, &value.target_id) {
                Ok(receipt) => Ok((StatusCode::CREATED, Json(CortexReceipt { recorded: true, receipt })).into_response()),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
            }
        }
    }
}

pub async fn search_get_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<serde_json::Value>)> {
    let start = Instant::now();
    let q = params.q.or(params.query).unwrap_or_default();
    let limit = params.limit.unwrap_or(12).min(50);
    let space = params.space.as_deref();

    match state.db.search_entities(&q, space, limit) {
        Ok(results) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            Ok(Json(SearchResponse {
                results,
                degraded: Vec::new(),
                elapsed_ms,
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
    }
}

pub async fn search_post_handler(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SearchParams>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<serde_json::Value>)> {
    let start = Instant::now();
    let q = params.q.or(params.query).unwrap_or_default();
    let limit = params.limit.unwrap_or(12).min(50);
    let space = params.space.as_deref();

    match state.db.search_entities(&q, space, limit) {
        Ok(results) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            Ok(Json(SearchResponse {
                results,
                degraded: Vec::new(),
                elapsed_ms,
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
    }
}

pub async fn recall_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RecallPayload>,
) -> Result<Json<RecallResponse>, (StatusCode, Json<serde_json::Value>)> {
    let start = Instant::now();
    let space = payload.space.as_deref();
    let limit = payload.limit.min(20);

    let mut emb = None;
    match fetch_embedding(&state.http_client, &payload.query, &state.encoder_url).await {
        Ok(vec) => emb = Some(vec),
        Err(e) => {
            tracing::warn!("Embedding encoder unavailable for recall: {}", e);
        }
    }

    match state.db.recall_entities(&payload.query, emb.as_deref(), space, limit) {
        Ok(results) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            Ok(Json(RecallResponse {
                results,
                degraded: Vec::new(),
                elapsed_ms,
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
    }
}

#[derive(serde::Deserialize)]
pub struct GetParams {
    pub id: Option<String>,
    pub name: Option<String>,
    pub space: Option<String>,
}

pub async fn get_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GetParams>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let target = params.id.or(params.name).unwrap_or_default();
    if target.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Missing id or name parameter"}))));
    }

    match state.db.get_entity(&target, params.space.as_deref()) {
        Ok(Some(entity)) => Ok(Json(entity).into_response()),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Entity not found"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
    }
}

pub async fn traverse_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TraversePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state.db.traverse_claims(&payload.subject_id, payload.space.as_deref()) {
        Ok(claims) => Ok(Json(json!({"claims": claims}))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
    }
}

pub async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "cortex-rs",
        "version": "0.1.0",
        "space": "atlas-memory",
        "runtime": "native-arm64-rust"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> Arc<AppState> {
        let db = Arc::new(Database::open_in_memory().expect("in memory db"));
        Arc::new(AppState {
            db,
            http_client: Client::new(),
            encoder_url: "http://127.0.0.1:18081".to_string(),
        })
    }

    #[tokio::test]
    async fn test_get_handler_missing_parameter() {
        let state = create_test_state();
        let query = GetParams {
            id: None,
            name: None,
            space: None,
        };
        let err = get_handler(State(state), Query(query)).await.unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert_eq!(err.1 .0["error"], "Missing id or name parameter");
    }

    #[tokio::test]
    async fn test_get_handler_not_found() {
        let state = create_test_state();
        let query = GetParams {
            id: Some("non_existent_uuid".to_string()),
            name: None,
            space: Some("atlas-memory".to_string()),
        };
        let err = get_handler(State(state), Query(query)).await.unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
        assert_eq!(err.1 .0["error"], "Entity not found");
    }

    #[tokio::test]
    async fn test_get_handler_success_and_unknown_space() {
        let state = create_test_state();
        let input = EntityWriteInput {
            id: None,
            space: "atlas-memory".to_string(),
            entity_type: "discovery".to_string(),
            canonical_name: "test_entity_get".to_string(),
            content: "Payload content".to_string(),
            aliases: vec![],
            metadata: json!({"key": "val"}),
            confidence: 1.0,
            valid_from: None,
            valid_to: None,
            external_id: None,
        };
        state.db.upsert_entity(&input, None).unwrap();

        // 1. Success fetch
        let query_ok = GetParams {
            id: None,
            name: Some("test_entity_get".to_string()),
            space: Some("atlas-memory".to_string()),
        };
        let resp = get_handler(State(state.clone()), Query(query_ok)).await;
        assert!(resp.is_ok());

        // 2. Unknown space fetch returns 404
        let query_unknown_space = GetParams {
            id: None,
            name: Some("test_entity_get".to_string()),
            space: Some("non_existent_space".to_string()),
        };
        let err_unknown = get_handler(State(state), Query(query_unknown_space)).await.unwrap_err();
        assert_eq!(err_unknown.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_search_unknown_space_returns_empty() {
        let state = create_test_state();
        let params = SearchParams {
            q: Some("anything".to_string()),
            query: None,
            space: Some("completely_unknown_space_slug".to_string()),
            limit: Some(10),
            include_retracted: Some(false),
        };
        let Json(res) = search_get_handler(State(state), Query(params)).await.unwrap();
        assert_eq!(res.results.len(), 0);
    }

    #[tokio::test]
    async fn test_write_entity_and_retract_lifecycle() {
        let state = create_test_state();

        // 1. Write entity
        let write_payload = WritePayload::Entity {
            value: EntityWriteInput {
                id: None,
                space: "atlas-memory".to_string(),
                entity_type: "lesson".to_string(),
                canonical_name: "lifecycle_test".to_string(),
                content: "Lifecycle verification".to_string(),
                aliases: vec![],
                metadata: json!({}),
                confidence: 1.0,
                valid_from: None,
                valid_to: None,
                external_id: None,
            },
        };
        let write_res = write_handler(State(state.clone()), Json(write_payload)).await;
        assert!(write_res.is_ok());

        let entity = state.db.get_entity("lifecycle_test", Some("atlas-memory")).unwrap().unwrap();
        assert_eq!(entity.canonical_name, "lifecycle_test");

        // 2. Retract entity
        let retract_payload = WritePayload::Retract {
            value: RetractInput {
                target_type: "entity".to_string(),
                target_id: entity.id.clone(),
                reason: Some("Obsolescence".to_string()),
            },
        };
        let retract_res = write_handler(State(state.clone()), Json(retract_payload)).await;
        assert!(retract_res.is_ok());

        let after = state.db.get_entity("lifecycle_test", Some("atlas-memory")).unwrap();
        assert!(after.is_none());
    }

    #[tokio::test]
    async fn test_health_handler_structure() {
        let Json(health) = health_handler().await;
        assert_eq!(health["status"], "ok");
        assert_eq!(health["service"], "cortex-rs");
        assert_eq!(health["space"], "atlas-memory");
    }
}
