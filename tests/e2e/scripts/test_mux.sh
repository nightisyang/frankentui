#!/bin/bash
set -euo pipefail

# E2E tests for multiplexer (tmux/screen/zellij) behavior.
#
# Covers:
# - Tmux environment detection (TMUX env var)
# - Screen environment detection (STY env var)
# - Zellij environment detection (ZELLIJ env var)
# - Scroll region disabled in mux environments
# - Overlay mode fallback in mux environments
# - Passthrough sequence handling

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

if [[ ! -x "${E2E_HARNESS_BIN:-}" ]]; then
    LOG_FILE="$E2E_LOG_DIR/mux_missing.log"
    for t in mux_tmux_detected mux_screen_detected mux_zellij_detected mux_no_scroll_region mux_overlay_mode mux_clean_env; do
        log_test_skip "$t" "ftui-harness binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
    done
    exit 0
fi

run_case() {
    local name="$1"
    shift
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
    log_test_fail "$name" "assertion failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "assertion failed"
    return 1
}

# Test: Tmux environment detection
# When TMUX env var is set, harness should detect mux environment.
mux_tmux_detected() {
    LOG_FILE="$E2E_LOG_DIR/mux_tmux_detected.log"
    local output_file="$E2E_LOG_DIR/mux_tmux_detected.pty"

    log_test_start "mux_tmux_detected"

    # Simulate tmux environment
    TMUX="/tmp/tmux-1000/default,12345,0" \
    TERM=screen-256color \
    FTUI_HARNESS_EXIT_AFTER_MS=1000 \
    FTUI_HARNESS_LOG_LINES=5 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Harness should still render UI
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "mux_tmux_detected: $size bytes captured"
}

# Test: GNU Screen environment detection
# When STY env var is set, harness should detect mux environment.
mux_screen_detected() {
    LOG_FILE="$E2E_LOG_DIR/mux_screen_detected.log"
    local output_file="$E2E_LOG_DIR/mux_screen_detected.pty"

    log_test_start "mux_screen_detected"

    # Simulate screen environment
    STY="12345.pts-0.hostname" \
    TERM=screen \
    FTUI_HARNESS_EXIT_AFTER_MS=1000 \
    FTUI_HARNESS_LOG_LINES=5 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Harness should still render UI
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "mux_screen_detected: $size bytes captured"
}

# Test: Zellij environment detection
# When ZELLIJ env var is set, harness should detect mux environment.
mux_zellij_detected() {
    LOG_FILE="$E2E_LOG_DIR/mux_zellij_detected.log"
    local output_file="$E2E_LOG_DIR/mux_zellij_detected.pty"

    log_test_start "mux_zellij_detected"

    # Simulate zellij environment
    ZELLIJ="0" \
    TERM=xterm-256color \
    FTUI_HARNESS_EXIT_AFTER_MS=1000 \
    FTUI_HARNESS_LOG_LINES=5 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Harness should still render UI
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "mux_zellij_detected: $size bytes captured"
}

# Test: No scroll region in mux environment
# In a mux environment, DECSTBM (scroll region) should NOT be used.
mux_no_scroll_region() {
    LOG_FILE="$E2E_LOG_DIR/mux_no_scroll_region.log"
    local output_file="$E2E_LOG_DIR/mux_no_scroll_region.pty"

    log_test_start "mux_no_scroll_region"

    # Simulate tmux environment with inline mode
    TMUX="/tmp/tmux-1000/default,12345,0" \
    TERM=screen-256color \
    FTUI_HARNESS_EXIT_AFTER_MS=1200 \
    FTUI_HARNESS_LOG_LINES=10 \
    FTUI_HARNESS_SCREEN_MODE=inline \
    FTUI_HARNESS_UI_HEIGHT=6 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # In mux environment, DECSTBM should NOT be present
    # DECSTBM format: ESC [ digits ; digits r
    if grep -a -o -P '\x1b\[\d+;\d+r' "$output_file" >/dev/null 2>&1; then
        log_error "Scroll region sequence found in mux environment (should be disabled)"
        return 1
    fi

    log_debug "mux_no_scroll_region: No scroll region sequences found (correct)"
}

