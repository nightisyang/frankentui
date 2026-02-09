#!/bin/bash
set -euo pipefail

# E2E: Remote OSC8 hyperlink rendering over WebSocket (bd-lff4p.10.5)
#
# Tests OSC 8 hyperlink open/close, id params, adjacent links,
# and long URLs through the WebSocket PTY bridge.

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
export E2E_SEED="${E2E_SEED:-0}"

REMOTE_PORT="${REMOTE_PORT:-9243}"
REMOTE_LOG_DIR="${REMOTE_LOG_DIR:-$E2E_LOG_DIR/remote_osc8}"
mkdir -p "$REMOTE_LOG_DIR"

trap remote_cleanup EXIT

echo "=== Remote OSC8 Links E2E Test ==="

remote_start --port "$REMOTE_PORT" --cols 80 --rows 24 --cmd /bin/sh
remote_wait_ready
echo "[OK] Bridge ready on port $REMOTE_PORT"

JSONL_OUT="$REMOTE_LOG_DIR/osc8_links.jsonl"
TRANSCRIPT_OUT="$REMOTE_LOG_DIR/osc8_links.transcript"

RESULT="$(remote_run_scenario "$SCENARIOS_DIR/osc8_links.json" \
    --jsonl "$JSONL_OUT" \
    --transcript "$TRANSCRIPT_OUT" \
    --summary 2>&1)" || {
    echo "[FAIL] Scenario failed"
    echo "$RESULT"
    exit 1
}

OUTCOME="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin)["outcome"])' 2>/dev/null || echo "unknown")"

if [[ "$OUTCOME" == "pass" ]]; then
    echo "[PASS] Remote OSC8 links"
    echo "$RESULT" | python3 -c "
import json, sys
r = json.load(sys.stdin)
print(f'  WS out: {r[\"ws_out_bytes\"]} bytes | Frames: {r[\"frames\"]}')
" 2>/dev/null || true
else
    echo "[FAIL] Remote OSC8 links: $OUTCOME"
    exit 1
fi
