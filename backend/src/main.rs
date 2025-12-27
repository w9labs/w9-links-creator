use axum::{
    routing::{get, post},
    Router, Json,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(root))
        .route("/api/health", get(root))
        .route("/api/shorten", post(shorten));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "W9 Links Creator API"
}

async fn shorten() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": true, 
        "short_url": "https://tools.w9.nu/s/example",
        "message": "This is a placeholder for the link creator service"
    }))
}
