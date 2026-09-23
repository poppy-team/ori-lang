//! E2E tests for Etapa 5 of the maturity plan: runtime ARC, type-specific
//! destructors, cycle collector and the `ORI_TEST_LEAK_CHECK` leak-check
//! convention. These tests compile small Ori programs to the native backend
//! and assert on stdout (and on the leak-check hook output) so regressions in
//! memory management surface as test failures.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::pipeline::run_compile;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);
const REQUIRE_RUNTIME_VALGRIND_ENV: &str = "ORI_REQUIRE_RUNTIME_VALGRIND";
const RUNTIME_VALGRIND_ENV: &str = "ORI_RUNTIME_VALGRIND";

const PARALLEL_OWNERSHIP_SOURCE: &str = r#"module app.main

import ori.channel = channel
import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.test = test

make_list(n: int) -> list[int]
    const values: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(values, i)
        i = i + 1
    end
    return values
end

exercise() -> int
    const shared: list[int] = make_list(3)

    var slots: list[list[int]] = lists.new()
    lists.push(slots, shared)
    lists.push(slots, shared)
    lists.remove(slots, 0)
    lists.push(slots[0], 9)

    var aliases: map[string, list[int]] = maps.new()
    maps.set(aliases, "left", shared)
    maps.set(aliases, "right", shared)
    maps.remove(aliases, "left")

    const work: channel.Channel[list[int]] = channel.create()
    channel.send(work, shared)
    channel.send(work, shared)
    channel.close(work)
    return lists.len(maps.get(aliases, "right"))
end

main()
    const score: int = exercise()
    const leaked: int = test.assert_no_leaks("parallel_ownership_matrix")
    io.print("score:" + string(score))
    io.print("leaks:" + string(leaked))
end
"#;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_memory_arc_test_{}_{}_{}",
            std::process::id(),
            id,
            name,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.path(name), source).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

/// Build a small Ori program, run it, and return its trimmed stdout.
fn compile_and_run(dir: &TestDir, source: &str, exe_name: &str) -> (String, String, bool) {
    dir.write("main.orl", source);
    let exe = exe_path(dir, exe_name);
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "compile errors: {:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();
    (stdout, stderr, success)
}

/// Build a small Ori program, run it with `ORI_TEST_LEAK_CHECK=1` set, and
/// return stdout, stderr, and the exit status.
fn compile_and_run_with_leak_check(
    dir: &TestDir,
    source: &str,
    exe_name: &str,
) -> (String, String, bool) {
    dir.write("main.orl", source);
    let exe = exe_path(dir, exe_name);
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "compile errors: {:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();
    (stdout, stderr, success)
}

fn run_jit_with_leak_check(dir: &TestDir, source: &str) -> (String, String, bool) {
    dir.write("main.orl", source);
    let output = Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .expect("failed to spawn `ori run` for JIT memory regression");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

fn find_valgrind() -> Result<std::ffi::OsString, String> {
    let candidate = std::env::var_os(RUNTIME_VALGRIND_ENV)
        .unwrap_or_else(|| std::ffi::OsString::from("valgrind"));
    let probe = Command::new(&candidate)
        .arg("--version")
        .output()
        .map_err(|error| format!("could not execute {candidate:?}: {error}"))?;
    if probe.status.success() {
        Ok(candidate)
    } else {
        Err(format!(
            "{candidate:?} --version exited with {}: {}",
            probe.status,
            String::from_utf8_lossy(&probe.stderr).trim()
        ))
    }
}

// ── 5.1 — Destrutores tipo-específicos ─────────────────────────────────────

/// Baseline: a program with no managed allocations should report zero live
/// allocations. This verifies the leak-check plumbing itself is sound before
/// we test destructors.
#[test]
fn compile_runs_native_no_managed_no_leak() {
    let dir = TestDir::new("no_managed_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

main()
    io.print("hello")
    const leaked: int = test.live_allocations()
    io.print("leaks:" + string(leaked))
end
"#,
        "no_managed_no_leak",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "hello\nleaks:0");
}

#[test]
fn compile_runs_custom_destructor_before_field_cleanup() {
    let dir = TestDir::new("custom_destructor");
    let (stdout, stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.core = core

var destroyed_total: int = 0

struct Resource
    id: int
    label: string
end

enum ResourceState
    Open(id: int, label: string)
    Closed
end

apply Resource use core.Destructor
    mut destroy(self)
        destroyed_total = destroyed_total + self.id
        io.println("destroy:" + self.label)
    end
end

apply ResourceState use core.Destructor
    mut destroy(self)
        match self
            case Open(id, label):
                destroyed_total = destroyed_total + id
                io.println("destroy-state:" + label)
            case Closed:
                io.println("destroy-state:closed")
        end
    end
end

consume()
    const resource: Resource = Resource { id: 7, label: "socket" + "-1" }
    const state: ResourceState = ResourceState.Open(id: 3, label: "socket" + "-2")
end

main()
    consume()
    io.println(f"{destroyed_total}")
end
"#,
        "custom_destructor",
    );
    assert!(success, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "destroy-state:socket-2\ndestroy:socket-1\n10\n");
}

/// A single list created and dropped in a helper function should leave no
/// live allocations. The list is created inside `exercise_list`, which
/// returns only an `int` (non-managed), so the list is released by scope
/// cleanup when the helper returns. `main` then checks live allocations
/// before any string-concat temporary is created.
#[test]
fn compile_runs_native_single_list_no_leak() {
    let dir = TestDir::new("single_list_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

exercise_list() -> int
    const xs: list[int] = lists.new()
    lists.push(xs, 1)
    lists.push(xs, 2)
    const n: int = lists.len(xs)
    return n
end

main()
    const n: int = exercise_list()
    const leaked: int = test.live_allocations()
    io.print("size:" + string(n))
    io.print("leaks:" + string(leaked))
end
"#,
        "single_list_no_leak",
    );
    assert!(success, "{stdout}");
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.first(), Some(&"size:2"));
    let leaks_line = lines
        .iter()
        .find(|l| l.starts_with("leaks:"))
        .unwrap_or_else(|| panic!("missing leaks line: {stdout}"));
    let leaks_n: i64 = leaks_line
        .trim_start_matches("leaks:")
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        leaks_n, 0,
        "expected 0 leaks for single list, got {leaks_n}: {stdout}"
    );
}

/// A struct holding a managed `list[int]` field must release the list when the
/// struct goes out of scope. The struct is created and consumed inside
/// `measure_holder`, which returns only an `int`, so the struct and its list
/// are released by scope cleanup before `main` checks for leaks.
#[test]
fn compile_runs_native_struct_with_managed_field_no_leak() {
    let dir = TestDir::new("struct_managed_field_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Holder
    items: list[int]
end

make_holder(n: int) -> Holder
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return Holder {items: xs}
end

measure_holder(n: int) -> int
    const h: Holder = make_holder(n)
    return lists.len(h.items)
end

main()
    const size: int = measure_holder(4)
    const leaked: int = test.assert_no_leaks("main_struct_managed")
    io.print("size:" + string(size))
    io.print("leaks:" + string(leaked))
end
"#,
        "struct_managed_no_leak",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "size:4\nleaks:0");
}

