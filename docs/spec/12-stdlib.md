# Ori Language Specification — Chapter 12: Standard Library Contracts

> Status: normative (contracts); informative (usage examples)
> Audience: stdlib implementers, compiler implementers
> Surface: **S3** (`0.3.0`) + stdlib merge policy **M2** (2026-07-13)

---

## Overview

The Ori standard library lives in the `ori.*` module hierarchy.
It is available in every Ori program without explicit installation.

Stdlib modules are imported explicitly:

```ori
import ori.io = io
import ori.fs = fs
import ori.string = str
```

**Canonical public module** for a domain is **`ori.X`** (one name per domain).
Prefer that form in all new code and examples. Nested historical paths
`ori.X.utils` and `ori.X.algorithms` remain accepted as **silent compatibility**
aliases while the on-disk merge completes — they are not the preferred API.
Product decision: `docs/planning/stdlib-merge-policy.md`.

The stdlib is small and layered:
- **Core types** (`optional`, `result`, `list`, etc.) — always available, no import.
- **Foundation modules** — general-purpose utilities under `ori.X`.
- **Domain modules** — specific areas (networking, JSON, etc.) under `ori.X`.

---

## Implementation Architecture (v1.x + Phase 0 `.orl`)

**Layer 1** is a Rust manifest plus a native runtime (hot path). **Layers 2/3**
are `.orl` sources under `stdlib/` loaded by the driver. Layer 1 remains the
ABI contract for primitives; cold ergonomics and algorithms live in `.orl`.

### Single source of truth

`compiler/crates/ori-types/src/stdlib.rs` owns the stdlib contract surface:

- `STDLIB_RUNTIME_FUNCTIONS` — every canonical path, alias, runtime symbol,
  and `native_runtime` flag. Adding a stdlib function means adding one entry here.
- `stdlib_func_sig()` — semantic type signature per canonical path.
- `stdlib_native_abi()` — Cranelift ABI metadata per runtime symbol.
- `is_implemented_stdlib_module()` / `implemented_stdlib_modules()` — the
  importable `ori.*` module set, derived from the manifest plus
  `STDLIB_MODULE_ONLY_PATHS` (a small documented allowlist for modules
  without runtime entries: `ori`, `ori.core`, `ori.Error`,
  `ori.mem` (inline intrinsics), `ori.concurrent` (umbrella)).

Downstream crates must not keep parallel hardcoded lists:

- `ori-hir::lower::stdlib_c_name` is a thin wrapper over
  `stdlib_runtime_symbol`.
- `ori-driver::pipeline::classify_stdlib_import` delegates to
  `is_implemented_stdlib_module`.

### Runtime

`compiler/crates/ori-runtime/src/lib.rs` implements each manifest symbol as
an `extern "C" fn` (e.g. `ori_io_print`, `ori_bytes_len`). The `extern "C"`
ABI is the stable link contract between Cranelift-generated object code and
the pre-compiled `libori_runtime.a`. `ORI_ABI_VERSION` in the runtime marks
the ABI revision.

### Parity guards

The manifest is protected by tests in `ori-types::stdlib::tests`:

- `manifest_paths_and_aliases_are_unique` — no duplicate paths or aliases.
- `manifest_runtime_entries_have_type_and_native_abi_metadata` — every
  manifest entry has both a semantic sig and a native ABI entry.
- `manifest_module_prefixes_are_all_implemented` — every module prefix
  derived from the manifest is accepted by `is_implemented_stdlib_module`.
- `implemented_stdlib_modules_covers_legacy_hardcoded_list` — regression
  guard against the pre-consolidation hardcoded module list.
- `unknown_stdlib_modules_are_rejected` — unknown `ori.*` modules are
  rejected so the driver can emit `bind.stdlib_module_unknown`.
- `spec_fs_and_json_contracts_match_stdlib_sig` — the `ori.fs.File` and
  `ori.json.Value` contracts match `stdlib_func_sig` return types.

### Adding a stdlib function

1. Add an entry to `STDLIB_RUNTIME_FUNCTIONS` (canonical path, aliases,
   runtime symbol, `native_runtime` flag).
2. Add the semantic type signature to `stdlib_func_sig()`.
3. Add the native ABI metadata to `stdlib_native_abi()`.
4. Implement the `extern "C" fn` in `ori-runtime/src/lib.rs`.
5. Add a regression test in `compiler/crates/ori-driver/tests/`.

Steps 1-3 are guarded by the parity tests above; the build fails fast if
they diverge.

### `.orl` modules (Layers 2/3)

Cold compositional APIs live in `stdlib/**/*.orl` and are loaded by the
compiler. Prefer a **single parent file** `stdlib/X.orl` (`module ori.X`) so
helpers land on the same public path as Layer 1 symbols for that domain.

On-disk layout policy (M2):

| Prefer | When |
|--------|------|
| `stdlib/X.orl` | Default |
| `stdlib/X/…` | Heavy algorithms, multi-file math (`vec2`/`mat3`), or oversized parents |

Nested `ori.X.utils` / `ori.X.algorithms` modules may still exist on disk and
remain importable for compatibility. New public APIs should target `ori.X`.

Examples of hybrid parents already in this style: `ori.string`, `ori.list`,
`ori.fs`, `ori.time`, `ori.path`, `ori.io`, `ori.net`, `ori.args`, `ori.config`,
`ori.log`, `ori.validate`.

---

## Current Implementation Status

Every module below is importable today. The list is verified against
`is_implemented_stdlib_module` in `compiler/crates/ori-types/src/stdlib.rs`;
`implemented_stdlib_modules()` returns it programmatically.

| Group | Modules |
|---|---|
| Core / runtime | `ori.core`, `ori.Error`, `ori.mem`, `ori.convert`, `ori.handle` |
| I/O and system | `ori.io`, `ori.fs`, `ori.files` *(compat alias for `ori.fs`)*, `ori.path`, `ori.os`, `ori.process`, `ori.args`, `ori.config`, `ori.log` |
| Text and bytes | `ori.string`, `ori.bytes`, `ori.buffer`, `ori.format`, `ori.json` |
| Collections | `ori.list`, `ori.map`, `ori.set`, `ori.iter` |
| Data structures | `ori.stack`, `ori.queue`, `ori.deque`, `ori.linked_list`, `ori.doubly_linked_list`, `ori.hash_table`, `ori.tree`, `ori.graph`, `ori.heap` |
| Numeric and time | `ori.math`, `ori.random`, `ori.time` |
| Concurrency | `ori.concurrent`, `ori.task`, `ori.channel`, `ori.atomic`, `ori.lazy` |
| Network | `ori.net` |
| Other | `ori.crypto`, `ori.validate`, `ori.test` |

Modules with a `.orl` source live in `stdlib/`; the rest (`ori.core`,
`ori.mem`, `ori.atomic`, `ori.channel`, `ori.task`, `ori.lazy`, `ori.files`)
are Layer 1 and implemented in the runtime with no `.orl` file.

The sections later in this chapter do **not** cover every module. Data
structures, `ori.path`, `ori.process`, `ori.buffer`, and `ori.validate` are
importable but documented only by their `.orl` source and `ori doc`.

Hybrid modules expose native runtime functions and selected `.orl` helpers
under the **same** public module `ori.X`. Prefer:

```ori
import ori.string (is_empty)
import ori.fs (read_text_or)
import ori.string = str   -- alias; call str.is_empty when the parent defines it
```

Compatibility imports such as `import ori.fs.utils = fu` still work; prefer
`ori.fs` in new code.

No stdlib module listed above is intentionally blocked at import time. Importing
a planned future module is a compile-time error with
`bind.stdlib_module_unavailable`. Importing an unknown `ori.*` module is a
compile-time error with `bind.stdlib_module_unknown` — including a submodule
path that is not itself registered, such as `ori.io.sub`.

---

## Core (Always Available)

No import required. These types and functions are built into the language.

### Built-in Types

`bool`, `int`, `int8`–`int64`, `u8`–`u64`, `float`, `float32`–`float64`,
`string`, `bytes`, `void`, `list[T]`, `map[K,V]`, `set[T]`, `optional[T]`,
`result[T,E]`, `range[int]`, `lazy[T]`, `future[T]`, `handle[T]`, `any[Trait]`,
`tuple[...]`

`ori.handle.is_null` is the standard-library null-sentinel probe for borrowed
`handle[T]` values. It returns `bool` and only compares the opaque pointer with
zero; it never dereferences, retains, or releases the pointee. This helper does
not make a handle nullable by construction and does not prove lifetime or
thread affinity. Those guarantees remain part of the host API contract.

Current collection limit:

`Hashable` may remain a marker, or may provide a custom native hash:

```ori
apply UserKey use core.Hashable
    hash(self) -> int
        return self.id
    end
end
```

The method must return a stable `int` and obey the usual invariant: values
that `Equatable.equals` considers equal must return the same hash. The compiler
does not prove that invariant; violating it can make a key unreachable in a
hash table.

- `map` keys currently support `int`, `string`, or a non-recursive user-defined
  type that implements `ori.core.Hashable` and has structural or explicit
  `ori.core.Equatable`.
- `set` elements currently support `int`, `string`, or a non-recursive
  user-defined type that implements `ori.core.Hashable` and has structural or
  explicit `ori.core.Equatable`.
