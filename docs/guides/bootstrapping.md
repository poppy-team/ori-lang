# Bootstrapping Ori

> **Audience:** contributors building from source.  
> End users should use [install.md](../install.md) (release package).  
> Portuguese index: [../README.pt-BR.md](../README.pt-BR.md). 

> Audience: Core contributors and package maintainers

This guide details the process of building the Ori compiler from source, bootstrapping its standard library, and packaging the standalone distribution that does not require Rust.

Because Ori is not currently *self-hosted* (the compiler is written in Rust), "bootstrapping" in the context of Ori refers to the process of using the host system's Rust toolchain to build the `ori-driver` binary, compile the `ori-runtime` static and dynamic libraries, and bundle the `.orl` standard library.

An experimental compiler written in Ori lives under `selfhost/compiler/`. Its
Stage 1 can compile a scalar subset through `ori-bridge-server`, including
multiple `int` parameters, nested `if`/`else` and `while` blocks, mutable
assignments, scalar `match` arms, and transitive imports whose definitions fit
the same subset.
The scalar path also supports grouped and inline `if` expressions with
integer, Boolean, or string branches, string locals and typed string results.
It cannot yet compile itself: collections, enums, non-scalar pattern matching, string
interpolation, generic signatures, and broader standard library calls are
still outside the bridge. The CI gate in `tools/qa/test_bootstrap_stages.sh` requires Stage 1 to
produce Stage 2, then Stage 2 to produce Stage 3 and run a shared program;
until that gate passes, do not distribute Stage 1 as a self-hosted compiler.

## 1. Prerequisites (Host Environment)

To build Ori from source, you need:
- Rust toolchain (minimum 1.95.0, managed via `rustup` in `rust-toolchain.toml`).
- C Compiler toolchain (for the runtime C ABI and CRT linking):
  - Windows: Visual Studio Build Tools (MSVC `link.exe`).
  - Linux: `build-essential` (`gcc`, `ld`, `ar`).
  - macOS: Xcode Command Line Tools (`xcrun`, `ld`).

## 2. The Build Pipeline

### Step 2.1: Building the Runtime Library

The runtime library (`libori_runtime`) is written in Rust but exposes a C ABI. It is statically linked into AOT Ori programs and dynamically loaded for JIT execution.

```bash
# From repository root (workspace is under compiler/)
cd compiler
cargo build -p ori-runtime --lib --release
# Prefer the stage scripts from repo root:
#   sh tools/stage_native_runtime.sh
#   (Windows) .\tools\stage_native_runtime.ps1
```

### Step 2.2: Building the Compiler

```bash
cd compiler && cargo build -p ori-driver --release
# Binary: compiler/target/release/ori (or ori.exe)
```

## 3. The Standard Library (`.orl`)

The Ori standard library (Layer 2 and 3) is written in Ori (`.orl` files in the `stdlib/` directory).
The compiler intrinsically knows how to resolve `import ori.math`, looking first in the distribution's `stdlib/` directory.

During local development, `ori-driver` detects if it is running from the `target/` directory of the git repository and automatically resolves `stdlib/` relative to the repository root.

## 4. Creating a Standalone Release Package

To create an environment where the end user does *not* need Rust, we bundle the `ori` executable, the pre-built `runtime/` libraries, and the `.orl` standard library into a single archive.

Use the provided smoke and packaging scripts:
- **Windows:** `.\tools\package_native_release.ps1 -PackageRoot "target\dist\ori-windows" -ArchivePath "dist.zip"`
- **Unix:** `sh tools/package_native_release.sh --package-root "target/dist/ori-linux" --archive "dist.tar.gz"`

### Inside the Package
```
ori-linux-gnu/
├── ori                 # The compiler executable
├── stdlib/             # The .orl standard library sources
└── runtime/
    └── x86_64-unknown-linux-gnu/
        ├── libori_runtime.a    # For AOT compilation
        └── libori_runtime.so   # For JIT execution
```

## 5. End-User Installation

When an end-user downloads the package, they can immediately run `ori run file.orl` (which uses JIT via Cranelift and the dynamically loaded runtime cdylib without invoking any external linker). 

To use `ori compile` (AOT compilation), they only need their OS's default linker (e.g., `link.exe` on Windows or `ld` on Linux/macOS). The Rust toolchain is completely removed from the end-user equation.
