#!/bin/bash
set -euo pipefail

# E2E: Remote ligature shaping/fallback evidence over WebSocket (bd-2vr05.14.5)
#
# Validates deterministic fixture capture for ligature-mode workflows,
# normalization assertions, and failure-injection diagnostics with replay-grade
# JSONL artifacts.

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

REMOTE_PORT="${REMOTE_PORT:-9246}"
REMOTE_LOG_DIR="${REMOTE_LOG_DIR:-$E2E_LOG_DIR/remote_ligature_shaping}"
mkdir -p "$REMOTE_LOG_DIR"

trap remote_cleanup EXIT

echo "=== Remote Ligature Shaping E2E Test ==="

SCENARIO_PASS="$SCENARIOS_DIR/ligature_rendering.json"
SCENARIO_FAIL="$SCENARIOS_DIR/ligature_rendering_failure_injection.json"
JSONL_OUT="$REMOTE_LOG_DIR/ligature_rendering.jsonl"
TRANSCRIPT_OUT="$REMOTE_LOG_DIR/ligature_rendering.transcript"
FAIL_JSONL_OUT="$REMOTE_LOG_DIR/ligature_rendering_failure_injection.jsonl"
FAIL_TRANSCRIPT_OUT="$REMOTE_LOG_DIR/ligature_rendering_failure_injection.transcript"
REPORT_OUT="$REMOTE_LOG_DIR/ligature_rendering_report.json"

print_repro() {
    echo "Repro command:"
    echo "  E2E_DETERMINISTIC=$E2E_DETERMINISTIC E2E_SEED=$E2E_SEED REMOTE_PORT=$REMOTE_PORT bash $SCRIPT_DIR/test_remote_ligature_shaping.sh"
    echo "Artifacts:"
    echo "  Scenario(pass):   $SCENARIO_PASS"
    echo "  Scenario(fail):   $SCENARIO_FAIL"
    echo "  JSONL(pass):      $JSONL_OUT"
    echo "  JSONL(fail):      $FAIL_JSONL_OUT"
    echo "  Transcript(pass): $TRANSCRIPT_OUT"
    echo "  Transcript(fail): $FAIL_TRANSCRIPT_OUT"
    echo "  Report:           $REPORT_OUT"
    if [[ -n "${REMOTE_TELEMETRY_FILE:-}" ]]; then
        echo "  Bridge telemetry: $REMOTE_TELEMETRY_FILE"
    fi
}

python_ws_client="${E2E_PYTHON:-python3}"
if ! "$python_ws_client" "$LIB_DIR/ws_client.py" --self-test >/dev/null; then
    echo "[FAIL] ws_client self-tests failed"
    print_repro
    exit 1
fi

if ! remote_start --port "$REMOTE_PORT" --cols 110 --rows 28 --cmd /bin/sh; then
    echo "[FAIL] Unable to start bridge for ligature success scenario"
    print_repro
    exit 1
fi
if ! remote_wait_ready; then
    echo "[FAIL] Bridge did not become ready for ligature success scenario"
    print_repro
    exit 1
fi
echo "[OK] Bridge ready for success scenario on port $REMOTE_PORT"

SUCCESS_RESULT="$(remote_run_scenario "$SCENARIO_PASS" \
    --jsonl "$JSONL_OUT" \
    --transcript "$TRANSCRIPT_OUT" \
    --summary 2>&1)" || {
    echo "[FAIL] Ligature success scenario execution failed"
    echo "$SUCCESS_RESULT"
    print_repro
    exit 1
}

SUCCESS_OUTCOME="$(echo "$SUCCESS_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("outcome", "unknown"))' 2>/dev/null || echo "unknown")"
SUCCESS_FRAMES="$(echo "$SUCCESS_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("frames", 0))' 2>/dev/null || echo "0")"
SUCCESS_ASSERTIONS_TOTAL="$(echo "$SUCCESS_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("assertions_total", 0))' 2>/dev/null || echo "0")"
SUCCESS_ASSERTIONS_FAILED="$(echo "$SUCCESS_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("assertions_failed", 0))' 2>/dev/null || echo "0")"
SUCCESS_WS_IN="$(echo "$SUCCESS_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("ws_in_bytes", 0))' 2>/dev/null || echo "0")"
SUCCESS_WS_OUT="$(echo "$SUCCESS_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("ws_out_bytes", 0))' 2>/dev/null || echo "0")"

