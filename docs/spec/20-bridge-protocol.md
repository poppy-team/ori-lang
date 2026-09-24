# Ori Language Specification — Chapter 20: Compiler Host/Bridge Protocol

> Status: **experimental implementation, partial v1**
> Audience: compiler implementers, tool authors, runtime maintainers
> Surface: **S3 / Marco B** · workspace **`0.3.8`**
> Revision tag: **`ori-bridge-proto-1`**
> Process: [ADR-0006](../decisions/adr/0006-selfhost-modular-architecture.md)

---

## 1. Purpose and Scope

This chapter specifies the versioned framing protocol and data schema exchanged between the Ori self-hosted compiler frontend (`packages/compiler`) and the host code-generation bridge (`compiler/crates/ori-bridge-server`).

**Implementation boundary (2026-09-24):** The process entry point currently
accepts `--request-file <path>` containing an unframed JSON
`CompileModuleRequest` and constructs the envelope itself. The framing helpers
exist as library functions, but the self-host client does not use framed IPC.
The current scalar subset includes nested `If`/`While`/`Match`, `elif` chains lowered
to nested `If` nodes in the preceding `else` branch, `Let` mutability,
`Assign`, `Break`/`Continue`, and typed local calls with up to eight integer
parameters and integer, Boolean, or string results. String locals, string
returns, and `println` of a typed string expression retain their type.
The Ori client lowers unary `not` to a Boolean `IfExpr` and unary
minus to integer subtraction from zero. Scalar `Match` statements and
`MatchExpr` expressions accept signed `IntLit`,
`BoolLit`, and a final `Wildcard` arm; integer matches require a wildcard,
Boolean matches require either both literals or a wildcard. Statement arms
have isolated statement lists and retain the enclosing loop context;
expression arms yield values of one shared scalar type. Typed `list[string]`
locals, empty list literals, `ori.args.all` (through `ori.os.args`), and
`ori.list.len/get/push` use their native HIR signatures. The bridge checks the
list element and index types before code generation. The bridge rejects
duplicate, mismatched, or incomplete patterns
before writing the object file. The bridge checks scope, types, mutability,
and loop placement before lowering these nodes. Other collections, structural
types, generics, and non-scalar pattern matching remain outside this protocol implementation; this is not a
complete HIR, nor a production bootstrap contract. The proposed ADR-0006
describes the intended architecture. The schemas and timeouts below describe
the target protocol, not features already implemented by the CLI.

### In Scope
1. Length-prefixed framing and stream serialization.
2. Canonical request/response envelopes.
3. Intermediate Representation (HIR) transmission format.
4. Deterministic error diagnostics reporting.
5. Limits, timeouts, and resource constraints.

### Out of Scope
1. Process lifecycle management (handled by `ori-driver`).
2. JIT dynamic execution across the bridge (AOT object emission is the primary target).

---

## 2. Protocol Framing

The bridge communication takes place over standard bidirectional streams (Unix Domain Sockets, Named Pipes, or standard I/O pipes).

### Wire Format
All messages are binary framed with little-endian 32-bit integer prefixes:

```text
+-------------------+-------------------+---------------------------------------+
| Magic (4 bytes)   | Length (4 bytes)  | Payload (Length bytes)                |
| 0x4F 0x52 0x49 0x42| uint32_le         | JSON / Bincode serialized body        |
+-------------------+-------------------+---------------------------------------+
```

- **Magic**: `0x4F524942` (`"ORIB"`). Messages without this prefix must immediately abort the connection.
- **Length**: Unsigned 32-bit integer indicating payload size in bytes. Maximum payload size is `64 MiB` (67,108,864 bytes).
- **Encoding**: UTF-8 encoded canonical JSON for Protocol v1 (human-auditable and deterministic); binary bincode reserved for Protocol v2.

---

## 3. Envelope Definitions

Every payload begins with an envelope header declaring the schema version and correlation ID:

### Request Envelope
```json
{
  "protocol_version": 1,
  "request_id": 42,
  "command": "compile_module",
  "payload": { ... }
}
```

### Response Envelope
```json
{
  "protocol_version": 1,
  "request_id": 42,
  "status": "ok",
  "data": { ... },
  "diagnostics": []
}
```

If `status` is `"error"`:
```json
{
  "protocol_version": 1,
  "request_id": 42,
  "status": "error",
  "error": {
    "code": "bridge.invalid_ir",
    "message": "Type mismatch in HIR block: expected int, got string",
    "span": { "file_id": 1, "start": 104, "end": 120 }
  },
  "diagnostics": [ ... ]
}
```

---

## 4. Commands

### 4.1. `handshake`
Validates compatibility between client and bridge server.

**Request Payload:**
```json
{
  "client_version": "0.3.8",
  "supported_protocol": 1
}
```

