# Ori Language Specification — Chapter 10: Memory and Cleanup

> Status: normative
> Audience: compiler implementers
> Surface: **S3** (`0.3.0`)

---

## Overview

Ori manages memory automatically. There are no explicit `malloc`/`free` calls.
Memory safety is guaranteed without requiring the programmer to track ownership
or lifetimes in the source language.

The memory model has two layers:
1. **Value semantics** — the default behavior for all types.
2. **Automatic Reference Counting (ARC)** — for managed types.

---

## Value Semantics

All types in Ori have value semantics by default.

```ori
const a: Point = Point { x: 1, y: 2 }
const b: Point = a          -- b is a copy of a
```

Assigning a struct copies all its fields. There is no aliasing between `a`
and `b`. Mutating `b` (if `var`) does not affect `a`.

This applies to:
- All primitive types
- Structs
- Enums
- Tuples

---

## Managed Types

`string`, `bytes`, and all collection types (`list[T]`, `map[K, V]`, `set[T]`)
are **managed types**. They are heap-allocated and reference-counted.

Assigning a managed type copies the **reference**, not the heap data:

```ori
const a: list[int] = [1, 2, 3]
const b: list[int] = a          -- b holds the same reference as a
```

The heap data is freed when the last reference to it goes out of scope.

Sharing semantics by operation kind:

- **Functional string/bytes operations** (`+` concat, slicing, `str.*`
  helpers) produce fresh values; the inputs are never modified, so
  aliasing is unobservable there.
- **Collection mutators** (`lists.push`, `maps.set`, `sets.add`, index
  assignment, …) modify the shared heap object **in place**: every alias
  observes the change. `const` prevents rebinding the name, not mutation
  of the referenced collection through its API.

Copy-on-write (mutate-in-place only while the reference is unique) was
evaluated and **deferred** — it would change the observable aliasing
behavior above, which FREEZE-1 forbids. Decision record:
`docs/planning/adr-arc-cow-collections.md`.

---

## Automatic Reference Counting (ARC)

The compiler inserts retain/release calls at compile time based on lexical scope.

Rules:
- A reference is retained when it is stored in a binding or passed to a function.
- Reading a managed element from a collection produces a value that remains
  valid after the collection removes that element. The collection's reference
  and the returned value's reference have independent lifetimes.
- A reference is released when the binding goes out of scope.
- In the Rust runtime used by the native backend, retain/release use atomic
  reference counts.
- The native backend links against the Rust `ori-runtime` static library. That
  runtime is the source of truth for managed values, ARC behavior, and runtime
  symbols used by `ori compile` and `ori test`.

**Cycle detection:** the native runtime tracks compiler-registered strong edges
between managed heap objects. `ori_arc_collect_cycles()` reclaims unreachable
cycles from that registered graph using a trial-deletion algorithm. The return
value is the number of heap objects reclaimed in the pass.

**Suspect buffer (possible cycle roots):** `ori_arc_release` records a
payload as a *suspect* when it decrements the refcount to a non-zero value
**and** the object owns outgoing edges (Bacon-style possible roots — only
such objects can have disconnected a cycle). The buffer supports O(1)
dedupe/removal via an index stored in the allocation record; objects freed
through the normal zero path leave no stale entry.

**Cooperative collection points:** generated code uses an amortized gate at
function-level safe points; when the gate fires, the runtime runs a
**partial** trial-deletion pass restricted to the subgraph reachable from
the suspect buffer — cost is O(suspect subgraph), not O(live heap). Edges
from owners outside that subgraph count as external references, so a
restricted pass can only under-collect, never free a reachable object.

- At the end of a sync function body (after scope cleanup, before returning),
  when the function is a top-level scope (`managed_start == 0` and not inside a
  loop body), the backend calls **`ori_arc_maybe_collect_cycles()`**. The
  partial pass runs only when the process-wide managed allocation counter has
  advanced past the cooperative threshold since the last pass.
- After dropping dead frame values following an `await` resume (if any values
  were released): same amortized gate.
- Async executor safe points (`ori_executor_drain`, batches inside
  `ori_task_block_on`): same cooperative counter.
- Explicit **full scan** from Ori via `ori.test.collect_cycles()` (tests /
  diagnostics) or the native `ori_arc_collect_cycles` ABI — these keep
  whole-heap semantics (`assert_no_leaks` relies on it).

