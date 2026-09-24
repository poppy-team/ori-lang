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

cat > "$work/escaped-print.orl" <<'ORI'
module bootstrap.escaped_print
import ori.io as io
main()
    io.println("\"wrapped\"")
    io.println("line one\nline two")
    io.println("café\t開発")
end
ORI
echo 'Stage 0/1: UTF-8 and escaped string contents in the JSON request'
"$stage0" compile "$work/escaped-print.orl" -o "$work/escaped-print-stage0"
"$work/ori-stage1" compile "$work/escaped-print.orl" -o "$work/escaped-print-stage1"
"$work/escaped-print-stage0" > "$work/escaped-print-stage0.stdout"
"$work/escaped-print-stage1" > "$work/escaped-print-stage1.stdout"
cmp "$work/escaped-print-stage0.stdout" "$work/escaped-print-stage1.stdout"
python3 - "$work/escaped-print-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
strings = [stmt["Expr"]["Call"]["args"][0]["StrLit"] for stmt in body]
assert strings == ['"wrapped"', "line one\nline two", "café\t開発"], strings
PY

cat > "$work/string-flow.orl" <<'ORI'
module bootstrap.string_flow
import ori.io as io
main()
    var message: string = "first"
    io.println(message)
    message = label(2)
    io.println(message)
    io.println(label(1))
end
label(n: int) -> string
    return if n == 1 then "café" else "second"
end
ORI
echo 'Stage 0/1: string locals, mutation, and typed call results'
"$stage0" compile "$work/string-flow.orl" -o "$work/string-flow-stage0"
"$work/ori-stage1" compile "$work/string-flow.orl" -o "$work/string-flow-stage1"
"$work/string-flow-stage0" > "$work/string-flow-stage0.stdout"
"$work/string-flow-stage1" > "$work/string-flow-stage1.stdout"
cmp "$work/string-flow-stage0.stdout" "$work/string-flow-stage1.stdout"
python3 - "$work/string-flow-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
assert funcs[0]["body_stmts"][0]["Let"]["ty"] == "String", funcs
assert funcs[0]["body_stmts"][2]["Assign"]["value"] == {
    "Call": {"callee": "label", "args": [{"IntLit": 2}]}
}, funcs
assert funcs[0]["body_stmts"][3]["Expr"]["Call"]["args"] == [
    {"Var": "message"}
], funcs
assert funcs[1]["return_ty"] == "String", funcs
PY

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

cat > "$work/typed-parameter.orl" <<'ORI'
module bootstrap.typed_parameter
import ori.io as io
main() -> int
    const value = answer(42)
    ready(value)
    io.println("typed parameter executed")
    return value - 42
end
answer(value: int) -> int
    return value
end
ready(value: int) -> bool
    return value == 42
end
ORI
echo 'Stage 0/1: forward integer and boolean calls with typed parameters'
"$stage0" compile "$work/typed-parameter.orl" -o "$work/typed-parameter-stage0"
"$work/ori-stage1" compile "$work/typed-parameter.orl" -o "$work/typed-parameter-stage1"
"$work/typed-parameter-stage0" > "$work/typed-parameter-stage0.stdout"
"$work/typed-parameter-stage1" > "$work/typed-parameter-stage1.stdout"
cmp "$work/typed-parameter-stage0.stdout" "$work/typed-parameter-stage1.stdout"
python3 - "$work/typed-parameter-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
assert [f["params"] for f in funcs] == [
    [], [{"name": "value", "ty": "Int"}], [{"name": "value", "ty": "Int"}]
], funcs
assert funcs[0]["body_stmts"][0]["Let"]["value"] == {
    "Call": {"callee": "answer", "args": [{"IntLit": 42}]}
}, funcs
assert funcs[0]["body_stmts"][1]["Expr"] == {
    "Call": {"callee": "ready", "args": [{"Var": "value"}]}
}, funcs
assert funcs[1]["body_stmts"][0]["Return"] == {"Var": "value"}, funcs
assert funcs[2]["return_ty"] == "Bool", funcs
PY

