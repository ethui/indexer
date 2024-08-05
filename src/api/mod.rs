mod app;
mod app_state;
mod auth;
mod error;
mod test_utils;

use std::net::SocketAddr;

use tokio::task::JoinHandle;
use tracing::instrument;

use self::{app::app, app_state::AppState};
use crate::{config::Config, db::Db};

#[allow(clippy::async_yields_async)]
#[instrument(name = "api", skip(db, config), fields(port = config.http.clone().unwrap().port))]
pub async fn start(db: Db, config: Config) -> JoinHandle<Result<(), std::io::Error>> {
    let http_config = config.http.clone().unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], http_config.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    let state = AppState { db, config };
    let app = app(http_config.jwt_secret(), state);

    tokio::spawn(async move { axum::serve(listener, app).await })
}
