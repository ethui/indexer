mod app;
mod auth;
mod error;
mod test_utils;

use std::net::SocketAddr;

use tokio::task::JoinHandle;
use tracing::instrument;

use self::app::app;
use crate::{
    config::{HttpConfig, WhitelistConfig},
    db::Db,
};

#[allow(clippy::async_yields_async)]
#[instrument(name = "api", skip(db, config), fields(port = config.port))]
pub async fn start(
    db: Db,
    config: HttpConfig,
    whitelist: WhitelistConfig,
) -> JoinHandle<Result<(), std::io::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let app = app(db.clone(), config.jwt_secret(), whitelist);

    tokio::spawn(async move { axum::serve(listener, app).await })
}
