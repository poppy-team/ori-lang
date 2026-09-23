#!/usr/bin/env bash
set -euo pipefail

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
stage0=${ORI_STAGE0:-"$repo/compiler/target/debug/ori"}

if [[ ! -x "$stage0" ]]; then
    echo "Stage 0 compiler missing or not executable: $stage0" >&2
    exit 1
fi

# Keep the staged runtime and stdlib discoverable during the bootstrap.
cd "$repo"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
source_file="$repo/selfhost/compiler/main.orl"

echo 'Stage 0 -> Stage 1'
# The Stage 0 incremental cache can reuse an older output when an imported
# frontend module changes without touching main.orl. Bootstrap must compile
# the checked-out source tree, including imported compiler modules.
ORI_DISABLE_INCREMENTAL=1 "$stage0" compile "$source_file" -o "$work/ori-stage1"
test -x "$work/ori-stage1" || { echo 'Stage 0 did not emit Stage 1' >&2; exit 1; }

# Exercise inferred local bindings before attempting to compile the compiler.
cat > "$work/locals.orl" <<'ORI'
module bootstrap.locals
main()
    const original = 7
    var copy = original
    var names: list[string] = []
end
ORI
echo 'Stage 1: local binding smoke check'
"$work/ori-stage1" check "$work/locals.orl"

# Standard library namespaces can contain type keywords. Both the import and
# the imported module header must keep the full dotted path.
cat > "$work/keyword-import.orl" <<'ORI'
module bootstrap.keyword_import
import ori.list as lists
import ori.string as strings
main()
end
ORI
echo 'Stage 1: keyword-named stdlib module imports'
"$work/ori-stage1" check "$work/keyword-import.orl"

# An unresolved module must fail the check instead of making its alias a
# silently accepted name.
cat > "$work/missing-import.orl" <<'ORI'
module bootstrap.missing
import absent.bootstrap.module as missing
main()
    missing.run()
end
ORI
echo 'Stage 1: missing import must fail closed'
if "$work/ori-stage1" check "$work/missing-import.orl" > "$work/missing-import.log" 2>&1; then
    echo 'Stage 1 accepted a missing import' >&2
    exit 1
fi
if ! grep -q 'bind.import_not_found' "$work/missing-import.log"; then
    cat "$work/missing-import.log" >&2
    echo 'Stage 1 failed without diagnosing the missing import' >&2
    exit 1
fi

echo 'Stage 0/1: compile and run a shared program before self-compilation'
cat > "$work/hello-minimal.orl" <<'ORI'
module bootstrap.hello
import ori.io as io
main()
    io.println("Hello from Ori")
end
ORI
"$stage0" compile "$work/hello-minimal.orl" -o "$work/hello-stage0"
"$work/ori-stage1" compile "$work/hello-minimal.orl" -o "$work/hello-stage1"
test -x "$work/hello-stage0" && test -x "$work/hello-stage1"
"$work/hello-stage0" > "$work/hello-stage0.stdout"
"$work/hello-stage1" > "$work/hello-stage1.stdout"
cmp "$work/hello-stage0.stdout" "$work/hello-stage1.stdout"

# Preserve the operator and return signature in the bridge request. Previously
# the lexer discarded `->` and the flat expression pool serialized `*`/`-` as
# `+`, allowing a different program to pass a simple compile smoke check.
cat > "$work/arithmetic.orl" <<'ORI'
module bootstrap.arithmetic
main() -> int
    const product = 6 * 7
    return product - 42
end
ORI
echo 'Stage 0/1: integer operators and return signature'
"$stage0" compile "$work/arithmetic.orl" -o "$work/arithmetic-stage0"
"$work/ori-stage1" compile "$work/arithmetic.orl" -o "$work/arithmetic-stage1"
"$work/arithmetic-stage0"
"$work/arithmetic-stage1"
python3 - "$work/arithmetic-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    function = json.load(request_file)["module"]["funcs"][0]
assert function["return_ty"] == "Int", function
assert function["body_stmts"][0]["Let"]["value"]["Binary"]["op"] == "Mul", function
assert function["body_stmts"][1]["Return"]["Binary"]["op"] == "Sub", function
PY

