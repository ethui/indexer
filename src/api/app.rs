use axum::{
    extract::State,
    middleware::from_extractor,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use ethers_core::types::Signature;
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;

use super::{
    auth::{Claims, IndexerAuth},
    error::{ApiError, ApiResult},
};
use crate::db::Db;

pub fn app(db: Db, jwt_secret: String) -> Router {
    let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
    let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());

    let protected_routes = Router::new()
        .route("/test", post(test))
        .route_layer(from_extractor::<Claims>());

    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/auth", post(auth));

    Router::new()
        .nest("/api", protected_routes)
        .nest("/api", public_routes)
        .layer(CorsLayer::permissive())
        .layer(Extension(encoding_key))
        .layer(Extension(decoding_key))
        .with_state(db)
}

async fn health() -> impl IntoResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    signature: Signature,
    data: IndexerAuth,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResponse {
    access_token: String,
}

pub async fn test() -> impl IntoResponse {
    Json(json!({"foo": "bar"}))
}

pub async fn auth(
    Extension(encoding_key): Extension<EncodingKey>,
    State(db): State<Db>,
    Json(auth): Json<AuthRequest>,
) -> ApiResult<impl IntoResponse> {
    auth.data
        .check(&auth.signature)
        .map_err(|_| ApiError::InvalidCredentials)?;

    // TODO this registration needs to be verified (is the user whitelisted? did the user pay?)
    db.register(auth.data.address.into()).await?;
    let access_token = encode(&Header::default(), &Claims::from(auth.data), &encoding_key)?;

    // Send the authorized token
    Ok(Json(AuthResponse { access_token }))
}

#[cfg(test)]
mod test {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use color_eyre::Result;
    use ethers_core::types::Address;
    use rstest::{fixture, rstest};
    use serde::Serialize;
    use serial_test::serial;
    use tower::ServiceExt;

    use super::AuthRequest;
    use crate::{
        api::{
            app::AuthResponse,
            auth::IndexerAuth,
            test_utils::{address, now, sign_typed_data, to_json_resp},
        },
        db::Db,
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

    #[fixture]
    async fn app() -> Router {
        let jwt_secret = "secret".to_owned();
        let db = Db::connect_test().await.unwrap();

        super::app(db, jwt_secret)
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_auth(#[future(awt)] app: Router, address: Address, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;
        let data = IndexerAuth::new(address, valid_until);

        let req = post(
            "/api/auth",
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
    async fn test_auth_twice(#[future(awt)] app: Router, address: Address, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;
        let data = IndexerAuth::new(address, valid_until);

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

        let resp = app.clone().oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = app.oneshot(req2).await?;
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
    async fn test_auth_invalid_signature(
        #[future(awt)] app: Router,
        address: Address,
        now: u64,
    ) -> Result<()> {
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
    async fn test_protected_endpoint_without_auth(#[future(awt)] app: Router) -> Result<()> {
        let req = post("/api/test", ());
        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_protected_endpoint_with_auth(
        #[future(awt)] app: Router,
        address: Address,
        now: u64,
    ) -> Result<()> {
        let valid_until = now + 20;
        let data = IndexerAuth::new(address, valid_until);

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
    async fn test_unprotected_endpoint(#[future(awt)] app: Router) -> Result<()> {
        let req = get("/api/health");
        let resp = app.oneshot(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(())
    }
}
