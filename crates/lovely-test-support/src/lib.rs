//! Test fixtures shared across the workspace. Path-only, never published.

use sqlx::PgPool;
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
