use super::auth::jwt::{AuthError, Claims, KEYS};
use super::auth::signature::check_type_data;
use super::error::Result;
use crate::db::Db;
use axum::extract::State;
use axum::routing::{get, post};
use axum::Router;
use axum::{response::IntoResponse, Json};
use ethers_core::types::Address;
use jsonwebtoken::{encode, Header};
use serde::{Deserialize, Serialize};

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
) -> Result<impl IntoResponse> {
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

pub async fn auth(Json(auth): Json<AuthRequest>) -> Result<impl IntoResponse> {
    check_type_data(
        &auth.signature,
        auth.address,
        auth.current_timestamp,
        auth.expiration_timestamp,
    )?;

    let claims = Claims {
        sub: auth.address.to_string(),
        company: "iron-wallet".to_owned(),
        exp: auth.expiration_timestamp as usize,
    };
    // Create the authorization token
    let access_token = encode(&Header::default(), &claims, &KEYS.encoding)
        .map_err(|_| AuthError::TokenCreation)?;

    // Send the authorized token
    Ok(Json(AuthResponse { access_token }))
}

#[cfg(test)]
mod test {

    use super::auth;
    use super::AuthRequest;
    use crate::api::auth::signature::test_utils;
    use crate::api::auth::signature::SignatureData;
    use axum::http::StatusCode;
    use axum::{response::IntoResponse, Json};
    use color_eyre::Result;
    use ethers_core::types::Address;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_auth_valid_signature() -> Result<()> {
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
