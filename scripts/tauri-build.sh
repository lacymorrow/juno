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
#
# Every build names itself. The DMG comes out as
#   Juno-Demo-0.7.0-b2174-a19b4631.dmg
# with a .json manifest beside it, and the same facts are compiled into the
# binary by build.rs so Settings can show them. Two builds made on the same day
# used to be indistinguishable once they left this machine.
set -euo pipefail
cd "$(dirname "$0")/.."

key_path="$HOME/.tauri/juno.key"
demo_key_path="$HOME/.tauri/juno-demo.key"
demo=0

# --- Build identity -------------------------------------------------------
# Kept in step with src-tauri/build.rs, which compiles these same facts in.
version="$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)"
build_number="$(git rev-list --count HEAD 2>/dev/null || echo 0)"
commit="$(git rev-parse --short=8 HEAD 2>/dev/null || echo unknown)"
branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
built_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
  dirty=true
else
  dirty=false
fi

# Rename the bundle to something no other build can collide with, and drop a
# manifest next to it so the artifact stays identifiable after it is moved,
# renamed, or mailed to someone.
name_artifacts() {
  local product="$1"
  local built name target dmg_dir=""
  # This is a cargo workspace, so the target directory sits at the workspace
  # root, not under src-tauri. Looking only in src-tauri meant the rename and
  # the manifest silently never ran and every DMG kept its default name. Both
  # are checked because the layout depends on where the workspace is declared.
  local candidate
  for candidate in "target/release/bundle/dmg" "src-tauri/target/release/bundle/dmg"; do
    if [[ -d "$candidate" ]]; then
      dmg_dir="$candidate"
      break
    fi
  done
  if [[ -z "$dmg_dir" ]]; then
    echo "tauri-build: no bundle/dmg directory found, skipping naming" >&2
    return 0
  fi
  built="$(find "$dmg_dir" -maxdepth 1 -name '*.dmg' -newermt '-60 minutes' 2>/dev/null | head -1)"
  if [[ -z "$built" ]]; then
    echo "tauri-build: no DMG found in $dmg_dir, skipping naming" >&2
    return 0
  fi

  name="${product}-${version}-b${build_number}-${commit}"
  [[ "$dirty" == "true" ]] && name="${name}-dirty"
  target="${dmg_dir}/${name}.dmg"
  mv -f "$built" "$target"

  cat > "${dmg_dir}/${name}.json" <<JSON
{
  "product": "${product}",
  "version": "${version}",
  "build": "${build_number}",
  "commit": "${commit}",
  "branch": "${branch}",
  "dirty": ${dirty},
  "builtAt": "${built_at}",
  "demo": $([[ "$demo" == "1" ]] && echo true || echo false),
  "cohort": $([[ -n "${JUNO_DEMO_COHORT:-}" ]] && echo "\"${JUNO_DEMO_COHORT}\"" || echo null),
  "artifact": "${name}.dmg"
}
JSON

  echo "tauri-build: ${target}" >&2
  echo "tauri-build: manifest ${dmg_dir}/${name}.json" >&2
  if [[ "$dirty" == "true" ]]; then
    echo "tauri-build: NOTE this build has uncommitted changes, ${commit} does not fully describe it" >&2
  else
    echo "tauri-build: to mark it shipped: git tag build/${build_number} ${commit}" >&2
  fi
}

# Tauri has been seen returning non-zero after writing both bundles (the DMG
# step detaches a disk image and is flaky about its exit code). Naming the
# artifact is still worth doing, and the original status is still reported, so
# a real failure is never silently turned into a success.
run_build() {
  local product="$1"; shift
  local rc=0
  "$@" || rc=$?
  name_artifacts "$product"
  if [[ $rc -ne 0 ]]; then
    echo "tauri-build: tauri exited $rc; check whether the bundle above is usable" >&2
  fi
  return $rc
}

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
  # The workspace target directory is at the repo root. Searching only
  # src-tauri/target found nothing, returned "no binary, fine", and so this
  # check passed without ever reading a byte of the thing it guards.
  binary=$(find target src-tauri/target -maxdepth 4 -type f -perm -111 -name juno -newermt '-60 minutes' 2>/dev/null | head -1)
  if [[ -z "$binary" ]]; then
    echo "tauri-build: WARNING, no built binary found to scan for key material" >&2
    return 0
  fi
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
  echo "tauri-build: ${version} build ${build_number} ${commit} on ${branch}" >&2
  run_build "Juno-Demo" bunx tauri build --config '{"productName":"Juno Demo","identifier":"com.juno.desktop.demo","bundle":{"createUpdaterArtifacts":false}}' "$@"
  exit $?
fi

# Not a demo build: make sure nothing exported a demo key into this one.
unset JUNO_DEMO_ANTHROPIC_KEY JUNO_DEMO_COHORT
# Not `exec` below: an exec'd process replaces this shell, and the EXIT trap
# that checks for key material would never run.
trap assert_no_key_in_binary EXIT

echo "tauri-build: ${version} build ${build_number} ${commit} on ${branch}" >&2

if [[ "${JUNO_UNSIGNED_BUILD:-}" == "1" ]]; then
  echo "tauri-build: JUNO_UNSIGNED_BUILD=1, building without updater artifacts" >&2
  run_build "Juno" bunx tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}' "$@"
  exit $?
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

run_build "Juno" bunx tauri build "$@"
