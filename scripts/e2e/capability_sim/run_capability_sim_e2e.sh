#!/bin/bash
set -euo pipefail

# Capability Simulator E2E Test Suite (bd-k4lj.6)
#
# Runs the capability simulation tests with JSONL logging for
# env, capabilities, timings, seed, and checksums.
#
# Usage:
#   ./scripts/e2e/capability_sim/run_capability_sim_e2e.sh
#   ./scripts/e2e/capability_sim/run_capability_sim_e2e.sh --verbose
#   ./scripts/e2e/capability_sim/run_capability_sim_e2e.sh --json /tmp/cap_sim.jsonl
#   ./scripts/e2e/capability_sim/run_capability_sim_e2e.sh --seed 123 --deterministic 1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
LOG_DIR="${E2E_LOG_DIR:-/tmp/ftui-cap-sim-e2e-${TIMESTAMP}}"
RESULTS_DIR="${E2E_RESULTS_DIR:-$LOG_DIR/results}"
LOG_FILE="$LOG_DIR/capability_sim_e2e.log"
JSONL_OUT=""
VERBOSE=false
SEED="${CAP_SIM_SEED:-0}"
DETERMINISTIC="${CAP_SIM_DETERMINISTIC:-1}"
RUN_ID="cap_sim_${TIMESTAMP}_$$"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --json)
            if [[ $# -ge 2 ]]; then
                JSONL_OUT="$2"
                shift 2
            else
                JSONL_OUT="$RESULTS_DIR/capability_sim.jsonl"
                shift
            fi
            ;;
        --json=*)
            JSONL_OUT="${1#--json=}"
            shift
            ;;
        --seed)
            if [[ $# -ge 2 ]]; then
                SEED="$2"
                shift 2
            else
                shift
            fi
            ;;
        --seed=*)
            SEED="${1#--seed=}"
            shift
            ;;
        --deterministic)
            if [[ $# -ge 2 ]]; then
                DETERMINISTIC="$2"
                shift 2
            else
                DETERMINISTIC="1"
                shift
            fi
            ;;
        --deterministic=*)
            DETERMINISTIC="${1#--deterministic=}"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--verbose] [--json <path>] [--seed <n>] [--deterministic 0|1]"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

mkdir -p "$LOG_DIR" "$RESULTS_DIR"

emit_jsonl() {
    local line="$1"
    printf '%s\n' "$line" >> "$RESULTS_DIR/capability_sim.jsonl"
    if [[ -n "$JSONL_OUT" ]]; then
        printf '%s\n' "$line" >> "$JSONL_OUT"
    fi
}

compute_checksum() {
    local file="$1"
    if command -v sha256sum >/dev/null 2>&1 && [[ -f "$file" ]]; then
        sha256sum "$file" | awk '{print $1}'
    else
        echo "unavailable"
    fi
}

log_env() {
    local term_val="${TERM:-}"
    local colorterm="${COLORTERM:-}"
    local no_color="${NO_COLOR:-}"
    local tmux="${TMUX:-}"
    local zellij="${ZELLIJ:-}"
    local kitty="${KITTY_WINDOW_ID:-}"
    local rustc_version
    rustc_version="$(rustc --version 2>/dev/null || echo 'N/A')"
    local cargo_version
    cargo_version="$(cargo --version 2>/dev/null || echo 'N/A')"
    local git_commit
    git_commit="$(cd "$PROJECT_ROOT" && git log -1 --oneline 2>/dev/null || echo 'N/A')"

    emit_jsonl "{\"run_id\":\"$RUN_ID\",\"event\":\"env\",\"timestamp\":\"$(date -Iseconds)\",\"seed\":$SEED,\"deterministic\":$DETERMINISTIC,\"env\":{\"term\":\"$term_val\",\"colorterm\":\"$colorterm\",\"no_color\":\"$no_color\",\"tmux\":\"$tmux\",\"zellij\":\"$zellij\",\"kitty_window_id\":\"$kitty\"},\"toolchain\":{\"rustc\":\"$rustc_version\",\"cargo\":\"$cargo_version\"},\"git_commit\":\"$git_commit\"}"
}

run_case() {
    local name="$1"
    shift
    local start_ms
    start_ms="$(date +%s%3N)"
    emit_jsonl "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"event\":\"start\",\"ts_ms\":$start_ms}"

    local exit_code=0
    if $VERBOSE; then
        if (cd "$PROJECT_ROOT" && "$@" 2>&1 | tee -a "$LOG_FILE"); then
            exit_code=0
        else
            exit_code=1
        fi
    else
        if (cd "$PROJECT_ROOT" && "$@" >> "$LOG_FILE" 2>&1); then
            exit_code=0
        else
            exit_code=1
        fi
    fi

    local end_ms
    end_ms="$(date +%s%3N)"
    local duration_ms=$((end_ms - start_ms))
    local checksum
    checksum="$(compute_checksum "$LOG_FILE")"

    if [[ $exit_code -eq 0 ]]; then
        emit_jsonl "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"event\":\"complete\",\"status\":\"passed\",\"timings\":{\"start_ms\":$start_ms,\"end_ms\":$end_ms,\"duration_ms\":$duration_ms},\"checksums\":{\"log\":\"$checksum\"}}"
        return 0
    fi

    emit_jsonl "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"event\":\"complete\",\"status\":\"failed\",\"timings\":{\"start_ms\":$start_ms,\"end_ms\":$end_ms,\"duration_ms\":$duration_ms},\"checksums\":{\"log\":\"$checksum\"}}"
    return 1
}

export E2E_LOG_DIR="$LOG_DIR"
export E2E_RESULTS_DIR="$RESULTS_DIR"
export LOG_FILE
export CAP_SIM_LOG=1
export CAP_SIM_SEED="$SEED"
CAP_SIM_DIR="${TMPDIR:-/tmp}/ftui_cap_sim_e2e"

if [[ "$DETERMINISTIC" == "1" ]]; then
    export RUST_TEST_THREADS=1
fi

log_env

FAILURES=0

run_case "capability_sim_core" \
    cargo test -p ftui-harness --test capability_sim_e2e -- --nocapture || FAILURES=$((FAILURES + 1))

if [[ -d "$CAP_SIM_DIR" ]]; then
    files_json=""
    for f in "$CAP_SIM_DIR"/*.jsonl; do
        [[ -f "$f" ]] || continue
        file_name="$(basename "$f")"
        file_hash="$(compute_checksum "$f")"
        if [[ -n "$files_json" ]]; then
            files_json="${files_json},"
        fi
        files_json="${files_json}{\\\"file\\\":\\\"$file_name\\\",\\\"sha256\\\":\\\"$file_hash\\\"}"
    done
    emit_jsonl "{\\\"run_id\\\":\\\"$RUN_ID\\\",\\\"event\\\":\\\"cap_sim_logs\\\",\\\"files\\\":[${files_json}]}"
fi

run_case "capability_sim_terminal_quirks" \
    bash tests/e2e/scripts/test_terminal_quirks.sh || FAILURES=$((FAILURES + 1))

run_case "capability_sim_terminal_caps" \
    bash tests/e2e/scripts/test_terminal_capabilities.sh || FAILURES=$((FAILURES + 1))

emit_jsonl "{\"run_id\":\"$RUN_ID\",\"event\":\"summary\",\"failures\":$FAILURES}"

exit "$FAILURES"
