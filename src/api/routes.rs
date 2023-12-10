use super::error::Result;
use crate::db::Db;
use axum::extract::State;
use axum::{response::IntoResponse, Json};
use serde::Deserialize;

pub async fn health() -> impl IntoResponse {}

#[derive(Debug, Deserialize)]
pub struct Register {
    pub address: alloy_primitives::Address,
}

pub async fn register(
    State(db): State<Db>,
    Json(register): Json<Register>,
) -> Result<impl IntoResponse> {
    db.register(register.address.into()).await?;

    Ok(())
}
