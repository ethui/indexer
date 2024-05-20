// use crate::api::auth::jwt::AuthError;
// use axum::http::StatusCode;
// use axum::response::{IntoResponse, Response};
//
// #[derive(thiserror::Error, Debug)]
// pub enum Error {
//     #[error(transparent)]
//     Generic(#[from] color_eyre::Report),
//     #[error(transparent)]
//     Auth(#[from] AuthError),
// }
//
// pub type Result<T> = std::result::Result<T, Error>;
//
// impl IntoResponse for Error {
//     fn into_response(self) -> Response {
//         match self {
//             Self::Generic(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
//             Self::Auth(e) => e.into_response(),
//         }
//     }
// }

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub struct ApiError(color_eyre::Report);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<color_eyre::Report>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;
