#!/bin/bash
set -euo pipefail

pty_run() {
    local output_file="$1"
    shift

    if [[ -z "${E2E_PYTHON:-}" ]]; then
        echo "E2E_PYTHON is not set (python3/python not found)" >&2
        return 1
    fi

    local timeout="${PTY_TIMEOUT:-5}"
    local send_data="${PTY_SEND:-}"
    local send_delay_ms="${PTY_SEND_DELAY_MS:-0}"
    local cols="${PTY_COLS:-80}"
    local rows="${PTY_ROWS:-24}"
    local drain_timeout_ms="${PTY_DRAIN_TIMEOUT_MS:-200}"
    local terminate_grace_ms="${PTY_TERMINATE_GRACE_MS:-300}"
    local read_poll_ms="${PTY_READ_POLL_MS:-50}"
    local read_chunk="${PTY_READ_CHUNK:-4096}"
    local retries="${PTY_RETRIES:-1}"
    local retry_delay_ms="${PTY_RETRY_DELAY_MS:-100}"
    local min_bytes="${PTY_MIN_BYTES:-0}"

    local attempt=1
    local exit_code=0
    while [[ "$attempt" -le "$retries" ]]; do
        if PTY_OUTPUT="$output_file" \
            PTY_TIMEOUT="$timeout" \
            PTY_SEND="$send_data" \
            PTY_SEND_DELAY_MS="$send_delay_ms" \
            PTY_COLS="$cols" \
            PTY_ROWS="$rows" \
            PTY_DRAIN_TIMEOUT_MS="$drain_timeout_ms" \
            PTY_TERMINATE_GRACE_MS="$terminate_grace_ms" \
            PTY_READ_POLL_MS="$read_poll_ms" \
            PTY_READ_CHUNK="$read_chunk" \
            "$E2E_PYTHON" - "$@" <<'PY'
import codecs
import os
import pty
import select
import subprocess
import sys
import time
import signal

cmd = sys.argv[1:]
if not cmd:
    print("No command provided", file=sys.stderr)
    sys.exit(2)

output_path = os.environ.get("PTY_OUTPUT")
if not output_path:
    print("PTY_OUTPUT not set", file=sys.stderr)
    sys.exit(2)

timeout = float(os.environ.get("PTY_TIMEOUT", "5"))
raw_send = os.environ.get("PTY_SEND", "")
send_delay_ms = int(os.environ.get("PTY_SEND_DELAY_MS", "0"))
cols = int(os.environ.get("PTY_COLS", "80"))
rows = int(os.environ.get("PTY_ROWS", "24"))
drain_timeout = float(os.environ.get("PTY_DRAIN_TIMEOUT_MS", "200")) / 1000.0
terminate_grace = float(os.environ.get("PTY_TERMINATE_GRACE_MS", "300")) / 1000.0
read_poll = float(os.environ.get("PTY_READ_POLL_MS", "50")) / 1000.0
read_chunk = int(os.environ.get("PTY_READ_CHUNK", "4096"))

send_bytes = b""
if raw_send:
    send_bytes = codecs.decode(raw_send, "unicode_escape").encode("utf-8")

master_fd, slave_fd = pty.openpty()

try:
    import fcntl
    import struct
    import termios

    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(slave_fd, termios.TIOCSWINSZ, winsize)
except Exception:
    pass

start = time.monotonic()
deadline = start + timeout

proc = subprocess.Popen(
    cmd,
    stdin=slave_fd,
    stdout=slave_fd,
    stderr=slave_fd,
    close_fds=True,
    env=os.environ.copy(),
    start_new_session=True,
)

os.close(slave_fd)

captured = bytearray()
sent = False
last_data = start
terminate_at = None
stop_at = None

try:
    while True:
        now = time.monotonic()
        if (not sent) and send_bytes and (now - start) >= (send_delay_ms / 1000.0):
            try:
                os.write(master_fd, send_bytes)
                sent = True
            except OSError:
                pass

        if terminate_at is None and now >= deadline:
            terminate_at = now + terminate_grace
            stop_at = terminate_at + drain_timeout
            try:
                os.killpg(proc.pid, signal.SIGTERM)
            except Exception:
                try:
                    proc.terminate()
                except Exception:
                    pass

        if terminate_at is not None and now >= terminate_at:
            if proc.poll() is None:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except Exception:
                    try:
                        proc.kill()
                    except Exception:
                        pass

        rlist, _, _ = select.select([master_fd], [], [], read_poll)
        if rlist:
            try:
                chunk = os.read(master_fd, read_chunk)
            except OSError:
                break
            if not chunk:
                break
            captured.extend(chunk)
            last_data = now

        exit_code = proc.poll()
        if exit_code is not None:
            if now - last_data >= drain_timeout:
                break

        if stop_at is not None and now >= stop_at and (now - last_data >= drain_timeout):
            break
finally:
    try:
        os.close(master_fd)
    except Exception:
        pass

exit_code = proc.poll()
if exit_code is None:
    exit_code = 124

with open(output_path, "wb") as handle:
    handle.write(captured)

sys.exit(exit_code)
PY
        then
            exit_code=0
        else
            exit_code=$?
        fi
        if [[ "$retries" -le 1 ]]; then
            return "$exit_code"
        fi
        local size
        size=$(wc -c < "$output_file" | tr -d ' ')
        if [[ "$exit_code" -eq 0 ]] && [[ "$size" -ge "$min_bytes" ]]; then
            return 0
        fi
        if [[ "$attempt" -ge "$retries" ]]; then
            return "$exit_code"
        fi
        local retry_delay_s
        retry_delay_s="$(awk -v ms="$retry_delay_ms" 'BEGIN {printf "%.3f", ms/1000}')"
        sleep "$retry_delay_s"
        attempt=$((attempt + 1))
    done
}
