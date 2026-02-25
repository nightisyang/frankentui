# OpenTUI Transformation Policy Matrix

Bead: `bd-3bxhj.1.2`  
Policy artifact: `crates/doctor_frankentui/contracts/opentui_transformation_policy_v1.json`

## Purpose

Defines the deterministic handling class for every OpenTUI construct in migration scope:

- `exact`
- `approximate`
- `extend_ftui`
- `unsupported`

Each policy cell includes rationale, risk level, fallback behavior, and user messaging so migration outcomes are explicit and auditable.

## Contract Identity

- `policy_id`: `opentui-transform-policy-matrix`
- `schema_version`: `transform-policy-v1`
- `policy_version`: `2026-02-25`

## Coverage Requirements

The matrix is required to cover all construct categories:

- `state`
- `layout`
- `style`
- `effects`
- `accessibility`
- `terminal_capability`

No construct in `construct_catalog` may be missing from `policy_cells`.

## Determinism Requirements

Policy cells are sorted by `construct_signature` for deterministic consumption.

Planner and certification projections define required exported fields and enforce consistent sorting and risk-level reporting.

## Planner Consumption

`planner_projection` defines fields consumed by translator planning:

- construct signature and category
- handling class
- chosen planner strategy
- deterministic fallback behavior
- risk level

`TransformationPolicyMatrix::planner_rows()` materializes this projection.

## Certification Consumption

`certification_projection` defines fields consumed by certification report generation:

- construct signature
- handling class and risk level
- semantic clause links
- certification evidence
- user-facing messaging

`TransformationPolicyMatrix::certification_rows()` materializes this projection.

## Clause Traceability

Every policy cell links to one or more semantic equivalence clauses from:

- `crates/doctor_frankentui/contracts/opentui_semantic_equivalence_v1.json`

Validation fails if a policy cell references an unknown clause ID.
