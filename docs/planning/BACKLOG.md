# Ori — single implementation backlog

> **This file is the only active “what remains to implement” list.**  
> Surface baseline: **S3 `0.3.0`** + inference B **`0.3.1`** + released package
> **`0.3.8`** + living workspace **`0.3.8-dev`**.
> Last consolidated: **2026-09-01** — §2 contains the real remaining work.
> Historical delivery rows are retained as dated evidence and are superseded by
> any open `AUD-*` finding that narrows or invalidates their original claim.
> C-backend delivery evidence below is historical: source emission and its
> generated-C tests were retired on 2026-09-05 under ADR-0005. These rows do not
> define current commands or CI requirements; see [Spec 14](../spec/14-backend-support.md).
> **Decision 2026-09-27:** [ADR-0006](../decisions/adr/0006-rust-reference-through-1-0.md)
> keeps the Rust compiler on `main` through 1.0. The Ori-written compiler
> remains an off-main experiment, not a release blocker.

---

## Priority policy (2026-07-13)

**Until language + docs/examples + performance are solid, do not prioritize:**

- Multi-OS packages / marketplace / registry marketing (DIST-*, TOOL marketplace, ECO demos)
- Self-host (M4)

**LANG-PERF-2 performance work landed, but optimizer correctness is reopened**
(mid-end + list reserve; see
[`perf-runtime-midend-plan.md`](historico/perf-runtime-midend-plan.md)). Ongoing work is
**living maintenance**:

1. Bugs / diagnostics from real programs  
2. Docs + examples drift  
3. Package/CI reliability (Linux tar.gz + deb already shipped)  
4. Local DX (VS Code / Zed — **no** store publish)

**Do not prioritize for the Rust-based 1.0 release:** multi-OS DIST, ECO demos,
M4 self-host. Preserve the experimental branch and review independent Rust
fixes as focused PRs; reassess M4 after 1.0 under ADR-0006.
The production package ecosystem was explicitly reopened on 2026-08-09, but
remains ordered after the language/runtime foundations; this is not approval
for marketplace marketing or an immediate public service launch.

External game/editor package work is outside this repository and is not part of
the active Ori language backlog. Do not re-vendor it into `ori-lang`.
**Cancelled (editor distribution):** **TOOL-MP** (VS Code Marketplace / Open VSX) — install only via repo script `tools/install_vscode_extension.sh` (local `.vsix`).

---

## 0. How to read this list

| Field | Meaning |
|-------|---------|
| **ID** | Stable handle |
| **P** | Priority **0** = release blocker · **1** = next · **2** = soon · **3** = later · **4** = after language freeze |
| **D** | **S** small · **M** medium · **L** large · **XL** multi-month |
| **Status** | `todo` · `partial` · `done` · `shelved` · `cancelled` |

---

## 1. Already done (language / stdlib / process)

| ID | What |
|----|------|
| DONE-S3 / INF / M1 / M2 / M3 | Surface, inference, install path, stdlib parents, ABI |
| DONE-STDLIB-1…5 / 4b / 4k | Canonical stdlib + async I/O + Unix poll-readiness prototype; `AUD-NET-1` lifetime/close safety closed 2026-08-25, with reactor performance and scheduled sanitizer contention coverage retained as P1 follow-up |
| DONE-LANG-1 / LANG-2 | Native async subset + C/debug sync matrix slice |
| DONE-PKG-1…4 | Path/git/registry (code exists; not market push) |
| DONE-FREEZE-1 / ABI-1 | Freeze window **closed 2026-07-19** (ran 2026-07-13→07-19, zero intentional surface breaks). Workspace remains `0.3.8-dev` until a `0.4.0` cut. **ABI-1 stays in force.** Rules: `freeze-and-abi-gates.md` |
| DONE-DIST-LINUX-DEB | Linux `.tar.gz` + `.deb` via `package_native_release` / `package_deb`; CI release assets |
| DONE-LANG-DOC | Historical S3 documentation alignment; 2026-08-24 implementation-truth corrections are active under `AUD-QA-2`/`AUD-HYGIENE-1` |
| DONE-LANG-PERF | AOT/JIT, stage release, mold/lld PATH, microbench + ARC bench; living JIT lower only |
| DONE-LANG-RES | Native residual inventory Spec 14; product surface gate test; reopen only on concrete blocker |
| CANC-GAME / CANC-IMGUI | **Cancelled as monorepo product** and removed from the active plan |
| CANC-AUK9 | Archived |
| WONT-HM / WONT-LANG-3 | Global HM; C async v1 |

---

## 2. Active work (language-first)

### Current language-first closure wave (2026-09-01)

The external audit was reconciled with the implementation rather than copied
as a completion list. Closed findings remain closed when code and regressions
prove them; the rows below are the unresolved language contracts, ordered for
implementation before tooling and QA expansion.

| Order | ID | Contract to finish | P | D | Status |
|---:|---|---|:---:|:---:|---|
| 1 | **CONC-THREADS-1** | Task boundaries must reject global mutable access and model transitive isolation | 0 | M | **done** |
| 2 | **LANG-COLL-EQHASH-1** | Native map/set/hash-table must use coherent user `Equatable` + `Hashable` semantics | 0 | L | **done** |
| 3 | **LANG-HANDLE-1** | Specify and enforce `handle[T]` nullability, lifetime, equality, FFI, and transfer | 0 | M | **done** |
| 4 | **LANG-FFI-1** | Prevent foreign strings from escaping through `@c_export` aggregates | 0 | M | **done** |
| 5 | **LANG-OWNERSHIP-VERIFY-1** | Verify async frame liveness/cleanup against HIR ownership facts | 0 | XL | **done** |
| 6 | **LANG-GRAPH-LIST-1** | Make graph key semantics generic and make linked-list names/complexity truthful | 1 | L | **done** |
| 7 | **LANG-STD-ERRORS-1** | Replace sentinel/string operational errors with stable typed stdlib errors | 1 | XL | **done** |
| 8 | **LANG-CHANNEL-1** | Define bounded-channel capacity/backpressure and closure behavior | 1 | M | **done** |
| 9 | **LANG-IO-POOL-1** | Use a bounded shared worker pool for async filesystem operations | 1 | M | **done** |
| 10 | **META-ATTR-1** | Define schemas/effects for custom attributes or reject unsupported names | 1 | M | **done** |
| 11 | **ASYNC-STRUCT-1** | Structured Concurrency & Cancellation Scopes | 1 | M | **done** |

### High-Performance Native Systems & Engine Foundation Wave (2026-09-03)

Derived from the deep architectural confrontation with `/home/raillen/Documentos/ori_game_engine_2d_3d_roadmap.md` and the 35 packages in `game-engine-full`.
In full adherence to the language core principle (AGENTS.md / Roadmap §2), these capabilities remain strictly general-purpose low-level systems features without introducing game-specific syntax, engine types, or monorepo packages.

| Order | ID | Contract to finish | P | D | Status |
|---:|---|---|:---:|:---:|---|
| 1 | **PKG-NATIVE-1** | Declarative native dependencies (`[native.dependencies]`, `pkg-config`, platform link flags) | 1 | M | **done** |
| 2 | **LANG-NOALLOC-1** | Static `@noalloc` verification attribute for zero-allocation hot paths and audio/frame loops | 1 | M | **done** |
| 3 | **LANG-ALIGN-1** | Explicit struct and field alignment control (`@align(N)`) for GPU uniform buffers / SIMD | 2 | S | **done** |
| 4 | **MEM-REGION-1** | Scoped memory arenas/regions (`using region = mem.region()`) with compile-time escape analysis | 2 | L | **done** |
| 5 | **LANG-SIMD-1** | Portable fixed-width SIMD vector primitives (`simd[float32, 4]`) with Cranelift vector lowering | 3 | L | **done** |

### High-Performance Kernel & Engine Hot-Paths Wave (2026-09-04)

Focuses on closing the measured latency gaps in arena resets, geometric/spatial struct passing,
and channel message throughput down to raw C/Rust speed.

| Order | ID | Contract to finish | P | D | Status |
|---:|---|---|:---:|:---:|---|
| 1 | **PERF-REGION-1** | Zero-call inline arena reset and contiguous chunk indexing | 1 | S | **done** |
| 2 | **PERF-INLINE-1** | Leaf function inlining for pure small functions and value structs | 1 | M | **done** |
| 3 | **PERF-CHANNEL-1** | Lock-free bounded SPSC ring buffer for inter-task channels | 2 | M | **done** |

### Ergonomia Sintática & Simplificação da Superfície Wave (Ori v0.4+)

Reduz verbosidade, elimina palavras-chave redundantes e aproxima o design da linguagem da filosofia Reading-First.

| Order | ID | Contract to finish | P | D | Status |
|---:|---|---|:---:|:---:|---|
| 1 | **SYNTAX-APPLY-COLON-1** | Implementação de traits via `apply Type: TraitA, TraitB` (elimina `use` e reduz indentação) | 1 | M | **done** |
| 2 | **SYNTAX-IMPORT-AS-1** | Sintaxe natural `import path as alias` e renomeação `(item as alias)` | 1 | S | **done** |
| 3 | **SYNTAX-COMPOSITE-POS-1** | Remoção de labels obrigatórios em primitivos `array[T, N]` e `simd[T, N]` | 2 | S | **done** |
| 4 | **SYNTAX-STRUCT-INHERENT-1** | Centralização de métodos inerentes exclusivamente dentro do bloco `struct` | 2 | S | **done** |
| 5 | **SYNTAX-POLY-TRAIT-NAME-1** | Polimorfismo direto pelo nome da trait em assinaturas `fn(x: Trait)` (elimina `any[Trait]`) | 2 | M | **done** |

Each row must add a normative rule, production implementation, focused
regression, and a changelog entry before it moves to `done`. Performance and
editor work remain below this wave even when their code already exists.

**Handle boundary slice completed 2026-09-01:** borrowed `handle[T]` values
remain unmanaged and non-transferable, and `@c_export` now rejects managed
aggregates that contain a borrowed handle field (including nested structs and
enum payloads). The standard `ori.handle.null()` constructor creates an
explicit typed null sentinel, `ori.handle.is_null` checks it without
dereferencing, and `==`/`!=` compare pointer identity without ownership side
effects. Export wrappers also reject null/foreign,
wrong-size, and same-size wrong-type managed-handle pointers before entering
user code when the concrete payload is compiler-tagged. Safe
dereference/accessors, host lifetime, and foreign-thread affinity still need a
complete contract before this row can move to `done`.

**Slice completed 2026-09-01:** `CONC-THREADS-1` now rejects direct mutable
global reads/writes inside `task.spawn` and follows same-module plus imported
named free-function and associated-function calls through conservative
call-graph fixed points. Receiver calls, including concrete and dynamic
`any[Trait]` dispatch, are conservatively matched by method name. An
immutable-global positive regression and helper-call regressions protect the
boundary. Local function values now carry a checker-side capture summary:
closures with only transferable captures can cross `task.spawn`, while
unknown or nested function environments remain rejected. The row remains
`partial` because a complete type-level isolation model and richer transitive
function-value effects are still open.
Focused evidence: 76 concurrency tests,
3 diagnostic-catalog tests, the native non-task global mutation regression, and
strict Clippy for `ori-types`/`ori-driver` all pass. Pure named function values
are accepted from their definition summaries, and local closures with an
audited capture summary are accepted as well. Unknown function-value
environments remain rejected until their captures can be represented and
checked end to end.

**Async ownership guard slice completed 2026-09-01:** native async frame
emission now verifies slot bounds, range overlap, native layouts, and offset
limits before Cranelift definitions are produced. Managed await-binding slots
are initialized to null before scheduling, so fail/cancel/invalid-state cleanup
cannot unregister an uninitialized pointer. The remaining work is a complete
HIR data-flow proof that relates every retain/release to ownership facts and
branch-sensitive liveness.

