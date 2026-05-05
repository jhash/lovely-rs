//! Test fixtures shared across the workspace. Path-only, never published.

use lovely_db::StubSqliteAppStore;
use lovely_web::AppState;
use secrecy::SecretString;
use sqlx::PgPool;
use std::sync::Arc;
use tempfile::TempDir;
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

pub struct PgTestContainer {
    _container: ContainerAsync<Postgres>,
    pub admin_url: String,
}

impl PgTestContainer {
    pub async fn start() -> anyhow::Result<Self> {
        let container = Postgres::default().with_tag("17").start().await?;
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432.tcp()).await?;
        let admin_url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
        Ok(Self {
            _container: container,
            admin_url,
        })
    }

    pub async fn fresh_db(&self) -> anyhow::Result<PgPool> {
        let admin = PgPool::connect(&self.admin_url).await?;
        let dbname = format!("test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE \"{dbname}\""))
            .execute(&admin)
            .await?;
        admin.close().await;
        let url = self.admin_url.replace("/postgres", &format!("/{dbname}"));
        let pool = PgPool::connect(&url).await?;
        lovely_db::pg::MIGRATOR.run(&pool).await?;
        Ok(pool)
    }
}

/// Boots `lovely-web` on an ephemeral port against an isolated Postgres
/// database. Returns a `reqwest::Client` with cookie jar enabled and
/// redirect-following disabled (so tests can inspect 3xx responses).
pub struct TestApp {
    pub url: String,
    pub pg: PgPool,
    pub client: reqwest::Client,
    pub data_dir: TempDir,
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

impl TestApp {
    pub async fn start_with_pool(pg: PgPool) -> anyhow::Result<Self> {
        let data_dir = tempfile::tempdir()?;
        let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("static");
        let app_store: Arc<dyn lovely_db::SqliteAppStore> = Arc::new(StubSqliteAppStore);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let url = format!("http://127.0.0.1:{}", addr.port());
        let state = AppState::new(
            pg.clone(),
            app_store,
            url.clone(),
            SecretString::from("test-secret-not-for-prod".to_string()),
            static_dir,
        );
        let app = lovely_web::router(state);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        for _ in 0..50 {
            if let Ok(r) = client.get(format!("{url}/healthz")).send().await {
                if r.status() == 200 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(Self {
            url,
            pg,
            client,
            data_dir,
            _shutdown: tx,
        })
    }

    /// Hits `/auth/login` to provoke the csrf_token cookie, then returns
    /// the value parsed from the Set-Cookie header.
    pub async fn csrf_token(&self) -> anyhow::Result<String> {
        let resp = self
            .client
            .get(format!("{}/auth/login", self.url))
            .send()
            .await?;
        for sc in resp.headers().get_all(reqwest::header::SET_COOKIE).iter() {
            let s = sc.to_str()?;
            if let Some(rest) = s.strip_prefix("csrf_token=") {
                if let Some(end) = rest.find(';') {
                    return Ok(rest[..end].to_string());
                }
                return Ok(rest.to_string());
            }
        }
        anyhow::bail!("csrf_token cookie not set on /auth/login response")
    }
}
