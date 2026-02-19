#!/bin/bash
set -euo pipefail

# E2E: Remote resize storm over WebSocket.
#
# Drives mixed resize + synthetic signal markers (DPR/zoom/font/same-size)
# and validates deterministic JSONL evidence for geometry/frame invariants.

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
SCENARIO="$SCENARIOS_DIR/resize_storm.json"
JSONL_OUT="$REMOTE_LOG_DIR/resize_storm.jsonl"
TRANSCRIPT_OUT="$REMOTE_LOG_DIR/resize_storm.transcript"
REPORT_OUT="$REMOTE_LOG_DIR/resize_storm_report.json"

print_repro() {
    echo "Repro command:"
    echo "  E2E_DETERMINISTIC=$E2E_DETERMINISTIC E2E_SEED=$E2E_SEED REMOTE_PORT=$REMOTE_PORT bash $SCRIPT_DIR/test_remote_resize_storm.sh"
    echo "Artifacts:"
    echo "  Scenario:   $SCENARIO"
    echo "  JSONL:      $JSONL_OUT"
    echo "  Transcript: $TRANSCRIPT_OUT"
    echo "  Report:     $REPORT_OUT"
    if [[ -n "${REMOTE_TELEMETRY_FILE:-}" ]]; then
        echo "  Telemetry:  $REMOTE_TELEMETRY_FILE"
    fi
}

python_ws_client="${E2E_PYTHON:-python3}"
if ! "$python_ws_client" "$LIB_DIR/ws_client.py" --self-test >/dev/null; then
    echo "[FAIL] ws_client self-tests failed"
    print_repro
    exit 1
fi

if ! remote_start --port "$REMOTE_PORT" --cols 120 --rows 40 --cmd /bin/sh; then
    echo "[FAIL] Unable to start bridge for resize-storm scenario"
    print_repro
    exit 1
fi
if ! remote_wait_ready; then
    echo "[FAIL] Bridge did not become ready for resize-storm scenario"
    print_repro
    exit 1
fi
echo "[OK] Bridge ready on port $REMOTE_PORT (PID=$REMOTE_BRIDGE_PID)"

RESULT="$(remote_run_scenario "$SCENARIO" \
    --jsonl "$JSONL_OUT" \
    --transcript "$TRANSCRIPT_OUT" \
    --summary 2>&1)" || {
    echo "[FAIL] Resize-storm scenario execution failed"
    echo "$RESULT"
    print_repro
    exit 1
}

OUTCOME="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin)["outcome"])' 2>/dev/null || echo "unknown")"
FRAMES="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("frames", 0))' 2>/dev/null || echo "0")"
ASSERTIONS_TOTAL="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("assertions_total", 0))' 2>/dev/null || echo "0")"
ASSERTIONS_FAILED="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("assertions_failed", 0))' 2>/dev/null || echo "0")"
WS_IN="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("ws_in_bytes", 0))' 2>/dev/null || echo "0")"
WS_OUT="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("ws_out_bytes", 0))' 2>/dev/null || echo "0")"
CHECKSUM="$(echo "$RESULT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("checksum_chain", ""))' 2>/dev/null || echo "")"

if [[ "$OUTCOME" != "pass" ]]; then
    echo "[FAIL] Resize-storm scenario outcome: $OUTCOME"
    echo "$RESULT"
    print_repro
    exit 1
fi
if [[ "${FRAMES:-0}" -lt 1 ]]; then
    echo "[FAIL] Expected at least one frame from resize-storm scenario, got: ${FRAMES:-0}"
    print_repro
    exit 1
fi
if [[ "${ASSERTIONS_FAILED:-0}" -ne 0 ]]; then
    echo "[FAIL] Scenario assertions failed: ${ASSERTIONS_FAILED}/${ASSERTIONS_TOTAL}"
    echo "$RESULT"
    print_repro
    exit 1
fi

python3 - "$TRANSCRIPT_OUT" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_bytes().decode("utf-8", errors="replace")
required = [
    "RESIZE_STORM_START",
    "SIGNAL:DPR_SHIFT:1.00->1.25",
    "SIGNAL:ZOOM_CHANGE:1.00->1.10",
    "SIGNAL:FONT_RELOAD:JetBrainsMono->Iosevka",
    "GEOM_PROBE:80x24|signal=dpr_shift",
    "GEOM_PROBE:120x40|signal=same_size",
    "GEOM_PROBE:1x1|signal=extreme_min",
    "GEOM_PROBE:300x100|signal=extreme_max",
    "GEOM_PROBE:80x24|signal=final",
    "RESIZE_STORM_END",
]
missing = [marker for marker in required if marker not in text]
if missing:
    raise SystemExit(f"missing transcript markers: {missing}")
PY

python3 - "$JSONL_OUT" <<'PY'
import json
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
if not events:
    raise SystemExit("resize-storm JSONL is empty")

