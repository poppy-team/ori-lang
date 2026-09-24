# Changelog — Ori Language

Todas as mudanças notáveis na implementação da linguagem Ori serão documentadas
neste arquivo.

O formato segue [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
e o projeto adere a [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Self-hosting progress

- Stage 1 and the bridge now preserve `bool` local bindings and Boolean
  variable reads, including an explicitly annotated binding. A new bootstrap
  fixture compares native execution with Stage 0 and checks the `Bool` HIR
  payload. Local annotations that disagree with their values report the
  existing `type.type_mismatch` diagnostic.
- Stage 1 now preserves one `int` parameter per local function through its
  parser, scope checks, type checks, and bridge payload. Local forward calls
  pass one supported integer argument; the bridge validates argument types and
  lowers the declared signature. The bootstrap gate compares a compiled
  parameter call with Stage 0 and rejects wrong arguments and leaked bindings.
  Full self-compilation still requires control flow, collections, imported
  definitions, and other expression forms.
- Stage 1 and the experimental bridge now preserve typed calls to local
  zero-argument functions returning `int`, including forward calls inside
  integer expressions. The bootstrap gate compares their native execution
  with Stage 0 and checks the serialized call; unsupported signatures and
  argument lists still fail before code generation.
- Stage 1 checks local bindings against each function's statement scope;
  a binding from a later statement or another function no longer passes
  `check`. The bootstrap script forces a fresh Stage 0 compilation of all
  self-hosted modules so an incremental cache cannot reuse an older Stage 1.
- The Stage 1 type checker resolves return types through local bindings,
  forward calls to functions in the same module, and integer comparisons.
  Its bootstrap checks valid boolean calls and rejects incompatible returns
  from variables and functions during `check`.
- The experimental bridge now emits zero-argument local calls returning
  `bool`, boolean literals, and integer comparisons in boolean returns.
  The bootstrap gate compares a native boolean call chain with Stage 0 and
  inspects the `Bool` signatures and `Eq` operation in its bridge request.
- The experimental frontend checks return types within each function's own
  statement range. A later function no longer inherits the entry function's
  return type; the bootstrap gate covers both valid and invalid signatures.
- The experimental Stage 1 now keeps `->` return signatures and binary
  operators and method receivers in its bridge payload. Unsupported interpolation,
  unrepresentable call arguments, declarations and unparsed function tokens cause compilation to fail instead
  of emitting a program with silently omitted or substituted code. The
  bootstrap gate checks the serialized operators and negative cases. Its
  minimal print path requires `import ori.io as io`, with the import retaining
  namespace precedence over a same-named local binding.
- The experimental Stage 1 parser now preserves keyword-named stdlib path
  segments such as `ori.list` and `ori.string` in imports and module headers.
  The bootstrap checks those imports before attempting self-compilation.
- Stage 1 now exits with the driver result and reports bridge/link errors as
  failures. Bootstrap verification requires Stage 1 to build Stage 2 and
  Stage 2 to build Stage 3, then compares the built binaries. The bridge
  rejects calls with unknown signatures instead of inventing C externs.
  These changes expose remaining gaps; they do not establish a completed
  self-hosted compiler or change the native ABI.
- The experimental Ori client preserves `int`/`void` return types and rejects
  parameters and statements absent from its emitter before native compilation.
  The bridge rejects undefined variables and mismatched binding, return and
  arithmetic types before code generation. The Linux CI now runs bridge
  protocol and invalid-IR regression tests.
- Updated the pinned `rustls` and `rustls-webpki` dependencies to address
  RUSTSEC-2026-0285 while keeping the Cargo audit gate enabled.

### Fixed

- **Collection equality/hash callbacks retain borrowed keys before calling Ori methods.**
  Generated callbacks pass an owned reference for each managed method
  parameter, matching the direct equality path. This avoids releasing a
  map, set, or graph key while the collection still owns it. The native ABI
  and runtime layouts are unchanged.

- **Managed `ori.list.get` results keep their own reference.** A retrieved
  struct, enum, string or collection remains valid when the source list later
  removes or releases it. The native compiler retains the borrowed runtime
  result before cleaning up temporary arguments; the native ABI is unchanged.

- **Aggressive leaf inlining can materialize scalar argument temporaries.**
  Direct same-module calls used as the complete value of `Let`, `Return`, or
  `Expr` can bind every numeric/bool argument once in source order, including
  ignored effects and traps. Substitution remains simultaneous and names use an
  inaccessible internal namespace. Contracts, async and managed signatures remain
  excluded; nested expressions, conditions and assignments receive no hoisting.
  No runtime or ABI contract changes; no general performance-gain claim.

- **Aggressive leaf inlining preserves parameter names and argument snapshots.**
  Parameter substitution is simultaneous, so caller variables matching another
  parameter are not substituted again. The conservative expression path keeps
  calls intact when the return contains calls and an argument is not a scalar
  literal, preserving compound reads of mutable globals. The statement-level
  temporary fallback above extends only scalar calls; managed-value support
  remains excluded.

### Removed

- **C/debug source backend and `ori emit c`.** Native Cranelift AOT/JIT remain
  the execution routes. Removed the inline C runtime and stdlib
  `c_backend_runtime` / `c_backend` support flags. Generated-C scripts no longer
  work and need explicit migration; native compilation is not a replacement
  for C-source inspection or GCC instrumentation. `c_header.rs`, generated FFI
  headers, `extern c`, and `@c_export` remain supported. Workspace version stays
  **0.3.8**; no future release date or version is assigned (ADR-0005).

- **Disconnected `ori.window` stub and unused `ori_window_*` runtime stubs (`AUD-HYGIENE-1`, `GFX-WINDOW-1`).**
  Removed `stdlib/window.orl` and dead `ori_window_*` functions in `ori-runtime`. Freestanding window
  and software canvas graphics are out of product scope per AGENTS.md; direct BMP/PPM image export
  remains supported via `ori.image`.

### Added

- **Compiler interaction, stress, and incremental test batteries.**
  - `compiler/crates/ori-driver/tests/feature_interaction_matrix.rs`: non-trivial pairwise subsystem combinations
    (`@align(N)` structs inside fixed-size arrays with arithmetic traits, generic traits with closures and `using`
    cleanup, complex enums with conditional guards and `try` returns, structs with SIMD and array methods, and
    scoped bump arenas with value batching). Fixed `mem.align_of` evaluation in HIR lowering.
  - `compiler/crates/ori-driver/tests/concurrency_stress.rs`: heavy multithreaded contention with multi-task
    producer/consumer bounded-channel ping-pong, concurrent cancellation across sleeping tasks, and atomic counter updates.
  - `compiler/crates/ori-driver/tests/simd_edge_cases.rs`: IEEE-754 floating-point edge cases (division by zero,
    Inf/NaN propagation) in SIMD vector lanes, integer vector arithmetic (`simd[int32, 4]`), and bitwise shift boundary tests.
    Fixed `ori_abort_shift_overflow` runtime reference collection in Cranelift codegen.
  - `compiler/crates/ori-driver/tests/incremental_invalidation.rs`: verified `.ori/incremental.json` cache hit
    invariance on unchanged sources, interface modification invalidation on downstream callers, and `ORI_DISABLE_INCREMENTAL=1` bypass.

- **AOT/JIT differential test suite with optimization parity (`QA-DIFF-1`).**
  Implemented `compiler/crates/ori-driver/tests/differential_testing.rs` covering Cranelift Native AOT,
  in-process JIT, and aggressive optimizer parity across all high-risk language features:
  multi-trait colon dispatch (`apply Type: TraitA, TraitB` + `for T: Trait`), SIMD vectors, struct alignment,
  scoped memory arenas, closures with structured captures, enum pattern guards, try-propagation,
  and fixed-size arrays. Asserted on byte-for-byte identical stdout and exit status across routes.

- **Surface ergonomics wave (`SYNTAX-APPLY-COLON-1`, `SYNTAX-IMPORT-AS-1`, `SYNTAX-COMPOSITE-POS-1`, `SYNTAX-STRUCT-INHERENT-1`, `SYNTAX-POLY-TRAIT-NAME-1`).**
  Canonical `apply Type: TraitA, TraitB` multi-trait header; natural `import path as alias` and `(item as alias)`
  (legacy `=` still accepted); compact positional `array[T, N]` alongside `array[T, size: N]`;
  inherent methods directly inside `struct` bodies with `apply` reserved for traits;
  bare trait names in parameter positions (`p: Trait`) lowering to the same `Ty::Any` as `any[Trait]`.
  Covered by E2E tests in `method_resolution.rs` (`apply` colon, multi-trait, bare trait params, struct inherents),
  `ori_spec.rs` (`array`/`simd` positionals), and `multifile_imports.rs` (`as` aliases).

- **Pure small-function and value-struct leaf inlining (`PERF-INLINE-1`).**
  Broadened conservative HIR leaf inlining for pure small functions with branchy bodies (e.g. aligned AABB
  early-exit intersection), allowing up to 8 pure statements and 4 parameter reads per pure variable/struct argument.
  Covered by polyglot `spatial_grid_bvh` comparison.

- **Reduced allocation overhead on channel send/receive fast paths (`PERF-CHANNEL-1`).**
  Optimized bounded channel message loops with static result singletons (`RESULT_OK_ZERO`), eliminating per-message
  heap allocations and ARC mutex acquisitions in high-throughput producer-consumer patterns. Covered by polyglot
  `channel_throughput` comparison.

- **Zero-call inline arena reset (`PERF-REGION-1`).**
  Emitted direct inline stores in Cranelift for `mem.reset` on `Region` instances, eliminating
  FFI call frame transitions on high-frequency frame tick resets. Covered by polyglot `arena_bulk_alloc`
  benchmark.

- **Zed editor DAP debugging integration (`ZED-EXT-1`).**
  Implemented `get_dap_binary` and `dap_request_kind` in `extensions/zed-ori` using `zed_extension_api` 0.7:
  registers `ori-dap` adapter launching `ori debug --dap`, connecting Zed's native debug UI to the compiler's
  cooperative DAP adapter for breakpoints, step debugging, and variable inspection.

- **Starlight documentation portal synchronization (`DOC-SITE-1`).**
  Synchronized `ori-website` reference pages with 88 modules and 1012 exported symbols via `ori doc export`,
  including full interactive API references for `ori.mem.Region`, `@align`, and `@noalloc`. Validated with
  Astro 5 + Starlight build and Pagefind search index generation.

- **Portable fixed-width SIMD vector primitives (`LANG-SIMD-1`).**
  Added first-class `simd[T, N]` types (`simd[float32, 4]`, `simd[int32, lanes: 4]`) lowered directly to
  Cranelift SIMD IR instructions (x86_64 SSE/AVX `F32x4`/`I32x4` and aarch64 NEON) with native vector
  operations (`+`, `-`, `*`, `/`), lane-indexed extraction (`v[i]`), and constant-time vector construction
  from literals. Verified in both AOT and JIT paths. Covered by E2E verification in `dx_scripting.rs`.

- **Scoped bump-arena memory regions (`MEM-REGION-1`).**
  Added `ori.mem.Region` with stack-local deterministic lifecycle (`using r: mem.Region = mem.region()`):
  native `OriRegion` bump-arena allocator with 64 KiB chunks, O(1) allocation via pointer bump,
  O(1) bulk reset (`mem.reset`) without traversing object graphs, and deterministic disposal
  (`core.Disposable`) upon block exit. Includes inspection utilities (`mem.size`, `mem.count`)
  and compile-time escape analysis: `Region` cannot escape its declaring block via `return`
  (`using.escape`) or cross threads/tasks (not `Transferable`). Covered by native runtime unit
  tests and E2E verification in `dx_scripting.rs`.

- **Explicit struct alignment and padding control `@align(N)` (`LANG-ALIGN-1`).**
  Added the built-in struct attribute `@align(N)` accepting power-of-two alignment boundaries (1, 2, 4, 8,
  16, 32, 64). Lowered directly through AST, checker, HIR (`HirStruct.explicit_align`), and Cranelift
  native codegen (`StructLayout`), enforcing minimum struct alignment and padding. Reflected in compile-time
  constants and runtime introspection (`ori.mem.align_of` and `ori.mem.size_of`), and propagated to generated
  C export headers (`alignas(N)` in C++ / `__attribute__((aligned(N)))` in C) for GPU uniform/storage buffers
  (std140/std430), SIMD vectors, and foreign engine boundaries (GDExtension). Covered by unit and E2E tests
  in `dx_scripting.rs` and `multifile_imports.rs`.

- **Static zero-allocation verification attribute `@noalloc` (`LANG-NOALLOC-1`).**
  Introduced the built-in function attribute `@noalloc` for performance-critical hot paths (e.g. 60/120 FPS
  engine tick loops, audio DSP callbacks, physics integration). The type checker statically verifies the
  function body and strictly rejects dynamic heap allocations: collection literals (`list`, `map`, `set`),
  interpolated strings (`f"..."`), string concatenation (`+`), closures, asynchronous operations (`await`,
  `async`), `using` blocks, dynamic collection iteration, and calls to functions not marked `@noalloc` or
  known allocating standard library functions (`fmt.*`, `lists.*`, `maps.*`, `sets.*`, `strings.*`, `task.spawn`).
  Emits diagnostic `perf.allocation_in_noalloc` on violation with source span, cause, and actionable advice.
  Covered by unit tests and E2E verification in `tests/dx_scripting.rs`.

- **Declarative native dependencies, pkg-config resolution, and platform link configurations (`PKG-NATIVE-1`).**
  Extended package manifests (`ori.pkg.toml`) with declarative native dependency management:
  supports `[native.dependencies.<lib>]` with `pkg_config = "..."`, `static = bool`, `framework = "..."`,
  and version constraints, plus inline tables under `[native.dependencies]`; per-platform library,
  framework, directory, and flag specifications (`[native.linux]`, `[native.windows]`, `[native.macos]`,
  and common `[native]`). Automated `pkg-config` querying translates `--libs` output into `-L`, `-l`,
  and `-framework` link lines for Unix/macOS or `.lib` and `/LIBPATH` directives for Windows MSVC,
  enabling direct linking of system libraries (e.g. Raylib, Box2D, OpenGL, X11) without manual `.a`
  linker scripts or staging workarounds. Covered by unit tests in `package.rs` and `native_deps.rs`
  and E2E compilation tests in `multifile_imports.rs`.

- **Persistent daemon session caching, warm check hits, and invalidation (`CLI-DAEMON-1`).**
  Extended `ori daemon` with `DaemonSession` state tracking: caches type-check results keyed
  by source content SHA-256 with bounded capacity (256 entries) and FIFO eviction. Enables
  warm check cache hits (`"cached": true`), explicit file or full cache invalidation (`"invalidate"`),
  and operational statistics (`"stats"` for `check_hits`, `check_misses`, `invalidations`, and
  `cached_entries`). Covered by 9 unit tests in `pipeline/daemon.rs` and E2E daemon smoke in `tests/dx_scripting.rs`.

- **Doctest output assertions, compile-fail directives, and rich diagnostics (`DX-DOCTEST-1`).**
  Extended `ori test --doc` doctest runner with directive parsing and execution:
  captures stdout during in-process Cranelift JIT execution to validate `-- output: <text>`
  (including multiline expected output); supports `-- compile_fail` (optionally matching specific
  diagnostic codes or substrings) for documentation examples demonstrating type or syntax errors;
  formats rich source-labelled diagnostics (severity, code, message, `--> file:line`, why, and action);
  automatically extracts top-level import statements to module scope without duplication.
  Covered by 6 unit tests in `pipeline/doctest.rs` and 2 E2E tests in `tests/dx_scripting.rs`.

- **Hostile-input fuzzing and pathological literal robust testing (`AUD-QA-3`).**
  Enriched `tools/qa/fuzz_smoke.py` with structured generators covering extreme number literals (4,000 digits),
  pathological identifiers (4,000 chars), unclosed strings/f-strings/comments, embedded null bytes (`\0`),
  malformed `@cfg` predicates, and 256-level nested expressions/calls/types/patterns with signal, timeout,
  and panic detection. Added in-memory hostile input tests in `tests/security_robustness.rs` ensuring graceful
  termination and bounded diagnostic spans.

- **Multi-phase diagnostic shape verification and expanded curated example execution (`AUD-QA-2`).**
  Expanded `tests/diagnostic_catalog.rs` to validate real diagnostic shapes, severities, primary spans, and
  actionable advice across all major compiler phases (parser `parse.module_missing`, resolver `name.undefined`,
  type checker `type.type_mismatch`, attribute `attr.c_export_not_public`, and semantic linter `lint.unused_variable`).
  Expanded default curated L3 execution in `tools/qa/daily_full.sh` to 8 examples (`hello`, `language_features`,
  `native_showcase`, `collections_demo`, `string_toolkit`, `bytes_usage`, `conditional_config`, `error_handling`).

- **Exhaustive expression and structured binding traversal in AST linter and LSP (`AUD-LSP-5`).**
  Extended `ori-driver` and `ori-lsp` in-memory semantic linting (`run_lint_source`) to comprehensively traverse
  all AST expression variants (`MatchExpr` arms and patterns, closures, struct updates, `try`, `await`,
  `is`, tuple indices, and `suspend` statements). This eliminates false unused variable warnings for bindings
  used within expression arms, closures, and error propagation, covered by unit tests in `ori-lsp`.

- **Operational ABI export verification and layout regressions (`AUD-ABI-QA-1`).**
  Expanded `tools/qa/abi_exports.sh` to check all 20 canonical public runtime symbols in the staged cdylib
  (`ori_rt_*`, `ori_alloc*`, `ori_arc_*`, `ori_handle_*`, `ori_host_*`, `ori_reactor_*`) in addition to
  manifest-derived static symbols. Added layout regressions in `ori-runtime/src/tests.rs` for `OriHeap`,
  `OriDeque`, `OriGraph`, and `OriBytes`, and typed the `OriBytes` bridge in `ori-runtime`.

- **Runtime allocation boundary matrix, failure contracts, and typed FFI validation (`AUD-RT-1`, `AUD-RT-2`).**
  Formalized and regression-tested the public allocation boundary matrix covering `ori_alloc`,
  `ori_alloc_typed`, collection capacity growth, zeroed buffer allocations, string repeat/padding,
  and slice length boundaries. Added explicit test coverage for pointer provenance verification
  (`ori_handle_validate_size_type`, `retain_registered_payload`), safe foreign/stack pointer no-ops,
  and typed UTF-8 ingress failure diagnostics (`ORI_HOST_ERROR_INVALID_UTF8`).

- **Unicode case-folding full non-Turkic parity across native and C backends (`AUD-UNICODE-1`, `TEXT-UNICODE-1`).**
  Implemented complete Unicode 9.0.0 full non-Turkic case folding in `ori-codegen` C backend (`c_casefold.rs`),
  matching `unicode-casefold` in `ori-runtime` bit-for-bit. Added formal test vectors in
  `tests/unicode_case_fold_conformance.json` covering ASCII, German sharp S (`ß → ss`), umlauts, Latin
  accents, ligatures (`ﬁ`, `ﬂ`, `ﬃ`, etc.), Greek sigma, Cyrillic, fullwidth Latin, and emoji preservation,
  with runtime and end-to-end multi-backend regression tests.

- **Eliminate `DefId::INVALID` recovery sentinels in HIR and codegen (`AUD-FRONT-2`).**
  Replaced dummy `DefId::INVALID` sentinel fallbacks with typed `Option<DefId>` representations
  on `HirExprKind::StructLit`, `HirExprKind::EnumVariant`, and `HirExprKind::StructUpdate`.
  Anonymous struct literals start as `None` and are refined monotonically during expected-type
  application; top-level module item lowering lookups fail closed directly; native Cranelift and C
  backends inspect and unpack `Option<DefId>` safely.

- **Enforce `@inline` and `@no_inline` attributes during optimization and lowering.**
  Functions decorated with `@inline` and `@no_inline` now have their intent propagated
  through HIR `is_inline` and `is_no_inline` flags; leaf-inlining optimization respects
  `@no_inline` by preserving calls under all optimization levels, and prioritizes `@inline`
  functions beyond default heuristic statement limits.

- **Zero-cost error return traces (`ERR-TRACE-1`).**
  Exposed `ori.err_trace` in the standard library (`stdlib/err_trace.orl`) with
  `push(file: string, line: int, message: string) -> string` and
  `format(message: string) -> string`, wired into `ori_err_trace_push` and
  `ori_err_trace_format` in `ori-runtime`, the Cranelift native backend, and the C
  backend, enabling source location tracking during error propagation.

- **Residual feature matrix and diagnostics (`AUD-FRONT-1`).**
  Added explicit fail-closed diagnostics for unsupported residual syntax forms,
  including `parse.newtype_generics_unsupported` for generic newtypes and
  `parse.async_iter_unsupported` for async iterators, completing test coverage
  across the iterator and newtype matrix in `ori_spec.rs`.

- **Structured concurrency scopes (`ASYNC-STRUCT-1`).**
  `ori.cancel.CancelScope` and `ori.cancel.TaskScope` now implement `core.Disposable`,
  providing deterministic cancellation on scope exit and child task joining with `using`.
  `TaskScope` tracks spawned child jobs, propagates token cancellation on block exit,
  and waits for all child tasks to complete, preventing orphaned tasks from outliving
  the parent scope.

- **Typed stdlib errors, process output, and safe crypto helpers (`LANG-STD-ERRORS-1`).**
  Added structured, exhaustive error enums in the standard library replacing bare strings:
  `ori.fs.FsError` (`NotFound`, `PermissionDenied`, `AlreadyExists`, `InvalidPath`, `Other`),
  `ori.net.NetError` (`ConnectionRefused`, `TimedOut`, `HostUnreachable`, `AddressInUse`, `Closed`, `Other`),
  and `ori.json.JsonError` (`ParseError`, `IoError`). Added typed operations `fs.try_read_text`,
  `net.try_connect`, `json.try_parse`, and `json.try_read`.
  Added `ori.process.ProcessOutput` (`status: int`, `stdout: bytes`, `stderr: bytes`)
  and `proc.run_output(program, args) -> result[ProcessOutput, string]`, capturing
  unmodified binary output without lossy UTF-8 conversion. Added safe string decoding
  helpers `stdout_text(output)` and `stderr_text(output) -> result[string, string]`.
  Added `try_hash_password`, `try_totp_generate_secret`, and `try_totp_code` in
  `ori.crypto` returning `result[string, string]` instead of empty strings on failure.

- **Language-first external-audit reconciliation (2026-09-01).** The current
  implementation truth, priority order, and remaining P0/P1 language
  contracts are now recorded in the audit roadmap, Atlas, and single backlog.
  Historical enum-transferability, bytes-length, ARC-edge, hosted-ABI, and
  linker findings remain closed only where code and regressions prove them.

- **Global mutable task-boundary diagnostic (`CONC-THREADS-1`).** The checker
  now rejects direct reads and writes of top-level mutable `var` values inside
  `task.spawn` closures with `concurrency.global_mutable_capture`, and follows
  same-module helper calls plus imported named free and associated helpers
  through conservative call-graph fixed points. Pure named functions and local
  closures with checker-side transferable-capture summaries are accepted;
  unsafe or unknown function environments remain blocked. Receiver methods and
  `any[Trait]` dispatch use conservative method-name effect summaries; a
  complete type-level isolation model remains follow-up work. OS resource
  handles (`fs.File`, `io.Input`, `io.Output`, `net.Connection`, `net.Listener`,
  `net.UdpSocket`) are now explicitly rejected at task boundaries, while
  `task.CancelToken` remains transferable by design. Native and check-only
  regressions cover both local closure paths.

- **Bounded channels (`LANG-CHANNEL-1`).** Added
  `ori.channel.create_bounded(capacity) -> optional[channel.Channel[T]]`.
  Positive capacities enforce FIFO backpressure, invalid capacities return
  `none`, and closing a channel wakes blocked senders with `err(...)`. Runtime
  and native regressions cover managed ownership, blocking, closure, and
  teardown.

- **Shared blocking-I/O pool (`LANG-IO-POOL-1`).** Filesystem, connect, and TLS
  futures now share a lazily-created pool capped at four workers and a 256-job
  FIFO queue. Queue admission is bounded, worker panics become failed futures,
  and shutdown or thread-creation failure completes work deterministically.

- **Fail-closed custom attributes (`META-ATTR-1`).** Unsupported namespaced
  attributes now emit `attr.unknown`; only the seven built-in attributes have
  schemas. This removes the previous inert name-based acceptance path while
  leaving third-party schema design for a future language decision.

- **User-defined collection equality (`LANG-COLL-EQHASH-1`, partial).** Native
  `Equatable.equals` implementations on user-defined structs now drive
  `==`/`!=` and custom `map`/`set`/`hash_table` membership, including distinct
  values that compare equal. Non-recursive structural structs/enums receive
  generated native hash callbacks; an optional user `hash(self) -> int` method
  on `Hashable` overrides the generated callback, while explicit
  non-structural equality without `hash` uses a constant-hash correctness
  fallback. Generic graph nodes use the same equality callback for
  add/find/edge/traversal and preserve it through graph copies. Structural
  enum equality now works for direct `==`/`!=`, and non-recursive enums carrying
  `Hashable` can be used as graph nodes and collection keys; recursive-key,
  recursive-key ABI and performance portions of this P0 remain open.

- **User Hashable method and async frame guard.** `core.Hashable` now accepts
  an optional `hash(self) -> int` method; native collection callbacks invoke it
  when present, while marker-only types retain generated structural hashing.
  Async frame emission now verifies slot bounds/layout and zero-initializes
  managed await bindings before scheduling, preventing terminal cleanup from
  reading uninitialized pointers. Full HIR ownership/data-flow verification
  remains open (`LANG-OWNERSHIP-VERIFY-1`).

- **Borrowed-handle export boundary (`LANG-HANDLE-1`, partial).** `@c_export`
  now rejects managed aggregates containing `handle[T]` fields, including
  nested structs and enum payloads, so an unmanaged host pointer cannot escape
  through an ARC-owned return value. The new `ori.handle.is_null` helper checks
  the null sentinel without dereferencing it, and `ori.handle.null()` creates
  that sentinel explicitly for a typed `handle[T]` value. Handle `==`/`!=` now compare
  pointer identity without retaining or dereferencing the pointee; nullable
  safe accessors, host lifetime, and foreign-thread affinity remain open
  contract work. Concrete non-generic managed aggregates now carry a
  compiler source-type tag; wrappers reject same-size handles from another
  source type before user code.

- **Opaque-handle provenance guard (`LANG-FFI-1`, partial).** Generated
  `@c_export` wrappers validate managed-handle parameters against the live ARC
  registry before retaining or entering user code. For concrete non-generic
  payloads they also validate the registered payload size and compiler
  source-type tag. Null, foreign, wrong-size, and same-size wrong-type pointers
  now take the deterministic bounds-failure path instead of being interpreted
  as an Ori aggregate; generic aggregate exports remain rejected. The Linux
  managed-handle C host now runs under ASan/UBSan when available; a
  cross-platform hostile foreign-host matrix remains open.

- **Disjoint synthetic definition IDs (AUD-FRONT-2).** Literal recovery,
  applied type parameters, and compiler-generated closures now use named,
  non-overlapping ID ranges; the first generated closure can no longer collide
  with the invalid-definition sentinel.

- **ARC contention instrumentation (AUD-RT-3).** Runtime registry access now
  goes through one poison-safe lock helper, with a low-overhead atomic counter
  for contended acquisitions so sharding decisions can be based on measurements.

- **Fail-closed synthetic type rendering (AUD-FRONT-2).** Diagnostics no
  longer panic when a recovery or synthetic `DefId` reaches type formatting;
  unknown named types render as a stable unresolved-type label.

- **Workspace Rust quality gate (RUST-QUALITY-1).** The current workspace is
  warning-free under strict all-target Clippy and passes the full Cargo format
  check; the same checks are required by the daily QA gate.

- **Atlas schema QA (AUD-QA-2).** `docs_coverage.sh` now validates the
  machine-readable feature Atlas with a dependency-free schema checker, including
  required fields, allowed statuses, unique IDs, safe paths, and path existence.
  The diagnostic catalog gate now also validates row metadata and a real
  `parse.module_missing` severity/message/span fixture.

- **Native example build tier (AUD-QA-2).** `examples_smoke.sh` can now
  validate all 25 root and nested example entrypoints through their native
  route (binary, C-export library, or test harness) in an isolated temporary
  directory with `ORI_EXAMPLES_COMPILE=1`; `daily_full.sh` enables this tier and
  runs the curated `hello`, `language_features`, and `native_showcase`
  binaries at L3, then cleans the generated binaries automatically.

- **Timer heap compaction (AUD-RT-5).** The native timer heap now
  periodically removes futures that already reached a terminal state and
  releases the timer-owned ARC reference, avoiding retention until distant
  deadlines while preserving deterministic deadline ordering. Cancellation
  tokens also release excess vector capacity after a burst of associations.
  `tools/bench/run_timer_heap_churn.sh` provides a reproducible 128-sleep
  workload with a completion canary.

- **Stale guidance cleanup (AUD-HYGIENE-1, partial).** Exhaustiveness and
  `ori explain` messages now name the canonical `ok`/`err` result constructors;
  AST/HIR crate docs describe the implemented modules, and unused LSP
  workspace-root state/accessors were removed. Legacy angle syntax remains only
  as bounded diagnostic recovery.

- **Structured semantic lint bindings (AUD-LSP-5).** The shared CLI/LSP
  linter now tracks bindings introduced by destructuring, `for`, `while some`,
  `match` patterns, `using`, `repeat`, and `loop`, with lexical shadowing and
  unused-read coverage in an integration regression.

- **Bounded editor linting (AUD-LSP-5, partial).** LSP linting now skips source
  buffers above 1 MiB to keep reparsing/checking responsive; a regression locks
  the budget while the remaining resolver-identity work stays explicit.

- **Stdlib compatibility deduplication (AUD-HYGIENE-1, partial).** The legacy
  `ori.concurrent.utils` and `ori.process.utils` modules now forward to their
  canonical parent modules instead of carrying copied function bodies.

- **Inert SIMD scaffold removed (AUD-HYGIENE-1 / GFX-SIMD-1).** The HIR
  optimization pipeline no longer traverses a vectorizer that could never
  rewrite a loop; real SIMD remains a future, benchmark-gated feature.

- **Identity-aware editor linting (AUD-LSP-5, partial).** Usage and mutation
  tracking now stores a stable binding identity per lexical declaration, so an
  inner shadowed name cannot hide an outer unused-variable warning. Resolver
  `DefId` integration remains a later step.

- **Typed daemon protocol (CLI-DAEMON-1, partial).** JSON-RPC requests now use
  `serde_json` DTOs and structural response builders; malformed envelopes,
  escaped IDs, invalid parameters, exact shutdown handling, and bounded line
  buffering at 1 MiB requests/8 MiB sources are regression-tested. Warm session
  caching and incremental reuse remain open.

- **Hostile-input front-end smoke (AUD-QA-3, partial).** Added a deterministic,
  dependency-free `fuzz_smoke.py` gate covering malformed bytes, truncation,
  invalid checks, deep nesting, process timeouts, and panic-like output. The
  full QA script reports missing compiler binaries explicitly as incomplete.

- **Shared JSON type normalization (AUD-FRONT-2, partial).** Checker and HIR
  now call one recursive helper to replace the stdlib JSON placeholder; if the
  concrete `ori.json.Value` definition is unavailable, nested positions become
  `Ty::Error` instead of leaking a synthetic definition into backend lowering.

- **Failure-safe doctest execution (DX-DOCTEST-1, partial).** `ori test --doc`
  now extracts directory trees in stable order, reports check/JIT/temp-file and
  cleanup failures as named test results, uses a per-case temporary directory,
  and returns the checked snippet sources through its `SourceCache`.

- **Declarative runtime-link schema (AUD-ABI-QA-1, partial).** The runtime
  metadata validator now consumes `tools/qa/runtime-link.schema.json` for its
  required fields, types, patterns, and enum values before applying artifact
  path and SHA-256 checks.

- **Enum transferability checks (CONC-THREADS-1, partial).** Spawn/channel type
  checking now walks enum variant payloads with cycle protection, so an enum
  containing a non-transferable slice, lazy value, function, or `any` payload
  cannot cross a task boundary silently.

- **Concurrency documentation cleanup (AUD-HYGIENE-1, partial).** Removed the
  obsolete warning that prohibited managed channel values and closing pending
  network handles; the documentation now reflects the completed ownership and
  close-synchronization fixes while retaining the `Transferable` requirement.

- **NUL-safe native bytes I/O (AUD-BYTES-1).** Managed `bytes` now
  keep their registered length through list conversion, synchronous TCP
  writes, and synchronous/asynchronous UDP sends. Embedded `NUL` bytes are no
  longer truncated by an accidental `CStr` conversion; foreign unregistered
  buffers now fail closed with host error `1002`; foreign buffers use explicit
  length-aware `OriBytes` views. `str.to_bytes` now always copies into a
  managed byte allocation, so static string literals cannot silently become
  zero-length foreign pointers in async network writes.
- **I/O worker panic containment (AUD-RT-4).** Panics from readiness
  or blocking I/O jobs now fail only their future, release the job keepalive,
  and leave the shared reactor available for subsequent work.
- **Race-free task cancellation association (AUD-CANCEL-2).** Future/token
  association now rechecks cancellation under the token lock, and bulk cancel
  balances every retained future reference. Runtime regressions cover both
  normal cancellation and association after a token was already cancelled.
- **Poisoned reactor queues recover (AUD-RT-4).** I/O queue and
  condition-variable locks now recover poisoned state, so a later job can
  still complete after an isolated panic or poisoned lock. A focused runtime
  regression protects this behavior.
- **Fallible runtime thread creation (AUD-SPAWN-1).** Timer, task,
  readiness-reactor, blocking-I/O, and async filesystem workers now use one
  fallible named-spawn helper. Creation failures complete futures as `Failed`,
  release transferred ARC ownership, and return an empty task handle instead
  of panicking or leaving work pending. Timer and reactor startup retry after a
  transient creation failure instead of caching a permanent error. Failures
  record host error `1004` (`ORI_HOST_ERROR_THREAD_SPAWN`) with the worker/cause.
  Deterministic worker-failure regressions protect the terminal-state contract.
- **Safe C check diagnostics (AUD-C-1).** The C backend no longer
  interpolates user-controlled `check` text into an `fprintf` format literal.
  It emits a constant `%s` format and escapes quotes, backslashes, control
  bytes, NUL, and percent sequences. Regressions cover the generated shape,
  strict C format compilation, and optional ASan+UBSan execution with an
  explicit unsupported-host skip and a CI-required Linux native-route mode.
- **Explicit channel queue ownership tags (AUD-CHANNEL-1).** Typed lowering
  selects separate scalar and managed send symbols; queue entries retain that
  ownership tag through receive/destruction. Scalar `i64` values are never
  guessed to be pointers, while managed entries own one edge per send. Runtime,
  contention, and AOT/JIT leak regressions cover both routes.
- **Package archive preflight (AUD-PKG-1).** Remote package tarballs
  are inspected before extraction: traversal/absolute paths, duplicate or
  non-UTF-8 names, excessive entry counts, and symlink/hardlink/device/FIFO
  entries are rejected. Extraction no longer restores archive owners or
  permissions. Digest enforcement, resource limits, and atomic cache publish
  remain covered by atomic cache publication and verified SHA-256 metadata.
- **Typed invalid-UTF-8 host reporting (AUD-RT-2).** Runtime C-string
  boundaries now record host error `1003` (`ORI_HOST_ERROR_INVALID_UTF8`) with
  a stable message when decoding fails, instead of silently treating malformed
  input as a valid empty string. Legacy pointer-returning functions keep their
  compatibility return until typed result wrappers are available.
- **Full Unicode case folding (TEXT-UNICODE-1 / AUD-UNICODE-1).** Native AOT
  and JIT now use the versioned `unicode-casefold` full, non-Turkic mapping;
  multi-scalar folds such as `ß` → `ss` are covered by runtime regression
  tests. The C/debug backend remains reduced-parity until it has the same table.
- **Host-only native target contract (AUD-TARGET-1).** AOT, JIT, and native
  tests now reject a non-host `ORI_TARGET_TRIPLE` before incremental reuse or
  code generation with `native.target_unsupported`.
- **Operational ABI export gate (AUD-ABI-QA-1, partial).** Added the
  cross-platform `tools/qa/abi_exports.sh` daily check for static/shared
  runtime exports and fixed the PowerShell checker to use `compiler/target` and
  inspect cdylib lifecycle symbols.
- **Runtime-link metadata validation (AUD-ABI-QA-1, partial).** Added a
  dependency-free validator for target/profile/version fields, safe artifact
  names, staged-file presence, and declared SHA-256 identity.
- **Named JSON synthetic definition (AUD-FRONT-2, partial).** Replaced the
  magic JSON type placeholder with a reserved `DefId` outside the sequential
  definition arena.
- **Checked NUL-terminated string sizes (AUD-RT-1, partial).** String
  concatenation and conversion now check sentinel-byte arithmetic before
  calling the allocator.
- **DefId validation (AUD-FRONT-2, partial).** Named invalid/synthetic IDs now
  replace repeated numeric sentinels; `DefMap::try_get` returns `None`, invalid
  `get` calls fail closed, and a 10,001-definition regression protects the
  sequential arena.
- **Removed inert optimization scaffolds.** Deleted the name-only RC elision
  pass and the unused `Ty::is_acyclic()` helper; both lacked the ownership/type
  graph information required to make a sound optimization.
- **Removed the unconsumed type-interner scaffold (OPT-TYPE-INTERN-1).** The
  public `TyInterner`/`TyId` API was not used by checker or HIR and exposed an
  unchecked arena lookup; it will return only with validated handles and a
  measured migration plan.
- **One-build native test dispatcher (AUD-TEST-1).** `ori test` now emits and
  links one suite binary, then launches one isolated process per selected test
  with `ORI_TEST_INDEX`; filtering and failure isolation remain unchanged.
- **Correct delayed cancellation (ASYNC-STRUCT-1, partial).**
  `cancel.defer_cancel` is now asynchronous and awaits its sleep future before
  cancelling the scope token; an end-to-end regression covers the deadline.
- **Strict `check` message contract (AUD-PARSE-4).** All messages after the
  comma must be string literals. Dynamic or scalar expressions now emit
  `parse.check_message_literal` and are consumed during parser recovery instead
  of being silently discarded.
- **Length-aware `bytes` C exports (AUD-FFI-1).** `@c_export` now
  accepts `bytes` through generated `OriBytes { data, len }` views. Inputs are
  copied exactly (including embedded `NUL`), and returns use an `OriBytes *out`
  bridge with explicit ARC ownership. NUL-terminated string ingress is copied
  and UTF-8 validated before Ori can retain it.
- **Sound hosted managed values (AUD-EMBED-1).** Raw managed constructors were
  removed. Strings and bytes are Rust-owned copies with safe accessors; opaque
  slices carry session/module/generation identity and reject cross-session,
  cross-module, stale, and unload/reload use before invocation.
- **Hosted Host ABI v1 lifecycle (EMBED-HOST-1, AUD-UNLOAD-1, AUD-EMBED-2).**
  Added versioned C contexts, generation-bound function/value handles,
  nominal host-owned opaque handles, structured diagnostics, aggregate
  callbacks with capability and thread-affinity dispatch, serialized runtime
  leases, and shutdown-safe `dlclose` sequencing. Callback/dispatcher panics
  are contained at the `C-unwind` boundary.
- **Defined C `any` dispatch ABI (AUD-C-2).** Stored `any` values now route
  instance calls through typed translation-unit trampolines that convert the
  boxed pointer receiver to the concrete by-value method signature. UBSan no
  longer reports an incompatible function-pointer call. Default trait methods
  receive the field-less trait representation instead of an incompatible
  concrete struct. The managed-field, vtable-lifetime, ownership, C-compile,
  and sanitizer regressions cover both dispatch paths.
- **Reproducible native release archives (AUD-REL-1).** Native metadata
  emission now sorts string/global data and function-reference/wrapper snapshots
  by semantic key instead of per-process `HashMap` order, keeping generated
  machine code stable. Archive traversal prunes `.ori` compiler caches before
  reading them, reducing package time and avoiding quota pressure. Same-epoch
  archives now compare byte-for-byte across extracted package roots.
- **Canonical S3 LSP completion and hover (AUD-LSP-4).** Removed pre-S3
  keyword suggestions, corrected `apply`/`using` snippets, and changed generic
  and optional hover types to bracket syntax. Focused LSP unit tests prevent
  the removed forms from returning.
- **AST-backed LSP linting (AUD-LSP-5, partial).** Editor lint requests now
  reuse the driver's in-memory parser/checker and AST traversal instead of a
  line/substr scanner. Comments, strings, Unicode names, nested shadowing, and
  `check` expressions are covered; resolver binding identity and broader
  binding forms remain follow-up work.
- **Heap-backed runtime timers (AUD-RT-5, partial).** Timer scheduling now
  uses a min-heap with deterministic tie-breaking instead of sorting every
  pending timer on each wake. A workload benchmark and cancellation compaction
  remain open.

- **Loop-vectorization scaffold (GFX-SIMD-1, not implemented).** Added
  `ori-hir/src/optimize/vectorize.rs`, but its transformation currently always
  returns `None`; no loop is unrolled or vectorized yet.
- **Window/canvas prototype `ori.window` (GFX-WINDOW-1, incomplete).** Added
  `stdlib/window.orl` and runtime ABI stubs. The stdlib is not connected to the
  ABI, event polling is hardcoded, and pixel presentation is a no-op.
- **Doctest extraction & execution `ori test --doc` (DX-DOCTEST-1).** Added automatic extraction and
  in-process JIT execution of code examples in `.oridoc` files and `///` doc comments.
- **Error-trace ABI scaffold (ERR-TRACE-1, incomplete).** Added
  `ori_err_trace_push` and `ori_err_trace_format`; the compiler and stdlib do not
  currently call them.
- **Process-persistent daemon prototype (CLI-DAEMON-1, incomplete).** Added a
  line-oriented stdio service for check, eval, and format. Requests still build
  fresh pipelines and the parser is not a complete JSON-RPC implementation.
- **Executor queue polling (ASYNC-REACTOR-1, partial).** Added
  `ori_reactor_poll` and `ori_reactor_wake` over the executor condition variable.
  Unix readiness uses a separate single `poll` worker; this is not yet a
  cross-platform OS reactor.
- **Cancellation-token wrappers (ASYNC-STRUCT-1, partial).** Added
  `CancelScope`, `create_scope`, `cancel`, and `is_cancelled`. The
  `defer_cancel` helper now awaits its delay; a child-task structured-concurrency
  tree is still open.
- **Cross-thread copy helpers (CONC-THREADS-1, partial).** Added
  `transfer_int`, `transfer_string`, and `transfer_list_string`. They do not by
  themselves establish a complete type-level isolation/ownership model.
- **One-allocation string construction fast paths (OPT-SSO-1).** Direct slice
  copies remove an intermediate allocation. Ori does not yet use a tagged inline
  small-string representation.
- **Zero-copy string view slicing (STR-VIEW-1).** Added `stdlib/string_view.orl` providing `StringView`
  with zero-copy slicing, prefix/suffix inspection, and subviews over underlying strings.
- **Image export helpers `ori.image` (GFX-ECO-1).** Added `stdlib/image.orl` providing `encode_ppm`,
  `write_ppm`, `encode_bmp`, and `write_bmp` for direct PPM text and 24-bit uncompressed BMP binary
  image export from numeric color arrays and software rasterizer framebuffers.
- **Parallel module type-checking (OPT-PAR-TYPECHECK-1).** Enabled multi-threaded function-body
  type checking via `rayon` across independent loaded source modules in `check_loaded_sources`.
- **Extended semantic linters (DX-LINT-EXT-1).** Added `lint.prefer_const` for unmutated `var` bindings,
  `lint.shadowed_variable` for inner-scope shadowing, and complete AST expression traversal in `ori lint`.
- **Experimental persistent compiler service and modular JIT session
  (COMP-SVC-1, partial).** `ori-embed` provides persistent scalar JIT sessions,
  generation-checked handles, unload operations, structured trap results, and
  callbacks. Hosted pointer ownership and process/runtime unload lifecycle are
  now closed by `AUD-EMBED-1`/`AUD-UNLOAD-1`; daemon cache/session invalidation
  remains a separate P2 follow-up.
- **Embedded and freestanding execution profile support (EMBEDDED-1).** Added explicit target facts
  and `@cfg(execution_profile: "embedded")` / `--execution-profile embedded` separating OS-dependent
  runtime features from core freestanding code.
- **Numeric-loop optimizations and bounds-check elimination (GFX-BCE-1, GFX-MIDEND-1).** Compile-time
  bounds checks for constant array indices, direct address arithmetic on inline `array[T, N]`,
  and bounded fixed-point optimization pipeline in `ori-hir` for numeric loops.
- **Package ecosystem protocol prototype (PKG-REG, partial).** Registry v1 protocol, package
  publishing (`ori publish`), dependency retrieval (`ori get`), package installation (`ori install`),
  and lockfile validation (`ori lock --locked`). Archive integrity/containment
  and lock-driven reproducibility are now implemented under `AUD-PKG-1/2`;
  hermetic HTTP-registry integration tests remain P2 QA follow-up.
- **Native binding generation `ori bindgen` (FFI-BINDGEN-1).** Added `ori bindgen` CLI sub-command
  generating clean, type-checked low-level Ori `extern "c"` declarations, `@repr("C")` structs,
  type aliases, and integer constants directly from C header files.
- **HTTP web foundation (WEB-FOUND-1).** Added `Request`, `parse_request`, and `build_response`
  in `stdlib/net/http.orl` supporting full request extraction and server responses with structured headers.
- **Runtime control and observability (RUNTIME-CTRL-1).** Added value-based pseudo-random number
  generator `ori.random.Rng` (`new_rng`, `next_int`, `next_range`) for reproducible simulations
  and generational container `ori.slotmap.SlotMap` in `stdlib/slotmap.orl` rejecting stale keys.
- **Extensible namespaced attributes (META-ATTR-1).** Added parser and type checker support for
  declarative namespaced attributes (e.g. `@editor.inspect`, `@editor.range(min: 0, max: 100)`,
  `@schema.table(name: "users")`, `@route.get(path: "/api")`) on top-level items without macro overhead
  or compile-time side-effects.
- **Mutable views and spans `ori.span` (GFX-VIEW-1).** Added `ori.span` module in `stdlib/span.orl`
  providing zero-copy, mutable window views over contiguous buffers. Provides `from_buffer(buf, offset, len)`,
  `len(s)`, `is_empty(s)`, `get(s, i)`, `set_at(s, i, v)`, `fill(s, v)`, and `subspan(s, offset, len)`
  with native runtime `OriSpan` backing and automatic ARC liveness preservation.
- **Contiguous numeric buffer `ori.buffer` (GFX-BUFFER-1).** Enabled high-performance,
  fixed-length, mutably indexable numeric `buffer[T]` backed by `OriBuffer` in the native runtime.
  Provides `ori.buffer.new(size)`, `ori.buffer.len(b)`, `ori.buffer.is_empty(b)`, `ori.buffer.get(b, i)`,
  `ori.buffer.set(b, i, v)`, `ori.buffer.fill(b, v)`, and `ori.buffer.as_slice(b)`, along with
  stdlib helpers `from_list` and `get_or` in `stdlib/buffer.orl`.
- **CLI program argument forwarding for `ori run` (DX-SCRIPT-1.0).** `ori run <file> -- <args...>`
  now forwards trailing command-line arguments to the executed Ori program in both Cranelift JIT
  and AOT compilation paths. The runtime arguments are accessible via `ori.os.args()` and
  `ori.args` helpers.
- **Recursive formatting with `--write` and `--check` (DX-SCRIPT-1.1).** `ori fmt` now supports
  directory recursion across all `*.orl` files, in-place file formatting with `--write` (`-w`),
  and dry-run check mode with `--check` (`-c`) that exits with code 1 if unformatted files are found.
- **Semantic code linter command `ori lint` (DX-SCRIPT-1.2).** Added `ori lint <path>` command
  and pipeline analyzing Ori source graphs for code quality and redundancy. Emits warnings for
  unused variable bindings (`lint.unused_variable`), redundant boolean comparisons (`lint.redundant_bool_comparison`),
  redundant if-boolean expressions (`lint.redundant_if_boolean`), double negations (`lint.double_negation`),
  and unnecessary `@cfg` attributes (`lint.unnecessary_cfg`).
- **Value types baseline benchmark suite (VALUE-PERF-1).** Added canonical benchmark kernels
  under `tools/bench/` (`vec3_add_loop.orl`, `mat3_multiply.orl`, `optional_scalar_loop.orl`) and
  runner `run_value_perf.sh` establishing baseline metrics for small non-escaping aggregates.
- **Experimental hosted `string` and `bytes` boundary across the JIT.** Public functions in
  hosted JIT modules can now return `string` and `bytes` or receive them as
  homogeneous parameters. `ori-embed` exposes `OriValue::String` and
  `OriValue::Bytes` as Rust-owned copies, with safe `as_str()`/`as_bytes()`
  accessors and exact embedded-NUL preservation for bytes. Opaque slices carry
  private session/module/generation identity rather than host-constructible raw
  pointers. Functions
  operating on strings (e.g. concatenation, `len`) and bytes (`to_bytes()`,
  `by.len`) are covered with regression tests; host function and callback
  registries strictly reject string/bytes parameters until Host ABI C callbacks
  are defined.
- **Bitwise and shift operators (GFX-BITWISE-1).** `&` (AND), `|` (OR),
  `^` (XOR), `~` (unary complement), `<<` (left shift), `>>` (right shift)
  now work on every integer type with the width preserved. `>>` is arithmetic
  on signed types and logical on unsigned; shift counts outside
  `0..bit_width` trap with `ori_abort_shift_overflow` (runtime and hosted
  modes). Precedence follows C: `~` > `<<`/`>>` > `&` > `^` > `|` > `and` >
  `or`. The checker emits `type.bitwise_type_mismatch`,
  `type.shift_type_mismatch`, and `type.unary_bitnot_non_integer` for invalid
  operands; CT-0 constant evaluation supports bitwise too. Native and C
  backends emit matching code. The GFX-BENCH-02 gradient kernel now packs
  RGBA with `(r << 16) | (g << 8) | b`.
- **Graphics benchmark suite (GFX-BENCH-1).** `tools/bench/graphics/` now
  ships six official software-rendering kernels — GFX-BENCH-01 fill,
  GFX-BENCH-02 gradient, GFX-BENCH-03 Bresenham lines, GFX-BENCH-04 triangle
  rasterization, GFX-BENCH-05 z-buffer, GFX-BENCH-06 vertex transform — with a
  runner (`run_graphics_bench.sh`) that compiles AOT, samples wall time, and
  reports medians, throughput (px/s, lines/s, tris/s, verts/s), and compile
  times. Each kernel prints a deterministic canary so output regressions are
  caught independently of timing noise. Framebuffers use `list[int]` until a
  contiguous `buffer[T]` lands (GFX-BUFFER-1); the depth buffer uses
  fixed-point ints; RGBA packing uses multiply-add until bitwise operators
  land (GFX-BITWISE-1).
- **`array[InlineStruct, size: N]` (GFX-INLINE-1).** A struct whose fields are
  all inline (`Inline(T)`: scalars, inline arrays, inline structs) is itself
  inline and can now be stored inside an `array` block with no ARC: `Vec3`,
  `Triangle`, and `array[Vec3, size: 8]` all compile and lay out contiguously.
  Struct literals, index reads/writes, field access through indexes
  (`verts[i].y`), struct fields holding inline-struct arrays, `ori.mem.size_of`
  (block-accurate), and JIT/AOT parity are covered. Structs holding a managed
  field stay rejected with `type.array_element_not_inline`, and the diagnostic
  now names the offending field; recursive structs (no finite size) are
  rejected as well. Spec 04 and the error catalog are updated.

### Fixed

- **Patched unsound transitive dependency and automated the audit.** The lockfile
  now uses `anyhow 1.0.104`, which fixes `RUSTSEC-2026-0190`; the Linux native
  CI route installs pinned `cargo-audit 0.22.2` and rejects future RustSec
  advisories.
- **DCE preserves observable evaluation (`AUD-OPT-1`).**
  The HIR optimizer now classifies unused expressions as `Pure`, `MayTrap`, or
  `Effectful`. Integer division/remainder/shift guards, indexing, runtime
  allocations (including `bytes` literals), interpolation, closures, contracts,
  and custom destructors are retained. Associated-call arguments and `match`
  guards participate in the use scan, so their bindings cannot be deleted.
  Unit allocation-retention gates plus AOT/JIT differential regressions cover
  division, remainder, shifts, indexing, failed contracts, and destructors at
  every optimization level.
- **Workspace strict Clippy is green again (`RUST-AUDIT-2`).** The HIR
  vectorizer's loop rewrite accepts a mutable slice instead of requiring a
  `Vec`, counter-increment matching uses exhaustive patterns, and unsafe hosted
  accessors have recognized `# Safety` sections. `cargo clippy --workspace
  --all-targets -- -D warnings` passes on 2026-08-25; format drift and
  false-success QA suppression remain tracked under `AUD-QA-1`.
- **Workspace test matrix rerun after optimizer/JIT changes.**
  `cargo test --workspace -- --test-threads=1` passed on 2026-08-25,
  including 381 multifile tests, 245 `ori_spec` tests, 9 JIT tests, 30 embed
  tests, 64 runtime tests, and all available unit/E2E/doc tests. Slow stress
  probes remain intentionally ignored.
- **Aggressive inlining preserves call-boundary semantics (`AUD-OPT-2`).**
  The leaf inliner accepts only stable scalar literal-derived arguments used at
  most once. Variable reads, calls, managed/allocating values, parameter
  contracts, closures, binding scopes, propagation, and `await` remain behind
  the call boundary until HIR gains explicit temporaries and binding IDs.
  AOT/JIT regressions cover ignored traps, mutable snapshots, ordered/single
  evaluation, contracts, destructors, and managed arguments.
- **ARC now preserves parallel ownership slots (`AUD-ARC-1`).**
  Registering the same child in two managed fields or collection positions now
  creates two retains, and each unregister removes exactly one. Redundant map
  and JSON registrations were removed, and registration/retain validation now
  shares the ARC-state lock with the refcount mutation. Runtime regressions cover
  explicit unregister, owner teardown, and parallel-edge cycle collection; a
  native AOT regression replaces one of two fields sharing a temporary child and
  reads the survivor with zero leaks. Native map lowering no longer duplicates
  the runtime-owned entry edges, and a list/map/channel matrix passes under AOT
  and JIT with zero leaks. An optional Valgrind test has an explicit required
  mode for supported QA environments.
- **Channels now own managed queued values (`AUD-CHANNEL-1`).**
  Sending a managed payload registers one channel edge per queue slot; receive
  transfers that edge into the result wrapper, and dropping a channel releases
  unreceived entries. Runtime and native concurrency regressions cover managed
  strings and collection handles. A four-sender race against close verifies that
  each successful send is drained exactly once, repeated pointers keep independent
  edges, failed sends take no ownership, and all allocations are released.
  Typed HIR lowering now uses separate scalar/managed runtime symbols, so a
  pointer-shaped integer cannot be misclassified and invalid managed payloads
  fail instead of entering the queue.
- **Async network operations now retain handles through completion (`AUD-NET-1`).**
  TCP, listener, and UDP readiness jobs retain their managed resource until the
  worker finishes, while connection/listener/socket state is mutex-protected
  against explicit close. Unix readiness uses a duplicated owned descriptor and
  reprobes after each 50 ms slice, preventing descriptor reuse after close;
  pending jobs rotate and cancelled jobs release their future/resource
  keepalives without running work. Runtime close/cancel regressions and native
  TCP/UDP tests cover the lifetime boundary.
- **C export ingress copies foreign strings and bytes (`AUD-FFI-1`).**
  Direct, optional, and result managed text/byte inputs now use runtime copy
  helpers before entering Ori-managed aggregates, preventing host buffer
  lifetime from escaping through ARC edges. A required-capable ASan+UBSan host
  regression frees and clobbers inputs, validates UTF-8 and byte-view failures,
  and checks returned ownership and allocation balance.
- **Invalid `-->` tokens no longer panic the parser (`AUD-PARSE-1`).** The
  lexer token now has a total diagnostic name, the malformed-source corpus
  covers `using value -->`, and a complete all-token diagnostic matrix guards
  every producible lexer variant.
- **Short `result`/`map` type forms recover without indexing panics (`AUD-PARSE-2`).**
  Parser recovery now checks fixed arity before reading type arguments. A
  matrix covers zero, one, exact, and extra arguments for built-in constructors
  in canonical brackets and legacy angle forms.
- **Deep unary expressions and patterns are bounded (`AUD-PARSE-3`, partial).**
  Recursive unary operators and `some(...)`/tuple pattern constructors now use
  the parser nesting budget. The robustness corpus covers 512 nested levels
  without stack overflow; broader recursive-constructor and LSP/CLI parity
  coverage remains open.
- **Long module-constant chains are bounded (`AUD-CT-1`, partial).**
  CT-0 dependency evaluation now stops at 128 recursive references with the
  typed `consteval.recursion_limit` diagnostic. A 2,048-constant regression
  confirms that checking cannot overflow the process; an iterative evaluator
  for accepted chains remains open.
- **Hosted managed values no longer expose raw-pointer construction (`AUD-EMBED-1`).**
  String/bytes inputs and returns are owned Rust values; the only remaining
  pointer-bearing value is an opaque slice token with runtime ownership and
  session/module/generation checks. A compile-fail doctest locks the public API.
- **Managed allocation size arithmetic is checked (`AUD-RT-1`, partial).**
  `ori_alloc` now rejects header-size overflow and allocation failure through
  the runtime abort contract instead of returning a pointer that callers might
  dereference. Set/map backing arrays now use the checked allocator too, graph
  and heap zeroed storage uses a checked `calloc` wrapper, and every count ×
  element-size multiplication is validated before entering libc. The remaining
  work is the boundary/failure matrix for all public allocation APIs. C-facing
  argument and JSON/hex conversion buffers now use fallible `try_reserve` paths;
  oversized capacities report the same runtime abort instead of unwinding.
  `io.read` and `files.read` now return a typed error for unrepresentable
  buffer requests; `string.repeat`, string padding, and length-aware stdout/
  stderr writes check multiplication, `usize`, and `isize` limits first.
- **Implementation-status audit corrected planning and user documentation.** The
  2026-08-24 code audit reopened semantic optimizer bugs, ARC/FFI safety,
  package integrity, LSP correctness, and incomplete features that had been
  described as finished. The canonical evidence and correction gates now live
  in `docs/planning/roadmap-code-audit-performance-architecture.md` and
  `docs/planning/BACKLOG.md`.
- **Focused strict Clippy cleanup and code modernization (RUST-AUDIT-2).** The
  earlier cleanup modernized map/filter closures, slicing, matching, borrows,
  and clones. Full-workspace strict Clippy is green again as of 2026-08-25;
  `cargo fmt --all -- --check` and the daily QA suppression paths remain open.
- **Hosted slice windows cross the JIT boundary.** Public functions may now
  return `slice[T]` (read-only windows over lists) and take them as
  parameters; `ori-embed` exposes them as opaque `OriValue::Slice` pointers that
  the host hands back to the runtime's checked accessors. Host functions and
  callbacks still reject slices.
- **`check_source` no longer advances the module generation.** A valid check
  used to bump the generation while leaving the executable JIT unchanged,
  silently invalidating handles for code that was never replaced. Checking now
  only records the accepted candidate source; only `compile_source` publishes a
  new generation. A regression proves handles stay callable across accepted and
  rejected checks.
- **Loop/if epilogues no longer emit invalid CFG for early exits.** When a
  loop body ended with `break` or `return`, `emit_while`/`emit_loop`/`while
  some`/`for` could leave the exit block unfilled (breaking the verifier and
  `alias_analysis`) or continue emitting instructions after a terminator
  (corrupting iterators). Epilogues now distinguish a real `break` (continue on
  a fresh block) from a `return` (fill the unreachable exit with a trap), and
  division guards reset `terminated` on their live continuation so the rest of
  the block is emitted. Iterator functions (`suspend`), async state machines
  with nested control flow, and async `break`/`return` are covered by
  regressions.
- **Async general state machine fills un-reached poll blocks.** When the async
  body terminated before a collected `await` was emitted (e.g. an early
  `return`), the dispatch target for that state was left without instructions
  and crashed `alias_analysis` at compile time. Unused poll blocks are now
  filled with an unreachable trap before sealing.
- **`@repr` now enforces its only supported contract.** The checker accepts
  exactly `@repr("C")`; missing, named, and unsupported string arguments emit
  `attr.invalid_arg` with an actionable correction. A driver regression covers
  all rejected forms.
- **C/debug string positions now match native Unicode semantics.** Generated C
  and the native global `len(string)` count Unicode scalar values and use the
  same unit for method length, slicing, indexing, `index_of`, `chars`, and
  direct iteration. Generated-C input rejects malformed UTF-8 instead of
  constructing an invalid `string`. The Unix regression requires `cc`, then
  compiles, links, and executes generated C with accented text, emoji, and an
  invalid-input case. C emission also mangles C reserved identifiers, so a
  valid Ori binding such as `char` no longer produces invalid generated C.
- **Constant folding no longer corrupts sized integers.** Folding rewrote every
  result to `int`, which widened `int8`/`int16`/`int32` and the unsigned types
  and desynchronised HIR from the narrower backend slot. Folding now wraps in
  the declared width and keeps the declared type, so `100i8 + 100i8` produces
  `-56` and `250u8 + 10u8` produces `4` at every `ORI_OPT` level. Collapsing an
  `if` expression is likewise limited to branches whose type already matches
  the expression's.
- **Constant folding no longer aborts on trapping division.** `x / 0` and
  `MIN / -1` overflowed inside the folder itself and killed the compiler with a
  Rust panic. Both are now left in the program for the runtime guard to report.
- **Loop strength reduction is guarded.** The closed form `n * (n - 1) / 2`
  (and its nested `n * n` variant) was applied unconditionally, so a loop with a
  negative or overflowing bound produced a different result from the loop it
  replaced. The rewrite now emits the closed form only for a bound in the range
  where it is exact, keeps the original loop as the fallback branch, and only
  substitutes bounds that are pure `int` expressions.
- **Unsigned integers use unsigned instructions.** `u8`–`u64` division,
  remainder, and the four ordering comparisons emitted the signed forms, so any
  `u64` above `i64::MAX` compared and divided as a negative number and printed
  with a minus sign. Both backends now emit the unsigned operations and route
  every unsigned width through the type-directed unsigned formatter.
- **Runtime aborts preserve prior output.** Native abort, panic, bounds, and
  process-exit paths now flush stdout before terminating, so messages printed
  before a runtime failure are not lost when stdout is redirected to a pipe or
  file. The generated C runtime follows the same rule.
- **Generated C is strict-dialect compatible.** The emitted runtime requests
  the POSIX feature set it needs for `nanosleep`, `gmtime_r`, and `getline`, so
  `ori emit c` output compiles under `-std=c11` without extra host flags.
- **Integer division reports the trap.** A zero divisor, or `MIN / -1`, raised
  `SIGFPE` and killed the process with no message. Both backends now check the
  operands and abort through the runtime with a named error.
- **Collection growth cannot overflow the allocator.** `list`, `set`, `map`,
  the trees, the graph, and the heap doubled their capacity without an upper
  bound and passed the wrapped byte count to `realloc`. Every growth path now
  shares one checked helper that reports `ori collection capacity exceeds the
  addressable maximum` and treats a null allocation as an error.
- **`ori compile` derives usable output paths.** A bare file name
  (`ori compile main.orl`) produced an empty project root and failed the
  incremental input scan; a project root (`ori compile .`) named the directory
  itself as the link target. `compile` now derives its default output the same
  way `build` does, and `--lib` places the library inside a project root
  instead of beside it.
- **Type errors no longer cascade.** A binding whose initialiser failed to
  resolve produced a second `expected 'int', found '<error>'` mismatch that
  buried the real diagnostic. Assignability checks now stay silent when either
  side already carries an error type.
- **Managed list mutation preserves ARC ownership.** Native lowering for
  `lists.set` now updates the old element edge before replacing it, and
  `lists.insert` registers the new element edge before the producer releases
  its temporary. Managed nested-list regressions cover both AOT and leak
  checking paths.

### Fixed

- **Generation-safe LSP background validation (AUD-LSP-3).** `ori-lsp`
  no longer blocks Tokio workers nor publishes stale diagnostics/indexes.
  Both `validate_uri` (didOpen/didSave) and debounced `didChange` capture an
  immutable `(uri, version)` snapshot, run `run_check*` via `spawn_blocking`,
  and re-validate debounce instant + document version immediately before
  committing the semantic index and publishing diagnostics with version.
  Regressions: `incremental_edit_after_emoji_uses_utf16_columns`,
  `incremental_edit_rejects_middle_of_surrogate_pair`,
  `incremental_edit_rejects_reversed_range`, `document_version_tracks_upsert_and_apply`,
  `generation_safe_stale_validation_is_discarded` (slow-first discard) plus full
  LSP matrix 31 unit + 13 e2e.

### Added

- **Structured conditional compilation.** `@cfg` now accepts typed
  `target_os`, `target_arch`, `target_family`, `execution_profile`, and
  manifest-declared `feature` predicates composed with `all`, `any`, and
  `not`. The complete file is parsed first; inactive top-level declarations
  are then removed before resolution, so checker, documentation, AOT, JIT,
  C/debug, and LSP indexes share one active program. Both manifest formats,
  CLI target/profile/feature selection, incremental fingerprints, diagnostics,
  normative specs, bilingual guidance, and regressions are included. Unknown
  target triples are rejected instead of being mistaken for OS-free targets,
  and the VS Code extension restarts the LSP when `ori.cfg.*` changes.
- **Hosted compiler session and scalar JIT.** Extended the experimental
  `ori-embed` Rust crate with persistent Cranelift modules, public-function
  metadata, generation-bound opaque handles, and validated homogeneous scalar
  calls (`bool`, `int`, `float`, up to four arguments). Invalid updates preserve
  the last accepted executable generation; successful replacements stale old
  handles while retaining retired code for safety. Aggregates/managed values,
  async execution, callback C ABI, traps beyond the scalar boundary, unload
  concurrency, and a versioned C ABI remain later slices.
- **Graphics evolution planning.** A re-audit of
  [`docs/planning/ORI_GRAPHICS_LANGUAGE_EVOLUTION.md`](docs/planning/ORI_GRAPHICS_LANGUAGE_EVOLUTION.md)
  (2026-08-16) recorded the real state of the numeric/CPU graphics program:
  `array[InlineStruct, N]` still requires an `Inline(T)` classification (all
  structs are treated as runtime-managed today), the bitwise surface is absent
  from `BinaryOp`, `ori.buffer` is only a managed stub, and no official graphics
  benchmark suite exists. The document and the backlog now carry the corrected
  implementation map (`GFX-INLINE-1`, `GFX-BENCH-1`, `GFX-BITWISE-1`,
  `GFX-BUFFER-1`, `GFX-VIEW-1`, `GFX-BCE-1`, `GFX-MIDEND-1`, `GFX-SIMD-1`,
  `GFX-ECO-1`) with a contract decision required for `ori.buffer` before the
  contiguous-buffer slice.
- **Scalar host-function registry.** `ori-embed` can now validate and bind
  trusted `extern host` `bool`/`int`/`float` symbols before JIT finalization.
  Their addresses are cached by the persistent module, with missing symbols,
  duplicate registrations, reserved runtime names, and signature mismatches
  rejected before a generation is published. The same registry now supports
  trusted integer callbacks with opaque `user_data`, stable callback IDs,
  arity-specific JIT dispatch, structured cancellation after unregister, an
  active-call lifecycle guard, and bounded synchronous reentrancy. C-header
  callbacks, thread dispatch, aggregate values, and the versioned C ABI remain
  future work.
- **Explicit hosted module unload.** `OriEngine::unload_module` now drops all
  executable generations for one module and makes its existing handles return
  a typed unavailable-module error. Concurrent frame/task coordination remains
  intentionally outside this first lifecycle slice.
- **Runtime identity queries for embedded hosts.** The native runtime now exports
  `ori_rt_version()` and `ori_rt_abi_version()` as borrowed, NUL-terminated C
  strings. Generated `--lib` headers declare both symbols so hosts can reject
  an incompatible ABI before calling exports.
- **Recoverable scalar hosted traps.** Persistent hosted JIT calls now use a
  thread-local runtime error slot and explicit return paths for contracts,
  `check`, integer division guards, and direct scalar list/string/bytes bounds
  checks. `ori-embed` exposes these failures as `OriExecutionError`; standalone
  AOT/JIT retains its abort policy, and arbitrary native faults remain outside
  the recovery boundary.
- **Persistent hosted-call benchmark.** Added `BRASA-ORI-CALL-001` as a
  release example and `tools/bench/hosted_scalar_calls.sh` to measure one
  million calls through one cached function handle without treating a
  machine-specific timing as a CI threshold.
- **`parse.nesting_too_deep`.** Deeply nested expressions, blocks, or types
  exhausted the stack and killed the process without a diagnostic. The parser
  now bounds nesting at 128 levels and reports it once. The CLI and the
  language server additionally run the front end on a thread with an explicit
  stack budget (`pipeline::with_frontend_stack`), so the bound does not depend
  on the platform's default stack size.

### Changed

- **General embedding implementation maps.** Added reviewable plans for the
  [hosted runtime and Host ABI v1](docs/planning/embedded-runtime-host-abi-v1.md),
  [static metadata and extensible attributes](docs/planning/static-metadata-attributes.md),
  [a persistent compiler service and modular JIT](docs/planning/interactive-compiler-service.md),
  and [value-type performance](docs/planning/value-types-performance.md). The
  Atlas and sole active backlog now distinguish those proposals from the
  implemented `compile --lib` baseline. Planning/version drift was corrected.
  The audit exposed permissive `@repr` validation and C/debug UTF-8 byte
  indexing; both gaps were then closed by `ATTR-REPR-1` and `BUG-UTF8-LEN`.
- **Cross-domain usability program approved.** Added implementation maps for
  structured `@cfg`, scripts/automation, runtime control and observability,
  Unicode text, web runtime primitives, an embedded execution profile, native
  binding generation, and a package-ecosystem protocol plan. The sole active
  backlog owns their IDs and priorities; domain frameworks and engine-specific
  syntax remain outside the compiler.
- **Documentation path/example checks are now a CI gate.** The Linux native-route job runs
  the Atlas path audit and selected canonical/inline documentation checks. The
  machine-readable registry now maps attributes through HIR lowering and maps
  stdlib text behavior through both native runtime and C/debug codegen. These
  checks prove paths and happy paths, not behavioral completeness; expansion is
  open under `AUD-QA-2`.
- **Documentation audit and Atlas.** Added the canonical documentation map in
  [`docs/ATLAS.md`](docs/ATLAS.md) and the machine-readable feature registry in
  [`docs/atlas/features.yaml`](docs/atlas/features.yaml). The registry links
  language features to implementation, tests, references, guides, and examples.
  `tools/qa/docs_coverage.sh` validates those paths and stale canonical links;
  `tools/qa/docs_examples.sh` validates the example corpus and `ori doc check`.
- **User documentation synchronized.** Corrected the S3 grammar index, current
  workspace/release baseline, linker behavior, ABI-1 aggregate bridges, async
  terminology, install examples, and historical-status labels. Added English
  and Portuguese guides for advanced language features, concurrency, interop,
  stdlib usage, debugging, and bootstrapping.
- **Documentation architecture clarified.** Normative specification, user
  guides, planning snapshots, and historical archives now point to their
  respective canonical sources. The book and performance pages explicitly
  distinguish generated or historical artifacts from current compiler status.

- **Mid-end fixed point without re-serialisation.** The optimiser detected its
  fixed point by formatting the whole module through `Debug` twice per round,
  which cost time and memory proportional to program size on every build. Each
  pass now reports whether it rewrote anything.

- **Cross-platform runtime ownership.** String-keyed maps now retain newly
  inserted managed keys and values before producers release their temporaries,
  fixing process-capture fields that could disappear on macOS and other
  allocators. The Linux-only stack-overflow guard is now isolated behind its
  platform boundary, so Windows and macOS builds keep their native crash
  behavior without compiling unavailable `libc` signal APIs.
- **Windows split-module linking.** Generated function-pointer wrappers are
  emitted by every owning object and use external object linkage, so callbacks
  can cross source-file boundaries. Portable Windows fallback metadata now
  includes the system randomness libraries for both MSVC and GNU targets.
  Versioned runtime metadata no longer contains a machine-specific raylib path.
- **Native target selection.** Release packages now select the host
  architecture for Linux, Windows, and macOS instead of assuming x86_64;
  macOS Apple Silicon packages therefore find their staged `aarch64` runtime.
- **No-Rust smoke checks.** CI now removes hosted-runner Rust locations from
  `PATH` before validating packaged installs, so the smoke jobs test the
  intended end-user environment instead of failing on preinstalled tools.
- **Driver modularization.** Native runtime discovery, ABI metadata validation,
  platform artifact naming, and linker argument construction now live in a
  dedicated driver pipeline module. The compile, run, test, and doctor routes
  keep their existing interfaces and behavior.
- **Project loading modularization.** Manifest parsing, dependency scope
  discovery, stdlib lookup, import resolution, and source-graph loading now
  live in a dedicated project pipeline module while preserving the existing
  compile, check, doc, test, and LSP behavior.
- **Frontend modularization.** Lexing, parsing, source checking, and their
  public driver entry points now live in a dedicated frontend module. Existing
  diagnostics, timing hooks, and `ori-driver` APIs remain unchanged.
- **HIR modularization.** Source lowering, stdlib enum registration, generic
  preparation, and native module splitting now live in a dedicated HIR
  pipeline module. Native compilation keeps the same object boundaries and
  incremental cache behavior.
- **Native execution modularization.** JIT execution and the native test
  harness now live in a dedicated execution module. The public `run_jit`
  route and `ori test` results remain unchanged.
- **AOT compilation modularization.** Incremental reuse, HIR-to-object
  generation, native linking, C-header emission, and debug-symbol sidecars
  now live in a dedicated compile module. `ori compile`, `ori build`, and
  shared-library output keep their existing behavior.
- **Pipeline interface stabilization.** Source loading now returns one
  `ResolvedSources` contract containing the loaded graph, semantic model, and
  dependency context. Cross-cutting timing diagnostics moved out of the
  project loader into their own internal module.
- **Historical focused Rust cleanup.** Runtime, codegen, and driver passed Clippy
  with warnings denied across library, binary, and test targets. Runtime C ABI
  exports keep stable `#[no_mangle]` symbols with minimal Rust visibility;
  graph traversal, loop emission, linker, debugger, documentation, and source
  loading now use domain state/request types instead of long parameter lists.
  The 2026-08-24 audit temporarily reopened the required gate; as of
  2026-08-31 workspace check, strict Clippy, scoped rustfmt, and `daily_fast`
  pass. Full-workspace formatting debt and observational sanitizer/coverage
  stages remain explicitly tracked as P2 QA work.
- **Bytes equality parity.** Native `bytes` equality now compares exact
  lengths and payloads, including embedded NUL bytes, through a registered
  runtime ABI function. The C/debug backend rejects this operation explicitly
  instead of silently comparing raw pointers.
- **Grammar/spec synchronization.** Corrected the normative EBNF to use the
  implemented S3 selective-import rename (`=`) and result patterns (`ok`/`err`)
  rather than the removed legacy spellings.
- **Semantic documentation audit.** Synchronized the normative chapters with
  the current runtime for embedded-NUL file reads, `ok`/`err` propagation,
  async `using` cleanup, contracts, and formatter defaults.
- **Audit regression matrix.** Added checker, runtime, native AOT, and C/debug
  regressions for logical `bytes` equality, while retaining the existing S3
  coverage for result patterns, trait receivers, async cleanup, contracts, and
  formatter behavior.
- **Linux integration project.** Added `examples/linux_log_report`, a
  multi-module log analyzer with filesystem I/O, CLI handling, a deterministic
  fixture, English/Portuguese usage notes, and a standalone native test module.
- **CLI argument lifetime.** `ori.args.get_or` now copies process arguments
  before the temporary argument list is released. The runtime keeps host argv
  strings outside ARC for their process lifetime, and a native regression
  covers passing a real argument to a compiled program.
- **Thin pipeline façade.** Every driver command now has a domain-owned module
  contract: frontend, project loading, HIR lowering, AOT compilation, native
  execution, formatting, and documentation. The documentation index,
  validation, signature extraction, Markdown rendering, and static HTML
  rendering moved to `pipeline/docs.rs`; `pipeline.rs` is now a small
  orchestration and re-export façade (392 lines) with no hidden child-module
  aliases.
- **Deterministic dependency snapshots (not yet reproducible builds).** Added `ori lock`, the deterministic
  `ori.lock` snapshot format, `ori lock --locked` validation, Git revision
  recording, and automatic lock refresh from `ori get`. A present lockfile is
  checked during project loading so stale dependency metadata fails before
  code generation. The resolver does not yet restore from the lock or verify
  content digests; that work is reopened as `AUD-PKG-2`.
- **Package namespace isolation.** Local imports now stop at the owning
  package boundary. Modules from dependencies are addressed with their
  package-qualified prefix (`package.module`), preventing same-named modules
  from colliding across dependencies.
- **Safe incremental native builds.** Successful native outputs are recorded
  in `.ori/incremental.json` and reused when the complete source graph,
  manifests, lockfile, compiler version, and build options match. Set
  `ORI_DISABLE_INCREMENTAL=1` to force a rebuild. Dependency-bearing projects
  without `ori.lock` rebuild conservatively; no partial HIR is restored.
  A rebuild now emits content-addressed objects under `.ori/modules/`, one per
  source file, and links all of them together. An implementation-only change
  recompiles that file while unchanged callers reuse their objects; public
  signature/layout changes naturally change the shared interface key. Shared
  libraries, dynamic global initializers and explicit debug instrumentation
  keep the safe monolithic route. The index reports how many source modules
  changed during a rebuild.
- **Native debug symbols.** Linux ELF output now receives a compact DWARF v4
  line table mapped to Ori functions and source lines, with an accompanying
  `*.debug.json` map for every platform. The map now includes each function's
  visible parameters, locals, pattern bindings and closure captures with
  source lines, so tooling has the same variable catalogue on Linux, macOS and
  Windows. The cooperative DAP continues to provide live values (including
  managed aggregates) on all three. Windows native linkers receive an explicit
  deterministic `*.pdb` path and debug switch; rich CodeView local-variable
  locations remain pending because Cranelift does not yet emit them. Missing
  `objcopy` or equivalent tooling leaves the valid binary intact and reports a
  warning.

- **Debugger cooperativo — `stackTrace` e `variables`.** O backend agora
  registra entrada/saída das funções Ori instrumentadas e envia ao agente a
  pilha de chamadas e variáveis escalares visíveis (`bool`, inteiros e floats)
  em cada evento `stopped`. A coleta também expõe campos escalares de `struct`
  (inclusive aninhados), `length`/`capacity` de listas por caminhos qualificados
  (`user.name`, `items.length`), frames async entre suspensões/retomadas e
  capturas de closures. Valores gerenciados sem leitura segura permanecem
  resumidos como `<managed>`. Strings e bytes gerenciados agora mostram uma
  prévia limitada e segura (bytes em hexadecimal); literais estáticos e buffers
  estrangeiros só são lidos quando registram um comprimento exato, e ponteiros
  não registrados continuam sem dereference. O protocolo JSON continua
  compatível com adapters que ignoram campos adicionais. Elementos de listas
  aparecem por referência DAP, com leitura limitada aos primeiros 64 itens na
  raiz e 8 por nível aninhado; optional/result, enums, mapas, conjuntos e
  coleções opacas expõem snapshots estruturados. O DAP também avalia
  expressões locais puramente sobre o snapshot parado, sem executar código no
  alvo. Regressão E2E: `ori-driver/tests/debugger.rs`.
- **Adaptador de debugger no CLI.** `ori debug <arquivo>` agora compila com
  instrumentação, conecta o agente cooperativo e oferece continue/step no
  terminal; `ori debug --dap` expõe o mesmo fluxo por um servidor DAP mínimo,
  incluindo breakpoints, pilha, escopos, variáveis escalares, snapshots de
  structs/optional/result/enums e coleções, elementos indexados, frames async,
  capturas de closures, prévias seguras de strings/bytes gerenciados e
  `evaluate` limitado ao snapshot local do último stop.
- **Integração do debugger no VS Code.** A extensão registra o tipo `ori`,
  inicia `ori debug --dap`, adiciona o comando `Ori: Debug Current File` e
  oferece uma configuração inicial de `launch.json`. O plugin Zed documenta o
  comando manual porque sua API atual não expõe um descriptor de debugger.
- Reduced large-module native compile time by indexing function signatures,
  collecting Cranelift references per function, looking up selected symbols
  directly, and generating named-function closure wrappers only on demand. The
  10,000-function AOT probe now completes in about 21 seconds on the
  development host instead of the previously reported roughly four minutes;
  complex managed/trait/async bodies keep conservative symbol visibility.
  Added opt-in pipeline-stage timings and strict ignored scaling guards for
  both `check` and `compile`.
- Shelved general scoped arenas until a real short-lived-allocation workload
  demonstrates a bottleneck and a measured prototype justifies region/lifetime
  semantics. A safe ID-based `Pool[T]` remains the preferred narrower design
  if demand appears.
- Defined the C emitter as a permanently partial synchronous debug/transpile
  route. Cranelift AOT/JIT remains the product and semantic reference; C
  maintenance covers invalid output, crashes, and wrong semantics inside the
  documented subset rather than async/concurrency parity.
- Shelved implicit first-class iterator objects. Inline `iter` + `suspend`
  remains the zero-allocation path for direct `for` consumption, and an
  explicit state struct implementing `core.Iterable` already covers stored,
  passed, and returned lazy producers.
- Established `0.3.8` as the living development baseline after the published
  `v0.3.7`; synchronized the Cargo workspace and packaged runtime metadata.
- Backend diagnostics now render declared type names instead of leaking
  internal `<def DefId(N)>` values, including names nested in collection and
  function types.
- Generic associated-function calls now reject ambiguous matches when more
  than one bound provides the same receiver-less method, instead of selecting
  whichever bound happened to appear first.
- The C backend reports an ordinary unsupported-operation diagnostic if an
  opaque collection reaches an invalid equality path instead of panicking.
- Updated the VS Code/Cursor and Zed extensions for the current Ori surface: canonical match cases, compact trait application, `newtype`, struct destructuring, conditional payload bindings, current literals, and delimiter pairing.
- Refreshed the examples catalog and source programs for the current language surface: match-expressions, or-patterns, struct destructuring, `newtype`, local inference, capacity-aware collections, and readable English comments. Fixed the standalone const-generic probe and validated every `.orl` example with `ori check`.

### Adicionado
- **CT-0: expressões constantes sem keyword nova.** Argumentos constantes de
  tipos e comprimentos de `array` agora aceitam constantes inteiras/booleanas
  de módulo (inclusive importadas), aritmética inteira checada, comparações,
  lógica booleana e `if` inline. O avaliador roda antes do lowering de tipos e
  independe do backend. Overflow, divisão por zero, ciclos, inicializadores que exigem
  runtime, incompatibilidade escalar e resultado final não inteiro têm
  diagnósticos próprios. Chamadas, alocação, I/O, ambiente, FFI e macros ficam
  fora do CT-0; parâmetros const simbólicos continuam aceitos diretamente, mas
  `cap + 1` foi adiado para uma futura etapa de monomorfização.
- **Destrutores customizados com `core.Destructor`.** Structs e enums podem
  implementar `mut destroy(self)` para executar limpeza automática quando a
  última referência ARC morre. O callback roda uma vez, enquanto o payload e
  seus campos ainda estão legíveis; depois o runtime libera o payload e
  propaga a liberação pelas arestas registradas. A DCE preserva construções
  cujo descarte é observável, inclusive quando o binding não é lido. Em ciclos,
  todos os callbacks rodam antes que qualquer payload do ciclo seja liberado.
  O backend C rejeita o recurso com `backend.c_unsupported`, em vez de omitir a
  limpeza silenciosamente. Para fechamento determinístico, `using` +
  `core.Disposable` continua sendo a ferramenta apropriada.
- **`optional` e `result` diretos em `@c_export`.** O ABI gerado divide
  `optional[T]` em tag `bool` + payload e `result[T, E]` em `OriResultTag` +
  payloads `ok`/`error`; retornos usam parâmetros `out`. Somente a variante
  ativa é lida ou escrita. Strings e handles ativos transferidos ao host
  seguem o mesmo contrato ARC dos retornos diretos. O header gerado declara
  `OriResultTag`, as assinaturas expandidas e o ownership. Um host C real cobre
  `some`/`none`, `ok`/`error`, handles gerenciados, strings estrangeiras e
  retorno ao baseline de alocações.
- **Handles opacos para structs gerenciadas em `@c_export`.** Structs não
  genéricas e não vazias com `string`, structs aninhadas ou coleções agora
  aparecem no header como tipos incompletos `OriTypeHandle`. Parâmetros são
  emprestados — o wrapper retém uma referência temporária — e retornos
  transferem uma referência ARC ao host, liberada com `ori_arc_release`.
  A mesma retenção torna seguro repassar como parâmetro uma `string` antes
  retornada por Ori. O host C E2E cobre empréstimos repetidos, alias retornado,
  liberação de campos internos e contador de alocações de volta ao baseline.
- **Header C gerado para `ori compile --lib`.** Todo build de biblioteca
  bem-sucedido agora grava também o `.h` irmão, com guards C/C++, tipos
  escalares portáveis, `typedef` das structs exportadas, assinaturas
  pointer/out, lifecycle do runtime e nota de ownership para retornos
  `string`. Os hosts C E2E incluem esse arquivo em vez de repetir a ABI à mão.
  Nomes customizados de `@c_export` agora precisam ser identificadores C
  portáveis (`attr.c_export_bad_name`).
- **`@c_export` de structs escalares.** Structs não genéricas e não vazias,
  compostas apenas por números e `bool`, atravessam a ABI por ponteiros:
  parâmetros usam `const Type *` e retornos usam um último `Type *out`. O
  wrapper traduz o layout C naturalmente alinhado para o layout interno Ori,
  libera temporárias de retorno e mantém ownership no host. O teste E2E com um
  host C real cobre padding misto, ida/volta e ausência de crescimento nas
  alocações vivas. O contrato também explicita `int` como `int64_t` portátil
  (em vez do `long` dependente de plataforma usado na documentação anterior).
- **Benchmark de churn de temporárias gerenciadas.**
  `tools/bench/managed_temporary_churn.orl` reproduz dois milhões de
  concatenações curtas. Uma free list local por classes de tamanho foi
  implementada e medida, mas ficou cerca de 2% mais lenta que o `tcache` da
  glibc (0,355 s contra 0,348 s); a implementação foi revertida e o benchmark
  ficou como guarda para não reabrir a otimização sem uma carga diferente.
- **Funções associadas em bounds genéricos.** Uma função genérica pode chamar
  uma função associada declarada pelo bound, como `T.default()` sob
  `for T: core.Default`. A monomorfização resolve o `apply` concreto e ambos os
  backends emitem uma chamada estática sem receptor ou vtable.
- **Geradores: `iter` + `suspend` (inline, estilo Nim).** `iter counter(stop: int) -> int`
  declara um gerador; `suspend v` entrega um valor ao `for` consumidor e retoma
  no próximo passo. O corpo é **colado no laço** (transform AST→AST em
  `ori-hir/lower.rs`): zero função, zero alocação, zero máquina de estados —
  `break`/`continue`/`return` do corpo consumidor atravessam o inlining com a
  semântica de um `for` comum (cascata via flag, sem labeled breaks). Bench:
  cadeia `iter.map`+`iter.filter` ansiosa 330 ms vs gerador 24 ms (200k × 20).
  `iter` e `suspend` são contextuais (`import ori.iter = iter` e variáveis
  chamadas `suspend` seguem válidos). Limites B1 com diagnóstico dedicado:
  só funções livres, mesmo módulo, sem genéricos/variádicos/recursão, não
  `async`. Contratos de parâmetro (`stop: int if it > 0`) atravessam o
  inlining via `check` sintetizado no ponto de bind. Specs 02/03/06/07/13 +
  tour atualizados; 6 testes e2e novos.
- **Funções associadas: método sem `self` = sem receptor (destrava `core.Default`).**
  A regra agora é explícita: `self` declarado = método de instância; sem
  `self` = função associada, chamada como `User.default()`. `core.Default`
  virou trait real (`default() -> Self`), validado por `impl.missing_method`
  / `impl.wrong_signature`. Chamar associada numa instância dá
  `type.assoc_fn_instance_call`; `self` dentro de associada dá
  `bind.self_outside_method`. **Quebra intencional pré-freeze:** o `self`
  implícito (corpo de `greet()` lendo `self.name` sem declarar) foi
  removido — contradizia todos os exemplos da spec; 6 testes internos
  atualizados para `self` explícito. Binds (`compare = compare_points`)
  inalterados. Specs 08/12/13/18 atualizadas; 4 testes e2e novos.
- **`check` que falha agora imprime o motivo.** Uma asserção violada gerava
  trap silencioso (SIGILL, exit 132, zero output). Agora o backend nativo
  roteia a falha por `ori_panic`: `ori panic: check failed: <mensagem>`
  no stderr (ou `check failed` sem mensagem) e aborta. `ori test` passa a
  mostrar o motivo da asserção que derrubou o teste.
- **Driver: erro de lowering não grava mais binário.** `ori compile` e
  `ori emit c` prosseguiam para codegen/link mesmo com diagnóstico emitido
  no lowering (ex.: iterador recursivo), deixando um binário órfão com
  exit code 1. Agora o gate `sink.has_errors()` é reavaliado após o
  lowering, como o JIT e o `ori test` já faziam.
- **Livro Ori (rascunho `0.3.x-book.2`).** Narrativa + processo + prática +
  consulta em português sob [`docs/book/`](docs/book/README.md) (22 capítulos,
  apêndices, template e mapa de exemplos). Índices
  [`docs/README.md`](docs/README.md) / [`docs/README.pt-BR.md`](docs/README.pt-BR.md)
  apontam para o livro. Não substitui a spec normativa em `docs/spec/`.
  Revisão: snippets principais validados com `ori check`; caps 3/10/12/13/15/17
  e soluções de exercícios aprofundados; multiarquivo documenta `ori.proj`+`entry`.
  A geração de PDF/HTML é opcional e os artefatos não são versionados; a fonte
  canônica está em [`docs/book/`](docs/book/), com o fluxo em [`tools/book/`](tools/book/).

### Mudado
- **ARC: elisão de RC no return (LANG-MEM-4, ação 1).** `return x` de um
  local managed transfere o +1 do binding ao caller — o par
  `retain`(retorno) + `release`(cleanup do frame) não é mais emitido.
  Builders no padrão `const xs = ...; return xs` ficam com zero operações
  de RC.
- **DX: `ORI_DUMP_ARC` (LANG-MEM-7).** Com `ORI_DUMP_ARC=1` (ou caminho de
  arquivo), o compile imprime por função as operações ARC inseridas
  (contagens + sequência) — análogo do `--expandArc` do Nim, para medir
  elisões e auditar inserções.
- **ARC: collector de ciclos incremental (LANG-MEM-3).** O passe
  cooperativo deixou de escanear o heap inteiro: `ori_arc_release` registra
  candidatos a raiz de ciclo (decremento para >0 em objeto com edges de
  saída) e o passe roda trial deletion apenas no subgrafo desses suspeitos
  — O(subgrafo), não O(alocações vivas). O threshold cooperativo (256)
  agora adapta pela eficácia do passe (encolhe ×2/3 quando eficaz, cresce
  ×1,5 quando não; bounds 64–65 536; `ORI_COOPERATIVE_COLLECT_THRESHOLD`
  pina o valor). `ori.test.collect_cycles`/`assert_no_leaks` e o símbolo
  `ori_arc_collect_cycles` mantêm o full scan. Spec 10 atualizado; nota:
  [`docs/planning/historico/nim-study-2026-07-17-c3.md`](docs/planning/historico/nim-study-2026-07-17-c3.md).

### Corrigido
- **ARC de payloads gerenciados em `optional`.** O wrapper nativo registrava o
  payload como uma aresta ARC e também o liberava manualmente no destrutor; a
  cascata genérica liberava a mesma referência uma segunda vez. A aresta
  registrada agora é a única dona da liberação em cascata. Regressões no
  runtime, AOT e JIT cobrem `path.relative("a/b/c", "a/b")`, que antes
  abortava ou retornava texto corrompido.
- **Inventário interno da ABI nativa.** O símbolo `ori_bytes_eq` agora integra
  a lista testada de imports diretos do codegen, restaurando a guarda que exige
  documentação explícita para cada dependência interna do runtime.
- **Smoke local dos exemplos.** `tools/qa/examples_smoke.sh` agora escolhe o
  binário Ori de desenvolvimento ou release dentro de `compiler/target`
  quando `ORI_BIN` não é informado, sem exigir uma instalação global durante
  o QA do repositório.
- **VS Code/Cursor:** o VSIX volta a incluir `vscode-languageclient` e suas
  dependências de runtime. O pacote `0.3.5` era gerado com
  `--no-dependencies`, falhava ao ativar com `Cannot find module
  'vscode-languageclient/node'` e, por consequência, deixava autocomplete,
  hover, diagnostics e comandos LSP inoperantes. O smoke agora empacota o
  VSIX e verifica explicitamente a presença do módulo. A extensão também
  reconhece `.oridoc` com grammar própria e destaca expressões dentro de
  f-strings simples e multilinha como código Ori. O sidecar `ori.io` também
  deixou de duplicar três símbolos Layer 1, restaurando `ori doc check` no
  projeto temporário do smoke; os fixtures E2E do LSP agora usam o import S3
  canônico (`import path = alias`) e executam o `ori-lsp` recém-compilado por
  Cargo, não um binário antigo encontrado por fallback em `target/debug`.
- **ARC: args owned nas coleções restantes + arestas de graph canônicas.**
  Os emissores especiais de `hash_table`/`graph`/`heap`/`tree`/
  `linked_list.find` e as listas-fonte de `maps.from_entries`/
  `sets.from_list`/`heap.from_list` não liberavam chaves/valores
  temporários owned (6–12 alocações vazadas por punhado de operações com
  strings frescas). Todos seguem agora a regra uniforme "call → release do
  temp owned". O fix expôs um bug real no runtime do graph: as arestas
  guardavam o ponteiro cru do argumento — com nós deduplicados por
  conteúdo, a aresta podia referenciar um temporário liberado;
  `graph_add_node_raw` agora retorna o nó canônico armazenado e as
  arestas o utilizam. Regressão:
  `memory_arc::compile_runs_native_collections_owned_args_no_leak`.
- **Otimizador: capturas de closure e contratos de campo agora contam como
  uso/efeito no DCE.** O passe de dead-code (LANG-PERF-2-1) removia
  `const` capturados apenas por closures (o corpo vive numa função
  liftada) — toda closure com captura falhava no codegen nativo com
  `closure capture X is not available`, inclusive across await — e
  removia literais de struct com contrato de campo violado (o trap de
  contrato sumia). Capturas agora são usos; literais de tipos com
  contrato têm efeito. Isso zerou os últimos 9 testes vermelhos do repo
  (8 em `multifile_imports` + 1 async); 4 deles tinham expectativas
  calibradas no comportamento bugado e foram atualizados (showcase com a
  linha `Displayable`, catálogo real de `examples/`, e os 2 de build com
  bindings realmente usados).
- **Resolução de nomes: binding local agora sombreia builtin sem prefixo
  (LANG-FRONT-1).** `const len: int = lists.len(xs); return len` falhava
  no codegen nativo com `undefined variable ori_len` — o identificador de
  segmento único era resolvido para o símbolo do builtin (`len` → `ori_len`)
  antes de consultar os bindings locais. Locais têm precedência agora;
  chamar o builtin continua funcionando no mesmo escopo. Regressão:
  `ori_spec::compile_runs_local_binding_shadows_bare_builtin`.
- **ARC: wrappers `result`/`optional` do runtime nativo eram invisíveis ao
  ARC (LANG-MEM-9).** `new_result*`/`new_optional_ptr` usavam `malloc` cru:
  releases do codegen eram no-ops e toda chamada de `ori.fs`/`ori.process`/
  net vazava o payload (12 alocações a cada 10 `fs.read_text_or`). Agora os
  wrappers passam por `ori_alloc` e possuem o payload via edge (dono único
  da cascata). Junto, o `try`/`?` foi corrigido: abandonava o wrapper owned
  no caminho ok e não consumia o +1 no caminho err (32 leaks a cada 10
  `try`); o payload extraído agora é sempre owned. Regressão:
  `memory_arc.rs` (`fs_read_text_or_loop`, `try_unwrap_loop`).
- **ARC: scrutinee owned de `match`/`if some` era vazado.** Um scrutinee
  managed fresco (ex. `match mk(i)` direto, sem binding) nunca era
  liberado — 1-2 alocações vazadas por execução do match; o mesmo em
  `if some(x) = mk(i)`. Agora os payloads extraídos ganham +1 próprio
  (registrados para cleanup por arm) e o wrapper é liberado nos dois
  caminhos. `none` também entrou na lista de expressões owned (o wrapper
  optional de `return none` ficava sem dono). Regressão: `memory_arc.rs`
  (`match_owned_*`, `if_some_owned_*`).
- **Codegen: crash do Cranelift com nomes de binding repetidos em match
  aninhado.** Reusar o nome de payload (ex. `value`) em matches aninhados
  de tipos nativos diferentes (float × string) derrubava o compilador com
  panic interno (`declared type of variable varN doesn't match type of
  value vNN`): a `Variable` era reusada de qualquer escopo sem checar o
  tipo. Agora só é reusada com tipo idêntico; caso contrário há shadowing
  léxico correto. Afetava `match`, `let`, `using`, `if some`/`while some`.
  Regressão: `memory_arc.rs` (`nested_match_same_binding_name_*`).
  Verificação completa dos 4 bugs reportados pelo projeto native-ori-ide:
  [`docs/planning/historico/bugcheck-native-ori-ide-2026-07-18.md`](docs/planning/historico/bugcheck-native-ori-ide-2026-07-18.md).
- **ABI nativa: zero-extension de argumentos de 8/16 bits para símbolos
  externos.** O Cranelift passava um `bool` (I8, ex. resultado de `sete`)
  com lixo nos bits altos do registrador; o runtime Rust (LLVM assume
  caller-extension SysV) lia o registrador largo e, p.ex.,
  `io.println(string("a" == "b"))` imprimia `fals` (length de "true" com
  conteúdo de "false"). Params < 32 bits nas declarações de runtime agora
  carregam `uext`. Pré-existente; exposto pela mudança de layout do C3 —
  o fix também fez 8 testes de `multifile_imports` marcados como
  pré-quebrados voltarem a passar.
  Regressão: `ori_spec::compile_runs_bool_from_string_eq_prints_correctly`.
- **ARC: maps e sets (args owned + posse do resultado de `get`).**
  `maps.set`/`sets.add` (e demais calls de map/set) vazavam o +1 de chaves/
  valores temporários owned; `maps.get` retornava valor borrowed do map que
  o codegen tratava como owned (o cleanup do binding roubava o +1 vazado —
  bugs pareados; corrigir um lado só causaria use-after-free). Agora todos
  os calls liberam args owned, `maps.get` faz retain do resultado managed e
  `maps.try_get` devolve optional que possui o payload via edge própria
  (como `try_remove` já fazia). Regressão: `memory_arc.rs`
  (`map_managed_values`, `set_owned_elements`, `map_get_value_survives_map_free`,
  `map_try_get_payload`).
- **ARC: temporários de string (print / f-string).** `io.print` com
  argumento fresco (concat, f-string, conversões) vazava o +1 do
  temporário, e cada parte/intermediário de f-string vazava (um f-string
  de 5 partes deixava 7 alocações vivas). Agora o branch de print libera
  args owned após a chamada e a interpolação libera intermediários e
  partes conforme consumidos (bindings borrowed preservados). Regressão:
  `memory_arc.rs` (`print_string_temps`, `fstring_intermediates`). Nota:
  [`docs/planning/historico/nim-study-2026-07-17-c2.md`](docs/planning/historico/nim-study-2026-07-17-c2.md).
- **ARC: dono único da cascata (native backend).** Structs/enums/tuples
  liberavam campos managed **duas vezes** no free do dono (dtor gerado
  `__dtor_*` + edge ARC registrada): um filho compartilhado com um binding
  vivo era liberado cedo demais (use-after-free), e stores de elementos que
  não liberavam o +1 do temporário owned vazavam — **literais de lista
  aninhada e `lists.push` vazavam em programas reais**. Agora as edges
  registradas são o único dono (regra uniforme: store → edge → release do
  temp owned); os hooks `__dtor_*` foram removidos. ADR:
  [`docs/planning/adr-arc-single-cascade-owner.md`](docs/planning/adr-arc-single-cascade-owner.md);
  Spec 10 §"Cascade ownership"; regressão: `memory_arc.rs`
  (`shared_child_*`, `nested_list_*`, `list_index_assign_*`,
  `field_assign_owned_*`).

### Notas
- Working tree after **v0.3.5**.
- **Web stack feature freeze v1** was documented in package files that were
  later removed from this repository. The web/package experiments are
  historical and are not part of the current Ori product surface.

### Adicionado
- **`ori compile --lib` (cdylib embed, P1):** shared library output with C ABI
  exports. Annotate free `public` functions with `@c_export` /
  `@c_export("name")` (`int`/`float`/`bool`/void). Runtime boot:
  `ori_rt_init` / `ori_rt_shutdown`; module globals via `__ori_module_init`.
  Links against staged `libori_runtime.so` (Linux). Plan:
  [`docs/planning/PLANO-CDYLIB-EMBED.md`](docs/planning/PLANO-CDYLIB-EMBED.md).
  Smoke: `tools/qa/embed_smoke.sh`, example `examples/embed/`.
- **Web stack (packages):** templates (S8 trim), `ori-web` (A/B/C + SEC8 +
  nested JSON + middleware + upload + keep-alive + B7 + custom sessions),
  `ori-web-app` generators, `ori-web-auth` (TOTP), `ori-web-session-sqlite`,
  demos (hello, notes, API, auth+2FA, upload, `blog_app`).
- **Runtime/stdlib for web:** `ori.crypto` argon2id + TOTP; `ori.net`
  `set_read_timeout_ms` / `set_write_timeout_ms`; package `native_libs`
  forwarded from path deps.
- **QA:** `tools/qa/web_sec8.sh`, `web_auth_smoke.sh`,
  `web_session_sqlite_smoke.sh` (daily_full S6b–S6d).
- **Windows install + PATH:** `tools/windows/Install-Ori.ps1` /
  `Uninstall-Ori.ps1` (+ `install.cmd` / `uninstall.cmd`). Copies the full
  package to `%LOCALAPPDATA%\Programs\Ori` (or Program Files with `-System`)
  and permanently updates User/Machine `PATH`. Bundled into Windows release
  zips via `smoke_native_release.ps1`. Developer helper
  `tools/update_global.ps1` now packages then installs.
- **Windows Scoop-style bootstrap:** `tools/windows/get.ps1` for
  `irm …/get.ps1 | iex` (env: `ORI_VERSION`, `ORI_FORCE`, `ORI_SYSTEM`, …).
- **Editor extension release packages:** `tools/package_editor_extensions.sh`
  builds `ori-vscode-orl-<ver>.vsix` and `ori-zed-<ver>.zip` for GitHub
  Releases (VS Code/Cursor VSIX + Zed dev-extension zip with prebuilt wasm).
- **CI Windows packaging:** PowerShell scripts pass `--manifest-path` *after*
  the cargo subcommand (required by Cargo 1.95+), unblocking the MSVC zip.

### Documentação
- Stack index + packaging note: [`docs/README.md`](docs/README.md) and
  [`docs/install.md`](docs/install.md); the former package index was removed
  with the historical web experiments.
- Freeze policy: the current language/ABI policy is
  [`docs/planning/freeze-and-abi-gates.md`](docs/planning/freeze-and-abi-gates.md).
- Phase B/C/D, middleware, ori-sqlite symlink README.
- Windows installer flow in `docs/install.md` / `install.pt-BR.md` and
  `tools/windows/README.md`.
- Root README / README.pt-BR Quick start: end-user Windows one-liner + Linux
  package; website getting-started (EN/pt) updated for 0.3.5 Windows package.

---

## [0.3.5] — 2026-07-14

### Adicionado
- **Multi-OS release packages (DIST):** GitHub Actions `release.yml` builds
  Linux (tar.gz + deb), Windows MSVC (zip), macOS Apple Silicon + Intel
  (tar.gz). `native-route` smoke-no-rust enabled for Windows and macOS.
  Docs: `docs/install.md` / `install.pt-BR.md`.
- **`ori.crypto` (C10):** argon2id password hash/verify in runtime + stdlib.
- **Web stack packages:** `ori-templates`, `ori-web` (A+B+C), `ori-web-app`
  generators, demos (`ori-web-demo*`, `blog_app`).
- **`packages/ori-templates` (MVP):** server-side HTML templates for Ori —
  delimiters `@{ }`, comments `@{-- --}`, escape default, `|> raw` last-stage only,
  `if`/`elif`/`else`/`for`/`include`/`layout`/`assign`, path jail, `.orix` files.
  Design: [`docs/planning/web-templates-discussion-roadmap.md`](docs/planning/web-templates-discussion-roadmap.md)
  (D3–D28). Smoke: `packages/ori-templates/examples/smoke`.
- **`packages/ori-web` (MVP):** minimal HTTP Library layer — router
  (`get`/`post`/… + `:id`), static jail, in-memory session (`ori_sid` HttpOnly),
  CSRF synchronizer, flash, `dispatch`/`serve`, security headers baseline.
  Public entry is **`dispatch`** (Ori reserves keyword `handle`). Helpers:
  `form_body`, `is_htmx`, form-urlencoded decode (`+` / common `%XX`).
  Design D14–D20; smoke: `packages/ori-web/examples/hello_server`.
- **`packages/ori-web-demo`:** HTML-first notes app on `web` + `templates`
  (layout, partials, CSRF form, PRG redirect, htmx partial POST, static CSS).
  Design D19/§11; run: `packages/ori-web-demo`.
- **`ori-web` phase B:** rate limit (`set_rate_limit`), CSP (`set_csp`),
  trust-proxy client keys, file session store (`use_file_sessions`), session
  idle/absolute timeouts, `session_regenerate`, `json`/`too_many_requests`,
  `require_secret` / `ORI_WEB_ENV=production`, Permissions-Policy + edge TLS docs
  (`packages/ori-web/docs/phase-b.md`). App config uses module globals (avoid
  copying `App` with embedded lists — ARC crash).
- **`packages/ori-web-demo-api`:** JSON notes API (port 3458) with CSRF header.
- **`packages/ori-web-demo-auth`:** login/dashboard (port 3459, `demo`/`demo`),
  file sessions, regenerate on login.
- **`packages/ori-web-app` (Level 3 App):** Rails-like conventions — `standard_app`,
  `render`/`page_data`/`csrf_field`, boot `run`, generators `bin/new` and
  `bin/generate-controller`. Example scaffold: `packages/blog_app`.
  Design §12 / APP8–APP9.
- **`ori-web` phase C helpers:** login lockout (`login_fail`/`login_allowed`),
  audit log (`set_audit_log`/`audit`), re-auth (`mark_reauth`/`require_reauth`),
  CSRF rotation (`set_csrf_rotate`), optional `__Host-` cookie. Docs:
  `packages/ori-web/docs/phase-c.md`. Auth demo uses lockout + audit.
- **`bin/generate-scaffold`:** resource with index/new/create + form views.
- **`ori.crypto` (C10):** `password_hash` / `password_verify` — **argon2id** PHC
  via `ori-runtime` (`argon2` crate). Stdlib `stdlib/crypto.orl` wrappers.
  Auth demo stores/verifies demo password with argon2id.
- **`bin/generate-model`:** domain stub under `app/domain/` with optional
  password helpers.
- **Phase D docs:** `packages/ori-web/docs/phase-d.md` (proxy TLS, secrets, ops).
- **`packages/README.md`:** index of the web stack packages.
- **Web App conventions (design closed):** APP1–APP10 + security D15–D20 + Rails-like
  future D21 — same planning doc + learning course.
- **Runtime/DAP cooperativo (Ori IDE):** agent `debug_agent` no `ori-runtime` (`ori_debug_line` / `ori_debug_init`) ativado por `ORI_DEBUG_PORT`; codegen nativo instrumenta statements quando `ORI_DEBUG_INSTRUMENT=1` + `ORI_DEBUG_SOURCE=<path>`; adapter `ori-dap` (repo ori-ide) faz bind TCP e controla continue/step/breakpoints.
- **Polyglot performance harness** `tools/bench/polyglot/`: Ori AOT vs **Python,
  Rust, C, Go, JavaScript, TypeScript, Ruby, Nim** — kernels `sum_loop`,
  `fib_iter`, `list_sum`, `nested`; high-res timer; auto report under `results/`.
- **Performance docs:** [docs/guides/performance.md](docs/guides/performance.md)
  + [performance.pt-BR.md](docs/guides/performance.pt-BR.md); snapshot section on
  root [README.md](README.md) / [README.pt-BR.md](README.pt-BR.md); planning note
  in [docs/planning/perf-baseline-2026-07-13.md](docs/planning/perf-baseline-2026-07-13.md).
- **LANG-PERF-2 plan:** [docs/planning/historico/perf-runtime-midend-plan.md](docs/planning/historico/perf-runtime-midend-plan.md)
  — mid-end HIR opts, loop hygiene, strength reduction, inlining; ORC/LLVM deferred.
- **LANG-PERF-2-0/1/2/3/4 mid-end land:**
  - HIR mid-end `ori_hir::optimize` — const fold + DCE + pure-loop **strength
    reduction** (default); monomorphic **leaf inlining** under
    `ORI_OPT=aggressive` (also `none` / `default` / `2`)
  - `ORI_DUMP_CLIF=1` / path dumps Cranelift IR for defined functions
  - `tools/qa/perf_polyglot_smoke.sh` for fib+list smoke
- **LANG-PERF-2-5 list capacity API (additive):** `ori.list.with_capacity`,
  `ori.list.capacity`, `ori.list.reserve` (runtime `ori_list_*`); push/insert
  share `list_ensure_capacity`; slice/clone path pre-sizes. Polyglot
  `list_sum` uses `with_capacity` like Rust `Vec::with_capacity`.
- **Native list scalar hot path:** `list[int]` (and other non-managed integer
  slots) **inline** push (when capacity remains) and bounds-checked get in
  Cranelift — no per-iter `ori_list_push` / `ori_list_get` call. Managed
  element types keep the runtime + ARC edge path. `list_sum` ~1.25× Rust on
  the benchmark host (was ~1.8× after reserve alone).
- **Living QA kit:** `tools/qa/` daily stages and matrix
  [`docs/planning/qa/test-matrix-ori.md`](docs/planning/qa/test-matrix-ori.md);
  Spec 13 message-quality section + Spec index product facts.
- **Examples polish:** `collections_demo` shows `with_capacity` / `reserve` /
  `capacity`; `examples/README` links performance guide.

### Corrigido
- **LANG-PERF-3 — ARC registry linear scans made FFI-boundary cost grow with
  live heap size** (`~1.5ms` per extern call in large programs vs `~0.55µs` in
  small ones; Studio ImGui shell at ~2fps). `ori_arc_retain` / `ori_arc_release`
  / edge (un)registration resolved payloads by scanning a `Vec` of every live
  allocation under the global lock, so each call cost O(live allocations); frees
  also scanned every edge. The registry now keys allocations by payload address
  (`HashMap`) and indexes ownership edges by owner and by child, making
  retain/release/edge ops O(1) and `ori_arc_collect_cycles` O(n + e) instead of
  O(n²). Synthetic repro (extern call + managed temp per iteration, 10k live
  strings): 226µs → **1.5µs per iteration**, flat up to 100k live allocations.
  Regression guard: `performance_guard::run_ffi_boundary_cost_stays_flat_with_many_live_allocations`.
  Issue: [`docs/planning/historico/issue-ffi-dispatch-large-binary-2026-07-16.md`](docs/planning/historico/issue-ffi-dispatch-large-binary-2026-07-16.md).
- **LANG-MEM-3 partial — function-root cycle collect is amortized** (residual of
  LANG-PERF-3 lab: large live heap still ~2fps). Sync function roots and post-
  `await` dead-frame cleanup call `ori_arc_maybe_collect_cycles` instead of
  unconditional `ori_arc_collect_cycles`. A full trial-deletion pass runs only
  when the managed allocation counter advances by
  `ORI_COOPERATIVE_COLLECT_THRESHOLD` (default 256) since the last pass — same
  gate already used by the async executor. Explicit `ori.test.collect_cycles` /
  `ori_arc_collect_cycles` still force a full scan. C backend mirrors the gate.
  Spec: `docs/spec/10-memory.md`. Regression:
  `performance_guard::run_function_root_collect_stays_cheap_with_many_live_allocations`.
  Full suspect-buffer collector remains LANG-MEM-3 F3. Lab remeasure
  (`game-engine-full` `studio_shell`, AOT + release runtime): **~58fps avg**
  (48–60 over 36 `STUDIO-PERF` samples; was ~2fps); `DIAG-FFI` 100k×`app.fps()`
  = **5ms** total.
- **Native loops no longer call `ori_arc_collect_cycles` every iteration**
  (was triggered whenever a block entered with empty managed stack, including
  `while`/`for` bodies). Cycle collection is gated at function-root cleanups
  outside loops (see amortized `maybe` above). Tight integer loops drop from
  ~50× Rust to ~1.6× on `fib_iter` (20M steps) on the benchmark host; pure
  sum/nested closed forms via strength reduction drop further toward
  process-start noise.

### Notas
- Superfície S3 = **`[0.3.0]`**; inference B = **`[0.3.1]`**; package line **`[0.3.4]`**.
- Polyglot snapshot (2026-07-13/14, 9 langs, post GC fix + mid-end): Ori
  ~50–440× ahead of CPython; ~parity with Go on sum/fib/list; ~1.6× Rust on
  fib — see performance guide.

---

## [0.3.4] — 2026-07-13

### Notas
- Patch release: package smoke / linker living maintenance after `v0.3.3`.
- FREEZE-1 still open on `0.3.x`.

### Corrigido
- **Package smoke linker:** always prefer **SystemLinker** for release packaging.
  Auto-picking `RustcDriver` when `rustc` is on PATH broke AOT smoke by
  double-linking libstd against `libori_runtime.a` (`duplicate symbol:
  rust_eh_personality`). Hint added on that failure mode.
- **Linker diagnostics:** prefer high-signal messages (`duplicate symbol`,
  `cannot find -l…`) over the generic rustc “linking with cc failed” line.
- **SystemLinker:** multiarch `-L` + `cc -print-file-name=libc.so` /
  `-print-search-dirs` library paths; clear `LIBRARY_PATH` during link.
- **CI release:** package validated with **JIT + doctor** smoke
  (`ORI_PACKAGE_SMOKE_JIT_ONLY=1`) — GitHub-hosted runners still cannot AOT-link
  with multiarch `-lc` despite `libc6-dev`. Full AOT smoke remains the local gate
  (`tools/smoke_native_release.sh` without that env).

---

## [0.3.3] — 2026-07-13

### Notas
- Language-first implementation queue **closed** (LANG-DOC/PERF/RES done).
- **FREEZE-1** remains open on `0.3.x` (readiness: `docs/planning/freeze-and-abi-gates.md`).
- Linux release assets: **`.tar.gz` + `.deb`**.

### Adicionado (distribuição Linux)
- **`tools/package_deb.sh`:** builds `ori_<ver>_amd64.deb` (`/usr/lib/ori` +
  `/usr/bin/ori{,-lsp}`).
- **`tools/package_native_release.sh`:** also emits `.deb` when `dpkg-deb` is
  available.
- **CI `release.yml`:** publishes tarball **and** deb on tag `v*`.
- **Install docs:** deb path in `docs/install.md`.
- **Package smoke:** does not bundle non-portable `rust-lld` (needs libLLVM);
  AOT uses **SystemLinker**. BundledRustLld only if `rust-lld --version` runs.
- **Freeze readiness:** `docs/planning/freeze-and-abi-gates.md` (FREEZE-1 process
  finalized; window remains open on 0.3.x).

### Adicionado (editor DX local)
- **VS Code extension `0.3.3`:** discovery de `ori`/`ori-lsp` em
  `compiler/target/{debug,release}`; setting `ori.useAot`; install local via
  `.vsix` apenas (sem Marketplace). README alinhado ao monorepo.
- **Zed extension** `extensions/zed-ori` **0.3.3**: linguagem `.orl` + discovery de
  `ori-lsp` no PATH; install como **dev extension** (sem store).

### Adicionado / refatorado (exemplos P1–P4)
- **Catálogo enxuto (21 mini-projetos):** removidos/fundidos duplicatas
  (`hello_world`, `scratch_interp`, `release_smoke`, demos de collection
  isolados, `calculator`, `struct_demo`, `logic_and_matching`,
  `generics_showcase`, `map_set_graph`); `task_cli` → `cli_args`.
- **Novos:** `tests_demo` (`ori test` + `@test`), `using_fs` (streams +
  `using`), `async_io` (FS async), `multi_module` (+ `greeter.orl`),
  `concurrency` (spawn/join, channel, atomic), `random_format_iter`.
- **Polidos:** `collections_demo` (tour único), `language_features`,
  `native_showcase` (`Displayable` via `ori.core`), `async_demo`, `cli_args`.
- **`examples/README.md`:** trilha de aprendizado + catálogo alinhado.
- **Smoke/release:** `tools/smoke_native_release.*` usam `examples/hello`
  (em vez de `hello_world` removido).
- **Imports S3:** exemplos com 2+ imports usam bloco `imports … end`
  (não pilha de `import` soltos).

### Corrigido (linguagem / exemplos)
- **TLS / rustls:** enable feature `ring` + install default
  `CryptoProvider` so `connect_tls` / `http.get_tls` no longer panic at
  process start. Example `examples/http_get` runs again.

### Fechado (LANG-RES)
- **Native residual gate:** Spec 14 inventory confirmed; all official
  examples AOT-compile; regression
  `compile_runs_lang_res_product_surface_native` (for list/map/string/bytes/
  range, index assign, async await, using+dispose, spawn/join).
- Closure: `docs/planning/historico/lang-res-closure.md`. Reopen only with a concrete
  product program that hits `backend.native_unsupported`.

### Performance (LANG-PERF)
- **Cranelift product flags:** disable IR verifier; AOT `opt_level=speed`;
  JIT `opt_level=none` for faster `ori run` lower.
- **Default AOT linker:** prefer **BundledRustLld** when packaged/discovered
  (`runtime/bin/rust-lld`), then SystemLinker, then rustc driver. Measured
  `ori compile examples/hello` ~1.0 s (was ~2.5–4 s with system `ld`).
  Force: `ORI_USE_SYSTEM_LINKER=1` / `ORI_USE_BUNDLED_RUST_LLD=1`.
- **SystemLinker (Linux):** PATH discovery prefers **`mold` → `ld.lld` → `ld`**
  before `cc -print-prog-name=ld` (GNU-compatible drivers).
- **Stage runtime default:** `tools/stage_native_runtime.sh` / `.ps1` default
  to **release** (override `--profile debug` or `ORI_STAGE_PROFILE`).
- **Microbench:** `tools/microbench_lang_perf.sh` (check/run/compile samples).
- **ARC bench:** `tools/bench/arc_list_churn.orl` (list push + nested lists).
- **LANG-PERF closed** in `BACKLOG.md` (further JIT lower = living/Cranelift-bound).
- Numbers: `docs/planning/perf-baseline-2026-07-13.md`.

### Documentação (LANG-DOC — fechado como onda)
- Tour EN/PT: trait `Displayable` com `import ori.core`, `string(value)`, seção
  async; links para `examples/`.
- Cookbook PT alinhado ao EN (args, config, fs, time, HTTP, streams, pipe).
- Spec `01-overview` example: `ok`/`err` (não `success`/`error`).
- Guides errors/first-project/testing/install + índices: snippets com `module`,
  registry note, Zed + VS Code local, link a examples.
- Root **README** EN/PT: layout `main.orl` (não `src/`), editores locais,
  roadmap language-first, BACKLOG único, CLI package/registry atualizado.
- `ori new` documentado sem pasta `docs/` obrigatória.

### Adicionado (close-backlog Linux plan)
- **Linux-only distribution:** `release.yml` packages/publishes
  `x86_64-unknown-linux-gnu` only; Windows/macOS smoke jobs deferred
  (`if: false` on multi-OS smoke). Policy in `BACKLOG.md` + `docs/install.md`.
- **PKG-4:** `docs/planning/manifest-schema.md` + edge tests
  (`package_manifest_rejects_git_and_path_together`,
  `package_manifest_rejects_invalid_version`).
- **FREEZE-1 / ABI-1:** freeze window opened 2026-07-13; ABI enforcement in
  force (`ori-native-abi-1`, spec 19). Criteria:
  `docs/planning/freeze-and-abi-gates.md`.
- **STDLIB-4 MVP:** file async via L1 `fs.read_text_async` /
  `write_text_async` (`compile_runs_async_fs_read_and_write_native`);
  net offload via `*_in_background` + `task.run_blocking`.
- **STDLIB-4b:** await-able net I/O via worker-thread `OriFuture` —
  `net.connect_async` / `connect_tls_async` / `accept_async` /
  `read_some_async` / `write_all_async`. Gate:
  `compile_runs_net_connect_async_loopback`. Match pattern bindings now
  persist into the async frame (fixes Connection null after nested
  `await` / `match`).
- **STDLIB-4k:** shared I/O reactor with Unix `poll(2)` readiness for
  `accept_async` / `read_some_async` / `write_all_async` /
  `udp_recv_from_async` / `udp_send_to_async` (one reactor thread,
  multiplexed waits). Connect/TLS/FS async remain worker-backed.
  Gate: `compile_runs_net_udp_async_loopback`.
- **LANG-2 (closed):** C/debug real bodies for `string.*`, `io.eprint` /
  `read_line`, `convert.*`, `len`; matrix flags +
  `build_c_backend_compiles_convert_eprint_and_string_surface`. Prior
  slice: open_input shadow fix; trait/Displayable C tests green.
  C async remains **wontfix v1** (LANG-3).
- **STDLIB-5:** closed as wontfix — no mass L1→.orl ports (Layer 1 by design).
- **DOC-1:** `install.md` / `install.pt-BR.md` + tour links Linux-primary.
- Design: `docs/planning/historico/design-close-backlog-linux-2026-07-13.md`.

### Adicionado (packages / language)
- **PKG-1 / PKG-2 git dependencies:** declare
  `dep = { git = "url", rev|tag|branch = "...", version = "..."? }` in
  `ori.proj` or `ori.pkg.toml`. `ori get [path]` fetches into
  `ORI_PACKAGE_CACHE` / `~/.ori/packages` (`git/<url>/<ref>/` checkout +
  `name/version` layout). check/build auto-fetch git deps and resolve
  version-only deps from cache. Tests: `package_git_dependency_fetches_and_resolves_during_check`,
  `project_git_dependency_resolves_during_check_from_ori_proj`,
  `package_version_dependency_resolves_from_cache_after_install`.
- **PKG-3 registry + `ori publish`:** `ORI_REGISTRY` as directory or HTTP base;
  file layout `packages/{name}/{version}/` + `versions.json` + tarball;
  `ori publish <path> [--registry] [--force] [--token]`; `ori install name[@ver]`
  from registry; version pins fetch on cache miss. Contract:
  `docs/planning/registry-v1.md`. Tests: `package_registry_publish_install_and_resolve_on_check`,
  `package_publish_refuses_overwrite_without_force`.
- **LANG-1 async honesty:** promised native async subset treated as closed
  (coverage in `concurrency_async.rs`). Spec `14-backend-support.md` documents
  residual `backend.native_unsupported` as layout residual or non-async gaps;
  negative test `compile_rejects_for_iterable_without_native_abi`.

### Adicionado (stdlib)
- **STDLIB-2 `ori.net.http`:** HTTP/1.1 helpers in `stdlib/net/http.orl` —
  `build_request`, `parse_response`, `get`/`post`/`get_tls`/`get_plain` over
  existing TCP/TLS. Tests: `check_accepts_http_parse_and_build_request`,
  `compile_runs_http_get_loopback_native`. Example: `examples/http_get`.
- **STDLIB-3 file stream adapters:** Layer 1 `ori.io.open_input` /
  `open_output` (file-backed `Input`/`Output`); `using` accepts Input/Output
  (dispose → `close_input`/`close_output`). Test:
  `compile_runs_io_file_stream_adapters_native`.
- **STDLIB-1 canonical parents:** public surface is **`ori.X` only**.
  Layer-1 symbols and true Layer-2/3 helpers are imported via the parent path;
  nested `ori.X.utils` / `ori.X.algorithms` remain **silent compat** (not taught).
  Do **not** re-wrap same-named L1 entry points on the parent (shadowing breaks
  arity / monomorphization). True L2 lifts that remain: e.g.
  `ori.bytes.compare_lex` / `is_prefix_of` (from algorithms). Gate:
  `compile_runs_stdlib_parent_canonical_no_utils_import`. Policy:
  `docs/planning/stdlib-merge-policy.md`, `stdlib/README.md`.

### Documentação
- **Reorganização e padronização:** `docs/README.md` + `docs/README.pt-BR.md`
  (política: EN primário no GitHub, PT paralelo); `docs/language/tour` EN/PT;
  guias S3 atualizados (`first-project`, `cookbook`, `errors-null-void`,
  `report-bugs`, `testing`); `install.md` EN + `install.pt-BR.md`; índices de
  guides/planning; planos mortos em `planning/historico/`.
- **Backlog único:** `docs/planning/BACKLOG.md` — única lista ativa do que falta
  implementar (prioridade, dificuldade, dependências, waves). `PENDENTES`,
  `uso-real`, `roadtov1` apontam para ela.

---

## [0.3.2] — 2026-07-13

> **Package release** Win/Linux. M2 residual + M3 ABI + M1 Rust-indep fechados.
> `ori-game`/`ori-imgui` **fora do produto**. Auk9 arquivada. Ordem restante: **M4 self-host**.

### Removido
- **`packages/ori-game` e `packages/ori-imgui`:** fora do produto; removidos do
  repositório e dos planos de migração. `ori migrate-syntax` deixa de ter skip
  especial para esses paths.

### Adicionado
- **Release pipeline:** `.github/workflows/release.yml` — package Linux + Windows
  em tag `v*` e publica assets no GitHub Releases.
- **M1 / independência do Rust (usuário final):** `docs/install.md` S3-aligned;
  `tools/smoke_no_rust.sh`; smoke/package/stage scripts usam
  `compiler/Cargo.toml` + `compiler/target`, exemplos S3 e
  `examples/*/main.orl`; CI `smoke-no-rust-*` sem Rust no PATH.
- **Stdlib / public aliases de domínio:** `public alias` em
  `ori.fs` / `ori.io` / `ori.net` / `ori.json` / `ori.config` (+ `*/utils`).
  Teste `check_accepts_stdlib_public_type_aliases`.
- **M3 / ABI nativo documentado:** `docs/spec/19-abi.md` = **`ori-native-abi-1`**
  (layouts reais, ARC, mangling `ORI__*`, política de bump).

### Corrigido
- **Stdlib (ciclo string↔bytes):** `empty_bytes` sem import de `ori.string`.
- **Driver/M1:** `ORI_REQUIRE_PACKAGED_RUNTIME=1` prefere `<ori>/stdlib` empacotada.
- **Codegen/Link (SystemLinker):** resolve `ld` bare no `PATH` (GCC).

### Decidido (sem mudança de código)
- **Inferência global:** abandonada permanentemente; Ori permanece reading-first com anotações explícitas.

### Documentação
- **Stdlib/.oridoc (Layer 2/3):** criados **40 arquivos `.oridoc` sidecar** ao lado de todos os módulos `.orl` da stdlib (`stdlib/string.oridoc`, `stdlib/list.oridoc`, `stdlib/map.oridoc`, `stdlib/path.oridoc`, `stdlib/validate.oridoc`, `stdlib/time.oridoc`, `stdlib/fs.oridoc`, `stdlib/io.oridoc`, `stdlib/net.oridoc`, `stdlib/args.oridoc`, `stdlib/config.oridoc`, `stdlib/log.oridoc`, e os submódulos `*/utils.oridoc`/`*/algorithms.oridoc` de `bytes`, `concurrent`, `convert`, `deque`, `doubly_linked_list`, `format`, `fs`, `graph`, `hash_table`, `heap`, `io`, `iter`, `json`, `linked_list`, `math`, `net`, `os`, `process`, `queue`, `random`, `set`, `stack`, `test`, `time`, `tree`). Cada `.oridoc` documenta o módulo (`doc module self`) e todas as funções públicas (`doc func`) com `summary`/`param`/`returns` em inglês, seguindo a filosofia sidecar-first da spec `docs/spec/17-project-and-docs.md`. Todos validam com `ori doc check` (exit 0, zero `doc.symbol_not_found`). Os sidecars são empacotados nos releases (`stdlib/*.oridoc`) e disponíveis ao LSP hover. Layer 1 (runtime Rust, sem `.orl`) permanece coberta pela spec 12 + `ori doc export`.
- **Pacotes de distribuição:** gerados os artefatos de release `target/dist/ori-0.2.0-x86_64-pc-windows-msvc.zip` (Windows MSVC, ~46 MB) e `target/dist/ori-0.2.0-x86_64-unknown-linux-gnu.tar.gz` (Linux GNU, ~25 MB), ambos com smoke validado (`ori compile` + `ori test` + `ori run` JIT + `ori doctor`) em package isolado com runtime empacotado e stdlib incluindo os `.oridoc`.
- **Rede v2 / docs drift:** `stdlib-gap-parity.md`, `uso-real-pequeno-medio.md`, `PLANO-MATURIDADE-COMPLETO.md` (Apêndice C), `AGENTS.md`, `stdlib/README.md`, `docs/spec/12-stdlib.md` e `docs/spec/14-backend-support.md` sincronizados com TLS/UDP/servidor TCP síncronos entregues; backlog remanescente = rede async nativa.
- **Planejamento:** adicionado `docs/planning/uso-real-pequeno-medio.md` como plano ativo para levar Ori a 100% de usabilidade em projetos pequenos e médios; `PENDENTES.md`, `PLANO-MATURIDADE-COMPLETO.md` e o índice de planejamento agora apontam o plano mestre `0.2.0` como histórico/referência.
- **README:** reescrito o README principal em inglês com menu, overview completo, quick start, CLI, arquitetura, stdlib, tooling, release layout, limitações e roadmap; adicionadas traduções completas em português (`README.pt-BR.md`) e japonês (`README.ja.md`).
- **README:** removido o bloco de logo do topo das versões em inglês, português e japonês para evitar associação visual incorreta.
- **Linguagem/Planejamento:** adicionados `docs/planning/language-direction-decisions-2026-06-30.md` e `docs/planning/c-backend-redefinition.md`, registrando decisões sobre `try`, ARC + ciclos, mutabilidade, concorrência, FFI, pacotes, referências de linguagem, monomorfização e redefinição futura do C backend/`ori build`.
- **CLI:** `ori build` agora usa a rota nativa/Cranelift para construir arquivo ou projeto; a emissao C parcial foi movida para `ori emit c`.
- **CLI:** adicionado `ori new <path>` para criar um projeto app ou lib com `ori.proj`, `src/` e `docs/api/`.
- **CLI:** adicionado `ori repl`, um REPL inicial apoiado no JIT para imports, bindings simples, chamadas e expressoes curtas.
- **CLI/Testes:** `ori test <arquivo> --filter <texto>` agora executa apenas testes cujo nome completo ou curto contem o filtro; a saida mostra quantos testes foram descobertos e quantos foram selecionados. O comando LSP `ori.runTests` usa o mesmo filtro.
- **Pacotes:** adicionado parser/validador inicial de `ori.pkg.toml`, dependencias locais por `path`, cache local (`ORI_PACKAGE_CACHE` ou `~/.ori/packages`) e `ori install <name> --path <dir>`. O pipeline de `check/run/test/doc` agora resolve imports de dependencias locais declaradas em `ori.proj [dependencies]` ou `ori.pkg.toml [dependencies]`, incluindo entrada direta via `ori.pkg.toml`. Registry remoto e upload por `ori publish` continuam futuros.
- **Stdlib:** adicionados `ori.time` (`Instant`/`Duration`), `ori.log` (`error_message` para evitar keyword), `ori.args` e `ori.config` como modulos `.orl` de uso real pequeno/medio.
- **Exemplos:** adicionados exemplos reais e testados para organizador de arquivos, validador JSON, analisador de logs, CLI de tarefas e executor de processos.
- **Tooling/Release:** `tools/smoke_native_release.ps1` e `.sh` agora empacotam `ori-lsp` e `stdlib/`, alem de validar um programa que importa modulo `.orl` da stdlib dentro do pacote isolado. Novos scripts `tools/package_native_release.ps1` e `.sh` geram `.zip`/`.tar.gz` somente depois do smoke passar.
- **CI/Release:** workflow `native-route` agora gera artefatos de release por matriz (Windows MSVC/GNU, Linux GNU, macOS x86_64/aarch64) usando os scripts de package, que rodam smoke antes de produzir o archive.
- **CI/Release (smoke-no-rust):** novo job `smoke-no-rust` no workflow `native-route` que baixa o artefato `ori-linux-gnu`, extrai em um runner `ubuntu-latest` que **não tem Rust instalado** (validado com `command -v rustc`/`cargo`), instala apenas `build-essential`, e executa `ori doctor`, `ori run` (JIT), `ori compile` (AOT via SystemLinker), e `ori test`. Isso valida end-to-end que um usuário final pode usar Ori sem nunca precisar da toolchain Rust.
- **Tooling/VS Code:** adicionado `tools/smoke_vscode_extension.ps1` e `.sh` para compilar a extensao, validar JSONs, rodar LSP E2E e executar `check/run/test/fmt/doc/summary` em projeto temporario fora do repo.
- **Spec:** capítulos 02, 03, 04, 05, 06, 09, 10, 11, 13 e 14 sincronizados para documentar `try expr` como forma legível de propagação, `expr?` como forma compacta e o norte futuro para reduzir code bloat de monomorfização.
- **Instalação:** adicionado `docs/install.md` — guia completo de instalação para usuários finais por OS (Windows, Linux, macOS), com pré-requisitos do sistema, download do release package, verificação via `ori doctor`, troubleshooting, e variáveis de ambiente para override.
- **README:** seções "Known limitations" e "Roadmap" atualizadas para refletir que a independência do Rust para usuários finais está "mostly done" (JIT default + SystemLinker default para AOT), e que self-hosting é "deferred" (não pré-requisito para utilidade).

### Corrigido
- **Release/Linux:** `stage_native_runtime` agora registra `-no-pie` no `runtime-link.json` para Linux, inclusive quando usa `cargo --print native-static-libs`; o fallback do driver tambem usa `-lpthread`, `-ldl`, `-lm` e `-no-pie`; `ORI_USE_BUNDLED_RUST_LLD=1` descobre `runtime/bin/rust-lld` dentro do pacote e cai para paths GNU/Linux comuns quando `cc` nao existe, evitando falha `R_X86_64_64 ... recompile with -fPIC` no smoke de pacote Linux.
- **Formatter:** `ori fmt` agora preserva assinaturas obrigatorias de traits sem indentar como corpo de funcao, continua indentando metodos default e mantem a pilha interna alinhada apos `else`/`case`.
- **Async/Codegen:** corrigido `await` em loops profundamente aninhados (`for { while { await } }`) no backend nativo. A state machine geral recarrega valores vivos do frame apos retomada e evita reutilizar temporarios de blocos nao-dominantes em binarios como `total + await compute(value)`.
- **LSP:** lints agora respeitam `LintConfig`, incluindo desligar `unused_variable`/`prefer_const` e emitir `lint.shadowed_variable` quando habilitado; imports passam a entrar no indice semantico/completion, inlay hints respeitam o range pedido pelo editor e `ori.runTests` aceita filtro de teste.
- **VS Code Extension (bugfix):** Corrigido crash crítico na inicialização do Language Server devido ao uso de método inexistente (`config().onDidChange is not a function`), substituído pelo escutador correto `vscode.workspace.onDidChangeConfiguration`.
- **VS Code Extension (correção/UX):** Adicionado suporte completo a colchetes (`[` e `]`) em `language-configuration.json` para fechamento automático e envolvimento de seleções de listas e indexações no editor.
- **VS Code Extension (destaque/UX):** Adicionado destaque de sintaxe TextMate em `ori.tmLanguage.json` para as palavras-chave de concorrência `async` e `await`.
- **Driver/Pipeline (bugfix):** corrigido fallback de descoberta da stdlib root em `find_stdlib_root()` com varredura ascendente a partir do CWD, garantindo que `ori check/run` consiga resolver módulos `.orl` da stdlib (Layer 2/3) mesmo fora do diretório do workspace de desenvolvimento. Teste de regressão adicionado em `multifile_imports.rs`.
- **Tooling/Release:** `tools/smoke_native_release.ps1` agora inclui `ori doctor` na suite de validação do package isolado, verificando que o linker strategy ativo é reportado corretamente.
- **Doctor (bugfix):** `ori doctor` agora chama `NativeLinker::discover()` em vez de inferir o linker strategy a partir de variáveis de ambiente manualmente. Isso corrige a divergência entre o strategy real usado pelo compilador e o reportado pelo doctor. `NativeLinker` ganhou método `strategy_name()` para inspeção. Testes `doctor.rs` atualizados.

### Adicionado
- **Qualidade/Seguranca/Performance:** novas suites `security_robustness.rs` e `performance_guard.rs` no `ori-driver`, script Ori `tools/quality_metrics.orl` para coletar metricas em CSV/TXT, runner `tools/compare_language_workloads.ps1` para comparar Ori, Rust, C, Python e Node.js em workloads equivalentes, manual completo `docs/guides/testing-manual.md`, relatorio `docs/guides/language-comparison.md`, corpus adversarial de lexer/parser/checker, validacao de spans de diagnostico, escaping HTML de `ori doc`, smoke nativo com leak-check e budgets opcionais via `ORI_PERF_STRICT=1`. Documento de uso: `docs/planning/security-performance-testing.md`.
- **Parser/Checker:** `try expr` aceito como forma prefixada de propagação para `result<T, E>` e `optional<T>`, compartilhando a mesma semântica de `expr?`. Testes de regressão cobrem propagação de `result`, propagação de `optional` e rejeição em valores que não são `result`/`optional`.
- **Imports:** sintaxe de import seletivo `import origem only (nome, outro as alias)` adicionada sem reservar `only` globalmente. O checker valida membros selecionados na origem, detecta colisões locais com `bind.duplicate_alias`, reporta membro inexistente com `bind.import_member_unknown` e preserva `bind.unused_import` por nome importado.
- **Docs/Projeto:** `ori.proj` ampliado com `manifest`, `kind`, `[source]` e `[docs]` (`paths`, `mode`, `require_public`) mantendo compatibilidade com manifestos antigos que possuem apenas `entry`. Novo formato `.oridoc` para documentação externa de símbolos, carregado como sidecar (`foo.oridoc`) ou por pastas configuradas em `docs.paths`. `ori doc file` inclui docs externas, `ori doc check` valida sintaxe/símbolos/parâmetros/retornos, e o LSP usa `.oridoc` no hover de símbolos locais. Novos diagnósticos: `doc.syntax`, `doc.symbol_not_found`, `doc.missing_public`.
- **Stdlib/Ergonomia:** `ori.string`, `ori.list` e `ori.fs` agora têm módulos pai `.orl` achatados (`stdlib/string.orl`, `stdlib/list.orl`, `stdlib/fs.orl`) para import seletivo de helpers/algoritmos no namespace principal, por exemplo `import ori.string only (is_empty, truncate as cut)`. Os caminhos antigos (`ori.string.utils`, `ori.string.algorithms`, `ori.list.utils`, `ori.list.algorithms`, `ori.fs.utils`) continuam compatíveis. Imports normais de módulos runtime (`import ori.string as str`) continuam leves e não forçam o carregamento do módulo pai `.orl`.
- **Stdlib Layer 1 — uniformização FS/IO (backlog v2, breaking):** Funções FS que retornavam `bool` agora retornam `result<void, string>` (mutações: `append_text`, `delete`, `create_dir`, `create_dir_all`, `copy`, `rename`) ou `result<bool, string>` (queries: `exists`, `is_file`, `is_dir`). **`io.read_line`** agora retorna `optional<string>` (`none` em EOF). Runtime FFI migrado; Layer 2 `fs/utils.orl` e `io/utils.orl` simplificados para pass-through. Testes E2E + `spec_fs_and_json_contracts_match_stdlib_sig` estendido.
- **Ergonomia — `if then else` expressão (backlog v2):** Feature fechada — sintaxe `if cond then expr else expr`; HIR lowering corrigido para ramo `never`; `expr_accepts_inline_if_expression` inclui compile+run.
- **Toolchain pedagógica (backlog v2):** **`ori explain <code>`** — `ori-driver/src/explain.rs` imprime resumo, causa provável e correção sugerida para ≥15 códigos do catálogo; CLI `ori explain`. Testes: `explain.rs` (gate codes + unknown). **`ori summary [path]`** — `pipeline::run_summary()` lista entry, módulos descobertos, imports transitivos e contagem de diagnósticos; CLI `ori summary`. Teste: `summary.rs`. **Guia pedagógico** — `docs/guides/errors-null-void.md` (void/optional/result/check + tabela comparativa); linkado do `README.md`.
- **LSP/VS Code extension v0.2.2 (`[Unreleased]`):** Testes E2E LSP — `e2e_lsp_stdlib_layer2_hover` (hover em `ori.string.utils`) e `e2e_lsp_incremental_edit_completion` (sync incremental + completion). Extensão: doctor no Output Channel, comando **`Ori: Summary Project`** (`ori summary`), auto-discovery de `target/debug` e `stdlib/` a partir do workspace. Signature help para chamadas stdlib qualificadas via `stdlib_catalog::signature_for_call`.
- **LSP/VS Code extension (`[Unreleased]`):** Catálogo stdlib unificado em `ori-lsp/src/stdlib_catalog.rs` (Layer 1 runtime manifest + scan recursivo de `stdlib/**/*.orl` Layer 2). Completion/hover/goto para símbolos qualificados (`io.print`, `ori.string.utils.is_empty`) com resolução de aliases `import … as`. Sync de documentos **INCREMENTAL** (`TextDocumentSyncKind::INCREMENTAL` + `ProjectManager::apply_change`). Dot-complete ampliado: aliases de import, `value_sigs` top-level, tipos opacos. **`ori doctor`** — `pipeline::run_doctor()` verifica stdlib root, runtime AOT/cdylib, triple, linker strategy, modo `ori run`; CLI `ori doctor` + comando LSP/extensão `Ori: Run Doctor`. Extensão **`extensions/vscode-orl/`** (LanguageClient → `ori-lsp`, settings `ori.lsp.path`/`ori.compiler.path`/`ori.stdlib.root`/`ori.runtime.*`/`ori.useJit`, grammar TextMate, snippets, comandos Check/Run/Test/Format). Testes: 2 unitários `stdlib_catalog`, 2 integração `doctor.rs`. API pública: `find_stdlib_root`, `stdlib_source_path`, `stdlib_doc_signature`.
- **Stdlib/Gap parity — Layer 2/3 fechados (`[Unreleased]`):** Complemento ao ciclo gap parity — todos os módulos `.orl` planejados para paridade `std.*` v1 entregues. **Layer 2 novos:** `format.utils`, `iter.utils`, `net.utils`, `os.utils`, `random.utils`, `queue.utils`, `stack.utils`, `deque.utils`, `heap.utils`, `hash_table.utils`, `linked_list.utils`, `doubly_linked_list.utils`. **Layer 3 novos:** `map.algorithms`, `set.algorithms`, `string.algorithms`, `bytes.algorithms`, `math.algorithms`. **Expansões:** `validate.orl` (+`even`, `blank`, `in_range`, …), `path.relative` real, `concurrent.utils` (+`transfer_*`), `ori-types/lower.rs` registra `ori.net.Connection` para assinaturas `.orl`. Testes: `compile_runs_stdlib_layer2_remaining_utils`, `compile_runs_stdlib_layer3_algorithms_extensions`, `check_accepts_stdlib_gap_parity_imports` (imports ampliados). Docs: `stdlib-gap-parity.md`, `stdlib/README.md` atualizados com inventário completo + lacunas remanescentes para uso da linguagem.
- **Stdlib/Gap parity (Stdlib Phase 0 — paridade `std.*`, `[Unreleased]`):** Plano normativo `docs/planning/stdlib-gap-parity.md` (mapa de equivalência, lacunas fechadas, backlog remanescente). **Layer 2 (`.orl`):** `stdlib/validate.orl` (`ori.validate`), `stdlib/path.orl` (`ori.path`), `stdlib/json/utils.orl`, `stdlib/io/utils.orl`, `stdlib/fs/utils.orl`, `stdlib/time/utils.orl`, `stdlib/test/utils.orl`, `stdlib/process/utils.orl`, `stdlib/concurrent/utils.orl`; expansões em `string.utils` (`last_index_of`, `is_digits`, `has_whitespace`, `limit`, `replace_all`, `has_prefix`, `has_suffix`; `swap_case` via bytes ASCII), `bytes.utils` (`starts_with`, `ends_with`, `contains`, `join`, `from_list`, `to_list`), `math.utils` (`deg_to_rad`, `rad_to_deg`, `trunc_float`, `log10`, `abs_float`), `map.utils` (`has_key`, `is_empty`). **Layer 1 (runtime + manifesto):** `fs.file_size`/`modified_at`/`created_at`, `fs.create_dir_all`, `os.current_dir`/`change_dir`, `random.seed`, `process.run`/`run_capture`, `net.*` (TCP síncrono + `OpaqueTy::Connection`), `test.skip` (exit 77), `lazy.is_consumed` (codegen inline), `bytes.from_list`/`to_list`, `math.trunc`/`ln`/`exp`/`asin`/`acos`/`atan`/`atan2`/`log10`/`is_finite`. **Driver:** `ori test` trata exit 77 como skipped (`skip:` + contagem separada). **C backend:** stubs inline para novos símbolos `c_backend_runtime`. 14 testes E2E adicionais em `multifile_imports.rs` (validate, path, json/fs/io/time utils, gap parity expansions, Layer 1 os/lazy/math/process).
- **Codegen/Link (Rust removal Phase 1, Windows MSVC):** Nova estratégia `BundledRustLld` no `NativeLinker` que invoca `rust-lld` diretamente, sem usar `rustc` como driver de link. Opt-in via `ORI_USE_BUNDLED_RUST_LLD=1`. CRT discovery para Windows MSVC implementado via `vswhere.exe` + Windows SDK layout (`<VS>\VC\Tools\MSVC\<ver>\lib\<arch>` + `<WindowsKats>\Lib\<sdk>\um\<arch>` + `<WindowsKats>\Lib\<sdk>\ucrt\<arch>`), sem exigir `vcvarsall.bat` carregado. Descoberta de `rust-lld` em 3 níveis: `ORI_RUST_LLD` (override explícito) → `<ori.exe dir>/rust-lld[.exe]` (bundled no release package) → `<rustc sysroot>/lib/rustlib/<host>/bin/rust-lld[.exe]` (bootstrap). Fallback gracioso desabilitado quando opt-in: se `ORI_USE_BUNDLED_RUST_LLD=1` e a descoberta falha, erro actionable é emitido em vez de silently cair para `RustcDriver`. 6 testes de regressão em `native_backend/tests.rs`: `env_flag_treats_truthy_values_as_set`, `msvc_arch_dir_matches_target_pointer_width`, `discover_bundled_rust_lld_next_to_exe_returns_none_when_absent`, `vswhere_discovers_vs_install_or_reports_clear_error` (Windows-only), `msvc_crt_lib_dirs_resolve_to_existing_directories` (Windows-only), `bundled_rust_lld_strategy_falls_back_on_non_windows`.
- **Codegen/Link (Rust removal Phase 1, Linux GNU):** Estratégia `BundledRustLld` estendida para `x86_64-unknown-linux-gnu`. CRT discovery via `cc -print-file-name` (descobre `crt1.o`, `crti.o`, `crtn.o`) + `cc -print-search-dirs` (descobre lib dirs) + fallback de paths comuns (`/usr/lib/x86_64-linux-gnu`, `/usr/lib64`, etc.) para dynamic linker (`ld-linux-x86-64.so.2`). Link line `rust-lld -flavor gnu` ordena CRT objects corretamente: `crt1.o`+`crti.o` antes do obj+libs, `crtn.o` depois, com `-dynamic-linker`, `-L<dir>`, `-no-pie`, `-lc`. `cc` é usado apenas como discovery tool (não como driver de link) — o link real é feito por `rust-lld` diretamente. Estratégia estendida com campos `crt_pre`, `crt_post`, `dynamic_linker` no enum `NativeLinkerStrategy::BundledRustLld` (Windows MSVC usa esses campos vazios/None). Teste `linux_gnu_crt_discovery_resolves_existing_paths` (Linux-only, `#[cfg(target_os = "linux")]`) valida CRT objects + dynamic linker + lib dirs existem; `bundled_rust_lld_strategy_falls_back_on_non_windows` atualizado para validar flavor `gnu` e dynamic linker `Some` em Linux.
- **Codegen/Link (Rust removal Phase 1, macOS):** Estratégia `BundledRustLld` estendida para macOS (`x86_64-apple-darwin` e `aarch64-apple-darwin`). CRT/SDK discovery via `xcrun --show-sdk-path` (descobre SDK root) + `xcrun --show-sdk-version` (descobre SDK version) — requer Xcode Command Line Tools instalado. Link line `rust-lld -flavor darwin` com `-arch <arch>`, `-platform_version macos <deployment_target> <sdk_version>`, `-syslibroot <sdk_path>` em `extra_args`. CRT objects não passados explicitamente (darwin flavor handle implicitamente via `-platform_version` + `-syslibroot`). Deployment target default `10.12` (x86_64) ou `11.0` (arm64), override via `MACOSX_DEPLOYMENT_TARGET` env. Arch descoberto via `cfg!(target_arch)` (`x86_64` ou `arm64`). `lib_dirs`/`crt_pre`/`crt_post`/`dynamic_linker` vazios/None (macOS usa `-syslibroot` em vez de `-L`, e dyld é implícito). Teste `macos_crt_discovery_resolves_existing_sdk` (macOS-only, `#[cfg(target_os = "macos")]`) valida SDK path existe + version não vazia + arch válida; `bundled_rust_lld_strategy_falls_back_on_non_windows` atualizado para validar flavor `darwin` + extra_args contém `-arch`/`-platform_version`/`-syslibroot` em macOS. **Phase 1 agora completa para todos os 3 desktop OSes** (Windows MSVC, Linux GNU, macOS).
- **Infra/Stage (Rust removal Phase 1):** `tools/stage_native_runtime.ps1` e `tools/stage_native_runtime.sh` agora também copiam `rust-lld[.exe]` para `<stage_root>/bin/` (encontram via `ORI_RUST_LLD` env → `rustc --print sysroot` → PATH lookup). Switch `-SkipBundleLld`/`--skip-bundle-lld` adicionado para pular o bundling quando explícito. `Get-RustLldPath` helper (PS) e `find_rust_lld()` function (sh) adicionados com 3 níveis de fallback.
- **AGENTS.md (Rust removal Phase 1):** Variáveis de ambiente `ORI_USE_BUNDLED_RUST_LLD` e `ORI_RUST_LLD` documentadas na tabela de env vars.
- **Stdlib/Bootstrap (Stdlib Phase 0 — prelude loading):** Infraestrutura de prelude loading para `stdlib/*.orl` implementada em `ori-driver/src/pipeline.rs`. Novo status `StdlibImportStatus::StdlibSource(PathBuf)` permite que `import ori.string.utils` (e qualquer `ori.*` com arquivo `.orl` correspondente) carregue fonte da stdlib em vez de rejeitar como `bind.stdlib_module_unknown`. Descoberta de path em 2 estágios: `find_stdlib_source_module` mapeia `ori.X.Y` → `<stdlib_root>/X/Y.orl`; `find_stdlib_root` resolve em 3 níveis (`ORI_STDLIB_ROOT` env → `CARGO_MANIFEST_DIR/../../../stdlib` dev mode → `<ori.exe dir>/stdlib` release package). Cycle detection e `validate_import_namespace` reutilizados (arquivos stdlib seguem as mesmas regras de namespace que arquivos de usuário). **Primeiro módulo Layer 2 portado:** `stdlib/string/utils.orl` com `namespace ori.string.utils`, importando `ori.string as str` (Layer 1 FFI) e expondo funções `public`. O módulo demonstra o padrão de 3 camadas: Layer 2 em `.orl` chamando Layer 1 FFI via import normal. Palavras reservadas evitadas: `string`, `repeat`, `result` são keywords em Ori (não podem ser identificadores) — o módulo usa `str` como alias, `replicate` em vez de `repeat`, `acc` em vez de `result`. 2 testes de regressão em `multifile_imports.rs`: `compile_runs_stdlib_source_module_string_utils` (valida check→compile→run end-to-end, saída `true\nfalse\ntrue\nfalse\nababab\n`), `check_stdlib_source_module_unknown_still_reports_error` (valida que `ori.string.nonexistent` ainda rejeita com `bind.stdlib_module_unknown`).
- **Stdlib/Bootstrap (Stdlib Phase 0 — expansão Layer 2):** `stdlib/string/utils.orl` expandido de 3 para 7 funções `public` Layer 2, todas compostas sobre primitivas Layer 1 (`str.len`, `str.concat`, `str.trim`, `str.to_lower`, `str.pad_left`, `str.pad_right`, `str.slice`): `default(s, fallback) -> string` (retorna fallback se `is_empty(s)` — Layer 2 chamando Layer 2), `equals_ignore_case(a, b) -> bool` (`str.to_lower(a) == str.to_lower(b)` — paridade de igualdade case-insensitive), `center(s, width) -> string` (compõe `pad_left` + `pad_right` com divisão de padding `left = total/2`, `right = total - left` — demonstra composição de múltiplas primitivas Layer 1), `count(s, sub) -> int` (loop `loop`+`break` com janela deslizante via `str.slice` — conta ocorrências não-sobrepostas; retorna 0 para `sub` vazio). Naming collision evitada: variável local nomeada `len` colide com símbolo interno `ori_len` do runtime nativo (declarado em `native_backend.rs` para `ori_len(ptr: *u8) -> i64`) — renomeado para `s_len`. 1 teste de regressão adicional em `multifile_imports.rs`: `compile_runs_stdlib_source_module_string_utils_layer2` (valida 10 asserções cobrindo `default`/`equals_ignore_case`/`center`/`count` com casos normais, edge cases `center` com `len >= width`, `count` com sub vazio, `count` não-sobreposto `"aaa"`/`"aa"` = 1). Saída esperada: `fb\nx\ntrue\nfalse\n  hi  \nhello\n3\n1\n0\n0\n`. Total de testes multifile_imports: 263 (era 262). Workspace completo: 589 testes, 0 falhas.
- **Codegen/Link (Rust removal Phase 2 — SystemLinker):** Nova estratégia `SystemLinker` no `NativeLinker` que invoca o linker nativo do sistema diretamente (`link.exe` no Windows MSVC, `ld` no Linux GNU, `ld` via `xcrun` no macOS), sem `rust-lld` nem `rustc`. Opt-in via `ORI_USE_SYSTEM_LINKER=1`. Override do caminho do linker via `ORI_SYSTEM_LINKER`. Reutiliza as mesmas funções de CRT discovery da Phase 1 (`discover_msvc_crt_lib_dirs`, `discover_linux_gnu_crt`, `discover_macos_crt`). Discovery do linker: Windows — `ORI_SYSTEM_LINKER` → `<VS>\VC\Tools\MSVC\<ver>\bin\Hostx64\<arch>\link.exe` (fallback `Hostx86`); Linux — `ORI_SYSTEM_LINKER` → `cc -print-prog-name=ld`; macOS — `ORI_SYSTEM_LINKER` → `xcrun --find ld`. Link line Windows: `/OUT:` `/LIBPATH:` `/NOLOGO` `/SUBSYSTEM:CONSOLE` + obj + runtime libs. Link line Linux: `-o` `-dynamic-linker` `-no-pie` `-L` CRT objects + obj + libs + `-lc` + `crtn.o`. Link line macOS: `-o` `-arch` `-platform_version` `-syslibroot` + obj + libs. Prioridade em `NativeLinker::discover()`: `ORI_NATIVE_LINKER` (raw escape hatch) → `ORI_USE_BUNDLED_RUST_LLD` → `ORI_USE_SYSTEM_LINKER` → `RustcDriver` (default). HARD FAIL se opt-in e discovery falha (mesmo padrão de `BundledRustLld`). 4 testes de regressão em `native_backend/tests.rs`: `system_linker_strategy_engages_on_supported_os_or_reports_actionable_error` (cross-platform), `windows_link_exe_discovery_resolves_existing_path` (Windows-only), `linux_system_linker_discovery_resolves_existing_paths` (Linux-only), `macos_system_linker_discovery_resolves_existing_ld` (macOS-only). **Phase 2 completa para todos os 3 desktop OSes** (Windows MSVC, Linux GNU, macOS). Workspace: 591 testes, 0 falhas.
- **Stdlib/Bootstrap (Stdlib Phase 0 — expansão Layer 2, segunda leva):** `stdlib/string/utils.orl` expandido de 7 para 11 funções `public` Layer 2: `reverse`, `capitalize`, `title`, `swap_case` (+ helpers anteriores). Novos módulos Layer 2: `stdlib/list/utils.orl` (`get_or`/`first_or`/`last_or`), `stdlib/convert/utils.orl` (`parse_int_or`/`parse_float_or`). 3 testes de regressão stdlib + 1 teste for-in list string.
- **Stdlib/Bootstrap (Stdlib Phase 0 — Layer 2 completa + Layer 3 inicial):** Expansão final dos wrappers Layer 2 e primeiros algoritmos Layer 3 em `.orl`. **Layer 2 (novos módulos):** `stdlib/map/utils.orl` (`get_or`, `get_or_string`, `contains_key`), `stdlib/set/utils.orl` (`contains_all`, `from_list`, `is_subset`, `contains_all_int`), `stdlib/bytes/utils.orl` (`is_empty`, `equals`, `from_hex_or`, `empty_bytes`), `stdlib/math/utils.orl` (`sign`, `approx_eq`, `clamp_int`, `lerp`). **Layer 2 (expansões):** `stdlib/string/utils.orl` (+`lines`, `left`, `right`, `words`, `trim_all`; `reverse`/`title`/`swap_case`/`words` usam iteração indexada para evitar corrupção ARC em `for-in list<string>`), `stdlib/list/utils.orl` (+`singleton`), `stdlib/convert/utils.orl` (+`parse_bool_or`). **Layer 3 (algoritmos puros):** `stdlib/list/algorithms.orl` (`sum_int`, `binary_search_int`, `all_equal_int`), `stdlib/tree/algorithms.orl` (`is_leaf`, `values_preorder`, `leaf_count`, `max_depth_from` — travessias iterativas com stack, sem recursão genérica), `stdlib/graph/algorithms.orl` (`has_path`, `reachable_count`, `is_reachable`, `has_path_int` — BFS em `.orl` sobre primitivas Layer 1). Limitação documentada: map/set/graph Layer 2/3 usam tipos concretos (`string`/`int`) enquanto genéricos de chave (`K`/`N`) aguardam trait gate `Hashable`+`Equatable`. 10 testes de regressão adicionais em `multifile_imports.rs`. **Layer 1 permanece manifesto Rust** — hot path (ARC, async, I/O, FFI) não portado por design.
- **Docs/CLI (backlog v2 — `ori doc` HTML):** `ori doc --format html` gera página HTML estática (`pipeline/doc_html.rs`); `--out` grava em arquivo. Teste `doc_renders_static_html_output`.
- **Docs website + `ori doc export` (`[Unreleased]`):** Site Starlight em [github.com/raillen/ori-website](https://github.com/raillen/ori-website) — i18n en/pt/es/ja, Pagefind + busca ⌘K de símbolos, referência stdlib/erros gerada de `ori doc export`. CLI refatorada: `ori doc file <path>` (extrai docs de arquivo), `ori doc export [--out path]` (JSON Layer 1+2 + catálogo de erros + keywords). Módulo `doc_export.rs`.
- **Registry (backlog v2 — planning + stubs):** `docs/planning/registry-v2.md`; stubs `ori install` / `ori publish`.
- **Docs/Spec (backlog v2 — paridade C async):** Seção "C/debug async parity (v2 backlog — deferred)" em `docs/spec/14-backend-support.md` — C backend permanece sync-only; async nativo é referência.
- **Codegen/Native (for-in managed elements):** Corrigido bug de corrupção em `for item in list<string>` — retain/release correto no binding do loop (`emit_for_element_binding`). Teste `compile_runs_for_in_over_list_string_without_corruption`.
- **Release/Smoke (JIT no package empacotado):** `tools/smoke_native_release.ps1` e `.sh` agora verificam que o cdylib do runtime foi staged em `runtime/<triple>/` e executam `ori run examples/hello_world.orl` no package isolado com `ORI_REQUIRE_PACKAGED_RUNTIME=1` (JIT default quando cdylib empacotada existe).
- **Driver/Run (JIT default):** `ori run` usa o path JIT por default quando um cdylib do runtime está disponível (layout empacotado ou artefato cargo-built). Opt-in explícito permanece `ORI_USE_JIT=1`; opt-out via `ORI_USE_AOT=1`. `pipeline::should_use_jit_for_run()` centraliza a decisão. 1 teste adicional em `jit_run.rs`: `jit_run_uses_jit_by_default_when_cdylib_available`.
- **Codegen/Run (Rust removal Phase 3 — JIT Cranelift):** Modo JIT adicionado a `ori run` que executa código Cranelift diretamente em memória, sem escrever `.o`, sem linker, sem subprocesso. Opt-in via `ORI_USE_JIT=1`. `NativeBackend` refatorado para genérico sobre `M: Module` (`NativeBackend<M>`), com método `prepare(hir)` extraído (lower HIR + declare/define) e `compile(hir)` especializado para `ObjectModule` (AOT, chama `prepare` + `module.finish().emit()`). Novo método `into_module()` consome o backend e retorna o módulo; `main_func_id()` expõe o `FuncId` do wrapper C `main` (setado em `define_all`). Novo módulo `compiler/crates/ori-codegen/src/native_backend/jit.rs` com `run_jit(hir, cdylib_path) -> Result<i32, String>`: carrega o runtime cdylib via `libloading::Library`, registra um `symbol_lookup_fn` no `JITBuilder` que resolve qualquer símbolo `ori_*` (e `strlen`/`strcmp`) on-demand da cdylib, constrói `JITModule`, chama `NativeBackend::new(module)?.prepare(hir)?`, `finalize_definitions()`, `get_finalized_function(main_id)`, e invoca o wrapper in-process com `(0, null)`. Runtime `ori-runtime` agora builda 3 artefatos: `staticlib` (`ori_runtime.lib`/`libori_runtime.a`), `rlib` (`libori_runtime.rlib`), `cdylib` (`ori_runtime.dll`/`libori_runtime.so`/`libori_runtime.dylib`) — adicionado `crate-type = ["staticlib", "rlib", "cdylib"]` em `ori-runtime/Cargo.toml`. Stage scripts (`tools/stage_native_runtime.ps1`, `.sh`) copiam cdylib para `runtime/<triple>/` e registram campo `runtime_cdylib` em `runtime-link.json`. Driver: `find_native_runtime_cdylib()` resolve path do cdylib (override `ORI_RUNTIME_CDYLIB` → packaged → cargo fallback), `pipeline::run_jit()` executa lex→parse→resolve→check→lower→JIT, branch JIT em `Commands::Run` no `main.rs` despacha para `pipeline::run_jit` antes do path AOT. `ori compile` e `ori test` permanecem AOT (distribuição + isolamento de processo para `ori_test_assert` que chama `std::process::abort()`). `ori-types::stdlib::stdlib_runtime_symbols()` adicionado como iterador público sobre `runtime_symbol` onde `native_runtime == true` (usado pelo path JIT para validação e disponível para callers externos). 1 teste unitário em `native_backend/jit.rs`: `run_jit_reports_missing_cdylib_with_descriptive_error`. 2 testes de integração em `ori-driver/tests/jit_run.rs`: `jit_run_hello_world`, `jit_run_computes_arithmetic` — spawn `ori run` como subprocesso com `ORI_USE_JIT=1` (evita races de env var no test runner paralelo). Teste existente `native_compile_and_test_pipeline_do_not_use_legacy_c_runtime_hooks` ajustado para não flaggear `ORI_RUNTIME_CDYLIB` (match em `ORI_RUNTIME_C"` em vez de substring `ORI_RUNTIME_C`). **Phase 3 completa o híbrido A→B→D** — `ori run` agora pode executar sem `rustc`, sem linker, sem `.o` temporário. Workspace: 594 testes, 0 falhas.

### Alterado
- **Stdlib Layer 1 (breaking):** `ori.fs.*` queries/mutações e `ori.io.read_line` migrados de `bool`/`string` para `result`/`optional` — ver entrada em `### Adicionado`.

### Decidido (sem mudança de código)
- **Roadmap (Rust removal):** Decisão arquitetural fechada — remoção da dependência de Rust seguirá híbrido A→B→D: Phase 1 (completa, `[Unreleased]`) bundle `rust-lld` + CRT discovery próprio para Windows MSVC, Linux GNU e macOS; Phase 2 (completa, `[Unreleased]`) system linker via `ORI_USE_SYSTEM_LINKER=1` (`link.exe`/`ld`/`ld64` direto com CRT discovery, sem `rust-lld` nem `rustc`); Phase 3 (completa, `[Unreleased]`) JIT Cranelift para `ori run` via `ORI_USE_JIT=1` (elimina link step — código executado in-process via `JITModule` + `libloading` sobre cdylib do runtime; `ori compile` e `ori test` permanecem AOT para distribuição e isolamento de processo). `ORI_NATIVE_LINKER` permanece como escape hatch raw sem CRT discovery (diagnóstico), distinto de `ORI_USE_SYSTEM_LINKER`. `ORI_RUNTIME_CDYLIB` adicionado como override explícito do path do cdylib para o path JIT.
- **Roadmap (Stdlib):** Stdlib seguirá modelo de 3 camadas explícitas: Layer 1 (Rust runtime, nunca portar — `ori.mem`, `ori.task`, `ori.channel`, `ori.atomic`, `ori.fs`), Layer 2 (safe wrappers em `.orl`, port gradual — iniciado com `ori.string.utils` em Stdlib Phase 0), Layer 3 (algoritmos em `.orl`, port futuro — `ori.tree`, `ori.graph`, `ori.heap`). Boundary Layer 1/2/3 confirmado na prática em Stdlib Phase 0.
- **Stdlib Phase 0 (prelude loading + Layer 2 + Layer 3):** Infraestrutura de prelude loading para `stdlib/*.orl` entregue (ver `### Adicionado`). Boundary Layer 1/2/3 confirmado na prática: Layer 1 congelado (manifesto Rust), Layer 2 com 7 módulos utils, Layer 3 com 3 módulos algorithms. Próximos marcos (futuro): mais módulos Layer 2 cold-path (`ori.format.utils`, `ori.iter.utils`), trait gate para genéricos em map/set/graph, self-hosting.
- **Versionamento (2026-06-29, histórico):** Congelado em `0.2.x` na época. Critérios de 1.0 e ordem tática atuais: ver `AGENTS.md` e `docs/planning/PENDENTES.md` (**M2 stdlib → M3 ABI → M1 Rust-indep → M4 self-host**).

---


## [0.3.1] — 2026-07-13 (Nim-local inference)

### Adicionado
- **Tipos / bindings locais:** omissão de anotação em `const`/`var` **locais** quando o RHS é óbvio na mesma linha (feeling Nim, não HM global). Exemplos: `const n = 1`, `const name = "Ada"`, `const u = User { … }`, `const xs = [1, 2, 3]`.
- **Diagnóstico:** `type.local_inference_failed` quando a omissão não é segura (`try`, `[]`/`{}` vazios, `none` sem contexto, tipos não concretos).
- **Testes:** `type_accepts_local_nim_style_inference`, `type_rejects_local_inference_on_try`, `type_rejects_local_inference_on_empty_list`.
- **Docs:** caps. 04 e 06 atualizados; catálogo 13.

### Corrigido (pós-tag de superfície)
- **Codegen/ARC — `ori_list_push`:** path especial no backend nativo (`emit_list_push_value`) em vez do FFI genérico que liberava o temporário gerenciado após a chamada — corrigia corrupção de `list[string]` / stdlib utils.
- **Codegen/ABI — layout de enum:** `compute_enum_layout` usa alinhamento natural (`repr_c=true`) para `payload_offset` bater com o runtime (ex.: `ori.json.Value` em offset 8).
- **Driver:** warning dead_code em `classify_stdlib_import` (`_has_selected_items`).
- **LSP:** índice semântico de bindings locais (`const`/`var` omitidos) para inlay/hover de tipos óbvios (0.3.1).
- **VS Code:** `extensions/vscode-orl` version bump para `0.3.1`.

### Não incluído (no corte 0.3.1; ver Unreleased / opção B)
- Inferência global; omissão em `pub`/params/retornos de API.
- Opção B (campo/index/call/pipe + reject void) — documentada e entregue em
  **`[Unreleased]`** após 0.3.1; ver spec 04/05/06.
- **Pipe `|>`:** **permanece** na Ori (já existia; teste `compile_runs_pipe_operator_native`). A menção “fora do 0.3” na ata S3 foi **corrigida** — não era decisão de produto.

---

## [0.3.0] — 2026-07-12 (surface cutover S3)

**Breaking release of language surface.** Ori absorbs the Auk9-inspired **S3**
syntax. Pre-S3 forms are **hard errors** (no dual acceptance). Product purpose
and identity: [`docs/spec/00-manifesto.md`](docs/spec/00-manifesto.md). Decision
log: [`docs/planning/ori-surface-s3-auk9.md`](docs/planning/ori-surface-s3-auk9.md).
ADR: [`docs/planning/adr-ori-surface-s3-auk9.md`](docs/planning/adr-ori-surface-s3-auk9.md).

**Versioning note:** language surface **`0.3.0`**; workspace Cargo **`0.3.1`**
(after inference slice). **Package** zip/tar remains deferred until remaining
pendencies close.

**Not in 0.3.0:** Nim-style local inference (**`0.3.1` / PR 11**); migration of
`packages/ori-game` and `packages/ori-imgui` (**última** fatia). Pipe `|>` **já
era** feature Ori e **permanece** (não foi cortado no S3).

### Breaking — surface S3

| Area | Canonical (S3) | Removed (error) |
|------|----------------|-----------------|
| File header | `module path` | `namespace` → `parse.namespace_removed` |
| Function decl | `name(params) -> T` / `=> expr`; `async name(...)` | declaration keyword `func` → `parse.func_removed` (callable type `func(T)->R` kept) |
| Compound types | `list[T]`, `map[K,V]`, `optional[T]`, `result[T,E]`, `Name[T]` | `Type<…>` → `parse.removed_angle_type`; `list of T` / `map of K to V` → `parse.removed_of_type` |
| Generic bounds | `for T: Trait` / `for T: not Trait` | `where T is` → `parse.removed_where_bound` |
| Propagation | `try expr` only | postfix `expr?` → `parse.question_propagate_removed` |
| Conditionals | `elif` | `else if` → `parse.else_if_removed` |
| Match cases | `case Variant` / `case Variant(...)` | leading `.` → `parse.case_dot_variant_removed` |
| Struct literals | `Type { f: v }`, `{ f: v }` | `Type(...)`, `.{…}`, guided `(…)` → `parse.removed_struct_call_literal` |
| Map literals | `{ "k": v }` (literal key) | (disambiguation: ident before `:` = struct) |
| Imports | `import path (A, B)`; `import path = alias`; `import path` | `as` → `parse.import_as_removed`; `only` → `parse.import_only_removed`; no Auk9 order `import alias = path` |
| Imports block | `imports … end` with multi-comma **only** in block | — |
| Traits | `apply Type` + `use Trait`; bind `slot = freeFn` | `implement Trait for Type` → `parse.implement_removed`; `apply Trait to/for Type` → `parse.apply_trait_to_removed` |
| Closures | `(params) => expr` / `(params) … end` | `do(...)` → `parse.do_removed` |
| Rhythm | poetic one-arg call; optional labeled `end if` / `end match` | nested poetic → `parse.poetic_call_nested`; label mismatch → `parse.end_label_mismatch` |

### Added

- **Manifesto** `docs/spec/00-manifesto.md` — purpose: study, AI-assisted programming, ND readability; **not** market competition.
- **CLI** `ori migrate-syntax` (+ `tools/migrate_syntax.sh`) — best-effort rewrite pre-S3 → S3 (skips `ori-game` / `ori-imgui`).
- **Diagnostics** emitted for all removed forms and rhythm errors listed above (catalog chapter 13).
- **Docs reforma** — overview, lexical, EBNF, functions, traits, catalog, guides and READMEs aligned to S3.

### Changed

- **Stdlib / examples / tests** in-repo migrated to S3 (`.orl` sources).
- **Formatter / VS Code grammar / snippets / templates** keyword surface aligned.
- **Auk9** — retired as a parallel **product**; remains a syntax **lab** reference only. Living surface is Ori S3.

### Migration

```bash
# best-effort (re-runnable)
ori migrate-syntax stdlib examples tests
# or
sh tools/migrate_syntax.sh
```

Manual review still required for complex `apply` rewrites and packages outside
this repository. See also `docs/spec/01-overview.md` (Surface S3 summary table).

### Deferred to 0.3.1

- Local Nim-style type omission on obvious same-line bindings (design: surface
  doc bloco 8b; PR 11 of `pr-plan-ori-surface-s3.md`).
- Public APIs, parameters, and return types remain annotated.

---
## [0.2.0] — 2026-06-29

Etapa 9 (Release e Publicação) do `docs/planning/PLANO-MATURIDADE-COMPLETO.md`. Esta release consolida as Etapas 0–8 (estabilização do workspace, features bloqueadoras, sistema de tipos avançado, sync documental normativa, dívida técnica do compilador, runtime/ARC, LSP semântico cross-file, catálogo de diagnósticos auditado, organização/infra/qualidade) e formaliza o versionamento semver do projeto.

### Adicionado
- **Release (Etapa 9):** Versionamento semver formal — workspace version bumpado de `0.1.0` para `0.2.0` em `Cargo.toml [workspace.package]` (propaga para os 10 crates via `version.workspace = true`). Runtime estática re-stageada com `ori_version: 0.2.0` em `runtime-link.json`. Seção `[Unreleased]` do CHANGELOG esvaziada para o próximo ciclo de desenvolvimento.
- **Docs/Release (Etapa 9.4):** `IMPLEMENTADOS.md` seção 13 "Release v0.2.0 — Snapshot (2026-06-29)" adicionada com componentes versionados, tamanhos de binários (ori.exe ~9.65 MB, ori-lsp.exe ~11.83 MB, ori_runtime.lib ~12.76 MB release), validação de release (smoke + tests + catalog + LSP E2E), CI, known issues, backlog v2. `README.md` seção "Status" reescrita de "Early development" para "v0.2.0 — feature-complete for v1 targets" com detalhes (Cranelift, LSP cross-file, ~580 testes, 5 CI triples, pre-1.0 caveat). `AGENTS.md` "Current Status (2026-06-29)" atualizada com version `0.2.0` + release smoke passing. `PENDENTES.md` Etapa 6 reconciliada com Etapa 9 (4 de 5 itens `[x]`; `git push` pendente de aprovação explícita).

### Alterado
- **Stdlib/Arquitetura:** Consolidação do manifesto `STDLIB_RUNTIME_FUNCTIONS` como fonte única de verdade para classificação de imports stdlib. `ori-types::stdlib` agora expõe `is_implemented_stdlib_module()` e `implemented_stdlib_modules()`, derivados do manifesto + `STDLIB_MODULE_ONLY_PATHS` (allowlist documentada para módulos sem entries de runtime: `ori`, `ori.core`, `ori.Error`, `ori.mem` (intrínsecos inline), `ori.concurrent` (umbrella)). `pipeline.rs::classify_stdlib_import` reescrito para delegar ao manifesto (lista hardcoded de 35 módulos removida). `lower.rs::stdlib_c_name` reduzido a wrapper fino sobre `stdlib_runtime_symbol` (155 linhas de match duplicado removidas — todo path já estava no manifesto). `append_stdlib_documentation` em `pipeline.rs` agora usa `implemented_stdlib_modules()` em vez de derivar módulos inline (output de doc agora inclui `ori.files`, `ori.core`, `ori.mem`, `ori.concurrent`, `ori.Error` consistentemente com a classificação de imports). Testes de paridade em `ori-types::tests`: `manifest_module_prefixes_are_all_implemented`, `implemented_stdlib_modules_covers_legacy_hardcoded_list` (regressão contra lista antiga), `unknown_stdlib_modules_are_rejected`. Teste de paridade em `pipeline::tests`: `collection_stdlib_doc_signatures_reference_implemented_modules` guarda contra drift em `COLLECTION_STDLIB_DOC_SIGNATURES`. Guarda contra drift futuro spec/manifesto/lower/doc.
- **Docs/Spec:** Cap. 12 (stdlib) — seção "Implementation Architecture (v1.x)" adicionada documentando o manifesto como fonte única de verdade, runtime `extern "C"`, parity guards, e workflow para adicionar funções stdlib.
- **Diagnósticos/Catálogo (Etapa 7):** Auditoria de nomenclatura do catálogo concluída. Os 4 códigos `project.*` (`circular_import`, `entry_not_found`, `namespace_file_mismatch`, `no_proj_file`) já emitidos (Etapa 6.5). Os 9 códigos planejados restantes foram **removidos do catálogo v1 com justificativa** (seção "Removed From v1 Catalog" em cap. 13): `contract.check_failure`/`field_violation`/`param_violation` (runtime-only, deferido v2), `doc.unclosed_block` (redundante com `lex.unclosed_block_comment`), `generic.ambiguous_type_arg` (deferido v2, coberto por `type.type_mismatch`), `match.guard_not_exhaustive` (deferido v2, `match.non_exhaustive` cobre unguarded), `type.ambiguous_generic` (alias), `type.annotation_required` (não aplicável — Ori explicitamente tipado), `using.non_result_init` (coberto por `using.not_disposable`). Os 9 reserved aliases (`bind.undefined`, `type.mismatch`, `type.callable_mismatch`, `type.constraint_not_satisfied`, `type.incompatible_result_error`, `type.index_non_indexable`, `type.invalid_is_check`, `type.propagation_context`, `type.undefined`) permanecem documentados como aliases não emitidos. Teste `diagnostic_catalog_matches_emitted_codes` fortalecido com guarda contra reintrodução dos códigos removidos na auditoria.
- **Arquitetura/Monolitos (Etapa 8.3):** Refatoração incremental de monolitos com uma extração por arquivo: (1) `pipeline.rs` → `pipeline/fmt.rs` — `format_source_text` + 3 helpers (~70 linhas) extraídos como submódulo; API pública `ori_driver::pipeline::format_source_text` preservada via wrapper. (2) `native_backend.rs` → `native_backend/string_collector.rs` — `StringCollector` + 6 funções de travessia HIR (~255 linhas) extraídas; `pub(super) fn collect_all_strings` re-exportado via `use`. (3) `ori-runtime/lib.rs` → `test_harness.rs` — 13 funções `ori_test_*` (~125 linhas) extraídas; delegam para `super::cstr_str`/`super::ori_arc_*`. Testes `native_string_collectors_are_exhaustive_over_hir_shapes` e `native_codegen_unsupported_errors_are_coded` atualizados para ler de `string_collector.rs`; `rust_runtime_exports_manifest_native_symbols` atualizado para incluir `test_harness.rs`.
- **Workspace/Infra (Etapa 8.4):** `libc` e `serde_json` centralizados em `[workspace.dependencies]` — `ori-runtime` e `ori-lsp` agora usam `{ workspace = true }` para ambos. `rust-toolchain.toml` criado fixando `channel = "1.95.0"` + componentes `rustfmt`/`clippy`. Menção a `vendor/` em `AGENTS.md` esclarecida como slot reservado futuro (diretório não existe).
- **Docs/Stdlib (Etapa 8.1):** Cap. 15 (`15-stdlib-maintenance.md`) reescrito com arquitetura SSOT (Single Source of Truth), `STDLIB_MODULE_ONLY_PATHS`, funções derivadas (`is_implemented_stdlib_module`, `implemented_stdlib_modules`, `stdlib_runtime_symbol`), testes de paridade completos e seção `.orl` futura. Cap. 12 mantém a visão de contrato público com a seção "Implementation Architecture (v1.x)".
- **Docs/Runtime (Etapa 8.2):** `runtime/README.md` atualizado com tabela de staging para os 5 triples do CI (windows-msvc, windows-gnu, linux-gnu, macos-x86_64, macos-aarch64) + comando de staging para cada. `CONTRIBUTING.md` reescrito (era stale "Zenith"): política de triples versionados vs gerados em CI, layout do release package, gates de qualidade, smoke com `ORI_REQUIRE_PACKAGED_RUNTIME=1`, checklist de PR para mudanças stdlib/diagnósticos.
- **Docs/Tests (Etapa 8.5):** `tests/README.md` reescrito com tabela de 7 suites de teste (ori_spec, multifile_imports, concurrency_async, memory_arc, method_resolution, diagnostic_catalog, LSP E2E) + caminhos + cobertura + instruções para adicionar novos testes. `tests/run/bytes_stdlib.orl` deletado (sintaxe obsoleta + redundante com `multifile_imports.rs`); diretório `tests/run/` vazio removido.
- **Docs/Dedup (Etapa 8.6):** `docs/plano-correcao-implementacao-linguagem.md` deletado (duplicata stale sem banner; `docs/archive/` já contém a versão completa de 44882 chars). `PENDENTES.md` Etapa 5 (Diagnósticos) atualizada para refletir a auditoria da Etapa 7: todos os 14 códigos marcados `[x]` (4 emitidos na Etapa 6.5 + 1 reserved alias + 9 removidos com justificativa); critério de passagem atualizado.

### Corrigido
- **Codegen/Cranelift:** Corrigido `collect_all_tys` para `Ty::Func { ret }` e cobertura de `HirStmt::Break`/`Continue` em `collect_tys_from_stmt`, desbloqueando compilação após extensão da state machine async.
- **Codegen/Cranelift:** `emit_async_terminal_cleanup` garante `dispose()` em caminhos terminais async (cancel, fail, propagate) via `emit_async_frame_dispose_live_values`.
- **Checker:** `stdlib_native_codegen_available` — `lazy.once`/`lazy.force` não emitem mais warning falso de runtime indisponível (codegen inline nativo).
- **Codegen:** Warnings residuais em `native_backend.rs` eliminados (`cargo check -p ori-codegen` limpo).
- **Codegen/Cranelift:** Saída de escopo síncrona sem `return` explícito agora emite `emit_scope_cleanup_calls_from(0, 0)` antes do return implícito — antes valores managed em bindings locais vazavam ao cair do fim da função.
- **Codegen/Cranelift:** Chamadas a funções stdlib runtime (FFI) não reteêm mais argumentos managed no call site — o runtime empresta os argumentos sem tomar ownership, então o retain extra era não-balanceado e vazava.
- **Codegen/Cranelift:** Corrigido over-retain de valores managed no codegen nativo. Introduzido `expr_produces_owned_ref` para classificar expressões "fresh" (+1 refcount) vs. "borrowed". Retains seletivos agora aplicam-se apenas a valores borrowed em `emit_return`, `HirStmt::Let`, `HirStmt::Assign` e `HirStmt::Using`. Temporários fresh consumidos em `HirStmt::Expr`, `HirExprKind::Binary` (concat string/bytes), `HirExprKind::Some_`/`Ok_`/`Err_` (payloads) e `HirExprKind::StructLit`/`EnumVariant` (campos) agora são explicitamente release após transferência de ownership para a edge ARC. Introduzido `user_func_names` para distinguir funções de usuário de stdlib FFI no tratamento de argumentos de chamada. 7 testes E2E em `memory_arc.rs` un-ignored e reestruturados para zero-leak.
- **Docs:** Sincronização parcial da spec normativa (cap. 04, 07, 08, 10, 11, 12, 14, 13) com implementação das Etapas 1–2.
- **Docs:** Etapa 3 — spec cap. 08 (traits): seção "Current implementation status" consolidada com tabela feature→teste de sanidade; cap. 11 (generics): seção "Limitations in v1" reescrita com sintaxe concreta para associated types (`type Item`), const generics (`struct Matrix<const N: int>`), HKT (`trait Functor<F<_>>`) + subseção "Sanity tests" referenciando os 7 testes `generic_accepts_*`; cap. 13 (error catalog): nota de convenção `name.*` (resolução de nomes) vs `bind.*` (binding/import/field/param) adicionada.
- **Docs:** `AGENTS.md` — nota de prefixos de diagnóstico corrigida: agora documenta a convenção real (`name.*` para undefined/private/duplicate top-level; `bind.*` para duplicate_field/param/variant/alias/import) em vez da orientação stale "use `bind.duplicate_*` não `name.duplicate_*`".
- **Docs:** `PENDENTES.md` — Etapa 3 (Runtime/ARC) e Etapa 4 (LSP) reconciliadas com CHANGELOG `[Unreleased]` (Sprints 1–5): itens entregues marcados `[x]` com referência ao sprint; pendentes mantidos `[ ]` com nota (completion type-aware, testes E2E LSP, diagnósticos project-level).
- **Docs:** `CHANGELOG.md` seção `[0.1.0]` — lista "Não implementado (planejado)" substituída por nota histórica apontando para `[Unreleased]` (todos os 8 itens entregues: `ori.Error`, cycle collector, `fs.File`, `using` async, `CancelToken`, type alias em `where`, `lazy` nativo, `iter` nativo).
- **Docs/Planning:** Etapa 4 (dívida técnica do compilador) reconciliada: o item 4.3 registrava que `await` em loops aninhados (`for→while`) ainda falhava no general async path; em `[Unreleased]` esse caso foi corrigido e o teste `compile_runs_async_await_in_deeply_nested_bodies_native` deixou de ser ignorado. Item 4.4 (tabela C×stdlib) confirmado já entregue na Etapa 3 (seção matriz em cap. 14 + teste de sanidade `spec_c_backend_matrix_matches_manifest_flags`).
- **Docs/LSP:** Etapa 6.6 — README seção "Current Tooling Status" atualizada com capacidades LSP reais pós-Etapa 6 (signature help, code lens, code actions adicionados; E2E harness mencionado; formatter idempotente em async). `docs/plano-implementacao-lsp-avancado.md` tabela "Estado Atual vs Alvo" reescrita com status entregue (Sprints 1–5 + Etapa 6.1–6.6): 22 funcionalidades ✅, 2 ❌ (goto stdlib, diagnostics lint); pendências remanescentes 6.1/6.2/6.5 entregues posteriormente nesta mesma unreleased cycle (ver entrada "Etapa 6 concluída" acima). `PENDENTES.md` Etapa 4 item 2 (testes E2E LSP) marcado `[x]` com referência à Etapa 6.3; item formatter atualizado com referência à Etapa 6.4.
- **Docs/Planning:** Etapa 6 concluída (2026-06-28): 6.1 (ProjectSemanticIndex cross-file reusing `run_check` `ResolvedModule`+`SourceCache`), 6.2 (completion `AfterDot` type-aware + find references cross-file + rename cross-file), 6.5 (diagnósticos `project.*` — rename de `bind.import_cycle`/`bind.import_namespace_mismatch` para `project.circular_import`/`project.namespace_file_mismatch` + mapeamento LSP de `project.entry_not_found`/`project.no_proj_file` + roteamento cross-file de project diagnostics) entregues. Catálogo cap. 13 atualizado (seção `project` em Emitted). Critérios de passagem da Etapa 6: 4 de 4 `[x]`.
- **Known Issues:** itens antigos de Etapa 4/6 reconciliados em `[Unreleased]`: `await` em loops aninhados agora passa no backend nativo, e o formatter de `trait` preserva assinaturas obrigatórias e métodos default.

### Adicionado
- **Workspace:** `rust-toolchain.toml` — fixa a versão Rust do CI em `1.95.0` com componentes `rustfmt` e `clippy`; garante que desenvolvedores e CI usem a mesma versão.
- **Runtime:** Disparo cooperativo de `ori_arc_collect_cycles` no executor async. `maybe_collect_cycles_cooperative()` verifica `COOPERATIVE_ALLOC_COUNTER` a cada batch de tasks em `ori_task_block_on` e ao fim de `ori_executor_drain`; threshold default 256 alocações, override via `ORI_COOPERATIVE_COLLECT_THRESHOLD`. Teste unitário `cooperative_collect_fires_after_allocation_threshold` valida o gatilho e o no-op abaixo do threshold.
- **Runtime:** `ori_test_live_allocations()`, `ori_test_collect_cycles()`, `ori_test_assert_no_leaks(label)` — hooks para programas de teste verificarem vazamentos de memória ao fim da execução. `assert_no_leaks` aborta com diagnóstico em stderr quando `ORI_TEST_LEAK_CHECK=1` está setado e há alocações vivas.
- **Stdlib:** `ori.test.live_allocations`, `ori.test.collect_cycles`, `ori.test.assert_no_leaks` expostos no registro stdlib (native + C backend com stubs inline).
- **Docs:** Spec cap. 10 (memória) — seções sobre destrutores tipo-específicos, pontos de coleta cooperativa e modo leak-check.
- **Docs:** Spec cap. 16 (runtime FFI safety) — seções sobre cycle collector e leak-check FFI.
- **Docs:** `AGENTS.md` — `ORI_TEST_LEAK_CHECK=1` documentado em Environment Variables.
- **Docs:** `AGENTS.md` — `ORI_COOPERATIVE_COLLECT_THRESHOLD=N` documentado em Environment Variables.
- **Docs:** Spec cap. 14 (backend support) — seção "C/debug backend stdlib matrix (`c_backend` flag)" adicionada, documentando por módulo quais funções stdlib têm runtime C (flag `c_backend` no macro `stdlib!`) vs. native-only, com regras de evolução da flag.
- **Tests:** `compiler/crates/ori-driver/tests/memory_arc.rs` — suite E2E para Etapa 5: plumbing de leak-check, cycle collector runs, leak-check env abort/clean. Testes que exigem zero-leak marcados `#[ignore]` até auditoria da convenção ARC (ver known issues).
- **Tests:** `compile_runs_async_file_using_dispose_on_cancel`, `compile_runs_async_await_in_match_native` — regressão dispose async com `fs.File` e await em `match`.
- **Tests:** `compile_runs_async_await_in_for_loop_native` — completa a matriz async if/else/match/while/for com `await` no corpo do loop `for` (state machine levanta iterador através do await).
- **Tests:** `compile_runs_native_linked_list_and_graph_no_leak` — Etapa 5: estresse com `linked_list` + `graph` cíclico em loop, `assert_no_leaks` retorna 0. Valida destrutores de coleções opacas e release ARC cobrem grafos com ciclos internos.
- **Tests:** `build_c_backend_emits_json_parse_extern_without_c_lowering` — JSON no C backend via extern.
- **Tests:** `spec_fs_and_json_contracts_match_stdlib_sig` (ori-types/stdlib.rs) — Etapa 3: valida que os contratos de `ori.fs.File` (`open_read`/`open_write`/`read`/`write`/`close`) e `ori.json` (`parse`/`stringify`/`stringify_pretty`) documentados na spec cap. 12 batem com `stdlib_func_sig`.
- **Tests:** `spec_c_backend_matrix_matches_manifest_flags` (ori-types/stdlib.rs) — Etapa 3: valida as atribuições yes/no da matriz C×stdlib (spec cap. 14) contra os flags `c_backend_runtime` reais do manifesto `STDLIB_RUNTIME_FUNCTIONS`.
- **Tests:** `compile_runs_async_await_in_deeply_nested_bodies_native` (concurrency_async.rs) — regressão ativa para `await` em loops aninhados (`for→while`); o teste não é mais `#[ignore]` e valida a correção do general async path.
- **Tests:** `ori-lsp/tests/e2e.rs` — Etapa 6.3: harness E2E LSP (subprocess + JSON-RPC framing sobre stdio + reader thread com `mpsc` channel para timeouts). 5 testes, 12 cenários: `e2e_lsp_session_covers_8_scenarios` (initialize, didOpen, diagnostics, hover, definition, completion, formatting, rename, shutdown em sequência), `e2e_lsp_publishes_diagnostics_for_type_error`, `e2e_lsp_returns_document_symbols`, `e2e_lsp_formatting_is_idempotent` (formata 2x → ponto fixo), `e2e_lsp_formatting_emits_edits_for_unformatted`. Gate "mínimo 8 cenários" excedido.
- **Tests:** `fmt_preserves_async_spawn_nested_using_and_multiline_match_idempotent` (concurrency_async.rs) — Etapa 6.4: auditoria do formatter para `async func`/`await`/`task.spawn`/`using` aninhado/`match` multi-linha + verificação de idempotência (formatar 2x = mesmo). Valida indentação canônica (4 espaços por nível; `case` ao mesmo nível de `match` no estilo switch/case).
- **LSP:** Etapa 6.1 — `ProjectSemanticIndex` em `ori-lsp/src/index/project_semantic.rs` reusa o `ResolvedModule` (DefMap + sigs) e o `SourceCache` de `run_check_source` (capturado em `validate_uri`/`schedule_debounced_validate`, armazenado por-URI no `ProjectManager`). Habilita hover, go-to-definition e find-references cross-file (símbolos em imports transitivos).
- **LSP:** Etapa 6.2 — completion `AfterDot` type-aware (`complete_after_dot` infere o tipo declarado do receptor via varredura sintática de bindings/parâmetros com anotação de tipo e lista campos/variantes/métodos do struct/enum via `struct_sigs`/`enum_sigs`/`impl_sigs`); find references cross-file (`find_references_cross_file` varredura word-boundary sobre todos os arquivos no `SourceCache`); rename cross-file agrupa edits por URI.
- **LSP:** Etapa 6.5 — diagnósticos `project.*` publicados no LSP: `project.circular_import`, `project.namespace_file_mismatch` (emitidos pelo driver), `project.entry_not_found`, `project.no_proj_file` (mapeados no LSP via `project_error_diagnostic` a partir dos erros canônicos de `resolve_entry_path`). Roteamento cross-file via `project_diagnostics_for_path` (project diagnostics cujo label está em arquivo back-edge são publicados no arquivo aberto).
- **Driver:** Etapa 6.5 — rename `bind.import_cycle`→`project.circular_import` e `bind.import_namespace_mismatch`→`project.namespace_file_mismatch` para alinhar ao catálogo cap. 13 (seção `project` em Emitted; os 4 códigos `project.*` movidos de Planned para Emitted).
- **Tests:** Etapa 6.1/6.2/6.5 — `e2e_lsp_cross_file_goto_definition` (main.orl importa lib.orl; goto-def em `Point` resolve para `crossdef_lib.orl`), `e2e_lsp_type_aware_dot_completion` (`var p: Point` → `p.` lista campos `x`, `y`), `e2e_lsp_cross_file_find_references` (find-references em `Point` retorna ocorrência em `findref_main.orl`), `e2e_lsp_circular_import_diagnostic` (cyc_a.orl↔cyc_b.orl; abrir cyc_a publica `project.circular_import`). Teste unitário `project_error_diagnostic_maps_known_messages` valida o mapeamento LSP de `project.*`. Testes `ori_spec`/`multifile_imports` atualizados para os novos códigos `project.*`.
- **Planning:** `docs/planning/PLANO-MATURIDADE-COMPLETO.md` — plano mestre de maturidade com 10 etapas, checkboxes obrigatórios, testes de gate e critérios de passagem (Etapas 0–9 + backlog v2).
- **Codegen/Cranelift:** Interceptação robusta de chamadas sobrecarregadas de matemática (como `math.abs`, `math.min`, `math.max`) escritas como acessos a campos qualificados para selecionar a função FFI correspondente em float/int.
- **Codegen/Cranelift:** Interceptação robusta da função builtin `string(...)` para mapear corretamente para as funções FFI especializadas (`ori_to_string`, `ori_float_to_string`, `ori_bool_to_string`) com base no tipo do argumento em tempo de compilação.
- **C Backend:** Suporte a conversão correta de thunk no `emit_lazy_force` garantindo que o tipo de retorno FFI do closure coincida com o tipo de dado lazy.
- **Codegen/Checker:** Suporte completo a igualdade estrutural avançada para structs genéricas nos backends Cranelift nativo e C, realizando a substituição correta de parâmetros genéricos nos campos em tempo de compilação.
- **Checker:** Habilitação de comparação estrutural para mapas (`map<K,V>`) e conjuntos (`set<T>`) cujos elementos/chaves implementam o trait `core.Equatable` (seja por implementação explícita ou por suporte implícito a igualdade estrutural).
- **Stdlib:** Novo tipo opaco `task.CancelToken` e funções nativas `task.create_token`, `task.cancel`, `task.is_cancelled` e `task.associate` para cancelamento cooperativo de tarefas assíncronas.
- **Runtime:** Suporte nativo para cancelamento cooperativo de futures assíncronas e cleanups automáticos associados ao ciclo de vida em `ori-runtime`.
- **Parser:** Token `...` (Ellipsis) para parâmetros variádicos
- **Parser:** Validação de `parse.variadic_not_last` e `parse.default_before_required`
- **Parser:** Validação de `parse.import_after_declaration` para imports após declarações
- **Parser:** Validação de `parse.namespace_missing` e `parse.namespace_not_first` para posição obrigatória do namespace
- **Binder:** Validação de `bind.duplicate_param` para parâmetros repetidos em funções, métodos e assinaturas
- **Checker:** `check_loop_control()` — diagnostica `break`/`continue` fora de loop (`control.loop_required`)
- **Checker:** `expect_bool()` para operadores `and`/`or`/`not` (`type.expected_bool`)
- **Checker:** `warn_unused_result()` — warning para `result` descartado (`type.unused_result`)
- **Checker:** `check_closure_var_capture()` — rejeita captura de `var` em closure (`mut.closure_captures_var`)
- **Checker:** `infer_never_form_call()` — suporte a `panic`, `todo`, `unreachable` com tipo `never`
- **Checker:** `infer_wrapper_form_call()` — suporte a `.or()` / `.or_return()` / `.or_wrap()`
- **Checker:** `.or_return()` completo — desugaring para operador `?` (propagate) em `optional<T>` e `result<T,E>`
- **Checker:** `.or()` type-checking para `optional<T>` e `result<T,E>` com fallback
- **Parser/Codegen:** `.or(fallback)` completo para `optional<T>` e `result<T,E>` no backend nativo e no C backend, com fallback avaliado apenas em `none`/`error(_)`
- **Parser/Checker/Codegen:** `.or_wrap(context)` completo para `result<T, string>` no backend nativo e no C backend, com contexto avaliado apenas em `error(_)`
- **Checker:** `supports_builtin_equality` expandido para `optional<T>`, `result<T,E>`, `tuple<...>`, `bytes`, `list<T>` e structs sem genéricos
- **Checker:** `using` permitido dentro de `async func` (state machine armazena recurso no frame; dispose pendente nos terminais)
- **Stdlib:** `ori.Error` agora possui campo `cause: string` para encadeamento básico de erros
- **Codegen:** Igualdade estrutural nativa para `optional<T>`, `result<T,E>`, `tuple<...>`, `bytes`, `list<T>` e structs sem genéricos
- **C Backend:** Igualdade estrutural para `optional<T>`, `result<T,E>`, `tuple<...>`, `list<T>`, structs sem genéricos, `set<int|string>` e `map<int|string, V>` no backend de debug
- **Codegen:** State machine async aceita `Using` statements como prefix locals
- **Core Traits:** `ori.core.Displayable` agora possui método `display(self) -> string`
- **Checker/Lowering:** `string(value)` e f-strings agora usam `ori.core.Displayable` para tipos concretos definidos pelo usuário
- **Checker:** Type aliases agora são resolvidos em `where` constraints (ex: `where T is MyAlias` onde `type MyAlias = ori.core.Equatable`)
- **Checker:** `emit_undefined_name()` — nomes desconhecidos geram `name.undefined` + `Ty::Error`
- **Checker:** Validação de runtime para map/set com `type.collection_hash_unsupported`
- **Checker:** `stdlib_native_runtime_available()` — warning para funções stdlib sem runtime nativo (`bind.stdlib_module_unavailable`)
- **Resolver:** Validação de campos duplicados em struct (`bind.duplicate_field`)
- **Resolver:** Validação de variantes duplicadas em enum (`bind.duplicate_variant`)
- **Resolver:** Validação de campos duplicados em variantes de enum (`bind.duplicate_field`)
- **Lexer:** Aceita BOM UTF-8 no início do arquivo e rejeita no meio
- **Lexer:** `find_unclosed_block_comment()` respeita strings, bytes, f-strings e triple-quoted
- **Lexer:** Diagnóstico dedicado `lex.unclosed_block_comment` com span e ação
- **Literal parser:** `parse_int_literal()` e `parse_float_literal()` com validação de sufixos, overflow e range
- **Parser:** `expr_to_lvalue_or_error()` emite `parse.invalid_lvalue` em vez de descartar silenciosamente
- **C Backend:** Propagação correta de `?` com cleanup de escopo para `result` e `optional`
- **C Backend:** `ori_abort_bounds` para acesso fora de limites em listas
- **Stdlib:** `ori.panic` como built-in com tipo `never`
- **Stdlib:** Novos módulos: `ori.deque`, `ori.queue`, `ori.stack`, `ori.linked_list`, `ori.doubly_linked_list`, `ori.tree`, `ori.hash_table`, `ori.graph`, `ori.heap`
- **Stdlib:** Novas funções em `ori.list`: `try_get`, `is_empty`, `clear`, `clone`, `to_list`, `from_list`, `try_pop`, `try_remove`
- **Stdlib:** Novas funções em `ori.map`: `try_get`, `is_empty`, `capacity`, `reserve`, `clear`, `clone`, `from_entries`, `try_remove`
- **Stdlib:** Novas funções em `ori.set`: `is_empty`, `capacity`, `reserve`, `clear`, `clone`, `to_list`, `from_list`, `try_remove`
- **Stdlib:** `ori.string.parse_int`, `ori.string.parse_float` com tipo `result<T, string>`
- **Stdlib:** `ori.string.index_of`, `ori.string.join`, `ori.string.repeat`, `ori.string.pad_left`, `ori.string.pad_right`
- **Stdlib:** `ori.string.to_bytes`, `ori.string.from_bytes`
- **Stdlib:** `ori.bytes` com `len`, `concat`, `slice`, `to_hex`, `from_hex`, `decode_utf8`, `get`
- **Stdlib:** `ori.convert` com `float_to_string`, `bool_to_string`, `string_to_int`, `string_to_float`
- **Stdlib:** `ori.iter` com `any`, `all`, `count_where`, `take`, `skip`, `reverse`, `reduce`, `find`, `sort`, `sort_by`, `unique`, `flat_map`, `zip`, `partition`, `group_by`, `flatten`
- **Stdlib:** `ori.random.choice`, `ori.random.shuffle`
- **Stdlib:** `ori.json.stringify_pretty`
- **Stdlib:** `ori.lazy.once`, `ori.lazy.force` (declarados, sem runtime nativo)
- **LSP:** Servidor LSP funcional com diagnostics, hover, go-to-definition, completions de stdlib
- **LSP:** Índice semântico para hover de structs, enums, traits, funções e bindings locais
- **LSP:** Suporte a texto em buffer (didOpen/didChange) + fallback a arquivo em disco
- **LSP:** Refatoração modular (Sprint 1): main.rs focado em orquestração, handlers/ (diagnostics, hover, completion), index/ (semantic, project), utils/ (position, uri)
- **LSP:** Sprint 2 — context-aware completions (AfterDot, Import, Default), find references (word-boundary scan), cross-file goto-definition (resolve imports via AST)
- **LSP:** Sprint 3 — diagnósticos com debounce (300ms), Document Symbols hierárquico, Code Actions (quick fixes), Lint engine (unused_variable, prefer_const)
- **LSP:** Sprint 4 — Inlay Hints (type annotations), Semantic Tokens (syntax highlighting), Workspace Symbols (busca global), Rename (refatoração), Signature Help, Code Lens (contagem de referências)
- **LSP:** Sprint 5 — Formatting via `ori fmt` pipeline, Test Runner (`ori.runTests` via executeCommand), range_for_whole_document helper
- **Spec:** Capítulo 14 — Backend Support
- **Spec:** Capítulo 15 — Stdlib Maintenance
- **Spec:** Capítulo 16 — Runtime FFI Safety
- **CI:** `native-route.yml` validando Windows MSVC, Windows GNU, Linux GNU, macOS x86_64, macOS aarch64
- **Tooling:** `smoke_native_release.ps1` / `.sh` para validação de release package
- **Tooling:** `ORI_REQUIRE_PACKAGED_RUNTIME=1` para validar package de release

### Corrigido
- **Lexer:** BOM UTF-8 rejeitado → aceito no início do arquivo
- **Lexer:** `--|` dentro de strings tratado como comentário → tratado como texto
- **Lexer:** Comentário não fechado virava erro genérico → diagnóstico dedicado
- **Lexer/Parser:** String não terminada virava erro léxico genérico → agora emite `parse.unterminated_string`
- **Parser:** `b.value = 2` descartado silenciosamente → emite `parse.invalid_lvalue`
- **Parser/Checker:** Range com limite não inteiro emitia `type.type_mismatch` → agora emite `parse.invalid_range`
- **Parser:** Variadic `...` não parseava → parseia `...` e `..` (compat)
- **Parser:** Default antes de required não validado → emite `parse.default_before_required`
- **Parser:** ABI desconhecida em `extern` usava fallback silencioso para `C` → agora emite `extern.unknown_abi`
- **Parser:** Bloco sem `end` chegava ao EOF como erro genérico → agora emite `parse.unterminated_block`
- **Checker:** Tipos managed em fronteira `extern c` passavam até o backend → agora emitem `extern.managed_type_in_ffi`
- **Parser:** Inline `if` sem `else` emitia erro genérico → agora emite `parse.missing_else_in_if_expr`
- **Checker:** Nomes desconhecidos passavam como `Ty::Infer(0)` → emitem `name.undefined` + `Ty::Error`
- **Docs:** Função documentada com retorno não-`void` e sem `@return` → agora emite warning `doc.missing_return`
- **Checker:** `and`/`or`/`not` não validavam booleanos → validam com `expect_bool()`
- **Checker:** `break`/`continue` fora de loop passavam → emitem `control.loop_required`
- **Checker:** Result descartado sem warning → emite `type.unused_result`
- **Checker:** Closure capturando `var` → emite `mut.closure_captures_var`
- **Checker:** Literais numéricos corrompidos para zero → validados com diagnóstico
- **Checker:** F-strings aceitavam valores sem conversão para texto até falhar no backend → agora emitem `type.arg_type_mismatch`
- **Checker:** `self` fora de método caía em `name.undefined` → agora emite `bind.self_outside_method`
- **Checker:** Mutação de campo de `self` em método não-`mut` caía em erro genérico → agora emite `mut.field_mutation_in_func`
- **Checker:** Igualdade estrutural com campo sem igualdade caía em erro genérico → agora emite `type.equality_unsupported_field`
- **Checker:** `match` com case duplicado passava sem aviso → agora emite warning `match.duplicate_case`
- **Checker:** `match` com case após catch-all passava sem aviso → agora emite warning `match.unreachable_case`
- **Codegen:** `?` no backend C sem propagação → propaga com cleanup de escopo
- **Codegen:** Runtime bounds não seguiam spec → `ori_abort_bounds` para out-of-bounds
- **Codegen:** `optional<T>` e `result<T,E>` com `!=` podiam comparar payload da variante errada → agora comparam payload apenas quando as variantes batem
- **Codegen:** Structs sem genéricos não suportavam igualdade estrutural → agora comparam campos em ordem de declaração nos backends nativo e C
- **Codegen:** `set<int|string>` e `map<int|string, V>` não suportavam igualdade estrutural completa nos backends nativo e C → agora comparam por tamanho, presença de chaves/itens e igualdade dos valores
- **C Backend:** F-strings podiam avaliar expressões interpoladas de string duas vezes e truncar buffers fixos → agora avaliam cada parte uma vez e alocam pelo tamanho real
- **Runtime:** `heap.pop`/`heap.peek` para valores gerenciados não transferiam a aresta ARC ao `optional` retornado → agora o valor continua vivo após o heap sair de escopo
- **Stdlib:** `panic`/`todo`/`unreachable` não implementados → implementados
- **Stdlib:** `.or`/`.or_return`/`.or_wrap` inexistentes ou incompletos → implementados para o escopo atual (`.or_wrap` em `result<T, string>`)
- **CLI:** `ori compile` help dizia "no C compiler needed" → atualizado para refletir dependência de linker
- **Resolver:** Campos/variantes duplicados em struct/enum não diagnosticados → emite `name.duplicate_field` / `name.duplicate_variant`
- **Lexer:** `check_unclosed_block_comments()` era no-op → removida (lógica já está em `find_unclosed_block_comment`)
- **Cargo:** Lock file v4 ilegível por Rust 1.75 → downgradado para v3
- **Spec:** `math.floor/ceil/round` tipo de retorno divergente → alinhado (`-> int`)
- **Stdlib:** `stdlib_native_runtime_available()` adicionada como infraestrutura para detectar funções sem runtime nativo

### Alterado
- **CLI:** `ori compile` é a rota nativa principal; `ori build` é o C debug backend
- **CLI:** `ori test` usa a rota nativa, não depende do C backend
- **Runtime:** `ori-runtime` (Rust) é a fonte canônica de semântica de runtime
- **Stdlib:** Manifesto centralizado em `compiler/crates/ori-types/src/stdlib.rs`
- **Documentação:** Reorganização de `docs/planning/` e `docs/spec/`

### Segurança
- **Runtime FFI:** Documentadas regras de ownership, ARC e transferência para strings, bytes, collections (spec capítulo 16)

---

## [0.1.0] — 2026-05-17 (Release Inicial)

### Adicionado
- Compilador completo escrito em Rust (~25K linhas)
- 10 crates: lexer, parser, AST, types, HIR, codegen (C + Cranelift nativo), runtime, diagnostics, LSP, driver
- Lexer com suporte a 65+ palavras-chave, BOM, todos os literais, comentários, strings
- Parser recursivo descendente com recuperação de erros
- Type checker com inferência, genéricos, traits, implementações, contratos, where constraints
- HIR com monomorphization, lowering de closures, async state machine
- Backend nativo via Cranelift com ARC, async, closures, managed types
- Backend C (debug) com runtime inline, suporte parcial
- Runtime Rust como static library com ARC, executor async, channels, atomics
- Standard library: io, string, list, map, set, math, time, format, os, random, json, fs, bytes, convert, test, task, channel, atomic, deque, queue, stack, linked_list, doubly_linked_list, tree, hash_table, graph, heap, iter, lazy
- LSP server com diagnostics, hover, go-to-definition, completions
- CLI: `check`, `compile`, `build`, `test`, `run`, `fmt`
- Multi-file imports com resolução de namespaces
- Async/await com state machine nativa e executor não-bloqueante
- Especificação formal da linguagem (16 capítulos)
- CI/CD multi-plataforma para rota nativa

### Não implementado (planejado em 2026-05-17)

> **Histórico — todos os itens abaixo foram entregues em `[Unreleased]` (maio–jun/2026).**
> Mantido como registro do estado no cut do 0.1.0; para o status corrente veja
> `[Unreleased]` e `docs/planning/PLANO-MATURIDADE-COMPLETO.md`.

- `ori.Error` como tipo rico de erro — entregue (`Error` trait + campo `cause`).
- Cycle collector para ARC — entregue (`ori_arc_collect_cycles` + gatilho cooperativo).
- `ori.fs.File` como tipo — entregue (`open_read`/`open_write`/`read`/`write`/`close`).
- `using` dentro de `async func` — entregue (state machine armazena recurso no frame).
- Cancelamento público de futures/tasks — entregue (`task.CancelToken`).
- Type alias no lado esquerdo de `where` constraints — entregue.
- `lazy` runtime nativo — entregue (codegen inline nativo).
- `ori.iter` runtime nativo (apenas C backend) — entregue (flag `c_backend` em `iter.*`).
