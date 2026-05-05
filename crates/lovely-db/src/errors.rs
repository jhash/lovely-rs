use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("app {0} not found")]
    AppNotFound(Uuid),

    #[error("user not found")]
    UserNotFound,

    #[error("invalid identifier: {0:?}")]
    InvalidIdentifier(String),

    #[error("schema conflict: {0}")]
    SchemaConflict(String),

    #[error("uniqueness violated: {0}")]
    Conflict(String),

    #[error(transparent)]
    Tree(#[from] lovely_tree::TreeError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("snapshot decode: {0}")]
    SnapshotDecode(String),
}