/// Nested structs (struct holding struct holding managed list) must cascade
/// releases through the generated destructors. The branch is created and
/// consumed inside `measure_branch`, which returns only an `int`.
#[test]
fn compile_runs_native_nested_struct_arc_cascade_no_leak() {
    let dir = TestDir::new("nested_struct_cascade_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Leaf
    data: list[int]
end

struct Branch
    leaf: Leaf
end

make_branch(n: int) -> Branch
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i * i)
        i = i + 1
    end
    return Branch { leaf: Leaf { data: xs } }
end

measure_branch(n: int) -> int
    const b: Branch = make_branch(n)
    return lists.len(b.leaf.data)
end

main()
    const size: int = measure_branch(3)
    const leaked: int = test.assert_no_leaks("main_nested_struct")
    io.print("leaf-size:" + string(size))
    io.print("leaks:" + string(leaked))
end
"#,
        "nested_struct_cascade",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "leaf-size:3\nleaks:0");
}

/// An enum variant carrying a managed payload must release the payload when
/// the enum value goes out of scope. The enum is created and consumed inside
/// `measure_polygon`, which returns only an `int`.
#[test]
fn compile_runs_native_enum_with_managed_payload_no_leak() {
    let dir = TestDir::new("enum_managed_payload_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

enum Shape
    Empty
    Polygon(points: list[int])
end

make_polygon(n: int) -> Shape
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return Shape.Polygon(points: xs)
end

measure_polygon(n: int) -> int
    const s: Shape = make_polygon(n)
    match s
        case Empty:
            return 0
        case Polygon(points):
            return lists.len(points)
    end
    return 0
end

main()
    const size: int = measure_polygon(5)
    const leaked: int = test.assert_no_leaks("main_enum_payload")
    io.print("polygon:" + string(size))
    io.print("leaks:" + string(leaked))
end
"#,
        "enum_managed_payload",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "polygon:5\nleaks:0");
}

/// Optional holding a managed struct must release the inner allocation when
/// the optional goes out of scope. The optional is created and consumed inside
/// `measure_box`, which returns only an `int`.
#[test]
fn compile_runs_native_optional_struct_no_leak() {
    let dir = TestDir::new("optional_struct_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Box
    items: list[int]
end

maybe_box(n: int) -> optional[Box]
    if n <= 0
        return none
    end
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return some(Box {items: xs})
end

measure_box(n: int) -> int
    const opt: optional[Box] = maybe_box(n)
    match opt
        case some(box):
            return lists.len(box.items)
        case none:
            return 0
    end
    return 0
end

main()
    const size: int = measure_box(3)
    const leaked: int = test.assert_no_leaks("main_optional_struct")
    io.print("box:" + string(size))
    io.print("leaks:" + string(leaked))
end
"#,
        "optional_struct_no_leak",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "box:3\nleaks:0");
}

// ── 5.2 — Cycle collector ──────────────────────────────────────────────────

/// The cycle collector runs and reports a non-negative count. This verifies
/// the `ori.test.collect_cycles` plumbing works end-to-end through the native
/// backend. The cycle collector itself is tested extensively in
/// `ori-runtime/src/tests.rs` (4 cycle scenarios).
///
/// Note: this test does NOT assert zero leaks because the codegen currently
/// over-retains managed values (see Etapa 5 known issues). It only verifies
/// the collector runs and returns a count.
#[test]
fn compile_runs_native_cycle_collector_runs() {
    let dir = TestDir::new("cycle_collector_runs");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

main()
    const xs: list[int] = lists.new()
    lists.push(xs, 1)
    lists.push(xs, 2)
    io.print("size:" + string(lists.len(xs)))
    const collected: int = test.collect_cycles()
    io.print("collected:" + string(collected))
end
"#,
        "cycle_collector_runs",
    );
    assert!(success, "{stdout}");
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.first(), Some(&"size:2"));
    assert!(
        lines.iter().any(|l| l.starts_with("collected:")),
        "missing collected line: {stdout}"
    );
}

/// Two Peer structs whose `link` fields reference each other's backing lists
/// must be fully released when the creating function returns. The cycle
/// collector runs in `main` after the function returns; no live allocations
/// should remain.
#[test]
fn compile_runs_native_orphan_cycle_reclaimed() {
    let dir = TestDir::new("orphan_cycle_reclaimed");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Peer
    id: int
    link: list[int]
end

build_and_measure() -> int
    const a_link: list[int] = lists.new()
    const b_link: list[int] = lists.new()
    lists.push(a_link, 10)
    lists.push(b_link, 20)
    const a: Peer = Peer {id: 1, link: b_link}
    const b: Peer = Peer {id: 2, link: a_link}
    return a.id + b.id
end

main()
    const total: int = build_and_measure()
    const collected: int = test.collect_cycles()
    const leaked: int = test.live_allocations()
    io.print("total:" + string(total))
    io.print("collected:" + string(collected))
    io.print("leaks:" + string(leaked))
end
"#,
        "orphan_cycle_reclaimed",
    );
    assert!(success, "{stdout}");
    let lines = stdout.lines().collect::<Vec<_>>();
    assert!(
        lines.iter().any(|l| l.starts_with("total:3")),
        "missing total:3 line: {stdout}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("collected:")),
        "missing collected line: {stdout}"
    );
    let leaks_line = lines
        .iter()
        .find(|l| l.starts_with("leaks:"))
        .unwrap_or_else(|| panic!("missing leaks line: {stdout}"));
    assert_eq!(leaks_line.trim(), "leaks:0");
}

/// A long chain of managed allocations created and dropped in a loop must not
/// accumulate live allocations. Each iteration's list is released by the loop
/// body's scope cleanup. The leak check runs before any string-concat
/// temporary is created.
#[test]
fn compile_runs_native_loop_managed_allocations_no_leak() {
    let dir = TestDir::new("loop_managed_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

main()
    var i: int = 0
    var total: int = 0
    while i < 100
        const xs: list[int] = lists.new()
        lists.push(xs, i)
        lists.push(xs, i * 2)
        total = total + lists.len(xs)
        i = i + 1
    end
    const leaked: int = test.assert_no_leaks("main_loop_managed")
    io.print("total:" + string(total))
    io.print("leaks:" + string(leaked))
end
"#,
        "loop_managed_no_leak",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "total:200\nleaks:0");
}

// ── 5.3 — ORI_TEST_LEAK_CHECK env ──────────────────────────────────────────