if [[ "$SUCCESS_OUTCOME" != "pass" ]]; then
    echo "[FAIL] Ligature success scenario outcome: $SUCCESS_OUTCOME"
    echo "$SUCCESS_RESULT"
    print_repro
    exit 1
fi
if [[ "${SUCCESS_FRAMES:-0}" -lt 1 ]]; then
    echo "[FAIL] Expected at least one frame for ligature success scenario, got: ${SUCCESS_FRAMES:-0}"
    print_repro
    exit 1
fi
if [[ "${SUCCESS_ASSERTIONS_FAILED:-0}" -ne 0 ]]; then
    echo "[FAIL] Ligature success assertions failed: ${SUCCESS_ASSERTIONS_FAILED}/${SUCCESS_ASSERTIONS_TOTAL}"
    echo "$SUCCESS_RESULT"
    print_repro
    exit 1
fi

python3 - "$TRANSCRIPT_OUT" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_bytes().decode("utf-8", errors="replace")
required = [
    "LIGATURE_FIXTURE_START",
    "FONT_PROFILE: JetBrainsMono,FiraCode,Iosevka",
    "LIGATURE_MODE: AUTO",
    "RAW_SEQ: office affine offline",
    "LIGATURE_CHAR: ﬁ ﬂ",
    "NFKC_EXPECTED: ﬁ ﬂ",
    "FALLBACK_POLICY: ligatures_disabled_when_unsupported",
    "LIGATURE_FIXTURE_END",
]
missing = [marker for marker in required if marker not in text]
if missing:
    raise SystemExit(f"missing transcript markers: {missing}")
PY

python3 - "$JSONL_OUT" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
if not events:
    raise SystemExit("ligature success JSONL is empty")

correlation_ids = {event.get("correlation_id", "") for event in events}
if "" in correlation_ids or not correlation_ids:
    raise SystemExit("missing correlation_id in ligature success JSONL")
if len(correlation_ids) != 1:
    raise SystemExit(f"expected exactly one correlation_id, got {sorted(correlation_ids)}")

assert_events = [event for event in events if event.get("type") == "assert"]
if len(assert_events) < 8:
    raise SystemExit(f"expected >=8 assert events, got {len(assert_events)}")
failed_asserts = [event for event in assert_events if event.get("status") == "failed"]
if failed_asserts:
    names = [event.get("assertion", "?") for event in failed_asserts]
    raise SystemExit(f"unexpected failed asserts in success scenario: {names}")

run_end = [event for event in events if event.get("type") == "run_end"]
if len(run_end) != 1:
    raise SystemExit(f"expected one run_end event, got {len(run_end)}")
if run_end[0].get("status") != "passed":
    raise SystemExit(f"run_end status is not passed: {run_end[0].get('status')}")
PY

remote_stop
if ! remote_start --port "$REMOTE_PORT" --cols 90 --rows 24 --cmd /bin/sh; then
    echo "[FAIL] Unable to start bridge for ligature failure scenario"
    print_repro
    exit 1
fi
if ! remote_wait_ready; then
    echo "[FAIL] Bridge did not become ready for ligature failure scenario"
    print_repro
    exit 1
fi
echo "[OK] Bridge ready for failure scenario on port $REMOTE_PORT"

set +e
FAILURE_RESULT="$(remote_run_scenario "$SCENARIO_FAIL" \
    --jsonl "$FAIL_JSONL_OUT" \
    --transcript "$FAIL_TRANSCRIPT_OUT" \
    --summary 2>&1)"
FAILURE_RC=$?
set -e

FAILURE_OUTCOME="$(echo "$FAILURE_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("outcome", "unknown"))' 2>/dev/null || echo "unknown")"
FAILURE_ASSERTIONS_FAILED="$(echo "$FAILURE_RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("assertions_failed", -1))' 2>/dev/null || echo "-1")"