- For user-defined keys/elements, `Equatable.equals` is the native membership
  predicate, so distinct values that compare equal are treated as one key or
  element. A user-defined `hash(self) -> int` method on `Hashable` is used when
  present; marker-only `Hashable` uses generated hashing for non-recursive
  structural values (scalar/string/bytes fields and nested generated
  aggregates). An explicit non-structural `Equatable` without `hash` safely
  falls back to a constant hash. Recursive aggregate-key support and
  benchmarked performance remain open.
- `graph.Graph[T]` uses the same `Equatable.equals` callback for user-defined
  struct and non-recursive enum nodes. Because `graph.new` is type-erased at the
  source boundary, the first concrete node operation attaches the callback;
  later operations keep the established node kind, and `clone`/closure preserve
  the callback.
- The checker rejects unsupported map keys and set elements with
  `type.collection_hash_unsupported`.

`lazy[T]` is available through `lazy.once` and `lazy.force`.

```ori
const delayed: lazy[int] = lazy.once(() => compute())
const value: int = lazy.force(delayed)
```

### Built-in Functions

```ori
len(text: string)            -- int: Unicode scalar-value count
string(value: int)           -- string: convert an integer to text
string(value: float)         -- string: convert a float to text
string(value: bool)          -- string: convert a boolean to text
string(value: string)        -- string: return the same string value
string(value: Displayable)   -- string: call display(self) for user types
int(value)                   -- int: explicit numeric conversion where supported
float(value)                 -- float: explicit numeric conversion where supported
u8(value)                    -- u8: explicit narrowing conversion where supported
```

Collection and byte lengths are exposed through their modules or method-call
syntax, for example `ori.list.len(values)` and `bytes.len(data)`.

For diagnostics with messages, use `ori.string.parse_int` and
`ori.string.parse_float`. The older `ori.convert.string_to_int` and
`ori.convert.string_to_float` helpers return `optional[T]`.

### Built-in Traits (in `ori.core`)

`Displayable`, `Addable`, `Subtractable`, `Multiplicable`, `Divisible`,
`Equatable`, `Comparable`, `Hashable`, `Disposable`, `Iterable`, `Default`,
`Error`, `Cloneable`, `Transferable`

All fourteen names are registered as real `ori.core` traits, but they fall into
two groups that behave very differently.

**Traits with registered method signatures.** Implementing one of these makes
the method callable on the value (`value.display()`):

| Trait | Method | Used by |
|---|---|---|
| `Displayable` | `display(self) -> string` | `string(value)` and f-string interpolation |
| `Error` | `message(self) -> string` | error types (chapter 09) |
| `Cloneable` | `clone(self) -> Self` | explicit copies; `Self` resolves to the implementing type |
| `Default` | `default() -> Self` | an **associated function** (no `self`): implement it without a receiver and call it as `Type.default()` |
| `Disposable` | `mut dispose(self)` | `using` |
| `Equatable`, `Comparable` | comparison methods | operator overloading, `for T:` bounds |
| `Addable`, `Subtractable`, `Multiplicable`, `Divisible` | arithmetic methods | operator overloading |

**Marker traits with no registered methods.** A type may `use` these, but any
method written inside that section is **not** registered as a trait method —
calling it reports `type.no_such_field`:

| Trait | Effect today |
|---|---|
| `Iterable` | Recognized by `for` when the type exposes `mut next() -> optional[T]` — checked structurally, not through the trait table (`type.iterable_next_missing`). **This is the lazy-iteration mechanism**; see chapter 06 |
| `Transferable` | Enforced for values crossing task or channel boundaries (`concurrency.not_transferable`) |

`Hashable` is the exception: it gates `map`/`set`/`hash_table` keys and
registers an optional `hash(self) -> int` method. Marker-only implementations
use generated structural hashes for non-recursive structs/enums; explicit
non-structural `Equatable` without `hash` uses a constant hash. Generic graph
nodes use structural or explicit `Equatable`.

If you need a callable contract that a marker does not provide, declare your
own trait.

---

## `ori.io` — Basic Input/Output

```text
import ori.io = io

io.print(value: string)                              -> void
io.println(value: string)                            -> void
io.eprint(value: string)                             -> void
io.eprintln(value: string)                           -> void
io.read_line()                                       -> optional[string]

-- I/O streams (v2)
stdin()                                                -> io.Input
stdout()                                               -> io.Output
stderr()                                               -> io.Output

read(input: io.Input, max_bytes: int)                -> result[optional[bytes], string]
write(output: io.Output, data: bytes)                -> result[int, string]
flush(output: io.Output)                             -> result[void, string]
close_input(input: io.Input)                         -> void
close_output(output: io.Output)                      -> void
```

`io.read_line` returns `none` at end of input or when the input line is not
valid UTF-8. A successful value is therefore always a valid Ori `string`.

`io.Input` and `io.Output` are opaque runtime-managed handles. They are created
from the three standard streams (`stdin`, `stdout`, `stderr`) and support
byte-oriented read/write with `result` error propagation.

`read` returns `none` inside `ok` on EOF, or `err(msg)` on failure.
`write` returns the number of bytes written on success.
`flush` is a no-op on `stderr` in some backends but is provided for consistency.

`close_input` / `close_output` mark the handle closed; subsequent operations
return errors.

### Layer 2 helpers (on `ori.io`)

```text
import ori.io = io

io.read_text(input: io.Input, max_chars: int)       -> result[optional[string], string]
io.write_text(output: io.Output, text: string)        -> result[int, string]
```

Nested `ori.io.utils` remains a silent compat module; prefer `ori.io`.

---

## `ori.fs` — File System

```text
import ori.fs = fs

fs.read_text(path: string)             -> result[string, string]
fs.read_text_async(path: string)       -> future[result[string, string]]
fs.write_text(path: string, content: string) -> result[string, string]
fs.write_text_async(path: string, content: string) -> future[result[string, string]]
fs.read_bytes(path: string)            -> result[bytes, string]
fs.write_bytes(path: string, content: bytes) -> result[string, string]
fs.read_all(path: string)              -> result[string, string]
fs.append_text(path: string, content: string) -> result[void, string]
fs.exists(path: string)                -> result[bool, string]
fs.delete(path: string)                -> result[void, string]
fs.list_dir(path: string)              -> result[list[string], string]
fs.create_dir(path: string)            -> result[void, string]
fs.create_dir_all(path: string)        -> result[void, string]
fs.is_file(path: string)               -> result[bool, string]
fs.is_dir(path: string)                -> result[bool, string]
fs.copy(from: string, to: string)      -> result[void, string]
fs.rename(from: string, to: string)    -> result[void, string]
```

`ori.fs` is the canonical module name. `ori.files` is accepted as a
compatibility alias for the same functions.

Additional `.orl` helpers are available directly under `ori.fs`:

```text
import ori.fs (read_text_or, remove_file)

read_text_or(path: string, fallback: string) -> string
write_text_result(path: string, content: string) -> result[string, string]
exists_result(path: string) -> result[bool, string]
remove_file(path: string) -> result[void, string]
move_path(from: string, to: string) -> result[void, string]
try_read_text(path: string) -> result[string, FsError]
```

Typed errors for filesystem operations:

```ori
public enum FsError
    NotFound
    PermissionDenied
    AlreadyExists
    InvalidPath
    Other(message: string)
end
```

`try_read_text` maps the host's missing-file message to `FsError.NotFound`,
including the Windows "cannot find the file specified" form.

The async variants complete on the native runtime and return the same
`result[string,string]` shape after `await`. Blocking filesystem operations
share a bounded native I/O pool: the queue holds at most 256 pending jobs and
the runtime creates no more than four workers (clamped to the host's available
parallelism). Submitting while the queue is full waits for capacity, preserving
bounded memory use. Worker panics, shutdown admission, and worker-creation
failures complete the associated future as a terminal failure and never discard
the future ownership.

`read_all(path)` is a text convenience alias for `read_text(path)`.
`read_bytes(path)` returns the exact file bytes, including embedded `0x00`
values. The runtime keeps the payload NUL-terminated for safe interop and tracks
the logical length separately, so the terminator is not part of the value.

All file-handle APIs listed above are implemented in the current
compiler/runtime; see **Dedicated file handle** below for the opaque handle
variant.

### Dedicated file handle (`ori.fs.File`)

Status: **implemented** in the native runtime.

```text
import ori.fs = fs

fs.open_read(path: string)  -> result[fs.File, string]
fs.open_write(path: string) -> result[fs.File, string]
fs.read(file: fs.File, bytes_count: int) -> result[bytes, string]
fs.write(file: fs.File, data: bytes)     -> result[int, string]
fs.close(file: fs.File)                  -> void
```

`File` is an opaque managed type. Use `using` or explicit `close` for cleanup.

---

## `ori.string` — String Operations

```text
import ori.string = string

string.len(s: string)                         -> int
string.concat(a: string, b: string)           -> string
string.split(s: string, sep: string)          -> list[string]
string.contains(s: string, sub: string)       -> bool
string.starts_with(s: string, prefix: string) -> bool
string.ends_with(s: string, suffix: string)   -> bool
string.trim(s: string)                        -> string
string.trim_start(s: string)                  -> string
string.trim_end(s: string)                    -> string
string.to_upper(s: string)                    -> string
string.to_lower(s: string)                    -> string
string.replace(s: string, from: string, to: string) -> string
string.slice(s: string, start: int, end: int) -> string
string.chars(s: string)                       -> list[string]
string.parse_int(s: string)                   -> result[int, string]
string.parse_float(s: string)                 -> result[float, string]
string.to_bytes(s: string)                    -> bytes
string.from_bytes(b: bytes)                   -> result[string, string]
```