# Each function has its own return signature; the old checker reused the
# return type of the first function for every statement in the source file.
cat > "$work/return-scope-ok.orl" <<'ORI'
module bootstrap.return_scope_ok
main() -> int
    return 0
end
is_ready() -> bool
    return true
end
ORI
cat > "$work/return-scope-bad.orl" <<'ORI'
module bootstrap.return_scope_bad
main() -> int
    return 0
end
is_ready() -> bool
    return 42
end
ORI
echo 'Stage 1: check return types per function'
"$work/ori-stage1" check "$work/return-scope-ok.orl"
if "$work/ori-stage1" check "$work/return-scope-bad.orl" > "$work/return-scope-bad.log" 2>&1; then
    echo 'Stage 1 accepted a mismatched return in a second function' >&2
    exit 1
fi
grep -q 'type.return_mismatch' "$work/return-scope-bad.log" || {
    cat "$work/return-scope-bad.log" >&2
    exit 1
}

cat > "$work/return-scope-call-ok.orl" <<'ORI'
module bootstrap.return_scope_call_ok
main()
end
ready_again() -> bool
    return is_ready()
end
is_ready() -> bool
    return true
end
ORI
echo 'Stage 0/1: valid forward boolean return types'
"$stage0" check "$work/return-scope-call-ok.orl"
"$work/ori-stage1" check "$work/return-scope-call-ok.orl"

cat > "$work/two-functions.orl" <<'ORI'
module bootstrap.two_functions
main()
end
answer() -> int
    return 42
end
ORI
echo 'Stage 0/1: compile a second function with its own return type'
"$stage0" compile "$work/two-functions.orl" -o "$work/two-functions-stage0"
"$work/ori-stage1" compile "$work/two-functions.orl" -o "$work/two-functions-stage1"
"$work/two-functions-stage0"
"$work/two-functions-stage1"

# Calls to a function declared later in the same module need its exact
# signature on both sides of the bridge. The exit code stays zero only when
# the function is actually called and its integer result reaches the caller.
cat > "$work/local-call.orl" <<'ORI'
module bootstrap.local_call
main() -> int
    const value = answer()
    return value - 42
end
answer() -> int
    return 42
end
ORI
echo 'Stage 0/1: forward call to a local integer function'
"$stage0" compile "$work/local-call.orl" -o "$work/local-call-stage0"
"$work/ori-stage1" compile "$work/local-call.orl" -o "$work/local-call-stage1"
"$work/local-call-stage0"
"$work/local-call-stage1"
python3 - "$work/local-call-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    module = json.load(request_file)["module"]
assert [f["name"] for f in module["funcs"]] == ["main", "answer"]
assert module["funcs"][0]["body_stmts"][0]["Let"]["value"] == {
    "Call": {"callee": "answer", "args": []}
}
assert module["funcs"][1]["return_ty"] == "Int"
PY

cat > "$work/boolean-call.orl" <<'ORI'
module bootstrap.boolean_call
import ori.io as io
main()
    ready_again()
end
ready_again() -> bool
    return is_ready()
end
is_ready() -> bool
    io.println("boolean call executed")
    return 7 == 7
end
ORI
echo 'Stage 0/1: forward boolean call and comparison'
"$stage0" compile "$work/boolean-call.orl" -o "$work/boolean-call-stage0"
"$work/ori-stage1" compile "$work/boolean-call.orl" -o "$work/boolean-call-stage1"
"$work/boolean-call-stage0" > "$work/boolean-call-stage0.stdout"
"$work/boolean-call-stage1" > "$work/boolean-call-stage1.stdout"
cmp "$work/boolean-call-stage0.stdout" "$work/boolean-call-stage1.stdout"
python3 - "$work/boolean-call-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
assert [f["return_ty"] for f in funcs] == ["Void", "Bool", "Bool"], funcs
assert funcs[1]["body_stmts"][0]["Return"] == {
    "Call": {"callee": "is_ready", "args": []}
}, funcs
assert funcs[2]["body_stmts"][1]["Return"]["Binary"]["op"] == "Eq", funcs
PY

