#!/bin/bash
set -euo pipefail

# E2E: Typography + rescale browser-oriented suite (bd-2vr05.15.5.3).
#
# Orchestrates deterministic remote browser-facing scenarios:
# - rescale storms (with DPR/zoom/font/same-size/extreme geometry probes)
# - mixed-script unicode typography
# - overload stress (long scrollback)
#
# Produces an aggregated JSONL/JSON evidence bundle with:
# - run_id + seed
# - capability context (terminal + browser tags)
# - geometry + frame hash evidence
# - derived tier transition traces from frame present-time bands
# - failure-context excerpts (assert/error events)
# - one-command reproduction hints

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export E2E_DETERMINISTIC="${E2E_DETERMINISTIC:-1}"
export E2E_TIME_STEP_MS="${E2E_TIME_STEP_MS:-100}"
export E2E_SEED="${E2E_SEED:-0}"

if [[ -z "${E2E_LOG_DIR:-}" ]]; then
    E2E_LOG_DIR="/tmp/ftui_e2e_typography_rescale_$(date +%Y%m%d_%H%M%S)"
fi

SUITE_LOG_DIR="${E2E_TYPOGRAPHY_RESCALE_LOG_DIR:-$E2E_LOG_DIR/remote_typography_rescale}"
SUITE_JSONL="${E2E_TYPOGRAPHY_RESCALE_JSONL:-$SUITE_LOG_DIR/typography_rescale_e2e.jsonl}"
SUITE_REPORT="${E2E_TYPOGRAPHY_RESCALE_REPORT:-$SUITE_LOG_DIR/typography_rescale_e2e_report.json}"

RESIZE_LOG_DIR="$SUITE_LOG_DIR/resize_storm"
UNICODE_LOG_DIR="$SUITE_LOG_DIR/unicode"
OVERLOAD_LOG_DIR="$SUITE_LOG_DIR/overload"

RUN_CROSS_BROWSER="${E2E_TYPOGRAPHY_RESCALE_CROSS_BROWSER:-0}"
if [[ "$RUN_CROSS_BROWSER" == "1" ]]; then
    CROSS_LOG_DIR="$SUITE_LOG_DIR/resize_cross_browser"
    CROSS_REPORT="$CROSS_LOG_DIR/resize_storm_cross_browser_report.json"
else
    CROSS_LOG_DIR=""
    CROSS_REPORT=""
fi

mkdir -p "$SUITE_LOG_DIR" "$RESIZE_LOG_DIR" "$UNICODE_LOG_DIR" "$OVERLOAD_LOG_DIR"

print_suite_repro() {
    echo "Repro commands:"
    echo "  E2E_DETERMINISTIC=$E2E_DETERMINISTIC E2E_TIME_STEP_MS=$E2E_TIME_STEP_MS E2E_SEED=$E2E_SEED REMOTE_PORT=9440 REMOTE_LOG_DIR=$RESIZE_LOG_DIR bash $SCRIPT_DIR/test_remote_resize_storm.sh"
    echo "  E2E_DETERMINISTIC=$E2E_DETERMINISTIC E2E_SEED=$E2E_SEED REMOTE_PORT=9442 REMOTE_LOG_DIR=$UNICODE_LOG_DIR bash $SCRIPT_DIR/test_remote_unicode.sh"
    echo "  E2E_DETERMINISTIC=$E2E_DETERMINISTIC E2E_SEED=$E2E_SEED REMOTE_PORT=9444 REMOTE_LOG_DIR=$OVERLOAD_LOG_DIR bash $SCRIPT_DIR/test_remote_scrollback.sh"
    if [[ "$RUN_CROSS_BROWSER" == "1" ]]; then
        echo "  E2E_DETERMINISTIC=$E2E_DETERMINISTIC E2E_SEED=$E2E_SEED E2E_LOG_DIR=$CROSS_LOG_DIR E2E_DIFF_LOG_DIR=$CROSS_LOG_DIR bash $SCRIPT_DIR/test_remote_resize_storm_cross_browser_diff.sh"
    fi
    echo "Artifacts:"
    echo "  Suite JSONL:  $SUITE_JSONL"
    echo "  Suite report: $SUITE_REPORT"
}

run_case() {
    local name="$1"
    local script_name="$2"
    local port="$3"
    local log_dir="$4"

    echo "--- Running case: $name ---"
    if ! REMOTE_PORT="$port" \
        REMOTE_LOG_DIR="$log_dir" \
        E2E_DETERMINISTIC="$E2E_DETERMINISTIC" \
        E2E_TIME_STEP_MS="$E2E_TIME_STEP_MS" \
        E2E_SEED="$E2E_SEED" \
        bash "$SCRIPT_DIR/$script_name"; then
        echo "[FAIL] Case failed: $name"
        print_suite_repro
        return 1
    fi
    echo "[OK] Case passed: $name"
}