**Adaptive threshold:** the cooperative threshold starts at **256**
allocations and adapts by pass efficacy (Nim-ORC-style feedback): a pass
that frees at least half of what it touched shrinks the window (×2/3, min
64); an ineffective pass grows it (×1.5, max 65 536). Setting
`ORI_COOPERATIVE_COLLECT_THRESHOLD` pins the threshold (used by tests).

Full scans are **not** performed on every function return (that residual kept
large-heap interactive programs ~2fps after the LANG-PERF-3 registry fix).
Tight loops still never collect on each iteration (LANG-PERF-2).

### Cascade ownership (single owner: registered edges)

Registered ARC edges are the **only** owner of a stored managed child
(ADR: `docs/planning/adr-arc-single-cascade-owner.md`). The invariant:

> **store → register/update the edge → release the temporary's own +1 if
> the stored expression produced an owned reference.**

- Composite allocations use no synthetic field-releasing destructor hook.
  A struct or enum implementing `core.Destructor` may install a callback for
  user cleanup, but registered edges remain the sole owners of its managed
  fields. When an owner's refcount reaches zero, the runtime runs that callback,
  frees the payload, and releases the registered edges, cascading through
  nested managed children (structs, enums, tuples, optional/result payloads,
  collection elements, closure environments, async frames alike).
- Borrowed references (loads from bindings or fields) keep their existing
  +1 untouched when stored; the edge adds its own +1.
