#!/usr/bin/env bash
# Get a runnable Juno without compiling it here.
#
#   juno-build                  build the current branch in CI, install it, open it
#   juno-build <branch>         same, for a branch that is on GitHub
#   juno-build <tag>            install a published release or prerelease (no build)
#   juno-build promote [tag]    make a prerelease the release users auto-update to
#                               (default: the newest prerelease)
#
#   --no-install   download only and print the path
#   --yes          don't ask before promoting
#
# Branch builds run .github/workflows/build-branch.yml on GitHub's macOS
# runners (free for this public repo). A commit that was already built is
# downloaded, not rebuilt. Downloads live in ~/.cache/juno-builds.
set -euo pipefail

repo="lacymorrow/juno"
dest="/Applications/Juno.app"
cache="$HOME/.cache/juno-builds"

install=1
yes=0
args=()
for a in "$@"; do
  case "$a" in
    --no-install) install=0 ;;
    --yes|-y) yes=1 ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "juno-build: unknown option $a" >&2; exit 2 ;;
    *) args+=("$a") ;;
  esac
done

say() { printf '%s\n' "$*" >&2; }
die() { say "juno-build: $*"; exit 1; }

command -v gh >/dev/null || die "needs the GitHub CLI (brew install gh)"
gh auth status >/dev/null 2>&1 || die "gh is not logged in. Run: gh auth login"

is_release() { gh release view "$1" --repo "$repo" >/dev/null 2>&1; }

# ---------------------------------------------------------------- promote
promote() {
  local tag="${1:-}"
  if [[ -z "$tag" ]]; then
    tag=$(gh release list --repo "$repo" --limit 50 --json tagName,isPrerelease \
      -q '.[] | select(.isPrerelease) | .tagName' \
      | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sed 's/^v//' \
      | sort -t. -k1,1n -k2,2n -k3,3n | tail -1 || true)
    [[ -n "$tag" ]] || die "there is no prerelease to promote"
    tag="v$tag"
  fi

  local info
  info=$(gh release view "$tag" --repo "$repo" --json isPrerelease,assets \
    -q '[.isPrerelease, ([.assets[].name] | index("latest.json") != null)] | @tsv' 2>/dev/null) \
    || die "no release called $tag"
  [[ "${info%%$'\t'*}" == "true" ]] || die "$tag is already a full release"
  [[ "${info##*$'\t'}" == "true" ]] || die "$tag has no latest.json yet; its build may still be running"

  if [[ $yes -eq 0 ]]; then
    [[ -t 0 ]] || die "promoting ships $tag to every user; rerun with --yes to confirm"
    read -r -p "Ship $tag to everyone on the auto-updater? [y/N] " reply
    [[ "$reply" == [yY]* ]] || { say "Not promoted."; exit 1; }
  fi

  gh release edit "$tag" --repo "$repo" --prerelease=false --latest >/dev/null
  say "$tag is now the release users update to: https://github.com/$repo/releases/tag/$tag"
}

# ---------------------------------------------------------------- install
install_app() {
  local app="$1" label="$2"
  if [[ $install -eq 0 ]]; then
    say "Downloaded $label:"
    printf '%s\n' "$app"
    return
  fi

  if pgrep -f "$dest/Contents/MacOS/" >/dev/null; then
    say "Quitting Juno..."
    # Ask the installed app for its own bundle id instead of keeping a copy
    # here: src-tauri/tauri.conf.json owns it and a copy goes stale.
    app_id=$(defaults read "$dest/Contents/Info" CFBundleIdentifier 2>/dev/null || true)
    [[ -n "$app_id" ]] && osascript -e "quit app id \"$app_id\"" >/dev/null 2>&1 || true
    for _ in $(seq 20); do
      pgrep -f "$dest/Contents/MacOS/" >/dev/null || break
      sleep 0.5
    done
    pgrep -f "$dest/Contents/MacOS/" >/dev/null && die "Juno did not quit; quit it and run this again"
  fi

  if [[ -d "$dest" ]]; then
    if command -v trash >/dev/null; then trash "$dest"; else mv "$dest" "$HOME/.Trash/Juno-$(date +%s).app"; fi
  fi
  ditto "$app" "$dest"
  open "$dest"
  say "Installed $label in $dest and opened it. The old copy is in the Trash."
}

extract() {
  local tarball="$1" dir="$2"
  tar -xzf "$tarball" -C "$dir"
  [[ -d "$dir/Juno.app" ]] || die "the download did not contain Juno.app"
}