cat > "$work/boolean-local.orl" <<'ORI'
module bootstrap.boolean_local
import ori.io as io
main()
    const outcome: bool = ready()
    const another = outcome
    another
    io.println("boolean local executed")
end
ready() -> bool
    const good: bool = 6 == 6
    return good
end
ORI
echo 'Stage 0/1: boolean locals retain their type through native codegen'
"$stage0" compile "$work/boolean-local.orl" -o "$work/boolean-local-stage0"
"$work/ori-stage1" compile "$work/boolean-local.orl" -o "$work/boolean-local-stage1"
"$work/boolean-local-stage0" > "$work/boolean-local-stage0.stdout"
"$work/boolean-local-stage1" > "$work/boolean-local-stage1.stdout"
cmp "$work/boolean-local-stage0.stdout" "$work/boolean-local-stage1.stdout"
python3 - "$work/boolean-local-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
assert funcs[0]["body_stmts"][0]["Let"]["ty"] == "Bool", funcs
assert funcs[0]["body_stmts"][1]["Let"] == {
    "name": "another", "ty": "Bool", "value": {"Var": "outcome"}, "mutable": False
}, funcs
assert funcs[1]["body_stmts"][0]["Let"]["ty"] == "Bool", funcs
assert funcs[1]["body_stmts"][1]["Return"] == {"Var": "good"}, funcs
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

# Nested statements must survive parsing and be executed by the native
# backend. The intermediate request also proves that both branches remain.
cat > "$work/control-flow.orl" <<'ORI'
module bootstrap.control_flow
main() -> int
    var value = 0
    while value < 4
        if value == 2
            value = value + 2
        else
            value = value + 1
        end
    end
    return value - 4
end
ORI
echo 'Stage 0/1: nested if/else, while, mutable assignment'
"$stage0" compile "$work/control-flow.orl" -o "$work/control-flow-stage0"
"$work/ori-stage1" compile "$work/control-flow.orl" -o "$work/control-flow-stage1"
timeout 10s "$work/control-flow-stage0"
timeout 10s "$work/control-flow-stage1"
python3 - "$work/control-flow-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
assert body[0]["Let"]["mutable"] is True, body
loop = body[1]["While"]
assert loop["cond"]["Binary"]["op"] == "Lt", loop
conditional = loop["body_stmts"][0]["If"]
assert conditional["then_stmts"][0]["Assign"]["name"] == "value", conditional
assert conditional["else_stmts"][0]["Assign"]["name"] == "value", conditional
PY

cat > "$work/scalar-match.orl" <<'ORI'
module bootstrap.scalar_match
import ori.io as io
main()
    var value = 0
    while value < 3
        match value
        case 0:
            io.println("zero")
        case 1:
            io.println("one")
        case else:
            io.println("other")
        end
        value = value + 1
    end
    match true
    case true:
        io.println("ready")
    case false:
        io.println("wait")
    end
end
ORI
echo 'Stage 0/1: integer and Boolean match arms inside a loop'
"$stage0" compile "$work/scalar-match.orl" -o "$work/scalar-match-stage0"
"$work/ori-stage1" compile "$work/scalar-match.orl" -o "$work/scalar-match-stage1"
timeout 10s "$work/scalar-match-stage0" > "$work/scalar-match-stage0.stdout"
timeout 10s "$work/scalar-match-stage1" > "$work/scalar-match-stage1.stdout"
cmp "$work/scalar-match-stage0.stdout" "$work/scalar-match-stage1.stdout"
python3 - "$work/scalar-match-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
arms = body[1]["While"]["body_stmts"][0]["Match"]["arms"]
assert [arm["pattern"] for arm in arms] == [
    {"IntLit": 0}, {"IntLit": 1}, "Wildcard"
], arms
assert [arm["pattern"] for arm in body[2]["Match"]["arms"]] == [
    {"BoolLit": True}, {"BoolLit": False}
], body
PY

# An integer match without a fallback must not become a partial executable.
cat > "$work/nonexhaustive-match.orl" <<'ORI'
module bootstrap.nonexhaustive_match
main()
    match 1
    case 1:
        return
    end