**Response Data:**
```json
{
  "server_version": "0.3.8",
  "protocol_version": 1,
  "target_triple": "x86_64-unknown-linux-gnu",
  "features": ["cranelift", "object", "system_linker"]
}
```

### 4.2. `compile_module`
Submits a lowered HIR module to the bridge to generate an object file or final binary.

**Request Payload:**
```json
{
  "module": {
    "namespace": "main",
    "funcs": [
      {
        "def_id": 1,
        "name": "main",
        "params": [],
        "return_ty": "int",
        "body": {
          "stmts": [
            {
              "kind": "return",
              "value": { "kind": "literal_int", "value": 0, "ty": "int" }
            }
          ]
        },
        "is_public": true,
        "is_async": false
      }
    ]
  },
  "options": {
    "opt_level": "default",
    "output_type": "object",
    "output_path": "/tmp/out.o"
  }
}
```

**Response Data:**
```json
{
  "output_path": "/tmp/out.o",
  "bytes_written": 1420,
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```

---

## 5. Limits & Defensive Hardening

1. **Maximum Frame Size**: `64 MiB`. Frames exceeding this limit will trigger an immediate fatal close with `bridge.frame_too_large`.
2. **Read Timeout**: Individual frame read timeout is `30 seconds`.
3. **Execution Timeout**: Module compilation times out after `120 seconds`.
4. **Deterministic Ordering**: All dictionaries/maps in the IR must be serialized in deterministic sorted key order to guarantee reproducible builds.

## 6. Current experimental request-file subset

The implemented process path reads a JSON object with `module`, `output_path`
and `lib_mode`. `module` contains `namespace` and `funcs`; functions contain
`name`, `params`, `return_ty`, `body_stmts`, and `is_public`. Types are tagged
`Int`, `Float`, `Bool`, `String`, `Void`, or `{"List":"String"}` for the
current client subset. A typed empty list is `{"EmptyList":{"elem_ty":"String"}}`.
The supported statement shapes are
`Let` (with mutability), `Assign`, `Return`, `Expr`, `If`, `While`, `Break`,
and `Continue`; expressions are `IntLit`, `StrLit`, `BoolLit`, `Var`, `Add`,
`Binary`, `Call`, and scalar `IfExpr`. Arithmetic `%` and Boolean `and`/`or`
have distinct operations and type validation. A built-in print call accepts
zero or one string literal. String literals preserve their decoded UTF-8
contents, including quotes, backslashes, tabs, and newlines, as JSON escapes.
Bytes literals cannot be substituted for strings. The experimental lexer
rejects unknown escapes and embedded NUL escapes until it can preserve them.
Local calls with up to eight `Int` arguments and an `Int` or `Bool` return use the declared module signature,
including forward calls. The bridge checks argument count and type before
emitting an object. Calls requiring an unknown function signature are rejected
with `bridge.unsupported_ir`; no external signature is inferred.
The bridge additionally validates local integer and Boolean bindings, return types,
integer arithmetic, comparisons between integers (producing `Bool`),
Boolean `if`/`while` conditions, mutable assignments, branch-local scope,
and loop control placement. Undefined variables and variables outside the supported
scalar types and the explicitly typed list subset are rejected. The Ori client
preserves explicit primitive local annotations and rejects an annotation that
disagrees with its value. It can emit zero-argument
functions with up to eight `int` parameters; it preserves parameter
names, resolves them within each function, and accepts integer arguments for local
calls. Parameters on `main`, other parameter types, more than eight parameters,
unsupported return types, and statements absent from its emitter prevent
object emission.
The client preserves the operator in integer `+`, `-`, `*`, and `/` expressions
and recognizes `-> int` return signatures. The limited type check evaluates
return expressions against each function's own signature. It must refuse code generation for
source it cannot represent: interpolated strings, floats, unsupported argument
types, non-IO method calls, unknown body tokens, user-defined type declarations, top-level
constants, and incomplete function bodies. A `check` result does not imply
that this restricted code generation path supports the checked program.
Generic and qualified type signatures are preserved for declaration discovery
but cannot yet be lowered through the scalar bridge.
The current print path accepts `io.println` only when `io` is an import alias
for `ori.io`. The imported namespace takes precedence over a same-named local
binding; arbitrary receiver names and renamed aliases remain
outside the implemented bridge subset. For user modules with compatible
scalar functions, the client discovers the transitive graph, checks module
headers, detects cycles, validates public visibility, and includes their
definitions in the same bridge request. Generic and structural module
definitions still block native emission.

The existing `--request-file` path does not enforce the proposed 30-second
read timeout, 120-second compile timeout, 64 MiB frame limit, deterministic
serialization, or all schema fields illustrated above. Do not claim those
guards or full HIR compatibility based on the library framing tests.