# ---------------------------------------------------------------- release tag
fetch_release() {
  local tag="$1" dir="$cache/$1"
  if [[ ! -d "$dir/Juno.app" ]]; then
    mkdir -p "$dir"
    say "Downloading $tag..."
    # Arch-agnostic on purpose. Releases up to v0.8.52 published
    # Juno_aarch64.app.tar.gz; universal ones publish
    # Juno_universal.app.tar.gz. Both hold the same Juno.app, and this has to
    # keep installing the tags that already exist.
    gh release download "$tag" --repo "$repo" --pattern 'Juno_*.app.tar.gz' --dir "$dir" --clobber \
      || die "$tag has no app download yet; its build may still be running"
    local tarball
    tarball=$(find "$dir" -maxdepth 1 -name 'Juno_*.app.tar.gz' | head -1)
    [[ -n "$tarball" ]] || die "$tag published no Juno app tarball"
    extract "$tarball" "$dir"
  fi
  install_app "$dir/Juno.app" "$tag"
}

# ---------------------------------------------------------------- branch
build_branch() {
  local ref="$1"
  local sha
  sha=$(gh api "repos/$repo/branches/$ref" -q .commit.sha 2>/dev/null) \
    || die "$ref is not on GitHub. Push it first: git push -u origin $ref"
  local short="${sha:0:8}"

  if git rev-parse --is-inside-work-tree >/dev/null 2>&1 \
     && [[ "$(git rev-parse --abbrev-ref HEAD)" == "$ref" ]]; then
    local head; head=$(git rev-parse HEAD)
    if [[ "$head" != "$sha" ]]; then
      say "Note: your local $ref (${head:0:8}) differs from GitHub's ($short). Building GitHub's."
      say "      Push first if you want your latest commits in the build."
    fi
    [[ -z "$(git status --porcelain)" ]] || say "Note: uncommitted changes are not part of the build."
  fi

  local dir="$cache/$short"
  if [[ -d "$dir/Juno.app" ]]; then
    install_app "$dir/Juno.app" "$ref @ $short"
    return
  fi

  # Reuse a finished build of this exact commit if its artifact is still there.
  local run
  run=$(gh run list --repo "$repo" --workflow build-branch.yml --branch "$ref" --status success \
        --limit 20 --json databaseId,headSha -q ".[] | select(.headSha == \"$sha\") | .databaseId" | head -1)

  if [[ -z "$run" ]]; then
    local started; started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    gh workflow run build-branch.yml --repo "$repo" --ref "$ref" >/dev/null
    say "Asked GitHub to build $ref @ $short. This takes about 15 to 30 minutes."
    for _ in $(seq 30); do
      run=$(gh run list --repo "$repo" --workflow build-branch.yml --branch "$ref" --event workflow_dispatch \
            --limit 5 --json databaseId,createdAt -q ".[] | select(.createdAt >= \"$started\") | .databaseId" | tail -1)
      [[ -n "$run" ]] && break
      sleep 2
    done
    [[ -n "$run" ]] || die "the build was requested but did not show up; see https://github.com/$repo/actions/workflows/build-branch.yml"
    say "Watching https://github.com/$repo/actions/runs/$run (Ctrl-C stops watching, not the build)"
    # The watcher is only a progress display. It also exits non-zero when the
    # network drops or the Mac sleeps, so ask GitHub how the run ended.
    gh run watch "$run" --repo "$repo" --compact --interval 30 >&2 || true
    local status="" conclusion=""
    while :; do
      IFS=$'\t' read -r status conclusion < <(gh run view "$run" --repo "$repo" \
        --json status,conclusion -q '[.status, .conclusion] | @tsv' 2>/dev/null || printf 'unknown\t\n')
      [[ "$status" == "completed" ]] && break
      sleep 30
    done
    if [[ "$conclusion" != "success" ]]; then
      say ""
      say "The build ended with: $conclusion. Last lines of the failing step:"
      gh run view "$run" --repo "$repo" --log-failed 2>/dev/null | tail -30 >&2 || true
      die "full log: https://github.com/$repo/actions/runs/$run"
    fi
  else
    say "$ref @ $short was already built (run $run). Downloading it."
  fi

  mkdir -p "$dir"
  gh run download "$run" --repo "$repo" --name "juno-$short" --dir "$dir" \
    || { rmdir "$dir" 2>/dev/null; die "could not download the build; artifacts expire after 7 days, run this again to rebuild"; }
  extract "$dir/Juno.app.tar.gz" "$dir"
  install_app "$dir/Juno.app" "$ref @ $short"
}

# ---------------------------------------------------------------- main
if [[ "${args[0]:-}" == "promote" ]]; then
  promote "${args[1]:-}"
  exit 0
fi

target="${args[0]:-}"
if [[ -z "$target" ]]; then
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 \
    || die "run this inside a Juno checkout, or name a branch or tag"
  target=$(git rev-parse --abbrev-ref HEAD)
  [[ "$target" != "HEAD" ]] || die "detached HEAD; name a branch or tag"
fi

if is_release "$target"; then
  fetch_release "$target"
else
  build_branch "$target"
fi
