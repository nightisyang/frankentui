#!/bin/bash
set -euo pipefail

# E2E: Cross-browser differential checks for remote resize-storm semantics.
#
# Runs resize-storm scenario for multiple browser labels, then compares
# normalized JSONL traces for deterministic geometry/interaction parity.
#
# Usage:
#   tests/e2e/scripts/test_remote_resize_storm_cross_browser_diff.sh
#   E2E_DIFF_BROWSERS=chromium,webkit tests/e2e/scripts/test_remote_resize_storm_cross_browser_diff.sh
#   E2E_DIFF_USE_EXISTING_ARTIFACTS=1 tests/e2e/scripts/test_remote_resize_storm_cross_browser_diff.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"
FIXTURES_DIR="$SCRIPT_DIR/../fixtures"

export E2E_DETERMINISTIC="${E2E_DETERMINISTIC:-1}"
export E2E_TIME_STEP_MS="${E2E_TIME_STEP_MS:-100}"
export E2E_SEED="${E2E_SEED:-0}"

if [[ -z "${E2E_LOG_DIR:-}" ]]; then
    E2E_LOG_DIR="/tmp/ftui_e2e_resize_diff_$(date +%Y%m%d_%H%M%S)"
fi

E2E_DIFF_BROWSERS="${E2E_DIFF_BROWSERS:-chromium,webkit,firefox}"
E2E_DIFF_BASE_PORT="${E2E_DIFF_BASE_PORT:-9360}"
E2E_DIFF_LOG_DIR="${E2E_DIFF_LOG_DIR:-$E2E_LOG_DIR/remote_resize_storm_cross_browser}"
E2E_DIFF_USE_EXISTING_ARTIFACTS="${E2E_DIFF_USE_EXISTING_ARTIFACTS:-0}"
E2E_DIFF_KNOWN_DIVERGENCES="${E2E_DIFF_KNOWN_DIVERGENCES:-$FIXTURES_DIR/remote_resize_storm_known_divergences.tsv}"
E2E_DIFF_REPORT_OUT="${E2E_DIFF_REPORT_OUT:-$E2E_DIFF_LOG_DIR/resize_storm_cross_browser_report.json}"

E2E_DIFF_MODE="${E2E_DIFF_MODE:-}"
if [[ -z "$E2E_DIFF_MODE" ]]; then
    if [[ "${CI:-}" == "1" || "${CI:-}" == "true" ]]; then
        E2E_DIFF_MODE="strict"
    else
        E2E_DIFF_MODE="warn"
    fi
fi

mkdir -p "$E2E_DIFF_LOG_DIR"

echo "=== Remote Resize Storm Cross-Browser Differential ==="
echo "Browsers: $E2E_DIFF_BROWSERS"
echo "Mode: $E2E_DIFF_MODE"
echo "Log dir: $E2E_DIFF_LOG_DIR"

IFS=',' read -r -a RAW_BROWSERS <<< "$E2E_DIFF_BROWSERS"
declare -a BROWSERS=()
for raw in "${RAW_BROWSERS[@]}"; do
    browser="${raw//[[:space:]]/}"
    if [[ -n "$browser" ]]; then
        BROWSERS+=("$browser")
    fi
done

if (( ${#BROWSERS[@]} < 2 )); then
    echo "[FAIL] Need at least two browsers in E2E_DIFF_BROWSERS"
    exit 1
fi

if [[ ! -f "$E2E_DIFF_KNOWN_DIVERGENCES" ]]; then
    echo "[FAIL] Known divergences file not found: $E2E_DIFF_KNOWN_DIVERGENCES"
    exit 1
fi

declare -a TRACE_ARGS=()

idx=0
for browser in "${BROWSERS[@]}"; do
    browser_log_dir="$E2E_DIFF_LOG_DIR/$browser"
    jsonl_path="$browser_log_dir/resize_storm.jsonl"
    mkdir -p "$browser_log_dir"

    if [[ "$E2E_DIFF_USE_EXISTING_ARTIFACTS" == "1" && -s "$jsonl_path" ]]; then
        echo "[INFO] Reusing existing JSONL for $browser: $jsonl_path"
    else
        port=$((E2E_DIFF_BASE_PORT + idx))
        echo "--- Running resize-storm for $browser (port $port) ---"
        REMOTE_PORT="$port" \
        REMOTE_LOG_DIR="$browser_log_dir" \
        E2E_BROWSER="$browser" \
        E2E_BROWSER_USER_AGENT="frankentui-e2e/$browser" \
        bash "$SCRIPT_DIR/test_remote_resize_storm.sh"
    fi

    if [[ ! -s "$jsonl_path" ]]; then
        echo "[FAIL] Missing JSONL artifact for $browser: $jsonl_path"
        exit 1
    fi

    TRACE_ARGS+=(--trace "${browser}=${jsonl_path}")
    idx=$((idx + 1))
done

python3 "$LIB_DIR/resize_storm_differential.py" \
    "${TRACE_ARGS[@]}" \
    --mode "$E2E_DIFF_MODE" \
    --known "$E2E_DIFF_KNOWN_DIVERGENCES" \
    --report "$E2E_DIFF_REPORT_OUT"

echo "[PASS] Remote resize-storm cross-browser differential check complete"
echo "  Report: $E2E_DIFF_REPORT_OUT"
