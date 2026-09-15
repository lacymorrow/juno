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
#
# Demo builds: `bun run tauri:build --demo` makes a golden copy that carries an
# Anthropic key, for handing to someone who should be able to open Juno and
# talk to it without signing up for a provider. The key is read from
# ~/.tauri/juno-demo.key (never the repo) and compiled in with option_env!.
# It installs beside a normal Juno under its own bundle id, and it does not
# self-update, because the public build has no key and an update would end the
# demo. The key is readable in the binary by anyone holding it: give the demo
# its own Anthropic workspace with a spend cap, one key per cohort, and hand
# the build out privately.
#
#   JUNO_DEMO_COHORT=sept-investors bun run tauri:build --demo
set -euo pipefail
cd "$(dirname "$0")/.."

key_path="$HOME/.tauri/juno.key"
demo_key_path="$HOME/.tauri/juno-demo.key"
demo=0

args=()
for arg in "$@"; do
  if [[ "$arg" == "--demo" ]]; then
    demo=1
  else
    args+=("$arg")
  fi
done
set -- ${args[@]+"${args[@]}"}

# The key must never reach a normal build. Checked after every build, because
# a stale target dir or a stray export is exactly how that would happen.
assert_no_key_in_binary() {
  local binary
  binary=$(find src-tauri/target -maxdepth 4 -type f -perm -111 -name juno -newermt '-10 minutes' 2>/dev/null | head -1)
  [[ -z "$binary" ]] && return 0
  if strings "$binary" 2>/dev/null | grep -q 'sk-ant-'; then
    echo "tauri-build: FAILED, an Anthropic key is present in a non-demo binary: $binary" >&2
    exit 1
  fi
  echo "tauri-build: checked, no key material in $binary" >&2
}

if [[ "$demo" == "1" ]]; then
  if [[ -z "${JUNO_DEMO_ANTHROPIC_KEY:-}" ]]; then
    if [[ ! -f "$demo_key_path" ]]; then
      cat >&2 <<MSG
tauri-build: --demo needs a key at $demo_key_path

  Put the demo workspace's Anthropic key there (one line, nothing else), or
  export JUNO_DEMO_ANTHROPIC_KEY yourself. Never put it in the repo.
MSG
      exit 1
    fi
    JUNO_DEMO_ANTHROPIC_KEY="$(tr -d '[:space:]' < "$demo_key_path")"
  fi
  export JUNO_DEMO_ANTHROPIC_KEY
  export JUNO_DEMO_COHORT="${JUNO_DEMO_COHORT:-}"
  echo "tauri-build: demo build, key ${JUNO_DEMO_ANTHROPIC_KEY:0:7}... (${#JUNO_DEMO_ANTHROPIC_KEY} chars)${JUNO_DEMO_COHORT:+, cohort $JUNO_DEMO_COHORT}" >&2
  exec bunx tauri build --config '{"productName":"Juno Demo","identifier":"com.juno.desktop.demo","bundle":{"createUpdaterArtifacts":false}}' "$@"
fi

# Not a demo build: make sure nothing exported a demo key into this one.
unset JUNO_DEMO_ANTHROPIC_KEY JUNO_DEMO_COHORT
trap assert_no_key_in_binary EXIT

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
