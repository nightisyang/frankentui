#!/bin/bash
# Performance HUD Demo Launcher (bd-3k3x.5)
#
# Usage:
#   ./scripts/perf_hud_demo.sh [--inline] [--ui-height N] [--auto-exit MS] [--pty] [--no-mouse]
#
# Notes:
# - Press Ctrl+P to toggle the HUD
# - Press ? for help, q to quit

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SCREEN_MODE="alt"
UI_HEIGHT=20
AUTO_EXIT_MS=0
USE_PTY=false
NO_MOUSE=false

while [ $# -gt 0 ]; do
    case "$1" in
        --inline)
            SCREEN_MODE="inline"
            ;;
        --ui-height)
            if [ $# -lt 2 ]; then
                echo "ERROR: --ui-height requires a value" >&2
                exit 1
            fi
            UI_HEIGHT="$2"
            shift
            ;;
        --auto-exit)
            if [ $# -lt 2 ]; then
                echo "ERROR: --auto-exit requires a value" >&2
                exit 1
            fi
            AUTO_EXIT_MS="$2"
            shift
            ;;
        --pty)
            USE_PTY=true
            ;;
        --no-mouse)
            NO_MOUSE=true
            ;;
        --help|-h)
            echo "Usage: $0 [--inline] [--ui-height N] [--auto-exit MS] [--pty] [--no-mouse]"
            exit 0
            ;;
    esac
    shift
done

cd "$PROJECT_ROOT"

CMD=(cargo run -p ftui-demo-showcase -- \
    "--screen-mode=${SCREEN_MODE}" \
    "--ui-height=${UI_HEIGHT}" \
    "--screen=10")

if $NO_MOUSE; then
    CMD+=("--no-mouse")
fi

if [ "$AUTO_EXIT_MS" -gt 0 ]; then
    CMD+=("--exit-after-ms=${AUTO_EXIT_MS}")
fi

echo "Performance HUD Demo"
echo "- Screen mode: ${SCREEN_MODE}"
echo "- UI height:   ${UI_HEIGHT}"
if [ "$AUTO_EXIT_MS" -gt 0 ]; then
    echo "- Auto-exit:   ${AUTO_EXIT_MS} ms"
fi
if $NO_MOUSE; then
    echo "- Mouse:       disabled"
fi
echo ""
echo "Controls: Ctrl+P (toggle HUD), ? (help), q (quit)"
echo ""

if $USE_PTY; then
    if ! command -v script >/dev/null 2>&1; then
        echo "ERROR: 'script' is required for --pty but not found." >&2
        exit 1
    fi
    if [ "$(uname)" = "Linux" ]; then
        script -qec "stty rows 40 cols 120 2>/dev/null; ${CMD[*]}" /dev/null
    else
        script -q /dev/null bash -c "stty rows 40 cols 120 2>/dev/null; ${CMD[*]}"
    fi
else
    "${CMD[@]}"
fi
