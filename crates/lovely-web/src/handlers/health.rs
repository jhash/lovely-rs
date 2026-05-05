use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;

pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn readyz(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    let r: Result<(i32,), _> = sqlx::query_as("SELECT 1").fetch_one(&state.pg).await;
    match r {
        Ok(_) => Ok("ok"),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
