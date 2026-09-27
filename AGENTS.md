# Ori operational rules

Ori is a reading-first, explicitly typed native programming language. The current canonical version is **0.3.8**.

This file is the mandatory operational manual for contributors and AI agents. Detailed knowledge lives in canonical documents linked from [`docs/ATLAS.md`](docs/ATLAS.md).

## Start here

Read, in order:

1. [`PROJECT_START.md`](PROJECT_START.md)
2. [`docs/ATLAS.md`](docs/ATLAS.md)
3. the canonical product, architecture, specification, implementation, quality, security, and operations documents for the task
4. relevant accepted ADRs/RFCs
5. active plan only when the change is complex and already planned

Do not use archived plans as current instructions.

## Source-of-truth order

1. Security, data integrity, and vulnerability requirements
2. Accepted product requirements and compatibility policy
3. Normative specification and native ABI contracts
4. Accepted ADRs and RFCs
5. Current architecture documents
6. This file and implementation standards
7. Automated conformance and regression tests
8. Current implementation
9. Active plans and backlog
10. Archived material

When implementation contradicts a higher contract, classify the conflict instead of silently copying accidental behavior into the docs.

## Mandatory rules

- Keep current project/compiler/workspace version references at **0.3.8**.
- Use current S3 syntax in active examples and diagnostics.
- One subject has one canonical document.
- Architecture describes current implementation; ADRs explain decisions; RFCs evaluate proposals; plans sequence complex accepted work.
- Public behavior changes update specification, tests, user docs, and changelog in the same PR.
- Bugs require regression tests.
- New diagnostics require catalog entries and negative tests.
- Runtime/stdlib changes keep semantic signatures, ABI metadata, exported symbols, docs, LSP, and tests synchronized.
- The native backend is the semantic reference.
- The Rust compiler on `main` remains the product implementation through Ori
  1.0; self-hosting is experimental outside `main` under ADR-0006 and does not
  gate the 1.0 release.
- Unsupported native behavior must reject explicitly according to the support matrix.
- AOT and JIT must agree on their shared support surface.
- Do not weaken a quality gate to make a change pass without an explicit recorded decision.
- Do not add product-specific frameworks or external ecosystems to the core repository without an explicit decision.

## Implementation standards

Follow [`docs/implementation/standards.md`](docs/implementation/standards.md).

In particular:

- work in the earliest phase with enough information;
- use focused phase inputs and outputs;
- keep orchestration façades small;
- group state by responsibility;
- prefer typed domain errors internally;
- keep Rust visibility narrow;
- isolate `unsafe` at FFI/ABI boundaries;
- preserve exported symbols and layouts during refactoring;
- use declarative shared catalogs where duplication causes drift;
- avoid generic `utils` dumping grounds;
- measure before optimizing;
- separate structural refactoring from semantic changes.

## Language-feature path

Inspect all applicable layers:

```text
lexer -> AST -> parser -> resolution/signatures -> type checker
-> HIR -> optimization -> AOT/JIT -> runtime/ABI
-> formatter -> LSP/editor -> diagnostics -> tests -> spec -> changelog
```

Use [`docs/implementation/feature-delivery.md`](docs/implementation/feature-delivery.md).

## Runtime and unsafe code

Before changing runtime, ARC, collections, async, I/O, network, FFI, or native layouts, read:

- [`docs/architecture/runtime-and-memory.md`](docs/architecture/runtime-and-memory.md)
- [`docs/architecture/invariants.md`](docs/architecture/invariants.md)
- [`docs/security/unsafe-code-policy.md`](docs/security/unsafe-code-policy.md)
- `docs/spec/10-memory.md`
- `docs/spec/16-runtime-ffi-safety.md`
- `docs/spec/19-abi.md`

Preserve the single-cascade-owner rule: registered ARC edges own child release; destructors must not release the same edge-owned children.

Runtime source changes require staticlib and cdylib staging/parity review.

## Standard library

Before changing stdlib, read:

- [`docs/architecture/stdlib.md`](docs/architecture/stdlib.md)
- `docs/spec/12-stdlib.md`
- `docs/spec/15-stdlib-maintenance.md`
- `stdlib/README.md`

Do not create parallel hardcoded catalogs downstream of the canonical manifest.

## Commands

Run from repository root:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test diagnostic_catalog
cargo --manifest-path compiler/Cargo.toml test -p ori-lsp
```

Fast gate:

```bash
sh tools/qa/daily_fast.sh
```

Runtime staging:

```bash
sh tools/stage_native_runtime.sh
```

Windows:

```powershell
.\tools\stage_native_runtime.ps1
```

Use [`docs/operations/development.md`](docs/operations/development.md) for focused routes and troubleshooting.

## Required evidence

A PR must state:

- problem and delivered behavior;
- scope and exclusions;
- affected contracts/components;
- tests and commands executed;
- compatibility and migration impact;
- security and performance impact;
- runtime/ABI/target impact when applicable;
- documentation updated;
- residual risk and deliberately deferred work.

## ADR, RFC, and plan rules

Create an ADR for a durable architecture or implementation decision.

Create an RFC for a significant language, ABI, project/package, stdlib-direction, or ecosystem proposal.

Create an ExecPlan only for complex accepted work that spans several PRs, requires staged migration, or carries substantial refactoring risk.

Small fixes and features should stay in focused issues and PRs.

## PR policy

- One coherent vertical outcome per PR.
- Avoid unrelated cleanup.
- Add characterization tests before risky refactoring.
- Preserve public APIs, diagnostics, symbols, layouts, and semantics unless the PR explicitly changes their contract.
- Keep completed plans and backlog state synchronized.
- Do not merge with unexplained failing gates.

## Documentation policy

- Start from `docs/ATLAS.md`.
- Update the canonical source, then link to it.
- Add new canonical docs to `docs/catalog.yaml` and the ATLAS.
- Active docs must not depend on archive instructions.
- User-facing EN/PT siblings are updated together where that parallel is maintained.
- Normative specification remains English-only.
- Historical files move to `docs/archive/` by category.

## Definition of done

A change is done when code, tests, diagnostics, specification, architecture, user documentation, compatibility, security, performance, operations, and planning state agree for the delivered scope.
