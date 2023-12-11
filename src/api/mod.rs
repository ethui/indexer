mod auth;
mod error;
mod routes;

use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;
use tracing::instrument;

use crate::{config::HttpConfig, db::Db};

use self::routes::router;

#[allow(clippy::async_yields_async)]
#[instrument(name = "api", skip(db, config), fields(port = config.port))]
pub async fn start(db: Db, config: HttpConfig) -> JoinHandle<Result<(), std::io::Error>> {
    tokio::spawn(async move {
        let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

        let app = router().layer(CorsLayer::permissive()).with_state(db);
        axum::serve(listener, app).await
    })
}
