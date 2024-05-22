use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use ethers_core::types::{Address, Signature};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use super::{
    auth::{Claims, IndexAuth},
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
    _: Claims,
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
    address: Address,
    valid_until: u64,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    access_token: String,
}

pub async fn auth(
    Extension(encoding_key): Extension<EncodingKey>,
    Json(auth): Json<AuthRequest>,
) -> ApiResult<impl IntoResponse> {
    let data = IndexAuth::new(auth.address, auth.valid_until);
    data.check(&auth.signature)
        .map_err(|_| ApiError::InvalidCredentials)?;

    let claims = Claims::new(auth.address, auth.valid_until as usize);
    let access_token = encode(&Header::default(), &claims, &encoding_key)?;

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
    use tower::ServiceExt;

    use super::{auth, AuthRequest};
    use crate::api::{
        auth::IndexAuth,
        test_utils::{address, now, sign_typed_data},
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
    fn app() -> Router {
        let jwt_secret = "secret".to_owned();
        let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());

        Router::new()
            .route("/auth", routing::post(auth))
            .layer(Extension(encoding_key))
            .layer(Extension(decoding_key))
    }

    #[rstest]
    #[tokio::test]
    async fn test_auth(app: Router, address: Address, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;
        let data: IndexAuth = IndexAuth::new(address, valid_until);

        let req = post(
            "/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                address,
                valid_until,
            },
        );

        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_auth_expired_signature(app: Router, address: Address, now: u64) -> Result<()> {
        let valid_until = now - 20;
        let data: IndexAuth = IndexAuth::new(address, valid_until);

        let req = post(
            "/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                address,
                valid_until,
            },
        );

        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_auth_invalid_signature(address: Address, app: Router, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;
        let data: IndexAuth = IndexAuth::new(Address::zero(), valid_until);

        let req = post(
            "/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                address,
                valid_until,
            },
        );

        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}