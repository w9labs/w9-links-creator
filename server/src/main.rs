use axum::{
    extract::State, http::StatusCode, response::Html, routing::{get, post}, Json, Router,
    extract::Path, response::IntoResponse,
};
use chrono::{Utc, Duration};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_postgres::{Client, NoTls};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use nanoid::nanoid;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Client>,
    pub base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLinkReq {
    pub url: String,
    pub domain: Option<String>,
    pub code: Option<String>,
    pub title: Option<String>,
    pub ttl_hours: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateNoteReq {
    pub content: String,
    pub password: Option<String>,
    pub ttl_hours: Option<i64>,
    pub max_views: Option<i32>,
}

fn html_root() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html><html><head><title>W9 Links</title></head><body style="background:#160c13;color:#fce126;font-family:monospace;text-align:center;padding:3rem"><h1>W9 LINKS</h1><p>Short Links &amp; Note Drops — PostgreSQL</p></body></html>"#)
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.query_one("SELECT 1", &[]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({
            "status": "ok", "service": "w9-links-creator", "database": "connected",
            "timestamp": Utc::now().to_rfc3339()
        }))),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "status": "error", "service": "w9-links-creator", "error": e.to_string()
        }))),
    }
}

async fn handle_create_link(
    State(state): State<AppState>,
    Json(req): Json<CreateLinkReq>,
) -> impl IntoResponse {
    let code = req.code.unwrap_or_else(|| nanoid!(8));
    let domain = req.domain.unwrap_or_else(|| "w9.nu".into());
    let expires_at = req.ttl_hours.map(|h| Utc::now() + Duration::hours(h));
    let id = Uuid::new_v4();
    match state.db.execute(
        "INSERT INTO short_links (id, code, target_url, domain, title, expires_at) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&id, &code, &req.url, &domain, &req.title, &expires_at],
    ).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({
            "code": code, "url": format!("https://{}/s/{}", domain, code),
            "target": req.url, "domain": domain,
        }))),
        Err(e) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

async fn handle_redirect(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    let row = match state.db.query_opt(
        "SELECT target_url FROM short_links WHERE code = $1 AND (expires_at IS NULL OR expires_at > $2)",
        &[&code, &Utc::now()],
    ).await {
        Ok(Some(r)) => r,
        _ => return (StatusCode::NOT_FOUND, Html("Not found")).into_response(),
    };
    let target: String = row.get("target_url");
    let _ = state.db.execute("UPDATE short_links SET clicks = clicks + 1 WHERE code = $1", &[&code]).await;
    (StatusCode::FOUND, [(axum::http::header::LOCATION, target.as_str())]).into_response()
}

fn hash_pw(pw: &str) -> String {
    let mut h = Sha256::new();
    h.update(pw.as_bytes());
    format!("{:x}", h.finalize())
}

async fn handle_create_note(
    State(state): State<AppState>,
    Json(req): Json<CreateNoteReq>,
) -> impl IntoResponse {
    let code = nanoid!(8);
    let expires_at = Utc::now() + Duration::hours(req.ttl_hours.unwrap_or(24));
    let pw_hash = req.password.as_ref().map(|pw| hash_pw(pw));
    let id = Uuid::new_v4();
    match state.db.execute(
        "INSERT INTO notes (id, code, content, password_hash, expires_at, max_views) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&id, &code, &req.content, &pw_hash, &expires_at, &req.max_views],
    ).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({
            "code": code, "url": format!("https://w9.nu/n/{}", code),
        }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

async fn handle_get_note(
    State(state): State<AppState>,
    Path(code): Path<String>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let row = match state.db.query_opt(
        "SELECT content, password_hash, views, max_views FROM notes WHERE code = $1 AND expires_at > $2",
        &[&code, &Utc::now()],
    ).await {
        Ok(Some(r)) => r,
        _ => return (StatusCode::NOT_FOUND, Html("Note not found or expired")).into_response(),
    };
    let content: String = row.get("content");
    let pw_hash: Option<String> = row.get("password_hash");
    let views: i32 = row.get("views");
    let max_views: Option<i32> = row.get("max_views");
    if let Some(mx) = max_views { if views >= mx {
        let _ = state.db.execute("DELETE FROM notes WHERE code = $1", &[&code]).await;
        return (StatusCode::GONE, Html("Note consumed")).into_response();
    }}
    if pw_hash.is_some() {
        if let Some(auth) = headers.get("X-Note-Password").and_then(|v| v.to_str().ok()) {
            if pw_hash.as_ref() != Some(&hash_pw(auth)) {
                return (StatusCode::UNAUTHORIZED, Html("Wrong password")).into_response();
            }
        } else {
            return (StatusCode::UNAUTHORIZED, Html("Password required")).into_response();
        }
    }
    let _ = state.db.execute("UPDATE notes SET views = views + 1 WHERE code = $1", &[&code]).await;
    (StatusCode::OK, Json(serde_json::json!({"content": content}))).into_response()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer()).init();
    dotenvy::dotenv().ok();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8085".into());
    let db_url = std::env::var("W9_LINKS_DB_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://w9_admin:password@w9-postgres:5432/w9_links".into());
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "https://w9.nu".into());
    tracing::info!("Connecting to PostgreSQL...");
    let (client, conn) = tokio_postgres::connect(&db_url, NoTls).await?;
    tokio::spawn(async move { if let Err(e) = conn.await { tracing::error!("DB: {}", e); } });
    client.query_one("SELECT 1", &[]).await?;
    tracing::info!("Connected to PostgreSQL");
    let state = AppState { db: Arc::new(client), base_url };
    let router = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/links", post(handle_create_link))
        .route("/api/notes", post(handle_create_note))
        .route("/s/:code", get(handle_redirect))
        .route("/n/:code", get(handle_get_note))
        .fallback(|| async { html_root() })
        .with_state(state)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()).layer(CorsLayer::permissive()));
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("W9 Links listening on {}", addr);
    axum::serve(listener, router).await?;
    Ok(())
}