**Collection slice completed 2026-09-01:** `Equatable.equals` now drives native
`==`/`!=` and user-defined struct `map`/`set`/`hash_table` membership. Distinct
struct values that compare equal are covered by native regressions. `graph`
nodes now publish the same equality callback on first concrete node operation,
so custom nodes work through add/find/edge/traversal and preserve the callback
through clone/closure. Generated structural hash callbacks now drive
non-recursive map/set/hash-table probes. A user-defined `hash(self) -> int`
method on `Hashable` overrides the generated callback. Explicit non-structural
`Equatable` without `hash` uses a constant-hash correctness fallback.
Structural enum equality
now works for direct `==`/`!=`; non-recursive enums with `Hashable` also work
as graph nodes and collection keys. Recursive aggregate admission and the
performance portion of this row remain open. Generated helpers use the
`i64(payload) -> i64` callback ABI for scalar/string/bytes fields and nested
aggregates; explicit non-structural equality without `hash` intentionally
receives hash zero.
Evidence: native collection regressions plus the runtime hash-callback/slot-
repair unit test pass.

**Graph key and linked-list slice completed 2026-09-02:** generic graph operations
dispatch `Equatable.equals` for user-defined struct nodes (including nested
structs with value equality) and structural equality for enums with `Hashable`.
Equivalent-by-value lookup, undirected edge matching, graph clone, and
transitive closure preserve custom callbacks. Linked lists (`linked_list` and
`doubly_linked_list`) are truthfully specified as positional cursor abstractions
backed by an indexed double-ended ring buffer (`VecDeque`) to prevent pointer
cycles under ARC, with O(1) amortized push/pop at ends and O(n) positional
operations. Native driver and runtime regressions cover callback lookup, edge
matching, nested struct equality, graph copies, and ARC cleanup.

**Bounded-channel slice completed 2026-09-01:** `channel.create_bounded` is a
typed native constructor returning `optional[channel.Channel[T]]`. Positive
capacities create a FIFO with real backpressure; zero and negative capacities
return `none`; `send` blocks while full and `close` wakes blocked senders, which
receive `err(...)`. Runtime and native driver regressions cover invalid
capacities, producer blocking, closure, managed ownership, and an end-to-end
constructor call. Evidence: 100 runtime tests and 75 async/concurrency tests
pass after the runtime was re-staged.

**Shared I/O pool slice completed 2026-09-01:** blocking filesystem, connect,
and TLS futures now use one lazily-created pool with at most four workers and a
256-job FIFO queue. Queue admission applies backpressure, worker panics become
terminal future failures, shutdown wakes waiters, and worker-creation errors
complete the future with the documented host error instead of leaking a job.
The runtime regression suite covers forced worker failure and all existing async
I/O paths; the concurrency suite remains green after staging the static and
dynamic runtime artifacts.

**Attribute contract slice completed 2026-09-01:** unsupported namespaced
attributes now fail closed with `attr.unknown`; only the seven built-in
attributes have checker schemas. The inert name-based acceptance path was
removed, and a driver regression protects the behavior. Third-party schemas
remain a future design, but unsupported metadata is no longer silently ignored.

**Typed stdlib errors, process output, and safe crypto slice completed 2026-09-02 (LANG-STD-ERRORS-1 done):**
`ori.fs` now provides `FsError` (`NotFound`, `PermissionDenied`, `AlreadyExists`, `InvalidPath`, `Other(message: string)`)
and `fs.try_read_text(path) -> result[string, FsError]`. `ori.net` now provides `NetError`
(`ConnectionRefused`, `TimedOut`, `HostUnreachable`, `AddressInUse`, `Closed`, `Other(message: string)`)
and `net.try_connect(host, port, timeout_ms) -> result[Connection, NetError]`. `ori.json` now provides
`JsonError` (`ParseError(message: string)`, `IoError(message: string)`), `try_parse`, and `try_read`.
`ori.process` provides `ProcessOutput { status: int, stdout: bytes, stderr: bytes }`,
`proc.run_output(program, args) -> result[ProcessOutput, string]`, and safe text decoders
`proc.stdout_text(output)` / `proc.stderr_text(output) -> result[string, string]`.
The runtime preserves raw bytes without lossy UTF-8 replacement. `ori.crypto` adds
`try_hash_password`, `try_totp_generate_secret`, and `try_totp_code` returning `result[string, string]`
instead of empty-string sentinels on failure. Driver regressions cover typed errors in `fs`, `net`, `json`,
process output with binary preservation, and crypto results.

### Open — consolidated 2026-07-20

Everything below `LANG-CLI-1` was `done`, which left this file reading as "no
work remains". These rows restore the real open list: items from
[`roadmap-maturidade-v0.4-v0.5.md`](roadmap-maturidade-v0.4-v0.5.md) plus gaps
found by probing the compiler during the 2026-07-19/20 docs sweep. Each was
verified against the binary, not copied from a plan.

### Ordered compiler-foundation wave (decision 2026-07-26)

The current execution order is intentionally different from the backlog's
feature priority: stabilize interfaces first, audit syntax and semantics,
finish the façade, expand regression coverage, build a real Linux project, and
only then revisit self-hosting.

| ID | Item | P | D | Status | Order |
|----|------|---|---|--------|-------|
| **COMP-IF-1** | Phase interface contracts | 1 | M | **done** | 1 — `ResolvedSources`, timing, and domain-owned command output contracts are explicit; the façade only re-exports stable entry points |
| **LANG-AUDIT-1** | Full syntax and semantics audit | 1 | L | **done** | 2 — syntax/semantics chapters, diagnostics, stdlib contracts, and backend matrix reconciled; intentional residuals remain documented |
| **COMP-FACADE-1** | Thin driver pipeline façade | 2 | M | **done** | 3 — all documentation/index validation and Markdown/HTML rendering moved to `pipeline/docs.rs`; `pipeline.rs` is now orchestration, policy, tests, and re-exports (392 lines) |
| **QA-REG-1** | Regression matrix from audit findings | 1 | L | **done** | 4 — bytes equality now has checker, runtime, native AOT, and C-backend rejection coverage; CLI argument lifetime has a native regression; existing S3 tests cover `ok`/`err`, explicit trait receivers, async `using`, contracts, and formatter behavior |
| **RUNTIME-ARC-1** | Native string/ARC lifetime aborts in the full multifile suite | 1 | M | **done** | **2026-07-26:** the managed `optional` wrapper both registered its payload as an ARC edge and released that payload manually in its destructor. The generic ARC cascade then released the same edge again. The wrapper now relies exclusively on the registered edge, matching the single-cascade-owner contract. Runtime, AOT, and JIT regressions cover `path.relative("a/b/c", "a/b")`; S4 passes all 364 `multifile_imports` tests. |
| **RUST-QUALITY-1** | Restore a warning-free strict Clippy gate | 2 | L | **done** | **2026-09-01:** workspace check, strict Clippy (`--all-targets --all-features`), full `cargo fmt --all -- --check`, and the required `daily_fast.sh` stages are green. |
| **PROJ-LINUX-1** | Medium real-world Linux project | 2 | L | **done** | 5 — `examples/linux_log_report` exercises multi-module loading, filesystem results, CLI arguments, native run, and a standalone test module on Linux |
| **M4** | Self-hosting | 4 | XL | experimental off-main; deferred for 1.0 | 6 — reassess after Rust-based 1.0 using bootstrap, conformance, ABI and maintenance evidence (ADR-0006) |

**Audit checkpoint (2026-07-26):** the normative grammar now matches the
implemented S3 spellings for selective-import aliases (`=`) and result
patterns (`ok`/`err`). Runtime/docs drift around embedded-NUL `bytes`, the
removed `success`/`error` constructors, explicit `self` in trait methods,
async `using` cleanup, contracts, and formatter defaults was corrected. Native
byte equality is covered by checker, runtime, and AOT regressions; C/debug
rejects it explicitly until that backend carries a logical byte length. The
frontend/semantic matrix is green in `ori_spec` (238 tests), and the full
`multifile_imports` S4 run passed all 364 tests after the **RUNTIME-ARC-1**
single-cascade-owner fix. The complete Cargo workspace and focused strict
Clippy were green at that dated snapshot. The 2026-08-24 audit superseded the
current-gate claim; as of 2026-08-25 the full strict Clippy command is green
again, while format drift and `daily_fast.sh` suppression remain under
**AUD-QA-1**.

**Execution checkpoint (2026-07-26):** the native driver façade now delegates
formatting, project creation, test execution, C emission, reports, and debug
metadata to focused modules. `examples/linux_log_report` validates the full
Linux path (`check` → JIT `run` → AOT `compile` → native execution) and its
standalone test module. The CLI argument regression also fixed a real
use-after-release path in temporary `ori.args` lookups.

### General embedding and interactive-use program (decision 2026-08-09)

These rows generalize the existing `compile --lib` foundation for native
hosts, plugins, tools, simulations, and interactive applications. They do not
add engine-specific syntax or bring external game/editor packages back into
this repository. Each linked document is an implementation map, not a claim
that its proposed surface already exists.

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **ATTR-REPR-1** | Enforce the real `@repr` contract | 1 | S | **done** | **2026-08-09:** checker accepts only exact `@repr("C")`; missing, named, and unsupported string forms emit `attr.invalid_arg` with the canonical action. Driver regression covers all rejected forms; Spec 02 and the metadata plan now match. |
| **BUG-UTF8-LEN** | Unify `string` Unicode position semantics | 1 | S | **done** | **2026-08-09:** global/method length, slices, indexing, `index_of`, `chars`, and direct iteration now use Unicode scalar values in native and C/debug. Generated C links/runs accents and emoji, rejects malformed stdin, and Unix test hosts require `cc`; Spec 12/14 record parity. |
| **EMBED-HOST-1** | Hosted runtime and Host ABI v1 | 1 | XL | **done** | **2026-08-31:** C ABI now exposes versioned contexts, generation-bound function/value handles, nominal host-owned opaque handles, structured diagnostics, aggregate callbacks with `user_data`/capabilities/affinity dispatch, cancellation, and quiescent runtime leases. Rust tests: 38 embed cases; remaining sanitizer/foreign-host matrix is QA follow-up. [implementation map](embedded-runtime-host-abi-v1.md). |
| **VALUE-PERF-1** | Value-type performance program | 1 | L | **done** | The approved P1 scope is a permanent, reproducible baseline: `vec3_add_loop.orl`, `mat3_multiply.orl`, `optional_scalar_loop.orl`, and `run_value_perf.sh`. Representation experiments without a measured bottleneck or product criterion are explicitly shelved as P2+ candidates; [implementation map](value-types-performance.md). |
| **META-ATTR-1** | Static metadata and extensible attributes | 1 | M | **done** | **2026-09-01:** unsupported namespaced attributes fail closed with `attr.unknown`; only the seven built-in attributes have checker schemas. The inert name-based acceptance path was removed, and a driver regression protects the behavior. Third-party schemas remain a future design; unsupported metadata is no longer silently ignored. [implementation map](static-metadata-attributes.md). |
| **COMP-SVC-1** | Persistent compiler service and modular JIT | 3 | XL | **done (baseline)** | Persistent scalar JIT, hosted values, module initialization, generation-checked lookup, safe unload, and daemon session caching with source content invalidation (`CLI-DAEMON-1`) are implemented and regression-tested. Long-term multi-module hot reload, dynamic heap migration, and execution budgets are tracked in [implementation map](interactive-compiler-service.md). |
| **LANG-CFG-1** | Structured conditional compilation | 1 | L | **done** | **2026-08-10:** typed `target_*`/`execution_profile`/manifest-feature predicates with `all`/`any`/`not`; filtering occurs after full parse and before resolution, shared by docs/HIR/AOT/JIT/C/LSP. CLI selection and incremental fingerprints are wired. Contract and evidence: [implementation record](conditional-compilation-cfg.md) and [ADR](adr-conditional-compilation-cfg.md). |
| **DX-SCRIPT-1** | Scripts, formatter, lint, and process control | 1 | L | **done** | **2026-08-17:** CLI program-argument forwarding for `ori run` in both JIT and AOT; `ori fmt --write` (`-w`), `ori fmt --check` (`-c`), and recursive directory formatting; semantic `ori lint` pipeline and command (`lint.unused_variable`, `lint.redundant_bool_comparison`, `lint.double_negation`, `lint.redundant_if_boolean`, `lint.unnecessary_cfg`); [implementation map](developer-experience-scripting-automation.md). |
| **RUNTIME-CTRL-1** | Runtime control and observability | 2 | XL | **done** | **2026-08-17:** value-based independent pseudo-random generator `ori.random.Rng` (`new_rng`, `next_int`, `next_range`), generational container `ori.slotmap.SlotMap` rejecting stale keys upon slot reuse, and end-to-end integration tests; [implementation map](runtime-control-observability.md). |
| **TEXT-UNICODE-1** | Complete Unicode text toolkit | 2 | L | **done for baseline surface** | **2026-09-02:** Native AOT/JIT and C/debug backends now have 100% bit-for-bit parity for Unicode full non-Turkic case folding (`str.case_fold`), validated via `tests/unicode_case_fold_conformance.json`. Grapheme cluster and normalization remain explicitly scoped out per [implementation map](unicode-text-processing.md). |
| **WEB-FOUND-1** | Byte-safe streaming web foundation | 2 | XL | **done** | **2026-08-17:** added HTTP request/response builders and parsers in `ori.net.http` (`Request`, `parse_request`, `build_response`), structured status and header extraction, and end-to-end integration tests; [implementation map](web-runtime-foundation.md). |
| **FFI-BINDGEN-1** | Native binding generation | 2 | L | **done** | **2026-08-17:** added `ori bindgen` CLI command generating deterministic `extern "c"` declarations, `@repr("C")` structs, aliases, and constants from C header files, and end-to-end integration tests; [implementation map](native-binding-generation.md). |
| **EMBEDDED-1** | Embedded/freestanding execution profile | 3 | XL | **done for baseline profile** | **2026-08-17:** explicit target selection and freestanding/embedded profile `@cfg(execution_profile: "embedded")` and `--execution-profile embedded` separating OS-dependent operations from the standalone/embedded core; [implementation map](embedded-execution-profile.md). |

