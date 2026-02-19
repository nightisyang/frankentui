#!/usr/bin/env python3
"""Cross-browser differential checker for remote resize-storm JSONL traces.

Compares deterministic resize/input/frame semantics across browser-labeled traces,
reports actionable diffs, and classifies known/unknown divergence classes.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class TraceInput:
    browser: str
    path: Path


@dataclass(frozen=True)
class KnownDivergence:
    class_pattern: str
    left_browser_pattern: str
    right_browser_pattern: str
    path_pattern: str
    rationale: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare resize-storm JSONL traces across browser runs."
    )
    parser.add_argument(
        "--trace",
        action="append",
        default=[],
        metavar="BROWSER=PATH",
        help="Trace input mapping (repeat for each browser, min 2)",
    )
    parser.add_argument(
        "--baseline",
        default="",
        help="Baseline browser label (default: first --trace entry)",
    )
    parser.add_argument(
        "--known",
        default="",
        help="Known divergences TSV (class\\tleft\\tright\\tpath_pattern\\trationale)",
    )
    parser.add_argument(
        "--report",
        required=True,
        help="Output report path (JSON)",
    )
    parser.add_argument(
        "--mode",
        choices=("warn", "strict"),
        default="warn",
        help="Fail behavior for unknown divergences",
    )
    parser.add_argument(
        "--max-printed-unknown",
        type=int,
        default=20,
        help="Limit unknown diff lines printed to stdout",
    )
    return parser.parse_args()


def parse_trace_args(raw_items: list[str]) -> list[TraceInput]:
    traces: list[TraceInput] = []
    for raw in raw_items:
        if "=" not in raw:
            raise ValueError(f"invalid --trace value (expected BROWSER=PATH): {raw}")
        browser_raw, path_raw = raw.split("=", 1)
        browser = browser_raw.strip()
        path = Path(path_raw.strip())
        if not browser:
            raise ValueError(f"empty browser label in --trace: {raw}")
        traces.append(TraceInput(browser=browser, path=path))
    if len(traces) < 2:
        raise ValueError("at least two --trace inputs are required")
    dedup = {trace.browser for trace in traces}
    if len(dedup) != len(traces):
        raise ValueError("duplicate browser labels in --trace inputs are not allowed")
    return traces


def _as_int(value: Any, default: int = -1) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def _as_str(value: Any) -> str:
    if isinstance(value, str):
        return value
    return ""


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        raise FileNotFoundError(f"trace not found: {path}")
    lines = path.read_text(encoding="utf-8").splitlines()
    events: list[dict[str, Any]] = []
    for idx, line in enumerate(lines, start=1):
        raw = line.strip()
        if not raw:
            continue
        try:
            obj = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise ValueError(f"{path}:{idx}: invalid JSONL entry: {exc}") from exc
        if not isinstance(obj, dict):
            raise ValueError(f"{path}:{idx}: expected JSON object per line")
        events.append(obj)
    if not events:
        raise ValueError(f"{path}: JSONL is empty")
    return events


def extract_signature(events: list[dict[str, Any]]) -> dict[str, Any]:
    run_end = None
    for event in events:
        if event.get("type") == "run_end":
            run_end = event
    if run_end is None:
        raise ValueError("missing run_end event")

    resize_inputs: list[dict[str, Any]] = []
    frames: list[dict[str, Any]] = []
    for event in events:
        if event.get("type") == "input" and event.get("input_type") == "resize":
            resize_inputs.append(
                {
                    "cols": _as_int(event.get("cols")),
                    "rows": _as_int(event.get("rows")),
                    "hash_key": _as_str(event.get("hash_key")),
                }
            )
        elif event.get("type") == "frame":
            frames.append(
                {
                    "cols": _as_int(event.get("cols")),
                    "rows": _as_int(event.get("rows")),
                    "frame_hash": _as_str(event.get("frame_hash")),
                    "hash_key": _as_str(event.get("hash_key")),
                }
            )

    geometry_sequence: list[str] = []
    for frame in frames:
        geom = f"{frame['cols']}x{frame['rows']}"
        if not geometry_sequence or geometry_sequence[-1] != geom:
            geometry_sequence.append(geom)

    return {
        "run_end": {
            "status": _as_str(run_end.get("status")),
            "outcome": _as_str(run_end.get("outcome")),
            "frames": _as_int(run_end.get("frames")),
            "checksum_chain": _as_str(run_end.get("checksum_chain")),
            "output_sha256": _as_str(run_end.get("output_sha256")),
        },
        "resize_inputs": resize_inputs,
        "frames": frames,
        "geometry_sequence": geometry_sequence,
    }


def _append_diff(
    diffs: list[dict[str, Any]],
    class_name: str,
    path: str,
    expected: Any,
    actual: Any,
    details: str,
) -> None:
    diffs.append(
        {
            "class": class_name,
            "path": path,
            "expected": expected,
            "actual": actual,
            "details": details,
        }
    )


def compare_signatures(
    baseline_sig: dict[str, Any],
    target_sig: dict[str, Any],
) -> list[dict[str, Any]]:
    diffs: list[dict[str, Any]] = []

    for field in ("status", "outcome", "frames"):
        expected = baseline_sig["run_end"].get(field)
        actual = target_sig["run_end"].get(field)
        if expected != actual:
            _append_diff(
                diffs,
                "run_end_mismatch",
                f"run_end.{field}",
                expected,
                actual,
                "run_end field diverged",
            )

    baseline_resize = baseline_sig["resize_inputs"]
    target_resize = target_sig["resize_inputs"]
    if len(baseline_resize) != len(target_resize):
        _append_diff(
            diffs,
            "resize_inputs_length",
            "resize_inputs.length",
            len(baseline_resize),
            len(target_resize),
            "resize input event count diverged",
        )
    for idx in range(min(len(baseline_resize), len(target_resize))):
        left = baseline_resize[idx]
        right = target_resize[idx]
        if (left["cols"], left["rows"]) != (right["cols"], right["rows"]):
            _append_diff(
                diffs,
                "resize_input_geometry",
                f"resize_inputs[{idx}].cols_rows",
                f"{left['cols']}x{left['rows']}",
                f"{right['cols']}x{right['rows']}",
                "resize geometry diverged",
            )
        if left["hash_key"] != right["hash_key"]:
            _append_diff(
                diffs,
                "resize_input_hash_key",
                f"resize_inputs[{idx}].hash_key",
                left["hash_key"],
                right["hash_key"],
                "resize hash_key diverged",
            )

    baseline_geom = baseline_sig["geometry_sequence"]
    target_geom = target_sig["geometry_sequence"]
    if len(baseline_geom) != len(target_geom):
        _append_diff(
            diffs,
            "geometry_sequence_length",
            "geometry_sequence.length",
            len(baseline_geom),
            len(target_geom),
            "unique frame geometry sequence length diverged",
        )
    for idx in range(min(len(baseline_geom), len(target_geom))):
        if baseline_geom[idx] != target_geom[idx]:
            _append_diff(
                diffs,
                "geometry_sequence_value",
                f"geometry_sequence[{idx}]",
                baseline_geom[idx],
                target_geom[idx],
                "unique frame geometry diverged",
            )

    baseline_frames = baseline_sig["frames"]
    target_frames = target_sig["frames"]
    if len(baseline_frames) != len(target_frames):
        _append_diff(
            diffs,
            "frame_count",
            "frames.length",
            len(baseline_frames),
            len(target_frames),
            "frame event count diverged",
        )

    if baseline_frames and target_frames:
        baseline_last = baseline_frames[-1]
        target_last = target_frames[-1]
        if (baseline_last["cols"], baseline_last["rows"]) != (
            target_last["cols"],
            target_last["rows"],
        ):
            _append_diff(
                diffs,
                "final_frame_geometry",
                "frames[-1].cols_rows",
                f"{baseline_last['cols']}x{baseline_last['rows']}",
                f"{target_last['cols']}x{target_last['rows']}",
                "final frame geometry diverged",
            )
        if baseline_last["hash_key"] != target_last["hash_key"]:
            _append_diff(
                diffs,
                "final_frame_hash_key",
                "frames[-1].hash_key",
                baseline_last["hash_key"],
                target_last["hash_key"],
                "final frame hash_key diverged",
            )

    return diffs


def parse_known_divergences(path: Path | None) -> list[KnownDivergence]:
    if path is None:
        return []
    if not path.exists():
        raise FileNotFoundError(f"known divergences file not found: {path}")

    rules: list[KnownDivergence] = []
    for idx, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        raw = line.strip()
        if not raw or raw.startswith("#"):
            continue
        parts = raw.split("\t")
        if len(parts) < 5:
            raise ValueError(
                f"{path}:{idx}: expected 5 tab-separated fields "
                "(class, left, right, path_pattern, rationale)"
            )
        rules.append(
            KnownDivergence(
                class_pattern=parts[0].strip() or "*",
                left_browser_pattern=parts[1].strip() or "*",
                right_browser_pattern=parts[2].strip() or "*",
                path_pattern=parts[3].strip() or "*",
                rationale=parts[4].strip(),
            )
        )
    return rules


def _match_pair(rule: KnownDivergence, left: str, right: str) -> bool:
    forward = (
        fnmatch.fnmatchcase(left, rule.left_browser_pattern)
        and fnmatch.fnmatchcase(right, rule.right_browser_pattern)
    )
    reverse = (
        fnmatch.fnmatchcase(left, rule.right_browser_pattern)
        and fnmatch.fnmatchcase(right, rule.left_browser_pattern)
    )
    return forward or reverse


def classify_diffs(
    diffs: list[dict[str, Any]],
    baseline_browser: str,
    target_browser: str,
    rules: list[KnownDivergence],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    known: list[dict[str, Any]] = []
    unknown: list[dict[str, Any]] = []
    for diff in diffs:
        matched_rule: KnownDivergence | None = None
        for rule in rules:
            if not fnmatch.fnmatchcase(diff["class"], rule.class_pattern):
                continue
            if not fnmatch.fnmatchcase(diff["path"], rule.path_pattern):
                continue
            if not _match_pair(rule, baseline_browser, target_browser):
                continue
            matched_rule = rule
            break

        diff_with_context = {
            **diff,
            "baseline_browser": baseline_browser,
            "target_browser": target_browser,
        }
        if matched_rule is None:
            unknown.append(diff_with_context)
        else:
            known.append(
                {
                    **diff_with_context,
                    "known_rationale": matched_rule.rationale,
                }
            )
    return known, unknown


def main() -> int:
    args = parse_args()
    try:
        traces = parse_trace_args(args.trace)
        baseline_browser = args.baseline.strip() or traces[0].browser
        trace_map = {trace.browser: trace.path for trace in traces}
        if baseline_browser not in trace_map:
            raise ValueError(f"baseline browser '{baseline_browser}' not found in --trace set")

        known_rules = parse_known_divergences(Path(args.known) if args.known else None)

        signatures: dict[str, dict[str, Any]] = {}
        for browser, path in trace_map.items():
            events = load_jsonl(path)
            signatures[browser] = extract_signature(events)

        comparisons: list[dict[str, Any]] = []
        total_known = 0
        total_unknown = 0
        total_diffs = 0

        for browser in trace_map:
            if browser == baseline_browser:
                continue
            raw_diffs = compare_signatures(signatures[baseline_browser], signatures[browser])
            known, unknown = classify_diffs(raw_diffs, baseline_browser, browser, known_rules)
            total_known += len(known)
            total_unknown += len(unknown)
            total_diffs += len(raw_diffs)
            comparisons.append(
                {
                    "baseline_browser": baseline_browser,
                    "target_browser": browser,
                    "total_diffs": len(raw_diffs),
                    "known_diffs": known,
                    "unknown_diffs": unknown,
                    "status": "pass" if not unknown else ("warn" if args.mode == "warn" else "fail"),
                }
            )

        overall_status = "pass"
        if total_unknown > 0:
            overall_status = "warn" if args.mode == "warn" else "fail"

        report = {
            "suite": "remote_resize_storm_cross_browser_diff",
            "mode": args.mode,
            "baseline_browser": baseline_browser,
            "trace_inputs": {browser: str(path) for browser, path in trace_map.items()},
            "known_divergence_rules_loaded": len(known_rules),
            "summary": {
                "total_comparisons": len(comparisons),
                "total_diffs": total_diffs,
                "known_diffs": total_known,
                "unknown_diffs": total_unknown,
                "status": overall_status,
            },
            "comparisons": comparisons,
        }

        report_path = Path(args.report)
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

        print(
            f"[DIFF] baseline={baseline_browser} comparisons={len(comparisons)} "
            f"known={total_known} unknown={total_unknown} mode={args.mode}"
        )
        printed = 0
        if total_unknown > 0:
            print("[DIFF] Unknown divergences:")
            for cmp_result in comparisons:
                for diff in cmp_result["unknown_diffs"]:
                    if printed >= args.max_printed_unknown:
                        print(
                            f"[DIFF] ... truncated unknown divergence output at {args.max_printed_unknown} entries"
                        )
                        break
                    print(
                        "  - "
                        f"{diff['baseline_browser']} vs {diff['target_browser']} "
                        f"{diff['class']} {diff['path']} "
                        f"expected={diff['expected']!r} actual={diff['actual']!r}"
                    )
                    printed += 1
                if printed >= args.max_printed_unknown:
                    break
        print(f"[DIFF] Report: {report_path}")

        if overall_status == "fail":
            return 1
        return 0
    except Exception as exc:  # pragma: no cover - guardrail
        print(f"[DIFF][ERROR] {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
