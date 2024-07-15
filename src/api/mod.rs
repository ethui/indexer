mod app;
mod auth;
mod error;
mod registration;
mod test_utils;

use std::net::SocketAddr;

use tokio::task::JoinHandle;
use tracing::instrument;

use self::app::app;
use crate::{config::HttpConfig, db::Db};

#[allow(clippy::async_yields_async)]
#[instrument(name = "api", skip(db, http), fields(port = http.port))]
pub async fn start(db: Db, http: HttpConfig) -> JoinHandle<Result<(), std::io::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], http.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let app = app(db.clone(), http.jwt_secret());

    tokio::spawn(async move { axum::serve(listener, app).await })
}