/// When `ORI_TEST_LEAK_CHECK=1` is set and a program retains allocations on
/// purpose, the runtime aborts with a non-zero exit code and a stderr
/// diagnostic. This is the convention test harnesses opt into to fail fast
/// on leaks.
#[test]
fn compile_runs_native_leak_check_env_aborts_on_intentional_leak() {
    let dir = TestDir::new("leak_check_env_aborts");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

var kept: list[int] = lists.new()

main()
    lists.push(kept, 1)
    lists.push(kept, 2)
    io.print("kept:" + string(lists.len(kept)))
    const leaked: int = test.assert_no_leaks("main_intentional_leak")
    io.print("leaks:" + string(leaked))
end
"#,
    );
    let exe = exe_path(&dir, "leak_check_env_aborts");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let plain = Command::new(&exe).output().unwrap();
    let plain_stdout = String::from_utf8_lossy(&plain.stdout).to_string();
    let plain_stderr = String::from_utf8_lossy(&plain.stderr).to_string();
    assert!(
        plain.status.success(),
        "plain run failed: {plain_stdout} {plain_stderr}"
    );
    assert!(
        plain_stdout.contains("kept:2"),
        "missing kept:2 in {plain_stdout}"
    );

    let strict = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let strict_stderr = String::from_utf8_lossy(&strict.stderr).to_string();
    assert!(
        !strict.status.success(),
        "expected non-zero exit with leak check, got success: {}",
        strict_stderr
    );
    assert!(
        strict_stderr.contains("ori leak check"),
        "expected leak-check diagnostic in stderr, got: {strict_stderr}"
    );
}

/// With leak check enabled, a program with no managed allocations completes
/// successfully and emits no leak-check diagnostic. This verifies the
/// `ORI_TEST_LEAK_CHECK=1` env does not false-positive on clean programs.
#[test]
fn compile_runs_native_leak_check_env_clean() {
    let dir = TestDir::new("leak_check_env_clean");
    let (stdout, stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

main()
    io.print("clean")
    const leaked: int = test.assert_no_leaks("main_clean")
    io.print("leaks:" + string(leaked))
end
"#,
        "leak_check_env_clean",
    );
    assert!(success, "stderr: {stderr}");
    assert!(
        !stderr.contains("ori leak check"),
        "unexpected leak diagnostic in clean run: {stderr}"
    );
    assert_eq!(stdout.trim(), "clean\nleaks:0");
}

// ── 5.2 — Stress (opt-in) ──────────────────────────────────────────────────

/// Stress test for 10k iterations. Marked `#[ignore]` by default to keep the
/// default test suite fast; run with `cargo test -p ori-driver --test
/// memory_arc -- --ignored cycle_stress`. The test verifies the collector
/// runs without crashing on a large workload.
#[test]
#[ignore = "slow stress test: run with --ignored cycle_stress"]
fn compile_runs_native_cycle_stress_10k() {
    let dir = TestDir::new("cycle_stress_10k");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Peer
    id: int
    link: list[int]
end

build_one_cycle(seed: int) -> int
    const a_link: list[int] = lists.new()
    const b_link: list[int] = lists.new()
    const a: Peer = Peer {id: seed, link: b_link}
    const b: Peer = Peer {id: seed + 1, link: a_link}
    return a.id + b.id
end

main()
    var i: int = 0
    var acc: int = 0
    while i < 10000
        acc = acc + build_one_cycle(i)
        i = i + 1
    end
    io.print("acc:" + string(acc))
    const collected: int = test.collect_cycles()
    io.print("collected:" + string(collected))
    const leaked: int = test.live_allocations()
    io.print("leaks:" + string(leaked))
end
"#,
        "cycle_stress_10k",
    );
    assert!(success, "{stdout}");
    let lines = stdout.lines().collect::<Vec<_>>();
    let acc_line = lines
        .iter()
        .find(|l| l.starts_with("acc:"))
        .unwrap_or_else(|| panic!("missing acc line: {stdout}"));
    assert!(acc_line.starts_with("acc:"), "missing acc line: {stdout}");
    assert!(
        lines.iter().any(|l| l.starts_with("collected:")),
        "missing collected line: {stdout}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("leaks:")),
        "missing leaks line: {stdout}"
    );
}

/// Etapa 5 checkbox 251: ciclo envolvendo coleções opacas (`linked_list` /
/// `graph`) — sem leak após teste de estresse. The graph runtime owns internal
/// managed edges (adjacency lists); the linked_list runtime owns managed
/// nodes. Both are created and dropped in a loop. After the loop, the leak
/// check must report zero live allocations, proving the type-specific
/// destructors and ARC release paths cover opaque collections.
#[test]
fn compile_runs_native_linked_list_and_graph_no_leak() {
    let dir = TestDir::new("linked_list_graph_no_leak");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.graph = graph
import ori.linked_list = llist
import ori.list = lists
import ori.test = test

exercise_collections(seed: int) -> int
    const xs: llist.LinkedList[int] = llist.new()
    llist.push_back(xs, seed)
    llist.push_back(xs, seed + 1)
    llist.push_back(xs, seed + 2)

    const g: graph.Graph[int] = graph.new(true)
    graph.add_edge(g, seed, seed + 1)
    graph.add_edge(g, seed + 1, seed + 2)
    graph.add_edge(g, seed + 2, seed)

    const ns: list[int] = graph.neighbors(g, seed)
    return llist.len(xs) + lists.len(ns)
end

main()
    var i: int = 0
    var total: int = 0
    while i < 50
        total = total + exercise_collections(i)
        i = i + 1
    end
    const collected: int = test.collect_cycles()
    const leaked: int = test.assert_no_leaks("linked_list_graph_stress")
    io.print("total:" + string(total))
    io.print("collected:" + string(collected))
    io.print("leaks:" + string(leaked))
end
"#,
        "linked_list_graph_no_leak",
    );
    assert!(success, "{stdout}");
    let lines = stdout.lines().collect::<Vec<_>>();
    let total_line = lines
        .iter()
        .find(|l| l.starts_with("total:"))
        .unwrap_or_else(|| panic!("missing total line: {stdout}"));
    // Each iteration: llist.len=3 + neighbors of seed (1 edge -> 1 neighbor) = 4.
    // 50 iterations * 4 = 200.
    assert_eq!(total_line.trim(), "total:200");
    let leaks_line = lines
        .iter()
        .find(|l| l.starts_with("leaks:"))
        .unwrap_or_else(|| panic!("missing leaks line: {stdout}"));
    assert_eq!(leaks_line.trim(), "leaks:0");
}

// ── C1 — single cascade owner (dtor × edges overlap) ───────────────────────

/// A list keeps its own reference to an element. `list.get` must acquire a
/// separate reference for the returned value before the list drops its edge.
#[test]
fn compile_runs_native_list_get_keeps_removed_struct_alive() {
    let dir = TestDir::new("list_get_removed_struct");
    let (stdout, stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists

struct Entry
    name: string
end

main()
    const entries: list[Entry] = []
    lists.push(entries, Entry { name: "retained" })
    const fetched = lists.get(entries, 0)
    lists.remove(entries, 0)
    var i = 0
    while i < 100
        lists.push(entries, Entry { name: "unrelated" })
        i = i + 1
    end
    io.println(fetched.name)
end
"#,
        "list_get_removed_struct",
    );
    assert!(success, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "retained");
}