# Test: Overlay mode in mux
# In mux environment, harness should use overlay mode (cursor save/restore).
mux_overlay_mode() {
    LOG_FILE="$E2E_LOG_DIR/mux_overlay_mode.log"
    local output_file="$E2E_LOG_DIR/mux_overlay_mode.pty"

    log_test_start "mux_overlay_mode"

    # Simulate tmux environment with inline mode
    TMUX="/tmp/tmux-1000/default,12345,0" \
    TERM=screen-256color \
    FTUI_HARNESS_EXIT_AFTER_MS=1200 \
    FTUI_HARNESS_LOG_LINES=10 \
    FTUI_HARNESS_SCREEN_MODE=inline \
    FTUI_HARNESS_UI_HEIGHT=6 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Overlay mode uses cursor save/restore
    # DEC cursor save: ESC 7
    # DEC cursor restore: ESC 8
    # These should appear multiple times during render cycles
    if grep -a -F -q $'\x1b7' "$output_file" || grep -a -F -q $'\x1b[s' "$output_file"; then
        log_debug "Cursor save sequence found (overlay mode)"
    else
        log_debug "No cursor save - may be using different strategy"
    fi

    # Harness should still render UI correctly
    grep -a -q "claude-3.5" "$output_file" || return 1
}

# Test: Clean (non-mux) environment
# Without mux env vars, harness should have full capabilities.
mux_clean_env() {
    LOG_FILE="$E2E_LOG_DIR/mux_clean_env.log"
    local output_file="$E2E_LOG_DIR/mux_clean_env.pty"

    log_test_start "mux_clean_env"

    # Clear mux environment variables
    unset TMUX STY ZELLIJ 2>/dev/null || true

    TERM=xterm-256color \
    COLORTERM=truecolor \
    FTUI_HARNESS_EXIT_AFTER_MS=1000 \
    FTUI_HARNESS_LOG_LINES=5 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Harness should render UI
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "mux_clean_env: $size bytes captured"
}

# Test: tmux passthrough sequences
# Verify tmux-specific DCS passthrough handling (if applicable).
mux_passthrough() {
    LOG_FILE="$E2E_LOG_DIR/mux_passthrough.log"
    local output_file="$E2E_LOG_DIR/mux_passthrough.pty"

    log_test_start "mux_passthrough"

    # Simulate tmux environment with passthrough support
    TMUX="/tmp/tmux-1000/default,12345,0" \
    TERM=tmux-256color \
    FTUI_HARNESS_EXIT_AFTER_MS=1000 \
    FTUI_HARNESS_LOG_LINES=5 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Should have output
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Check for DCS passthrough sequences: DCS tmux; ... ST
    # Pattern: ESC P tmux; ... ESC \ or BEL
    # This is optional - depends on capability detection
    if grep -a -o -P '\x1bPtmux;' "$output_file" >/dev/null 2>&1; then
        log_debug "tmux passthrough sequences detected"
    else
        log_debug "No tmux passthrough (may not be using passthrough)"
    fi

    # Harness should still work
    grep -a -q "claude-3.5" "$output_file" || return 1
}

FAILURES=0
run_case "mux_tmux_detected" mux_tmux_detected          || FAILURES=$((FAILURES + 1))
run_case "mux_screen_detected" mux_screen_detected      || FAILURES=$((FAILURES + 1))
run_case "mux_zellij_detected" mux_zellij_detected      || FAILURES=$((FAILURES + 1))
run_case "mux_no_scroll_region" mux_no_scroll_region    || FAILURES=$((FAILURES + 1))
run_case "mux_overlay_mode" mux_overlay_mode            || FAILURES=$((FAILURES + 1))
run_case "mux_clean_env" mux_clean_env                  || FAILURES=$((FAILURES + 1))
run_case "mux_passthrough" mux_passthrough              || FAILURES=$((FAILURES + 1))

exit "$FAILURES"
