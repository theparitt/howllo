#!/usr/bin/env bash
#
# make-zip.sh — bundle the important howllo-server (backend) source into a
# single zip, ready to upload to the web (e.g. to give an LLM full context).
#
# Scope: ONLY the howllo-server/ directory. The frontends and shared packages
#        are intentionally not included.
#
# Includes: Rust source, SQL migrations, Cargo.toml, and .env (dev creds —
#           see note below).
# Excludes: build artifacts that bloat the bundle and add no signal:
#           target/, .sqlx cache, Cargo.lock.
#
# .env NOTE: .env files ARE included per project choice (dev-only throwaway
# credentials). If you ever point this at real secrets, flip INCLUDE_ENV=0.
#
# Usage:
#   ./make-zip.sh            # -> howllo-server-llm-YYYYMMDD-HHMMSS.zip
#   ./make-zip.sh out.zip    # -> out.zip

IS_SOURCED=0
if [ "${BASH_SOURCE[0]}" != "$0" ]; then
  IS_SOURCED=1
fi

CALLER_PWD="$PWD"

abort() {
  if [ "$IS_SOURCED" -eq 1 ]; then
    return 1
  fi
  exit 1
}

# --- Colors ------------------------------------------------------------------
# Only emit ANSI codes when writing to a real terminal and NO_COLOR is unset.
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  BOLD=$'\033[1m';  DIM=$'\033[2m';   RESET=$'\033[0m'
  RED=$'\033[31m';  GREEN=$'\033[32m'; YELLOW=$'\033[33m'
  BLUE=$'\033[34m'; MAGENTA=$'\033[35m'; CYAN=$'\033[36m'
else
  BOLD=''; DIM=''; RESET=''
  RED=''; GREEN=''; YELLOW=''; BLUE=''; MAGENTA=''; CYAN=''
fi

# This script lives inside howllo-server/. Its own directory is the target, and
# its parent is the repo root (used so zip paths keep the howllo-server/ prefix).
TARGET="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_NAME="$(basename "$TARGET")"
ROOT="$(dirname "$TARGET")"

if [ "$IS_SOURCED" -eq 1 ]; then
  trap 'cd "$CALLER_PWD"' RETURN
fi

cd "$ROOT"

INCLUDE_ENV=1

OUT="${1:-howllo-server-llm-$(date +%Y%m%d-%H%M%S).zip}"
# Make OUT absolute so it lands where the user expects regardless of cd.
case "$OUT" in
  /*) : ;;
  *) OUT="$TARGET/$OUT" ;;
esac

# Directories whose entire contents are noise.
PRUNE_DIRS=(
  target .sqlx .git
)

# File globs to keep (backend source + manifests).
KEEP_GLOBS=(
  '*.rs' '*.sql' '*.toml' '*.md' '.env' '.env.*'
)

# Filenames to always drop (lockfiles, and this script itself).
DROP_NAMES=(
  'package-lock.json' 'Cargo.lock' 'pnpm-lock.yaml' 'yarn.lock'
  'make-zip.sh'
)

printf '\n'
printf '%s━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%s\n' "$MAGENTA" "$RESET"
printf '  %s🐺 howllo-server%s %s·%s %smake-zip%s\n' "$BOLD" "$RESET" "$DIM" "$RESET" "$MAGENTA$BOLD" "$RESET"
printf '%s━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%s\n\n' "$MAGENTA" "$RESET"
printf '%s▸%s collecting from %s%s%s\n' "$CYAN" "$RESET" "$DIM" "$TARGET" "$RESET"

# --- Build the file list with find -------------------------------------------
# Start the prune expression: ( -path ./a -o -path ./b ... ) -prune
prune_expr=()
for d in "${PRUNE_DIRS[@]}"; do
  prune_expr+=( -path "*/$d" -o )
done
# remove trailing -o
unset 'prune_expr[${#prune_expr[@]}-1]'

# Keep expression: ( -name '*.rs' -o -name ... )
keep_expr=()
for g in "${KEEP_GLOBS[@]}"; do
  keep_expr+=( -iname "$g" -o )
done
unset 'keep_expr[${#keep_expr[@]}-1]'

# Collect into a temp list (NUL-separated for safe filenames).
TMP_LIST="$(mktemp)"
trap 'rm -f "$TMP_LIST"' EXIT

find "$SERVER_NAME" \
  \( "${prune_expr[@]}" \) -prune -o \
  -type f \( "${keep_expr[@]}" \) -print0 > "$TMP_LIST"

# --- Optionally drop .env and always drop lockfiles --------------------------
FILTERED="$(mktemp)"
trap 'rm -f "$TMP_LIST" "$FILTERED"' EXIT

while IFS= read -r -d '' f; do
  base="$(basename "$f")"

  # Drop lockfiles.
  skip=0
  for d in "${DROP_NAMES[@]}"; do
    [ "$base" = "$d" ] && skip=1 && break
  done
  [ "$skip" = 1 ] && continue

  # Drop .env if disabled.
  if [ "$INCLUDE_ENV" -ne 1 ]; then
    case "$base" in
      .env|.env.*) continue ;;
    esac
  fi

  printf '%s\0' "$f" >> "$FILTERED"
done < "$TMP_LIST"

COUNT="$(tr -cd '\0' < "$FILTERED" | wc -c | tr -d ' ')"
if [ "$COUNT" -eq 0 ]; then
  printf '%s✗ no files matched — nothing to zip.%s\n' "$RED" "$RESET" >&2
  abort
fi

printf '%s▸%s found %s%s%s files to bundle\n' "$CYAN" "$RESET" "$BOLD" "$COUNT" "$RESET"

# --- Zip ---------------------------------------------------------------------
rm -f "$OUT"
# zip -@ reads a newline-separated list; translate from our NUL-separated one.
tr '\0' '\n' < "$FILTERED" | zip -q "$OUT" -@

SIZE="$(du -h "$OUT" | cut -f1)"

# --- Summary -----------------------------------------------------------------
printf '\n%s✔ done%s\n' "$GREEN$BOLD" "$RESET"
printf '  %sfiles%s   %s%s%s\n'  "$DIM" "$RESET" "$BOLD" "$COUNT" "$RESET"
printf '  %ssize%s    %s%s%s\n'  "$DIM" "$RESET" "$BOLD" "$SIZE"  "$RESET"
printf '  %soutput%s  %s%s%s\n'  "$DIM" "$RESET" "$YELLOW" "$OUT" "$RESET"
printf '\n%s↑%s upload the zip above to share full project context.\n\n' "$BLUE" "$RESET"