String positions use Unicode scalar values, not UTF-8 byte offsets.
the global `len(string)`, `string.len`, `string.slice`, string indexing,
`string.index_of`, `string.chars`, and direct string iteration share that index
space in the native backend.
Extended grapheme clusters can contain multiple scalar values; grapheme
segmentation is not currently a stdlib API.

Additional `.orl` helpers are available directly under `ori.string`:

```text
import ori.string (is_empty, truncate = cut)

is_empty(s: string)                           -> bool
blank(s: string)                              -> bool
replicate(s: string, n: int)                  -> string
replace_all(s: string, needle: string, replacement: string) -> string
join_non_empty(parts: list[string], separator: string) -> string
truncate(s: string, max_len: int)             -> string
```

Invalid input returns `err(message)`. The `ori.convert` parsing helpers are
kept for optional-style parsing where invalid input should become `none`.

## `ori.convert` - Type Conversion

```text
import ori.convert = conv

conv.float_to_string(n: float)        -> string
conv.bool_to_string(b: bool)          -> string
conv.string_to_int(s: string)         -> optional[int]
conv.string_to_float(s: string)       -> optional[float]
```

Compatibility aliases without the `ori.convert` module prefix are accepted
today (`float_to_string`, `bool_to_string`, `string_to_int`,
`string_to_float`), but new code should prefer the explicit module form.

---

## `ori.bytes` — Byte Operations

```text
import ori.bytes = bytes

bytes.len(b: bytes)                          -> int
bytes.concat(a: bytes, b: bytes)             -> bytes
bytes.slice(b: bytes, start: int, end: int)  -> bytes
bytes.to_hex(b: bytes)                       -> string
bytes.from_hex(s: string)                    -> result[bytes, string]
bytes.decode_utf8(b: bytes)                  -> result[string, string]
bytes.get(b: bytes, index: int)              -> u8
```

---

## `ori.list`, `ori.map`, and `ori.set` - Collections

The native runtime stores collection values as runtime handles. `map` keys and
`set` elements support built-in hashable scalar values and non-recursive
user-defined values that satisfy the checker rules for `Hashable` plus
structural or explicit `Equatable`; a user `hash(self) -> int` method supplies
the native hash callback when present, while marker-only non-recursive
structural values use generated hashing. Explicit non-structural equality
without `hash` keeps a constant-hash correctness fallback.

`ori.list` also exposes small `.orl` helpers directly:

```text
import ori.list (singleton, sum_int)

get_or[T](items: list[T], index: int, fallback: T) -> T
first_or[T](items: list[T], fallback: T) -> T
last_or[T](items: list[T], fallback: T) -> T
singleton[T](item: T) -> list[T]
sum_int(items: list[int]) -> int
binary_search_int(items: list[int], target: int) -> int
all_equal_int(items: list[int], expected: int) -> bool
```

