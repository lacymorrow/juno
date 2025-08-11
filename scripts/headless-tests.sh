#!/bin/bash
set -euo pipefail

# Automated Headless Test Suite for Juno (Rust/Tauri)
# - Fast compile check
# - Unit tests
# - Headless CLI smoke tests (no network, no UI)
# - Structured logs under logs/headless/

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$ROOT_DIR/logs/headless"
mkdir -p "$LOG_DIR"

echo "=============================================="
echo "Juno Headless Automated Test Suite"
echo "=============================================="

# 1) Mandatory compilation check (short format)
echo "[1/6] Running cargo check (short format)…"
(
  cd "$ROOT_DIR"
  cargo check --manifest-path src-tauri/Cargo.toml --message-format=short 2>&1 | tee cargo-check-results.log | tee "$LOG_DIR/cargo-check-results.log" >/dev/null
)
echo "✅ cargo check completed"

# 2) Unit tests
echo "[2/6] Running cargo test (unit tests)…"
(
  cd "$ROOT_DIR/src-tauri"
  cargo test --quiet 2>&1 | tee "$LOG_DIR/cargo-test.log" >/dev/null
)
echo "✅ cargo test completed"

# 3) Build headless CLI binary
echo "[3/6] Building release binary…"
(
  cd "$ROOT_DIR"
  cargo build --release --manifest-path src-tauri/Cargo.toml 2>&1 | tee "$LOG_DIR/cargo-build.log" >/dev/null
)
BIN="$ROOT_DIR/target/release/juno"
if [[ ! -x "$BIN" ]]; then
  echo "❌ Binary not found at $BIN" | tee -a "$LOG_DIR/errors.log"
  exit 1
fi
echo "✅ build completed: $BIN"

# Helper: run a CLI test and store output + simple success detection
run_cli_test() {
  local name="$1"; shift
  local cmd=("$BIN" "$@")
  local outfile="$LOG_DIR/${name}.json"
  echo "[cli] $name → ${cmd[*]}"
  if ! "${cmd[@]}" --output json >"$outfile" 2>>"$LOG_DIR/errors.log"; then
    echo "❌ $name: command failed" | tee -a "$LOG_DIR/errors.log"
    return 1
  fi
  # Basic success check: look for \"success\": true at top level
  # Portable grep: avoid non-POSIX \s; use basic classes
  if ! grep -E '"success"[[:space:]]*:[[:space:]]*true' "$outfile" >/dev/null; then
    echo "❌ $name: success=false (see $outfile)" | tee -a "$LOG_DIR/errors.log"
    return 1
  fi
  echo "✅ $name passed"
}

# 4) Headless smoke tests (no network): tool + events components
echo "[4/6] Running headless CLI smoke tests…"
run_cli_test test_system_tool test system --component tool
run_cli_test test_system_events test system --component events

# 5) Agent status check (structured JSON validation)
echo "[5/6] Checking agent status…"
STATUS_OUT="$LOG_DIR/agent-status.json"
if "$BIN" --output json agent status >"$STATUS_OUT" 2>>"$LOG_DIR/errors.log"; then
  # Minimal field presence checks without jq dependency
  if grep -q '"agent_executing"' "$STATUS_OUT" && \
     grep -q '"provider"' "$STATUS_OUT" && \
     grep -q '"model"' "$STATUS_OUT"; then
    echo "✅ agent status ok"
  else
    echo "❌ agent status missing expected fields (see $STATUS_OUT)" | tee -a "$LOG_DIR/errors.log"
    exit 1
  fi
else
  echo "❌ agent status command failed" | tee -a "$LOG_DIR/errors.log"
  exit 1
fi

# 6) Bounded long-running/daemon-like probe
echo "[6/6] Simulating bounded daemon probe (3 status cycles)…"
for i in 1 2 3; do
  if ! "$BIN" --output json agent status >>"$LOG_DIR/daemon-probe.log" 2>>"$LOG_DIR/errors.log"; then
    echo "❌ daemon probe iteration $i failed" | tee -a "$LOG_DIR/errors.log"
    exit 1
  fi
  sleep 1
done
echo "✅ daemon probe ok"

echo ""
echo "=============================================="
echo "All headless tests passed"
echo "Logs: $LOG_DIR"
echo "=============================================="