# The imported namespace remains a module even when a local binding has the
# same spelling. Compare the actual output against the reference compiler.
cat > "$work/shadowed-io.orl" <<'ORI'
module bootstrap.shadowed_io
import ori.io as io
main()
    const io = 4
    io.println("module wins")
end
ORI
"$stage0" compile "$work/shadowed-io.orl" -o "$work/shadowed-io-stage0"
"$work/ori-stage1" compile "$work/shadowed-io.orl" -o "$work/shadowed-io-stage1"
"$work/shadowed-io-stage0" > "$work/shadowed-io-stage0.stdout"
"$work/shadowed-io-stage1" > "$work/shadowed-io-stage1.stdout"
cmp "$work/shadowed-io-stage0.stdout" "$work/shadowed-io-stage1.stdout"

# Every source construct that the flat IR cannot express must fail before
# invoking the bridge; otherwise a successful binary could change semantics.
assert_unsupported() {
    local name=$1
    if "$work/ori-stage1" compile "$work/$name.orl" -o "$work/$name.bin" > "$work/$name.log" 2>&1; then
        echo "Stage 1 silently compiled unsupported $name" >&2
        exit 1
    fi
    if ! grep -q 'bridge.unsupported_ir' "$work/$name.log"; then
        cat "$work/$name.log" >&2
        echo "Stage 1 failed without the unsupported IR diagnostic: $name" >&2
        exit 1
    fi
    test ! -e "$work/$name.bin" || { echo "Stage 1 emitted unsupported $name" >&2; exit 1; }
}
cat > "$work/interpolation.orl" <<'ORI'
module bootstrap.interpolation
import ori.io as io
main()
    const value = 42
    io.println(f"value: {value}")
end
ORI
cat > "$work/multiple-arguments.orl" <<'ORI'
module bootstrap.multiple_arguments
main()
    println("first", "second")
end
ORI
cat > "$work/struct-declaration.orl" <<'ORI'
module bootstrap.struct_declaration
struct Person
    name: string
end
main()
end
ORI
cat > "$work/false-print.orl" <<'ORI'
module bootstrap.false_print
import ori.io as io
main()
    const number = 4
    number.println("wrong")
end
ORI
cat > "$work/local-io.orl" <<'ORI'
module bootstrap.local_io
main()
    const io = 4
    io.println("wrong")
end
ORI
cat > "$work/unknown-body.orl" <<'ORI'
module bootstrap.unknown_body
main()
    @
end
ORI
cat > "$work/missing-end.orl" <<'ORI'
module bootstrap.missing_end
main()
    const value = 42
ORI
cat > "$work/unknown-call.orl" <<'ORI'
module bootstrap.unknown_call
main() -> int
    return nowhere()
end
ORI
cat > "$work/wrong-call-arity.orl" <<'ORI'
module bootstrap.wrong_call_arity
main() -> int
    return answer(3)
end
answer() -> int
    return 42
end
ORI
cat > "$work/wrong-call-return.orl" <<'ORI'
module bootstrap.wrong_call_return
main() -> int
    return no_value()
end
no_value()
end
ORI
cat > "$work/mistyped-local.orl" <<'ORI'
module bootstrap.mistyped_local
main() -> int
    const flag = true
    return flag
end
ORI
cat > "$work/mistyped-forward-call.orl" <<'ORI'
module bootstrap.mistyped_forward_call
main() -> int
    return is_ready()
end
is_ready() -> bool
    return true
end
ORI
cat > "$work/mistyped-comparison.orl" <<'ORI'
module bootstrap.mistyped_comparison
main() -> int
    return 7 == 7