```text
import ori.deque = deque
import ori.doubly_linked_list = dll
import ori.graph = graph
import ori.hash_table = hash_table
import ori.heap = heap
import ori.linked_list = ll
import ori.map = maps
import ori.queue = queue
import ori.set = sets
import ori.stack = stack
import ori.tree = tree

deque.new[T]() -> deque.Deque[T]
deque.push_front[T](d: deque.Deque[T], value: T) -> void
deque.push_back[T](d: deque.Deque[T], value: T) -> void
deque.pop_front[T](d: deque.Deque[T]) -> optional[T]
deque.pop_back[T](d: deque.Deque[T]) -> optional[T]
deque.front[T](d: deque.Deque[T]) -> optional[T]
deque.back[T](d: deque.Deque[T]) -> optional[T]
deque.len[T](d: deque.Deque[T]) -> int
deque.is_empty[T](d: deque.Deque[T]) -> bool
deque.clear[T](d: deque.Deque[T]) -> void
deque.clone[T](d: deque.Deque[T]) -> deque.Deque[T]
deque.to_list[T](d: deque.Deque[T]) -> list[T]

queue.new[T]() -> queue.Queue[T]
queue.enqueue[T](q: queue.Queue[T], value: T) -> void
queue.dequeue[T](q: queue.Queue[T]) -> optional[T]
queue.peek[T](q: queue.Queue[T]) -> optional[T]
queue.len[T](q: queue.Queue[T]) -> int
queue.is_empty[T](q: queue.Queue[T]) -> bool
queue.clear[T](q: queue.Queue[T]) -> void
queue.clone[T](q: queue.Queue[T]) -> queue.Queue[T]
queue.to_list[T](q: queue.Queue[T]) -> list[T]

stack.new[T]() -> stack.Stack[T]
stack.push[T](s: stack.Stack[T], value: T) -> void
stack.pop[T](s: stack.Stack[T]) -> optional[T]
stack.peek[T](s: stack.Stack[T]) -> optional[T]
stack.len[T](s: stack.Stack[T]) -> int
stack.is_empty[T](s: stack.Stack[T]) -> bool
stack.clear[T](s: stack.Stack[T]) -> void
stack.clone[T](s: stack.Stack[T]) -> stack.Stack[T]
stack.to_list[T](s: stack.Stack[T]) -> list[T]

ll.new[T]() -> ll.LinkedList[T]
ll.push_front[T](list: ll.LinkedList[T], value: T) -> void
ll.push_back[T](list: ll.LinkedList[T], value: T) -> void
ll.pop_front[T](list: ll.LinkedList[T]) -> optional[T]
ll.front[T](list: ll.LinkedList[T]) -> optional[T]
ll.cursor_front[T](list: ll.LinkedList[T]) -> optional[int]
ll.cursor_back[T](list: ll.LinkedList[T]) -> optional[int]
ll.value_at[T](list: ll.LinkedList[T], cursor: int) -> optional[T]
ll.insert_after[T](list: ll.LinkedList[T], cursor: int, value: T) -> bool
ll.remove_at[T](list: ll.LinkedList[T], cursor: int) -> optional[T]
ll.find[T](list: ll.LinkedList[T], value: T) -> optional[int]
ll.len[T](list: ll.LinkedList[T]) -> int
ll.is_empty[T](list: ll.LinkedList[T]) -> bool
ll.clear[T](list: ll.LinkedList[T]) -> void
ll.clone[T](list: ll.LinkedList[T]) -> ll.LinkedList[T]
ll.to_list[T](list: ll.LinkedList[T]) -> list[T]

dll.new[T]() -> dll.DoublyLinkedList[T]
dll.push_front[T](list: dll.DoublyLinkedList[T], value: T) -> void
dll.push_back[T](list: dll.DoublyLinkedList[T], value: T) -> void
dll.pop_front[T](list: dll.DoublyLinkedList[T]) -> optional[T]
dll.pop_back[T](list: dll.DoublyLinkedList[T]) -> optional[T]
dll.front[T](list: dll.DoublyLinkedList[T]) -> optional[T]
dll.back[T](list: dll.DoublyLinkedList[T]) -> optional[T]
dll.cursor_front[T](list: dll.DoublyLinkedList[T]) -> optional[int]
dll.cursor_back[T](list: dll.DoublyLinkedList[T]) -> optional[int]
dll.value_at[T](list: dll.DoublyLinkedList[T], cursor: int) -> optional[T]
dll.insert_after[T](list: dll.DoublyLinkedList[T], cursor: int, value: T) -> bool
dll.insert_before[T](list: dll.DoublyLinkedList[T], cursor: int, value: T) -> bool
dll.remove_at[T](list: dll.DoublyLinkedList[T], cursor: int) -> optional[T]
dll.find[T](list: dll.DoublyLinkedList[T], value: T) -> optional[int]
dll.len[T](list: dll.DoublyLinkedList[T]) -> int
dll.is_empty[T](list: dll.DoublyLinkedList[T]) -> bool
dll.clear[T](list: dll.DoublyLinkedList[T]) -> void
dll.clone[T](list: dll.DoublyLinkedList[T]) -> dll.DoublyLinkedList[T]
dll.to_list[T](list: dll.DoublyLinkedList[T]) -> list[T]

tree.new[T](root: T) -> tree.Tree[T]
tree.root[T](t: tree.Tree[T]) -> tree.NodeId
tree.value[T](t: tree.Tree[T], node: tree.NodeId) -> T
tree.try_value[T](t: tree.Tree[T], node: tree.NodeId) -> optional[T]
tree.contains_node[T](t: tree.Tree[T], node: tree.NodeId) -> bool
tree.set_value[T](t: tree.Tree[T], node: tree.NodeId, value: T) -> bool
tree.add_child[T](t: tree.Tree[T], parent: tree.NodeId, value: T) -> tree.NodeId
tree.children[T](t: tree.Tree[T], node: tree.NodeId) -> list[tree.NodeId]
tree.parent[T](t: tree.Tree[T], node: tree.NodeId) -> optional[tree.NodeId]
tree.remove_subtree[T](t: tree.Tree[T], node: tree.NodeId) -> void
tree.move_subtree[T](t: tree.Tree[T], node: tree.NodeId, new_parent: tree.NodeId) -> bool
tree.find[T](t: tree.Tree[T], value: T) -> optional[tree.NodeId]
tree.len[T](t: tree.Tree[T]) -> int
tree.depth[T](t: tree.Tree[T], node: tree.NodeId) -> int
tree.pre_order[T](t: tree.Tree[T]) -> list[tree.NodeId]
tree.post_order[T](t: tree.Tree[T]) -> list[tree.NodeId]
tree.breadth_first[T](t: tree.Tree[T]) -> list[tree.NodeId]
tree.clone[T](t: tree.Tree[T]) -> tree.Tree[T]
tree.clone_subtree[T](t: tree.Tree[T], node: tree.NodeId) -> tree.Tree[T]

hash_table.new[K, V]() -> hash_table.HashTable[K, V]
hash_table.with_capacity[K, V](capacity: int) -> hash_table.HashTable[K, V]
hash_table.set[K, V](table: hash_table.HashTable[K, V], key: K, value: V) -> void
hash_table.get[K, V](table: hash_table.HashTable[K, V], key: K) -> optional[V]
hash_table.remove[K, V](table: hash_table.HashTable[K, V], key: K) -> optional[V]
hash_table.contains[K, V](table: hash_table.HashTable[K, V], key: K) -> bool
hash_table.len[K, V](table: hash_table.HashTable[K, V]) -> int
hash_table.is_empty[K, V](table: hash_table.HashTable[K, V]) -> bool
hash_table.capacity[K, V](table: hash_table.HashTable[K, V]) -> int
hash_table.reserve[K, V](table: hash_table.HashTable[K, V], capacity: int) -> void
hash_table.clear[K, V](table: hash_table.HashTable[K, V]) -> void
hash_table.clone[K, V](table: hash_table.HashTable[K, V]) -> hash_table.HashTable[K, V]
hash_table.from_entries[K, V](entries: list[tuple[K, V]]) -> hash_table.HashTable[K, V]
hash_table.keys[K, V](table: hash_table.HashTable[K, V]) -> list[K]
hash_table.values[K, V](table: hash_table.HashTable[K, V]) -> list[V]
hash_table.entries[K, V](table: hash_table.HashTable[K, V]) -> list[tuple[K, V]]

graph.new[N](directed: bool) -> graph.Graph[N]
graph.add_node[N](g: graph.Graph[N], node: N) -> void
graph.remove_node[N](g: graph.Graph[N], node: N) -> void
graph.add_edge[N](g: graph.Graph[N], from: N, to: N) -> void
graph.add_weighted_edge[N](g: graph.Graph[N], from: N, to: N, weight: int) -> void
graph.remove_edge[N](g: graph.Graph[N], from: N, to: N) -> void
graph.has_node[N](g: graph.Graph[N], node: N) -> bool
graph.has_edge[N](g: graph.Graph[N], from: N, to: N) -> bool
graph.edge_weight[N](g: graph.Graph[N], from: N, to: N) -> optional[int]
graph.neighbors[N](g: graph.Graph[N], node: N) -> list[N]
graph.nodes[N](g: graph.Graph[N]) -> list[N]
graph.edges[N](g: graph.Graph[N]) -> list[tuple[N, N]]
graph.bfs[N](g: graph.Graph[N], start: N) -> list[N]
graph.dfs[N](g: graph.Graph[N], start: N) -> list[N]
graph.topological_sort[N](g: graph.Graph[N]) -> list[N]
graph.try_topological_sort[N](g: graph.Graph[N]) -> optional[list[N]]
graph.is_directed[N](g: graph.Graph[N]) -> bool
graph.len[N](g: graph.Graph[N]) -> int
graph.edge_len[N](g: graph.Graph[N]) -> int
graph.has_cycle[N](g: graph.Graph[N]) -> bool
graph.components[N](g: graph.Graph[N]) -> list[list[N]]
graph.strongly_connected_components[N](g: graph.Graph[N]) -> list[list[N]]
graph.transitive_closure[N](g: graph.Graph[N]) -> graph.Graph[N]
graph.shortest_path[N](g: graph.Graph[N], start: N, goal: N) -> optional[list[N]]
graph.shortest_weighted_path[N](g: graph.Graph[N], start: N, goal: N) -> optional[list[N]]
graph.clone[N](g: graph.Graph[N]) -> graph.Graph[N]

heap.new[T]() -> heap.Heap[T]
heap.push[T](h: heap.Heap[T], value: T) -> void
heap.pop[T](h: heap.Heap[T]) -> optional[T]
heap.peek[T](h: heap.Heap[T]) -> optional[T]
heap.len[T](h: heap.Heap[T]) -> int
heap.is_empty[T](h: heap.Heap[T]) -> bool
heap.clear[T](h: heap.Heap[T]) -> void
heap.clone[T](h: heap.Heap[T]) -> heap.Heap[T]
heap.to_list[T](h: heap.Heap[T]) -> list[T]
heap.from_list[T](values: list[T]) -> heap.Heap[T]
heap.merge[T](left: heap.Heap[T], right: heap.Heap[T]) -> heap.Heap[T]
heap.remove[T](h: heap.Heap[T], value: T) -> bool
heap.into_sorted_list[T](h: heap.Heap[T]) -> list[T]

maps.new[K, V]() -> map[K, V]
maps.set[K, V](m: map[K, V], key: K, value: V) -> void
maps.get[K, V](m: map[K, V], key: K) -> V
maps.try_get[K, V](m: map[K, V], key: K) -> optional[V]
maps.contains[K, V](m: map[K, V], key: K) -> bool
maps.remove[K, V](m: map[K, V], key: K) -> void
maps.try_remove[K, V](m: map[K, V], key: K) -> optional[V]
maps.len[K, V](m: map[K, V]) -> int
maps.is_empty[K, V](m: map[K, V]) -> bool
maps.capacity[K, V](m: map[K, V]) -> int
maps.reserve[K, V](m: map[K, V], capacity: int) -> void
maps.clear[K, V](m: map[K, V]) -> void
maps.clone[K, V](m: map[K, V]) -> map[K, V]
maps.from_entries[K, V](entries: list[tuple[K, V]]) -> map[K, V]
maps.keys[K, V](m: map[K, V]) -> list[K]
maps.values[K, V](m: map[K, V]) -> list[V]
maps.entries[K, V](m: map[K, V]) -> list[tuple[K, V]]

sets.new[T]() -> set[T]
sets.add[T](s: set[T], value: T) -> void
sets.contains[T](s: set[T], value: T) -> bool
sets.remove[T](s: set[T], value: T) -> void
sets.try_remove[T](s: set[T], value: T) -> bool
sets.len[T](s: set[T]) -> int
sets.is_empty[T](s: set[T]) -> bool
sets.capacity[T](s: set[T]) -> int
sets.reserve[T](s: set[T], capacity: int) -> void
sets.clear[T](s: set[T]) -> void
sets.clone[T](s: set[T]) -> set[T]
sets.to_list[T](s: set[T]) -> list[T]
sets.from_list[T](values: list[T]) -> set[T]
sets.union[T](a: set[T], b: set[T]) -> set[T]
sets.intersection[T](a: set[T], b: set[T]) -> set[T]
sets.difference[T](a: set[T], b: set[T]) -> set[T]
```

`reserve(collection, capacity)` guarantees at least that many dense slots.
`clear(collection)` removes all entries and keeps the allocated capacity for
reuse.
`maps.get(m, key)` keeps the v1 direct-value contract for compatibility with
existing code. New code should prefer `maps.try_get(m, key)` when absence is
possible. `maps.try_remove(m, key)` returns the removed value as
`optional[V]`.
`deque`, `queue`, `stack`, `linked_list`, and `doubly_linked_list` are distinct
opaque stdlib types. `deque`, `queue`, and `stack` use the native deque runtime
so front/back operations avoid the old list-front shifting cost. The linked-list
modules expose cursor APIs as positional indices over an indexed double-ended
buffer sequence (backed by `VecDeque`) rather than raw heap-allocated pointer
nodes, ensuring cyclic reference safety under ARC. Consequently, push/pop at
the ends are O(1) amortized, while cursor lookup, `insert_after`, and `remove_at`
are O(n).
`cursor_front`, `cursor_back`, `find`, `value_at`, `insert_after`,
`insert_before`, and `remove_at` return `optional`/`bool` instead of panicking on
invalid cursors. A cursor is invalid after structural changes that move or remove
items before it. `for` over these opaque handles uses a snapshot list. Use
`to_list` when the snapshot needs to be stored explicitly.

`tree.Tree[T]` is an opaque arena tree handle. `tree.NodeId` identifies a node
inside one tree. `tree.children` and traversal functions return snapshot lists
of node ids. `tree.try_value` and `tree.contains_node` are the safe APIs for
unknown node ids. `tree.value` still reports invalid ids as a runtime error with
the message `ori tree node id is invalid`. `tree.move_subtree` rejects the root,
foreign/removed ids, and moves that would create a cycle. `tree.OrderedTree[T]`
is not part of v1; ordered insert/search/remove remain reserved until the
Comparable contract is stable.