// Regression tests from docs/planning/historico/nim-study-2026-07-17-c1.md.
// Before the fix, composite owners released managed fields twice (generated
// __dtor_* plus the registered ARC edge), so a child shared with a live
// binding was freed prematurely; list element slots leaked the owned temp +1.

/// S4d: a list shared between a live caller binding and a struct field must
/// survive the struct owner's death. `live_allocations` must not drop while
/// the caller binding is still in scope.
#[test]
fn compile_runs_native_shared_child_survives_owner_free() {
    let dir = TestDir::new("shared_child_survives_owner_free");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Holder
    items: list[int]
end

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

consume(shared: list[int]) -> int
    const h: Holder = Holder { items: shared }
    return lists.len(h.items)
end

exercise() -> int
    const xs: list[int] = make_list(4)
    const before: int = test.live_allocations()
    const a: int = consume(xs)
    const after: int = test.live_allocations()
    if after < before
        return 0 - 1
    end
    const b: int = consume(xs)
    return lists.len(xs) + a + b
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("shared_child_owner_free")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "shared_child_owner_free",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:12\nleaks:0");
}

/// S4d via assignment: storing a borrowed binding into a struct field must
/// leave the binding's own reference intact after the struct owner dies.
#[test]
fn compile_runs_native_shared_child_survives_field_assign_owner_free() {
    let dir = TestDir::new("shared_child_field_assign");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Holder
    items: list[int]
end

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

consume_via_assign(shared: list[int]) -> int
    var h: Holder = Holder { items: make_list(1) }
    h.items = shared
    return lists.len(h.items)
end

exercise() -> int
    const xs: list[int] = make_list(3)
    const before: int = test.live_allocations()
    const a: int = consume_via_assign(xs)
    const after: int = test.live_allocations()
    if after < before
        return 0 - 1
    end
    return lists.len(xs) + a
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("shared_child_field_assign")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "shared_child_field_assign",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:6\nleaks:0");
}

/// Two fields in the same owner are separate ARC slots even when they store the
/// same managed pointer. Replacing one field must not invalidate the other after
/// the helper's original binding has left scope.
#[test]
fn compile_runs_native_parallel_field_edge_survives_one_replacement() {
    let dir = TestDir::new("parallel_field_edge");
    let (stdout, stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Pair
    left: list[int]
    right: list[int]
end

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

make_pair() -> Pair
    const shared: list[int] = make_list(3)
    return Pair { left: shared, right: shared }
end

exercise() -> int
    var pair: Pair = make_pair()
    pair.left = make_list(1)
    lists.push(pair.right, 9)
    return lists.len(pair.left) * 10 + lists.len(pair.right)
end

main()
    const score: int = exercise()
    const leaked: int = test.assert_no_leaks("parallel_field_edge")
    io.print("score:" + string(score))
    io.print("leaks:" + string(leaked))
end
"#,
        "parallel_field_edge",
    );
    assert!(success, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout.trim(), "score:14\nleaks:0");
}

/// Parallel collection/channel slots must have identical ownership behavior in
/// AOT and JIT. Removing one list/map slot and consuming one channel entry must
/// leave every surviving alias valid and release the unreceived queue entry.
#[test]
fn aot_and_jit_preserve_parallel_collection_and_channel_slots() {
    let aot_dir = TestDir::new("parallel_ownership_aot");
    let (aot_stdout, aot_stderr, aot_success) = compile_and_run_with_leak_check(
        &aot_dir,
        PARALLEL_OWNERSHIP_SOURCE,
        "parallel_ownership_aot",
    );
    assert!(
        aot_success,
        "AOT ownership matrix failed: stdout={aot_stdout:?} stderr={aot_stderr:?}"
    );
    assert_eq!(aot_stdout.trim(), "score:4\nleaks:0");

    let jit_dir = TestDir::new("parallel_ownership_jit");
    let (jit_stdout, jit_stderr, jit_success) =
        run_jit_with_leak_check(&jit_dir, PARALLEL_OWNERSHIP_SOURCE);
    assert!(
        jit_success,
        "JIT ownership matrix failed: stdout={jit_stdout:?} stderr={jit_stderr:?}"
    );
    assert_eq!(jit_stdout.trim(), "score:4\nleaks:0");
}

/// Optional local memory-checker gate for the native runtime path. Unsupported
/// hosts skip explicitly; release/CI jobs can require it instead of silently
/// passing by setting `ORI_REQUIRE_RUNTIME_VALGRIND=1`.
#[test]
fn native_parallel_ownership_runs_cleanly_under_valgrind_when_available() {
    let valgrind = match find_valgrind() {
        Ok(valgrind) => valgrind,
        Err(reason)
            if std::env::var_os(REQUIRE_RUNTIME_VALGRIND_ENV).is_some_and(|value| value == "1") =>
        {
            panic!("runtime Valgrind gate is required but unavailable: {reason}")
        }
        Err(reason) => {
            eprintln!(
                "SKIP native_parallel_ownership_runs_cleanly_under_valgrind_when_available: \
                 {reason}; set {REQUIRE_RUNTIME_VALGRIND_ENV}=1 to require this gate"
            );
            return;
        }
    };

    let dir = TestDir::new("parallel_ownership_valgrind");
    dir.write("main.orl", PARALLEL_OWNERSHIP_SOURCE);
    let exe = exe_path(&dir, "parallel_ownership_valgrind");
    let compiled = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);

    let output = Command::new(&valgrind)
        .arg("--error-exitcode=86")
        .arg("--leak-check=full")
        .arg("--show-leak-kinds=definite,indirect")
        .arg("--errors-for-leak-kinds=definite,indirect")
        .arg(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn {valgrind:?}: {error}"));
    assert!(
        output.status.success(),
        "Valgrind runtime gate failed: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "score:4\nleaks:0"
    );
}

/// S4b: assigning a fresh owned value into a struct field must consume the
/// temporary's +1 (the edge becomes the only owner besides bindings).
#[test]
fn compile_runs_native_field_assign_owned_temp_no_leak() {
    let dir = TestDir::new("field_assign_owned_temp");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Holder
    items: list[int]
end

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

exercise() -> int
    var h: Holder = Holder { items: make_list(2) }
    h.items = make_list(5)
    return lists.len(h.items)
end

main()
    const size: int = exercise()
    const leaked: int = test.assert_no_leaks("field_assign_owned_temp")
    io.print("size:" + string(size))
    io.print("leaks:" + string(leaked))
end
"#,
        "field_assign_owned_temp",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "size:5\nleaks:0");
}