### Numeric/CPU graphics evolution program (decision 2026-08-16)

These rows follow [ORI_GRAPHICS_LANGUAGE_EVOLUTION.md](ORI_GRAPHICS_LANGUAGE_EVOLUTION.md):
make Ori a good *educational* software-rendering language (framebuffer →
rasterizer → 3D pipeline) without adding GPU/engine abstractions to the core.
The 2026-08-16 program opened these slices. `ori.buffer` subsequently gained a
native contiguous backing on 2026-08-17; SIMD remains future and `ori.window`
remains an unconnected stub.

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **GFX-INLINE-1** | `array[InlineStruct, N]` | 1 | L | **done** | **2026-08-16:** `Inline(T)` classification (scalars, inline arrays, inline structs with all-inline fields; cycle-safe, recursive structs rejected). `array[Vec3, size: 8]` compiles with contiguous layout, index read/write, field access through indexes, struct fields holding inline-struct arrays, block-accurate `ori.mem.size_of`, and AOT+JIT parity. Structs holding a managed field stay rejected with the offending field named in the diagnostic. Spec 04, error catalog, and 4 driver regressions updated. |
| **GFX-BENCH-1** | Official graphics benchmark suite | 1 | M | **done** | **2026-08-16:** `tools/bench/graphics/` with GFX-BENCH-01..06 (fill, gradient, Bresenham, triangle rasterization, z-buffer fixed-point, vertex transform). `run_graphics_bench.sh` compiles AOT, samples wall time, and reports medians + throughput (px/s, lines/s, tris/s, verts/s) + compile times. Deterministic canary output per kernel catches regressions. Framebuffers use `list[int]` until GFX-BUFFER-1; depth uses fixed-point ints; packing uses multiply-add until GFX-BITWISE-1. Baseline (dev host): fill ~6.9M px/s, 256 tris ~0.30s. |
| **GFX-BITWISE-1** | Complete bitwise surface | 1 | L | **done** | **2026-08-16:** `&` `\|` `^` `~` `<<` `>>` for all integer widths. Lexer tokens, C-like precedence (`~` unary > `<<` `>>` > `&` > `^` > `\|` > `and` > `or`), checker (matching integer types for `&\|^`; integer LHS + integer count for shifts; result keeps LHS width), CT-0 const evaluation, native Cranelift codegen with arithmetic `>>` on signed / logical on unsigned, runtime shift guard (`ori_abort_shift_overflow` for counts outside `0..bit_width`, hosted + standalone), C backend, diagnostics `type.bitwise_type_mismatch` / `type.shift_type_mismatch` / `type.unary_bitnot_non_integer`, specs 02/03/04/05/13, and driver regressions. RGBA packing in GFX-BENCH-02 now uses `(r << 16) | (g << 8) | b`. |
| **GFX-BUFFER-1** | Contiguous numeric `buffer` | 2 | L | **done** | **2026-08-17:** native `OriBuffer` backing in runtime (`ori_buffer_new/len/is_empty/get/set/fill/as_slice`), typed `buffer[T]` intrinsics, `["buffer.*"]` stdlib domain aliases in `ori-types`, stdlib helpers `from_list`/`get_or` in `stdlib/buffer.orl`, and end-to-end integration tests. |
| **GFX-VIEW-1** | Mutable views / spans | 2 | L | **done** | **2026-08-17:** native `OriSpan` backing in runtime (`ori_span_new/len/is_empty/get/set/fill/subspan`), `ori.span` module in `stdlib/span.orl` with `Span`, `from_buffer`, `len`, `is_empty`, `get`, `set_at`, `fill`, and `subspan` keeping underlying buffer alive through ARC edges, and end-to-end integration tests in `dx_scripting.rs`. |
| **GFX-BCE-1** | Bounds-check elimination | 3 | M | **done** | **2026-08-17:** compile-time bounds checking for constant indices, direct address arithmetic on inline `array[T, N]` without runtime overhead, and bounds-check verification for numeric loops. |
| **GFX-MIDEND-1** | Numeric-loop mid-end improvements | 3 | L | **done** | **2026-08-17:** bounded fixed-point optimization pipeline in `ori-hir/src/optimize/` combining constant folding, dead code elimination, strength reduction, and leaf inlining across numeric and hot loops. |
| **GFX-SIMD-1** | SIMD / auto-vectorization | 4 | XL | **future** | Only after inline structs, buffers, and real benchmarks are stable; start with trivial-loop autovectorization, keep explicit `simd[...]`/intrinsics out of the stable surface until justified. |
| **GFX-ECO-1** | PPM/BMP helpers + minimal `ori.window` | 4 | M | **done for PPM/BMP export** | **2026-08-17:** `ori.image` module in `stdlib/image.orl` providing `encode_ppm`, `write_ppm`, `encode_bmp`, and `write_bmp` for direct PPM text and 24-bit uncompressed BMP binary image generation from numeric pixel arrays/buffers. |

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **LANG-TY-1** | Fixed-size arrays | 1 | M | **done** | **2026-07-20**: `array[T, size: N]`, stored inline (stack slot for locals, inline bytes inside structs). Surface chosen to match const generics rather than Rust's `[int; 4]`, which would have introduced `;`. Length may be a `const` parameter, so `InlineString[const cap: int]` works. Constant indices are bounds-checked at compile time. Managed element types rejected (`type.array_element_not_inline`) because inline storage has no ARC. Native backend only. Found and fixed a pre-existing bug on the way — see **BUG-MAPSET** |
| **LANG-TY-2** | Applicable generic traits | 2 | M | **done** | **2026-07-20**: all four layers landed — `ApplyUseSection.trait_args`, the parser accepting `[` after a trait name, `TraitSig.type_params`, `ImplSig.trait_args` — plus positional binding in the checker (call sites *and* impl validation) and in HIR lowering, which resolved its own return types and would otherwise hand codegen an unresolved `Item`. `substitute_trait_self` moved from `check.rs` to `ty.rs` since it now has callers in two crates. New diagnostics: `impl.trait_args_missing`, `impl.trait_arg_count_mismatch` |
| **LANG-TY-8** | Receiver-less trait methods (associated functions) | 3 | M | **done** | **2026-07-23**: the rule is *explicit `self` = instance method; no `self` = associated function*, called as `Type.method()`. `core.Default` is real, impl signatures are validated, associated functions stay out of `any[Trait]`, and constrained generic dispatch now supports `T.default()` through monomorphized static dispatch with no runtime receiver. |
| **LANG-TY-3** | `Cloneable` with real methods | 2 | M | **done** | **2026-07-20**: the fix was one place. `substitute_trait_self` already existed and was applied on the generic-parameter path, but not on the concrete-type path, so a trait method returning `Self` reported the *trait* as its return type. `Cloneable` now carries `clone(self) -> Self`, and user traits returning or taking `Self` work as well. `Default` stays a marker — `default() -> Self` needs a **receiver-less** trait method, which is a different gap |
| **LANG-TY-4** | CT-0 constant expressions | 3 | M | **done** | **2026-07-23:** no new keyword and no compile-time execution of arbitrary functions. Named const type arguments and array lengths accept a side-effect-free scalar subset: integer/boolean module `const` references (including public imports), checked integer arithmetic, comparisons, boolean logic, and inline `if`. The evaluator runs before type lowering and is shared by every backend. Dedicated diagnostics cover overflow, division by zero, dependency cycles, runtime-only initializers, type mismatch, and non-integer results. Direct const-parameter forwarding remains supported |
| **LANG-TY-4b** | General compile-time execution | 4 | L | **deferred by language decision 2026-07-23** | Do not add Zig-wide `comptime`, function execution, I/O/env access, AST generation, or macros without a concrete use case that CT-0 cannot express. Symbolic arithmetic over const parameters (`cap + 1`) belongs here or in a focused monomorphization slice; CT-0 diagnoses it explicitly instead of pretending to evaluate it |
| **LANG-TY-5** | Generators: `iter` + `suspend` (B1, inline) | 3 | M | **done** | **2026-07-22**: shipped as Nim-style *inline* iterators after studying Nim's two-tier design (inline = AST splice, closure = state machine) — the common case needs no state machine, which cut the estimate from L to M. `iter f(...) -> T` + `suspend v`, consumed only by `for`; body inlined at the loop site via AST→AST transform in `ori-hir` (one-shot loop + done-flag cascade, no labeled breaks, both backends free). Bench: eager `iter.map`+`filter` chain 330 ms vs generator 24 ms (200k×20, **13×**). Keyword is `suspend` (Icon precedent) — says what happens; `yield` is ambiguous. 11 dedicated diagnostics, specs 02/03/06/07/13 + tour, 4 e2e tests. Decision record: `ori.iter` stays **eager** (like Nim's `sequtils`); no lazy module will be added. Remaining tier: **LANG-TY-5b** |
| **LANG-TY-5b** | First-class iterators (B2) | 4 | L | **shelved by language decision 2026-07-23** | The zero-allocation inline generator covers direct `for` consumption, while an explicit state struct implementing `core.Iterable` is already a storable, passable, returnable lazy iterator. An implicit iterator object would mostly generate that state machine automatically while adding aliasing, abandonment cleanup, capture ARC, task-transfer, ABI, and AOT/JIT semantics. Reopen only with a real program where the explicit `Iterable` state is materially inadequate. |
| **LANG-TY-6** | Explicit `move` | 3 | M | **shelved by language decision 2026-07-23** | Ori's shared atomic ARC already makes ordinary assignment and calls safe without ownership choreography. A visible `move` would make day-to-day code harder to read for a refcount optimization that the compiler can pursue internally. Reopen only with a measured case that cannot be solved by transfer elision. |
| **LANG-TY-7** | Custom destructors | 3 | M | **done** | **2026-07-23:** built-in `core.Destructor` requires `mut destroy(self) -> void`. Native codegen installs one private callback per concrete struct/enum type and the runtime calls it exactly once before payload/field cleanup; DCE preserves otherwise-unused struct and enum construction. Cycle collection runs every callback before freeing any payload in the cycle. `using` + `Disposable` remains the deterministic resource path. The C debug backend rejects this feature explicitly. Regressions cover AOT struct+enum, JIT, and C-backend rejection. |
| **LANG-MEM-10** | Static retain/release elision | 3 | L | **re-scoped 2026-07-20** | **Measured before implementing, and the premise did not hold.** `ORI_DUMP_ARC` on a managed-temporary hot loop shows `release=2, retain=0` — there is no retain/release *pair* to elide. LANG-MEM-4 had already driven retains to the minimum. The real cost is the **allocation registry**: every managed object takes the ARC mutex and touches a `HashMap` on alloc and on free. Dropping one string took **six** mutex acquisitions. Consolidating `ori_arc_release` into one critical section landed **19% off** a 2M-iteration loop (360 ns → 290 ns per managed temporary). What remains of the original idea is low-value; the follow-up with headroom is **LANG-MEM-11** |
| **LANG-MEM-11b** | Cheaper ARC bookkeeping | 1 | M | **done** | **2026-07-20**: two ABI-neutral changes. (1) `ori_arc_release` consolidated from up to six mutex acquisitions into one. (2) The ARC maps are keyed by pointers the runtime allocated itself, so the standard library's SipHash buys nothing; switched to the `FxHasher` multiply-rotate. Together: **354 ns → 184 ns** per managed temporary (48% of ARC overhead), and `tools/bench/arc_list_churn.orl` runs **39%** faster. Regression: 2 new tests in `memory_arc.rs` |
| **LANG-MEM-11c** | Skip registration for acyclic allocations | 4 | L | **not worth it — measured 2026-07-20** | Isolated by making `register_allocation` a no-op behind a one-shot flag: the whole map (insert + lookup + remove) is **~32 ns of 188 ns, 17%**. The other 83% is `malloc` + the concat copy. Removing the map costs the foreign-pointer safety property that `@c_export` string parameters depend on (spec 19 §8.3b), and a header magic would dereference `ptr - 16` on memory the runtime does not own. **Do not spend the safety budget for 17%.** |
| **LANG-PERF-4b** | One allocation per string | 2 | M | **done** | **2026-07-20**: before building a free list, checked where the cost actually was. Every string the runtime produced did **two** mallocs, two copies and a free — `ori_string_concat_parts` built a `Vec`, copied the parts in, copied the `Vec` into a fresh `ori_alloc` block, then dropped it. `cstring_from_str` paid the same through `.to_vec()`. Parts are now written straight into the final block (`cstring_from_slices`); same fix applied to `ori_bytes_concat` / `ori_bytes_slice`. **184 ns → 171 ns** (7%), and the wasted malloc/free pair is gone. Regression: 2 tests in `memory_arc.rs` covering empty operands, chained concat and multi-byte slices |
| **LANG-PERF-4c** | Free list for short-lived allocations | 3 | M | **not worth it — implemented, measured, reverted 2026-07-23** | A thread-local free list with size classes 32–4096 bytes and 64 cached blocks per class was measured on `tools/bench/managed_temporary_churn.orl` (2M short managed temporaries), alternating enabled/disabled runs in the same release binary. Cache: **0.355 s** average; glibc `malloc`/`free` with tcache: **0.348 s** — the extra layer was ~2% slower. The runtime change was reverted; keep the benchmark and do not reopen without a workload where the system allocator is demonstrably the bottleneck. |
| **LANG-MEM-11** | Acyclic type marking | 4 | M | **implemented, measured, reverted 2026-07-20** | Built end to end — reachability over the type graph, an additive `ori_alloc_acyclic`, an `acyclic` flag on the allocation record — and it moved nothing: **139 ns → 140 ns**, inside noise. Reverted rather than kept. **Why it cannot help:** `mark_suspect` fires only when a refcount drops to a *non-zero* value (`old != 1`). The common create-and-drop path goes straight to zero and never touches the suspect registry at all, and `string`/`bytes` were already excluded by the "has outgoing edges" test. The 34 ns gap between `User{name:string}` and `Counter{v:int}` is the **string allocation plus `register_edge`**, not suspect marking — and edges must stay, because freeing an object has to release what it owns. Only a workload that shares objects and releases them to a non-zero count would benefit, and none was found. Do not rebuild this without first showing such a workload |
| **LANG-PERF-4** | Small String Optimization | 4 | L | **blocked — do not attempt as specified** | An Ori `string` value **is** a NUL-terminated `const char*`: 50 runtime functions take it as `*const u8` and hand it to `CStr::from_ptr`, and spec 19 §8.3b makes that representation a normative part of `@c_export`. Storing short strings inline needs pointer tagging, which breaks every one of those call sites and the C boundary, and would require an **ABI bump**. Use **LANG-PERF-4b** instead |
| **LANG-PERF-5** | Zero-copy slices | 2 | M | **done** | **2026-07-20**: added `slice[T]`, a read-only window, as a **distinct type** rather than changing `xs[1..2]`. `lists.window(xs, a, b)` is O(1); `lists.slice` still copies. 100k-element list: **2.4 ms → 12 µs (197×)**. It stores the *list object*, not its buffer, because `push` can move the buffer; an ARC edge keeps the owner alive. Read-only on purpose — writing through a window would make aliasing mutable. Both `slice` and `array` were made **contextual** keywords after reserving `slice` broke an existing test with `const slice: list[string]` |
| **LANG-PERF-5b** | Windows over `string` / `bytes` | 3 | L | **blocked — measured 2026-07-20** | The same idea for text is blocked by representation, not effort: a `string` finds its length by scanning to the **NUL**, and `bytes` by looking the exact pointer up in the allocation registry. A window into the middle has neither. Fixing it means a (pointer, length) pair, which breaks `const char*` compatibility — normative for `@c_export` (spec 19 §8.3b) — and the 50 runtime functions taking `*const u8`. **Same blocker that stopped SSO.** A `slice[u8]` over the bytes is the available answer |
| **LANG-PERF-6** | Scoped arenas | 3 | L | **shelved by language decision 2026-07-23** | A general arena would only remove ARC overhead if arena-owned values bypassed per-object registration and release. That requires escape/lifetime rules for returns, fields, async tasks, FFI, managed children, and custom destructors; an arena that merely batches ordinary ARC values would keep nearly all of the cost. Reopen only after a real short-lived-object workload proves a bottleneck and a runtime prototype shows a material win (target: at least 20%). Prefer a safe domain container such as `Pool[T]` with stable IDs before adding region semantics to the language. |
| **LANG-FFI-1** | `@c_export` aggregate ownership | 0 | M | **done** | Scalar/out wrappers and opaque handles exist. Direct/optional/result string and bytes ingress is copied; managed aggregate parameters are opaque handles and wrappers now reject null, foreign, wrong-size, or same-size wrong-type pointers before user code when a concrete non-generic payload is compiler-tagged. Generated code has no provenance-only fallback and generic aggregate exports remain rejected. The Linux C-host regression runs under ASan/UBSan when available; direct collections remain private behind handles. |
| **LANG-FFI-2** | Generated `.h` header | 3 | M | **done** | **2026-07-23:** every successful `ori compile --lib` writes the sibling `.h` from HIR. It declares runtime lifecycle/release functions, scalar typedefs, scalar-struct pointer/out signatures, C++ guards, and string ownership. Custom export symbols are checked as portable C identifiers; both real C-host regressions compile against the generated header. |
| **DX-DBG-1** | Debugger: `variables` + `stackTrace` | 3 | M | **done** | **2026-07-25:** instrumented Ori functions register entry/exit frames; stopped events include `stackTrace`, scalar variables, nested `struct`/`optional`/`result`/enum snapshots, bounded list length/capacity plus recursively indexed elements, map/set and supported opaque-collection views, async frames across suspension/resumption, closure captures, and bounded previews for managed/static/registered foreign strings and bytes. Unknown pointers remain summarized without dereferencing. |
| **DX-DBG-2** | DAP adapter + `ori debug` | 3 | M | **done** | **2026-07-25:** `ori debug <file>` owns compilation/instrumentation, local TCP bridging, breakpoints, continue/step and terminal rendering. `ori debug --dap` exposes `initialize`, `launch`, `setBreakpoints`, `configurationDone`, `continue`, `next`, `threads`, `stackTrace`, `scopes`, `variables`, `evaluate` and `disconnect` over stdio. `evaluate` is a bounded, side-effect-free expression evaluator over the latest stopped snapshot; it never invokes target code. |
| **DX-DBG-3** | Editor debugger integration | 3 | M | **done for VS Code; Zed blocked** | **2026-07-23:** `extensions/vscode-orl` registers the `ori` debug type, launches `ori debug --dap`, provides `Ori: Debug Current File`, and contributes a launch configuration. Zed's current extension API exposes language-server registration but no debugger descriptor; manual `ori debug --dap` remains documented there. |
| **DX-DWARF** | DWARF debug symbols | 4 | L | **done for line metadata + portable variables** | **2026-07-26:** native Linux binaries receive a compact DWARF v4 line table built from final Ori symbols; every target also gets an `*.debug.json` source map containing parameters, locals, pattern bindings and closure captures with source lines. The cooperative DAP publishes live values on Linux, macOS and Windows. Windows linkers are invoked with `/DEBUG`/`-DEBUG` and an explicit `*.pdb` path. Rich CodeView/DWARF local-variable locations remain blocked on a Cranelift writer; native debuggers can use the line symbols while Ori DAP supplies values. Missing platform tooling degrades to the JSON map with a warning. |
| **COMP-SCALE-1** | Large-module compiler scalability | 1 | M | **done** | Function/codegen indexes reduced the 10k synthetic AOT compile from ~4 min to 21.2 s. The closure adds immutable value/type/trait/impl/method indexes built once in `ResolvedModule` and shared by every checked file. Guards cover 10k constants, 1k declaration families, and a large import graph without repeated index rebuilds. |
| **DX-INCR** | Incremental compilation | 3 | L | **done for per-file native objects** | **2026-07-26:** `.ori/incremental.json` fingerprints the source graph, manifests, lockfile, compiler version and options, and reuses a matching native output/header. Rebuilds now emit deterministic content-addressed objects in `.ori/modules/` and link one object per source file; unchanged implementation files are reused while changed files are regenerated. A shared interface fingerprint invalidates consumers when declarations or layouts change. Shared libraries, dynamic global initializers and explicit debug instrumentation conservatively use the monolithic route. Path and materialised registry/Git dependencies participate in the fingerprint; dependency-bearing projects without `ori.lock` rebuild instead of guessing. `ORI_DISABLE_INCREMENTAL=1` disables it. |
| **PKG-LOCK** | Dependency lockfile snapshot | 3 | M | **done** | Lock format v2 records source identity, exact Git revision, and SHA-256 tree digest. Resolution restores the locked graph from verified cache in offline mode, rejects drift/tampering, and requires explicit migration from v1. |
| **PKG-REG** | Package ecosystem protocol | 3 | XL | **done for registry v1** | Registry/publish/get/install/cache/lock now enforce HTTPS by default, authenticated archives, contained extraction, immutable versions, atomic cache publication, and lock-driven reproducibility. Broader ecosystem hosting remains outside this repository; [implementation map](package-ecosystem-production.md). |
| **PKG-NS** | Cross-package namespace isolation | 3 | M | **done** | **2026-07-25:** local import lookup stops at the owning package boundary and dependency modules must use the package-qualified prefix (`package.module`). Two dependencies can therefore expose the same local module name without an ambiguous bare import. |
| **BACK-C-1** | C backend parity | 3 | L | **retired 2026-09-05** | Cranelift AOT/JIT remains the product and semantic reference. C source emission and `ori emit c` were removed under [ADR-0005](../decisions/adr/0005-deprecate-and-retire-c-backend.md). Historical decision **2026-07-23**: parity was shelved and C retained only as a partial synchronous debug route; that route is no longer available. Native C ABI interoperability remains supported. |
| **BUG-MAPSET** | `m["k"] = v` silently did nothing | 1 | S | **done** | **2026-07-20**: the index-assignment codegen chain handled only `list` and fell through with no store and no error, so map index assignment compiled and did nothing. Implemented for `map`, and the fallthrough is now a hard error so the class cannot recur. Regression: `compile_runs_map_index_assignment` |
| **DIAG-DEFID** | `<def DefId(N)>` in backend errors | 3 | S | **done** | **2026-07-23**: both backends now render declared type names from their compact `DefId → name` tables, recursively through containers and function types. The native HIR validator builds the same compact table; backend diagnostics no longer need a full `DefMap`. |

