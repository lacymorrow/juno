#!/usr/bin/env bash
# Production build wrapper: `bun run tauri:build [extra tauri args]`.
#
# tauri.conf.json sets bundle.createUpdaterArtifacts = true, so `tauri build`
# refuses to run unless TAURI_SIGNING_PRIVATE_KEY is set. CI gets it from a
# GitHub secret (.github/workflows/release-tauri.yml). Locally the key lives at
# ~/.tauri/juno.key; this script loads it so a plain build just works.
#
# That updater key is not the same thing as Apple code signing, and confusing
# the two cost us a demo. This script also does the Apple half now: it finds
# the Developer ID identity, hands it to tauri so the bundle is signed, then
# notarizes and staples the DMG and tells you whether the result will actually
# open on someone else's Mac. Before that, a locally built DMG was ad-hoc
# signed with no Team ID and no notarization ticket. It ran fine here and
# died with "This app is damaged and can't be opened" on the first Mac that
# downloaded it, because downloads are quarantined and Gatekeeper will not
# accept an ad-hoc signature. CI had been signing all along; the local path
# never did and never said so.
#
# Env:
#   TAURI_SIGNING_PRIVATE_KEY           key content or path; honoured as-is when set
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD  key password; if unset, tauri prompts on a TTY
#   APPLE_SIGNING_IDENTITY              Developer ID identity; auto-detected when unset
#   JUNO_NOTARY_PROFILE                 notarytool keychain profile (default: juno)
#   JUNO_SKIP_NOTARIZE=1                sign, but skip notarizing and stapling
#   JUNO_UNSIGNED_BUILD=1               no updater artifacts, no Apple signing, no
#                                       notarization; local testing only
#
# Signing is not separately skippable. It costs seconds, it needs no network,
# and every build that is worth bundling is worth signing. The only supported
# way to get an unsigned artifact is JUNO_UNSIGNED_BUILD=1, which announces
# itself loudly at both ends of the build so it cannot be mistaken for
# something shippable.
#
# Demo builds: `bun run tauri:build --demo` makes a golden copy that carries an
# Anthropic key, for handing to someone who should be able to open Juno and
# talk to it without signing up for a provider. The key is read from
# ~/.tauri/juno-demo.key (never the repo) and compiled in with option_env!.
# It installs beside a normal Juno under its own bundle id, and it does not
# self-update, because the public build has no key and an update would end the
# demo. The key is readable in the binary by anyone holding it: give the demo
# its own Anthropic workspace with a spend cap, one key per cohort, and hand
# the build out privately. The demo gets signed and notarized like any other
# build: it is the one most likely to be handed to someone else, so it is the
# one that most needs to open cleanly.
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

# Set by the arg loop, read by name_artifacts.
build_target=""
# Set by name_artifacts, read by finish_macos_signing. Without a DMG there is
# nothing to notarize, and saying so beats failing on an empty path.
named_dmg=""
named_app=""
# The .app basename tauri writes, which is the productName and not the artifact
# prefix. The demo overrides both.
product_app_name="Juno"

signing_identity=""
notary_profile="${JUNO_NOTARY_PROFILE:-juno}"

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

# Anything that decides whether an artifact is shippable has to survive
# scrolling past thousands of lines of cargo output. A one-line warning in
# that stream is invisible, which is exactly how an unsigned demo got handed
# over. So those messages get a box and blank lines around them.
banner() {
  local line
  echo >&2
  echo "################################################################################" >&2
  for line in "$@"; do
    if [[ -z "$line" ]]; then
      echo "#" >&2
    else
      echo "# $line" >&2
    fi
  done
  echo "################################################################################" >&2
  echo >&2
}