end
ORI
if "$work/ori-stage1" compile "$work/nonexhaustive-match.orl" -o "$work/nonexhaustive-match-stage1" > "$work/nonexhaustive-match.log" 2>&1; then
    echo 'Stage 1 compiled a non-exhaustive integer match' >&2
    exit 1
fi

cat > "$work/elif-chain.orl" <<'ORI'
module bootstrap.elif_chain
import ori.io as io
main() -> int
    var value = 0
    while value < 4
        if value == 0
            io.println("first")
            value = value + 1
        elif value == 1
            io.println("second")
            value = value + 1
        elif value == 2
            io.println("third")
            value = value + 1
        else
            io.println("last")
            value = value + 1
        end
    end
    return value - 4
end
ORI
echo 'Stage 0/1: elif chain in a nested loop'
"$stage0" compile "$work/elif-chain.orl" -o "$work/elif-chain-stage0"
"$work/ori-stage1" compile "$work/elif-chain.orl" -o "$work/elif-chain-stage1"
timeout 10s "$work/elif-chain-stage0" > "$work/elif-chain-stage0.stdout"
timeout 10s "$work/elif-chain-stage1" > "$work/elif-chain-stage1.stdout"
cmp "$work/elif-chain-stage0.stdout" "$work/elif-chain-stage1.stdout"
python3 - "$work/elif-chain-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
branch = body[1]["While"]["body_stmts"][0]["If"]
for name in ("second", "third"):
    nested = branch["else_stmts"]
    assert len(nested) == 1 and "If" in nested[0], nested
    branch = nested[0]["If"]
    assert branch["then_stmts"][0]["Expr"]["Call"]["args"] == [{"StrLit": name}], branch
assert len(branch["else_stmts"]) == 2, branch
PY

cat > "$work/unary-expr.orl" <<'ORI'
module bootstrap.unary_expr
import ori.io as io
main() -> int
    const value = -(2 + 3)
    const ready = not (value != -5)
    if ready
        io.println("unary operators executed")
    end
    return value + 5
end
ORI
echo 'Stage 0/1: unary minus and Boolean not'
"$stage0" compile "$work/unary-expr.orl" -o "$work/unary-expr-stage0"
"$work/ori-stage1" compile "$work/unary-expr.orl" -o "$work/unary-expr-stage1"
"$work/unary-expr-stage0" > "$work/unary-expr-stage0.stdout"
"$work/unary-expr-stage1" > "$work/unary-expr-stage1.stdout"
cmp "$work/unary-expr-stage0.stdout" "$work/unary-expr-stage1.stdout"
python3 - "$work/unary-expr-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
negated = body[0]["Let"]["value"]["Binary"]
assert negated["op"] == "Sub" and negated["left"] == {"IntLit": 0}, negated
inverted = body[1]["Let"]["value"]["IfExpr"]
assert inverted["then_expr"] == {"BoolLit": False}, inverted
assert inverted["else_expr"] == {"BoolLit": True}, inverted
PY

cat > "$work/multi-arg-expr.orl" <<'ORI'
module bootstrap.multi_arg_expr
add(a: int, b: int) -> int
    return a + b
end
main() -> int
    return add(6 * 7, (1 + 1)) - 44
end
ORI
echo 'Stage 0/1: multiple arguments and grouped expressions'
"$stage0" compile "$work/multi-arg-expr.orl" -o "$work/multi-arg-expr-stage0"
"$work/ori-stage1" compile "$work/multi-arg-expr.orl" -o "$work/multi-arg-expr-stage1"
"$work/multi-arg-expr-stage0"
"$work/multi-arg-expr-stage1"
python3 - "$work/multi-arg-expr-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
args = funcs[1]["body_stmts"][0]["Return"]["Binary"]["left"]["Call"]["args"]
assert len(args) == 2 and args[0]["Binary"]["op"] == "Mul", funcs
assert args[1]["Add"] == [{"IntLit": 1}, {"IntLit": 1}], funcs
PY

