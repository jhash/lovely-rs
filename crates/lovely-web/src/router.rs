use crate::handlers;
use crate::state::AppState;
use axum::routing::get;
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn router(state: AppState) -> Router {
    let static_svc = ServeDir::new(state.static_dir.clone());
    Router::new()
        .route("/", get(handlers::home::home))
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz))
        .nest_service("/static", static_svc)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