echo "=== Remote Typography + Rescale E2E Suite ==="
echo "Seed: $E2E_SEED"
echo "Deterministic: $E2E_DETERMINISTIC"
echo "Suite log dir: $SUITE_LOG_DIR"

run_case "resize_storm" "test_remote_resize_storm.sh" "9440" "$RESIZE_LOG_DIR"
run_case "unicode_mixed_script" "test_remote_unicode.sh" "9442" "$UNICODE_LOG_DIR"
run_case "overload_scrollback" "test_remote_scrollback.sh" "9444" "$OVERLOAD_LOG_DIR"

if [[ "$RUN_CROSS_BROWSER" == "1" ]]; then
    mkdir -p "$CROSS_LOG_DIR"
    echo "--- Running case: resize_cross_browser_diff ---"
    if ! E2E_LOG_DIR="$CROSS_LOG_DIR" \
        E2E_DIFF_LOG_DIR="$CROSS_LOG_DIR" \
        E2E_DETERMINISTIC="$E2E_DETERMINISTIC" \
        E2E_TIME_STEP_MS="$E2E_TIME_STEP_MS" \
        E2E_SEED="$E2E_SEED" \
        bash "$SCRIPT_DIR/test_remote_resize_storm_cross_browser_diff.sh"; then
        echo "[FAIL] Case failed: resize_cross_browser_diff"
        print_suite_repro
        exit 1
    fi
    echo "[OK] Case passed: resize_cross_browser_diff"
fi

if ! python3 - \
    "$SUITE_JSONL" \
    "$SUITE_REPORT" \
    "$RESIZE_LOG_DIR/resize_storm.jsonl" \
    "$UNICODE_LOG_DIR/unicode_rendering.jsonl" \
    "$UNICODE_LOG_DIR/unicode_rendering_failure_injection.jsonl" \
    "$OVERLOAD_LOG_DIR/long_scrollback.jsonl" \
    "$E2E_SEED" \
    "$E2E_TIME_STEP_MS" \
    "$RUN_CROSS_BROWSER" \
    "$CROSS_REPORT" <<'PY'
import json
import os
import sys
from pathlib import Path
from typing import Any


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        raise SystemExit(f"missing JSONL artifact: {path}")
    rows: list[dict[str, Any]] = []
    for idx, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        raw = line.strip()
        if not raw:
            continue
        try:
            value = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"{path}:{idx}: invalid JSON: {exc}") from exc
        if not isinstance(value, dict):
            raise SystemExit(f"{path}:{idx}: expected JSON object rows")
        rows.append(value)
    if not rows:
        raise SystemExit(f"empty JSONL artifact: {path}")
    return rows


def classify_tier(present_ms: float) -> str:
    if present_ms <= 20.0:
        return "nominal"
    if present_ms <= 60.0:
        return "degraded"
    return "overload"


