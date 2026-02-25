# OpenTUI Migration Semantic Equivalence Contract

Bead: `bd-3bxhj.1.1`  
Contract artifact: `crates/doctor_frankentui/contracts/opentui_semantic_equivalence_v1.json`

## Purpose

Defines the normative contract for OpenTUI -> FrankenTUI migrations:

- what must remain semantically equivalent
- what may improve without violating intent
- how deterministic tie-breaks are applied when multiple translation strategies are viable
- how validators map findings to explicit clause IDs

## Contract Identity

- `contract_id`: `opentui-migration-semantic-equivalence`
- `schema_version`: `sem-eq-contract-v1`
- `contract_version`: `2026-02-25`

The JSON artifact is machine-readable, versioned, and intended as the authoritative source.

## Normative Axes

- `state_transition`
- `event_ordering`
- `side_effect_observability`

These axes are represented by explicit clause IDs in the contract JSON and consumed by validators via `validator_clause_map`.

## Visual Tolerance Classes

- Strict classes: exact canonical equivalence required.
- Perceptual classes: bounded perceptual delta permitted (`max_perceptual_delta`) as long as readability and interaction semantics remain intact.

## Improvement Envelope

Allowed dimensions include performance and accessibility improvements with explicit evidence.

Forbidden rewrites are hard-gated by policy unless an explicit exemption artifact exists.

## Deterministic Tie-Break Rules

Priority order is explicit and deterministic:

1. Minimize semantic regression risk
2. Maximize user value
3. Minimize runtime cost
4. Stable lexical strategy ID tie-break

No hidden randomness is allowed in candidate selection.

## Validator Traceability

`validator_clause_map` binds validator IDs to clause IDs so every finding can be mapped directly to one or more normative clauses.

This mapping is intended for:

- contract parsers/linters
- semantic diff and replay validators
- visual and improvement policy validators
- release gate reasoning and diagnostics
