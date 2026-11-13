#!/usr/bin/env bash
#
# Rewrite the juno-cua Homebrew formula for a release, then PROVE the rewrite
# landed before the caller is allowed to commit it.
#
# Why the proof is the point (LAC-7401). The homebrew job in
# .github/workflows/release-cua.yml used to rewrite the formula inline and then
# run `git commit -m ... || echo "No changes"`. That `||` cannot tell two cases
# apart:
#
#   1. Nothing to commit because this version is already published. Legitimate -
#      re-running a release must stay green.
#   2. Nothing to commit because the rewrite wrote NOTHING. The patterns below
#      matched nothing: the formula was reformatted, renamed, had its
#      `version "..."` or `sha256 "..."` style changed, or grew a third platform
#      block. The tap stays on the previous version while the release announces
#      the new one.
#
# In case 2 the job was green. PR #960's `remote HEAD == local HEAD` assertion
# passed too, because it is a liveness check on the push, not a correctness
# check on the content: nothing was committed, so local HEAD never moved, so the
# equality held vacuously. Measured on a scratch clone: `No changes`,
# `Everything up-to-date`, assertion PASSED, exit 0, formula still at the old
# version. `brew install lacymorrow/tap/juno-cua` then serves the old binary.
#
# So this script asserts the FILE, separately from the workflow asserting the
# PUSH. Anything that reaches the caller's `git commit` has already been proven
# correct on disk, which is what makes the "nothing to commit" branch safe to
# keep for case 1: a formula already at the target version passes every check
# here and commits nothing.
#
# Lives in a script rather than inline in release-cua.yml so these branches can
# be tested - scripts/__tests__/update-homebrew-formula.test.sh - because
# otherwise their only exercise is a real release, which is the whole reason
# case 2 shipped unnoticed. That moves it out of actionlint's shellcheck reach,
# so ci.yml shellchecks this file explicitly, the same arrangement
# scripts/notarize-dmg.sh and scripts/release-failure-report.sh have.
#
# Usage:  update-homebrew-formula.sh <path-to-Formula/juno-cua.rb>
# Env:    VERSION    the X.Y.Z being published (no `cua-v` prefix)
#         ARM64_SHA  sha256 of juno-cua-darwin-arm64.tar.gz
#         X64_SHA    sha256 of juno-cua-darwin-x64.tar.gz
#
# Does no git and no network: it rewrites one file in place and exits non-zero
# if the result is not what was asked for.

set -euo pipefail

formula="${1:-}"
if [ -z "$formula" ]; then
  echo "::error title=update-homebrew-formula::usage: update-homebrew-formula.sh <path-to-juno-cua.rb>"
  exit 1
fi
if [ ! -f "$formula" ]; then
  echo "::error title=Formula missing::$formula does not exist. The tap layout changed, or the clone failed."
  exit 1
fi

die() { # die <title> <message>
  echo "::error title=$1::$2"
  exit 1
}

# The inputs are evidence later in this script, so a malformed one has to fail
# here rather than be asserted about. An empty ARM64_SHA is the sharp case: it
# writes `sha256 ""` into the formula and then `grep -q "${ARM64_SHA}"` is
# `grep -q ""`, which matches every file ever written. A fail-open check is
# worse than no check, because it reads as coverage.
[ -n "${VERSION:-}" ]   || die "VERSION unset" "VERSION is required (the X.Y.Z being published)."
[ -n "${ARM64_SHA:-}" ] || die "ARM64_SHA unset" "ARM64_SHA is required."
[ -n "${X64_SHA:-}" ]   || die "X64_SHA unset" "X64_SHA is required."

printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
  || die "Bad VERSION" "'$VERSION' is not X.Y.Z. The gate job strips the cua-v prefix; this wants the bare version."
printf '%s' "$ARM64_SHA" | grep -Eq '^[0-9a-f]{64}$' \
  || die "Bad ARM64_SHA" "ARM64_SHA is not 64 lowercase hex characters (got ${#ARM64_SHA} chars). An empty or truncated checksum would make the assertions below pass vacuously."
printf '%s' "$X64_SHA" | grep -Eq '^[0-9a-f]{64}$' \
  || die "Bad X64_SHA" "X64_SHA is not 64 lowercase hex characters (got ${#X64_SHA} chars)."
