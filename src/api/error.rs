use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Invalid Credentials")]
    InvalidCredentials,

    #[error("Not Registered")]
    NotRegistered,

    #[error(transparent)]
    Jsonwebtoken(#[from] jsonwebtoken::errors::Error),

    #[error(transparent)]
    Unknown(#[from] color_eyre::Report),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = match self {
            ApiError::NotRegistered | ApiError::InvalidCredentials | ApiError::Jsonwebtoken(_) => {
                StatusCode::UNAUTHORIZED
            }
            ApiError::Unknown(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}