# --- Apple code signing ---------------------------------------------------
# Tauri signs the .app and the DMG during bundling when APPLE_SIGNING_IDENTITY
# is set (tauri.conf.json already asks for the hardened runtime and the
# entitlements file, both of which notarization requires). So all this has to
# do is find the right identity and export it.
detect_signing_identity() {
  if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    signing_identity="$APPLE_SIGNING_IDENTITY"
    echo "tauri-build: signing as ${signing_identity} (from APPLE_SIGNING_IDENTITY)" >&2
    return 0
  fi

  # find-identity prints `  1) <sha1> "<name>"`. Only Developer ID Application
  # certs are usable for distribution; an Apple Development or Mac Developer
  # cert lists here too and produces something that Gatekeeper rejects just as
  # hard as an ad-hoc signature, so match the name exactly.
  local candidates
  candidates="$(security find-identity -v -p codesigning 2>/dev/null \
    | sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p' \
    | sort -u)"

  if [[ -z "$candidates" ]]; then
    return 1
  fi

  local count
  count="$(printf '%s\n' "$candidates" | wc -l | tr -d ' ')"
  signing_identity="$(printf '%s\n' "$candidates" | head -1)"

  if [[ "$count" -gt 1 ]]; then
    # Deterministic and out loud beats convenient. Silently picking one of
    # several certs is how a build goes out signed by the wrong team and you
    # hear about it from whoever downloaded it.
    local msg=("More than one Developer ID Application identity is installed:" "")
    local candidate
    while IFS= read -r candidate; do
      msg+=("    ${candidate}")
    done <<<"$candidates"
    msg+=("" "Picking the first in sorted order:" "    ${signing_identity}" "" \
          "Set APPLE_SIGNING_IDENTITY to pick a different one.")
    banner "${msg[@]}"
  fi

  export APPLE_SIGNING_IDENTITY="$signing_identity"
  echo "tauri-build: signing as ${signing_identity}" >&2
  return 0
}

# Answered locally, in well under a second: notarytool reads the keychain
# before it touches the network, so a missing profile fails immediately. Only
# that case is fatal. Being offline, or Apple having a bad afternoon, is not
# worth blocking a build over, because the real submission reports it anyway.
notary_profile_present() {
  local out
  if out="$(xcrun notarytool history --keychain-profile "$notary_profile" 2>&1)"; then
    return 0
  fi
  if grep -q 'No Keychain password item found' <<<"$out"; then
    return 1
  fi
  echo "tauri-build: could not reach the notary service to check profile '${notary_profile}'; continuing, notarization may still fail" >&2
  return 0
}

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
  #
  # A --target argument moves it again, to target/<triple>/release, which is
  # what `bun run build:universal` does. That was the same silent miss, and it
  # now also decides whether the build gets notarized, so the triple is
  # checked first when one was given.
  local candidates=()
  if [[ -n "$build_target" ]]; then
    candidates+=("target/${build_target}/release/bundle/dmg" "src-tauri/target/${build_target}/release/bundle/dmg")
  fi
  candidates+=("target/release/bundle/dmg" "src-tauri/target/release/bundle/dmg")

  local candidate
  for candidate in "${candidates[@]}"; do
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

  named_dmg="$target"
  # The .app sits beside the dmg directory. It is what Gatekeeper actually
  # assesses, so the verification step needs it, and stapling it is what lets
  # a copy dragged out of the DMG launch on a Mac that is offline.
  local macos_dir="${dmg_dir%/dmg}/macos"
  if [[ -d "${macos_dir}/${product_app_name}.app" ]]; then
    named_app="${macos_dir}/${product_app_name}.app"
  fi

  echo "tauri-build: ${target}" >&2
  echo "tauri-build: manifest ${dmg_dir}/${name}.json" >&2
  if [[ "$dirty" == "true" ]]; then
    echo "tauri-build: NOTE this build has uncommitted changes, ${commit} does not fully describe it" >&2
  else
    echo "tauri-build: to mark it shipped: git tag build/${build_number} ${commit}" >&2
  fi
}

