#!/usr/bin/env bash
# Production build wrapper: `bun run tauri:build [extra tauri args]`.
#
# tauri.conf.json sets bundle.createUpdaterArtifacts = true, so `tauri build`
# refuses to run unless TAURI_SIGNING_PRIVATE_KEY is set. CI gets it from a
# GitHub secret (.github/workflows/release-tauri.yml). Locally the key lives at
# ~/.tauri/juno.key; this script loads it so a plain build just works.
#
# Env:
#   TAURI_SIGNING_PRIVATE_KEY           key content or path; honoured as-is when set
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD  key password; if unset, tauri prompts on a TTY
#   JUNO_UNSIGNED_BUILD=1               skip updater artifacts entirely (no key needed)
set -euo pipefail
cd "$(dirname "$0")/.."

key_path="$HOME/.tauri/juno.key"

if [[ "${JUNO_UNSIGNED_BUILD:-}" == "1" ]]; then
  echo "tauri-build: JUNO_UNSIGNED_BUILD=1, building without updater artifacts" >&2
  exec bunx tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}' "$@"
fi

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  if [[ ! -f "$key_path" ]]; then
    cat >&2 <<MSG
tauri-build: no updater signing key found at $key_path

  Signed build:    put the key at that path, or export TAURI_SIGNING_PRIVATE_KEY
  Unsigned build:  JUNO_UNSIGNED_BUILD=1 bun run tauri:build
MSG
    exit 1
  fi
  TAURI_SIGNING_PRIVATE_KEY="$(<"$key_path")"
  export TAURI_SIGNING_PRIVATE_KEY
  echo "tauri-build: signing updater artifacts with $key_path" >&2
fi

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" && ! -t 0 ]]; then
  echo "tauri-build: TAURI_SIGNING_PRIVATE_KEY_PASSWORD is unset and stdin is not a terminal; tauri cannot prompt for it" >&2
  exit 1
fi

exec bunx tauri build "$@"