`hash_table.HashTable[K,V]` is a public advanced API over the same native hash
engine used by `map[K,V]`. It exists for explicit capacity control and for
`get/remove` APIs that return `optional[V]`. Keys follow the same rule as
`map`: use `int`, `string`, or a non-recursive user type with
`ori.core.Hashable` plus structural or explicit `ori.core.Equatable`.

`graph.Graph[N]` is an opaque adjacency-list graph. `graph.new(true)` creates a
directed graph; `graph.new(false)` creates an undirected graph. `graph.add_edge`
ensures both endpoint nodes exist and stores weight `1`. `graph.add_weighted_edge`
also ensures both endpoint nodes exist and stores a non-negative weight. Re-adding
the same edge updates its weight. `graph.edges` keeps the old `tuple[N,N]`
snapshot contract; use `graph.edge_weight` when the weight matters.
`graph.topological_sort` keeps the old list contract and returns an empty list
when the graph is undirected or cyclic. New code can use
`graph.try_topological_sort` for an explicit `optional[list[N]]` result.
`graph.shortest_path` is unweighted BFS shortest path.
`graph.shortest_weighted_path` uses stored edge weights.
`for node in graph_value` iterates a snapshot of graph nodes.
For user-defined struct and non-recursive enum nodes, `Equatable.equals` is used
for node identity and edge endpoint matching. Since `graph.new` is type-erased,
the first operation that carries a concrete node value installs this callback;
subsequent operations must use a compatible node kind. `graph.clone` and
`graph.transitive_closure` preserve the callback. Recursive/generic edge cases
and linked-list complexity remain tracked by `LANG-GRAPH-LIST-1`.

`heap.Heap[T]` is an opaque min-heap. The smallest value according to the
element ordering is returned first. The v1 runtime supports `int`, `string`,
and user-defined types that implement `ori.core.Comparable`. Empty `pop` and
`peek` calls return `none`. `heap.to_list` returns heap storage order, not
sorted order. Use `heap.into_sorted_list` for sorted output. Custom closure
comparators are reserved for a later phase because the public closure-comparator
ABI is intentionally separate from the heap API.
`for item in heap_value` iterates a snapshot in heap storage order.

Collection complexity contract for v1 native runtime:

| Family | Expected cost |
| --- | --- |
| `list` | `get`/`set` O(1), `push` amortized O(1), `insert`/`remove` O(n), `sort` O(n log n). |
| `map`, `set`, `hash_table` | `get`/`set`/`contains`/`remove` average O(1), worst-case O(n), snapshots O(n). |
| `deque`, `queue`, `stack` | Front/back push/pop O(1) amortized, snapshots O(n). |
| `linked_list`, `doubly_linked_list` | Cursor lookup and positional insert/remove O(n), front/back push/pop O(1) amortized, `find` O(n). |
| `tree` | Node lookup O(1), traversal/find/clone O(n), `move_subtree` O(n) because it validates cycles. |
| `graph` | Node lookup is linear in v1, traversal O(V + E), unweighted shortest path O(V + E), weighted shortest path O(V^2 + E). |
| `heap` | `push`/`pop` O(log n), `peek` O(1), `from_list`/`merge` O(n log n), sorted output O(n log n). |

These opaque collection handles are `Transferable` when their element type is
`Transferable`. They do not implement structural `Equatable` or `Hashable` in
v1. Direct `for` iteration works for `list`, `set`, `map`, `range`, `string`,
`bytes`, `deque`, `queue`, `stack`, `linked_list`, `doubly_linked_list`,
`hash_table`, `graph`, and `heap`. Opaque collection iteration is snapshot
based, not lazy.

---

## `ori.iter` — Functional Collection Operations

Status: implemented for the current eager list ABI. The module is importable
today. The checker accepts generic list contracts and the native runtime stores
list items as word-sized values, so scalar values and runtime handles such as
strings, tuples, lists, maps, and user values can flow through the same
operations.

```text
import ori.iter = iter

iter.map[T, R](values: list[T], mapper: func(T) -> R) -> list[R]
iter.filter[T](values: list[T], predicate: func(T) -> bool) -> list[T]
iter.any[T](values: list[T], predicate: func(T) -> bool) -> bool
iter.all[T](values: list[T], predicate: func(T) -> bool) -> bool
iter.count_where[T](values: list[T], predicate: func(T) -> bool) -> int
iter.take[T](values: list[T], n: int) -> list[T]
iter.skip[T](values: list[T], n: int) -> list[T]
iter.reverse[T](values: list[T]) -> list[T]
iter.reduce[T, R](values: list[T], initial: R, reducer: func(R, T) -> R) -> R
iter.find[T](values: list[T], predicate: func(T) -> bool) -> optional[T]
iter.flat_map[T, R](values: list[T], mapper: func(T) -> list[R]) -> list[R]
iter.sort[T](values: list[T]) -> list[T] for T: Comparable
iter.sort_by[T](values: list[T], compare: func(T, T) -> int) -> list[T]
iter.unique[T](values: list[T]) -> list[T] for T: Equatable
iter.zip[A, B](a: list[A], b: list[B]) -> list[tuple[A, B]]
iter.partition[T](values: list[T], predicate: func(T) -> bool) -> tuple[list[T], list[T]]
iter.group_by[T, K](values: list[T], key: func(T) -> K) -> map[K, list[T]]
    for K: Hashable and K is Equatable
iter.flatten[T](nested: list[list[T]]) -> list[T]
```

All `iter.*` functions are **eager**: they return a new `list[T]` immediately.
Lazy evaluation is explicit through `lazy[T]`, `lazy.once`, and `lazy.force`.

Current implementation status:

- `iter.map(values, mapper)`, `iter.filter(values, predicate)`,
  `iter.any(values, predicate)`, `iter.all(values, predicate)`, and
  `iter.count_where(values, predicate)` use the current closure ABI and keep
  the source element type.
- `iter.take(values, n)`, `iter.skip(values, n)`, and `iter.reverse(values)`
  return new lists with the same element type.
- `iter.reduce(values, initial, reducer)` folds from left to right.
- `iter.find(values, predicate)` returns `some(value)` for the first matching
  value, or `none`.
- `iter.flat_map(values, mapper)` concatenates returned lists eagerly.
- `iter.sort(values)` has native int and string ordering coverage.
- `iter.sort_by(values, compare)` uses an `int` comparator: negative or zero
  keeps `a` before `b`, positive places `a` after `b`.
- `iter.unique(values)` keeps the first occurrence of each value. Native string
  uniqueness compares string contents.
- `iter.zip(a, b)` returns a new list[pairs and stops at the shorter input
  list].
- `iter.partition(values, predicate)` returns two new lists: matching values
  first, non-matching values second.
- `iter.group_by(values, key)` appends each input value to the list stored under
  its computed key. Native runtime covers `int` and `string` keys.
- `iter.flatten(nested)` flattens one level of `list[list[int]]` into a new
  `list[T]`.
- Native tests cover the non-`int` path with `list[string]`,
  `map[string, list[string]]`, `optional[string]`, `tuple[string, int]`, and
  `list[list[string]]`.

---

## `ori.math` — Mathematics

```text
import ori.math = math

math.abs(x: int) -> int
math.abs(x: float) -> float
math.min(a: int, b: int) -> int
math.min(a: float, b: float) -> float
math.max(a: int, b: int) -> int
math.max(a: float, b: float) -> float
math.clamp(value: int, min: int, max: int) -> int
math.floor(x: float) -> int
math.ceil(x: float) -> int
math.round(x: float) -> int
math.sqrt(x: float where x >= 0.0) -> float
math.pow(base: float, exp: float) -> float
math.log(x: float where x > 0.0) -> float
math.log2(x: float where x > 0.0) -> float
math.sin(x: float) -> float
math.cos(x: float) -> float
math.tan(x: float) -> float
math.pi: float
math.e: float
math.infinity: float
math.nan: float
math.is_nan(x: float) -> bool
math.is_infinite(x: float) -> bool
```

Mixed integer/float calls are not implicitly widened. Use `float(value)` when a
call should use the float overload.

## `ori.mem` - Memory Inspection

```text
import ori.mem = mem

mem.size_of(value) -> int
mem.align_of(value) -> int
```

Both functions return compile-time constants for the static type of `value`.
The current parser does not support type-argument call syntax such as
`size_of[T]()`; use a value as the type witness.

## `ori.time` - Time

```text
import ori.time = time
import ori.time (Instant, Duration, instant_now, duration_seconds)

time.now() -> int
time.sleep(millis: int) -> void
time.duration_ms(start: int, end: int) -> int

Instant
Duration

time.instant_now() -> time.Instant
time.instant_from_unix_ms(value: int) -> time.Instant
time.instant_to_unix_ms(value: time.Instant) -> int
time.duration_millis(value: int) -> time.Duration
time.duration_seconds(value: int) -> time.Duration
time.duration_minutes(value: int) -> time.Duration
time.duration_hours(value: int) -> time.Duration
time.duration_to_millis(value: time.Duration) -> int
time.elapsed_since(start: time.Instant) -> time.Duration
time.between(start: time.Instant, finish: time.Instant) -> time.Duration
time.add(value: time.Instant, duration: time.Duration) -> time.Instant
time.sub(value: time.Instant, duration: time.Duration) -> time.Instant
time.sleep_duration(duration: time.Duration) -> void
```

