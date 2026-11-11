#!/usr/bin/env bash
#
# Behaviour tests for scripts/update-homebrew-formula.sh.
#
# No git, no network, no release: the script rewrites one file, so every case
# here is a fixture formula plus an expected exit code. The baseline fixture is
# a verbatim copy of lacymorrow/homebrew-tap Formula/juno-cua.rb as it stood at
# v0.8.277 (read 2026-11-11), so the happy path is tested against the real thing
# rather than a stub that models fewer dimensions than the bug lives in.
#
# The suite exists because the bug it covers (LAC-7401) was invisible from the
# outside: a rewrite that matched nothing left the step GREEN, and PR #960's
# `remote HEAD == local HEAD` assertion green with it, because nothing was
# committed so local HEAD never moved. Measured on a scratch bare origin before
# the fix: `No changes`, `Everything up-to-date`, assertion PASSED, STEP EXIT=0,
# formula still at version "0.4.11" while the release announced 0.4.99. Each
# "rewrite matched nothing" case below is one way to reach that state.

set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/../update-homebrew-formula.sh"

pass=0
fail=0

ARM=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
X64=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
OLD_ARM=ba6e966f41d7579f37fe0748996d899b51a74f7386acf9d6c809d68d962e46f6
OLD_X64=b29756d5f462258258a61c6516b3165d973598ea6b4ef27c61716434caffedbe

# --- harness ---------------------------------------------------------------

setup() {
  work="$(mktemp -d)"
  formula="$work/juno-cua.rb"
  baseline >"$formula"
}

teardown() { rm -rf "$work"; }

# The live tap formula at v0.8.277.
baseline() {
  cat <<'FORMULA'
# typed: false
# frozen_string_literal: true

class JunoCua < Formula
  desc "Headless computer use agent — screenshot, click, type, scroll from the CLI"
  homepage "https://github.com/lacymorrow/juno"
  version "0.8.277"

  depends_on :macos

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/lacymorrow/juno/releases/download/cua-v0.8.277/juno-cua-darwin-arm64.tar.gz"
      sha256 "ba6e966f41d7579f37fe0748996d899b51a74f7386acf9d6c809d68d962e46f6"

      def install
        bin.install "juno-cua"
      end
    end
    if Hardware::CPU.intel?
      url "https://github.com/lacymorrow/juno/releases/download/cua-v0.8.277/juno-cua-darwin-x64.tar.gz"
      sha256 "b29756d5f462258258a61c6516b3165d973598ea6b4ef27c61716434caffedbe"

      def install
        bin.install "juno-cua"
      end
    end
  end

  test do
    assert_match "Computer Use Agent", shell_output("#{bin}/juno-cua capabilities")
  end
end
FORMULA
}

# run -- runs the script against $formula with VERSION/ARM64_SHA/X64_SHA from
# the environment, prints the exit code, keeps output in $work/out.
#
# `${VAR-default}` and not `${VAR:-default}`: an explicitly EMPTY value is a
# case under test (an empty checksum is the fail-open one), and `:-` would
# replace it with the default and quietly test the happy path instead.
run() {
  VERSION="${VERSION-0.8.278}" ARM64_SHA="${ARM64_SHA-$ARM}" X64_SHA="${X64_SHA-$X64}" \
    bash "$script" "$formula" >"$work/out" 2>&1
  echo $?
}

ok()  { pass=$((pass + 1)); printf '  ok   %s\n' "$1"; }
bad() { fail=$((fail + 1)); printf '  FAIL %s\n' "$1"; sed 's/^/       /' "$work/out" | tail -8; }

check() { if [ "$2" -eq 0 ]; then ok "$1"; else bad "$1"; fi; }

# `--` is mandatory: a checksum of all-a's is not an option, but a pattern read
# as one exits grep 2, and absence is checked by exit code rather than by
# negation so a broken grep fails the assertion instead of passing it.
in_formula() { grep -qF -- "$1" "$formula"; }
not_in_formula() {
  local rc
  grep -qF -- "$1" "$formula"
  rc=$?
  case $rc in
    0) return 1 ;;
    1) return 0 ;;
    *) printf '       grep failed (rc=%s) checking absence of: %s\n' "$rc" "$1" >&2; return 1 ;;
  esac
}
said() { grep -qF -- "$1" "$work/out"; }