/// Nested list literals and `lists.push` with fresh owned elements must not
/// leak the element temporaries (edge ownership replaces the temp +1).
#[test]
fn compile_runs_native_nested_list_literal_and_push_no_leak() {
    let dir = TestDir::new("nested_list_literal_push");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

exercise() -> int
    const outer: list[list[int]] = [make_list(2), make_list(3)]
    var inner: list[list[int]] = lists.new()
    lists.push(inner, make_list(4))
    return lists.len(outer) + lists.len(inner)
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("nested_list_literal_push")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "nested_list_literal_push",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:3\nleaks:0");
}

/// Index assignment with a fresh owned element must release both the
/// replaced element's edge reference and the new temporary's +1.
#[test]
fn compile_runs_native_list_index_assign_owned_no_leak() {
    let dir = TestDir::new("list_index_assign_owned");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

exercise() -> int
    var xs: list[list[int]] = [make_list(1)]
    xs[0] = make_list(6)
    return lists.len(xs[0])
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("list_index_assign_owned")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "list_index_assign_owned",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:6\nleaks:0");
}

/// Collection mutators must transfer fresh managed values into their ARC edge
/// ownership just like direct list index assignment does.
#[test]
fn compile_runs_native_list_set_and_insert_managed_no_leak() {
    let dir = TestDir::new("list_set_insert_managed");
    let (stdout, stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

make_list(n: int) -> list[int]
    const values: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(values, i)
        i = i + 1
    end
    return values
end

exercise() -> int
    var values: list[list[int]] = [make_list(1)]
    lists.set(values, 0, make_list(6))
    lists.insert(values, 1, make_list(7))
    return lists.len(values[0]) + lists.len(values[1])
end

main()
    const total: int = exercise()
    const leaked: int = test.assert_no_leaks("list_set_insert_managed")
    io.print("total:" + string(total))
    io.print("leaks:" + string(leaked))
end
"#,
        "list_set_insert_managed",
    );
    assert!(success, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "total:13\nleaks:0");
}

// ── C2 — string temporary accounting (print / interpolation) ───────────────
// Before the fix, io.print with a fresh string argument leaked the temp's +1,
// and every f-string part/intermediate concat leaked (7 live allocations for
// a five-part f-string bound to a local).

/// io.print with concat and f-string arguments must release the temporaries;
/// printing a borrowed binding must NOT release the binding's reference.
#[test]
fn compile_runs_native_print_string_temps_no_leak() {
    let dir = TestDir::new("print_string_temps");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

exercise()
    io.print("x:" + string(1))
    io.print(f"p{40 + 2}q")
    const s: string = "keep" + string(9)
    io.print(s)
    io.print(s)
end

main()
    exercise()
    const leaked: int = test.assert_no_leaks("print_string_temps")
    io.print("leaks:" + string(leaked))
end
"#,
        "print_string_temps",
    );
    assert!(success, "{stdout}");
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.first().copied(), Some("x:1"), "{stdout}");
    assert_eq!(lines.get(1).copied(), Some("p42q"), "{stdout}");
    assert_eq!(lines.get(2).copied(), Some("keep9"), "{stdout}");
    assert_eq!(lines.get(3).copied(), Some("keep9"), "{stdout}");
    assert_eq!(lines.last().copied(), Some("leaks:0"), "{stdout}");
}

/// An f-string bound to a local must leave exactly the final string alive:
/// scalar conversions and intermediate concats are released as they are
/// consumed.
#[test]
fn compile_runs_native_fstring_intermediates_no_leak() {
    let dir = TestDir::new("fstring_intermediates");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

exercise()
    const before: int = test.live_allocations()
    const s: string = f"x{1 + 2}y{3}z"
    const after: int = test.live_allocations()
    io.print("delta:" + string(after - before))
    io.print(s)
end

main()
    exercise()
    const leaked: int = test.assert_no_leaks("fstring_intermediates")
    io.print("leaks:" + string(leaked))
end
"#,
        "fstring_intermediates",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "delta:1\nx3y3z\nleaks:0");
}

// ── LANG-MEM-2 — map/set owned-arg accounting and borrowed get ─────────────
// Before the fix, map/set runtime calls leaked the +1 of owned key/value
// temporaries, and maps.get returned a borrowed value that codegen treated
// as owned (the binding's scope release stole the leaked +1 - paired bugs).

/// Map with managed values: set (including key overwrite) plus get must not
/// leak the value temporaries; the overwritten value must be released.
#[test]
fn compile_runs_native_map_managed_values_no_leak() {
    let dir = TestDir::new("map_managed_values");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.test = test

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

exercise() -> int
    var m: map[string, list[int]] = maps.new()
    maps.set(m, "a", make_list(2))
    maps.set(m, "a", make_list(5))
    maps.set(m, "b", make_list(1))
    return lists.len(maps.get(m, "a"))
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("map_managed_values")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "map_managed_values",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:5\nleaks:0");
}

/// Set of strings: fresh owned element temporaries (including a rejected
/// duplicate) must be released; the set owns stored elements via edges.
#[test]
fn compile_runs_native_set_owned_elements_no_leak() {
    let dir = TestDir::new("set_owned_elements");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.set = sets
import ori.test = test

exercise() -> int
    var s: set[string] = sets.new()
    sets.add(s, "k" + string(1))
    sets.add(s, "k" + string(1))
    sets.add(s, "other")
    return sets.len(s)
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("set_owned_elements")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "set_owned_elements",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:2\nleaks:0");
}

/// UAF guard: a value read from a map must survive the map's death (the get
/// result is retained; it does not borrow the map's edge reference).
#[test]
fn compile_runs_native_map_get_value_survives_map_free() {
    let dir = TestDir::new("map_get_survives");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.test = test

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

take_from_map() -> list[int]
    var m: map[string, list[int]] = maps.new()
    maps.set(m, "a", make_list(3))
    return maps.get(m, "a")
end

exercise() -> int
    const xs: list[int] = take_from_map()
    -- o map ja morreu; xs deve continuar valido
    return lists.len(xs)
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("map_get_survives")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "map_get_survives",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:3\nleaks:0");
}

/// try_get must produce an optional that owns its managed payload via an
/// edge, so the payload survives the map and nothing leaks.
#[test]
fn compile_runs_native_map_try_get_payload_no_leak() {
    let dir = TestDir::new("map_try_get_payload");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.test = test

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

take_try() -> optional[list[int]]
    var m: map[string, list[int]] = maps.new()
    maps.set(m, "a", make_list(4))
    return maps.try_get(m, "a")
end

exercise() -> int
    const o: optional[list[int]] = take_try()
    match o
        case some(xs):
            return lists.len(xs)
        case none:
            return 0
    end
    return 0
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("map_try_get_payload")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "map_try_get_payload",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:4\nleaks:0");
}

// ── LANG-MEM: match/if-some scrutinee release (found while re-verifying an
// external report of ori.fs memory corruption under multi-module linking) ─
//
// bind_pattern extracted match/if-some payloads as plain loads (borrows of
// the scrutinee), but emit_match/emit_if_some never released a scrutinee
// that was itself a fresh owned temporary (e.g. `match some_call()`, not a
// bound Var). The scrutinee — and, for codegen-constructed Ok_/Some_/Err_
// values, anything it owned via an ARC edge — leaked on every match.