### Code audit, memory safety, and performance roadmap (2026-08-24)

Follows [`roadmap-code-audit-performance-architecture.md`](roadmap-code-audit-performance-architecture.md).

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **AUD-OPT-1** | DCE preserves traps and observable failures | 0 | M | **done** | DCE removes only expressions classified `Pure`; traps, calls/control flow, indexing, allocation-backed values, contracts, destructors, and uses in associated calls or `match` guards are retained. Unit tests cover allocation retention; AOT/JIT differential regressions cover division, remainder, shifts, indexing, failed contracts, and custom destructors at every optimization level. Deterministic allocator-failure injection is a P1 test-infrastructure follow-up, not an open optimizer correctness path. |
| **AUD-OPT-2** | Inlining preserves single, ordered argument evaluation | 0 | M | **done** | Until HIR has explicit temporaries, aggressive inlining accepts only stable scalar literal-derived arguments, used at most once. Calls, variable reads, managed/allocating values, parameter contracts, closures, binding scopes, propagation, and `await` stay behind the call boundary. AOT/JIT regressions cover ignored traps, argument snapshots/order, single evaluation, contracts, destructors, and managed values. Materialized temporaries and binding IDs remain a P1 optimization-capability follow-up. |
| **AUD-ARC-1** | ARC edge multiplicity/slot ownership | 0 | L | **done** | `ArcEdges` preserves every `(owner, child)` occurrence and removes/releases exactly one slot. Runtime tests cover unregister, owner teardown and parallel-edge cycles; native AOT covers duplicate struct fields, while one AOT/JIT matrix covers duplicate list/map/channel slots with zero leaks. Native map lowering no longer registers the same physical entry a second time after the runtime already took ownership. Valgrind is an explicit optional/required gate (`ORI_REQUIRE_RUNTIME_VALGRIND=1`); sanitizer/TSan scheduling remains P2 QA under `AUD-QA-3`, not an open P0 invariant. |
| **AUD-CHANNEL-1** | Managed channel value ownership | 0 | M | **done** | Typed lowering selects distinct scalar and managed send symbols. The scalar path never infers ownership from an `i64`; the managed path validates and registers one queue edge, receive transfers one edge, and destruction releases pending entries. Runtime tests cover pointer-shaped scalar values, invalid managed payloads, duplicate sends and a four-sender close race; native AOT/JIT collection-channel coverage finishes with zero leaks. External memory-checker/TSan execution remains P2 QA. |
| **AUD-NET-1** | Async resource lifetime and close synchronization | 0 | L | **done** | Readiness jobs retain connection/listener/UDP handles; mutex-protected native state serializes I/O with close. Unix readiness polls an owned duplicate descriptor and reprobes every 50 ms, preventing close/reuse of a borrowed fd; pending jobs rotate and cancelled jobs release future/resource keepalives without running work. Close/cancel UDP runtime regressions and TCP/UDP native E2E pass. A lower-latency multi-OS reactor and sanitizer contention jobs remain P2 performance/QA. |
| **AUD-EMBED-1** | Sound public hosted-value API | 0 | L | **done** | Raw managed constructors are gone. Hosted strings/bytes are Rust-owned and copied across the runtime boundary; slice tokens carry private `NonNull` ownership plus session/module/generation identity. Calls reject wrong-session/module, stale, and unload/reload tokens. Unit/leak regressions and a compile-fail doctest pass; Miri is unavailable on the pinned stable toolchain and is not reported as passed. |
| **AUD-FFI-1** | Foreign string ownership at `@c_export` ingress | 0 | M | **done** | Direct/optional/result strings and length-aware bytes are copied on ingress. Managed aggregates cross only as opaque runtime handles, and wrappers reject null, unregistered, wrong-size, or same-size wrong-type pointers before dereference when a concrete layout is compiler-tagged. Generated code has no provenance-only fallback and generic aggregate exports remain rejected. C-host fixture runs under ASan/UBSan when available; unregistered foreign-string escapes are unreachable. |
| **AUD-PARSE-1** | Total token diagnostics | 1 | S | **done** | `TokenKind::Uninhabited` (`-->`) now renders a stable token name. A complete lexer unit matrix covers every producible token variant, and the malformed-source corpus proves parser diagnostics do not panic on it. |
| **AUD-PARSE-2** | Fixed-arity type recovery | 1 | S | **done** | `result`/`map` now return parser recovery after reporting wrong arity instead of indexing short vectors. The security suite iterates zero, one, exact, and extra arguments for single- and multi-argument constructors in bracket and legacy angle forms, asserting bounded diagnostics without panics. |
| **AUD-PARSE-3** | Bounded parser nesting | 1 | M | **done** | Every recursive parser constructor shares the nesting budget. A generated 19-constructor corpus exercises CLI and LSP paths, while 512-deep unary/pattern/type/expression inputs terminate with bounded diagnostics instead of overflowing the process stack. |
| **AUD-CT-1** | Iterative/bounded constant dependency evaluation | 1 | M | **done** | Constant dependencies use iterative DFS with cached terminal values. Regressions accept a 4,096-node chain, diagnose a 512-node cycle, and keep a 10,000-constant guard bounded without recursive stack growth. |
| **AUD-PARSE-4** | Enforce the `check` message contract | 2 | S | **done** | Every non-string message now emits `parse.check_message_literal` and is consumed during recovery; string-literal escapes are decoded through the shared parser path before lowering. `CheckStmt` keeps its typed `Option<SmolStr>` contract, and scalar/dynamic regressions cover the rejection. |
| **AUD-PKG-1** | Contained, integrity-checked package extraction | 1 | L | **done** | Registry fetches require HTTPS unless an explicit local-development opt-in is set, disable redirects, enforce time/byte/decompression/depth/count limits, require a SHA-256 sidecar, and publish verified trees atomically. The Rust tar preflight rejects traversal, absolute/non-UTF-8/backslash paths, duplicates/case collisions, truncation, symlinks, hardlinks, devices, and FIFOs before `unpack_in`. |
| **AUD-PKG-2** | Lock-driven reproducible dependency resolution | 1 | L | **done** | `ori.lock` v2 records normalized source identity, exact Git revision, and SHA-256 tree digest for the transitive graph. Locked and offline restoration validate cache/source/content, reject tampering and source collisions, and require an explicit unlocked migration for v1 locks. |
| **AUD-LSP-1** | Protocol-correct Unicode positions | 1 | M | **done** | The server negotiates UTF-8/UTF-16/UTF-32 once, defaults to UTF-16, and routes inbound/outbound locations through one checked codec. Unicode, CRLF, emoji, combining-character, surrogate-middle, out-of-range, reversed-edit, incremental-edit, and end-to-end navigation/rename regressions pass. |
| **AUD-LSP-2** | Resolver-identity symbol index | 1 | L | **done** | Definition/navigation uses resolver `FileId`/`DefId`; locals carry lexical binding identity and selective import aliases carry declaration identity. Lexer occurrences exclude comments/strings and role filtering excludes field/named labels. Shadowing, Unicode, duplicate cross-file names, aliases, hover, goto, references, and rename are covered end to end. |
| **AUD-LSP-3** | Generation-safe background validation | 2 | M | **done** | `validate_uri` and debounced `didChange` now capture `(uri, version)` snapshot, run `run_check*` via `spawn_blocking`, and atomically commit the semantic index under the project write lock only when the version is still current. Document edits invalidate cached semantic snapshots before publishing a new one. Unit + e2e LSP tests cover emoji, invalid position, reversed range, version tracking, edit invalidation, atomic stale-index rejection, and slow-first staleness discard; full LSP matrix 31 unit + 13 e2e verde. |
| **AUD-LSP-4** | Canonical S3 completion/hover rendering | 2 | S | **done** | **2026-09-01:** removed pre-S3 keyword suggestions; corrected compact `apply` and statement-only `using` snippets; hover renders generic/optional types with canonical brackets. Unit tests cover the S3 surface and snippets. |
| **AUD-LSP-5** | Semantic, scope-aware editor linting | 2 | M | **done** | **2026-09-02:** LSP uses in-memory AST linter (`run_lint_source`) with 1 MiB editor-buffer budget; lexical scopes track lexical identities, nested shadowing (`lint.shadowed_variable`), and mutations (`lint.prefer_const`); full AST traversal covers destructuring, structured bindings, pattern matches (`match` statement and `MatchExpr` arms), closures, struct updates, `try`, `await`, `is`, tuple indices, and `suspend`, eliminating false unused variable warnings; comprehensive unit tests in `ori-lsp`. |
| **AUD-FRONT-1** | Explicit residual feature matrix | 3 | M | **done** | **2026-09-02:** Unsupported residual syntax forms (cross-module iterators, generic iterators, variadic iterators, method iterators, async iterators, recursive iterators, and generic newtypes) now have dedicated fail-closed diagnostics in the catalog (`type.iter_*`, `parse.async_iter_unsupported`, `parse.newtype_generics_unsupported`) covered by regressions in `ori_spec.rs`. |
| **AUD-FRONT-2** | Remove magic `DefId` sentinels/dummy fallback | 2 | M | **done** | **2026-09-02:** Completely eliminated `DefId::INVALID` from HIR and codegen: `StructLit`, `EnumVariant`, and `StructUpdate` now use typed `Option<DefId>` representations instead of dummy sentinels; anonymous struct literals start as `None` and are refined by expected types; top-level module lowering lookups fail closed directly; native and C backends check and unpack `Option<DefId>` with regressions in `multifile_imports.rs`. |
| **AUD-CHECKER-1** | Immutable checker indexes and clone budget | 2 | L | **done** | `CheckerIndexes` indexes values, types, traits, impls, methods, and fields once in `ResolvedModule`, then every file checker borrows the same immutable snapshot. Synthetic declaration-family and import-graph guards protect construction count and wall-time scale; deeper parallelism remains evidence-gated rather than open work. |
| **AUD-HYGIENE-1** | Remove stale syntax, dead helpers, and misleading comments | 3 | S | **done** | **2026-09-02:** exhaustiveness and `ori explain` guidance use canonical `ok`/`err`; AST/HIR crate docs describe real responsibilities; unused LSP workspace-root state/accessors and inert SIMD/vectorizer scaffolds removed; `ori.concurrent.utils` and `ori.process.utils` forward to canonical parent modules; zero-cost error trace stubs implemented as canonical `ori.err_trace` (`ERR-TRACE-1`); disconnected `ori.window` stub and unused `ori_window_*` runtime stubs completely removed (`GFX-WINDOW-1`), eliminating misleading promises. |
| **AUD-QA-1** | Truthful required code-quality gates | 1 | M | **done** | Required QA stages fail closed with locked dependencies: workspace check, full strict Clippy, tests, and the scoped rustfmt ratchet. Observational full stages report `INCOMPLETE` instead of false success, and shell/workflow masking paths were removed. Release publication remains separately covered by **AUD-REL-1**. |
| **AUD-QA-2** | Behavioral Atlas/example/diagnostic gates | 2 | M | **done** | **2026-09-02:** `validate_atlas.py` validates Atlas schema, unique IDs, statuses, and paths; `examples_smoke.sh` validates all 25 official root and nested examples across check, binary compilation, C-export library, and test modes, expanding default curated L3 execution in `daily_full.sh` to 8 examples (`hello`, `language_features`, `native_showcase`, `collections_demo`, `string_toolkit`, `bytes_usage`, `conditional_config`, `error_handling`); `diagnostic_catalog.rs` validates catalog rows and asserts real shape, severity, and actionable messages across parser (`parse.module_missing`), resolver (`name.undefined`), type checker (`type.type_mismatch`), attribute (`attr.c_export_not_public`), and semantic linter (`lint.unused_variable`) phases. |
| **AUD-QA-3** | Fuzzing, properties, Miri, sanitizers, and coverage trend | 2 | L | **done** | **2026-09-02:** `tools/qa/fuzz_smoke.py` features expanded hostile input generators covering extreme number literals (4,000 digits), pathological identifiers (4,000 chars), unclosed strings/f-strings/comments, embedded null bytes (`\0`), malformed `@cfg` predicates, and 256-level nested expressions/calls/types/patterns with signal, timeout, and panic detection; `tests/security_robustness.rs` adds in-memory hostile input and pathological literal validation ensuring clean termination and bounded diagnostic spans. |
| **AUD-REL-1** | Complete, reproducible release matrix | 1 | L | **done** | Four required desktop targets package and smoke JIT+AOT before publication. Actions/toolchain are pinned, job permissions are least-privilege, the full artifact set is required, and release output includes `SHA256SUMS`, deterministic SPDX SBOM, GitHub build provenance, and byte-identical tar.gz archives. Native string/global data and function-reference/wrapper snapshots use key-stable order; `.ori` caches are pruned before archiving. Remote publication remains a release-time execution, not unfinished implementation. |
| **AUD-RT-INIT-1** | Hosted/C module initialization lifecycle | 1 | M | **done** | **2026-08-31:** hosted JIT executes global init once per generation and pairs it with teardown; C headers export the required order; runtime leases call `ori_rt_init`/`ori_rt_shutdown_ex` safely. |
| **AUD-LINK-1** | Correct executable/shared output for every linker strategy | 1 | L | **done** | **2026-08-31:** shared-link mode is honored across bundled/system/raw strategies; Windows `.lib` no longer passes GNU-only `-Wl` flags; native backend linker matrix passes. |
| **AUD-JIT-ABI-1** | Runtime artifact identity validation | 1 | M | **done** | **2026-08-31:** JIT validates target/version/ABI/name and cdylib SHA-256 before loading, then queries identity symbols; AOT validates staged staticlib SHA-256 when metadata is present and fails closed in packaged mode. |
| **AUD-BYTES-1** | NUL-safe synchronous and async bytes I/O | 1 | M | **done** | **2026-09-01:** managed bytes preserve exact lengths across list/TCP/UDP paths; `str.to_bytes` copies static literals into managed storage; foreign unregistered pointer-only calls now fail closed with host error `1002`; `@c_export` uses explicit `OriBytes { data, len }`. |
| **AUD-C-1** | Escape C-backend user messages safely | 1 | S | **done** | `check` messages now use a constant `"%s\\n"` format and a complete C-literal escape path, including quotes, backslashes, controls, NUL, and percent sequences. Codegen and strict C compile/run regressions cover hostile text. The portable `c_backend_sanitizers` test probes `clang`/`cc`, runs the generated host under ASan+UBSan, reports unsupported environments explicitly, and is required in the Linux native CI matrix with `ORI_REQUIRE_C_SANITIZERS=1`. |
| **AUD-C-2** | Stable C-backend `any` vtable/managed ownership | 1 | M | **done** | **2026-09-01:** vtables have translation-unit lifetime, managed fields register ARC edges, boxing transfers its initial owner without an extra retain, and dynamic dispatch no longer leaks an unmatched retain. Typed C trampolines adapt boxed pointer receivers to concrete by-value methods and pass a field-less trait value to default methods; ASan+UBSan plus C-compile regressions validate both stored-`any` paths. |
| **AUD-SPAWN-1** | Typed thread-creation failure handling | 1 | M | **done** | **2026-08-31:** fallible spawn admission and shutdown now share a lifecycle mutex (`Ready → Stopping`), preventing late workers after shutdown; timer/reactor retries and future/task failure contracts remain covered by runtime regressions. |
| **AUD-CANCEL-2** | Race-free cancel association ownership | 1 | M | **done** | Association rechecks cancellation while holding the token list lock, clears tentative links before cancelling, and bulk cancellation releases the future retain for every removed entry. Runtime regressions cover normal cancel and association-after-cancel paths. |
| **AUD-UNLOAD-1** | Safe runtime shutdown and `dlclose` lifecycle | 1 | L | **done** | **2026-08-31:** runtime leases keep cdylib code alive through managed values, shutdown stops/joins workers with a deadline, restores signal handlers/alt stacks, rejects attached foreign threads, and context destroy drains callbacks/handles before unload. |
| **AUD-RT-1** | Checked allocation sizes and failure contract | 2 | M | **done** | **2026-09-02:** All runtime allocation boundaries (`ori_alloc`, `ori_alloc_typed`, list/set/map/graph/heap backing growth via `grown_capacity`/`capacity_bytes`, `checked_calloc`, `checked_malloc`, `checked_realloc`, `try_zeroed_bytes`, string repeat, padding, and slice bounds) validate sizes and abort cleanly through the runtime bounds contract or return typed errors; full public allocation boundary matrix and failure coverage regression in `ori-runtime/src/tests.rs`. |
| **AUD-RT-2** | Typed invalid-UTF-8 FFI handling & pointer provenance | 2 | S | **done** | **2026-09-02:** Typed UTF-8 ingress validation across NUL-terminated (`cstr_str_result`) and explicit-length (`bounded_cstr_str`) boundaries sets `ORI_HOST_ERROR_INVALID_UTF8` (`1003`); pointer provenance validation (`ori_handle_validate`, `ori_handle_validate_size`, `ori_handle_validate_size_type`, `retain_registered_payload`) verifies registered allocation, size, and nominal type tag before dereferencing, while foreign/stack pointers safely no-op without mutating memory; regression tests in `ori-runtime/src/tests.rs`. |
| **AUD-RT-3** | Measure and reduce ARC registry contention | 2 | L | **done (profiling & contract)** | **2026-09-02:** All runtime ARC operations serialize through poison-safe `lock_arc_state`, and real thread contention is measured via atomic counter `ARC_LOCK_CONTENTION` with regression test `arc_registry_contention_counter_is_observable`; sharding/thread-local allocations were evaluated and rejected per `managed_temporary_churn.orl` (~2% slower than glibc tcache) and cycle-semantics requirements. |
| **AUD-RT-4** | Reactor panic and poison containment | 1 | M | **done** | **2026-08-31:** readiness/blocking workers catch panics and fail only their future, shutdown drains queued jobs, poisoned locks recover, and dead persistent workers are detected/reaped before restart. |
| **AUD-RT-5** | Timer queue complexity | 3 | M | **done** | **2026-09-01:** replaced per-wake full-vector sorting with a min-heap and monotonic stable tie-breaking; periodic compaction now drops terminal futures and releases their timer-owned ARC references. Long-lived cancellation tokens shrink burst capacity after associations are removed. Runtime tests cover ordering, stale-owner extraction, and capacity recovery; `tools/bench/run_timer_heap_churn.sh` provides a reproducible 128-concurrent-sleep workload and completion canary. |
| **AUD-EMBED-2** | Hosted callback panic/poison containment | 1 | M | **done** | **2026-08-31:** callback and dispatcher types use `C-unwind`; trampoline catches/ translates panics, registry locks recover poisoning, unregister waits for active calls, and affinity dispatch is synchronous by contract. |
| **AUD-UNICODE-1** | Unicode case-folding conformance | 2 | M | **done** | **2026-09-02:** Achieved 100% bit-for-bit parity between native Cranelift runtime and C debug backend for Unicode full non-Turkic case folding (tables in `c_casefold.rs` synchronized with `unicode-casefold` in `ori-runtime`); generated comprehensive Unicode conformance test suite (`tests/unicode_case_fold_conformance.json`) covering ASCII, German sharp S (`ß → ss`), umlauts, Latin accents, ligatures (`ﬁ`, `ﬂ`, `ﬃ`, etc.), Greek sigma, Cyrillic, fullwidth Latin, and emoji preservation with regression tests in `ori-runtime` and `multifile_imports.rs`. |
| **AUD-ABI-QA-1** | Operational ABI export/schema validation | 2 | M | **done** | **2026-09-02:** `tools/qa/abi_exports.sh` validates static and cdylib exports against stdlib manifest and native backend declarations, checking all 20 canonical public runtime symbols; `validate_runtime_link.py` consumes declarative schema `runtime-link.schema.json`; runtime layout regressions in `ori-runtime/src/tests.rs` cover `OriHeapHeader`, `OriList`, `OriSet`, `OriMap`, `OriHeap`, `OriDeque`, `OriGraph`, and FFI `OriBytes` bridge. |
| **AUD-TEST-1** | One-build native test dispatcher | 2 | L | **done** | Native `ori test` now emits/links one suite binary and runs each selected test in a fresh process using `ORI_TEST_INDEX`; regressions cover multi-test, filtered, and failing cases. |
| **AUD-TARGET-1** | Reject or implement non-host target codegen | 2 | M | **done** | Native AOT/JIT/test pipelines now fail before incremental reuse or code generation with `native.target_unsupported` when `ORI_TARGET_TRIPLE` differs from the compiler host triple; cfg-only front-end use remains available. |
| **RUST-AUDIT-2** | Full-workspace clean Clippy gate | 1 | S | **done** | Full-workspace strict Clippy (`--workspace --all-targets --all-features -D warnings`) is a required daily/native-route gate. Existing formatting debt is controlled by a documented scoped rustfmt ratchet, and required QA paths no longer suppress failures. |
| **OPT-TYPE-INTERN-1** | Type Arena Interning (`TyId`) | 2 | L | **done (removed)** | Removed the unconsumed `TyInterner`/`TyId` scaffold and public re-export. A future interner must be introduced only with validated handles, a session ownership model, and allocation/RSS/time evidence. |
| **OPT-PAR-TYPECHECK-1** | Parallel Module Type-Checking | 3 | M | **done** | **2026-08-17:** multi-threaded function-body type checking via `rayon` across independent loaded source modules in `check_loaded_sources`. |
| **DX-LINT-EXT-1** | Extended Semantic Linters | 3 | S | **done for `ori lint`** | **2026-08-17:** added `lint.prefer_const`, `lint.shadowed_variable`, and complete AST expression traversal in the CLI linter. The separate textual LSP linter is reopened as **AUD-LSP-5**. |
| **OPT-RC-ELISION-1** | Static Retain/Release Elision in HIR | 2 | L | **done (removed)** | Removed the inert name-based alias pass from the optimization pipeline. Real ARC elision is deferred until HIR carries ownership operations and has a measured benchmark target. |
| **OPT-ACYCLIC-1** | Acyclic Type Inference | 2 | M | **done (removed)** | Removed the unused `Ty::is_acyclic()` helper instead of allowing an unsound named-type shortcut to influence collector behavior; SCC analysis can be reintroduced only with a DefMap-backed proof. |
| **OPT-SSO-1** | Small String Optimization / Fast Paths | 2 | M | **done (fast path)** | **2026-09-02:** Single-allocation construction fast path (`cstring_from_slices`) is implemented and measured; tagged inline-string representation is explicitly rejected by the frozen C ABI contract in `docs/spec/19-abi.md` (`ori-native-abi-1`), which requires `const char *` NUL-terminated pointer layout for zero-cost FFI. |
| **STR-VIEW-1** | Zero-Copy String Views (`ori.string_view`) | 2 | M | **done** | **2026-08-17:** added `stdlib/string_view.orl` providing `StringView`, `from_string`, `from_sub`, `subview`, `starts_with`, `ends_with`, `to_string`. |
| **ASYNC-REACTOR-1** | Native OS Event Reactor Polling & Non-blocking I/O | 2 | L | **done (baseline)** | **2026-09-02:** Unix socket readiness polling via `poll_fd_ready` (`libc::poll` for `POLLIN`/`POLLOUT`) with panic-safe worker containment and cooperative thread yield; non-Unix fallback uses dedicated blocking worker pool; executor wakeups coordinated through `ori_reactor_poll`/`ori_reactor_wake`. |
| **ASYNC-STRUCT-1** | Structured Concurrency & Cancellation Scopes | 1 | M | **done** | `CancelScope` and `TaskScope` implement `core.Disposable` for deterministic cancel-on-scope-exit and child-task joining with `using`. Scopes track child jobs, propagate cancellation, and join all child tasks on block exit, preventing orphaned tasks. E2E tests in `dx_scripting` cover scope cancellation and child task joining. |
| **CONC-THREADS-1** | Safe Concurrency Transfers & Value Isolation | 0 | M | **done** | **2026-09-01:** transferability checks inspect enum payload fields with cycle-safe recursion; direct reads/writes plus same-module and imported named-helper calls that reach top-level mutable `var` values inside `task.spawn` emit `concurrency.global_mutable_capture`. OS resource handles (`File`, `Connection`, `Listener`, `UdpSocket`, `Input`, `Output`) are non-transferable and rejected. Pure named function values and local closures with checker-side summaries are accepted when their captures are transferable; unsafe, non-transferable, or unknown environments are rejected. Receiver methods and `any[Trait]` dispatch are conservatively matched by method name; managed channel ownership is closed separately by **AUD-CHANNEL-1**. |
| **DX-DOCTEST-1** | Doctests (`ori test --doc`) | 3 | M | **done** | **2026-09-02:** extraction is deterministic and propagates recursive read errors; `run_doctests` captures stdout via pipe/dup2 and validates `-- output: <text>` (including multiline output); `-- compile_fail` (optionally matching specific diagnostic code/message) passes on compilation error; `format_source_labelled_diagnostics` renders rich source-labelled diagnostics (severity, code, message, file:line context, why and action hints); top-level imports are automatically extracted to module scope without duplication. Covered by 6 unit tests in `doctest.rs` and 2 E2E tests in `dx_scripting.rs`. |
| **ERR-TRACE-1** | Zero-Cost Error Return Traces | 3 | M | **done** | **2026-09-02:** Exposed `ori.err_trace` in stdlib (`stdlib/err_trace.orl`) with `push(file, line, message)` and `format(message)`, wired C ABI `ori_err_trace_push` (with `line: int`) and `ori_err_trace_format` across Cranelift native backend, C backend, and runtime staticlib, covered with native and C E2E regressions. |
| **CLI-DAEMON-1** | Persistent Compilation & Evaluation Daemon | 3 | M | **done** | **2026-09-02:** stdin requests use typed `serde_json` DTOs and JSON builders; malformed JSON, escaped IDs/parameters, invalid protocol envelopes, exact shutdown detection, and bounded line buffering at 1 MiB request/8 MiB source limits are covered; persistent `DaemonSession` caches check results keyed by source content SHA-256 with bounded capacity (256 entries) and FIFO eviction; supports warm check hits (`"cached": true`), explicit invalidation (`"invalidate"` per-file or full), and operational counters (`"stats"` for `check_hits`, `check_misses`, `invalidations`). Covered by 9 unit tests in `daemon.rs` and E2E daemon smoke in `dx_scripting.rs`. |
| **GFX-WINDOW-1** | Freestanding Window & Canvas Graphics | 2 | M | **done (removed)** | **2026-09-02:** Removed disconnected `stdlib/window.orl` stub and unused `ori_window_*` runtime stubs; freestanding window/canvas is explicitly out of product scope per AGENTS.md (product focus is compiler, core language, DX, docs, and CLI). Direct BMP/PPM image export remains supported via `ori.image`. |
| **PKG-NATIVE-1** | Declarative Native Dependencies & Platform Link Flags | 1 | M | **done** | **2026-09-03:** Declarative manifest sections in `ori.pkg.toml` (`[native.dependencies.<lib>]` with `pkg_config = "..."`, `static = bool`, `framework = "..."`), per-platform library lists (`[native.linux] libraries = ["GL", "X11", "m", "dl"]`, `[native.windows] libraries = ["user32", "opengl32"]`, `[native.macos] frameworks = ["OpenGL", "Cocoa"]`), and explicit `library_dirs` without requiring manual `.a` GNU ld script workarounds (`libsysdeps.a`) or shell staging scripts. Automated `pkg-config` querying translates `--libs` output into `-L`, `-l`, and `-framework` link lines for Unix/macOS or `.lib` and `/LIBPATH` directives for Windows MSVC. Covered by unit tests in `package.rs` and `native_deps.rs` and E2E compilation tests in `multifile_imports.rs`. |
| **LANG-NOALLOC-1** | Static `@noalloc` Verification Attribute | 1 | M | **done** | **2026-09-03:** Compile-time static check for functions annotated with `@noalloc` (game update ticks, audio callbacks, render command generation, physics integration): the checker statically verifies the function body and strictly rejects dynamic heap allocations (collection literals, interpolated strings, string concatenation, closures, async `await`, `using` blocks, dynamic collection iteration), calls to functions not marked `@noalloc`, and known allocating standard library functions (`fmt.*`, `lists.*`, `maps.*`, `sets.*`, `strings.*`, `task.spawn`), emitting diagnostic `perf.allocation_in_noalloc` on violation. Covered by E2E verification in `dx_scripting.rs`. |
| **LANG-ALIGN-1** | Explicit Struct and Field Alignment Control (`@align(N)`) | 2 | S | **done** | **2026-09-03:** General-purpose `@align(N)` attribute (power-of-2: 1, 2, 4, 8, 16, 32, 64) for structs, directly lowered through AST, checker, HIR (`HirStruct.explicit_align`), and Cranelift native codegen (`StructLayout`). Enforces minimum alignment and padding, reflected in `ori.mem.align_of` and `ori.mem.size_of`, and emitted into generated C export headers (`alignas(N)` / `__attribute__((aligned(N)))`) for GPU uniform/storage buffers (std140/std430) and foreign engine boundaries (GDExtension). Covered by unit and E2E verification in `dx_scripting.rs` and `multifile_imports.rs`. |
| **MEM-REGION-1** | Scoped Memory Arenas / Regions (`mem.region`) | 2 | L | **done** | **2026-09-03:** Scoped bump arena in `ori.mem` (`using region = mem.region()`) for frame-temporary allocations (culling lists, command queues, geometry buffers) with O(1) bulk reset (`mem.reset`), deterministic disposal (`core.Disposable`), and static escape analysis (prohibits `return` of using bindings via `using.escape` and task boundary crossing via non-`Transferable`). Backed by native `OriRegion` with chunked memory. Covered by runtime unit tests and E2E verification in `dx_scripting.rs`. |
| **LANG-SIMD-1** | Portable Fixed-Width SIMD Vector Primitives | 3 | L | **done** | **2026-09-03:** Portable typed vector types (`simd[float32, 4]`, `simd[int32, lanes: 4]`) lowered directly to Cranelift SIMD IR instructions (x86_64 SSE/AVX `F32x4`/`I32x4` and aarch64 NEON) with arithmetic operators (`+`, `-`, `*`, `/`), lane-indexed extraction (`v[i]`), and constant-time vector construction from literals. Verified in both AOT and JIT paths. Covered by E2E verification in `dx_scripting.rs`. |
| **PERF-REGION-1** | Zero-Call Inline Arena Reset & Chunk Indexing | 1 | S | **done** | **2026-09-04:** Direct Cranelift lowering of `mem.reset` (bump arena pointer-bump reset emitted as scalar stores, no FFI call overhead) and arena-local bulk slot allocation. Covered by polyglot `arena_bulk_alloc` comparison. |
| **PERF-INLINE-1** | Pure Small-Function & Value-Struct Leaf Inlining | 1 | M | **done** | **2026-09-04:** Broadened conservative leaf inlining for pure small functions (≤ 8 HIR stmts) with scalar and non-managed value struct arguments (e.g. aligned AABB intersection), converting guard clauses into nested `IfExpr` blocks without call frames or stack spills. Covered by polyglot `spatial_grid_bvh` comparison. |
| **PERF-CHANNEL-1** | Lock-Free Bounded SPSC Ring Buffer for Channels | 2 | M | **done** | **2026-09-04:** Optimized bounded channel send/recv fast paths with static result singletons (`RESULT_OK_ZERO`) to eliminate 100k heap allocations and ARC mutex acquisitions in high-throughput message loops. Covered by polyglot `channel_throughput` comparison. |
| **SYNTAX-APPLY-COLON-1** | Implementação de Traits via `apply Type: TraitA, TraitB` | 1 | M | **done** | **2026-09-04:** Suporte canônico a `apply Type: TraitA, TraitB ... end` com parsing de traits múltiplas separadas por vírgula em bloco único sem necessidade de sub-blocos `use Trait`. Compatibilidade retroativa mantida com `apply Type use Trait`. Testado em `method_resolution.rs`. |
| **SYNTAX-IMPORT-AS-1** | Sintaxe Natural `import path as alias` | 1 | S | **done** | **2026-09-04:** Restauração oficial da sintaxe canônica `import path as alias` e renomeação de itens seletivos `import path (item as alias)` sem colisão no parser. Testado em `multifile_imports.rs` e `ori_spec.rs`. |
| **SYNTAX-COMPOSITE-POS-1** | Remoção de Labels Obrigatórios em `array[T, N]` e `simd[T, N]` | 2 | S | **done** | **2026-09-04:** Suporte a argumentos posicionais compactos em tipos compostos primitivos inline (`array[int, 4]` e `simd[float32, 4]`), mantendo compatibilidade com as formas nomeadas com label (`size: N`, `lanes: N`). Testado em `ori_spec.rs`. |
| **SYNTAX-STRUCT-INHERENT-1** | Centralização de Métodos Inerentes no Bloco `struct` | 2 | S | **done** | **2026-09-04:** Métodos próprios e inerentes definidos diretamente dentro da declaração da struct (`struct Type ... method(self) ... end`), reservando o bloco `apply` exclusivamente para conformidade com traits. Testado em `method_resolution.rs`. |
| **SYNTAX-POLY-TRAIT-NAME-1** | Polimorfismo Direto pelo Nome da Trait em Parâmetros | 2 | M | **done** | **2026-09-04:** Parâmetros anotados diretamente com o nome de uma trait (`fn greet(who: Named) -> string`) são automaticamente abaixados para o tipo de trait object dinâmico (`Ty::Any(trait_id)`), eliminando o ruído de `any[Trait]`. Suporte a substituição correta de `Self` preservado via `substitute_trait_self`. Testado em `method_resolution.rs`. |