end
ORI
cat > "$work/cross-function-binding.orl" <<'ORI'
module bootstrap.cross_function_binding
main() -> int
    return secret
end
helper() -> int
    const secret = 42
    return secret
end
ORI
cat > "$work/early-use.orl" <<'ORI'
module bootstrap.early_use
main() -> int
    const value = later
    const later = 42
    return value - 42
end
ORI
echo 'Stage 1: unsupported source must fail closed'
for name in interpolation multiple-arguments struct-declaration false-print local-io unknown-body missing-end unknown-call wrong-call-arity; do
    assert_unsupported "$name"
done

# Resolve local value types and forward function signatures when checking a
# return. These programs were previously accepted because variables had an
# unknown type and every call was guessed to return int.
echo 'Stage 1: wrong return types from locals, calls, and comparisons'
for name in wrong-call-return mistyped-local mistyped-forward-call mistyped-comparison; do
    if "$work/ori-stage1" check "$work/$name.orl" > "$work/$name.check.log" 2>&1; then
        echo "Stage 1 accepted a mismatched return: $name" >&2
        exit 1
    fi
    grep -q 'type.return_mismatch' "$work/$name.check.log" || {
        cat "$work/$name.check.log" >&2
        exit 1
    }
done

# A module-wide symbol table must not expose another function's local binding
# to check or compile. The reference frontend reports name.undefined here.
echo 'Stage 1: local bindings stay within their function and declaration order'
for name in cross-function-binding early-use; do
    if "$work/ori-stage1" check "$work/$name.orl" > "$work/$name.check.log" 2>&1; then
        echo "Stage 1 accepted an out-of-scope binding: $name" >&2
        exit 1
    fi
    grep -q 'name.undefined' "$work/$name.check.log" || {
        cat "$work/$name.check.log" >&2
        exit 1
    }
    if "$work/ori-stage1" compile "$work/$name.orl" -o "$work/$name.bin" > "$work/$name.compile.log" 2>&1; then
        echo "Stage 1 compiled an out-of-scope binding: $name" >&2
        exit 1
    fi
    grep -q 'name.undefined' "$work/$name.compile.log" || {
        cat "$work/$name.compile.log" >&2
        exit 1
    }
done

echo 'Stage 1 -> Stage 2 (must use Stage 1, not Stage 0)'
"$work/ori-stage1" compile "$source_file" -o "$work/ori-stage2"
test -x "$work/ori-stage2" || { echo 'Stage 1 did not emit Stage 2' >&2; exit 1; }

echo 'Stage 2 -> Stage 3 (must use Stage 2)'
"$work/ori-stage2" compile "$source_file" -o "$work/ori-stage3"
test -x "$work/ori-stage3" || { echo 'Stage 2 did not emit Stage 3' >&2; exit 1; }

# A binary comparison is useful evidence only after a real staged build.
sha256sum "$work/ori-stage2" "$work/ori-stage3"
cmp "$work/ori-stage2" "$work/ori-stage3" || {
    echo 'Stage 2 and Stage 3 binaries differ; classify the divergence' >&2
    exit 1
}

echo 'Stage 0/1/2: independent program checks'
for compiler in "$stage0" "$work/ori-stage1" "$work/ori-stage2"; do
    "$compiler" check "$repo/examples/hello/main.orl"
done

echo 'Stage 0/1/2: compile and execute the same independent program'
index=0
for compiler in "$stage0" "$work/ori-stage1" "$work/ori-stage2"; do
    "$compiler" compile "$repo/examples/hello/main.orl" -o "$work/hello-$index"
    test -x "$work/hello-$index" || { echo "Stage $index emitted no example binary" >&2; exit 1; }
    "$work/hello-$index" > "$work/hello-$index.stdout"
    index=$((index + 1))
done
cmp "$work/hello-0.stdout" "$work/hello-1.stdout"
cmp "$work/hello-0.stdout" "$work/hello-2.stdout"

echo 'Real Stage 0 -> 1 -> 2 -> 3 bootstrap verified'
