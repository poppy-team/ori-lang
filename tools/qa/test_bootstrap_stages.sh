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