# Notarization is a round trip to Apple: the whole DMG is uploaded, scanned,
# and a ticket comes back, which is then stapled into the file so Gatekeeper
# can check it without a network connection. Minutes, not seconds, so say what
# is happening and let notarytool's own output through rather than sitting
# silent and looking hung.
notarize_and_staple() {
  local dmg="$1" app="$2"
  local notary_log rc=0 size submission_id status

  size="$(du -h "$dmg" 2>/dev/null | cut -f1 | tr -d ' ')"
  echo "tauri-build: notarizing ${dmg##*/} (${size}) with keychain profile '${notary_profile}'" >&2
  echo "tauri-build: this uploads the DMG to Apple and waits for a verdict, usually 2-10 minutes" >&2

  # Kept in a temp file rather than a variable so the live output still reaches
  # the terminal and the exit status still belongs to notarytool and not to
  # tee. The file is left in $TMPDIR on purpose: when notarization fails, the
  # first thing you want is the whole transcript.
  notary_log="$(mktemp -t juno-notarize)"
  set +e
  xcrun notarytool submit "$dmg" \
    --keychain-profile "$notary_profile" \
    --wait --timeout 30m 2>&1 | tee "$notary_log" >&2
  rc=${PIPESTATUS[0]}
  set -e

  submission_id="$(sed -n 's/^ *id: *//p' "$notary_log" | head -1)"
  status="$(sed -n 's/^ *status: *//p' "$notary_log" | tail -1)"

  # notarytool has been known to exit 0 on an Invalid verdict, so the status
  # line is checked as well as the exit code.
  if [[ $rc -ne 0 || "$status" != "Accepted" ]]; then
    echo "tauri-build: notarization did not succeed (status: ${status:-none}, exit ${rc})" >&2
    # "Invalid" on its own tells you nothing. The log names the nested binary
    # that was unsigned or missing the hardened runtime, and it is the only
    # way to fix it, so fetch it now instead of leaving a breadcrumb.
    if [[ -n "$submission_id" ]]; then
      echo "tauri-build: fetching the notary log for ${submission_id}" >&2
      xcrun notarytool log "$submission_id" --keychain-profile "$notary_profile" >&2 || true
    fi
    echo "tauri-build: full notary output: ${notary_log}" >&2
    return 1
  fi

  echo "tauri-build: notarized, stapling the ticket" >&2
  xcrun stapler staple "$dmg" >&2

  # The copy inside the DMG cannot be changed after the DMG is built, so this
  # staples the .app that is still on disk. Notarizing a DMG notarizes
  # everything inside it, so the ticket for this app already exists.
  if [[ -n "$app" && -d "$app" ]]; then
    xcrun stapler staple "$app" >&2 || echo "tauri-build: WARNING could not staple ${app}; the DMG is still stapled" >&2
  fi
  return 0
}

# The point of the whole change. Signing that silently fails is no better than
# not signing, so the build says out loud whether the artifact will open on
# someone else's Mac, using the same three checks that diagnosed the original
# failure.
verify_gatekeeper() {
  local dmg="$1" app="$2" ok=1 out

  echo "tauri-build: checking the artifact the way a downloader's Mac will" >&2

  if [[ -n "$app" && -d "$app" ]]; then
    out="$(codesign -dv --verbose=4 "$app" 2>&1 || true)"
    printf '%s\n' "$out" | grep -E '^(Signature|TeamIdentifier|Authority|Runtime)' | sed 's/^/tauri-build:   /' >&2
    printf '%s\n' "$out" | grep -q 'Signature=adhoc' && ok=0
    printf '%s\n' "$out" | grep -q 'TeamIdentifier=not set' && ok=0

    # -t install and the default exec assessment give the same answer for a
    # .app on macOS 15 and 26; install is used because it is what the original
    # diagnosis used and it reads the same as what a downloader triggers.
    out="$(spctl -a -vvv -t install "$app" 2>&1)" || ok=0
    printf '%s\n' "$out" | sed 's/^/tauri-build:   /' >&2
    # "accepted" alone is not enough: an app can be accepted from a local
    # exception or a developer policy and still fail on a stranger's Mac.
    # Notarized Developer ID is the only source that travels.
    printf '%s\n' "$out" | grep -q 'Notarized Developer ID' || ok=0
  else
    echo "tauri-build:   no .app found to assess" >&2
    ok=0
  fi

  out="$(xcrun stapler validate "$dmg" 2>&1)" || ok=0
  printf '%s\n' "$out" | sed 's/^/tauri-build:   /' >&2

  return $(( 1 - ok ))
}

