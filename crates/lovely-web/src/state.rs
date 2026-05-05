use lovely_db::SqliteAppStore;
use secrecy::SecretString;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pg: PgPool,
    pub app_store: Arc<dyn SqliteAppStore>,
    pub base_url: String,
    pub session_secret: SecretString,
    pub static_dir: PathBuf,
}

impl AppState {
    pub fn new(
        pg: PgPool,
        app_store: Arc<dyn SqliteAppStore>,
        base_url: String,
        session_secret: SecretString,
        static_dir: PathBuf,
    ) -> Self {
        Self {
            pg,
            app_store,
            base_url,
            session_secret,
            static_dir,
        }
    }
}
