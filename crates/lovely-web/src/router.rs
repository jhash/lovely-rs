use crate::handlers;
use crate::state::AppState;
use axum::routing::{get, post};
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn router(state: AppState) -> Router {
    let static_svc = ServeDir::new(state.static_dir.clone());
    Router::new()
        .route("/", get(handlers::home::home))
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz))
        .route("/auth/login", get(handlers::auth_username::get_login))
        .route("/auth/login", post(handlers::auth_username::post_login))
        .route("/auth/register", get(handlers::auth_username::get_register))
        .route(
            "/auth/register",
            post(handlers::auth_username::post_register),
        )
        .route("/auth/logout", post(handlers::auth_username::post_logout))
        .route("/pages", get(handlers::pages::get_pages_index))
        .route("/pages/new", get(handlers::pages::get_pages_new))
        .route("/pages", post(handlers::pages::post_pages_create))
        .route("/pages/{slug}", get(handlers::pages::get_page_by_slug))
        .route(
            "/pages/{slug}/delete",
            post(handlers::pages::delete_page_handler),
        )
        .nest_service("/static", static_svc)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
