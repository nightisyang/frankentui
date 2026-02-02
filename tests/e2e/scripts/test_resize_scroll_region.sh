#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

E2E_SUITE_SCRIPT="$SCRIPT_DIR/test_resize_scroll_region.sh"
export E2E_SUITE_SCRIPT
ONLY_CASE="${E2E_ONLY_CASE:-}"

ALL_CASES=(
    resize_scroll_region_bounds
)

if [[ ! -x "${E2E_HARNESS_BIN:-}" ]]; then
    LOG_FILE="$E2E_LOG_DIR/resize_scroll_region_missing.log"
    for t in "${ALL_CASES[@]}"; do
        log_test_skip "$t" "ftui-harness binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
    done
    exit 0
fi

run_case() {
    local name="$1"
    shift
    if [[ -n "$ONLY_CASE" && "$ONLY_CASE" != "$name" ]]; then
        LOG_FILE="$E2E_LOG_DIR/${name}.log"
        log_test_skip "$name" "filtered (E2E_ONLY_CASE=$ONLY_CASE)"
        record_result "$name" "skipped" 0 "$LOG_FILE" "filtered"
        return 0
    fi
    local start_ms
    start_ms="$(date +%s%3N)"

    if "$@"; then
        local end_ms
        end_ms="$(date +%s%3N)"
        local duration_ms=$((end_ms - start_ms))
        log_test_pass "$name"
        record_result "$name" "passed" "$duration_ms" "$LOG_FILE"
        return 0
    fi

    local end_ms
    end_ms="$(date +%s%3N)"
    local duration_ms=$((end_ms - start_ms))
    log_test_fail "$name" "resize/scroll-region assertions failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "resize/scroll-region assertions failed"
    return 1
}

resize_scroll_region_bounds() {
    LOG_FILE="$E2E_LOG_DIR/resize_scroll_region_bounds.log"
    local output_file="$E2E_LOG_DIR/resize_scroll_region_bounds.pty"

    log_test_start "resize_scroll_region_bounds"

    local initial_cols=80
    local initial_rows=24
    local resize_cols=100
    local resize_rows=30
    local resize_delay_ms=400
    local ui_height=8

    log_info "Resize schedule: ${initial_cols}x${initial_rows} -> ${resize_cols}x${resize_rows} @ ${resize_delay_ms}ms"
    log_info "Expected scroll region: 1;16r then 1;22r (ui_height=${ui_height})"

    unset TMUX ZELLIJ TERM_PROGRAM TERM_PROGRAM_VERSION 2>/dev/null || true

    TERM="xterm-256color" \
    PTY_COLS="$initial_cols" \
    PTY_ROWS="$initial_rows" \
    PTY_RESIZE_COLS="$resize_cols" \
    PTY_RESIZE_ROWS="$resize_rows" \
    PTY_RESIZE_DELAY_MS="$resize_delay_ms" \
    FTUI_HARNESS_SCREEN_MODE=inline \
    FTUI_HARNESS_UI_HEIGHT="$ui_height" \
    FTUI_HARNESS_LOG_LINES=25 \
    FTUI_HARNESS_EXIT_AFTER_MS=1600 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    log_info "Observed resize lines (raw PTY capture)"
    grep -a "Resize:" "$output_file" >> "$LOG_FILE" 2>&1 || true

    # Resize event should be logged by the harness.
    grep -a -q "Resize: ${resize_cols}x${resize_rows}" "$output_file" || return 1
    # UI chrome should still render.
    grep -a -q "claude-3.5" "$output_file" || return 1

    # Scroll region bounds should be set for initial + resized terminal sizes.
    grep -a -F -q $'\x1b[1;16r' "$output_file" || return 1
    grep -a -F -q $'\x1b[1;22r' "$output_file" || return 1

    # Cursor save/restore sequences should be present in inline mode.
    grep -a -F -q $'\x1b7' "$output_file" || return 1
    grep -a -F -q $'\x1b8' "$output_file" || return 1

    # Log a final buffer snapshot for diagnostics.
    log_info "Final PTY tail (printable)"
    if command -v strings >/dev/null 2>&1; then
        strings -n 3 "$output_file" | tail -n 30 >> "$LOG_FILE" 2>&1 || true
    fi
    log_info "Final PTY tail (hex)"
    if command -v xxd >/dev/null 2>&1; then
        tail -c 256 "$output_file" | xxd -g 1 >> "$LOG_FILE" 2>&1 || true
    fi
}

FAILURES=0
run_case "resize_scroll_region_bounds" resize_scroll_region_bounds || FAILURES=$((FAILURES + 1))
exit "$FAILURES"