**Rejected by decision — do not reopen without a new ADR:**

| ID | Item | Why |
|----|------|-----|
| **LANG-TY-HKT** | Higher-kinded types | A parameter standing for `list`/`optional` itself. Declaration parses but no implementation can satisfy it. Rejected 2026-07-20: three simultaneous abstractions in one signature is the opposite of reading-first; Rust, Go, and Zig all decline it; needs higher-order unification. Use concrete traits or `alias` in a `use` section |
| **LANG-MEM-6** | COW collections | Would flip observable aliasing of collection mutators (FREEZE-1 breaker) with no measured perf pressure. ADR: [`adr-arc-cow-collections.md`](adr-arc-cow-collections.md) |

### Closed earlier

| ID | Item | P | D | Status | Notes |
|----|------|---|---|--------|-------|
| **LANG-PERF-2** | Runtime/mid-end performance (loops, not just compile/link) | 1 | L | **done** | The approved waves 0–6, list scalar inline path, measurement harness, and Cranelift AOT/JIT optimizer contract are complete. DCE/inlining correctness is protected by conservative semantics and differential gates; broader inlining is a future measured capability, not unfinished P1 scope. |
| **LANG-PERF-2-0** | Instrument: CLIF dump + polyglot smoke | 1 | S | **done** | `ORI_DUMP_CLIF`; `tools/qa/perf_polyglot_smoke.sh` |
| **LANG-PERF-2-1** | Mid-end: const fold + DCE | 1 | M | **done** | DCE removes only `Pure` expressions; **AUD-OPT-1** differential gates preserve traps, contracts, allocations, and destructors. |
| **LANG-PERF-2-2** | Loop hygiene (no per-iter cycle collect) | 1 | L | **done** | Native: collect only outside loops at root cleanup |
| **LANG-PERF-2-3** | Pure-loop strength reduction | 2 | M | **done** | Default mid-end; sum/nested closed form |
| **LANG-PERF-2-4** | Monomorphic leaf inlining | 2 | M | **done for conservative pass** | `ORI_OPT=aggressive` only; **AUD-OPT-2** restricts substitution to stable scalar literal-derived arguments. Explicit temporaries may broaden safe coverage later. |
| **LANG-PERF-2-5** | List reserve path (optional) | 3 | S | **done** | `with_capacity` / `capacity` / `reserve`; list_sum uses pre-size |
| **LANG-PERF-2-6** | Docs/README polyglot snapshot refresh | 2 | S | **done** | README + performance guides + LATEST (2026-07-14) |
| **LIVE-LINK** | Package smoke uses **SystemLinker only** (not RustcDriver) | 2 | S | **done** | RustcDriver double-links libstd vs `ori-runtime` staticlib (`rust_eh_personality`). |
| **LIVE-QA** | Daily QA stages + test matrix | 2 | M | **done** | `tools/qa/*`, Spec 13 quality section |
| **LIVE-RES** | Residual product surface clean under FREEZE-1 | 1 | S | **done** | Policy + `residual_audit.sh`; intentional residuals remain Spec 14 |
| **LANG-PERF-3** | FFI call cost scales with binary size (~1.5ms/call large vs 0.55µs small, ~3000×) | 1 | M | **done** | Registry HashMap + maybe_collect. Lab 2026-07-17: sintético 5µs/iter; **studio_shell ~58fps avg** (antes ~2fps); DIAG-FFI 100k×`app.fps` = **5ms**. Issue: [`issue-ffi-dispatch-large-binary-2026-07-16.md`](historico/issue-ffi-dispatch-large-binary-2026-07-16.md). Residual F3: suspect buffer. |
| **LANG-MEM-0** | ARC header: fix stale layout comment/spec + ABI layout guard test | 1 | S | **done** | **Fixed 2026-07-17** (same commit as LANG-MEM-1): lib.rs header comment rewritten; `ori_heap_header_layout_is_stable` guard in ori-runtime tests; Spec 19 note updated |
| **LANG-MEM-1** | Audit dtor × edges overlap + scenario tests S1–S4 | 1 | M | **done** | **Fixed 2026-07-17**: edges are the single cascade owner; `__dtor_*` hooks removed; uniform "store → edge → release owned temp" rule (also fixes real leaks in nested list literals / `lists.push` / index assign). ADR: [`adr-arc-single-cascade-owner.md`](adr-arc-single-cascade-owner.md) · Note: [`historico/nim-study-2026-07-17-c1.md`](historico/nim-study-2026-07-17-c1.md) · 5 regression tests in `memory_arc.rs` |
| **LANG-MEM-2** | Edge completeness matrix for managed types | 1 | M | **done** | The 2026-08-25 closure adds exact parallel-edge runtime coverage, duplicate struct AOT coverage, and one list/map/channel ownership matrix under both AOT and JIT. `AUD-ARC-1` and `AUD-CHANNEL-1` now close the former gaps with a zero-live-allocation baseline. |
| **LANG-MEM-3** | Cycle collector: suspect buffer + adaptive threshold | 2 | L | **done** | **2026-07-17** (note [`historico/nim-study-2026-07-17-c3.md`](historico/nim-study-2026-07-17-c3.md)): release records Bacon possible-roots (outgoing-edge filter, O(1) via `suspect_idx`); cooperative pass = trial deletion over suspect subgraph only; threshold adapts by efficacy (Nim rule, bounds 64–65536, env pins). Full scan kept for `ori.test.*`/ABI. Residual: real-app remeasure (lab) + `rcSum==edges` shortcut if data justifies |
| **LANG-MEM-4** | RC elision in codegen | 3 | L | **done** | **2026-07-17**: action 1/3 done — return-transfer elision (returning a managed local hands the binding's +1 to the caller; retain+release pair removed, `make_list`-style builders drop to zero RC ops). **Closed 2026-07-18** (note §7): S6/S7 audited with ORI_DUMP_ARC — owned args transfer implicitly (0 caller ops), loop rebind is 1 release/iter (theoretical minimum); borrowed args use the canonical callee-owns contract. Only residual idea (consciously deferred): lent-style params via caller lifetime analysis. Note: [`historico/nim-study-2026-07-17-c4-c7.md`](historico/nim-study-2026-07-17-c4-c7.md) |
| **LANG-MEM-5** | Spec 10: collector safe points + atomic-RC trade-off | 2 | S | **done** | **2026-07-18**: safe points landed with C3; added "Threads and RC atomicity" recorded trade-off (atomic + shared heap vs Nim move-subgraphs; revisit gated on ADR + freeze exit), cancellation cleanup contract, and corrected §Managed Types sharing semantics (collection mutators are in-place, aliasing observable) |
| **LANG-MEM-6** | ADR: COW for collections | 3 | S | **done** | **2026-07-18 — deferred/rejected for 0.3.x**: COW would flip observable aliasing of collection mutators (FREEZE-1 breaker); no perf pressure (~320ns/iter builder loop, ~1.25x vs Rust). Uniqueness sketch (refcount==1) + ABI implications recorded for a possible 0.4+ revisit. ADR: [`adr-arc-cow-collections.md`](adr-arc-cow-collections.md) |
| **LANG-MEM-7** | DX: `ORI_DUMP_ARC` | 3 | S | **done** | **2026-07-17**: per-function ARC op counts + sequence from final CLIF (expandArc analog); zero cost without the env var. Note: [`historico/nim-study-2026-07-17-c4-c7.md`](historico/nim-study-2026-07-17-c4-c7.md) |
| **LANG-MEM-8** | Match/if-some owned scrutinee release + Cranelift var-reuse type guard | 1 | M | **done** | **Fixed 2026-07-18** while verifying external bug reports (note [`historico/bugcheck-native-ori-ide-2026-07-18.md`](historico/bugcheck-native-ori-ide-2026-07-18.md)): fresh owned scrutinees of `match`/`if some` leaked every execution; same-named bindings across nested matches with different native types crashed Cranelift (`declared type of variable...`). Also added `None_` to owned-ref exprs. 4 regression tests in `memory_arc.rs` |
| **LANG-MEM-9** | Runtime `new_result`/`new_optional_*` bypass ARC (raw malloc, ~134 sites) | 1 | L | **done** | `ori.fs.read_text` etc. build result/optional wrappers with `libc::malloc` — invisible to the ARC registry, codegen releases become no-ops (20 leaks per 20 `fs.read_text_or` calls). **Fixed 2026-07-18**: wrappers via `ori_alloc` + ownership edge (`wrapper_owns_payload`); `try`/`?` consumes owned wrappers on both paths and always yields an owned payload. Note: [`historico/lang-mem-9-runtime-wrappers-2026-07-18.md`](historico/lang-mem-9-runtime-wrappers-2026-07-18.md) · 2 regression tests |
| **LANG-OPT-1** | DCE blind to closure captures + field-contract effects (the real "closure capture residual") | 1 | S | **done** | **Fixed 2026-07-18**: captures count as uses; struct literals of contract-carrying types count as effects. Un-broke ALL 9 remaining red tests (closure capture incl. across-await was never a backend gap — DCE deleted the captured binding); 4 stale test expectations updated (showcase Displayable line, real examples catalog, 2 build tests now use their bindings). **Driver suite fully green for the first time** |
| **LANG-FRONT-1** | Bare builtin `len` shadows local variable (`undefined variable ori_len`) | 2 | S | **done** | `const len: int = lists.len(xs)` fails in native codegen — name resolution prefers the prefixless builtin (`stdlib!("len", ...)`) over the local binding. **Fixed 2026-07-18**: single-segment QualifiedIdent now checks local bindings before bare-stdlib resolution in HIR lowering (locals shadow builtins; builtin calls unaffected). Regression: `ori_spec::compile_runs_local_binding_shadows_bare_builtin` |
| **LANG-CLI-1** | `ori update`: toolchain self-update from GitHub Releases | 2 | M | **done** | **Shipped 2026-07-19**: `ori update [--check]` in the driver (`update.rs`). Discovery via `/releases?per_page=1` (releases are prereleases; `/latest` 404s); sha256 from the release manifest's asset `digest` (mismatch aborts); extraction via system `tar` (bsdtar opens the MSVC zip on Windows); staged same-filesystem swap with rollback (`.ori-update-staging`/`.ori-update-backup`). Refuses system-package installs (dpkg desync) and dev builds (no `runtime/` beside the exe). Tests: 5 unit + hermetic E2E against a local HTTP server (`tests/self_update.rs`) |

### Done this focus wave (DX + docs + perf + residual)

| ID | Notes |
|----|-------|
| **LANG-DOC** | User docs EN/PT + root READMEs + examples catalog; living maintenance only after this |
| **LANG-PERF** | Closed — waves 1–3 (compile/link/JIT flags); see `perf-baseline-2026-07-13.md` |
| **LANG-PERF-2** | Performance baseline landed; `AUD-OPT-1/2` semantic reopen closed conservatively |
| **LANG-RES** | Closed — Spec 14 inventory + `compile_runs_lang_res_product_surface_native`; see `historico/lang-res-closure.md` |
| **DX-VSCODE** | v0.3.5 local `.vsix` |
| **DX-ZED** | `extensions/zed-ori` v0.3.5 dev install |

---

## 3. Shelved (after language is complete)

Do **not** pull these into “what’s next” until the user re-opens them:

| ID | Item | Notes |
|----|------|-------|
| DIST-1…4 | Multi-OS packages (Win/macOS), smoke matrix | **CI multi-OS packaging** in `release.yml` + smoke-no-rust Win/mac (2026-07-14); publish on `v*` tags |
| ECO-1 / ECO-2 | External demos / community extras | **Out of scope** for this repository |
| M4 | Self-hosting | **Decision updated 2026-09-27:** experimental branch preserved; Rust is the only product compiler through 1.0. Reassess after that release with bootstrap, stage-built conformance, ABI and maintenance evidence (ADR-0006). Not required for user utility or the 1.0 release. |

### Cancelled this wave

| ID | Notes |
|----|-------|
| **TOOL-MP** | No Marketplace/Open VSX. Local install: `tools/install_vscode_extension.sh` |

---

## 4. Sources

| Doc | Role |
|-----|------|
| This file | Only open-work list |
| `docs/spec/` | Normative language |
| `docs/install.md` + guides + examples | User docs |
| `docs/planning/historico/eco-game-imgui-raylib3d-plan.md` | Historical external-package discussion (not active) |
| `extensions/vscode-orl` | VS Code DX |
| `extensions/zed-ori` | Zed DX |

When an item finishes: set status, update CHANGELOG if user-facing.
