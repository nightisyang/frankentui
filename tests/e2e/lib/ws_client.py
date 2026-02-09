#!/usr/bin/env python3
"""WebSocket client for scripted remote terminal sessions.

Connects to the frankenterm_ws_bridge, sends scripted input sequences,
captures output, computes checksums, and emits JSONL event logs.

Usage:
    python3 ws_client.py --url ws://127.0.0.1:9231 --scenario scenario.json
    python3 ws_client.py --url ws://127.0.0.1:9231 --scenario scenario.json --golden golden.transcript

Scenario JSON format:
{
    "name": "resize_storm",
    "description": "Rapid resize events over WebSocket",
    "initial_cols": 120,
    "initial_rows": 40,
    "steps": [
        {"type": "send", "data_hex": "6c730a", "delay_ms": 100},
        {"type": "resize", "cols": 80, "rows": 24, "delay_ms": 50},
        {"type": "send", "data_b64": "bHMgLWxhCg==", "delay_ms": 100},
        {"type": "wait", "ms": 500},
        {"type": "drain"}
    ],
    "timeout_s": 30
}
"""

import argparse
import asyncio
import base64
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

try:
    import websockets
except ImportError:
    print("ERROR: 'websockets' package not available", file=sys.stderr)
    sys.exit(1)


def git_sha() -> str:
    """Return short git SHA of the working tree."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, timeout=5
        )
        return result.stdout.strip() if result.returncode == 0 else "unknown"
    except Exception:
        return "unknown"


def make_run_id(seed: int) -> str:
    """Deterministic run ID from seed."""
    if os.environ.get("E2E_DETERMINISTIC", "1") == "1":
        return f"remote-{seed:08x}"
    return f"remote-{int(time.time() * 1000):x}"


def sha256_hex(data: bytes) -> str:
    """Compute SHA-256 hex digest."""
    return hashlib.sha256(data).hexdigest()


class SessionRecorder:
    """Records session events and computes rolling checksums."""

    def __init__(self, run_id: str, scenario_name: str, jsonl_path: str | None):
        self.run_id = run_id
        self.scenario_name = scenario_name
        self.jsonl_path = jsonl_path
        self.jsonl_file = None
        self.events: list[dict] = []
        self.output_chunks: list[bytes] = []
        self.total_ws_in = 0
        self.total_ws_out = 0
        self.frame_idx = 0
        self.checksum_chain = "0" * 64

        if jsonl_path:
            self.jsonl_file = open(jsonl_path, "a")

    def emit(self, event_type: str, data: dict | None = None):
        """Emit a JSONL event."""
        event = {
            "schema_version": "e2e-jsonl-v1",
            "type": event_type,
            "timestamp": self._timestamp(),
            "run_id": self.run_id,
            "seed": int(os.environ.get("E2E_SEED", "0")),
        }
        if data:
            event.update(data)
        self.events.append(event)
        if self.jsonl_file:
            self.jsonl_file.write(json.dumps(event, separators=(",", ":")) + "\n")
            self.jsonl_file.flush()

    def record_output(self, data: bytes):
        """Record PTY output received over WebSocket."""
        self.output_chunks.append(data)
        self.total_ws_out += len(data)
        chunk_hash = sha256_hex(data)
        self.checksum_chain = sha256_hex(
            (self.checksum_chain + chunk_hash).encode()
        )
        self.frame_idx += 1
        self.emit("frame", {
            "frame_idx": self.frame_idx,
            "chunk_bytes": len(data),
            "chunk_hash": f"sha256:{chunk_hash[:16]}",
            "checksum_chain": f"sha256:{self.checksum_chain[:16]}",
        })

    def record_send(self, data: bytes):
        """Record data sent to PTY."""
        self.total_ws_in += len(data)

    def full_output(self) -> bytes:
        """Return concatenated output."""
        return b"".join(self.output_chunks)

    def final_checksum(self) -> str:
        """Return the final rolling checksum."""
        return self.checksum_chain

    def summary(self) -> dict:
        """Return session summary dict."""
        output = self.full_output()
        return {
            "scenario": self.scenario_name,
            "ws_in_bytes": self.total_ws_in,
            "ws_out_bytes": self.total_ws_out,
            "frames": self.frame_idx,
            "output_sha256": f"sha256:{sha256_hex(output)}",
            "checksum_chain": f"sha256:{self.checksum_chain}",
        }

    def close(self):
        if self.jsonl_file:
            self.jsonl_file.close()
            self.jsonl_file = None

    def _timestamp(self) -> str:
        if os.environ.get("E2E_DETERMINISTIC", "1") == "1":
            step_ms = int(os.environ.get("E2E_TIME_STEP_MS", "100"))
            ts = self.frame_idx * step_ms
            return f"T{ts:06d}"
        return time.strftime("%Y-%m-%dT%H:%M:%S%z")


async def run_session(url: str, scenario: dict, recorder: SessionRecorder,
                      golden_path: str | None = None) -> dict:
    """Execute a scripted WebSocket session."""
    timeout_s = scenario.get("timeout_s", 30)
    steps = scenario.get("steps", [])

    recorder.emit("env", {
        "git_commit": git_sha(),
        "git_dirty": False,
        "scenario": scenario["name"],
        "initial_cols": scenario.get("initial_cols", 120),
        "initial_rows": scenario.get("initial_rows", 40),
    })
    recorder.emit("run_start", {
        "scenario": scenario["name"],
        "step_count": len(steps),
        "timeout_s": timeout_s,
    })

    result = {"outcome": "pass", "errors": []}

    try:
        async with websockets.connect(
            url,
            max_size=256 * 1024,
            open_timeout=10,
            close_timeout=5,
        ) as ws:
            # Background reader task.
            read_task = asyncio.create_task(_read_loop(ws, recorder))

            for i, step in enumerate(steps):
                step_type = step["type"]
                delay_ms = step.get("delay_ms", 0)

                if delay_ms > 0:
                    await asyncio.sleep(delay_ms / 1000.0)

                if step_type == "send":
                    data = _decode_step_data(step)
                    await ws.send(data)
                    recorder.record_send(data)
                    recorder.emit("input", {
                        "step": i,
                        "bytes": len(data),
                        "input_hash": f"sha256:{sha256_hex(data)[:16]}",
                    })

                elif step_type == "resize":
                    cols = step["cols"]
                    rows = step["rows"]
                    msg = json.dumps({"type": "resize", "cols": cols, "rows": rows})
                    await ws.send(msg)
                    recorder.emit("resize", {
                        "step": i,
                        "cols": cols,
                        "rows": rows,
                    })

                elif step_type == "wait":
                    wait_ms = step.get("ms", 100)
                    await asyncio.sleep(wait_ms / 1000.0)

                elif step_type == "drain":
                    # Wait for output to settle.
                    await asyncio.sleep(0.5)

            # Give a final drain period.
            await asyncio.sleep(0.3)
            read_task.cancel()
            try:
                await read_task
            except asyncio.CancelledError:
                pass

    except Exception as e:
        result["outcome"] = "fail"
        result["errors"].append(str(e))
        recorder.emit("error", {"message": str(e)})

    # Compute summary.
    summary = recorder.summary()
    result.update(summary)

    # Golden transcript comparison.
    if golden_path and os.path.exists(golden_path):
        with open(golden_path, "r") as f:
            golden = json.load(f)
        expected_checksum = golden.get("checksum_chain", "")
        if expected_checksum and expected_checksum != summary["checksum_chain"]:
            result["outcome"] = "fail"
            result["errors"].append(
                f"Golden checksum mismatch: expected {expected_checksum}, "
                f"got {summary['checksum_chain']}"
            )
            recorder.emit("golden_mismatch", {
                "expected": expected_checksum,
                "actual": summary["checksum_chain"],
                "frames_expected": golden.get("frames", -1),
                "frames_actual": summary["frames"],
            })
        else:
            recorder.emit("golden_match", {
                "checksum": summary["checksum_chain"],
                "frames": summary["frames"],
            })

    recorder.emit("run_end", {
        "outcome": result["outcome"],
        "ws_in_bytes": summary["ws_in_bytes"],
        "ws_out_bytes": summary["ws_out_bytes"],
        "frames": summary["frames"],
        "output_sha256": summary["output_sha256"],
        "checksum_chain": summary["checksum_chain"],
    })

    return result


async def _read_loop(ws, recorder: SessionRecorder):
    """Background task to read WebSocket output."""
    try:
        async for message in ws:
            if isinstance(message, bytes):
                recorder.record_output(message)
    except websockets.exceptions.ConnectionClosed:
        pass


def _decode_step_data(step: dict) -> bytes:
    """Decode step data from hex, base64, or utf-8."""
    if "data_hex" in step:
        return bytes.fromhex(step["data_hex"])
    if "data_b64" in step:
        return base64.b64decode(step["data_b64"])
    if "data" in step:
        return step["data"].encode("utf-8")
    return b""


def save_transcript(output: bytes, path: str):
    """Save raw output as a transcript file."""
    with open(path, "wb") as f:
        f.write(output)


def main():
    parser = argparse.ArgumentParser(description="WebSocket remote terminal client")
    parser.add_argument("--url", default="ws://127.0.0.1:9231", help="Bridge URL")
    parser.add_argument("--scenario", required=True, help="Scenario JSON file")
    parser.add_argument("--golden", default=None, help="Golden transcript JSON")
    parser.add_argument("--jsonl", default=None, help="JSONL output file")
    parser.add_argument("--transcript", default=None, help="Save raw output transcript")
    parser.add_argument("--summary", action="store_true", help="Print summary JSON to stdout")
    args = parser.parse_args()

    with open(args.scenario, "r") as f:
        scenario = json.load(f)

    seed = int(os.environ.get("E2E_SEED", "0"))
    run_id = make_run_id(seed)
    recorder = SessionRecorder(run_id, scenario["name"], args.jsonl)

    try:
        result = asyncio.run(run_session(args.url, scenario, recorder, args.golden))
    finally:
        recorder.close()

    if args.transcript:
        save_transcript(recorder.full_output(), args.transcript)

    if args.summary or not args.jsonl:
        print(json.dumps(result, indent=2))

    sys.exit(0 if result["outcome"] == "pass" else 1)


if __name__ == "__main__":
    main()
