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
    current_timestamp: u64,
    expiration_timestamp: u64,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    access_token: String,
}

pub async fn auth(
    Extension(encoding_key): Extension<EncodingKey>,
    Json(auth): Json<AuthRequest>,
) -> ApiResult<impl IntoResponse> {
    check_type_data(
        &auth.signature,
        auth.address,
        auth.current_timestamp,
        auth.expiration_timestamp,
    )?;

    let claims = Claims::new(auth.address, auth.expiration_timestamp as usize);
    // Create the authorization token
    let access_token = encode(&Header::default(), &claims, &encoding_key)?;

    // Send the authorized token
    Ok(Json(AuthResponse { access_token }))
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use axum::{http::StatusCode, response::IntoResponse, Json};
    use color_eyre::Result;
    use ethers_core::types::Address;

    use super::{auth, AuthRequest};
    use crate::api::{
        auth::signature::{test_utils, SignatureData},
        error::ApiResult,
    };

    #[tokio::test]
    async fn test_auth_valid_signature() -> ApiResult<()> {
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let expiration_timestamp = current_timestamp + 20 * 60;
        let address: Address =
            Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();

        let data: SignatureData =
            SignatureData::new(address, current_timestamp, expiration_timestamp);

        let signature = test_utils::sign_type_data(data).await?.to_string();

        let auth_request = AuthRequest {
            signature,
            address,
            current_timestamp,
            expiration_timestamp,
        };

        let auth_response = auth(Json(auth_request)).await?.into_response();

        assert_eq!(auth_response.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn test_auth_valid_signature_invalid_timestamp() -> Result<()> {
        let twenty_minutes = 20 * 60;
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            - twenty_minutes;
        let expiration_timestamp = current_timestamp + twenty_minutes;
        let address: Address =
            Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();

        let data: SignatureData =
            SignatureData::new(address, current_timestamp, expiration_timestamp);

        let signature = test_utils::sign_type_data(data).await?.to_string();
        let auth_request = AuthRequest {
            signature,
            address,
            current_timestamp,
            expiration_timestamp,
        };

        let auth_response = auth(Json(auth_request)).await;
        assert!(auth_response.is_err());

        let _ = auth_response.map_err(|e| {
            let response = e.into_response();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        });

        Ok(())
    }

    #[tokio::test]
    async fn test_auth_invalid_signature() -> Result<()> {
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let expiration_timestamp = current_timestamp + 20 * 60;
        let address: Address =
            Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92269").unwrap();

        let data: SignatureData =
            SignatureData::new(address, current_timestamp, expiration_timestamp);

        let signature = test_utils::sign_type_data(data).await?.to_string();

        let auth_request = AuthRequest {
            signature,
            address,
            current_timestamp,
            expiration_timestamp,
        };

        let auth_response = auth(Json(auth_request)).await;
        assert!(auth_response.is_err());

        let _ = auth_response.map_err(|e| {
            let response = e.into_response();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        });

        Ok(())
    }
}
