mod crawler;
mod fetcher;
mod frontier;
mod parser;
mod storage;

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::info;

use crawler::CrawlerState;

type SharedState = Arc<Mutex<CrawlerState>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "web_crawler=info".into()),
        )
        .init();

    let output_path = PathBuf::from("crawl_output.jsonl");
    let max_pages: usize = 50;

    let state = Arc::new(Mutex::new(
        CrawlerState::new(&output_path, max_pages).expect("failed to initialise crawler state"),
    ));

    let app = Router::new()
        .route("/health", get(health))
        .route("/seed", post(seed_url))
        .route("/start", post(start_crawl))
        .route("/stop", post(stop_crawl))
        .route("/status", get(get_status))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:3000";
    info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

// ---------- Handlers ----------

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct SeedRequest {
    url: String,
}

#[derive(Serialize)]
struct SeedResponse {
    added: bool,
    pending: usize,
    total_seen: usize,
}

async fn seed_url(
    State(state): State<SharedState>,
    Json(body): Json<SeedRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let url = url::Url::parse(&body.url)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid URL: {e}")))?;

    let mut s = state.lock().await;
    let added = s.frontier.push(url);

    Ok(Json(SeedResponse {
        added,
        pending: s.frontier.pending(),
        total_seen: s.frontier.total_seen(),
    }))
}

#[derive(Serialize)]
struct StatusResponse {
    running: bool,
    pages_crawled: usize,
    max_pages: usize,
    pending_urls: usize,
    total_seen: usize,
    output_file: String,
}

async fn get_status(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.lock().await;
    Json(StatusResponse {
        running: s.running,
        pages_crawled: s.pages_crawled,
        max_pages: s.max_pages,
        pending_urls: s.frontier.pending(),
        total_seen: s.frontier.total_seen(),
        output_file: s.storage.path().display().to_string(),
    })
}

#[derive(Serialize)]
struct MessageResponse {
    message: String,
}

async fn start_crawl(State(state): State<SharedState>) -> impl IntoResponse {
    {
        let mut s = state.lock().await;
        if s.running {
            return (
                StatusCode::CONFLICT,
                Json(MessageResponse {
                    message: "crawler is already running".into(),
                }),
            );
        }
        if s.frontier.pending() == 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(MessageResponse {
                    message: "frontier is empty — seed URLs first via POST /seed".into(),
                }),
            );
        }
        s.running = true;
    }

    // Spawn the crawl loop in the background.
    let crawl_state = Arc::clone(&state);
    tokio::spawn(async move {
        crawler::crawl_loop(crawl_state).await;
    });

    (
        StatusCode::OK,
        Json(MessageResponse {
            message: "crawl started".into(),
        }),
    )
}

async fn stop_crawl(State(state): State<SharedState>) -> impl IntoResponse {
    let mut s = state.lock().await;
    if !s.running {
        return (
            StatusCode::CONFLICT,
            Json(MessageResponse {
                message: "crawler is not running".into(),
            }),
        );
    }
    s.running = false;
    (
        StatusCode::OK,
        Json(MessageResponse {
            message: "stop signal sent — crawler will halt after current page".into(),
        }),
    )
}
