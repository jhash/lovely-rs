# lovely-rs deploy

This directory contains everything needed to run `lovely-server` in three
shapes: locally via `docker compose`, on a Docker Swarm node, and on a
single-namespace Kubernetes cluster.

```
deploy/
  Dockerfile          multi-stage build, distroless runtime
  .dockerignore
  compose.yaml        local dev + Swarm
  k8s/
    deployment.yaml   replicas:1, strategy: Recreate, mounts /data
    service.yaml      ClusterIP :8080
    ingress.yaml      cert-manager + letsencrypt-prod
    pvc.yaml          lovely-data, RWO 20Gi
    postgres.yaml     StatefulSet + Service (replaceable with managed PG)
    secret.yaml       placeholders only
```

The k8s manifests are deliberately namespace-agnostic — apply them into
whatever namespace you prefer with `kubectl apply -n <ns> -f deploy/k8s/`.

---

## Environment variables (the matrix)

All flags read from CLI arg or env (clap `env =`). Secrets are marked.

| Env var                          | Required | Default                     | Notes                                                               |
| -------------------------------- | -------- | --------------------------- | ------------------------------------------------------------------- |
| `LOVELY_BIND`                    | no       | `0.0.0.0:8080`              | Listen address.                                                     |
| `LOVELY_DATABASE_URL`            | yes      | -                           | Postgres connection string. Use the secret indirection in k8s.      |
| `LOVELY_SQLITE_DATA_DIR`         | no       | `./data/apps`               | One SQLite file per app. Backed by the `/data` PVC in k8s.          |
| `LOVELY_BASE_URL`                | no       | `http://localhost:8080`     | Used to build OAuth redirect URIs and absolute links.               |
| `LOVELY_SESSION_SECRET`          | yes      | -                           | **Secret.** 32+ random bytes. Rotating invalidates all sessions.    |
| `LOVELY_GITHUB_CLIENT_ID`        | no       | -                           | Public OAuth client id.                                             |
| `LOVELY_GITHUB_CLIENT_SECRET`    | no       | -                           | **Secret.** GitHub OAuth client secret.                             |
| `LOVELY_GOOGLE_CLIENT_ID`        | no       | -                           | Public OAuth client id.                                             |
| `LOVELY_GOOGLE_CLIENT_SECRET`    | no       | -                           | **Secret.** Google OAuth client secret.                             |
| `LOVELY_APPLE_CLIENT_ID`         | no       | -                           | Apple Services ID (e.g. `us.workhands.lovely.signin`).              |
| `LOVELY_APPLE_TEAM_ID`           | no       | -                           | Apple Developer team id.                                            |
| `LOVELY_APPLE_KEY_ID`            | no       | -                           | Key id matching the `.p8` file (e.g. `ABC123XYZ4`).                 |
| `LOVELY_APPLE_PRIVATE_KEY`       | no       | -                           | **Secret.** PEM contents of the `AuthKey_*.p8` file.                |
| `LOVELY_LOG_FORMAT`              | no       | `auto`                      | `auto`, `json`, or `pretty`. Use `json` in k8s.                     |
| `LOVELY_LOG_LEVEL`               | no       | `info`                      | `tracing-subscriber` env-filter directive.                          |
| `LOVELY_STATIC_DIR`              | no       | `./static`                  | The image sets this to `/opt/lovely/static`.                        |
| `LOVELY_DOTENV`                  | no       | -                           | Set to `1` to load `.env` at startup. Off by default in production. |

`.env.example` at the repo root is the canonical local template — copy to
`.env`, set values, run `LOVELY_DOTENV=1 cargo run -p lovely-server`.

---

## Secrets layout

### Compose (Swarm)

`compose.yaml` references four `external: true` Docker secrets. Create them
once on the manager node:

