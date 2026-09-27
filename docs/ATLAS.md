# Ori documentation Atlas

This file is the human-readable entry point for the documentation coverage
map. The compiler and its tests are the source of truth; this Atlas records
where each user-visible feature is implemented, tested, explained, and
illustrated.

The machine-readable registry is [`atlas/features.yaml`](atlas/features.yaml).
It is intentionally small and path-based so a future CI check can detect a
feature whose implementation changed without a corresponding documentation
update.

## Current baseline

| Item | Value |
|---|---|
| Language surface | S3 (`0.3.0`) + local inference B (`0.3.1`) |
| Workspace baseline | `0.3.8` |
| Latest released baseline | `v0.3.8` |
| Native ABI | `ori-native-abi-1` |
| Execution | Native AOT; `ori run` may use the staged Cranelift JIT |
| Compiler implementation through 1.0 | Rust on `main`; Ori-written self-host prototype remains experimental ([ADR-0006](decisions/adr/0006-rust-reference-through-1-0.md)) |
| Normative source | [`spec/`](spec/README.md) |
| User guide | [`language/`](language/tour.md) and [`guides/`](guides/README.md) |
| Open implementation list | [`planning/BACKLOG.md`](planning/BACKLOG.md) |
| Machine catalog | [`catalog.yaml`](catalog.yaml) |

FREEZE-1 closed on 2026-07-19. The workspace has not been bumped to `0.4.0`,
so documents must not describe `0.4` as an already released compiler line.
ABI-1 remains in force.

## How to use the Atlas

When changing a language feature:

1. update the implementation and regression tests;
2. update the corresponding registry entry;
3. update the normative reference and user guide when behavior is public;
4. add or repair a canonical example;
5. run `tools/qa/docs_coverage.sh` and the relevant compiler tests.

Statuses are deliberately conservative:

- `stable`: supported on the documented native path;
- `implemented`: available, but not yet a broad user contract;
- `partial`: restricted by a documented backend or semantic limitation;
- `experimental`: exposed for exploration and allowed to change;
- `unclear`: implementation exists but the public contract is not settled.

The Atlas is not a second language specification. Detailed grammar and
semantics remain in [`spec/`](spec/README.md).

## Planned implementation maps

The following documents describe proposed work and incremental implementation.
They remain indexed here with the current truth so a partial slice is not
mistaken for completion of the whole plan. The machine-readable registry marks
implemented or experimental slices explicitly.

| Area | Implementation map | Current truth |
|---|---|---|
| Hosted execution and embedding | [`planning/embedded-runtime-host-abi-v1.md`](planning/embedded-runtime-host-abi-v1.md) | [`spec/19-abi.md`](spec/19-abi.md), `compile --lib`, experimental `ori-embed`; `AUD-EMBED-1` safe hosted-value ownership is closed with Rust-owned string/bytes values and generation-bound slice tokens |
| Static metadata and attributes | [`planning/static-metadata-attributes.md`](planning/static-metadata-attributes.md) | [`spec/02-lexical.md`](spec/02-lexical.md), built-in top-level attributes, unsupported namespaced attributes rejected with `attr.unknown`, and `META-ATTR-1` |
| Persistent compiler/JIT service | [`planning/interactive-compiler-service.md`](planning/interactive-compiler-service.md) | Experimental `ori-embed` scalar persistent JIT with generational handles, whole-program JIT, file-granular AOT cache, and `COMP-SVC-1` |
| Value-type performance | [`planning/value-types-performance.md`](planning/value-types-performance.md) | Inline fixed arrays, heap-backed aggregate construction, and `VALUE-PERF-1` |
| Scripts and automation | [`planning/developer-experience-scripting-automation.md`](planning/developer-experience-scripting-automation.md) | Current CLI/process surface and `DX-SCRIPT-1` |
| Runtime control/observability | [`planning/runtime-control-observability.md`](planning/runtime-control-observability.md) | Current task/ARC/DAP primitives and `RUNTIME-CTRL-1` |
| Unicode text | [`planning/unicode-text-processing.md`](planning/unicode-text-processing.md) | UTF-8 `string`, scalar indexing, and `TEXT-UNICODE-1` |
| Web runtime foundation | [`planning/web-runtime-foundation.md`](planning/web-runtime-foundation.md) | `ori.net`, basic `ori.net.http`, and `WEB-FOUND-1` |
| Embedded profile | [`planning/embedded-execution-profile.md`](planning/embedded-execution-profile.md) | Hosted desktop runtime and `EMBEDDED-1` |
| Native binding generation | [`planning/native-binding-generation.md`](planning/native-binding-generation.md) | Manual `extern c`, generated export headers, and `FFI-BINDGEN-1` |
| Package ecosystem | [`planning/package-ecosystem-production.md`](planning/package-ecosystem-production.md) | Local/HTTP registry proof, lockfile, and reopened `PKG-REG` |
| Numeric/CPU graphics evolution | [`planning/ORI_GRAPHICS_LANGUAGE_EVOLUTION.md`](planning/ORI_GRAPHICS_LANGUAGE_EVOLUTION.md) | Inline structs, benchmark suite, bitwise/shift, managed `ori.buffer`, views, image helpers, and portable `simd[T, N]` vectors exist (`LANG-SIMD-1`); disconnected `ori.window` stubs were removed |
| Code audit, architecture, and high-performance systems | [`planning/roadmap-code-audit-performance-architecture.md`](planning/roadmap-code-audit-performance-architecture.md) | External audit and high-performance systems wave reconciled and closed on 2026-09-04. Includes global mutable task isolation (`CONC-THREADS-1`), structured concurrency (`ASYNC-STRUCT-1`), elimination of `DefId::INVALID` (`AUD-FRONT-2`), Unicode case-folding parity (`AUD-UNICODE-1`), typed stdlib errors (`LANG-STD-ERRORS-1`), declarative native dependencies (`PKG-NATIVE-1`), static `@noalloc` (`LANG-NOALLOC-1`), explicit `@align(N)` (`LANG-ALIGN-1`), bump-arenas `mem.region` (`MEM-REGION-1`), portable SIMD vectors (`LANG-SIMD-1`), and inline arena reset (`PERF-REGION-1`). Full evidence lives in [`planning/BACKLOG.md`](planning/BACKLOG.md) |
