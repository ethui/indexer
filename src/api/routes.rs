use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use ethers_core::types::Address;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use super::{
    auth::{jwt::Claims, signature::check_type_data},
    error::ApiResult,
};
use crate::db::Db;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/health", get(health))
        .route("/auth", post(auth))
        .route("/register", post(register))
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

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    signature: String,
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
    check_type_data(&auth.signature, auth.address, auth.valid_until)?;

    let claims = Claims::new(auth.address, auth.valid_until as usize);
    let access_token = encode(&Header::default(), &claims, &encoding_key)?;

    // Send the authorized token
    Ok(Json(AuthResponse { access_token }))
}

#[cfg(test)]
mod test {
    use axum::{http::StatusCode, response::IntoResponse, Json};
    use color_eyre::Result;
    use ethers_core::types::Address;
    use rstest::rstest;

    use super::{auth, AuthRequest};
    use crate::api::{
        auth::signature::SignatureData,
        error::ApiResult,
        test_utils::{address, encoding_key, now, sign_typed_data},
    };

    #[rstest]
    #[tokio::test]
    async fn test_auth_valid_signature(address: Address, now: u64) -> ApiResult<()> {
        let valid_until = now + 20 * 60;

        let data: SignatureData = SignatureData::new(address, valid_until);
        let signature = sign_typed_data(data).await?.to_string();

        let auth_request = AuthRequest {
            signature,
            address,
            valid_until,
        };

        let resp = auth(encoding_key(), Json(auth_request))
            .await?
            .into_response();

        assert_eq!(resp.status(), StatusCode::OK);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_auth_valid_signature_invalid_timestamp(address: Address, now: u64) -> Result<()> {
        let twenty_minutes = 20 * 60;
        let valid_until = now + twenty_minutes;

        let data: SignatureData = SignatureData::new(address, valid_until);

        let signature = sign_typed_data(data).await?.to_string();
        let auth_request = AuthRequest {
            signature,
            address,
            valid_until,
        };

        let auth_response = auth(encoding_key(), Json(auth_request)).await;
        assert!(auth_response.is_err());

        let _ = auth_response.map_err(|e| {
            let response = e.into_response();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        });

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_auth_invalid_signature(address: Address, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;

        let data: SignatureData = SignatureData::new(address, valid_until);

        let signature = sign_typed_data(data).await?.to_string();

        let auth_request = AuthRequest {
            signature,
            address,
            valid_until,
        };

        let resp = auth(encoding_key(), Json(auth_request)).await;
        assert!(resp.is_err());

        let _ = resp.map_err(|e| {
            let response = e.into_response();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        });

        Ok(())
    }
}