# The arm block's checksum, read positionally the way brew would read it.
sha_in_arm_block()   { awk '/Hardware::CPU.arm\?/,/^    end$/' "$formula" | grep -qF -- "$1"; }
sha_in_intel_block() { awk '/Hardware::CPU.intel\?/,/^    end$/' "$formula" | grep -qF -- "$1"; }

# --- cases -----------------------------------------------------------------

echo "update-homebrew-formula.sh"

# 1. The happy path, against the real formula.
setup
rc=$(run)
[ "$rc" = 0 ] && in_formula 'version "0.8.278"' \
  && in_formula "releases/download/cua-v0.8.278/juno-cua-darwin-arm64.tar.gz" \
  && in_formula "releases/download/cua-v0.8.278/juno-cua-darwin-x64.tar.gz" \
  && not_in_formula "cua-v0.8.277" \
  && sha_in_arm_block "$ARM" && sha_in_intel_block "$X64" \
  && not_in_formula "$OLD_ARM" && not_in_formula "$OLD_X64"
check "live formula: version, both urls and both checksums rewritten in place" $?
teardown

# 2. THE REGRESSION. The version line changed shape, so the version rewrite
#    matches nothing. Before LAC-7401 this was a green step and a stale tap.
setup
sed 's/^  version "0.8.277"$/  VERSION = "0.8.277"/' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=Version not written" && not_in_formula 'version "0.8.278"'
check "version pattern matches nothing: exits 1 instead of leaving a stale version" $?
teardown

# 3. The download urls changed shape, so the url rewrite matches nothing. The
#    version still lands, so assertion 1 alone would not catch this.
setup
sed 's|releases/download/cua-v0.8.277/|archive/refs/tags/cua-v0.8.277/|g' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=Unexpected url count" && in_formula 'version "0.8.278"'
check "url pattern matches nothing: exits 1 even though the version rewrite landed" $?
teardown

# 4. One url the pattern cannot reach. `releases/download/[^/]*/` needs a slash
#    after the tag, so a url ending at the tag is skipped while its sibling is
#    rewritten - two urls present, one on the target release. The count check
#    alone passes here; only comparing the two counts catches it.
setup
sed 's|/releases/download/cua-v0.8.277/juno-cua-darwin-x64.tar.gz|/releases/download/cua-v0.8.277|' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=Download urls not rewritten" && said "1 of 2" \
  && in_formula "releases/download/cua-v0.8.277\""
check "one url the pattern cannot reach: exits 1 (1 of 2 naming the target)" $?
teardown

# 5. The checksum lines changed shape, so the checksum pass matches nothing.
#    The version and urls still land, so only a checksum assertion catches it.
setup
sed 's/^      sha256 /      sha256sum /' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=Unexpected formula shape" && said "found 0"
check "checksum pattern matches nothing: exits 1 before anything is committed" $?
teardown

# 6. A third platform block. The positional pass only knows two checksums, so
#    the third keeps a stale one - red, not a quietly wrong formula.
setup
awk '
  /Hardware::CPU.intel\?/ && !done {
    print "    if Hardware::CPU.something_else?"
    print "      url \"https://github.com/lacymorrow/juno/releases/download/cua-v0.8.277/juno-cua-darwin-universal.tar.gz\""
    print "      sha256 \"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\""
    print "    end"
    done = 1
  }
  { print }
' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=Unexpected formula shape" && said "found 3" && said "LAC-7748"
check "three checksum lines: exits 1 and names the follow-up, no partial rewrite shipped" $?
teardown

# 7. THE CASE THAT MUST STAY GREEN. Re-running a release whose formula is
#    already published: nothing to rewrite, every assertion passes, and the
#    caller's `git commit || echo "No changes"` is then safe to keep.
setup
rc=$(VERSION=0.8.277 ARM64_SHA="$OLD_ARM" X64_SHA="$OLD_X64" run)
[ "$rc" = 0 ] && in_formula 'version "0.8.277"' && in_formula "$OLD_ARM" \
  && [ -z "$(diff <(baseline) "$formula")" ]
check "already-published version: exits 0 and leaves the file byte-identical" $?
teardown

# 8. An empty checksum is rejected before it is used as evidence. Without this
#    the formula gets sha256 "" and `grep -q "$ARM64_SHA"` is `grep -q ""`,
#    which matches every file - a check that reads as coverage and is not.
setup
rc=$(ARM64_SHA="" run)
[ "$rc" = 1 ] && said "::error title=ARM64_SHA unset" && not_in_formula 'sha256 ""' \
  && in_formula 'version "0.8.277"'