if [[ "$FAILURE_RC" -eq 0 ]]; then
    echo "[FAIL] Ligature failure-injection scenario unexpectedly succeeded"
    echo "$FAILURE_RESULT"
    print_repro
    exit 1
fi
if [[ "$FAILURE_OUTCOME" != "fail" ]]; then
    echo "[FAIL] Ligature failure-injection outcome should be fail, got: $FAILURE_OUTCOME"
    echo "$FAILURE_RESULT"
    print_repro
    exit 1
fi

python3 - "$FAIL_JSONL_OUT" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
if not events:
    raise SystemExit("ligature failure-injection JSONL is empty")

failed_asserts = [
    event for event in events
    if event.get("type") == "assert" and event.get("status") == "failed"
]
if not failed_asserts:
    raise SystemExit("expected at least one failed assert in ligature failure JSONL")
if not any(event.get("assertion") == "ligature_failure_intentional_missing_marker" for event in failed_asserts):
    raise SystemExit("intentional missing marker assertion was not recorded as failed")

error_events = [event for event in events if event.get("type") == "error"]
if not error_events:
    raise SystemExit("expected error events in ligature failure JSONL")
if not any("ligature_failure_intentional_missing_marker" in event.get("message", "") for event in error_events):
    raise SystemExit("error events do not include ligature assertion identifier context")

run_end = [event for event in events if event.get("type") == "run_end"]
if len(run_end) != 1:
    raise SystemExit(f"expected one run_end event, got {len(run_end)}")
if run_end[0].get("status") != "failed":
    raise SystemExit(f"run_end status is not failed: {run_end[0].get('status')}")
PY

python3 - "$SUCCESS_RESULT" "$FAILURE_RESULT" "$JSONL_OUT" "$FAIL_JSONL_OUT" "$REPORT_OUT" "$SCRIPT_DIR" "$REMOTE_PORT" "$E2E_SEED" "$E2E_DETERMINISTIC" <<'PY'
import json
import sys
from pathlib import Path

success = json.loads(sys.argv[1])
failure = json.loads(sys.argv[2])
jsonl_ok = sys.argv[3]
jsonl_fail = sys.argv[4]
report = Path(sys.argv[5])
script_dir = sys.argv[6]
remote_port = sys.argv[7]
seed = sys.argv[8]
deterministic = sys.argv[9]

payload = {
    "suite": "remote_ligature_shaping",
    "status": "pass",
    "success_scenario": {
        "name": success.get("scenario"),
        "outcome": success.get("outcome"),
        "frames": success.get("frames"),
        "ws_in_bytes": success.get("ws_in_bytes"),
        "ws_out_bytes": success.get("ws_out_bytes"),
        "assertions_total": success.get("assertions_total"),
        "assertions_failed": success.get("assertions_failed"),
        "jsonl": jsonl_ok,
    },
    "failure_injection": {
        "name": failure.get("scenario"),
        "outcome": failure.get("outcome"),
        "assertions_total": failure.get("assertions_total"),
        "assertions_failed": failure.get("assertions_failed"),
        "errors": failure.get("errors", []),
        "jsonl": jsonl_fail,
    },
    "repro": (
        f"E2E_DETERMINISTIC={deterministic} E2E_SEED={seed} REMOTE_PORT={remote_port} "
        f"bash {script_dir}/test_remote_ligature_shaping.sh"
    ),
}

report.write_text(json.dumps(payload, indent=2), encoding="utf-8")
PY

echo "[PASS] Remote ligature shaping"
echo "  Success outcome: $SUCCESS_OUTCOME"
echo "  Success frames:  $SUCCESS_FRAMES"
echo "  Success assertions: ${SUCCESS_ASSERTIONS_TOTAL} total, ${SUCCESS_ASSERTIONS_FAILED} failed"
echo "  Success WS in/out: ${SUCCESS_WS_IN}/${SUCCESS_WS_OUT}"
echo "  Failure injected assertions failed: $FAILURE_ASSERTIONS_FAILED"
echo "  Report: $REPORT_OUT"