`time.now()` returns the Unix timestamp in milliseconds. `time.sleep(0)` is
valid and returns immediately. `Instant` and `Duration` are `.orl` value
wrappers over milliseconds; they make APIs read better without changing the
runtime ABI.

## `ori.format` - Presentation Formatting

```text
import ori.format = format

format.number(value: float, decimals: int) -> string
format.percent(value: float, decimals: int) -> string
format.hex(value: int) -> string
format.binary(value: int) -> string
format.date(millis: int, style: string) -> string
format.datetime(millis: int, style: string, locale: string) -> string
format.bytes_size(bytes: int, style: string) -> string
```

Current status:

- `decimals` is explicit by API design; default arguments are supported by the
  language, but this formatter keeps its precision choice visible at each call.
- `date` and `datetime` currently format UTC ISO output.
- `bytes_size` accepts `"binary"` for KiB/MiB units. Other style values use
  decimal KB/MB units.

## Additional Module Contracts

The modules below carry their own implementation notes. Some are implemented
only for a subset of functions or backend targets, and future modules may use
`bind.stdlib_module_unavailable` while they are documented but intentionally
blocked.

---

## `ori.random` — Random Numbers

```text
import ori.random = random

random.int(min: int, max: int) -> int       -- inclusive range
random.float(min: float, max: float) -> float
random.bool() -> bool
random.choice[T](items: list[T]) -> optional[T]
random.shuffle[T](items: list[T]) -> list[T]
```

Current implementation status:

- `random.int`, `random.float`, and `random.bool` are importable and available
  in the native backend.
- `random.choice(items)` returns `some(value)` for a random item, or `none` for
  an empty list.
- `random.shuffle(items)` returns a new shuffled list with the same element
  type.
- If `min > max`, `random.int` and `random.float` swap the bounds before
  generating the value.
- `random.int(min, max)` uses an inclusive range.
- `random.float(min, max)` returns a value inside the normalized range.
- Native tests cover generic `choice[T]` and `shuffle[T]` for non-`int`
  elements through the list/optional storage ABI.

---

## `ori.crypto` — Cryptographic Helpers

Status: **password hashing** (Layer 1). Algorithm: **argon2id** with PHC string
encoding (salted, parameters in the hash string).

```ori
import ori.crypto = crypto

const hash: string = crypto.password_hash("secret")
const ok: bool = crypto.password_verify("secret", hash)

const secret: string = crypto.totp_generate_secret()
const code: string = crypto.totp_code(secret, 1_700_000_000)
const totp_ok: bool = crypto.totp_verify(secret, code, 1_700_000_000, 1)
```

| Function | Type | Notes |
|----------|------|--------|
| `password_hash(password)` | `string → string` | Empty string on failure |
| `password_verify(password, encoded)` | `string, string → bool` | Constant-time verify via `argon2` crate |
| `totp_generate_secret()` | `→ string` | Base32 secret (160-bit) |
| `totp_code(secret, unix_secs)` | `string, int → string` | 6-digit code; empty on failure |
| `totp_verify(secret, code, unix_secs, window)` | `… → bool` | ±`window` steps (max 10) |
| `try_hash_password(password)` | `string → result[string, string]` | Safe result wrapper, `err(...)` on failure |
| `try_totp_generate_secret()` | `→ result[string, string]` | Safe result wrapper, `err(...)` on failure |
| `try_totp_code(secret, unix_secs)` | `string, int → result[string, string]` | Safe result wrapper, `err(...)` on failure |

### Net timeouts (B7)

| Function | Type | Notes |
|----------|------|--------|
| `set_read_timeout_ms(conn, ms)` | `Connection, int → result[void,string]` | `ms <= 0` clears |
| `set_write_timeout_ms(conn, ms)` | same | used by `web.set_read_timeout` |

Layer 2 wrappers in `stdlib/crypto.orl`: `hash_password` / `verify_password` /
`totp_generate_secret` / `totp_code` / `totp_verify` / `try_hash_password` /
`try_totp_generate_secret` / `try_totp_code`.

Do **not** use plain MD5/SHA for password storage. Prefer this API for auth
(web C10 / SEC9). TOTP is for 2FA (web C3 / `ori-web-auth`).

---

## `ori.lazy` - Lazy Values

Status: implemented. `lazy[T]` stores a zero-argument thunk and caches the
computed value after the first force.

```ori
import ori.lazy = lz

const delayed: lazy[int] = lz.once(() => compute())
const value: int = lz.force(delayed)
```

Functions:

```text
lazy.once[T](thunk: func() -> T) -> lazy[T]
lazy.force[T](value: lazy[T]) -> T
```

**Native runtime note:** `lazy.once` and `lazy.force` are implemented via **inline Cranelift
codegen** (no runtime FFI symbols). They are fully supported on the native route.

The shorthand `lazy.once(...)` and `lazy.force(...)` are also accepted without
an import.

---

## Concurrency Foundation

Status: implemented for the native backend.

Values that cross a task or channel boundary must be `Transferable`.

Currently transferable:

- primitive scalar values;
- `string` and `bytes`;
- `list[T]`, `map[K, V]`, `set[T]`, `optional[T]`, `result[T, E]`, tuples,
  `future[T]`, `task.Job[T]`, `channel.Channel[T]`, and the opaque collection
  handles above when their contents are transferable;
- structs when all fields are transferable;
- enum values when every variant payload field is transferable (recursive enum
  definitions are checked with a cycle-safe instantiation guard);
- `atomic.AtomicInt` and the opaque task/channel error handles.

Resource handles are deliberately not transferable: `fs.File`, `io.Input`,
`io.Output`, `net.Connection`, `net.Listener`, and `net.UdpSocket` borrow
process/OS state whose lifetime and thread affinity are controlled by their
owning scope. `task.CancelToken` is the explicit exception because it contains
only an atomic cancellation flag and is designed for cross-task coordination.

Function values, lazy thunks, and `any[Trait]` values are not transferable by
default. A direct named function value is accepted when its definition summary
proves no mutable-global effect; an unsafe named function is rejected with
`concurrency.global_mutable_capture`. Function values stored in local bindings
are accepted when the checker records their closure captures and every
captured value is transferable. Unknown function environments and nested
function captures remain rejected with `concurrency.not_transferable`. A
closure passed to `task.spawn` also
cannot capture a lexical `var` binding or directly read/write a top-level
mutable `var`. A module-level `var`
is shared by all threads and has no implicit synchronization. Use an
`atomic.AtomicInt`, a channel, or explicit single-owner state instead. The
checker reports `concurrency.global_mutable_capture`. Same-module helper calls
and imported named helpers are checked by conservative call-graph passes;
receiver methods and `any[Trait]` dispatch are conservatively matched by method
name. Local closure environments are represented by a checker-side capture
summary today; a complete type-level isolation model and richer transitive
function-value effects remain tracked in `CONC-THREADS-1`.

Extern function values are treated as unknown effects and remain rejected at a
task boundary until the host contract supplies an explicit transfer/effect
summary.

`ori.concurrent` is importable today as the umbrella module for this contract.
Its concrete APIs currently live in `ori.task`, `ori.channel`, and
`ori.atomic`. Generic value helpers copy and transfer values across task boundaries:

```ori
import ori.concurrent as conc

const v: int = conc.copy(42)
const s: string = conc.transfer("message")
```

| Function | Type | Notes |
|---|---|---|
| `copy[T](value: T) -> T` | generic copy | Returns the value; collections clone via their module helpers. |
| `transfer[T](value: T) -> T` | generic transfer | Value must satisfy `Transferable` when crossing tasks. |
| `copy_int`, `copy_string`, `copy_list_int`, ... | monomorphic legacy | Preserved for compatibility; delegate to the generic form. |

---

## `ori.task` - Tasks and Futures

Status: implemented in the native runtime for explicit task and future APIs.
The runtime now supports pollable futures, private failed/cancelled states, a
FIFO executor queue, continuation scheduling, and non-blocking timers.
`async` functions currently use native state-machine lowering for the supported v1
subset. The call returns a `future[T]` immediately, the generated frame is
scheduled on the native executor, and supported `await` shapes suspend through
`ori_future_poll` plus `ori_future_on_ready` instead of calling
`task.block_on`.

Async bodies outside the current subset fail with `backend.native_unsupported`
before Cranelift. `task.block_on` stays available only as an explicit sync
bridge.

```text
import ori.task = task

task.spawn[T](work: func() -> T) -> task.Job[T]
task.join[T](job: task.Job[T]) -> result[T, task.JoinError]
task.detach[T](job: task.Job[T]) -> void
task.block_on[T](future: future[T]) -> T
task.sleep(ms: int) -> future[void]
task.create_token() -> task.CancelToken
task.cancel(token: task.CancelToken) -> void
task.is_cancelled(token: task.CancelToken) -> bool
task.associate(token: task.CancelToken, future: future[void]) -> void
```

Rules:

- `task.spawn` runs a no-argument function or closure on a native thread.
- Captured values and the return value must satisfy `Transferable`.
- `task.join` returns `ok(value)` when the task finishes normally.
- `task.join` returns `err(...)` when the job was already joined, missing, or
  the native thread panicked. The error is an opaque `task.JoinError` value.