- The `ori_alloc` destructor hook serves runtime-internal cleanup (for example
  a list's internal element storage) and compiler-generated callbacks for
  `core.Destructor`. Neither kind may release compiler-registered children —
  that would reintroduce a double release.

Historical note: before 2026-07-17 the backend also generated
`__dtor_struct_*`/`__dtor_enum_*`/`__dtor_tuple_*` hooks that released the
same fields the edges owned. The double release could free a child shared
with a live binding (use-after-free) and masked missing temporary releases
in element stores. See the ADR and `ori-driver/tests/memory_arc.rs`
(`shared_child_*`, `nested_list_*`, `*_owned_*` regression tests).

### Release is one critical section

`ori_arc_release` performs the decrement, the unregistration and the edge
removal under a **single** ARC lock. The destructor and the recursive release of
owned children run **after** the lock is dropped, because a destructor may
re-enter runtime code and would otherwise deadlock.

This shape is load-bearing for performance, not just tidiness. Before
2026-07-20 the same work took up to **six** mutex acquisitions per released
object (decrement, then unregister, take-edges and remove-incoming inside
`free_registered_object`, plus a redundant `header_for_registered` lookup).
Consolidating them removed **19%** of the wall time from a 2-million-iteration
loop over managed temporaries (360 ns → 290 ns per temporary).

### The ARC maps are keyed by pointers, and hashed as such

`ArcState::allocations` and both edge indexes are keyed by payload address.
They use a multiply-rotate hasher (the `FxHasher` construction) rather than the
standard library default.

That default is SipHash-1-3, which resists collision flooding from untrusted
keys — a property with no meaning here, because the keys are pointers the
runtime allocated itself. It cost more than the rest of a retain or release put
together: switching the hasher took another **33%** off the same loop, and the
project's own `tools/bench/arc_list_churn.orl` runs **39%** faster overall.

### One allocation per string

Runtime string constructors write their parts straight into the final
`ori_alloc` block. They previously built a `Vec`, copied the parts into it,
copied that into a fresh block and freed the `Vec` — two allocations, two
copies and a free for every string produced, paid on every concatenation and
every slice.

Combined with the single-lock release and the pointer hasher above, a managed
temporary went from **355 ns to 171 ns** — **52%** of the ARC overhead removed
across the three changes.

What remains is `malloc` plus one map insert and one removal. Removing those
would mean not registering the allocation at all, which is what makes
`ori_arc_retain` / `ori_arc_release` **safe no-ops on foreign pointers** — the
property `@c_export` string parameters rely on (chapter 19 §8.3b). Any further
gain here has to preserve that, or replace it with a check that never
dereferences a pointer the runtime does not own. See `LANG-MEM-11b` in
[`../planning/BACKLOG.md`](../planning/BACKLOG.md).

Regression tests: `compile_runs_nested_owners_cascade_on_release`,
`compile_runs_managed_temporaries_in_a_hot_loop_without_leaking`.

### Leak check mode

The runtime exposes `ori.test.live_allocations()`, `ori.test.collect_cycles()`
and `ori.test.assert_no_leaks(label)` for test programs:

- `ori.test.live_allocations()` returns the number of live ARC-managed heap
  allocations (does not run the collector).
- `ori.test.collect_cycles()` runs the cycle collector and returns the number
  of objects reclaimed.
- `ori.test.assert_no_leaks(label)` runs the collector, then returns the
  remaining live count. If the `ORI_TEST_LEAK_CHECK=1` environment variable is
  set and the count is non-zero, it prints a diagnostic to stderr and aborts
  with a non-zero exit code so the test fails loudly.

These hooks are available on the native backend. See
`AGENTS.md` for the `ORI_TEST_LEAK_CHECK` env var convention.

### Backend Status

- The native backend inserts ARC retain/release calls for managed values.
- The Rust `ori-runtime` crate provides the runtime symbols consumed by the
  native backend.
- Native AOT/JIT share the Rust runtime; C source emission has been removed.

---

## Async and ARC

Managed values that remain live across an `await` must stay retained until the
async continuation can use them again.

Current native status:

- `await` is accepted only inside an `async` function.
- The runtime has pollable `future[T]` values, continuation registration, a FIFO
  executor queue, failed/cancelled internal states, and non-blocking timers.
- The current backend creates a `future[T]` immediately when an `async` function is
  called, allocates a native async frame, and schedules the generated `step`
  function on the native executor.
- Supported source-level `await` shapes suspend through `ori_future_poll` and
  `ori_future_on_ready`; they do not call `task.block_on`.
- Managed params, pre-await locals and await bindings stored in the frame have
  ARC edges. The state machine calculates liveness after each `await`, releases
  dead managed frame values after resumption, and still runs terminal cleanup.
  Before emission, a frame ownership verifier checks native slot bounds,
  alignment/range overlap, and layout availability; managed await-binding slots
  are zero-initialized so failure/cancellation cleanup never inspects garbage.
  The verifier does not yet prove full HIR data-flow ownership.
- Failed/cancelled future states observed by the state machine are propagated by
  the generated async wrapper.
- `using` inside an `async` function is **allowed**. The async frame stores the
  managed resource and injects `dispose()` on normal return, `try`
  propagation, cancellation, failure, and loop exit. These paths are covered
  by the native async regression suite.

Async shapes outside the current state-machine subset are rejected before
Cranelift with `backend.native_unsupported` instead of falling back to a sync
bridge.

### Threads and RC atomicity (recorded trade-off)

Ori uses a **shared heap with atomic reference counts** plus a global
registry lock for allocation/edge bookkeeping. This is a deliberate
divergence from Nim ORC, which uses *non-atomic* RC and stays sound only
because entire subgraphs are **moved** between threads rather than shared.

- Any managed value may be touched from any thread or task; retain/release
  are always safe. There is no thread-local heap and no ownership-transfer
  requirement in the runtime model.
- Values sent through channels or returned from task joins carry their +1
  through the transfer (the receiving side owns them); the wrappers built
  for those results hold raw `i64` payloads whose type only the generated
  code knows, so the runtime never guesses at managed-ness there.
- The cost of this choice (a registry lookup under the global lock per RC
  op) is the known performance ceiling; RC **elision** (return transfer,
  implicit caller→callee argument transfer) reduces op counts instead of
  weakening atomicity.
- Revisiting non-atomic RC requires an explicit ADR **and** a thread model
  that guarantees subgraph isolation (Nim-style move semantics) — gated on
  a FREEZE exit, since it changes observable sharing semantics.

### Cancellation and cleanup

Cancel tokens mark associated futures as cancelled. The generated state
machine observes the failed/cancelled status on resume, runs terminal
cleanup (releasing the frame's managed edges) and propagates the status
through the async wrapper. `using` disposal runs on every supported terminal
path, including cancellation, failure, propagation, and loop exit; the native
async regression suite covers these cases.

---

## `using` — Deterministic Cleanup

For resources that need explicit cleanup (file handles, network connections,
database connections), use `using`:

```ori
read_file(path: string) -> result[string, string]
    using file: ori.fs.File = try ori.fs.open_read(path)
    const content: string = try ori.fs.read_all(file)
    return ok(content)
end
```

When `file` goes out of scope, `file.dispose()` is called automatically.

### Cleanup Guarantee

`using` cleanup runs on **every** exit path from the scope:

- Normal `end` of block
- `return` statement
- `try` propagation (error path)
- `break` or `continue` in a loop
- Panic

### Cleanup Order (LIFO)

Multiple `using` bindings are disposed in **reverse declaration order**:

```ori
using a: ResourceA = try open_a()
using b: ResourceB = try open_b()
using c: ResourceC = try open_c()
-- When scope exits: c.dispose(), then b.dispose(), then a.dispose()
```

### `Disposable` Trait

A type participates in `using` by implementing `Disposable`:

```ori
trait Disposable
    mut dispose()
end
```

Attempting to use a type in `using` that does not implement `Disposable`
is a compile error.

### `Destructor` Trait

A struct or enum may opt into automatic last-reference cleanup by implementing
`core.Destructor`:

```ori
import ori.core = core

struct NativeHandle
    id: int
    label: string
end

apply NativeHandle use core.Destructor
    mut destroy(self)
        close_external(self.id)
    end
end
```

The required signature is `mut destroy(self) -> void`. The native backend calls
it exactly once when the value's ARC lifetime ends, before the payload or its
registered child fields are released. The callback may read those fields. It
must not make `self` escape, resurrect it, manually release its fields, or
perform asynchronous work.

`Destructor` is automatic but not deterministic in the presence of cycles:
cycle collection may delay the call. When a cycle is reclaimed, every custom
destructor in that cycle runs while all cycle payloads are still allocated;
only then are the payloads freed. Use `using` with `core.Disposable` whenever a
resource must close at a specific lexical scope boundary.

Native AOT and JIT share the same callback semantics.

### Interaction with `try`

```ori
using conn: Connection = try get_connection()
-- If get_connection() returns error: conn is never bound, nothing to dispose.

const data: bytes = try conn.fetch(url)
-- If fetch() returns error: conn.dispose() IS called before error propagates.
```

---

## Stack vs Heap Allocation

The compiler decides allocation strategy. The programmer does not control this.

General rules (subject to optimization):
- Primitive types and small structs: stack-allocated.
- Managed types (`string`, `bytes`, collections): heap-allocated.
- `any[Trait]` values: heap-allocated (boxed).
- Closures that capture values: heap-allocated if they escape the current scope.

---

## No Manual Memory Management

Ori does not expose:
- Pointer arithmetic
- `malloc` / `free` / `realloc`
- Raw pointers (except through `extern c` FFI, where they must be handled carefully)
- Stack allocation directives

`ori.unsafe` provides escape hatches for low-level operations, but its use
is restricted to explicit `unsafe` annotated contexts and is not part of
the ordinary language surface.

---

## FFI Memory

When crossing the `extern c` boundary:
- Ori-managed values may not be passed as raw pointers without explicit conversion.
- C-allocated memory returned to Ori must be wrapped in a type implementing
  `Disposable` that frees it via the appropriate C function.
- The programmer is responsible for memory safety at FFI boundaries.

See the FFI documentation for detailed ABI shapes.

---

## `ori.mem`

`ori.mem` provides memory inspection utilities and scoped bump-arena regions (`MEM-REGION-1`):

```ori
import ori.mem = mem

mem.size_of(value)         -- size in bytes of value's static type
mem.align_of(value)        -- alignment in bytes of value's static type
```

These are compile-time constants. The argument is used as a type witness and
is not needed in ordinary code.

### Scoped Arenas (`mem.region`)

`mem.region()` creates a high-performance scoped bump-arena (`Region`) designed for frame-temporary
allocations (game tick loops, physics updates, visibility lists, and intermediate transformations).
Allocations bump a cursor in O(1) without touching the global ARC lock or registry.

```ori
import ori.mem = mem

main()
    using r: mem.Region = mem.region()
    -- perform frame allocations...
    mem.reset(r)           -- O(1) bulk reset: bumps discarded, chunks preserved for next tick
end                        -- deterministic cleanup via core.Disposable upon block exit
```

**Escape Analysis Guarantees:**
1. A `Region` cannot escape its declaring block via `return` (enforced by `using.escape`).
2. A `Region` cannot cross threads or tasks (it is not `Transferable`, preventing `task.spawn` capture).
3. Reset is instant O(1) without traversing object pointer graphs.
