#!/usr/bin/env sh
# Ori language QA — fast daily stages (S0–S4 + S8).
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
if [ -f "$repo/compiler/Cargo.toml" ]; then
  comp="$repo/compiler"
elif [ -f "$repo/Cargo.toml" ]; then
  comp="$repo"
else
  echo "cannot find Ori workspace from $repo" >&2
  exit 2
fi
echo "== D0 documentation Atlas =="
"$repo/tools/qa/docs_coverage.sh"
echo "== D0 reproducible archive metadata =="
"$repo/tools/qa/archive_repro_smoke.sh"
echo "== D0 runtime ABI exports =="
"$repo/tools/qa/abi_exports.sh"
echo "== S0 scoped rustfmt baseline =="
python3 "$repo/tools/qa/rustfmt_scoped.py"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
cd "$comp"
echo "== S0 cargo check --workspace =="
cargo check --workspace --locked
echo "== S0 strict clippy --workspace --all-targets --all-features =="
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
echo "== S1 unit crates =="
cargo test -p ori-lexer --locked -- --quiet
cargo test -p ori-parser --locked -- --quiet
cargo test -p ori-types --locked -- --quiet
cargo test -p ori-hir --locked -- --quiet
echo "== S2 ori_spec + diagnostic_catalog =="
cargo test -p ori-driver --test ori_spec --locked -- --quiet
cargo test -p ori-driver --test diagnostic_catalog --locked -- --quiet
echo "== S3 memory + security =="
cargo test -p ori-driver --test memory_arc --locked -- --quiet
cargo test -p ori-driver --test security_robustness --locked -- --quiet
echo "== S8 residual product surface =="
cargo test -p ori-driver --test concurrency_async compile_runs_lang_res_product_surface_native --locked -- --quiet
echo "== S9 selfhost native compiler & bridge =="
cargo test -p ori-bridge-server --locked -- --quiet
"$repo/tools/qa/test_selfhost_complete.sh"
echo "daily_fast: OK"