[ "$ARM64_SHA" != "$X64_SHA" ] \
  || die "Identical checksums" "ARM64_SHA and X64_SHA are the same value ($ARM64_SHA). Two different archives cannot share a checksum, so the build's sha step produced one of them twice."

# count <fixed-string> -- lines in the formula containing it.
#
# grep exits 1 on zero matches, which `set -e` would turn into a silent abort,
# and exits 2 on a real error. Collapsing both to "0" is how an output-parsing
# gate becomes a silent pass, so the two are told apart and only rc<=1 is a
# count. -F throughout: every pattern here is a literal, and a version like
# 0.4.11 read as a regex also matches 0X4Y11.
#
# Reports to stderr rather than through die(): every caller is a `$(count ...)`
# substitution, which would capture an annotation written to stdout into the
# variable instead of the log. The non-zero exit still aborts the script through
# `set -e` on the failed assignment.
count() {
  local n rc=0
  n=$(grep -cF -- "$1" "$formula") || rc=$?
  if [ "$rc" -gt 1 ]; then
    echo "::error title=grep failed::grep exited $rc counting '$1' in $formula." >&2
    exit 1
  fi
  printf '%s\n' "$n"
}

# One scratch file for the whole script, cleaned up on every exit path. `set -e`
# aborting midway would otherwise leave it behind; harmless on an ephemeral
# runner, but a leak that only ever happens on the failure path is the kind
# nobody notices until it is not harmless.
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

# replace_formula -- overwrite the formula with $tmp, preserving its mode.
# `mv` would move mktemp's 0600 onto it instead. git tracks only the exec bit so
# the committed blob is identical either way, but leaving a release artifact
# readable only by the runner user is a pointless difference from what every
# other writer of this file produces.
replace_formula() { cat "$tmp" >"$formula"; }

# rewrite <sed-expression> -- `sed -i` is not portable (BSD wants `-i ''`, GNU
# refuses it), and this runs on macos-latest in the release and ubuntu-latest
# in CI's test. A temp file behaves the same on both.
rewrite() {
  sed "$1" "$formula" >"$tmp"
  replace_formula
}

# --- precondition ---------------------------------------------------------

# The checksum pass below is POSITIONAL: first sha256 line gets arm64, second
# gets x64. That assumption is only safe while there are exactly two, so it is
# checked rather than hoped for. A formula that grew a universal block (or lost
# a platform) fails the release red here instead of quietly keeping a stale
# checksum on the block the pass never reached.
#
# The remaining hole this does NOT close is a SWAP: reorder the
# `if Hardware::CPU.arm?` / `.intel?` blocks and both checksums are still
# present and the count is still 2, but each is in the wrong block. Keying the
# rewrite on the enclosing block retires that, and is LAC-7748.
before_shas="$(count 'sha256 "')"
if [ "$before_shas" != 2 ]; then
  die "Unexpected formula shape" "expected 2 'sha256 \"' lines in $formula, found $before_shas. The checksum rewrite assigns arm64 to the first and x64 to the second, so it cannot be trusted on this formula. Update scripts/update-homebrew-formula.sh (see LAC-7748) rather than loosening this check."
fi

# --- rewrite --------------------------------------------------------------

# Anchored to the line's first non-whitespace token. The unanchored
# `s/version ".*"/.../` this replaces would also rewrite any other line
# containing `version "` - a comment, or a `desc` that mentions one - and its
# greedy `.*` would eat the rest of that line. The anchor takes any
# indentation, so it is not coupled to the formula's current two spaces; what it
# requires is that `version` be the first thing on the line, which is the only
# place a formula declares one. A reindented formula that no longer matches
# fails the assertion below rather than being quietly half-rewritten.
#
# `[^"]*` rather than `.*`, matching the checksum pattern below: BRE has no
# non-greedy quantifier, so `.*"` on `version "1.0" # see "docs"` would run to
# the LAST quote and delete the trailing comment. The value a formula declares
# never contains a quote, so the character class is exact.
rewrite "s/^\\([[:space:]]*\\)version \"[^\"]*\"/\\1version \"${VERSION}\"/"
# CLI assets live on the cua-v* release (see the gate job's tag), not the app's
# v* release.
rewrite "s|releases/download/[^/]*/|releases/download/cua-v${VERSION}/|g"