/// A managed value read out of a fresh `result[...]` scrutinee not bound to
/// a variable must not leak the result wrapper.
#[test]
fn compile_runs_native_match_owned_result_scrutinee_no_leak() {
    let dir = TestDir::new("match_owned_result_scrutinee");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

mk(i: int) -> result[string, string]
    return ok("hi-" + string(i))
end

exercise(n: int) -> int
    var i: int = 0
    var total: int = 0
    while i < n
        match mk(i)
            case ok(x):
                total = total + 1
            case err(_):
                total = total + 0
        end
        i = i + 1
    end
    return total
end

main()
    const r: int = exercise(20)
    const leaked: int = test.assert_no_leaks("match_owned_result_scrutinee")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "match_owned_result_scrutinee",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:20\nleaks:0");
}

/// A payload extracted from a fresh owned `match` scrutinee and returned
/// from the enclosing function must survive the scrutinee's release
/// (return-transfer elision must apply to the extracted binding too).
#[test]
fn compile_runs_native_match_owned_scrutinee_payload_returned_no_leak() {
    let dir = TestDir::new("match_owned_scrutinee_payload_returned");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

mk(i: int) -> result[string, string]
    return ok("payload-" + string(i))
end

unwrap_or_default(i: int) -> string
    match mk(i)
        case ok(text):
            return text
        case err(_):
            return "fallback"
    end
    return "fallback"
end

exercise(n: int) -> int
    var i: int = 0
    var total: int = 0
    while i < n
        const text: string = unwrap_or_default(i)
        total = total + str_len(text)
        i = i + 1
    end
    return total
end

str_len(s: string) -> int
    return len(s)
end

main()
    const r: int = exercise(20)
    const leaked: int = test.assert_no_leaks("match_owned_scrutinee_payload_returned")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "match_owned_scrutinee_payload_returned",
    );
    assert!(success, "{stdout}");
    // "payload-0".."payload-9" are 9 chars each (90), "payload-10".."payload-19"
    // are 10 chars each (100): 90 + 100 = 190.
    assert_eq!(stdout.trim(), "r:190\nleaks:0");
}

/// A managed payload extracted from a fresh owned `optional[...]` scrutinee
/// via `if some(x) = ...` must not leak the optional wrapper, on both the
/// some and none paths.
#[test]
fn compile_runs_native_if_some_owned_scrutinee_no_leak() {
    let dir = TestDir::new("if_some_owned_scrutinee");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

mk(i: int) -> optional[string]
    if i % 2 == 0
        return some("even-" + string(i))
    end
    return none
end

exercise(n: int) -> int
    var i: int = 0
    var total: int = 0
    while i < n
        if some(x) = mk(i)
            total = total + 1
        end
        i = i + 1
    end
    return total
end

main()
    const r: int = exercise(20)
    const leaked: int = test.assert_no_leaks("if_some_owned_scrutinee")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "if_some_owned_scrutinee",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:10\nleaks:0");
}

// ── LANG-MEM: nested match binding shadowing (Cranelift crash) ─────────────
//
// bind_pattern/HirStmt::Let/emit_if_some/emit_while_some reused an existing
// Cranelift Variable whenever `lookup_var` found a same-named binding
// ANYWHERE on the current scope stack — including an enclosing match arm or
// if-some still in scope. When the reused binding's native type differed
// (e.g. an outer `float` payload vs. an inner `string` payload sharing the
// name `value`), Cranelift panicked internally on `def_var`
// ("declared type of variable varN doesn't match type of value vNN").

/// Two nested matches over different enum types that both bind a payload
/// under the same name, with genuinely different native Cranelift types
/// (float vs. string — different register classes), must not crash the
/// backend and must resolve each binding to its own value.
#[test]
fn compile_runs_native_nested_match_same_binding_name_different_types() {
    let dir = TestDir::new("nested_match_shadow_types");
    let (stdout, _stderr, success) = compile_and_run(
        &dir,
        r#"module app.main

import ori.io = io

enum Outer
    Empty
    Wrapped(value: float)
end

enum Inner
    None_
    Wrapped(value: string)
end

make_inner(tag: string) -> Inner
    return Inner.Wrapped(value: tag)
end

top(o: Outer, tag: string) -> string
    match o
        case Empty:
            return "empty"
        case Wrapped(value):
            match make_inner(tag)
                case None_:
                    return "none"
                case Wrapped(value):
                    return value
            end
            return "fallthrough"
    end
    return "outer-fallthrough"
end

main()
    io.println(top(Outer.Wrapped(value: 3.5), "inner-value"))
end
"#,
        "nested_match_shadow_types",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "inner-value");
}

// ── LANG-MEM-9 — runtime-built result/optional wrappers are ARC-managed ────
//
// `new_result*` / `new_optional_ptr` in ori-runtime used raw libc::malloc:
// wrappers were invisible to the ARC registry, codegen releases were silent
// no-ops, and every runtime result (ori.fs, ori.process, net, ...) leaked
// its payload. Wrappers now go through ori_alloc and own their managed
// payload via an edge; the `try`/`?` unwrap consumes an owned wrapper on
// both paths and always produces an owned payload.

/// fs.read_text_or in a loop must not leak the runtime-built result
/// wrappers or their string payloads (12 leaked allocations per 10 calls
/// before the fix).
#[test]
fn compile_runs_native_fs_read_text_or_loop_no_leak() {
    let dir = TestDir::new("fs_read_text_or_loop");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

imports
    ori.io = io
    ori.fs = fs
    ori.test = test
end

exercise(path: string, n: int) -> int
    var i: int = 0
    var total: int = 0
    while i < n
        const text: string = fs.read_text_or(path, "MISSING")
        total = total + len(text)
        i = i + 1
    end
    return total
end

main()
    const path: string = "/tmp/ori_mem9_regression.txt"
    fs.write_text(path, "regression-content")
    const r: int = exercise(path, 10)
    const leaked: int = test.assert_no_leaks("fs_read_text_or_loop")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "fs_read_text_or_loop",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:180\nleaks:0");
}

/// `try` over codegen-built results in a loop must consume the wrapper on
/// both the ok and err paths (32 leaked allocations per 10 iterations
/// before the fix).
#[test]
fn compile_runs_native_try_unwrap_loop_no_leak() {
    let dir = TestDir::new("try_unwrap_loop");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.test = test

mk(i: int) -> result[string, string]
    if i % 2 == 0
        return ok("val-" + string(i))
    end
    return err("odd-" + string(i))
end

use_try(i: int) -> result[int, string]
    const text: string = try mk(i)
    return ok(len(text))
end

exercise(n: int) -> int
    var i: int = 0
    var total: int = 0
    while i < n
        match use_try(i)
            case ok(v):
                total = total + v
            case err(_):
                total = total + 100
        end
        i = i + 1
    end
    return total
end

main()
    const r: int = exercise(10)
    const leaked: int = test.assert_no_leaks("try_unwrap_loop")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "try_unwrap_loop",
    );
    assert!(success, "{stdout}");
    // Even i (0,2,4,6,8): ok("val-N") -> len 5 each = 25.
    // Odd i: err propagated -> +100 each = 500. Total 525.
    assert_eq!(stdout.trim(), "r:525\nleaks:0");
}

