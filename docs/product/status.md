# Ori current status

> Canonical public version: **0.3.8**  
> Language surface: **S3 / 0.4 ergonomics**  
> Native ABI: **`ori-native-abi-1`**  
> Maturity: **pre-1.0, active development**

This document is the canonical human-readable status page. Historical release numbers must not be presented as the current project version.

## Current implementation

| Area | Status |
|---|---|
| Compiler | Rust workspace under `compiler/` |
| Ori-written compiler | Experimental prototype outside `main`; not a 1.0 release gate |
| Frontend | Lexer, parser, name resolution, type checking, diagnostics |
| Intermediate representation | Typed HIR with optimization passes |
| Native compilation | Cranelift AOT |
| `ori run` | JIT when a packaged runtime cdylib is available; explicit AOT fallback |
| Runtime | Rust native runtime with ARC and cooperative cycle collection |
| Standard library | Rust runtime primitives plus `.orl` wrappers and algorithms |
| Tooling | CLI, formatter, documentation extraction, LSP, debugger support |
| Editors | Local VS Code/Cursor and Zed integrations |
| Projects | Root-first `ori.proj` projects with optional package dependencies |
| Packages | Path, Git, registry, lockfile, publish/install/get workflows |
| Documentation | English primary, Portuguese user-facing parallels where maintained |

## Stable and experimental areas

The normative classification lives in [`../spec/18-stability-and-compatibility.md`](../spec/18-stability-and-compatibility.md).

In practical terms:

- S3 syntax and the documented additive 0.3.x surface are public contracts.
- The native runtime ABI is versioned independently as `ori-native-abi-1`.
- Diagnostics use stable public codes.
- The native backend is the semantic reference.
- Package, registry, debugger, incremental, and ecosystem details may still evolve before 1.0 where explicitly marked experimental.

## Current priorities

The project should prioritize, in order:

1. correctness and crash prevention;
2. specification, implementation, examples, and diagnostics staying synchronized;
3. runtime and ABI safety;
4. regression coverage and conformance;
5. performance measured on realistic programs;
6. package and release reliability;
7. local development experience;
8. carefully justified additive language work.

Self-hosting remains a long-term maturity topic, not a prerequisite for the language to be useful.

## Compiler implementation through 1.0

The Rust compiler on `main` is the only product compiler and the semantic
reference through Ori 1.0. Work toward 1.0 prioritizes correctness, native
AOT/JIT conformance, runtime and ABI safety, diagnostics, packages, developer
tools, and measured performance. Building the compiler from source continues
to require Rust. Installed release packages retain their documented Rust-free
end-user routes.

The Ori-written self-host compiler is preserved as an **experimental branch**.
Its Stage 1 does not yet produce Stage 2; it is not merged into `main`, required
for a 1.0 release, or a supported alternative CLI. Independent Rust fixes
from the experiment may be proposed as focused, tested PRs. After the
Rust-based 1.0 release, reassess self-hosting against bootstrap, stage-built
conformance, ABI/target coverage, maintenance cost, and performance evidence
before any product promotion. See [ADR-0006](../decisions/adr/0006-rust-reference-through-1-0.md).

## Known structural work

The current implementation is functional, but several structural improvements are recommended:

- split the runtime monolith into domain modules without changing exported symbols or layouts;
- separate type-checker contexts and rule families while preserving diagnostic behavior;
- strengthen the stdlib manifest as a single declarative source;
- generate or type diagnostic identifiers instead of relying on repeated string literals;
- expand specification-to-test traceability;
- add stronger fuzzing, differential, property, and reproducibility checks;
- complete migration of planning history into the archive framework.

These are implementation-quality initiatives. They must not be mixed with unrelated language changes.

## Supported development baseline

The repository pins its Rust toolchain through `rust-toolchain.toml`. The Cargo workspace is located in `compiler/`.

Canonical validation from the repository root:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
sh tools/qa/daily_fast.sh
```

End users installing a release do not need the Rust toolchain to use the packaged JIT route. AOT compilation still requires a usable platform linker unless the package provides an appropriate bundled linker path.

## Status update rule

Update this file in the same change when any of these facts change:

- current public version;
- supported execution path;
- ABI version;
- platform or tooling support;
- major maturity classification;
- project priority order;
- a limitation that materially changes user expectations.

Release history belongs in `CHANGELOG.md`, not here.
