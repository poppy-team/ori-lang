---
id: ADR-0006
title: Modular architecture, strict boundaries, and self-host compiler decomposition for Marco B
status: proposed
date: 2026-09-07
deciders: [Raillen, Ori Core Contributors]
supersedes: []
superseded_by: []
related_docs:
  - docs/ATLAS.md
  - docs/architecture/compiler-pipeline.md
  - docs/implementation/standards.md
  - docs/plans/active/selfhost-exec-plan.md
related_code:
  - compiler/crates/ori-lexer
  - compiler/crates/ori-parser
  - compiler/crates/ori-types
  - compiler/crates/ori-hir
  - compiler/crates/ori-codegen
  - packages/compiler
---

# ADR-0006: Modular architecture, strict boundaries, and self-host compiler decomposition for Marco B

## Context

The initial Rust implementation of the Ori compiler accumulated massive monolith files:
- `native_backend.rs` (~21,150 lines): handles layout, SSA lowering, Cranelift invocation, ARC emissions, vtables, and C ABI interop in a single compilation unit.
- `check.rs` (~10,980 lines): couples bidirectional type inference, unification, pattern exhaustiveness, trait validation, and expression checking.
- `lower.rs` (~6,110 lines): AST to HIR lowering with interleaved desugaring and AST mutation.

Marco B defines the initial implementation of the compiler's frontend, intermediate representation, and host bridge directly in Ori (`packages/compiler`). Translating the existing monolithic Rust code directly into Ori would reproduce technical debt, degrade compilation times, and make incremental verification and maintenance unsustainable.

A durable architectural boundary is required to enforce Clean Code, High Cohesion, Low Coupling (SRP, Ports & Adapters / Hexagonal Architecture), and deterministic dataflow across all compiler components.

## Decision drivers

- **Single Responsibility Principle (SRP)**: Each module/file must own exactly one language construct or transformation domain, capped at ~500–800 lines.
- **Pure Functional Pipeline**: Elimination of global mutable states, ambient queries, or bidirectional cyclic dependencies between compilation stages.
- **Explicit Domain Newtypes**: Eradication of Primitive Obsession; zero raw integer IDs or unstructured tuples across phase boundaries.
- **Isolated I/O and Side Effects**: Compiler phases are pure in-memory transformations; file system access, terminal diagnostics, and process spawning are strictly confined to the driver layer.
- **Stable Versioned Protocol**: Clear separation of concerns between the Ori-based frontend/mid-end and the Rust-based Cranelift backend via a versioned, frame-bounded IPC contract (`CONTRACT01`).

## Considered options

### Option A: Monolithic Translation
Re-implement the Rust crates in Ori following the current layout (e.g., massive files per crate like `check.orl` and `codegen.orl`).
- *Strengths*: Direct 1:1 mapping with current Rust code.
- *Risks*: Extreme technical debt, huge files impossible to reason about, tight coupling between type checking and code generation, unmaintainable.

### Option B: Modular Architecture with Pure Stages and Isolated Bridge (Selected)
Decompose the self-host compiler into focused, single-purpose packages and submodules with clean boundaries:
1. **Frontend Core (`packages/compiler/frontend/`)**:
   - `lex/`: Low-level tokenization emitting flat token arrays with explicit spans.
   - `parse/`: Recursive descent parsing decomposed by syntactic family:
     - `parse/expr.orl`: Expression parsing and precedence climbing.
     - `parse/stmt.orl`: Statements, bindings, assignments.
     - `parse/item.orl`: Functions, structs, enums, traits, type aliases.
     - `parse/pat.orl`: Match and let patterns.
     - `parse/ty.orl`: Type annotations and signatures.
   - `resolve/`: Scoping, symbol interning, module discovery, and name resolution.
   - `types/`: Pure type checking and inference:
     - `infer.orl`: Bidirectional type inference.
     - `unify.orl`: Unification engine with occurs check.
     - `traits.orl`: Trait resolution and method dispatch verification.
     - `exhaustiveness.orl`: Pattern matching decision trees and exhaustiveness checks.
2. **Intermediate Representation (`packages/compiler/hir/`)**:
   - Explicit desugared representation (lowered loops, explicit ARC inc/dec sites, resolved dispatch).
   - Strict structural and semantic validation before emission.
3. **Bridge & Codegen Driver (`packages/compiler/bridge/` & `compiler/crates/ori-bridge-server`)**:
   - Versioned binary framing protocol (Length-Prefixed Framing + Schema Versioning).
   - Pure serialization/deserialization into stable intermediate byte buffers.
   - Cranelift native code generation kept in an isolated Rust worker process during early bootstrap stages.

## Decision

Adopt **Option B**. Every self-host compiler module in Ori must adhere to:
1. Pure unidirectional pipeline: `Source` -> `Tokens` -> `AST` -> `ResolvedAST` -> `HIR` -> `CodegenRequest`.
2. Hard size ceiling: max 600–800 lines per source file. If a file grows beyond 600 lines, it must be decomposed into a domain subfolder.
3. Decoupled Diagnostics: Compilation phases never print or abort directly. They return domain errors collected in an accumulator: `Result[T, list[Diagnostic]]`.
4. Boundary Ports: File system, network, and subprocess invocation cannot be imported into `parse`, `resolve`, `types`, or `hir`.

## Consequences

### Positive
- Independent unit testing for each syntactic and semantic domain without pulling the entire compiler stack.
- Predictable incremental compilation and lower cognitive load for contributors and agents.
- Clear contract boundary between Ori and Cranelift, allowing backends to be tested, mocked, or swapped.
- Elimination of circular dependencies and memory leaks caused by shared mutable environments.

### Negative
- Initial development requires writing structured boilerplate (DTOs, serializers, and explicit visitors).
- IPC boundary introduces a minor serialization overhead during the initial transition stages.

## Invariants established
- **No Phase Backtracking**: Later phases never mutate AST or tokens from previous phases.
- **Clean Separation of I/O**: Only the driver CLI (`ori-driver` / `main.orl`) may perform I/O.
- **Deterministic Diagnostics**: Phase output is a deterministic function of its inputs; identical source inputs produce identical diagnostics and ordering.

## Validation
- Unit tests per subfolder (`test/lex/`, `test/parse/`, `test/types/`).
- Round-trip IPC serialization tests for the bridge protocol.
- Clean code linters checking file line limits and cross-crate import bans.