# Called at the end of every build path, including the demo. Prints the last
# thing you see, which is the only thing that matters: whether this is
# shippable.
finish_macos_signing() {
  if [[ -z "$named_dmg" ]]; then
    echo "tauri-build: no DMG was produced, nothing to notarize" >&2
    return 0
  fi

  if [[ "${JUNO_UNSIGNED_BUILD:-}" == "1" ]]; then
    banner "UNSIGNED BUILD. NOT SHIPPABLE." \
           "" \
           "  ${named_dmg}" \
           "" \
           "JUNO_UNSIGNED_BUILD=1 was set, so this artifact has no Developer ID" \
           "signature and no notarization ticket. macOS quarantines anything that" \
           "is downloaded, and Gatekeeper WILL refuse this with \"This app is" \
           "damaged and can't be opened. You should move it to the Trash.\"" \
           "" \
           "It runs on this Mac. Do not hand it to anyone."
    return 0
  fi

  if [[ "${JUNO_SKIP_NOTARIZE:-}" == "1" ]]; then
    verify_gatekeeper "$named_dmg" "$named_app" || true
    banner "SIGNED, BUT NOT NOTARIZED. NOT SHIPPABLE." \
           "" \
           "  ${named_dmg}" \
           "" \
           "JUNO_SKIP_NOTARIZE=1 was set. The bundle carries a real Developer ID" \
           "signature, but no notarization ticket, so once it has been downloaded" \
           "Gatekeeper WILL refuse it with \"Apple could not verify this app is" \
           "free of malware.\"" \
           "" \
           "Rebuild without JUNO_SKIP_NOTARIZE before handing this to anyone."
    return 0
  fi

  if ! notarize_and_staple "$named_dmg" "$named_app"; then
    banner "NOTARIZATION FAILED. NOT SHIPPABLE." \
           "" \
           "  ${named_dmg}" \
           "" \
           "The bundle is signed but Apple did not issue a ticket, so once it has" \
           "been downloaded Gatekeeper WILL refuse it. The notary log printed" \
           "above names the file that caused it, usually a nested binary that was" \
           "not signed or was built without the hardened runtime."
    return 1
  fi

  if verify_gatekeeper "$named_dmg" "$named_app"; then
    banner "SIGNED, NOTARIZED, STAPLED. SHIPPABLE." \
           "" \
           "  ${named_dmg}" \
           "" \
           "Gatekeeper accepts it as a Notarized Developer ID app and the ticket is" \
           "stapled, so it opens on someone else's Mac, downloaded, offline, first" \
           "try, with no right-click-Open dance."
    return 0
  fi

  banner "VERIFICATION FAILED. DO NOT SHIP THIS." \
         "" \
         "  ${named_dmg}" \
         "" \
         "Notarization reported success but the checks above disagree, so" \
         "something is wrong with the bundle itself. Read the codesign, spctl and" \
         "stapler output above before this artifact goes anywhere."
  return 1
}

# Tauri has been seen returning non-zero after writing both bundles (the DMG
# step detaches a disk image and is flaky about its exit code). Naming the
# artifact is still worth doing, and the original status is still reported, so
# a real failure is never silently turned into a success. Notarization hangs
# off the same reasoning: it runs whenever a DMG actually came out, because
# that flaky exit code otherwise costs you a shippable build.
run_build() {
  local product="$1"; shift
  local rc=0 sign_rc=0
  "$@" || rc=$?
  name_artifacts "$product"
  if [[ $rc -ne 0 ]]; then
    echo "tauri-build: tauri exited $rc; check whether the bundle above is usable" >&2
  fi
  finish_macos_signing || sign_rc=$?
  if [[ $rc -eq 0 ]]; then
    rc=$sign_rc
  fi
  return $rc
}

