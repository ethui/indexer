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