echo 'Stage 0/1: transitive imported definitions and public visibility'
"$stage0" compile "$repo/tests/fixtures/selfhost_modules/root.orl" -o "$work/import-root-stage0"
"$work/ori-stage1" compile "$repo/tests/fixtures/selfhost_modules/root.orl" -o "$work/import-root-stage1"
"$work/import-root-stage0"
"$work/import-root-stage1"
python3 - "$work/import-root-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
assert [f["name"] for f in funcs] == [
    "main",
    "tests.fixtures.selfhost_modules.middle.compute",
    "tests.fixtures.selfhost_modules.leaf.twice",
    "tests.fixtures.selfhost_modules.leaf.hidden",
], funcs
assert funcs[1]["body_stmts"][0]["Return"]["Add"][0]["Call"]["callee"] == funcs[2]["name"], funcs
PY

echo 'Stage 0/1: match inside a transitive imported definition'
"$stage0" compile "$repo/tests/fixtures/selfhost_modules/match_root.orl" -o "$work/import-match-stage0"
"$work/ori-stage1" compile "$repo/tests/fixtures/selfhost_modules/match_root.orl" -o "$work/import-match-stage1"
"$work/import-match-stage0" > "$work/import-match-stage0.stdout"
"$work/import-match-stage1" > "$work/import-match-stage1.stdout"
cmp "$work/import-match-stage0.stdout" "$work/import-match-stage1.stdout"
python3 - "$work/import-match-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    funcs = json.load(request_file)["module"]["funcs"]
assert [f["name"] for f in funcs] == [
    "main",
    "tests.fixtures.selfhost_modules.match_middle.value",
    "tests.fixtures.selfhost_modules.match_middle.greet",
    "tests.fixtures.selfhost_modules.match_leaf.choose",
    "tests.fixtures.selfhost_modules.match_leaf.greeting",
], funcs
assert funcs[3]["body_stmts"][0]["Match"]["arms"][1]["pattern"] == "Wildcard", funcs
assert funcs[4]["return_ty"] == "String", funcs
PY

for case in cycle_root missing_leaf private_member; do
    if "$work/ori-stage1" check "$repo/tests/fixtures/selfhost_modules/$case.orl" > "$work/$case.log" 2>&1; then
        echo "Stage 1 accepted invalid import graph: $case" >&2
        exit 1
    fi
done
grep -q 'bind.import_cycle' "$work/cycle_root.log"
grep -q 'bind.import_not_found' "$work/missing_leaf.log"
grep -q 'bind.private_import' "$work/private_member.log"

# A generic signature and an inline if-expression have no block End of their
# own. Recovery must still see the next function and refuse unsupported IR.
cat > "$work/generic-signature.orl" <<'ORI'
module bootstrap.generic_signature
helper(values: list[int]) -> int
    const choice = if true then 1 else 0
    return choice
end
main() -> int
    return 0
end
ORI
if "$work/ori-stage1" compile "$work/generic-signature.orl" -o "$work/generic-signature.bin" > "$work/generic-signature.log" 2>&1; then
    echo 'Stage 1 emitted an unsupported generic signature' >&2
    exit 1
fi
grep -q 'PIPELINE_PARSED: items=2' "$work/generic-signature.log"
grep -q 'bridge.unsupported_ir' "$work/generic-signature.log"

cat > "$work/inline-if.orl" <<'ORI'
module bootstrap.inline_if
main() -> int
    const ready = 2 > 1
    const answer = if ready then 42 else 0
    return answer - 42
end
ORI
echo 'Stage 0/1: inline if-expression with a boolean local'
"$stage0" compile "$work/inline-if.orl" -o "$work/inline-if-stage0"
"$work/ori-stage1" compile "$work/inline-if.orl" -o "$work/inline-if-stage1"
"$work/inline-if-stage0"
"$work/inline-if-stage1"
python3 - "$work/inline-if-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
assert body[1]["Let"]["value"]["IfExpr"] == {
    "cond": {"Var": "ready"},
    "then_expr": {"IntLit": 42},
    "else_expr": {"IntLit": 0},
}, body
PY

cat > "$work/logical-mod.orl" <<'ORI'
module bootstrap.logical_mod
main() -> int
    const ready = (7 % 3 == 1) and (4 > 3)
    return if ready or false then 0 else 1