// ── C2 final — owned-arg accounting across every collection emitter ────────
// The special-cased hash_table/graph/heap/tree emitters (and from_entries/
// from_list source lists) never released fresh owned key/value temporaries;
// graph edges additionally stored the caller's raw argument pointer, which
// dangles once temporaries are released (nodes are deduplicated by content,
// so the canonical stored node must back each edge).

/// Every remaining collection exercised with fresh owned string arguments in
/// loops must end with zero live allocations, and graph adjacency built from
/// content-equal temporaries must stay intact.
#[test]
fn compile_runs_native_collections_owned_args_no_leak() {
    let dir = TestDir::new("collections_owned_args");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

imports
    ori.io = io
    ori.list = lists
    ori.map = maps
    ori.set = sets
    ori.hash_table = ht
    ori.graph = graph
    ori.heap = heap
    ori.tree = tree
    ori.test = test
end

case_hash_table(n: int) -> int
    const table: ht.HashTable[string, int] = ht.new()
    var i: int = 0
    while i < n
        ht.set(table, "key-" + string(i), i)
        i = i + 1
    end
    match ht.get(table, "key-" + string(1))
        case some(v):
            return v
        case none:
            return 0 - 1
    end
    return 0 - 1
end

case_graph(n: int) -> int
    const g: graph.Graph[string] = graph.new(false)
    var i: int = 0
    while i < n
        graph.add_edge(g, "n" + string(i), "n" + string(i + 1))
        i = i + 1
    end
    const nb: list[string] = graph.neighbors(g, "n" + string(1))
    return lists.len(nb)
end

case_heap(n: int) -> int
    const h: heap.Heap[string] = heap.new()
    var i: int = 0
    while i < n
        heap.push(h, "item-" + string(i))
        i = i + 1
    end
    return heap.len(h)
end

case_from_entries(n: int) -> int
    var total: int = 0
    var i: int = 0
    while i < n
        const m: map[string, int] = maps.from_entries([tuple("a" + string(i), 1), tuple("b", 2)])
        total = total + maps.len(m)
        i = i + 1
    end
    return total
end

case_set_from_list(n: int) -> int
    var total: int = 0
    var i: int = 0
    while i < n
        const s: set[string] = sets.from_list(["x" + string(i), "y", "x" + string(i)])
        total = total + sets.len(s)
        i = i + 1
    end
    return total
end

case_tree(n: int) -> int
    const t: tree.Tree[string] = tree.new("root-" + string(n))
    const root: tree.NodeId = tree.root(t)
    var i: int = 0
    while i < n
        const child: tree.NodeId = tree.add_child(t, root, "child-" + string(i))
        tree.set_value(t, child, "renamed-" + string(i))
        i = i + 1
    end
    return lists.len(tree.children(t, root))
end

case_heap_from_list(n: int) -> int
    var total: int = 0
    var i: int = 0
    while i < n
        const h: heap.Heap[string] = heap.from_list(["c-" + string(i), "a", "b"])
        total = total + heap.len(h)
        i = i + 1
    end
    return total
end

main()
    const a: int = case_hash_table(5)
    const la: int = test.assert_no_leaks("hash_table")
    const b: int = case_graph(5)
    const lb: int = test.assert_no_leaks("graph")
    const c: int = case_heap(5)
    const lc: int = test.assert_no_leaks("heap")
    const d: int = case_from_entries(3)
    const ld: int = test.assert_no_leaks("from_entries")
    const e: int = case_set_from_list(3)
    const le: int = test.assert_no_leaks("set_from_list")
    const f: int = case_tree(4)
    const lf: int = test.assert_no_leaks("tree")
    const g: int = case_heap_from_list(3)
    const lg: int = test.assert_no_leaks("heap_from_list")
    io.print("r:" + string(a + b + c + d + e + f + g))
    io.print("leaks:" + string(la + lb + lc + ld + le + lf + lg))
end
"#,
        "collections_owned_args",
    );
    assert!(success, "{stdout}");
    // a=1 (value at key-1), b=2 (n0/n2 neighbors of n1 — canonical nodes),
    // c=5, d=6 (2 entries x3), e=6 (2 uniques x3), f=4 children, g=9 (3x3).
    assert_eq!(stdout.trim(), "r:33\nleaks:0");
}

// ── Plan F1 gap closure — enum rebind across variants (scenario S2) ────────

/// Rebinding a `var` enum to a different variant must release the previous
/// allocation's managed payload for the variant that was actually active
/// (the tag lives in the old allocation, so cascade release reads the right
/// variant by construction) — including variant→variant and variant→empty.
#[test]
fn compile_runs_native_enum_rebind_across_variants_no_leak() {
    let dir = TestDir::new("enum_rebind_variants");
    let (stdout, _stderr, success) = compile_and_run_with_leak_check(
        &dir,
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

enum Shape
    Empty
    Polygon(points: list[int])
    Label(name: string, tags: list[int])
end

make_list(n: int) -> list[int]
    const xs: list[int] = lists.new()
    var i: int = 0
    while i < n
        lists.push(xs, i)
        i = i + 1
    end
    return xs
end

exercise() -> int
    var s: Shape = Shape.Polygon(points: make_list(3))
    s = Shape.Label(name: "x" + string(1), tags: make_list(2))
    s = Shape.Polygon(points: make_list(5))
    s = Shape.Empty
    match s
        case Empty:
            return 1
        case Polygon(points):
            return lists.len(points)
        case Label(name, tags):
            return lists.len(tags)
    end
    return 0
end

main()
    const r: int = exercise()
    const leaked: int = test.assert_no_leaks("enum_rebind_variants")
    io.print("r:" + string(r))
    io.print("leaks:" + string(leaked))
end
"#,
        "enum_rebind_variants",
    );
    assert!(success, "{stdout}");
    assert_eq!(stdout.trim(), "r:1\nleaks:0");
}

/// A managed value leaving a `match` expression is handed back owned exactly
/// once — whether the arm returned a borrowed pattern binding or a freshly
/// allocated string.
#[test]
fn compile_runs_match_expression_managed_values_no_leak() {
    let dir = TestDir::new("match_expr_managed_no_leak");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

unwrap_or(v: optional[string], fallback: string) -> string
    return match v
    case some(inner): inner
    case else: fallback
    end
end

describe(n: int) -> string
    return match n
    case 1: f"numero {n}"
    case else: "outro"
    end
end

main()
    var i: int = 0
    while i < 200
        const a: string = unwrap_or(some("x"), "y")
        const b: string = unwrap_or(none, "y")
        const c: string = describe(1)
        const d: string = describe(2)
        i = i + 1
    end
    io.println("done")
end
"#,
    );

    let exe = exe_path(&dir, "match_expr_managed");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "done", "stderr: {stderr}");
}