- `task.detach` lets the native thread continue without requiring a join.
- `task.sleep(ms)` creates a pending `future[void]` that becomes ready after
  the requested delay. It uses the runtime timer thread and does not block the
  executor queue.
- `task.block_on` is the explicit sync bridge. It waits for a future, drains
  queued executor continuations while waiting, and returns the stored value.

Future runtime state:

- `pending`: value is not ready yet;
- `ready`: value is available;
- `failed`: internal runtime failure state;
- `cancelled`: internal cancellation state.

Public cooperative cancellation is available through `task.CancelToken`.
Associate a token with a future via `task.associate`; poll cancellation with
`task.is_cancelled` or cancel explicitly with `task.cancel`.

Ownership rule: a future keeps its stored value alive until `block_on` observes
it or until the future object is released. `task.sleep` produces
`future[void]`. `async f(...) -> T` produces `future[T]` immediately; in
the state-machine path, the future becomes ready, failed, or cancelled when the
generated async frame reaches its terminal state.

---

## `ori.cancel` — Structured Cancellation and Task Scopes

`ori.cancel` provides structured lifetime management for cancellation tokens and child tasks, ensuring child tasks do not leak and cancellation propagates on scope exit.

```ori
import ori.cancel = cancel
import ori.io = io
import ori.list = lists
import ori.task = task

run_tasks() -> void
    using scope: cancel.TaskScope = cancel.create_task_scope()
    const job1: task.Job[void] = task.spawn(() => io.println("child 1"))
    const job2: task.Job[void] = task.spawn(() => io.println("child 2"))
    lists.push(scope.jobs, job1)
    lists.push(scope.jobs, job2)
    -- On block exit, scope.dispose() cancels the token and joins all tracked jobs.
end
```

| Type / Function | Type | Notes |
|---|---|---|
| `CancelScope` | `struct { token: task.CancelToken }` | Cancellation scope implementing `core.Disposable`. |
| `create_scope()` | `→ CancelScope` | Creates a scope with a fresh cancellation token. |
| `create_scope_with(token)` | `task.CancelToken → CancelScope` | Wraps an existing token in a scope. |
| `cancel(scope)` | `CancelScope → void` | Cancels the scope's token immediately. |
| `is_cancelled(scope)` | `CancelScope → bool` | Checks if the scope has been cancelled. |
| `defer_cancel(scope, millis)` | `CancelScope, int → future[void]` | Asynchronously waits then cancels the scope. |
| `TaskScope` | `struct { token: task.CancelToken, jobs: list[task.Job[void]] }` | Structured task scope implementing `core.Disposable`. |
| `create_task_scope()` | `→ TaskScope` | Creates a task scope with a fresh token and empty job list. |
| `create_task_scope_with(token)` | `task.CancelToken → TaskScope` | Wraps an existing token into a task scope. |
| `cancel_tasks(scope)` | `TaskScope → void` | Cancels the task scope's token. |
| `is_tasks_cancelled(scope)` | `TaskScope → bool` | Checks if the task scope has been cancelled. |
| `join_tasks(scope)` | `TaskScope → void` | Explicitly joins all tracked jobs. |

Structured concurrency rule:
Both `CancelScope` and `TaskScope` implement `core.Disposable`. When used with `using`, their `dispose` method is guaranteed to run upon exiting the block (by normal completion, return, or error). For `CancelScope`, `dispose` calls `task.cancel(scope.token)`. For `TaskScope`, `dispose` calls `task.cancel(scope.token)` and then joins every child job in `scope.jobs`, preventing detached background tasks from outliving the parent scope.

---

## `ori.channel` - Channels

Status: implemented in the native runtime with real synchronization.

```text
import ori.channel = channel

channel.create[T]() -> channel.Channel[T]
channel.create_bounded[T](capacity: int) -> optional[channel.Channel[T]]
channel.send[T](ch: channel.Channel[T], value: T) -> result[void, channel.SendError]
channel.receive[T](ch: channel.Channel[T]) -> result[T, channel.ReceiveError]
channel.close[T](ch: channel.Channel[T]) -> void
```

Behavior:

- `channel.create` creates an unbounded FIFO channel. Use
  `channel.create_bounded` when the producer must have a finite queue.
- `channel.create_bounded` returns `some(channel)` for a positive capacity and
  `none` for zero or negative capacities. A bounded channel blocks `send` while
  its queue is full; closing it wakes blocked senders, which then receive
  `err(...)`. The queue never grows beyond the declared capacity.
- `channel.send` enqueues a transferable value, or returns `err(...)` when
  the channel is closed.
- `channel.receive` waits until a value is available, or returns `err(...)`
  when the channel is closed and empty.
- `channel.close` closes the channel and wakes waiting receivers.

The error values are opaque handles: `channel.SendError` and
`channel.ReceiveError`.

---

## `ori.atomic` - Atomic Integers

Status: implemented in the native runtime.

```text
import ori.atomic = atomic

atomic.new(value: int) -> atomic.AtomicInt
atomic.load(value: atomic.AtomicInt) -> int
atomic.store(value: atomic.AtomicInt, next: int) -> void
atomic.add(value: atomic.AtomicInt, delta: int) -> int
```

`atomic.add` returns the new value after the addition. Generic `Atomic[T]` is
intentionally deferred until there is a concrete need.

---

## `ori.json` — JSON

Status: **implemented** in the native runtime with a structured recursive value type.

```ori
import ori.json = json

enum Value
    Null
    Bool { value: bool }
    Number { value: float }
    String { value: string }
    Array { items: list[Value] }
    Object { fields: map[string, Value] }
end

json.parse(text: string) -> result[json.Value, string]
json.try_parse(text: string) -> result[json.Value, JsonError]
json.try_read(file_path: string) -> result[json.Value, JsonError]
json.stringify(value: json.Value) -> string
json.stringify_pretty(value: json.Value) -> string
```

Typed errors for JSON operations:

```ori
public enum JsonError
    ParseError(message: string)
    IoError(message: string)
end
```

Current behavior:

- `json.parse(text)` returns `ok(Value)` for valid JSON.
- `json.parse(text)` returns `err("invalid json")` for invalid JSON.
- `json.stringify` and `json.stringify_pretty` serialize the structured `Value`
  enum recursively.

---

## `ori.test` — Testing

Status: implemented for the native test runner. Test functions marked with
`@test` can be run with `ori test <file-or-project>`. The assertion module is
intentionally small and grows only when a runtime-backed assertion has a stable
contract.

Use `ori test <file-or-project> --filter <name>` to run only tests whose full
name or short function name contains `<name>`. The runner reports how many tests
were discovered and how many matched the filter.

The `ori.test` module is importable today for basic assertion helpers.

```ori
import ori.test = test
import ori.task = task

-- Test functions are marked with attr:
@test
test_addition()
    check 1 + 1 == 2
end

@test
async test_async_work()
    await task.sleep(1)
    test.assert(true, "async test should run")
end

-- Assertions:
test.assert(condition: bool, message: string) -- implemented
test.assert_eq[T](a: T, b: T)                 -- implemented
test.assert_ne[T](a: T, b: T)                 -- implemented
test.fail(message: string)                    -- implemented
```

`assert_eq` and `assert_ne` currently cover `int`, `bool`, `float`, `string`,
and user values that pass the `ori.core.Equatable` trait gate. `check`
statements also work inside `@test` functions. Async tests must have no
parameters and must return `void`; the test runner waits for their returned
future before recording the result.

Native runtime coverage is canonical for standard-library behavior. AOT and
JIT must agree on their shared support surface.

---

## `ori.net` — Networking (TCP/TLS/UDP)

```text
import ori.net = net

net.connect(host, port, timeout_ms) -> result[net.Connection, string]
net.connect_async(host, port, timeout_ms) -> future[result[net.Connection, string]]
net.connect_tls(host, port, timeout_ms) -> result[net.Connection, string]
net.connect_tls_async(host, port, timeout_ms) -> future[result[net.Connection, string]]
net.listen(host, port) -> result[net.Listener, string]
net.accept(listener) -> result[net.Connection, string]
net.accept_async(listener) -> future[result[net.Connection, string]]
net.close_listener(listener)
net.listener_port(listener) -> int
net.read_some(conn, max_bytes) -> result[bytes, string]
net.read_some_async(conn, max_bytes) -> future[result[bytes, string]]
net.write_all(conn, data) -> result[void, string]
net.write_all_async(conn, data) -> future[result[int, string]]
net.close(conn)
net.is_closed(conn) -> bool
net.udp_bind(host, port) -> result[net.UdpSocket, string]
net.udp_send_to(sock, host, port, data) -> result[int, string]
net.udp_send_to_async(sock, host, port, data) -> future[result[int, string]]
net.udp_recv_from(sock, max_bytes) -> result[bytes, string]
net.udp_recv_from_async(sock, max_bytes) -> future[result[bytes, string]]
net.udp_close(sock)
net.udp_local_port(sock) -> int
net.try_connect(host, port, timeout_ms) -> result[net.Connection, NetError]
```

Typed errors for networking operations:

```ori
public enum NetError
    ConnectionRefused
    TimedOut
    HostUnreachable
    AddressInUse
    Closed
    Other(message: string)
end
```

Current implementation notes:

- Sync network I/O is **blocking** in the native runtime.
- **STDLIB-4b (shipped):** `*_async` helpers return `future[…]` so the async
  executor is not blocked. Gate: `compile_runs_net_connect_async_loopback`.
- **STDLIB-4k (shipped):** readiness-multiplexed I/O reactor (`poll(2)` on Unix)
  for `accept_async` / `read_some_async` / `write_all_async` /
  `udp_recv_from_async` / `udp_send_to_async`. Connect/TLS/FS async still use
  worker threads (no pollable fd before the op starts). Gate:
  `compile_runs_net_udp_async_loopback`.
- `connect_tls` performs a TCP connect then a TLS client handshake (rustls,
  system trust roots via webpki-roots). The returned `Connection` is the same
  opaque type as plain TCP.
- `listen` with port `0` binds an ephemeral port; `listener_port` reads the
  assigned port. Same pattern for `udp_bind` + `udp_local_port`.
- Layer 2 helpers live in `stdlib/net.orl` (flatten imports) and
  `stdlib/net/utils.orl`.

---

## `ori.os` — Operating System

```text
import ori.os = os

os.args() -> list[string]        -- command-line arguments
os.env(name: string) -> optional[string]
os.exit(code: int)
os.pid() -> int
os.platform() -> string          -- "linux", "windows", "macos"
os.arch() -> string              -- "x86_64", "aarch64", etc.
```

Current implementation notes:

- `os.args()` returns the process argument vector as reported by the host. The
  first item is the executable path/name when the platform provides it. The
  runtime keeps these host-owned strings valid for the process lifetime.
- `os.env(name)` returns `some(value)` when the variable exists and `none`
  otherwise.
- `os.platform()` currently normalizes known targets to `"windows"`, `"linux"`,
  `"macos"`, or `"unknown"`.
- `os.arch()` currently normalizes known targets to `"x86_64"`, `"aarch64"`,
  `"x86"`, `"arm"`, or `"unknown"`.

---

## `ori.process` — Subprocess Execution

`ori.process` provides subprocess execution with structured, binary-safe process output.

```ori
import ori.io = io
import ori.process = proc
import ori.string = string_mod

struct ProcessOutput
    status: int
    stdout: bytes
    stderr: bytes
end

main()
    match proc.run_output("echo", ["hello"])
        case ok(out):
            io.println(string(out.status == 0))
            match proc.stdout_text(out)
                case ok(text): io.print(text)
                case err(e): io.eprint("decode error: " + e)
            end
        case err(msg):
            io.eprint("failed to run: " + msg)
    end
end
```

| Function / Type | Type | Notes |
|---|---|---|
| `ProcessOutput` | `struct { status: int, stdout: bytes, stderr: bytes }` | Binary-preserving execution result. |
| `run(program, args)` | `string, list[string] → result[int, string]` | Spawns process, waits, returns exit code. |
| `run_output(program, args)` | `string, list[string] → result[ProcessOutput, string]` | Spawns process, captures raw stdout and stderr bytes without loss. |
| `stdout_text(output)` | `ProcessOutput → result[string, string]` | Decodes `stdout` bytes to UTF-8 text safely. |
| `stderr_text(output)` | `ProcessOutput → result[string, string]` | Decodes `stderr` bytes to UTF-8 text safely. |
| `run_capture(program, args)` | `string, list[string] → result[map[string, string], string]` | Legacy map capture for backward compatibility. |
| `exit_code(capture)` | `map[string, string] → int` | Extracts exit code from legacy map capture. |
| `stdout(capture)` | `map[string, string] → string` | Extracts stdout string from legacy map capture. |
| `stderr(capture)` | `map[string, string] → string` | Extracts stderr string from legacy map capture. |

---

## `ori.args` - CLI Arguments

```text
import ori.args = args

args.all() -> list[string]
args.count() -> int
args.get_or(index: int, fallback: string) -> string
args.program_name_or(fallback: string) -> string
```

`ori.args` is a small `.orl` convenience layer over `ori.os.args`.
`get_or` copies a selected argument before the temporary argument list is
released, so the returned `string` can safely outlive the lookup expression.

---

## `ori.log` - Minimal Logging

```ori
import ori.log = log

log.info(message: string)
log.warn(message: string)
log.error_message(message: string)
log.debug(message: string)
```

The first version is intentionally simple and CLI-oriented. `info`, `warn`, and
`debug` write to stdout. `error_message` writes to stderr.

---

## `ori.config` - Local Config Helpers

```text
import ori.config = config

config.read_text(path: string) -> result[string, string]
config.read_text_or(path: string, fallback: string) -> string
config.write_text(path: string, content: string) -> result[string, string]
config.read_json(path: string) -> result[json.Value, string]
config.write_json(path: string, value: json.Value) -> result[string, string]
```

`ori.config` is a small `.orl` layer over `ori.fs` and `ori.json`. It is meant
for local project/tool config, not for a full schema-validation framework.

---

## `ori.Error` — Standard Error Type

Status: implemented base value type. `import ori.Error` is accepted. Prefer an
alias when constructing the value, because `err(...)`/`Error(...)` are also
result wrapper forms.

```ori
struct ori.Error
    code: string
    message: string
end

import ori.Error = StdError

const err: StdError = StdError { code: "E_IO", message: "could not read file" }
```

Future `ori.*` functions that expose rich errors are expected to return
`result[T, ori.Error]`. Cause chaining and full `ori.core.Error` trait-method
integration are still planned. Current implemented filesystem and parse helpers
still use `string` errors or `optional[T]` where documented above.

## `ori.err_trace` — Error Return Traces

Status: implemented (`ERR-TRACE-1`). Provides runtime-backed helpers to append
source file and line locations to error messages during propagation and format
trace strings.

```ori
import ori.err_trace = trace

public fail_level1() -> result[int, string]
    match fail_level2()
        case ok(v):
            return ok(v)
        case err(msg):
            return err(trace.push("service.orl", 42, msg))
    end
end

main()
    match fail_level1()
        case ok(_):
            -- ...
        case err(e):
            const formatted = trace.format(e)
            -- e.g. "connection reset\n  at service.orl:42"
    end
end
```

### Functions

- `push(file: string, line: int, message: string) -> string`: Appends `\n  at <file>:<line>` to the error message string.
- `format(message: string) -> string`: Formats the error trace or returns `<nil error>` for empty errors.

---

## `ori.net.http` — HTTP Client and Types

`ori.net.http` provides low-level HTTP/1.1 client utilities built over `ori.net` TCP and TLS.

```ori
import ori.net.http as http

public struct Response
    status: int
    status_text: string
    headers: string
    body: string

    is_success(self) -> bool
        return self.status >= 200 and self.status < 300
    end

    is_redirect(self) -> bool
        return self.status >= 300 and self.status < 400
    end

    is_client_error(self) -> bool
        return self.status >= 400 and self.status < 500
    end

    is_server_error(self) -> bool
        return self.status >= 500 and self.status < 600
    end
end

public enum HttpError
    ConnectionFailed(message: string)
    Timeout
    InvalidResponse(message: string)
    TlsError(message: string)
    Other(message: string)
end
```

| Function | Signature | Notes |
|---|---|---|
| `get_plain(host, port, path, timeout)` | `string, int, string, int -> ResponseResult` | Blocking HTTP GET over plain TCP. |
| `get_tls(host, port, path, timeout)` | `string, int, string, int -> ResponseResult` | Blocking HTTP GET over TLS. |
| `post_plain(host, port, path, headers, body, timeout)` | `string, int, string, string, string, int -> ResponseResult` | Blocking HTTP POST over plain TCP. |
| `post_tls(host, port, path, headers, body, timeout)` | `string, int, string, string, string, int -> ResponseResult` | Blocking HTTP POST over TLS. |
| `build_request(...)` | `string, string, string, string, string -> string` | Serializes an HTTP/1.1 request wire buffer. |
| `parse_response(raw)` | `string -> ResponseResult` | Parses an HTTP/1.1 response wire buffer. |

---

## `ori.image` — Image Export (BMP & PPM)

`ori.image` provides direct software rendering export for pixels without external dependencies.

```ori
import ori.image as img

public enum ImageFormat
    Bmp
    Ppm
end

public struct Image
    width: int
    height: int
    pixels: list[int]

    pixel_count(self) -> int
        return self.width * self.height
    end

    get_pixel(self, x: int, y: int) -> int
        if x < 0 or x >= self.width or y < 0 or y >= self.height
            return 0
        end
        const idx: int = y * self.width + x
        if idx < ori.list.len(self.pixels)
            return ori.list.get(self.pixels, idx)
        end
        return 0
    end
end
```

| Function | Signature | Notes |
|---|---|---|
| `encode_ppm(w, h, pixels)` | `int, int, list[int] -> string` | Encodes pixel buffer into ASCII PPM (P3) format. |
| `write_ppm(path, w, h, pixels)` | `string, int, int, list[int] -> result[string, string]` | Writes pixel buffer to a PPM file. |
| `encode_bmp(w, h, pixels)` | `int, int, list[int] -> bytes` | Encodes pixel buffer into standard 24-bit uncompressed Windows BMP. |
| `write_bmp(path, w, h, pixels)` | `string, int, int, list[int] -> result[string, string]` | Writes pixel buffer to a BMP file. |

