#!/bin/sh
set -eu
# Read Swarm secrets mounted at /run/secrets/lovelyrs_* and export as env vars.
# Strips the lovelyrs_ prefix: lovelyrs_LOVELY_DATABASE_URL → LOVELY_DATABASE_URL.
# Falls through harmlessly when no secrets are mounted (local dev or env-var mode).
for f in /run/secrets/lovelyrs_*; do
    [ -f "$f" ] || continue
    key="${f##*/lovelyrs_}"
    export "$key=$(cat "$f")"
done
exec "$@"