/// Managed payloads bound by `if ok` / `if err` are released exactly once,
/// on both the taken and the not-taken branch.
#[test]
fn compile_runs_if_ok_err_managed_payloads_no_leak() {
    let dir = TestDir::new("if_ok_err_no_leak");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

load(flag: bool) -> result[string, string]
    if flag
        return ok("allocated-ok-value")
    end
    return err("allocated-err-value")
end

main()
    var i: int = 0
    while i < 200
        if ok(v) = load(true)
            const a: string = v
        end
        if err(e) = load(false)
            const b: string = e
        end
        if ok(v) = load(false)
            const c: string = v
        else
            const d: string = "fallback"
        end
        i = i + 1
    end
    io.println("done")
end
"#,
    );

    let exe = exe_path(&dir, "if_ok_err_managed");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "done",
        "stderr: {stderr}"
    );
}

/// A `newtype` over a managed representation (`string`) is exactly its
/// representation at runtime: constructing and unwrapping it in a loop must
/// not leak.
#[test]
fn compile_runs_newtype_over_string_no_leak() {
    let dir = TestDir::new("newtype_string_no_leak");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

newtype Email = string

domain(mail: Email) -> string
    return string(mail)
end

main()
    var i: int = 0
    while i < 200
        const mail: Email = Email("someone@example.com")
        const text: string = domain(mail)
        i = i + 1
    end
    io.println("done")
end
"#,
    );

    let exe = exe_path(&dir, "newtype_string_managed");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "done",
        "stderr: {stderr}"
    );
}

/// Destructuring managed fields binds each one exactly once: the desugared
/// temporary plus field reads must not leak.
#[test]
fn compile_runs_destructuring_managed_fields_no_leak() {
    let dir = TestDir::new("destructure_managed_no_leak");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct User
    name: string
    tag: string
end

make() -> User
    return User { name: "alice", tag: "admin" }
end

main()
    var i: int = 0
    while i < 200
        const User { name, tag } = make()
        const joined: string = name + tag
        i = i + 1
    end
    io.println("done")
end
"#,
    );

    let exe = exe_path(&dir, "destructure_managed");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "done",
        "stderr: {stderr}"
    );
}

// ── Release consolidated into one critical section (LANG-MEM-10) ───────────
//
// `ori_arc_release` used to take the ARC mutex up to six times to drop a single
// object: once to decrement, then again inside `free_registered_object` for
// unregister, take-edges and remove-incoming, plus a redundant
// `header_for_registered`. Dropping one string dominated the cost of every
// managed temporary (~360 ns/iteration).
//
// The decrement, unregister and edge removal now share one lock; the destructor
// and the recursive child releases stay outside it, because a destructor
// re-enters `ori_arc_release`. These tests pin the behaviour that must survive
// that restructuring: nested owners still cascade, and nothing leaks.

#[test]
fn compile_runs_nested_owners_cascade_on_release() {
    let dir = TestDir::new("nested_cascade_release");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Inner
    label: string
end

struct Outer
    inner: Inner
    name: string
end

build(n: int) -> Outer
    return Outer {
        inner: Inner { label: "deep" },
        name: "outer"
    }
end

main()
    var i: int = 0
    while i < 300
        const o: Outer = build(i)
        const joined: string = o.name + o.inner.label
        i = i + 1
    end
    io.println("done")
end
"#,
    );

    let exe = exe_path(&dir, "nested_cascade");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "done",
        "stderr: {stderr}"
    );
}

#[test]
fn compile_runs_managed_temporaries_in_a_hot_loop_without_leaking() {
    let dir = TestDir::new("hot_loop_temporaries");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

work(prefix: string) -> int
    const full: string = prefix + "-x"
    return str.len(full)
end

main()
    var i: int = 0
    var total: int = 0
    while i < 2000
        total = total + work("name")
        i = i + 1
    end
    io.println(f"{total}")
end
"#,
    );

    let exe = exe_path(&dir, "hot_loop_temporaries");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "12000",
        "stderr: {stderr}"
    );
}

// ── Strings are built with one allocation (LANG-PERF-4b) ───────────────────
//
// `ori_string_concat_parts` and friends used to build a `Vec`, copy the parts
// in, copy the `Vec` into a fresh `ori_alloc` block, then free the `Vec` — two
// mallocs, two copies and a free for every string the runtime produced. The
// parts are now written straight into the final block. These cases pin the
// behaviour that the rewrite could have broken: empty operands, chained
// concatenation, and multi-byte characters across a slice boundary.

#[test]
fn compile_runs_string_building_edge_cases() {
    let dir = TestDir::new("string_one_alloc");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

main()
    io.println("[" + "" + "x" + "][" + "x" + "" + "][" + "" + "" + "]")
    io.println("a" + "b" + "c" + "d")

    const u: string = "café" + " ☕"
    io.println(u)
    io.println(f"{str.len(u)}")

    io.println(str.slice("abcdef", 1, 4))
    io.println(str.slice("café☕", 0, 4))

    var s: string = ""
    var i: int = 0
    while i < 200
        s = s + "ab"
        i = i + 1
    end
    io.println(f"{str.len(s)}")
end
"#,
    );

    let exe = exe_path(&dir, "string_one_alloc");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[x][x][]\nabcd\ncafé ☕\n6\nbcd\ncafé\n400\n",
        "stderr: {stderr}"
    );
}

#[test]
fn compile_runs_bytes_concat_and_slice() {
    let dir = TestDir::new("bytes_one_alloc");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.bytes = by

main()
    const joined: bytes = by.concat(b"abc", b"def")
    const sliced: bytes = by.slice(joined, 1, 4)
    const from_empty: bytes = by.concat(b"", b"z")
    io.println(f"{by.len(joined)}")
    io.println(f"{by.len(sliced)}")
    io.println(f"{by.len(from_empty)}")
end
"#,
    );

    let exe = exe_path(&dir, "bytes_one_alloc");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "leak check failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "6\n3\n1\n",
        "stderr: {stderr}"
    );
}

#[test]
fn compile_runs_bytes_equality_with_embedded_nul_native() {
    let dir = TestDir::new("bytes_equality_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const same_left: bytes = b"A\x00B"
    const same_right: bytes = b"A\x00B"
    const different: bytes = b"A\x00C"
    io.println(string(same_left == same_right))
    io.println(string(same_left != different))
end
"#,
    );

    let exe = exe_path(&dir, "bytes_equality_native");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "bytes equality failed: {stderr}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "true\ntrue\n",
        "stderr: {stderr}"
    );
}
