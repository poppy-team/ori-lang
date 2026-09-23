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
"$stage0" compile "$source_file" -o "$work/ori-stage1"
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
echo 'Stage 1: unsupported source must fail closed'
for name in interpolation multiple-arguments struct-declaration false-print unknown-body missing-end; do
    assert_unsupported "$name"
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
