#!/bin/bash
# Modal/Dialog E2E Test Suite
# bd-39vx.6: End-to-end validation of modal + dialog behavior
#
# Usage:
#   ./scripts/e2e/modal_dialog/run_modal_dialog_e2e.sh
#   ./scripts/e2e/modal_dialog/run_modal_dialog_e2e.sh --verbose
#   ./scripts/e2e/modal_dialog/run_modal_dialog_e2e.sh --json /tmp/modal_dialog.jsonl
#
# Environment:
#   MODAL_DIALOG_SEED           Deterministic seed (default: 42)
#   MODAL_DIALOG_DETERMINISTIC  1 to force single-threaded test run (default: 1)
#   E2E_LOG_DIR                 Override log directory

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
LIB_DIR="$PROJECT_ROOT/tests/e2e/lib"
# shellcheck source=/dev/null
if [[ -f "$LIB_DIR/logging.sh" ]]; then
    source "$LIB_DIR/logging.sh"
fi
if ! declare -f e2e_timestamp >/dev/null 2>&1; then
    e2e_timestamp() { date -Iseconds; }
fi
if ! declare -f e2e_log_stamp >/dev/null 2>&1; then
    e2e_log_stamp() { date +%Y%m%d_%H%M%S; }
fi
if ! declare -f e2e_now_ms >/dev/null 2>&1; then
    e2e_now_ms() { date +%s%3N; }
fi

TIMESTAMP="$(e2e_log_stamp)"
LOG_DIR="${E2E_LOG_DIR:-/tmp/ftui-modal-dialog-e2e-${TIMESTAMP}}"
JSONL_OUT=""
VERBOSE=false
SEED="${MODAL_DIALOG_SEED:-42}"
DETERMINISTIC="${MODAL_DIALOG_DETERMINISTIC:-1}"
RUN_ID="modal_dialog_${TIMESTAMP}"

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
                JSONL_OUT="$LOG_DIR/results.jsonl"
                shift
            fi
            ;;
        --json=*)
            JSONL_OUT="${1#--json=}"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--verbose] [--json <path>]"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

mkdir -p "$LOG_DIR"
RESULTS_JSONL="$LOG_DIR/results.jsonl"
LOG_FILE="$LOG_DIR/modal_dialog_e2e.log"
export E2E_DETERMINISTIC="$DETERMINISTIC"
export E2E_SEED="$SEED"
if declare -f e2e_seed >/dev/null 2>&1; then
    e2e_seed >/dev/null 2>&1 || true
fi

emit_jsonl() {
    local line="$1"
    printf '%s\n' "$line" >> "$RESULTS_JSONL"
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

detect_capabilities() {
    local term_val="${TERM:-}"
    local colorterm="${COLORTERM:-}"
    if [[ -n "$colorterm" ]]; then
        echo "truecolor"
    elif [[ "$term_val" == *"256color"* ]]; then
        echo "256color"
    else
        echo "basic"
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
    local capabilities
    capabilities="$(detect_capabilities)"

    emit_jsonl "{\"run_id\":\"$RUN_ID\",\"event\":\"env\",\"timestamp\":\"$(e2e_timestamp)\",\"seed\":\"$SEED\",\"term\":\"$term_val\",\"colorterm\":\"$colorterm\",\"no_color\":\"$no_color\",\"tmux\":\"$tmux\",\"zellij\":\"$zellij\",\"kitty_window_id\":\"$kitty\",\"capabilities\":\"$capabilities\",\"rustc\":\"$rustc_version\",\"cargo\":\"$cargo_version\",\"git_commit\":\"$git_commit\"}"
}

run_case() {
    local name="$1"
    shift
    local start_ms
    start_ms="$(e2e_now_ms)"

    emit_jsonl "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"event\":\"start\"}"

    local exit_code=0
    if $VERBOSE; then
        if (cd "$PROJECT_ROOT" && "$@" 2>&1 | tee "$LOG_FILE"); then
            exit_code=0
        else
            exit_code=1
        fi
    else
        if (cd "$PROJECT_ROOT" && "$@" > "$LOG_FILE" 2>&1); then
            exit_code=0
        else
            exit_code=1
        fi
    fi

    local end_ms
    end_ms="$(e2e_now_ms)"
    local duration_ms=$((end_ms - start_ms))
    local checksum
    checksum="$(compute_checksum "$LOG_FILE")"

    if [[ $exit_code -eq 0 ]]; then
        emit_jsonl "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"seed\":\"$SEED\",\"timings\":{\"start_ms\":$start_ms,\"end_ms\":$end_ms,\"duration_ms\":$duration_ms},\"checksums\":{\"log\":\"$checksum\"},\"outcome\":{\"status\":\"passed\"}}"
        return 0
    fi

    emit_jsonl "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"seed\":\"$SEED\",\"timings\":{\"start_ms\":$start_ms,\"end_ms\":$end_ms,\"duration_ms\":$duration_ms},\"checksums\":{\"log\":\"$checksum\"},\"outcome\":{\"status\":\"failed\",\"reason\":\"test failed\"}}"
    return 1
}

if [[ "$DETERMINISTIC" == "1" ]]; then
    export RUST_TEST_THREADS=1
fi
export FTUI_SEED="$SEED"
export MODAL_DIALOG_SEED="$SEED"

log_env

run_case "modal_dialog_e2e" \
    cargo test -p ftui-demo-showcase --test modal_e2e -- --nocapture
