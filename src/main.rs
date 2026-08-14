//! Server binary — thin wrapper that builds the app and binds to :3000.

use chat_room::{build_app, state::AppState};
use std::{net::SocketAddr, sync::Arc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "chat_room=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_path = std::env::var("CHAT_ROOM_DATABASE_PATH").ok();
    let state = Arc::new(
        AppState::load(database_path)
            .await
            .expect("initialize SQLite database"),
    );
    let app = build_app(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
