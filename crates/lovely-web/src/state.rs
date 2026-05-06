use lovely_db::{SchemaService, SqliteAppStore};
use secrecy::SecretString;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pg: PgPool,
    pub app_store: Arc<dyn SqliteAppStore>,
    pub schema: Arc<SchemaService>,
    pub base_url: String,
    pub session_secret: SecretString,
    pub static_dir: PathBuf,
}

impl AppState {
    pub fn new(
        pg: PgPool,
        app_store: Arc<dyn SqliteAppStore>,
        schema: Arc<SchemaService>,
        base_url: String,
        session_secret: SecretString,
        static_dir: PathBuf,
    ) -> Self {
        Self {
            pg,
            app_store,
            schema,
            base_url,
            session_secret,
            static_dir,
        }
    }
}
