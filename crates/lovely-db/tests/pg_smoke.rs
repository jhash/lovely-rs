//! Integration smoke tests that boot Postgres via testcontainers.
//! These tests require Docker. Run with:
//!     cargo test -p lovely-db --test pg_smoke
//! Or skip with `--skip pg_` if Docker is unavailable.

use lovely_test_support::PgTestContainer;

#[tokio::test]
#[ignore = "requires Docker; run with: cargo test -p lovely-db -- --ignored"]
async fn migrations_apply_and_users_table_exists() {
    let pg = PgTestContainer::start()
        .await
        .expect("docker required for this test");
    let pool = pg.fresh_db().await.unwrap();
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, 0);
}

#[tokio::test]
#[ignore = "requires Docker; run with: cargo test -p lovely-db -- --ignored"]
async fn all_milestone_a_tables_exist() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    for table in ["users", "oauth_identities", "sessions", "pages", "elements"] {
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT FROM information_schema.tables \
                            WHERE table_schema = 'public' AND table_name = $1)",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(exists.0, "table {table} should exist");
    }
}
