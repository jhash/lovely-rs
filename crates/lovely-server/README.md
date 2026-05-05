# lovely-server

The main binary. Parses configuration via `clap` (with env-var fallbacks), connects to Postgres, runs migrations at startup, constructs `lovely_web::AppState`, and serves the router on the configured bind address.

Graceful shutdown handles SIGTERM/SIGINT and lets axum drain in-flight requests.

## Run

```sh
LOVELY_DATABASE_URL=postgres://… LOVELY_SESSION_SECRET=$(openssl rand -hex 32) \
    cargo run -p lovely-server
```

`lovely-server --help` lists every flag and corresponding env var.