```sh
printf '%s' "$(openssl rand -hex 32)"            | docker secret create session_secret -
printf '%s' "ghp_xxxxxxxxxxxxxxxx"               | docker secret create github_client_secret -
printf '%s' "GOCSPX-xxxxxxxxxxxxxxxx"            | docker secret create google_client_secret -
docker secret create apple_private_key ./AuthKey_ABC123XYZ4.p8
```

The secrets land at `/run/secrets/<name>` inside the container. The server
reads them either by env (passed via an entrypoint shim) or by path. The
`LOVELY_DB_PASSWORD` value is interpolated into `LOVELY_DATABASE_URL` from
the shell environment of `docker compose up` — keep it in your local
`.env` (gitignored) or pass on the command line.

### Kubernetes

Create a single `lovely-secrets` Secret holding all sensitive values; the
Deployment references it via `valueFrom.secretKeyRef`:

```sh
kubectl create secret generic lovely-secrets \
  --from-literal=SESSION_SECRET="$(openssl rand -hex 32)" \
  --from-literal=GITHUB_CLIENT_SECRET="ghp_xxxxxxxxxxxxxxxx" \
  --from-literal=GOOGLE_CLIENT_SECRET="GOCSPX-xxxxxxxxxxxxxxxx" \
  --from-file=APPLE_PRIVATE_KEY=./AuthKey_ABC123XYZ4.p8 \
  --from-literal=DB_PASSWORD="$(openssl rand -base64 24)" \
  --from-literal=DATABASE_URL="postgres://lovely:THE_DB_PASSWORD@postgres:5432/lovely"
```

`secret.yaml` in this directory is a placeholder template only — never apply
it as-is. Prefer creating the Secret imperatively or via
`sealed-secrets` / `external-secrets`.

---

## Postgres major-version upgrades

`compose.yaml` and `postgres.yaml` both pin `postgres:17`. Postgres data
directories are NOT compatible across major versions: bumping the image
tag (e.g. `postgres:17` -> `postgres:18`) against an existing volume will
fail to start, with an error like:

```
FATAL: database files are incompatible with server
DETAIL: The data directory was initialized by PostgreSQL version 17,
        which is not compatible with this version 18.x.
```

The pod / container will then crashloop until you roll the tag back. To
upgrade safely, follow one of these procedures:

### `pg_upgrade` (in-place, fastest)

1. Snapshot the volume (`kubectl debug`, `volsnap`, or your CSI snapshot tool).
2. Spin up a one-shot pod with both binaries available (`tianon/postgres-upgrade:17-to-18`).
3. Bind both the old `PGDATA` and a new empty `PGDATA` directory.
4. Run `pg_upgrade --link` (or `--copy` for safety) and let it migrate.
5. Update the StatefulSet image tag to `postgres:18`.
6. Roll the StatefulSet, watch readiness, run `ANALYZE` and the post-upgrade
   stats refresh script.

### Dump and restore (simplest, safest, more downtime)

1. `kubectl exec sts/postgres -- pg_dumpall -U lovely > all.sql`.
2. Stop `lovely-server` (`kubectl scale deploy/lovely-server --replicas=0`).
3. Delete the Postgres PVC, bump the StatefulSet image to `postgres:18`,
   re-apply.
4. Wait for the new pod's readiness probe.
5. `cat all.sql | kubectl exec -i sts/postgres-0 -- psql -U lovely`.
6. Scale `lovely-server` back to 1.

For managed Postgres (RDS, Cloud SQL, Crunchy Bridge, Neon) just use the
provider's blue/green or in-place upgrade tooling — the warning still
applies in the sense that the wire URL doesn't change but the major
version under it must be confirmed compatible with the current `sqlx`
support matrix (sqlx supports 11–17 today; bumping to 18 once GA needs
a quick smoke test before rollout).

---

## Apple `.p8` private key rotation

Apple "Sign in with Apple" requires a developer-team-signed JWT as the
OAuth `client_secret`. The signing key is a `.p8` file downloaded from
the Apple Developer portal. Each key has:

