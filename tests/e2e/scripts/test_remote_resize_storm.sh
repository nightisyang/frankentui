#!/bin/bash
set -euo pipefail

# E2E: Remote resize storm over WebSocket (bd-lff4p.10.5)
#
# Connects to the frankenterm_ws_bridge and fires rapid resize events
# to verify geometry stability and JSONL telemetry correctness.
#
# JSONL events: env, run_start, input, resize, frame, run_end
#
# Usage:
#   ./test_remote_resize_storm.sh
#   REMOTE_PORT=9240 ./test_remote_resize_storm.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"
SCENARIOS_DIR="$SCRIPT_DIR/../scenarios/remote"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/remote.sh"

export E2E_DETERMINISTIC="${E2E_DETERMINISTIC:-1}"
export E2E_TIME_STEP_MS="${E2E_TIME_STEP_MS:-100}"
export E2E_SEED="${E2E_SEED:-0}"

REMOTE_PORT="${REMOTE_PORT:-9240}"
REMOTE_LOG_DIR="${REMOTE_LOG_DIR:-$E2E_LOG_DIR/remote_resize_storm}"
mkdir -p "$REMOTE_LOG_DIR"

trap remote_cleanup EXIT

echo "=== Remote Resize Storm E2E Test ==="
echo "Port: $REMOTE_PORT | Seed: $E2E_SEED | Deterministic: $E2E_DETERMINISTIC"

# Start bridge.
remote_start --port "$REMOTE_PORT" --cols 120 --rows 40 --cmd /bin/sh
remote_wait_ready
echo "[OK] Bridge ready on port $REMOTE_PORT (PID=$REMOTE_BRIDGE_PID)"

# Run scenario.
JSONL_OUT="$REMOTE_LOG_DIR/resize_storm.jsonl"
TRANSCRIPT_OUT="$REMOTE_LOG_DIR/resize_storm.transcript"

RESULT="$(remote_run_scenario "$SCENARIOS_DIR/resize_storm.json" \
    --jsonl "$JSONL_OUT" \
    --transcript "$TRANSCRIPT_OUT" \
    --summary 2>&1)" || {
    echo "[FAIL] Scenario failed"
    echo "$RESULT"
    exit 1
}

echo "$RESULT" | python3 -c "
import json, sys
r = json.load(sys.stdin)
print(f'  Outcome:   {r[\"outcome\"]}')
print(f'  WS in:     {r[\"ws_in_bytes\"]} bytes')
print(f'  WS out:    {r[\"ws_out_bytes\"]} bytes')
print(f'  Frames:    {r[\"frames\"]}')
print(f'  Checksum:  {r[\"checksum_chain\"]}')
" 2>/dev/null || echo "$RESULT"

OUTCOME="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin)["outcome"])' 2>/dev/null || echo "unknown")"

if [[ "$OUTCOME" == "pass" ]]; then
    echo "[PASS] Remote resize storm"
else
    echo "[FAIL] Remote resize storm: $OUTCOME"
    exit 1
fi