end
ORI
echo 'Stage 0/1: modulo and Boolean operators in a grouped expression'
"$stage0" compile "$work/logical-mod.orl" -o "$work/logical-mod-stage0"
"$work/ori-stage1" compile "$work/logical-mod.orl" -o "$work/logical-mod-stage1"
"$work/logical-mod-stage0"
"$work/logical-mod-stage1"
python3 - "$work/logical-mod-stage1.tmp.o.req.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    body = json.load(request_file)["module"]["funcs"][0]["body_stmts"]
expr = body[0]["Let"]["value"]["Binary"]
assert expr["op"] == "And", body
assert expr["left"]["Binary"]["left"]["Binary"]["op"] == "Mod", body
assert body[1]["Return"]["IfExpr"]["cond"]["Binary"]["op"] == "Or", body
PY

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
cat > "$work/bytes-literal.orl" <<'ORI'
module bootstrap.bytes_literal
import ori.io as io
main()
    io.println(b"bytes are not strings")
end
ORI
cat > "$work/invalid-escape.orl" <<'ORI'
module bootstrap.invalid_escape
import ori.io as io
main()
    io.println("bad\q")
end
ORI
cat > "$work/nul-literal.orl" <<'ORI'
module bootstrap.nul_literal
import ori.io as io
main()
    io.println("zero\0end")
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
cat > "$work/invalid-byte.orl" <<'ORI'
module bootstrap.invalid_byte
main() -> int
    return 4 # 2
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
cat > "$work/missing-parameter-argument.orl" <<'ORI'
module bootstrap.missing_parameter_argument
main() -> int
    return answer()
end
answer(value: int) -> int
    return value
end
ORI
cat > "$work/mistyped-parameter-argument.orl" <<'ORI'
module bootstrap.mistyped_parameter_argument
main() -> int
    return answer(true)
end
answer(value: int) -> int
    return value
end
ORI
cat > "$work/unsupported-parameter-type.orl" <<'ORI'
module bootstrap.unsupported_parameter_type
main()
end
identity(value: bool) -> bool
    return value
end
ORI
cat > "$work/missing-parameter-colon.orl" <<'ORI'
module bootstrap.missing_parameter_colon
main() -> int
    return answer(42)
end
answer(value int) -> int
    return value
end
ORI
cat > "$work/leading-call-comma.orl" <<'ORI'
module bootstrap.leading_call_comma
main() -> int
    return answer(, 42)
end
answer(value: int) -> int
    return value
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
cat > "$work/mistyped-binding-annotation.orl" <<'ORI'
module bootstrap.mistyped_binding_annotation
main()
    const flag: bool = 42
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
cat > "$work/cross-function-parameter.orl" <<'ORI'
module bootstrap.cross_function_parameter
main() -> int
    return secret
end
helper(secret: int) -> int
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
for name in interpolation bytes-literal multiple-arguments struct-declaration false-print local-io unknown-body invalid-byte missing-end unknown-call wrong-call-arity missing-parameter-argument mistyped-parameter-argument unsupported-parameter-type missing-parameter-colon leading-call-comma; do
    assert_unsupported "$name"
done
for name in invalid-escape nul-literal; do
    if "$work/ori-stage1" compile "$work/$name.orl" -o "$work/$name.bin" > "$work/$name.log" 2>&1; then
        echo "Stage 1 silently compiled a malformed or unsupported string: $name" >&2
        exit 1
    fi
    test ! -e "$work/$name.bin"
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

echo 'Stage 1: a local type annotation must match the expression'
if "$work/ori-stage1" check "$work/mistyped-binding-annotation.orl" > "$work/mistyped-binding-annotation.check.log" 2>&1; then
    echo 'Stage 1 accepted a mismatched local type annotation' >&2
    exit 1
fi
grep -q 'type.type_mismatch' "$work/mistyped-binding-annotation.check.log" || {
    cat "$work/mistyped-binding-annotation.check.log" >&2
    exit 1
}

# A module-wide symbol table must not expose another function's local binding
# to check or compile. The reference frontend reports name.undefined here.
echo 'Stage 1: local bindings stay within their function and declaration order'
for name in cross-function-binding cross-function-parameter early-use; do
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
