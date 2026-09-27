<p align="center">
  <img src="branding/ori-logo-w_text.svg" alt="Ori" width="280">
</p>

# Ori

Ori is a reading-first, explicitly typed programming language compiled to native
code (**AOT**), with optional JIT for `ori run`. Its compiler is written in Rust.

**Surface S3/0.4 (`0.3.0` + ergonomics):** Auk9-inspired readable syntax on the Ori feature engine.
See [manifesto](docs/spec/00-manifesto.md) — Ori exists for **compiler study**,
**AI-assisted programming**, and **ND-friendly readability**, **not** market
competition. The Auk9 lab is **retired as a product**; the living surface is Ori.

Ori is pre-1.0. Syntax before S3 is rejected; further change is still allowed
before a stable 1.0 contract.

**Languages:** English (primary) | [Portuguese](README.pt-BR.md) | [Japanese](README.ja.md)

**Documentation:** [Docs index](docs/README.md) · [Install](docs/install.md) ·
[Language tour](docs/language/tour.md) · [Guides](docs/guides/README.md) ·
[Performance](docs/guides/performance.md) ·
[Specification](docs/spec/README.md) · [Planning](docs/planning/README.md)

**Also:** [Manifesto](docs/spec/00-manifesto.md) · [Stdlib](stdlib/README.md) ·
[Runtime](runtime/README.md) · [Examples](examples/) · [Changelog](CHANGELOG.md) ·
[Contributing](CONTRIBUTING.md)

## Contents

