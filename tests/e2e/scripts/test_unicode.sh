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

FIXTURE_DIR="$E2E_ROOT/fixtures"

ALL_CASES=(
    unicode_basic_ascii
    unicode_accented
    unicode_wide_cjk
    unicode_emoji
    unicode_mixed_content
)

if [[ ! -x "${E2E_HARNESS_BIN:-}" ]]; then
    LOG_FILE="$E2E_LOG_DIR/unicode_missing.log"
    for t in "${ALL_CASES[@]}"; do
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
    log_test_fail "$name" "unicode assertions failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "unicode assertions failed"
    return 1
}

# Test: Basic ASCII content renders without issues
unicode_basic_ascii() {
    LOG_FILE="$E2E_LOG_DIR/unicode_basic_ascii.log"
    local output_file="$E2E_LOG_DIR/unicode_basic_ascii.pty"

    log_test_start "unicode_basic_ascii"

    FTUI_HARNESS_EXIT_AFTER_MS=800 \
    FTUI_HARNESS_LOG_LINES=10 \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # ASCII log lines should appear in output
    grep -a -q "Log line" "$output_file" || return 1
    # Status bar text should be present
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "Basic ASCII rendering verified"
}

# Test: Accented characters render correctly
unicode_accented() {
    LOG_FILE="$E2E_LOG_DIR/unicode_accented.log"
    local output_file="$E2E_LOG_DIR/unicode_accented.pty"

    log_test_start "unicode_accented"

    # Create a temp log file with accented content
    local log_content
    log_content="$(mktemp)"
    printf 'café résumé naïve\n' > "$log_content"
    printf 'Héllo àccénted wörld\n' >> "$log_content"

    FTUI_HARNESS_EXIT_AFTER_MS=800 \
    FTUI_HARNESS_LOG_FILE="$log_content" \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    rm -f "$log_content"

    # The output should contain the accented text (rendered through the PTY)
    # Accented chars are single-width, so should pass through.
    grep -a -q "caf" "$output_file" || return 1

    # Output should be substantial (app rendered without crashing on accented input)
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    log_debug "Accented character rendering verified"
}

# Test: CJK wide characters do not crash the renderer
unicode_wide_cjk() {
    LOG_FILE="$E2E_LOG_DIR/unicode_wide_cjk.log"
    local output_file="$E2E_LOG_DIR/unicode_wide_cjk.pty"

    log_test_start "unicode_wide_cjk"

    local log_content
    log_content="$(mktemp)"
    printf '日本語テスト\n' > "$log_content"
    printf '中文测试内容\n' >> "$log_content"
    printf '한국어 테스트\n' >> "$log_content"

    FTUI_HARNESS_EXIT_AFTER_MS=800 \
    FTUI_HARNESS_LOG_FILE="$log_content" \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    rm -f "$log_content"

    # The app must not crash when rendering wide characters.
    # Verify the output file has content (render cycles completed).
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Status bar should still render (app didn't panic on wide chars)
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "CJK wide character rendering verified (no crash)"
}

# Test: Emoji characters do not crash the renderer
unicode_emoji() {
    LOG_FILE="$E2E_LOG_DIR/unicode_emoji.log"
    local output_file="$E2E_LOG_DIR/unicode_emoji.pty"

    log_test_start "unicode_emoji"

    local log_content
    log_content="$(mktemp)"
    printf '🎉 Party time!\n' > "$log_content"
    printf '🚀 Launch 🌍 Earth\n' >> "$log_content"
    printf '✅ Done ❌ Failed ⚠️ Warning\n' >> "$log_content"

    FTUI_HARNESS_EXIT_AFTER_MS=800 \
    FTUI_HARNESS_LOG_FILE="$log_content" \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=3 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    rm -f "$log_content"

    # The app must not crash when rendering emoji.
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Status bar should still render
    grep -a -q "claude-3.5" "$output_file" || return 1

    log_debug "Emoji rendering verified (no crash)"
}

# Test: Mixed content (ASCII + Unicode + Emoji) in a single session
unicode_mixed_content() {
    LOG_FILE="$E2E_LOG_DIR/unicode_mixed_content.log"
    local output_file="$E2E_LOG_DIR/unicode_mixed_content.pty"

    log_test_start "unicode_mixed_content"

    FTUI_HARNESS_EXIT_AFTER_MS=1000 \
    FTUI_HARNESS_LOG_FILE="$FIXTURE_DIR/unicode_lines.txt" \
    FTUI_HARNESS_SUPPRESS_WELCOME=1 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # The app must not crash on the full unicode fixture file.
    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1

    # Status bar should still render
    grep -a -q "claude-3.5" "$output_file" || return 1

    # At least some ASCII content from the fixture should appear
    grep -a -q "Hello" "$output_file" || grep -a -q "ASCII" "$output_file" || return 1

    log_debug "Mixed unicode content rendering verified"
}

FAILURES=0
run_case "unicode_basic_ascii" unicode_basic_ascii         || FAILURES=$((FAILURES + 1))
run_case "unicode_accented" unicode_accented               || FAILURES=$((FAILURES + 1))
run_case "unicode_wide_cjk" unicode_wide_cjk              || FAILURES=$((FAILURES + 1))
run_case "unicode_emoji" unicode_emoji                     || FAILURES=$((FAILURES + 1))
run_case "unicode_mixed_content" unicode_mixed_content     || FAILURES=$((FAILURES + 1))
exit "$FAILURES"