def summarize_case(case_name: str, path: Path, require_resize: bool) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    events = load_jsonl(path)

    run_ids = {
        event.get("run_id")
        for event in events
        if isinstance(event.get("run_id"), str) and event.get("run_id")
    }
    if len(run_ids) != 1:
        raise SystemExit(f"{case_name}: expected exactly one run_id, got {sorted(run_ids)}")
    run_id = next(iter(run_ids))

    seeds = {
        event.get("seed")
        for event in events
        if isinstance(event.get("seed"), int)
    }
    if len(seeds) != 1:
        raise SystemExit(f"{case_name}: expected exactly one integer seed, got {sorted(seeds)}")
    seed = next(iter(seeds))

    env = next((e for e in events if e.get("type") == "env"), None)
    browser_env = next((e for e in events if e.get("type") == "browser_env"), None)
    if env is None:
        raise SystemExit(f"{case_name}: missing env event")
    if browser_env is None:
        raise SystemExit(f"{case_name}: missing browser_env event")

    frames = [e for e in events if e.get("type") == "frame"]
    if not frames:
        raise SystemExit(f"{case_name}: missing frame events")

    frame_hashes: list[str] = []
    dims: list[tuple[int, int]] = []
    tier_transitions: list[dict[str, Any]] = []
    prev_tier: str | None = None

    for frame in frames:
        frame_hash = frame.get("frame_hash")
        if not isinstance(frame_hash, str) or not frame_hash.startswith("sha256:"):
            raise SystemExit(f"{case_name}: invalid frame_hash in frame event: {frame}")
        frame_hashes.append(frame_hash)

        cols = frame.get("cols")
        rows = frame.get("rows")
        if not isinstance(cols, int) or not isinstance(rows, int) or cols <= 0 or rows <= 0:
            raise SystemExit(f"{case_name}: invalid geometry in frame event: {frame}")
        dims.append((cols, rows))

        present_ms_raw = frame.get("present_ms")
        if isinstance(present_ms_raw, (int, float)):
            present_ms = float(present_ms_raw)
            tier = classify_tier(present_ms)
            if prev_tier is None:
                prev_tier = tier
            elif tier != prev_tier:
                tier_transitions.append(
                    {
                        "frame_idx": frame.get("frame_idx"),
                        "from_tier": prev_tier,
                        "to_tier": tier,
                        "present_ms": round(present_ms, 3),
                    }
                )
                prev_tier = tier

    resize_inputs = [
        e for e in events
        if e.get("type") == "input" and e.get("input_type") == "resize"
    ]
    if require_resize and not resize_inputs:
        raise SystemExit(f"{case_name}: expected resize input events")

    failed_asserts = [
        e for e in events
        if e.get("type") == "assert" and e.get("status") == "failed"
    ]
    errors = [e for e in events if e.get("type") == "error"]

    cols_values = [c for c, _ in dims]
    rows_values = [r for _, r in dims]
    unique_hashes = len(set(frame_hashes))

    summary = {
        "case": case_name,
        "run_id": run_id,
        "seed": seed,
        "event_count": len(events),
        "frame_count": len(frames),
        "resize_input_count": len(resize_inputs),
        "failed_assert_count": len(failed_asserts),
        "error_count": len(errors),
        "capabilities": {
            "term": env.get("term", ""),
            "colorterm": env.get("colorterm", ""),
            "no_color": env.get("no_color", ""),
            "deterministic": env.get("deterministic", False),
            "browser": browser_env.get("browser", ""),
            "browser_version": browser_env.get("browser_version", ""),
            "user_agent": browser_env.get("user_agent", ""),
            "dpr": browser_env.get("dpr"),
            "headless": browser_env.get("headless"),
        },
        "geometry": {
            "min_cols": min(cols_values),
            "max_cols": max(cols_values),
            "min_rows": min(rows_values),
            "max_rows": max(rows_values),
            "unique_sizes": len(set(dims)),
        },
        "frame_hashes": {
            "first": frame_hashes[0],
            "last": frame_hashes[-1],
            "unique": unique_hashes,
        },
        "tier_transition_count": len(tier_transitions),
        "jsonl_path": str(path),
    }
    return summary, tier_transitions


def make_suite_event_factory(run_id: str, seed: int, step_ms: int):
    seq = {"value": 0}

    def emit(event_type: str, payload: dict[str, Any]) -> dict[str, Any]:
        event_seq = seq["value"]
        seq["value"] += 1
        event = {
            "schema_version": "typography-rescale-suite-v1",
            "type": event_type,
            "timestamp": f"T{event_seq * step_ms:06d}",
            "run_id": run_id,
            "seed": seed,
            "event_seq": event_seq,
        }
        event.update(payload)
        return event

    return emit


suite_jsonl = Path(sys.argv[1])
suite_report = Path(sys.argv[2])
resize_jsonl = Path(sys.argv[3])
unicode_jsonl = Path(sys.argv[4])
unicode_failure_jsonl = Path(sys.argv[5])
overload_jsonl = Path(sys.argv[6])
seed = int(sys.argv[7])
step_ms = int(sys.argv[8])
run_cross_browser = sys.argv[9] == "1"
cross_report_path = Path(sys.argv[10]) if sys.argv[10] else None

suite_run_id = f"typography-rescale-{seed:08x}"
emit = make_suite_event_factory(suite_run_id, seed, step_ms)

resize_summary, resize_tiers = summarize_case("resize_storm", resize_jsonl, require_resize=True)
unicode_summary, unicode_tiers = summarize_case("unicode_mixed_script", unicode_jsonl, require_resize=False)
overload_summary, overload_tiers = summarize_case("overload_scrollback", overload_jsonl, require_resize=False)

unicode_failure_events = load_jsonl(unicode_failure_jsonl)
failure_asserts = [
    e for e in unicode_failure_events
    if e.get("type") == "assert" and e.get("status") == "failed"
]
failure_errors = [e for e in unicode_failure_events if e.get("type") == "error"]
if not failure_asserts:
    raise SystemExit("unicode_failure_injection: expected failed assert events")
if not failure_errors:
    raise SystemExit("unicode_failure_injection: expected error events")

