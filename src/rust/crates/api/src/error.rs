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
        let status = match &e {
            AuthError::InvalidCredentials | AuthError::TotpRequired | AuthError::InvalidTotp => {
                StatusCode::UNAUTHORIZED
            }
            AuthError::SignupClosed | AuthError::InvalidInvite => StatusCode::FORBIDDEN,
            AuthError::UsernameTaken => StatusCode::CONFLICT,
            AuthError::PasswordTooShort
            | AuthError::PasswordTooLong
            | AuthError::InvalidUsername(_)
            | AuthError::DisplayNameTooLong => StatusCode::UNPROCESSABLE_ENTITY,
            AuthError::TooManyAttempts { .. } => StatusCode::TOO_MANY_REQUESTS,
            AuthError::App(_) | AuthError::Store(_) | AuthError::Oidc(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        // every sign-up / sign-in rule has its own code (contracts::auth::AuthErrorCode)
        if let Some(code) = e.code() {
            return Self::new(status, code.code(), msg);
        }
        if let AuthError::App(a) = e {
            return a.into();
        }
        tracing::error!(error = %msg, "auth error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error",
        )
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
