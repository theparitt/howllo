#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
env_file="$repo_dir/.env.quickstart"

for command_name in docker openssl curl; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Missing command: %s\n' "$command_name" >&2
    exit 1
  fi
done

if [[ ! -f "$env_file" ]]; then
  umask 077
  {
    printf 'POSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 24)"
    printf 'HOWLLO_JWT_SECRET=%s\n' "$(openssl rand -hex 32)"
    printf 'HOWLLO_ADMIN_JWT_SECRET=%s\n' "$(openssl rand -hex 32)"
    printf 'HOWLLO_ADMIN_BOOTSTRAP_KEY=%s\n' "$(openssl rand -hex 24)"
    printf 'HOWLLO_OIDC_CONFIG_KEY=%s\n' "$(openssl rand -hex 32)"
    printf 'HOWLLO_EMAIL_CONFIG_KEY=%s\n' "$(openssl rand -hex 32)"
  } > "$env_file"
fi

if command -v git >/dev/null 2>&1 && git -C "$repo_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  export HOWLLO_IMAGE_TAG="$(git -C "$repo_dir" rev-parse HEAD)"
fi

compose=(docker compose --env-file "$env_file" -f "$repo_dir/compose.quickstart.yml")
"${compose[@]}" pull
"${compose[@]}" up -d

printf 'Waiting for the sample board'
for attempt in $(seq 1 90); do
  if curl -fsS http://localhost:7700/api/ready >/dev/null 2>&1 \
    && curl -fsSL http://localhost:7703/ | grep -q 'Community'; then
    printf '\n\nReady:\n'
    printf '  Board:  http://localhost:7703\n'
    printf '  Staff:  http://localhost:7702\n'
    printf '  API:    http://localhost:7700/api/health\n\n'
    printf 'Stop: docker compose --env-file .env.quickstart -f compose.quickstart.yml down\n'
    exit 0
  fi
  printf '.'
  sleep 2
done

printf '\nHowllo did not become ready. Check the logs:\n' >&2
printf '  docker compose --env-file .env.quickstart -f compose.quickstart.yml logs --tail=80\n' >&2
exit 1