args=()
expect_target=0
for arg in "$@"; do
  if [[ "$arg" == "--demo" ]]; then
    demo=1
    continue
  fi
  # --target is remembered, not consumed: tauri still needs the flag. It moves
  # the bundle to target/<triple>/release, and name_artifacts has to look there
  # or the rename, the manifest, and the notarization all skip a
  # cross-compiled build without saying anything.
  if [[ $expect_target -eq 1 ]]; then
    build_target="$arg"
    expect_target=0
  elif [[ "$arg" == "--target" ]]; then
    expect_target=1
  elif [[ "$arg" == --target=* ]]; then
    build_target="${arg#--target=}"
  fi
  args+=("$arg")
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

# --- Signing pre-flight ---------------------------------------------------
# Deliberately before the build, not after. A build here is twenty to thirty
# minutes; learning that you have no certificate should take one second. It is
# also why a missing identity is an error and not a warning: an unsigned build
# is legitimate for local testing, but it must never be mistaken for a
# shippable one, so it has to be asked for by name.
if [[ "${JUNO_UNSIGNED_BUILD:-}" == "1" ]]; then
  banner "JUNO_UNSIGNED_BUILD=1: building with no Apple code signing and no" \
         "notarization. The result runs on this Mac only. Anyone who downloads" \
         "it gets \"This app is damaged and can't be opened.\""
else
  if ! detect_signing_identity; then
    banner "NO CODE SIGNING IDENTITY. REFUSING TO BUILD." \
           "" \
           "There is no \"Developer ID Application\" certificate in the keychain, so" \
           "this build would come out ad-hoc signed with no Team ID. macOS" \
           "quarantines downloads, and Gatekeeper WILL refuse an ad-hoc signed app" \
           "with \"This app is damaged and can't be opened. You should move it to" \
           "the Trash.\" It would work on this Mac and nowhere else." \
           "" \
           "Fix it with one of:" \
           "  Install the Developer ID Application certificate into the login" \
           "    keychain (Xcode > Settings > Accounts > Manage Certificates, or" \
           "    double-click the .p12), then check:" \
           "      security find-identity -v -p codesigning" \
           "  Name one explicitly:" \
           "      APPLE_SIGNING_IDENTITY=\"Developer ID Application: You (TEAMID)\" \\" \
           "        bun run tauri:build" \
           "" \
           "Or build anyway, for local testing only, never to hand to anyone:" \
           "      JUNO_UNSIGNED_BUILD=1 bun run tauri:build"
    exit 1
  fi

  if [[ "${JUNO_SKIP_NOTARIZE:-}" == "1" ]]; then
    banner "JUNO_SKIP_NOTARIZE=1: the build will be signed but not notarized." \
           "Fine while iterating. Not shippable: a downloaded copy is refused with" \
           "\"Apple could not verify this app is free of malware.\""
  elif ! notary_profile_present; then
    banner "NO NOTARY CREDENTIALS. REFUSING TO BUILD." \
           "" \
           "notarytool has no keychain profile named '${notary_profile}'. A signed but" \
           "un-notarized build is still refused once it has been downloaded: macOS" \
           "says \"Apple could not verify this app is free of malware.\"" \
           "" \
           "Create the profile once (it is stored in the keychain, not in the repo):" \
           "  xcrun notarytool store-credentials ${notary_profile} \\" \
           "    --apple-id <your-apple-id> --team-id <TEAMID> \\" \
           "    --password <app-specific-password>" \
           "" \
           "Already have one under another name:" \
           "  JUNO_NOTARY_PROFILE=<name> bun run tauri:build" \
           "" \
           "Or build without notarizing, for local testing only:" \
           "  JUNO_SKIP_NOTARIZE=1 bun run tauri:build"
    exit 1
  fi
fi

if [[ "$demo" == "1" ]]; then
  product_app_name="Juno Demo"
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