run_starts = [event for event in events if event.get("type") == "run_start"]
if len(run_starts) != 1:
    raise SystemExit(f"expected one run_start event, got {len(run_starts)}")
run_ends = [event for event in events if event.get("type") == "run_end"]
if len(run_ends) != 1:
    raise SystemExit(f"expected one run_end event, got {len(run_ends)}")
if run_ends[0].get("status") != "passed":
    raise SystemExit(f"run_end status is not passed: {run_ends[0].get('status')}")

correlation_ids = {event.get("correlation_id", "") for event in events}
if "" in correlation_ids or not correlation_ids:
    raise SystemExit("missing correlation_id in resize-storm JSONL")
if len(correlation_ids) != 1:
    raise SystemExit(f"expected exactly one correlation_id, got {sorted(correlation_ids)}")

resize_inputs = [
    event for event in events
    if event.get("type") == "input" and event.get("input_type") == "resize"
]
if len(resize_inputs) < 10:
    raise SystemExit(f"expected at least 10 resize input events, got {len(resize_inputs)}")

expected_sizes = {
    (80, 24),
    (160, 50),
    (40, 10),
    (200, 60),
    (120, 40),
    (1, 1),
    (300, 100),
}
observed_input_sizes = {
    (int(event.get("cols", 0)), int(event.get("rows", 0)))
    for event in resize_inputs
}
missing_sizes = sorted(expected_sizes - observed_input_sizes)
if missing_sizes:
    raise SystemExit(f"missing expected resize input sizes: {missing_sizes}")

for event in resize_inputs:
    cols = event.get("cols")
    rows = event.get("rows")
    if not isinstance(cols, int) or not isinstance(rows, int) or cols <= 0 or rows <= 0:
        raise SystemExit(f"invalid resize geometry in input event: {event}")
    hash_key = event.get("hash_key")
    if not isinstance(hash_key, str) or f"{cols}x{rows}" not in hash_key:
        raise SystemExit(f"resize input event hash_key does not encode geometry: {event}")

frame_events = [event for event in events if event.get("type") == "frame"]
if len(frame_events) < 4:
    raise SystemExit(f"expected at least 4 frame events, got {len(frame_events)}")
for event in frame_events:
    frame_hash = event.get("frame_hash")
    if not isinstance(frame_hash, str) or not frame_hash.startswith("sha256:"):
        raise SystemExit(f"frame event missing sha256 frame_hash: {event}")
    hash_key = event.get("hash_key")
    if not isinstance(hash_key, str) or not hash_key:
        raise SystemExit(f"frame event missing hash_key: {event}")
    cols = event.get("cols")
    rows = event.get("rows")
    if not isinstance(cols, int) or not isinstance(rows, int) or cols <= 0 or rows <= 0:
        raise SystemExit(f"frame event has invalid geometry: {event}")

deterministic_ts = re.compile(r"^T\d{6}$")
timestamps = [event.get("timestamp", "") for event in events]
if any(not isinstance(ts, str) or not deterministic_ts.match(ts) for ts in timestamps):
    raise SystemExit("timestamps are not deterministic Txxxxxx format")
PY

python3 - "$RESULT" "$JSONL_OUT" "$TRANSCRIPT_OUT" "$REPORT_OUT" "$SCRIPT_DIR" "$REMOTE_PORT" "$E2E_SEED" "$E2E_DETERMINISTIC" <<'PY'
import json
import sys
from pathlib import Path

result = json.loads(sys.argv[1])
jsonl_path = sys.argv[2]
transcript_path = sys.argv[3]
report_path = Path(sys.argv[4])
script_dir = sys.argv[5]
remote_port = sys.argv[6]
seed = sys.argv[7]
deterministic = sys.argv[8]

report = {
    "suite": "remote_resize_storm",
    "status": "pass",
    "scenario": result.get("scenario"),
    "outcome": result.get("outcome"),
    "frames": result.get("frames"),
    "ws_in_bytes": result.get("ws_in_bytes"),
    "ws_out_bytes": result.get("ws_out_bytes"),
    "checksum_chain": result.get("checksum_chain"),
    "assertions_total": result.get("assertions_total"),
    "assertions_failed": result.get("assertions_failed"),
    "artifacts": {
        "jsonl": jsonl_path,
        "transcript": transcript_path,
    },
    "repro_command": (
        f"E2E_DETERMINISTIC={deterministic} E2E_SEED={seed} REMOTE_PORT={remote_port} "
        f"bash {script_dir}/test_remote_resize_storm.sh"
    ),
}
report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
PY

echo "[PASS] Remote resize storm"
echo "  Outcome:    $OUTCOME"
echo "  Frames:     $FRAMES"
echo "  WS in/out:  ${WS_IN}/${WS_OUT}"
echo "  Assertions: ${ASSERTIONS_TOTAL} total, ${ASSERTIONS_FAILED} failed"
echo "  Checksum:   $CHECKSUM"
echo "  Report:     $REPORT_OUT"
