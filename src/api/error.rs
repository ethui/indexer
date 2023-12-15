use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Generic(#[from] color_eyre::Report),
}

pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status_code, message) = match self {
            Self::Generic(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        };

        (status_code, message).into_response()
    }
}
