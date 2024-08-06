use std::str::FromStr as _;

use axum::{
    extract::State,
    middleware::from_extractor,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use ethers_core::types::{Address, Signature};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;

use super::{
    app_state::AppState,
    auth::{Claims, IndexerAuth},
    error::{ApiError, ApiResult},
    registration::RegistrationProof,
};

pub fn app(jwt_secret: String, state: AppState) -> Router {
    let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
    let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());

    let protected_routes = Router::new()
        .route("/test", post(test))
        .route_layer(from_extractor::<Claims>());

    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/auth", post(auth))
        .route("/register", post(register));

    Router::new()
        .nest("/api", protected_routes)
        .nest("/api", public_routes)
        .layer(CorsLayer::permissive())
        .layer(Extension(encoding_key))
        .layer(Extension(decoding_key))
        .with_state(state)
}

async fn health() -> impl IntoResponse {}

pub async fn test() -> impl IntoResponse {
    Json(json!({"foo": "bar"}))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterRequest {
    address: Address,
    proof: RegistrationProof,
}

// POST /api/register
pub async fn register(
    State(state): State<AppState>,
    Json(register): Json<RegisterRequest>,
) -> ApiResult<impl IntoResponse> {
    let addr = reth_primitives::Address::from_str(&format!("0x{:x}", register.address)).unwrap();

    register.proof.validate(addr, &state).await?;

    state.db.register(register.address.into()).await?;

    Ok(Json(json!({"result": "success"})))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    signature: Signature,
    data: IndexerAuth,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResponse {
    access_token: String,
}

// POST /api/auth
pub async fn auth(
    Extension(encoding_key): Extension<EncodingKey>,
    State(AppState { db, .. }): State<AppState>,
    Json(auth): Json<AuthRequest>,
) -> ApiResult<impl IntoResponse> {
    auth.data
        .check(&auth.signature)
        .map_err(|_| ApiError::InvalidCredentials)?;

    if !db.is_registered(auth.data.address.into()).await? {
        return Err(ApiError::NotRegistered);
    }

    let access_token = encode(&Header::default(), &Claims::from(auth.data), &encoding_key)?;

    // Send the authorized token
    Ok(Json(AuthResponse { access_token }))
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use color_eyre::Result;
    use ethers_core::types::Address;
    use rstest::rstest;
    use serde::Serialize;
    use serial_test::serial;
    use tower::{Service, ServiceExt};

    use super::AuthRequest;
    use crate::{
        api::{
            app::{AuthResponse, RegisterRequest},
            app_state::AppState,
            auth::IndexerAuth,
            registration::RegistrationProof,
            test_utils::{address, now, sign_typed_data, to_json_resp},
        },
        config::Config,
        db::Db,
        sync::RethProviderFactory,
    };

    fn get(uri: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .method("GET")
            .header("content-type", "application/json")
            .body(Body::empty())
            .unwrap()
    }

    fn post<B: Serialize>(uri: &str, body: B) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    fn post_with_jwt<B: Serialize>(uri: &str, jwt: String, body: B) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .header("Authorization", format!("Bearer {}", jwt))
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    async fn build_app() -> Router {
        let jwt_secret = "secret".to_owned();
        let db = Db::connect_test().await.unwrap();
        let config = Config::for_test();
        let chain = db.setup_chain(&config.chain).await.unwrap();
        let provider_factory = Arc::new(RethProviderFactory::new(&config, &chain).unwrap());

        let state = AppState {
            db,
            config,
            provider_factory,
        };

        super::app(jwt_secret, state)
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_register(address: Address) -> Result<()> {
        let app = build_app().await;
        let req = post(
            "/api/register",
            RegisterRequest {
                address,
                proof: RegistrationProof::Test,
            },
        );
        let resp = app.clone().oneshot(req).await?;

        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth(address: Address, now: u64) -> Result<()> {
        let app = build_app().await;
        let valid_until = now + 20 * 60;
        let data = IndexerAuth::new(address, valid_until);

        let registration = post(
            "/api/register",
            RegisterRequest {
                address,
                proof: RegistrationProof::Test,
            },
        );
        app.clone().oneshot(registration).await?;

        let auth = post(
            "/api/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                data,
            },
        );

        let resp = app.oneshot(auth).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth_twice(address: Address, now: u64) -> Result<()> {
        let mut app = build_app().await;
        let valid_until = now + 20 * 60;
        let data = IndexerAuth::new(address, valid_until);

        let registration = post(
            "/api/register",
            RegisterRequest {
                address,
                proof: RegistrationProof::Test,
            },
        );
        app.clone().oneshot(registration).await?;

        let req = post(
            "/api/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                data: data.clone(),
            },
        );
        let req2 = post(
            "/api/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                data,
            },
        );

        let resp = app.call(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app.oneshot(req2).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth_expired_signature(address: Address, now: u64) -> Result<()> {
        let app = build_app().await;
        let valid_until = now - 20;
        let data = IndexerAuth::new(address, valid_until);

        let req = post(
            "/api/auth",
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
    async fn test_auth_invalid_signature(address: Address, now: u64) -> Result<()> {
        let app = build_app().await;
        let valid_until = now + 20 * 60;
        let data = IndexerAuth::new(address, valid_until);
        let invalid_data = IndexerAuth::new(Address::zero(), valid_until);

        let req = post(
            "/api/auth",
            AuthRequest {
                signature: sign_typed_data(&invalid_data).await?,
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
    async fn test_protected_endpoint_without_auth() -> Result<()> {
        let app = build_app().await;
        let req = post("/api/test", ());
        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_protected_endpoint_with_auth(address: Address, now: u64) -> Result<()> {
        let app = build_app().await;
        let valid_until = now + 20;
        let data = IndexerAuth::new(address, valid_until);

        let registration = post(
            "/api/register",
            RegisterRequest {
                address,
                proof: RegistrationProof::Test,
            },
        );
        app.clone().oneshot(registration).await?;

        let req = post(
            "/api/auth",
            AuthRequest {
                signature: sign_typed_data(&data).await?,
                data,
            },
        );

        let resp = app.clone().oneshot(req).await?;
        let jwt: AuthResponse = to_json_resp(resp).await?;
        //
        let req = post_with_jwt("/api/test", jwt.access_token, ());
        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_unprotected_endpoint() -> Result<()> {
        let app = build_app().await;
        let req = get("/api/health");
        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }
}
