use crate::api::auth::jwt::AuthError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Generic(#[from] color_eyre::Report),
    #[error(transparent)]
    Auth(#[from] AuthError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::Generic(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
            Self::Auth(e) => e.into_response(),
        }
    }
}
