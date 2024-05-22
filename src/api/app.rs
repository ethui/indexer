use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use ethers_core::types::Signature;
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use super::{
    auth::IndexerAuth,
    error::{ApiError, ApiResult},
};
use crate::{config::HttpConfig, db::Db};

pub fn app(db: Db, config: HttpConfig) -> Router {
    let jwt_secret = config.jwt_secret();
    let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
    let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());

    Router::new()
        .route("/health", get(health))
        .route("/auth", post(auth))
        .route("/register", post(register))
        .layer(CorsLayer::permissive())
        .layer(Extension(encoding_key))
        .layer(Extension(decoding_key))
        .with_state(db)
}

async fn health() -> impl IntoResponse {}

#[derive(Debug, Deserialize)]
struct Register {
    address: alloy_primitives::Address,
}

async fn register(
    _: IndexerAuth,
    State(db): State<Db>,
    Json(register): Json<Register>,
) -> ApiResult<impl IntoResponse> {
    // TODO this registration needs to be verified (is the user whitelisted? did the user pay?)
    db.register(register.address.into()).await?;

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    signature: Signature,
    data: IndexerAuth,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    access_token: String,
}

pub async fn auth(
    Extension(encoding_key): Extension<EncodingKey>,
    Json(auth): Json<AuthRequest>,
) -> ApiResult<impl IntoResponse> {
    auth.data
        .check(&auth.signature)
        .map_err(|_| ApiError::InvalidCredentials)?;

    let access_token = encode(&Header::default(), &auth.data, &encoding_key)?;

    // Send the authorized token
    Ok(Json(AuthResponse { access_token }))
}

#[cfg(test)]
mod test {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing, Extension, Router,
    };
    use color_eyre::Result;
    use ethers_core::types::Address;
    use jsonwebtoken::{DecodingKey, EncodingKey};
    use rstest::{fixture, rstest};
    use serde::Serialize;
    use serial_test::serial;
    use tower::ServiceExt;

    use super::{auth, AuthRequest};
    use crate::{
        api::{
            auth::IndexerAuth,
            test_utils::{address, now, sign_typed_data},
        },
        db::Db,
    };

    fn post<B: Serialize>(uri: &str, body: B) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    #[fixture]
    async fn app() -> Router {
        let jwt_secret = "secret".to_owned();
        let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());
        let db = Db::connect_test().await.unwrap();

        Router::new()
            .route("/auth", routing::post(auth))
            .layer(Extension(encoding_key))
            .layer(Extension(decoding_key))
            .with_state(db)
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth(#[future(awt)] app: Router, address: Address, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;
        let data: IndexerAuth = IndexerAuth::new(address, valid_until);

        let req = post(
            "/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                data,
            },
        );

        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth_expired_signature(
        #[future(awt)] app: Router,
        address: Address,
        now: u64,
    ) -> Result<()> {
        let valid_until = now - 20;
        let data: IndexerAuth = IndexerAuth::new(address, valid_until);

        let req = post(
            "/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                data,
            },
        );

        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth_invalid_signature(
        #[future(awt)] app: Router,
        address: Address,
        now: u64,
    ) -> Result<()> {
        let valid_until = now + 20 * 60;
        let data: IndexerAuth = IndexerAuth::new(address, valid_until);
        let invalid_data: IndexerAuth = IndexerAuth::new(Address::zero(), valid_until);

        let req = post(
            "/auth",
            AuthRequest {
                signature: sign_typed_data(&invalid_data).await?,
                data,
            },
        );

        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
