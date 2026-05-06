use anyhow::Context;
use clap::Parser;
use lovely_db::{LocalSqliteAppStore, PgConfig, SchemaService};
use lovely_web::AppState;
use secrecy::SecretString;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug, Clone)]
#[command(name = "lovely-server", version)]
struct Args {
    #[arg(long, env = "LOVELY_BIND", default_value = "0.0.0.0:8080")]
    bind: String,

    #[arg(long, env = "LOVELY_DATABASE_URL")]
    database_url: String,

    #[arg(long, env = "LOVELY_SQLITE_DATA_DIR", default_value = "./data/apps")]
    sqlite_data_dir: PathBuf,

    #[arg(long, env = "LOVELY_BASE_URL", default_value = "http://localhost:8080")]
    base_url: String,

    #[arg(env = "LOVELY_SESSION_SECRET")]
    session_secret: SecretString,

    #[arg(long, env = "LOVELY_GITHUB_CLIENT_ID")]
    github_client_id: Option<String>,

    #[arg(env = "LOVELY_GITHUB_CLIENT_SECRET")]
    github_client_secret: Option<SecretString>,

    #[arg(long, env = "LOVELY_GOOGLE_CLIENT_ID")]
    google_client_id: Option<String>,

    #[arg(env = "LOVELY_GOOGLE_CLIENT_SECRET")]
    google_client_secret: Option<SecretString>,

    #[arg(long, env = "LOVELY_APPLE_TEAM_ID")]
    apple_team_id: Option<String>,

    #[arg(long, env = "LOVELY_APPLE_KEY_ID")]
    apple_key_id: Option<String>,

    #[arg(long, env = "LOVELY_APPLE_SERVICES_ID")]
    apple_services_id: Option<String>,

    #[arg(long, env = "LOVELY_APPLE_PRIVATE_KEY_PATH")]
    apple_private_key_path: Option<PathBuf>,

    #[arg(long, env = "LOVELY_LOG_FORMAT", default_value = "auto")]
    log_format: String,

    #[arg(long, env = "LOVELY_LOG_LEVEL", default_value = "info")]
    log_level: String,

    #[arg(long, env = "LOVELY_STATIC_DIR", default_value = "./static")]
    static_dir: PathBuf,
}

fn setup_tracing(log_format: &str, log_level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));
    let json = matches!(log_format, "json")
        || (log_format == "auto" && !std::io::IsTerminal::is_terminal(&std::io::stdout()));
    if json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("LOVELY_DOTENV").as_deref() == Ok("1") {
        let _ = dotenvy::dotenv();
    }
    let args = Args::parse();
    setup_tracing(&args.log_format, &args.log_level)?;

    std::fs::create_dir_all(&args.sqlite_data_dir)
        .with_context(|| format!("create sqlite data dir {:?}", args.sqlite_data_dir))?;

    let pg_config = PgConfig::from_url(&args.database_url);
    let pg = lovely_db::pg::connect(&pg_config)
        .await
        .context("connect to postgres")?;
    lovely_db::pg::run_migrations(&pg)
        .await
        .context("run postgres migrations")?;

    let schema = Arc::new(SchemaService::new(pg.clone()));
    let app_store: Arc<dyn lovely_db::SqliteAppStore> = Arc::new(
        LocalSqliteAppStore::new(&args.sqlite_data_dir, schema.clone())
            .context("init sqlite app store")?,
    );
    let state = AppState::new(
        pg,
        app_store,
        schema,
        args.base_url.clone(),
        args.session_secret.clone(),
        args.static_dir.clone(),
    );
    let app = lovely_web::router(state);

    // Reuse a socket inherited from `systemfd` if present — keeps the
    // port bound across cargo-watch rebuilds so the browser doesn't
    // hit "connection refused" mid-restart. Falls back to a fresh
    // bind in production or when systemfd isn't in the picture.
    let listener = match listenfd::ListenFd::from_env().take_tcp_listener(0)? {
        Some(std_listener) => {
            std_listener.set_nonblocking(true)?;
            tracing::info!("lovely-server reusing inherited socket from systemfd");
            tokio::net::TcpListener::from_std(std_listener)?
        }
        None => {
            let l = tokio::net::TcpListener::bind(&args.bind)
                .await
                .context("bind tcp listener")?;
            tracing::info!(addr = %args.bind, "lovely-server listening");
            l
        }
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM");
    let mut int = signal(SignalKind::interrupt()).expect("install SIGINT");
    tokio::select! {
        _ = term.recv() => {},
        _ = int.recv() => {},
    }
    tracing::info!("shutdown signal received");
}