check "empty ARM64_SHA: exits 1 before touching the file (no fail-open grep)" $?
teardown

# 9. A truncated checksum is rejected too: it would be a real substring match
#    against nothing, and brew would reject the download.
setup
rc=$(X64_SHA="${X64:0:40}" run)
[ "$rc" = 1 ] && said "::error title=Bad X64_SHA" && said "got 40 chars"
check "truncated X64_SHA: exits 1 naming the length" $?
teardown

# 10. Uppercase hex is rejected rather than written: brew wants lowercase and a
#     case mismatch fails the install, not the release.
setup
rc=$(ARM64_SHA="$(printf '%s' "$ARM" | tr 'a-f' 'A-F')" run)
[ "$rc" = 1 ] && said "::error title=Bad ARM64_SHA"
check "uppercase ARM64_SHA: exits 1" $?
teardown

# 11. The same checksum for both archives cannot be right, and would otherwise
#     satisfy both presence assertions.
setup
rc=$(X64_SHA="$ARM" run)
[ "$rc" = 1 ] && said "::error title=Identical checksums"
check "ARM64_SHA == X64_SHA: exits 1 rather than writing one checksum twice" $?
teardown

# 12. A cua-v-prefixed VERSION is rejected: it would write
#     `releases/download/cua-vcua-v0.8.278/` and a version line to match.
setup
rc=$(VERSION=cua-v0.8.278 run)
[ "$rc" = 1 ] && said "::error title=Bad VERSION" && not_in_formula "cua-vcua-v"
check "VERSION still carrying the cua-v prefix: exits 1" $?
teardown

# 13. A line that merely mentions `version "..."` is not the version
#     declaration, and the anchored rewrite must leave it alone. The
#     unanchored `s/version ".*"/.../` this replaced rewrote the FIRST such
#     line and its greedy `.*` ate the rest of it, which on this fixture means
#     the real version line is never reached (PR #976 review, 2).
setup
awk '/^  version /{print "  # pin the version \"by hand\" if the tap moves"} {print}' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 0 ] && in_formula 'version "0.8.278"' \
  && in_formula '# pin the version "by hand" if the tap moves'
check "a comment mentioning version \"...\": left alone, real version still rewritten" $?
teardown

# 14. The checksum has to land on a sha256 LINE, not merely appear somewhere in
#     the file (PR #976 review, 1).
#
#     Reaching that assertion needs a fixture that gets PAST the precondition,
#     which is what the first attempt at this case got wrong: renaming `sha256`
#     left 0 checksum lines, so the script died on the count and the case passed
#     without ever running the check it was named after - green against the
#     bare-substring version too, which is how the mutant caught it.
#
#     So: drop the closing quote on the first checksum line. `sha256 "` still
#     matches, so the count is still 2 and the precondition passes, but the
#     rewrite's `sha256 "[^"]*"` finds nothing to replace there and arm64's
#     checksum is never written. A comment carrying that same value - a note of
#     a previous release's checksum - is then enough to satisfy a check that
#     only asks whether the string is present.
setup
awk -v arm="$ARM" '
  /Hardware::CPU.arm\?/ { print; print "      # previous arm64 checksum was " arm; next }
  /sha256 "/ && !seen   { sub(/"$/, ""); seen = 1 }
  { print }
' "$formula" >"$work/t" && mv "$work/t" "$formula"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=arm64 checksum not written" \
  && in_formula "$ARM" && not_in_formula "sha256 \"$ARM\""
check "checksum present only in a comment: exits 1 (presence is not evidence)" $?
teardown

# 15. A missing formula is an error, not a no-op. The tap layout changing is
#     exactly the class of thing that made the rewrite match nothing.
setup
formula="$work/does-not-exist.rb"
rc=$(run)
[ "$rc" = 1 ] && said "::error title=Formula missing"
check "formula path does not exist: exits 1" $?
teardown

# 16. The script is given its path, not left to guess one.
setup
VERSION=0.8.278 ARM64_SHA="$ARM" X64_SHA="$X64" bash "$script" >"$work/out" 2>&1
rc=$?
[ "$rc" = 1 ] && said "usage: update-homebrew-formula.sh"
check "no formula argument: exits 1 with usage" $?
teardown

printf '\n%s passed, %s failed\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
