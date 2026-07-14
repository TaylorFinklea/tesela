use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

/// Unified error type for all route handlers.
#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Validation(String),
    Conflict(String),
    RetrySafe {
        message: String,
        move_id: uuid::Uuid,
    },
    Internal(anyhow::Error),
}

pub type AppResult<T> = std::result::Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, Json(json!({ "error": msg }))).into_response()
            }
            AppError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response()
            }
            AppError::Conflict(msg) => {
                (StatusCode::CONFLICT, Json(json!({ "error": msg }))).into_response()
            }
            AppError::RetrySafe { message, move_id } => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": message,
                    "move_id": move_id,
                    "retry_safe": true,
                })),
            )
                .into_response(),
            AppError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response(),
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        AppError::Internal(e.into())
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};

    use super::AppError;

    async fn response_json(error: AppError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn conflict_maps_to_http_409() {
        let (status, body) = response_json(AppError::Conflict("move id reused".into())).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body, serde_json::json!({ "error": "move id reused" }));
    }

    #[tokio::test]
    async fn retry_safe_maps_to_http_503_with_move_id() {
        let move_id = uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        let (status, body) = response_json(AppError::RetrySafe {
            message: "relocation requires recovery".into(),
            move_id,
        })
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body,
            serde_json::json!({
                "error": "relocation requires recovery",
                "move_id": move_id,
                "retry_safe": true,
            })
        );
    }
}