- [What Ori is](#what-ori-is)
- [Why Ori exists](#why-ori-exists)
- [Current status](#current-status)
- [Performance snapshot](#performance-snapshot)
- [Quick start](#quick-start)
- [A first program](#a-first-program)
- [CLI overview](#cli-overview)
- [Project docs](#project-docs)
- [Language overview](#language-overview)
- [Compiler architecture](#compiler-architecture)
- [Standard library](#standard-library)
- [Editor tooling](#editor-tooling)
- [Repository layout](#repository-layout)
- [Development workflow](#development-workflow)
- [Release layout](#release-layout)
- [Known limitations](#known-limitations)
- [Roadmap](#roadmap)
- [License](#license)

## What Ori is

Ori is a statically typed language with explicit modules (`module`), explicit
types (`optional[T]`, `result[T, E]`), structured errors (`try`), traits via
`apply`/`use`, deterministic cleanup (`using`), and native code generation.

The current compiler pipeline is:

```text
.orl source
  -> lexer
  -> parser
  -> name resolver
  -> type checker
  -> HIR
  -> Cranelift native backend
  -> runtime-linked binary or JIT execution
```

The repository contains the compiler, runtime, standard library sources,
language specification, local editor extensions (VS Code + Zed), examples, and
release tooling.

## Why Ori exists

Ori optimizes for reading before writing.

Code should make important information visible at the point where the reader
needs it:

| Question | Ori makes it visible through |
|---|---|
| Where does this file belong? | `module path` at the top of every file |
| What type does this value have? | explicit type annotations |
| Can this value be absent? | `optional[T]` |
| Can this operation fail? | `result[T, E]` |
| When is a resource released? | `using` |
| Where does behavior come from? | `trait` + `apply Type` / `use Trait` |
| What went wrong? | structured diagnostic codes |

This design is especially important for readers who need lower cognitive load:
shorter inference chains, fewer hidden rules, and clearer error messages.

## Current status

| Area | Status |
|---|---|
| Version | **S3/0.4 surface** · inference B **`0.3.1`** · workspace **`0.3.8-dev`** (latest release `v0.3.8`) |
| Stability | pre-1.0; S3 hard-breaks pre-0.3 syntax; further change still possible |
| Compiler | Rust workspace under `compiler/` (lexer → parser → HIR → types → Cranelift + runtime) |
| Native backend | Cranelift AOT + packaged `ori-runtime`; ABI tag `ori-native-abi-1` |
| `ori run` | JIT by default when a runtime cdylib is available |
| `ori compile` / `ori test` | AOT; bundled `rust-lld` first, then the OS linker |
| Standard library | Layer 1 Rust FFI + Layer 2/3 `.orl`; canonical API `ori.X` |
| Tooling | CLI, formatter, docs export, LSP; **local** VS Code + Zed extensions (no store publish) |
| Docs | English primary + Portuguese parallel (`docs/README.md`) · [examples/](examples/) |
| Focus now | Language completeness, docs/examples accuracy, performance — not multi-OS marketing |

> **Development release status:** the 2026-08-24 implementation audit found
> P0/P1 semantic, memory-safety, packaging, and lifecycle defects in the
> audited `0.3.8-dev` baseline. The current correction wave has closed bounded
> channels, the shared blocking-I/O pool, fail-closed attributes, and the typed
> null handle constructor. Remaining language blockers include full task
> isolation, recursive collection keys, handle lifetime/affinity, complete HIR
> async ownership proof, and the hostile cross-platform FFI sanitizer matrix.
> Full evidence and order: [implementation audit](docs/planning/roadmap-code-audit-performance-architecture.md).

S3 breaking list: [CHANGELOG.md](CHANGELOG.md) `[0.3.0]`. Inference: `[0.3.1]`.
Package / install without Rust: M1 is complete in the current workspace. Migrate old sources with
`ori migrate-syntax`.

## Performance snapshot

Local polyglot microbench of **Ori AOT** against Python, Rust, C, Go, JavaScript, and Ruby
(September 2026, Linux x86_64, median of **3** runs — expanded 8-workload suite). Full write-up and caveats:
**[docs/guides/performance.md](docs/guides/performance.md)**
([PT](docs/guides/performance.pt-BR.md)).

| Workload | Ori | Python | Rust | C | Go | JS (Node) | Ruby |
|---|---|---|---|---|---|---|---|
| `sum_loop` 10⁷ steps | **0.003 s**\* | 2.558 s | 0.003 s\* | 0.003 s\* | 0.021 s | 0.066 s | 0.351 s |
| `fib_iter` 2·10⁷ steps | **0.037 s** | 8.486 s | 0.021 s | 0.028 s | 0.041 s | 2.009 s | 9.670 s |
| `list_sum` 10⁶ push+sum | **0.009 s** | 0.467 s | 0.008 s | 0.009 s | 0.010 s | 0.097 s | 0.178 s |
| `nested` 2000² steps | **0.002 s**\* | 1.588 s | 0.002 s | 0.002 s | 0.004 s | 0.058 s | 0.239 s |
| `vec4_simd` 5·10⁶ 4D adds | **0.008 s** | 1.878 s | 0.007 s | 0.007 s | 0.010 s | 0.087 s | 1.496 s |
| `arena_bulk_alloc` 10⁵ resets | **0.015 s** | 0.032 s | 0.002 s | 0.001 s | 0.003 s | 0.047 s | 0.089 s |
| `channel_throughput` 10⁵ msgs | **0.114 s** | 0.049 s | 0.006 s | 0.001 s | 0.010 s | 0.060 s | 0.103 s |
| `spatial_grid_bvh` 10⁶ AABB | **0.543 s** | 0.745 s | 0.003 s | 0.001 s | 0.004 s | 0.060 s | 0.512 s |

\* `sum_loop` and `nested` may be strength-reduced to closed forms by the Ori mid-end. Prefer **`fib_iter`**,
**`list_sum`**, and **`vec4_simd`** for compiled execution speed comparisons.

**Reading (pre-1.0):**
- **SIMD vectorization:** Ori lowers `simd[float32, 4]` directly to Cranelift `F32x4` vector registers, completing 5M additions in **8.1 ms** (≈1.1× GCC -O2, ≈1.15× Rust, beating Go and **~230× faster than Python**).
- **Iterative & scalar loops:** Ori completes 20M fib steps in **37 ms**, near C and Rust, and faster than Go and dynamic interpreters.
- **Lists & collections:** 1M integer push and sum completes in **9.3 ms**, matching C and Rust.

Reproduce:

```bash
SAMPLES=3 ./tools/bench/polyglot/run_polyglot_bench.sh
```

## Quick start

### End users (no Rust)

Full guide: [docs/install.md](docs/install.md) · [PT](docs/install.pt-BR.md).

**Windows** (Scoop-style — installs to `%LOCALAPPDATA%\Programs\Ori` + User PATH):

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser   # optional, once
irm https://raw.githubusercontent.com/raillen/ori-lang/main/tools/windows/get.ps1 | iex
ori --version
ori doctor
```

**Linux** — download from [Releases](https://github.com/raillen/ori-lang/releases), or:

```bash
# example with the latest v0.3.8 tarball
tar -xzf ori-v0.3.8-x86_64-unknown-linux-gnu.tar.gz
export PATH="$PWD/ori-v0.3.8-x86_64-unknown-linux-gnu:$PATH"
# or: sudo dpkg -i ori_0.3.8_amd64.deb
ori doctor
```

AOT uses the packaged `rust-lld` when available; otherwise it needs the OS linker
(VS Build Tools / `build-essential` / Xcode CLT).
`ori run` (JIT) only needs the packaged runtime.

### Compiler development

Prerequisites:

- Rust `1.95.0` from `rust-toolchain.toml`
- A platform linker or one of Ori's explicit linker strategies
- PowerShell on Windows for the release smoke scripts
- A C toolchain on Linux/macOS when using system discovery paths

Compiler workspace lives under `compiler/` (see
`docs/planning/repo-and-project-layout.md`):

```bash
cd compiler
cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- check ../examples/hello
cargo run -p ori-driver -- run ../examples/hello
```

On Windows, validate a release-style package with:

```powershell
.\tools\smoke_native_release.ps1
```

Create a validated `.zip` package with:

```powershell
.\tools\package_native_release.ps1
```

On Linux or macOS:

```sh
sh tools/smoke_native_release.sh
```

Create a validated `.tar.gz` package with:

```sh
sh tools/package_native_release.sh
```

## A first program

```ori
module app.hello

import ori.io = io

main()
    io.print("Hello, Ori!")

    const answer: int = 21 * 2
    io.print(f"The answer is {answer}")
end
```

Run it from this repository with:

```bash
cd compiler
cargo run -p ori-driver -- run ../examples/hello/main.orl
# or, with a release package on PATH:
ori run examples/hello/main.orl
```

Ori uses `end`-delimited blocks, newline-separated declarations, explicit
imports, and explicit types for bindings and public contracts.

## CLI overview

The `ori` CLI is implemented by `compiler/crates/ori-driver`.

| Command | Purpose |
|---|---|
| `ori new <path>` | create a new app project skeleton |
| `ori check <file.orl>` | parse, resolve, and type-check a source file |
| `ori run <file.orl>` | compile and run through JIT or AOT, depending on runtime availability and env vars |
| `ori compile <file.orl>` | emit a native executable through the Cranelift backend |
| `ori test <file.orl> [--filter name]` | run functions marked with `@test`; `--filter` selects matching fully-qualified or short test names |
| `ori repl` | run a small interactive JIT-backed REPL |
| `ori fmt <file.orl>` | format source and print the formatted result |
| `ori doc file <file.orl>` | extract documentation comments as Markdown or HTML |
| `ori doc check <path>` | validate inline docs and `.oridoc` sidecar files |
| `ori doc export` | export stdlib symbols, diagnostics, and keywords as JSON |
| `ori doctor` | report stdlib, runtime, linker, target, and JIT health |
| `ori explain <code>` | explain a diagnostic code |
| `ori summary [path]` | print entry file, modules, imports, and diagnostics count |
| `ori debug <file.orl>` | run the cooperative debugger; `--dap` serves DAP over stdio |
| `ori build <path>` | build a file or project through the native backend |
| `ori lex <file.orl>` | print the token stream for compiler debugging |
| `ori parse <file.orl>` | print the AST for compiler debugging |
| `ori install <name> --path <dir>` | validate a local `ori.pkg.toml` package and copy it to the package cache |
| `ori install name[@ver]` | install from `ORI_REGISTRY` into the package cache |
| `ori install github.com/org/repo` | shallow-clone a Git package and install into the cache |
| `ori get [path]` | fetch `git`/`path` dependencies declared in `ori.proj` or `ori.pkg.toml` |
| `ori lock [path]` | resolve dependencies and write an `ori.lock` snapshot (`--locked` detects current-resolution drift; v2 records content digests and restores exact locked sources) |
| `ori publish <path>` | publish to `ORI_REGISTRY` (file tree or HTTP PUT tarball) |
| `ori migrate-syntax <paths…>` | best-effort rewrite of pre-S3 syntax to S3 (`--dry-run`, `-v`) |

Remote registry/package consumption is experimental. `AUD-PKG-1/2` now enforce
digest, extraction containment, and lock-driven source identity; use HTTPS and
review registry provenance. Hermetic HTTP-registry and cross-platform release
stress coverage remain P2 QA work.

Useful environment variables:

| Variable | Purpose |
|---|---|
| `ORI_STDLIB_ROOT` | override the `stdlib/` source root |
| `ORI_RUNTIME_LIB` | override the native runtime static library |
| `ORI_RUNTIME_CDYLIB` | override the runtime cdylib used by JIT |
| `ORI_USE_JIT=1` | force JIT for `ori run` |
| `ORI_USE_AOT=1` | force AOT for `ori run` |
| `ORI_USE_BUNDLED_RUST_LLD=1` | link through bundled `rust-lld` without the `rustc` driver |
| `ORI_USE_SYSTEM_LINKER=1` | link through the platform linker directly |
| `ORI_DISABLE_INCREMENTAL=1` | disable reuse of matching native outputs from `.ori/incremental.json` |
| `ORI_OBJCOPY` | choose the `objcopy`/`llvm-objcopy` executable used for Linux DWARF sections |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | reject workspace runtime fallback during package validation |
| `ORI_PACKAGE_CACHE` | override the local package cache used by `ori install` / `ori get` / dep resolve |
| `ORI_REGISTRY` | package registry root (directory path or `https://…` base) for publish/fetch |
| `ORI_REGISTRY_TOKEN` | optional Bearer token for HTTP `ori publish` |

The full environment matrix lives in [AGENTS.md](AGENTS.md).

Dependencies are isolated by package. Imports inside a package may use its
local modules; imports from a dependency use the dependency's package-qualified
name, for example `import math.format`, even when another dependency also has a
`format` module. `ori lock` records a path, registry version, or resolved Git
revision snapshot. The current resolver still re-resolves sources before lock
validation; exact lock-driven restoration and content digests are tracked by
`AUD-PKG-2`.

## Project docs

Projects use `ori.proj` at the **project root** (no required `src/`):

```ini
manifest = 1
name = "demo"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[source]
root_namespace = "app"

[docs]
paths = ["docs"]
mode = "sidecar-first"
require_public = "off"
```

Long symbol documentation can live in `.oridoc` sidecars (see
[spec/17-project-and-docs.md](docs/spec/17-project-and-docs.md)).

Shorter workflow: [First project](docs/guides/first-project.md) ·
[Examples](examples/).

## Language overview

Ori's core model is small:

- every file starts with `module path`;
- imports: `import path (A)`, `import path = alias`, or bare `import path`;
- top-level declarations are private unless marked `public`;
- `struct` and `enum` define data; literals use `Type { field: v }`;
- `trait` + `apply Type` / `use Trait` define behavior;
- `optional[T]` models absence; `result[T, E]` models recoverable failure;
- only `try expr` propagates (postfix `?` removed);
- closures use `(u) => expr` (no `do`);
- `using` makes cleanup explicit (e.g. `using r: mem.Region = mem.region()`);
- zero-allocation systems capabilities: static `@noalloc`, explicit `@align(N)`, bump arenas (`mem.region`), and portable fixed-width SIMD vectors (`simd[float32, 4]`);
- declarative native platform linking and `pkg-config` via `[native.dependencies]` in `ori.pkg.toml`;
- diagnostics use stable codes such as `name.undefined` and `perf.allocation_in_noalloc`.

Example with `result`:

```ori
module app.errors

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("division by zero")
    end

    return ok(a / b)
end

main() -> result[void, string]
    const value: int = try divide(84, 2)
    io.print(f"value: {value}")
    return ok()
end
```

For the normative language contract, start with
[docs/spec/01-overview.md](docs/spec/01-overview.md).

## Compiler architecture

The compiler is split into focused crates:

| Crate | Role |
|---|---|
| `ori-lexer` | tokenization |
| `ori-ast` | AST node definitions |
| `ori-parser` | recursive descent parser |
| `ori-hir` | name resolution and lowered high-level IR |
| `ori-types` | type system, stdlib manifest, and checker contracts |
| `ori-codegen` | Cranelift native backend, JIT path, and native FFI headers |
| `ori-runtime` | native runtime library and runtime ABI |
| `ori-diagnostics` | diagnostic codes and rendering support |
| `ori-lsp` | Language Server Protocol implementation |
| `ori-driver` | CLI, pipeline orchestration, integration tests |

The native runtime is the semantic reference for `ori compile`, `ori run`, and
`ori test`. C source emission has been removed; native C FFI and export
headers remain supported.

## Standard library

The stdlib lives under the `ori.*` namespace.

Current shape:

| Layer | Location | Purpose |
|---|---|---|
| Layer 1 | `compiler/crates/ori-types/src/stdlib.rs` and `compiler/crates/ori-runtime/src/lib.rs` | manifest, ABI, hot runtime primitives |
| Layer 2 | `stdlib/**/*.orl` | safe wrappers over runtime primitives |
| Layer 3 | `stdlib/**/*.orl` | pure algorithms written in Ori |

Examples of available areas:

- `ori.io`, `ori.fs`, `ori.path`
- `ori.string`, `ori.bytes`, `ori.convert`
- `ori.list`, `ori.map`, `ori.set`
- `ori.math`, `ori.random`, `ori.time`
- `ori.json`, `ori.net`, `ori.process`
- `ori.task`, `ori.channel`, `ori.concurrent`
- `ori.test` and test helpers

See [stdlib/README.md](stdlib/README.md) for the current module inventory and
[docs/spec/12-stdlib.md](docs/spec/12-stdlib.md) for normative contracts.

## Editor tooling

Ori ships `ori-lsp` plus **local** editor extensions (no Marketplace / store
publish for now — language work comes first):

| Editor | Path | Install |
|--------|------|---------|
| VS Code / Cursor | [extensions/vscode-orl](extensions/vscode-orl/) | local `.vsix` (`npm run package:vsix`) |
| Zed | [extensions/zed-ori](extensions/zed-ori/) | **dev extension** (see extension README) |

LSP features: diagnostics, hover, goto, rename, semantic tokens, symbols, inlays,
type-aware completion, stdlib-aware help, format, incremental sync.

```bash
cd compiler && cargo build -p ori-lsp -p ori-driver
# VS Code:
cd ../extensions/vscode-orl && npm install && npm run compile
# put compiler/target/debug on PATH for ori-lsp
```

## Repository layout

```text
ori-lang/
  compiler/crates/        Rust workspace for compiler, LSP, runtime, driver
  docs/spec/              normative language and implementation contracts
  docs/planning/          roadmap, backlog, and implementation plans
  stdlib/                 Ori standard library source modules
  runtime/                staged native runtime artifacts by target triple
  examples/               example Ori programs (S3 mini-projects)
  tests/                  end-to-end Ori fixtures and test documentation
  extensions/             local editor DX (vscode-orl, zed-ori)
  tools/                  staging, smoke, export, and validation scripts
  tools/bench/polyglot/   Ori / Python / Rust runtime microbench harness
  branding/               project logo assets
  docs/archive/           historical reverse-engineering audit documents
```

## Development workflow

Common gates:

```bash
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-lsp
```

For stdlib changes:

```bash
cargo test -p ori-types --lib stdlib
cargo test -p ori-driver --test multifile_imports
```

For runtime or native backend changes, re-stage the runtime before running
compile/run integration tests:

```powershell
.\tools\stage_native_runtime.ps1
```

Unix:

```sh
./tools/stage_native_runtime.sh
```

Project rules:

- bug fixes need regression tests in `compiler/crates/ori-driver/tests/`;
- new behavior must update docs and `CHANGELOG.md`;
- new diagnostic codes must be registered in
  [docs/spec/13-error-catalog.md](docs/spec/13-error-catalog.md);
- stdlib runtime changes must keep the manifest, lowering, runtime ABI, tests,
  and docs in sync.

## Release layout

A release-style package is expected to keep this shape:

```text
ori.exe                         # or `ori` on Unix
ori-lsp.exe                     # or `ori-lsp` on Unix
stdlib/
  *.orl                         # packaged stdlib source modules
runtime/
  bin/
    rust-lld[.exe]              # optional bundled linker
  {target-triple}/
    ori_runtime.lib             # Windows MSVC static runtime
    libori_runtime.a            # Unix-style static runtime
    ori_runtime.dll             # Windows runtime cdylib for JIT
    libori_runtime.so           # Linux runtime cdylib for JIT
    libori_runtime.dylib        # macOS runtime cdylib for JIT
    runtime-link.json
examples/
README.md
```

The `native-route` workflow covers Windows MSVC, Windows GNU, Linux GNU,
macOS x86_64, and macOS aarch64. Runtime staging details live in
[runtime/README.md](runtime/README.md).

## Known limitations

Current pre-1.0 limitations:

- Ori is not self-hosting. The Ori-written compiler is an experimental
  prototype outside `main`; Rust remains the product compiler through 1.0.
- `ori compile` is AOT and requires the platform linker (Visual Studio Build
  Tools on Windows, `build-essential` on Linux, Xcode Command Line Tools on
  macOS). `ori run` uses JIT by default and needs no linker.
- The compiler itself is written in Rust, so building Ori from source still
  requires Rust. End users who install via a **Linux** release package do not
  need Rust (multi-OS packages are shelved).
- C source emission has been removed; native AOT/JIT are the supported backends.
- Packages support path/git/registry protocols; a public hosted marketplace is
  **not** a current product goal.
- Official extension **stores** (VS Code Marketplace, Zed store) are shelved;
  use local install / dev extension.
- `ori repl` is intentionally small.
- Public contracts can still change before 1.0.

**Single open-work list:** [docs/planning/BACKLOG.md](docs/planning/BACKLOG.md).

## Roadmap

**Now (language-first):** docs honesty, performance, residual language fixes that
block real programs. See BACKLOG.

**Already landed for 1.0 criteria:** stdlib parents (M2), ABI `ori-native-abi-1`
(M3), installer path without Rust toolchain (M1) on the Linux package story.

**Later:** multi-OS packages, store publish, and external demos remain shelved.
Self-hosting is experimental and will be reassessed after the Rust-based 1.0
release, not used as a 1.0 gate ([ADR-0006](docs/decisions/adr/0006-rust-reference-through-1-0.md)).

## License

Ori is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