# Checksums, first occurrence arm64 and second x64. awk rather than the `python3
# -c "..."` block this replaced: that interpolated the checksums into python
# source through the shell, and shellcheck cannot see inside a heredoc, so the
# one part of the step with a real injection surface was also the one part no
# linter read. The validation above means neither value can carry an awk-special
# character, and the behaviour is identical for any formula passing the
# precondition.
ARM64_SHA="$ARM64_SHA" X64_SHA="$X64_SHA" awk '
  /sha256 "/ && n < 2 {
    n++
    sha = (n == 1 ? ENVIRON["ARM64_SHA"] : ENVIRON["X64_SHA"])
    sub(/sha256 "[^"]*"/, "sha256 \"" sha "\"")
  }
  { print }
' "$formula" >"$tmp"
replace_formula

# --- prove it landed ------------------------------------------------------

# The version DECLARATION, anchored the same way the rewrite is - not merely the
# string somewhere in the file, which a comment mentioning the target version
# would satisfy. Same distinction the checksum checks below make. VERSION is
# validated above as digits and dots, so escaping the dots is all it takes to
# make this a literal.
version_decl="^[[:space:]]*version \"$(printf '%s' "$VERSION" | sed 's/\./\\./g')\""
grep -q -- "$version_decl" "$formula" \
  || die "Version not written" "$formula has no 'version \"${VERSION}\"' declaration after the rewrite. The version pattern matched nothing - the formula's version line changed shape. The tap would have stayed on its previous version while this release announced ${VERSION}."

urls_total="$(count 'releases/download/')"
urls_ours="$(count "releases/download/cua-v${VERSION}/")"
if [ "$urls_total" != 2 ]; then
  # The lines are named because the count alone is opaque: the url rewrite is
  # global, so a `homepage` or `livecheck` url pointing at a release would trip
  # this with nothing to go on. Failing is still right - a third url means the
  # arm64/x64 pair is no longer what this script thinks it is - but the reader
  # should not have to go and clone the tap to find out which lines it saw.
  #
  # `|| true` on the listing and not on anything that decides: `urls_total` is 0
  # here when grep finds nothing, and an unguarded substitution would fail under
  # `set -e` and kill the script before die() ever printed. A guard on a message
  # is fine; the same guard on a check is how a gate passes silently.
  url_lines="$(grep -nF -- 'releases/download/' "$formula" | tr '\n' ' ' || true)"
  die "Unexpected url count" "expected 2 'releases/download/' urls in $formula, found ${urls_total}. Lines seen: ${url_lines:-<none>}"
fi
if [ "$urls_ours" != "$urls_total" ]; then
  die "Download urls not rewritten" "$urls_ours of $urls_total 'releases/download/' urls in $formula name cua-v${VERSION}. The rest still point at another release, so brew would fetch the wrong archive."
fi

# A checksum DECLARATION, anchored like the version one above: not the bare
# value (which a comment or a url mentioning it would satisfy), and not even the
# unanchored `sha256 "<value>"` (which `# sha256 "<value>"` would satisfy). Only
# a line whose first token is `sha256` is one brew will read.
#
# A validated checksum is 64 hex characters, so it carries no regex
# metacharacter and needs no escaping - unlike VERSION, which has dots.
#
# What this still does NOT prove is WHICH block each checksum landed in: both
# anchored, both present, count 2, and swapped is indistinguishable from
# correct. That is LAC-7748.
sha_decl() { grep -q -- "^[[:space:]]*sha256 \"$1\"" "$formula"; }

sha_decl "$ARM64_SHA" \
  || die "arm64 checksum not written" "$formula has no 'sha256 \"${ARM64_SHA}\"' declaration after the rewrite."
sha_decl "$X64_SHA" \
  || die "x64 checksum not written" "$formula has no 'sha256 \"${X64_SHA}\"' declaration after the rewrite."

after_shas="$(count 'sha256 "')"
if [ "$after_shas" != "$before_shas" ]; then
  die "Checksum lines changed" "$formula had $before_shas 'sha256 \"' lines before the rewrite and $after_shas after. The rewrite damaged the formula."
fi

# Printed so the release log carries what was published, not just that a step
# was green.
echo "Formula verified on disk before commit:"
echo "  file       $formula"
echo "  version    ${VERSION}"
echo "  urls       ${urls_ours}/${urls_total} naming cua-v${VERSION}"
echo "  arm64 sha  ${ARM64_SHA}"
echo "  x64 sha    ${X64_SHA}"
