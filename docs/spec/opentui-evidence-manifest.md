# OpenTUI Evidence Manifest Schema

> Canonical artifact manifest emitted by every migration run. Every field is deterministic and machine-readable.

## Schema Version

`evidence-manifest-v1`

## Purpose

The evidence manifest provides a complete, deterministic record of a migration run from source intake through certification. It serves three goals:

1. **Stability**: Identical inputs produce identical manifests (byte-for-byte).
2. **Structured stages**: Every pipeline stage writes JSONL records with correlation IDs.
3. **Replay lineage**: The manifest alone reconstructs the full stage chain.

## Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `manifest_id` | string | yes | Stable identifier for the manifest schema instance |
| `schema_version` | string | yes | Must be `evidence-manifest-v1` |
| `manifest_version` | string | yes | Date-stamped version of this manifest |
| `run_id` | string | yes | Unique run identifier (e.g., `intake_20260225_120000`) |
| `source_fingerprint` | object | yes | Immutable snapshot of the source project |
| `stages` | array | yes | Ordered pipeline stages with hash chain |
| `generated_code_fingerprint` | object | yes | Hash and tool versions for generated code |
| `certification_verdict` | object | yes | Final certification outcome |
| `determinism_attestation` | object | yes | Evidence of cross-run stability |

## Source Fingerprint

| Field | Type | Description |
|-------|------|-------------|
| `repo_url` | string? | Git remote URL (null for local-only) |
| `repo_commit` | string? | Resolved commit SHA |
| `local_path` | string? | Local filesystem path |
| `source_hash` | string | SHA-256 of the complete source tree |
| `lockfiles` | array | Per-lockfile path, SHA-256, and size |
| `parser_versions` | map | Tool name to version string |

At least one of `repo_url` or `local_path` must be present.

## Stage Record

Each stage in the pipeline is recorded with:

| Field | Type | Description |
|-------|------|-------------|
| `stage_id` | string | Unique stage name (e.g., `intake`, `extraction`) |
| `stage_index` | u32 | Zero-based consecutive index |
| `correlation_id` | string | `run:<run_id>:stage:<stage_id>` |
| `claim_id` | string | Semantic contract clause claim linked to this stage |
| `evidence_id` | string | Stable artifact-graph evidence node ID for this stage |
| `policy_id` | string | Transformation policy contract identifier (must be run-consistent) |
| `trace_id` | string | End-to-end trace identifier (must be run-consistent) |
| `started_at` | string | ISO 8601 timestamp |
| `finished_at` | string | ISO 8601 timestamp |
| `status` | enum | `ok`, `failed`, or `skipped` |
| `input_hash` | string | SHA-256 of stage inputs |
| `output_hash` | string | SHA-256 of stage outputs |
| `artifact_paths` | array | Relative paths to produced artifacts |
| `error` | string? | Error message (required when status=failed) |

### Hash Chain Invariant

For consecutive stages `[i]` and `[i+1]`, when stage `[i]` status is `ok`:

```
stages[i].output_hash == stages[i+1].input_hash
```

This enables replay tooling to verify lineage integrity.

## JSONL Evidence Records

Each stage emits a structured JSONL record on completion:

```json
{"event":"stage_completed","run_id":"...","correlation_id":"...","stage_id":"...","stage_index":0,"claim_id":"ST-001","evidence_id":"evidence:...","policy_id":"opentui-transform-policy-v1","trace_id":"trace:...","timestamp":"...","status":"ok","input_hash":"...","output_hash":"...","artifact_count":2,"error":null}
```

## Certification Verdict

| Field | Type | Description |
|-------|------|-------------|
| `verdict` | enum | `accept`, `hold`, `reject`, `rollback` |
| `confidence` | f64 | [0.0, 1.0] calibrated probability |
| `test_pass_count` | u32 | Tests that passed |
| `test_fail_count` | u32 | Tests that failed (must be 0 for `accept`) |
| `test_skip_count` | u32 | Tests skipped |
| `semantic_clause_coverage` | object | Covered/uncovered contract clause IDs |
| `benchmark_summary` | object | p50, p99 latency and throughput |
| `risk_flags` | array | Active risk identifiers |

## Determinism Attestation

| Field | Type | Description |
|-------|------|-------------|
| `identical_runs_count` | u32 | Number of identical-input runs performed (>0) |
| `manifest_hash_stable` | bool | All runs produced identical manifests |
| `divergence_detected` | bool | Any non-determinism observed (inconsistent with stable=true) |

## Validation Rules

1. `schema_version` must match `evidence-manifest-v1`.
2. `manifest_id` and `run_id` must be non-empty.
3. Source fingerprint must have a non-empty `source_hash` and at least one of `repo_url`/`local_path`.
4. Stages must be non-empty, consecutively indexed starting at 0, with unique `stage_id` values.
5. Every stage must emit non-empty `claim_id`, `evidence_id`, `policy_id`, and `trace_id`.
6. `evidence_id` values must be unique within a manifest; `policy_id` and `trace_id` must be consistent across all stages.
7. Every stage `claim_id` must appear in semantic clause coverage `covered` (stage evidence may not point at `uncovered` claims).
8. Every `covered` claim must have at least one linked stage record; no orphan covered claims are allowed.
9. Hash chain must be unbroken between consecutive `ok` stages.
10. Failed stages must include an error message.
11. Certification confidence must be in [0.0, 1.0]; `accept` requires zero test failures.
12. Determinism attestation runs must be >0; divergence=true is incompatible with stable=true.
