use crate::cortex_sync::CortexSync;
use crate::models::{EmailMessage, MailboxStatus, SendEmailRequest};
use crate::relay::MailRelay;
use crate::store::MailStore;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<MailStore>,
    pub cortex: Arc<CortexSync>,
    pub relay: Arc<MailRelay>,
    pub smtp_port: u16,
    pub api_port: u16,
    pub operator_email: String,
}

pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/api/mail/status", get(handle_status))
        .route("/api/mail/inbox", get(handle_list_inbox))
        .route("/api/mail/sent", get(handle_list_sent))
        .route("/api/mail/archive", get(handle_list_archive))
        .route("/api/mail/message/{id}", get(handle_get_message))
        .route("/api/mail/send", post(handle_send_mail))
        .route("/api/mail/ingest-test", post(handle_ingest_test))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

async fn handle_health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "spark-mail-rs",
        "version": "0.1.0"
    }))
}

async fn handle_status(State(state): State<ApiState>) -> Json<MailboxStatus> {
    let cortex_ok = state.cortex.check_health().await;
    let bridge_ok = state.relay.check_bridge().await;

    let status = state.store.get_status(
        &state.operator_email,
        state.smtp_port,
        state.api_port,
        cortex_ok,
        bridge_ok,
    );
    Json(status)
}

async fn handle_list_inbox(State(state): State<ApiState>) -> Json<Vec<EmailMessage>> {
    let messages = state.store.list_folder("inbox");
    Json(messages)
}

async fn handle_list_sent(State(state): State<ApiState>) -> Json<Vec<EmailMessage>> {
    let messages = state.store.list_folder("sent");
    Json(messages)
}

async fn handle_list_archive(State(state): State<ApiState>) -> Json<Vec<EmailMessage>> {
    let messages = state.store.list_folder("archive");
    Json(messages)
}

async fn handle_get_message(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<EmailMessage>, StatusCode> {
    state
        .store
        .get_message(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn handle_send_mail(
    State(state): State<ApiState>,
    Json(req): Json<SendEmailRequest>,
) -> Result<Json<EmailMessage>, (StatusCode, String)> {
    state
        .relay
        .send_email(req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn handle_ingest_test(
    State(state): State<ApiState>,
) -> Result<Json<EmailMessage>, (StatusCode, String)> {
    let test_req = SendEmailRequest {
        to: vec![state.operator_email.clone()],
        subject: "Sovereign Mail Node Online".to_string(),
        body: "Your sovereign mail server is running directly on Spark hardware. Messages are stored on local NVMe disk with zero cloud telemetry and synced to Atlas Cortex memory.".to_string(),
        from: Some("atlas@sovereign.spark".to_string()),
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut headers = std::collections::HashMap::new();
    headers.insert("From".to_string(), "atlas@sovereign.spark".to_string());
    headers.insert("To".to_string(), state.operator_email.clone());
    headers.insert("Subject".to_string(), test_req.subject.clone());

    let mut msg = EmailMessage {
        id: id.clone(),
        from: "atlas@sovereign.spark".to_string(),
        to: test_req.to,
        subject: test_req.subject,
        body: test_req.body,
        headers,
        received_at: now,
        folder: "inbox".to_string(),
        cortex_indexed: false,
    };

    state
        .store
        .save_message(&msg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if state.cortex.commit_mail(&msg).await.is_ok() {
        msg.cortex_indexed = true;
        let _ = state.store.save_message(&msg);
    }

    Ok(Json(msg))
}
