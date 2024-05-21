mod auth;
mod error;
mod routes;
mod test_utils;

use std::net::SocketAddr;

use axum::Extension;
use jsonwebtoken::{DecodingKey, EncodingKey};
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;
use tracing::instrument;

use self::routes::router;
use crate::{config::HttpConfig, db::Db};

#[allow(clippy::async_yields_async)]
#[instrument(name = "api", skip(db, config), fields(port = config.port))]
pub async fn start(db: Db, config: HttpConfig) -> JoinHandle<Result<(), std::io::Error>> {
    tokio::spawn(async move {
        let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

        let jwt_secret = config.jwt_secret();
        let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());

        let app = router()
            .layer(CorsLayer::permissive())
            .layer(Extension(encoding_key))
            .layer(Extension(decoding_key))
            .with_state(db);
        axum::serve(listener, app).await
    })
}
