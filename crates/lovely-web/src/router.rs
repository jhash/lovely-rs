use crate::handlers;
use crate::state::AppState;
use axum::routing::{get, patch, post};
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn router(state: AppState) -> Router {
    let static_svc = ServeDir::new(state.static_dir.clone());
    Router::new()
        .route("/", get(handlers::home::home))
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz))
        // --- auth ---
        .route("/auth/login", get(handlers::auth_username::get_login))
        .route("/auth/login", post(handlers::auth_username::post_login))
        .route("/auth/register", get(handlers::auth_username::get_register))
        .route(
            "/auth/register",
            post(handlers::auth_username::post_register),
        )
        .route("/auth/logout", post(handlers::auth_username::post_logout))
        // --- apps + editor ---
        .route("/apps", get(handlers::apps::get_apps_index))
        .route("/apps/{app_slug}", get(handlers::apps::get_app_dashboard))
        .route(
            "/apps/{app_slug}/pages/new",
            get(handlers::pages::get_pages_new),
        )
        .route(
            "/apps/{app_slug}/pages",
            post(handlers::pages::post_pages_create),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/edit",
            get(handlers::pages::get_page_edit),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/preview",
            get(handlers::pages::get_page_preview),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/tree",
            get(handlers::builder::get_tree_fragment),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/inspector",
            get(handlers::builder::get_inspector_fragment),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}",
            patch(handlers::builder::patch_element),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}/move",
            post(handlers::builder::post_move_element),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}",
            post(handlers::pages::post_page_update),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/delete",
            post(handlers::pages::delete_page_handler),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements",
            post(handlers::elements::post_add_element),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}",
            post(handlers::elements::post_update_element),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}/delete",
            post(handlers::elements::post_delete_element),
        )
        // --- legacy /pages redirects ---
        .route("/pages", get(handlers::apps::redirect_pages_index))
        .route("/pages/new", get(handlers::apps::redirect_pages_new))
        // --- public render (must be last so /apps, /auth, /static win) ---
        .route("/{username}", get(handlers::pages::get_public_user_root))
        .route(
            "/{username}/{slug}",
            get(handlers::pages::get_public_user_page),
        )
        .nest_service("/static", static_svc)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
