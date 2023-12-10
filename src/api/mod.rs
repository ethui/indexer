mod error;
mod routes;

use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;
use tracing::instrument;

use crate::{config::HttpConfig, db::Db};
use axum::{
    routing::{get, post},
    Router,
};

use self::routes::{health, register};

#[allow(clippy::async_yields_async)]
#[instrument(name = "api", skip(db, config), fields(port = config.port))]
pub async fn start(db: Db, config: HttpConfig) -> JoinHandle<Result<(), std::io::Error>> {
    let app: Router = Router::new()
        .route("/health", get(health))
        .route("/register", post(register))
        .layer(CorsLayer::permissive())
        .with_state(db);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    tokio::spawn(async move { axum::serve(listener, app).await })
}
