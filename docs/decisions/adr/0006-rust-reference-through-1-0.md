---
id: ADR-0006
title: Keep the Rust compiler as the product reference through Ori 1.0
status: accepted
date: 2026-09-27
deciders: [Ori maintainers]
supersedes: []
superseded_by: []
related_docs:
  - docs/product/status.md
  - docs/planning/BACKLOG.md
  - docs/spec/14-backend-support.md
  - docs/spec/18-stability-and-compatibility.md
  - docs/spec/19-abi.md
related_code:
  - compiler/crates/ori-driver
  - compiler/crates/ori-types
  - compiler/crates/ori-hir
  - compiler/crates/ori-codegen
  - compiler/crates/ori-runtime
---

# ADR-0006: Keep the Rust compiler as the product reference through Ori 1.0

## Context

`main` already contains the Rust compiler and is the repository default branch.
The experimental Ori-written compiler lives outside `main`, in
`feat/selfhost-marco-b` and draft PR #14. Its Stage 1 can compile a bounded
subset, but cannot compile `selfhost/compiler/main.orl` into Stage 2. Its host
bridge still uses Rust native code generation. Bootstrap, full conformance,
packaging, and product parity are therefore unproven.

Maintaining two evolving frontends while stabilizing the language, runtime,
tooling, and native ABI adds duplicate semantic and regression work. The
public value of Ori 1.0 depends on reliable language contracts and tools, not
on the implementation language of the compiler.

## Decision drivers

- Keep correctness, runtime/ABI safety, conformance, and package reliability
  ahead of implementation-language migration.
- Preserve the experimental work and its useful differential tests without
  presenting it as a complete or supported compiler.
- Make the Rust route and the 1.0 release gate unambiguous for contributors.

## Considered options

1. Complete self-hosting on the product critical path now. This duplicates
   ongoing language work and delays stabilization while Stage 2 is blocked.
2. Discard the experiment entirely. This loses the prototype and its value
   as a stress test of Ori's current language surface.
3. Keep Rust as the product compiler through 1.0 and preserve self-hosting as
   an isolated experiment (selected).

## Decision

- `main` remains Rust-based and is the **only product compiler implementation
  and semantic reference through Ori 1.0**. Native AOT is the reference
  execution route; JIT agrees on its documented shared surface.
- The Ori-written compiler remains experimental outside `main`. Its branch,
  tests, and bootstrap failure must be described honestly. Experimental CI
  continues to check real Stage 0 → 1 → 2 → 3 progress where that experiment
  is developed; a failing experiment is not made green by removing assertions.
- Do not merge the incomplete self-host implementation into `main`, make it a
  required 1.0 gate, or retire Rust as part of the 1.0 release.
- Independent, validated Rust compiler/runtime improvements discovered on an
  experimental branch may be delivered as focused PRs against `main`, with
  their own contracts and tests. Do not merge the entire experimental branch
  to obtain such a fix.
- After the Rust-based 1.0 release, reassess whether continued self-hosting
  provides enough maintenance, educational, or product benefit to justify
  parity work. Reassessment does **not** automatically replace Rust.

## Consequences

### Positive

- Language and runtime improvements have one supported compiler path.
- The 1.0 definition of done can focus on conformance, safety, support,
  documentation, performance evidence, and release reliability.
- Experimental self-host work is preserved for later study and validation.

### Negative

- Building Ori from source continues to require Rust through 1.0.
- The self-host prototype may need rebaselining after product semantics settle.
- A completed self-host compiler is not promised for the 1.0 release.

## Invariants established

- Rust code in `compiler/` owns production frontend, HIR, native AOT/JIT,
  runtime, CLI, and tooling implementation through 1.0.
- The native ABI remains independently versioned as `ori-native-abi-1` until a
  separately approved incompatible change.
- An experimental bootstrap result does not certify product parity or 1.0
  readiness; unsupported shapes reject explicitly.
- Preserved unmerged work is reviewed before its branch is deleted.

## Affected contracts and components

Product status, backlog priorities, contributor guidance, PR targeting, and
experimental bootstrap ownership. No S3 syntax, diagnostic code, runtime ABI,
package format, or user-visible compiler behavior changes in this ADR.

## Validation

The 2026-09-27 repository inspection confirmed `main` as the default Rust
branch. Draft PR #14 documents Stage 1's supported scalar subset and the
current `CODEGEN_UNSUPPORTED_BODY` / `CODEGEN_UNSUPPORTED_IMPORTED_MODULE`
failure before Stage 2. The ordinary `main` native-route workflow does not
depend on the experimental self-host bootstrap job.

## Migration and compatibility

No source or binary migration is required: the Rust compiler is already on
`main`. Keep the experimental branch rather than deleting its unmerged code.
Close or park its draft PR so it is not mistaken for a planned 1.0 merge;
record the exact preserved head. Remove only branches whose changes are
already reachable from `main` or have been deliberately classified as
superseded. Do not rewrite `main` or remove release tags.

## Security and performance

No runtime behavior changes. Rust-side correctness, unsafe-boundary review,
and measured performance stay on the production roadmap. Any experimental
bridge remains subject to its own input validation and resource limits.

## Reconsideration criteria

After the Rust-based 1.0 release, evaluate the experiment using a real
Stage 0 → 1 → 2 → 3 build, stage-built conformance and negative tests, AOT/JIT
and ABI parity where claimed, package/target coverage, maintenance cost, and
measured performance. A new decision is required before changing the default
compiler or retiring Rust.
