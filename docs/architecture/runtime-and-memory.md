# Runtime and memory architecture

The Ori runtime provides the native services that generated programs cannot or should not implement directly. It is linked statically for AOT output and loaded dynamically for JIT execution.

## Scope

The runtime owns:

- managed allocation and object headers;
- atomic reference counting;
- ownership-edge registration;
- cooperative cycle collection;
- built-in managed values and collections;
- strings and bytes;
- deterministic and custom cleanup hooks;
- task, future, channel, I/O, filesystem, process, network, and platform primitives;
- exported `ori_*` symbols used by generated code;
- runtime-side test support and debugging hooks.

The normative contracts are:

- [`../spec/10-memory.md`](../spec/10-memory.md);
- [`../spec/16-runtime-ffi-safety.md`](../spec/16-runtime-ffi-safety.md);
- [`../spec/19-abi.md`](../spec/19-abi.md).

## Artifact model

```text
compiler/crates/ori-runtime/src/   runtime source
             |
             v
static library + cdylib
             |
             v
runtime/<target-triple>/           staged artifacts
             |
             +--> AOT linker
             +--> JIT symbol loading
```

The static library and cdylib staged for a target must originate from compatible source and share the same project and ABI versions.

## Managed allocation model

Every managed allocation begins with an ABI-defined header before its payload. Generated code and runtime helpers operate on the payload pointer while runtime allocation logic can recover the header.

The current header includes:

- atomic reference count;
- optional destructor callback;
- ABI-stable layout requirements documented in Spec 19.

The runtime registry tracks live managed payloads and ownership edges needed for collection and validation.

## ARC lifecycle

The ordinary lifecycle is:

```text
allocate -> initial ownership -> retain on additional owner
         -> release when owner leaves -> destroy at zero
         -> release registered child edges -> free allocation
```

Requirements:

- each ownership transfer is explicit in codegen or runtime helpers;
- a retain corresponds to a real additional owner;
- a release corresponds to an owner ending;
- collection wrappers preserve managed keys, values, and elements according to their contracts;
- temporary values are not released before a receiving collection or result wrapper has taken ownership;
- null handling is explicit for each ABI function.

## Single cascade owner

Registered ARC edges are the sole owner of cascaded child release.

A destructor callback may:

- release non-edge resources such as OS handles or Rust-owned buffers;
- run user-defined finalization;
- observe fields where the contract permits it.

It must not independently release a managed child already registered as an ownership edge. Doing both would release the same child twice.

The accepted decision currently lives in the existing ARC ADR and should be migrated into `docs/decisions/adr/` during the decision-history migration.

## Cycle collection

Pure reference counting cannot reclaim unreachable cycles. Ori uses cooperative cycle collection over suspect subgraphs.

High-level model:

1. a release that leaves a non-zero count on an object with outgoing edges may mark it as a suspect;
2. the collector examines reachable ownership relationships within suspect subgraphs;
3. externally reachable objects remain alive;
4. unreachable cyclic groups are finalized and freed safely;
5. registry and reverse-edge indexes are cleaned without leaving dangling relationships.

Collection must preserve:

- single finalization;
- valid destructor ordering;
- no release of already freed payloads;
- no stale suspect indexes;
- no edge entries referencing removed allocations;
- bounded overhead on ordinary retain/release paths.

## Deterministic cleanup

`using` and `Disposable` are the deterministic resource path for resources that must be closed at scope exit.

Custom destructors are object-lifetime hooks and are not a substitute for deterministic cleanup of files, sockets, and similar resources where timing matters.

Async lowering must preserve cleanup on:

- normal return;
- propagated failure;
- task cancellation;
- terminal state-machine failure;
- destruction of live frame values.

## Strings and bytes

Strings and bytes share some allocation infrastructure but have different semantic requirements.

- Strings use valid text-oriented contracts and C-string-compatible runtime paths where documented.
- Bytes are length-aware and may contain embedded NUL values.
- Byte equality, slicing, I/O, and network operations must use length-aware payload views.
- Conversion from bytes to string validates UTF-8 and any current string-representation restrictions.

A runtime helper must not use `CStr` for arbitrary bytes.

## Collections

Runtime-backed collections must define:

- ownership of inserted and returned values;
- mutation and iterator invalidation rules;
- equality and hashing requirements;
- clone/copy behavior;
- cleanup of elements, keys, and values;
- thread-transfer behavior;
- layout exposure or opacity at the ABI boundary.

Managed iterators retain the underlying collection when their contract requires it and release that ownership exactly once.

The runtime's `ori_list_get` returns a borrowed element. A generated Ori
`ori.list.get` call retains a managed result before releasing temporary
arguments, so the caller owns its result independently of the list edge.

## Scoped memory arenas (`OriRegion`)

For performance-critical code requiring rapid temporary allocation (game loop frames, visibility culling, command batches), the runtime provides `OriRegion` bump arenas (`mem.region` in stdlib):

- `OriRegion` maintains chunked blocks (64 KiB) allocated from the system;
- instantaneous O(1) bulk reset via `ori_region_reset` (scalar stores inline in Cranelift without FFI overhead);
- deterministic teardown via `core.Disposable` (`ori_region_free`);
- static escape analysis prevents returning values derived from a region outside its declaring `using` block (`using.escape`);
- regions are non-`Transferable` and cannot be sent across tasks or threads.

## Concurrency and transferability

Values crossing task or channel boundaries must satisfy the language's transferability rules.

The compiler checks structural transferability for supported values. The runtime assumes generated code has respected that contract but still must preserve thread safety for shared internal state.

Global runtime state must document:

- synchronization primitive;
- lock scope;
- reentrancy assumptions;
- callbacks that can run while locked;
- shutdown behavior;
- test isolation strategy.

## FFI boundary pattern

Preferred structure:

```text
raw exported function
  -> validate pointer/tag/length/ownership
  -> convert to typed internal representation
  -> call safe domain logic
  -> convert result to ABI representation
```

`unsafe extern "C"` functions should remain thin. Complex logic should be moved into domain modules that can be tested through safe Rust interfaces.

## Target architecture

The runtime source should be split gradually into domain modules while preserving every exported symbol and ABI layout:

```text
ori-runtime/src/
  lib.rs
  abi/
  arc/
  values/
  collections/
  strings/
  bytes/
  io/
  fs/
  net/
  process/
  async_runtime/
  concurrency/
  debugger/
  test_support/
  ffi/
```

This is a refactoring direction, not permission to perform a big-bang rewrite. Each extraction requires characterization tests and symbol/layout parity.

## Validation requirements

Runtime changes should run, as applicable:

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-runtime
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test memory_arc
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test concurrency_async
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test multifile_imports
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test jit_run
```

Also validate:

- strict Clippy gate;
- AOT and JIT parity;
- exported symbol inventory;
- runtime header layouts;
- static and dynamic staging;
- package smoke tests;
- performance baselines for retain, release, allocation, and collection when touched.

## Failure policy

- Invalid user programs should be rejected by compiler diagnostics when possible.
- ABI misuse and impossible internal corruption may abort rather than continue with undefined state.
- Recoverable OS and library failures should return structured Ori results.
- Panics must not unwind across FFI boundaries.
- Error strings should describe the failed operation and relevant resource without exposing unnecessary internal implementation detail.
