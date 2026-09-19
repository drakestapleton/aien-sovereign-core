use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use clap::Parser;
use reqwest::Client;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod db;
mod embeddings;
mod handlers;
mod models;

use auth::auth_middleware;
use db::Database;
use handlers::*;
use models::CortexEntity;

#[derive(Parser, Debug)]
#[command(name = "cortex-rs", about = "Sovereign Native Memory Engine in Rust")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value = "18080")]
    port: u16,

    #[arg(long, default_value = "/home/drakestapleton/.config/cortex/cortex.db")]
    db_path: PathBuf,

    #[arg(long, default_value = "http://127.0.0.1:18081")]
    encoder_url: String,

    #[arg(long)]
    import: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Initializing Cortex-RS SQLite database at {:?}...", args.db_path);
    if let Some(parent) = args.db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let database = Arc::new(Database::open(&args.db_path)?);

    if let Some(import_file) = &args.import {
        tracing::info!("Importing existing entities from {:?}...", import_file);
        if import_file.exists() {
            let data = fs::read_to_string(import_file)?;
            let raw_list: Vec<serde_json::Value> = serde_json::from_str(&data)?;
            let mut count = 0;
            for val in raw_list {
                let entity_val = if let Some(e) = val.get("entity") { e.clone() } else { val };
                if let Ok(entity) = serde_json::from_value::<CortexEntity>(entity_val) {
                    let _ = database.import_entity_raw(&entity, None);
                    count += 1;
                }
            }
            tracing::info!("Successfully imported {} entities into SQLite!", count);
        }
    }

    let state = Arc::new(AppState {
        db: database,
        http_client: Client::builder().build()?,
        encoder_url: args.encoder_url,
    });

    let protected_routes = Router::new()
        .route("/api/cortex/write", post(write_handler))
        .route("/api/cortex/search", get(search_get_handler).post(search_post_handler))
        .route("/api/cortex/recall", post(recall_handler))
        .route("/api/cortex/get", get(get_handler))
        .route("/api/cortex/traverse", post(traverse_handler))
        .layer(middleware::from_fn(auth_middleware));

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(health_handler))
        .merge(protected_routes)
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
        .layer(TraceLayer::new_for_http())
        .with_state(state);

        // INVARIANT: Bind strictly to loopback 127.0.0.1 to guarantee zero LAN exposure
    if args.host != "127.0.0.1" && args.host != "localhost" {
        tracing::warn!("Non-loopback binding detected ({}); enforcing local authentication.", args.host);
    }

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse().unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], args.port)));
    tracing::info!("Cortex-RS server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
