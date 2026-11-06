use app::AppError;
use auth::AuthError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use contracts::ErrorDto;

/// Error answer: status plus a stable machine-readable code.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    #[must_use]
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "sign in first")
    }

    #[must_use]
    pub fn bad_request(m: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", m)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorDto {
                code: self.code.into(),
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::NotFound(what) => Self::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("{what} not found"),
            ),
            AppError::Forbidden => Self::new(StatusCode::FORBIDDEN, "forbidden", "forbidden"),
            AppError::Conflict(m) => Self::new(StatusCode::CONFLICT, "conflict", m),
            AppError::Domain(d) => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", d.to_string())
            }
            AppError::Repo(r) => {
                tracing::error!(error = %r, "storage error");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal error",
                )
            }
        }
    }
}

impl From<AuthError> for ApiError {
    fn from(e: AuthError) -> Self {
        let msg = e.to_string();
        match e {
            AuthError::InvalidCredentials => {
                Self::new(StatusCode::UNAUTHORIZED, "invalid_credentials", msg)
            }
            AuthError::TotpRequired => Self::new(StatusCode::UNAUTHORIZED, "totp_required", msg),
            AuthError::InvalidTotp => Self::new(StatusCode::UNAUTHORIZED, "invalid_totp", msg),
            AuthError::SignupClosed => Self::new(StatusCode::FORBIDDEN, "signup_closed", msg),
            AuthError::InvalidInvite => Self::new(StatusCode::FORBIDDEN, "invalid_invite", msg),
            AuthError::WeakPassword => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "weak_password", msg)
            }
            AuthError::InvalidUsername => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_username", msg)
            }
            AuthError::InvalidDisplayName => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_display_name",
                msg,
            ),
            AuthError::TooManyAttempts { .. } => {
                Self::new(StatusCode::TOO_MANY_REQUESTS, "too_many_attempts", msg)
            }
            AuthError::App(a) => a.into(),
            AuthError::Store(_) | AuthError::Oidc(_) => {
                tracing::error!(error = %msg, "auth error");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal error",
                )
            }
        }
    }
}

impl From<app::ports::PortError> for ApiError {
    fn from(e: app::ports::PortError) -> Self {
        tracing::error!(error = %e, "port error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error",
        )
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
