#!/bin/sh
set -eu
for f in /run/secrets/lovelyrs_*; do
  [ -f "$f" ] || continue
  name=$(basename "$f")
  key="LOVELY_${name#lovelyrs_}"
  export "$key=$(cat "$f")"
done
exec /usr/local/bin/lovely-server