cross_browser = None
if run_cross_browser:
    if cross_report_path is None or not cross_report_path.exists():
        raise SystemExit("cross-browser mode enabled but report artifact missing")
    cross_browser = json.loads(cross_report_path.read_text(encoding="utf-8"))

events: list[dict[str, Any]] = []
events.append(
    emit(
        "suite_start",
        {
            "suite": "remote_typography_rescale",
            "status": "running",
            "cases": ["resize_storm", "unicode_mixed_script", "overload_scrollback"],
            "cross_browser_enabled": run_cross_browser,
        },
    )
)

for summary in (resize_summary, unicode_summary, overload_summary):
    events.append(emit("suite_case_summary", summary))

for case_name, transitions in (
    ("resize_storm", resize_tiers),
    ("unicode_mixed_script", unicode_tiers),
    ("overload_scrollback", overload_tiers),
):
    for transition in transitions:
        events.append(
            emit(
                "tier_transition",
                {
                    "case": case_name,
                    "transition": transition,
                    "correlation_id": f"{suite_run_id}:{case_name}:{transition.get('frame_idx')}",
                },
            )
        )

for failed in failure_asserts:
    events.append(
        emit(
            "failure_context",
            {
                "case": "unicode_failure_injection",
                "kind": "assert",
                "assertion": failed.get("assertion", ""),
                "details": failed.get("details", ""),
            },
        )
    )
for err in failure_errors:
    events.append(
        emit(
            "failure_context",
            {
                "case": "unicode_failure_injection",
                "kind": "error",
                "message": err.get("message", ""),
                "details": err.get("details", ""),
            },
        )
    )

if cross_browser is not None:
    events.append(
        emit(
            "cross_browser_diff",
            {
                "report_path": str(cross_report_path),
                "outcome": cross_browser.get("outcome", ""),
                "comparisons": cross_browser.get("comparisons", []),
            },
        )
    )

events.append(
    emit(
        "suite_end",
        {
            "suite": "remote_typography_rescale",
            "status": "passed",
            "case_count": 3,
            "failure_context_count": len(failure_asserts) + len(failure_errors),
            "tier_transition_count": len(resize_tiers) + len(unicode_tiers) + len(overload_tiers),
        },
    )
)

suite_jsonl.parent.mkdir(parents=True, exist_ok=True)
with suite_jsonl.open("w", encoding="utf-8") as fh:
    for event in events:
        fh.write(json.dumps(event, separators=(",", ":")) + "\n")

report_payload = {
    "suite": "remote_typography_rescale",
    "status": "pass",
    "run_id": suite_run_id,
    "seed": seed,
    "artifacts": {
        "suite_jsonl": str(suite_jsonl),
        "resize_jsonl": str(resize_jsonl),
        "unicode_jsonl": str(unicode_jsonl),
        "unicode_failure_jsonl": str(unicode_failure_jsonl),
        "overload_jsonl": str(overload_jsonl),
        "cross_browser_report": str(cross_report_path) if cross_report_path else "",
    },
    "cases": [resize_summary, unicode_summary, overload_summary],
    "tier_transitions": {
        "resize_storm": resize_tiers,
        "unicode_mixed_script": unicode_tiers,
        "overload_scrollback": overload_tiers,
    },
    "failure_context": {
        "failed_asserts": failure_asserts,
        "errors": failure_errors,
    },
    "repro_commands": {
        "resize_storm": "REMOTE_PORT=9440 bash tests/e2e/scripts/test_remote_resize_storm.sh",
        "unicode_mixed_script": "REMOTE_PORT=9442 bash tests/e2e/scripts/test_remote_unicode.sh",
        "overload_scrollback": "REMOTE_PORT=9444 bash tests/e2e/scripts/test_remote_scrollback.sh",
    },
}
if run_cross_browser:
    report_payload["repro_commands"]["resize_cross_browser_diff"] = (
        "E2E_LOG_DIR=/tmp/ftui_e2e_resize_diff E2E_DIFF_LOG_DIR=/tmp/ftui_e2e_resize_diff "
        "bash tests/e2e/scripts/test_remote_resize_storm_cross_browser_diff.sh"
    )

suite_report.parent.mkdir(parents=True, exist_ok=True)
suite_report.write_text(json.dumps(report_payload, indent=2), encoding="utf-8")
PY
then
    echo "[FAIL] Aggregate validation/report generation failed"
    print_suite_repro
    exit 1
fi

echo "[PASS] Remote typography + rescale suite"
echo "  Suite JSONL:  $SUITE_JSONL"
echo "  Suite report: $SUITE_REPORT"
if [[ "$RUN_CROSS_BROWSER" == "1" ]]; then
    echo "  Cross report: $CROSS_REPORT"
fi
