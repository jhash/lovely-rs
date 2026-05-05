use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_htmx::HxRedirect;

#[derive(thiserror::Error, Debug)]
pub enum WebError {
    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("invalid input: {0}")]
    BadRequest(String),

    #[error("CSRF token missing or invalid")]
    Csrf,

    #[error(transparent)]
    Db(#[from] lovely_db::DbError),

    #[error(transparent)]
    Tree(#[from] lovely_tree::TreeError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::NotFound => (StatusCode::NOT_FOUND, "Not Found").into_response(),
            WebError::Unauthorized => htmx_aware_redirect("/auth/login"),
            WebError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden").into_response(),
            WebError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            WebError::Csrf => (StatusCode::FORBIDDEN, "CSRF").into_response(),
            WebError::Db(e) => {
                tracing::error!(error = %e, "db error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            }
            WebError::Tree(e) => {
                tracing::error!(error = %e, "tree error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            }
            WebError::Other(e) => {
                tracing::error!(error = %e, "other error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            }
        }
    }
}

fn htmx_aware_redirect(target: &'static str) -> Response {
    let url: axum::http::Uri = target.parse().expect("valid uri");
    (
        StatusCode::SEE_OTHER,
        HxRedirect(url),
        [(axum::http::header::LOCATION, target)],
    )
        .into_response()
}

pub type WebResult<T> = Result<T, WebError>;