- A key id (e.g. `ABC123XYZ4`) — public, goes in `LOVELY_APPLE_KEY_ID`.
- The private key contents — secret, goes in `LOVELY_APPLE_PRIVATE_KEY`.

**Calendar reminder: the team's Apple developer account expires
2026-11-25.** If it lapses the existing key stops working and Apple sign-in
breaks for every user. Rotate before that date.

### Rotation procedure

1. In the Apple Developer portal, create a new "Sign in with Apple" key.
   You may have multiple active keys; the old one keeps working until you
   revoke it.
2. Download the new `AuthKey_<KEY_ID>.p8` (one-time download — store it in
   the password manager immediately).
3. Update the deployed secret:

   - **Compose / Swarm:** `docker secret rm apple_private_key` (after the
     replacement is in place — Swarm will not let you remove a secret in
     use), or roll forward: create `apple_private_key_v2`, change the
     compose mapping, redeploy, then remove the old secret.
   - **Kubernetes:**

     ```sh
     kubectl create secret generic lovely-secrets-new \
       --from-file=APPLE_PRIVATE_KEY=./AuthKey_NEWKEY.p8 \
       --from-literal=... # all other keys
     kubectl delete secret lovely-secrets
     kubectl get secret lovely-secrets-new -o yaml | sed 's/lovely-secrets-new/lovely-secrets/' | kubectl apply -f -
     ```

     or use `kubectl create secret generic lovely-secrets ... --dry-run=client -o yaml | kubectl apply -f -`.

4. Update `LOVELY_APPLE_KEY_ID` to match the new key id.
5. Roll the Deployment: `kubectl rollout restart deploy/lovely-server`.
6. Verify a fresh Apple sign-in flow end-to-end.
7. Once the rollout is stable, revoke the old key in the developer portal.

The server caches the signed Apple JWT for ~5 min and re-signs from the
private key on demand, so a rolling restart is sufficient — no in-process
SIGHUP handling is required for milestone A.

---

## Backups (documented, not implemented in v1)

This is intentionally not wired up yet — the design calls it out as a v2
concern. Two paths are on the table:

### Postgres

- **`pg_dump` cron sidecar.** A small CronJob that runs nightly:

  ```sh
  pg_dump --format=custom --file=/backup/lovely-$(date +%F).dump \
          "$LOVELY_DATABASE_URL"
  ```

  Push the result to S3 / R2 with `aws s3 cp` or `rclone`. Keep 30
  daily + 12 monthly snapshots. Restore via `pg_restore`.

- **Continuous WAL archiving** with `wal-g` or `barman` — only worth the
  ops cost once we have multi-tenant production traffic.

- **Managed Postgres** — defer the entire problem to the provider's
  point-in-time-recovery feature. Recommended for production.

### Per-app SQLite

- **Nightly `.backup` per file.** Walk `LOVELY_SQLITE_DATA_DIR`, run
  `sqlite3 app.db ".backup '/backup/app-$(date +%F).db'"` for each file.
  Safe with WAL because `.backup` takes a consistent snapshot.

- **`litestream`** (preferred path). One sidecar replicates every
  SQLite file's WAL to S3 in near-real-time. Restore is `litestream
  restore -o app.db s3://bucket/app.db`. The cost is a sidecar
  per-pod and a couple of extra fsyncs — acceptable.

When we wire this up, both will live in `deploy/k8s/cronjob-backup.yaml`
and `deploy/k8s/litestream-sidecar.yaml`.

---

## Quick references

```sh
# Local dev (Postgres only)
docker compose -f deploy/compose.yaml up -d postgres
LOVELY_DOTENV=1 cargo run -p lovely-server

# Full local stack
docker compose -f deploy/compose.yaml up --build

# Build the production image
docker build -t lovely-rs:latest -f deploy/Dockerfile .

# K8s dry-run
kubectl apply --dry-run=client -f deploy/k8s/

# K8s apply
kubectl apply -f deploy/k8s/
```
