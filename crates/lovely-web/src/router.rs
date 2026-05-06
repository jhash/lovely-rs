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
        .route(
            "/auth/check-username",
            get(handlers::auth_username::get_check_username),
        )
        .route("/auth/logout", post(handlers::auth_username::post_logout))
        // --- apps + editor ---
        .route("/apps", get(handlers::apps::get_apps_index))
        .route("/apps", post(handlers::apps::post_apps_create))
        .route("/apps/new", get(handlers::apps::get_apps_new))
        .route(
            "/apps/check-slug",
            get(handlers::apps::get_check_app_slug),
        )
        .route("/apps/{app_slug}", get(handlers::apps::get_app_dashboard))
        .route("/apps/{app_slug}/settings", get(handlers::apps::get_app_settings))
        .route("/apps/{app_slug}/rename", post(handlers::apps::post_app_rename))
        .route("/apps/{app_slug}/delete", post(handlers::apps::post_app_delete))
        .route("/apps/{app_slug}/theme", post(handlers::apps::post_app_theme))
        .route("/apps/{app_slug}/publish", post(handlers::apps::post_app_publish))
        .route("/profile/publish", post(handlers::apps::post_profile_publish))
        // --- data (collections + records) ---
        .route(
            "/apps/{app_slug}/data",
            get(handlers::data::get_data_index)
                .post(handlers::data::post_collection_create),
        )
        .route(
            "/apps/{app_slug}/data/new",
            get(handlers::data::get_collection_new),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}",
            get(handlers::data::get_collection),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/edit",
            get(handlers::data::get_collection_edit),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/rename",
            post(handlers::data::post_collection_rename),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/fields",
            post(handlers::data::post_field_add),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/fields/{field_name}/rename",
            post(handlers::data::post_field_rename),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/fields/{field_name}/delete",
            post(handlers::data::post_field_delete),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/delete",
            post(handlers::data::post_collection_delete),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/records",
            post(handlers::data::post_record_create),
        )
        .route(
            "/apps/{app_slug}/data/{coll_name}/records/delete",
            post(handlers::data::post_record_delete),
        )
        .route(
            "/p/{username}/{slug}/_submit/{coll_name}",
            post(handlers::data::post_public_submit),
        )
        .route(
            "/apps/{app_slug}/pages/new",
            get(handlers::pages::get_pages_new),
        )
        .route(
            "/apps/{app_slug}/pages/check-slug",
            get(handlers::pages::get_check_page_slug),
        )
        .route(
            "/apps/{app_slug}/pages",
            get(handlers::apps::get_app_pages).post(handlers::pages::post_pages_create),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/edit",
            get(handlers::pages::get_page_edit),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/canvas",
            get(handlers::builder::get_canvas_fragment),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/head",
            post(handlers::pages::post_page_head),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/access",
            post(handlers::pages::post_page_access),
        )
        .route(
            "/p/{username}/{slug}/_unlock",
            post(handlers::pages::post_page_unlock),
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
            "/apps/{app_slug}/pages/{page_slug}/undo",
            post(handlers::builder::post_undo),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/redo",
            post(handlers::builder::post_redo),
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
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}/add-before",
            post(handlers::elements::post_add_before),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}/add-after",
            post(handlers::elements::post_add_after),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}/duplicate",
            post(handlers::elements::post_duplicate_element),
        )
        .route(
            "/apps/{app_slug}/pages/{page_slug}/elements/{element_id}/wrap",
            post(handlers::elements::post_wrap_element),
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
        .route(
            "/{username}/{app_slug}/{page_slug}",
            get(handlers::pages::get_public_user_app_page),
        )
        .nest_service("/static", static_svc)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
