# Architecture decisions

This directory is the canonical home for durable Architecture Decision Records (ADRs).

## Decision index

| ID | Title | Status | Date | Supersedes |
|---|---|---|---|---|
| [ADR-0001](adr/0001-s3-language-surface.md) | Adopt the S3 language surface | accepted | 2026-07-12 | — |
| [ADR-0002](adr/0002-arc-single-cascade-owner.md) | Registered ARC edges are the single cascade owner | accepted | 2026-07-17 | — |
| [ADR-0003](adr/0003-defer-copy-on-write-collections.md) | Defer copy-on-write collection semantics | accepted | 2026-07-18 | — |
| [ADR-0004](adr/0004-repository-and-project-layout.md) | Keep the Cargo workspace under `compiler/` and use root-first Ori projects | accepted | 2026-07-13 | — |
| [ADR-0005](adr/0005-deprecate-and-retire-c-backend.md) | Retire the C backend; retain the native reference pipeline | accepted | 2026-09-05 | — |
| [ADR-0006](adr/0006-selfhost-modular-architecture.md) | Modular architecture, strict boundaries, and self-host compiler decomposition for Marco B | proposed | 2026-09-07 | — |

## What belongs in an ADR

Use an ADR when a decision:

- changes or establishes a long-lived component boundary;
- defines ownership, lifecycle, ABI, storage, package, or target behavior;
- selects between meaningful alternatives;
- constrains future implementation;
- would be difficult to infer from code alone;
- is likely to be revisited without a written rationale.

Do not use an ADR for ordinary implementation steps, task tracking, meeting notes, or a list of future ideas.

## Directory

```text
docs/decisions/
├── README.md
├── TEMPLATE.md
└── adr/
    ├── 0001-s3-language-surface.md
    ├── 0002-arc-single-cascade-owner.md
    ├── 0003-defer-copy-on-write-collections.md
    └── 0004-repository-and-project-layout.md
```

Existing accepted decisions still located under `docs/planning/` should be migrated by:

1. preserving the actual historical context and rationale;
2. adding normalized metadata;
3. assigning the next ADR number;
4. connecting current architecture/specification;
5. replacing the former path with a compatibility pointer;
6. updating inbound links and this index.

## Status values

- `proposed`
- `accepted`
- `rejected`
- `deprecated`
- `superseded`

A superseded ADR remains in the repository and links to its replacement.

A decision to defer or reject a design may still use `accepted` when the accepted decision is explicitly “do not adopt under the current conditions.” The title and consequences must make that clear.

## Required content

- context;
- decision drivers;
- alternatives considered;
- decision;
- consequences;
- invariants established;
- affected contracts and code;
- validation/evidence;
- compatibility/migration;
- status and date;
- reconsideration criteria;
- superseding/superseded relationships.

Use [`TEMPLATE.md`](TEMPLATE.md).

## ADR versus architecture

The ADR explains why a choice was made. Current architecture documents explain how the system works now.

When implementation evolves without changing the decision, update architecture only. When the decision itself changes, add a new ADR and supersede the old one.

## ADR versus RFC

An RFC evaluates a significant public proposal. An ADR records a durable system decision. A language RFC may result in ADRs for parser, runtime, ABI, or repository design.

## ADR versus plan

An ADR establishes a durable choice. An ExecPlan sequences implementation. Completing a plan does not make the plan an ADR.

## Index policy

The index must be updated in the same PR that adds, supersedes, rejects, or deprecates an ADR.