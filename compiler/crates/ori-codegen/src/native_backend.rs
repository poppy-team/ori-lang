use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use cranelift_codegen::ir::{self, types, AbiParam, BlockArg, InstBuilder, MemFlags};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use ori_ast::expr::{BinaryOp, UnaryOp};
use ori_diagnostics::Span;
use ori_hir::hir::*;
use ori_types::{
    stdlib::{stdlib_func_sig, stdlib_native_abi, stdlib_runtime_functions, StdlibNativeAbiTy},
    substitute_ty_params, OpaqueTy, Ty,
};

mod string_collector;
use string_collector::collect_all_strings;

mod reference_collector;
use reference_collector::collect_function_references;

pub mod jit;

/// Diagnostic codes for violated value contracts.
///
/// These are reported at **runtime** through `ori_panic`, not by the checker,
/// so they are prefixed onto the panic message rather than carried by a
/// `Diagnostic`. They stay named constants so the catalog parity test in
/// `tests/diagnostic_catalog.rs` can see them.
const CONTRACT_PARAM_VIOLATION: &str = "contract.param_violation";
const CONTRACT_FIELD_VIOLATION: &str = "contract.field_violation";

const DEBUG_TYPE_BOOL: i64 = 1;
const DEBUG_TYPE_INT: i64 = 2;
const DEBUG_TYPE_UINT: i64 = 3;
const DEBUG_TYPE_FLOAT: i64 = 4;
const DEBUG_TYPE_FLOAT32: i64 = 5;
const DEBUG_TYPE_MANAGED: i64 = 6;
const DEBUG_MANAGED_STRING: i64 = 1;
const DEBUG_MANAGED_BYTES: i64 = 2;
/// Maximum number of list elements materialized in one debugger snapshot.
/// The cap keeps a breakpoint from turning a large list into unbounded work.
const DEBUG_MAX_LIST_ELEMENTS: usize = 64;
/// Nested list views are intentionally smaller than root list views so a
/// nested aggregate cannot multiply debugger work without a bound.
const DEBUG_MAX_NESTED_LIST_ELEMENTS: usize = 8;

#[cfg(test)]
const INTERNAL_NATIVE_RUNTIME_IMPORTS: &[&str] = &[
    "ori_abort_concurrent_modification",
    "ori_abort_division_by_zero",
    "ori_abort_division_overflow",
    "ori_abort_shift_overflow",
    "ori_host_clear_error",
    "ori_host_error_code",
    "ori_host_error_message",
    "ori_host_report_error",
    "ori_handle_null",
    "ori_handle_is_null",
    "ori_handle_validate_size_type",
    "ori_arc_collect_cycles",
    "ori_arc_maybe_collect_cycles",
    "ori_arc_register_edge",
    "ori_arc_release",
    "ori_arc_retain",
    "ori_arc_unregister_edge",
    "ori_arc_update_edge",
    "ori_alloc",
    "ori_alloc_typed",
    "ori_test_selected",
    "ori_bool_to_string_parts",
    "ori_bytes_copy_parts",
    "ori_bytes_eq",
    "ori_debug_init",
    "ori_debug_enter",
    "ori_debug_leave",
    "ori_debug_line",
    "ori_debug_variable",
    "ori_debug_managed",
    "ori_debug_register_static",
    "ori_list_capacity",
    "ori_list_get",
    "ori_list_len",
    "ori_deque_iterator_new",
    "ori_deque_iterator_next",
    "ori_queue_iterator_new",
    "ori_queue_iterator_next",
    "ori_stack_iterator_new",
    "ori_stack_iterator_next",
    "ori_linked_list_iterator_new",
    "ori_linked_list_iterator_next",
    "ori_doubly_linked_list_iterator_new",
    "ori_doubly_linked_list_iterator_next",
    "ori_heap_iterator_new",
    "ori_heap_iterator_next",
    "ori_graph_iterator_new",
    "ori_graph_iterator_next",
    "ori_executor_drain",
    "ori_executor_run_one",
    "ori_executor_schedule",
    "ori_float_to_string_parts",
    "ori_future_cancel",
    "ori_future_complete_f64",
    "ori_future_complete_i64",
    "ori_future_complete_ptr",
    "ori_future_complete_void",
    "ori_future_fail",
    "ori_future_on_ready",
    "ori_future_pending",
    "ori_future_poll",
    "ori_future_ready_f64",
    "ori_future_ready_i64",
    "ori_future_ready_ptr",
    "ori_future_ready_void",
    "ori_future_value_f64",
    "ori_future_value_i64",
    "ori_future_value_ptr",
    "ori_graph_add_edge_string",
    "ori_graph_add_weighted_edge_string",
    "ori_graph_add_node_string",
    "ori_graph_new_custom",
    "ori_graph_set_eq",
    "ori_graph_bfs_string",
    "ori_graph_dfs_string",
    "ori_graph_edge_weight_string",
    "ori_graph_has_edge_string",
    "ori_graph_has_node_string",
    "ori_graph_neighbors_string",
    "ori_graph_remove_edge_string",
    "ori_graph_remove_node_string",
    "ori_graph_shortest_path_string",
    "ori_graph_shortest_weighted_path_string",
    "ori_hash_table_contains_string",
    "ori_hash_table_contains_custom",
    "ori_hash_table_from_entries_string",
    "ori_hash_table_get_custom",
    "ori_hash_table_get_string",
    "ori_hash_table_new_custom",
    "ori_hash_table_remove_custom",
    "ori_hash_table_remove_string",
    "ori_hash_table_set_custom_with_eq",
    "ori_hash_table_set_custom_with_hash_eq",
    "ori_hash_table_set_string",
    "ori_hash_table_with_capacity_custom",
    "ori_hash_table_new_custom_with_hash",
    "ori_hash_table_with_capacity_custom_with_hash",
    "ori_hash_i64",
    "ori_hash_string",
    "ori_hash_bytes",
    "ori_heap_from_list_custom",
    "ori_heap_from_list_string",
    "ori_heap_new_custom",
    "ori_heap_new_string",
    "ori_heap_push_custom",
    "ori_heap_push_string",
    "ori_heap_remove_custom",
    "ori_heap_remove_string",
    "ori_int_to_cstr",
    "ori_doubly_linked_list_find_string",
    "ori_linked_list_find_string",
    "ori_map_contains_custom",
    "ori_map_contains_string",
    "ori_map_get_custom",
    "ori_map_get_string",
    "ori_map_key_at",
    "ori_map_new_custom",
    "ori_map_set_custom_with_eq",
    "ori_map_set_custom_with_hash_eq",
    "ori_map_remove_custom",
    "ori_map_remove_string",
    "ori_map_set_custom",
    "ori_map_set_string",
    "ori_map_from_entries_string",
    "ori_map_from_entries_custom",
    "ori_map_from_entries_custom_with_hash",
    "ori_map_try_get_custom",
    "ori_map_try_get_string",
    "ori_map_try_remove_custom",
    "ori_map_try_remove_string",
    "ori_map_value_at",
    "ori_math_abs_float",
    "ori_math_max_float",
    "ori_math_min_float",
    "ori_new_result",
    "ori_os_set_args",
    "ori_set_add_string",
    "ori_set_add_custom_with_eq",
    "ori_set_contains_custom",
    "ori_set_from_list_custom",
    "ori_set_new_custom",
    "ori_set_new_custom_with_hash",
    "ori_set_add_custom_with_hash_eq",
    "ori_set_from_list_custom_with_hash",
    "ori_set_remove_custom",
    "ori_set_try_remove_custom",
    "ori_set_contains_string",
    "ori_set_from_list_string",
    "ori_set_remove_string",
    "ori_set_try_remove_string",
    "ori_tree_find_string",
    "ori_string_copy_host",
    "ori_string_concat_parts",
    "ori_to_string_parts",
    "ori_uint_to_cstr",
    "ori_uint_to_string_parts",
];

// == Type mapping ==

fn cl_type(ty: &Ty, ptr_ty: types::Type) -> Option<types::Type> {
    match ty {
        Ty::Bool => Some(types::I8),
        Ty::Int | Ty::Int64 | Ty::U64 => Some(types::I64),
        Ty::Int32 | Ty::U32 => Some(types::I32),
        Ty::Int16 | Ty::U16 => Some(types::I16),
        Ty::Int8 | Ty::U8 => Some(types::I8),
        Ty::Float | Ty::Float64 => Some(types::F64),
        Ty::Float32 => Some(types::F32),
        Ty::Simd(elem, lanes) => match (&**elem, *lanes) {
            (Ty::Float32, 4) => Some(types::F32X4),
            (Ty::Float32, 2) => Some(types::F32X2),
            (Ty::Float32, 8) => Some(types::F32X8),
            (Ty::Int32 | Ty::U32, 4) => Some(types::I32X4),
            (Ty::Int32 | Ty::U32, 2) => Some(types::I32X2),
            (Ty::Int32 | Ty::U32, 8) => Some(types::I32X8),
            (Ty::Float64, 2) => Some(types::F64X2),
            (Ty::Int64 | Ty::U64 | Ty::Int, 2) => Some(types::I64X2),
            (Ty::Int16 | Ty::U16, 8) => Some(types::I16X8),
            (Ty::Int8 | Ty::U8, 16) => Some(types::I8X16),
            _ => Some(types::F32X4),
        },
        Ty::String
        | Ty::Bytes
        | Ty::Func { .. }
        | Ty::Lazy(_)
        | Ty::Handle(_)
        | Ty::Future(_)
        | Ty::TaskJob(_)
        | Ty::Channel(_)
        | Ty::AtomicInt
        | Ty::TaskJoinError
        | Ty::ChannelSendError
        | Ty::ChannelReceiveError => Some(ptr_ty),
        Ty::Opaque {
            kind: OpaqueTy::NodeId,
            ..
        } => Some(types::I64),
        Ty::Opaque { .. } => Some(ptr_ty),
        Ty::Any(_) => Some(ptr_ty),
        Ty::Optional(_)
        | Ty::Result(_, _)
        | Ty::List(_)
        | Ty::Map(_, _)
        | Ty::Set(_)
        | Ty::Range(_)
        | Ty::Tuple(_) => Some(ptr_ty),
        Ty::Void | Ty::Never => None,
        Ty::Named(_, _) => Some(ptr_ty),
        Ty::Infer(_) => Some(types::I64),
        _ => Some(types::I64),
    }
}

fn native_codegen_unsupported(message: impl Into<String>) -> String {
    const CODE: &str = "backend.native_unsupported";
    format!("{CODE}: {}", message.into())
}

fn custom_destructor_callback_name(type_def_id: ori_types::DefId) -> SmolStr {
    SmolStr::new(format!("__ori_destructor_{}", type_def_id.0))
}

fn custom_destructor_impls(hir: &HirModule) -> Vec<(ori_types::DefId, SmolStr)> {
    let destructor_trait_ids = hir
        .traits
        .iter()
        .filter(|trait_decl| trait_decl.name == "ori.core.Destructor")
        .map(|trait_decl| trait_decl.def_id)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    hir.trait_impls
        .iter()
        .filter(|implementation| destructor_trait_ids.contains(&implementation.trait_def_id))
        .filter(|implementation| seen.insert(implementation.type_def_id))
        .filter_map(|implementation| {
            implementation
                .methods
                .iter()
                .find(|method| method.name == "destroy")
                .map(|method| (implementation.type_def_id, method.func_name.clone()))
        })
        .collect()
}

fn cl_stdlib_abi_type(ty: StdlibNativeAbiTy, ptr_ty: types::Type) -> types::Type {
    match ty {
        StdlibNativeAbiTy::Ptr => ptr_ty,
        StdlibNativeAbiTy::I64 => types::I64,
        StdlibNativeAbiTy::I32 => types::I32,
        StdlibNativeAbiTy::I8 => types::I8,
        StdlibNativeAbiTy::F64 => types::F64,
    }
}

fn is_managed_ty(ty: &Ty) -> bool {
    ty.is_runtime_managed()
}

fn debug_scalar_type_tag(ty: &Ty) -> Option<i64> {
    match ty {
        Ty::Bool => Some(DEBUG_TYPE_BOOL),
        Ty::Int | Ty::Int8 | Ty::Int16 | Ty::Int32 | Ty::Int64 => Some(DEBUG_TYPE_INT),
        Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 => Some(DEBUG_TYPE_UINT),
        Ty::Float | Ty::Float64 => Some(DEBUG_TYPE_FLOAT),
        Ty::Float32 => Some(DEBUG_TYPE_FLOAT32),
        _ => None,
    }
}

fn collect_debug_pattern_names(pattern: &HirPattern, names: &mut HashSet<SmolStr>) {
    match pattern {
        HirPattern::Binding(name, _) => {
            names.insert(name.clone());
        }
        HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
            collect_debug_pattern_names(inner, names);
        }
        HirPattern::Variant { fields, .. } => {
            for (_, field) in fields {
                collect_debug_pattern_names(field, names);
            }
        }
        HirPattern::Tuple(elements) | HirPattern::Or(elements) => {
            for element in elements {
                collect_debug_pattern_names(element, names);
            }
        }
        HirPattern::Wildcard
        | HirPattern::BoolLit(_)
        | HirPattern::IntLit(_)
        | HirPattern::StrLit(_)
        | HirPattern::None_ => {}
    }
}

fn collect_debug_block_names(
    block: &HirBlock,
    names: &mut HashSet<SmolStr>,
    types: &mut HashMap<SmolStr, Ty>,
) {
    collect_debug_statement_names(&block.stmts, names, types);
}

fn collect_debug_statement_names(
    statements: &[HirStmt],
    names: &mut HashSet<SmolStr>,
    types: &mut HashMap<SmolStr, Ty>,
) {
    for statement in statements {
        match statement {
            HirStmt::Let { name, ty, .. } | HirStmt::Using { name, ty, .. } => {
                names.insert(name.clone());
                types.insert(name.clone(), ty.clone());
            }
            HirStmt::If {
                then,
                else_ifs,
                else_,
                ..
            } => {
                collect_debug_block_names(then, names, types);
                for (_, block) in else_ifs {
                    collect_debug_block_names(block, names, types);
                }
                if let Some(block) = else_ {
                    collect_debug_block_names(block, names, types);
                }
            }
            HirStmt::While { body, .. }
            | HirStmt::Loop { body, .. }
            | HirStmt::Repeat { body, .. }
            | HirStmt::WhileSome { body, .. } => {
                collect_debug_block_names(body, names, types);
            }
            HirStmt::For {
                binding,
                index_binding,
                elem_ty,
                body,
                ..
            } => {
                names.insert(binding.clone());
                if let Some(index_binding) = index_binding {
                    names.insert(index_binding.clone());
                    types.insert(index_binding.clone(), Ty::Int);
                }
                types.insert(binding.clone(), elem_ty.clone());
                collect_debug_block_names(body, names, types);
            }
            HirStmt::Match { arms, .. } => {
                for arm in arms {
                    collect_debug_pattern_names(&arm.pattern, names);
                    collect_debug_statement_names(&arm.body, names, types);
                }
            }
            HirStmt::IfSome {
                binding,
                inner_ty,
                then,
                else_,
                ..
            } => {
                names.insert(binding.clone());
                types.insert(binding.clone(), inner_ty.clone());
                collect_debug_block_names(then, names, types);
                if let Some(block) = else_ {
                    collect_debug_block_names(block, names, types);
                }
            }
            HirStmt::Assign { .. }
            | HirStmt::Return(_, _)
            | HirStmt::Break(_)
            | HirStmt::Continue(_)
            | HirStmt::Expr(_)
            | HirStmt::Check { .. } => {}
        }
    }
}

fn collect_debug_names(hir: &HirModule) -> HashSet<SmolStr> {
    let mut names = HashSet::new();
    let mut types = HashMap::new();
    for function in &hir.funcs {
        names.insert(function.name.clone());
        for param in &function.params {
            names.insert(param.name.clone());
            types.insert(param.name.clone(), param.ty.clone());
        }
        for capture in &function.closure_captures {
            names.insert(capture.name.clone());
            types.insert(capture.name.clone(), capture.ty.clone());
        }
        collect_debug_block_names(&function.body, &mut names, &mut types);
    }

    // Aggregate paths are emitted ahead of function lowering so the native
    // backend can reference stable string data from Cranelift. The paths are
    // intentionally generated for every visible binding and every declared
    // struct; the code generator still checks the concrete type before using a
    // path, so unused combinations have no runtime cost.
    let base_names: Vec<SmolStr> = names.iter().cloned().collect();
    let struct_paths = collect_debug_struct_field_paths(hir);
    let enum_paths = collect_debug_enum_field_paths(hir);
    for base in base_names {
        names.insert(SmolStr::new(format!("{base}.length")));
        names.insert(SmolStr::new(format!("{base}.capacity")));
        names.insert(SmolStr::new(format!("{base}.has_value")));
        names.insert(SmolStr::new(format!("{base}.is_ok")));
        names.insert(SmolStr::new(format!("{base}.tag")));
        names.insert(SmolStr::new(format!("{base}.value")));
        names.insert(SmolStr::new(format!("{base}.ok")));
        names.insert(SmolStr::new(format!("{base}.error")));
        for index in 0..8 {
            names.insert(SmolStr::new(format!("{base}.{index}")));
        }
        for child in ["keys", "values", "items"] {
            let child = format!("{base}.{child}");
            names.insert(SmolStr::new(child.clone()));
            names.insert(SmolStr::new(format!("{child}.length")));
            names.insert(SmolStr::new(format!("{child}.capacity")));
            collect_debug_index_paths(&child, &mut names, &struct_paths, &enum_paths, 0);
        }
        for path in &struct_paths {
            names.insert(SmolStr::new(format!("{base}.{path}")));
        }
        for path in &enum_paths {
            names.insert(SmolStr::new(format!("{base}.{path}")));
        }
        if let Some(ty) = types.get(&base) {
            collect_debug_typed_index_paths(&base, ty, &mut names, &struct_paths, &enum_paths, 0);
        } else {
            collect_debug_index_paths(&base, &mut names, &struct_paths, &enum_paths, 0);
        }
    }
    names
}

fn collect_debug_typed_index_paths(
    base: &str,
    ty: &Ty,
    names: &mut HashSet<SmolStr>,
    struct_paths: &[SmolStr],
    enum_paths: &[SmolStr],
    depth: u8,
) {
    let Ty::List(element_ty) = ty else {
        collect_debug_index_paths(base, names, struct_paths, enum_paths, depth);
        return;
    };
    let max_elements = if depth == 0 {
        DEBUG_MAX_LIST_ELEMENTS
    } else {
        DEBUG_MAX_NESTED_LIST_ELEMENTS
    };
    for index in 0..max_elements {
        let element = format!("{base}[{index}]");
        names.insert(SmolStr::new(element.clone()));
        for path in struct_paths {
            names.insert(SmolStr::new(format!("{element}.{path}")));
        }
        for path in enum_paths {
            names.insert(SmolStr::new(format!("{element}.{path}")));
        }
        if matches!(element_ty.as_ref(), Ty::List(_)) && depth < 3 {
            collect_debug_typed_index_paths(
                &element,
                element_ty,
                names,
                struct_paths,
                enum_paths,
                depth + 1,
            );
        }
    }
}

fn collect_debug_index_paths(
    base: &str,
    names: &mut HashSet<SmolStr>,
    struct_paths: &[SmolStr],
    enum_paths: &[SmolStr],
    depth: u8,
) {
    let max_elements = if depth == 0 {
        DEBUG_MAX_LIST_ELEMENTS
    } else {
        DEBUG_MAX_NESTED_LIST_ELEMENTS
    };
    for index in 0..max_elements {
        let element = format!("{base}[{index}]");
        names.insert(SmolStr::new(element.clone()));
        for path in struct_paths {
            names.insert(SmolStr::new(format!("{element}.{path}")));
        }
        for path in enum_paths {
            names.insert(SmolStr::new(format!("{element}.{path}")));
        }
        // The names table is shared by every visible binding. Keep the
        // precomputed tree to one nested level so debug instrumentation stays
        // cheap even in large functions; the emitter remains depth-capped and
        // null-safe for deeper aggregates.
        if depth == 0 {
            for nested_index in 0..DEBUG_MAX_NESTED_LIST_ELEMENTS {
                names.insert(SmolStr::new(format!("{element}[{nested_index}]")));
            }
        }
    }
}

/// Collect direct and nested struct field paths (for example `address.city`).
/// Recursive type graphs are bounded by the visited set so a self-referential
/// struct cannot make debug metadata generation loop forever.
fn collect_debug_struct_field_paths(hir: &HirModule) -> Vec<SmolStr> {
    fn visit_struct(
        def_id: ori_types::DefId,
        prefix: &str,
        hir: &HirModule,
        paths: &mut HashSet<SmolStr>,
        visiting: &mut HashSet<ori_types::DefId>,
    ) {
        if !visiting.insert(def_id) {
            return;
        }
        let Some(structure) = hir
            .structs
            .iter()
            .find(|structure| structure.def_id == def_id)
        else {
            visiting.remove(&def_id);
            return;
        };
        for field in &structure.fields {
            let path = if prefix.is_empty() {
                field.name.to_string()
            } else {
                format!("{prefix}.{}", field.name)
            };
            paths.insert(SmolStr::new(path.clone()));
            if let Ty::Named(nested_id, _) = field.ty {
                visit_struct(nested_id, &path, hir, paths, visiting);
            }
        }
        visiting.remove(&def_id);
    }

    let mut paths = HashSet::new();
    for structure in &hir.structs {
        visit_struct(structure.def_id, "", hir, &mut paths, &mut HashSet::new());
    }
    paths.into_iter().collect()
}

fn collect_debug_enum_field_paths(hir: &HirModule) -> Vec<SmolStr> {
    hir.enums
        .iter()
        .flat_map(|enum_decl| {
            enum_decl.variants.iter().flat_map(|variant| {
                variant
                    .fields
                    .iter()
                    .map(move |field| SmolStr::new(format!("{}.{}", variant.name, field.name)))
            })
        })
        .collect()
}

fn is_c_export_scalar_field_ty(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Bool
            | Ty::Int
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Float
            | Ty::Float32
            | Ty::Float64
    )
}

#[derive(Clone)]
enum CExportParameterBridge {
    Direct,
    ScalarStruct {
        internal: StructLayout,
        external: StructLayout,
    },
    Optional {
        inner: CExportPayloadBridge,
    },
    Result {
        ok: Option<CExportPayloadBridge>,
        err: Option<CExportPayloadBridge>,
    },
}

#[derive(Clone)]
enum CExportReturnBridge {
    Direct,
    Bytes,
    ScalarStruct {
        internal: StructLayout,
        external: StructLayout,
    },
    Optional {
        inner: CExportPayloadBridge,
    },
    Result {
        ok: Option<CExportPayloadBridge>,
        err: Option<CExportPayloadBridge>,
    },
}

#[derive(Clone)]
struct CExportPayloadBridge {
    ty: Ty,
    scalar_struct_layouts: Option<(StructLayout, StructLayout)>,
    /// Payload size for opaque managed handles. Scalar-only structs use the
    /// by-value bridge and therefore leave this unset.
    managed_handle_size: Option<u32>,
    /// Reserved non-zero source-type tag for opaque managed handles.
    managed_handle_type_tag: Option<i64>,
}

#[derive(Clone, Copy)]
struct CExportHandleValidation {
    size_type: Option<ir::FuncRef>,
    payload_size: Option<u32>,
    payload_type_tag: Option<i64>,
}

/// Scalar list elements stored as raw `i64` slots (no ARC edge on push/get).
/// Floats are excluded: storage uses integer slots via bitcast, handled by the
/// runtime call path until a dedicated bitcast path is added.
fn is_list_inline_scalar_elem(ty: &Ty) -> bool {
    !is_managed_ty(ty)
        && matches!(
            ty,
            Ty::Bool
                | Ty::Int
                | Ty::Int8
                | Ty::Int16
                | Ty::Int32
                | Ty::Int64
                | Ty::U8
                | Ty::U16
                | Ty::U32
                | Ty::U64
        )
}

/// `OriList` field offsets for the native `repr(C)` layout:
/// `{ data: *i64, len: i64, cap: i64, version: i64 }`.
const ORI_LIST_DATA_OFFSET: i32 = 0;
const ORI_LIST_LEN_OFFSET: i32 = 8;
const ORI_LIST_CAP_OFFSET: i32 = 16;
const ORI_LIST_VERSION_OFFSET: i32 = 24;

/// Opaque I/O handles must survive across `await` even when liveness analysis
/// misses uses in nested match arms (e.g. `await write_all_async(client, …)` then
/// `read_some(client)`). Auto-dropping them yields "invalid connection".
fn is_async_keep_alive_resource_ty(ty: &Ty) -> bool {
    match ty {
        Ty::Opaque { kind, .. } => matches!(
            kind,
            OpaqueTy::Connection
                | OpaqueTy::Listener
                | OpaqueTy::File
                | OpaqueTy::Input
                | OpaqueTy::Output
                | OpaqueTy::UdpSocket
        ),
        // Keep Result-wrapped handles if analysis stores the outer result binding.
        Ty::Result(ok, _) => is_async_keep_alive_resource_ty(ok),
        Ty::Optional(inner) => is_async_keep_alive_resource_ty(inner),
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct StringParts {
    ptr: ir::Value,
    len: ir::Value,
}

fn const_static_bytes(expr: &HirExpr, ty: &Ty) -> Option<Vec<u8>> {
    match (&expr.kind, ty) {
        (HirExprKind::BoolLit(value), Ty::Bool) => Some(vec![if *value { 1 } else { 0 }]),
        (HirExprKind::IntLit(value), Ty::Int8 | Ty::U8) => Some(vec![*value as u8]),
        (HirExprKind::IntLit(value), Ty::Int16 | Ty::U16) => {
            Some((*value as i16).to_le_bytes().to_vec())
        }
        (HirExprKind::IntLit(value), Ty::Int32 | Ty::U32) => {
            Some((*value as i32).to_le_bytes().to_vec())
        }
        (HirExprKind::IntLit(value), Ty::Int | Ty::Int64 | Ty::U64) => {
            Some(value.to_le_bytes().to_vec())
        }
        (HirExprKind::FloatLit(value), Ty::Float32) => Some((*value as f32).to_le_bytes().to_vec()),
        (HirExprKind::FloatLit(value), Ty::Float | Ty::Float64) => {
            Some(value.to_le_bytes().to_vec())
        }
        _ => None,
    }
}

fn needs_runtime_global_init(expr: &HirExpr, ty: &Ty) -> bool {
    const_static_bytes(expr, ty).is_none()
}

/// Per-module object emission is safe only when initialization does not need
/// to be coordinated across object files. Dynamic globals still use the
/// monolithic path until the linker can call every module initializer.
pub fn has_runtime_global_initializers(hir: &HirModule) -> bool {
    hir.consts
        .iter()
        .any(|constant| needs_runtime_global_init(&constant.value, &constant.ty))
}

fn is_float_ty(ty: &Ty) -> bool {
    match ty {
        Ty::Float | Ty::Float32 | Ty::Float64 => true,
        Ty::Simd(elem, _) => is_float_ty(elem),
        _ => false,
    }
}

fn validate_native_hir(hir: &HirModule) -> Result<(), String> {
    NativeHirValidator::new(hir)
        .module(hir)
        .map_err(|err| format!("invalid HIR for native backend: {err}"))
}

struct NativeHirValidator {
    type_names: HashMap<ori_types::DefId, SmolStr>,
}

impl NativeHirValidator {
    fn new(hir: &HirModule) -> Self {
        let type_names = hir
            .structs
            .iter()
            .map(|item| (item.def_id, item.name.clone()))
            .chain(
                hir.enums
                    .iter()
                    .map(|item| (item.def_id, item.name.clone())),
            )
            .collect();
        Self { type_names }
    }

    fn display_ty(&self, ty: &Ty) -> String {
        ty.display_with_names(&self.type_names)
    }

    fn module(&self, hir: &HirModule) -> Result<(), String> {
        for s in &hir.structs {
            for field in &s.fields {
                self.reject_error_ty(&field.ty, "struct field type", field.span)?;
                if let Some(contract) = &field.contract {
                    self.expr(contract)?;
                }
            }
        }
        for e in &hir.enums {
            for variant in &e.variants {
                for field in &variant.fields {
                    self.reject_error_ty(&field.ty, "enum variant field type", field.span)?;
                    if let Some(contract) = &field.contract {
                        self.expr(contract)?;
                    }
                }
            }
        }
        for t in &hir.traits {
            for method in &t.methods {
                for param in &method.params {
                    self.reject_error_ty(param, "trait method parameter type", Span::DUMMY)?;
                }
                self.reject_error_ty(&method.return_ty, "trait method return type", Span::DUMMY)?;
            }
        }
        for f in &hir.funcs {
            for param in &f.params {
                self.reject_error_ty(&param.ty, "function parameter type", param.span)?;
                if let Some(default) = &param.default {
                    self.expr(default)?;
                }
                if let Some(contract) = &param.contract {
                    self.expr(contract)?;
                }
            }
            self.reject_error_ty(&f.return_ty, "function return type", f.span)?;
            for capture in &f.closure_captures {
                self.reject_error_ty(&capture.ty, "closure capture type", f.span)?;
            }
            self.block(&f.body)?;
        }
        for c in &hir.consts {
            self.reject_error_ty(&c.ty, "const type", c.span)?;
            self.expr(&c.value)?;
        }
        for ext in &hir.externs {
            match ext {
                HirExtern::Func {
                    path: _,
                    params,
                    return_ty,
                    span,
                    ..
                } => {
                    for param in params {
                        self.reject_error_ty(
                            &param.ty,
                            "extern function parameter type",
                            param.span,
                        )?;
                    }
                    self.reject_error_ty(return_ty, "extern function return type", *span)?;
                }
                HirExtern::Var { ty, span, .. } => {
                    self.reject_error_ty(ty, "extern variable type", *span)?;
                }
            }
        }
        Ok(())
    }

    fn block(&self, block: &HirBlock) -> Result<(), String> {
        for stmt in &block.stmts {
            self.stmt(stmt)?;
        }
        Ok(())
    }

    fn stmt(&self, stmt: &HirStmt) -> Result<(), String> {
        match stmt {
            HirStmt::Let {
                ty, value, span, ..
            } => {
                self.reject_error_ty(ty, "let binding type", *span)?;
                self.expr(value)?;
            }
            HirStmt::Assign { value, .. } | HirStmt::Return(Some(value), _) => {
                self.expr(value)?;
            }
            HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
            HirStmt::Expr(expr) => self.expr(expr)?,
            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                self.expr(cond)?;
                self.expect_bool(&cond.ty, "if condition", cond.span)?;
                self.block(then)?;
                for (else_cond, else_block) in else_ifs {
                    self.expr(else_cond)?;
                    self.expect_bool(&else_cond.ty, "else-if condition", else_cond.span)?;
                    self.block(else_block)?;
                }
                if let Some(block) = else_ {
                    self.block(block)?;
                }
            }
            HirStmt::While { cond, body, .. } => {
                self.expr(cond)?;
                self.expect_bool(&cond.ty, "while condition", cond.span)?;
                self.block(body)?;
            }
            HirStmt::For {
                elem_ty,
                iterable,
                body,
                span,
                ..
            } => {
                self.reject_error_ty(elem_ty, "for element type", *span)?;
                self.expr(iterable)?;
                self.block(body)?;
            }
            HirStmt::Loop { body, .. } => self.block(body)?,
            HirStmt::Repeat { count, body, .. } => {
                self.expr(count)?;
                self.expect_integer(&count.ty, "repeat count", count.span)?;
                self.block(body)?;
            }
            HirStmt::Match {
                scrutinee, arms, ..
            } => {
                self.expr(scrutinee)?;
                for arm in arms {
                    self.pattern(&arm.pattern, arm.span)?;
                    if let Some(guard) = &arm.guard {
                        self.expr(guard)?;
                    }
                    for stmt in &arm.body {
                        self.stmt(stmt)?;
                    }
                }
            }
            HirStmt::IfSome {
                inner_ty,
                value,
                then,
                else_,
                span,
                ..
            } => {
                self.reject_error_ty(inner_ty, "if-some binding type", *span)?;
                self.expr(value)?;
                self.block(then)?;
                if let Some(block) = else_ {
                    self.block(block)?;
                }
            }
            HirStmt::WhileSome {
                inner_ty,
                value,
                body,
                span,
                ..
            } => {
                self.reject_error_ty(inner_ty, "while-some binding type", *span)?;
                self.expr(value)?;
                self.block(body)?;
            }
            HirStmt::Using {
                ty, value, span, ..
            } => {
                self.reject_error_ty(ty, "using binding type", *span)?;
                self.expr(value)?;
            }
            HirStmt::Check { condition, .. } => {
                self.expr(condition)?;
                self.expect_bool(&condition.ty, "check condition", condition.span)?;
            }
        }
        Ok(())
    }

    fn expr(&self, expr: &HirExpr) -> Result<(), String> {
        self.reject_error_ty(&expr.ty, "expression type", expr.span)?;
        match &expr.kind {
            HirExprKind::BoolLit(_)
            | HirExprKind::IntLit(_)
            | HirExprKind::FloatLit(_)
            | HirExprKind::StrLit(_)
            | HirExprKind::BytesLit(_)
            | HirExprKind::Unit
            | HirExprKind::None_
            | HirExprKind::Var(_) => {}
            HirExprKind::InterpolatedStr(parts) => {
                for part in parts {
                    if let HirStrPart::Expr(expr) = part {
                        self.expr(expr)?;
                    }
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                match op {
                    BinaryOp::And | BinaryOp::Or => {
                        self.expect_bool(&lhs.ty, "logical operator left operand", lhs.span)?;
                        self.expect_bool(&rhs.ty, "logical operator right operand", rhs.span)?;
                        self.expect_bool(&expr.ty, "logical operator result", expr.span)?;
                    }
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => {
                        self.expect_bool(&expr.ty, "comparison result", expr.span)?;
                    }
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => {}
                    // GFX-BITWISE-1: bitwise/shift operands are integers.
                    BinaryOp::Band
                    | BinaryOp::Bor
                    | BinaryOp::Bxor
                    | BinaryOp::Shl
                    | BinaryOp::Shr => {}
                }
            }
            HirExprKind::Unary { op, operand } => {
                self.expr(operand)?;
                if matches!(op, UnaryOp::Not) {
                    self.expect_bool(&operand.ty, "not operand", operand.span)?;
                    self.expect_bool(&expr.ty, "not result", expr.span)?;
                }
            }
            HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
                self.expr(object)?;
            }
            HirExprKind::Index { object, index } => {
                self.expr(object)?;
                self.expr(index)?;
                self.expect_integer(&index.ty, "index expression", index.span)?;
            }
            HirExprKind::Call { callee, args } => {
                self.expr(callee)?;
                for arg in args {
                    self.expr(&arg.value)?;
                }
            }
            HirExprKind::MethodCall { receiver, args, .. } => {
                self.expr(receiver)?;
                for arg in args {
                    self.expr(arg)?;
                }
            }
            HirExprKind::AssociatedCall { args, .. } => {
                for arg in args {
                    self.expr(arg)?;
                }
            }
            HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
                for (_, value) in fields {
                    self.expr(value)?;
                }
            }
            HirExprKind::ArrayLit { elem_ty, elements } => {
                self.reject_error_ty(elem_ty, "array element type", expr.span)?;
                for element in elements {
                    self.expr(element)?;
                }
            }
            HirExprKind::SimdLit {
                elem_ty, elements, ..
            } => {
                self.reject_error_ty(elem_ty, "simd lane type", expr.span)?;
                for element in elements {
                    self.expr(element)?;
                }
            }
            HirExprKind::ListLit { elem_ty, elements } => {
                self.reject_error_ty(elem_ty, "list element type", expr.span)?;
                for element in elements {
                    self.expr(element)?;
                }
            }
            HirExprKind::ListSpreadLit { elem_ty, elements } => {
                self.reject_error_ty(elem_ty, "list element type", expr.span)?;
                for element in elements {
                    self.expr(&element.value)?;
                }
            }
            HirExprKind::TupleLit(elements) => {
                for element in elements {
                    self.expr(element)?;
                }
            }
            HirExprKind::Some_(inner)
            | HirExprKind::Ok_(inner)
            | HirExprKind::Err_(inner)
            | HirExprKind::Propagate(inner)
            | HirExprKind::Await(inner) => self.expr(inner)?,
            HirExprKind::IfExpr { cond, then, else_ } => {
                self.expr(cond)?;
                self.expect_bool(&cond.ty, "if expression condition", cond.span)?;
                self.expr(then)?;
                self.expr(else_)?;
            }
            HirExprKind::MatchExpr { scrutinee, arms } => {
                self.expr(scrutinee)?;
                for arm in arms {
                    self.pattern(&arm.pattern, arm.span)?;
                    if let Some(guard) = &arm.guard {
                        self.expr(guard)?;
                        self.expect_bool(&guard.ty, "match guard", guard.span)?;
                    }
                    self.expr(&arm.body)?;
                }
            }
            HirExprKind::Range { start, end } => {
                self.expr(start)?;
                self.expr(end)?;
                self.expect_integer(&start.ty, "range start", start.span)?;
                self.expect_integer(&end.ty, "range end", end.span)?;
            }
            HirExprKind::MapLit {
                key_ty,
                value_ty,
                entries,
            } => {
                self.reject_error_ty(key_ty, "map key type", expr.span)?;
                self.reject_error_ty(value_ty, "map value type", expr.span)?;
                for (key, value) in entries {
                    self.expr(key)?;
                    self.expr(value)?;
                }
            }
            HirExprKind::SetLit { elem_ty, elements } => {
                self.reject_error_ty(elem_ty, "set element type", expr.span)?;
                for element in elements {
                    self.expr(element)?;
                }
            }
            HirExprKind::StructUpdate { base, updates, .. } => {
                self.expr(base)?;
                for (_, value) in updates {
                    self.expr(value)?;
                }
            }
            HirExprKind::Closure { captures, .. } => {
                for capture in captures {
                    self.reject_error_ty(&capture.ty, "closure capture type", expr.span)?;
                }
            }
            HirExprKind::IsCheck { value, check_ty } => {
                self.expr(value)?;
                self.reject_error_ty(check_ty, "is-check type", expr.span)?;
                self.expect_bool(&expr.ty, "is-check result", expr.span)?;
            }
        }
        Ok(())
    }

    fn pattern(&self, pattern: &HirPattern, span: Span) -> Result<(), String> {
        match pattern {
            HirPattern::Binding(_, ty) => self.reject_error_ty(ty, "pattern binding type", span)?,
            HirPattern::Or(alternatives) => {
                for alternative in alternatives {
                    self.pattern(alternative, span)?;
                }
            }
            HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
                self.pattern(inner, span)?;
            }
            HirPattern::Variant { fields, .. } => {
                for (_, field_pattern) in fields {
                    self.pattern(field_pattern, span)?;
                }
            }
            HirPattern::Tuple(items) => {
                for item in items {
                    self.pattern(item, span)?;
                }
            }
            HirPattern::Wildcard
            | HirPattern::BoolLit(_)
            | HirPattern::IntLit(_)
            | HirPattern::StrLit(_)
            | HirPattern::None_ => {}
        }
        Ok(())
    }

    fn expect_bool(&self, ty: &Ty, context: &str, span: Span) -> Result<(), String> {
        if matches!(ty, Ty::Bool) {
            Ok(())
        } else {
            Err(format!(
                "{context} must be bool, got `{}` at {span}",
                self.display_ty(ty)
            ))
        }
    }

    fn expect_integer(&self, ty: &Ty, context: &str, span: Span) -> Result<(), String> {
        if ty.is_integer() {
            Ok(())
        } else {
            Err(format!(
                "{context} must be an integer, got `{}` at {span}",
                self.display_ty(ty)
            ))
        }
    }

    fn reject_error_ty(&self, ty: &Ty, context: &str, span: Span) -> Result<(), String> {
        if contains_error_ty(ty) {
            Err(format!(
                "{context} contains unresolved error type at {span}"
            ))
        } else {
            Ok(())
        }
    }
}

fn contains_error_ty(ty: &Ty) -> bool {
    match ty {
        Ty::Error => true,
        Ty::Optional(inner)
        | Ty::List(inner)
        | Ty::Set(inner)
        | Ty::Range(inner)
        | Ty::Lazy(inner)
        | Ty::Handle(inner)
        | Ty::Future(inner)
        | Ty::TaskJob(inner)
        | Ty::Channel(inner) => contains_error_ty(inner),
        Ty::Result(ok, err) | Ty::Map(ok, err) => contains_error_ty(ok) || contains_error_ty(err),
        Ty::Opaque { args, .. } => args.iter().any(contains_error_ty),
        Ty::Tuple(items) => items.iter().any(contains_error_ty),
        Ty::Func { params, ret } => params.iter().any(contains_error_ty) || contains_error_ty(ret),
        Ty::Named(_, args) => args.iter().any(contains_error_ty),
        _ => false,
    }
}

fn mangle_symbol(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 2);
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if c == '.' {
            out.push_str("_dot_");
        } else {
            use std::fmt::Write;
            write!(&mut out, "_x{:02x}_", c as u32).unwrap();
        }
    }
    out
}

pub fn native_func_symbol(name: &str) -> String {
    format!("ORI__{}", mangle_symbol(name))
}

pub fn native_func_wrapper_symbol(name: &str) -> String {
    format!("ORI__{}__fnptr_wrapper", mangle_symbol(name))
}

pub fn native_global_symbol(name: &str) -> String {
    format!("ORI_GLOBAL__{}", mangle_symbol(name))
}

fn is_entry_main(hir: &HirModule, f: &HirFunc) -> bool {
    let entry = if hir.namespace.is_empty() {
        "main".to_string()
    } else {
        format!("{}.main", hir.namespace)
    };
    f.params.is_empty() && f.name.as_str() == entry
}

fn is_synthetic_closure_func(f: &HirFunc) -> bool {
    f.params
        .first()
        .is_some_and(|param| param.name.as_str() == "__env")
        && f.name.contains(".__closure_")
}

fn async_step_name(f: &HirFunc) -> SmolStr {
    SmolStr::new(format!("{}.__async_step", f.name))
}

fn async_inner_return_ty(f: &HirFunc) -> Option<Ty> {
    match &f.return_ty {
        Ty::Future(inner) => Some(inner.as_ref().clone()),
        _ => None,
    }
}

#[derive(Clone)]
struct SimpleAsyncBinding {
    name: SmolStr,
    ty: Ty,
}

#[derive(Clone)]
struct SimpleAsyncParam {
    name: SmolStr,
    ty: Ty,
}

#[derive(Clone)]
struct SimpleAsyncLocal {
    name: SmolStr,
    ty: Ty,
    value: HirExpr,
}

#[derive(Clone)]
struct SimpleAsyncAwaitStep {
    await_future: HirExpr,
    binding: Option<SimpleAsyncBinding>,
    propagate_result_ty: Option<Ty>,
}

#[derive(Clone)]
struct SimpleAsyncStateMachinePlan {
    params: Vec<SimpleAsyncParam>,
    locals: Vec<SimpleAsyncLocal>,
    awaits: Vec<SimpleAsyncAwaitStep>,
    tail_stmts: Vec<HirStmt>,
    return_expr: Option<HirExpr>,
    tail_expr: Option<HirExpr>,
    inner_ty: Ty,
    is_general: bool,
}

const ASYNC_FRAME_STATE_OFFSET: i32 = 0;
const ASYNC_FRAME_RESULT_OFFSET: i32 = 8;
const ASYNC_FRAME_AWAITED_BASE_OFFSET: u32 = 16;

fn align_u32(value: u32, align: u8) -> u32 {
    let align = align.max(1) as u32;
    (value + align - 1) & !(align - 1)
}

fn simple_async_frame_binding_offset(
    plan: &SimpleAsyncStateMachinePlan,
    step_index: usize,
    ptr_ty: types::Type,
) -> Option<u32> {
    let binding = plan.awaits.get(step_index)?.binding.as_ref()?;
    let mut offset = simple_async_frame_param_base_offset(plan, ptr_ty);
    for (param_index, param) in plan.params.iter().enumerate() {
        let param_offset = simple_async_frame_param_offset(plan, param_index, ptr_ty)
            .expect("param offset exists for param");
        let (size, _) = field_size_align(&param.ty, ptr_ty);
        offset = param_offset + size;
    }
    for (local_index, local) in plan.locals.iter().enumerate() {
        let local_offset = simple_async_frame_local_offset(plan, local_index, ptr_ty)
            .expect("local offset exists for local");
        let (size, _) = field_size_align(&local.ty, ptr_ty);
        offset = local_offset + size;
    }
    for step in plan.awaits.iter().take(step_index) {
        if let Some(binding) = &step.binding {
            let (size, align) = field_size_align(&binding.ty, ptr_ty);
            offset = align_u32(offset, align) + size;
        }
    }
    let (_, align) = field_size_align(&binding.ty, ptr_ty);
    Some(align_u32(offset, align))
}

fn simple_async_frame_local_offset(
    plan: &SimpleAsyncStateMachinePlan,
    local_index: usize,
    ptr_ty: types::Type,
) -> Option<u32> {
    let local = plan.locals.get(local_index)?;
    let mut offset = simple_async_frame_param_base_offset(plan, ptr_ty);
    for param in &plan.params {
        let (size, align) = field_size_align(&param.ty, ptr_ty);
        offset = align_u32(offset, align) + size;
    }
    for previous in plan.locals.iter().take(local_index) {
        let (size, align) = field_size_align(&previous.ty, ptr_ty);
        offset = align_u32(offset, align) + size;
    }
    let (_, align) = field_size_align(&local.ty, ptr_ty);
    Some(align_u32(offset, align))
}

fn simple_async_frame_param_base_offset(
    plan: &SimpleAsyncStateMachinePlan,
    ptr_ty: types::Type,
) -> u32 {
    ASYNC_FRAME_AWAITED_BASE_OFFSET + (plan.awaits.len() as u32 * ptr_ty.bytes())
}

fn simple_async_frame_param_offset(
    plan: &SimpleAsyncStateMachinePlan,
    param_index: usize,
    ptr_ty: types::Type,
) -> Option<u32> {
    let param = plan.params.get(param_index)?;
    let mut offset = simple_async_frame_param_base_offset(plan, ptr_ty);
    for previous in plan.params.iter().take(param_index) {
        let (size, align) = field_size_align(&previous.ty, ptr_ty);
        offset = align_u32(offset, align) + size;
    }
    let (_, align) = field_size_align(&param.ty, ptr_ty);
    Some(align_u32(offset, align))
}

fn simple_async_frame_size(plan: &SimpleAsyncStateMachinePlan, ptr_ty: types::Type) -> u32 {
    let mut offset = simple_async_frame_param_base_offset(plan, ptr_ty);
    let mut max_align = ptr_ty.bytes().clamp(1, 8) as u8;
    for param in &plan.params {
        let (size, align) = field_size_align(&param.ty, ptr_ty);
        offset = align_u32(offset, align) + size;
        max_align = max_align.max(align);
    }
    for local in &plan.locals {
        let (size, align) = field_size_align(&local.ty, ptr_ty);
        offset = align_u32(offset, align) + size;
        max_align = max_align.max(align);
    }
    for step in &plan.awaits {
        if let Some(binding) = &step.binding {
            let (size, align) = field_size_align(&binding.ty, ptr_ty);
            offset = align_u32(offset, align) + size;
            max_align = max_align.max(align);
        }
    }
    align_u32(offset, max_align).max(ASYNC_FRAME_AWAITED_BASE_OFFSET + ptr_ty.bytes())
}

/// Verify the ownership-relevant shape of an async frame before emitting any
/// state-machine code. Every slot that terminal cleanup can inspect must be
/// inside the allocation, have a native layout, and occupy a disjoint range.
/// This does not replace a full data-flow ownership proof, but it turns frame
/// layout drift and uninitialized managed-slot cleanup into compile-time errors
/// instead of latent native memory corruption.
fn verify_async_frame_ownership(
    plan: &SimpleAsyncStateMachinePlan,
    ptr_ty: types::Type,
) -> Result<(), String> {
    let frame_size = simple_async_frame_size(plan, ptr_ty);
    let mut ranges: Vec<(String, u32, u32)> = Vec::new();

    let mut add_slot = |label: String, offset: u32, size: u32| -> Result<(), String> {
        let end = offset
            .checked_add(size)
            .ok_or_else(|| format!("async frame slot `{label}` overflows its offset"))?;
        if end > frame_size || offset > i32::MAX as u32 {
            return Err(format!(
                "async frame slot `{label}` [{offset}..{end}) exceeds frame size {frame_size}"
            ));
        }
        let new_end = end;
        if let Some((other, other_offset, other_end)) = ranges
            .iter()
            .find(|(_, start, existing_end)| offset < *existing_end && *start < new_end)
        {
            return Err(format!(
                "async frame slots `{label}` and `{other}` overlap ({offset}..{end} vs {other_offset}..{other_end})"
            ));
        }
        ranges.push((label, offset, end));
        Ok(())
    };

    add_slot("state".to_string(), ASYNC_FRAME_STATE_OFFSET as u32, 8)?;
    add_slot(
        "result_future".to_string(),
        ASYNC_FRAME_RESULT_OFFSET as u32,
        ptr_ty.bytes(),
    )?;
    for index in 0..plan.awaits.len() {
        add_slot(
            format!("awaited[{index}]"),
            simple_async_frame_awaited_offset(index, ptr_ty) as u32,
            ptr_ty.bytes(),
        )?;
    }

    for (index, param) in plan.params.iter().enumerate() {
        let cl_ty = cl_type(&param.ty, ptr_ty)
            .ok_or_else(|| format!("async param `{}` has no native value", param.name))?;
        let (size, _) = field_size_align(&param.ty, ptr_ty);
        if size != cl_ty.bytes() && is_managed_ty(&param.ty) {
            return Err(format!(
                "managed async param `{}` has inconsistent native size",
                param.name
            ));
        }
        let offset = simple_async_frame_param_offset(plan, index, ptr_ty)
            .ok_or_else(|| format!("async param `{}` has no frame offset", param.name))?;
        add_slot(format!("param `{}`", param.name), offset, size)?;
    }
    for (index, local) in plan.locals.iter().enumerate() {
        cl_type(&local.ty, ptr_ty)
            .ok_or_else(|| format!("async local `{}` has no native value", local.name))?;
        let (size, _) = field_size_align(&local.ty, ptr_ty);
        let offset = simple_async_frame_local_offset(plan, index, ptr_ty)
            .ok_or_else(|| format!("async local `{}` has no frame offset", local.name))?;
        add_slot(format!("local `{}`", local.name), offset, size)?;
    }
    for (index, step) in plan.awaits.iter().enumerate() {
        let Ty::Future(_) = step.await_future.ty else {
            return Err(format!(
                "async await step {index} does not produce a future value"
            ));
        };
        let Some(binding) = &step.binding else {
            continue;
        };
        cl_type(&binding.ty, ptr_ty)
            .ok_or_else(|| format!("async binding `{}` has no native value", binding.name))?;
        let (size, _) = field_size_align(&binding.ty, ptr_ty);
        let offset = simple_async_frame_binding_offset(plan, index, ptr_ty)
            .ok_or_else(|| format!("async binding `{}` has no frame offset", binding.name))?;
        add_slot(format!("binding `{}`", binding.name), offset, size)?;
    }
    Ok(())
}

fn simple_async_frame_awaited_offset(step_index: usize, ptr_ty: types::Type) -> i32 {
    (ASYNC_FRAME_AWAITED_BASE_OFFSET + (step_index as u32 * ptr_ty.bytes())) as i32
}

fn expr_contains_await(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Await(_) => true,
        HirExprKind::Unary { operand, .. } => expr_contains_await(operand),
        HirExprKind::Binary { lhs, rhs, .. } => {
            expr_contains_await(lhs) || expr_contains_await(rhs)
        }
        HirExprKind::Call { callee, args } => {
            expr_contains_await(callee) || args.iter().any(|arg| expr_contains_await(&arg.value))
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            expr_contains_await(receiver) || args.iter().any(expr_contains_await)
        }
        HirExprKind::AssociatedCall { args, .. } => args.iter().any(expr_contains_await),
        HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
            expr_contains_await(object)
        }
        HirExprKind::Index { object, index } => {
            expr_contains_await(object) || expr_contains_await(index)
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => elements.iter().any(expr_contains_await),
        HirExprKind::ListSpreadLit { elements, .. } => elements
            .iter()
            .any(|element| expr_contains_await(&element.value)),
        HirExprKind::MapLit { entries, .. } => entries
            .iter()
            .any(|(key, value)| expr_contains_await(key) || expr_contains_await(value)),
        HirExprKind::StructLit { fields, .. } => {
            fields.iter().any(|(_, value)| expr_contains_await(value))
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            expr_contains_await(base) || updates.iter().any(|(_, value)| expr_contains_await(value))
        }
        HirExprKind::EnumVariant { fields, .. } => {
            fields.iter().any(|(_, value)| expr_contains_await(value))
        }
        HirExprKind::Some_(inner) | HirExprKind::Ok_(inner) | HirExprKind::Err_(inner) => {
            expr_contains_await(inner)
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            expr_contains_await(cond) || expr_contains_await(then) || expr_contains_await(else_)
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            expr_contains_await(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_contains_await)
                        || expr_contains_await(&arm.body)
                })
        }
        HirExprKind::Range { start, end, .. } => {
            expr_contains_await(start) || expr_contains_await(end)
        }
        HirExprKind::Propagate(inner) => expr_contains_await(inner),
        HirExprKind::InterpolatedStr(parts) => parts.iter().any(|part| match part {
            HirStrPart::Literal(_) => false,
            HirStrPart::Expr(expr) => expr_contains_await(expr),
        }),
        HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::BoolLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::None_
        | HirExprKind::Var(_)
        | HirExprKind::Closure { .. }
        | HirExprKind::IsCheck { .. } => false,
    }
}

fn block_contains_await(block: &HirBlock) -> bool {
    block.stmts.iter().any(stmt_contains_await)
}

fn arm_body_contains_await(stmts: &[HirStmt]) -> bool {
    stmts.iter().any(stmt_contains_await)
}

fn stmt_contains_await(stmt: &HirStmt) -> bool {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Using { value, .. } => expr_contains_await(value),
        HirStmt::Assign { value, .. } => expr_contains_await(value),
        HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => expr_contains_await(value),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => false,
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            expr_contains_await(cond)
                || block_contains_await(then)
                || else_ifs
                    .iter()
                    .any(|(cond, block)| expr_contains_await(cond) || block_contains_await(block))
                || else_.as_ref().is_some_and(block_contains_await)
        }
        HirStmt::While { cond, body, .. } => {
            expr_contains_await(cond) || block_contains_await(body)
        }
        HirStmt::For { iterable, body, .. } => {
            expr_contains_await(iterable) || block_contains_await(body)
        }
        HirStmt::Loop { body, .. } => block_contains_await(body),
        HirStmt::Repeat { count, body, .. } => {
            expr_contains_await(count) || block_contains_await(body)
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            expr_contains_await(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_contains_await)
                        || arm_body_contains_await(&arm.body)
                })
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            expr_contains_await(value)
                || block_contains_await(then)
                || else_.as_ref().is_some_and(block_contains_await)
        }
        HirStmt::WhileSome { value, body, .. } => {
            expr_contains_await(value) || block_contains_await(body)
        }
        HirStmt::Check { condition, .. } => expr_contains_await(condition),
    }
}

fn block_contains_return(block: &HirBlock) -> bool {
    block.stmts.iter().any(stmt_contains_return)
}

fn arm_body_contains_return(stmts: &[HirStmt]) -> bool {
    stmts.iter().any(stmt_contains_return)
}

fn stmt_contains_return(stmt: &HirStmt) -> bool {
    match stmt {
        HirStmt::Return(_, _) => true,
        HirStmt::Let { .. }
        | HirStmt::Assign { .. }
        | HirStmt::Break(_)
        | HirStmt::Continue(_)
        | HirStmt::Expr(_)
        | HirStmt::Using { .. }
        | HirStmt::Check { .. } => false,
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            block_contains_return(then)
                || else_ifs
                    .iter()
                    .any(|(_, block)| block_contains_return(block))
                || else_.as_ref().is_some_and(block_contains_return)
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => block_contains_return(body),
        HirStmt::Match { arms, .. } => arms.iter().any(|arm| arm_body_contains_return(&arm.body)),
        HirStmt::IfSome { then, else_, .. } => {
            block_contains_return(then) || else_.as_ref().is_some_and(block_contains_return)
        }
    }
}

fn expr_collect_var_uses(expr: &HirExpr, uses: &mut HashSet<SmolStr>) {
    match &expr.kind {
        HirExprKind::Var(name) => {
            uses.insert(name.clone());
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand)
        | HirExprKind::Propagate(operand)
        | HirExprKind::Await(operand) => expr_collect_var_uses(operand, uses),
        HirExprKind::Binary { lhs, rhs, .. } => {
            expr_collect_var_uses(lhs, uses);
            expr_collect_var_uses(rhs, uses);
        }
        HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
            expr_collect_var_uses(object, uses);
        }
        HirExprKind::Index { object, index } => {
            expr_collect_var_uses(object, uses);
            expr_collect_var_uses(index, uses);
        }
        HirExprKind::Call { callee, args } => {
            expr_collect_var_uses(callee, uses);
            for arg in args {
                expr_collect_var_uses(&arg.value, uses);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            expr_collect_var_uses(receiver, uses);
            for arg in args {
                expr_collect_var_uses(arg, uses);
            }
        }
        HirExprKind::AssociatedCall { args, .. } => {
            for arg in args {
                expr_collect_var_uses(arg, uses);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, value) in fields {
                expr_collect_var_uses(value, uses);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for element in elements {
                expr_collect_var_uses(element, uses);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for element in elements {
                expr_collect_var_uses(&element.value, uses);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (key, value) in entries {
                expr_collect_var_uses(key, uses);
                expr_collect_var_uses(value, uses);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            expr_collect_var_uses(cond, uses);
            expr_collect_var_uses(then, uses);
            expr_collect_var_uses(else_, uses);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            expr_collect_var_uses(scrutinee, uses);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    expr_collect_var_uses(guard, uses);
                }
                expr_collect_var_uses(&arm.body, uses);
            }
        }
        HirExprKind::Range { start, end, .. } => {
            expr_collect_var_uses(start, uses);
            expr_collect_var_uses(end, uses);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            expr_collect_var_uses(base, uses);
            for (_, value) in updates {
                expr_collect_var_uses(value, uses);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for part in parts {
                if let HirStrPart::Expr(expr) = part {
                    expr_collect_var_uses(expr, uses);
                }
            }
        }
        HirExprKind::Closure { captures, .. } => {
            for capture in captures {
                uses.insert(capture.name.clone());
            }
        }
        HirExprKind::IsCheck { value, .. } => expr_collect_var_uses(value, uses),
        HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::BoolLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::None_ => {}
    }
}

fn lvalue_collect_var_uses(lvalue: &HirLValue, uses: &mut HashSet<SmolStr>) {
    match lvalue {
        HirLValue::Var(name) => {
            uses.insert(name.clone());
        }
        HirLValue::Field { base, .. } => lvalue_collect_var_uses(base, uses),
        HirLValue::Index { base, index } => {
            lvalue_collect_var_uses(base, uses);
            expr_collect_var_uses(index, uses);
        }
    }
}

fn stmt_collect_var_uses(stmt: &HirStmt, uses: &mut HashSet<SmolStr>) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Using { value, .. } => {
            expr_collect_var_uses(value, uses);
        }
        HirStmt::Assign { lvalue, value, .. } => {
            lvalue_collect_var_uses(lvalue, uses);
            expr_collect_var_uses(value, uses);
        }
        HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => {
            expr_collect_var_uses(value, uses)
        }
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            expr_collect_var_uses(cond, uses);
            for stmt in &then.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
            for (cond, block) in else_ifs {
                expr_collect_var_uses(cond, uses);
                for stmt in &block.stmts {
                    stmt_collect_var_uses(stmt, uses);
                }
            }
            if let Some(block) = else_ {
                for stmt in &block.stmts {
                    stmt_collect_var_uses(stmt, uses);
                }
            }
        }
        HirStmt::While { cond, body, .. } => {
            expr_collect_var_uses(cond, uses);
            for stmt in &body.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
        }
        HirStmt::For { iterable, body, .. } => {
            expr_collect_var_uses(iterable, uses);
            for stmt in &body.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
        }
        HirStmt::Loop { body, .. } => {
            for stmt in &body.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
        }
        HirStmt::Repeat { count, body, .. } => {
            expr_collect_var_uses(count, uses);
            for stmt in &body.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            expr_collect_var_uses(scrutinee, uses);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    expr_collect_var_uses(guard, uses);
                }
                for stmt in &arm.body {
                    stmt_collect_var_uses(stmt, uses);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            expr_collect_var_uses(value, uses);
            for stmt in &then.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
            if let Some(block) = else_ {
                for stmt in &block.stmts {
                    stmt_collect_var_uses(stmt, uses);
                }
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            expr_collect_var_uses(value, uses);
            for stmt in &body.stmts {
                stmt_collect_var_uses(stmt, uses);
            }
        }
        HirStmt::Check { condition, .. } => expr_collect_var_uses(condition, uses),
    }
}

fn simple_async_uses_after_await(
    plan: &SimpleAsyncStateMachinePlan,
    await_index: usize,
) -> HashSet<SmolStr> {
    let mut uses = HashSet::new();
    for step in plan.awaits.iter().skip(await_index + 1) {
        expr_collect_var_uses(&step.await_future, &mut uses);
    }
    for stmt in &plan.tail_stmts {
        stmt_collect_var_uses(stmt, &mut uses);
    }
    if let Some(expr) = &plan.return_expr {
        expr_collect_var_uses(expr, &mut uses);
    }
    if let Some(expr) = &plan.tail_expr {
        expr_collect_var_uses(expr, &mut uses);
    }
    uses
}

#[cfg(test)]
fn simple_async_name_used_after_await(
    plan: &SimpleAsyncStateMachinePlan,
    name: &SmolStr,
    await_index: usize,
) -> bool {
    simple_async_uses_after_await(plan, await_index).contains(name)
}

fn simple_async_lift_expr_awaits(
    expr: &HirExpr,
    awaits: &mut Vec<SimpleAsyncAwaitStep>,
    first_index: usize,
) -> Option<HirExpr> {
    match &expr.kind {
        HirExprKind::Await(await_future) => {
            cl_type(&expr.ty, types::I64)?;
            let Ty::Future(await_ty) = &await_future.ty else {
                return None;
            };
            if await_ty.as_ref() != &expr.ty {
                return None;
            }
            let name = SmolStr::new(format!(
                ".__async_expr_await_{}",
                first_index + awaits.len()
            ));
            awaits.push(SimpleAsyncAwaitStep {
                await_future: await_future.as_ref().clone(),
                binding: Some(SimpleAsyncBinding {
                    name: name.clone(),
                    ty: expr.ty.clone(),
                }),
                propagate_result_ty: None,
            });
            Some(HirExpr {
                kind: HirExprKind::Var(name),
                ty: expr.ty.clone(),
                span: expr.span,
            })
        }
        HirExprKind::Binary { op, lhs, rhs } => Some(HirExpr {
            kind: HirExprKind::Binary {
                op: *op,
                lhs: Box::new(simple_async_lift_expr_awaits(lhs, awaits, first_index)?),
                rhs: Box::new(simple_async_lift_expr_awaits(rhs, awaits, first_index)?),
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Unary { op, operand } => Some(HirExpr {
            kind: HirExprKind::Unary {
                op: *op,
                operand: Box::new(simple_async_lift_expr_awaits(operand, awaits, first_index)?),
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Field { object, field } => Some(HirExpr {
            kind: HirExprKind::Field {
                object: Box::new(simple_async_lift_expr_awaits(object, awaits, first_index)?),
                field: field.clone(),
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Index { object, index } => Some(HirExpr {
            kind: HirExprKind::Index {
                object: Box::new(simple_async_lift_expr_awaits(object, awaits, first_index)?),
                index: Box::new(simple_async_lift_expr_awaits(index, awaits, first_index)?),
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::TupleIndex { object, index } => Some(HirExpr {
            kind: HirExprKind::TupleIndex {
                object: Box::new(simple_async_lift_expr_awaits(object, awaits, first_index)?),
                index: *index,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Call { callee, args } => Some(HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(simple_async_lift_expr_awaits(callee, awaits, first_index)?),
                args: args
                    .iter()
                    .map(|arg| {
                        Some(HirArg {
                            label: arg.label.clone(),
                            spread: arg.spread,
                            value: simple_async_lift_expr_awaits(&arg.value, awaits, first_index)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::MethodCall {
            receiver,
            method,
            args,
        } => Some(HirExpr {
            kind: HirExprKind::MethodCall {
                receiver: Box::new(simple_async_lift_expr_awaits(
                    receiver,
                    awaits,
                    first_index,
                )?),
                method: method.clone(),
                args: args
                    .iter()
                    .map(|arg| simple_async_lift_expr_awaits(arg, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::AssociatedCall {
            receiver_ty,
            method,
            args,
        } => Some(HirExpr {
            kind: HirExprKind::AssociatedCall {
                receiver_ty: receiver_ty.clone(),
                method: method.clone(),
                args: args
                    .iter()
                    .map(|arg| simple_async_lift_expr_awaits(arg, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::StructLit { def_id, fields } => Some(HirExpr {
            kind: HirExprKind::StructLit {
                def_id: *def_id,
                fields: fields
                    .iter()
                    .map(|(name, value)| {
                        Some((
                            name.clone(),
                            simple_async_lift_expr_awaits(value, awaits, first_index)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::EnumVariant {
            def_id,
            variant,
            fields,
        } => Some(HirExpr {
            kind: HirExprKind::EnumVariant {
                def_id: *def_id,
                variant: variant.clone(),
                fields: fields
                    .iter()
                    .map(|(name, value)| {
                        Some((
                            name.clone(),
                            simple_async_lift_expr_awaits(value, awaits, first_index)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::ListLit { elem_ty, elements } => Some(HirExpr {
            kind: HirExprKind::ListLit {
                elem_ty: elem_ty.clone(),
                elements: elements
                    .iter()
                    .map(|element| simple_async_lift_expr_awaits(element, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::ArrayLit { elem_ty, elements } => Some(HirExpr {
            kind: HirExprKind::ArrayLit {
                elem_ty: elem_ty.clone(),
                elements: elements
                    .iter()
                    .map(|element| simple_async_lift_expr_awaits(element, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::SimdLit {
            elem_ty,
            lanes,
            elements,
        } => Some(HirExpr {
            kind: HirExprKind::SimdLit {
                elem_ty: elem_ty.clone(),
                lanes: *lanes,
                elements: elements
                    .iter()
                    .map(|element| simple_async_lift_expr_awaits(element, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::ListSpreadLit { elem_ty, elements } => Some(HirExpr {
            kind: HirExprKind::ListSpreadLit {
                elem_ty: elem_ty.clone(),
                elements: elements
                    .iter()
                    .map(|element| {
                        Some(HirListElement {
                            spread: element.spread,
                            value: simple_async_lift_expr_awaits(
                                &element.value,
                                awaits,
                                first_index,
                            )?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::TupleLit(elements) => Some(HirExpr {
            kind: HirExprKind::TupleLit(
                elements
                    .iter()
                    .map(|element| simple_async_lift_expr_awaits(element, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            ),
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Some_(inner) => Some(HirExpr {
            kind: HirExprKind::Some_(Box::new(simple_async_lift_expr_awaits(
                inner,
                awaits,
                first_index,
            )?)),
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Ok_(inner) => Some(HirExpr {
            kind: HirExprKind::Ok_(Box::new(simple_async_lift_expr_awaits(
                inner,
                awaits,
                first_index,
            )?)),
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Err_(inner) => Some(HirExpr {
            kind: HirExprKind::Err_(Box::new(simple_async_lift_expr_awaits(
                inner,
                awaits,
                first_index,
            )?)),
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::Propagate(inner) => {
            if expr_contains_await(inner) {
                None
            } else {
                Some(expr.clone())
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            if expr_contains_await(then) || expr_contains_await(else_) {
                return None;
            }
            Some(HirExpr {
                kind: HirExprKind::IfExpr {
                    cond: Box::new(simple_async_lift_expr_awaits(cond, awaits, first_index)?),
                    then: then.clone(),
                    else_: else_.clone(),
                },
                ty: expr.ty.clone(),
                span: expr.span,
            })
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            // Only the scrutinee may carry an await here: an await inside a
            // guard or arm body runs conditionally, which the simple
            // state-machine lowering cannot express.
            if arms.iter().any(|arm| {
                arm.guard.as_ref().is_some_and(expr_contains_await)
                    || expr_contains_await(&arm.body)
            }) {
                return None;
            }
            Some(HirExpr {
                kind: HirExprKind::MatchExpr {
                    scrutinee: Box::new(simple_async_lift_expr_awaits(
                        scrutinee,
                        awaits,
                        first_index,
                    )?),
                    arms: arms.clone(),
                },
                ty: expr.ty.clone(),
                span: expr.span,
            })
        }
        HirExprKind::Range { start, end } => Some(HirExpr {
            kind: HirExprKind::Range {
                start: Box::new(simple_async_lift_expr_awaits(start, awaits, first_index)?),
                end: Box::new(simple_async_lift_expr_awaits(end, awaits, first_index)?),
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::MapLit {
            key_ty,
            value_ty,
            entries,
        } => Some(HirExpr {
            kind: HirExprKind::MapLit {
                key_ty: key_ty.clone(),
                value_ty: value_ty.clone(),
                entries: entries
                    .iter()
                    .map(|(key, value)| {
                        Some((
                            simple_async_lift_expr_awaits(key, awaits, first_index)?,
                            simple_async_lift_expr_awaits(value, awaits, first_index)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::SetLit { elem_ty, elements } => Some(HirExpr {
            kind: HirExprKind::SetLit {
                elem_ty: elem_ty.clone(),
                elements: elements
                    .iter()
                    .map(|element| simple_async_lift_expr_awaits(element, awaits, first_index))
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::StructUpdate {
            def_id,
            base,
            updates,
        } => Some(HirExpr {
            kind: HirExprKind::StructUpdate {
                def_id: *def_id,
                base: Box::new(simple_async_lift_expr_awaits(base, awaits, first_index)?),
                updates: updates
                    .iter()
                    .map(|(name, value)| {
                        Some((
                            name.clone(),
                            simple_async_lift_expr_awaits(value, awaits, first_index)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::InterpolatedStr(parts) => Some(HirExpr {
            kind: HirExprKind::InterpolatedStr(
                parts
                    .iter()
                    .map(|part| match part {
                        HirStrPart::Literal(text) => Some(HirStrPart::Literal(text.clone())),
                        HirStrPart::Expr(expr) => Some(HirStrPart::Expr(
                            simple_async_lift_expr_awaits(expr, awaits, first_index)?,
                        )),
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::IsCheck { value, check_ty } => Some(HirExpr {
            kind: HirExprKind::IsCheck {
                value: Box::new(simple_async_lift_expr_awaits(value, awaits, first_index)?),
                check_ty: check_ty.clone(),
            },
            ty: expr.ty.clone(),
            span: expr.span,
        }),
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::None_
        | HirExprKind::Var(_)
        | HirExprKind::Closure { .. } => Some(expr.clone()),
    }
}

fn simple_async_lift_stmt_awaits(
    stmt: &HirStmt,
    first_index: usize,
) -> Option<(HirStmt, Vec<SimpleAsyncAwaitStep>)> {
    let mut awaits = Vec::new();
    let lifted = match stmt {
        HirStmt::Let {
            name,
            ty,
            mutable,
            value,
            span,
        } => HirStmt::Let {
            name: name.clone(),
            ty: ty.clone(),
            mutable: *mutable,
            value: simple_async_lift_expr_awaits(value, &mut awaits, first_index)?,
            span: *span,
        },
        HirStmt::Using {
            name,
            ty,
            value,
            span,
        } => HirStmt::Using {
            name: name.clone(),
            ty: ty.clone(),
            value: simple_async_lift_expr_awaits(value, &mut awaits, first_index)?,
            span: *span,
        },
        HirStmt::Assign {
            lvalue,
            value,
            span,
        } => HirStmt::Assign {
            lvalue: lvalue.clone(),
            value: simple_async_lift_expr_awaits(value, &mut awaits, first_index)?,
            span: *span,
        },
        HirStmt::Return(Some(expr), span) => HirStmt::Return(
            Some(simple_async_lift_expr_awaits(
                expr,
                &mut awaits,
                first_index,
            )?),
            *span,
        ),
        HirStmt::Expr(expr) => HirStmt::Expr(simple_async_lift_expr_awaits(
            expr,
            &mut awaits,
            first_index,
        )?),
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            span,
        } => {
            if block_contains_await(then)
                || else_ifs
                    .iter()
                    .any(|(_, block)| block_contains_await(block))
                || else_.as_ref().is_some_and(block_contains_await)
            {
                return None;
            }
            HirStmt::If {
                cond: simple_async_lift_expr_awaits(cond, &mut awaits, first_index)?,
                then: then.clone(),
                else_ifs: else_ifs
                    .iter()
                    .map(|(cond, block)| {
                        Some((
                            simple_async_lift_expr_awaits(cond, &mut awaits, first_index)?,
                            block.clone(),
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
                else_: else_.clone(),
                span: *span,
            }
        }
        HirStmt::IfSome {
            kind,
            binding,
            inner_ty,
            value,
            then,
            else_,
            span,
        } => {
            if block_contains_await(then) || else_.as_ref().is_some_and(block_contains_await) {
                return None;
            }
            HirStmt::IfSome {
                kind: *kind,
                binding: binding.clone(),
                inner_ty: inner_ty.clone(),
                value: simple_async_lift_expr_awaits(value, &mut awaits, first_index)?,
                then: then.clone(),
                else_: else_.clone(),
                span: *span,
            }
        }
        HirStmt::Match {
            scrutinee,
            arms,
            span,
        } => {
            if arms.iter().any(|arm| {
                arm.guard.as_ref().is_some_and(expr_contains_await)
                    || arm_body_contains_await(&arm.body)
            }) {
                return None;
            }
            HirStmt::Match {
                scrutinee: simple_async_lift_expr_awaits(scrutinee, &mut awaits, first_index)?,
                arms: arms.clone(),
                span: *span,
            }
        }
        HirStmt::Check {
            condition,
            message,
            span,
        } => HirStmt::Check {
            condition: simple_async_lift_expr_awaits(condition, &mut awaits, first_index)?,
            message: message.clone(),
            span: *span,
        },
        HirStmt::For {
            binding,
            index_binding,
            elem_ty,
            iterable,
            body,
            span,
        } => {
            if block_contains_await(body) {
                return None;
            }
            HirStmt::For {
                binding: binding.clone(),
                index_binding: index_binding.clone(),
                elem_ty: elem_ty.clone(),
                iterable: simple_async_lift_expr_awaits(iterable, &mut awaits, first_index)?,
                body: body.clone(),
                span: *span,
            }
        }
        HirStmt::Return(None, _)
        | HirStmt::Break(_)
        | HirStmt::Continue(_)
        | HirStmt::While { .. }
        | HirStmt::Loop { .. }
        | HirStmt::Repeat { .. }
        | HirStmt::WhileSome { .. } => return None,
    };
    if awaits.is_empty() {
        return None;
    }
    Some((lifted, awaits))
}

fn simple_async_state_machine_plan(f: &HirFunc) -> Option<SimpleAsyncStateMachinePlan> {
    if f.body.stmts.is_empty() {
        return None;
    }
    let mut params = Vec::with_capacity(f.params.len());
    for param in &f.params {
        cl_type(&param.ty, types::I64)?;
        params.push(SimpleAsyncParam {
            name: param.name.clone(),
            ty: param.ty.clone(),
        });
    }
    let inner_ty = async_inner_return_ty(f)?;
    let mut locals = Vec::new();
    let mut awaits = Vec::new();
    let mut tail_stmts = Vec::new();
    let mut return_expr = None;
    let mut tail_expr = None;
    let mut saw_await = false;
    let mut terminal_return_await = false;

    for stmt in &f.body.stmts {
        if terminal_return_await {
            return None;
        }

        let parsed_await = match stmt {
            HirStmt::Expr(await_expr) => match &await_expr.kind {
                HirExprKind::Await(await_future) => {
                    Some((await_future.as_ref().clone(), None, None, None::<HirExpr>))
                }
                _ => None,
            },
            HirStmt::Let {
                name, ty, value, ..
            }
            | HirStmt::Using {
                name, ty, value, ..
            } => {
                if let HirExprKind::Await(await_future) = &value.kind {
                    cl_type(ty, types::I64)?;
                    Some((
                        await_future.as_ref().clone(),
                        Some(SimpleAsyncBinding {
                            name: name.clone(),
                            ty: ty.clone(),
                        }),
                        None,
                        None,
                    ))
                } else if let HirExprKind::Propagate(inner) = &value.kind {
                    let HirExprKind::Await(await_future) = &inner.kind else {
                        return None;
                    };
                    cl_type(ty, types::I64)?;
                    let Ty::Future(await_result_ty) = &await_future.ty else {
                        return None;
                    };
                    let Ty::Result(ok_ty, _) = await_result_ty.as_ref() else {
                        return None;
                    };
                    if ok_ty.as_ref() != ty || await_result_ty.as_ref() != &inner_ty {
                        return None;
                    }
                    Some((
                        await_future.as_ref().clone(),
                        Some(SimpleAsyncBinding {
                            name: name.clone(),
                            ty: ty.clone(),
                        }),
                        Some(await_result_ty.as_ref().clone()),
                        None,
                    ))
                } else {
                    None
                }
            }
            HirStmt::Return(Some(expr), _) if matches!(expr.kind, HirExprKind::Await(_)) => {
                let HirExprKind::Await(await_future) = &expr.kind else {
                    unreachable!("guarded by matches")
                };
                let binding_name = SmolStr::new(".__async_return_value");
                let return_value = HirExpr {
                    kind: HirExprKind::Var(binding_name.clone()),
                    ty: expr.ty.clone(),
                    span: expr.span,
                };
                Some((
                    await_future.as_ref().clone(),
                    Some(SimpleAsyncBinding {
                        name: binding_name,
                        ty: expr.ty.clone(),
                    }),
                    None,
                    Some(return_value),
                ))
            }
            _ => None,
        };

        if let Some((await_future, binding, propagate_result_ty, return_value)) = parsed_await {
            if !tail_stmts.is_empty() {
                return None;
            }
            saw_await = true;
            awaits.push(SimpleAsyncAwaitStep {
                await_future,
                binding,
                propagate_result_ty,
            });
            if let Some(return_value) = return_value {
                return_expr = Some(return_value);
                terminal_return_await = true;
            }
            continue;
        }

        if stmt_contains_await(stmt) {
            if !tail_stmts.is_empty() {
                return None;
            }
            let (lifted_stmt, lifted_awaits) = simple_async_lift_stmt_awaits(stmt, awaits.len())?;
            saw_await = true;
            awaits.extend(lifted_awaits);
            tail_stmts.push(lifted_stmt);
            continue;
        }

        if !saw_await {
            match stmt {
                HirStmt::Let {
                    name, ty, value, ..
                }
                | HirStmt::Using {
                    name, ty, value, ..
                } => {
                    cl_type(ty, types::I64)?;
                    locals.push(SimpleAsyncLocal {
                        name: name.clone(),
                        ty: ty.clone(),
                        value: value.clone(),
                    });
                }
                _ => return None,
            }
        } else {
            tail_stmts.push(stmt.clone());
        }
    }

    if awaits.is_empty() {
        return None;
    }
    if !terminal_return_await {
        match tail_stmts.pop() {
            Some(HirStmt::Return(expr, _)) => {
                if expr.as_ref().is_some_and(expr_contains_await) {
                    return None;
                }
                return_expr = expr;
            }
            Some(HirStmt::Expr(expr)) if matches!(inner_ty, Ty::Void | Ty::Never) => {
                if expr_contains_await(&expr) {
                    return None;
                }
                tail_expr = Some(expr);
            }
            Some(stmt) if matches!(inner_ty, Ty::Void | Ty::Never) => {
                if stmt_contains_return(&stmt) {
                    return None;
                }
                tail_stmts.push(stmt);
                tail_expr = Some(HirExpr {
                    kind: HirExprKind::Unit,
                    ty: Ty::Void,
                    span: f.span,
                });
            }
            None if matches!(inner_ty, Ty::Void | Ty::Never) => {
                tail_expr = Some(HirExpr {
                    kind: HirExprKind::Unit,
                    ty: Ty::Void,
                    span: f.span,
                });
            }
            _ => return None,
        }
    }
    if tail_stmts.iter().any(stmt_contains_return) {
        return None;
    }
    Some(SimpleAsyncStateMachinePlan {
        params,
        locals,
        awaits,
        tail_stmts,
        return_expr,
        tail_expr,
        inner_ty,
        is_general: false,
    })
}

fn collect_pattern_bindings(pat: &HirPattern, bindings: &mut Vec<(SmolStr, Ty)>) {
    match pat {
        HirPattern::Binding(name, ty) => {
            bindings.push((name.clone(), ty.clone()));
        }
        HirPattern::Some_(p) | HirPattern::Ok_(p) | HirPattern::Err_(p) => {
            collect_pattern_bindings(p, bindings);
        }
        HirPattern::Variant { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_bindings(p, bindings);
            }
        }
        HirPattern::Tuple(patterns) => {
            for p in patterns {
                collect_pattern_bindings(p, bindings);
            }
        }
        _ => {}
    }
}

struct GeneralAsyncCollector {
    locals: Vec<SimpleAsyncLocal>,
    awaits: Vec<SimpleAsyncAwaitStep>,
}

impl GeneralAsyncCollector {
    fn collect_stmt(&mut self, stmt: &HirStmt, loop_counter: &mut usize) {
        match stmt {
            HirStmt::Let {
                name, ty, value, ..
            }
            | HirStmt::Using {
                name, ty, value, ..
            } => {
                self.locals.push(SimpleAsyncLocal {
                    name: name.clone(),
                    ty: ty.clone(),
                    value: value.clone(),
                });
                self.collect_expr(value);
            }
            HirStmt::Assign { lvalue, value, .. } => {
                self.collect_lvalue(lvalue);
                self.collect_expr(value);
            }
            HirStmt::Return(expr, _) => {
                if let Some(e) = expr {
                    self.collect_expr(e);
                }
            }
            HirStmt::Expr(expr) => {
                self.collect_expr(expr);
            }
            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                self.collect_expr(cond);
                self.collect_block(then, loop_counter);
                for (e_cond, e_block) in else_ifs {
                    self.collect_expr(e_cond);
                    self.collect_block(e_block, loop_counter);
                }
                if let Some(e_block) = else_ {
                    self.collect_block(e_block, loop_counter);
                }
            }
            HirStmt::While { cond, body, .. } => {
                self.collect_expr(cond);
                self.collect_block(body, loop_counter);
            }
            HirStmt::For {
                iterable,
                body,
                index_binding,
                elem_ty,
                binding,
                span,
            } => {
                let has_await = stmt_contains_await(stmt);
                if has_await {
                    let loop_id = *loop_counter;
                    *loop_counter += 1;
                    let dummy = HirExpr {
                        kind: HirExprKind::Unit,
                        ty: Ty::Void,
                        span: *span,
                    };
                    self.locals.push(SimpleAsyncLocal {
                        name: binding.clone(),
                        ty: elem_ty.clone(),
                        value: dummy.clone(),
                    });
                    if let Some(ib) = index_binding {
                        self.locals.push(SimpleAsyncLocal {
                            name: ib.clone(),
                            ty: Ty::Int,
                            value: dummy.clone(),
                        });
                    }
                    match &iterable.ty {
                        Ty::Range(..) => {
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_idx_{}", loop_id)),
                                ty: Ty::Int,
                                value: dummy.clone(),
                            });
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_end_{}", loop_id)),
                                ty: Ty::Int,
                                value: dummy.clone(),
                            });
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_asc_{}", loop_id)),
                                ty: Ty::Bool,
                                value: dummy.clone(),
                            });
                            if index_binding.is_some() {
                                self.locals.push(SimpleAsyncLocal {
                                    name: SmolStr::new(format!(".__loop_iter_{}", loop_id)),
                                    ty: Ty::Int,
                                    value: dummy,
                                });
                            }
                        }
                        _ => {
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_idx_{}", loop_id)),
                                ty: Ty::Int,
                                value: dummy.clone(),
                            });
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_len_{}", loop_id)),
                                ty: Ty::Int,
                                value: dummy.clone(),
                            });
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_list_{}", loop_id)),
                                ty: iterable.ty.clone(),
                                value: dummy.clone(),
                            });
                            self.locals.push(SimpleAsyncLocal {
                                name: SmolStr::new(format!(".__loop_version_{}", loop_id)),
                                ty: Ty::Int,
                                value: dummy,
                            });
                        }
                    }
                }
                self.collect_expr(iterable);
                self.collect_block(body, loop_counter);
            }
            HirStmt::Loop { body, .. } => {
                self.collect_block(body, loop_counter);
            }
            HirStmt::Repeat { count, body, span } => {
                let has_await = stmt_contains_await(stmt);
                if has_await {
                    let loop_id = *loop_counter;
                    *loop_counter += 1;
                    let dummy = HirExpr {
                        kind: HirExprKind::Unit,
                        ty: Ty::Void,
                        span: *span,
                    };
                    self.locals.push(SimpleAsyncLocal {
                        name: SmolStr::new(format!(".__loop_idx_{}", loop_id)),
                        ty: Ty::Int,
                        value: dummy.clone(),
                    });
                    self.locals.push(SimpleAsyncLocal {
                        name: SmolStr::new(format!(".__loop_limit_{}", loop_id)),
                        ty: Ty::Int,
                        value: dummy,
                    });
                }
                self.collect_expr(count);
                self.collect_block(body, loop_counter);
            }
            HirStmt::Match {
                scrutinee, arms, ..
            } => {
                self.collect_expr(scrutinee);
                let has_await = stmt_contains_await(stmt);
                for arm in arms {
                    if has_await {
                        let mut bindings = Vec::new();
                        collect_pattern_bindings(&arm.pattern, &mut bindings);
                        let dummy = HirExpr {
                            kind: HirExprKind::Unit,
                            ty: Ty::Void,
                            span: arm.span,
                        };
                        for (name, ty) in bindings {
                            self.locals.push(SimpleAsyncLocal {
                                name,
                                ty,
                                value: dummy.clone(),
                            });
                        }
                    }
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(guard);
                    }
                    for arm_stmt in &arm.body {
                        self.collect_stmt(arm_stmt, loop_counter);
                    }
                }
            }
            HirStmt::IfSome {
                binding,
                inner_ty,
                value,
                then,
                else_,
                span,
                ..
            } => {
                let has_await = stmt_contains_await(stmt);
                if has_await {
                    let dummy = HirExpr {
                        kind: HirExprKind::Unit,
                        ty: Ty::Void,
                        span: *span,
                    };
                    self.locals.push(SimpleAsyncLocal {
                        name: binding.clone(),
                        ty: inner_ty.clone(),
                        value: dummy,
                    });
                }
                self.collect_expr(value);
                self.collect_block(then, loop_counter);
                if let Some(e_block) = else_ {
                    self.collect_block(e_block, loop_counter);
                }
            }
            HirStmt::WhileSome {
                binding,
                inner_ty,
                value,
                body,
                span,
            } => {
                let has_await = stmt_contains_await(stmt);
                if has_await {
                    let dummy = HirExpr {
                        kind: HirExprKind::Unit,
                        ty: Ty::Void,
                        span: *span,
                    };
                    self.locals.push(SimpleAsyncLocal {
                        name: binding.clone(),
                        ty: inner_ty.clone(),
                        value: dummy,
                    });
                }
                self.collect_expr(value);
                self.collect_block(body, loop_counter);
            }
            HirStmt::Check { condition, .. } => {
                self.collect_expr(condition);
            }
            HirStmt::Break(_) | HirStmt::Continue(_) => {}
        }
    }

    fn collect_block(&mut self, block: &HirBlock, loop_counter: &mut usize) {
        for stmt in &block.stmts {
            self.collect_stmt(stmt, loop_counter);
        }
    }

    fn collect_lvalue(&mut self, lvalue: &HirLValue) {
        match lvalue {
            HirLValue::Var(_) => {}
            HirLValue::Field { base, .. } => {
                self.collect_lvalue(base);
            }
            HirLValue::Index { base, index } => {
                self.collect_lvalue(base);
                self.collect_expr(index);
            }
        }
    }

    fn collect_expr(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Await(inner) => {
                self.collect_expr(inner);
                self.awaits.push(SimpleAsyncAwaitStep {
                    await_future: inner.as_ref().clone(),
                    binding: None,
                    propagate_result_ty: None,
                });
            }
            HirExprKind::Unary { operand, .. } => {
                self.collect_expr(operand);
            }
            HirExprKind::Binary { lhs, rhs, .. } => {
                self.collect_expr(lhs);
                self.collect_expr(rhs);
            }
            HirExprKind::Call { callee, args } => {
                self.collect_expr(callee);
                for arg in args {
                    self.collect_expr(&arg.value);
                }
            }
            HirExprKind::MethodCall { receiver, args, .. } => {
                self.collect_expr(receiver);
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
                self.collect_expr(object);
            }
            HirExprKind::Index { object, index } => {
                self.collect_expr(object);
                self.collect_expr(index);
            }
            HirExprKind::ListLit { elements, .. }
            | HirExprKind::ArrayLit { elements, .. }
            | HirExprKind::SimdLit { elements, .. }
            | HirExprKind::TupleLit(elements)
            | HirExprKind::SetLit { elements, .. } => {
                for elem in elements {
                    self.collect_expr(elem);
                }
            }
            HirExprKind::ListSpreadLit { elements, .. } => {
                for elem in elements {
                    self.collect_expr(&elem.value);
                }
            }
            HirExprKind::MapLit { entries, .. } => {
                for (key, value) in entries {
                    self.collect_expr(key);
                    self.collect_expr(value);
                }
            }
            HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
                for (_, value) in fields {
                    self.collect_expr(value);
                }
            }
            HirExprKind::Some_(inner)
            | HirExprKind::Ok_(inner)
            | HirExprKind::Err_(inner)
            | HirExprKind::Propagate(inner) => {
                self.collect_expr(inner);
            }
            HirExprKind::IfExpr { cond, then, else_ } => {
                self.collect_expr(cond);
                self.collect_expr(then);
                self.collect_expr(else_);
            }
            HirExprKind::MatchExpr { scrutinee, arms } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(guard);
                    }
                    self.collect_expr(&arm.body);
                }
            }
            HirExprKind::Range { start, end } => {
                self.collect_expr(start);
                self.collect_expr(end);
            }
            HirExprKind::StructUpdate { base, updates, .. } => {
                self.collect_expr(base);
                for (_, value) in updates {
                    self.collect_expr(value);
                }
            }
            HirExprKind::IsCheck { value, .. } => {
                self.collect_expr(value);
            }
            _ => {}
        }
    }
}

fn collect_general_async_plan(f: &HirFunc) -> Option<SimpleAsyncStateMachinePlan> {
    let mut params = Vec::with_capacity(f.params.len());
    for param in &f.params {
        cl_type(&param.ty, types::I64)?;
        params.push(SimpleAsyncParam {
            name: param.name.clone(),
            ty: param.ty.clone(),
        });
    }
    let inner_ty = async_inner_return_ty(f)?;
    let mut collector = GeneralAsyncCollector {
        locals: Vec::new(),
        awaits: Vec::new(),
    };
    let mut loop_counter = 0;
    collector.collect_block(&f.body, &mut loop_counter);
    if collector.awaits.is_empty() {
        return None;
    }
    Some(SimpleAsyncStateMachinePlan {
        params,
        locals: collector.locals,
        awaits: collector.awaits,
        tail_stmts: Vec::new(),
        return_expr: None,
        tail_expr: None,
        inner_ty,
        is_general: true,
    })
}

/// Layout of an `optional[T]`: `{ has_value: i8, [padding], value: T }`.
fn optional_layout(inner: &Ty, ptr_ty: types::Type) -> (u32, u32) {
    // Returns (value_offset, total_size)
    let (val_size, val_align) = field_size_align(inner, ptr_ty);
    let val_offset = (1u32 + val_align as u32 - 1) & !(val_align as u32 - 1);
    let total = ((val_offset + val_size + val_align as u32 - 1) & !(val_align as u32 - 1)).max(2);
    (val_offset, total)
}

/// Layout of `result[T,E]`: `{ is_ok: i8, [padding], union { ok: T | err: E } }`.
fn result_layout(ok: &Ty, err: &Ty, ptr_ty: types::Type) -> (u32, u32, u32) {
    // Returns (payload_offset, ok_size, total_size)
    let (ok_size, ok_align) = field_size_align(ok, ptr_ty);
    let (err_size, err_align) = field_size_align(err, ptr_ty);
    let pay_align = ok_align.max(err_align);
    let pay_size = ok_size.max(err_size);
    let pay_offset = (1u32 + pay_align as u32 - 1) & !(pay_align as u32 - 1);
    let total = ((pay_offset + pay_size + pay_align as u32 - 1) & !(pay_align as u32 - 1)).max(2);
    (pay_offset, ok_size, total)
}

/// Layout of `lazy[T]`: `{ thunk: ptr, forced: i8, [padding], value: T }`.
fn lazy_layout(inner: &Ty, ptr_ty: types::Type) -> (u32, u32) {
    let ptr_size = ptr_ty.bytes();
    let (val_size, val_align) = field_size_align(inner, ptr_ty);
    let val_offset = (ptr_size + 1 + val_align as u32 - 1) & !(val_align as u32 - 1);
    let max_align = (ptr_ty.bytes() as u8).max(val_align).max(1);
    let total = ((val_offset + val_size + max_align as u32 - 1) & !(max_align as u32 - 1))
        .max(ptr_size + 1);
    (val_offset, total)
}

// == Struct layout ==

#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub offset: u32,
    pub ty: Ty,
    pub contract: Option<HirExpr>,
}

#[derive(Debug, Clone, Default)]
pub struct StructLayout {
    pub size: u32,
    pub align: u8,
    pub fields: Vec<(SmolStr, FieldLayout)>,
}

impl StructLayout {
    pub fn field(&self, name: &str) -> Option<&FieldLayout> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, f)| f)
    }
}

fn field_size_align(ty: &Ty, ptr_ty: types::Type) -> (u32, u8) {
    // An array stores its elements inline, so it occupies `elem * len` bytes
    // rather than one pointer. Without this branch the layout would silently
    // reserve 8 bytes while `ori.mem.size_of` reported the real size.
    if let Ty::Array(elem, size) = ty {
        if let Ty::ConstInt(_, len) = &**size {
            if *len >= 0 {
                let (elem_size, elem_align) = field_size_align(elem, ptr_ty);
                return (elem_size.saturating_mul(*len as u32), elem_align);
            }
        }
    }
    if let Ty::Simd(elem, lanes) = ty {
        let (elem_size, _) = field_size_align(elem, ptr_ty);
        let size = elem_size.saturating_mul(*lanes as u32);
        let align = size.min(16).clamp(1, 16) as u8;
        return (size, align);
    }
    let cl = cl_type(ty, ptr_ty).unwrap_or(ptr_ty);
    let bytes = cl.bytes();
    let align = bytes.clamp(1, 8) as u8;
    (bytes, align)
}

fn tuple_layout(elems: &[Ty], ptr_ty: types::Type) -> (Vec<(u32, Ty)>, u32, u8) {
    let mut offset = 0u32;
    let mut max_align = 1u8;
    let mut fields = Vec::new();

    for ty in elems {
        let (size, align) = field_size_align(ty, ptr_ty);
        let aligned = (offset + align as u32 - 1) & !(align as u32 - 1);
        fields.push((aligned, ty.clone()));
        offset = aligned + size;
        if align > max_align {
            max_align = align;
        }
    }

    let total = ((offset + max_align as u32 - 1) & !(max_align as u32 - 1)).max(1);
    (fields, total, max_align)
}

fn closure_env_layout(captures: &[HirClosureCapture], ptr_ty: types::Type) -> (Vec<u32>, u32) {
    let mut offset = 0u32;
    let mut max_align = 1u8;
    let mut offsets = Vec::with_capacity(captures.len());

    for capture in captures {
        let (size, align) = field_size_align(&capture.ty, ptr_ty);
        let aligned = (offset + align as u32 - 1) & !(align as u32 - 1);
        offsets.push(aligned);
        offset = aligned + size;
        if align > max_align {
            max_align = align;
        }
    }

    let total = if captures.is_empty() {
        0
    } else {
        ((offset + max_align as u32 - 1) & !(max_align as u32 - 1)).max(1)
    };
    (offsets, total)
}

fn compute_struct_layout(fields: &[HirField], ptr_ty: types::Type, repr_c: bool) -> StructLayout {
    compute_struct_layout_with(
        fields,
        ptr_ty,
        repr_c,
        None,
        &HashMap::new(),
        &HashSet::new(),
    )
}

/// GFX-INLINE-1: layout computation with inline-struct awareness.
///
/// `struct_layouts` holds layouts of already-resolved dependencies (nested
/// inline structs); `inline_struct_ids` marks which of them are inline.
fn compute_struct_layout_with(
    fields: &[HirField],
    ptr_ty: types::Type,
    repr_c: bool,
    explicit_align: Option<u32>,
    struct_layouts: &HashMap<ori_types::DefId, StructLayout>,
    inline_struct_ids: &HashSet<ori_types::DefId>,
) -> StructLayout {
    let mut offset = 0u32;
    let mut max_align = explicit_align.map(|a| a as u8).unwrap_or(1);
    let mut result = Vec::new();
    for f in fields {
        let (size, align) =
            field_size_align_with_layouts_typed(&f.ty, ptr_ty, struct_layouts, inline_struct_ids);
        // For repr(C), use natural alignment; for packed, skip alignment
        let aligned = if repr_c {
            (offset + align as u32 - 1) & !(align as u32 - 1)
        } else {
            offset
        };
        result.push((
            f.name.clone(),
            FieldLayout {
                offset: aligned,
                ty: f.ty.clone(),
                contract: f.contract.clone(),
            },
        ));
        offset = aligned + size;
        if repr_c && align > max_align {
            max_align = align;
        }
    }
    // Pad total to struct alignment (only for repr(C))
    let total = if repr_c {
        ((offset + max_align as u32 - 1) & !(max_align as u32 - 1)).max(1)
    } else {
        offset.max(1)
    };
    StructLayout {
        size: total,
        align: max_align,
        fields: result,
    }
}

/// `field_size_align` with both the layout table and the inline-id set, used
/// while computing struct layouts.
fn field_size_align_with_layouts_typed(
    ty: &Ty,
    ptr_ty: types::Type,
    struct_layouts: &HashMap<ori_types::DefId, StructLayout>,
    inline_struct_ids: &HashSet<ori_types::DefId>,
) -> (u32, u8) {
    // An array stores its elements inline, so it occupies `elem * len` bytes
    // rather than one pointer.
    if let Ty::Array(elem, size) = ty {
        if let Ty::ConstInt(_, len) = &**size {
            if *len >= 0 {
                let (elem_size, elem_align) = field_size_align_with_layouts_typed(
                    elem,
                    ptr_ty,
                    struct_layouts,
                    inline_struct_ids,
                );
                return (elem_size.saturating_mul(*len as u32), elem_align);
            }
        }
    }
    if let Ty::Named(def_id, _) = ty {
        if inline_struct_ids.contains(def_id) {
            if let Some(layout) = struct_layouts.get(def_id) {
                return (layout.size, layout.align);
            }
        }
    }
    let cl = cl_type(ty, ptr_ty).unwrap_or(ptr_ty);
    let bytes = cl.bytes();
    let align = bytes.clamp(1, 8) as u8;
    (bytes, align)
}

/// Order structs so dependencies (nested inline structs) come before users.
fn topo_order_structs(hir: &HirModule) -> Vec<ori_types::DefId> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    for s in &hir.structs {
        visit_struct_topo(s.def_id, hir, &mut order, &mut visited, &mut visiting);
    }
    order
}

fn visit_struct_topo(
    def_id: ori_types::DefId,
    hir: &HirModule,
    order: &mut Vec<ori_types::DefId>,
    visited: &mut HashSet<ori_types::DefId>,
    visiting: &mut HashSet<ori_types::DefId>,
) {
    if visited.contains(&def_id) || !visiting.insert(def_id) {
        return;
    }
    let Some(s) = hir.structs.iter().find(|s| s.def_id == def_id) else {
        visiting.remove(&def_id);
        return;
    };
    let deps: Vec<ori_types::DefId> = s
        .fields
        .iter()
        .flat_map(|f| named_def_ids(&f.ty, hir))
        .collect();
    for dep in deps {
        visit_struct_topo(dep, hir, order, visited, visiting);
    }
    visiting.remove(&def_id);
    if visited.insert(def_id) {
        order.push(def_id);
    }
}

/// Collect `DefId`s of named structs referenced by `ty` (excluding arrays of
/// non-inline elements, which are stored as pointers).
fn named_def_ids(ty: &Ty, hir: &HirModule) -> Vec<ori_types::DefId> {
    let _ = hir;
    let mut ids = Vec::new();
    collect_named_def_ids(ty, &mut ids, &mut HashSet::new());
    ids
}

fn collect_named_def_ids(
    ty: &Ty,
    out: &mut Vec<ori_types::DefId>,
    seen: &mut HashSet<ori_types::DefId>,
) {
    match ty {
        Ty::Named(def_id, _) if seen.insert(*def_id) => out.push(*def_id),
        Ty::Array(elem, _) => collect_named_def_ids(elem, out, seen),
        _ => {}
    }
}

fn compute_c_export_struct_layout(
    internal_layout: &StructLayout,
    ptr_ty: types::Type,
) -> StructLayout {
    let mut offset = 0u32;
    let mut max_align = 1u8;
    let mut fields = Vec::with_capacity(internal_layout.fields.len());
    for (name, field) in &internal_layout.fields {
        let (size, align) = field_size_align(&field.ty, ptr_ty);
        let aligned = (offset + u32::from(align) - 1) & !(u32::from(align) - 1);
        fields.push((
            name.clone(),
            FieldLayout {
                offset: aligned,
                ty: field.ty.clone(),
                contract: None,
            },
        ));
        offset = aligned + size;
        max_align = max_align.max(align);
    }
    let size = ((offset + u32::from(max_align) - 1) & !(u32::from(max_align) - 1)).max(1);
    StructLayout {
        size,
        align: max_align,
        fields,
    }
}

fn emit_copy_struct_fields(
    builder: &mut FunctionBuilder<'_>,
    source: ir::Value,
    source_layout: &StructLayout,
    destination: ir::Value,
    destination_layout: &StructLayout,
    ptr_ty: types::Type,
) -> Result<(), String> {
    for (name, source_field) in &source_layout.fields {
        let destination_field = destination_layout.field(name).ok_or_else(|| {
            format!("C export layout is missing field `{name}` during wrapper generation")
        })?;
        let field_ty = cl_type(&source_field.ty, ptr_ty).ok_or_else(|| {
            format!("C export field `{name}` has no scalar native representation")
        })?;
        let value = builder.ins().load(
            field_ty,
            MemFlags::new(),
            source,
            source_field.offset as i32,
        );
        builder.ins().store(
            MemFlags::new(),
            value,
            destination,
            destination_field.offset as i32,
        );
    }
    Ok(())
}

fn emit_c_export_allocation(
    builder: &mut FunctionBuilder<'_>,
    size: u32,
    alloc_ref: ir::FuncRef,
    ptr_ty: types::Type,
) -> ir::Value {
    let size = builder.ins().iconst(types::I64, i64::from(size));
    let no_destructor = builder.ins().iconst(ptr_ty, 0);
    let allocation = builder.ins().call(alloc_ref, &[size, no_destructor]);
    builder.inst_results(allocation)[0]
}

fn emit_c_export_payload_from_external(
    builder: &mut FunctionBuilder<'_>,
    bridge: &CExportPayloadBridge,
    external_value: ir::Value,
    alloc_ref: ir::FuncRef,
    ptr_ty: types::Type,
    copy_ref: Option<ir::FuncRef>,
    handle_validation: CExportHandleValidation,
) -> Result<(ir::Value, bool), String> {
    let Some((internal_layout, external_layout)) = &bridge.scalar_struct_layouts else {
        if matches!(bridge.ty, Ty::Named(_, _)) {
            let validate_handle_size_type_ref = handle_validation.size_type.ok_or_else(|| {
                "missing typed handle validator for managed C export payload".to_string()
            })?;
            let expected_size = handle_validation.payload_size.ok_or_else(|| {
                "managed C export payload has no concrete layout size".to_string()
            })?;
            let expected_tag = handle_validation.payload_type_tag.ok_or_else(|| {
                "managed C export payload has no concrete source-type tag".to_string()
            })?;
            // Opaque managed structs are valid only when the pointer came from
            // Ori's allocator and carries the exact source type. Validation
            // happens before retaining or entering user code; there is no
            // provenance-only fallback for generic or legacy handles.
            let expected_size = builder.ins().iconst(types::I64, i64::from(expected_size));
            let expected_tag = builder.ins().iconst(types::I64, expected_tag);
            builder.ins().call(
                validate_handle_size_type_ref,
                &[external_value, expected_size, expected_tag],
            );
        }
        if let Some(copy_ref) = copy_ref {
            // C callers borrow foreign storage. Copy before registering an ARC
            // edge; retaining an unregistered pointer is a no-op and would let
            // the host free storage still referenced by Ori. Bytes use the
            // explicit `(data, len)` view emitted by the C header, while the
            // legacy string path remains NUL-terminated for ABI compatibility.
            let copy = builder.ins().call(copy_ref, &[external_value]);
            return Ok((builder.inst_results(copy)[0], true));
        }
        return Ok((external_value, false));
    };
    let internal_value = emit_c_export_allocation(builder, internal_layout.size, alloc_ref, ptr_ty);
    emit_copy_struct_fields(
        builder,
        external_value,
        external_layout,
        internal_value,
        internal_layout,
        ptr_ty,
    )?;
    Ok((internal_value, true))
}

fn emit_c_export_payload_to_external(
    builder: &mut FunctionBuilder<'_>,
    bridge: &CExportPayloadBridge,
    internal_value: ir::Value,
    output: ir::Value,
    retain_ref: Option<ir::FuncRef>,
    ptr_ty: types::Type,
    bytes_len_ref: Option<ir::FuncRef>,
) -> Result<(), String> {
    if let Some((internal_layout, external_layout)) = &bridge.scalar_struct_layouts {
        return emit_copy_struct_fields(
            builder,
            internal_value,
            internal_layout,
            output,
            external_layout,
            ptr_ty,
        );
    }
    if matches!(&bridge.ty, Ty::Bytes) {
        let len_ref = bytes_len_ref
            .ok_or_else(|| "missing `ori_bytes_len` for C export bytes payload".to_string())?;
        builder.ins().call(
            retain_ref.ok_or_else(|| {
                "missing `ori_arc_retain` for managed C export result payload".to_string()
            })?,
            &[internal_value],
        );
        builder
            .ins()
            .store(MemFlags::new(), internal_value, output, 0);
        let length = builder.ins().call(len_ref, &[internal_value]);
        let length = builder.inst_results(length)[0];
        builder.ins().store(
            MemFlags::new(),
            length,
            output,
            i32::try_from(std::mem::size_of::<*const u8>())
                .map_err(|_| "C export bytes length offset does not fit i32".to_string())?,
        );
        return Ok(());
    }
    if is_managed_ty(&bridge.ty) {
        builder.ins().call(
            retain_ref.ok_or_else(|| {
                "missing `ori_arc_retain` for managed C export result payload".to_string()
            })?,
            &[internal_value],
        );
    }
    builder
        .ins()
        .store(MemFlags::new(), internal_value, output, 0);
    Ok(())
}

#[derive(Clone, Copy)]
struct CExportInputPayload<'a> {
    tag: ir::Value,
    active_when_true: bool,
    external_value: ir::Value,
    bridge: &'a CExportPayloadBridge,
    wrapper: ir::Value,
    payload_offset: u32,
}

#[derive(Clone, Copy)]
struct CExportArcFunctions {
    allocate: ir::FuncRef,
    register_edge: ir::FuncRef,
    release: ir::FuncRef,
    copy_string_input: Option<ir::FuncRef>,
    copy_bytes_parts_input: Option<ir::FuncRef>,
    validate_handle_size_type: Option<ir::FuncRef>,
}

fn emit_c_export_input_payload_if(
    builder: &mut FunctionBuilder<'_>,
    payload: CExportInputPayload<'_>,
    arc: CExportArcFunctions,
    ptr_ty: types::Type,
) -> Result<(), String> {
    let CExportInputPayload {
        tag,
        active_when_true,
        external_value,
        bridge,
        wrapper,
        payload_offset,
    } = payload;
    let active_block = builder.create_block();
    let inactive_block = builder.create_block();
    let merge_block = builder.create_block();
    if active_when_true {
        builder
            .ins()
            .brif(tag, active_block, &[], inactive_block, &[]);
    } else {
        builder
            .ins()
            .brif(tag, inactive_block, &[], active_block, &[]);
    }

    builder.seal_block(active_block);
    builder.switch_to_block(active_block);
    let copy_ref = match &bridge.ty {
        Ty::String => arc.copy_string_input,
        Ty::Bytes => arc.copy_bytes_parts_input,
        _ => None,
    };
    let (internal_value, owns_temporary) = emit_c_export_payload_from_external(
        builder,
        bridge,
        external_value,
        arc.allocate,
        ptr_ty,
        copy_ref,
        CExportHandleValidation {
            size_type: arc.validate_handle_size_type,
            payload_size: bridge.managed_handle_size,
            payload_type_tag: bridge.managed_handle_type_tag,
        },
    )?;
    builder.ins().store(
        MemFlags::new(),
        internal_value,
        wrapper,
        payload_offset as i32,
    );
    if is_managed_ty(&bridge.ty) {
        builder
            .ins()
            .call(arc.register_edge, &[wrapper, internal_value]);
        if owns_temporary {
            builder.ins().call(arc.release, &[internal_value]);
        }
    }
    builder.ins().jump(merge_block, &[]);

    builder.seal_block(inactive_block);
    builder.switch_to_block(inactive_block);
    builder.ins().jump(merge_block, &[]);

    builder.seal_block(merge_block);
    builder.switch_to_block(merge_block);
    Ok(())
}

#[derive(Clone, Copy)]
struct CExportOutputPayload<'a> {
    tag: ir::Value,
    active_when_true: bool,
    wrapper: ir::Value,
    payload_offset: u32,
    bridge: &'a CExportPayloadBridge,
    output: ir::Value,
}

fn emit_c_export_output_payload_if(
    builder: &mut FunctionBuilder<'_>,
    payload: CExportOutputPayload<'_>,
    retain_ref: ir::FuncRef,
    ptr_ty: types::Type,
    bytes_len_ref: Option<ir::FuncRef>,
) -> Result<(), String> {
    let CExportOutputPayload {
        tag,
        active_when_true,
        wrapper,
        payload_offset,
        bridge,
        output,
    } = payload;
    let active_block = builder.create_block();
    let inactive_block = builder.create_block();
    let merge_block = builder.create_block();
    if active_when_true {
        builder
            .ins()
            .brif(tag, active_block, &[], inactive_block, &[]);
    } else {
        builder
            .ins()
            .brif(tag, inactive_block, &[], active_block, &[]);
    }

    builder.seal_block(active_block);
    builder.switch_to_block(active_block);
    let payload_type = cl_type(&bridge.ty, ptr_ty)
        .ok_or_else(|| "C export result payload has no native representation".to_string())?;
    let payload = builder.ins().load(
        payload_type,
        MemFlags::new(),
        wrapper,
        payload_offset as i32,
    );
    emit_c_export_payload_to_external(
        builder,
        bridge,
        payload,
        output,
        Some(retain_ref),
        ptr_ty,
        bytes_len_ref,
    )?;
    builder.ins().jump(merge_block, &[]);

    builder.seal_block(inactive_block);
    builder.switch_to_block(inactive_block);
    builder.ins().jump(merge_block, &[]);

    builder.seal_block(merge_block);
    builder.switch_to_block(merge_block);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct VariantLayout {
    pub tag: u32,
    pub fields: StructLayout, // Layout of the payload fields
}

#[derive(Debug, Clone, Default)]
pub struct EnumLayout {
    pub size: u32,
    pub align: u8,
    pub payload_offset: u32, // Offset where the union payload begins
    pub variants: HashMap<SmolStr, VariantLayout>,
}

impl EnumLayout {
    pub fn variant(&self, name: &str) -> Option<&VariantLayout> {
        self.variants.get(name)
    }
}

fn compute_enum_layout(variants: &[ori_hir::hir::HirVariant], ptr_ty: types::Type) -> EnumLayout {
    let mut variant_layouts = HashMap::new();
    let tag_size = 4u32; // int32_t tag
    let tag_align = 4u8;

    let mut max_payload_size = 0u32;
    let mut max_payload_align = 1u8;

    for (tag_idx, v) in variants.iter().enumerate() {
        // Use natural alignment (`repr_c=true`) so payload_offset matches the
        // runtime ABI (e.g. ori.json.Value writes payloads at offset 8 for
        // pointer-bearing variants). Packed layout left max_align at 1 and
        // produced payload_offset=4 → match field loads of garbage pointers.
        let payload_layout = compute_struct_layout(&v.fields, ptr_ty, true);
        if payload_layout.size > max_payload_size {
            max_payload_size = payload_layout.size;
        }
        if payload_layout.align > max_payload_align {
            max_payload_align = payload_layout.align;
        }
        variant_layouts.insert(
            v.name.clone(),
            VariantLayout {
                tag: tag_idx as u32,
                fields: payload_layout,
            },
        );
    }

    let overall_align = tag_align.max(max_payload_align);
    let payload_offset =
        (tag_size + max_payload_align as u32 - 1) & !(max_payload_align as u32 - 1);

    // Total size padded to overall alignment
    let total = ((payload_offset + max_payload_size + overall_align as u32 - 1)
        & !(overall_align as u32 - 1))
        .max(tag_size);

    EnumLayout {
        size: total,
        align: overall_align,
        payload_offset,
        variants: variant_layouts,
    }
}

// == Module-level backend ==

#[derive(Debug, Clone)]
struct GlobalDataInfo {
    data_id: DataId,
    ty: Ty,
    mutable: bool,
}

/// Return string-keyed map entries in a stable order.
///
/// Native object emission must not depend on `HashMap`'s per-process hash
/// seed. Apart from making release archives noisy, an unstable declaration
/// order changes Cranelift's register allocation and can produce different
/// machine code for the same source. Lookups remain O(1) on the maps; only
/// the small emission snapshots pay the sorting cost.
fn sorted_smol_entries<V>(map: &HashMap<SmolStr, V>) -> Vec<(&SmolStr, &V)> {
    let mut entries = map.iter().collect::<Vec<_>>();
    entries.sort_unstable_by_key(|(key, _)| key.as_str());
    entries
}

#[derive(Debug, Clone)]
struct UsingCleanup {
    var: Variable,
    ty: Ty,
}

#[derive(Debug, Clone)]
struct ManagedCleanup {
    var: Variable,
    ty: Ty,
}

#[derive(Debug, Clone, Copy)]
struct LoopContext {
    continue_target: ir::Block,
    break_target: ir::Block,
    cleanup_start: usize,
    managed_cleanup_start: usize,
}

#[derive(Debug, Clone, Copy)]
enum EqualityHelperTarget {
    Struct,
    Enum,
}

#[derive(Debug, Clone, Copy)]
enum HashHelperTarget {
    Struct,
    Enum,
}

pub struct NativeBackend<M: Module> {
    module: M,
    ptr_ty: types::Type,
    func_ids: HashMap<SmolStr, FuncId>,
    func_wrapper_ids: HashMap<SmolStr, FuncId>,
    stdlib_ids: HashMap<SmolStr, FuncId>,
    string_data: HashMap<SmolStr, DataId>,
    global_data: HashMap<SmolStr, GlobalDataInfo>,
    struct_layouts: HashMap<ori_types::DefId, StructLayout>,
    /// GFX-INLINE-1: `DefId`s of structs whose fields are all inline
    /// (`Inline(T)`). These are laid out in place inside arrays and other
    /// structs instead of being stored as one pointer.
    inline_struct_ids: HashSet<ori_types::DefId>,
    enum_layouts: HashMap<ori_types::DefId, EnumLayout>,
    type_names: HashMap<ori_types::DefId, SmolStr>,
    trait_layouts: HashMap<ori_types::DefId, HirTrait>,
    trait_impls: HashMap<(ori_types::DefId, ori_types::DefId), HirTraitImpl>,
    func_param_tys: HashMap<SmolStr, Vec<Ty>>,
    /// Names of user-defined functions only (excludes stdlib FFI imports).
    /// Used to distinguish user functions (which release params via scope
    /// cleanup) from stdlib FFI (which borrows args without releasing).
    user_func_names: HashSet<SmolStr>,
    /// `FuncId` of the exported C `main` wrapper, set by `define_all` when the
    /// HIR has an entry `main`. Used by the JIT backend to locate the entry
    /// function pointer after `finalize_definitions`.
    main_func_id: Option<FuncId>,
    /// Runtime initializer for non-static globals. Hosted JIT has no generated
    /// C `main` wrapper, so its owner must invoke this once after finalization
    /// and before publishing the generation.
    global_init_func_id: Option<FuncId>,
    /// Releases managed runtime globals before a hosted generation is dropped.
    global_drop_func_id: Option<FuncId>,
    /// When true (`ori compile --lib`), emit C ABI `@c_export` wrappers and
    /// `__ori_module_init` instead of requiring a process `main`.
    lib_mode: bool,
    /// Map from Ori function qualified name → C export `FuncId` (lib mode).
    c_export_ids: HashMap<SmolStr, FuncId>,
    /// Emit explicit error returns for the hosted JIT instead of process
    /// aborts. Standalone AOT/JIT keeps the historical abort policy.
    hosted_traps: bool,
    /// Hidden callback IDs inserted before arguments for trusted hosted
    /// `extern host` callback imports.
    host_callback_ids: HashMap<SmolStr, u64>,
    /// Cooperative line debugger (ORI_DEBUG_INSTRUMENT=1 + ORI_DEBUG_SOURCE).
    debug_source_path: Option<String>,
    debug_line_starts: Vec<u32>,
    debug_path_data: Option<DataId>,
    debug_name_data: HashMap<SmolStr, DataId>,
}

impl<M: Module> NativeBackend<M> {
    /// GFX-INLINE-1: is `ty` stored in place (inline) rather than behind one
    /// pointer? Mirrors `Inline(T)` from the checker: scalars, inline arrays,
    /// and structs whose fields are all inline.
    fn ty_is_inline_for_layout(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Array(elem, _) => self.ty_is_inline_for_layout(elem),
            Ty::Named(def_id, _) => self.inline_struct_ids.contains(def_id),
            _ => ori_types::is_inline_ty(ty),
        }
    }
    pub fn new(module: M) -> Result<Self, String> {
        let ptr_ty = module.isa().pointer_type();
        Ok(Self {
            module,
            ptr_ty,
            func_ids: HashMap::new(),
            func_wrapper_ids: HashMap::new(),
            stdlib_ids: HashMap::new(),
            string_data: HashMap::new(),
            global_data: HashMap::new(),
            struct_layouts: HashMap::new(),
            inline_struct_ids: HashSet::new(),
            enum_layouts: HashMap::new(),
            type_names: HashMap::new(),
            trait_layouts: HashMap::new(),
            trait_impls: HashMap::new(),
            func_param_tys: HashMap::new(),
            user_func_names: HashSet::new(),
            main_func_id: None,
            global_init_func_id: None,
            global_drop_func_id: None,
            lib_mode: false,
            c_export_ids: HashMap::new(),
            hosted_traps: false,
            host_callback_ids: HashMap::new(),
            debug_source_path: None,
            debug_line_starts: Vec::new(),
            debug_path_data: None,
            debug_name_data: HashMap::new(),
        })
    }

    /// Enable recoverable trap lowering for a host-owned JIT module.
    pub fn set_hosted_traps(&mut self, enabled: bool) {
        self.hosted_traps = enabled;
    }

    /// Configure callback imports for the hosted JIT.
    ///
    /// Each entry is keyed by the qualified HIR name. The backend adds one
    /// leading `i64` callback ID to the native import signature and to every
    /// call site; the dispatcher uses that ID to retrieve host `user_data`.
    pub fn set_host_callback_ids(&mut self, callbacks: &[(String, u64)]) {
        self.host_callback_ids = callbacks
            .iter()
            .map(|(name, id)| (SmolStr::new(name), *id))
            .collect();
    }

    /// Lower the HIR into the module: validate, compute layouts, declare and
    /// define all functions and data. Consumes `self` and returns the backend
    /// with `main_func_id` populated (when an entry `main` exists). After this
    /// returns, the caller can either:
    /// - call `compile` (AOT, `ObjectModule` only) to emit a `.o`/`.obj`, or
    /// - call `into_module` (JIT) to retrieve the `JITModule` and
    ///   `finalize_definitions` + `get_address(main_func_id)`.
    pub fn prepare(mut self, hir: &HirModule) -> Result<Self, String> {
        validate_native_hir(hir)?;
        self.load_debug_instrument_from_env()?;
        self.emit_debug_names(hir)?;
        // Compute struct layouts before anything else. Nested inline structs
        // must be laid out before their parents, so sort by dependency.
        //
        // The inline set must be fixed BEFORE layouts: a struct whose fields
        // are all inline is itself inline, and only inline structs are laid
        // out in place (a wrong layout would under-allocate struct copies).
        let mut changed = true;
        while changed {
            changed = false;
            for s in &hir.structs {
                if self.inline_struct_ids.contains(&s.def_id) {
                    continue;
                }
                let all_inline = s.fields.iter().all(|f| self.ty_is_inline_for_layout(&f.ty));
                if all_inline && !s.fields.is_empty() {
                    self.inline_struct_ids.insert(s.def_id);
                    changed = true;
                }
            }
        }
        let struct_order = topo_order_structs(hir);
        for &def_id in &struct_order {
            let Some(s) = hir.structs.iter().find(|s| s.def_id == def_id) else {
                continue;
            };
            let layout = compute_struct_layout_with(
                &s.fields,
                self.ptr_ty,
                s.repr_c,
                s.explicit_align,
                &self.struct_layouts,
                &self.inline_struct_ids,
            );
            self.struct_layouts.insert(s.def_id, layout);
            self.type_names.insert(s.def_id, s.name.clone());
        }
        for e in &hir.enums {
            let layout = compute_enum_layout(&e.variants, self.ptr_ty);
            self.enum_layouts.insert(e.def_id, layout);
            self.type_names.insert(e.def_id, e.name.clone());
        }
        for t in &hir.traits {
            self.trait_layouts.insert(t.def_id, t.clone());
        }
        for imp in &hir.trait_impls {
            self.trait_impls
                .insert((imp.trait_def_id, imp.type_def_id), imp.clone());
        }
        for f in &hir.funcs {
            self.func_param_tys.insert(
                f.name.clone(),
                f.params.iter().map(|param| param.ty.clone()).collect(),
            );
            self.user_func_names.insert(f.name.clone());
        }
        self.emit_module_strings(hir)?;
        self.emit_global_data(hir)?;
        self.declare_stdlib()?;
        self.declare_all(hir)?;
        self.define_all(hir)?;
        Ok(self)
    }

    /// Consume the backend and return the underlying module. Used by the JIT
    /// path to call `finalize_definitions` and `get_address` on the
    /// `JITModule`.
    pub fn into_module(self) -> M {
        self.module
    }

    /// Returns the `FuncId` of the exported C `main` wrapper, or `None` if the
    /// HIR has no entry `main`.
    pub fn main_func_id(&self) -> Option<FuncId> {
        self.main_func_id
    }

    /// Returns the initializer for globals that require runtime construction.
    ///
    /// JIT owners must invoke this exactly once after definitions are
    /// finalized and before exposing any function from the generation.
    pub fn global_init_func_id(&self) -> Option<FuncId> {
        self.global_init_func_id
    }

    /// Returns the teardown function paired with `global_init_func_id`.
    pub fn global_drop_func_id(&self) -> Option<FuncId> {
        self.global_drop_func_id
    }

    /// Returns the native function identifier for a lowered Ori function.
    ///
    /// The identifier is only valid while the owning Cranelift module remains
    /// alive. Hosted callers should resolve it before finalization and retain
    /// the resulting module-owned function address instead of exposing
    /// `FuncId` across their public boundary.
    pub fn function_id(&self, name: &str) -> Option<FuncId> {
        self.func_ids.get(name).copied()
    }

    /// Emit all string literals as static null-terminated data in .data.
    fn emit_module_strings(&mut self, hir: &HirModule) -> Result<(), String> {
        for s in collect_all_strings(hir) {
            if self.string_data.contains_key(&s) {
                continue;
            }
            let mut bytes: Vec<u8> = Vec::new();
            // Prepend ori_heap_header_t (16 bytes on 64-bit: refcount i64, destructor ptr)
            bytes.extend_from_slice(&1_000_000_000i64.to_le_bytes()); // huge refcount so it never frees
            bytes.extend_from_slice(&0i64.to_le_bytes()); // null destructor
            bytes.extend_from_slice(s.as_bytes());
            bytes.push(0); // null-terminate for `puts` compatibility
            let mut desc = DataDescription::new();
            desc.define(bytes.into_boxed_slice());
            let id = self
                .module
                .declare_anonymous_data(true, false) // writable to allow refcount mutation
                .map_err(|e| format!("declare string data: {e}"))?;
            self.module
                .define_data(id, &desc)
                .map_err(|e| format!("define string data: {e}"))?;
            self.string_data.insert(s, id);
        }
        Ok(())
    }

    /// When `ORI_DEBUG_INSTRUMENT=1`, load `ORI_DEBUG_SOURCE` and prepare line-map
    /// + rodata path for cooperative `ori_debug_line` probes.
    fn load_debug_instrument_from_env(&mut self) -> Result<(), String> {
        if std::env::var_os("ORI_DEBUG_INSTRUMENT").is_none() {
            return Ok(());
        }
        let path = match std::env::var("ORI_DEBUG_SOURCE") {
            Ok(p) if !p.is_empty() => p,
            _ => return Ok(()),
        };
        let content = std::fs::read_to_string(&path).map_err(|e| {
            format!("ORI_DEBUG_INSTRUMENT: cannot read ORI_DEBUG_SOURCE `{path}`: {e}")
        })?;
        let line_starts: Vec<u32> = std::iter::once(0u32)
            .chain(
                content
                    .char_indices()
                    .filter(|&(_, c)| c == '\n')
                    .map(|(i, _)| (i + 1) as u32),
            )
            .collect();
        // Plain UTF-8 path bytes (no ori string heap header) for ori_debug_line.
        let mut bytes = path.as_bytes().to_vec();
        bytes.push(0);
        let mut desc = DataDescription::new();
        desc.define(bytes.into_boxed_slice());
        let id = self
            .module
            .declare_anonymous_data(false, false)
            .map_err(|e| format!("declare debug path data: {e}"))?;
        self.module
            .define_data(id, &desc)
            .map_err(|e| format!("define debug path data: {e}"))?;
        self.debug_path_data = Some(id);
        self.debug_line_starts = line_starts;
        self.debug_source_path = Some(path);
        Ok(())
    }

    fn emit_debug_names(&mut self, hir: &HirModule) -> Result<(), String> {
        if self.debug_path_data.is_none() {
            return Ok(());
        }
        let mut names = collect_debug_names(hir).into_iter().collect::<Vec<_>>();
        names.sort_unstable_by_key(|name| name.clone());
        for name in names {
            let mut bytes = name.as_bytes().to_vec();
            bytes.push(0);
            let mut description = DataDescription::new();
            description.define(bytes.into_boxed_slice());
            let data_id = self
                .module
                .declare_anonymous_data(false, false)
                .map_err(|e| format!("declare debug name data `{name}`: {e}"))?;
            self.module
                .define_data(data_id, &description)
                .map_err(|e| format!("define debug name data `{name}`: {e}"))?;
            self.debug_name_data.insert(name, data_id);
        }
        Ok(())
    }

    fn emit_global_data(&mut self, hir: &HirModule) -> Result<(), String> {
        for c in &hir.consts {
            let Some(cl_ty) = cl_type(&c.ty, self.ptr_ty) else {
                continue;
            };
            let static_bytes = const_static_bytes(&c.value, &c.ty);
            let runtime_init = static_bytes.is_none();
            let bytes = static_bytes.unwrap_or_else(|| {
                let size = cl_ty.bytes().max(1) as usize;
                vec![0; size]
            });
            let mut desc = DataDescription::new();
            desc.define(bytes.into_boxed_slice());
            let link = if c.is_public {
                Linkage::Export
            } else {
                Linkage::Local
            };
            let writable = c.mutable || runtime_init;
            let id = self
                .module
                .declare_data(&native_global_symbol(&c.name), link, writable, false)
                .map_err(|e| format!("declare global '{}': {e}", c.name))?;
            self.module
                .define_data(id, &desc)
                .map_err(|e| format!("define global '{}': {e}", c.name))?;
            self.global_data.insert(
                c.name.clone(),
                GlobalDataInfo {
                    data_id: id,
                    ty: c.ty.clone(),
                    mutable: c.mutable,
                },
            );
        }
        Ok(())
    }

    /// Declare C library / runtime functions used by the stdlib mapping.
    /// FuncId index → ARC runtime symbol, for `ORI_DUMP_ARC` (LANG-MEM-7).
    /// Returns an empty map when the env var is unset so the per-function
    /// call stays free in normal builds.
    fn arc_dump_symbols(&self) -> HashMap<u32, &'static str> {
        let mut map = HashMap::new();
        if std::env::var("ORI_DUMP_ARC").map_or(true, |v| v.is_empty() || v == "0") {
            return map;
        }
        const ARC_SYMBOLS: &[&str] = &[
            "ori_arc_retain",
            "ori_arc_release",
            "ori_arc_register_edge",
            "ori_arc_unregister_edge",
            "ori_arc_update_edge",
            "ori_arc_maybe_collect_cycles",
            "ori_arc_collect_cycles",
        ];
        for &symbol in ARC_SYMBOLS {
            if let Some(id) = self
                .stdlib_ids
                .get(symbol)
                .or_else(|| self.func_ids.get(symbol))
            {
                map.insert(id.as_u32(), symbol);
            }
        }
        map
    }

    fn declare_stdlib(&mut self) -> Result<(), String> {
        let pt = self.ptr_ty;
        let mut declared_imports = HashMap::new();
        let mut decl = |name: &'static str,
                        params: &[types::Type],
                        params_ty: Vec<Ty>,
                        ret: Option<types::Type>|
         -> Result<FuncId, String> {
            if let Some(existing) = declared_imports.get(name).copied() {
                return Ok(existing);
            }
            let mut sig = self.module.make_signature();
            for &p in params {
                // C ABI (SysV/LLVM): the caller zero-extends integer args
                // narrower than 32 bits. Rust `extern "C"` relies on it;
                // without `uext`, an I8 produced by `sete` carries garbage
                // in the upper bits and optimized runtime code that reads a
                // wider register misbehaves (e.g. bool→string printed with
                // the wrong length).
                if p.is_int() && p.bits() < 32 {
                    sig.params.push(AbiParam::new(p).uext());
                } else {
                    sig.params.push(AbiParam::new(p));
                }
            }
            if let Some(r) = ret {
                // Returns stay unextended: rustc does not guarantee extended
                // returns for small ints, so the caller must only trust the
                // low bits (Cranelift's default for a typed I8 return).
                sig.returns.push(AbiParam::new(r));
            }
            self.func_param_tys.insert(SmolStr::new(name), params_ty);
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig)
                .map_err(|e| format!("declare {name}: {e}"))?;
            declared_imports.insert(name, id);
            Ok(id)
        };
        let mut declared_manifest_symbols = std::collections::HashSet::new();
        for entry in stdlib_runtime_functions()
            .iter()
            .filter(|entry| entry.native_runtime)
        {
            if !declared_manifest_symbols.insert(entry.runtime_symbol) {
                continue;
            }
            let (abi_params, abi_ret) =
                stdlib_native_abi(entry.runtime_symbol).ok_or_else(|| {
                    format!(
                        "stdlib manifest entry `{}` is missing native ABI metadata",
                        entry.runtime_symbol
                    )
                })?;
            let cl_params: Vec<_> = abi_params
                .into_iter()
                .map(|ty| cl_stdlib_abi_type(ty, pt))
                .collect();
            let cl_ret = abi_ret.map(|ty| cl_stdlib_abi_type(ty, pt));
            let semantic_params = stdlib_func_sig(entry.canonical_path)
                .map(|(params, _)| params)
                .unwrap_or_default();
            let id = decl(entry.runtime_symbol, &cl_params, semantic_params, cl_ret)?;
            self.stdlib_ids
                .insert(SmolStr::new(entry.runtime_symbol), id);
        }
        // ori_io_print(ptr: *u8, len: i64) -- prints len bytes from ptr
        let id = decl(
            "ori_io_print",
            &[pt, types::I64],
            vec![Ty::String, Ty::Int64],
            None,
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_io_print"), id);
        // ori_io_eprint(ptr: *u8, len: i64) -- stderr print
        let id = decl(
            "ori_io_eprint",
            &[pt, types::I64],
            vec![Ty::String, Ty::Int64],
            None,
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_io_eprint"), id);
        let id = decl("ori_io_read_line", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_io_read_line"), id);
        let id = decl("ori_future_ready_i64", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_ready_i64"), id);
        let id = decl("ori_future_ready_f64", &[types::F64], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_ready_f64"), id);
        let id = decl("ori_future_ready_ptr", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_ready_ptr"), id);
        let id = decl("ori_future_ready_void", &[], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_ready_void"), id);
        let id = decl("ori_future_pending", &[], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_pending"), id);
        let id = decl("ori_future_poll", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_future_poll"), id);
        let id = decl("ori_future_value_i64", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_value_i64"), id);
        let id = decl("ori_future_value_f64", &[pt], vec![], Some(types::F64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_value_f64"), id);
        let id = decl("ori_future_value_ptr", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_value_ptr"), id);
        let id = decl("ori_future_on_ready", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_on_ready"), id);
        let id = decl("ori_future_complete_i64", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_complete_i64"), id);
        let id = decl("ori_future_complete_f64", &[pt, types::F64], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_complete_f64"), id);
        let id = decl("ori_future_complete_ptr", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_complete_ptr"), id);
        let id = decl("ori_future_complete_void", &[pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_complete_void"), id);
        let id = decl("ori_future_fail", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_future_fail"), id);
        let id = decl("ori_future_cancel", &[pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_future_cancel"), id);
        let id = decl("ori_executor_schedule", &[pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_executor_schedule"), id);
        let id = decl("ori_executor_run_one", &[], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_executor_run_one"), id);
        let id = decl("ori_executor_drain", &[], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_executor_drain"), id);
        // Compatibility pointer return for stored `string(n)` values.
        let id = decl("ori_int_to_cstr", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids
            .entry(SmolStr::new("ori_to_string"))
            .or_insert(id);
        let id = decl("ori_uint_to_cstr", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids
            .entry(SmolStr::new("ori_uint_to_string"))
            .or_insert(id);
        // Length-aware numeric conversion used by direct print/interpolation paths.
        let id = decl("ori_to_string_parts", &[types::I64, pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_to_string_parts"), id);
        let id = decl(
            "ori_uint_to_string_parts",
            &[types::I64, pt, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_uint_to_string_parts"), id);
        // strlen(ptr: *u8) -> i64
        let id = decl("strlen", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("strlen"), id);
        let id = decl("strcmp", &[pt, pt], vec![], Some(types::I32))?;
        self.stdlib_ids.insert(SmolStr::new("strcmp"), id);
        let id = decl("ori_hash_i64", &[types::I64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_hash_i64"), id);
        let id = decl("ori_hash_string", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_hash_string"), id);
        let id = decl("ori_hash_bytes", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_hash_bytes"), id);
        let id = decl("ori_string_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_string_len"), id);
        let id = decl("ori_string_concat", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_concat"), id);
        let id = decl("ori_string_copy_host", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_copy_host"), id);
        let id = decl(
            "ori_string_concat_parts",
            &[pt, types::I64, pt, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_concat_parts"), id);
        let id = decl("ori_string_split", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_string_split"), id);
        let id = decl(
            "ori_string_slice",
            &[pt, types::I64, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_string_slice"), id);
        let id = decl("ori_string_contains", &[pt, pt], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_contains"), id);
        let id = decl("ori_string_starts_with", &[pt, pt], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_starts_with"), id);
        let id = decl("ori_string_ends_with", &[pt, pt], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_ends_with"), id);
        let id = decl("ori_string_trim", &[pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_string_trim"), id);
        let id = decl("ori_string_trim_start", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_trim_start"), id);
        let id = decl("ori_string_trim_end", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_trim_end"), id);
        let id = decl("ori_string_to_upper", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_to_upper"), id);
        let id = decl("ori_string_to_lower", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_to_lower"), id);
        let id = decl("ori_string_is_ascii", &[pt], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_is_ascii"), id);
        let id = decl("ori_string_case_fold", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_case_fold"), id);
        let id = decl(
            "ori_err_trace_push",
            &[pt, types::I64, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_err_trace_push"), id);
        let id = decl("ori_err_trace_format", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_err_trace_format"), id);
        let id = decl("ori_string_replace", &[pt, pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_replace"), id);
        let id = decl("ori_string_chars", &[pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_string_chars"), id);
        let id = decl("ori_string_index_of", &[pt, pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_index_of"), id);
        let id = decl("ori_string_join", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_string_join"), id);
        let id = decl("ori_string_repeat", &[pt, types::I64], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_repeat"), id);
        let id = decl(
            "ori_string_pad_left",
            &[pt, types::I64, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_pad_left"), id);
        let id = decl(
            "ori_string_pad_right",
            &[pt, types::I64, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_pad_right"), id);
        // ori_len(ptr: *u8) -> i64
        let id = decl("ori_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_len"), id);

        let id = decl("ori_mem_string_as_ptr", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_mem_string_as_ptr"), id);

        let id = decl("ori_mem_string_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_mem_string_len"), id);

        // ori_math_abs(n: i64) -> i64
        let id = decl("ori_math_sqrt", &[types::F64], vec![], Some(types::F64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_sqrt"), id);
        let id = decl("ori_math_abs", &[types::I64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_abs"), id);
        let id = decl(
            "ori_math_abs_float",
            &[types::F64],
            vec![],
            Some(types::F64),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_math_abs_float"), id);
        // ori_math_min / ori_math_max
        let id = decl(
            "ori_math_min",
            &[types::I64, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_min"), id);
        let id = decl(
            "ori_math_min_float",
            &[types::F64, types::F64],
            vec![],
            Some(types::F64),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_math_min_float"), id);
        let id = decl(
            "ori_math_max",
            &[types::I64, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_max"), id);
        let id = decl(
            "ori_math_max_float",
            &[types::F64, types::F64],
            vec![],
            Some(types::F64),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_math_max_float"), id);
        let id = decl(
            "ori_math_clamp",
            &[types::I64, types::I64, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_clamp"), id);
        let id = decl(
            "ori_math_pow",
            &[types::F64, types::F64],
            vec![],
            Some(types::F64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_pow"), id);
        let id = decl("ori_math_floor", &[types::F64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_floor"), id);
        let id = decl("ori_math_ceil", &[types::F64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_ceil"), id);
        let id = decl("ori_math_round", &[types::F64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_round"), id);
        let id = decl("ori_time_now", &[], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_time_now"), id);
        let id = decl("ori_time_sleep", &[types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_time_sleep"), id);
        let id = decl(
            "ori_time_duration_ms",
            &[types::I64, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_time_duration_ms"), id);
        let id = decl(
            "ori_format_number",
            &[types::F64, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_format_number"), id);
        let id = decl(
            "ori_format_percent",
            &[types::F64, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_format_percent"), id);
        let id = decl("ori_format_hex", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_format_hex"), id);
        let id = decl("ori_format_binary", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_format_binary"), id);
        let id = decl(
            "ori_format_date",
            &[types::I64, pt],
            vec![Ty::Int64, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_format_date"), id);
        let id = decl(
            "ori_format_datetime",
            &[types::I64, pt, pt],
            vec![Ty::Int64, Ty::String, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_format_datetime"), id);
        let id = decl(
            "ori_format_bytes_size",
            &[types::I64, pt],
            vec![Ty::Int64, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_format_bytes_size"), id);
        let id = decl("ori_os_set_args", &[types::I32, pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_set_args"), id);
        let id = decl("ori_os_args", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_args"), id);
        let id = decl("ori_os_env", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_env"), id);
        let id = decl("ori_os_exit", &[types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_exit"), id);
        let id = decl("ori_os_pid", &[], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_pid"), id);
        let id = decl("ori_os_platform", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_platform"), id);
        let id = decl("ori_os_arch", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_os_arch"), id);
        let id = decl(
            "ori_random_int",
            &[types::I64, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_random_int"), id);
        let id = decl(
            "ori_random_float",
            &[types::F64, types::F64],
            vec![],
            Some(types::F64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_random_float"), id);
        let id = decl("ori_random_bool", &[], vec![], Some(types::I8))?;
        self.stdlib_ids.insert(SmolStr::new("ori_random_bool"), id);
        let id = decl(
            "ori_test_assert",
            &[types::I8, pt],
            vec![Ty::Bool, Ty::String],
            None,
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_test_assert"), id);
        let id = decl("ori_test_fail", &[pt], vec![Ty::String], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_test_fail"), id);
        let id = decl("ori_test_live_allocations", &[], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_test_live_allocations"), id);
        let id = decl("ori_test_selected", &[types::I64], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_test_selected"), id);
        let id = decl("ori_test_collect_cycles", &[], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_test_collect_cycles"), id);
        let id = decl(
            "ori_test_assert_no_leaks",
            &[pt],
            vec![Ty::String],
            Some(types::I64),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_test_assert_no_leaks"), id);
        let test_assert_overloads = [
            ("ori_test_assert_eq_float", vec![types::F64, types::F64]),
            ("ori_test_assert_ne_float", vec![types::F64, types::F64]),
            ("ori_test_assert_eq_bool", vec![types::I8, types::I8]),
            ("ori_test_assert_ne_bool", vec![types::I8, types::I8]),
            ("ori_test_assert_eq_string", vec![pt, pt]),
            ("ori_test_assert_ne_string", vec![pt, pt]),
        ];
        for (name, abi_params) in test_assert_overloads {
            let id = decl(name, &abi_params, Vec::new(), None)?;
            self.stdlib_ids.insert(SmolStr::new(name), id);
        }
        let iter_overloads = [
            ("ori_iter_sort_string", vec![pt], Some(pt)),
            ("ori_iter_unique_string", vec![pt], Some(pt)),
            ("ori_iter_group_by_string", vec![pt, pt, pt], Some(pt)),
        ];
        for (name, abi_params, ret) in iter_overloads {
            let id = decl(name, &abi_params, Vec::new(), ret)?;
            self.stdlib_ids.insert(SmolStr::new(name), id);
        }

        // Bytes methods
        let id = decl(
            "ori_bytes_len",
            &[self.ptr_ty],
            vec![Ty::Bytes],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_bytes_len"), id);
        let id = decl(
            "ori_bytes_eq",
            &[self.ptr_ty, self.ptr_ty],
            vec![Ty::Bytes, Ty::Bytes],
            Some(types::I8),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_bytes_eq"), id);
        let id = decl(
            "ori_bytes_get",
            &[self.ptr_ty, types::I64],
            vec![Ty::Bytes, Ty::Int64],
            Some(types::I8),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_bytes_get"), id);
        let id = decl(
            "ori_bytes_concat",
            &[self.ptr_ty, self.ptr_ty],
            vec![Ty::Bytes, Ty::Bytes],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_bytes_concat"), id);
        let id = decl(
            "ori_bytes_copy_parts",
            &[self.ptr_ty],
            vec![],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_bytes_copy_parts"), id);
        let id = decl(
            "ori_bytes_slice",
            &[self.ptr_ty, types::I64, types::I64],
            vec![Ty::Bytes, Ty::Int64, Ty::Int64],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_bytes_slice"), id);
        let id = decl(
            "ori_bytes_from_hex",
            &[self.ptr_ty],
            vec![Ty::String],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_bytes_from_hex"), id);
        let id = decl(
            "ori_bytes_to_hex",
            &[self.ptr_ty],
            vec![Ty::Bytes],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_bytes_to_hex"), id);
        let id = decl(
            "ori_bytes_decode_utf8",
            &[self.ptr_ty],
            vec![Ty::Bytes],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_bytes_decode_utf8"), id);
        let id = decl(
            "ori_string_to_bytes",
            &[self.ptr_ty],
            vec![Ty::String],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_to_bytes"), id);
        let id = decl(
            "ori_string_from_bytes",
            &[self.ptr_ty],
            vec![Ty::Bytes],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_from_bytes"), id);

        // Primitive conversions
        let id = decl(
            "ori_to_int",
            &[types::I64],
            vec![Ty::Int64],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_to_int"), id);
        let id = decl(
            "ori_to_float",
            &[types::I64],
            vec![Ty::Int64],
            Some(types::F64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_to_float"), id);
        let id = decl(
            "ori_new_result",
            &[types::I8, self.ptr_ty],
            vec![Ty::Bool, Ty::Int64],
            Some(self.ptr_ty),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_new_result"), id);
        let id = decl("ori_math_log", &[types::F64], vec![], Some(types::F64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_log"), id);
        let id = decl("ori_math_log2", &[types::F64], vec![], Some(types::F64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_log2"), id);
        let id = decl("ori_math_sin", &[types::F64], vec![], Some(types::F64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_sin"), id);
        let id = decl("ori_math_cos", &[types::F64], vec![], Some(types::F64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_cos"), id);
        let id = decl("ori_math_tan", &[types::F64], vec![], Some(types::F64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_tan"), id);
        let id = decl("ori_math_is_nan", &[types::F64], vec![], Some(types::I8))?;
        self.stdlib_ids.insert(SmolStr::new("ori_math_is_nan"), id);
        let id = decl(
            "ori_math_is_infinite",
            &[types::F64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_math_is_infinite"), id);
        let id = decl("ori_float_to_string", &[types::F64], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_float_to_string"), id);
        let id = decl(
            "ori_float_to_string_parts",
            &[types::F64, pt, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_float_to_string_parts"), id);
        let id = decl("ori_bool_to_string", &[types::I8], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_bool_to_string"), id);
        let id = decl(
            "ori_bool_to_string_parts",
            &[types::I8, pt, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_bool_to_string_parts"), id);
        let id = decl("ori_string_to_int", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_to_int"), id);
        let id = decl("ori_string_to_float", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_to_float"), id);
        let id = decl("ori_string_parse_int", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_parse_int"), id);
        let id = decl("ori_string_parse_float", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_string_parse_float"), id);
        // list[T] runtime
        let id = decl("ori_list_new", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_new"), id);
        let id = decl("ori_list_with_capacity", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_list_with_capacity"), id);
        let id = decl("ori_list_capacity", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_list_capacity"), id);
        let id = decl("ori_list_reserve", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_reserve"), id);
        let id = decl("ori_list_push", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_push"), id);
        let id = decl("ori_list_get", &[pt, types::I64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_get"), id);
        let id = decl("ori_list_set", &[pt, types::I64, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_set"), id);
        let id = decl("ori_list_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_len"), id);
        let id = decl("ori_list_free", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_free"), id);
        // buffer[T] runtime (contiguous fixed-length block)
        let id = decl("ori_buffer_new", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_buffer_new"), id);
        let id = decl("ori_buffer_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_buffer_len"), id);
        let id = decl("ori_buffer_is_empty", &[pt], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_buffer_is_empty"), id);
        let id = decl(
            "ori_buffer_get",
            &[pt, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_buffer_get"), id);
        let id = decl(
            "ori_buffer_set",
            &[pt, types::I64, types::I64],
            vec![],
            None,
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_buffer_set"), id);
        let id = decl("ori_buffer_fill", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_buffer_fill"), id);
        let id = decl("ori_buffer_as_slice", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_buffer_as_slice"), id);
        // Region bump arena allocator (MEM-REGION-1)
        let id = decl("ori_region_new", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_region_new"), id);
        let id = decl(
            "ori_region_alloc",
            &[pt, types::I64, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_region_alloc"), id);
        let id = decl("ori_region_reset", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_region_reset"), id);
        let id = decl("ori_region_free", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_region_free"), id);
        let id = decl("ori_region_size", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_region_size"), id);
        let id = decl("ori_region_count", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_region_count"), id);
        let id = decl("ori_list_pop", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_pop"), id);
        let id = decl("ori_list_remove", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_remove"), id);
        let id = decl(
            "ori_list_insert",
            &[pt, types::I64, types::I64],
            vec![],
            None,
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_insert"), id);
        let id = decl(
            "ori_list_contains",
            &[pt, types::I64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_list_contains"), id);
        let id = decl(
            "ori_list_index_of",
            &[pt, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_list_index_of"), id);
        let id = decl("ori_list_sort", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_sort"), id);
        let id = decl("ori_list_reverse", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_reverse"), id);
        let id = decl(
            "ori_list_slice",
            &[pt, types::I64, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_slice"), id);
        let id = decl("ori_set_new", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_new"), id);
        let id = decl("ori_set_new_custom", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_new_custom"), id);
        let id = decl("ori_set_new_custom_with_hash", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_new_custom_with_hash"), id);
        let id = decl("ori_set_add", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_add"), id);
        let id = decl(
            "ori_set_add_custom_with_eq",
            &[pt, types::I64, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_add_custom_with_eq"), id);
        let id = decl(
            "ori_set_add_custom_with_hash_eq",
            &[pt, types::I64, pt, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_add_custom_with_hash_eq"), id);
        let id = decl("ori_set_add_string", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_add_string"), id);
        let id = decl(
            "ori_set_contains",
            &[pt, types::I64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_contains"), id);
        let id = decl(
            "ori_set_contains_string",
            &[pt, pt],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_contains_string"), id);
        let id = decl(
            "ori_set_contains_custom",
            &[pt, types::I64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_contains_custom"), id);
        let id = decl("ori_set_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_len"), id);
        let id = decl("ori_set_capacity", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_capacity"), id);
        let id = decl("ori_set_reserve", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_reserve"), id);
        let id = decl("ori_set_clear", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_clear"), id);
        let id = decl("ori_set_free", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_free"), id);
        let id = decl("ori_set_remove", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_remove"), id);
        let id = decl("ori_set_remove_string", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_remove_string"), id);
        let id = decl("ori_set_remove_custom", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_remove_custom"), id);
        let id = decl(
            "ori_set_try_remove_custom",
            &[pt, types::I64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_try_remove_custom"), id);
        let id = decl(
            "ori_set_try_remove_string",
            &[pt, pt],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_try_remove_string"), id);
        let id = decl("ori_set_from_list_string", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_from_list_string"), id);
        let id = decl("ori_set_from_list_custom", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_from_list_custom"), id);
        let id = decl(
            "ori_set_from_list_custom_with_hash",
            &[pt, pt, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_from_list_custom_with_hash"), id);
        let id = decl("ori_set_union", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_set_union"), id);
        let id = decl("ori_set_intersection", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_intersection"), id);
        let id = decl("ori_set_difference", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_set_difference"), id);
        let id = decl("ori_list_map", &[pt, pt, pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_map"), id);
        let id = decl("ori_list_filter", &[pt, pt, pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_list_filter"), id);
        let id = decl("ori_map_new", &[], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_new"), id);
        let id = decl("ori_map_new_custom", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_new_custom"), id);
        let id = decl(
            "ori_map_set_custom_with_eq",
            &[pt, pt, types::I64, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_set_custom_with_eq"), id);
        let id = decl(
            "ori_map_set_custom_with_hash_eq",
            &[pt, pt, types::I64, pt, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_set_custom_with_hash_eq"), id);
        let id = decl("ori_map_set", &[pt, types::I64, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_set"), id);
        let id = decl("ori_map_set_string", &[pt, pt, types::I64], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_set_string"), id);
        let id = decl("ori_map_set_custom", &[pt, pt, types::I64], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_set_custom"), id);
        let id = decl("ori_map_get", &[pt, types::I64], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_get"), id);
        let id = decl("ori_map_get_string", &[pt, pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_get_string"), id);
        let id = decl("ori_map_get_custom", &[pt, pt], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_get_custom"), id);
        let id = decl("ori_map_try_get_string", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_try_get_string"), id);
        let id = decl("ori_map_try_get_custom", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_try_get_custom"), id);
        let id = decl(
            "ori_map_contains",
            &[pt, types::I64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_contains"), id);
        let id = decl(
            "ori_map_contains_string",
            &[pt, pt],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_contains_string"), id);
        let id = decl(
            "ori_map_contains_custom",
            &[pt, pt],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_contains_custom"), id);
        let id = decl("ori_map_len", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_len"), id);
        let id = decl("ori_map_capacity", &[pt], vec![], Some(types::I64))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_capacity"), id);
        let id = decl("ori_map_reserve", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_reserve"), id);
        let id = decl("ori_map_clear", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_clear"), id);
        let id = decl(
            "ori_map_key_at",
            &[pt, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_key_at"), id);
        let id = decl(
            "ori_map_value_at",
            &[pt, types::I64],
            vec![],
            Some(types::I64),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_value_at"), id);
        let id = decl("ori_map_free", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_free"), id);
        let id = decl("ori_map_remove", &[pt, types::I64], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_remove"), id);
        let id = decl("ori_map_remove_string", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_remove_string"), id);
        let id = decl("ori_map_remove_custom", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_remove_custom"), id);
        let id = decl("ori_map_try_remove_string", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_try_remove_string"), id);
        let id = decl("ori_map_try_remove_custom", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_try_remove_custom"), id);
        let id = decl("ori_map_from_entries_string", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_from_entries_string"), id);
        let id = decl("ori_map_from_entries_custom", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_from_entries_custom"), id);
        let id = decl(
            "ori_map_from_entries_custom_with_hash",
            &[pt, pt, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_map_from_entries_custom_with_hash"), id);
        let id = decl("ori_map_keys", &[pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_keys"), id);
        let id = decl("ori_map_values", &[pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_values"), id);
        let id = decl("ori_map_entries", &[pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_map_entries"), id);
        let id = decl(
            "ori_hash_table_set_string",
            &[pt, pt, types::I64],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_set_string"), id);
        let id = decl(
            "ori_hash_table_set_custom_with_eq",
            &[pt, types::I64, types::I64, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_set_custom_with_eq"), id);
        let id = decl(
            "ori_hash_table_set_custom_with_hash_eq",
            &[pt, types::I64, types::I64, pt, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_set_custom_with_hash_eq"), id);
        let id = decl("ori_hash_table_get_string", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_get_string"), id);
        let id = decl(
            "ori_hash_table_get_custom",
            &[pt, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_get_custom"), id);
        let id = decl("ori_hash_table_remove_string", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_remove_string"), id);
        let id = decl(
            "ori_hash_table_remove_custom",
            &[pt, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_remove_custom"), id);
        let id = decl(
            "ori_hash_table_contains_string",
            &[pt, pt],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_contains_string"), id);
        let id = decl(
            "ori_hash_table_contains_custom",
            &[pt, types::I64],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_contains_custom"), id);
        let id = decl("ori_hash_table_new_custom", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_new_custom"), id);
        let id = decl(
            "ori_hash_table_new_custom_with_hash",
            &[pt, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_new_custom_with_hash"), id);
        let id = decl(
            "ori_hash_table_with_capacity_custom",
            &[types::I64, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_with_capacity_custom"), id);
        let id = decl(
            "ori_hash_table_with_capacity_custom_with_hash",
            &[types::I64, pt, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids.insert(
            SmolStr::new("ori_hash_table_with_capacity_custom_with_hash"),
            id,
        );
        let id = decl(
            "ori_hash_table_from_entries_string",
            &[pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_hash_table_from_entries_string"), id);
        let id = decl("ori_tree_find_string", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_tree_find_string"), id);
        let id = decl("ori_linked_list_find_string", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_linked_list_find_string"), id);
        let id = decl(
            "ori_doubly_linked_list_find_string",
            &[pt, pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_doubly_linked_list_find_string"), id);
        for name in ["ori_graph_add_node_string", "ori_graph_remove_node_string"] {
            let id = decl(name, &[pt, pt], vec![], None)?;
            self.stdlib_ids.insert(SmolStr::new(name), id);
        }
        for name in ["ori_graph_add_edge_string", "ori_graph_remove_edge_string"] {
            let id = decl(name, &[pt, pt, pt], vec![], None)?;
            self.stdlib_ids.insert(SmolStr::new(name), id);
        }
        let id = decl(
            "ori_graph_add_weighted_edge_string",
            &[pt, pt, pt, types::I64],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_graph_add_weighted_edge_string"), id);
        let name = "ori_graph_has_node_string";
        let id = decl(name, &[pt, pt], vec![], Some(types::I8))?;
        self.stdlib_ids.insert(SmolStr::new(name), id);
        let name = "ori_graph_has_edge_string";
        let id = decl(name, &[pt, pt, pt], vec![], Some(types::I8))?;
        self.stdlib_ids.insert(SmolStr::new(name), id);
        for name in [
            "ori_graph_edge_weight_string",
            "ori_graph_shortest_path_string",
            "ori_graph_shortest_weighted_path_string",
        ] {
            let id = decl(name, &[pt, pt, pt], vec![], Some(pt))?;
            self.stdlib_ids.insert(SmolStr::new(name), id);
        }
        for name in [
            "ori_graph_neighbors_string",
            "ori_graph_bfs_string",
            "ori_graph_dfs_string",
        ] {
            let id = decl(name, &[pt, pt], vec![], Some(pt))?;
            self.stdlib_ids.insert(SmolStr::new(name), id);
        }
        let id = decl("ori_heap_new_string", &[], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_new_string"), id);
        let id = decl("ori_heap_new_custom", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_new_custom"), id);
        let id = decl("ori_heap_push_string", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_push_string"), id);
        let id = decl("ori_heap_push_custom", &[pt, types::I64, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_push_custom"), id);
        let id = decl("ori_heap_from_list_string", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_from_list_string"), id);
        let id = decl("ori_heap_from_list_custom", &[pt, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_from_list_custom"), id);
        let id = decl("ori_heap_remove_string", &[pt, pt], vec![], Some(types::I8))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_remove_string"), id);
        let id = decl(
            "ori_heap_remove_custom",
            &[pt, types::I64, pt],
            vec![],
            Some(types::I8),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_remove_custom"), id);
        let id = decl("ori_files_read_text", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_files_read_text"), id);
        let id = decl(
            "ori_files_write_text",
            &[pt, pt],
            vec![Ty::String, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_files_write_text"), id);
        let id = decl(
            "ori_files_append_text",
            &[pt, pt],
            vec![Ty::String, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_files_append_text"), id);
        let id = decl("ori_files_exists", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_files_exists"), id);
        let id = decl("ori_files_delete", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_files_delete"), id);
        let id = decl("ori_files_list_dir", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_files_list_dir"), id);
        let id = decl("ori_files_create_dir", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_files_create_dir"), id);
        let id = decl("ori_files_is_file", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_files_is_file"), id);
        let id = decl("ori_files_is_dir", &[pt], vec![Ty::String], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_files_is_dir"), id);
        let id = decl(
            "ori_files_copy",
            &[pt, pt],
            vec![Ty::String, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_files_copy"), id);
        let id = decl(
            "ori_files_rename",
            &[pt, pt],
            vec![Ty::String, Ty::String],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_files_rename"), id);
        let id = decl("ori_arc_retain", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_arc_retain"), id);
        let id = decl(
            "ori_handle_validate_size_type",
            &[pt, types::I64, types::I64],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_handle_validate_size_type"), id);
        let id = decl("ori_arc_release", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_arc_release"), id);
        let id = decl("ori_arc_register_edge", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_arc_register_edge"), id);
        let id = decl("ori_arc_unregister_edge", &[pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_arc_unregister_edge"), id);
        let id = decl("ori_arc_update_edge", &[pt, pt, pt], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_arc_update_edge"), id);
        let id = decl("ori_arc_collect_cycles", &[], vec![], Some(types::I64))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_arc_collect_cycles"), id);
        // Amortized safe point: full trial-deletion only every N allocations
        // (LANG-PERF-3 residual / LANG-MEM-3 partial — see ori-runtime).
        let id = decl("ori_arc_maybe_collect_cycles", &[], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_arc_maybe_collect_cycles"), id);
        // malloc / free for runtime allocation
        let id = decl("malloc", &[types::I64], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("malloc"), id);
        let id = decl("free", &[pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("free"), id);
        let id = decl("ori_alloc", &[types::I64, pt], vec![], Some(pt))?;
        self.stdlib_ids.insert(SmolStr::new("ori_alloc"), id);
        let id = decl(
            "ori_alloc_typed",
            &[types::I64, pt, types::I64],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_alloc_typed"), id);

        let id = decl("ori_abort_concurrent_modification", &[], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_abort_concurrent_modification"), id);
        let id = decl("ori_abort_division_by_zero", &[], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_abort_division_by_zero"), id);
        let id = decl("ori_abort_division_overflow", &[], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_abort_division_overflow"), id);
        let id = decl("ori_abort_shift_overflow", &[], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_abort_shift_overflow"), id);
        let id = decl(
            "ori_host_report_error",
            &[types::I32, pt],
            vec![Ty::Int, Ty::String],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_host_report_error"), id);

        let id = decl("ori_deque_iterator_new", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_deque_iterator_new"), id);
        let id = decl("ori_deque_iterator_next", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_deque_iterator_next"), id);

        let id = decl("ori_queue_iterator_new", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_queue_iterator_new"), id);
        let id = decl("ori_queue_iterator_next", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_queue_iterator_next"), id);

        let id = decl("ori_stack_iterator_new", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_stack_iterator_new"), id);
        let id = decl("ori_stack_iterator_next", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_stack_iterator_next"), id);

        let id = decl("ori_linked_list_iterator_new", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_linked_list_iterator_new"), id);
        let id = decl("ori_linked_list_iterator_next", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_linked_list_iterator_next"), id);

        let id = decl(
            "ori_doubly_linked_list_iterator_new",
            &[pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_doubly_linked_list_iterator_new"), id);
        let id = decl(
            "ori_doubly_linked_list_iterator_next",
            &[pt],
            vec![],
            Some(pt),
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_doubly_linked_list_iterator_next"), id);

        let id = decl("ori_heap_iterator_new", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_iterator_new"), id);
        let id = decl("ori_heap_iterator_next", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_heap_iterator_next"), id);

        let id = decl("ori_graph_iterator_new", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_graph_iterator_new"), id);
        let id = decl("ori_graph_iterator_next", &[pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_graph_iterator_next"), id);
        let id = decl("ori_graph_new_custom", &[types::I8, pt], vec![], Some(pt))?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_graph_new_custom"), id);
        let id = decl("ori_graph_set_eq", &[pt, pt], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_graph_set_eq"), id);

        // Cooperative DAP line probes (no-op in runtime unless ORI_DEBUG_PORT is set).
        let id = decl(
            "ori_debug_line",
            &[pt, types::I32, types::I32],
            vec![],
            None,
        )?;
        self.stdlib_ids.insert(SmolStr::new("ori_debug_line"), id);
        let id = decl("ori_debug_init", &[], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_debug_init"), id);
        let id = decl("ori_debug_enter", &[pt, types::I32], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_debug_enter"), id);
        let id = decl("ori_debug_leave", &[], vec![], None)?;
        self.stdlib_ids.insert(SmolStr::new("ori_debug_leave"), id);
        let id = decl(
            "ori_debug_variable",
            &[pt, types::I32, types::I32, types::I64],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_debug_variable"), id);
        let id = decl(
            "ori_debug_managed",
            &[pt, types::I32, types::I32, pt],
            vec![],
            None,
        )?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_debug_managed"), id);
        let id = decl("ori_debug_register_static", &[pt, types::I32], vec![], None)?;
        self.stdlib_ids
            .insert(SmolStr::new("ori_debug_register_static"), id);

        Ok(())
    }

    fn make_sig(&self, f: &HirFunc) -> ir::Signature {
        let mut sig = self.module.make_signature();
        for p in &f.params {
            if let Some(t) = cl_type(&p.ty, self.ptr_ty) {
                sig.params.push(AbiParam::new(t));
            }
        }
        if let Some(t) = cl_type(&f.return_ty, self.ptr_ty) {
            sig.returns.push(AbiParam::new(t));
        }
        sig
    }

    fn c_export_scalar_struct_layouts(&self, ty: &Ty) -> Option<(StructLayout, StructLayout)> {
        let Ty::Named(def_id, args) = ty else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        let internal = self.struct_layouts.get(def_id)?.clone();
        if internal.fields.is_empty()
            || !internal
                .fields
                .iter()
                .all(|(_, field)| is_c_export_scalar_field_ty(&field.ty))
        {
            return None;
        }
        let external = compute_c_export_struct_layout(&internal, self.ptr_ty);
        Some((internal, external))
    }

    fn c_export_payload_type(&self, ty: &Ty) -> Option<types::Type> {
        match ty {
            Ty::Named(_, _) => Some(self.ptr_ty),
            _ => cl_type(ty, self.ptr_ty),
        }
    }

    fn c_export_parameter_bridge(&self, ty: &Ty) -> CExportParameterBridge {
        match ty {
            Ty::Optional(inner) => CExportParameterBridge::Optional {
                inner: self.c_export_payload_bridge(inner),
            },
            Ty::Result(ok, err) => CExportParameterBridge::Result {
                ok: (!matches!(**ok, Ty::Void)).then(|| self.c_export_payload_bridge(ok)),
                err: (!matches!(**err, Ty::Void)).then(|| self.c_export_payload_bridge(err)),
            },
            _ => match self.c_export_scalar_struct_layouts(ty) {
                Some((internal, external)) => {
                    CExportParameterBridge::ScalarStruct { internal, external }
                }
                None => CExportParameterBridge::Direct,
            },
        }
    }

    fn c_export_return_bridge(&self, ty: &Ty) -> CExportReturnBridge {
        match ty {
            Ty::Bytes => CExportReturnBridge::Bytes,
            Ty::Optional(inner) => CExportReturnBridge::Optional {
                inner: self.c_export_payload_bridge(inner),
            },
            Ty::Result(ok, err) => CExportReturnBridge::Result {
                ok: (!matches!(**ok, Ty::Void)).then(|| self.c_export_payload_bridge(ok)),
                err: (!matches!(**err, Ty::Void)).then(|| self.c_export_payload_bridge(err)),
            },
            _ => match self.c_export_scalar_struct_layouts(ty) {
                Some((internal, external)) => {
                    CExportReturnBridge::ScalarStruct { internal, external }
                }
                None => CExportReturnBridge::Direct,
            },
        }
    }

    fn c_export_payload_bridge(&self, ty: &Ty) -> CExportPayloadBridge {
        CExportPayloadBridge {
            ty: ty.clone(),
            scalar_struct_layouts: self.c_export_scalar_struct_layouts(ty),
            managed_handle_size: self.c_export_managed_handle_size(ty),
            managed_handle_type_tag: self.c_export_managed_handle_type_tag(ty),
        }
    }

    /// Return the runtime payload size for an opaque, non-generic managed
    /// handle. The generated C wrapper uses this as a provenance/layout guard
    /// before retaining or dereferencing a host-provided pointer. Generic
    /// handles intentionally fall back to the legacy provenance-only check
    /// because their monomorphized layout is not represented in this table.
    fn c_export_managed_handle_size(&self, ty: &Ty) -> Option<u32> {
        let Ty::Named(def_id, args) = ty else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        self.struct_layouts
            .get(def_id)
            .map(|layout| layout.size)
            .or_else(|| self.enum_layouts.get(def_id).map(|layout| layout.size))
    }

    fn c_export_managed_handle_type_tag(&self, ty: &Ty) -> Option<i64> {
        let Ty::Named(def_id, args) = ty else {
            return None;
        };
        if !args.is_empty()
            || (!self.struct_layouts.contains_key(def_id)
                && !self.enum_layouts.contains_key(def_id))
        {
            return None;
        }
        // Zero is reserved for legacy/runtime allocations in the registry.
        Some(i64::from(def_id.0) + 1)
    }

    fn make_c_export_sig(&self, f: &HirFunc) -> ir::Signature {
        let mut sig = self.module.make_signature();
        for param in &f.params {
            match &param.ty {
                Ty::Optional(inner) => {
                    sig.params.push(AbiParam::new(types::I8));
                    if let Some(ty) = self.c_export_payload_type(inner) {
                        sig.params.push(AbiParam::new(ty));
                    }
                }
                Ty::Result(ok, err) => {
                    sig.params.push(AbiParam::new(types::I8));
                    if let Some(ty) = self.c_export_payload_type(ok) {
                        sig.params.push(AbiParam::new(ty));
                    }
                    if let Some(ty) = self.c_export_payload_type(err) {
                        sig.params.push(AbiParam::new(ty));
                    }
                }
                _ => {
                    if let Some(ty) = cl_type(&param.ty, self.ptr_ty) {
                        sig.params.push(AbiParam::new(ty));
                    }
                }
            }
        }
        match &f.return_ty {
            Ty::Bytes => {
                sig.params.push(AbiParam::new(self.ptr_ty));
            }
            Ty::Optional(inner) => {
                if self.c_export_payload_type(inner).is_some() {
                    sig.params.push(AbiParam::new(self.ptr_ty));
                }
                sig.returns.push(AbiParam::new(types::I8));
            }
            Ty::Result(ok, err) => {
                if self.c_export_payload_type(ok).is_some() {
                    sig.params.push(AbiParam::new(self.ptr_ty));
                }
                if self.c_export_payload_type(err).is_some() {
                    sig.params.push(AbiParam::new(self.ptr_ty));
                }
                sig.returns.push(AbiParam::new(types::I8));
            }
            _ if self.c_export_scalar_struct_layouts(&f.return_ty).is_some() => {
                sig.params.push(AbiParam::new(self.ptr_ty));
            }
            _ => {
                if let Some(ty) = cl_type(&f.return_ty, self.ptr_ty) {
                    sig.returns.push(AbiParam::new(ty));
                }
            }
        }
        sig
    }

    fn make_closure_wrapper_sig(&self, f: &HirFunc) -> ir::Signature {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(self.ptr_ty));
        let synthetic_env_params = usize::from(is_synthetic_closure_func(f));
        let params = f.params.iter().skip(synthetic_env_params);
        for p in params {
            if let Some(t) = cl_type(&p.ty, self.ptr_ty) {
                sig.params.push(AbiParam::new(t));
            }
        }
        if let Some(t) = cl_type(&f.return_ty, self.ptr_ty) {
            sig.returns.push(AbiParam::new(t));
        }
        sig
    }

    fn make_async_step_sig(&self) -> ir::Signature {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.returns.push(AbiParam::new(types::I64));
        sig
    }

    fn make_struct_eq_sig(&self) -> ir::Signature {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.returns.push(AbiParam::new(types::I8));
        sig
    }

    fn make_hash_sig(&self) -> ir::Signature {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.returns.push(AbiParam::new(types::I64));
        sig
    }

    fn declare_all(&mut self, hir: &HirModule) -> Result<(), String> {
        // Declare equality helper functions for all structs and enums.  The
        // helpers share the same pointer/pointer -> bool ABI so collections
        // can compare user-defined values without knowing their concrete
        // representation.
        let mut declared_structs = std::collections::HashSet::new();
        for s in &hir.structs {
            if !declared_structs.insert(s.def_id) {
                continue;
            }
            let sig = self.make_struct_eq_sig();
            let name = format!("__eq_helper_struct_{}", s.def_id.0);
            let id = self
                .module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(|e| format!("declare struct eq helper '{name}': {e}"))?;
            self.func_ids.insert(SmolStr::new(name), id);
        }
        let mut declared_enums = std::collections::HashSet::new();
        for e in &hir.enums {
            if !declared_enums.insert(e.def_id) {
                continue;
            }
            let sig = self.make_struct_eq_sig();
            let name = format!("__eq_helper_enum_{}", e.def_id.0);
            let id = self
                .module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(|e| format!("declare enum eq helper '{name}': {e}"))?;
            self.func_ids.insert(SmolStr::new(name), id);
        }
        let mut declared_hash_structs = std::collections::HashSet::new();
        for s in &hir.structs {
            if !declared_hash_structs.insert(s.def_id) {
                continue;
            }
            let sig = self.make_hash_sig();
            let name = format!("__hash_helper_struct_{}", s.def_id.0);
            let id = self
                .module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(|e| format!("declare struct hash helper '{name}': {e}"))?;
            self.func_ids.insert(SmolStr::new(name), id);
        }
        let mut declared_hash_enums = std::collections::HashSet::new();
        for e in &hir.enums {
            if !declared_hash_enums.insert(e.def_id) {
                continue;
            }
            let sig = self.make_hash_sig();
            let name = format!("__hash_helper_enum_{}", e.def_id.0);
            let id = self
                .module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(|e| format!("declare enum hash helper '{name}': {e}"))?;
            self.func_ids.insert(SmolStr::new(name), id);
        }
        for (type_def_id, _) in custom_destructor_impls(hir) {
            let mut sig = self.module.make_signature();
            sig.params.push(AbiParam::new(self.ptr_ty));
            let name = custom_destructor_callback_name(type_def_id);
            let id = self
                .module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(|e| format!("declare custom destructor callback '{name}': {e}"))?;
            self.func_ids.insert(name, id);
        }
        for f in &hir.funcs {
            let sig = self.make_sig(f);
            let link = if f.is_public {
                Linkage::Export
            } else {
                Linkage::Local
            };
            let id = self
                .module
                .declare_function(&native_func_symbol(&f.name), link, &sig)
                .map_err(|e| format!("declare '{}': {e}", f.name))?;
            self.func_ids.insert(f.name.clone(), id);
        }
        for ext in &hir.externs {
            match ext {
                HirExtern::Func {
                    path,
                    name,
                    params,
                    return_ty,
                    abi,
                    ..
                } => {
                    let mut sig = self.module.make_signature();
                    if self.host_callback_ids.contains_key(path) {
                        sig.params.push(AbiParam::new(types::I64));
                    }
                    for p in params {
                        if let Some(t) = cl_type(&p.ty, self.ptr_ty) {
                            sig.params.push(AbiParam::new(t));
                        }
                    }
                    if let Some(t) = cl_type(return_ty, self.ptr_ty) {
                        sig.returns.push(AbiParam::new(t));
                    }
                    let id = self
                        .module
                        .declare_function(name, Linkage::Import, &sig)
                        .map_err(|e| format!("declare extern '{}': {e}", name))?;

                    self.func_ids.insert(path.clone(), id);
                    self.func_param_tys.insert(
                        path.clone(),
                        params.iter().map(|param| param.ty.clone()).collect(),
                    );
                    // Synthetic `abi = "ori"` imports are functions defined
                    // in a sibling Ori object. They follow the same ARC
                    // ownership contract as local user functions; only real
                    // foreign ABI imports borrow their managed arguments.
                    if abi == "ori" {
                        self.user_func_names.insert(path.clone());
                    }
                }
                HirExtern::Var { path, name, ty, .. } => {
                    let data_id = self
                        .module
                        .declare_data(name, Linkage::Import, false, false)
                        .map_err(|e| format!("declare extern variable '{}': {e}", name))?;
                    self.global_data.insert(
                        path.clone(),
                        GlobalDataInfo {
                            data_id,
                            ty: ty.clone(),
                            mutable: false,
                        },
                    );
                }
            }
        }
        for f in &hir.funcs {
            let has_plan = f.is_async
                && (simple_async_state_machine_plan(f).is_some()
                    || collect_general_async_plan(f).is_some());
            if has_plan {
                let step_name = async_step_name(f);
                let sig = self.make_async_step_sig();
                let id = self
                    .module
                    .declare_function(&native_func_symbol(&step_name), Linkage::Local, &sig)
                    .map_err(|e| format!("declare async step '{}': {e}", step_name))?;
                self.func_ids.insert(step_name, id);
            }
        }
        for f in &hir.funcs {
            // A split module can pass any public function to a callback in a
            // different object, so local reference collection cannot know all
            // wrappers needed at this boundary. Emit every concrete wrapper so
            // a cross-module function value always has a definition to link.
            let sig = self.make_closure_wrapper_sig(f);
            let id = self
                .module
                // Split native modules call wrappers across object files. The
                // wrapper therefore needs external linkage even when the
                // underlying Ori function is private to its module.
                .declare_function(&native_func_wrapper_symbol(&f.name), Linkage::Export, &sig)
                .map_err(|e| format!("declare closure wrapper '{}': {e}", f.name))?;
            self.func_wrapper_ids.insert(f.name.clone(), id);
        }
        // Process entry `main` only for executables (not shared libraries).
        if !self.lib_mode && hir.funcs.iter().any(|f| is_entry_main(hir, f)) {
            let mut sig = self.module.make_signature();
            sig.params.push(AbiParam::new(types::I32));
            sig.params.push(AbiParam::new(self.ptr_ty));
            sig.returns.push(AbiParam::new(types::I32));
            self.module
                .declare_function("main", Linkage::Export, &sig)
                .map_err(|e| format!("declare main: {e}"))?;
        }
        // C ABI exports for embed hosts (`@c_export` / `--lib`).
        for f in &hir.funcs {
            if let Some(export_name) = &f.c_export_name {
                let sig = self.make_c_export_sig(f);
                let id = self
                    .module
                    .declare_function(export_name.as_str(), Linkage::Export, &sig)
                    .map_err(|e| format!("declare c_export '{export_name}': {e}"))?;
                self.c_export_ids.insert(f.name.clone(), id);
            }
        }
        // Module lifecycle exports for shared-library hosts.
        if self.lib_mode {
            let sig = self.module.make_signature();
            self.module
                .declare_function("__ori_module_init", Linkage::Export, &sig)
                .map_err(|e| format!("declare __ori_module_init: {e}"))?;
            self.module
                .declare_function("__ori_module_shutdown", Linkage::Export, &sig)
                .map_err(|e| format!("declare __ori_module_shutdown: {e}"))?;
        }
        Ok(())
    }

    fn declare_references_for_function(
        &mut self,
        function: &HirFunc,
        cranelift_function: &mut ir::Function,
    ) -> HashMap<SmolStr, ir::FuncRef> {
        let references = collect_function_references(function);
        let mut selected = HashMap::<SmolStr, FuncId>::new();

        if references.needs_all_runtime_symbols {
            selected.extend(
                self.stdlib_ids
                    .iter()
                    .map(|(name, function_id)| (name.clone(), *function_id)),
            );
        } else {
            for referenced_name in &references.functions {
                if let Some(function_id) = self.stdlib_ids.get(referenced_name) {
                    selected.insert(referenced_name.clone(), *function_id);
                }
            }
            if self.debug_path_data.is_some() {
                for name in [
                    "ori_arc_release",
                    "ori_debug_enter",
                    "ori_debug_leave",
                    "ori_debug_line",
                    "ori_debug_variable",
                    "ori_debug_managed",
                    "ori_list_capacity",
                    "ori_list_get",
                    "ori_list_len",
                    "ori_map_capacity",
                    "ori_map_keys",
                    "ori_map_len",
                    "ori_map_values",
                    "ori_set_capacity",
                    "ori_set_len",
                    "ori_set_to_list",
                    "ori_deque_len",
                    "ori_deque_to_list",
                    "ori_queue_len",
                    "ori_queue_to_list",
                    "ori_stack_len",
                    "ori_stack_to_list",
                    "ori_linked_list_len",
                    "ori_linked_list_to_list",
                    "ori_doubly_linked_list_len",
                    "ori_doubly_linked_list_to_list",
                    "ori_heap_len",
                    "ori_heap_to_list",
                ] {
                    if let Some(function_id) = self.stdlib_ids.get(name) {
                        selected.insert(SmolStr::new(name), *function_id);
                    }
                }
            }
        }

        if references.needs_all_runtime_symbols {
            // Complex lowering paths can select trait implementations and
            // internal helpers from types (for example `using`, `any<Trait>`,
            // and heap comparators) without naming them in the HIR.
            selected.extend(
                self.func_ids
                    .iter()
                    .map(|(name, function_id)| (name.clone(), *function_id)),
            );
        } else {
            for referenced_name in &references.functions {
                if let Some(function_id) = self.func_ids.get(referenced_name) {
                    selected.insert(referenced_name.clone(), *function_id);
                }
            }
            if function.is_async {
                let step_name = async_step_name(function);
                if let Some(function_id) = self.func_ids.get(&step_name) {
                    selected.insert(step_name, *function_id);
                }
            }
        }

        if references.needs_trait_implementations {
            for implementation in self.trait_impls.values() {
                for method in &implementation.methods {
                    if let Some(function_id) = self.func_ids.get(&method.func_name) {
                        selected.insert(method.func_name.clone(), *function_id);
                    }
                }
            }
        }

        if self.hosted_traps {
            for name in [
                "ori_host_report_error",
                "ori_alloc",
                "ori_arc_release",
                "ori_arc_maybe_collect_cycles",
                "ori_list_get",
                "ori_list_len",
                "ori_string_len",
                "ori_string_slice",
                "ori_bytes_get",
                "ori_bytes_len",
            ] {
                if let Some(function_id) = self.stdlib_ids.get(name) {
                    selected.insert(SmolStr::new(name), *function_id);
                }
            }
        }

        for wrapper_name in references.wrappers {
            let external_wrapper_name = SmolStr::new(format!("{wrapper_name}.__fnptr_wrapper"));
            let Some(function_id) = self
                .func_wrapper_ids
                .get(&wrapper_name)
                .or_else(|| self.func_ids.get(&external_wrapper_name))
            else {
                continue;
            };
            selected.insert(external_wrapper_name, *function_id);
        }

        let mut selected = selected.into_iter().collect::<Vec<_>>();
        selected.sort_unstable_by_key(|(name, _)| name.clone());
        selected
            .into_iter()
            .map(|(name, function_id)| {
                let function_reference = self
                    .module
                    .declare_func_in_func(function_id, cranelift_function);
                (name, function_reference)
            })
            .collect()
    }

    fn define_all(&mut self, hir: &HirModule) -> Result<(), String> {
        let const_exprs: HashMap<SmolStr, HirExpr> = hir
            .consts
            .iter()
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect();
        let module_init_guard_id = if self.lib_mode {
            let id = self
                .module
                .declare_anonymous_data(true, false)
                .map_err(|error| format!("declare module init guard: {error}"))?;
            let mut description = DataDescription::new();
            description.define(vec![0_u8].into_boxed_slice());
            self.module
                .define_data(id, &description)
                .map_err(|error| format!("define module init guard: {error}"))?;
            Some(id)
        } else {
            None
        };
        let global_init_id = if hir.consts.iter().any(|c| {
            cl_type(&c.ty, self.ptr_ty).is_some() && needs_runtime_global_init(&c.value, &c.ty)
        }) {
            let sig = self.module.make_signature();
            Some(
                self.module
                    .declare_function("__ori_init_globals", Linkage::Local, &sig)
                    .map_err(|e| format!("declare global initializer: {e}"))?,
            )
        } else {
            None
        };
        self.global_init_func_id = global_init_id;
        let global_drop_id = if global_init_id.is_some() {
            let sig = self.module.make_signature();
            Some(
                self.module
                    .declare_function("__ori_drop_globals", Linkage::Local, &sig)
                    .map_err(|error| format!("declare global teardown: {error}"))?,
            )
        } else {
            None
        };
        self.global_drop_func_id = global_drop_id;
        for f in &hir.funcs {
            let sig = self.make_sig(f);
            let func_id = self.func_ids[&f.name];
            let mut ctx = self.module.make_context();
            ctx.func.signature = sig;

            let func_refs = self.declare_references_for_function(f, &mut ctx.func);

            // Pre-declare all string global values
            let mut string_gvs: HashMap<SmolStr, ir::GlobalValue> = HashMap::new();
            for (s, &data_id) in sorted_smol_entries(&self.string_data) {
                let gv = self.module.declare_data_in_func(data_id, &mut ctx.func);
                string_gvs.insert(s.clone(), gv);
            }

            let mut global_gvs: HashMap<SmolStr, ir::GlobalValue> = HashMap::new();
            for (name, info) in sorted_smol_entries(&self.global_data) {
                let gv = self
                    .module
                    .declare_data_in_func(info.data_id, &mut ctx.func);
                global_gvs.insert(name.clone(), gv);
            }

            let instrument_debug = self.debug_path_data.is_some()
                && !f.is_async
                && !f.name.as_str().starts_with("ori.");
            let debug_file_gv = if instrument_debug {
                self.debug_path_data
                    .map(|id| self.module.declare_data_in_func(id, &mut ctx.func))
            } else {
                None
            };
            let debug_file_len = if instrument_debug {
                self.debug_source_path
                    .as_ref()
                    .map(|s| s.len() as u32)
                    .unwrap_or(0)
            } else {
                0
            };
            let debug_line_starts: &[u32] = if instrument_debug {
                self.debug_line_starts.as_slice()
            } else {
                &[]
            };
            let debug_function_gv = if instrument_debug {
                self.debug_name_data
                    .get(&f.name)
                    .map(|id| self.module.declare_data_in_func(*id, &mut ctx.func))
            } else {
                None
            };
            let mut debug_variable_gvs = HashMap::new();
            if instrument_debug {
                for (name, data_id) in sorted_smol_entries(&self.debug_name_data) {
                    let gv = self.module.declare_data_in_func(*data_id, &mut ctx.func);
                    debug_variable_gvs.insert(name.clone(), gv);
                }
            }

            let mut bctx = FunctionBuilderContext::new();
            {
                let builder = FunctionBuilder::new(&mut ctx.func, &mut bctx);
                FuncCodegen {
                    builder,
                    func_refs: &func_refs,
                    string_gvs: &string_gvs,
                    global_gvs: &global_gvs,
                    global_data: &self.global_data,
                    const_exprs: &const_exprs,
                    struct_layouts: &self.struct_layouts,
                    inline_struct_ids: &self.inline_struct_ids,
                    enum_layouts: &self.enum_layouts,
                    type_names: &self.type_names,
                    trait_layouts: &self.trait_layouts,
                    trait_impls: &self.trait_impls,
                    func_param_tys: &self.func_param_tys,
                    user_func_names: &self.user_func_names,
                    host_callback_ids: &self.host_callback_ids,
                    vars: vec![HashMap::new()],
                    ptr_ty: self.ptr_ty,
                    loop_stack: Vec::new(),
                    using_stack: Vec::new(),
                    managed_stack: Vec::new(),
                    current_return_ty: f.return_ty.clone(),
                    terminated: false,
                    broke_loop_target: None,
                    async_frame: None,
                    async_plan: None,
                    async_await_index: 0,
                    async_loop_index: 0,
                    async_poll_blocks: Vec::new(),
                    func_name: f.name.clone(),
                    debug_file_gv,
                    debug_file_len,
                    debug_line_starts,
                    debug_function_gv,
                    debug_variable_gvs: &debug_variable_gvs,
                    hosted_traps: self.hosted_traps && !f.is_async,
                }
                .emit_user_func(f)?;
            }
            // LANG-PERF-2-0: dump CLIF when ORI_DUMP_CLIF=1 or a file path.
            maybe_dump_clif(f.name.as_str(), &ctx);
            // LANG-MEM-7: dump inserted ARC ops when ORI_DUMP_ARC is set.
            maybe_dump_arc(f.name.as_str(), &ctx, &self.arc_dump_symbols());
            self.module
                .define_function(func_id, &mut ctx)
                .map_err(|e| format!("define '{}': {e}", f.name))?;
        }

        for (type_def_id, destructor_func_name) in custom_destructor_impls(hir) {
            let callback_name = custom_destructor_callback_name(type_def_id);
            let callback_id = self.func_ids[&callback_name];
            let destructor_id = self
                .func_ids
                .get(&destructor_func_name)
                .copied()
                .ok_or_else(|| {
                    format!(
                        "custom destructor `{destructor_func_name}` is missing from native codegen"
                    )
                })?;
            let mut ctx = self.module.make_context();
            let mut signature = self.module.make_signature();
            signature.params.push(AbiParam::new(self.ptr_ty));
            ctx.func.signature = signature;
            let destructor_ref = self
                .module
                .declare_func_in_func(destructor_id, &mut ctx.func);
            let mut bctx = FunctionBuilderContext::new();
            {
                let mut builder = FunctionBuilder::new(&mut ctx.func, &mut bctx);
                let block = builder.create_block();
                builder.append_block_params_for_function_params(block);
                builder.switch_to_block(block);
                builder.seal_block(block);
                let value = builder.block_params(block)[0];
                builder.ins().call(destructor_ref, &[value]);
                builder.ins().return_(&[]);
                builder.seal_all_blocks();
                builder.finalize();
            }
            self.module
                .define_function(callback_id, &mut ctx)
                .map_err(|e| format!("define custom destructor callback '{callback_name}': {e}"))?;
        }

        for f in &hir.funcs {
            let plan_opt =
                simple_async_state_machine_plan(f).or_else(|| collect_general_async_plan(f));
            let Some(plan) = plan_opt else {
                continue;
            };
            verify_async_frame_ownership(&plan, self.ptr_ty)?;
            self.define_simple_async_step(f, &plan, &const_exprs)?;
        }

        for f in &hir.funcs {
            let Some(&wrapper_id) = self.func_wrapper_ids.get(&f.name) else {
                continue;
            };
            let original_id = self.func_ids[&f.name];
            let mut ctx = self.module.make_context();
            ctx.func.signature = self.make_closure_wrapper_sig(f);
            let original_ref = self.module.declare_func_in_func(original_id, &mut ctx.func);
            let mut bctx = FunctionBuilderContext::new();
            {
                let mut b = FunctionBuilder::new(&mut ctx.func, &mut bctx);
                let block = b.create_block();
                b.append_block_params_for_function_params(block);
                b.switch_to_block(block);
                b.seal_block(block);
                let params: Vec<ir::Value> = if is_synthetic_closure_func(f) {
                    b.block_params(block).to_vec()
                } else {
                    b.block_params(block).iter().skip(1).copied().collect()
                };
                let call = b.ins().call(original_ref, &params);
                let results = b.inst_results(call).to_vec();
                if results.is_empty() {
                    b.ins().return_(&[]);
                } else {
                    b.ins().return_(&results);
                }
                b.seal_all_blocks();
                b.finalize();
            }
            self.module
                .define_function(wrapper_id, &mut ctx)
                .map_err(|e| format!("define closure wrapper '{}': {e}", f.name))?;
        }

        if let Some(global_init_id) = global_init_id {
            self.define_global_initializer(hir, global_init_id)?;
        }
        if let Some(global_drop_id) = global_drop_id {
            self.define_global_teardown(hir, global_drop_id)?;
        }

        // C ABI export wrappers (`@c_export`) — unmangled symbols for dlopen hosts.
        for f in &hir.funcs {
            let Some(&export_id) = self.c_export_ids.get(&f.name) else {
                continue;
            };
            let ori_id = self.func_ids[&f.name];
            let parameter_bridges = f
                .params
                .iter()
                .map(|param| self.c_export_parameter_bridge(&param.ty))
                .collect::<Vec<_>>();
            let return_bridge = self.c_export_return_bridge(&f.return_ty);
            let mut ctx = self.module.make_context();
            ctx.func.signature = self.make_c_export_sig(f);
            let ori_ref = self.module.declare_func_in_func(ori_id, &mut ctx.func);
            let alloc_id = self.stdlib_ids.get("ori_alloc").copied().ok_or_else(|| {
                "missing runtime function `ori_alloc` for C export wrapper".to_string()
            })?;
            let alloc_ref = self.module.declare_func_in_func(alloc_id, &mut ctx.func);
            let retain_id = self
                .stdlib_ids
                .get("ori_arc_retain")
                .copied()
                .ok_or_else(|| {
                    "missing runtime function `ori_arc_retain` for C export wrapper".to_string()
                })?;
            let retain_ref = self.module.declare_func_in_func(retain_id, &mut ctx.func);
            let release_id = self
                .stdlib_ids
                .get("ori_arc_release")
                .copied()
                .ok_or_else(|| {
                    "missing runtime function `ori_arc_release` for C export wrapper".to_string()
                })?;
            let release_ref = self.module.declare_func_in_func(release_id, &mut ctx.func);
            let copy_string_ref = self
                .stdlib_ids
                .get("ori_string_copy_host")
                .copied()
                .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
            let copy_bytes_ref = self
                .stdlib_ids
                .get("ori_bytes_copy_parts")
                .copied()
                .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
            let validate_handle_size_type_ref = self
                .stdlib_ids
                .get("ori_handle_validate_size_type")
                .copied()
                .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
            let bytes_len_ref = self
                .stdlib_ids
                .get("ori_bytes_len")
                .copied()
                .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
            let register_edge_id = self
                .stdlib_ids
                .get("ori_arc_register_edge")
                .copied()
                .ok_or_else(|| {
                    "missing runtime function `ori_arc_register_edge` for C export wrapper"
                        .to_string()
                })?;
            let register_edge_ref = self
                .module
                .declare_func_in_func(register_edge_id, &mut ctx.func);
            let mut bctx = FunctionBuilderContext::new();
            {
                let mut b = FunctionBuilder::new(&mut ctx.func, &mut bctx);
                let block = b.create_block();
                b.append_block_params_for_function_params(block);
                b.switch_to_block(block);
                b.seal_block(block);
                let external_params = b.block_params(block).to_vec();
                let mut ori_params = Vec::with_capacity(f.params.len());
                let mut external_index = 0usize;
                for (parameter, bridge) in f.params.iter().zip(&parameter_bridges) {
                    match bridge {
                        CExportParameterBridge::Direct => {
                            let external_value = external_params[external_index];
                            external_index += 1;
                            let bridge = self.c_export_payload_bridge(&parameter.ty);
                            let copy_ref = match &parameter.ty {
                                Ty::String => copy_string_ref,
                                Ty::Bytes => copy_bytes_ref,
                                _ => None,
                            };
                            let (internal_value, owns_temporary) =
                                emit_c_export_payload_from_external(
                                    &mut b,
                                    &bridge,
                                    external_value,
                                    alloc_ref,
                                    self.ptr_ty,
                                    copy_ref,
                                    CExportHandleValidation {
                                        size_type: validate_handle_size_type_ref,
                                        payload_size: bridge.managed_handle_size,
                                        payload_type_tag: bridge.managed_handle_type_tag,
                                    },
                                )?;
                            if !owns_temporary && is_managed_ty(&parameter.ty) {
                                b.ins().call(retain_ref, &[internal_value]);
                            }
                            ori_params.push(internal_value);
                        }
                        CExportParameterBridge::ScalarStruct { internal, external } => {
                            let external_value = external_params[external_index];
                            external_index += 1;
                            let internal_value = emit_c_export_allocation(
                                &mut b,
                                internal.size,
                                alloc_ref,
                                self.ptr_ty,
                            );
                            emit_copy_struct_fields(
                                &mut b,
                                external_value,
                                external,
                                internal_value,
                                internal,
                                self.ptr_ty,
                            )?;
                            ori_params.push(internal_value);
                        }
                        CExportParameterBridge::Optional { inner } => {
                            let tag = external_params[external_index];
                            external_index += 1;
                            let (payload_offset, total_size) =
                                optional_layout(&inner.ty, self.ptr_ty);
                            let wrapper = emit_c_export_allocation(
                                &mut b,
                                total_size,
                                alloc_ref,
                                self.ptr_ty,
                            );
                            b.ins().store(MemFlags::new(), tag, wrapper, 0);
                            let external_value = external_params[external_index];
                            external_index += 1;
                            emit_c_export_input_payload_if(
                                &mut b,
                                CExportInputPayload {
                                    tag,
                                    active_when_true: true,
                                    external_value,
                                    bridge: inner,
                                    wrapper,
                                    payload_offset,
                                },
                                CExportArcFunctions {
                                    allocate: alloc_ref,
                                    register_edge: register_edge_ref,
                                    release: release_ref,
                                    copy_string_input: copy_string_ref,
                                    copy_bytes_parts_input: copy_bytes_ref,
                                    validate_handle_size_type: validate_handle_size_type_ref,
                                },
                                self.ptr_ty,
                            )?;
                            ori_params.push(wrapper);
                        }
                        CExportParameterBridge::Result { ok, err } => {
                            let tag = external_params[external_index];
                            external_index += 1;
                            let ok_ty = ok.as_ref().map_or(&Ty::Void, |bridge| &bridge.ty);
                            let err_ty = err.as_ref().map_or(&Ty::Void, |bridge| &bridge.ty);
                            let (payload_offset, _, total_size) =
                                result_layout(ok_ty, err_ty, self.ptr_ty);
                            let wrapper = emit_c_export_allocation(
                                &mut b,
                                total_size,
                                alloc_ref,
                                self.ptr_ty,
                            );
                            b.ins().store(MemFlags::new(), tag, wrapper, 0);
                            if let Some(ok) = ok {
                                let external_value = external_params[external_index];
                                external_index += 1;
                                emit_c_export_input_payload_if(
                                    &mut b,
                                    CExportInputPayload {
                                        tag,
                                        active_when_true: true,
                                        external_value,
                                        bridge: ok,
                                        wrapper,
                                        payload_offset,
                                    },
                                    CExportArcFunctions {
                                        allocate: alloc_ref,
                                        register_edge: register_edge_ref,
                                        release: release_ref,
                                        copy_string_input: copy_string_ref,
                                        copy_bytes_parts_input: copy_bytes_ref,
                                        validate_handle_size_type: validate_handle_size_type_ref,
                                    },
                                    self.ptr_ty,
                                )?;
                            }
                            if let Some(err) = err {
                                let external_value = external_params[external_index];
                                external_index += 1;
                                emit_c_export_input_payload_if(
                                    &mut b,
                                    CExportInputPayload {
                                        tag,
                                        active_when_true: false,
                                        external_value,
                                        bridge: err,
                                        wrapper,
                                        payload_offset,
                                    },
                                    CExportArcFunctions {
                                        allocate: alloc_ref,
                                        register_edge: register_edge_ref,
                                        release: release_ref,
                                        copy_string_input: copy_string_ref,
                                        copy_bytes_parts_input: copy_bytes_ref,
                                        validate_handle_size_type: validate_handle_size_type_ref,
                                    },
                                    self.ptr_ty,
                                )?;
                            }
                            ori_params.push(wrapper);
                        }
                    }
                }
                let call = b.ins().call(ori_ref, &ori_params);
                let results = b.inst_results(call).to_vec();
                match &return_bridge {
                    CExportReturnBridge::Direct => {
                        if results.is_empty() {
                            b.ins().return_(&[]);
                        } else {
                            b.ins().return_(&results);
                        }
                    }
                    CExportReturnBridge::Bytes => {
                        let result = results.first().copied().ok_or_else(|| {
                            format!("C export `{}` did not return its bytes value", f.name)
                        })?;
                        let output = external_params[external_index];
                        emit_c_export_payload_to_external(
                            &mut b,
                            &CExportPayloadBridge {
                                ty: Ty::Bytes,
                                scalar_struct_layouts: None,
                                managed_handle_size: None,
                                managed_handle_type_tag: None,
                            },
                            result,
                            output,
                            Some(retain_ref),
                            self.ptr_ty,
                            bytes_len_ref,
                        )?;
                        b.ins().call(release_ref, &[result]);
                        b.ins().return_(&[]);
                    }
                    CExportReturnBridge::ScalarStruct { internal, external } => {
                        let result = results.first().copied().ok_or_else(|| {
                            format!("C export `{}` did not return its struct value", f.name)
                        })?;
                        let output = external_params[external_index];
                        emit_copy_struct_fields(
                            &mut b,
                            result,
                            internal,
                            output,
                            external,
                            self.ptr_ty,
                        )?;
                        b.ins().call(release_ref, &[result]);
                        b.ins().return_(&[]);
                    }
                    CExportReturnBridge::Optional { inner } => {
                        let result = results.first().copied().ok_or_else(|| {
                            format!("C export `{}` did not return its optional value", f.name)
                        })?;
                        let tag = b.ins().load(types::I8, MemFlags::new(), result, 0);
                        let (payload_offset, _) = optional_layout(&inner.ty, self.ptr_ty);
                        let output = external_params[external_index];
                        emit_c_export_output_payload_if(
                            &mut b,
                            CExportOutputPayload {
                                tag,
                                active_when_true: true,
                                wrapper: result,
                                payload_offset,
                                bridge: inner,
                                output,
                            },
                            retain_ref,
                            self.ptr_ty,
                            bytes_len_ref,
                        )?;
                        b.ins().call(release_ref, &[result]);
                        b.ins().return_(&[tag]);
                    }
                    CExportReturnBridge::Result { ok, err } => {
                        let result = results.first().copied().ok_or_else(|| {
                            format!("C export `{}` did not return its result value", f.name)
                        })?;
                        let tag = b.ins().load(types::I8, MemFlags::new(), result, 0);
                        let ok_ty = ok.as_ref().map_or(&Ty::Void, |bridge| &bridge.ty);
                        let err_ty = err.as_ref().map_or(&Ty::Void, |bridge| &bridge.ty);
                        let (payload_offset, _, _) = result_layout(ok_ty, err_ty, self.ptr_ty);
                        if let Some(ok) = ok {
                            let output = external_params[external_index];
                            external_index += 1;
                            emit_c_export_output_payload_if(
                                &mut b,
                                CExportOutputPayload {
                                    tag,
                                    active_when_true: true,
                                    wrapper: result,
                                    payload_offset,
                                    bridge: ok,
                                    output,
                                },
                                retain_ref,
                                self.ptr_ty,
                                bytes_len_ref,
                            )?;
                        }
                        if let Some(err) = err {
                            let output = external_params[external_index];
                            emit_c_export_output_payload_if(
                                &mut b,
                                CExportOutputPayload {
                                    tag,
                                    active_when_true: false,
                                    wrapper: result,
                                    payload_offset,
                                    bridge: err,
                                    output,
                                },
                                retain_ref,
                                self.ptr_ty,
                                bytes_len_ref,
                            )?;
                        }
                        b.ins().call(release_ref, &[result]);
                        b.ins().return_(&[tag]);
                    }
                }
                b.seal_all_blocks();
                b.finalize();
            }
            self.module
                .define_function(export_id, &mut ctx)
                .map_err(|e| {
                    format!(
                        "define c_export '{}': {e}",
                        f.c_export_name.as_deref().unwrap_or("?")
                    )
                })?;
        }

        // Shared-library module lifecycle. Hosts call this explicitly after
        // `ori_rt_init`; keeping it separate supports multiple generations.
        if self.lib_mode {
            let sig = self.module.make_signature();
            let init_id = self
                .module
                .declare_function("__ori_module_init", Linkage::Export, &sig)
                .map_err(|e| format!("re-declare __ori_module_init: {e}"))?;
            let mut ctx = self.module.make_context();
            ctx.func.signature = sig;
            let global_ref =
                global_init_id.map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
            let guard_value =
                module_init_guard_id.map(|id| self.module.declare_data_in_func(id, &mut ctx.func));
            let mut bctx = FunctionBuilderContext::new();
            {
                let mut b = FunctionBuilder::new(&mut ctx.func, &mut bctx);
                let blk = b.create_block();
                let initialize_block = b.create_block();
                let return_block = b.create_block();
                b.switch_to_block(blk);
                b.seal_block(blk);
                if let Some(guard_value) = guard_value {
                    let guard_address = b.ins().global_value(self.ptr_ty, guard_value);
                    let initialized = b.ins().load(types::I8, MemFlags::new(), guard_address, 0);
                    b.ins()
                        .brif(initialized, return_block, &[], initialize_block, &[]);
                    b.seal_block(initialize_block);
                    b.switch_to_block(initialize_block);
                    let one = b.ins().iconst(types::I8, 1);
                    b.ins().store(MemFlags::new(), one, guard_address, 0);
                    if let Some(init_ref) = global_ref {
                        b.ins().call(init_ref, &[]);
                    }
                    b.ins().jump(return_block, &[]);
                    b.seal_block(return_block);
                    b.switch_to_block(return_block);
                } else if let Some(init_ref) = global_ref {
                    b.ins().call(init_ref, &[]);
                }
                b.ins().return_(&[]);
                b.seal_all_blocks();
                b.finalize();
            }
            self.module
                .define_function(init_id, &mut ctx)
                .map_err(|e| format!("define __ori_module_init: {e}"))?;

            let sig = self.module.make_signature();
            let shutdown_id = self
                .module
                .declare_function("__ori_module_shutdown", Linkage::Export, &sig)
                .map_err(|e| format!("re-declare __ori_module_shutdown: {e}"))?;
            let mut ctx = self.module.make_context();
            ctx.func.signature = sig;
            let drop_ref =
                global_drop_id.map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
            let guard_value =
                module_init_guard_id.map(|id| self.module.declare_data_in_func(id, &mut ctx.func));
            let mut builder_context = FunctionBuilderContext::new();
            {
                let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
                let block = builder.create_block();
                let drop_block = builder.create_block();
                let return_block = builder.create_block();
                builder.switch_to_block(block);
                builder.seal_block(block);
                if let Some(guard_value) = guard_value {
                    let guard_address = builder.ins().global_value(self.ptr_ty, guard_value);
                    let initialized =
                        builder
                            .ins()
                            .load(types::I8, MemFlags::new(), guard_address, 0);
                    builder
                        .ins()
                        .brif(initialized, drop_block, &[], return_block, &[]);
                    builder.seal_block(drop_block);
                    builder.switch_to_block(drop_block);
                    let zero = builder.ins().iconst(types::I8, 0);
                    builder.ins().store(MemFlags::new(), zero, guard_address, 0);
                    if let Some(drop_ref) = drop_ref {
                        builder.ins().call(drop_ref, &[]);
                    }
                    builder.ins().jump(return_block, &[]);
                    builder.seal_block(return_block);
                    builder.switch_to_block(return_block);
                } else if let Some(drop_ref) = drop_ref {
                    builder.ins().call(drop_ref, &[]);
                }
                builder.ins().return_(&[]);
                builder.seal_all_blocks();
                builder.finalize();
            }
            self.module
                .define_function(shutdown_id, &mut ctx)
                .map_err(|e| format!("define __ori_module_shutdown: {e}"))?;
        }

        // Define C main wrapper (executables only)
        if !self.lib_mode {
            if let Some(entry_main) = hir.funcs.iter().find(|f| is_entry_main(hir, f)) {
                let ori_main_id = self.func_ids[&entry_main.name];
                let mut sig = self.module.make_signature();
                sig.params.push(AbiParam::new(types::I32));
                sig.params.push(AbiParam::new(self.ptr_ty));
                sig.returns.push(AbiParam::new(types::I32));
                let main_id = self
                    .module
                    .declare_function("main", Linkage::Export, &sig)
                    .map_err(|e| format!("re-declare main: {e}"))?;
                self.main_func_id = Some(main_id);
                let mut ctx = self.module.make_context();
                ctx.func.signature = sig;
                let ori_ref = self.module.declare_func_in_func(ori_main_id, &mut ctx.func);
                let init_ref =
                    global_init_id.map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
                let set_args_ref = self
                    .stdlib_ids
                    .get("ori_os_set_args")
                    .copied()
                    .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
                let debug_init_ref = self
                    .stdlib_ids
                    .get("ori_debug_init")
                    .copied()
                    .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
                let debug_register_static_ref = self
                    .stdlib_ids
                    .get("ori_debug_register_static")
                    .copied()
                    .map(|id| self.module.declare_func_in_func(id, &mut ctx.func));
                let static_string_gvs = sorted_smol_entries(&self.string_data)
                    .into_iter()
                    .map(|(literal, data_id)| {
                        (
                            literal.len() as u32,
                            self.module.declare_data_in_func(*data_id, &mut ctx.func),
                        )
                    })
                    .collect::<Vec<_>>();
                let block_on_ref = if matches!(entry_main.return_ty, Ty::Future(_)) {
                    Some(
                        self.stdlib_ids
                            .get("ori_task_block_on")
                            .copied()
                            .map(|id| self.module.declare_func_in_func(id, &mut ctx.func))
                            .ok_or_else(|| {
                                "missing runtime function `ori_task_block_on` for async main"
                                    .to_string()
                            })?,
                    )
                } else {
                    None
                };
                let mut bctx = FunctionBuilderContext::new();
                {
                    let mut b = FunctionBuilder::new(&mut ctx.func, &mut bctx);
                    let blk = b.create_block();
                    b.append_block_params_for_function_params(blk);
                    b.switch_to_block(blk);
                    b.seal_block(blk);
                    if let Some(init_ref) = init_ref {
                        b.ins().call(init_ref, &[]);
                    }
                    if let Some(debug_init_ref) = debug_init_ref {
                        b.ins().call(debug_init_ref, &[]);
                    }
                    if let Some(register_ref) = debug_register_static_ref {
                        for (length, data_gv) in &static_string_gvs {
                            let base = b.ins().global_value(self.ptr_ty, *data_gv);
                            let payload = b.ins().iadd_imm(base, 16);
                            let length = b.ins().iconst(types::I32, i64::from(*length));
                            b.ins().call(register_ref, &[payload, length]);
                        }
                    }
                    if let Some(set_args_ref) = set_args_ref {
                        let (argc, argv) = {
                            let params = b.block_params(blk);
                            (params[0], params[1])
                        };
                        b.ins().call(set_args_ref, &[argc, argv]);
                    }
                    let call = b.ins().call(ori_ref, &[]);
                    if let Some(block_on_ref) = block_on_ref {
                        let future = b.inst_results(call)[0];
                        b.ins().call(block_on_ref, &[future]);
                    }
                    let zero = b.ins().iconst(types::I32, 0);
                    b.ins().return_(&[zero]);
                    b.seal_all_blocks();
                    b.finalize();
                }
                self.module
                    .define_function(main_id, &mut ctx)
                    .map_err(|e| format!("define main wrapper: {e}"))?;
            }
        } // !lib_mode
        self.define_equality_helpers(hir)?;
        self.define_hash_helpers(hir)?;
        Ok(())
    }

    fn define_equality_helpers(&mut self, hir: &HirModule) -> Result<(), String> {
        let const_exprs: HashMap<SmolStr, HirExpr> = hir
            .consts
            .iter()
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect();
        let mut defined_structs = std::collections::HashSet::new();
        for s in &hir.structs {
            if !defined_structs.insert(s.def_id) {
                continue;
            }
            self.define_equality_helper(s.def_id, EqualityHelperTarget::Struct, &const_exprs)?;
        }
        let mut defined_enums = std::collections::HashSet::new();
        for e in &hir.enums {
            if !defined_enums.insert(e.def_id) {
                continue;
            }
            self.define_equality_helper(e.def_id, EqualityHelperTarget::Enum, &const_exprs)?;
        }
        Ok(())
    }

    fn define_equality_helper(
        &mut self,
        def_id: ori_types::DefId,
        target: EqualityHelperTarget,
        const_exprs: &HashMap<SmolStr, HirExpr>,
    ) -> Result<(), String> {
        let name = match target {
            EqualityHelperTarget::Struct => format!("__eq_helper_struct_{}", def_id.0),
            EqualityHelperTarget::Enum => format!("__eq_helper_enum_{}", def_id.0),
        };
        let func_id = self.func_ids[name.as_str()];
        let mut ctx = self.module.make_context();
        ctx.func.signature = self.make_struct_eq_sig();

        let mut func_refs: HashMap<SmolStr, ir::FuncRef> = HashMap::new();
        for (name, &id) in sorted_smol_entries(&self.func_ids)
            .into_iter()
            .chain(sorted_smol_entries(&self.stdlib_ids))
        {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(name.clone(), fref);
        }
        for (name, &id) in sorted_smol_entries(&self.func_wrapper_ids) {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(SmolStr::new(format!("{name}.__fnptr_wrapper")), fref);
        }

        let mut bctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut bctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            builder.seal_block(block);

            let params = builder.block_params(block);
            let left_ptr = params[0];
            let right_ptr = params[1];

            let mut codegen = FuncCodegen {
                builder,
                func_refs: &func_refs,
                string_gvs: &HashMap::new(),
                global_gvs: &HashMap::new(),
                global_data: &self.global_data,
                const_exprs,
                struct_layouts: &self.struct_layouts,
                inline_struct_ids: &self.inline_struct_ids,
                enum_layouts: &self.enum_layouts,
                type_names: &self.type_names,
                trait_layouts: &self.trait_layouts,
                trait_impls: &self.trait_impls,
                func_param_tys: &self.func_param_tys,
                user_func_names: &self.user_func_names,
                vars: vec![HashMap::new()],
                ptr_ty: self.ptr_ty,
                loop_stack: Vec::new(),
                using_stack: Vec::new(),
                managed_stack: Vec::new(),
                current_return_ty: Ty::Bool,
                terminated: false,
                broke_loop_target: None,
                async_frame: None,
                async_plan: None,
                async_await_index: 0,
                async_loop_index: 0,
                async_poll_blocks: Vec::new(),
                func_name: SmolStr::new(name.as_str()),
                debug_file_gv: None,
                debug_file_len: 0,
                debug_line_starts: &[],
                debug_function_gv: None,
                debug_variable_gvs: &HashMap::new(),
                hosted_traps: false,
                host_callback_ids: &self.host_callback_ids,
            };

            let res = if let Some(method_name) =
                codegen.trait_method_func_name_for_trait(def_id, ".Equatable", "equals")
            {
                let fref = *func_refs
                    .get(method_name.as_str())
                    .ok_or_else(|| format!("missing Equatable implementation `{method_name}`"))?;
                let call = codegen.builder.ins().call(fref, &[left_ptr, right_ptr]);
                codegen.builder.inst_results(call)[0]
            } else if matches!(target, EqualityHelperTarget::Struct)
                && codegen.struct_supports_equality(def_id)
            {
                codegen.emit_struct_equality(left_ptr, right_ptr, def_id, &[], true)?
            } else if matches!(target, EqualityHelperTarget::Enum)
                && codegen.enum_supports_equality(def_id)
            {
                codegen.emit_enum_equality(left_ptr, right_ptr, def_id, &[], true)?
            } else {
                codegen.builder.ins().iconst(types::I8, 0)
            };

            codegen.builder.ins().return_(&[res]);
            codegen.builder.seal_all_blocks();
            codegen.builder.finalize();
        }

        self.module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("define equality helper '{}': {e}", name))?;
        Ok(())
    }

    fn define_hash_helpers(&mut self, hir: &HirModule) -> Result<(), String> {
        let const_exprs: HashMap<SmolStr, HirExpr> = hir
            .consts
            .iter()
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect();
        let mut defined_structs = std::collections::HashSet::new();
        for s in &hir.structs {
            if defined_structs.insert(s.def_id) {
                self.define_hash_helper(s.def_id, HashHelperTarget::Struct, &const_exprs)?;
            }
        }
        let mut defined_enums = std::collections::HashSet::new();
        for e in &hir.enums {
            if defined_enums.insert(e.def_id) {
                self.define_hash_helper(e.def_id, HashHelperTarget::Enum, &const_exprs)?;
            }
        }
        Ok(())
    }

    fn define_hash_helper(
        &mut self,
        def_id: ori_types::DefId,
        target: HashHelperTarget,
        const_exprs: &HashMap<SmolStr, HirExpr>,
    ) -> Result<(), String> {
        let name = match target {
            HashHelperTarget::Struct => format!("__hash_helper_struct_{}", def_id.0),
            HashHelperTarget::Enum => format!("__hash_helper_enum_{}", def_id.0),
        };
        let func_id = self.func_ids[name.as_str()];
        let mut ctx = self.module.make_context();
        ctx.func.signature = self.make_hash_sig();

        let mut func_refs: HashMap<SmolStr, ir::FuncRef> = HashMap::new();
        for (symbol, &id) in sorted_smol_entries(&self.func_ids)
            .into_iter()
            .chain(sorted_smol_entries(&self.stdlib_ids))
        {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(symbol.clone(), fref);
        }
        for (symbol, &id) in sorted_smol_entries(&self.func_wrapper_ids) {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(SmolStr::new(format!("{symbol}.__fnptr_wrapper")), fref);
        }

        let mut bctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut bctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            builder.seal_block(block);
            let value_ptr = builder.block_params(block)[0];
            let mut codegen = FuncCodegen {
                builder,
                func_refs: &func_refs,
                string_gvs: &HashMap::new(),
                global_gvs: &HashMap::new(),
                global_data: &self.global_data,
                const_exprs,
                struct_layouts: &self.struct_layouts,
                inline_struct_ids: &self.inline_struct_ids,
                enum_layouts: &self.enum_layouts,
                type_names: &self.type_names,
                trait_layouts: &self.trait_layouts,
                trait_impls: &self.trait_impls,
                func_param_tys: &self.func_param_tys,
                user_func_names: &self.user_func_names,
                vars: vec![HashMap::new()],
                ptr_ty: self.ptr_ty,
                loop_stack: Vec::new(),
                using_stack: Vec::new(),
                managed_stack: Vec::new(),
                current_return_ty: Ty::Int64,
                terminated: false,
                broke_loop_target: None,
                async_frame: None,
                async_plan: None,
                async_await_index: 0,
                async_loop_index: 0,
                async_poll_blocks: Vec::new(),
                func_name: SmolStr::new(name.as_str()),
                debug_file_gv: None,
                debug_file_len: 0,
                debug_line_starts: &[],
                debug_function_gv: None,
                debug_variable_gvs: &HashMap::new(),
                hosted_traps: false,
                host_callback_ids: &self.host_callback_ids,
            };
            // A user Hashable method is authoritative and must be used with a
            // custom Equatable implementation. Marker-only Hashable keeps the
            // generated structural hash path; a non-structural Equatable
            // without `hash` falls back to a constant so equal values remain
            // safe to probe.
            let result = if let Some(method_name) =
                codegen.trait_method_func_name_for_trait(def_id, ".Hashable", "hash")
            {
                let fref = *func_refs
                    .get(method_name.as_str())
                    .ok_or_else(|| format!("missing Hashable implementation `{method_name}`"))?;
                let call = codegen.builder.ins().call(fref, &[value_ptr]);
                codegen.builder.inst_results(call)[0]
            } else if codegen
                .trait_method_func_name_for_trait(def_id, ".Equatable", "equals")
                .is_some()
            {
                codegen.builder.ins().iconst(types::I64, 0)
            } else if matches!(target, HashHelperTarget::Struct)
                && codegen.struct_supports_equality(def_id)
            {
                codegen.emit_struct_hash(value_ptr, def_id, &[], true)?
            } else if matches!(target, HashHelperTarget::Enum)
                && codegen.enum_supports_equality(def_id)
            {
                codegen.emit_enum_hash(value_ptr, def_id, &[], true)?
            } else {
                codegen.builder.ins().iconst(types::I64, 0)
            };
            codegen.builder.ins().return_(&[result]);
            codegen.builder.seal_all_blocks();
            codegen.builder.finalize();
        }
        self.module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("define hash helper '{name}': {e}"))?;
        Ok(())
    }

    fn define_simple_async_step(
        &mut self,
        f: &HirFunc,
        plan: &SimpleAsyncStateMachinePlan,
        const_exprs: &HashMap<SmolStr, HirExpr>,
    ) -> Result<(), String> {
        let step_name = async_step_name(f);
        let sig = self.make_async_step_sig();
        let func_id = self.func_ids[&step_name];
        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let mut func_refs: HashMap<SmolStr, ir::FuncRef> = HashMap::new();
        for (name, &id) in sorted_smol_entries(&self.func_ids)
            .into_iter()
            .chain(sorted_smol_entries(&self.stdlib_ids))
        {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(name.clone(), fref);
        }
        for (name, &id) in sorted_smol_entries(&self.func_wrapper_ids) {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(SmolStr::new(format!("{name}.__fnptr_wrapper")), fref);
        }

        let mut string_gvs: HashMap<SmolStr, ir::GlobalValue> = HashMap::new();
        for (s, &data_id) in sorted_smol_entries(&self.string_data) {
            let gv = self.module.declare_data_in_func(data_id, &mut ctx.func);
            string_gvs.insert(s.clone(), gv);
        }

        let mut global_gvs: HashMap<SmolStr, ir::GlobalValue> = HashMap::new();
        for (name, info) in sorted_smol_entries(&self.global_data) {
            let gv = self
                .module
                .declare_data_in_func(info.data_id, &mut ctx.func);
            global_gvs.insert(name.clone(), gv);
        }

        let instrument_debug =
            self.debug_path_data.is_some() && !f.name.as_str().starts_with("ori.");
        let debug_file_gv = if instrument_debug {
            self.debug_path_data
                .map(|id| self.module.declare_data_in_func(id, &mut ctx.func))
        } else {
            None
        };
        let debug_file_len = if instrument_debug {
            self.debug_source_path
                .as_ref()
                .map(|s| s.len() as u32)
                .unwrap_or(0)
        } else {
            0
        };
        let debug_line_starts: &[u32] = if instrument_debug {
            self.debug_line_starts.as_slice()
        } else {
            &[]
        };
        let debug_function_gv = if instrument_debug {
            self.debug_name_data
                .get(&f.name)
                .map(|id| self.module.declare_data_in_func(*id, &mut ctx.func))
        } else {
            None
        };
        let mut debug_variable_gvs = HashMap::new();
        if instrument_debug {
            for (name, data_id) in sorted_smol_entries(&self.debug_name_data) {
                let gv = self.module.declare_data_in_func(*data_id, &mut ctx.func);
                debug_variable_gvs.insert(name.clone(), gv);
            }
        }

        let mut bctx = FunctionBuilderContext::new();
        {
            let builder = FunctionBuilder::new(&mut ctx.func, &mut bctx);
            let codegen = FuncCodegen {
                builder,
                func_refs: &func_refs,
                string_gvs: &string_gvs,
                global_gvs: &global_gvs,
                global_data: &self.global_data,
                const_exprs,
                struct_layouts: &self.struct_layouts,
                inline_struct_ids: &self.inline_struct_ids,
                enum_layouts: &self.enum_layouts,
                type_names: &self.type_names,
                trait_layouts: &self.trait_layouts,
                trait_impls: &self.trait_impls,
                func_param_tys: &self.func_param_tys,
                user_func_names: &self.user_func_names,
                host_callback_ids: &self.host_callback_ids,
                vars: vec![HashMap::new()],
                ptr_ty: self.ptr_ty,
                loop_stack: Vec::new(),
                using_stack: Vec::new(),
                managed_stack: Vec::new(),
                current_return_ty: plan.inner_ty.clone(),
                terminated: false,
                broke_loop_target: None,
                async_frame: None,
                async_plan: None,
                async_await_index: 0,
                async_loop_index: 0,
                async_poll_blocks: Vec::new(),
                func_name: f.name.clone(),
                debug_file_gv,
                debug_file_len,
                debug_line_starts,
                debug_function_gv,
                debug_variable_gvs: &debug_variable_gvs,
                hosted_traps: false,
            };
            if plan.is_general {
                codegen.emit_general_async_step(f, plan)?;
            } else {
                codegen.emit_simple_async_step(f, plan)?;
            }
        }
        if std::env::var("ORI_DUMP_CLIF").is_ok() {
            eprintln!(
                "--- CLIF for async step `{}` ---\n{}",
                step_name,
                ctx.func.display()
            );
        }
        self.module
            .define_function(func_id, &mut ctx)
            .map_err(|e| {
                if std::env::var("ORI_DUMP_CLIF").is_ok() {
                    format!("define async step '{}': {e} ({e:?})", step_name)
                } else {
                    format!("define async step '{}': {e}", step_name)
                }
            })?;
        Ok(())
    }

    fn define_global_initializer(
        &mut self,
        hir: &HirModule,
        init_id: FuncId,
    ) -> Result<(), String> {
        let mut ctx = self.module.make_context();
        ctx.func.signature = self.module.make_signature();

        let mut func_refs: HashMap<SmolStr, ir::FuncRef> = HashMap::new();
        for (name, &id) in sorted_smol_entries(&self.func_ids)
            .into_iter()
            .chain(sorted_smol_entries(&self.stdlib_ids))
        {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(name.clone(), fref);
        }
        for (name, &id) in sorted_smol_entries(&self.func_wrapper_ids) {
            let fref = self.module.declare_func_in_func(id, &mut ctx.func);
            func_refs.insert(SmolStr::new(format!("{name}.__fnptr_wrapper")), fref);
        }

        let mut string_gvs: HashMap<SmolStr, ir::GlobalValue> = HashMap::new();
        for (s, &data_id) in sorted_smol_entries(&self.string_data) {
            let gv = self.module.declare_data_in_func(data_id, &mut ctx.func);
            string_gvs.insert(s.clone(), gv);
        }

        let mut global_gvs: HashMap<SmolStr, ir::GlobalValue> = HashMap::new();
        for (name, info) in sorted_smol_entries(&self.global_data) {
            let gv = self
                .module
                .declare_data_in_func(info.data_id, &mut ctx.func);
            global_gvs.insert(name.clone(), gv);
        }

        let const_exprs: HashMap<SmolStr, HirExpr> = hir
            .consts
            .iter()
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect();
        let mut bctx = FunctionBuilderContext::new();
        {
            let mut codegen = FuncCodegen {
                builder: FunctionBuilder::new(&mut ctx.func, &mut bctx),
                func_refs: &func_refs,
                string_gvs: &string_gvs,
                global_gvs: &global_gvs,
                global_data: &self.global_data,
                const_exprs: &const_exprs,
                struct_layouts: &self.struct_layouts,
                inline_struct_ids: &self.inline_struct_ids,
                enum_layouts: &self.enum_layouts,
                type_names: &self.type_names,
                trait_layouts: &self.trait_layouts,
                trait_impls: &self.trait_impls,
                func_param_tys: &self.func_param_tys,
                user_func_names: &self.user_func_names,
                host_callback_ids: &self.host_callback_ids,
                vars: vec![HashMap::new()],
                ptr_ty: self.ptr_ty,
                loop_stack: Vec::new(),
                using_stack: Vec::new(),
                managed_stack: Vec::new(),
                current_return_ty: Ty::Void,
                terminated: false,
                broke_loop_target: None,
                async_frame: None,
                async_plan: None,
                async_await_index: 0,
                async_loop_index: 0,
                async_poll_blocks: Vec::new(),
                func_name: SmolStr::new(""),
                debug_file_gv: None,
                debug_file_len: 0,
                debug_line_starts: &[],
                debug_function_gv: None,
                debug_variable_gvs: &HashMap::new(),
                hosted_traps: self.hosted_traps,
            };
            let block = codegen.builder.create_block();
            codegen.builder.switch_to_block(block);
            codegen.builder.seal_block(block);
            for global in &hir.consts {
                if cl_type(&global.ty, self.ptr_ty).is_none()
                    || !needs_runtime_global_init(&global.value, &global.ty)
                {
                    continue;
                }
                let value_is_owned = FuncCodegen::expr_produces_owned_ref(&global.value);
                let value = codegen.emit_expr_for_expected(&global.value, &global.ty)?;
                if !value_is_owned {
                    codegen.emit_arc_retain_if_managed(&global.ty, value)?;
                }
                if !codegen.initialize_global(&global.name, value) {
                    return Err(format!(
                        "global `{}` is not available during native initialization",
                        global.name
                    ));
                }
            }
            codegen.builder.ins().return_(&[]);
            codegen.builder.seal_all_blocks();
            codegen.builder.finalize();
        }

        self.module
            .define_function(init_id, &mut ctx)
            .map_err(|e| format!("define global initializer: {e}"))?;
        Ok(())
    }

    fn define_global_teardown(
        &mut self,
        hir: &HirModule,
        teardown_id: FuncId,
    ) -> Result<(), String> {
        let release_id = self
            .stdlib_ids
            .get("ori_arc_release")
            .copied()
            .ok_or_else(|| "missing `ori_arc_release` for global teardown".to_owned())?;
        let mut context = self.module.make_context();
        context.func.signature = self.module.make_signature();
        let release = self
            .module
            .declare_func_in_func(release_id, &mut context.func);
        let globals = hir
            .consts
            .iter()
            .filter(|global| {
                is_managed_ty(&global.ty)
                    && needs_runtime_global_init(&global.value, &global.ty)
                    && self.global_data.contains_key(&global.name)
            })
            .filter_map(|global| {
                let info = self.global_data.get(&global.name)?;
                let value_type = cl_type(&info.ty, self.ptr_ty)?;
                let global_value = self
                    .module
                    .declare_data_in_func(info.data_id, &mut context.func);
                Some((global_value, value_type))
            })
            .collect::<Vec<_>>();
        let mut builder_context = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
            let block = builder.create_block();
            builder.switch_to_block(block);
            builder.seal_block(block);
            for (global, value_type) in globals {
                let address = builder.ins().global_value(self.ptr_ty, global);
                let value = builder.ins().load(value_type, MemFlags::new(), address, 0);
                let zero = builder.ins().iconst(value_type, 0);
                builder.ins().store(MemFlags::new(), zero, address, 0);
                // Clear the published slot before release so a custom
                // destructor that re-enters generated code cannot observe or
                // release the object currently being finalized.
                builder.ins().call(release, &[value]);
            }
            builder.ins().return_(&[]);
            builder.seal_all_blocks();
            builder.finalize();
        }
        self.module
            .define_function(teardown_id, &mut context)
            .map_err(|error| format!("define global teardown: {error}"))
    }
}

// == Per-function codegen ==

#[derive(Clone, Copy)]
struct ForLoopInput<'a> {
    binding: &'a SmolStr,
    index_binding: Option<&'a SmolStr>,
    iterable: &'a HirExpr,
    body: &'a HirBlock,
    has_await: bool,
}

struct FuncCodegen<'a> {
    builder: FunctionBuilder<'a>,
    func_refs: &'a HashMap<SmolStr, ir::FuncRef>,
    string_gvs: &'a HashMap<SmolStr, ir::GlobalValue>,
    global_gvs: &'a HashMap<SmolStr, ir::GlobalValue>,
    global_data: &'a HashMap<SmolStr, GlobalDataInfo>,
    const_exprs: &'a HashMap<SmolStr, HirExpr>,
    struct_layouts: &'a HashMap<ori_types::DefId, StructLayout>,
    inline_struct_ids: &'a HashSet<ori_types::DefId>,
    enum_layouts: &'a HashMap<ori_types::DefId, EnumLayout>,
    type_names: &'a HashMap<ori_types::DefId, SmolStr>,
    trait_layouts: &'a HashMap<ori_types::DefId, HirTrait>,
    trait_impls: &'a HashMap<(ori_types::DefId, ori_types::DefId), HirTraitImpl>,
    func_param_tys: &'a HashMap<SmolStr, Vec<Ty>>,
    user_func_names: &'a HashSet<SmolStr>,
    vars: Vec<HashMap<SmolStr, (Variable, Ty)>>,
    ptr_ty: types::Type,
    loop_stack: Vec<LoopContext>,
    using_stack: Vec<UsingCleanup>,
    managed_stack: Vec<ManagedCleanup>,
    current_return_ty: Ty,
    terminated: bool,
    /// When the most recent terminator was a `break`, the loop exit it jumped
    /// to; lets loop epilogues distinguish a real break (exit has a jump) from
    /// a `return` (exit is unreachable) without inspecting the CFG.
    broke_loop_target: Option<ir::Block>,
    async_frame: Option<ir::Value>,
    async_plan: Option<&'a SimpleAsyncStateMachinePlan>,
    async_await_index: usize,
    async_loop_index: usize,
    async_poll_blocks: Vec<ir::Block>,
    func_name: SmolStr,
    /// When set, emit `ori_debug_line` at statement boundaries.
    debug_file_gv: Option<ir::GlobalValue>,
    debug_file_len: u32,
    debug_line_starts: &'a [u32],
    debug_function_gv: Option<ir::GlobalValue>,
    debug_variable_gvs: &'a HashMap<SmolStr, ir::GlobalValue>,
    hosted_traps: bool,
    host_callback_ids: &'a HashMap<SmolStr, u64>,
}

impl<'a> FuncCodegen<'a> {
    fn display_ty(&self, ty: &Ty) -> String {
        ty.display_with_names(self.type_names)
    }

    /// GFX-INLINE-1: is `ty` stored in place (inline) rather than behind one
    /// pointer? Mirrors `Inline(T)` from the checker.
    fn ty_is_inline_for_layout(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Array(elem, _) => self.ty_is_inline_for_layout(elem),
            Ty::Named(def_id, _) => self.inline_struct_ids.contains(def_id),
            _ => ori_types::is_inline_ty(ty),
        }
    }

    /// GFX-INLINE-1: layout-aware field size for codegen. Inline structs and
    /// arrays of inline structs resolve to their real byte size.
    fn field_size_align_for_layout(&self, ty: &Ty) -> (u32, u8) {
        field_size_align_with_layouts_typed(
            ty,
            self.ptr_ty,
            self.struct_layouts,
            self.inline_struct_ids,
        )
    }

    /// GFX-INLINE-1: copy the bytes of an inline value (`source`, a heap
    /// pointer produced by a struct literal) into `destination + offset`.
    /// Scalars are stored directly by the caller; only inline aggregates
    /// (structs and arrays of inline structs) reach here.
    fn emit_copy_inline_into(
        &mut self,
        source: ir::Value,
        ty: &Ty,
        destination: ir::Value,
        offset: i32,
    ) -> Result<(), String> {
        match ty {
            Ty::Named(def_id, _) => {
                let layout = self.struct_layouts.get(def_id).cloned().ok_or_else(|| {
                    format!(
                        "missing native layout for inline struct `{}`",
                        self.display_ty(ty)
                    )
                })?;
                for (name, field) in &layout.fields {
                    let field_offset = offset + field.offset as i32;
                    if field.ty.is_runtime_managed() {
                        // Managed fields cannot appear inside inline structs
                        // (the checker rejects them); defensively error.
                        return Err(format!(
                            "inline struct field `{name}` of `{}` is managed",
                            self.display_ty(ty)
                        ));
                    }
                    if matches!(field.ty, Ty::Named(..)) || matches!(field.ty, Ty::Array(..)) {
                        let sub_source = self.builder.ins().iadd_imm(source, field.offset as i64);
                        self.emit_copy_inline_into(
                            sub_source,
                            &field.ty,
                            destination,
                            field_offset,
                        )?;
                    } else {
                        let Some(cl) = cl_type(&field.ty, self.ptr_ty) else {
                            return Err(format!(
                                "missing Cranelift type for inline field `{name}`"
                            ));
                        };
                        let value = self.builder.ins().load(
                            cl,
                            MemFlags::new(),
                            source,
                            field.offset as i32,
                        );
                        self.builder
                            .ins()
                            .store(MemFlags::new(), value, destination, field_offset);
                    }
                }
                Ok(())
            }
            Ty::Array(elem_ty, size) => {
                let Ty::ConstInt(_, len) = &**size else {
                    return Err("array size must be constant for inline copy".to_string());
                };
                if *len <= 0 {
                    return Ok(());
                }
                let (elem_size, _) = self.field_size_align_for_layout(elem_ty);
                let elem_inline = matches!(elem_ty.as_ref(), Ty::Named(..))
                    && self.ty_is_inline_for_layout(elem_ty);
                if elem_inline {
                    for i in 0..*len {
                        let byte = (i as u32).saturating_mul(elem_size) as i32;
                        let sub_source = self.builder.ins().iadd_imm(source, byte as i64);
                        self.emit_copy_inline_into(
                            sub_source,
                            elem_ty,
                            destination,
                            offset + byte,
                        )?;
                    }
                    Ok(())
                } else {
                    // Scalar elements: copy each by value.
                    let Some(cl) = cl_type(elem_ty, self.ptr_ty) else {
                        return Err(format!(
                            "missing Cranelift type for inline array element `{}`",
                            self.display_ty(elem_ty)
                        ));
                    };
                    for i in 0..*len {
                        let byte = (i as u32).saturating_mul(elem_size) as i32;
                        let value = self.builder.ins().load(cl, MemFlags::new(), source, byte);
                        self.builder.ins().store(
                            MemFlags::new(),
                            value,
                            destination,
                            offset + byte,
                        );
                    }
                    Ok(())
                }
            }
            _ => Err(format!(
                "internal: inline copy of non-aggregate `{}`",
                self.display_ty(ty)
            )),
        }
    }

    fn push_scope(&mut self) {
        self.vars.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.vars.pop();
        if self.vars.is_empty() {
            self.vars.push(HashMap::new());
        }
    }

    fn lookup_var(&self, name: &str) -> Option<(Variable, Ty)> {
        for scope in self.vars.iter().rev() {
            if let Some(entry) = scope.get(name) {
                return Some(entry.clone());
            }
        }
        None
    }

    fn insert_var(&mut self, name: SmolStr, entry: (Variable, Ty)) {
        if let Some(scope) = self.vars.last_mut() {
            scope.insert(name, entry);
        }
    }

    fn struct_supports_equality(&self, def_id: ori_types::DefId) -> bool {
        self.type_supports_equality(&Ty::Named(def_id, Vec::new()), &mut Vec::new())
    }

    fn enum_supports_equality(&self, def_id: ori_types::DefId) -> bool {
        self.type_supports_equality(&Ty::Named(def_id, Vec::new()), &mut Vec::new())
    }

    fn type_supports_equality(&self, ty: &Ty, visiting: &mut Vec<ori_types::DefId>) -> bool {
        match ty {
            Ty::Void
            | Ty::Never
            | Ty::Bool
            | Ty::Int
            | Ty::Float
            | Ty::Float32
            | Ty::Float64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::String
            | Ty::Bytes
            | Ty::Any(_) => true,
            Ty::Optional(inner) => self.type_supports_equality(inner, visiting),
            Ty::Result(ok, err) => {
                self.type_supports_equality(ok, visiting)
                    && self.type_supports_equality(err, visiting)
            }
            Ty::Tuple(elements) => elements
                .iter()
                .all(|e| self.type_supports_equality(e, visiting)),
            Ty::List(inner) | Ty::Set(inner) => self.type_supports_equality(inner, visiting),
            Ty::Map(k, v) => {
                self.type_supports_equality(k, visiting) && self.type_supports_equality(v, visiting)
            }
            Ty::Named(def_id, args) => {
                if visiting.contains(def_id) {
                    // Recursive aggregates cannot be expanded into an
                    // inline equality helper: doing so would recurse during
                    // code generation (for example `json.Value` contains
                    // `list[json.Value]`). A future cycle-aware helper can
                    // handle these by calling through an existing callback;
                    // for now the generated helper reports unsupported.
                    return false;
                }
                visiting.push(*def_id);
                let ok = if let Some(layout) = self.struct_layouts.get(def_id) {
                    layout.fields.iter().all(|(_, field)| {
                        let concrete = substitute_ty_params(&field.ty, args);
                        self.type_supports_equality(&concrete, visiting)
                    })
                } else if let Some(layout) = self.enum_layouts.get(def_id) {
                    layout.variants.values().all(|variant| {
                        variant.fields.fields.iter().all(|(_, field)| {
                            let concrete = substitute_ty_params(&field.ty, args);
                            self.type_supports_equality(&concrete, visiting)
                        })
                    })
                } else {
                    false
                };
                visiting.pop();
                ok
            }
            Ty::Opaque { kind, args } if kind.is_list_backed_collection() => {
                if let Some(elem_ty) = args.first() {
                    self.type_supports_equality(elem_ty, visiting)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn emit_opaque_collection_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        kind: OpaqueTy,
        elem_ty: &Ty,
        eq: bool,
    ) -> Result<ir::Value, String> {
        let to_list_fn_name = match kind {
            OpaqueTy::Deque => "ori_deque_to_list",
            OpaqueTy::Queue => "ori_queue_to_list",
            OpaqueTy::Stack => "ori_stack_to_list",
            OpaqueTy::LinkedList => "ori_linked_list_to_list",
            OpaqueTy::DoublyLinkedList => "ori_doubly_linked_list_to_list",
            _ => {
                return Err(format!(
                    "unsupported opaque collection for equality: {:?}",
                    kind
                ))
            }
        };
        let to_list_ref = *self
            .func_refs
            .get(to_list_fn_name)
            .ok_or_else(|| format!("missing runtime function `{}`", to_list_fn_name))?;

        // Convert both collections to lists
        let left_list_call = self.builder.ins().call(to_list_ref, &[lv]);
        let left_list = self.builder.inst_results(left_list_call)[0];
        let right_list_call = self.builder.ins().call(to_list_ref, &[rv]);
        let right_list = self.builder.inst_results(right_list_call)[0];

        // Compare the lists using emit_list_equality
        let result = self.emit_list_equality(left_list, right_list, elem_ty, eq)?;

        // Release the temporary lists
        let list_ty = Ty::List(Box::new(elem_ty.clone()));
        self.emit_arc_release_if_managed(&list_ty, left_list)?;
        self.emit_arc_release_if_managed(&list_ty, right_list)?;

        Ok(result)
    }

    fn store_async_local_if_any(&mut self, name: &str, val: ir::Value) -> Result<(), String> {
        if let Some(frame) = self.async_frame {
            let plan = self
                .async_plan
                .expect("must have async plan if frame is set");
            if let Some(local_index) = plan.locals.iter().position(|l| l.name == name) {
                let offset = simple_async_frame_local_offset(plan, local_index, self.ptr_ty)
                    .expect("async frame local offset");
                let ty = &plan.locals[local_index].ty;
                if is_managed_ty(ty) {
                    let old =
                        self.builder
                            .ins()
                            .load(self.ptr_ty, MemFlags::new(), frame, offset as i32);
                    self.emit_arc_update_edge_if_managed(ty, frame, old, val)?;
                }
                self.builder
                    .ins()
                    .store(MemFlags::new(), val, frame, offset as i32);
            } else if let Some(await_index) = plan
                .awaits
                .iter()
                .position(|a| a.binding.as_ref().is_some_and(|b| b.name == name))
            {
                let offset = simple_async_frame_binding_offset(plan, await_index, self.ptr_ty)
                    .expect("async frame binding offset");
                let ty = &plan.awaits[await_index]
                    .binding
                    .as_ref()
                    .expect("binding checked above")
                    .ty;
                if is_managed_ty(ty) {
                    let old =
                        self.builder
                            .ins()
                            .load(self.ptr_ty, MemFlags::new(), frame, offset as i32);
                    self.emit_arc_update_edge_if_managed(ty, frame, old, val)?;
                }
                self.builder
                    .ins()
                    .store(MemFlags::new(), val, frame, offset as i32);
            } else if let Some(param_index) = plan.params.iter().position(|p| p.name == name) {
                let offset = simple_async_frame_param_offset(plan, param_index, self.ptr_ty)
                    .expect("async frame param offset");
                let ty = &plan.params[param_index].ty;
                if is_managed_ty(ty) {
                    let old =
                        self.builder
                            .ins()
                            .load(self.ptr_ty, MemFlags::new(), frame, offset as i32);
                    self.emit_arc_update_edge_if_managed(ty, frame, old, val)?;
                }
                self.builder
                    .ins()
                    .store(MemFlags::new(), val, frame, offset as i32);
            }
        }
        Ok(())
    }

    fn reload_async_frame_vars(
        &mut self,
        plan: &SimpleAsyncStateMachinePlan,
        frame: ir::Value,
        initialized_bindings: usize,
    ) -> Result<(), String> {
        for (index, param) in plan.params.iter().enumerate() {
            let cl_ty = cl_type(&param.ty, self.ptr_ty)
                .ok_or_else(|| format!("async param `{}` has no native value", param.name))?;
            let offset = simple_async_frame_param_offset(plan, index, self.ptr_ty)
                .expect("async frame param offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self
                .lookup_var(&param.name)
                .map(|(var, _)| var)
                .unwrap_or_else(|| {
                    let var = self.builder.declare_var(cl_ty);
                    self.insert_var(param.name.clone(), (var, param.ty.clone()));
                    var
                });
            self.builder.def_var(var, value);
        }

        for (index, local) in plan.locals.iter().enumerate() {
            let cl_ty = cl_type(&local.ty, self.ptr_ty)
                .ok_or_else(|| format!("async local `{}` has no native value", local.name))?;
            let offset = simple_async_frame_local_offset(plan, index, self.ptr_ty)
                .expect("async frame local offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self
                .lookup_var(&local.name)
                .map(|(var, _)| var)
                .unwrap_or_else(|| {
                    let var = self.builder.declare_var(cl_ty);
                    self.insert_var(local.name.clone(), (var, local.ty.clone()));
                    var
                });
            self.builder.def_var(var, value);
        }

        for (index, step) in plan.awaits.iter().take(initialized_bindings).enumerate() {
            let Some(binding) = &step.binding else {
                continue;
            };
            let cl_ty = cl_type(&binding.ty, self.ptr_ty)
                .ok_or_else(|| format!("async binding `{}` has no native value", binding.name))?;
            let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                .expect("async frame binding offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self
                .lookup_var(&binding.name)
                .map(|(var, _)| var)
                .unwrap_or_else(|| {
                    let var = self.builder.declare_var(cl_ty);
                    self.insert_var(binding.name.clone(), (var, binding.ty.clone()));
                    var
                });
            self.builder.def_var(var, value);
        }

        Ok(())
    }

    fn emit_user_func(self, f: &HirFunc) -> Result<(), String> {
        if f.is_async {
            self.emit_async_wrapper(f)
        } else {
            self.emit(f)
        }
    }

    fn emit_async_wrapper(self, f: &HirFunc) -> Result<(), String> {
        if let Some(plan) = simple_async_state_machine_plan(f) {
            return self.emit_simple_async_state_machine_wrapper(f, &plan);
        }
        if let Some(plan) = collect_general_async_plan(f) {
            return self.emit_simple_async_state_machine_wrapper(f, &plan);
        }
        if !block_contains_await(&f.body) {
            return self.emit(f);
        }
        Err(native_codegen_unsupported(format!(
            "async function `{}` contains an `await` shape not yet covered by the native state machine; supported forms include top-level and nested control-flow bodies (`if`, `match`, `while`, `for`), `const x: T = await value`, `return await value`, expression-level awaits in calls/operators/conditions, and `const x = (await value)?`; when both the simple and general state-machine plans fail, codegen rejects the function",
            f.name
        )))
    }

    fn emit_simple_async_state_machine_wrapper(
        mut self,
        f: &HirFunc,
        plan: &SimpleAsyncStateMachinePlan,
    ) -> Result<(), String> {
        let entry = self.builder.create_block();
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        self.builder.seal_block(entry);

        self.emit_param_contracts(&f.params, f.name.as_str())?;

        let pending_ref = *self
            .func_refs
            .get("ori_future_pending")
            .ok_or_else(|| "missing runtime function `ori_future_pending`".to_string())?;
        let pending_call = self.builder.ins().call(pending_ref, &[]);
        let future = self.builder.inst_results(pending_call)[0];

        let frame = self.malloc_bytes(simple_async_frame_size(plan, self.ptr_ty))?;
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), zero, frame, ASYNC_FRAME_STATE_OFFSET);
        self.builder
            .ins()
            .store(MemFlags::new(), future, frame, ASYNC_FRAME_RESULT_OFFSET);
        self.emit_arc_register_edge(frame, future)?;
        for (index, param) in plan.params.iter().enumerate() {
            let cl_ty = cl_type(&param.ty, self.ptr_ty)
                .ok_or_else(|| format!("async param `{}` has no native value", param.name))?;
            let value = self.builder.block_params(entry)[index];
            debug_assert_eq!(self.builder.func.dfg.value_type(value), cl_ty);
            let offset = simple_async_frame_param_offset(plan, index, self.ptr_ty)
                .expect("async frame param offset");
            self.builder
                .ins()
                .store(MemFlags::new(), value, frame, offset as i32);
            self.emit_arc_register_edge_if_managed(&param.ty, frame, value)?;
        }
        for (index, local) in plan.locals.iter().enumerate() {
            let value = self.zero_val(&local.ty);
            let offset = simple_async_frame_local_offset(plan, index, self.ptr_ty)
                .expect("async frame local offset");
            self.builder
                .ins()
                .store(MemFlags::new(), value, frame, offset as i32);
        }
        // Await bindings are written only after their future becomes ready.
        // Initialize managed slots before scheduling so failure, cancellation,
        // or an invalid state cannot unregister an uninitialized pointer when
        // terminal cleanup walks the full plan.
        for (index, step) in plan.awaits.iter().enumerate() {
            let Some(binding) = &step.binding else {
                continue;
            };
            if !is_managed_ty(&binding.ty) {
                continue;
            }
            let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                .expect("async frame binding offset");
            let zero_ptr = self.builder.ins().iconst(self.ptr_ty, 0);
            self.builder
                .ins()
                .store(MemFlags::new(), zero_ptr, frame, offset as i32);
        }

        let step_name = async_step_name(f);
        let step_ref = *self
            .func_refs
            .get(step_name.as_str())
            .ok_or_else(|| format!("missing async step function `{step_name}`"))?;
        let step_closure = self.emit_closure_object(step_ref, Some(frame))?;
        let schedule_ref = *self
            .func_refs
            .get("ori_executor_schedule")
            .ok_or_else(|| "missing runtime function `ori_executor_schedule`".to_string())?;
        self.builder.ins().call(schedule_ref, &[step_closure]);
        self.emit_arc_release_if_managed(&Ty::Bytes, frame)?;

        self.emit_arc_retain_if_managed(&f.return_ty, future)?;
        self.emit_scope_cleanup_calls_from(0, 0)?;
        self.builder.ins().return_(&[future]);
        self.terminated = true;

        self.builder.seal_all_blocks();
        self.builder.finalize();
        Ok(())
    }

    fn emit(mut self, f: &HirFunc) -> Result<(), String> {
        let entry = self.builder.create_block();
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        self.builder.seal_block(entry);
        self.emit_debug_enter();

        // Bind parameters
        let params: Vec<(SmolStr, Ty, ir::Value)> = f
            .params
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                cl_type(&p.ty, self.ptr_ty).map(|_| {
                    let v = self.builder.block_params(entry)[i];
                    (p.name.clone(), p.ty.clone(), v)
                })
            })
            .collect();
        for (name, ty, val) in params {
            if let Some(cl_ty) = cl_type(&ty, self.ptr_ty) {
                let var = self.builder.declare_var(cl_ty);
                self.builder.def_var(var, val);
                self.insert_var(name.clone(), (var, ty.clone()));
                if name.as_str() != "__env" && is_managed_ty(&ty) {
                    self.managed_stack.push(ManagedCleanup { var, ty });
                }
            }
        }

        self.emit_closure_capture_prologue(f)?;

        self.emit_param_contracts(&f.params, f.name.as_str())?;
        self.emit_block(&f.body)?;

        if !self.terminated {
            if let Ty::Future(inner) = &f.return_ty {
                let future = self.emit_future_ready(inner, None)?;
                self.emit_debug_leave();
                self.builder.ins().return_(&[future]);
            } else {
                // Implicit fall-through: run scope cleanup (using + managed
                // values) before returning, mirroring the explicit `return`
                // statement path. Without this, any managed value still live
                // at the end of the function would be leaked.
                self.emit_scope_cleanup_calls_from(0, 0)?;
                if cl_type(&f.return_ty, self.ptr_ty).is_none() {
                    self.emit_debug_leave();
                    self.builder.ins().return_(&[]);
                } else {
                    let zero = self.zero_val(&f.return_ty);
                    self.emit_debug_leave();
                    self.builder.ins().return_(&[zero]);
                }
            }
        }

        self.builder.seal_all_blocks();
        self.builder.finalize();
        Ok(())
    }

    fn emit_param_contracts(&mut self, params: &[HirParam], func_name: &str) -> Result<(), String> {
        for param in params {
            let Some(contract) = &param.contract else {
                continue;
            };
            let Some((var, ty)) = self.lookup_var(&param.name) else {
                continue;
            };
            let value = self.builder.use_var(var);
            let message = format!(
                "{CONTRACT_PARAM_VIOLATION}: parameter `{}` of `{func_name}` broke its `if` contract",
                param.name
            );
            self.emit_value_contract(&ty, value, contract, 2, false, Some(&message))?;
        }
        Ok(())
    }

    fn emit_closure_capture_prologue(&mut self, f: &HirFunc) -> Result<(), String> {
        if f.closure_captures.is_empty() {
            return Ok(());
        }
        let Some((env_var, _)) = self.lookup_var("__env") else {
            return Err(format!(
                "closure `{}` has captures but no environment parameter",
                f.name
            ));
        };
        let env = self.builder.use_var(env_var);
        let (offsets, _) = closure_env_layout(&f.closure_captures, self.ptr_ty);
        for (capture, offset) in f.closure_captures.iter().zip(offsets) {
            let Some(cl_ty) = cl_type(&capture.ty, self.ptr_ty) else {
                continue;
            };
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), env, offset as i32);
            let var = self.builder.declare_var(cl_ty);
            self.builder.def_var(var, value);
            self.insert_var(capture.name.clone(), (var, capture.ty.clone()));
            if is_managed_ty(&capture.ty) {
                self.emit_arc_retain_if_managed(&capture.ty, value)?;
                self.managed_stack.push(ManagedCleanup {
                    var,
                    ty: capture.ty.clone(),
                });
            }
        }
        Ok(())
    }

    fn emit_value_contract(
        &mut self,
        ty: &Ty,
        value: ir::Value,
        contract: &HirExpr,
        trap_code: u8,
        run_cleanup: bool,
        message: Option<&str>,
    ) -> Result<(), String> {
        let Some(cl_ty) = cl_type(ty, self.ptr_ty) else {
            return Ok(());
        };
        let it_var = self.builder.declare_var(cl_ty);
        self.builder.def_var(it_var, value);
        let it_name = SmolStr::new("it");
        self.push_scope();
        self.insert_var(it_name, (it_var, ty.clone()));
        let condition = self.emit_expr(contract);
        self.pop_scope();
        let condition = condition?;
        let trap = ir::TrapCode::user(trap_code)
            .ok_or_else(|| format!("invalid runtime contract trap code `{trap_code}`"))?;
        self.emit_trap_unless_with_message(condition, trap, run_cleanup, message)
    }

    fn emit_trap_unless(
        &mut self,
        condition: ir::Value,
        trap_code: ir::TrapCode,
        run_cleanup: bool,
    ) -> Result<(), String> {
        self.emit_trap_unless_with_message(condition, trap_code, run_cleanup, None)
    }

    /// Branch to a failure block when `condition` is false.
    ///
    /// With a `message`, the failure block calls `ori_panic` first so the user
    /// sees what broke. A bare `trap` aborts with a signal and prints nothing,
    /// which is useless for a violated value contract.
    fn emit_trap_unless_with_message(
        &mut self,
        condition: ir::Value,
        trap_code: ir::TrapCode,
        run_cleanup: bool,
        message: Option<&str>,
    ) -> Result<(), String> {
        let ok_blk = self.builder.create_block();
        let fail_blk = self.builder.create_block();
        self.builder
            .ins()
            .brif(condition, ok_blk, &[], fail_blk, &[]);
        self.builder.seal_block(fail_blk);
        self.builder.switch_to_block(fail_blk);
        if run_cleanup {
            if self.hosted_traps {
                self.emit_host_trap_return(i32::from(trap_code.as_raw().get()), message, true)?;
            } else {
                self.emit_scope_cleanup_calls_from(0, 0)?;
            }
        } else if self.hosted_traps {
            self.emit_host_trap_return(i32::from(trap_code.as_raw().get()), message, false)?;
        }
        if !self.hosted_traps {
            if let Some(message) = message {
                if let Some(fref) = self.func_refs.get("ori_panic").copied() {
                    let text = self.bytes_ptr(message.as_bytes())?;
                    self.builder.ins().call(fref, &[text]);
                }
            }
            self.builder.ins().trap(trap_code);
        }
        self.builder.seal_block(ok_blk);
        self.builder.switch_to_block(ok_blk);
        self.terminated = false;
        Ok(())
    }

    /// Report a controlled failure to the hosted runtime and return the
    /// function's zero value. Standalone code never enters this helper.
    fn emit_host_trap_return(
        &mut self,
        code: i32,
        message: Option<&str>,
        run_cleanup: bool,
    ) -> Result<(), String> {
        debug_assert!(self.hosted_traps);
        let text = self.bytes_ptr(message.unwrap_or("").as_bytes())?;
        self.emit_host_trap_return_value(code, text, true, run_cleanup)
    }

    fn emit_host_error(&mut self, code: i32, message: &str) -> Result<(), String> {
        debug_assert!(self.hosted_traps);
        let text = self.bytes_ptr(message.as_bytes())?;
        self.emit_host_error_value(code, text, true)
    }

    fn emit_host_error_value(
        &mut self,
        code: i32,
        text: ir::Value,
        release_message: bool,
    ) -> Result<(), String> {
        let report_ref = *self
            .func_refs
            .get("ori_host_report_error")
            .ok_or_else(|| "missing runtime function `ori_host_report_error`".to_string())?;
        let code_value = self.builder.ins().iconst(types::I32, i64::from(code));
        self.builder.ins().call(report_ref, &[code_value, text]);
        if release_message {
            self.emit_arc_release_if_managed(&Ty::Bytes, text)?;
        }
        Ok(())
    }

    fn emit_host_trap_return_value(
        &mut self,
        code: i32,
        text: ir::Value,
        release_message: bool,
        run_cleanup: bool,
    ) -> Result<(), String> {
        self.emit_host_error_value(code, text, release_message)?;
        if run_cleanup {
            self.emit_scope_cleanup_calls_from(0, 0)?;
        }
        self.emit_debug_leave();
        let return_ty = self.current_return_ty.clone();
        if cl_type(&return_ty, self.ptr_ty).is_some() {
            let zero = self.zero_val(&return_ty);
            self.builder.ins().return_(&[zero]);
        } else {
            self.builder.ins().return_(&[]);
        }
        self.terminated = true;
        Ok(())
    }

    fn emit_expr_for_expected(
        &mut self,
        expr: &HirExpr,
        expected: &Ty,
    ) -> Result<ir::Value, String> {
        if matches!(expected, Ty::Func { .. }) {
            if let HirExprKind::Var(name) = &expr.kind {
                if self.lookup_var(name).is_none()
                    && self.global_data.get(name).is_none()
                    && self.const_exprs.get(name).is_none()
                    && self
                        .func_refs
                        .contains_key(format!("{name}.__fnptr_wrapper").as_str())
                {
                    return self.emit_named_function_closure(name);
                }
            }
        }
        let value = self.emit_expr(expr)?;
        if let (Ty::Any(trait_def_id), Ty::Named(type_def_id, _)) = (expected, &expr.ty) {
            return self.emit_any_box(*trait_def_id, *type_def_id, value);
        }

        // Numeric widening casts
        let actual_ty = self.builder.func.dfg.value_type(value);
        let expected_ty = cl_type(expected, self.ptr_ty).unwrap_or(self.ptr_ty);
        if actual_ty != expected_ty {
            if actual_ty == types::F32 && expected_ty == types::F64 {
                return Ok(self.builder.ins().fpromote(expected_ty, value));
            }
            if actual_ty == types::F64 && expected_ty == types::F32 {
                return Ok(self.builder.ins().fdemote(expected_ty, value));
            }

            let actual_size = match actual_ty {
                types::I8 => 1,
                types::I16 => 2,
                types::I32 => 4,
                types::I64 => 8,
                _ => 0,
            };
            let expected_size = match expected_ty {
                types::I8 => 1,
                types::I16 => 2,
                types::I32 => 4,
                types::I64 => 8,
                _ => 0,
            };
            if actual_size > 0 && expected_size > actual_size {
                // Determine if we should sign-extend or zero-extend
                let is_unsigned = matches!(expr.ty, Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64);
                if is_unsigned {
                    return Ok(self.builder.ins().uextend(expected_ty, value));
                } else {
                    return Ok(self.builder.ins().sextend(expected_ty, value));
                }
            }
        }

        Ok(value)
    }

    fn emit_named_function_closure(&mut self, name: &SmolStr) -> Result<ir::Value, String> {
        let wrapper_name = SmolStr::new(format!("{name}.__fnptr_wrapper"));
        let fref = *self.func_refs.get(wrapper_name.as_str()).ok_or_else(|| {
            format!("missing closure wrapper reference `{wrapper_name}` in native codegen")
        })?;
        self.emit_closure_object(fref, None)
    }

    fn emit_closure_object(
        &mut self,
        fref: ir::FuncRef,
        env_ptr: Option<ir::Value>,
    ) -> Result<ir::Value, String> {
        let ptr_size = self.ptr_ty.bytes() as i64;
        let object = self.malloc_bytes((ptr_size * 2) as u32)?;
        let fn_ptr = self.builder.ins().func_addr(self.ptr_ty, fref);
        let env = env_ptr.unwrap_or_else(|| self.builder.ins().iconst(self.ptr_ty, 0));
        self.builder.ins().store(MemFlags::new(), fn_ptr, object, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), env, object, ptr_size as i32);
        if env_ptr.is_some() {
            self.emit_arc_register_edge(object, env)?;
        }
        Ok(object)
    }

    fn emit_any_box(
        &mut self,
        trait_def_id: ori_types::DefId,
        type_def_id: ori_types::DefId,
        data_ptr: ir::Value,
    ) -> Result<ir::Value, String> {
        let trait_layout = self
            .trait_layouts
            .get(&trait_def_id)
            .ok_or_else(|| format!("missing trait layout for `{trait_def_id}`"))?
            .clone();
        let impl_sig = self
            .trait_impls
            .get(&(trait_def_id, type_def_id))
            .ok_or_else(|| {
                format!("missing implementation of `{trait_def_id}` for `{type_def_id}`")
            })?
            .clone();

        let ptr_size = self.ptr_ty.bytes() as i64;
        let vtable_size = (trait_layout.methods.len() as i64 + 2) * ptr_size;
        let vtable = self.malloc_bytes(vtable_size as u32)?;

        let type_id_val = self.builder.ins().iconst(self.ptr_ty, type_def_id.0 as i64);
        self.builder
            .ins()
            .store(MemFlags::new(), type_id_val, vtable, 0);

        let struct_eq_helper_name = format!("__eq_helper_struct_{}", type_def_id.0);
        let enum_eq_helper_name = format!("__eq_helper_enum_{}", type_def_id.0);
        let eq_fn_val = if let Some(&fref) = self
            .func_refs
            .get(struct_eq_helper_name.as_str())
            .or_else(|| self.func_refs.get(enum_eq_helper_name.as_str()))
        {
            self.builder.ins().func_addr(self.ptr_ty, fref)
        } else {
            self.builder.ins().iconst(self.ptr_ty, 0)
        };
        self.builder
            .ins()
            .store(MemFlags::new(), eq_fn_val, vtable, ptr_size as i32);

        for (index, method) in trait_layout.methods.iter().enumerate() {
            let func_name = impl_sig
                .methods
                .iter()
                .find(|m| m.name == method.name)
                .map(|method| method.func_name.clone())
                .or_else(|| method.default_func_name.clone())
                .ok_or_else(|| format!("trait method `{}` has no implementation", method.name))?;
            let fref = *self.func_refs.get(func_name.as_str()).ok_or_else(|| {
                format!(
                    "missing function reference `{}` for trait dispatch",
                    func_name
                )
            })?;
            let fn_ptr = self.builder.ins().func_addr(self.ptr_ty, fref);
            self.builder.ins().store(
                MemFlags::new(),
                fn_ptr,
                vtable,
                ((index as i64 + 2) * ptr_size) as i32,
            );
        }

        let object = self.malloc_bytes((ptr_size * 2) as u32)?;
        self.builder
            .ins()
            .store(MemFlags::new(), data_ptr, object, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), vtable, object, ptr_size as i32);
        self.emit_arc_register_edge(object, data_ptr)?;
        Ok(object)
    }

    fn emit_dynamic_method_call(
        &mut self,
        receiver: &HirExpr,
        method_name: &SmolStr,
        args: &[HirExpr],
    ) -> Result<ir::Value, String> {
        let Ty::Any(trait_def_id) = &receiver.ty else {
            return Err(format!(
                "dynamic method call requires `any[Trait]`, got `{}`",
                self.display_ty(&receiver.ty)
            ));
        };
        let trait_def_id = *trait_def_id;
        let trait_layout = self
            .trait_layouts
            .get(&trait_def_id)
            .ok_or_else(|| format!("missing trait layout for `{trait_def_id}`"))?;
        let Some((method_index, method)) = trait_layout
            .methods
            .iter()
            .enumerate()
            .find(|(_, method)| method.name == *method_name)
        else {
            return Err(format!(
                "trait `{}` has no method `{}`",
                trait_layout.name, method_name
            ));
        };

        let any_ptr = self.emit_expr(receiver)?;
        let data_ptr = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), any_ptr, 0);
        let ptr_size = self.ptr_ty.bytes() as i64;
        let vtable =
            self.builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), any_ptr, ptr_size as i32);
        let fn_ptr = self.builder.ins().load(
            self.ptr_ty,
            MemFlags::new(),
            vtable,
            ((method_index as i64 + 2) * ptr_size) as i32,
        );

        let mut sig = ir::Signature::new(self.builder.func.signature.call_conv);
        sig.params.push(AbiParam::new(self.ptr_ty));
        for param_ty in method.params.iter().skip(1) {
            if let Some(cl_ty) = cl_type(param_ty, self.ptr_ty) {
                sig.params.push(AbiParam::new(cl_ty));
            }
        }
        if let Some(ret_ty) = cl_type(&method.return_ty, self.ptr_ty) {
            sig.returns.push(AbiParam::new(ret_ty));
        }
        let sig_ref = self.builder.func.import_signature(sig);

        let mut call_args = vec![data_ptr];
        // The trait method expects `self` to be retained by the caller
        if let Some(&retain_ref) = self.func_refs.get("ori_arc_retain") {
            self.builder.ins().call(retain_ref, &[data_ptr]);
        }
        for (arg, expected) in args.iter().zip(method.params.iter().skip(1)) {
            let value = self.emit_expr_for_expected(arg, expected)?;
            self.emit_arc_retain_if_managed(expected, value)?;
            call_args.push(value);
        }
        let call = self
            .builder
            .ins()
            .call_indirect(sig_ref, fn_ptr, &call_args);
        let res = self.builder.inst_results(call);
        if res.is_empty() {
            Ok(self.builder.ins().iconst(types::I8, 0))
        } else {
            Ok(res[0])
        }
    }

    fn emit_closure_value(
        &mut self,
        func_name: &SmolStr,
        captures: &[HirClosureCapture],
    ) -> Result<ir::Value, String> {
        let fref = *self.func_refs.get(func_name.as_str()).ok_or_else(|| {
            format!("missing closure function reference `{func_name}` in native codegen")
        })?;
        let env_ptr = if captures.is_empty() {
            None
        } else {
            let (offsets, total) = closure_env_layout(captures, self.ptr_ty);
            let env = self.malloc_bytes(total)?;
            for (capture, offset) in captures.iter().zip(offsets) {
                let value = if let Some((var, _)) = self.lookup_var(&capture.name) {
                    self.builder.use_var(var)
                } else if let Some(value) = self.load_global(&capture.name) {
                    value
                } else if let Some(expr) = self.const_exprs.get(&capture.name).cloned() {
                    self.emit_expr(&expr)?
                } else {
                    return Err(format!(
                        "closure capture `{}` is not available in native codegen",
                        capture.name
                    ));
                };
                if cl_type(&capture.ty, self.ptr_ty).is_some() {
                    self.builder
                        .ins()
                        .store(MemFlags::new(), value, env, offset as i32);
                    self.emit_arc_register_edge_if_managed(&capture.ty, env, value)?;
                }
            }
            Some(env)
        };
        self.emit_closure_object(fref, env_ptr)
    }

    fn emit_closure_call(
        &mut self,
        callee: &HirExpr,
        args: &[HirArg],
    ) -> Result<ir::Value, String> {
        let Ty::Func { params, ret } = &callee.ty else {
            return Err(format!(
                "closure call requires a function value, got `{}`",
                self.display_ty(&callee.ty)
            ));
        };
        let params = params.clone();
        let ret = *ret.clone();
        let closure = self.emit_expr(callee)?;
        let ptr_size = self.ptr_ty.bytes() as i64;
        let fn_ptr = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), closure, 0);
        let env_ptr =
            self.builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), closure, ptr_size as i32);

        let mut sig = ir::Signature::new(self.builder.func.signature.call_conv);
        sig.params.push(AbiParam::new(self.ptr_ty));
        for param_ty in &params {
            if let Some(cl_ty) = cl_type(param_ty, self.ptr_ty) {
                sig.params.push(AbiParam::new(cl_ty));
            }
        }
        if let Some(ret_ty) = cl_type(&ret, self.ptr_ty) {
            sig.returns.push(AbiParam::new(ret_ty));
        }
        let sig_ref = self.builder.func.import_signature(sig);

        let mut call_args = vec![env_ptr];
        for (arg, expected) in args.iter().zip(params.iter()) {
            let value = self.emit_expr_for_expected(&arg.value, expected)?;
            self.emit_arc_retain_if_managed(expected, value)?;
            call_args.push(value);
        }
        let call = self
            .builder
            .ins()
            .call_indirect(sig_ref, fn_ptr, &call_args);
        let res = self.builder.inst_results(call);
        if res.is_empty() {
            Ok(self.builder.ins().iconst(types::I8, 0))
        } else {
            Ok(res[0])
        }
    }

    fn emit_lazy_once(&mut self, thunk: &HirExpr, lazy_ty: &Ty) -> Result<ir::Value, String> {
        let Ty::Lazy(inner) = lazy_ty else {
            return Err(format!(
                "lazy.once expected lazy result type, got `{}`",
                self.display_ty(lazy_ty)
            ));
        };
        let thunk_value = self.emit_expr(thunk)?;
        let ptr_size = self.ptr_ty.bytes() as i32;
        let (_, total) = lazy_layout(inner, self.ptr_ty);
        let object = self.malloc_bytes(total)?;
        let zero8 = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), thunk_value, object, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), zero8, object, ptr_size);
        self.emit_arc_register_edge(object, thunk_value)?;
        self.emit_arc_release_if_managed(&thunk.ty, thunk_value)?;
        Ok(object)
    }

    fn emit_lazy_is_consumed(&mut self, value: &HirExpr) -> Result<ir::Value, String> {
        let Ty::Lazy(_inner) = &value.ty else {
            return Err(format!(
                "lazy.is_consumed expected lazy value, got `{}`",
                self.display_ty(&value.ty)
            ));
        };
        let lazy_value = self.emit_expr(value)?;
        let ptr_size = self.ptr_ty.bytes() as i32;
        let forced = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), lazy_value, ptr_size);
        Ok(forced)
    }

    fn emit_lazy_force(&mut self, value: &HirExpr, ret_ty: &Ty) -> Result<ir::Value, String> {
        let Ty::Lazy(inner) = &value.ty else {
            return Err(format!(
                "lazy.force expected lazy value, got `{}`",
                self.display_ty(&value.ty)
            ));
        };
        let ret_cl = cl_type(ret_ty, self.ptr_ty).ok_or_else(|| {
            format!(
                "native backend cannot force lazy value returning `{}`",
                self.display_ty(ret_ty)
            )
        })?;
        let lazy_value = self.emit_expr(value)?;
        let ptr_size = self.ptr_ty.bytes() as i32;
        let (value_offset, _) = lazy_layout(inner, self.ptr_ty);
        let forced = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), lazy_value, ptr_size);
        let forced_block = self.builder.create_block();
        let compute_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        let slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            ret_cl.bytes(),
            3,
        ));
        let slot_addr = self.builder.ins().stack_addr(self.ptr_ty, slot, 0);
        self.builder
            .ins()
            .brif(forced, forced_block, &[], compute_block, &[]);

        self.builder.seal_block(forced_block);
        self.builder.switch_to_block(forced_block);
        let cached =
            self.builder
                .ins()
                .load(ret_cl, MemFlags::new(), lazy_value, value_offset as i32);
        self.builder
            .ins()
            .store(MemFlags::new(), cached, slot_addr, 0);
        self.builder.ins().jump(merge_block, &[]);

        self.builder.seal_block(compute_block);
        self.builder.switch_to_block(compute_block);
        let thunk = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), lazy_value, 0);
        let fn_ptr = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), thunk, 0);
        let env_ptr = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), thunk, ptr_size);
        let mut sig = ir::Signature::new(self.builder.func.signature.call_conv);
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.returns.push(AbiParam::new(ret_cl));
        let sig_ref = self.builder.func.import_signature(sig);
        let call = self
            .builder
            .ins()
            .call_indirect(sig_ref, fn_ptr, &[env_ptr]);
        let computed = self.builder.inst_results(call)[0];
        self.builder
            .ins()
            .store(MemFlags::new(), computed, lazy_value, value_offset as i32);
        let one8 = self.builder.ins().iconst(types::I8, 1);
        self.builder
            .ins()
            .store(MemFlags::new(), one8, lazy_value, ptr_size);
        self.emit_arc_register_edge_if_managed(inner, lazy_value, computed)?;
        self.emit_arc_release_if_managed(inner, computed)?;
        self.builder
            .ins()
            .store(MemFlags::new(), computed, slot_addr, 0);
        self.builder.ins().jump(merge_block, &[]);

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        Ok(self
            .builder
            .ins()
            .load(ret_cl, MemFlags::new(), slot_addr, 0))
    }

    fn emit_new_list(&mut self) -> Result<ir::Value, String> {
        let new_ref = *self
            .func_refs
            .get("ori_list_new")
            .ok_or_else(|| "missing runtime function `ori_list_new`".to_string())?;
        let call = self.builder.ins().call(new_ref, &[]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_push_value(
        &mut self,
        list: ir::Value,
        value: ir::Value,
        elem_ty: &Ty,
    ) -> Result<(), String> {
        let push_ref = *self
            .func_refs
            .get("ori_list_push")
            .ok_or_else(|| "missing runtime function `ori_list_push`".to_string())?;
        let stored = self.encode_list_storage_value(value, elem_ty);

        // Fast path for non-managed scalars when capacity remains: store in-place
        // without a runtime call (dominant cost in list_sum / push loops).
        if is_list_inline_scalar_elem(elem_ty) {
            let len =
                self.builder
                    .ins()
                    .load(types::I64, MemFlags::new(), list, ORI_LIST_LEN_OFFSET);
            let cap =
                self.builder
                    .ins()
                    .load(types::I64, MemFlags::new(), list, ORI_LIST_CAP_OFFSET);
            let has_room = self
                .builder
                .ins()
                .icmp(ir::condcodes::IntCC::SignedLessThan, len, cap);

            let fast = self.builder.create_block();
            let slow = self.builder.create_block();
            let join = self.builder.create_block();
            self.builder.ins().brif(has_room, fast, &[], slow, &[]);

            self.builder.seal_block(fast);
            self.builder.switch_to_block(fast);
            let data =
                self.builder
                    .ins()
                    .load(self.ptr_ty, MemFlags::new(), list, ORI_LIST_DATA_OFFSET);
            let scale = self.builder.ins().iconst(types::I64, 8);
            let byte_off = self.builder.ins().imul(len, scale);
            let slot = self.builder.ins().iadd(data, byte_off);
            self.builder.ins().store(MemFlags::new(), stored, slot, 0);
            let one = self.builder.ins().iconst(types::I64, 1);
            let new_len = self.builder.ins().iadd(len, one);
            self.builder
                .ins()
                .store(MemFlags::new(), new_len, list, ORI_LIST_LEN_OFFSET);
            let version =
                self.builder
                    .ins()
                    .load(types::I64, MemFlags::new(), list, ORI_LIST_VERSION_OFFSET);
            let new_version = self.builder.ins().iadd(version, one);
            self.builder
                .ins()
                .store(MemFlags::new(), new_version, list, ORI_LIST_VERSION_OFFSET);
            self.builder.ins().jump(join, &[]);

            self.builder.seal_block(slow);
            self.builder.switch_to_block(slow);
            self.builder.ins().call(push_ref, &[list, stored]);
            self.builder.ins().jump(join, &[]);

            self.builder.seal_block(join);
            self.builder.switch_to_block(join);
            self.terminated = false;
            // Non-managed: no ARC edge.
            return Ok(());
        }

        self.builder.ins().call(push_ref, &[list, stored]);
        self.emit_arc_register_edge_if_managed(elem_ty, list, value)?;
        Ok(())
    }

    fn emit_list_set_value(
        &mut self,
        list: ir::Value,
        index: ir::Value,
        value: ir::Value,
        elem_ty: &Ty,
    ) -> Result<(), String> {
        if is_managed_ty(elem_ty) {
            let get_ref = *self
                .func_refs
                .get("ori_list_get")
                .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;
            let old_call = self.builder.ins().call(get_ref, &[list, index]);
            let old = self.builder.inst_results(old_call)[0];
            let old = self.decode_list_storage_value(old, elem_ty);
            self.emit_arc_update_edge_if_managed(elem_ty, list, old, value)?;
        }

        let set_ref = *self
            .func_refs
            .get("ori_list_set")
            .ok_or_else(|| "missing runtime function `ori_list_set`".to_string())?;
        let stored = self.encode_list_storage_value(value, elem_ty);
        self.builder.ins().call(set_ref, &[list, index, stored]);
        Ok(())
    }

    fn emit_list_insert_value(
        &mut self,
        list: ir::Value,
        index: ir::Value,
        value: ir::Value,
        elem_ty: &Ty,
    ) -> Result<(), String> {
        let insert_ref = *self
            .func_refs
            .get("ori_list_insert")
            .ok_or_else(|| "missing runtime function `ori_list_insert`".to_string())?;
        let stored = self.encode_list_storage_value(value, elem_ty);
        self.builder.ins().call(insert_ref, &[list, index, stored]);
        self.emit_arc_register_edge_if_managed(elem_ty, list, value)?;
        Ok(())
    }

    /// Load `list[index]` — inline bounds-checked load for scalar elements.
    fn emit_list_get_value(
        &mut self,
        list: ir::Value,
        index: ir::Value,
        elem_ty: &Ty,
    ) -> Result<ir::Value, String> {
        let get_ref = *self
            .func_refs
            .get("ori_list_get")
            .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;

        if !is_list_inline_scalar_elem(elem_ty) {
            if self.hosted_traps {
                let len =
                    self.builder
                        .ins()
                        .load(types::I64, MemFlags::new(), list, ORI_LIST_LEN_OFFSET);
                let zero = self.builder.ins().iconst(types::I64, 0);
                let ge_zero = self.builder.ins().icmp(
                    ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    index,
                    zero,
                );
                let lt_len =
                    self.builder
                        .ins()
                        .icmp(ir::condcodes::IntCC::SignedLessThan, index, len);
                let in_bounds = self.builder.ins().band(ge_zero, lt_len);
                let fast = self.builder.create_block();
                let slow = self.builder.create_block();
                let join = self.builder.create_block();
                self.builder.append_block_param(join, types::I64);
                self.builder.ins().brif(in_bounds, fast, &[], slow, &[]);

                self.builder.seal_block(fast);
                self.builder.switch_to_block(fast);
                let call = self.builder.ins().call(get_ref, &[list, index]);
                let stored = self.builder.inst_results(call)[0];
                self.builder.ins().jump(join, &[BlockArg::Value(stored)]);

                self.builder.seal_block(slow);
                self.builder.switch_to_block(slow);
                self.emit_host_error(8, "list index out of bounds")?;
                self.builder.ins().jump(join, &[BlockArg::Value(zero)]);

                self.builder.seal_block(join);
                self.builder.switch_to_block(join);
                self.terminated = false;
                let stored = self.builder.block_params(join)[0];
                return Ok(self.decode_list_storage_value(stored, elem_ty));
            }
            let call = self.builder.ins().call(get_ref, &[list, index]);
            let stored = self.builder.inst_results(call)[0];
            return Ok(self.decode_list_storage_value(stored, elem_ty));
        }

        let len = self
            .builder
            .ins()
            .load(types::I64, MemFlags::new(), list, ORI_LIST_LEN_OFFSET);
        let zero = self.builder.ins().iconst(types::I64, 0);
        let ge_zero =
            self.builder
                .ins()
                .icmp(ir::condcodes::IntCC::SignedGreaterThanOrEqual, index, zero);
        let lt_len = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::SignedLessThan, index, len);
        let in_bounds = self.builder.ins().band(ge_zero, lt_len);

        let fast = self.builder.create_block();
        let slow = self.builder.create_block();
        let join = self.builder.create_block();
        self.builder.append_block_param(join, types::I64);
        self.builder.ins().brif(in_bounds, fast, &[], slow, &[]);

        self.builder.seal_block(fast);
        self.builder.switch_to_block(fast);
        let data =
            self.builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), list, ORI_LIST_DATA_OFFSET);
        let scale = self.builder.ins().iconst(types::I64, 8);
        let byte_off = self.builder.ins().imul(index, scale);
        let slot = self.builder.ins().iadd(data, byte_off);
        let stored = self
            .builder
            .ins()
            .load(types::I64, MemFlags::new(), slot, 0);
        self.builder.ins().jump(join, &[BlockArg::Value(stored)]);

        self.builder.seal_block(slow);
        self.builder.switch_to_block(slow);
        let slow_stored = if self.hosted_traps {
            self.emit_host_error(8, "list index out of bounds")?;
            self.builder.ins().iconst(types::I64, 0)
        } else {
            // Runtime path aborts with the standard bounds diagnostic.
            let call = self.builder.ins().call(get_ref, &[list, index]);
            self.builder.inst_results(call)[0]
        };
        self.builder
            .ins()
            .jump(join, &[BlockArg::Value(slow_stored)]);

        self.builder.seal_block(join);
        self.builder.switch_to_block(join);
        self.terminated = false;
        let stored = self.builder.block_params(join)[0];
        Ok(self.decode_list_storage_value(stored, elem_ty))
    }

    fn emit_list_extend_from(
        &mut self,
        target: ir::Value,
        source: ir::Value,
        elem_ty: &Ty,
    ) -> Result<(), String> {
        let len_ref = *self
            .func_refs
            .get("ori_list_len")
            .ok_or_else(|| "missing runtime function `ori_list_len`".to_string())?;
        let get_ref = *self
            .func_refs
            .get("ori_list_get")
            .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;
        let len_call = self.builder.ins().call(len_ref, &[source]);
        let len_v = self.builder.inst_results(len_call)[0];
        let idx_var = self.builder.declare_var(types::I64);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.def_var(idx_var, zero);
        let len_var = self.builder.declare_var(types::I64);
        self.builder.def_var(len_var, len_v);

        let header = self.builder.create_block();
        let body = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();

        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let cur = self.builder.use_var(idx_var);
        let len = self.builder.use_var(len_var);
        let cond = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::SignedLessThan, cur, len);
        self.builder.ins().brif(cond, body, &[], exit, &[]);

        self.builder.seal_block(body);
        self.builder.switch_to_block(body);
        let cur = self.builder.use_var(idx_var);
        let call = self.builder.ins().call(get_ref, &[source, cur]);
        let stored = self.builder.inst_results(call)[0];
        let value = self.decode_list_storage_value(stored, elem_ty);
        self.emit_list_push_value(target, value, elem_ty)?;
        self.builder.ins().jump(step, &[]);

        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur, one);
        self.builder.def_var(idx_var, next);
        self.builder.ins().jump(header, &[]);

        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        self.terminated = false;
        Ok(())
    }

    fn zero_val(&mut self, ty: &Ty) -> ir::Value {
        match ty {
            Ty::Float | Ty::Float64 => self.builder.ins().f64const(0.0),
            Ty::Float32 => self.builder.ins().f32const(0.0),
            _ => {
                let cl = cl_type(ty, self.ptr_ty).unwrap_or(types::I64);
                self.builder.ins().iconst(cl, 0)
            }
        }
    }

    fn load_global(&mut self, name: &str) -> Option<ir::Value> {
        let gv = *self.global_gvs.get(name)?;
        let info = self.global_data.get(name)?;
        let addr = self.builder.ins().global_value(self.ptr_ty, gv);
        let cl = cl_type(&info.ty, self.ptr_ty)?;
        Some(self.builder.ins().load(cl, MemFlags::new(), addr, 0))
    }

    fn store_global(&mut self, name: &str, value: ir::Value) -> bool {
        self.store_global_inner(name, value, true)
    }

    fn initialize_global(&mut self, name: &str, value: ir::Value) -> bool {
        self.store_global_inner(name, value, false)
    }

    fn store_global_inner(&mut self, name: &str, value: ir::Value, require_mutable: bool) -> bool {
        let Some(gv) = self.global_gvs.get(name).copied() else {
            return false;
        };
        let Some(info) = self.global_data.get(name) else {
            return false;
        };
        if require_mutable && !info.mutable {
            return false;
        }
        let addr = self.builder.ins().global_value(self.ptr_ty, gv);
        self.builder.ins().store(MemFlags::new(), value, addr, 0);
        true
    }

    fn emit_lvalue_value(&mut self, lvalue: &HirLValue) -> Result<(ir::Value, Ty), String> {
        match lvalue {
            HirLValue::Var(name) => {
                if let Some((var, ty)) = self.lookup_var(name) {
                    Ok((self.builder.use_var(var), ty))
                } else if let Some(info) = self.global_data.get(name).cloned() {
                    let value = self.load_global(name).ok_or_else(|| {
                        format!("global `{name}` is not available in native codegen")
                    })?;
                    Ok((value, info.ty))
                } else {
                    Err(format!("undefined lvalue base `{name}` in native codegen"))
                }
            }
            HirLValue::Field { base, field } => {
                let (addr, field_layout, _) = self.emit_field_lvalue_addr(base, field)?;
                // Inline aggregate fields (array or inline struct) are
                // represented by their address; scalars load by value.
                if matches!(field_layout.ty, Ty::Array(_, _))
                    || (matches!(field_layout.ty, Ty::Named(..))
                        && self.ty_is_inline_for_layout(&field_layout.ty))
                {
                    Ok((addr, field_layout.ty))
                } else {
                    let cl_ty = cl_type(&field_layout.ty, self.ptr_ty)
                        .ok_or_else(|| format!("missing Cranelift type for field `{field}`"))?;
                    let value = self.builder.ins().load(cl_ty, MemFlags::new(), addr, 0);
                    Ok((value, field_layout.ty))
                }
            }
            HirLValue::Index { base, index } => {
                // `xs[i].f = v`: the base is an array; return the element
                // address for inline struct elements (scalar elements are
                // loaded by value).
                let (container, container_ty) = self.emit_lvalue_value(base)?;
                match &container_ty {
                    Ty::Array(elem_ty, _) => {
                        let idx = self.emit_expr(index)?;
                        let (elem_size, _) = self.field_size_align_for_layout(elem_ty);
                        let stride = self.builder.ins().iconst(types::I64, elem_size as i64);
                        let byte_offset = self.builder.ins().imul(idx, stride);
                        let offset = if self.ptr_ty == types::I64 {
                            byte_offset
                        } else {
                            self.builder.ins().ireduce(self.ptr_ty, byte_offset)
                        };
                        let addr = self.builder.ins().iadd(container, offset);
                        let inline_struct = matches!(elem_ty.as_ref(), Ty::Named(..))
                            && self.ty_is_inline_for_layout(elem_ty);
                        if inline_struct {
                            Ok((addr, (**elem_ty).clone()))
                        } else {
                            let Some(cl) = cl_type(elem_ty, self.ptr_ty) else {
                                return Err(format!(
                                    "native codegen cannot index `array` of `{}` yet",
                                    self.display_ty(elem_ty)
                                ));
                            };
                            let value = self.builder.ins().load(cl, MemFlags::new(), addr, 0);
                            Ok((value, (**elem_ty).clone()))
                        }
                    }
                    _ => Err(native_codegen_unsupported(format!(
                        "indexed assignment base `{}` in native codegen",
                        self.display_ty(&container_ty)
                    ))),
                }
            }
        }
    }

    fn emit_field_lvalue_addr(
        &mut self,
        base: &HirLValue,
        field: &str,
    ) -> Result<(ir::Value, FieldLayout, ir::Value), String> {
        let (container, container_ty) = self.emit_lvalue_value(base)?;
        let Ty::Named(def_id, _) = &container_ty else {
            return Err(format!(
                "field assignment `{field}` requires a struct value, got `{}`",
                self.display_ty(&container_ty)
            ));
        };
        let layout = self.struct_layouts.get(def_id).cloned().ok_or_else(|| {
            format!(
                "missing native layout for `{}`",
                self.display_ty(&container_ty)
            )
        })?;
        let field_layout = layout
            .field(field)
            .cloned()
            .ok_or_else(|| format!("layout is missing field `{field}`"))?;
        let addr = self
            .builder
            .ins()
            .iadd_imm(container, i64::from(field_layout.offset));
        Ok((addr, field_layout, container))
    }

    fn malloc_bytes(&mut self, size: u32) -> Result<ir::Value, String> {
        self.malloc_bytes_with_destructor(size, None)
    }

    fn malloc_typed_bytes(
        &mut self,
        size: u32,
        type_def_id: ori_types::DefId,
    ) -> Result<ir::Value, String> {
        let callback_name = custom_destructor_callback_name(type_def_id);
        let destructor = self.func_refs.get(&callback_name).copied();
        let alloc_ref = *self
            .func_refs
            .get("ori_alloc_typed")
            .ok_or_else(|| "missing runtime function `ori_alloc_typed`".to_string())?;
        let size_v = self.builder.ins().iconst(types::I64, size as i64);
        let dtor_v = match destructor {
            Some(destructor) => self.builder.ins().func_addr(self.ptr_ty, destructor),
            None => self.builder.ins().iconst(self.ptr_ty, 0),
        };
        // DefId zero is a valid compiler ID; reserve runtime tag zero for
        // untyped/legacy allocations so nominal validation cannot accept one.
        let type_tag = self
            .builder
            .ins()
            .iconst(types::I64, i64::from(type_def_id.0) + 1);
        let call = self
            .builder
            .ins()
            .call(alloc_ref, &[size_v, dtor_v, type_tag]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn malloc_bytes_with_destructor(
        &mut self,
        size: u32,
        destructor: Option<ir::FuncRef>,
    ) -> Result<ir::Value, String> {
        let alloc_ref = *self
            .func_refs
            .get("ori_alloc")
            .ok_or_else(|| "missing runtime function `ori_alloc`".to_string())?;
        let size_v = self.builder.ins().iconst(types::I64, size as i64);
        let dtor_v = match destructor {
            Some(destructor) => self.builder.ins().func_addr(self.ptr_ty, destructor),
            None => self.builder.ins().iconst(self.ptr_ty, 0),
        };
        let call = self.builder.ins().call(alloc_ref, &[size_v, dtor_v]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn string_ptr(&mut self, value: &str) -> Result<ir::Value, String> {
        let gv = *self
            .string_gvs
            .get(value)
            .ok_or_else(|| format!("string literal `{value}` was not emitted in native codegen"))?;
        let base = self.builder.ins().global_value(self.ptr_ty, gv);
        // Skip the 16-byte header to point to the string payload
        Ok(self.builder.ins().iadd_imm(base, 16))
    }

    fn bytes_ptr(&mut self, bytes: &[u8]) -> Result<ir::Value, String> {
        let total = bytes.len() + 1;
        let base = self.malloc_bytes(total as u32)?;

        for (idx, byte) in bytes.iter().enumerate() {
            let value = self.builder.ins().iconst(types::I8, i64::from(*byte as i8));
            self.builder
                .ins()
                .store(MemFlags::new(), value, base, idx as i32);
        }
        let nul = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), nul, base, bytes.len() as i32);

        Ok(base)
    }

    fn call_string_parts_function(
        &mut self,
        function_name: &str,
        value: ir::Value,
    ) -> Result<StringParts, String> {
        let fref = *self
            .func_refs
            .get(function_name)
            .ok_or_else(|| format!("missing runtime function `{function_name}`"))?;
        let ptr_slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            8,
            3,
        ));
        let len_slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            8,
            3,
        ));
        let ptr_addr = self.builder.ins().stack_addr(self.ptr_ty, ptr_slot, 0);
        let len_addr = self.builder.ins().stack_addr(self.ptr_ty, len_slot, 0);
        self.builder.ins().call(fref, &[value, ptr_addr, len_addr]);
        let ptr = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), ptr_addr, 0);
        let len = self
            .builder
            .ins()
            .load(types::I64, MemFlags::new(), len_addr, 0);
        Ok(StringParts { ptr, len })
    }

    fn int_to_string_parts(&mut self, value: ir::Value) -> Result<StringParts, String> {
        self.call_string_parts_function("ori_to_string_parts", value)
    }

    fn uint_to_string_parts(&mut self, value: ir::Value) -> Result<StringParts, String> {
        self.call_string_parts_function("ori_uint_to_string_parts", value)
    }

    fn float_to_string_parts(&mut self, value: ir::Value) -> Result<StringParts, String> {
        self.call_string_parts_function("ori_float_to_string_parts", value)
    }

    fn bool_to_string_parts(&mut self, value: ir::Value) -> Result<StringParts, String> {
        self.call_string_parts_function("ori_bool_to_string_parts", value)
    }

    fn emit_to_string_call_parts(&mut self, expr: &HirExpr) -> Result<Option<StringParts>, String> {
        let HirExprKind::Call { callee, args } = &expr.kind else {
            return Ok(None);
        };
        let HirExprKind::Var(name) = &callee.kind else {
            return Ok(None);
        };
        let Some(arg) = args.first() else {
            return Ok(None);
        };
        let parts_function = match name.as_str() {
            "ori_to_string" => "ori_to_string_parts",
            "ori_uint_to_string" => "ori_uint_to_string_parts",
            "ori_float_to_string" => "ori_float_to_string_parts",
            "ori_bool_to_string" => "ori_bool_to_string_parts",
            _ => return Ok(None),
        };
        let value = self.emit_expr(&arg.value)?;
        let value = match &arg.value.ty {
            Ty::Int8 | Ty::Int16 | Ty::Int32 => self.builder.ins().sextend(types::I64, value),
            Ty::U8 | Ty::U16 | Ty::U32 => self.builder.ins().uextend(types::I64, value),
            Ty::Float32 if parts_function == "ori_float_to_string_parts" => {
                self.builder.ins().fpromote(types::F64, value)
            }
            _ => value,
        };
        self.call_string_parts_function(parts_function, value)
            .map(Some)
    }

    fn emit_as_string_parts(&mut self, expr: &HirExpr) -> Result<StringParts, String> {
        if let Some(parts) = self.emit_to_string_call_parts(expr)? {
            return Ok(parts);
        }
        if let HirExprKind::InterpolatedStr(parts) = &expr.kind {
            return self.emit_interpolated_string_parts(parts);
        }
        let value = self.emit_expr(expr)?;
        match &expr.ty {
            Ty::String => {
                let len = self.str_len_from_ptr(value)?;
                Ok(StringParts { ptr: value, len })
            }
            Ty::Int | Ty::Int64 => self.int_to_string_parts(value),
            // `u64` fills the whole slot, so it must be rendered unsigned.
            Ty::U64 => self.uint_to_string_parts(value),
            Ty::Int8 | Ty::Int16 | Ty::Int32 => {
                let widened = self.builder.ins().sextend(types::I64, value);
                self.int_to_string_parts(widened)
            }
            Ty::U8 | Ty::U16 | Ty::U32 => {
                let widened = self.builder.ins().uextend(types::I64, value);
                self.int_to_string_parts(widened)
            }
            Ty::Float | Ty::Float64 => self.float_to_string_parts(value),
            Ty::Float32 => {
                let widened = self.builder.ins().fpromote(types::F64, value);
                self.float_to_string_parts(widened)
            }
            Ty::Bool => self.bool_to_string_parts(value),
            _ => Err(format!(
                "native interpolated strings do not support expression type `{}`",
                self.display_ty(&expr.ty)
            )),
        }
    }

    fn emit_interpolated_string_parts(
        &mut self,
        parts: &[HirStrPart],
    ) -> Result<StringParts, String> {
        let mut current = StringParts {
            ptr: self.string_ptr("")?,
            len: self.builder.ins().iconst(types::I64, 0),
        };
        let mut current_owned = false;
        for part in parts {
            let (next, next_owned) = match part {
                HirStrPart::Literal(s) => (
                    StringParts {
                        ptr: self.string_ptr(s.as_str())?,
                        len: self.builder.ins().iconst(types::I64, s.len() as i64),
                    },
                    false,
                ),
                HirStrPart::Expr(expr) => {
                    // Scalar conversions allocate a fresh managed string in
                    // the runtime; string-typed parts follow expression
                    // ownership (borrowed bindings must not be released).
                    let owned = if matches!(expr.ty, Ty::String | Ty::Infer(_)) {
                        Self::expr_produces_owned_ref(expr)
                    } else {
                        true
                    };
                    (self.emit_as_string_parts(expr)?, owned)
                }
            };
            let merged = self.concat_string_parts(current, next)?;
            // The concat copied both inputs; release fresh temporaries so
            // intermediate results do not leak.
            if current_owned {
                self.emit_arc_release_if_managed(&Ty::String, current.ptr)?;
            }
            if next_owned {
                self.emit_arc_release_if_managed(&Ty::String, next.ptr)?;
            }
            current = merged;
            current_owned = true;
        }
        Ok(current)
    }

    fn emit_interpolated_string(&mut self, parts: &[HirStrPart]) -> Result<ir::Value, String> {
        Ok(self.emit_interpolated_string_parts(parts)?.ptr)
    }

    fn concat_string_parts(
        &mut self,
        left: StringParts,
        right: StringParts,
    ) -> Result<StringParts, String> {
        let concat_ref = *self
            .func_refs
            .get("ori_string_concat_parts")
            .ok_or_else(|| "missing runtime function `ori_string_concat_parts`".to_string())?;
        let call = self
            .builder
            .ins()
            .call(concat_ref, &[left.ptr, left.len, right.ptr, right.len]);
        let len = self.builder.ins().iadd(left.len, right.len);
        Ok(StringParts {
            ptr: self.builder.inst_results(call)[0],
            len,
        })
    }

    // == Statements ==

    fn emit_block(&mut self, block: &HirBlock) -> Result<(), String> {
        self.emit_scoped_stmts(&block.stmts)
    }

    fn emit_scoped_stmts(&mut self, stmts: &[HirStmt]) -> Result<(), String> {
        self.push_scope();
        let cleanup_start = self.using_stack.len();
        let managed_cleanup_start = self.managed_stack.len();
        for s in stmts {
            if self.terminated {
                break;
            }
            self.emit_stmt(s)?;
        }
        if !self.terminated {
            self.emit_scope_cleanup_calls_from(cleanup_start, managed_cleanup_start)?;
        }
        self.using_stack.truncate(cleanup_start);
        self.managed_stack.truncate(managed_cleanup_start);
        self.pop_scope();
        Ok(())
    }

    fn emit_scope_cleanup_calls_from(
        &mut self,
        using_start: usize,
        managed_start: usize,
    ) -> Result<(), String> {
        self.emit_using_cleanup_calls_from(using_start)?;
        self.emit_managed_cleanup_calls_from(managed_start)?;
        // LANG-PERF-2: never full-scan inside loops (empty managed stack at
        // while/for body entry used to call collect every iteration).
        // LANG-MEM-3 partial: function-root cleanups use the amortized
        // cooperative gate (`ori_arc_maybe_collect_cycles`) instead of an
        // unconditional O(n) trial-deletion pass. Full scans still run when
        // the allocation counter crosses the threshold (default 256), from
        // the async executor, or via explicit `ori_arc_collect_cycles` /
        // `ori.test.collect_cycles`.
        if managed_start == 0 && self.loop_stack.is_empty() {
            self.emit_arc_maybe_collect_cycles()?;
        }
        Ok(())
    }

    fn emit_using_cleanup_calls_from(&mut self, start: usize) -> Result<(), String> {
        let cleanups: Vec<UsingCleanup> = self.using_stack[start..].to_vec();
        for cleanup in cleanups.iter().rev() {
            self.emit_dispose_call(cleanup)?;
        }
        Ok(())
    }

    fn emit_managed_cleanup_calls_from(&mut self, start: usize) -> Result<(), String> {
        let cleanups: Vec<ManagedCleanup> = self.managed_stack[start..].to_vec();
        for cleanup in cleanups.iter().rev() {
            let value = self.builder.use_var(cleanup.var);
            self.emit_arc_release_if_managed(&cleanup.ty, value)?;
        }
        Ok(())
    }

    /// Full-frame cleanup for a sync `return`. When `transfer_var` is set,
    /// exactly one release (the innermost entry for that variable, i.e. the
    /// binding whose +1 the return transfers to the caller) is skipped.
    fn emit_return_scope_cleanup(&mut self, transfer_var: Option<Variable>) -> Result<(), String> {
        self.emit_using_cleanup_calls_from(0)?;
        let cleanups: Vec<ManagedCleanup> = self.managed_stack.to_vec();
        let mut transfer_pending = transfer_var;
        for cleanup in cleanups.iter().rev() {
            if transfer_pending == Some(cleanup.var) {
                transfer_pending = None;
                continue;
            }
            let value = self.builder.use_var(cleanup.var);
            self.emit_arc_release_if_managed(&cleanup.ty, value)?;
        }
        if self.loop_stack.is_empty() {
            self.emit_arc_maybe_collect_cycles()?;
        }
        Ok(())
    }

    fn emit_dispose_call(&mut self, cleanup: &UsingCleanup) -> Result<(), String> {
        let Some(func_name) = self.dispose_func_name_for_ty(&cleanup.ty) else {
            return Err(format!(
                "using cleanup for `{}` has no disposable function name",
                self.display_ty(&cleanup.ty)
            ));
        };
        let Some(&dispose_ref) = self.func_refs.get(func_name.as_str()) else {
            return Err(format!(
                "using cleanup for `{}` could not find native function `{}`",
                self.display_ty(&cleanup.ty),
                func_name
            ));
        };
        let value = self.builder.use_var(cleanup.var);
        self.builder.ins().call(dispose_ref, &[value]);
        Ok(())
    }

    fn emit_dispose_call_for_value(&mut self, ty: &Ty, value: ir::Value) -> Result<(), String> {
        let Some(func_name) = self.dispose_func_name_for_ty(ty) else {
            return Ok(());
        };
        let Some(&dispose_ref) = self.func_refs.get(func_name.as_str()) else {
            return Ok(());
        };
        self.builder.ins().call(dispose_ref, &[value]);
        Ok(())
    }

    fn emit_async_frame_dispose_live_values(
        &mut self,
        plan: &SimpleAsyncStateMachinePlan,
        frame: ir::Value,
        completed_await_count: usize,
    ) -> Result<(), String> {
        for (index, step) in plan
            .awaits
            .iter()
            .take(completed_await_count)
            .enumerate()
            .rev()
        {
            let Some(binding) = &step.binding else {
                continue;
            };
            let Some(func_name) = self.dispose_func_name_for_ty(&binding.ty) else {
                continue;
            };
            if !self.func_refs.contains_key(func_name.as_str()) {
                continue;
            }
            let cl_ty = cl_type(&binding.ty, self.ptr_ty).ok_or_else(|| {
                format!(
                    "async dispose for binding `{}` has no native value type",
                    binding.name
                )
            })?;
            let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                .expect("async frame binding offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            self.emit_dispose_call_for_value(&binding.ty, value)?;
        }
        for (local_index, local) in plan.locals.iter().enumerate().rev() {
            let Some(func_name) = self.dispose_func_name_for_ty(&local.ty) else {
                continue;
            };
            if !self.func_refs.contains_key(func_name.as_str()) {
                continue;
            }
            let cl_ty = cl_type(&local.ty, self.ptr_ty).ok_or_else(|| {
                format!(
                    "async dispose for local `{}` has no native value type",
                    local.name
                )
            })?;
            let offset = simple_async_frame_local_offset(plan, local_index, self.ptr_ty)
                .expect("async frame local offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            self.emit_dispose_call_for_value(&local.ty, value)?;
        }
        Ok(())
    }

    fn emit_async_terminal_cleanup(
        &mut self,
        plan: &SimpleAsyncStateMachinePlan,
        frame: ir::Value,
        completed_await_count: usize,
    ) -> Result<(), String> {
        self.emit_scope_cleanup_calls_from(0, 0)?;
        self.emit_async_frame_dispose_live_values(plan, frame, completed_await_count)?;
        self.emit_simple_async_frame_cleanup(plan, frame, completed_await_count, true)
    }

    fn dispose_func_name_for_ty(&self, ty: &Ty) -> Option<SmolStr> {
        match ty {
            Ty::Named(def_id, _) => {
                let mut matches = Vec::new();
                for ((trait_def_id, impl_type_def_id), impl_sig) in self.trait_impls {
                    if impl_type_def_id != def_id {
                        continue;
                    }
                    let Some(trait_sig) = self.trait_layouts.get(trait_def_id) else {
                        continue;
                    };
                    if !trait_sig.name.ends_with(".Disposable") {
                        continue;
                    }
                    if let Some(method) = impl_sig
                        .methods
                        .iter()
                        .find(|method| method.name == "dispose")
                    {
                        matches.push(method.func_name.clone());
                    }
                }
                if matches.len() == 1 {
                    return matches.pop();
                }
                self.type_names
                    .get(def_id)
                    .map(|name| SmolStr::new(format!("{name}.dispose")))
            }
            Ty::Opaque {
                kind: OpaqueTy::File,
                ..
            } => Some(SmolStr::new("ori_files_close")),
            Ty::Opaque {
                kind: OpaqueTy::Connection,
                ..
            } => Some(SmolStr::new("ori_net_close")),
            Ty::Opaque {
                kind: OpaqueTy::Input,
                ..
            } => Some(SmolStr::new("ori_io_close_input")),
            Ty::Opaque {
                kind: OpaqueTy::Output,
                ..
            } => Some(SmolStr::new("ori_io_close_output")),
            _ => None,
        }
    }

    fn trait_method_func_name_for_type(
        &self,
        type_def_id: ori_types::DefId,
        method_name: &SmolStr,
    ) -> Option<SmolStr> {
        let mut matches = Vec::new();
        for ((_, impl_type_def_id), impl_sig) in self.trait_impls {
            if *impl_type_def_id != type_def_id {
                continue;
            }
            if let Some(method) = impl_sig
                .methods
                .iter()
                .find(|method| method.name == *method_name)
            {
                matches.push(method.func_name.clone());
            }
        }
        (matches.len() == 1).then(|| matches.remove(0))
    }

    fn trait_method_func_name_for_trait(
        &self,
        type_def_id: ori_types::DefId,
        trait_suffix: &str,
        method_name: &str,
    ) -> Option<SmolStr> {
        let mut matches = Vec::new();
        for ((trait_def_id, impl_type_def_id), impl_sig) in self.trait_impls {
            if *impl_type_def_id != type_def_id {
                continue;
            }
            let Some(trait_layout) = self.trait_layouts.get(trait_def_id) else {
                continue;
            };
            if !trait_layout.name.ends_with(trait_suffix) {
                continue;
            }
            if let Some(method) = impl_sig
                .methods
                .iter()
                .find(|method| method.name == method_name)
            {
                matches.push(method.func_name.clone());
            }
        }
        (matches.len() == 1).then(|| matches.remove(0))
    }

    fn equality_callback_for_type(&mut self, ty: &Ty) -> Option<ir::Value> {
        let Ty::Named(def_id, _) = ty else {
            return None;
        };
        let struct_name = format!("__eq_helper_struct_{}", def_id.0);
        let enum_name = format!("__eq_helper_enum_{}", def_id.0);
        let fref = self
            .func_refs
            .get(struct_name.as_str())
            .or_else(|| self.func_refs.get(enum_name.as_str()))
            .copied()?;
        Some(self.builder.ins().func_addr(self.ptr_ty, fref))
    }

    fn hash_callback_for_type(&mut self, ty: &Ty) -> Option<ir::Value> {
        let Ty::Named(def_id, _) = ty else {
            return None;
        };
        let struct_name = format!("__hash_helper_struct_{}", def_id.0);
        let enum_name = format!("__hash_helper_enum_{}", def_id.0);
        let fref = self
            .func_refs
            .get(struct_name.as_str())
            .or_else(|| self.func_refs.get(enum_name.as_str()))
            .copied()?;
        Some(self.builder.ins().func_addr(self.ptr_ty, fref))
    }

    fn associated_func_name_for_type(
        &self,
        type_def_id: ori_types::DefId,
        method_name: &SmolStr,
    ) -> Option<SmolStr> {
        let mut matches = Vec::new();
        for ((trait_def_id, impl_type_def_id), impl_sig) in self.trait_impls {
            if *impl_type_def_id != type_def_id {
                continue;
            }
            let trait_method = self
                .trait_layouts
                .get(trait_def_id)?
                .methods
                .iter()
                .find(|method| method.name == *method_name && !method.has_self);
            let Some(trait_method) = trait_method else {
                continue;
            };
            if let Some(method) = impl_sig
                .methods
                .iter()
                .find(|method| method.name == *method_name)
            {
                matches.push(method.func_name.clone());
            } else if let Some(default_name) = &trait_method.default_func_name {
                matches.push(default_name.clone());
            }
        }
        (matches.len() == 1).then(|| matches.remove(0))
    }

    fn iterable_next_func_name_for_type(&self, ty: &Ty) -> Option<SmolStr> {
        let Ty::Named(type_def_id, _) = ty else {
            return None;
        };
        self.trait_impls
            .iter()
            .filter(|((_, impl_type_def_id), _)| impl_type_def_id == type_def_id)
            .find_map(|((trait_def_id, _), impl_sig)| {
                let trait_sig = self.trait_layouts.get(trait_def_id)?;
                if !trait_sig.name.ends_with(".Iterable") {
                    return None;
                }
                impl_sig
                    .methods
                    .iter()
                    .find(|method| method.name == "next")
                    .map(|method| method.func_name.clone())
            })
    }

    fn emit_arc_retain_if_managed(&mut self, ty: &Ty, value: ir::Value) -> Result<(), String> {
        if !is_managed_ty(ty) {
            return Ok(());
        }
        let Some(&retain_ref) = self.func_refs.get("ori_arc_retain") else {
            return Ok(());
        };
        self.builder.ins().call(retain_ref, &[value]);
        Ok(())
    }

    fn emit_arc_release_if_managed(&mut self, ty: &Ty, value: ir::Value) -> Result<(), String> {
        if !is_managed_ty(ty) {
            return Ok(());
        }
        let Some(&release_ref) = self.func_refs.get("ori_arc_release") else {
            return Ok(());
        };
        self.builder.ins().call(release_ref, &[value]);
        Ok(())
    }

    fn emit_arc_register_edge(&mut self, owner: ir::Value, child: ir::Value) -> Result<(), String> {
        let Some(&register_ref) = self.func_refs.get("ori_arc_register_edge") else {
            return Ok(());
        };
        self.builder.ins().call(register_ref, &[owner, child]);
        Ok(())
    }

    fn emit_arc_unregister_edge(
        &mut self,
        owner: ir::Value,
        child: ir::Value,
    ) -> Result<(), String> {
        let Some(&unregister_ref) = self.func_refs.get("ori_arc_unregister_edge") else {
            return Ok(());
        };
        self.builder.ins().call(unregister_ref, &[owner, child]);
        Ok(())
    }

    fn emit_arc_register_edge_if_managed(
        &mut self,
        ty: &Ty,
        owner: ir::Value,
        child: ir::Value,
    ) -> Result<(), String> {
        if !is_managed_ty(ty) {
            return Ok(());
        }
        self.emit_arc_register_edge(owner, child)
    }

    fn emit_arc_update_edge_if_managed(
        &mut self,
        ty: &Ty,
        owner: ir::Value,
        old_child: ir::Value,
        new_child: ir::Value,
    ) -> Result<(), String> {
        if !is_managed_ty(ty) {
            return Ok(());
        }
        let Some(&update_ref) = self.func_refs.get("ori_arc_update_edge") else {
            return Ok(());
        };
        self.builder
            .ins()
            .call(update_ref, &[owner, old_child, new_child]);
        Ok(())
    }

    fn emit_arc_maybe_collect_cycles(&mut self) -> Result<(), String> {
        if let Some(&collect_ref) = self.func_refs.get("ori_arc_maybe_collect_cycles") {
            self.builder.ins().call(collect_ref, &[]);
        }
        Ok(())
    }

    /// Whether `emit_expr` for this expression yields a freshly-owned +1 ref
    /// (from malloc or a callee's return path), meaning the binding/return
    /// path must NOT add an extra retain — otherwise the value leaks because
    /// the temporary is never pushed to the managed stack and scope cleanup
    /// never releases the extra ref.
    fn expr_produces_owned_ref(expr: &HirExpr) -> bool {
        matches!(
            expr.kind,
            HirExprKind::StructLit { .. }
                | HirExprKind::EnumVariant { .. }
                | HirExprKind::TupleLit(_)
                | HirExprKind::ListLit { .. }
                | HirExprKind::ListSpreadLit { .. }
                | HirExprKind::MapLit { .. }
                | HirExprKind::Call { .. }
                | HirExprKind::MethodCall { .. }
                | HirExprKind::Some_(_)
                | HirExprKind::None_
                | HirExprKind::Ok_(_)
                | HirExprKind::Err_(_)
                | HirExprKind::Propagate(_)
                | HirExprKind::Await(_)
                | HirExprKind::Range { .. }
                | HirExprKind::InterpolatedStr(_)
                // Binary/Unary on managed types (string/bytes concat, etc.)
                // allocate a fresh +1 ref via runtime helpers.
                | HirExprKind::Binary { .. }
                | HirExprKind::Unary { .. }
                // `emit_match_expr` retains any borrowed arm value before the
                // merge, so the value leaving a match expression is always +1.
                | HirExprKind::MatchExpr { .. }
        )
    }

    fn current_loop(&self) -> Option<LoopContext> {
        self.loop_stack.last().copied()
    }

    fn push_loop(&mut self, continue_target: ir::Block, break_target: ir::Block) {
        self.loop_stack.push(LoopContext {
            continue_target,
            break_target,
            cleanup_start: self.using_stack.len(),
            managed_cleanup_start: self.managed_stack.len(),
        });
    }

    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    fn emit_return(&mut self, val: Option<&HirExpr>) -> Result<(), String> {
        if let Some(frame) = self.async_frame {
            let plan = self
                .async_plan
                .expect("must have async plan if frame is set");
            let inner_ty = plan.inner_ty.clone();
            let value = match val {
                Some(expr) if inner_ty == Ty::Void => {
                    self.emit_expr(expr)?;
                    None
                }
                Some(expr) => Some(self.emit_expr_for_expected(expr, &inner_ty)?),
                None => None,
            };
            let result_future = self.builder.ins().load(
                self.ptr_ty,
                MemFlags::new(),
                frame,
                ASYNC_FRAME_RESULT_OFFSET,
            );
            if let Some(v) = value {
                self.emit_arc_retain_if_managed(&inner_ty, v)?;
                self.emit_future_complete(result_future, &inner_ty, Some(v))?;
                self.emit_arc_release_if_managed(&inner_ty, v)?;
            } else {
                self.emit_future_complete(result_future, &inner_ty, None)?;
            }
            self.emit_scope_cleanup_calls_from(0, 0)?;
            self.emit_simple_async_frame_cleanup(plan, frame, plan.awaits.len(), true)?;
            self.emit_debug_leave();
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.ins().return_(&[zero]);
            self.terminated = true;
            return Ok(());
        }
        let return_ty = self.current_return_ty.clone();
        if let Ty::Future(inner) = return_ty {
            let inner_ty = inner.as_ref().clone();
            let value = match val {
                Some(expr) if inner_ty == Ty::Void => {
                    self.emit_expr(expr)?;
                    None
                }
                Some(expr) => Some(self.emit_expr_for_expected(expr, &inner_ty)?),
                None => None,
            };
            let future = self.emit_future_ready(&inner_ty, value)?;
            self.emit_arc_retain_if_managed(&Ty::Future(Box::new(inner_ty)), future)?;
            self.emit_scope_cleanup_calls_from(0, 0)?;
            self.emit_debug_leave();
            self.builder.ins().return_(&[future]);
            self.terminated = true;
            return Ok(());
        }
        let return_expr_is_owned = val.map(Self::expr_produces_owned_ref).unwrap_or(false);
        // Return-transfer elision (LANG-MEM-4): returning a managed local
        // hands the binding's own +1 to the caller instead of emitting a
        // retain (for the return) plus a release (frame cleanup) on the
        // same value. Only a plain `Var` of the exact return type is a
        // transfer; anything else keeps the retain/cleanup pair.
        let transfer_var = val.and_then(|expr| {
            let HirExprKind::Var(name) = &expr.kind else {
                return None;
            };
            if expr.ty != return_ty || !is_managed_ty(&return_ty) {
                return None;
            }
            let (var, _) = self.lookup_var(name)?;
            self.managed_stack
                .iter()
                .any(|cleanup| cleanup.var == var)
                .then_some(var)
        });
        let return_value = val
            .map(|e| self.emit_expr_for_expected(e, &return_ty))
            .transpose()?;
        if let Some(value) = return_value {
            if !return_expr_is_owned && transfer_var.is_none() {
                self.emit_arc_retain_if_managed(&return_ty, value)?;
            }
        }
        self.emit_return_scope_cleanup(transfer_var)?;
        self.emit_debug_leave();
        if let Some(v) = return_value {
            self.builder.ins().return_(&[v]);
        } else {
            self.builder.ins().return_(&[]);
        }
        self.terminated = true;
        Ok(())
    }

    fn emit_future_ready(
        &mut self,
        inner_ty: &Ty,
        value: Option<ir::Value>,
    ) -> Result<ir::Value, String> {
        let runtime_name = match inner_ty {
            Ty::Void | Ty::Never => "ori_future_ready_void",
            Ty::Float | Ty::Float64 | Ty::Float32 => "ori_future_ready_f64",
            Ty::String
            | Ty::Bytes
            | Ty::Func { .. }
            | Ty::Lazy(_)
            | Ty::Future(_)
            | Ty::TaskJob(_)
            | Ty::Channel(_)
            | Ty::AtomicInt
            | Ty::TaskJoinError
            | Ty::ChannelSendError
            | Ty::ChannelReceiveError
            | Ty::Any(_)
            | Ty::Optional(_)
            | Ty::Result(_, _)
            | Ty::List(_)
            | Ty::Map(_, _)
            | Ty::Set(_)
            | Ty::Range(_)
            | Ty::Tuple(_)
            | Ty::Named(_, _) => "ori_future_ready_ptr",
            _ => "ori_future_ready_i64",
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        let args = match runtime_name {
            "ori_future_ready_void" => Vec::new(),
            "ori_future_ready_f64" => {
                let mut value = value.unwrap_or_else(|| self.builder.ins().f64const(0.0));
                if self.builder.func.dfg.value_type(value) == types::F32 {
                    value = self.builder.ins().fpromote(types::F64, value);
                }
                vec![value]
            }
            "ori_future_ready_ptr" => {
                vec![value.unwrap_or_else(|| self.builder.ins().iconst(self.ptr_ty, 0))]
            }
            _ => {
                let mut value = value.unwrap_or_else(|| self.builder.ins().iconst(types::I64, 0));
                let actual_ty = self.builder.func.dfg.value_type(value);
                if actual_ty != types::I64 {
                    value = match inner_ty {
                        Ty::Bool | Ty::U8 | Ty::U16 | Ty::U32 => {
                            self.builder.ins().uextend(types::I64, value)
                        }
                        Ty::Int8 | Ty::Int16 | Ty::Int32 => {
                            self.builder.ins().sextend(types::I64, value)
                        }
                        _ => value,
                    };
                }
                vec![value]
            }
        };
        let call = self.builder.ins().call(fref, &args);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_future_complete(
        &mut self,
        future: ir::Value,
        inner_ty: &Ty,
        value: Option<ir::Value>,
    ) -> Result<(), String> {
        let runtime_name = match inner_ty {
            Ty::Void | Ty::Never => "ori_future_complete_void",
            Ty::Float | Ty::Float64 | Ty::Float32 => "ori_future_complete_f64",
            Ty::String
            | Ty::Bytes
            | Ty::Func { .. }
            | Ty::Lazy(_)
            | Ty::Future(_)
            | Ty::TaskJob(_)
            | Ty::Channel(_)
            | Ty::AtomicInt
            | Ty::TaskJoinError
            | Ty::ChannelSendError
            | Ty::ChannelReceiveError
            | Ty::Any(_)
            | Ty::Optional(_)
            | Ty::Result(_, _)
            | Ty::List(_)
            | Ty::Map(_, _)
            | Ty::Set(_)
            | Ty::Range(_)
            | Ty::Tuple(_)
            | Ty::Named(_, _) => "ori_future_complete_ptr",
            _ => "ori_future_complete_i64",
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        let mut args = vec![future];
        match runtime_name {
            "ori_future_complete_void" => {}
            "ori_future_complete_f64" => {
                let mut value = value.unwrap_or_else(|| self.builder.ins().f64const(0.0));
                if self.builder.func.dfg.value_type(value) == types::F32 {
                    value = self.builder.ins().fpromote(types::F64, value);
                }
                args.push(value);
            }
            "ori_future_complete_ptr" => {
                args.push(value.unwrap_or_else(|| self.builder.ins().iconst(self.ptr_ty, 0)));
            }
            _ => {
                let mut value = value.unwrap_or_else(|| self.builder.ins().iconst(types::I64, 0));
                let actual_ty = self.builder.func.dfg.value_type(value);
                if actual_ty != types::I64 {
                    value = match inner_ty {
                        Ty::Bool | Ty::U8 | Ty::U16 | Ty::U32 => {
                            self.builder.ins().uextend(types::I64, value)
                        }
                        Ty::Int8 | Ty::Int16 | Ty::Int32 => {
                            self.builder.ins().sextend(types::I64, value)
                        }
                        _ => value,
                    };
                }
                args.push(value);
            }
        }
        self.builder.ins().call(fref, &args);
        Ok(())
    }

    fn emit_future_value_read(
        &mut self,
        future: ir::Value,
        result_ty: &Ty,
    ) -> Result<ir::Value, String> {
        let runtime_name = match result_ty {
            Ty::Float | Ty::Float64 | Ty::Float32 => "ori_future_value_f64",
            Ty::String
            | Ty::Bytes
            | Ty::Func { .. }
            | Ty::Lazy(_)
            | Ty::Future(_)
            | Ty::TaskJob(_)
            | Ty::Channel(_)
            | Ty::AtomicInt
            | Ty::TaskJoinError
            | Ty::ChannelSendError
            | Ty::ChannelReceiveError
            | Ty::Any(_)
            | Ty::Optional(_)
            | Ty::Result(_, _)
            | Ty::List(_)
            | Ty::Map(_, _)
            | Ty::Set(_)
            | Ty::Range(_)
            | Ty::Tuple(_)
            | Ty::Named(_, _) => "ori_future_value_ptr",
            _ => "ori_future_value_i64",
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        let call = self.builder.ins().call(fref, &[future]);
        let mut value = self.builder.inst_results(call)[0];
        if matches!(runtime_name, "ori_future_value_i64") {
            value = self.decode_list_storage_value(value, result_ty);
        } else if matches!(result_ty, Ty::Float32) {
            value = self.builder.ins().fdemote(types::F32, value);
        }
        Ok(value)
    }

    fn emit_simple_async_step(
        mut self,
        f: &HirFunc,
        plan: &SimpleAsyncStateMachinePlan,
    ) -> Result<(), String> {
        let entry = self.builder.create_block();
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        self.builder.seal_block(entry);
        let frame = self.builder.block_params(entry)[0];
        self.emit_debug_enter();
        let state =
            self.builder
                .ins()
                .load(types::I64, MemFlags::new(), frame, ASYNC_FRAME_STATE_OFFSET);
        for (index, param) in plan.params.iter().enumerate() {
            let cl_ty = cl_type(&param.ty, self.ptr_ty)
                .ok_or_else(|| format!("async param `{}` has no native value", param.name))?;
            let offset = simple_async_frame_param_offset(plan, index, self.ptr_ty)
                .expect("async frame param offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self.builder.declare_var(cl_ty);
            self.builder.def_var(var, value);
            self.insert_var(param.name.clone(), (var, param.ty.clone()));
        }
        for (index, local) in plan.locals.iter().enumerate() {
            let cl_ty = cl_type(&local.ty, self.ptr_ty)
                .ok_or_else(|| format!("async local `{}` has no native value", local.name))?;
            let offset = simple_async_frame_local_offset(plan, index, self.ptr_ty)
                .expect("async frame local offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self.builder.declare_var(cl_ty);
            self.builder.def_var(var, value);
            self.insert_var(local.name.clone(), (var, local.ty.clone()));
        }
        let await_count = plan.awaits.len();
        let eval_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let poll_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let pending_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let status_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let ready_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let non_pending_status_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let failed_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let cancelled_blocks: Vec<_> = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let dispatch_blocks: Vec<_> = (0..=await_count)
            .map(|_| self.builder.create_block())
            .collect();
        let invalid_state_block = self.builder.create_block();
        let complete_block = self.builder.create_block();

        self.builder.ins().jump(dispatch_blocks[0], &[]);
        for state_index in 0..=await_count {
            self.builder.switch_to_block(dispatch_blocks[state_index]);
            let is_state =
                self.builder
                    .ins()
                    .icmp_imm(ir::condcodes::IntCC::Equal, state, state_index as i64);
            let target = if state_index == 0 {
                eval_blocks[0]
            } else {
                poll_blocks[state_index - 1]
            };
            let next = if state_index < await_count {
                dispatch_blocks[state_index + 1]
            } else {
                invalid_state_block
            };
            self.builder.ins().brif(is_state, target, &[], next, &[]);
        }

        for (index, step) in plan.awaits.iter().enumerate() {
            let awaited_offset = simple_async_frame_awaited_offset(index, self.ptr_ty);

            self.builder.switch_to_block(eval_blocks[index]);
            if index == 0 {
                for (local_index, local) in plan.locals.iter().enumerate() {
                    let value = self.emit_expr_for_expected(&local.value, &local.ty)?;
                    let offset = simple_async_frame_local_offset(plan, local_index, self.ptr_ty)
                        .expect("async frame local offset");
                    self.builder
                        .ins()
                        .store(MemFlags::new(), value, frame, offset as i32);
                    self.emit_arc_register_edge_if_managed(&local.ty, frame, value)?;
                    if let Some((var, _)) = self.lookup_var(&local.name) {
                        self.builder.def_var(var, value);
                    }
                }
            }
            self.emit_debug_line_probe(step.await_future.span);
            let awaited = self.emit_expr(&step.await_future)?;
            self.builder
                .ins()
                .store(MemFlags::new(), awaited, frame, awaited_offset);
            self.emit_arc_register_edge(frame, awaited)?;
            self.emit_arc_release_if_managed(&step.await_future.ty, awaited)?;
            self.builder.ins().jump(poll_blocks[index], &[]);

            self.builder.switch_to_block(poll_blocks[index]);
            let awaited =
                self.builder
                    .ins()
                    .load(self.ptr_ty, MemFlags::new(), frame, awaited_offset);
            let poll_ref = *self
                .func_refs
                .get("ori_future_poll")
                .ok_or_else(|| "missing runtime function `ori_future_poll`".to_string())?;
            let poll_call = self.builder.ins().call(poll_ref, &[awaited]);
            let status = self.builder.inst_results(poll_call)[0];
            let ready = self
                .builder
                .ins()
                .icmp_imm(ir::condcodes::IntCC::Equal, status, 1);
            self.builder
                .ins()
                .brif(ready, ready_blocks[index], &[], status_blocks[index], &[]);

            self.builder.switch_to_block(ready_blocks[index]);
            let ready_continue_block = self.builder.create_block();
            if let Some(result_ty) = &step.propagate_result_ty {
                let result_value = self.emit_future_value_read(awaited, result_ty)?;
                let flag = self
                    .builder
                    .ins()
                    .load(types::I8, MemFlags::new(), result_value, 0);
                let ok_block = self.builder.create_block();
                let err_block = self.builder.create_block();
                self.builder.ins().brif(flag, ok_block, &[], err_block, &[]);

                self.builder.switch_to_block(err_block);
                let result_future = self.builder.ins().load(
                    self.ptr_ty,
                    MemFlags::new(),
                    frame,
                    ASYNC_FRAME_RESULT_OFFSET,
                );
                self.emit_future_complete(result_future, &plan.inner_ty, Some(result_value))?;
                self.emit_arc_unregister_edge(frame, awaited)?;
                self.emit_async_terminal_cleanup(plan, frame, index)?;
                self.emit_debug_leave();
                let zero = self.builder.ins().iconst(types::I64, 0);
                self.builder.ins().return_(&[zero]);

                self.builder.switch_to_block(ok_block);
                if step.binding.is_none() {
                    return Err("propagating await step has no binding".to_string());
                }
                let Ty::Result(ok_ty, err_ty) = result_ty else {
                    return Err("propagating await step requires result type".to_string());
                };
                let (pay_off, _, _) = result_layout(ok_ty, err_ty, self.ptr_ty);
                let cl_ty = cl_type(ok_ty, self.ptr_ty).unwrap_or(types::I64);
                let value =
                    self.builder
                        .ins()
                        .load(cl_ty, MemFlags::new(), result_value, pay_off as i32);
                let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                    .expect("async frame binding offset");
                self.builder
                    .ins()
                    .store(MemFlags::new(), value, frame, offset as i32);
                self.emit_arc_register_edge_if_managed(ok_ty, frame, value)?;
                self.builder.ins().jump(ready_continue_block, &[]);
            } else if let Some(binding) = &step.binding {
                let value = self.emit_future_value_read(awaited, &binding.ty)?;
                let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                    .expect("async frame binding offset");
                self.builder
                    .ins()
                    .store(MemFlags::new(), value, frame, offset as i32);
                self.emit_arc_register_edge_if_managed(&binding.ty, frame, value)?;
                self.builder.ins().jump(ready_continue_block, &[]);
            } else {
                self.builder.ins().jump(ready_continue_block, &[]);
            }
            self.builder.switch_to_block(ready_continue_block);
            self.emit_arc_unregister_edge(frame, awaited)?;
            self.emit_simple_async_drop_dead_frame_values_after_await(
                plan,
                frame,
                index,
                index + 1,
            )?;
            self.reload_async_frame_vars(plan, frame, index + 1)?;
            self.emit_debug_line_probe(step.await_future.span);
            if index + 1 < await_count {
                self.builder.ins().jump(eval_blocks[index + 1], &[]);
            } else {
                self.builder.ins().jump(complete_block, &[]);
            }

            self.builder.switch_to_block(status_blocks[index]);
            let pending = self
                .builder
                .ins()
                .icmp_imm(ir::condcodes::IntCC::Equal, status, 0);
            self.builder.ins().brif(
                pending,
                pending_blocks[index],
                &[],
                non_pending_status_blocks[index],
                &[],
            );

            self.builder.switch_to_block(pending_blocks[index]);
            let next_state = self.builder.ins().iconst(types::I64, (index + 1) as i64);
            self.builder
                .ins()
                .store(MemFlags::new(), next_state, frame, ASYNC_FRAME_STATE_OFFSET);
            let step_name = async_step_name(f);
            let step_ref = *self
                .func_refs
                .get(step_name.as_str())
                .ok_or_else(|| format!("missing async step function `{step_name}`"))?;
            let continuation = self.emit_closure_object(step_ref, Some(frame))?;
            let on_ready_ref = *self
                .func_refs
                .get("ori_future_on_ready")
                .ok_or_else(|| "missing runtime function `ori_future_on_ready`".to_string())?;
            self.builder
                .ins()
                .call(on_ready_ref, &[awaited, continuation]);
            self.emit_simple_async_drop_dead_frame_values_after_await(plan, frame, index, index)?;
            self.emit_debug_leave();
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.ins().return_(&[zero]);

            self.builder
                .switch_to_block(non_pending_status_blocks[index]);
            let failed = self
                .builder
                .ins()
                .icmp_imm(ir::condcodes::IntCC::Equal, status, 2);
            self.builder.ins().brif(
                failed,
                failed_blocks[index],
                &[],
                cancelled_blocks[index],
                &[],
            );

            self.builder.switch_to_block(cancelled_blocks[index]);
            let result_future = self.builder.ins().load(
                self.ptr_ty,
                MemFlags::new(),
                frame,
                ASYNC_FRAME_RESULT_OFFSET,
            );
            let cancel_ref = *self
                .func_refs
                .get("ori_future_cancel")
                .ok_or_else(|| "missing runtime function `ori_future_cancel`".to_string())?;
            self.emit_arc_unregister_edge(frame, awaited)?;
            self.builder.ins().call(cancel_ref, &[result_future]);
            self.emit_async_terminal_cleanup(plan, frame, index)?;
            self.emit_debug_leave();
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.ins().return_(&[zero]);

            self.builder.switch_to_block(failed_blocks[index]);
            let result_future = self.builder.ins().load(
                self.ptr_ty,
                MemFlags::new(),
                frame,
                ASYNC_FRAME_RESULT_OFFSET,
            );
            let fail_ref = *self
                .func_refs
                .get("ori_future_fail")
                .ok_or_else(|| "missing runtime function `ori_future_fail`".to_string())?;
            self.emit_arc_unregister_edge(frame, awaited)?;
            self.builder.ins().call(fail_ref, &[result_future]);
            self.emit_async_terminal_cleanup(plan, frame, index)?;
            self.emit_debug_leave();
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.ins().return_(&[zero]);
        }

        self.builder.switch_to_block(invalid_state_block);
        let result_future = self.builder.ins().load(
            self.ptr_ty,
            MemFlags::new(),
            frame,
            ASYNC_FRAME_RESULT_OFFSET,
        );
        let cancel_ref = *self
            .func_refs
            .get("ori_future_cancel")
            .ok_or_else(|| "missing runtime function `ori_future_cancel`".to_string())?;
        self.builder.ins().call(cancel_ref, &[result_future]);
        self.emit_async_terminal_cleanup(plan, frame, await_count)?;
        self.emit_debug_leave();
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().return_(&[zero]);

        self.builder.switch_to_block(complete_block);
        let result_future = self.builder.ins().load(
            self.ptr_ty,
            MemFlags::new(),
            frame,
            ASYNC_FRAME_RESULT_OFFSET,
        );
        for (index, step) in plan.awaits.iter().enumerate() {
            if let Some(binding) = &step.binding {
                let cl_ty = cl_type(&binding.ty, self.ptr_ty).ok_or_else(|| {
                    format!("async binding `{}` has no native value", binding.name)
                })?;
                let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                    .expect("async frame binding offset");
                let value = self
                    .builder
                    .ins()
                    .load(cl_ty, MemFlags::new(), frame, offset as i32);
                let var = self.builder.declare_var(cl_ty);
                self.builder.def_var(var, value);
                self.insert_var(binding.name.clone(), (var, binding.ty.clone()));
            }
        }
        for stmt in &plan.tail_stmts {
            self.emit_stmt(stmt)?;
        }
        if self.terminated {
            // The tail terminated (e.g. `return`), so the rest of the complete
            // block is unreachable. The emit_if/emit_while fixes already filled
            // the terminated blocks; make sure the complete block itself stays
            // well-formed by leaving the cursor on the last emitted block.
            self.builder.seal_all_blocks();
            self.builder.finalize();
            return Ok(());
        }
        if let Some(expr) = &plan.tail_expr {
            self.emit_expr(expr)?;
            self.emit_future_complete(result_future, &plan.inner_ty, None)?;
        } else {
            let return_value = plan
                .return_expr
                .as_ref()
                .map(|expr| self.emit_expr_for_expected(expr, &plan.inner_ty))
                .transpose()?;
            if let Some(value) = return_value {
                self.emit_arc_retain_if_managed(&plan.inner_ty, value)?;
                self.emit_future_complete(result_future, &plan.inner_ty, Some(value))?;
                self.emit_arc_release_if_managed(&plan.inner_ty, value)?;
            } else {
                self.emit_future_complete(result_future, &plan.inner_ty, None)?;
            }
        }
        self.emit_scope_cleanup_calls_from(0, 0)?;
        self.emit_simple_async_frame_cleanup(plan, frame, await_count, true)?;
        self.emit_debug_leave();
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().return_(&[zero]);

        self.builder.seal_all_blocks();
        self.builder.finalize();
        Ok(())
    }

    fn emit_simple_async_frame_cleanup(
        &mut self,
        plan: &SimpleAsyncStateMachinePlan,
        frame: ir::Value,
        initialized_bindings: usize,
        include_result_future: bool,
    ) -> Result<(), String> {
        if include_result_future {
            let result_future = self.builder.ins().load(
                self.ptr_ty,
                MemFlags::new(),
                frame,
                ASYNC_FRAME_RESULT_OFFSET,
            );
            self.emit_arc_unregister_edge(frame, result_future)?;
        }
        for (index, param) in plan.params.iter().enumerate() {
            if !is_managed_ty(&param.ty) {
                continue;
            }
            let offset = simple_async_frame_param_offset(plan, index, self.ptr_ty)
                .expect("async frame param offset");
            let value = self
                .builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), frame, offset as i32);
            self.emit_arc_unregister_edge(frame, value)?;
        }
        for (index, local) in plan.locals.iter().enumerate() {
            if !is_managed_ty(&local.ty) {
                continue;
            }
            let offset = simple_async_frame_local_offset(plan, index, self.ptr_ty)
                .expect("async frame local offset");
            let value = self
                .builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), frame, offset as i32);
            self.emit_arc_unregister_edge(frame, value)?;
        }
        for (index, step) in plan.awaits.iter().take(initialized_bindings).enumerate() {
            let Some(binding) = &step.binding else {
                continue;
            };
            if !is_managed_ty(&binding.ty) {
                continue;
            }
            let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                .expect("async frame binding offset");
            let value = self
                .builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), frame, offset as i32);
            self.emit_arc_unregister_edge(frame, value)?;
        }
        Ok(())
    }

    fn emit_simple_async_drop_dead_frame_values_after_await(
        &mut self,
        plan: &SimpleAsyncStateMachinePlan,
        frame: ir::Value,
        await_index: usize,
        initialized_bindings: usize,
    ) -> Result<(), String> {
        let live_after = simple_async_uses_after_await(plan, await_index);
        let mut dropped_any = false;

        for (index, param) in plan.params.iter().enumerate() {
            if !is_managed_ty(&param.ty)
                || live_after.contains(&param.name)
                || is_async_keep_alive_resource_ty(&param.ty)
            {
                continue;
            }
            let offset = simple_async_frame_param_offset(plan, index, self.ptr_ty)
                .expect("async frame param offset");
            self.emit_simple_async_drop_frame_edge(frame, offset as i32)?;
            dropped_any = true;
        }

        for (index, local) in plan.locals.iter().enumerate() {
            if !is_managed_ty(&local.ty)
                || live_after.contains(&local.name)
                || is_async_keep_alive_resource_ty(&local.ty)
            {
                continue;
            }
            let offset = simple_async_frame_local_offset(plan, index, self.ptr_ty)
                .expect("async frame local offset");
            self.emit_simple_async_drop_frame_edge(frame, offset as i32)?;
            dropped_any = true;
        }

        for (index, step) in plan.awaits.iter().take(initialized_bindings).enumerate() {
            let Some(binding) = &step.binding else {
                continue;
            };
            if !is_managed_ty(&binding.ty)
                || live_after.contains(&binding.name)
                || is_async_keep_alive_resource_ty(&binding.ty)
            {
                continue;
            }
            let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                .expect("async frame binding offset");
            self.emit_simple_async_drop_frame_edge(frame, offset as i32)?;
            dropped_any = true;
        }

        if dropped_any {
            // Same amortized gate as sync function roots: do not full-scan the
            // heap after every await resume when live allocations are large.
            self.emit_arc_maybe_collect_cycles()?;
        }
        Ok(())
    }

    fn emit_simple_async_drop_frame_edge(
        &mut self,
        frame: ir::Value,
        offset: i32,
    ) -> Result<(), String> {
        let value = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), frame, offset);
        self.emit_arc_unregister_edge(frame, value)?;
        let zero = self.builder.ins().iconst(self.ptr_ty, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), zero, frame, offset);
        Ok(())
    }

    fn emit_await(&mut self, future_expr: &HirExpr, result_ty: &Ty) -> Result<ir::Value, String> {
        let frame = self
            .async_frame
            .ok_or_else(|| "`await` can only be used inside async functions".to_string())?;
        let plan = self
            .async_plan
            .expect("must have async plan if frame is set");

        let index = self.async_await_index;
        self.async_await_index += 1;

        let awaited_offset = simple_async_frame_awaited_offset(index, self.ptr_ty);
        let awaited = self.emit_expr(future_expr)?;

        self.builder
            .ins()
            .store(MemFlags::new(), awaited, frame, awaited_offset);
        self.emit_arc_register_edge(frame, awaited)?;
        self.emit_arc_release_if_managed(&future_expr.ty, awaited)?;

        let poll_block = self.async_poll_blocks[index];
        self.builder.ins().jump(poll_block, &[]);

        // Switch to poll block (this is the dispatch target in the general
        // async step; it must be filled here, not left empty).
        self.builder.switch_to_block(poll_block);

        let awaited = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), frame, awaited_offset);

        let poll_ref = *self
            .func_refs
            .get("ori_future_poll")
            .ok_or_else(|| "missing runtime function `ori_future_poll`".to_string())?;
        let poll_call = self.builder.ins().call(poll_ref, &[awaited]);
        let status = self.builder.inst_results(poll_call)[0];

        let ready_block = self.builder.create_block();
        let status_block = self.builder.create_block();

        let ready = self
            .builder
            .ins()
            .icmp_imm(ir::condcodes::IntCC::Equal, status, 1);
        self.builder
            .ins()
            .brif(ready, ready_block, &[], status_block, &[]);

        // Status block: check pending vs non-pending (failed/cancelled)
        self.builder.switch_to_block(status_block);
        self.builder.seal_block(status_block);

        let pending_block = self.builder.create_block();
        let non_pending_status_block = self.builder.create_block();

        let pending = self
            .builder
            .ins()
            .icmp_imm(ir::condcodes::IntCC::Equal, status, 0);
        self.builder
            .ins()
            .brif(pending, pending_block, &[], non_pending_status_block, &[]);

        // Pending path: register continuation and return 0
        self.builder.switch_to_block(pending_block);
        self.builder.seal_block(pending_block);

        let next_state = self.builder.ins().iconst(types::I64, (index + 1) as i64);
        self.builder
            .ins()
            .store(MemFlags::new(), next_state, frame, ASYNC_FRAME_STATE_OFFSET);

        let step_name = SmolStr::new(format!("{}.__async_step", self.func_name));
        let step_ref = *self
            .func_refs
            .get(step_name.as_str())
            .ok_or_else(|| format!("missing async step function `{step_name}`"))?;
        let continuation = self.emit_closure_object(step_ref, Some(frame))?;

        let on_ready_ref = *self
            .func_refs
            .get("ori_future_on_ready")
            .ok_or_else(|| "missing runtime function `ori_future_on_ready`".to_string())?;
        self.builder
            .ins()
            .call(on_ready_ref, &[awaited, continuation]);

        self.emit_debug_leave();
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().return_(&[zero]);
        self.terminated = true;

        // Non-pending status path
        self.builder.switch_to_block(non_pending_status_block);
        self.builder.seal_block(non_pending_status_block);

        let failed_block = self.builder.create_block();
        let cancelled_block = self.builder.create_block();

        let failed = self
            .builder
            .ins()
            .icmp_imm(ir::condcodes::IntCC::Equal, status, 2);
        self.builder
            .ins()
            .brif(failed, failed_block, &[], cancelled_block, &[]);

        // Failed path
        self.builder.switch_to_block(failed_block);
        self.builder.seal_block(failed_block);
        let result_future = self.builder.ins().load(
            self.ptr_ty,
            MemFlags::new(),
            frame,
            ASYNC_FRAME_RESULT_OFFSET,
        );
        let fail_ref = *self
            .func_refs
            .get("ori_future_fail")
            .ok_or_else(|| "missing runtime function `ori_future_fail`".to_string())?;
        self.emit_arc_unregister_edge(frame, awaited)?;
        self.builder.ins().call(fail_ref, &[result_future]);
        self.emit_scope_cleanup_calls_from(0, 0)?;
        self.emit_simple_async_frame_cleanup(plan, frame, plan.awaits.len(), true)?;
        self.emit_debug_leave();
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().return_(&[zero]);
        self.terminated = true;

        // Cancelled path
        self.builder.switch_to_block(cancelled_block);
        self.builder.seal_block(cancelled_block);
        let result_future = self.builder.ins().load(
            self.ptr_ty,
            MemFlags::new(),
            frame,
            ASYNC_FRAME_RESULT_OFFSET,
        );
        let cancel_ref = *self
            .func_refs
            .get("ori_future_cancel")
            .ok_or_else(|| "missing runtime function `ori_future_cancel`".to_string())?;
        self.emit_arc_unregister_edge(frame, awaited)?;
        self.builder.ins().call(cancel_ref, &[result_future]);
        self.emit_scope_cleanup_calls_from(0, 0)?;
        self.emit_simple_async_frame_cleanup(plan, frame, plan.awaits.len(), true)?;
        self.emit_debug_leave();
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().return_(&[zero]);
        self.terminated = true;

        // Ready block path: load/read value, unregister and continue
        self.builder.switch_to_block(ready_block);
        self.builder.seal_block(ready_block);

        let value = self.emit_future_value_read(awaited, result_ty)?;
        self.emit_arc_unregister_edge(frame, awaited)?;
        self.reload_async_frame_vars(plan, frame, index)?;
        self.emit_debug_line_probe(future_expr.span);

        let cont_block = self.builder.create_block();
        self.builder.ins().jump(cont_block, &[]);

        self.builder.switch_to_block(cont_block);
        self.builder.seal_block(cont_block);
        self.terminated = false;

        Ok(value)
    }

    fn emit_general_async_step(
        mut self,
        f: &HirFunc,
        plan: &'a SimpleAsyncStateMachinePlan,
    ) -> Result<(), String> {
        let entry = self.builder.create_block();
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        self.builder.seal_block(entry);
        let frame = self.builder.block_params(entry)[0];
        self.emit_debug_enter();

        self.async_frame = Some(frame);
        self.async_plan = Some(plan);

        let state =
            self.builder
                .ins()
                .load(types::I64, MemFlags::new(), frame, ASYNC_FRAME_STATE_OFFSET);

        // Preload params, locals, and awaits bindings into self.vars
        for (index, param) in plan.params.iter().enumerate() {
            let cl_ty = cl_type(&param.ty, self.ptr_ty)
                .ok_or_else(|| format!("async param `{}` has no native value", param.name))?;
            let offset = simple_async_frame_param_offset(plan, index, self.ptr_ty)
                .expect("async frame param offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self.builder.declare_var(cl_ty);
            self.builder.def_var(var, value);
            self.insert_var(param.name.clone(), (var, param.ty.clone()));
        }
        for (index, local) in plan.locals.iter().enumerate() {
            let cl_ty = cl_type(&local.ty, self.ptr_ty)
                .ok_or_else(|| format!("async local `{}` has no native value", local.name))?;
            let offset = simple_async_frame_local_offset(plan, index, self.ptr_ty)
                .expect("async frame local offset");
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), frame, offset as i32);
            let var = self.builder.declare_var(cl_ty);
            self.builder.def_var(var, value);
            self.insert_var(local.name.clone(), (var, local.ty.clone()));
        }
        for (index, step) in plan.awaits.iter().enumerate() {
            if let Some(binding) = &step.binding {
                let cl_ty = cl_type(&binding.ty, self.ptr_ty).ok_or_else(|| {
                    format!("async binding `{}` has no native value", binding.name)
                })?;
                let offset = simple_async_frame_binding_offset(plan, index, self.ptr_ty)
                    .expect("async frame binding offset");
                let value = self
                    .builder
                    .ins()
                    .load(cl_ty, MemFlags::new(), frame, offset as i32);
                let var = self.builder.declare_var(cl_ty);
                self.builder.def_var(var, value);
                self.insert_var(binding.name.clone(), (var, binding.ty.clone()));
            }
        }

        let await_count = plan.awaits.len();
        self.async_poll_blocks = (0..await_count)
            .map(|_| self.builder.create_block())
            .collect();

        let body_start_block = self.builder.create_block();
        let invalid_state_block = self.builder.create_block();

        let dispatch_blocks: Vec<_> = (0..=await_count)
            .map(|_| self.builder.create_block())
            .collect();

        self.builder.ins().jump(dispatch_blocks[0], &[]);
        for state_index in 0..=await_count {
            self.builder.switch_to_block(dispatch_blocks[state_index]);
            let is_state =
                self.builder
                    .ins()
                    .icmp_imm(ir::condcodes::IntCC::Equal, state, state_index as i64);
            let target = if state_index == 0 {
                body_start_block
            } else {
                self.async_poll_blocks[state_index - 1]
            };
            let next = if state_index < await_count {
                dispatch_blocks[state_index + 1]
            } else {
                invalid_state_block
            };
            self.builder.ins().brif(is_state, target, &[], next, &[]);
        }

        self.builder.switch_to_block(body_start_block);
        self.builder.seal_block(body_start_block);

        self.emit_block(&f.body)?;

        if !self.terminated {
            let result_future = self.builder.ins().load(
                self.ptr_ty,
                MemFlags::new(),
                frame,
                ASYNC_FRAME_RESULT_OFFSET,
            );
            if let Ty::Future(inner) = &f.return_ty {
                let zero = self.zero_val(inner);
                self.emit_future_complete(result_future, inner, Some(zero))?;
            } else {
                self.emit_future_complete(result_future, &plan.inner_ty, None)?;
            }
            self.emit_scope_cleanup_calls_from(0, 0)?;
            self.emit_simple_async_frame_cleanup(plan, frame, await_count, true)?;
            self.emit_debug_leave();
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.ins().return_(&[zero]);
            self.terminated = true;
        }

        self.builder.switch_to_block(invalid_state_block);
        self.builder.seal_block(invalid_state_block);
        let result_future = self.builder.ins().load(
            self.ptr_ty,
            MemFlags::new(),
            frame,
            ASYNC_FRAME_RESULT_OFFSET,
        );
        let cancel_ref = *self
            .func_refs
            .get("ori_future_cancel")
            .ok_or_else(|| "missing runtime function `ori_future_cancel`".to_string())?;
        self.builder.ins().call(cancel_ref, &[result_future]);
        self.emit_simple_async_frame_cleanup(plan, frame, await_count, true)?;
        self.emit_debug_leave();
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().return_(&[zero]);

        for block in self.async_poll_blocks.iter() {
            let is_filled = self
                .builder
                .func
                .layout
                .block_insts(*block)
                .next()
                .is_some();
            if !is_filled {
                // The body terminated before this await was emitted (e.g. a
                // `return` in a previous branch), so this poll block has no
                // instructions yet. It is still a dispatch target and must be
                // filled to keep the CFG well-formed.
                self.builder.switch_to_block(*block);
                self.builder.ins().trap(ir::TrapCode::user(2).unwrap());
            }
            self.builder.seal_block(*block);
        }

        self.builder.seal_all_blocks();
        self.builder.finalize();
        Ok(())
    }

    fn emit_never_call_stmt(&mut self, expr: &HirExpr) -> Result<bool, String> {
        if !expr.ty.is_never() {
            return Ok(false);
        }
        let HirExprKind::Call { callee, args } = &expr.kind else {
            return Ok(false);
        };
        let HirExprKind::Var(name) = &callee.kind else {
            return Ok(false);
        };
        if !matches!(name.as_str(), "ori_panic" | "ori_test_fail") {
            return Ok(false);
        }

        let mut args_v = Vec::new();
        for arg in args {
            args_v.push(self.emit_expr_for_expected(&arg.value, &Ty::String)?);
        }
        if self.hosted_traps {
            let text = args_v.first().copied().unwrap_or(self.bytes_ptr(b"")?);
            self.emit_host_trap_return_value(2, text, false, true)?;
            return Ok(true);
        }
        self.emit_scope_cleanup_calls_from(0, 0)?;
        let fref = *self
            .func_refs
            .get(name.as_str())
            .ok_or_else(|| format!("missing runtime function `{name}`"))?;
        self.builder.ins().call(fref, &args_v);
        let trap = ir::TrapCode::user(2)
            .ok_or_else(|| "invalid panic fallback trap code `2`".to_string())?;
        self.builder.ins().trap(trap);
        self.terminated = true;
        Ok(true)
    }

    fn stmt_span(stmt: &HirStmt) -> Span {
        match stmt {
            HirStmt::Let { span, .. }
            | HirStmt::Assign { span, .. }
            | HirStmt::If { span, .. }
            | HirStmt::While { span, .. }
            | HirStmt::For { span, .. }
            | HirStmt::Loop { span, .. }
            | HirStmt::Repeat { span, .. }
            | HirStmt::Match { span, .. }
            | HirStmt::IfSome { span, .. }
            | HirStmt::WhileSome { span, .. }
            | HirStmt::Using { span, .. }
            | HirStmt::Check { span, .. } => *span,
            HirStmt::Return(_, span) | HirStmt::Break(span) | HirStmt::Continue(span) => *span,
            HirStmt::Expr(e) => e.span,
        }
    }

    fn line_from_byte_offset(&self, offset: u32) -> u32 {
        let starts = self.debug_line_starts;
        if starts.is_empty() {
            return 0;
        }
        let idx = starts.partition_point(|&s| s <= offset).saturating_sub(1);
        (idx as u32) + 1
    }

    fn emit_debug_enter(&mut self) {
        let Some(name_gv) = self.debug_function_gv else {
            return;
        };
        let Some(&enter_ref) = self.func_refs.get("ori_debug_enter") else {
            return;
        };
        let name_ptr = self.builder.ins().global_value(self.ptr_ty, name_gv);
        let name_len = self
            .builder
            .ins()
            .iconst(types::I32, self.func_name.len() as i64);
        self.builder.ins().call(enter_ref, &[name_ptr, name_len]);
    }

    fn emit_debug_leave(&mut self) {
        let Some(&leave_ref) = self.func_refs.get("ori_debug_leave") else {
            return;
        };
        if self.debug_function_gv.is_some() {
            self.builder.ins().call(leave_ref, &[]);
        }
    }

    fn emit_debug_scalar_value(&mut self, value: ir::Value, ty: &Ty) -> Option<ir::Value> {
        let value_type = self.builder.func.dfg.value_type(value);
        match debug_scalar_type_tag(ty)? {
            DEBUG_TYPE_FLOAT => Some(if value_type == types::I64 {
                value
            } else {
                self.builder
                    .ins()
                    .bitcast(types::I64, MemFlags::new(), value)
            }),
            DEBUG_TYPE_FLOAT32 => {
                let bits = self
                    .builder
                    .ins()
                    .bitcast(types::I32, MemFlags::new(), value);
                Some(self.builder.ins().uextend(types::I64, bits))
            }
            DEBUG_TYPE_UINT | DEBUG_TYPE_BOOL => Some(if value_type == types::I64 {
                value
            } else {
                self.builder.ins().uextend(types::I64, value)
            }),
            DEBUG_TYPE_INT => Some(if value_type == types::I64 {
                value
            } else {
                self.builder.ins().sextend(types::I64, value)
            }),
            _ => None,
        }
    }

    fn emit_debug_raw_value(&mut self, value: ir::Value) -> ir::Value {
        let value_type = self.builder.func.dfg.value_type(value);
        if value_type == types::I64 {
            value
        } else if value_type.is_int() {
            self.builder.ins().uextend(types::I64, value)
        } else {
            self.builder
                .ins()
                .bitcast(types::I64, MemFlags::new(), value)
        }
    }

    fn is_debug_managed_ty(&self, ty: &Ty) -> bool {
        debug_scalar_type_tag(ty).is_none() && cl_type(ty, self.ptr_ty) == Some(self.ptr_ty)
    }

    fn debug_managed_kind(ty: &Ty) -> Option<i64> {
        match ty {
            Ty::String => Some(DEBUG_MANAGED_STRING),
            Ty::Bytes => Some(DEBUG_MANAGED_BYTES),
            _ => None,
        }
    }

    fn emit_debug_variable_value(&mut self, name: &str, value: ir::Value, ty: &Ty) -> bool {
        let Some(&variable_ref) = self.func_refs.get("ori_debug_variable") else {
            return false;
        };
        let Some(name_gv) = self.debug_variable_gvs.get(name).copied() else {
            return false;
        };
        let Some(type_tag) = debug_scalar_type_tag(ty)
            .or_else(|| self.is_debug_managed_ty(ty).then_some(DEBUG_TYPE_MANAGED))
        else {
            return false;
        };
        let raw_value = debug_scalar_type_tag(ty)
            .and_then(|_| self.emit_debug_scalar_value(value, ty))
            .unwrap_or_else(|| self.emit_debug_raw_value(value));
        let name_ptr = self.builder.ins().global_value(self.ptr_ty, name_gv);
        let name_len = self.builder.ins().iconst(types::I32, name.len() as i64);
        let type_tag = self.builder.ins().iconst(types::I32, type_tag);
        self.builder
            .ins()
            .call(variable_ref, &[name_ptr, name_len, type_tag, raw_value]);
        true
    }

    /// Ask the runtime to render a bounded preview for an immutable managed
    /// payload. The pointer stays inside the runtime; only display text is
    /// sent to the debugger adapter.
    fn emit_debug_managed_value(&mut self, name: &str, value: ir::Value, ty: &Ty) -> bool {
        let Some(kind) = Self::debug_managed_kind(ty) else {
            return false;
        };
        let Some(&variable_ref) = self.func_refs.get("ori_debug_managed") else {
            return false;
        };
        let Some(name_gv) = self.debug_variable_gvs.get(name).copied() else {
            return false;
        };
        let name_ptr = self.builder.ins().global_value(self.ptr_ty, name_gv);
        let name_len = self.builder.ins().iconst(types::I32, name.len() as i64);
        let kind = self.builder.ins().iconst(types::I32, kind);
        self.builder
            .ins()
            .call(variable_ref, &[name_ptr, name_len, kind, value]);
        true
    }

    fn emit_debug_list_metadata(&mut self, name: &str, list: ir::Value) {
        let Some(&len_ref) = self.func_refs.get("ori_list_len") else {
            return;
        };
        let Some(&capacity_ref) = self.func_refs.get("ori_list_capacity") else {
            return;
        };
        let len_call = self.builder.ins().call(len_ref, &[list]);
        let capacity_call = self.builder.ins().call(capacity_ref, &[list]);
        let len = self.builder.inst_results(len_call)[0];
        let capacity = self.builder.inst_results(capacity_call)[0];
        let length_name = format!("{name}.length");
        let capacity_name = format!("{name}.capacity");
        self.emit_debug_variable_value(&length_name, len, &Ty::Int);
        self.emit_debug_variable_value(&capacity_name, capacity, &Ty::Int);
    }

    fn emit_debug_list_element(&mut self, name: &str, stored: ir::Value, elem_ty: &Ty, depth: u8) {
        if debug_scalar_type_tag(elem_ty).is_some() {
            let value = if matches!(elem_ty, Ty::Float32) {
                self.builder.ins().ireduce(types::I32, stored)
            } else {
                self.decode_list_storage_value(stored, elem_ty)
            };
            self.emit_debug_variable_value(name, value, elem_ty);
            return;
        }

        if self.is_debug_managed_ty(elem_ty) {
            let value = if self.ptr_ty == types::I64 {
                stored
            } else {
                self.builder.ins().ireduce(self.ptr_ty, stored)
            };
            self.emit_debug_aggregate_snapshot(name, value, elem_ty, depth + 1);
        }
    }

    /// Materialize a bounded, index-qualified view of a list for the current
    /// stopped event. Every `ori_list_get` call is guarded by the live length,
    /// so a short or null list cannot trigger a bounds abort in a debug probe.
    fn emit_debug_list_elements(&mut self, name: &str, list: ir::Value, elem_ty: &Ty, depth: u8) {
        let Some(&len_ref) = self.func_refs.get("ori_list_len") else {
            return;
        };
        let Some(&get_ref) = self.func_refs.get("ori_list_get") else {
            return;
        };
        let len_call = self.builder.ins().call(len_ref, &[list]);
        let len = self.builder.inst_results(len_call)[0];
        let first_test = self.builder.create_block();
        let done = self.builder.create_block();
        self.builder.ins().jump(first_test, &[]);

        let max_elements = if depth == 0 {
            DEBUG_MAX_LIST_ELEMENTS
        } else {
            DEBUG_MAX_NESTED_LIST_ELEMENTS
        };
        let mut test = first_test;
        for index in 0..max_elements {
            let element = self.builder.create_block();
            let next_test = if index + 1 < max_elements {
                self.builder.create_block()
            } else {
                done
            };

            self.builder.switch_to_block(test);
            let index_value = self.builder.ins().iconst(types::I64, index as i64);
            let present =
                self.builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::SignedGreaterThan, len, index_value);
            self.builder
                .ins()
                .brif(present, element, &[], next_test, &[]);
            self.builder.seal_block(test);

            self.builder.switch_to_block(element);
            self.terminated = false;
            let stored_call = self.builder.ins().call(get_ref, &[list, index_value]);
            let stored = self.builder.inst_results(stored_call)[0];
            let element_name = format!("{name}[{index}]");
            self.emit_debug_list_element(&element_name, stored, elem_ty, depth);
            if !self.terminated {
                self.builder.ins().jump(next_test, &[]);
            }
            self.builder.seal_block(element);
            test = next_test;
        }

        self.builder.seal_block(done);
        self.builder.switch_to_block(done);
        self.terminated = false;
    }

    fn emit_debug_optional_snapshot(
        &mut self,
        name: &str,
        value: ir::Value,
        inner_ty: &Ty,
        depth: u8,
    ) {
        let tag = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), value, 0);
        let one = self.builder.ins().iconst(types::I8, 1);
        let has_value = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, tag, one);
        let has_name = format!("{name}.has_value");
        self.emit_debug_variable_value(&has_name, has_value, &Ty::Bool);

        let zero = self.builder.ins().iconst(self.ptr_ty, 0);
        let non_null = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
        let present = self.builder.ins().band(has_value, non_null);
        let some_block = self.builder.create_block();
        let done_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(present, some_block, &[], done_block, &[]);
        self.builder.seal_block(some_block);
        self.builder.switch_to_block(some_block);
        self.terminated = false;
        if let Some(inner_cl_ty) = cl_type(inner_ty, self.ptr_ty) {
            let (offset, _) = optional_layout(inner_ty, self.ptr_ty);
            let inner = self
                .builder
                .ins()
                .load(inner_cl_ty, MemFlags::new(), value, offset as i32);
            self.emit_debug_aggregate_snapshot(
                &format!("{name}.value"),
                inner,
                inner_ty,
                depth + 1,
            );
        }
        if !self.terminated {
            self.builder.ins().jump(done_block, &[]);
        }
        self.builder.seal_block(done_block);
        self.builder.switch_to_block(done_block);
        self.terminated = false;
    }

    fn emit_debug_result_snapshot(
        &mut self,
        name: &str,
        value: ir::Value,
        ok_ty: &Ty,
        err_ty: &Ty,
        depth: u8,
    ) {
        let tag = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), value, 0);
        let one = self.builder.ins().iconst(types::I8, 1);
        let is_ok = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, tag, one);
        self.emit_debug_variable_value(&format!("{name}.is_ok"), is_ok, &Ty::Bool);

        let zero = self.builder.ins().iconst(self.ptr_ty, 0);
        let non_null = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
        let active = self.builder.ins().band(non_null, is_ok);
        let ok_block = self.builder.create_block();
        let error_test = self.builder.create_block();
        let error_block = self.builder.create_block();
        let done_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(active, ok_block, &[], error_test, &[]);

        self.builder.seal_block(ok_block);
        self.builder.switch_to_block(ok_block);
        self.terminated = false;
        if let Some(ok_cl_ty) = cl_type(ok_ty, self.ptr_ty) {
            let offset = result_layout(ok_ty, err_ty, self.ptr_ty).0;
            let payload = self
                .builder
                .ins()
                .load(ok_cl_ty, MemFlags::new(), value, offset as i32);
            self.emit_debug_aggregate_snapshot(&format!("{name}.ok"), payload, ok_ty, depth + 1);
        }
        if !self.terminated {
            self.builder.ins().jump(done_block, &[]);
        }

        self.builder.seal_block(error_test);
        self.builder.switch_to_block(error_test);
        self.terminated = false;
        let zero_tag = self.builder.ins().iconst(types::I8, 0);
        let is_error = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, tag, zero_tag);
        self.builder
            .ins()
            .brif(is_error, error_block, &[], done_block, &[]);

        self.builder.seal_block(error_block);
        self.builder.switch_to_block(error_block);
        self.terminated = false;
        if let Some(err_cl_ty) = cl_type(err_ty, self.ptr_ty) {
            let offset = result_layout(ok_ty, err_ty, self.ptr_ty).0;
            let payload = self
                .builder
                .ins()
                .load(err_cl_ty, MemFlags::new(), value, offset as i32);
            self.emit_debug_aggregate_snapshot(
                &format!("{name}.error"),
                payload,
                err_ty,
                depth + 1,
            );
        }
        if !self.terminated {
            self.builder.ins().jump(done_block, &[]);
        }
        self.builder.seal_block(done_block);
        self.builder.switch_to_block(done_block);
        self.terminated = false;
    }

    fn emit_debug_tuple_snapshot(
        &mut self,
        name: &str,
        value: ir::Value,
        elements: &[Ty],
        depth: u8,
    ) {
        let zero = self.builder.ins().iconst(self.ptr_ty, 0);
        let non_null = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
        let fields_block = self.builder.create_block();
        let done_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(non_null, fields_block, &[], done_block, &[]);
        self.builder.seal_block(fields_block);
        self.builder.switch_to_block(fields_block);
        self.terminated = false;
        let (layout, _, _) = tuple_layout(elements, self.ptr_ty);
        for (index, (offset, elem_ty)) in layout.iter().enumerate() {
            let Some(elem_cl_ty) = cl_type(elem_ty, self.ptr_ty) else {
                continue;
            };
            let elem = self
                .builder
                .ins()
                .load(elem_cl_ty, MemFlags::new(), value, *offset as i32);
            self.emit_debug_aggregate_snapshot(
                &format!("{name}.{index}"),
                elem,
                elem_ty,
                depth + 1,
            );
        }
        if !self.terminated {
            self.builder.ins().jump(done_block, &[]);
        }
        self.builder.seal_block(done_block);
        self.builder.switch_to_block(done_block);
        self.terminated = false;
    }

    fn emit_debug_enum_snapshot(
        &mut self,
        name: &str,
        value: ir::Value,
        layout: &EnumLayout,
        depth: u8,
    ) {
        let tag = self
            .builder
            .ins()
            .load(types::I32, MemFlags::new(), value, 0);
        self.emit_debug_variable_value(&format!("{name}.tag"), tag, &Ty::Int);
        let zero = self.builder.ins().iconst(self.ptr_ty, 0);
        let non_null = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
        let first_test = self.builder.create_block();
        let done_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(non_null, first_test, &[], done_block, &[]);

        let mut variants = layout.variants.iter().collect::<Vec<_>>();
        variants.sort_by_key(|(_, variant)| variant.tag);
        let mut test_block = first_test;
        for (variant_name, variant) in variants {
            let body_block = self.builder.create_block();
            let next_block = self.builder.create_block();
            self.builder.switch_to_block(test_block);
            let expected = self
                .builder
                .ins()
                .iconst(types::I32, i64::from(variant.tag));
            let matches = self
                .builder
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, tag, expected);
            self.builder
                .ins()
                .brif(matches, body_block, &[], next_block, &[]);
            self.builder.seal_block(test_block);

            self.builder.switch_to_block(body_block);
            self.terminated = false;
            for (field_name, field) in &variant.fields.fields {
                let Some(field_cl_ty) = cl_type(&field.ty, self.ptr_ty) else {
                    continue;
                };
                let offset = layout.payload_offset + field.offset;
                let field_value =
                    self.builder
                        .ins()
                        .load(field_cl_ty, MemFlags::new(), value, offset as i32);
                self.emit_debug_aggregate_snapshot(
                    &format!("{name}.{variant_name}.{field_name}"),
                    field_value,
                    &field.ty,
                    depth + 1,
                );
            }
            if !self.terminated {
                self.builder.ins().jump(done_block, &[]);
            }
            self.builder.seal_block(body_block);
            test_block = next_block;
        }
        self.builder.switch_to_block(test_block);
        self.builder.ins().jump(done_block, &[]);
        self.builder.seal_block(test_block);
        self.builder.seal_block(done_block);
        self.builder.switch_to_block(done_block);
        self.terminated = false;
    }

    fn emit_debug_collection_metadata(
        &mut self,
        name: &str,
        value: ir::Value,
        length_name: &str,
        capacity_name: &str,
    ) {
        if let Some(&length_ref) = self.func_refs.get(length_name) {
            let length_call = self.builder.ins().call(length_ref, &[value]);
            let length = self.builder.inst_results(length_call)[0];
            self.emit_debug_variable_value(&format!("{name}.length"), length, &Ty::Int);
        }
        if let Some(&capacity_ref) = self.func_refs.get(capacity_name) {
            let capacity_call = self.builder.ins().call(capacity_ref, &[value]);
            let capacity = self.builder.inst_results(capacity_call)[0];
            self.emit_debug_variable_value(&format!("{name}.capacity"), capacity, &Ty::Int);
        }
    }

    fn emit_debug_collection_list_view(
        &mut self,
        name: &str,
        value: ir::Value,
        to_list_name: &str,
        elem_ty: &Ty,
        depth: u8,
    ) {
        let Some(&to_list_ref) = self.func_refs.get(to_list_name) else {
            return;
        };
        let list_call = self.builder.ins().call(to_list_ref, &[value]);
        let list = self.builder.inst_results(list_call)[0];
        self.emit_debug_aggregate_snapshot(
            name,
            list,
            &Ty::List(Box::new(elem_ty.clone())),
            depth + 1,
        );
        if let Some(&release_ref) = self.func_refs.get("ori_arc_release") {
            self.builder.ins().call(release_ref, &[list]);
        }
    }

    /// Emit a useful first-level view of a managed value. Struct fields are
    /// loaded using the same native layout as ordinary field access; lists
    /// expose safe length/capacity metadata. Nested structs recurse with a
    /// small depth cap, while opaque managed payloads remain summarized.
    fn emit_debug_aggregate_snapshot(&mut self, name: &str, value: ir::Value, ty: &Ty, depth: u8) {
        if depth >= 4 {
            return;
        }
        if !Self::debug_managed_kind(ty)
            .is_some_and(|_| self.emit_debug_managed_value(name, value, ty))
        {
            self.emit_debug_variable_value(name, value, ty);
        }
        match ty {
            Ty::List(elem_ty) => {
                self.emit_debug_list_metadata(name, value);
                self.emit_debug_list_elements(name, value, elem_ty, depth);
            }
            Ty::Optional(inner_ty) => {
                self.emit_debug_optional_snapshot(name, value, inner_ty, depth);
            }
            Ty::Result(ok_ty, err_ty) => {
                self.emit_debug_result_snapshot(name, value, ok_ty, err_ty, depth);
            }
            Ty::Tuple(elements) => {
                self.emit_debug_tuple_snapshot(name, value, elements, depth);
            }
            Ty::Named(def_id, args) => {
                let Some(layout) = self.struct_layouts.get(def_id).cloned() else {
                    if let Some(enum_layout) = self.enum_layouts.get(def_id).cloned() {
                        self.emit_debug_enum_snapshot(name, value, &enum_layout, depth);
                    }
                    return;
                };
                // Struct values are normally non-null ARC allocations, but a
                // zeroed global or a foreign handle can still present a null
                // pointer. Keep a debugger probe from turning that into a
                // target crash while preserving the summary variable above.
                let zero = self.builder.ins().iconst(self.ptr_ty, 0);
                let non_null = self
                    .builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
                let fields_block = self.builder.create_block();
                let done_block = self.builder.create_block();
                self.builder
                    .ins()
                    .brif(non_null, fields_block, &[], done_block, &[]);
                self.builder.seal_block(fields_block);
                self.builder.switch_to_block(fields_block);
                self.terminated = false;
                for (field_name, field) in layout.fields {
                    let field_ty = substitute_ty_params(&field.ty, args);
                    let Some(field_cl_ty) = cl_type(&field_ty, self.ptr_ty) else {
                        continue;
                    };
                    let field_value = self.builder.ins().load(
                        field_cl_ty,
                        MemFlags::new(),
                        value,
                        field.offset as i32,
                    );
                    let field_path = format!("{name}.{field_name}");
                    if debug_scalar_type_tag(&field_ty).is_some() {
                        self.emit_debug_variable_value(&field_path, field_value, &field_ty);
                    } else {
                        self.emit_debug_aggregate_snapshot(
                            &field_path,
                            field_value,
                            &field_ty,
                            depth + 1,
                        );
                    }
                }
                if !self.terminated {
                    self.builder.ins().jump(done_block, &[]);
                }
                self.builder.seal_block(done_block);
                self.builder.switch_to_block(done_block);
                self.terminated = false;
            }
            Ty::Map(key_ty, value_ty) => {
                self.emit_debug_collection_metadata(name, value, "ori_map_len", "ori_map_capacity");
                self.emit_debug_collection_list_view(
                    &format!("{name}.keys"),
                    value,
                    "ori_map_keys",
                    key_ty,
                    depth,
                );
                self.emit_debug_collection_list_view(
                    &format!("{name}.values"),
                    value,
                    "ori_map_values",
                    value_ty,
                    depth,
                );
            }
            Ty::Set(elem_ty) => {
                self.emit_debug_collection_metadata(name, value, "ori_set_len", "ori_set_capacity");
                self.emit_debug_collection_list_view(
                    &format!("{name}.items"),
                    value,
                    "ori_set_to_list",
                    elem_ty,
                    depth,
                );
            }
            Ty::Opaque { kind, args } => {
                let Some(elem_ty) = args.first() else {
                    return;
                };
                let (length_name, capacity_name, to_list_name) = match kind {
                    OpaqueTy::Deque => ("ori_deque_len", "ori_deque_capacity", "ori_deque_to_list"),
                    OpaqueTy::Queue => ("ori_queue_len", "ori_queue_capacity", "ori_queue_to_list"),
                    OpaqueTy::Stack => ("ori_stack_len", "ori_stack_capacity", "ori_stack_to_list"),
                    OpaqueTy::LinkedList => (
                        "ori_linked_list_len",
                        "ori_linked_list_capacity",
                        "ori_linked_list_to_list",
                    ),
                    OpaqueTy::DoublyLinkedList => (
                        "ori_doubly_linked_list_len",
                        "ori_doubly_linked_list_capacity",
                        "ori_doubly_linked_list_to_list",
                    ),
                    OpaqueTy::Heap => ("ori_heap_len", "ori_heap_capacity", "ori_heap_to_list"),
                    _ => return,
                };
                self.emit_debug_collection_metadata(name, value, length_name, capacity_name);
                self.emit_debug_collection_list_view(
                    &format!("{name}.items"),
                    value,
                    to_list_name,
                    elem_ty,
                    depth,
                );
            }
            _ => {}
        }
    }

    fn emit_debug_variable_snapshot(&mut self) {
        if !self.func_refs.contains_key("ori_debug_variable") {
            return;
        }
        if self.debug_function_gv.is_none() {
            return;
        }

        let mut visible = Vec::new();
        let mut seen = HashSet::new();
        for scope in self.vars.iter().rev() {
            for (name, (variable, ty)) in scope {
                if seen.insert(name.clone()) && self.debug_variable_gvs.contains_key(name) {
                    visible.push((name.clone(), *variable, ty.clone()));
                }
            }
        }

        for (name, variable, ty) in visible {
            let value = self.builder.use_var(variable);
            if debug_scalar_type_tag(&ty).is_some() {
                self.emit_debug_variable_value(&name, value, &ty);
            } else {
                self.emit_debug_aggregate_snapshot(&name, value, &ty, 0);
            }
        }
    }

    fn emit_debug_line_probe(&mut self, span: Span) {
        let Some(gv) = self.debug_file_gv else {
            return;
        };
        if self.debug_file_len == 0 || self.debug_line_starts.is_empty() {
            return;
        }
        let Some(&fref) = self.func_refs.get("ori_debug_line") else {
            return;
        };
        let line = self.line_from_byte_offset(span.start);
        if line == 0 {
            return;
        }
        let ptr = self.builder.ins().global_value(self.ptr_ty, gv);
        let len = self
            .builder
            .ins()
            .iconst(types::I32, i64::from(self.debug_file_len));
        let line_v = self.builder.ins().iconst(types::I32, i64::from(line));
        self.emit_debug_variable_snapshot();
        self.builder.ins().call(fref, &[ptr, len, line_v]);
    }

    fn emit_stmt(&mut self, stmt: &HirStmt) -> Result<(), String> {
        if !self.terminated {
            self.emit_debug_line_probe(Self::stmt_span(stmt));
        }
        match stmt {
            HirStmt::Let {
                name, ty, value, ..
            } => {
                let value_is_owned = Self::expr_produces_owned_ref(value);
                let val = self.emit_expr_for_expected(value, ty)?;
                if let Some(cl_ty) = cl_type(ty, self.ptr_ty) {
                    // A same-named binding from an ENCLOSING scope (e.g. an
                    // outer match arm or if-some still on the scope stack)
                    // must not be reused if its native type differs — the
                    // Cranelift Variable was declared with that type, and
                    // def_var with a different type panics internally. Only
                    // reuse when the type matches; otherwise shadow with a
                    // fresh Variable (matches normal lexical shadowing).
                    let var = self
                        .lookup_var(name)
                        .filter(|(_, existing_ty)| existing_ty == ty)
                        .map(|(v, _)| v)
                        .unwrap_or_else(|| {
                            let v = self.builder.declare_var(cl_ty);
                            self.insert_var(name.clone(), (v, ty.clone()));
                            v
                        });
                    self.builder.def_var(var, val);
                    if is_managed_ty(ty) {
                        if !value_is_owned {
                            self.emit_arc_retain_if_managed(ty, val)?;
                        }
                        self.managed_stack.push(ManagedCleanup {
                            var,
                            ty: ty.clone(),
                        });
                    }
                    self.store_async_local_if_any(name, val)?;
                }
            }
            HirStmt::Assign { lvalue, value, .. } => {
                if let HirLValue::Var(name) = lvalue {
                    if let Some((var, ty)) = self.lookup_var(name) {
                        let value_is_owned = Self::expr_produces_owned_ref(value);
                        let val = self.emit_expr_for_expected(value, &ty)?;
                        let old = self.builder.use_var(var);
                        if !value_is_owned {
                            self.emit_arc_retain_if_managed(&ty, val)?;
                        }
                        self.emit_arc_release_if_managed(&ty, old)?;
                        self.builder.def_var(var, val);
                        self.store_async_local_if_any(name, val)?;
                    } else {
                        let val = if let Some(info) = self.global_data.get(name).cloned() {
                            let value_is_owned = Self::expr_produces_owned_ref(value);
                            let val = self.emit_expr_for_expected(value, &info.ty)?;
                            if info.mutable && is_managed_ty(&info.ty) {
                                if let Some(old) = self.load_global(name) {
                                    if !value_is_owned {
                                        self.emit_arc_retain_if_managed(&info.ty, val)?;
                                    }
                                    self.emit_arc_release_if_managed(&info.ty, old)?;
                                }
                            }
                            val
                        } else {
                            self.emit_expr(value)?
                        };
                        self.store_global(name, val);
                    }
                } else if let HirLValue::Index { base, index } = lvalue {
                    let value_is_owned = Self::expr_produces_owned_ref(value);
                    let val = self.emit_expr(value)?;
                    let (container, container_ty) = self.emit_lvalue_value(base)?;
                    if let Ty::List(elem_ty) = container_ty {
                        let idx = self.emit_expr(index)?;
                        if is_managed_ty(&elem_ty) {
                            let get_ref = *self.func_refs.get("ori_list_get").ok_or_else(|| {
                                "missing runtime function `ori_list_get`".to_string()
                            })?;
                            let old_call = self.builder.ins().call(get_ref, &[container, idx]);
                            let old = self.builder.inst_results(old_call)[0];
                            let old = self.decode_list_storage_value(old, &elem_ty);
                            self.emit_arc_update_edge_if_managed(&elem_ty, container, old, val)?;
                        }
                        let stored = self.encode_list_storage_value(val, &elem_ty);
                        let set_ref = *self
                            .func_refs
                            .get("ori_list_set")
                            .ok_or_else(|| "missing runtime function `ori_list_set`".to_string())?;
                        self.builder.ins().call(set_ref, &[container, idx, stored]);
                        // The edge owns the new element's +1; drop an owned
                        // temporary's own +1 (borrowed refs stay untouched).
                        if value_is_owned && is_managed_ty(&elem_ty) {
                            self.emit_arc_release_if_managed(&elem_ty, val)?;
                        }
                    } else if let Ty::Array(elem_ty, _) = container_ty {
                        // Inline storage: store straight at `base + i * stride`.
                        // Scalar elements have no edge to update (managed ones
                        // are rejected in the checker); inline struct elements
                        // are copied recursively (GFX-INLINE-1).
                        let idx = self.emit_expr(index)?;
                        let (elem_size, _) = self.field_size_align_for_layout(&elem_ty);
                        let stride = self.builder.ins().iconst(types::I64, elem_size as i64);
                        let byte_offset = self.builder.ins().imul(idx, stride);
                        let offset = if self.ptr_ty == types::I64 {
                            byte_offset
                        } else {
                            self.builder.ins().ireduce(self.ptr_ty, byte_offset)
                        };
                        let addr = self.builder.ins().iadd(container, offset);
                        let inline_struct = matches!(elem_ty.as_ref(), Ty::Named(..))
                            && self.ty_is_inline_for_layout(&elem_ty);
                        if inline_struct {
                            self.emit_copy_inline_into(val, &elem_ty, addr, 0)?;
                        } else {
                            self.builder.ins().store(MemFlags::new(), val, addr, 0);
                        }
                    } else if let Ty::Map(key_ty, value_ty) = container_ty {
                        // `m["k"] = v` used to fall through this chain and emit
                        // nothing at all: the assignment silently did not happen.
                        let key_value = self.emit_expr(index)?;
                        let set_symbol = if matches!(&*key_ty, Ty::String) {
                            "ori_map_set_string"
                        } else {
                            "ori_map_set"
                        };
                        let Some(&set_ref) = self.func_refs.get(set_symbol) else {
                            return Err(format!("missing runtime function `{set_symbol}`"));
                        };
                        let kv = self.encode_list_storage_value(key_value, &key_ty);
                        let vv = self.encode_list_storage_value(val, &value_ty);
                        self.builder.ins().call(set_ref, &[container, kv, vv]);
                        self.emit_arc_register_edge_if_managed(&key_ty, container, key_value)?;
                        self.emit_arc_register_edge_if_managed(&value_ty, container, val)?;
                        // The map edge owns the value's +1; drop an owned
                        // temporary's own +1 so it does not leak.
                        if value_is_owned && is_managed_ty(&value_ty) {
                            self.emit_arc_release_if_managed(&value_ty, val)?;
                        }
                    } else {
                        // Falling through used to emit nothing at all, so
                        // `xs[i] = v` on an unsupported container silently did
                        // nothing. Fail loudly instead.
                        return Err(format!(
                            "native index-assignment codegen is missing for type `{}`",
                            self.display_ty(&container_ty)
                        ));
                    }
                } else if let HirLValue::Field { base, field } = lvalue {
                    let value_is_owned = Self::expr_produces_owned_ref(value);
                    let (addr, field_layout, owner) = self.emit_field_lvalue_addr(base, field)?;
                    let val = self.emit_expr_for_expected(value, &field_layout.ty)?;
                    if let Some(contract) = &field_layout.contract {
                        let message = format!(
                            "{CONTRACT_FIELD_VIOLATION}: field `{field}` broke its `if` contract"
                        );
                        self.emit_value_contract(
                            &field_layout.ty,
                            val,
                            contract,
                            3,
                            false,
                            Some(&message),
                        )?;
                    }
                    let cl_ty = cl_type(&field_layout.ty, self.ptr_ty)
                        .ok_or_else(|| format!("missing Cranelift type for field `{field}`"))?;
                    let old = self.builder.ins().load(cl_ty, MemFlags::new(), addr, 0);
                    self.emit_arc_update_edge_if_managed(&field_layout.ty, owner, old, val)?;
                    self.builder.ins().store(MemFlags::new(), val, addr, 0);
                    // The edge owns the new value's +1; drop an owned
                    // temporary's own +1 (borrowed refs stay untouched).
                    if value_is_owned && is_managed_ty(&field_layout.ty) {
                        self.emit_arc_release_if_managed(&field_layout.ty, val)?;
                    }
                }
            }
            HirStmt::Return(val, _) => self.emit_return(val.as_ref())?,
            HirStmt::Break(_) => {
                if let Some(ctx) = self.current_loop() {
                    self.emit_scope_cleanup_calls_from(
                        ctx.cleanup_start,
                        ctx.managed_cleanup_start,
                    )?;
                    self.builder.ins().jump(ctx.break_target, &[]);
                    self.broke_loop_target = Some(ctx.break_target);
                    self.terminated = true;
                }
            }
            HirStmt::Continue(_) => {
                if let Some(ctx) = self.current_loop() {
                    self.emit_scope_cleanup_calls_from(
                        ctx.cleanup_start,
                        ctx.managed_cleanup_start,
                    )?;
                    self.builder.ins().jump(ctx.continue_target, &[]);
                    self.terminated = true;
                }
            }
            HirStmt::Expr(e) => {
                if !self.emit_never_call_stmt(e)? {
                    let v = self.emit_expr(e)?;
                    // Discarded managed temporaries (e.g. a struct literal
                    // or a Call returning a managed value used as a
                    // statement) must be released so the +1 from creation
                    // does not leak. Borrowed refs (Var/Field) are owned by
                    // their binding and are released by scope cleanup.
                    if is_managed_ty(&e.ty) && Self::expr_produces_owned_ref(e) {
                        self.emit_arc_release_if_managed(&e.ty, v)?;
                    }
                }
            }

            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                self.emit_if(cond, then, else_ifs, else_.as_ref())?;
            }
            HirStmt::While { cond, body, .. } => self.emit_while(cond, body)?,
            HirStmt::Loop { body, .. } => self.emit_loop(body)?,
            HirStmt::For {
                binding,
                index_binding,
                elem_ty,
                iterable,
                body,
                ..
            } => {
                let has_await = self.async_frame.is_some() && stmt_contains_await(stmt);
                self.emit_for(
                    binding,
                    index_binding.as_ref(),
                    elem_ty,
                    iterable,
                    body,
                    has_await,
                )?;
            }
            HirStmt::Match {
                scrutinee, arms, ..
            } => self.emit_match(scrutinee, arms)?,
            HirStmt::IfSome {
                kind,
                binding,
                inner_ty,
                value,
                then,
                else_,
                ..
            } => {
                self.emit_if_some(*kind, binding, inner_ty, value, then, else_.as_ref())?;
            }
            HirStmt::WhileSome {
                binding,
                inner_ty,
                value,
                body,
                ..
            } => {
                self.emit_while_some(binding, inner_ty, value, body)?;
            }
            HirStmt::Using {
                name, ty, value, ..
            } => {
                let value_is_owned = Self::expr_produces_owned_ref(value);
                let val = self.emit_expr_for_expected(value, ty)?;
                if let Some(cl_ty) = cl_type(ty, self.ptr_ty) {
                    // A same-named binding from an ENCLOSING scope (e.g. an
                    // outer match arm or if-some still on the scope stack)
                    // must not be reused if its native type differs — the
                    // Cranelift Variable was declared with that type, and
                    // def_var with a different type panics internally. Only
                    // reuse when the type matches; otherwise shadow with a
                    // fresh Variable (matches normal lexical shadowing).
                    let var = self
                        .lookup_var(name)
                        .filter(|(_, existing_ty)| existing_ty == ty)
                        .map(|(v, _)| v)
                        .unwrap_or_else(|| {
                            let v = self.builder.declare_var(cl_ty);
                            self.insert_var(name.clone(), (v, ty.clone()));
                            v
                        });
                    self.builder.def_var(var, val);
                    if is_managed_ty(ty) {
                        if !value_is_owned {
                            self.emit_arc_retain_if_managed(ty, val)?;
                        }
                        self.managed_stack.push(ManagedCleanup {
                            var,
                            ty: ty.clone(),
                        });
                    }
                    self.using_stack.push(UsingCleanup {
                        var,
                        ty: ty.clone(),
                    });
                    self.store_async_local_if_any(name, val)?;
                }
            }
            HirStmt::Check {
                condition, message, ..
            } => {
                let cv = self.emit_expr(condition)?;
                let trap = ir::TrapCode::user(1)
                    .ok_or_else(|| "invalid runtime check trap code `1`".to_string())?;
                // A silent trap (SIGILL, no output) is useless to whoever hit
                // it: route the failure through `ori_panic` so the message —
                // or at least the fact that a `check` failed — reaches stderr.
                let text = match message {
                    Some(m) => format!("check failed: {m}"),
                    None => "check failed".to_string(),
                };
                self.emit_trap_unless_with_message(cv, trap, true, Some(&text))?;
            }
            HirStmt::Repeat { count, body, .. } => {
                let has_await = self.async_frame.is_some() && stmt_contains_await(stmt);
                let count_v = self.emit_expr(count)?;
                let zero = self.builder.ins().iconst(types::I64, 0);
                let non_negative = self.builder.ins().icmp(
                    ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    count_v,
                    zero,
                );
                let trap = ir::TrapCode::user(4)
                    .ok_or_else(|| "invalid repeat-count trap code `4`".to_string())?;
                self.emit_trap_unless(non_negative, trap, true)?;

                let (idx_var, limit_val, repeat_loop_id) = if has_await {
                    let loop_id = self.async_loop_index;
                    self.async_loop_index += 1;
                    let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
                    let limit_name = SmolStr::new(format!(".__loop_limit_{}", loop_id));
                    let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
                    let (limit_var, _) = self.lookup_var(&limit_name).unwrap();
                    let loop_zero = self.builder.ins().iconst(types::I64, 0);
                    self.builder.def_var(idx_var, loop_zero);
                    self.builder.def_var(limit_var, count_v);
                    self.store_async_local_if_any(&idx_name, loop_zero)?;
                    self.store_async_local_if_any(&limit_name, count_v)?;
                    (idx_var, self.builder.use_var(limit_var), Some(loop_id))
                } else {
                    let idx_var = self.builder.declare_var(types::I64);
                    let loop_zero = self.builder.ins().iconst(types::I64, 0);
                    self.builder.def_var(idx_var, loop_zero);
                    (idx_var, count_v, None)
                };

                let header = self.builder.create_block();
                let body_b = self.builder.create_block();
                let exit = self.builder.create_block();

                self.builder.ins().jump(header, &[]);
                self.builder.switch_to_block(header);

                let cur = self.builder.use_var(idx_var);
                let cond =
                    self.builder
                        .ins()
                        .icmp(ir::condcodes::IntCC::SignedLessThan, cur, limit_val);
                self.builder.ins().brif(cond, body_b, &[], exit, &[]);

                self.builder.seal_block(body_b);
                self.builder.switch_to_block(body_b);
                self.terminated = false;
                self.push_loop(header, exit);
                self.emit_block(body)?;
                self.pop_loop();
                if !self.terminated {
                    let cur2 = self.builder.use_var(idx_var);
                    let one = self.builder.ins().iconst(types::I64, 1);
                    let next = self.builder.ins().iadd(cur2, one);
                    self.builder.def_var(idx_var, next);
                    if let Some(loop_id) = repeat_loop_id {
                        let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
                        self.store_async_local_if_any(&idx_name, next)?;
                    }
                    self.builder.ins().jump(header, &[]);
                }
                self.terminated = false;
                self.builder.seal_block(header);
                self.builder.seal_block(exit);
                self.builder.switch_to_block(exit);
                self.terminated = false;
            }
        }
        Ok(())
    }

    // == Control flow ==

    fn emit_unreachable_return(&mut self) -> Result<(), String> {
        // Fill an unreachable block with a trap. This is safe for both sync
        // and async functions: the real paths never reach it, and it keeps
        // the CFG well-formed without assuming a return ABI.
        let trap = ir::TrapCode::user(2).unwrap();
        self.builder.ins().trap(trap);
        self.terminated = true;
        Ok(())
    }

    fn emit_if(
        &mut self,
        cond: &HirExpr,
        then: &HirBlock,
        else_ifs: &[(HirExpr, HirBlock)],
        else_: Option<&HirBlock>,
    ) -> Result<(), String> {
        let then_block = self.builder.create_block();
        let merge_block = self.builder.create_block();

        let else_target = if else_ifs.is_empty() && else_.is_none() {
            merge_block
        } else {
            self.builder.create_block()
        };

        let cv = self.emit_expr(cond)?;
        self.builder
            .ins()
            .brif(cv, then_block, &[], else_target, &[]);

        // then branch
        self.builder.seal_block(then_block);
        self.builder.switch_to_block(then_block);
        self.terminated = false;
        self.emit_block(then)?;
        if !self.terminated {
            self.builder.ins().jump(merge_block, &[]);
        }

        // else / else-if branch
        if else_target != merge_block {
            self.builder.seal_block(else_target);
            self.builder.switch_to_block(else_target);
            self.terminated = false;
            if !else_ifs.is_empty() {
                self.emit_if(&else_ifs[0].0, &else_ifs[0].1, &else_ifs[1..], else_)?;
            } else if let Some(eb) = else_ {
                self.emit_block(eb)?;
            }
            if !self.terminated {
                // The nested elif chain may have left the cursor in a different
                // block; jump to this branch's own merge from wherever it is.
                self.builder.ins().jump(merge_block, &[]);
            }
        }

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        Ok(())
    }

    fn emit_while(&mut self, cond: &HirExpr, body: &HirBlock) -> Result<(), String> {
        let cond_blk = self.builder.create_block();
        let body_blk = self.builder.create_block();
        let exit_blk = self.builder.create_block();

        // Fall through to condition check
        if !self.terminated {
            self.builder.ins().jump(cond_blk, &[]);
        }
        self.terminated = false;

        // Condition block — do NOT seal yet (back-edge from body not known yet)
        self.builder.switch_to_block(cond_blk);
        let cv = self.emit_expr(cond)?;
        self.builder.ins().brif(cv, body_blk, &[], exit_blk, &[]);

        // Body block — only cond_blk jumps here, safe to seal immediately
        self.builder.seal_block(body_blk);
        self.builder.switch_to_block(body_blk);
        self.push_loop(cond_blk, exit_blk);
        self.emit_block(body)?;
        self.pop_loop();
        let body_terminated = self.terminated;
        if !self.terminated {
            self.builder.ins().jump(cond_blk, &[]);
        }
        self.terminated = false;

        // Seal cond_blk NOW — both predecessors (entry and back-edge) are known
        self.builder.seal_block(cond_blk);

        // Exit block — only cond_blk branches here
        self.builder.seal_block(exit_blk);
        self.builder.switch_to_block(exit_blk);
        if body_terminated {
            // The body ended with a `break` to this loop (the exit already has
            // a jump from the body) or with a `return` (the exit has no
            // predecessor). In the return case the exit must still be filled so
            // the CFG stays well-formed. In the break case the exit already has
            // a terminator, so continue on a fresh block for whatever follows
            // the loop.
            let was_break = self.broke_loop_target == Some(exit_blk);
            if !was_break {
                self.emit_unreachable_return()?;
            } else {
                let continue_block = self.builder.create_block();
                self.builder.ins().jump(continue_block, &[]);
                self.builder.seal_block(continue_block);
                self.builder.switch_to_block(continue_block);
                self.terminated = false;
            }
        } else {
            self.terminated = false;
        }
        Ok(())
    }

    fn emit_loop(&mut self, body: &HirBlock) -> Result<(), String> {
        let header = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        self.terminated = false;
        self.push_loop(header, exit);
        self.emit_block(body)?;
        self.pop_loop();
        let body_terminated = self.terminated;
        if !self.terminated {
            self.builder.ins().jump(header, &[]);
        }
        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        if body_terminated {
            let was_break = self.broke_loop_target == Some(exit);
            if !was_break {
                self.emit_unreachable_return()?;
            } else {
                let continue_block = self.builder.create_block();
                self.builder.ins().jump(continue_block, &[]);
                self.builder.seal_block(continue_block);
                self.builder.switch_to_block(continue_block);
                self.terminated = false;
            }
        } else {
            self.terminated = false;
        }
        Ok(())
    }

    fn emit_if_some(
        &mut self,
        kind: ori_ast::stmt::UnwrapKind,
        binding: &SmolStr,
        inner_ty: &Ty,
        value: &HirExpr,
        then: &HirBlock,
        else_: Option<&HirBlock>,
    ) -> Result<(), String> {
        use ori_ast::stmt::UnwrapKind;
        // 1. Emit the optional expression (returns a pointer to the stack struct)
        // A fresh owned optional (not a bound Var) has no binding to release
        // it and payload extraction below is a plain load (a borrow); retain
        // the extracted payload and release the wrapper once bound, on both
        // branches, mirroring emit_match's scrutinee handling. This needs a
        // real else block on both paths (even without a user `else`) so the
        // release has somewhere to run before falling into merge_blk.
        let value_owned = Self::expr_produces_owned_ref(value) && is_managed_ty(&value.ty);
        let opt_ptr = self.emit_expr(value)?;
        // 2. Read the discriminant (byte 0): `has_value` for optional,
        // `is_ok` for result. Both wrappers put it at offset 0.
        let flag = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), opt_ptr, 0);
        let then_blk = self.builder.create_block();
        let merge_blk = self.builder.create_block();
        let else_blk = if else_.is_some() || value_owned {
            self.builder.create_block()
        } else {
            merge_blk
        };
        // `if err(e)` binds the error side, so it takes the branch when the
        // result is NOT ok — same discriminant, inverted targets.
        let (true_blk, false_blk) = match kind {
            UnwrapKind::Some | UnwrapKind::Ok => (then_blk, else_blk),
            UnwrapKind::Err => (else_blk, then_blk),
        };
        self.builder.ins().brif(flag, true_blk, &[], false_blk, &[]);
        // then block: bind inner value
        self.builder.seal_block(then_blk);
        self.builder.switch_to_block(then_blk);
        self.terminated = false;
        self.push_scope();
        let managed_cleanup_start = self.managed_stack.len();
        if let Some(cl_ty) = cl_type(inner_ty, self.ptr_ty) {
            // Payload offset differs per wrapper: optional puts the value
            // after the flag; result puts one union at the payload offset,
            // shared by the ok and err sides.
            let val_off = match (kind, &value.ty) {
                (UnwrapKind::Ok | UnwrapKind::Err, Ty::Result(ok, err)) => {
                    result_layout(ok, err, self.ptr_ty).0
                }
                // Type checking rejects other shapes; the optional layout is
                // a harmless fallback during error recovery.
                _ => optional_layout(inner_ty, self.ptr_ty).0,
            };
            let inner_val =
                self.builder
                    .ins()
                    .load(cl_ty, MemFlags::new(), opt_ptr, val_off as i32);
            // A same-named binding from an ENCLOSING scope (e.g. an
            // outer match arm or if-some still on the scope stack)
            // must not be reused if its native type differs — the
            // Cranelift Variable was declared with that type, and
            // def_var with a different type panics internally. Only
            // reuse when the type matches; otherwise shadow with a
            // fresh Variable (matches normal lexical shadowing).
            let var = self
                .lookup_var(binding)
                .filter(|(_, existing_ty)| existing_ty == inner_ty)
                .map(|(v, _)| v)
                .unwrap_or_else(|| {
                    let v = self.builder.declare_var(cl_ty);
                    self.insert_var(binding.clone(), (v, inner_ty.clone()));
                    v
                });
            self.builder.def_var(var, inner_val);
            if value_owned && is_managed_ty(inner_ty) {
                self.emit_arc_retain_if_managed(inner_ty, inner_val)?;
                // Register for scope-exit release (or return-transfer
                // elision) below — mirrors emit_match's bind_pattern.
                self.managed_stack.push(ManagedCleanup {
                    var,
                    ty: inner_ty.clone(),
                });
            }
            self.store_async_local_if_any(binding, inner_val)?;
        }
        if value_owned {
            self.emit_arc_release_if_managed(&value.ty, opt_ptr)?;
        }
        self.emit_block(then)?;
        if !self.terminated {
            self.emit_managed_cleanup_calls_from(managed_cleanup_start)?;
        }
        self.managed_stack.truncate(managed_cleanup_start);
        self.pop_scope();
        if !self.terminated {
            self.builder.ins().jump(merge_blk, &[]);
        }
        // else block: runs on the has_val==false path. Always entered when
        // `value_owned` (even without a user `else`) so the "none" wrapper
        // gets released; otherwise only entered for a user-provided `else`.
        if else_.is_some() || value_owned {
            self.builder.seal_block(else_blk);
            self.builder.switch_to_block(else_blk);
            self.terminated = false;
            // `none` case: no payload to retain, still release the wrapper.
            if value_owned {
                self.emit_arc_release_if_managed(&value.ty, opt_ptr)?;
            }
            if let Some(eb) = else_ {
                self.emit_block(eb)?;
            }
            if !self.terminated {
                self.builder.ins().jump(merge_blk, &[]);
            }
        }
        self.builder.seal_block(merge_blk);
        self.builder.switch_to_block(merge_blk);
        self.terminated = false;
        Ok(())
    }

    fn emit_while_some(
        &mut self,
        binding: &SmolStr,
        inner_ty: &Ty,
        value: &HirExpr,
        body: &HirBlock,
    ) -> Result<(), String> {
        self.push_scope();
        let header_blk = self.builder.create_block();
        let body_blk = self.builder.create_block();
        let exit_blk = self.builder.create_block();
        if !self.terminated {
            self.builder.ins().jump(header_blk, &[]);
        }
        self.terminated = false;
        // Header: evaluate optional expression, check has_value
        self.builder.switch_to_block(header_blk);
        let opt_ptr = self.emit_expr(value)?;
        let has_val = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), opt_ptr, 0);
        self.builder
            .ins()
            .brif(has_val, body_blk, &[], exit_blk, &[]);
        // Body: extract inner value, run body
        self.builder.seal_block(body_blk);
        self.builder.switch_to_block(body_blk);
        self.terminated = false;
        if let Some(cl_ty) = cl_type(inner_ty, self.ptr_ty) {
            let (val_off, _) = optional_layout(inner_ty, self.ptr_ty);
            let inner_val =
                self.builder
                    .ins()
                    .load(cl_ty, MemFlags::new(), opt_ptr, val_off as i32);
            // A same-named binding from an ENCLOSING scope (e.g. an
            // outer match arm or if-some still on the scope stack)
            // must not be reused if its native type differs — the
            // Cranelift Variable was declared with that type, and
            // def_var with a different type panics internally. Only
            // reuse when the type matches; otherwise shadow with a
            // fresh Variable (matches normal lexical shadowing).
            let var = self
                .lookup_var(binding)
                .filter(|(_, existing_ty)| existing_ty == inner_ty)
                .map(|(v, _)| v)
                .unwrap_or_else(|| {
                    let v = self.builder.declare_var(cl_ty);
                    self.insert_var(binding.clone(), (v, inner_ty.clone()));
                    v
                });
            self.builder.def_var(var, inner_val);
            self.store_async_local_if_any(binding, inner_val)?;
        }
        self.push_loop(header_blk, exit_blk);
        self.emit_block(body)?;
        self.pop_loop();
        let body_terminated = self.terminated;
        if !self.terminated {
            self.builder.ins().jump(header_blk, &[]);
        }
        self.terminated = false;
        // Seal header after back-edge is known
        self.builder.seal_block(header_blk);
        self.builder.seal_block(exit_blk);
        self.builder.switch_to_block(exit_blk);
        if body_terminated {
            let was_break = self.broke_loop_target == Some(exit_blk);
            if !was_break {
                self.emit_unreachable_return()?;
            } else {
                let continue_block = self.builder.create_block();
                self.builder.ins().jump(continue_block, &[]);
                self.builder.seal_block(continue_block);
                self.builder.switch_to_block(continue_block);
                self.terminated = false;
            }
        } else {
            self.terminated = false;
        }
        self.pop_scope();
        Ok(())
    }

    fn emit_for(
        &mut self,
        binding: &SmolStr,
        index_binding: Option<&SmolStr>,
        elem_ty: &Ty,
        iterable: &HirExpr,
        body: &HirBlock,
        has_await: bool,
    ) -> Result<(), String> {
        self.push_scope();
        let input = ForLoopInput {
            binding,
            index_binding,
            iterable,
            body,
            has_await,
        };
        let res = match &iterable.kind {
            HirExprKind::Range { start, end } => self.emit_for_range(input, elem_ty, start, end),
            _ if matches!(&iterable.ty, Ty::List(_)) => {
                self.emit_for_list(binding, index_binding, elem_ty, iterable, body, has_await)
            }
            _ if matches!(&iterable.ty, Ty::Set(_)) => {
                // Set is backed by OriList internally — same get/len interface
                self.emit_for_list(binding, index_binding, elem_ty, iterable, body, has_await)
            }
            _ if matches!(&iterable.ty, Ty::Map(_, _)) => {
                let Ty::Map(key_ty, value_ty) = &iterable.ty else {
                    unreachable!();
                };
                self.emit_for_map(input, key_ty, value_ty)
            }
            _ if matches!(&iterable.ty, Ty::String) => {
                self.emit_for_string(binding, index_binding, iterable, body, has_await)
            }
            _ if matches!(&iterable.ty, Ty::Bytes) => {
                self.emit_for_bytes(binding, index_binding, iterable, body, has_await)
            }
            _ if matches!(&iterable.ty, Ty::Opaque { kind, .. } if kind.is_list_backed_collection()) =>
            {
                let Ty::Opaque { kind, args } = &iterable.ty else {
                    unreachable!();
                };
                let elem = args.first().cloned().unwrap_or(Ty::Infer(0));
                self.emit_for_opaque(input, &elem, *kind)
            }
            _ if matches!(
                &iterable.ty,
                Ty::Opaque {
                    kind: OpaqueTy::Heap,
                    ..
                }
            ) =>
            {
                let Ty::Opaque { args, .. } = &iterable.ty else {
                    unreachable!();
                };
                let elem = args.first().cloned().unwrap_or(Ty::Infer(0));
                self.emit_for_opaque(input, &elem, OpaqueTy::Heap)
            }
            _ if matches!(
                &iterable.ty,
                Ty::Opaque {
                    kind: OpaqueTy::Graph,
                    ..
                }
            ) =>
            {
                let Ty::Opaque { args, .. } = &iterable.ty else {
                    unreachable!();
                };
                let elem = args.first().cloned().unwrap_or(Ty::Infer(0));
                self.emit_for_opaque(input, &elem, OpaqueTy::Graph)
            }
            _ if matches!(
                &iterable.ty,
                Ty::Opaque {
                    kind: OpaqueTy::HashTable,
                    ..
                }
            ) =>
            {
                let Ty::Opaque { args, .. } = &iterable.ty else {
                    unreachable!();
                };
                let fref = *self
                    .func_refs
                    .get("ori_hash_table_keys")
                    .ok_or_else(|| "missing runtime function `ori_hash_table_keys`".to_string())?;
                let handle = self.emit_expr(iterable)?;
                let call = self.builder.ins().call(fref, &[handle]);
                let snapshot = self.builder.inst_results(call)[0];
                let elem = args.first().cloned().unwrap_or(Ty::Infer(0));
                self.emit_for_list_value(binding, index_binding, &elem, snapshot, body, has_await)?;
                let free_ref = *self
                    .func_refs
                    .get("ori_list_free")
                    .ok_or_else(|| "missing runtime function `ori_list_free`".to_string())?;
                self.builder.ins().call(free_ref, &[snapshot]);
                Ok(())
            }
            _ => {
                if let Some(next_func) = self.iterable_next_func_name_for_type(&iterable.ty) {
                    self.emit_for_custom_iterable(input, elem_ty, &next_func)
                } else {
                    Err(native_codegen_unsupported(format!(
                        "`for` iterable type `{}`",
                        self.display_ty(&iterable.ty)
                    )))
                }
            }
        };
        self.pop_scope();
        res
    }

    fn emit_for_opaque(
        &mut self,
        input: ForLoopInput<'_>,
        elem_ty: &Ty,
        kind: OpaqueTy,
    ) -> Result<(), String> {
        let ForLoopInput {
            binding,
            index_binding,
            iterable,
            body,
            has_await,
        } = input;
        let prefix = match kind {
            OpaqueTy::Deque => "ori_deque_iterator",
            OpaqueTy::Queue => "ori_queue_iterator",
            OpaqueTy::Stack => "ori_stack_iterator",
            OpaqueTy::LinkedList => "ori_linked_list_iterator",
            OpaqueTy::DoublyLinkedList => "ori_doubly_linked_list_iterator",
            OpaqueTy::Heap => "ori_heap_iterator",
            OpaqueTy::Graph => "ori_graph_iterator",
            _ => unreachable!(),
        };
        let new_func = format!("{}_new", prefix);
        let next_func = format!("{}_next", prefix);

        let new_ref = *self
            .func_refs
            .get(new_func.as_str())
            .ok_or_else(|| format!("missing runtime function `{new_func}`"))?;
        let next_ref = *self
            .func_refs
            .get(next_func.as_str())
            .ok_or_else(|| format!("missing runtime function `{next_func}`"))?;

        let elem_cl_ty = cl_type(elem_ty, self.ptr_ty).ok_or_else(|| {
            format!(
                "missing Cranelift type for element type `{}`",
                self.display_ty(elem_ty)
            )
        })?;

        let handle = self.emit_expr(iterable)?;
        let call = self.builder.ins().call(new_ref, &[handle]);
        let iter_value = self.builder.inst_results(call)[0];

        let (idx_var, iter_var, _iter_val, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (iter_var, _) = self.lookup_var(&list_name).unwrap();

            self.builder.def_var(iter_var, iter_value);
            self.store_async_local_if_any(&list_name, iter_value)?;
            self.emit_arc_register_edge_if_managed(
                &iterable.ty,
                self.async_frame.unwrap(),
                iter_value,
            )?;

            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            self.store_async_local_if_any(&idx_name, zero)?;

            (
                idx_var,
                iter_var,
                self.builder.use_var(iter_var),
                Some(loop_id),
            )
        } else {
            let iter_var = self.builder.declare_var(self.ptr_ty);
            self.builder.def_var(iter_var, iter_value);
            let idx_var = self.builder.declare_var(types::I64);
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            (idx_var, iter_var, iter_value, None)
        };

        let binding_var = if has_await {
            let (binding_var, _) = self.lookup_var(binding).unwrap();
            binding_var
        } else {
            let binding_var = self.builder.declare_var(elem_cl_ty);
            self.insert_var(binding.clone(), (binding_var, elem_ty.clone()));
            binding_var
        };

        let iter_cleanup_idx = self.managed_stack.len();
        self.managed_stack.push(ManagedCleanup {
            var: iter_var,
            ty: iterable.ty.clone(),
        });

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();

        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);

        let iter_current = self.builder.use_var(iter_var);
        let call = self.builder.ins().call(next_ref, &[iter_current]);
        let val_ptr = self.builder.inst_results(call)[0];
        let has_val = self
            .builder
            .ins()
            .icmp_imm(ir::condcodes::IntCC::NotEqual, val_ptr, 0);
        self.builder.ins().brif(has_val, body_b, &[], exit, &[]);

        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);

        let item_storage = self
            .builder
            .ins()
            .load(elem_cl_ty, MemFlags::new(), val_ptr, 0);
        let item = self.decode_list_storage_value(item_storage, elem_ty);
        self.builder.def_var(binding_var, item);
        if has_await {
            self.store_async_local_if_any(binding, item)?;
            self.emit_arc_register_edge_if_managed(elem_ty, self.async_frame.unwrap(), item)?;
        }

        if let Some(ib_name) = index_binding {
            let cur = self.builder.use_var(idx_var);
            if has_await {
                let (ib_var, _) = self.lookup_var(ib_name).unwrap();
                self.builder.def_var(ib_var, cur);
                self.store_async_local_if_any(ib_name, cur)?;
            } else {
                let ib_var = self.builder.declare_var(types::I64);
                self.builder.def_var(ib_var, cur);
                self.insert_var(ib_name.clone(), (ib_var, Ty::Int));
            }
        }

        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;

        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur, one);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);

        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        let final_iter = self.builder.use_var(iter_var);
        self.emit_arc_release_if_managed(&iterable.ty, final_iter)?;
        if let Some(loop_id) = loop_id {
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            self.emit_arc_unregister_edge(self.async_frame.unwrap(), final_iter)?;
            self.store_async_local_if_any(&list_name, zero)?;
        }
        self.managed_stack.truncate(iter_cleanup_idx);
        self.terminated = false;

        Ok(())
    }

    fn emit_for_custom_iterable(
        &mut self,
        input: ForLoopInput<'_>,
        elem_ty: &Ty,
        next_func: &SmolStr,
    ) -> Result<(), String> {
        let ForLoopInput {
            binding,
            index_binding,
            iterable,
            body,
            has_await,
        } = input;
        let Some(iter_cl_ty) = cl_type(&iterable.ty, self.ptr_ty) else {
            return Err(native_codegen_unsupported(format!(
                "`for` iterable type `{}`",
                self.display_ty(&iterable.ty)
            )));
        };
        let Some(elem_cl_ty) = cl_type(elem_ty, self.ptr_ty) else {
            return Err(native_codegen_unsupported(format!(
                "`for` element type `{}`",
                self.display_ty(elem_ty)
            )));
        };
        let next_ref = *self.func_refs.get(next_func.as_str()).ok_or_else(|| {
            format!("missing function reference `{next_func}` for custom Iterable")
        })?;
        let iter_value = self.emit_expr(iterable)?;
        self.emit_arc_retain_if_managed(&iterable.ty, iter_value)?;

        let (idx_var, iter_var, _iter_val, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (iter_var, _) = self.lookup_var(&list_name).unwrap();

            self.builder.def_var(iter_var, iter_value);
            self.store_async_local_if_any(&list_name, iter_value)?;
            self.emit_arc_register_edge_if_managed(
                &iterable.ty,
                self.async_frame.unwrap(),
                iter_value,
            )?;

            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            self.store_async_local_if_any(&idx_name, zero)?;

            (
                idx_var,
                iter_var,
                self.builder.use_var(iter_var),
                Some(loop_id),
            )
        } else {
            let iter_var = self.builder.declare_var(iter_cl_ty);
            self.builder.def_var(iter_var, iter_value);
            let idx_var = self.builder.declare_var(types::I64);
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            (idx_var, iter_var, iter_value, None)
        };

        let binding_var = if has_await {
            let (binding_var, _) = self.lookup_var(binding).unwrap();
            binding_var
        } else {
            let binding_var = self.builder.declare_var(elem_cl_ty);
            self.insert_var(binding.clone(), (binding_var, elem_ty.clone()));
            binding_var
        };

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let iter_current = self.builder.use_var(iter_var);
        self.emit_arc_retain_if_managed(&iterable.ty, iter_current)?;
        let call = self.builder.ins().call(next_ref, &[iter_current]);
        let opt_ptr = self.builder.inst_results(call)[0];
        let has_val = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), opt_ptr, 0);
        let has_val_bool = self
            .builder
            .ins()
            .icmp_imm(ir::condcodes::IntCC::NotEqual, has_val, 0);
        let idx_current = self.builder.use_var(idx_var);
        let idx_non_negative = self.builder.ins().icmp_imm(
            ir::condcodes::IntCC::SignedGreaterThanOrEqual,
            idx_current,
            0,
        );
        let keep_going = self.builder.ins().band(has_val_bool, idx_non_negative);
        self.builder.ins().brif(keep_going, body_b, &[], exit, &[]);

        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);
        let (val_off, _) = optional_layout(elem_ty, self.ptr_ty);
        let item = self
            .builder
            .ins()
            .load(elem_cl_ty, MemFlags::new(), opt_ptr, val_off as i32);
        self.builder.def_var(binding_var, item);
        if has_await {
            self.store_async_local_if_any(binding, item)?;
            self.emit_arc_register_edge_if_managed(elem_ty, self.async_frame.unwrap(), item)?;
        }
        if let Some(ib_name) = index_binding {
            let cur = self.builder.use_var(idx_var);
            if has_await {
                let (ib_var, _) = self.lookup_var(ib_name).unwrap();
                self.builder.def_var(ib_var, cur);
                self.store_async_local_if_any(ib_name, cur)?;
            } else {
                let ib_var = self.builder.declare_var(types::I64);
                self.builder.def_var(ib_var, cur);
                self.insert_var(ib_name.clone(), (ib_var, Ty::Int));
            }
        }
        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;
        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur, one);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        let final_iter = self.builder.use_var(iter_var);
        self.emit_arc_release_if_managed(&iterable.ty, final_iter)?;
        if let Some(loop_id) = loop_id {
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            self.emit_arc_unregister_edge(self.async_frame.unwrap(), final_iter)?;
            self.store_async_local_if_any(&list_name, zero)?;
        }
        self.terminated = false;
        Ok(())
    }

    fn emit_for_range(
        &mut self,
        input: ForLoopInput<'_>,
        elem_ty: &Ty,
        start: &HirExpr,
        end: &HirExpr,
    ) -> Result<(), String> {
        let ForLoopInput {
            binding,
            index_binding,
            body,
            has_await,
            ..
        } = input;
        let start_v = self.emit_expr(start)?;
        let end_v = self.emit_expr(end)?;
        let asc =
            self.builder
                .ins()
                .icmp(ir::condcodes::IntCC::SignedLessThanOrEqual, start_v, end_v);

        let (idx_var, end_var, asc_var, iter_count_var, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let end_name = SmolStr::new(format!(".__loop_end_{}", loop_id));
            let asc_name = SmolStr::new(format!(".__loop_asc_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (end_var, _) = self.lookup_var(&end_name).unwrap();
            let (asc_var, _) = self.lookup_var(&asc_name).unwrap();

            let iter_count_var = if index_binding.is_some() {
                let iter_name = SmolStr::new(format!(".__loop_iter_{}", loop_id));
                let (v, _) = self.lookup_var(&iter_name).unwrap();
                let zero = self.builder.ins().iconst(types::I64, 0);
                self.builder.def_var(v, zero);
                self.store_async_local_if_any(&iter_name, zero)?;
                Some(v)
            } else {
                None
            };

            self.builder.def_var(idx_var, start_v);
            self.builder.def_var(end_var, end_v);
            self.builder.def_var(asc_var, asc);
            self.store_async_local_if_any(&idx_name, start_v)?;
            self.store_async_local_if_any(&end_name, end_v)?;
            self.store_async_local_if_any(&asc_name, asc)?;

            (idx_var, end_var, asc_var, iter_count_var, Some(loop_id))
        } else {
            let idx_var = self.builder.declare_var(types::I64);
            self.builder.def_var(idx_var, start_v);
            let end_var = self.builder.declare_var(types::I64);
            self.builder.def_var(end_var, end_v);
            let asc_var = self.builder.declare_var(types::I8);
            self.builder.def_var(asc_var, asc);

            let iter_count_var = if index_binding.is_some() {
                let v = self.builder.declare_var(types::I64);
                let zero = self.builder.ins().iconst(types::I64, 0);
                self.builder.def_var(v, zero);
                Some(v)
            } else {
                None
            };
            (idx_var, end_var, asc_var, iter_count_var, None)
        };

        if !has_await {
            if let Some(cl_ty) = cl_type(elem_ty, self.ptr_ty) {
                let bvar = self.builder.declare_var(cl_ty);
                self.insert_var(binding.clone(), (bvar, elem_ty.clone()));
            }
        }

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let cur = self.builder.use_var(idx_var);
        let lim = self.builder.use_var(end_var);
        let asc_flag = self.builder.use_var(asc_var);
        let cond_asc =
            self.builder
                .ins()
                .icmp(ir::condcodes::IntCC::SignedLessThanOrEqual, cur, lim);
        let cond_desc =
            self.builder
                .ins()
                .icmp(ir::condcodes::IntCC::SignedGreaterThanOrEqual, cur, lim);
        let cond = self.builder.ins().select(asc_flag, cond_asc, cond_desc);
        self.builder.ins().brif(cond, body_b, &[], exit, &[]);
        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);

        if let Some((bvar, _)) = self.lookup_var(binding) {
            let cur2 = self.builder.use_var(idx_var);
            self.builder.def_var(bvar, cur2);
            if has_await {
                self.store_async_local_if_any(binding, cur2)?;
            }
        }

        if let Some(ib_name) = index_binding {
            if let Some(ic_var) = iter_count_var {
                let ic = self.builder.use_var(ic_var);
                if has_await {
                    let (ib_var, _) = self.lookup_var(ib_name).unwrap();
                    self.builder.def_var(ib_var, ic);
                    self.store_async_local_if_any(ib_name, ic)?;
                } else {
                    let ib_var = self.builder.declare_var(types::I64);
                    self.builder.def_var(ib_var, ic);
                    self.insert_var(ib_name.clone(), (ib_var, Ty::Int));
                }
            }
        }

        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;
        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur2 = self.builder.use_var(idx_var);
        let asc_flag = self.builder.use_var(asc_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let neg_one = self.builder.ins().iconst(types::I64, -1);
        let inc = self.builder.ins().select(asc_flag, one, neg_one);
        let next = self.builder.ins().iadd(cur2, inc);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }

        if let Some(ic_var) = iter_count_var {
            let ic = self.builder.use_var(ic_var);
            let one_ic = self.builder.ins().iconst(types::I64, 1);
            let next_ic = self.builder.ins().iadd(ic, one_ic);
            self.builder.def_var(ic_var, next_ic);
            if let Some(loop_id) = loop_id {
                let iter_name = SmolStr::new(format!(".__loop_iter_{}", loop_id));
                self.store_async_local_if_any(&iter_name, next_ic)?;
            }
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        self.terminated = false;
        Ok(())
    }

    fn emit_for_list(
        &mut self,
        binding: &SmolStr,
        index_binding: Option<&SmolStr>,
        elem_ty: &Ty,
        iterable: &HirExpr,
        body: &HirBlock,
        has_await: bool,
    ) -> Result<(), String> {
        let list_v = self.emit_expr(iterable)?;
        self.emit_for_list_value(binding, index_binding, elem_ty, list_v, body, has_await)
    }

    /// Assign a for-loop element binding. `ori_list_get` / map accessors return
    /// borrowed managed pointers; retain the new element and release the previous
    /// iteration's retained value before overwriting the loop variable.
    fn emit_for_element_binding(
        &mut self,
        binding: &SmolStr,
        elem_ty: &Ty,
        elem: ir::Value,
        has_await: bool,
    ) -> Result<(), String> {
        let Some((bvar, _)) = self.lookup_var(binding) else {
            return Err(format!(
                "for-loop binding `{binding}` missing in native codegen"
            ));
        };
        if is_managed_ty(elem_ty) {
            let old = self.builder.use_var(bvar);
            self.emit_arc_retain_if_managed(elem_ty, elem)?;
            self.emit_arc_release_if_managed(elem_ty, old)?;
        }
        self.builder.def_var(bvar, elem);
        if has_await {
            self.store_async_local_if_any(binding, elem)?;
            if let Some(frame) = self.async_frame {
                self.emit_arc_register_edge_if_managed(elem_ty, frame, elem)?;
            }
        }
        Ok(())
    }

    fn emit_for_release_element_binding(&mut self, binding: &SmolStr) -> Result<(), String> {
        if let Some((bvar, bty)) = self.lookup_var(binding) {
            if is_managed_ty(&bty) {
                let final_val = self.builder.use_var(bvar);
                self.emit_arc_release_if_managed(&bty, final_val)?;
            }
        }
        Ok(())
    }

    fn emit_for_list_value(
        &mut self,
        binding: &SmolStr,
        index_binding: Option<&SmolStr>,
        elem_ty: &Ty,
        list_v: ir::Value,
        body: &HirBlock,
        has_await: bool,
    ) -> Result<(), String> {
        let len_ref = *self
            .func_refs
            .get("ori_list_len")
            .ok_or_else(|| "missing runtime function `ori_list_len`".to_string())?;
        let get_ref = *self
            .func_refs
            .get("ori_list_get")
            .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;

        let version_offset = (self.ptr_ty.bytes() + 16) as i32;
        let expected_version =
            self.builder
                .ins()
                .load(types::I64, MemFlags::new(), list_v, version_offset);

        let (idx_var, len_var, expected_ver_var, list_var, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let len_name = SmolStr::new(format!(".__loop_len_{}", loop_id));
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let ver_name = SmolStr::new(format!(".__loop_version_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (len_var, _) = self.lookup_var(&len_name).unwrap();
            let (list_var, _) = self.lookup_var(&list_name).unwrap();
            let (expected_ver_var, _) = self.lookup_var(&ver_name).unwrap();

            self.builder.def_var(list_var, list_v);
            self.store_async_local_if_any(&list_name, list_v)?;
            self.emit_arc_register_edge_if_managed(
                &Ty::List(Box::new(elem_ty.clone())),
                self.async_frame.unwrap(),
                list_v,
            )?;

            let len_call = self.builder.ins().call(len_ref, &[list_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            self.builder.def_var(len_var, len_v);
            self.store_async_local_if_any(&len_name, len_v)?;

            self.builder.def_var(expected_ver_var, expected_version);
            self.store_async_local_if_any(&ver_name, expected_version)?;

            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            self.store_async_local_if_any(&idx_name, zero)?;

            (
                idx_var,
                len_var,
                expected_ver_var,
                Some(list_var),
                Some(loop_id),
            )
        } else {
            let len_call = self.builder.ins().call(len_ref, &[list_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            let idx_var = self.builder.declare_var(types::I64);
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            let len_var = self.builder.declare_var(types::I64);
            self.builder.def_var(len_var, len_v);
            let expected_ver_var = self.builder.declare_var(types::I64);
            self.builder.def_var(expected_ver_var, expected_version);
            (idx_var, len_var, expected_ver_var, None, None)
        };

        if !has_await {
            if let Some(cl_ty) = cl_type(elem_ty, self.ptr_ty) {
                let bvar = self.builder.declare_var(cl_ty);
                let zero = self.builder.ins().iconst(cl_ty, 0);
                self.builder.def_var(bvar, zero);
                self.insert_var(binding.clone(), (bvar, elem_ty.clone()));
            }
        }

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let cur = self.builder.use_var(idx_var);
        let len = self.builder.use_var(len_var);
        let cond = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::SignedLessThan, cur, len);
        self.builder.ins().brif(cond, body_b, &[], exit, &[]);
        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);

        // Concurrent modification check
        let list_current = list_var
            .map(|var| self.builder.use_var(var))
            .unwrap_or(list_v);
        let current_version =
            self.builder
                .ins()
                .load(types::I64, MemFlags::new(), list_current, version_offset);
        let expected_ver_val = self.builder.use_var(expected_ver_var);
        let version_eq = self.builder.ins().icmp(
            ir::condcodes::IntCC::Equal,
            current_version,
            expected_ver_val,
        );

        let abort_b = self.builder.create_block();
        let check_b = self.builder.create_block();
        self.builder
            .ins()
            .brif(version_eq, check_b, &[], abort_b, &[]);

        self.builder.seal_block(abort_b);
        self.builder.switch_to_block(abort_b);
        if self.hosted_traps {
            self.emit_host_trap_return(7, Some("concurrent modification during iteration"), true)?;
        } else {
            let abort_ref = *self
                .func_refs
                .get("ori_abort_concurrent_modification")
                .ok_or_else(|| {
                    "missing runtime function `ori_abort_concurrent_modification`".to_string()
                })?;
            self.builder.ins().call(abort_ref, &[]);
            self.builder.ins().trap(ir::TrapCode::user(2).unwrap());
        }

        self.builder.seal_block(check_b);
        self.builder.switch_to_block(check_b);

        if let Some((_bvar, _)) = self.lookup_var(binding) {
            let cur2 = self.builder.use_var(idx_var);
            let list_current = list_var
                .map(|var| self.builder.use_var(var))
                .unwrap_or(list_v);
            let call = self.builder.ins().call(get_ref, &[list_current, cur2]);
            let elem = self.builder.inst_results(call)[0];
            let elem = self.decode_list_storage_value(elem, elem_ty);
            self.emit_for_element_binding(binding, elem_ty, elem, has_await)?;
        }

        if let Some(ib_name) = index_binding {
            let cur2 = self.builder.use_var(idx_var);
            if has_await {
                let (ib_var, _) = self.lookup_var(ib_name).unwrap();
                self.builder.def_var(ib_var, cur2);
                self.store_async_local_if_any(ib_name, cur2)?;
            } else {
                let ib_var = self.builder.declare_var(types::I64);
                self.builder.def_var(ib_var, cur2);
                self.insert_var(ib_name.clone(), (ib_var, Ty::Int));
            }
        }

        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        let body_terminated = self.terminated;
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;
        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur2 = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur2, one);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);

        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        if body_terminated {
            let was_break = self.broke_loop_target == Some(exit);
            if !was_break {
                self.emit_unreachable_return()?;
                return Ok(());
            }
            let continue_block = self.builder.create_block();
            self.builder.ins().jump(continue_block, &[]);
            self.builder.seal_block(continue_block);
            self.builder.switch_to_block(continue_block);
        }
        if !has_await {
            self.emit_for_release_element_binding(binding)?;
        }
        if let Some(loop_id) = loop_id {
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let ver_name = SmolStr::new(format!(".__loop_version_{}", loop_id));
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            let zero_i64 = self.builder.ins().iconst(types::I64, 0);
            let list_current = list_var
                .map(|var| self.builder.use_var(var))
                .unwrap_or(list_v);
            self.emit_arc_unregister_edge(self.async_frame.unwrap(), list_current)?;
            self.store_async_local_if_any(&list_name, zero)?;
            self.store_async_local_if_any(&ver_name, zero_i64)?;
        }
        self.terminated = false;
        Ok(())
    }

    fn emit_for_map(
        &mut self,
        input: ForLoopInput<'_>,
        key_ty: &Ty,
        value_ty: &Ty,
    ) -> Result<(), String> {
        let ForLoopInput {
            binding,
            index_binding: value_binding,
            iterable,
            body,
            has_await,
        } = input;
        let map_v = self.emit_expr(iterable)?;
        let len_ref = *self
            .func_refs
            .get("ori_map_len")
            .ok_or_else(|| "missing runtime function `ori_map_len`".to_string())?;
        let key_at_ref = *self
            .func_refs
            .get("ori_map_key_at")
            .ok_or_else(|| "missing runtime function `ori_map_key_at`".to_string())?;
        let value_at_ref = *self
            .func_refs
            .get("ori_map_value_at")
            .ok_or_else(|| "missing runtime function `ori_map_value_at`".to_string())?;

        let version_offset = (self.ptr_ty.bytes() * 2 + 16) as i32;
        let expected_version =
            self.builder
                .ins()
                .load(types::I64, MemFlags::new(), map_v, version_offset);

        let (idx_var, len_var, expected_ver_var, map_val, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let len_name = SmolStr::new(format!(".__loop_len_{}", loop_id));
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let ver_name = SmolStr::new(format!(".__loop_version_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (len_var, _) = self.lookup_var(&len_name).unwrap();
            let (list_var, _) = self.lookup_var(&list_name).unwrap();
            let (expected_ver_var, _) = self.lookup_var(&ver_name).unwrap();

            self.builder.def_var(list_var, map_v);
            self.store_async_local_if_any(&list_name, map_v)?;
            self.emit_arc_register_edge_if_managed(&iterable.ty, self.async_frame.unwrap(), map_v)?;

            let len_call = self.builder.ins().call(len_ref, &[map_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            self.builder.def_var(len_var, len_v);
            self.store_async_local_if_any(&len_name, len_v)?;

            self.builder.def_var(expected_ver_var, expected_version);
            self.store_async_local_if_any(&ver_name, expected_version)?;

            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            self.store_async_local_if_any(&idx_name, zero)?;

            (
                idx_var,
                len_var,
                expected_ver_var,
                self.builder.use_var(list_var),
                Some(loop_id),
            )
        } else {
            let len_call = self.builder.ins().call(len_ref, &[map_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            let idx_var = self.builder.declare_var(types::I64);
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            let len_var = self.builder.declare_var(types::I64);
            self.builder.def_var(len_var, len_v);
            let expected_ver_var = self.builder.declare_var(types::I64);
            self.builder.def_var(expected_ver_var, expected_version);
            (idx_var, len_var, expected_ver_var, map_v, None)
        };

        if !has_await {
            if let Some(cl_ty) = cl_type(key_ty, self.ptr_ty) {
                let key_var = self.builder.declare_var(cl_ty);
                let zero = self.builder.ins().iconst(cl_ty, 0);
                self.builder.def_var(key_var, zero);
                self.insert_var(binding.clone(), (key_var, key_ty.clone()));
            }
            if let Some(value_name) = value_binding {
                if let Some(cl_ty) = cl_type(value_ty, self.ptr_ty) {
                    let value_var = self.builder.declare_var(cl_ty);
                    let zero = self.builder.ins().iconst(cl_ty, 0);
                    self.builder.def_var(value_var, zero);
                    self.insert_var(value_name.clone(), (value_var, value_ty.clone()));
                }
            }
        }

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let cur = self.builder.use_var(idx_var);
        let len = self.builder.use_var(len_var);
        let cond = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::SignedLessThan, cur, len);
        self.builder.ins().brif(cond, body_b, &[], exit, &[]);
        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);

        // Concurrent modification check
        let current_version =
            self.builder
                .ins()
                .load(types::I64, MemFlags::new(), map_val, version_offset);
        let expected_ver_val = self.builder.use_var(expected_ver_var);
        let version_eq = self.builder.ins().icmp(
            ir::condcodes::IntCC::Equal,
            current_version,
            expected_ver_val,
        );

        let abort_b = self.builder.create_block();
        let check_b = self.builder.create_block();
        self.builder
            .ins()
            .brif(version_eq, check_b, &[], abort_b, &[]);

        self.builder.seal_block(abort_b);
        self.builder.switch_to_block(abort_b);
        if self.hosted_traps {
            self.emit_host_trap_return(7, Some("concurrent modification during iteration"), true)?;
        } else {
            let abort_ref = *self
                .func_refs
                .get("ori_abort_concurrent_modification")
                .ok_or_else(|| {
                    "missing runtime function `ori_abort_concurrent_modification`".to_string()
                })?;
            self.builder.ins().call(abort_ref, &[]);
            self.builder.ins().trap(ir::TrapCode::user(2).unwrap());
        }

        self.builder.seal_block(check_b);
        self.builder.switch_to_block(check_b);

        let cur2 = self.builder.use_var(idx_var);
        if let Some((_key_var, _)) = self.lookup_var(binding) {
            let key_call = self.builder.ins().call(key_at_ref, &[map_val, cur2]);
            let key = self.builder.inst_results(key_call)[0];
            let key = self.decode_list_storage_value(key, key_ty);
            self.emit_for_element_binding(binding, key_ty, key, has_await)?;
        }
        if let Some(value_name) = value_binding {
            if let Some((_value_var, _)) = self.lookup_var(value_name) {
                let value_call = self.builder.ins().call(value_at_ref, &[map_val, cur2]);
                let value = self.builder.inst_results(value_call)[0];
                let value = self.decode_list_storage_value(value, value_ty);
                self.emit_for_element_binding(value_name, value_ty, value, has_await)?;
            }
        }

        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        let body_terminated = self.terminated;
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;
        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur3 = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur3, one);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        if body_terminated {
            let was_break = self.broke_loop_target == Some(exit);
            if !was_break {
                self.emit_unreachable_return()?;
                return Ok(());
            }
            let continue_block = self.builder.create_block();
            self.builder.ins().jump(continue_block, &[]);
            self.builder.seal_block(continue_block);
            self.builder.switch_to_block(continue_block);
        }
        if !has_await {
            self.emit_for_release_element_binding(binding)?;
            if let Some(value_name) = value_binding {
                self.emit_for_release_element_binding(value_name)?;
            }
        }
        if let Some(loop_id) = loop_id {
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let ver_name = SmolStr::new(format!(".__loop_version_{}", loop_id));
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            let zero_i64 = self.builder.ins().iconst(types::I64, 0);
            self.emit_arc_unregister_edge(self.async_frame.unwrap(), map_val)?;
            self.store_async_local_if_any(&list_name, zero)?;
            self.store_async_local_if_any(&ver_name, zero_i64)?;
        }
        self.terminated = false;
        Ok(())
    }

    fn emit_for_bytes(
        &mut self,
        binding: &SmolStr,
        index_binding: Option<&SmolStr>,
        iterable: &HirExpr,
        body: &HirBlock,
        has_await: bool,
    ) -> Result<(), String> {
        let bytes_v = self.emit_expr(iterable)?;
        let len_ref = *self
            .func_refs
            .get("ori_bytes_len")
            .ok_or_else(|| "missing runtime function `ori_bytes_len`".to_string())?;
        let get_ref = *self
            .func_refs
            .get("ori_bytes_get")
            .ok_or_else(|| "missing runtime function `ori_bytes_get`".to_string())?;

        let (idx_var, len_var, bytes_val, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let len_name = SmolStr::new(format!(".__loop_len_{}", loop_id));
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (len_var, _) = self.lookup_var(&len_name).unwrap();
            let (list_var, _) = self.lookup_var(&list_name).unwrap();

            self.builder.def_var(list_var, bytes_v);
            self.store_async_local_if_any(&list_name, bytes_v)?;
            self.emit_arc_register_edge_if_managed(
                &iterable.ty,
                self.async_frame.unwrap(),
                bytes_v,
            )?;

            let len_call = self.builder.ins().call(len_ref, &[bytes_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            self.builder.def_var(len_var, len_v);
            self.store_async_local_if_any(&len_name, len_v)?;

            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            self.store_async_local_if_any(&idx_name, zero)?;

            (
                idx_var,
                len_var,
                self.builder.use_var(list_var),
                Some(loop_id),
            )
        } else {
            let len_call = self.builder.ins().call(len_ref, &[bytes_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            let idx_var = self.builder.declare_var(types::I64);
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            let len_var = self.builder.declare_var(types::I64);
            self.builder.def_var(len_var, len_v);
            (idx_var, len_var, bytes_v, None)
        };

        let bvar = if has_await {
            let (bvar, _) = self.lookup_var(binding).unwrap();
            bvar
        } else {
            let bvar = self.builder.declare_var(types::I8);
            self.insert_var(binding.clone(), (bvar, Ty::U8));
            bvar
        };

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let cur = self.builder.use_var(idx_var);
        let len = self.builder.use_var(len_var);
        let cond = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::SignedLessThan, cur, len);
        self.builder.ins().brif(cond, body_b, &[], exit, &[]);
        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);

        let cur2 = self.builder.use_var(idx_var);
        let call = self.builder.ins().call(get_ref, &[bytes_val, cur2]);
        let elem = self.builder.inst_results(call)[0];
        self.builder.def_var(bvar, elem);
        if has_await {
            self.store_async_local_if_any(binding, elem)?;
        }

        if let Some(ib_name) = index_binding {
            let cur3 = self.builder.use_var(idx_var);
            if has_await {
                let (ib_var, _) = self.lookup_var(ib_name).unwrap();
                self.builder.def_var(ib_var, cur3);
                self.store_async_local_if_any(ib_name, cur3)?;
            } else {
                let ib_var = self.builder.declare_var(types::I64);
                self.builder.def_var(ib_var, cur3);
                self.insert_var(ib_name.clone(), (ib_var, Ty::Int));
            }
        }

        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;
        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur3 = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur3, one);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        if let Some(loop_id) = loop_id {
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            self.emit_arc_unregister_edge(self.async_frame.unwrap(), bytes_val)?;
            self.store_async_local_if_any(&list_name, zero)?;
        }
        self.terminated = false;
        Ok(())
    }

    fn emit_for_string(
        &mut self,
        binding: &SmolStr,
        index_binding: Option<&SmolStr>,
        iterable: &HirExpr,
        body: &HirBlock,
        has_await: bool,
    ) -> Result<(), String> {
        let str_v = self.emit_expr(iterable)?;
        let chars_ref = *self
            .func_refs
            .get("ori_string_chars")
            .ok_or_else(|| "missing runtime function `ori_string_chars`".to_string())?;
        let chars_call = self.builder.ins().call(chars_ref, &[str_v]);
        let list_v = self.builder.inst_results(chars_call)[0];

        let len_ref = *self
            .func_refs
            .get("ori_list_len")
            .ok_or_else(|| "missing runtime function `ori_list_len`".to_string())?;
        let get_ref = *self
            .func_refs
            .get("ori_list_get")
            .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;

        let (idx_var, len_var, list_val, loop_id) = if has_await {
            let loop_id = self.async_loop_index;
            self.async_loop_index += 1;
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            let len_name = SmolStr::new(format!(".__loop_len_{}", loop_id));
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            let (idx_var, _) = self.lookup_var(&idx_name).unwrap();
            let (len_var, _) = self.lookup_var(&len_name).unwrap();
            let (list_var, _) = self.lookup_var(&list_name).unwrap();

            self.builder.def_var(list_var, list_v);
            self.store_async_local_if_any(&list_name, list_v)?;
            self.emit_arc_register_edge_if_managed(
                &Ty::List(Box::new(Ty::String)),
                self.async_frame.unwrap(),
                list_v,
            )?;

            let len_call = self.builder.ins().call(len_ref, &[list_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            self.builder.def_var(len_var, len_v);
            self.store_async_local_if_any(&len_name, len_v)?;

            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            self.store_async_local_if_any(&idx_name, zero)?;

            (
                idx_var,
                len_var,
                self.builder.use_var(list_var),
                Some(loop_id),
            )
        } else {
            let len_call = self.builder.ins().call(len_ref, &[list_v]);
            let len_v = self.builder.inst_results(len_call)[0];
            let idx_var = self.builder.declare_var(types::I64);
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder.def_var(idx_var, zero);
            let len_var = self.builder.declare_var(types::I64);
            self.builder.def_var(len_var, len_v);
            (idx_var, len_var, list_v, None)
        };

        if has_await {
            let (_bvar, _) = self.lookup_var(binding).unwrap();
        } else {
            let bvar = self.builder.declare_var(self.ptr_ty);
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            self.builder.def_var(bvar, zero);
            self.insert_var(binding.clone(), (bvar, Ty::String));
        }

        let header = self.builder.create_block();
        let body_b = self.builder.create_block();
        let step = self.builder.create_block();
        let exit = self.builder.create_block();
        self.builder.ins().jump(header, &[]);
        self.builder.switch_to_block(header);
        let cur = self.builder.use_var(idx_var);
        let len = self.builder.use_var(len_var);
        let cond = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::SignedLessThan, cur, len);
        self.builder.ins().brif(cond, body_b, &[], exit, &[]);
        self.builder.seal_block(body_b);
        self.builder.switch_to_block(body_b);
        let cur2 = self.builder.use_var(idx_var);
        let call = self.builder.ins().call(get_ref, &[list_val, cur2]);
        let elem = self.builder.inst_results(call)[0];
        self.emit_for_element_binding(binding, &Ty::String, elem, has_await)?;
        if let Some(ib_name) = index_binding {
            let cur3 = self.builder.use_var(idx_var);
            if has_await {
                let (ib_var, _) = self.lookup_var(ib_name).unwrap();
                self.builder.def_var(ib_var, cur3);
                self.store_async_local_if_any(ib_name, cur3)?;
            } else {
                let ib_var = self.builder.declare_var(types::I64);
                self.builder.def_var(ib_var, cur3);
                self.insert_var(ib_name.clone(), (ib_var, Ty::Int));
            }
        }
        self.terminated = false;
        self.push_loop(step, exit);
        self.emit_block(body)?;
        self.pop_loop();
        let body_terminated = self.terminated;
        if !self.terminated {
            self.builder.ins().jump(step, &[]);
        }
        self.terminated = false;
        self.builder.seal_block(step);
        self.builder.switch_to_block(step);
        let cur3 = self.builder.use_var(idx_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(cur3, one);
        self.builder.def_var(idx_var, next);
        if let Some(loop_id) = loop_id {
            let idx_name = SmolStr::new(format!(".__loop_idx_{}", loop_id));
            self.store_async_local_if_any(&idx_name, next)?;
        }
        self.builder.ins().jump(header, &[]);
        self.builder.seal_block(header);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        if body_terminated {
            let was_break = self.broke_loop_target == Some(exit);
            if !was_break {
                self.emit_unreachable_return()?;
                return Ok(());
            }
            let continue_block = self.builder.create_block();
            self.builder.ins().jump(continue_block, &[]);
            self.builder.seal_block(continue_block);
            self.builder.switch_to_block(continue_block);
        }
        if !has_await {
            self.emit_for_release_element_binding(binding)?;
        }
        if let Some(loop_id) = loop_id {
            let list_name = SmolStr::new(format!(".__loop_list_{}", loop_id));
            self.emit_arc_release_if_managed(&Ty::List(Box::new(Ty::String)), list_val)?;
            let zero = self.builder.ins().iconst(self.ptr_ty, 0);
            self.emit_arc_unregister_edge(self.async_frame.unwrap(), list_val)?;
            self.store_async_local_if_any(&list_name, zero)?;
        } else {
            self.emit_arc_release_if_managed(&Ty::List(Box::new(Ty::String)), list_v)?;
        }
        self.terminated = false;
        Ok(())
    }

    /// `match` in expression position: every arm produces a value, and the
    /// arms merge through a block parameter (a phi) instead of the `select`
    /// used by `if` expressions — arms must not be evaluated speculatively.
    ///
    /// The result is always handed back OWNED when managed: an arm whose value
    /// is merely borrowed (a pattern binding, a variable) is retained before
    /// the arm's own bindings are released, so the value survives the jump.
    fn emit_match_expr(
        &mut self,
        expr: &HirExpr,
        scrutinee: &HirExpr,
        arms: &[HirExprArm],
    ) -> Result<ir::Value, String> {
        let result_cl = cl_type(&expr.ty, self.ptr_ty).ok_or_else(|| {
            format!(
                "unsupported match expression type `{}`",
                self.display_ty(&expr.ty)
            )
        })?;
        let scr = self.emit_expr(scrutinee)?;
        let scrutinee_owned =
            Self::expr_produces_owned_ref(scrutinee) && is_managed_ty(&scrutinee.ty);

        let merge_blk = self.builder.create_block();
        let result_param = self.builder.append_block_param(merge_blk, result_cl);

        for arm in arms {
            let arm_blk = self.builder.create_block();
            let next_blk = self.builder.create_block();
            let cond = self.pattern_cond(&arm.pattern, scr, &scrutinee.ty);
            self.builder.ins().brif(cond, arm_blk, &[], next_blk, &[]);
            self.builder.seal_block(arm_blk);
            self.builder.switch_to_block(arm_blk);
            self.terminated = false;

            self.push_scope();
            let managed_cleanup_start = self.managed_stack.len();
            self.bind_pattern(&arm.pattern, scr, &scrutinee.ty, false)?;

            if let Some(guard) = &arm.guard {
                let guard_val = self.emit_expr(guard)?;
                let body_blk = self.builder.create_block();
                let reject_blk = self.builder.create_block();
                self.builder
                    .ins()
                    .brif(guard_val, body_blk, &[], reject_blk, &[]);
                self.builder.seal_block(reject_blk);
                self.builder.switch_to_block(reject_blk);
                self.emit_managed_cleanup_calls_from(managed_cleanup_start)?;
                self.builder.ins().jump(next_blk, &[]);
                self.builder.seal_block(body_blk);
                self.builder.switch_to_block(body_blk);
            }

            let value = self.emit_expr_for_expected(&arm.body, &expr.ty)?;
            // Retain BEFORE releasing this arm's bindings: the value may be
            // one of them (`case some(v): v`), and the scrutinee it came from
            // is released right after.
            if is_managed_ty(&expr.ty) && !Self::expr_produces_owned_ref(&arm.body) {
                self.emit_arc_retain_if_managed(&expr.ty, value)?;
            }
            self.emit_managed_cleanup_calls_from(managed_cleanup_start)?;
            self.managed_stack.truncate(managed_cleanup_start);
            self.pop_scope();
            if !self.terminated {
                self.builder.ins().jump(merge_blk, &[value.into()]);
            }

            self.builder.seal_block(next_blk);
            self.builder.switch_to_block(next_blk);
            self.terminated = false;
        }

        // Exhaustiveness is a type-check guarantee, so falling out of every
        // arm is unreachable; the block still needs a terminator.
        if self.hosted_traps {
            self.emit_host_trap_return(2, Some("non-exhaustive match"), true)?;
        } else {
            self.builder.ins().trap(
                ir::TrapCode::user(2).ok_or_else(|| "invalid match trap code `2`".to_string())?,
            );
        }
        self.builder.seal_block(merge_blk);
        self.builder.switch_to_block(merge_blk);
        self.terminated = false;
        if scrutinee_owned {
            self.emit_arc_release_if_managed(&scrutinee.ty, scr)?;
        }
        Ok(result_param)
    }

    fn emit_match(&mut self, scrutinee: &HirExpr, arms: &[HirArm]) -> Result<(), String> {
        let scr = self.emit_expr(scrutinee)?;
        // A fresh owned scrutinee (e.g. `match some_call()`, not a bound Var)
        // has no binding to release it at scope exit and `bind_pattern`
        // extracts payloads as plain loads (borrows), so without this the
        // scrutinee — and anything only it owns via edge — leaks every
        // match. Retain each extracted managed binding so it survives
        // independently, then release the scrutinee once bound.
        let scrutinee_owned =
            Self::expr_produces_owned_ref(scrutinee) && is_managed_ty(&scrutinee.ty);
        let exit = self.builder.create_block();
        for arm in arms {
            let arm_blk = self.builder.create_block();
            let next_blk = self.builder.create_block();
            let cond = self.pattern_cond(&arm.pattern, scr, &scrutinee.ty);
            self.builder.ins().brif(cond, arm_blk, &[], next_blk, &[]);
            self.builder.seal_block(arm_blk);
            self.builder.switch_to_block(arm_blk);
            self.terminated = false;
            self.push_scope();
            // Captured before bind_pattern so its retained bindings (if any)
            // are release-tracked for THIS arm only. Every arm re-enters this
            // loop and calls bind_pattern again, so leaving those entries in
            // managed_stack across arms would let a later arm's cleanup (or
            // a statement after the match) try to release a Variable that
            // only THIS, not-taken-at-runtime arm ever defined.
            let managed_cleanup_start = self.managed_stack.len();
            self.bind_pattern(&arm.pattern, scr, &scrutinee.ty, scrutinee_owned)?;
            if let Some(guard) = &arm.guard {
                // The guard reads the pattern's bindings, so it runs after
                // bind_pattern — but BEFORE the owned-scrutinee release
                // below: a false guard falls through to the next arm's
                // test, which still loads from the scrutinee. The reject
                // path only undoes this arm's binding retains.
                let guard_val = self.emit_expr(guard)?;
                let body_blk = self.builder.create_block();
                let reject_blk = self.builder.create_block();
                self.builder
                    .ins()
                    .brif(guard_val, body_blk, &[], reject_blk, &[]);
                self.builder.seal_block(reject_blk);
                self.builder.switch_to_block(reject_blk);
                self.emit_managed_cleanup_calls_from(managed_cleanup_start)?;
                self.builder.ins().jump(next_blk, &[]);
                self.builder.seal_block(body_blk);
                self.builder.switch_to_block(body_blk);
            }
            if scrutinee_owned {
                self.emit_arc_release_if_managed(&scrutinee.ty, scr)?;
            }
            self.emit_scoped_stmts(&arm.body)?;
            if !self.terminated {
                // A terminated arm (return/break/continue) already released
                // or transferred these bindings along that path; only a
                // normal fallthrough still needs an explicit release here.
                self.emit_managed_cleanup_calls_from(managed_cleanup_start)?;
            }
            self.managed_stack.truncate(managed_cleanup_start);
            self.pop_scope();
            if !self.terminated {
                self.builder.ins().jump(exit, &[]);
            }
            self.builder.seal_block(next_blk);
            self.builder.switch_to_block(next_blk);
            self.terminated = false;
        }
        self.builder.ins().jump(exit, &[]);
        self.builder.seal_block(exit);
        self.builder.switch_to_block(exit);
        self.terminated = false;
        Ok(())
    }

    fn pattern_cond(&mut self, pat: &HirPattern, scr: ir::Value, scr_ty: &Ty) -> ir::Value {
        match pat {
            HirPattern::Wildcard | HirPattern::Binding(_, _) => {
                self.builder.ins().iconst(types::I8, 1)
            }
            // `a or b` — true when any alternative matches. Alternatives never
            // bind, so testing them all eagerly has no observable effect.
            HirPattern::Or(alternatives) => {
                let mut cond = self.builder.ins().iconst(types::I8, 0);
                for alternative in alternatives {
                    let alt_cond = self.pattern_cond(alternative, scr, scr_ty);
                    cond = self.builder.ins().bor(cond, alt_cond);
                }
                cond
            }
            HirPattern::BoolLit(b) => {
                let lit = self.builder.ins().iconst(types::I8, if *b { 1 } else { 0 });
                self.builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, scr, lit)
            }
            HirPattern::IntLit(n) => {
                let cl = cl_type(scr_ty, self.ptr_ty).unwrap_or(types::I64);
                let lit = self.builder.ins().iconst(cl, *n);
                self.builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, scr, lit)
            }
            HirPattern::StrLit(s) => {
                let Ok(rhs) = self.string_ptr(s.as_str()) else {
                    return self.builder.ins().iconst(types::I8, 0);
                };
                if let Some(&strcmp_ref) = self.func_refs.get("strcmp") {
                    let call = self.builder.ins().call(strcmp_ref, &[scr, rhs]);
                    let cmp = self.builder.inst_results(call)[0];
                    let zero = self.builder.ins().iconst(types::I32, 0);
                    self.builder
                        .ins()
                        .icmp(ir::condcodes::IntCC::Equal, cmp, zero)
                } else {
                    self.builder.ins().iconst(types::I8, 0)
                }
            }
            HirPattern::Variant {
                def_id,
                variant,
                fields,
            } => {
                if let Some(layout) = self.enum_layouts.get(def_id).cloned() {
                    if let Some(v_layout) = layout.variant(variant) {
                        let tag_val = self.builder.ins().load(types::I32, MemFlags::new(), scr, 0);
                        let expected = self.builder.ins().iconst(types::I32, v_layout.tag as i64);
                        let mut cond =
                            self.builder
                                .ins()
                                .icmp(ir::condcodes::IntCC::Equal, tag_val, expected);

                        for (fname, fpat) in fields {
                            if let Some(fi) = v_layout.fields.field(fname) {
                                let total_off = (layout.payload_offset + fi.offset) as i32;
                                let cl = cl_type(&fi.ty, self.ptr_ty).unwrap_or(types::I64);
                                let fval =
                                    self.builder.ins().load(cl, MemFlags::new(), scr, total_off);
                                let fcond = self.pattern_cond(fpat, fval, &fi.ty);
                                cond = self.builder.ins().band(cond, fcond);
                            }
                        }
                        return cond;
                    }
                }
                self.builder.ins().iconst(types::I8, 0)
            }
            HirPattern::None_ => {
                let has_val = self.builder.ins().load(types::I8, MemFlags::new(), scr, 0);
                let zero = self.builder.ins().iconst(types::I8, 0);
                self.builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, has_val, zero)
            }
            HirPattern::Some_(inner) => {
                let has_val = self.builder.ins().load(types::I8, MemFlags::new(), scr, 0);
                let inner_ty = if let Ty::Optional(t) = scr_ty {
                    &**t
                } else {
                    &Ty::Void
                };
                let cl = cl_type(inner_ty, self.ptr_ty).unwrap_or(types::I64);
                let (val_off, _) = optional_layout(inner_ty, self.ptr_ty);
                let val = self
                    .builder
                    .ins()
                    .load(cl, MemFlags::new(), scr, val_off as i32);
                let inner_cond = self.pattern_cond(inner, val, inner_ty);
                self.builder.ins().band(has_val, inner_cond)
            }
            HirPattern::Ok_(inner) => {
                let is_ok = self.builder.ins().load(types::I8, MemFlags::new(), scr, 0);
                let inner_ty = if let Ty::Result(ok, _) = scr_ty {
                    &**ok
                } else {
                    &Ty::Void
                };
                let cl = cl_type(inner_ty, self.ptr_ty).unwrap_or(types::I64);
                let (err_ty, pay_off) = if let Ty::Result(ok, err) = scr_ty {
                    (&**err, result_layout(ok, err, self.ptr_ty).0)
                } else {
                    (&Ty::Void, 1)
                };
                let _ = err_ty;
                let val = self
                    .builder
                    .ins()
                    .load(cl, MemFlags::new(), scr, pay_off as i32);
                let inner_cond = self.pattern_cond(inner, val, inner_ty);
                self.builder.ins().band(is_ok, inner_cond)
            }
            HirPattern::Err_(inner) => {
                let is_ok = self.builder.ins().load(types::I8, MemFlags::new(), scr, 0);
                let zero = self.builder.ins().iconst(types::I8, 0);
                let is_err = self
                    .builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, is_ok, zero);
                let inner_ty = if let Ty::Result(_, err) = scr_ty {
                    &**err
                } else {
                    &Ty::Void
                };
                let cl = cl_type(inner_ty, self.ptr_ty).unwrap_or(types::I64);
                let pay_off = if let Ty::Result(ok, err) = scr_ty {
                    result_layout(ok, err, self.ptr_ty).0
                } else {
                    1
                };
                let val = self
                    .builder
                    .ins()
                    .load(cl, MemFlags::new(), scr, pay_off as i32);
                let inner_cond = self.pattern_cond(inner, val, inner_ty);
                self.builder.ins().band(is_err, inner_cond)
            }
            HirPattern::Tuple(patterns) => {
                if let Ty::Tuple(elems) = scr_ty {
                    let (layout, _, _) = tuple_layout(elems, self.ptr_ty);
                    let mut cond = self.builder.ins().iconst(types::I8, 1);
                    for (pat, (offset, ty)) in patterns.iter().zip(layout.iter()) {
                        let cl = cl_type(ty, self.ptr_ty).unwrap_or(types::I64);
                        let val = self
                            .builder
                            .ins()
                            .load(cl, MemFlags::new(), scr, *offset as i32);
                        let next = self.pattern_cond(pat, val, ty);
                        cond = self.builder.ins().band(cond, next);
                    }
                    cond
                } else {
                    self.builder.ins().iconst(types::I8, 0)
                }
            }
        }
    }

    /// Binds `pat` to the payload(s) extracted from `val` (of type `ty`).
    ///
    /// Extraction is a plain load (a borrow of `val`'s lifetime) — safe as
    /// long as `val` outlives the bound names, which normally holds because
    /// `val` is a container the caller keeps alive (a local binding, a
    /// field). When `retain_bindings` is set (the scrutinee was a fresh
    /// owned temporary about to be released once bound — see `emit_match`),
    /// every managed leaf binding is retained here so it survives
    /// independently of that release.
    fn bind_pattern(
        &mut self,
        pat: &HirPattern,
        val: ir::Value,
        ty: &Ty,
        retain_bindings: bool,
    ) -> Result<(), String> {
        match pat {
            HirPattern::Binding(name, bind_ty) => {
                let bty = if *bind_ty == Ty::Infer(0) {
                    ty
                } else {
                    bind_ty
                };
                if let Some(cl_ty) = cl_type(bty, self.ptr_ty) {
                    // A same-named binding from an ENCLOSING scope (e.g. an
                    // outer match arm or if-some still on the scope stack)
                    // must not be reused if its native type differs — the
                    // Cranelift Variable was declared with that type, and
                    // def_var with a different type panics internally. Only
                    // reuse when the type matches; otherwise shadow with a
                    // fresh Variable (matches normal lexical shadowing).
                    let var = self
                        .lookup_var(name)
                        .filter(|(_, existing_ty)| existing_ty == bty)
                        .map(|(v, _)| v)
                        .unwrap_or_else(|| {
                            let v = self.builder.declare_var(cl_ty);
                            self.insert_var(name.clone(), (v, bty.clone()));
                            v
                        });
                    self.builder.def_var(var, val);
                    if retain_bindings && is_managed_ty(bty) {
                        self.emit_arc_retain_if_managed(bty, val)?;
                        // Register for scope-exit release (or return-transfer
                        // elision) exactly like a `let`/`const` local — the
                        // retain above gave it its own +1, independent of
                        // the scrutinee released right after bind_pattern.
                        self.managed_stack.push(ManagedCleanup {
                            var,
                            ty: bty.clone(),
                        });
                    }
                    // Match bindings must land in the async frame: after `await`
                    // resume, `reload_async_frame_vars` reloads locals from the
                    // frame. Without this store, handles like Connection become
                    // null across `await write_all_async` / nested match arms.
                    self.store_async_local_if_any(name, val)?;
                }
            }
            HirPattern::Variant {
                def_id,
                variant,
                fields,
            } => {
                if let Some(layout) = self.enum_layouts.get(def_id).cloned() {
                    if let Some(v_layout) = layout.variant(variant) {
                        for (fname, fpat) in fields {
                            if let Some(fi) = v_layout.fields.field(fname) {
                                let total_off = (layout.payload_offset + fi.offset) as i32;
                                let cl = cl_type(&fi.ty, self.ptr_ty).unwrap_or(types::I64);
                                let fval =
                                    self.builder.ins().load(cl, MemFlags::new(), val, total_off);
                                self.bind_pattern(fpat, fval, &fi.ty, retain_bindings)?;
                            }
                        }
                    }
                }
            }
            HirPattern::Some_(inner) => {
                let inner_ty = if let Ty::Optional(t) = ty {
                    &**t
                } else {
                    &Ty::Void
                };
                let cl = cl_type(inner_ty, self.ptr_ty).unwrap_or(types::I64);
                let (val_off, _) = optional_layout(inner_ty, self.ptr_ty);
                let fval = self
                    .builder
                    .ins()
                    .load(cl, MemFlags::new(), val, val_off as i32);
                self.bind_pattern(inner, fval, inner_ty, retain_bindings)?;
            }
            HirPattern::Ok_(inner) => {
                let inner_ty = if let Ty::Result(ok, _) = ty {
                    &**ok
                } else {
                    &Ty::Void
                };
                let cl = cl_type(inner_ty, self.ptr_ty).unwrap_or(types::I64);
                let pay_off = if let Ty::Result(ok, err) = ty {
                    result_layout(ok, err, self.ptr_ty).0
                } else {
                    1
                };
                let fval = self
                    .builder
                    .ins()
                    .load(cl, MemFlags::new(), val, pay_off as i32);
                self.bind_pattern(inner, fval, inner_ty, retain_bindings)?;
            }
            HirPattern::Err_(inner) => {
                let inner_ty = if let Ty::Result(_, err) = ty {
                    &**err
                } else {
                    &Ty::Void
                };
                let cl = cl_type(inner_ty, self.ptr_ty).unwrap_or(types::I64);
                let pay_off = if let Ty::Result(ok, err) = ty {
                    result_layout(ok, err, self.ptr_ty).0
                } else {
                    1
                };
                let fval = self
                    .builder
                    .ins()
                    .load(cl, MemFlags::new(), val, pay_off as i32);
                self.bind_pattern(inner, fval, inner_ty, retain_bindings)?;
            }
            HirPattern::Tuple(patterns) => {
                if let Ty::Tuple(elems) = ty {
                    let (layout, _, _) = tuple_layout(elems, self.ptr_ty);
                    for (pat, (offset, elem_ty)) in patterns.iter().zip(layout.iter()) {
                        let cl = cl_type(elem_ty, self.ptr_ty).unwrap_or(types::I64);
                        let fval =
                            self.builder
                                .ins()
                                .load(cl, MemFlags::new(), val, *offset as i32);
                        self.bind_pattern(pat, fval, elem_ty, retain_bindings)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // == Expressions ==

    fn emit_builtin_or(
        &mut self,
        value: &HirExpr,
        fallback: &HirExpr,
    ) -> Result<ir::Value, String> {
        let wrapper = self.emit_expr(value)?;
        match &value.ty {
            Ty::Optional(inner) => {
                let (value_offset, _) = optional_layout(inner, self.ptr_ty);
                let value_ty = cl_type(inner, self.ptr_ty).ok_or_else(|| {
                    format!(
                        "optional inner type `{}` has no native layout",
                        self.display_ty(inner)
                    )
                })?;
                let tag = self
                    .builder
                    .ins()
                    .load(types::I8, MemFlags::new(), wrapper, 0);
                let one = self.builder.ins().iconst(types::I8, 1);
                let has_value = self
                    .builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, tag, one);
                self.emit_lazy_or_result(
                    has_value,
                    value_ty,
                    |this| {
                        Ok(this.builder.ins().load(
                            value_ty,
                            MemFlags::new(),
                            wrapper,
                            value_offset as i32,
                        ))
                    },
                    fallback,
                )
            }
            Ty::Result(ok, err) => {
                let (value_offset, _, _) = result_layout(ok, err, self.ptr_ty);
                let value_ty = cl_type(ok, self.ptr_ty).ok_or_else(|| {
                    format!(
                        "result ok type `{}` has no native layout",
                        self.display_ty(ok)
                    )
                })?;
                let tag = self
                    .builder
                    .ins()
                    .load(types::I8, MemFlags::new(), wrapper, 0);
                let one = self.builder.ins().iconst(types::I8, 1);
                let is_ok = self
                    .builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, tag, one);
                self.emit_lazy_or_result(
                    is_ok,
                    value_ty,
                    |this| {
                        Ok(this.builder.ins().load(
                            value_ty,
                            MemFlags::new(),
                            wrapper,
                            value_offset as i32,
                        ))
                    },
                    fallback,
                )
            }
            other => Err(format!(
                "`.or()` expects optional/result receiver, got `{}`",
                self.display_ty(other)
            )),
        }
    }

    fn emit_builtin_or_wrap(
        &mut self,
        value: &HirExpr,
        context: &HirExpr,
    ) -> Result<ir::Value, String> {
        let wrapper = self.emit_expr(value)?;
        let Ty::Result(ok_ty, err_ty) = &value.ty else {
            return Err(format!(
                "`.or_wrap()` expects result receiver, got `{}`",
                self.display_ty(&value.ty)
            ));
        };
        if !matches!(**err_ty, Ty::String) {
            return Err(format!(
                "`.or_wrap()` currently requires `result[T, string]`, got `{}`",
                self.display_ty(&value.ty)
            ));
        }

        let (payload_offset, _, total) = result_layout(ok_ty, err_ty, self.ptr_ty);
        let tag = self
            .builder
            .ins()
            .load(types::I8, MemFlags::new(), wrapper, 0);
        let ok_block = self.builder.create_block();
        let err_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, self.ptr_ty);
        self.builder.ins().brif(tag, ok_block, &[], err_block, &[]);

        self.builder.seal_block(ok_block);
        self.builder.switch_to_block(ok_block);
        self.terminated = false;
        let ok_value_ty = cl_type(ok_ty, self.ptr_ty).ok_or_else(|| {
            format!(
                "result ok type `{}` has no native layout",
                self.display_ty(ok_ty)
            )
        })?;
        let ok_value =
            self.builder
                .ins()
                .load(ok_value_ty, MemFlags::new(), wrapper, payload_offset as i32);
        let ok_result = self.malloc_bytes(total)?;
        let one = self.builder.ins().iconst(types::I8, 1);
        self.builder.ins().store(MemFlags::new(), one, ok_result, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), ok_value, ok_result, payload_offset as i32);
        self.emit_arc_register_edge_if_managed(ok_ty, ok_result, ok_value)?;
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(ok_result)]);

        self.builder.seal_block(err_block);
        self.builder.switch_to_block(err_block);
        self.terminated = false;
        let original_error =
            self.builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), wrapper, payload_offset as i32);
        let original_error_len = self.str_len_from_ptr(original_error)?;
        let context_parts = self.emit_as_string_parts(context)?;
        let separator_parts = StringParts {
            ptr: self.bytes_ptr(b": ")?,
            len: self.builder.ins().iconst(types::I64, 2),
        };
        let prefix = self.concat_string_parts(context_parts, separator_parts)?;
        let wrapped_error = self.concat_string_parts(
            prefix,
            StringParts {
                ptr: original_error,
                len: original_error_len,
            },
        )?;
        let err_result = self.malloc_bytes(total)?;
        let zero = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .store(MemFlags::new(), zero, err_result, 0);
        self.builder.ins().store(
            MemFlags::new(),
            wrapped_error.ptr,
            err_result,
            payload_offset as i32,
        );
        self.emit_arc_register_edge_if_managed(err_ty, err_result, wrapped_error.ptr)?;
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(err_result)]);

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        Ok(self.builder.block_params(merge_block)[0])
    }

    fn emit_lazy_or_result(
        &mut self,
        condition: ir::Value,
        value_ty: ir::Type,
        emit_present_value: impl FnOnce(&mut Self) -> Result<ir::Value, String>,
        fallback: &HirExpr,
    ) -> Result<ir::Value, String> {
        let present_block = self.builder.create_block();
        let fallback_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, value_ty);

        self.builder
            .ins()
            .brif(condition, present_block, &[], fallback_block, &[]);

        self.builder.seal_block(present_block);
        self.builder.switch_to_block(present_block);
        self.terminated = false;
        let present_value = emit_present_value(self)?;
        if !self.terminated {
            let args = [BlockArg::Value(present_value)];
            self.builder.ins().jump(merge_block, &args);
        }

        self.builder.seal_block(fallback_block);
        self.builder.switch_to_block(fallback_block);
        self.terminated = false;
        let fallback_value = self.emit_expr(fallback)?;
        if !self.terminated {
            let args = [BlockArg::Value(fallback_value)];
            self.builder.ins().jump(merge_block, &args);
        }

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        Ok(self.builder.block_params(merge_block)[0])
    }

    fn expr_can_be_reloaded_after_await(expr: &HirExpr) -> bool {
        matches!(
            expr.kind,
            HirExprKind::BoolLit(_)
                | HirExprKind::IntLit(_)
                | HirExprKind::FloatLit(_)
                | HirExprKind::Unit
                | HirExprKind::StrLit(_)
                | HirExprKind::Var(_)
        )
    }

    fn emit_expr(&mut self, expr: &HirExpr) -> Result<ir::Value, String> {
        Ok(match &expr.kind {
            HirExprKind::BoolLit(b) => self.builder.ins().iconst(types::I8, if *b { 1 } else { 0 }),
            HirExprKind::IntLit(n) => {
                let cl = cl_type(&expr.ty, self.ptr_ty).unwrap_or(types::I64);
                self.builder.ins().iconst(cl, *n)
            }
            HirExprKind::FloatLit(f) => match &expr.ty {
                Ty::Float32 => self.builder.ins().f32const(*f as f32),
                _ => self.builder.ins().f64const(*f),
            },
            HirExprKind::Unit => self.builder.ins().iconst(self.ptr_ty, 0),
            HirExprKind::None_ => {
                let inner_ty = if let Ty::Optional(t) = &expr.ty {
                    &**t
                } else {
                    &Ty::Void
                };
                let (_, total) = optional_layout(inner_ty, self.ptr_ty);
                let base = self.malloc_bytes(total)?;
                let zero8 = self.builder.ins().iconst(types::I8, 0);
                self.builder.ins().store(MemFlags::new(), zero8, base, 0);
                base
            }
            HirExprKind::Some_(inner) => {
                let inner_is_owned = Self::expr_produces_owned_ref(inner);
                let val = self.emit_expr(inner)?;
                let inner_ty = if let Ty::Optional(t) = &expr.ty {
                    &**t
                } else {
                    &inner.ty
                };
                let (val_off, total) = optional_layout(inner_ty, self.ptr_ty);
                let base = self.malloc_bytes(total)?;
                let one8 = self.builder.ins().iconst(types::I8, 1);
                self.builder.ins().store(MemFlags::new(), one8, base, 0);
                self.builder
                    .ins()
                    .store(MemFlags::new(), val, base, val_off as i32);
                self.emit_arc_register_edge_if_managed(inner_ty, base, val)?;
                // The edge now owns the +1 from the inner temporary's malloc.
                // Release the temporary's own +1 so it does not leak. Borrowed
                // refs (Var/Field) keep their binding's ref untouched.
                if inner_is_owned && is_managed_ty(inner_ty) {
                    self.emit_arc_release_if_managed(inner_ty, val)?;
                }
                base
            }
            HirExprKind::Ok_(inner) => {
                let inner_is_owned = Self::expr_produces_owned_ref(inner);
                let val = self.emit_expr(inner)?;
                let (ok_ty, err_ty) = if let Ty::Result(o, e) = &expr.ty {
                    (&**o, &**e)
                } else {
                    (&inner.ty, &Ty::Void)
                };
                let (pay_off, _, total) = result_layout(ok_ty, err_ty, self.ptr_ty);
                let base = self.malloc_bytes(total)?;
                let one8 = self.builder.ins().iconst(types::I8, 1);
                self.builder.ins().store(MemFlags::new(), one8, base, 0);
                self.builder
                    .ins()
                    .store(MemFlags::new(), val, base, pay_off as i32);
                self.emit_arc_register_edge_if_managed(ok_ty, base, val)?;
                if inner_is_owned && is_managed_ty(ok_ty) {
                    self.emit_arc_release_if_managed(ok_ty, val)?;
                }
                base
            }
            HirExprKind::Err_(inner) => {
                let inner_is_owned = Self::expr_produces_owned_ref(inner);
                let val = self.emit_expr(inner)?;
                let (ok_ty, err_ty) = if let Ty::Result(o, e) = &expr.ty {
                    (&**o, &**e)
                } else {
                    (&Ty::Void, &inner.ty)
                };
                let (pay_off, _, total) = result_layout(ok_ty, err_ty, self.ptr_ty);
                let base = self.malloc_bytes(total)?;
                let zero8 = self.builder.ins().iconst(types::I8, 0);
                self.builder.ins().store(MemFlags::new(), zero8, base, 0);
                self.builder
                    .ins()
                    .store(MemFlags::new(), val, base, pay_off as i32);
                self.emit_arc_register_edge_if_managed(err_ty, base, val)?;
                if inner_is_owned && is_managed_ty(err_ty) {
                    self.emit_arc_release_if_managed(err_ty, val)?;
                }
                base
            }
            HirExprKind::StrLit(s) => self.string_ptr(s.as_str())?,
            HirExprKind::BytesLit(bytes) => self.bytes_ptr(bytes)?,
            HirExprKind::InterpolatedStr(parts) => self.emit_interpolated_string(parts)?,
            HirExprKind::Var(name) => {
                if let Some((var, _)) = self.lookup_var(name) {
                    self.builder.use_var(var)
                } else if let Some(value) = self.load_global(name) {
                    value
                } else if let Some(expr) = self.const_exprs.get(name).cloned() {
                    self.emit_expr(&expr)?
                } else {
                    return Err(format!("undefined variable `{name}` in native codegen"));
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.emit_short_circuit_binary(*op, lhs, rhs);
                }
                let lhs_owned = Self::expr_produces_owned_ref(lhs);
                let rhs_owned = Self::expr_produces_owned_ref(rhs);
                let rhs_has_await = self.async_frame.is_some() && expr_contains_await(rhs);
                let (lv, rv) = if rhs_has_await && Self::expr_can_be_reloaded_after_await(lhs) {
                    let rv = self.emit_expr(rhs)?;
                    let lv = self.emit_expr(lhs)?;
                    (lv, rv)
                } else {
                    let lv = self.emit_expr(lhs)?;
                    let rv = self.emit_expr(rhs)?;
                    (lv, rv)
                };
                let res = self.emit_binary(*op, lv, rv, &lhs.ty)?;
                // String/bytes concat runtime helpers borrow their operands
                // without releasing them, so fresh +1 temporaries passed as
                // operands would leak. Release owned managed operands after
                // the concat call. Literal/Var operands are either static or
                // owned by a binding (released by scope cleanup).
                if matches!(op, BinaryOp::Add) && matches!(lhs.ty, Ty::String | Ty::Bytes) {
                    if lhs_owned && is_managed_ty(&lhs.ty) {
                        self.emit_arc_release_if_managed(&lhs.ty, lv)?;
                    }
                    if rhs_owned && is_managed_ty(&rhs.ty) {
                        self.emit_arc_release_if_managed(&rhs.ty, rv)?;
                    }
                }
                res
            }
            HirExprKind::Unary { op, operand } => {
                let v = self.emit_expr(operand)?;
                match op {
                    UnaryOp::Neg if is_float_ty(&operand.ty) => self.builder.ins().fneg(v),
                    UnaryOp::Neg => self.builder.ins().ineg(v),
                    UnaryOp::Not => {
                        let zero = self.builder.ins().iconst(types::I8, 0);
                        self.builder
                            .ins()
                            .icmp(ir::condcodes::IntCC::Equal, v, zero)
                    }
                    // Bitwise complement is `x ^ -1` (two's complement).
                    UnaryOp::BitNot => {
                        let cl = cl_type(&operand.ty, self.ptr_ty).unwrap_or(types::I64);
                        let minus_one = self.builder.ins().iconst(cl, -1);
                        self.builder.ins().bxor(v, minus_one)
                    }
                }
            }
            HirExprKind::Call { callee, args } => {
                if let HirExprKind::Var(name) = &callee.kind {
                    if name.as_str() == "__ori_builtin_or" && args.len() == 2 {
                        return self.emit_builtin_or(&args[0].value, &args[1].value);
                    }
                    if name.as_str() == "__ori_builtin_or_wrap" && args.len() == 2 {
                        return self.emit_builtin_or_wrap(&args[0].value, &args[1].value);
                    }
                    if name.as_str() == "ori_lazy_once" && args.len() == 1 {
                        return self.emit_lazy_once(&args[0].value, &expr.ty);
                    }
                    if name.as_str() == "ori_lazy_force" && args.len() == 1 {
                        return self.emit_lazy_force(&args[0].value, &expr.ty);
                    }
                    if name.as_str() == "ori_lazy_is_consumed" && args.len() == 1 {
                        return self.emit_lazy_is_consumed(&args[0].value);
                    }
                    if name.as_str() == "ori_region_reset" && args.len() == 1 {
                        let handle_val = self.emit_expr(&args[0].value)?;
                        let inner_ptr =
                            self.builder
                                .ins()
                                .load(self.ptr_ty, MemFlags::new(), handle_val, 0);
                        let zero = self.builder.ins().iconst(types::I64, 0);
                        self.builder
                            .ins()
                            .store(MemFlags::new(), zero, inner_ptr, 0);
                        self.builder
                            .ins()
                            .store(MemFlags::new(), zero, inner_ptr, 8);
                        self.builder
                            .ins()
                            .store(MemFlags::new(), zero, inner_ptr, 16);
                        self.builder
                            .ins()
                            .store(MemFlags::new(), zero, inner_ptr, 24);
                        return Ok(self.builder.ins().iconst(types::I64, 0));
                    }
                    if name.as_str() == "ori_map_new" && args.is_empty() {
                        if let Ty::Map(key_ty, _) = &expr.ty {
                            if let Some(eq_fn) = self.equality_callback_for_type(key_ty) {
                                let hash_fn =
                                    self.hash_callback_for_type(key_ty).ok_or_else(|| {
                                        format!(
                                            "missing hash callback for `{}`",
                                            self.display_ty(key_ty)
                                        )
                                    })?;
                                let new_ref =
                                    *self.func_refs.get("ori_map_new_custom").ok_or_else(|| {
                                        "missing runtime function `ori_map_new_custom`".to_string()
                                    })?;
                                let call = self.builder.ins().call(new_ref, &[hash_fn, eq_fn]);
                                return Ok(self.builder.inst_results(call)[0]);
                            }
                        }
                    }
                    if name.as_str() == "ori_set_new" && args.is_empty() {
                        if let Ty::Set(elem_ty) = &expr.ty {
                            if let Some(eq_fn) = self.equality_callback_for_type(elem_ty) {
                                let hash_fn =
                                    self.hash_callback_for_type(elem_ty).ok_or_else(|| {
                                        format!(
                                            "missing hash callback for `{}`",
                                            self.display_ty(elem_ty)
                                        )
                                    })?;
                                let new_ref = *self
                                    .func_refs
                                    .get("ori_set_new_custom_with_hash")
                                    .ok_or_else(|| {
                                    "missing runtime function `ori_set_new_custom_with_hash`"
                                        .to_string()
                                })?;
                                let call = self.builder.ins().call(new_ref, &[hash_fn, eq_fn]);
                                return Ok(self.builder.inst_results(call)[0]);
                            }
                        }
                    }
                    if name.as_str() == "ori_hash_table_new" && args.is_empty() {
                        if let Ty::Opaque {
                            kind: OpaqueTy::HashTable,
                            args: table_args,
                        } = &expr.ty
                        {
                            if let Some(key_ty) = table_args.first() {
                                if let Some(eq_fn) = self.equality_callback_for_type(key_ty) {
                                    let hash_fn =
                                        self.hash_callback_for_type(key_ty).ok_or_else(|| {
                                            format!(
                                                "missing hash callback for `{}`",
                                                self.display_ty(key_ty)
                                            )
                                        })?;
                                    let new_ref = *self
                                        .func_refs
                                        .get("ori_hash_table_new_custom_with_hash")
                                        .ok_or_else(|| {
                                            "missing runtime function `ori_hash_table_new_custom_with_hash`"
                                                .to_string()
                                        })?;
                                    let call = self.builder.ins().call(new_ref, &[hash_fn, eq_fn]);
                                    return Ok(self.builder.inst_results(call)[0]);
                                }
                            }
                        }
                    }
                    if name.as_str() == "ori_hash_table_with_capacity" && args.len() == 1 {
                        if let Ty::Opaque {
                            kind: OpaqueTy::HashTable,
                            args: table_args,
                        } = &expr.ty
                        {
                            if let Some(key_ty) = table_args.first() {
                                if let Some(eq_fn) = self.equality_callback_for_type(key_ty) {
                                    let hash_fn =
                                        self.hash_callback_for_type(key_ty).ok_or_else(|| {
                                            format!(
                                                "missing hash callback for `{}`",
                                                self.display_ty(key_ty)
                                            )
                                        })?;
                                    let new_ref = *self
                                        .func_refs
                                        .get("ori_hash_table_with_capacity_custom_with_hash")
                                        .ok_or_else(|| {
                                            "missing runtime function `ori_hash_table_with_capacity_custom_with_hash`"
                                                .to_string()
                                        })?;
                                    let capacity = self.emit_expr(&args[0].value)?;
                                    let call = self
                                        .builder
                                        .ins()
                                        .call(new_ref, &[capacity, hash_fn, eq_fn]);
                                    return Ok(self.builder.inst_results(call)[0]);
                                }
                            }
                        }
                    }
                    if name.as_str() == "ori_graph_new" && args.len() == 1 {
                        if let Ty::Opaque {
                            kind: OpaqueTy::Graph,
                            args: graph_args,
                        } = &expr.ty
                        {
                            if let Some(node_ty) =
                                graph_args.first().filter(|ty| !ty.contains_infer())
                            {
                                if let Some(eq_fn) = self.equality_callback_for_type(node_ty) {
                                    let new_ref = *self
                                        .func_refs
                                        .get("ori_graph_new_custom")
                                        .ok_or_else(|| {
                                            "missing runtime function `ori_graph_new_custom`"
                                                .to_string()
                                        })?;
                                    let directed = self.emit_expr(&args[0].value)?;
                                    let call = self.builder.ins().call(new_ref, &[directed, eq_fn]);
                                    return Ok(self.builder.inst_results(call)[0]);
                                }
                            }
                        }
                    }
                    // ori_io_print takes (ptr: *u8, len: i64) — build args accordingly
                    if matches!(name.as_str(), "ori_test_assert_eq" | "ori_test_assert_ne") {
                        return self.emit_test_assert_equality_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_map_set"
                            | "ori_map_get"
                            | "ori_map_try_get"
                            | "ori_map_contains"
                            | "ori_map_remove"
                            | "ori_map_try_remove"
                            | "ori_map_from_entries"
                    ) {
                        return self.emit_map_runtime_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_hash_table_set"
                            | "ori_hash_table_get"
                            | "ori_hash_table_remove"
                            | "ori_hash_table_contains"
                    ) {
                        return self.emit_hash_table_runtime_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_linked_list_find" | "ori_doubly_linked_list_find"
                    ) {
                        return self.emit_linked_list_find_runtime_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_graph_add_node"
                            | "ori_graph_remove_node"
                            | "ori_graph_add_edge"
                            | "ori_graph_add_weighted_edge"
                            | "ori_graph_remove_edge"
                            | "ori_graph_has_node"
                            | "ori_graph_has_edge"
                            | "ori_graph_edge_weight"
                            | "ori_graph_neighbors"
                            | "ori_graph_bfs"
                            | "ori_graph_dfs"
                            | "ori_graph_shortest_path"
                            | "ori_graph_shortest_weighted_path"
                    ) {
                        return self.emit_graph_runtime_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_set_add"
                            | "ori_set_contains"
                            | "ori_set_remove"
                            | "ori_set_try_remove"
                            | "ori_set_from_list"
                    ) {
                        return self.emit_set_runtime_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_tree_new"
                            | "ori_tree_root"
                            | "ori_tree_value"
                            | "ori_tree_try_value"
                            | "ori_tree_contains_node"
                            | "ori_tree_set_value"
                            | "ori_tree_add_child"
                            | "ori_tree_children"
                            | "ori_tree_parent"
                            | "ori_tree_remove_subtree"
                            | "ori_tree_move_subtree"
                            | "ori_tree_find"
                            | "ori_tree_len"
                            | "ori_tree_depth"
                            | "ori_tree_pre_order"
                            | "ori_tree_post_order"
                            | "ori_tree_breadth_first"
                            | "ori_tree_clone"
                            | "ori_tree_clone_subtree"
                    ) {
                        return self.emit_tree_runtime_call(name.as_str(), args);
                    }
                    if matches!(
                        name.as_str(),
                        "ori_heap_new"
                            | "ori_heap_push"
                            | "ori_heap_pop"
                            | "ori_heap_peek"
                            | "ori_heap_len"
                            | "ori_heap_is_empty"
                            | "ori_heap_clear"
                            | "ori_heap_clone"
                            | "ori_heap_to_list"
                            | "ori_heap_from_list"
                            | "ori_heap_merge"
                            | "ori_heap_remove"
                            | "ori_heap_into_sorted_list"
                    ) {
                        return self.emit_heap_runtime_call(name.as_str(), args, &expr.ty);
                    }
                    if name == "ori_io_print" || name == "ori_io_eprint" {
                        if let Some(&fref) = self.func_refs.get(name.as_str()) {
                            let mut cl_args = Vec::new();
                            let mut owned_temps: Vec<(ir::Value, Ty)> = Vec::new();
                            for a in args {
                                // ori_io_print always takes (ptr, len); string-like args
                                // use length-aware parts or the Ori runtime length helper.
                                let is_known_string =
                                    matches!(&a.value.ty, Ty::String | Ty::Infer(_));
                                let is_ptr_like =
                                    cl_type(&a.value.ty, self.ptr_ty) == Some(self.ptr_ty);
                                if is_known_string {
                                    let owned = Self::expr_produces_owned_ref(&a.value);
                                    let parts = self.emit_as_string_parts(&a.value)?;
                                    cl_args.push(parts.ptr);
                                    cl_args.push(parts.len);
                                    // The print call borrows the string; a
                                    // fresh +1 temporary must be released
                                    // after the call or it leaks.
                                    if owned {
                                        owned_temps.push((parts.ptr, Ty::String));
                                    }
                                } else if is_ptr_like {
                                    let owned = Self::expr_produces_owned_ref(&a.value);
                                    let v = self.emit_expr(&a.value)?;
                                    let len = self.str_len_from_ptr(v)?;
                                    cl_args.push(v);
                                    cl_args.push(len);
                                    if owned && is_managed_ty(&a.value.ty) {
                                        owned_temps.push((v, a.value.ty.clone()));
                                    }
                                } else {
                                    let v = self.emit_expr(&a.value)?;
                                    cl_args.push(v);
                                }
                            }
                            self.builder.ins().call(fref, &cl_args);
                            for (temp, ty) in owned_temps {
                                self.emit_arc_release_if_managed(&ty, temp)?;
                            }
                            self.builder.ins().iconst(types::I8, 0)
                        } else {
                            return Err("missing runtime function `ori_io_print`".to_string());
                        }
                    } else {
                        if self.func_refs.get(name.as_str()).is_none()
                            && matches!(&callee.ty, Ty::Func { .. })
                        {
                            return self.emit_closure_call(callee, args);
                        }
                        if matches!(name.as_str(), "ori_iter_sort" | "ori_iter_unique")
                            && args.len() == 1
                            && matches!(
                                &args[0].value.ty,
                                Ty::List(elem) if matches!(elem.as_ref(), Ty::String)
                            )
                        {
                            let runtime_name = if name.as_str() == "ori_iter_sort" {
                                "ori_iter_sort_string"
                            } else {
                                "ori_iter_unique_string"
                            };
                            let list_v = self.emit_expr(&args[0].value)?;
                            let fref = *self.func_refs.get(runtime_name).ok_or_else(|| {
                                format!("missing runtime function `{runtime_name}`")
                            })?;
                            let call = self.builder.ins().call(fref, &[list_v]);
                            return Ok(self.builder.inst_results(call)[0]);
                        }
                        // Special-case: iter/list helpers pass closure as (fn_ptr, env_ptr).
                        if matches!(
                            name.as_str(),
                            "ori_list_map"
                                | "ori_list_filter"
                                | "ori_iter_flat_map"
                                | "ori_iter_any"
                                | "ori_iter_all"
                                | "ori_iter_count_where"
                                | "ori_iter_find"
                                | "ori_iter_partition"
                                | "ori_iter_group_by"
                        ) && args.len() == 2
                            && matches!(&args[1].value.ty, Ty::Func { .. })
                        {
                            let list_v = self.emit_expr(&args[0].value)?;
                            let closure_ptr = self.emit_expr(&args[1].value)?;
                            let ptr_size = self.ptr_ty.bytes() as i32;
                            let fn_ptr = self.builder.ins().load(
                                self.ptr_ty,
                                MemFlags::new(),
                                closure_ptr,
                                0,
                            );
                            let env_ptr = self.builder.ins().load(
                                self.ptr_ty,
                                MemFlags::new(),
                                closure_ptr,
                                ptr_size,
                            );
                            if self.func_refs.get(name.as_str()).is_some() {
                                let runtime_name = if name.as_str() == "ori_iter_group_by"
                                    && matches!(
                                        &expr.ty,
                                        Ty::Map(key, _) if matches!(key.as_ref(), Ty::String)
                                    ) {
                                    "ori_iter_group_by_string"
                                } else {
                                    name.as_str()
                                };
                                let fref = *self.func_refs.get(runtime_name).ok_or_else(|| {
                                    format!("missing runtime function `{runtime_name}`")
                                })?;
                                let call =
                                    self.builder.ins().call(fref, &[list_v, fn_ptr, env_ptr]);
                                let res = self.builder.inst_results(call);
                                return Ok(if res.is_empty() {
                                    self.builder.ins().iconst(types::I8, 0)
                                } else {
                                    res[0]
                                });
                            }
                        }
                        if name.as_str() == "ori_iter_reduce"
                            && args.len() == 3
                            && matches!(&args[2].value.ty, Ty::Func { .. })
                        {
                            let list_v = self.emit_expr(&args[0].value)?;
                            let initial_v = self.emit_expr(&args[1].value)?;
                            let closure_ptr = self.emit_expr(&args[2].value)?;
                            let ptr_size = self.ptr_ty.bytes() as i32;
                            let fn_ptr = self.builder.ins().load(
                                self.ptr_ty,
                                MemFlags::new(),
                                closure_ptr,
                                0,
                            );
                            let env_ptr = self.builder.ins().load(
                                self.ptr_ty,
                                MemFlags::new(),
                                closure_ptr,
                                ptr_size,
                            );
                            if let Some(&fref) = self.func_refs.get(name.as_str()) {
                                let call = self
                                    .builder
                                    .ins()
                                    .call(fref, &[list_v, initial_v, fn_ptr, env_ptr]);
                                let res = self.builder.inst_results(call);
                                return Ok(if res.is_empty() {
                                    self.builder.ins().iconst(types::I8, 0)
                                } else {
                                    res[0]
                                });
                            }
                        }
                        if name.as_str() == "ori_iter_sort_by"
                            && args.len() == 2
                            && matches!(&args[1].value.ty, Ty::Func { .. })
                        {
                            let list_v = self.emit_expr(&args[0].value)?;
                            let closure_ptr = self.emit_expr(&args[1].value)?;
                            let ptr_size = self.ptr_ty.bytes() as i32;
                            let fn_ptr = self.builder.ins().load(
                                self.ptr_ty,
                                MemFlags::new(),
                                closure_ptr,
                                0,
                            );
                            let env_ptr = self.builder.ins().load(
                                self.ptr_ty,
                                MemFlags::new(),
                                closure_ptr,
                                ptr_size,
                            );
                            if let Some(&fref) = self.func_refs.get(name.as_str()) {
                                let call =
                                    self.builder.ins().call(fref, &[list_v, fn_ptr, env_ptr]);
                                let res = self.builder.inst_results(call);
                                return Ok(if res.is_empty() {
                                    self.builder.ins().iconst(types::I8, 0)
                                } else {
                                    res[0]
                                });
                            }
                        }
                        // `ori_list_push` stores the element pointer without
                        // retaining; ownership is transferred via ARC edge.
                        // Must not use the generic FFI path (which would free
                        // a fresh managed temporary after the call).
                        if name.as_str() == "ori_list_push" && args.len() == 2 {
                            let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                            let list_v = self.emit_expr(&args[0].value)?;
                            let value_v = self.emit_expr(&args[1].value)?;
                            let elem_ty = match &args[0].value.ty {
                                Ty::List(elem) => elem.as_ref().clone(),
                                _ => args[1].value.ty.clone(),
                            };
                            self.emit_list_push_value(list_v, value_v, &elem_ty)?;
                            // The list edge owns the element's +1; drop an
                            // owned temporary's own +1.
                            if value_is_owned && is_managed_ty(&elem_ty) {
                                self.emit_arc_release_if_managed(&elem_ty, value_v)?;
                            }
                            return Ok(self.builder.ins().iconst(types::I8, 0));
                        }
                        if name.as_str() == "ori_list_set" && args.len() == 3 {
                            let list_is_owned = Self::expr_produces_owned_ref(&args[0].value);
                            let value_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                            let list_v = self.emit_expr(&args[0].value)?;
                            let index_v = self.emit_expr(&args[1].value)?;
                            let value_v = self.emit_expr(&args[2].value)?;
                            let elem_ty = match &args[0].value.ty {
                                Ty::List(elem) => elem.as_ref().clone(),
                                _ => args[2].value.ty.clone(),
                            };
                            self.emit_list_set_value(list_v, index_v, value_v, &elem_ty)?;
                            if value_is_owned && is_managed_ty(&elem_ty) {
                                self.emit_arc_release_if_managed(&elem_ty, value_v)?;
                            }
                            if list_is_owned {
                                self.emit_arc_release_if_managed(&args[0].value.ty, list_v)?;
                            }
                            return Ok(self.builder.ins().iconst(types::I8, 0));
                        }
                        if name.as_str() == "ori_list_insert" && args.len() == 3 {
                            let list_is_owned = Self::expr_produces_owned_ref(&args[0].value);
                            let value_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                            let list_v = self.emit_expr(&args[0].value)?;
                            let index_v = self.emit_expr(&args[1].value)?;
                            let value_v = self.emit_expr(&args[2].value)?;
                            let elem_ty = match &args[0].value.ty {
                                Ty::List(elem) => elem.as_ref().clone(),
                                _ => args[2].value.ty.clone(),
                            };
                            self.emit_list_insert_value(list_v, index_v, value_v, &elem_ty)?;
                            if value_is_owned && is_managed_ty(&elem_ty) {
                                self.emit_arc_release_if_managed(&elem_ty, value_v)?;
                            }
                            if list_is_owned {
                                self.emit_arc_release_if_managed(&args[0].value.ty, list_v)?;
                            }
                            return Ok(self.builder.ins().iconst(types::I8, 0));
                        }
                        let param_tys = self.func_param_tys.get(name).cloned();
                        let is_user_func = self.user_func_names.contains(name.as_str());
                        let mut args_v = Vec::new();
                        // Track owned (fresh +1) managed args so we can release
                        // them after a stdlib FFI call. User functions balance
                        // via callee scope cleanup of the param, but stdlib FFI
                        // borrows args without releasing, so the +1 from the
                        // temporary's malloc would leak.
                        let mut owned_temp_args: Vec<(ir::Value, Ty)> = Vec::new();
                        for (index, arg) in args.iter().enumerate() {
                            if let Some(expected) = param_tys
                                .as_ref()
                                .and_then(|params| params.get(index))
                                .cloned()
                            {
                                let arg_is_owned = Self::expr_produces_owned_ref(&arg.value);
                                let value = self.emit_expr_for_expected(&arg.value, &expected)?;
                                let retain_ty = if expected.contains_infer() {
                                    &arg.value.ty
                                } else {
                                    &expected
                                };
                                if is_user_func && !arg_is_owned {
                                    self.emit_arc_retain_if_managed(retain_ty, value)?;
                                }
                                if !is_user_func && arg_is_owned && is_managed_ty(retain_ty) {
                                    owned_temp_args.push((value, retain_ty.clone()));
                                }
                                args_v.push(value);
                            } else {
                                let arg_is_owned = Self::expr_produces_owned_ref(&arg.value);
                                let value = self.emit_expr(&arg.value)?;
                                if is_user_func && !arg_is_owned {
                                    self.emit_arc_retain_if_managed(&arg.value.ty, value)?;
                                }
                                if !is_user_func && arg_is_owned && is_managed_ty(&arg.value.ty) {
                                    owned_temp_args.push((value, arg.value.ty.clone()));
                                }
                                args_v.push(value);
                            }
                        }
                        if let Some(callback_id) = self.host_callback_ids.get(name) {
                            let callback_id =
                                self.builder.ins().iconst(types::I64, *callback_id as i64);
                            args_v.insert(0, callback_id);
                        }
                        if let Some(&fref) = self.func_refs.get(name.as_str()) {
                            let call = self.builder.ins().call(fref, &args_v);
                            let result = self.builder.inst_results(call).first().copied();
                            // `ori_list_get` borrows the element from its list.
                            // A call expression hands an owned result to its
                            // binding/caller, so acquire that reference before
                            // releasing a temporary list argument. Otherwise a
                            // later scope cleanup frees an element still in the
                            // list (or reads it after the list is removed).
                            if name.as_str() == "ori_list_get" {
                                if let Some(value) = result {
                                    self.emit_arc_retain_if_managed(&expr.ty, value)?;
                                }
                            }
                            // Release fresh managed temporaries passed to
                            // stdlib FFI after the call returns.
                            for (v, ty) in owned_temp_args {
                                self.emit_arc_release_if_managed(&ty, v)?;
                            }
                            result.unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0))
                        } else {
                            return Err(format!(
                                "missing function reference `{name}` in native codegen"
                            ));
                        }
                    }
                } else {
                    return self.emit_closure_call(callee, args);
                }
            }
            HirExprKind::IfExpr { cond, then, else_ } => {
                let cv = self.emit_expr(cond)?;
                let tv = self.emit_expr(then)?;
                let ev = self.emit_expr(else_)?;
                self.builder.ins().select(cv, tv, ev)
            }
            HirExprKind::MatchExpr { scrutinee, arms } => {
                self.emit_match_expr(expr, scrutinee, arms)?
            }
            HirExprKind::Propagate(inner) => {
                // `try expr` / Propagate — load has_value/is_ok; if false, early return; else unwrap
                //
                // Wrapper accounting (LANG-MEM-9): an owned wrapper (fresh
                // Call result) must be consumed on BOTH paths — before this,
                // the ok path abandoned it and the err path retained it
                // without ever dropping the temporary's +1, so every `try`
                // leaked once wrappers became ARC-managed. The unwrapped
                // payload is always produced OWNED (Propagate is listed in
                // `expr_produces_owned_ref`), independent of the wrapper.
                let inner_owned = Self::expr_produces_owned_ref(inner) && is_managed_ty(&inner.ty);
                let ptr = self.emit_expr(inner)?;
                let flag = self.builder.ins().load(types::I8, MemFlags::new(), ptr, 0);
                let ok_blk = self.builder.create_block();
                let err_blk = self.builder.create_block();
                self.builder.ins().brif(flag, ok_blk, &[], err_blk, &[]);
                // Error path: return the whole tagged pointer (propagate error upward)
                self.builder.seal_block(err_blk);
                self.builder.switch_to_block(err_blk);
                self.terminated = false;
                if let Some(frame) = self.async_frame {
                    let plan = self.async_plan.expect("must have async plan");
                    let result_future = self.builder.ins().load(
                        self.ptr_ty,
                        MemFlags::new(),
                        frame,
                        ASYNC_FRAME_RESULT_OFFSET,
                    );
                    // Owned wrapper: skip the retain so the release below
                    // consumes the temporary's +1 (the future takes its own
                    // reference in emit_future_complete).
                    if !inner_owned {
                        self.emit_arc_retain_if_managed(&plan.inner_ty, ptr)?;
                    }
                    self.emit_future_complete(result_future, &plan.inner_ty, Some(ptr))?;
                    self.emit_arc_release_if_managed(&plan.inner_ty, ptr)?;
                    self.emit_scope_cleanup_calls_from(0, 0)?;
                    self.emit_simple_async_frame_cleanup(plan, frame, plan.awaits.len(), true)?;
                    let zero = self.builder.ins().iconst(types::I64, 0);
                    self.builder.ins().return_(&[zero]);
                } else {
                    // Owned wrapper: its +1 transfers to the return value
                    // (return-transfer); borrowed wrappers get a retain for
                    // the caller.
                    if !inner_owned {
                        self.emit_arc_retain_if_managed(&self.current_return_ty.clone(), ptr)?;
                    }
                    self.emit_scope_cleanup_calls_from(0, 0)?;
                    self.builder.ins().return_(&[ptr]);
                }
                self.terminated = true;
                // Ok path: continue with unwrapped value
                self.builder.seal_block(ok_blk);
                self.builder.switch_to_block(ok_blk);
                self.terminated = false;

                let (pay_off, payload_ty, cl_ty) = match &inner.ty {
                    Ty::Result(ok, err) => {
                        let (off, _, _) = result_layout(ok, err, self.ptr_ty);
                        (
                            off as i32,
                            ok.as_ref().clone(),
                            cl_type(ok, self.ptr_ty).unwrap_or(types::I64),
                        )
                    }
                    Ty::Optional(t) => {
                        let (off, _) = optional_layout(t, self.ptr_ty);
                        (
                            off as i32,
                            t.as_ref().clone(),
                            cl_type(t, self.ptr_ty).unwrap_or(types::I64),
                        )
                    }
                    _ => {
                        return Err(format!(
                            "`?` requires optional/result, got `{}`",
                            self.display_ty(&inner.ty)
                        ))
                    }
                };

                let payload = self
                    .builder
                    .ins()
                    .load(cl_ty, MemFlags::new(), ptr, pay_off);
                // The payload leaves the wrapper with its own +1; an owned
                // wrapper is consumed here (releasing it may cascade, which
                // is why the payload is retained first).
                self.emit_arc_retain_if_managed(&payload_ty, payload)?;
                if inner_owned {
                    self.emit_arc_release_if_managed(&inner.ty, ptr)?;
                }
                payload
            }
            HirExprKind::Await(inner) => self.emit_await(inner, &expr.ty)?,
            HirExprKind::StructLit { def_id, fields } => {
                let def_id = def_id
                    .ok_or_else(|| "unresolved struct literal during native codegen".to_string())?;
                if let Some(layout) = self.struct_layouts.get(&def_id).cloned() {
                    // Managed fields are owned by their registered ARC edges
                    // (single cascade owner). A custom destructor may observe
                    // the fields, but it must not release their edge-owned +1.
                    let base = self.malloc_typed_bytes(layout.size, def_id)?;
                    for (fname, fexpr) in fields {
                        let fexpr_is_owned = Self::expr_produces_owned_ref(fexpr);
                        let val = self.emit_expr(fexpr)?;
                        if let Some(fi) = layout.field(fname) {
                            if let Some(contract) = &fi.contract {
                                let message = format!(
                                    "{CONTRACT_FIELD_VIOLATION}: field `{fname}` broke its `if` contract"
                                );
                                self.emit_value_contract(
                                    &fi.ty,
                                    val,
                                    contract,
                                    3,
                                    false,
                                    Some(&message),
                                )?;
                            }
                            // An array field owns its bytes inline, so the value
                            // (an address) has to be copied in, not stored as a
                            // pointer. Scalar elements copy by value; inline
                            // struct elements copy recursively (GFX-INLINE-1).
                            if let Ty::Array(elem_ty, size) = &fi.ty {
                                let len = match &**size {
                                    Ty::ConstInt(_, n) if *n >= 0 => *n,
                                    _ => {
                                        return Err(format!(
                                            "array field `{fname}` has no concrete length"
                                        ))
                                    }
                                };
                                let inline_struct = matches!(elem_ty.as_ref(), Ty::Named(..))
                                    && self.ty_is_inline_for_layout(elem_ty);
                                if inline_struct {
                                    self.emit_copy_inline_into(
                                        val,
                                        &fi.ty,
                                        base,
                                        fi.offset as i32,
                                    )?;
                                } else {
                                    let Some(elem_cl) = cl_type(elem_ty, self.ptr_ty) else {
                                        return Err(format!(
                                            "array field `{fname}` has no Cranelift element type"
                                        ));
                                    };
                                    let (elem_size, _) = field_size_align(elem_ty, self.ptr_ty);
                                    // The length is a compile-time constant and the
                                    // elements are scalars, so an unrolled copy needs
                                    // no target config and no runtime call.
                                    for i in 0..len {
                                        let byte = (i as u32).saturating_mul(elem_size) as i32;
                                        let elem = self.builder.ins().load(
                                            elem_cl,
                                            MemFlags::new(),
                                            val,
                                            byte,
                                        );
                                        self.builder.ins().store(
                                            MemFlags::new(),
                                            elem,
                                            base,
                                            fi.offset as i32 + byte,
                                        );
                                    }
                                }
                            } else if let Ty::Named(..) = &fi.ty {
                                // An inline struct field is copied recursively;
                                // a managed struct field is a stored pointer
                                // (handled in the next arm).
                                if self.ty_is_inline_for_layout(&fi.ty) {
                                    self.emit_copy_inline_into(
                                        val,
                                        &fi.ty,
                                        base,
                                        fi.offset as i32,
                                    )?;
                                } else {
                                    self.builder.ins().store(
                                        MemFlags::new(),
                                        val,
                                        base,
                                        fi.offset as i32,
                                    );
                                    self.emit_arc_register_edge_if_managed(&fi.ty, base, val)?;
                                    if fexpr_is_owned && is_managed_ty(&fi.ty) {
                                        self.emit_arc_release_if_managed(&fi.ty, val)?;
                                    }
                                }
                            } else if cl_type(&fi.ty, self.ptr_ty).is_some() {
                                self.builder.ins().store(
                                    MemFlags::new(),
                                    val,
                                    base,
                                    fi.offset as i32,
                                );
                                self.emit_arc_register_edge_if_managed(&fi.ty, base, val)?;
                                // The edge owns the field's +1. Release the
                                // temporary's own +1 so it does not leak.
                                // Borrowed refs (Var/Field) keep their
                                // binding's ref untouched.
                                if fexpr_is_owned && is_managed_ty(&fi.ty) {
                                    self.emit_arc_release_if_managed(&fi.ty, val)?;
                                }
                            }
                        } else {
                            return Err(format!(
                                "layout for struct literal is missing field `{fname}`"
                            ));
                        }
                    }
                    base
                } else {
                    return Err(format!(
                        "missing native layout for struct literal `{def_id:?}`"
                    ));
                }
            }
            HirExprKind::Field { object, field } => {
                let ptr = self.emit_expr(object)?;
                // Look up layout by DefId embedded in object's type
                let layout_opt = if let Ty::Named(def_id, _) = &object.ty {
                    self.struct_layouts.get(def_id).cloned()
                } else {
                    None
                };
                if let Some(layout) = layout_opt {
                    if let Some(fi) = layout.field(field) {
                        // An array field *is* the bytes at that offset, not a
                        // pointer stored there. Loading it would read the first
                        // element and treat it as an address. The same applies
                        // to inline struct fields (GFX-INLINE-1).
                        let inline_aggregate = matches!(fi.ty, Ty::Array(_, _))
                            || (matches!(fi.ty, Ty::Named(..))
                                && self.ty_is_inline_for_layout(&fi.ty));
                        if inline_aggregate {
                            self.builder.ins().iadd_imm(ptr, fi.offset as i64)
                        } else if let Some(cl_ty) = cl_type(&fi.ty, self.ptr_ty) {
                            self.builder
                                .ins()
                                .load(cl_ty, MemFlags::new(), ptr, fi.offset as i32)
                        } else {
                            return Err(format!("missing Cranelift type for field `{field}`"));
                        }
                    } else {
                        return Err(format!("layout is missing field `{field}`"));
                    }
                } else {
                    return Err(format!(
                        "field access `{field}` requires a struct value, got `{}`",
                        self.display_ty(&object.ty)
                    ));
                }
            }
            HirExprKind::Index { object, index } => {
                let container = self.emit_expr(object)?;
                let idx = self.emit_expr(index)?;
                match &object.ty {
                    Ty::List(elem_ty) => self.emit_list_get_value(container, idx, elem_ty)?,
                    // The elements sit inline, so indexing is address arithmetic
                    // on the array's own storage — no runtime call.
                    Ty::Array(elem_ty, _) => {
                        // An inline struct element is represented by its
                        // address inside the array's storage (like an array
                        // field), so indexing returns the element address.
                        // Scalar elements are loaded by value.
                        let inline_struct = matches!(elem_ty.as_ref(), Ty::Named(..))
                            && self.ty_is_inline_for_layout(elem_ty);
                        let (elem_size, _) = self.field_size_align_for_layout(elem_ty);
                        let stride = self.builder.ins().iconst(types::I64, elem_size as i64);
                        let byte_offset = self.builder.ins().imul(idx, stride);
                        let offset = if self.ptr_ty == types::I64 {
                            byte_offset
                        } else {
                            self.builder.ins().ireduce(self.ptr_ty, byte_offset)
                        };
                        let addr = self.builder.ins().iadd(container, offset);
                        if inline_struct {
                            addr
                        } else {
                            let Some(cl) = cl_type(elem_ty, self.ptr_ty) else {
                                return Err(format!(
                                    "native codegen cannot index `array` of `{}` yet",
                                    self.display_ty(elem_ty)
                                ));
                            };
                            self.builder.ins().load(cl, MemFlags::new(), addr, 0)
                        }
                    }
                    Ty::Simd(elem_ty, _) => {
                        let Some(elem_cl) = cl_type(elem_ty, self.ptr_ty) else {
                            return Err(native_codegen_unsupported(
                                "native codegen cannot index `simd` lane type yet",
                            ));
                        };
                        let (elem_size, _) = self.field_size_align_for_layout(elem_ty);
                        let slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
                            ir::StackSlotKind::ExplicitSlot,
                            32,
                            4,
                        ));
                        self.builder.ins().stack_store(container, slot, 0);
                        let base = self.builder.ins().stack_addr(self.ptr_ty, slot, 0);
                        let stride = self.builder.ins().iconst(types::I64, elem_size as i64);
                        let byte_offset = self.builder.ins().imul(idx, stride);
                        let offset = if self.ptr_ty == types::I64 {
                            byte_offset
                        } else {
                            self.builder.ins().ireduce(self.ptr_ty, byte_offset)
                        };
                        let addr = self.builder.ins().iadd(base, offset);
                        self.builder.ins().load(elem_cl, MemFlags::new(), addr, 0)
                    }
                    Ty::String => {
                        let slice_ref =
                            *self.func_refs.get("ori_string_slice").ok_or_else(|| {
                                "missing runtime function `ori_string_slice`".to_string()
                            })?;
                        let one = self.builder.ins().iconst(types::I64, 1);
                        let end = self.builder.ins().iadd(idx, one);
                        if !self.hosted_traps {
                            let call = self.builder.ins().call(slice_ref, &[container, idx, end]);
                            self.builder.inst_results(call)[0]
                        } else {
                            let len_ref =
                                *self.func_refs.get("ori_string_len").ok_or_else(|| {
                                    "missing runtime function `ori_string_len`".to_string()
                                })?;
                            let len_call = self.builder.ins().call(len_ref, &[container]);
                            let len = self.builder.inst_results(len_call)[0];
                            let zero = self.builder.ins().iconst(types::I64, 0);
                            let ge_zero = self.builder.ins().icmp(
                                ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                                idx,
                                zero,
                            );
                            let lt_len = self.builder.ins().icmp(
                                ir::condcodes::IntCC::SignedLessThan,
                                idx,
                                len,
                            );
                            let in_bounds = self.builder.ins().band(ge_zero, lt_len);
                            let fast = self.builder.create_block();
                            let slow = self.builder.create_block();
                            let join = self.builder.create_block();
                            self.builder.append_block_param(join, self.ptr_ty);
                            self.builder.ins().brif(in_bounds, fast, &[], slow, &[]);

                            self.builder.seal_block(fast);
                            self.builder.switch_to_block(fast);
                            let call = self.builder.ins().call(slice_ref, &[container, idx, end]);
                            let value = self.builder.inst_results(call)[0];
                            self.builder.ins().jump(join, &[BlockArg::Value(value)]);

                            self.builder.seal_block(slow);
                            self.builder.switch_to_block(slow);
                            self.emit_host_error(8, "string index out of bounds")?;
                            let empty_end = self.builder.ins().iconst(types::I64, 0);
                            let empty = self
                                .builder
                                .ins()
                                .call(slice_ref, &[container, empty_end, empty_end]);
                            let empty = self.builder.inst_results(empty)[0];
                            self.builder.ins().jump(join, &[BlockArg::Value(empty)]);

                            self.builder.seal_block(join);
                            self.builder.switch_to_block(join);
                            self.terminated = false;
                            self.builder.block_params(join)[0]
                        }
                    }
                    Ty::Bytes => {
                        let get_ref = *self.func_refs.get("ori_bytes_get").ok_or_else(|| {
                            "missing runtime function `ori_bytes_get`".to_string()
                        })?;
                        if !self.hosted_traps {
                            let call = self.builder.ins().call(get_ref, &[container, idx]);
                            self.builder.inst_results(call)[0]
                        } else {
                            let len_ref =
                                *self.func_refs.get("ori_bytes_len").ok_or_else(|| {
                                    "missing runtime function `ori_bytes_len`".to_string()
                                })?;
                            let len_call = self.builder.ins().call(len_ref, &[container]);
                            let len = self.builder.inst_results(len_call)[0];
                            let zero = self.builder.ins().iconst(types::I64, 0);
                            let ge_zero = self.builder.ins().icmp(
                                ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                                idx,
                                zero,
                            );
                            let lt_len = self.builder.ins().icmp(
                                ir::condcodes::IntCC::SignedLessThan,
                                idx,
                                len,
                            );
                            let in_bounds = self.builder.ins().band(ge_zero, lt_len);
                            let fast = self.builder.create_block();
                            let slow = self.builder.create_block();
                            let join = self.builder.create_block();
                            self.builder.append_block_param(join, types::I8);
                            self.builder.ins().brif(in_bounds, fast, &[], slow, &[]);

                            self.builder.seal_block(fast);
                            self.builder.switch_to_block(fast);
                            let call = self.builder.ins().call(get_ref, &[container, idx]);
                            let value = self.builder.inst_results(call)[0];
                            self.builder.ins().jump(join, &[BlockArg::Value(value)]);

                            self.builder.seal_block(slow);
                            self.builder.switch_to_block(slow);
                            self.emit_host_error(8, "bytes index out of bounds")?;
                            let zero_byte = self.builder.ins().iconst(types::I8, 0);
                            self.builder.ins().jump(join, &[BlockArg::Value(zero_byte)]);

                            self.builder.seal_block(join);
                            self.builder.switch_to_block(join);
                            self.terminated = false;
                            self.builder.block_params(join)[0]
                        }
                    }
                    _ => {
                        return Err(format!(
                            "native index codegen is missing for type `{}`",
                            self.display_ty(&object.ty)
                        ))
                    }
                }
            }
            // An array literal writes straight into a stack slot. No heap block,
            // no length field, no reference counting — the length is in the type
            // and the storage dies with the frame.
            HirExprKind::ArrayLit { elem_ty, elements } => {
                let (elem_size, align) = self.field_size_align_for_layout(elem_ty);
                let inline_struct =
                    matches!(elem_ty, Ty::Named(..)) && self.ty_is_inline_for_layout(elem_ty);
                let total = elem_size.saturating_mul(elements.len() as u32).max(1);
                let slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
                    ir::StackSlotKind::ExplicitSlot,
                    total,
                    align.trailing_zeros() as u8,
                ));
                let base = self.builder.ins().stack_addr(self.ptr_ty, slot, 0);
                for (index, element) in elements.iter().enumerate() {
                    let value = self.emit_expr(element)?;
                    let offset = (index as u32).saturating_mul(elem_size) as i32;
                    if inline_struct {
                        // The element literal is a heap pointer; copy its
                        // bytes into the inline slot.
                        self.emit_copy_inline_into(value, elem_ty, base, offset)?;
                    } else {
                        self.builder
                            .ins()
                            .store(MemFlags::new(), value, base, offset);
                    }
                }
                base
            }
            HirExprKind::SimdLit {
                elem_ty,
                lanes,
                elements,
            } => {
                let vec_cl_ty = cl_type(&expr.ty, self.ptr_ty).unwrap_or(types::F32X4);
                let (elem_size, _) = self.field_size_align_for_layout(elem_ty);
                let total = elem_size.saturating_mul(*lanes as u32).max(16);
                let slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
                    ir::StackSlotKind::ExplicitSlot,
                    total,
                    4,
                ));
                let base = self.builder.ins().stack_addr(self.ptr_ty, slot, 0);
                for (index, element) in elements.iter().enumerate() {
                    let value = self.emit_expr(element)?;
                    let offset = (index as u32).saturating_mul(elem_size) as i32;
                    self.builder
                        .ins()
                        .store(MemFlags::new(), value, base, offset);
                }
                self.builder.ins().stack_load(vec_cl_ty, slot, 0)
            }
            HirExprKind::ListLit { elem_ty, elements } => {
                let list_ptr = self.emit_new_list()?;
                for elem in elements {
                    let owned = Self::expr_produces_owned_ref(elem);
                    let value = self.emit_expr(elem)?;
                    self.emit_list_push_value(list_ptr, value, elem_ty)?;
                    // The list edge owns the element's +1. Release an owned
                    // temporary's own +1 so it does not leak.
                    if owned && is_managed_ty(elem_ty) {
                        self.emit_arc_release_if_managed(elem_ty, value)?;
                    }
                }
                list_ptr
            }
            HirExprKind::ListSpreadLit { elem_ty, elements } => {
                let list_ptr = self.emit_new_list()?;
                for elem in elements {
                    let owned = Self::expr_produces_owned_ref(&elem.value);
                    let value = self.emit_expr(&elem.value)?;
                    if elem.spread {
                        self.emit_list_extend_from(list_ptr, value, elem_ty)?;
                        // Extend copies the elements (each copy gets its own
                        // edge); an owned source list temporary must be
                        // released afterwards.
                        if owned {
                            self.emit_arc_release_if_managed(&elem.value.ty, value)?;
                        }
                    } else {
                        self.emit_list_push_value(list_ptr, value, elem_ty)?;
                        if owned && is_managed_ty(elem_ty) {
                            self.emit_arc_release_if_managed(elem_ty, value)?;
                        }
                    }
                }
                list_ptr
            }
            HirExprKind::Range { start, end } => {
                let sv = self.emit_expr(start)?;
                let ev = self.emit_expr(end)?;
                let base = self.malloc_bytes(16)?;
                self.builder.ins().store(MemFlags::new(), sv, base, 0);
                self.builder.ins().store(MemFlags::new(), ev, base, 8);
                base
            }
            HirExprKind::EnumVariant {
                def_id,
                variant,
                fields,
                ..
            } => {
                let def_id = def_id.ok_or_else(|| {
                    format!("unresolved enum variant `{variant}` during native codegen")
                })?;
                if let Some(layout) = self.enum_layouts.get(&def_id).cloned() {
                    // Payload fields are owned by registered ARC edges only;
                    // see StructLit above.
                    let base = self.malloc_typed_bytes(layout.size, def_id)?;

                    if let Some(v_layout) = layout.variant(variant) {
                        // Store the tag at offset 0
                        let tag = self.builder.ins().iconst(types::I32, v_layout.tag as i64);
                        self.builder.ins().store(MemFlags::new(), tag, base, 0);

                        // Store fields in the payload layout
                        for (fname, fexpr) in fields {
                            let fexpr_is_owned = Self::expr_produces_owned_ref(fexpr);
                            let val = self.emit_expr(fexpr)?;
                            if let Some(fi) = v_layout.fields.field(fname) {
                                let total_offset = (layout.payload_offset + fi.offset) as i32;
                                self.builder
                                    .ins()
                                    .store(MemFlags::new(), val, base, total_offset);
                                self.emit_arc_register_edge_if_managed(&fi.ty, base, val)?;
                                // The edge owns the field's +1. Release the
                                // temporary's own +1 so it does not leak.
                                if fexpr_is_owned && is_managed_ty(&fi.ty) {
                                    self.emit_arc_release_if_managed(&fi.ty, val)?;
                                }
                            } else {
                                return Err(format!("layout for enum variant `{variant}` is missing field `{fname}`"));
                            }
                        }
                    } else {
                        return Err(format!("missing variant `{variant}` in native enum layout"));
                    }
                    base
                } else {
                    return Err(format!(
                        "missing native layout for enum variant `{def_id:?}.{variant}`"
                    ));
                }
            }
            HirExprKind::TupleLit(elems) => {
                let mut vals_and_offsets = Vec::new();
                let elem_tys: Vec<Ty> = elems.iter().map(|e| e.ty.clone()).collect();
                let (layout, total, _) = tuple_layout(&elem_tys, self.ptr_ty);

                for (e, (offset, elem_ty)) in elems.iter().zip(layout.iter()) {
                    let owned = Self::expr_produces_owned_ref(e);
                    let v = self.emit_expr(e)?;
                    vals_and_offsets.push((v, *offset, elem_ty.clone(), owned));
                }
                // Elements are owned by registered ARC edges only; see
                // StructLit above.
                let base = self.malloc_bytes(total)?;

                for (v, off, elem_ty, owned) in vals_and_offsets {
                    self.builder
                        .ins()
                        .store(MemFlags::new(), v, base, off as i32);
                    self.emit_arc_register_edge_if_managed(&elem_ty, base, v)?;
                    // The edge owns the element's +1. Release the temporary's
                    // own +1 so it does not leak; borrowed refs keep their
                    // binding's ref untouched.
                    if owned && is_managed_ty(&elem_ty) {
                        self.emit_arc_release_if_managed(&elem_ty, v)?;
                    }
                }
                base
            }
            HirExprKind::TupleIndex { object, index } => {
                let ptr = self.emit_expr(object)?;
                if let Ty::Tuple(tys) = &object.ty {
                    let (layout, _, _) = tuple_layout(tys, self.ptr_ty);
                    let Some((target_off, target_ty)) = layout.get(*index as usize) else {
                        return Err(format!(
                            "tuple index `{index}` is out of bounds for `{}`",
                            self.display_ty(&object.ty)
                        ));
                    };
                    if let Some(cl) = cl_type(target_ty, self.ptr_ty) {
                        self.builder
                            .ins()
                            .load(cl, MemFlags::new(), ptr, *target_off as i32)
                    } else {
                        return Err(format!("missing Cranelift type for tuple index `{index}`"));
                    }
                } else {
                    return Err(format!(
                        "tuple index on non-tuple type `{}`",
                        self.display_ty(&object.ty)
                    ));
                }
            }
            HirExprKind::MapLit { entries, .. } => {
                let (key_ty, value_ty) = match &expr.ty {
                    Ty::Map(key, value) => (*key.clone(), *value.clone()),
                    _ => (Ty::Infer(0), Ty::Infer(0)),
                };
                let (map_ptr, custom_key) = if let Some(eq_fn) =
                    self.equality_callback_for_type(&key_ty)
                {
                    let hash_fn = self.hash_callback_for_type(&key_ty).ok_or_else(|| {
                        format!("missing hash callback for `{}`", self.display_ty(&key_ty))
                    })?;
                    let new_ref = *self.func_refs.get("ori_map_new_custom").ok_or_else(|| {
                        "missing runtime function `ori_map_new_custom`".to_string()
                    })?;
                    let call = self.builder.ins().call(new_ref, &[hash_fn, eq_fn]);
                    (self.builder.inst_results(call)[0], true)
                } else if let Some(&new_ref) = self.func_refs.get("ori_map_new") {
                    let call = self.builder.ins().call(new_ref, &[]);
                    (self.builder.inst_results(call)[0], false)
                } else {
                    return Err("missing runtime function `ori_map_new`".to_string());
                };
                let set_symbol = if custom_key {
                    "ori_map_set_custom"
                } else if matches!(&key_ty, Ty::String) {
                    "ori_map_set_string"
                } else {
                    "ori_map_set"
                };
                if let Some(&set_ref) = self.func_refs.get(set_symbol) {
                    for (k, v) in entries {
                        let key_value = self.emit_expr(k)?;
                        let map_value = self.emit_expr(v)?;
                        let kv = self.encode_list_storage_value(key_value, &key_ty);
                        let vv = self.encode_list_storage_value(map_value, &value_ty);
                        self.builder.ins().call(set_ref, &[map_ptr, kv, vv]);
                        self.emit_arc_register_edge_if_managed(&key_ty, map_ptr, key_value)?;
                        self.emit_arc_register_edge_if_managed(&value_ty, map_ptr, map_value)?;
                    }
                } else {
                    return Err(format!("missing runtime function `{set_symbol}`"));
                }
                map_ptr
            }
            HirExprKind::SetLit { elements, .. } => {
                let elem_ty = if let Ty::Set(elem_ty) = &expr.ty {
                    elem_ty.as_ref()
                } else {
                    &Ty::Int
                };
                let (set_ptr, custom_element) = if let Some(eq_fn) =
                    self.equality_callback_for_type(elem_ty)
                {
                    let hash_fn = self.hash_callback_for_type(elem_ty).ok_or_else(|| {
                        format!("missing hash callback for `{}`", self.display_ty(elem_ty))
                    })?;
                    let new_ref = *self
                        .func_refs
                        .get("ori_set_new_custom_with_hash")
                        .ok_or_else(|| {
                            "missing runtime function `ori_set_new_custom_with_hash`".to_string()
                        })?;
                    let call = self.builder.ins().call(new_ref, &[hash_fn, eq_fn]);
                    (self.builder.inst_results(call)[0], true)
                } else if let Some(&new_ref) = self.func_refs.get("ori_set_new") {
                    let call = self.builder.ins().call(new_ref, &[]);
                    (self.builder.inst_results(call)[0], false)
                } else {
                    return Err("missing runtime function `ori_set_new`".to_string());
                };
                let add_symbol = if matches!(elem_ty, Ty::String) {
                    "ori_set_add_string"
                } else if custom_element {
                    "ori_set_add_custom_with_hash_eq"
                } else {
                    "ori_set_add"
                };
                if let Some(&add_ref) = self.func_refs.get(add_symbol) {
                    for elem in elements {
                        let v = self.emit_expr_for_expected(elem, elem_ty)?;
                        let stored = self.encode_list_storage_value(v, elem_ty);
                        if custom_element {
                            let hash_fn =
                                self.hash_callback_for_type(elem_ty).ok_or_else(|| {
                                    format!(
                                        "missing hash callback for `{}`",
                                        self.display_ty(elem_ty)
                                    )
                                })?;
                            let eq_fn =
                                self.equality_callback_for_type(elem_ty).ok_or_else(|| {
                                    format!(
                                        "missing equality callback for `{}`",
                                        self.display_ty(elem_ty)
                                    )
                                })?;
                            self.builder
                                .ins()
                                .call(add_ref, &[set_ptr, stored, hash_fn, eq_fn]);
                        } else {
                            self.builder.ins().call(add_ref, &[set_ptr, stored]);
                        }
                        self.emit_arc_register_edge_if_managed(&elem.ty, set_ptr, v)?;
                    }
                } else {
                    return Err(format!("missing runtime function `{add_symbol}`"));
                }
                set_ptr
            }
            HirExprKind::StructUpdate {
                def_id,
                base,
                updates,
            } => {
                let def_id = def_id
                    .ok_or_else(|| "unresolved struct update during native codegen".to_string())?;
                if let Some(layout) = self.struct_layouts.get(&def_id).cloned() {
                    let base_ptr = self.emit_expr(base)?;
                    let new_ptr = self.malloc_typed_bytes(layout.size, def_id)?;
                    let updated_names: Vec<_> =
                        updates.iter().map(|(name, _)| name.clone()).collect();
                    // Copy all bytes from base
                    for (fname, fl) in &layout.fields {
                        if let Some(cl) = cl_type(&fl.ty, self.ptr_ty) {
                            let val = self.builder.ins().load(
                                cl,
                                MemFlags::new(),
                                base_ptr,
                                fl.offset as i32,
                            );
                            self.builder.ins().store(
                                MemFlags::new(),
                                val,
                                new_ptr,
                                fl.offset as i32,
                            );
                            if !updated_names.iter().any(|name| name == fname) {
                                self.emit_arc_register_edge_if_managed(&fl.ty, new_ptr, val)?;
                            }
                        }
                    }
                    // Override updated fields
                    for (fname, fexpr) in updates {
                        let val = self.emit_expr(fexpr)?;
                        if let Some(fi) = layout.field(fname) {
                            if cl_type(&fi.ty, self.ptr_ty).is_some() {
                                self.builder.ins().store(
                                    MemFlags::new(),
                                    val,
                                    new_ptr,
                                    fi.offset as i32,
                                );
                                self.emit_arc_register_edge_if_managed(&fi.ty, new_ptr, val)?;
                            }
                        }
                    }
                    new_ptr
                } else {
                    return Err(format!(
                        "missing native layout for struct update `{def_id:?}`"
                    ));
                }
            }
            HirExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                if matches!(&receiver.ty, Ty::Any(_)) {
                    return self.emit_dynamic_method_call(receiver, method, args);
                }
                let recv = self.emit_expr(receiver)?;
                match method.as_str() {
                    "__slice" => {
                        let runtime_name = match &receiver.ty {
                            Ty::String => "ori_string_slice",
                            Ty::List(_) => "ori_list_slice",
                            Ty::Bytes => "ori_bytes_slice",
                            other => {
                                return Err(format!(
                                    "native range slice codegen is missing for type `{}`",
                                    self.display_ty(other)
                                ))
                            }
                        };
                        let slice_ref = *self
                            .func_refs
                            .get(runtime_name)
                            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
                        let start = self.emit_expr(&args[0])?;
                        let end = self.emit_expr(&args[1])?;
                        let call = self.builder.ins().call(slice_ref, &[recv, start, end]);
                        self.builder.inst_results(call)[0]
                    }
                    _ => {
                        // Generic trait calls are lowered while the receiver may still be
                        // a type parameter. After monomorphization the receiver is concrete,
                        // so resolve the trait implementation symbol for that concrete type.
                        let target_method = match &receiver.ty {
                            Ty::Named(type_def_id, _) => self
                                .trait_method_func_name_for_type(*type_def_id, method)
                                .unwrap_or_else(|| method.clone()),
                            _ => method.clone(),
                        };
                        let mut all_args = vec![recv];
                        self.emit_arc_retain_if_managed(&receiver.ty, recv)?;
                        for a in args {
                            let value = self.emit_expr(a)?;
                            self.emit_arc_retain_if_managed(&a.ty, value)?;
                            all_args.push(value);
                        }
                        if let Some(&fref) = self.func_refs.get(target_method.as_str()) {
                            let call = self.builder.ins().call(fref, &all_args);
                            let res = self.builder.inst_results(call);
                            if res.is_empty() {
                                self.builder.ins().iconst(types::I8, 0)
                            } else {
                                res[0]
                            }
                        } else {
                            return Err(format!(
                                "missing function reference `{target_method}` in native codegen"
                            ));
                        }
                    }
                }
            }
            HirExprKind::AssociatedCall {
                receiver_ty,
                method,
                args,
            } => {
                let Ty::Named(type_def_id, _) = receiver_ty else {
                    return Err(format!(
                        "associated call `{method}` reached native codegen without a concrete type"
                    ));
                };
                let target = self
                    .associated_func_name_for_type(*type_def_id, method)
                    .ok_or_else(|| {
                        format!(
                            "missing associated implementation `{}` for `{}`",
                            method,
                            self.display_ty(receiver_ty)
                        )
                    })?;
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    let value = self.emit_expr(arg)?;
                    self.emit_arc_retain_if_managed(&arg.ty, value)?;
                    values.push(value);
                }
                let function = *self
                    .func_refs
                    .get(target.as_str())
                    .ok_or_else(|| format!("missing function reference `{target}`"))?;
                let call = self.builder.ins().call(function, &values);
                self.builder
                    .inst_results(call)
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0))
            }
            HirExprKind::Closure {
                func_name,
                captures,
            } => self.emit_closure_value(func_name, captures)?,
            HirExprKind::IsCheck { value, check_ty } => {
                let val = self.emit_expr(value)?;
                if let Ty::Named(check_def_id, _) = check_ty {
                    if matches!(&value.ty, Ty::Any(_)) {
                        let ptr_size = self.ptr_ty.bytes() as i64;
                        let vtable = self.builder.ins().load(
                            self.ptr_ty,
                            MemFlags::new(),
                            val,
                            ptr_size as i32,
                        );
                        let actual_type_id =
                            self.builder
                                .ins()
                                .load(self.ptr_ty, MemFlags::new(), vtable, 0);
                        let expected_type_id = self
                            .builder
                            .ins()
                            .iconst(self.ptr_ty, check_def_id.0 as i64);

                        let is_match = self.builder.ins().icmp(
                            ir::condcodes::IntCC::Equal,
                            actual_type_id,
                            expected_type_id,
                        );
                        is_match
                    } else if let Ty::Named(actual_def_id, _) = &value.ty {
                        let is_match = actual_def_id.0 == check_def_id.0;
                        self.builder
                            .ins()
                            .iconst(types::I8, if is_match { 1 } else { 0 })
                    } else {
                        self.builder.ins().iconst(types::I8, 0)
                    }
                } else {
                    self.builder
                        .ins()
                        .iconst(types::I8, if value.ty == *check_ty { 1 } else { 0 })
                }
            }
        })
    }

    /// Compute the byte length of a nul-terminated Ori string pointer as an i64.
    /// This is used by print/interpolation paths that write raw bytes.
    fn str_len_from_ptr(&mut self, ptr: ir::Value) -> Result<ir::Value, String> {
        if let Some(&fref) = self.func_refs.get("strlen") {
            let call = self.builder.ins().call(fref, &[ptr]);
            return Ok(self.builder.inst_results(call)[0]);
        }
        if let Some(&fref) = self.func_refs.get("ori_string_len") {
            let call = self.builder.ins().call(fref, &[ptr]);
            return Ok(self.builder.inst_results(call)[0]);
        }
        Err("missing runtime function `ori_string_len` or `strlen`".to_string())
    }

    fn emit_test_assert_equality_call(
        &mut self,
        name: &str,
        args: &[HirArg],
    ) -> Result<ir::Value, String> {
        if args.len() != 2 {
            return Err(format!("{name} expects two arguments"));
        }
        let ty = &args[0].value.ty;
        let is_ne = name == "ori_test_assert_ne";
        let runtime_name = match ty {
            Ty::String => {
                if is_ne {
                    "ori_test_assert_ne_string"
                } else {
                    "ori_test_assert_eq_string"
                }
            }
            Ty::Float | Ty::Float32 | Ty::Float64 => {
                if is_ne {
                    "ori_test_assert_ne_float"
                } else {
                    "ori_test_assert_eq_float"
                }
            }
            Ty::Bool => {
                if is_ne {
                    "ori_test_assert_ne_bool"
                } else {
                    "ori_test_assert_eq_bool"
                }
            }
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        let mut left = self.emit_expr_for_expected(&args[0].value, ty)?;
        let mut right = self.emit_expr_for_expected(&args[1].value, ty)?;
        match ty {
            Ty::Float32 => {
                left = self.builder.ins().fpromote(types::F64, left);
                right = self.builder.ins().fpromote(types::F64, right);
            }
            Ty::Int8 | Ty::Int16 | Ty::Int32 => {
                left = self.builder.ins().sextend(types::I64, left);
                right = self.builder.ins().sextend(types::I64, right);
            }
            Ty::U8 | Ty::U16 | Ty::U32 => {
                left = self.builder.ins().uextend(types::I64, left);
                right = self.builder.ins().uextend(types::I64, right);
            }
            _ => {}
        }
        self.builder.ins().call(fref, &[left, right]);
        Ok(self.builder.ins().iconst(types::I8, 0))
    }

    fn emit_map_runtime_call(&mut self, name: &str, args: &[HirArg]) -> Result<ir::Value, String> {
        if name == "ori_map_from_entries" {
            if args.len() != 1 {
                return Err("ori_map_from_entries expects one entries list".to_string());
            }
            let key_ty = match &args[0].value.ty {
                Ty::List(inner) => match inner.as_ref() {
                    Ty::Tuple(items) if items.len() == 2 => items[0].clone(),
                    _ => Ty::Infer(0),
                },
                _ => Ty::Infer(0),
            };
            let custom_key = matches!(key_ty, Ty::Named(_, _));
            let runtime_name = if custom_key {
                "ori_map_from_entries_custom"
            } else if matches!(key_ty, Ty::String) {
                "ori_map_from_entries_string"
            } else {
                name
            };
            let fref = *self
                .func_refs
                .get(runtime_name)
                .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
            let entries_is_owned = Self::expr_produces_owned_ref(&args[0].value);
            let entries = self.emit_expr(&args[0].value)?;
            let call = if custom_key {
                let eq_fn = self.equality_callback_for_type(&key_ty).ok_or_else(|| {
                    format!(
                        "missing equality callback for `{}`",
                        self.display_ty(&key_ty)
                    )
                })?;
                let hash_fn = self.hash_callback_for_type(&key_ty).ok_or_else(|| {
                    format!("missing hash callback for `{}`", self.display_ty(&key_ty))
                })?;
                self.builder.ins().call(fref, &[entries, hash_fn, eq_fn])
            } else {
                self.builder.ins().call(fref, &[entries])
            };
            // The new map copies/retains the entries itself; an owned
            // entries-list temporary must be released after the call.
            if entries_is_owned {
                self.emit_arc_release_if_managed(&args[0].value.ty, entries)?;
            }
            return Ok(self.builder.inst_results(call)[0]);
        }
        let Some(first_arg) = args.first() else {
            return Err(format!("map runtime call `{name}` expects a map argument"));
        };
        let Ty::Map(key_ty, value_ty) = &first_arg.value.ty else {
            return Err(format!(
                "map runtime call `{name}` received `{}`",
                self.display_ty(&first_arg.value.ty)
            ));
        };
        let key_ty = key_ty.as_ref();
        let value_ty = value_ty.as_ref();
        let map_v = self.emit_expr(&first_arg.value)?;
        let custom_key = matches!(key_ty, Ty::Named(_, _));
        let runtime_name = match (name, key_ty) {
            ("ori_map_set", Ty::String) => "ori_map_set_string",
            ("ori_map_get", Ty::String) => "ori_map_get_string",
            ("ori_map_try_get", Ty::String) => "ori_map_try_get_string",
            ("ori_map_contains", Ty::String) => "ori_map_contains_string",
            ("ori_map_remove", Ty::String) => "ori_map_remove_string",
            ("ori_map_try_remove", Ty::String) => "ori_map_try_remove_string",
            ("ori_map_from_entries", Ty::String) => "ori_map_from_entries_string",
            ("ori_map_set", Ty::Named(_, _)) => "ori_map_set_custom_with_hash_eq",
            ("ori_map_get", Ty::Named(_, _)) => "ori_map_get_custom",
            ("ori_map_try_get", Ty::Named(_, _)) => "ori_map_try_get_custom",
            ("ori_map_contains", Ty::Named(_, _)) => "ori_map_contains_custom",
            ("ori_map_remove", Ty::Named(_, _)) => "ori_map_remove_custom",
            ("ori_map_try_remove", Ty::Named(_, _)) => "ori_map_try_remove_custom",
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        match name {
            "ori_map_set" => {
                if args.len() != 3 {
                    return Err("ori_map_set expects map, key, and value".to_string());
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let map_value = self.emit_expr_for_expected(&args[2].value, value_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let stored_value = self.encode_list_storage_value(map_value, value_ty);
                if custom_key {
                    let eq_fn = self.equality_callback_for_type(key_ty).ok_or_else(|| {
                        format!(
                            "missing equality callback for `{}`",
                            self.display_ty(key_ty)
                        )
                    })?;
                    let hash_fn = self.hash_callback_for_type(key_ty).ok_or_else(|| {
                        format!("missing hash callback for `{}`", self.display_ty(key_ty))
                    })?;
                    self.builder
                        .ins()
                        .call(fref, &[map_v, stored_key, stored_value, hash_fn, eq_fn]);
                } else {
                    self.builder
                        .ins()
                        .call(fref, &[map_v, stored_key, stored_value]);
                }
                // `ori_map_set*` owns edge registration for both key and value.
                // Registering again here would create two logical slots for one
                // physical map entry and leak one retain on remove/destruction.
                // Drop only the caller's fresh temporaries (borrowed refs stay
                // untouched).
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                if value_is_owned && is_managed_ty(value_ty) {
                    self.emit_arc_release_if_managed(value_ty, map_value)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_map_get" => {
                if args.len() != 2 {
                    return Err("ori_map_get expects map and key".to_string());
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let call = self.builder.ins().call(fref, &[map_v, stored_key]);
                let stored_value = self.builder.inst_results(call)[0];
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                let result = self.decode_list_storage_value(stored_value, value_ty);
                // Calls produce owned references: retain the managed result
                // so the caller's release does not steal the map's edge +1.
                self.emit_arc_retain_if_managed(value_ty, result)?;
                Ok(result)
            }
            "ori_map_try_get" | "ori_map_try_remove" => {
                if args.len() != 2 {
                    return Err(format!("{name} expects map and key"));
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let call = self.builder.ins().call(fref, &[map_v, stored_key]);
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_map_contains" => {
                if args.len() != 2 {
                    return Err("ori_map_contains expects map and key".to_string());
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let call = self.builder.ins().call(fref, &[map_v, stored_key]);
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_map_remove" => {
                if args.len() != 2 {
                    return Err("ori_map_remove expects map and key".to_string());
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                self.builder.ins().call(fref, &[map_v, stored_key]);
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            _ => Err(native_codegen_unsupported(format!(
                "map runtime call `{name}`"
            ))),
        }
    }

    fn emit_hash_table_runtime_call(
        &mut self,
        name: &str,
        args: &[HirArg],
    ) -> Result<ir::Value, String> {
        let Some(first_arg) = args.first() else {
            return Err(format!(
                "hash_table runtime call `{name}` expects a table argument"
            ));
        };
        let Ty::Opaque {
            kind: OpaqueTy::HashTable,
            args: table_args,
        } = &first_arg.value.ty
        else {
            return Err(format!(
                "hash_table runtime call `{name}` received `{}`",
                self.display_ty(&first_arg.value.ty)
            ));
        };
        if table_args.len() != 2 {
            return Err(format!(
                "hash_table runtime call `{name}` received malformed table type"
            ));
        }
        let key_ty = &table_args[0];
        let value_ty = &table_args[1];
        let table_v = self.emit_expr(&first_arg.value)?;
        let custom_key = matches!(key_ty, Ty::Named(_, _));
        let runtime_name = match (name, key_ty) {
            ("ori_hash_table_set", Ty::String) => "ori_hash_table_set_string",
            ("ori_hash_table_get", Ty::String) => "ori_hash_table_get_string",
            ("ori_hash_table_remove", Ty::String) => "ori_hash_table_remove_string",
            ("ori_hash_table_contains", Ty::String) => "ori_hash_table_contains_string",
            ("ori_hash_table_set", Ty::Named(_, _)) => "ori_hash_table_set_custom_with_hash_eq",
            ("ori_hash_table_get", Ty::Named(_, _)) => "ori_hash_table_get_custom",
            ("ori_hash_table_remove", Ty::Named(_, _)) => "ori_hash_table_remove_custom",
            ("ori_hash_table_contains", Ty::Named(_, _)) => "ori_hash_table_contains_custom",
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        match name {
            "ori_hash_table_set" => {
                if args.len() != 3 {
                    return Err("ori_hash_table_set expects table, key, and value".to_string());
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let table_value = self.emit_expr_for_expected(&args[2].value, value_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let stored_value = self.encode_list_storage_value(table_value, value_ty);
                if custom_key {
                    let eq_fn = self.equality_callback_for_type(key_ty).ok_or_else(|| {
                        format!(
                            "missing equality callback for `{}`",
                            self.display_ty(key_ty)
                        )
                    })?;
                    let hash_fn = self.hash_callback_for_type(key_ty).ok_or_else(|| {
                        format!("missing hash callback for `{}`", self.display_ty(key_ty))
                    })?;
                    self.builder
                        .ins()
                        .call(fref, &[table_v, stored_key, stored_value, hash_fn, eq_fn]);
                } else {
                    self.builder
                        .ins()
                        .call(fref, &[table_v, stored_key, stored_value]);
                }
                self.emit_arc_register_edge_if_managed(key_ty, table_v, key_value)?;
                self.emit_arc_register_edge_if_managed(value_ty, table_v, table_value)?;
                // The container owns stored/lookup managed values via its
                // internal edges; drop an owned temporary's own +1.
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                if value_is_owned && is_managed_ty(value_ty) {
                    self.emit_arc_release_if_managed(value_ty, table_value)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_hash_table_get" | "ori_hash_table_remove" => {
                if args.len() != 2 {
                    return Err(format!("{name} expects table and key"));
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let call = self.builder.ins().call(fref, &[table_v, stored_key]);
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_hash_table_contains" => {
                if args.len() != 2 {
                    return Err("ori_hash_table_contains expects table and key".to_string());
                }
                let key_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let key_value = self.emit_expr_for_expected(&args[1].value, key_ty)?;
                let stored_key = self.encode_list_storage_value(key_value, key_ty);
                let call = self.builder.ins().call(fref, &[table_v, stored_key]);
                if key_is_owned && is_managed_ty(key_ty) {
                    self.emit_arc_release_if_managed(key_ty, key_value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            _ => Err(native_codegen_unsupported(format!(
                "hash_table runtime call `{name}`"
            ))),
        }
    }

    fn emit_linked_list_find_runtime_call(
        &mut self,
        name: &str,
        args: &[HirArg],
    ) -> Result<ir::Value, String> {
        if args.len() != 2 {
            return Err(format!("{name} expects list and value"));
        }
        let elem_ty = match &args[0].value.ty {
            Ty::Opaque {
                kind: OpaqueTy::LinkedList | OpaqueTy::DoublyLinkedList,
                args,
            } => args.first().cloned().unwrap_or(Ty::Infer(0)),
            other => {
                return Err(format!(
                    "linked-list find runtime call `{name}` received `{}`",
                    self.display_ty(other)
                ))
            }
        };
        let runtime_name = match (name, &elem_ty) {
            ("ori_linked_list_find", Ty::String) => "ori_linked_list_find_string",
            ("ori_doubly_linked_list_find", Ty::String) => "ori_doubly_linked_list_find_string",
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        let list = self.emit_expr(&args[0].value)?;
        let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
        let value = self.emit_expr_for_expected(&args[1].value, &elem_ty)?;
        let stored = self.encode_list_storage_value(value, &elem_ty);
        let call = self.builder.ins().call(fref, &[list, stored]);
        // Lookup only borrows the probe value; drop an owned temporary.
        if value_is_owned && is_managed_ty(&elem_ty) {
            self.emit_arc_release_if_managed(&elem_ty, value)?;
        }
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_graph_runtime_call(
        &mut self,
        name: &str,
        args: &[HirArg],
    ) -> Result<ir::Value, String> {
        let Some(first_arg) = args.first() else {
            return Err(format!(
                "graph runtime call `{name}` expects a graph argument"
            ));
        };
        let Ty::Opaque {
            kind: OpaqueTy::Graph,
            args: graph_args,
        } = &first_arg.value.ty
        else {
            return Err(format!(
                "graph runtime call `{name}` received `{}`",
                self.display_ty(&first_arg.value.ty)
            ));
        };
        let node_ty = graph_args
            .first()
            .filter(|ty| !ty.contains_infer())
            .cloned()
            .or_else(|| args.get(1).map(|arg| arg.value.ty.clone()))
            .unwrap_or(Ty::Infer(0));
        let runtime_name = match (name, &node_ty) {
            ("ori_graph_add_node", Ty::String) => "ori_graph_add_node_string",
            ("ori_graph_remove_node", Ty::String) => "ori_graph_remove_node_string",
            ("ori_graph_add_edge", Ty::String) => "ori_graph_add_edge_string",
            ("ori_graph_add_weighted_edge", Ty::String) => "ori_graph_add_weighted_edge_string",
            ("ori_graph_remove_edge", Ty::String) => "ori_graph_remove_edge_string",
            ("ori_graph_has_node", Ty::String) => "ori_graph_has_node_string",
            ("ori_graph_has_edge", Ty::String) => "ori_graph_has_edge_string",
            ("ori_graph_edge_weight", Ty::String) => "ori_graph_edge_weight_string",
            ("ori_graph_neighbors", Ty::String) => "ori_graph_neighbors_string",
            ("ori_graph_bfs", Ty::String) => "ori_graph_bfs_string",
            ("ori_graph_dfs", Ty::String) => "ori_graph_dfs_string",
            ("ori_graph_shortest_path", Ty::String) => "ori_graph_shortest_path_string",
            ("ori_graph_shortest_weighted_path", Ty::String) => {
                "ori_graph_shortest_weighted_path_string"
            }
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        let graph = self.emit_expr(&first_arg.value)?;
        if let Some(eq_fn) = self.equality_callback_for_type(&node_ty) {
            let set_eq_ref = *self
                .func_refs
                .get("ori_graph_set_eq")
                .ok_or_else(|| "missing runtime function `ori_graph_set_eq`".to_string())?;
            self.builder.ins().call(set_eq_ref, &[graph, eq_fn]);
        }
        match name {
            "ori_graph_add_node" => {
                if args.len() != 2 {
                    return Err("ori_graph_add_node expects graph and node".to_string());
                }
                let node_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let node = self.emit_expr_for_expected(&args[1].value, &node_ty)?;
                let stored = self.encode_list_storage_value(node, &node_ty);
                self.builder.ins().call(fref, &[graph, stored]);
                // The graph owns stored nodes via its internal edges; drop
                // an owned temporary's own +1.
                if node_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, node)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_graph_remove_node"
            | "ori_graph_has_node"
            | "ori_graph_neighbors"
            | "ori_graph_bfs"
            | "ori_graph_dfs" => {
                if args.len() != 2 {
                    return Err(format!("{name} expects graph and node"));
                }
                let node_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let node = self.emit_expr_for_expected(&args[1].value, &node_ty)?;
                let stored = self.encode_list_storage_value(node, &node_ty);
                let call = self.builder.ins().call(fref, &[graph, stored]);
                if node_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, node)?;
                }
                let res = self.builder.inst_results(call);
                Ok(res
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0)))
            }
            "ori_graph_add_edge" => {
                if args.len() != 3 {
                    return Err("ori_graph_add_edge expects graph, from, and to".to_string());
                }
                let from_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let to_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let from = self.emit_expr_for_expected(&args[1].value, &node_ty)?;
                let to = self.emit_expr_for_expected(&args[2].value, &node_ty)?;
                let stored_from = self.encode_list_storage_value(from, &node_ty);
                let stored_to = self.encode_list_storage_value(to, &node_ty);
                self.builder
                    .ins()
                    .call(fref, &[graph, stored_from, stored_to]);
                if from_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, from)?;
                }
                if to_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, to)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_graph_add_weighted_edge" => {
                if args.len() != 4 {
                    return Err(
                        "ori_graph_add_weighted_edge expects graph, from, to, and weight"
                            .to_string(),
                    );
                }
                let from_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let to_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let from = self.emit_expr_for_expected(&args[1].value, &node_ty)?;
                let to = self.emit_expr_for_expected(&args[2].value, &node_ty)?;
                let weight = self.emit_expr_for_expected(&args[3].value, &Ty::Int)?;
                let stored_from = self.encode_list_storage_value(from, &node_ty);
                let stored_to = self.encode_list_storage_value(to, &node_ty);
                self.builder
                    .ins()
                    .call(fref, &[graph, stored_from, stored_to, weight]);
                if from_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, from)?;
                }
                if to_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, to)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_graph_remove_edge"
            | "ori_graph_has_edge"
            | "ori_graph_edge_weight"
            | "ori_graph_shortest_path"
            | "ori_graph_shortest_weighted_path" => {
                if args.len() != 3 {
                    return Err(format!("{name} expects graph, from, and to"));
                }
                let from_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let to_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let from = self.emit_expr_for_expected(&args[1].value, &node_ty)?;
                let to = self.emit_expr_for_expected(&args[2].value, &node_ty)?;
                let stored_from = self.encode_list_storage_value(from, &node_ty);
                let stored_to = self.encode_list_storage_value(to, &node_ty);
                let call = self
                    .builder
                    .ins()
                    .call(fref, &[graph, stored_from, stored_to]);
                if from_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, from)?;
                }
                if to_is_owned && is_managed_ty(&node_ty) {
                    self.emit_arc_release_if_managed(&node_ty, to)?;
                }
                let res = self.builder.inst_results(call);
                Ok(res
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0)))
            }
            _ => Err(native_codegen_unsupported(format!(
                "graph runtime call `{name}`"
            ))),
        }
    }

    fn emit_set_runtime_call(&mut self, name: &str, args: &[HirArg]) -> Result<ir::Value, String> {
        if name == "ori_set_from_list" {
            if args.len() != 1 {
                return Err("ori_set_from_list expects one source list".to_string());
            }
            let elem_ty = match &args[0].value.ty {
                Ty::List(elem) => elem.as_ref(),
                other => {
                    return Err(format!(
                        "ori_set_from_list expects list input, got `{}`",
                        self.display_ty(other)
                    ))
                }
            };
            let custom_element = matches!(elem_ty, Ty::Named(_, _));
            let runtime_name = if custom_element {
                "ori_set_from_list_custom_with_hash"
            } else if matches!(elem_ty, Ty::String) {
                "ori_set_from_list_string"
            } else {
                name
            };
            let fref = *self
                .func_refs
                .get(runtime_name)
                .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
            let source_is_owned = Self::expr_produces_owned_ref(&args[0].value);
            let source = self.emit_expr(&args[0].value)?;
            let call = if custom_element {
                let eq_fn = self.equality_callback_for_type(elem_ty).ok_or_else(|| {
                    format!(
                        "missing equality callback for `{}`",
                        self.display_ty(elem_ty)
                    )
                })?;
                let hash_fn = self.hash_callback_for_type(elem_ty).ok_or_else(|| {
                    format!("missing hash callback for `{}`", self.display_ty(elem_ty))
                })?;
                self.builder.ins().call(fref, &[source, hash_fn, eq_fn])
            } else {
                self.builder.ins().call(fref, &[source])
            };
            // The new set copies/retains the elements itself; an owned
            // source-list temporary must be released after the call.
            if source_is_owned {
                self.emit_arc_release_if_managed(&args[0].value.ty, source)?;
            }
            return Ok(self.builder.inst_results(call)[0]);
        }
        let Some(first_arg) = args.first() else {
            return Err(format!("set runtime call `{name}` expects a set argument"));
        };
        let Ty::Set(elem_ty) = &first_arg.value.ty else {
            return Err(format!(
                "set runtime call `{name}` received `{}`",
                self.display_ty(&first_arg.value.ty)
            ));
        };
        let elem_ty = elem_ty.as_ref();
        let set_v = self.emit_expr(&first_arg.value)?;
        let custom_element = matches!(elem_ty, Ty::Named(_, _));
        let runtime_name = match (name, elem_ty) {
            ("ori_set_add", Ty::String) => "ori_set_add_string",
            ("ori_set_contains", Ty::String) => "ori_set_contains_string",
            ("ori_set_remove", Ty::String) => "ori_set_remove_string",
            ("ori_set_try_remove", Ty::String) => "ori_set_try_remove_string",
            ("ori_set_from_list", Ty::String) => "ori_set_from_list_string",
            ("ori_set_add", Ty::Named(_, _)) => "ori_set_add_custom_with_hash_eq",
            ("ori_set_contains", Ty::Named(_, _)) => "ori_set_contains_custom",
            ("ori_set_remove", Ty::Named(_, _)) => "ori_set_remove_custom",
            ("ori_set_try_remove", Ty::Named(_, _)) => "ori_set_try_remove_custom",
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        match name {
            "ori_set_add" => {
                if args.len() != 2 {
                    return Err("ori_set_add expects set and value".to_string());
                }
                let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value = self.emit_expr_for_expected(&args[1].value, elem_ty)?;
                let stored = self.encode_list_storage_value(value, elem_ty);
                if custom_element {
                    let eq_fn = self.equality_callback_for_type(elem_ty).ok_or_else(|| {
                        format!(
                            "missing equality callback for `{}`",
                            self.display_ty(elem_ty)
                        )
                    })?;
                    let hash_fn = self.hash_callback_for_type(elem_ty).ok_or_else(|| {
                        format!("missing hash callback for `{}`", self.display_ty(elem_ty))
                    })?;
                    self.builder
                        .ins()
                        .call(fref, &[set_v, stored, hash_fn, eq_fn]);
                } else {
                    self.builder.ins().call(fref, &[set_v, stored]);
                }
                self.emit_arc_register_edge_if_managed(elem_ty, set_v, value)?;
                // The set edge owns the stored element's +1; drop an owned
                // temporary's own +1 (borrowed refs stay untouched).
                if value_is_owned && is_managed_ty(elem_ty) {
                    self.emit_arc_release_if_managed(elem_ty, value)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_set_contains" => {
                if args.len() != 2 {
                    return Err("ori_set_contains expects set and value".to_string());
                }
                let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value = self.emit_expr_for_expected(&args[1].value, elem_ty)?;
                let stored = self.encode_list_storage_value(value, elem_ty);
                let call = self.builder.ins().call(fref, &[set_v, stored]);
                if value_is_owned && is_managed_ty(elem_ty) {
                    self.emit_arc_release_if_managed(elem_ty, value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_set_remove" | "ori_set_try_remove" => {
                if args.len() != 2 {
                    return Err(format!("{name} expects set and value"));
                }
                let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value = self.emit_expr_for_expected(&args[1].value, elem_ty)?;
                let stored = self.encode_list_storage_value(value, elem_ty);
                let call = self.builder.ins().call(fref, &[set_v, stored]);
                if value_is_owned && is_managed_ty(elem_ty) {
                    self.emit_arc_release_if_managed(elem_ty, value)?;
                }
                if name == "ori_set_try_remove" {
                    Ok(self.builder.inst_results(call)[0])
                } else {
                    Ok(self.builder.ins().iconst(types::I8, 0))
                }
            }
            _ => Err(native_codegen_unsupported(format!(
                "set runtime call `{name}`"
            ))),
        }
    }

    fn emit_tree_runtime_call(&mut self, name: &str, args: &[HirArg]) -> Result<ir::Value, String> {
        let node_id_ty = Ty::Opaque {
            kind: OpaqueTy::NodeId,
            args: vec![],
        };
        let tree_elem_ty = |arg: &HirArg| match &arg.value.ty {
            Ty::Opaque {
                kind: OpaqueTy::Tree,
                args,
            } => args.first().cloned(),
            _ => None,
        };
        let elem_ty = args.first().and_then(tree_elem_ty).unwrap_or(Ty::Infer(0));
        let runtime_name = match (name, &elem_ty) {
            ("ori_tree_find", Ty::String) => "ori_tree_find_string",
            _ => name,
        };
        let fref = *self
            .func_refs
            .get(runtime_name)
            .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
        match name {
            "ori_tree_new" => {
                if args.len() != 1 {
                    return Err("ori_tree_new expects one root value".to_string());
                }
                let elem_ty = args[0].value.ty.clone();
                let value_is_owned = Self::expr_produces_owned_ref(&args[0].value);
                let value = self.emit_expr(&args[0].value)?;
                let stored = self.encode_list_storage_value(value, &elem_ty);
                let call = self.builder.ins().call(fref, &[stored]);
                let tree = self.builder.inst_results(call)[0];
                // The tree owns stored managed values via its internal
                // edges; drop an owned temporary's own +1.
                if value_is_owned && is_managed_ty(&elem_ty) {
                    self.emit_arc_release_if_managed(&elem_ty, value)?;
                }
                Ok(tree)
            }
            "ori_tree_add_child" => {
                if args.len() != 3 {
                    return Err("ori_tree_add_child expects tree, parent, and value".to_string());
                }
                let tree = self.emit_expr(&args[0].value)?;
                let parent = self.emit_expr_for_expected(&args[1].value, &node_id_ty)?;
                let value_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let value = self.emit_expr_for_expected(&args[2].value, &elem_ty)?;
                let stored = self.encode_list_storage_value(value, &elem_ty);
                let call = self.builder.ins().call(fref, &[tree, parent, stored]);
                let node = self.builder.inst_results(call)[0];
                if value_is_owned && is_managed_ty(&elem_ty) {
                    self.emit_arc_release_if_managed(&elem_ty, value)?;
                }
                Ok(node)
            }
            "ori_tree_value" => {
                if args.len() != 2 {
                    return Err("ori_tree_value expects tree and node".to_string());
                }
                let tree = self.emit_expr(&args[0].value)?;
                let node = self.emit_expr_for_expected(&args[1].value, &node_id_ty)?;
                let call = self.builder.ins().call(fref, &[tree, node]);
                let stored = self.builder.inst_results(call)[0];
                Ok(self.decode_list_storage_value(stored, &elem_ty))
            }
            "ori_tree_set_value" => {
                if args.len() != 3 {
                    return Err("ori_tree_set_value expects tree, node, and value".to_string());
                }
                let tree = self.emit_expr(&args[0].value)?;
                let node = self.emit_expr_for_expected(&args[1].value, &node_id_ty)?;
                let value_is_owned = Self::expr_produces_owned_ref(&args[2].value);
                let value = self.emit_expr_for_expected(&args[2].value, &elem_ty)?;
                let stored = self.encode_list_storage_value(value, &elem_ty);
                let call = self.builder.ins().call(fref, &[tree, node, stored]);
                if value_is_owned && is_managed_ty(&elem_ty) {
                    self.emit_arc_release_if_managed(&elem_ty, value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_tree_find" => {
                if args.len() != 2 {
                    return Err("ori_tree_find expects tree and value".to_string());
                }
                let tree = self.emit_expr(&args[0].value)?;
                let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value = self.emit_expr_for_expected(&args[1].value, &elem_ty)?;
                let stored = self.encode_list_storage_value(value, &elem_ty);
                let call = self.builder.ins().call(fref, &[tree, stored]);
                // Lookup only borrows the probe value; drop an owned temp.
                if value_is_owned && is_managed_ty(&elem_ty) {
                    self.emit_arc_release_if_managed(&elem_ty, value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_tree_root"
            | "ori_tree_len"
            | "ori_tree_pre_order"
            | "ori_tree_post_order"
            | "ori_tree_breadth_first"
            | "ori_tree_clone" => {
                if args.len() != 1 {
                    return Err(format!("{name} expects one tree argument"));
                }
                let tree = self.emit_expr(&args[0].value)?;
                let call = self.builder.ins().call(fref, &[tree]);
                let res = self.builder.inst_results(call);
                Ok(res
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0)))
            }
            "ori_tree_children"
            | "ori_tree_parent"
            | "ori_tree_remove_subtree"
            | "ori_tree_depth"
            | "ori_tree_try_value"
            | "ori_tree_contains_node"
            | "ori_tree_clone_subtree" => {
                if args.len() != 2 {
                    return Err(format!("{name} expects tree and node"));
                }
                let tree = self.emit_expr(&args[0].value)?;
                let node = self.emit_expr_for_expected(&args[1].value, &node_id_ty)?;
                let call = self.builder.ins().call(fref, &[tree, node]);
                let res = self.builder.inst_results(call);
                Ok(res
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0)))
            }
            "ori_tree_move_subtree" => {
                if args.len() != 3 {
                    return Err("ori_tree_move_subtree expects tree, node, and parent".to_string());
                }
                let tree = self.emit_expr(&args[0].value)?;
                let node = self.emit_expr_for_expected(&args[1].value, &node_id_ty)?;
                let parent = self.emit_expr_for_expected(&args[2].value, &node_id_ty)?;
                let call = self.builder.ins().call(fref, &[tree, node, parent]);
                Ok(self.builder.inst_results(call)[0])
            }
            _ => Err(native_codegen_unsupported(format!(
                "tree runtime call `{name}`"
            ))),
        }
    }

    fn emit_heap_runtime_call(
        &mut self,
        name: &str,
        args: &[HirArg],
        result_ty: &Ty,
    ) -> Result<ir::Value, String> {
        let heap_elem_ty = |ty: &Ty| match ty {
            Ty::Opaque {
                kind: OpaqueTy::Heap,
                args,
            } => args.first().cloned(),
            _ => None,
        };
        match name {
            "ori_heap_new" => {
                if !args.is_empty() {
                    return Err("ori_heap_new expects no public arguments".to_string());
                }
                let elem_ty = heap_elem_ty(result_ty).unwrap_or(Ty::Int);
                let runtime_name = match &elem_ty {
                    Ty::String => "ori_heap_new_string",
                    Ty::Named(def_id, _) => {
                        let compare = SmolStr::new("compare");
                        let Some(compare_name) =
                            self.trait_method_func_name_for_type(*def_id, &compare)
                        else {
                            return Err(format!(
                                "heap element `{}` has no Comparable.compare implementation",
                                self.display_ty(&elem_ty)
                            ));
                        };
                        let compare_ref =
                            *self.func_refs.get(compare_name.as_str()).ok_or_else(|| {
                                format!("missing function reference `{compare_name}`")
                            })?;
                        let compare_ptr = self.builder.ins().func_addr(self.ptr_ty, compare_ref);
                        let new_ref =
                            *self.func_refs.get("ori_heap_new_custom").ok_or_else(|| {
                                "missing runtime function `ori_heap_new_custom`".to_string()
                            })?;
                        let call = self.builder.ins().call(new_ref, &[compare_ptr]);
                        return Ok(self.builder.inst_results(call)[0]);
                    }
                    _ => "ori_heap_new",
                };
                let fref = *self
                    .func_refs
                    .get(runtime_name)
                    .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
                let call = self.builder.ins().call(fref, &[]);
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_heap_push" => {
                if args.len() != 2 {
                    return Err("ori_heap_push expects heap and value".to_string());
                }
                let elem_ty = match heap_elem_ty(&args[0].value.ty) {
                    Some(ty) if !ty.contains_infer() => ty,
                    _ => args[1].value.ty.clone(),
                };
                let heap = self.emit_expr(&args[0].value)?;
                let value = self.emit_expr_for_expected(&args[1].value, &elem_ty)?;
                let stored = self.encode_list_storage_value(value, &elem_ty);
                match &elem_ty {
                    Ty::String => {
                        let fref =
                            *self.func_refs.get("ori_heap_push_string").ok_or_else(|| {
                                "missing runtime function `ori_heap_push_string`".to_string()
                            })?;
                        self.builder.ins().call(fref, &[heap, stored]);
                    }
                    Ty::Named(def_id, _) => {
                        let compare = SmolStr::new("compare");
                        let Some(compare_name) =
                            self.trait_method_func_name_for_type(*def_id, &compare)
                        else {
                            return Err(format!(
                                "heap element `{}` has no Comparable.compare implementation",
                                self.display_ty(&elem_ty)
                            ));
                        };
                        let compare_ref =
                            *self.func_refs.get(compare_name.as_str()).ok_or_else(|| {
                                format!("missing function reference `{compare_name}`")
                            })?;
                        let compare_ptr = self.builder.ins().func_addr(self.ptr_ty, compare_ref);
                        let fref =
                            *self.func_refs.get("ori_heap_push_custom").ok_or_else(|| {
                                "missing runtime function `ori_heap_push_custom`".to_string()
                            })?;
                        self.builder.ins().call(fref, &[heap, stored, compare_ptr]);
                    }
                    _ => {
                        let fref = *self
                            .func_refs
                            .get(name)
                            .ok_or_else(|| format!("missing runtime function `{name}`"))?;
                        self.builder.ins().call(fref, &[heap, stored]);
                    }
                }
                self.emit_arc_register_edge_if_managed(&elem_ty, heap, value)?;
                // The heap owns stored managed values via edges; drop an
                // owned temporary's own +1.
                if Self::expr_produces_owned_ref(&args[1].value) && is_managed_ty(&elem_ty) {
                    self.emit_arc_release_if_managed(&elem_ty, value)?;
                }
                Ok(self.builder.ins().iconst(types::I8, 0))
            }
            "ori_heap_from_list" => {
                if args.len() != 1 {
                    return Err("ori_heap_from_list expects one source list".to_string());
                }
                let elem_ty = heap_elem_ty(result_ty)
                    .or_else(|| match &args[0].value.ty {
                        Ty::List(elem) => Some(*elem.clone()),
                        _ => None,
                    })
                    .unwrap_or(Ty::Int);
                let source_is_owned = Self::expr_produces_owned_ref(&args[0].value);
                let source = self.emit_expr(&args[0].value)?;
                let runtime_name = match &elem_ty {
                    Ty::String => "ori_heap_from_list_string",
                    Ty::Named(def_id, _) => {
                        let compare = SmolStr::new("compare");
                        let Some(compare_name) =
                            self.trait_method_func_name_for_type(*def_id, &compare)
                        else {
                            return Err(format!(
                                "heap element `{}` has no Comparable.compare implementation",
                                self.display_ty(&elem_ty)
                            ));
                        };
                        let compare_ref =
                            *self.func_refs.get(compare_name.as_str()).ok_or_else(|| {
                                format!("missing function reference `{compare_name}`")
                            })?;
                        let compare_ptr = self.builder.ins().func_addr(self.ptr_ty, compare_ref);
                        let fref =
                            *self
                                .func_refs
                                .get("ori_heap_from_list_custom")
                                .ok_or_else(|| {
                                    "missing runtime function `ori_heap_from_list_custom`"
                                        .to_string()
                                })?;
                        let call = self.builder.ins().call(fref, &[source, compare_ptr]);
                        // The heap copies/retains the elements itself; an
                        // owned source-list temporary must be released.
                        if source_is_owned {
                            self.emit_arc_release_if_managed(&args[0].value.ty, source)?;
                        }
                        return Ok(self.builder.inst_results(call)[0]);
                    }
                    _ => "ori_heap_from_list",
                };
                let fref = *self
                    .func_refs
                    .get(runtime_name)
                    .ok_or_else(|| format!("missing runtime function `{runtime_name}`"))?;
                let call = self.builder.ins().call(fref, &[source]);
                if source_is_owned {
                    self.emit_arc_release_if_managed(&args[0].value.ty, source)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_heap_remove" => {
                if args.len() != 2 {
                    return Err("ori_heap_remove expects heap and value".to_string());
                }
                let elem_ty = match heap_elem_ty(&args[0].value.ty) {
                    Some(ty) if !ty.contains_infer() => ty,
                    _ => args[1].value.ty.clone(),
                };
                let heap = self.emit_expr(&args[0].value)?;
                let value_is_owned = Self::expr_produces_owned_ref(&args[1].value);
                let value = self.emit_expr_for_expected(&args[1].value, &elem_ty)?;
                let stored = self.encode_list_storage_value(value, &elem_ty);
                let call = match &elem_ty {
                    Ty::String => {
                        let fref =
                            *self
                                .func_refs
                                .get("ori_heap_remove_string")
                                .ok_or_else(|| {
                                    "missing runtime function `ori_heap_remove_string`".to_string()
                                })?;
                        self.builder.ins().call(fref, &[heap, stored])
                    }
                    Ty::Named(def_id, _) => {
                        let compare = SmolStr::new("compare");
                        let Some(compare_name) =
                            self.trait_method_func_name_for_type(*def_id, &compare)
                        else {
                            return Err(format!(
                                "heap element `{}` has no Comparable.compare implementation",
                                self.display_ty(&elem_ty)
                            ));
                        };
                        let compare_ref =
                            *self.func_refs.get(compare_name.as_str()).ok_or_else(|| {
                                format!("missing function reference `{compare_name}`")
                            })?;
                        let compare_ptr = self.builder.ins().func_addr(self.ptr_ty, compare_ref);
                        let fref =
                            *self
                                .func_refs
                                .get("ori_heap_remove_custom")
                                .ok_or_else(|| {
                                    "missing runtime function `ori_heap_remove_custom`".to_string()
                                })?;
                        self.builder.ins().call(fref, &[heap, stored, compare_ptr])
                    }
                    _ => {
                        let fref = *self
                            .func_refs
                            .get(name)
                            .ok_or_else(|| format!("missing runtime function `{name}`"))?;
                        self.builder.ins().call(fref, &[heap, stored])
                    }
                };
                // Removal only borrows the probe value; drop an owned temp.
                if value_is_owned && is_managed_ty(&elem_ty) {
                    self.emit_arc_release_if_managed(&elem_ty, value)?;
                }
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_heap_merge" => {
                if args.len() != 2 {
                    return Err("ori_heap_merge expects two heaps".to_string());
                }
                let left = self.emit_expr(&args[0].value)?;
                let right = self.emit_expr(&args[1].value)?;
                let fref = *self
                    .func_refs
                    .get(name)
                    .ok_or_else(|| format!("missing runtime function `{name}`"))?;
                let call = self.builder.ins().call(fref, &[left, right]);
                Ok(self.builder.inst_results(call)[0])
            }
            "ori_heap_pop"
            | "ori_heap_peek"
            | "ori_heap_len"
            | "ori_heap_is_empty"
            | "ori_heap_clear"
            | "ori_heap_clone"
            | "ori_heap_to_list"
            | "ori_heap_into_sorted_list" => {
                if args.len() != 1 {
                    return Err(format!("{name} expects one heap argument"));
                }
                let heap = self.emit_expr(&args[0].value)?;
                let fref = *self
                    .func_refs
                    .get(name)
                    .ok_or_else(|| format!("missing runtime function `{name}`"))?;
                let call = self.builder.ins().call(fref, &[heap]);
                let res = self.builder.inst_results(call);
                Ok(res
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0)))
            }
            _ => Err(native_codegen_unsupported(format!(
                "heap runtime call `{name}`"
            ))),
        }
    }

    fn encode_list_storage_value(&mut self, value: ir::Value, ty: &Ty) -> ir::Value {
        match ty {
            Ty::Bool | Ty::Int8 | Ty::U8 | Ty::Int16 | Ty::U16 | Ty::Int32 | Ty::U32 => {
                self.builder.ins().uextend(types::I64, value)
            }
            _ => value,
        }
    }

    fn decode_list_storage_value(&mut self, value: ir::Value, ty: &Ty) -> ir::Value {
        match ty {
            Ty::Bool | Ty::Int8 | Ty::U8 => self.builder.ins().ireduce(types::I8, value),
            Ty::Int16 | Ty::U16 => self.builder.ins().ireduce(types::I16, value),
            Ty::Int32 | Ty::U32 => self.builder.ins().ireduce(types::I32, value),
            _ => value,
        }
    }

    fn emit_short_circuit_binary(
        &mut self,
        op: BinaryOp,
        lhs: &HirExpr,
        rhs: &HirExpr,
    ) -> Result<ir::Value, String> {
        let lhs_value = self.emit_expr(lhs)?;
        let rhs_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I8);

        let skip_value = match op {
            BinaryOp::And => self.builder.ins().iconst(types::I8, 0),
            BinaryOp::Or => self.builder.ins().iconst(types::I8, 1),
            _ => unreachable!("short-circuit only handles logical operators"),
        };
        let skip_args = [BlockArg::Value(skip_value)];

        match op {
            BinaryOp::And => {
                self.builder
                    .ins()
                    .brif(lhs_value, rhs_block, &[], merge_block, &skip_args);
            }
            BinaryOp::Or => {
                self.builder
                    .ins()
                    .brif(lhs_value, merge_block, &skip_args, rhs_block, &[]);
            }
            _ => unreachable!("short-circuit only handles logical operators"),
        }

        self.builder.seal_block(rhs_block);
        self.builder.switch_to_block(rhs_block);
        self.terminated = false;
        let rhs_value = self.emit_expr(rhs)?;
        if !self.terminated {
            let rhs_args = [BlockArg::Value(rhs_value)];
            self.builder.ins().jump(merge_block, &rhs_args);
        }

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        Ok(self.builder.block_params(merge_block)[0])
    }

    /// Jump to a runtime abort helper that never returns.
    fn emit_abort_call(&mut self, symbol: &str) -> Result<(), String> {
        if self.hosted_traps {
            let (code, message) = match symbol {
                "ori_abort_division_by_zero" => (5, "ori integer division or remainder by zero"),
                "ori_abort_division_overflow" => (
                    6,
                    "ori integer division overflow: the minimum value has no positive counterpart",
                ),
                "ori_abort_concurrent_modification" => {
                    (7, "concurrent modification during iteration")
                }
                "ori_abort_shift_overflow" => {
                    (9, "ori shift count out of range: must be in 0..bit_width")
                }
                _ => (2, "ori runtime abort"),
            };
            self.emit_host_trap_return(code, Some(message), true)?;
            return Ok(());
        }
        let abort_ref = *self
            .func_refs
            .get(symbol)
            .ok_or_else(|| format!("missing runtime function `{symbol}`"))?;
        self.builder.ins().call(abort_ref, &[]);
        self.builder.ins().trap(ir::TrapCode::user(2).unwrap());
        Ok(())
    }

    /// Divert the two trapping integer division cases to a diagnosed abort.
    ///
    /// Cranelift lowers `sdiv`/`udiv` straight to the hardware instruction, so
    /// a zero divisor (or `MIN / -1` on signed types) raises `SIGFPE` and kills
    /// the process without printing anything the programmer can act on.
    fn emit_division_guard(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        signed: bool,
    ) -> Result<(), String> {
        use ir::condcodes::IntCC;

        let zero_block = self.builder.create_block();
        let nonzero_block = self.builder.create_block();
        let divisor_is_zero = self.builder.ins().icmp_imm(IntCC::Equal, rv, 0);
        self.builder
            .ins()
            .brif(divisor_is_zero, zero_block, &[], nonzero_block, &[]);

        self.builder.seal_block(zero_block);
        self.builder.switch_to_block(zero_block);
        self.emit_abort_call("ori_abort_division_by_zero")?;

        self.builder.seal_block(nonzero_block);
        self.builder.switch_to_block(nonzero_block);
        // The abort path set `terminated`; the nonzero block is a live
        // continuation and must not inherit it, or the enclosing statement
        // list would skip the rest of the block.
        self.terminated = false;

        if signed {
            let min = match self.builder.func.dfg.value_type(lv).bits() {
                8 => i8::MIN as i64,
                16 => i16::MIN as i64,
                32 => i32::MIN as i64,
                _ => i64::MIN,
            };
            let lhs_is_min = self.builder.ins().icmp_imm(IntCC::Equal, lv, min);
            let rhs_is_neg_one = self.builder.ins().icmp_imm(IntCC::Equal, rv, -1);
            let overflows = self.builder.ins().band(lhs_is_min, rhs_is_neg_one);

            let overflow_block = self.builder.create_block();
            let ok_block = self.builder.create_block();
            self.builder
                .ins()
                .brif(overflows, overflow_block, &[], ok_block, &[]);

            self.builder.seal_block(overflow_block);
            self.builder.switch_to_block(overflow_block);
            self.emit_abort_call("ori_abort_division_overflow")?;

            self.builder.seal_block(ok_block);
            self.builder.switch_to_block(ok_block);
            self.terminated = false;
        }
        Ok(())
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        lv: ir::Value,
        rv: ir::Value,
        ty: &Ty,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::{FloatCC, IntCC};
        use BinaryOp::*;
        let float = is_float_ty(ty);
        let string = matches!(ty, Ty::String);
        // Unsigned scalars share the integer instructions but need the unsigned
        // division and comparison forms; `sdiv`/`SignedLessThan` would reinterpret
        // values above the signed maximum as negative.
        let unsigned = ty.is_unsigned_integer();
        Ok(match op {
            Add => {
                if matches!(ty, Ty::String) {
                    let concat_ref = *self.func_refs.get("ori_string_concat").ok_or_else(|| {
                        "missing runtime function `ori_string_concat`".to_string()
                    })?;
                    let call = self.builder.ins().call(concat_ref, &[lv, rv]);
                    self.builder.inst_results(call)[0]
                } else if matches!(ty, Ty::Bytes) {
                    let concat_ref = *self
                        .func_refs
                        .get("ori_bytes_concat")
                        .ok_or_else(|| "missing runtime function `ori_bytes_concat`".to_string())?;
                    let call = self.builder.ins().call(concat_ref, &[lv, rv]);
                    self.builder.inst_results(call)[0]
                } else if float {
                    self.builder.ins().fadd(lv, rv)
                } else {
                    self.builder.ins().iadd(lv, rv)
                }
            }
            Sub => {
                if float {
                    self.builder.ins().fsub(lv, rv)
                } else {
                    self.builder.ins().isub(lv, rv)
                }
            }
            Mul => {
                if float {
                    self.builder.ins().fmul(lv, rv)
                } else {
                    self.builder.ins().imul(lv, rv)
                }
            }
            Div => {
                if float {
                    self.builder.ins().fdiv(lv, rv)
                } else if matches!(ty, Ty::Simd(..)) {
                    if unsigned {
                        self.builder.ins().udiv(lv, rv)
                    } else {
                        self.builder.ins().sdiv(lv, rv)
                    }
                } else if unsigned {
                    self.emit_division_guard(lv, rv, false)?;
                    self.builder.ins().udiv(lv, rv)
                } else {
                    self.emit_division_guard(lv, rv, true)?;
                    self.builder.ins().sdiv(lv, rv)
                }
            }
            Rem => {
                if unsigned {
                    self.emit_division_guard(lv, rv, false)?;
                    self.builder.ins().urem(lv, rv)
                } else {
                    self.emit_division_guard(lv, rv, true)?;
                    self.builder.ins().srem(lv, rv)
                }
            }
            Eq => {
                if string {
                    let strcmp_ref = *self
                        .func_refs
                        .get("strcmp")
                        .ok_or_else(|| "missing runtime function `strcmp`".to_string())?;
                    let call = self.builder.ins().call(strcmp_ref, &[lv, rv]);
                    let cmp = self.builder.inst_results(call)[0];
                    let zero = self.builder.ins().iconst(types::I32, 0);
                    self.builder.ins().icmp(IntCC::Equal, cmp, zero)
                } else if let Ty::Any(_) = ty {
                    return self.emit_any_equality(lv, rv, true);
                } else if let Ty::Opaque { kind, args } = ty {
                    if kind.is_list_backed_collection() {
                        return self.emit_opaque_collection_equality(lv, rv, *kind, &args[0], true);
                    }
                    self.builder.ins().icmp(IntCC::Equal, lv, rv)
                } else if let Ty::Optional(inner) = ty {
                    return self.emit_optional_equality(lv, rv, inner, true);
                } else if let Ty::Result(ok, err) = ty {
                    return self.emit_result_equality(lv, rv, ok, err, true);
                } else if let Ty::Tuple(elements) = ty {
                    return self.emit_tuple_equality(lv, rv, elements, true);
                } else if let Ty::List(inner) = ty {
                    return self.emit_list_equality(lv, rv, inner, true);
                } else if let Ty::Set(inner) = ty {
                    return self.emit_set_equality(lv, rv, inner, true);
                } else if let Ty::Map(key, value) = ty {
                    return self.emit_map_equality(lv, rv, key, value, true);
                } else if let Ty::Bytes = ty {
                    return self.emit_bytes_equality(lv, rv, true);
                } else if matches!(ty, Ty::Handle(_)) {
                    // Handles are borrowed opaque pointers. Identity compares
                    // the sentinel/address only; never retain or dereference
                    // the pointee while evaluating equality.
                    self.builder.ins().icmp(IntCC::Equal, lv, rv)
                } else if let Ty::Named(def_id, args) = ty {
                    if let Some(method_name) =
                        self.trait_method_func_name_for_trait(*def_id, ".Equatable", "equals")
                    {
                        let fref = *self.func_refs.get(method_name.as_str()).ok_or_else(|| {
                            format!("missing Equatable implementation `{method_name}`")
                        })?;
                        self.emit_arc_retain_if_managed(ty, lv)?;
                        self.emit_arc_retain_if_managed(ty, rv)?;
                        let call = self.builder.ins().call(fref, &[lv, rv]);
                        return Ok(self.builder.inst_results(call)[0]);
                    }
                    if self.enum_layouts.contains_key(def_id) {
                        return self.emit_enum_equality(lv, rv, *def_id, args, true);
                    }
                    return self.emit_struct_equality(lv, rv, *def_id, args, true);
                } else if float {
                    self.builder.ins().fcmp(FloatCC::Equal, lv, rv)
                } else {
                    self.builder.ins().icmp(IntCC::Equal, lv, rv)
                }
            }
            Ne => {
                if string {
                    let strcmp_ref = *self
                        .func_refs
                        .get("strcmp")
                        .ok_or_else(|| "missing runtime function `strcmp`".to_string())?;
                    let call = self.builder.ins().call(strcmp_ref, &[lv, rv]);
                    let cmp = self.builder.inst_results(call)[0];
                    let zero = self.builder.ins().iconst(types::I32, 0);
                    self.builder.ins().icmp(IntCC::NotEqual, cmp, zero)
                } else if let Ty::Any(_) = ty {
                    return self.emit_any_equality(lv, rv, false);
                } else if let Ty::Opaque { kind, args } = ty {
                    if kind.is_list_backed_collection() {
                        return self
                            .emit_opaque_collection_equality(lv, rv, *kind, &args[0], false);
                    }
                    self.builder.ins().icmp(IntCC::NotEqual, lv, rv)
                } else if let Ty::Optional(inner) = ty {
                    return self.emit_optional_equality(lv, rv, inner, false);
                } else if let Ty::Result(ok, err) = ty {
                    return self.emit_result_equality(lv, rv, ok, err, false);
                } else if let Ty::Tuple(elements) = ty {
                    return self.emit_tuple_equality(lv, rv, elements, false);
                } else if let Ty::List(inner) = ty {
                    return self.emit_list_equality(lv, rv, inner, false);
                } else if let Ty::Set(inner) = ty {
                    return self.emit_set_equality(lv, rv, inner, false);
                } else if let Ty::Map(key, value) = ty {
                    return self.emit_map_equality(lv, rv, key, value, false);
                } else if let Ty::Bytes = ty {
                    return self.emit_bytes_equality(lv, rv, false);
                } else if matches!(ty, Ty::Handle(_)) {
                    // Handle inequality is the inverse of pointer identity;
                    // it has no ownership or lifetime side effects.
                    self.builder.ins().icmp(IntCC::NotEqual, lv, rv)
                } else if let Ty::Named(def_id, args) = ty {
                    if let Some(method_name) =
                        self.trait_method_func_name_for_trait(*def_id, ".Equatable", "equals")
                    {
                        let fref = *self.func_refs.get(method_name.as_str()).ok_or_else(|| {
                            format!("missing Equatable implementation `{method_name}`")
                        })?;
                        self.emit_arc_retain_if_managed(ty, lv)?;
                        self.emit_arc_retain_if_managed(ty, rv)?;
                        let call = self.builder.ins().call(fref, &[lv, rv]);
                        let result = self.builder.inst_results(call)[0];
                        let zero = self.builder.ins().iconst(types::I8, 0);
                        return Ok(self.builder.ins().icmp(IntCC::Equal, result, zero));
                    }
                    if self.enum_layouts.contains_key(def_id) {
                        return self.emit_enum_equality(lv, rv, *def_id, args, false);
                    }
                    return self.emit_struct_equality(lv, rv, *def_id, args, false);
                } else if float {
                    self.builder.ins().fcmp(FloatCC::NotEqual, lv, rv)
                } else {
                    self.builder.ins().icmp(IntCC::NotEqual, lv, rv)
                }
            }
            Lt => {
                if float {
                    self.builder.ins().fcmp(FloatCC::LessThan, lv, rv)
                } else if unsigned {
                    self.builder.ins().icmp(IntCC::UnsignedLessThan, lv, rv)
                } else {
                    self.builder.ins().icmp(IntCC::SignedLessThan, lv, rv)
                }
            }
            Le => {
                if float {
                    self.builder.ins().fcmp(FloatCC::LessThanOrEqual, lv, rv)
                } else if unsigned {
                    self.builder
                        .ins()
                        .icmp(IntCC::UnsignedLessThanOrEqual, lv, rv)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, lv, rv)
                }
            }
            Gt => {
                if float {
                    self.builder.ins().fcmp(FloatCC::GreaterThan, lv, rv)
                } else if unsigned {
                    self.builder.ins().icmp(IntCC::UnsignedGreaterThan, lv, rv)
                } else {
                    self.builder.ins().icmp(IntCC::SignedGreaterThan, lv, rv)
                }
            }
            Ge => {
                if float {
                    self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lv, rv)
                } else if unsigned {
                    self.builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, lv, rv)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, lv, rv)
                }
            }
            And => self.builder.ins().band(lv, rv),
            Or => self.builder.ins().bor(lv, rv),
            // GFX-BITWISE-1: bitwise ops on integers. `And`/`Or` above are the
            // logical `and`/`or` keywords; the symbolic operators are distinct.
            Band => self.builder.ins().band(lv, rv),
            Bor => self.builder.ins().bor(lv, rv),
            Bxor => self.builder.ins().bxor(lv, rv),
            Shl => {
                self.emit_shift_guard(lv, rv)?;
                self.builder.ins().ishl(lv, rv)
            }
            Shr => {
                self.emit_shift_guard(lv, rv)?;
                if unsigned {
                    self.builder.ins().ushr(lv, rv)
                } else {
                    self.builder.ins().sshr(lv, rv)
                }
            }
        })
    }

    /// GFX-BITWISE-1: trap when the shift count is negative or >= the LHS bit
    /// width. Without the guard, Cranelift's shift instructions wrap or are
    /// target-dependent for out-of-range counts.
    fn emit_shift_guard(&mut self, lv: ir::Value, rv: ir::Value) -> Result<(), String> {
        use ir::condcodes::IntCC;
        let bits = self.builder.func.dfg.value_type(lv).bits() as i64;
        let zero = self.builder.ins().iconst(types::I64, 0);
        let width = self.builder.ins().iconst(types::I64, bits);
        let ge_zero = self
            .builder
            .ins()
            .icmp(IntCC::SignedGreaterThanOrEqual, rv, zero);
        let lt_width = self.builder.ins().icmp(IntCC::SignedLessThan, rv, width);
        let in_range = self.builder.ins().band(ge_zero, lt_width);

        let ok_block = self.builder.create_block();
        let bad_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(in_range, ok_block, &[], bad_block, &[]);

        self.builder.seal_block(bad_block);
        self.builder.switch_to_block(bad_block);
        self.emit_abort_call("ori_abort_shift_overflow")?;

        self.builder.seal_block(ok_block);
        self.builder.switch_to_block(ok_block);
        self.terminated = false;
        Ok(())
    }

    /// Compare two `optional[T]` values for equality (eq=true) or inequality (eq=false).
    fn emit_optional_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        inner: &Ty,
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;
        let zero = self.builder.ins().iconst(types::I8, 0);
        let one = self.builder.ins().iconst(types::I8, 1);
        let ltag = self.builder.ins().load(types::I8, MemFlags::new(), lv, 0);
        let rtag = self.builder.ins().load(types::I8, MemFlags::new(), rv, 0);
        let ltag_some = self.builder.ins().icmp(IntCC::Equal, ltag, one);
        let rtag_some = self.builder.ins().icmp(IntCC::Equal, rtag, one);
        let both_some = self.builder.ins().band(ltag_some, rtag_some);
        let ltag_none = self.builder.ins().icmp(IntCC::Equal, ltag, zero);
        let rtag_none = self.builder.ins().icmp(IntCC::Equal, rtag, zero);
        let both_none = self.builder.ins().band(ltag_none, rtag_none);

        let none_block = self.builder.create_block();
        let maybe_some_block = self.builder.create_block();
        let some_block = self.builder.create_block();
        let not_equal_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I8);

        self.builder
            .ins()
            .brif(both_none, none_block, &[], maybe_some_block, &[]);

        self.builder.seal_block(none_block);
        self.builder.switch_to_block(none_block);
        let true_value = self.builder.ins().iconst(types::I8, 1);
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(true_value)]);

        self.builder.seal_block(maybe_some_block);
        self.builder.switch_to_block(maybe_some_block);
        self.builder
            .ins()
            .brif(both_some, some_block, &[], not_equal_block, &[]);

        self.builder.seal_block(some_block);
        self.builder.switch_to_block(some_block);
        let (val_off, _) = optional_layout(inner, self.ptr_ty);
        let inner_cl =
            cl_type(inner, self.ptr_ty).ok_or("optional inner type has no native layout")?;
        let lval = self
            .builder
            .ins()
            .load(inner_cl, MemFlags::new(), lv, val_off as i32);
        let rval = self
            .builder
            .ins()
            .load(inner_cl, MemFlags::new(), rv, val_off as i32);
        let values_eq = self.emit_binary(BinaryOp::Eq, lval, rval, inner)?;
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(values_eq)]);

        self.builder.seal_block(not_equal_block);
        self.builder.switch_to_block(not_equal_block);
        let false_value = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(false_value)]);

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        let result = self.builder.block_params(merge_block)[0];
        Ok(self.maybe_invert_equality(result, eq))
    }

    /// Compare two `result[T,E]` values.
    fn emit_result_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        ok: &Ty,
        err: &Ty,
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;
        let zero = self.builder.ins().iconst(types::I8, 0);
        let one = self.builder.ins().iconst(types::I8, 1);
        let ltag = self.builder.ins().load(types::I8, MemFlags::new(), lv, 0);
        let rtag = self.builder.ins().load(types::I8, MemFlags::new(), rv, 0);
        let ltag_ok = self.builder.ins().icmp(IntCC::Equal, ltag, one);
        let rtag_ok = self.builder.ins().icmp(IntCC::Equal, rtag, one);
        let both_ok = self.builder.ins().band(ltag_ok, rtag_ok);
        let ltag_err = self.builder.ins().icmp(IntCC::Equal, ltag, zero);
        let rtag_err = self.builder.ins().icmp(IntCC::Equal, rtag, zero);
        let both_err = self.builder.ins().band(ltag_err, rtag_err);

        let ok_block = self.builder.create_block();
        let maybe_err_block = self.builder.create_block();
        let err_block = self.builder.create_block();
        let not_equal_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I8);

        self.builder
            .ins()
            .brif(both_ok, ok_block, &[], maybe_err_block, &[]);

        self.builder.seal_block(ok_block);
        self.builder.switch_to_block(ok_block);
        let (pay_off, _, _) = result_layout(ok, err, self.ptr_ty);
        let ok_cl = cl_type(ok, self.ptr_ty).ok_or("result ok type has no native layout")?;
        let lok_val = self
            .builder
            .ins()
            .load(ok_cl, MemFlags::new(), lv, pay_off as i32);
        let rok_val = self
            .builder
            .ins()
            .load(ok_cl, MemFlags::new(), rv, pay_off as i32);
        let ok_eq_val = self.emit_binary(BinaryOp::Eq, lok_val, rok_val, ok)?;
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(ok_eq_val)]);

        self.builder.seal_block(maybe_err_block);
        self.builder.switch_to_block(maybe_err_block);
        self.builder
            .ins()
            .brif(both_err, err_block, &[], not_equal_block, &[]);

        self.builder.seal_block(err_block);
        self.builder.switch_to_block(err_block);
        let err_cl = cl_type(err, self.ptr_ty).ok_or("result err type has no native layout")?;
        let lerr_val = self
            .builder
            .ins()
            .load(err_cl, MemFlags::new(), lv, pay_off as i32);
        let rerr_val = self
            .builder
            .ins()
            .load(err_cl, MemFlags::new(), rv, pay_off as i32);
        let err_eq_val = self.emit_binary(BinaryOp::Eq, lerr_val, rerr_val, err)?;
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(err_eq_val)]);

        self.builder.seal_block(not_equal_block);
        self.builder.switch_to_block(not_equal_block);
        let false_value = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(false_value)]);

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        let result = self.builder.block_params(merge_block)[0];
        Ok(self.maybe_invert_equality(result, eq))
    }

    /// Compare two `tuple[...]` values element-by-element.
    fn emit_tuple_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        elements: &[Ty],
        eq: bool,
    ) -> Result<ir::Value, String> {
        let mut offset: i32 = 0;
        // All elements must match for equality
        let mut result = self.builder.ins().iconst(types::I8, if eq { 1 } else { 0 });
        for elem_ty in elements {
            let cl_ty =
                cl_type(elem_ty, self.ptr_ty).ok_or("tuple element has no native layout")?;
            let le = self.builder.ins().load(cl_ty, MemFlags::new(), lv, offset);
            let re = self.builder.ins().load(cl_ty, MemFlags::new(), rv, offset);
            let elem_eq = self.emit_binary(
                if eq { BinaryOp::Eq } else { BinaryOp::Ne },
                le,
                re,
                elem_ty,
            )?;
            if eq {
                result = self.builder.ins().band(result, elem_eq);
            } else {
                result = self.builder.ins().bor(result, elem_eq);
            }
            offset += cl_ty.bytes() as i32;
        }
        Ok(result)
    }

    /// Compare two `bytes` values.
    fn emit_bytes_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;
        let bytes_eq_ref = *self
            .func_refs
            .get("ori_bytes_eq")
            .ok_or_else(|| "missing runtime function `ori_bytes_eq`".to_string())?;
        let call = self.builder.ins().call(bytes_eq_ref, &[lv, rv]);
        let cmp = self.builder.inst_results(call)[0];
        if eq {
            Ok(cmp)
        } else {
            let zero = self.builder.ins().iconst(types::I8, 0);
            Ok(self.builder.ins().icmp(IntCC::Equal, cmp, zero))
        }
    }

    fn emit_any_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        eq: bool,
    ) -> Result<ir::Value, String> {
        let ptr_size = self.ptr_ty.bytes() as i64;
        let vtable_l = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), lv, ptr_size as i32);
        let vtable_r = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), rv, ptr_size as i32);
        let type_id_l = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), vtable_l, 0);
        let type_id_r = self
            .builder
            .ins()
            .load(self.ptr_ty, MemFlags::new(), vtable_r, 0);
        let types_eq = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, type_id_l, type_id_r);

        let types_eq_block = self.builder.create_block();
        let types_ne_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I8);

        self.builder
            .ins()
            .brif(types_eq, types_eq_block, &[], types_ne_block, &[]);

        // --- types_ne_block ---
        self.builder.switch_to_block(types_ne_block);
        self.builder.seal_block(types_ne_block);
        let false_val = self.builder.ins().iconst(types::I8, if eq { 0 } else { 1 });
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(false_val)]);

        // --- types_eq_block ---
        self.builder.switch_to_block(types_eq_block);
        self.builder.seal_block(types_eq_block);
        let eq_fn =
            self.builder
                .ins()
                .load(self.ptr_ty, MemFlags::new(), vtable_l, ptr_size as i32);
        let zero_ptr = self.builder.ins().iconst(self.ptr_ty, 0);
        let has_eq_fn = self
            .builder
            .ins()
            .icmp(ir::condcodes::IntCC::NotEqual, eq_fn, zero_ptr);

        let call_block = self.builder.create_block();
        let no_call_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(has_eq_fn, call_block, &[], no_call_block, &[]);

        // --- no_call_block ---
        self.builder.switch_to_block(no_call_block);
        self.builder.seal_block(no_call_block);
        let false_val2 = self.builder.ins().iconst(types::I8, if eq { 0 } else { 1 });
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(false_val2)]);

        // --- call_block ---
        self.builder.switch_to_block(call_block);
        self.builder.seal_block(call_block);
        let data_l = self.builder.ins().load(self.ptr_ty, MemFlags::new(), lv, 0);
        let data_r = self.builder.ins().load(self.ptr_ty, MemFlags::new(), rv, 0);

        let mut sig = ir::Signature::new(self.builder.func.signature.call_conv);
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.params.push(AbiParam::new(self.ptr_ty));
        sig.returns.push(AbiParam::new(types::I8));
        let sig_ref = self.builder.func.import_signature(sig);
        let call = self
            .builder
            .ins()
            .call_indirect(sig_ref, eq_fn, &[data_l, data_r]);
        let eq_res = self.builder.inst_results(call)[0];

        let final_res = if eq {
            eq_res
        } else {
            let zero8 = self.builder.ins().iconst(types::I8, 0);
            self.builder
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, eq_res, zero8)
        };
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(final_res)]);

        // --- merge_block ---
        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        let result = self.builder.block_params(merge_block)[0];
        Ok(result)
    }

    /// Compare two enum values by discriminant and active payload fields.
    ///
    /// Enum values are heap-backed pointers with a four-byte tag followed by
    /// the largest variant payload.  Only fields belonging to the active
    /// variant are read; this is important for managed fields because reading
    /// an inactive payload would interpret unrelated bytes as a pointer.
    fn emit_enum_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        def_id: ori_types::DefId,
        args: &[Ty],
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;

        let layout = self
            .enum_layouts
            .get(&def_id)
            .cloned()
            .ok_or_else(|| format!("missing native enum layout for def {}", def_id.0))?;

        let left_tag = self.builder.ins().load(types::I32, MemFlags::new(), lv, 0);
        let right_tag = self.builder.ins().load(types::I32, MemFlags::new(), rv, 0);
        let same_tag = self.builder.ins().icmp(IntCC::Equal, left_tag, right_tag);

        let first_variant = self.builder.create_block();
        let false_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I8);

        self.builder
            .ins()
            .brif(same_tag, first_variant, &[], false_block, &[]);

        self.builder.seal_block(false_block);
        self.builder.switch_to_block(false_block);
        let false_value = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(false_value)]);

        let mut test_block = first_variant;
        let mut variants = layout.variants.iter().collect::<Vec<_>>();
        variants.sort_unstable_by_key(|(_, variant)| variant.tag);

        for (_, variant) in variants {
            let body_block = self.builder.create_block();
            let next_block = self.builder.create_block();

            self.builder.switch_to_block(test_block);
            let expected = self
                .builder
                .ins()
                .iconst(types::I32, i64::from(variant.tag));
            let matches = self.builder.ins().icmp(IntCC::Equal, left_tag, expected);
            self.builder
                .ins()
                .brif(matches, body_block, &[], next_block, &[]);
            self.builder.seal_block(test_block);

            self.builder.switch_to_block(body_block);
            self.terminated = false;
            let mut variant_equal = self.builder.ins().iconst(types::I8, 1);
            for (_, field) in &variant.fields.fields {
                let field_ty = substitute_ty_params(&field.ty, args);
                let field_cl_ty = cl_type(&field_ty, self.ptr_ty)
                    .ok_or_else(|| "enum field has no native layout".to_string())?;
                let offset = layout.payload_offset + field.offset;
                let left_field =
                    self.builder
                        .ins()
                        .load(field_cl_ty, MemFlags::new(), lv, offset as i32);
                let right_field =
                    self.builder
                        .ins()
                        .load(field_cl_ty, MemFlags::new(), rv, offset as i32);
                let field_equal =
                    self.emit_binary(BinaryOp::Eq, left_field, right_field, &field_ty)?;
                variant_equal = self.builder.ins().band(variant_equal, field_equal);
            }
            self.builder
                .ins()
                .jump(merge_block, &[BlockArg::Value(variant_equal)]);
            self.builder.seal_block(body_block);
            test_block = next_block;
        }

        // A well-formed enum cannot carry an unknown tag, but keeping this
        // defensive branch makes malformed foreign memory compare unequal
        // instead of reading an arbitrary payload.
        self.builder.switch_to_block(test_block);
        let false_value = self.builder.ins().iconst(types::I8, 0);
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(false_value)]);
        self.builder.seal_block(test_block);

        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        let result = self.builder.block_params(merge_block)[0];
        Ok(self.maybe_invert_equality(result, eq))
    }

    /// Hash one value using the stable runtime scalar ABI.  Aggregate values
    /// that do not yet have a public Hashable contract intentionally contribute
    /// zero; this preserves the required `a == b => hash(a) == hash(b)` rule
    /// while the equality-only compatibility path remains available.
    fn emit_hash_value(&mut self, value: ir::Value, ty: &Ty) -> Result<ir::Value, String> {
        let hash_i64 = *self
            .func_refs
            .get("ori_hash_i64")
            .ok_or_else(|| "missing runtime function `ori_hash_i64`".to_string())?;
        let hash_ptr =
            |this: &mut Self, name: &str, value: ir::Value| -> Result<ir::Value, String> {
                let fref = *this
                    .func_refs
                    .get(name)
                    .ok_or_else(|| format!("missing runtime function `{name}`"))?;
                let call = this.builder.ins().call(fref, &[value]);
                Ok(this.builder.inst_results(call)[0])
            };
        match ty {
            Ty::Int | Ty::Int64 | Ty::U64 => {
                let call = self.builder.ins().call(hash_i64, &[value]);
                Ok(self.builder.inst_results(call)[0])
            }
            Ty::Int8 | Ty::Int16 | Ty::Int32 => {
                let widened = self.builder.ins().sextend(types::I64, value);
                let call = self.builder.ins().call(hash_i64, &[widened]);
                Ok(self.builder.inst_results(call)[0])
            }
            Ty::U8 | Ty::U16 | Ty::U32 | Ty::Bool => {
                let widened = self.builder.ins().uextend(types::I64, value);
                let call = self.builder.ins().call(hash_i64, &[widened]);
                Ok(self.builder.inst_results(call)[0])
            }
            Ty::Float | Ty::Float64 => {
                let bits = self
                    .builder
                    .ins()
                    .bitcast(types::I64, MemFlags::new(), value);
                let call = self.builder.ins().call(hash_i64, &[bits]);
                Ok(self.builder.inst_results(call)[0])
            }
            Ty::Float32 => {
                let bits = self
                    .builder
                    .ins()
                    .bitcast(types::I32, MemFlags::new(), value);
                let widened = self.builder.ins().uextend(types::I64, bits);
                let call = self.builder.ins().call(hash_i64, &[widened]);
                Ok(self.builder.inst_results(call)[0])
            }
            Ty::String => hash_ptr(self, "ori_hash_string", value),
            Ty::Bytes => hash_ptr(self, "ori_hash_bytes", value),
            Ty::Named(def_id, _) => {
                let struct_name = format!("__hash_helper_struct_{}", def_id.0);
                let enum_name = format!("__hash_helper_enum_{}", def_id.0);
                let fref = self
                    .func_refs
                    .get(struct_name.as_str())
                    .or_else(|| self.func_refs.get(enum_name.as_str()))
                    .copied();
                let Some(fref) = fref else {
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                };
                let call = self.builder.ins().call(fref, &[value]);
                Ok(self.builder.inst_results(call)[0])
            }
            _ => Ok(self.builder.ins().iconst(types::I64, 0)),
        }
    }

    fn hash_combine(&mut self, accumulator: ir::Value, value: ir::Value) -> ir::Value {
        // FNV-style mixing with a fixed odd multiplier.  The operation is
        // deliberately target-independent and uses wrapping integer arithmetic.
        let multiplier = self.builder.ins().iconst(types::I64, 0x100000001b3_i64);
        let mixed = self.builder.ins().imul(accumulator, multiplier);
        self.builder.ins().bxor(mixed, value)
    }

    /// Hash a struct by the same ordered fields used by structural equality.
    fn emit_struct_hash(
        &mut self,
        ptr: ir::Value,
        def_id: ori_types::DefId,
        args: &[Ty],
        include_seed: bool,
    ) -> Result<ir::Value, String> {
        let layout = self
            .struct_layouts
            .get(&def_id)
            .cloned()
            .ok_or_else(|| format!("missing native struct layout for def {}", def_id.0))?;
        let mut result = self.builder.ins().iconst(
            types::I64,
            if include_seed {
                0xcbf29ce484222325_u64 as i64
            } else {
                0
            },
        );
        for (_, field) in layout.fields {
            let field_ty = substitute_ty_params(&field.ty, args);
            let cl_ty = cl_type(&field_ty, self.ptr_ty)
                .ok_or_else(|| "struct field has no native layout".to_string())?;
            let value = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), ptr, field.offset as i32);
            let value_hash = self.emit_hash_value(value, &field_ty)?;
            result = self.hash_combine(result, value_hash);
        }
        Ok(result)
    }

    /// Hash an enum by discriminant and only the active variant payload.
    /// Reading inactive bytes would interpret unrelated managed pointers and
    /// would make equal values depend on allocator addresses.
    fn emit_enum_hash(
        &mut self,
        ptr: ir::Value,
        def_id: ori_types::DefId,
        args: &[Ty],
        _include_seed: bool,
    ) -> Result<ir::Value, String> {
        let layout = self
            .enum_layouts
            .get(&def_id)
            .cloned()
            .ok_or_else(|| format!("missing native enum layout for def {}", def_id.0))?;
        let tag = self.builder.ins().load(types::I32, MemFlags::new(), ptr, 0);
        let first_variant = self.builder.create_block();
        let unknown_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I64);
        self.builder.ins().jump(first_variant, &[]);

        let mut test_block = first_variant;
        let mut variants = layout.variants.iter().collect::<Vec<_>>();
        variants.sort_unstable_by_key(|(_, variant)| variant.tag);
        for (_, variant) in variants {
            let body_block = self.builder.create_block();
            let next_block = self.builder.create_block();
            self.builder.switch_to_block(test_block);
            let expected = self
                .builder
                .ins()
                .iconst(types::I32, i64::from(variant.tag));
            let matches = self
                .builder
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, tag, expected);
            self.builder
                .ins()
                .brif(matches, body_block, &[], next_block, &[]);
            self.builder.seal_block(test_block);

            self.builder.switch_to_block(body_block);
            self.terminated = false;
            let seed = self
                .builder
                .ins()
                .iconst(types::I64, 0xcbf29ce484222325_u64 as i64);
            let tag_hash = self.builder.ins().sextend(types::I64, tag);
            let mut result = self.hash_combine(seed, tag_hash);
            for (_, field) in &variant.fields.fields {
                let field_ty = substitute_ty_params(&field.ty, args);
                let field_cl_ty = cl_type(&field_ty, self.ptr_ty)
                    .ok_or_else(|| "enum field has no native layout".to_string())?;
                let offset = layout.payload_offset + field.offset;
                let value =
                    self.builder
                        .ins()
                        .load(field_cl_ty, MemFlags::new(), ptr, offset as i32);
                let value_hash = self.emit_hash_value(value, &field_ty)?;
                result = self.hash_combine(result, value_hash);
            }
            self.builder
                .ins()
                .jump(merge_block, &[BlockArg::Value(result)]);
            self.builder.seal_block(body_block);
            test_block = next_block;
        }
        self.builder.switch_to_block(test_block);
        self.builder.ins().jump(unknown_block, &[]);
        self.builder.seal_block(test_block);
        self.builder.switch_to_block(unknown_block);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(zero)]);
        self.builder.seal_block(unknown_block);
        self.builder.seal_block(merge_block);
        self.builder.switch_to_block(merge_block);
        self.terminated = false;
        Ok(self.builder.block_params(merge_block)[0])
    }

    /// Compare two non-generic struct values field-by-field.
    fn emit_struct_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        def_id: ori_types::DefId,
        args: &[Ty],
        eq: bool,
    ) -> Result<ir::Value, String> {
        let layout = self
            .struct_layouts
            .get(&def_id)
            .cloned()
            .ok_or_else(|| format!("missing native struct layout for def {}", def_id.0))?;

        let mut result = self.builder.ins().iconst(types::I8, if eq { 1 } else { 0 });
        for (_, field) in layout.fields {
            let concrete_field_ty = substitute_ty_params(&field.ty, args);
            let cl_ty = cl_type(&concrete_field_ty, self.ptr_ty)
                .ok_or_else(|| "struct field has no native layout".to_string())?;
            let left = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), lv, field.offset as i32);
            let right = self
                .builder
                .ins()
                .load(cl_ty, MemFlags::new(), rv, field.offset as i32);
            let field_equal = self.emit_binary(
                if eq { BinaryOp::Eq } else { BinaryOp::Ne },
                left,
                right,
                &concrete_field_ty,
            )?;
            if eq {
                result = self.builder.ins().band(result, field_equal);
            } else {
                result = self.builder.ins().bor(result, field_equal);
            }
        }
        Ok(result)
    }

    /// Compare two `list[T]` values by length and ordered elements.
    fn emit_list_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        elem_ty: &Ty,
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;

        let len_ref = *self
            .func_refs
            .get("ori_list_len")
            .ok_or_else(|| "missing runtime function `ori_list_len`".to_string())?;
        let get_ref = *self
            .func_refs
            .get("ori_list_get")
            .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;

        let left_len_call = self.builder.ins().call(len_ref, &[lv]);
        let left_len = self.builder.inst_results(left_len_call)[0];
        let right_len_call = self.builder.ins().call(len_ref, &[rv]);
        let right_len = self.builder.inst_results(right_len_call)[0];
        let same_len = self.builder.ins().icmp(IntCC::Equal, left_len, right_len);

        let index_var = self.builder.declare_var(types::I64);
        let len_var = self.builder.declare_var(types::I64);
        let equal_var = self.builder.declare_var(types::I8);
        let zero_i64 = self.builder.ins().iconst(types::I64, 0);
        let one_i64 = self.builder.ins().iconst(types::I64, 1);
        let false_i8 = self.builder.ins().iconst(types::I8, 0);
        let true_i8 = self.builder.ins().iconst(types::I8, 1);
        self.builder.def_var(index_var, zero_i64);
        self.builder.def_var(len_var, left_len);
        self.builder.def_var(equal_var, false_i8);

        let init_block = self.builder.create_block();
        let header_block = self.builder.create_block();
        let body_block = self.builder.create_block();
        let step_block = self.builder.create_block();
        let fail_block = self.builder.create_block();
        let exit_block = self.builder.create_block();

        self.builder
            .ins()
            .brif(same_len, init_block, &[], exit_block, &[]);

        self.builder.seal_block(init_block);
        self.builder.switch_to_block(init_block);
        self.builder.def_var(equal_var, true_i8);
        self.builder.ins().jump(header_block, &[]);

        self.builder.switch_to_block(header_block);
        let index = self.builder.use_var(index_var);
        let len = self.builder.use_var(len_var);
        let keep_going = self.builder.ins().icmp(IntCC::SignedLessThan, index, len);
        self.builder
            .ins()
            .brif(keep_going, body_block, &[], exit_block, &[]);

        self.builder.seal_block(body_block);
        self.builder.switch_to_block(body_block);
        let index = self.builder.use_var(index_var);
        let left_call = self.builder.ins().call(get_ref, &[lv, index]);
        let left_stored = self.builder.inst_results(left_call)[0];
        let right_call = self.builder.ins().call(get_ref, &[rv, index]);
        let right_stored = self.builder.inst_results(right_call)[0];
        let left_value = self.decode_list_storage_value(left_stored, elem_ty);
        let right_value = self.decode_list_storage_value(right_stored, elem_ty);
        let elem_equal = self.emit_binary(BinaryOp::Eq, left_value, right_value, elem_ty)?;
        self.builder
            .ins()
            .brif(elem_equal, step_block, &[], fail_block, &[]);

        self.builder.seal_block(step_block);
        self.builder.switch_to_block(step_block);
        let index = self.builder.use_var(index_var);
        let next = self.builder.ins().iadd(index, one_i64);
        self.builder.def_var(index_var, next);
        self.builder.ins().jump(header_block, &[]);

        self.builder.seal_block(fail_block);
        self.builder.switch_to_block(fail_block);
        self.builder.def_var(equal_var, false_i8);
        self.builder.ins().jump(exit_block, &[]);

        self.builder.seal_block(header_block);
        self.builder.seal_block(exit_block);
        self.builder.switch_to_block(exit_block);
        self.terminated = false;
        let result = self.builder.use_var(equal_var);
        Ok(self.maybe_invert_equality(result, eq))
    }

    /// Compare two `set[T]` values by length and unordered membership.
    fn emit_set_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        elem_ty: &Ty,
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;

        let len_ref = *self
            .func_refs
            .get("ori_set_len")
            .ok_or_else(|| "missing runtime function `ori_set_len`".to_string())?;
        let get_ref = *self
            .func_refs
            .get("ori_list_get")
            .ok_or_else(|| "missing runtime function `ori_list_get`".to_string())?;
        let contains_name = if matches!(elem_ty, Ty::String) {
            "ori_set_contains_string"
        } else {
            "ori_set_contains"
        };
        let contains_ref = *self
            .func_refs
            .get(contains_name)
            .ok_or_else(|| format!("missing runtime function `{contains_name}`"))?;

        let left_len_call = self.builder.ins().call(len_ref, &[lv]);
        let left_len = self.builder.inst_results(left_len_call)[0];
        let right_len_call = self.builder.ins().call(len_ref, &[rv]);
        let right_len = self.builder.inst_results(right_len_call)[0];
        let same_len = self.builder.ins().icmp(IntCC::Equal, left_len, right_len);

        let index_var = self.builder.declare_var(types::I64);
        let len_var = self.builder.declare_var(types::I64);
        let equal_var = self.builder.declare_var(types::I8);
        let zero_i64 = self.builder.ins().iconst(types::I64, 0);
        let one_i64 = self.builder.ins().iconst(types::I64, 1);
        let false_i8 = self.builder.ins().iconst(types::I8, 0);
        let true_i8 = self.builder.ins().iconst(types::I8, 1);
        self.builder.def_var(index_var, zero_i64);
        self.builder.def_var(len_var, left_len);
        self.builder.def_var(equal_var, false_i8);

        let init_block = self.builder.create_block();
        let header_block = self.builder.create_block();
        let body_block = self.builder.create_block();
        let step_block = self.builder.create_block();
        let fail_block = self.builder.create_block();
        let exit_block = self.builder.create_block();

        self.builder
            .ins()
            .brif(same_len, init_block, &[], exit_block, &[]);

        self.builder.seal_block(init_block);
        self.builder.switch_to_block(init_block);
        self.builder.def_var(equal_var, true_i8);
        self.builder.ins().jump(header_block, &[]);

        self.builder.switch_to_block(header_block);
        let index = self.builder.use_var(index_var);
        let len = self.builder.use_var(len_var);
        let keep_going = self.builder.ins().icmp(IntCC::SignedLessThan, index, len);
        self.builder
            .ins()
            .brif(keep_going, body_block, &[], exit_block, &[]);

        self.builder.seal_block(body_block);
        self.builder.switch_to_block(body_block);
        let index = self.builder.use_var(index_var);
        let left_call = self.builder.ins().call(get_ref, &[lv, index]);
        let stored_item = self.builder.inst_results(left_call)[0];
        let contains_call = self.builder.ins().call(contains_ref, &[rv, stored_item]);
        let contains = self.builder.inst_results(contains_call)[0];
        self.builder
            .ins()
            .brif(contains, step_block, &[], fail_block, &[]);

        self.builder.seal_block(step_block);
        self.builder.switch_to_block(step_block);
        let index = self.builder.use_var(index_var);
        let next = self.builder.ins().iadd(index, one_i64);
        self.builder.def_var(index_var, next);
        self.builder.ins().jump(header_block, &[]);

        self.builder.seal_block(fail_block);
        self.builder.switch_to_block(fail_block);
        self.builder.def_var(equal_var, false_i8);
        self.builder.ins().jump(exit_block, &[]);

        self.builder.seal_block(header_block);
        self.builder.seal_block(exit_block);
        self.builder.switch_to_block(exit_block);
        self.terminated = false;
        let result = self.builder.use_var(equal_var);
        Ok(self.maybe_invert_equality(result, eq))
    }

    /// Compare two `map[K,V]` values by length, key membership, and value equality.
    fn emit_map_equality(
        &mut self,
        lv: ir::Value,
        rv: ir::Value,
        key_ty: &Ty,
        value_ty: &Ty,
        eq: bool,
    ) -> Result<ir::Value, String> {
        use ir::condcodes::IntCC;

        let len_ref = *self
            .func_refs
            .get("ori_map_len")
            .ok_or_else(|| "missing runtime function `ori_map_len`".to_string())?;
        let key_at_ref = *self
            .func_refs
            .get("ori_map_key_at")
            .ok_or_else(|| "missing runtime function `ori_map_key_at`".to_string())?;
        let value_at_ref = *self
            .func_refs
            .get("ori_map_value_at")
            .ok_or_else(|| "missing runtime function `ori_map_value_at`".to_string())?;
        let contains_name = if matches!(key_ty, Ty::String) {
            "ori_map_contains_string"
        } else {
            "ori_map_contains"
        };
        let get_name = if matches!(key_ty, Ty::String) {
            "ori_map_get_string"
        } else {
            "ori_map_get"
        };
        let contains_ref = *self
            .func_refs
            .get(contains_name)
            .ok_or_else(|| format!("missing runtime function `{contains_name}`"))?;
        let get_ref = *self
            .func_refs
            .get(get_name)
            .ok_or_else(|| format!("missing runtime function `{get_name}`"))?;

        let left_len_call = self.builder.ins().call(len_ref, &[lv]);
        let left_len = self.builder.inst_results(left_len_call)[0];
        let right_len_call = self.builder.ins().call(len_ref, &[rv]);
        let right_len = self.builder.inst_results(right_len_call)[0];
        let same_len = self.builder.ins().icmp(IntCC::Equal, left_len, right_len);

        let index_var = self.builder.declare_var(types::I64);
        let len_var = self.builder.declare_var(types::I64);
        let equal_var = self.builder.declare_var(types::I8);
        let zero_i64 = self.builder.ins().iconst(types::I64, 0);
        let one_i64 = self.builder.ins().iconst(types::I64, 1);
        let false_i8 = self.builder.ins().iconst(types::I8, 0);
        let true_i8 = self.builder.ins().iconst(types::I8, 1);
        self.builder.def_var(index_var, zero_i64);
        self.builder.def_var(len_var, left_len);
        self.builder.def_var(equal_var, false_i8);

        let init_block = self.builder.create_block();
        let header_block = self.builder.create_block();
        let body_block = self.builder.create_block();
        let compare_block = self.builder.create_block();
        let step_block = self.builder.create_block();
        let fail_block = self.builder.create_block();
        let exit_block = self.builder.create_block();

        self.builder
            .ins()
            .brif(same_len, init_block, &[], exit_block, &[]);

        self.builder.seal_block(init_block);
        self.builder.switch_to_block(init_block);
        self.builder.def_var(equal_var, true_i8);
        self.builder.ins().jump(header_block, &[]);

        self.builder.switch_to_block(header_block);
        let index = self.builder.use_var(index_var);
        let len = self.builder.use_var(len_var);
        let keep_going = self.builder.ins().icmp(IntCC::SignedLessThan, index, len);
        self.builder
            .ins()
            .brif(keep_going, body_block, &[], exit_block, &[]);

        self.builder.seal_block(body_block);
        self.builder.switch_to_block(body_block);
        let index = self.builder.use_var(index_var);
        let key_call = self.builder.ins().call(key_at_ref, &[lv, index]);
        let stored_key = self.builder.inst_results(key_call)[0];
        let contains_call = self.builder.ins().call(contains_ref, &[rv, stored_key]);
        let contains = self.builder.inst_results(contains_call)[0];
        self.builder
            .ins()
            .brif(contains, compare_block, &[], fail_block, &[]);

        self.builder.seal_block(compare_block);
        self.builder.switch_to_block(compare_block);
        let index = self.builder.use_var(index_var);
        let key_call = self.builder.ins().call(key_at_ref, &[lv, index]);
        let stored_key = self.builder.inst_results(key_call)[0];
        let left_value_call = self.builder.ins().call(value_at_ref, &[lv, index]);
        let left_stored = self.builder.inst_results(left_value_call)[0];
        let right_value_call = self.builder.ins().call(get_ref, &[rv, stored_key]);
        let right_stored = self.builder.inst_results(right_value_call)[0];
        let left_value = self.decode_list_storage_value(left_stored, value_ty);
        let right_value = self.decode_list_storage_value(right_stored, value_ty);
        let values_equal = self.emit_binary(BinaryOp::Eq, left_value, right_value, value_ty)?;
        self.builder
            .ins()
            .brif(values_equal, step_block, &[], fail_block, &[]);

        self.builder.seal_block(step_block);
        self.builder.switch_to_block(step_block);
        let index = self.builder.use_var(index_var);
        let next = self.builder.ins().iadd(index, one_i64);
        self.builder.def_var(index_var, next);
        self.builder.ins().jump(header_block, &[]);

        self.builder.seal_block(fail_block);
        self.builder.switch_to_block(fail_block);
        self.builder.def_var(equal_var, false_i8);
        self.builder.ins().jump(exit_block, &[]);

        self.builder.seal_block(header_block);
        self.builder.seal_block(exit_block);
        self.builder.switch_to_block(exit_block);
        self.terminated = false;
        let result = self.builder.use_var(equal_var);
        Ok(self.maybe_invert_equality(result, eq))
    }

    fn maybe_invert_equality(&mut self, equal: ir::Value, eq: bool) -> ir::Value {
        if eq {
            equal
        } else {
            use ir::condcodes::IntCC;
            let zero = self.builder.ins().iconst(types::I8, 0);
            self.builder.ins().icmp(IntCC::Equal, equal, zero)
        }
    }
}

// == Public entry points ==

/// When `ORI_DUMP_ARC=1` (or a path in `ORI_DUMP_ARC`), append a per-function
/// summary of the ARC runtime calls present in the final CLIF — retain,
/// release and edge traffic — to stderr or the given file. This is the
/// `--expandArc` analog (LANG-MEM-7): it makes inserted RC ops visible so
/// elision work (LANG-MEM-4) can count them before/after.
fn maybe_dump_arc(
    func_name: &str,
    ctx: &cranelift_codegen::Context,
    arc_symbol_by_func_index: &HashMap<u32, &'static str>,
) {
    let Ok(spec) = std::env::var("ORI_DUMP_ARC") else {
        return;
    };
    if spec.is_empty() || spec == "0" {
        return;
    }
    let mut counts: Vec<(&'static str, u32)> = Vec::new();
    let mut sequence: Vec<&'static str> = Vec::new();
    for block in ctx.func.layout.blocks() {
        for inst in ctx.func.layout.block_insts(block) {
            let ir::InstructionData::Call { func_ref, .. } = ctx.func.dfg.insts[inst] else {
                continue;
            };
            let ir::ExternalName::User(name_ref) = ctx.func.dfg.ext_funcs[func_ref].name else {
                continue;
            };
            let user_name = &ctx.func.params.user_named_funcs()[name_ref];
            let Some(symbol) = arc_symbol_by_func_index.get(&user_name.index) else {
                continue;
            };
            sequence.push(symbol);
            match counts.iter_mut().find(|(s, _)| s == symbol) {
                Some((_, n)) => *n += 1,
                None => counts.push((symbol, 1)),
            }
        }
    }
    let mut text = format!(";; ---- ARC ops for {func_name} ----\n");
    if sequence.is_empty() {
        text.push_str(";;   (none)\n");
    } else {
        text.push_str(";;  ");
        for (symbol, n) in &counts {
            text.push_str(&format!(" {symbol}={n}"));
        }
        text.push('\n');
        text.push_str(&format!(";;   seq: {}\n", sequence.join(" ")));
    }
    if spec == "1" {
        eprint!("{text}");
        return;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&spec)
    {
        let _ = write!(f, "{text}");
    } else {
        eprint!("{text}");
    }
}

/// When `ORI_DUMP_CLIF=1` (or a path in `ORI_DUMP_CLIF`), append the CLIF text
/// for `func_name` to stderr or the given file (LANG-PERF-2-0).
fn maybe_dump_clif(func_name: &str, ctx: &cranelift_codegen::Context) {
    let Ok(spec) = std::env::var("ORI_DUMP_CLIF") else {
        return;
    };
    if spec.is_empty() || spec == "0" {
        return;
    }
    let text = format!(";; ---- {func_name} ----\n{}\n", ctx.func.display());
    if spec == "1" {
        eprint!("{text}");
        return;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&spec)
    {
        let _ = write!(f, "{text}");
    } else {
        eprint!("{text}");
    }
}

/// Cranelift flags for product codegen (LANG-PERF).
///
/// Defaults in cranelift leave `enable_verifier=true` and `opt_level=none`,
/// which are correct for compiler-dev debugging but slow for end-user
/// `ori run` / `ori compile`. We disable the verifier on the product path and
/// use `speed` for AOT object emission / `none` for JIT (lower latency to first
/// instruction on small programs).
pub(crate) fn cranelift_product_flags(for_jit: bool) -> settings::Flags {
    let mut builder = settings::builder();
    // Verifier validates CL IR; expensive and only needed when debugging
    // the backend itself. Failures still surface as hard errors from emit.
    let _ = builder.set("enable_verifier", "false");
    let opt = if for_jit { "none" } else { "speed" };
    let _ = builder.set("opt_level", opt);
    // Position-independent code for PIE-friendly object files / shared runtimes.
    // (JITBuilder::with_flags forces is_pic=false after applying caller flags.)
    if !for_jit {
        let _ = builder.set("is_pic", "true");
    }
    settings::Flags::new(builder)
}

/// Construct an `ObjectModule` configured for the host target with the
/// standard Cranelift settings. Used by `emit_native` (AOT) and available to
/// callers that need to build a compatible module by hand.
fn make_object_module() -> Result<ObjectModule, String> {
    let flags = cranelift_product_flags(false);
    let isa = cranelift_native::builder()
        .map_err(|e| format!("native ISA unavailable: {e}"))?
        .finish(flags)
        .map_err(|e| format!("ISA build failed: {e}"))?;
    let builder = ObjectBuilder::new(isa, "ori_module", cranelift_module::default_libcall_names())
        .map_err(|e| format!("ObjectBuilder failed: {e}"))?;
    Ok(ObjectModule::new(builder))
}

impl NativeBackend<ObjectModule> {
    /// AOT compile: lower the HIR, finish the `ObjectModule`, and emit the
    /// object file bytes (`*.o` on Unix, `*.obj` on Windows MSVC).
    pub fn compile(self, hir: &HirModule) -> Result<Vec<u8>, String> {
        let backend = self.prepare(hir)?;
        backend
            .module
            .finish()
            .emit()
            .map_err(|e| format!("object emit failed: {e}"))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeEmitOptions {
    /// Shared-library / embed mode (`ori compile --lib`).
    pub lib: bool,
}

pub fn emit_native(hir: &HirModule, obj_path: &std::path::Path) -> Result<(), String> {
    emit_native_with_options(hir, obj_path, NativeEmitOptions::default())
}

pub fn emit_native_with_options(
    hir: &HirModule,
    obj_path: &std::path::Path,
    options: NativeEmitOptions,
) -> Result<(), String> {
    let module = make_object_module()?;
    let mut backend = NativeBackend::new(module)?;
    backend.lib_mode = options.lib;
    let bytes = backend.compile(hir)?;
    std::fs::write(obj_path, &bytes)
        .map_err(|e| format!("write {} failed: {e}", obj_path.display()))
}

/// Native linker facade used by the Cranelift backend.
///
/// The default path uses `rustc` as the native linker driver. This keeps the
/// route independent from a C compiler driver while still letting the Rust
/// toolchain provide the platform-specific CRT and linker configuration.
#[derive(Debug, Clone)]
pub struct NativeLinker {
    strategy: NativeLinkerStrategy,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLinkOptions {
    pub raw_diagnostics: bool,
    /// When true, produce a shared library (`-shared` / `/DLL`) for embed hosts.
    pub shared: bool,
}

#[derive(Clone, Copy)]
struct NativeLinkRequest<'a> {
    object_paths: &'a [PathBuf],
    output_path: &'a Path,
    extra_libraries: &'a [PathBuf],
    options: NativeLinkOptions,
}

#[derive(Clone, Copy)]
struct PlatformLinkContext<'a> {
    library_directories: &'a [PathBuf],
    crt_prefix_objects: &'a [PathBuf],
    crt_suffix_objects: &'a [PathBuf],
    dynamic_linker: Option<&'a Path>,
    extra_arguments: &'a [String],
}

#[derive(Debug, Clone)]
enum NativeLinkerStrategy {
    RustcDriver {
        command: PathBuf,
        linker_override: Option<PathBuf>,
    },
    RawNativeCommand {
        command: PathBuf,
    },
    /// Direct `rust-lld` invocation with compiler-side CRT discovery.
    ///
    /// Opt-in via `ORI_USE_BUNDLED_RUST_LLD=1`. Bypasses `rustc` entirely,
    /// so the user does not need a Rust toolchain installed to link Ori
    /// programs. Supported on `x86_64-pc-windows-msvc` (Rust removal Phase 1,
    /// Windows MSVC), `x86_64-unknown-linux-gnu` (Rust removal Phase 1,
    /// Linux GNU), and `x86_64-apple-darwin` / `aarch64-apple-darwin`
    /// (Rust removal Phase 1, macOS). Other triples fall back to
    /// `RustcDriver` with a diagnostic.
    BundledRustLld {
        lld: PathBuf,
        flavor: String,
        lib_dirs: Vec<PathBuf>,
        /// CRT object files that must precede the user object + libs
        /// (e.g. `crt1.o`, `crti.o` on Linux GNU). Empty on Windows MSVC
        /// where CRT init is driven by `/defaultlib:msvcrt`.
        crt_pre: Vec<PathBuf>,
        /// CRT object files that must follow the user object + libs
        /// (e.g. `crtn.o` on Linux GNU). Empty on Windows MSVC.
        crt_post: Vec<PathBuf>,
        /// Dynamic linker path for ELF targets (e.g.
        /// `/lib64/ld-linux-x86-64.so.2`). `None` on Windows.
        dynamic_linker: Option<PathBuf>,
        /// Extra flavor-specific flags (e.g. `-subsystem:console` on
        /// Windows, `-no-pie` on Linux).
        extra_args: Vec<String>,
    },
    /// Direct system linker invocation (`link.exe`/`ld`/`ld64`) with
    /// compiler-side CRT discovery. No `rust-lld`, no `rustc`.
    ///
    /// Opt-in via `ORI_USE_SYSTEM_LINKER=1`. Linker path override via
    /// `ORI_SYSTEM_LINKER`. Reuses the same CRT discovery as `BundledRustLld`.
    SystemLinker {
        linker: PathBuf,
        lib_dirs: Vec<PathBuf>,
        /// CRT object files that must precede the user object + libs
        /// (e.g. `crt1.o`, `crti.o` on Linux GNU). Empty on Windows MSVC
        /// where CRT init is driven by `/defaultlib:msvcrt`.
        crt_pre: Vec<PathBuf>,
        /// CRT object files that must follow the user object + libs
        /// (e.g. `crtn.o` on Linux GNU). Empty on Windows MSVC.
        crt_post: Vec<PathBuf>,
        /// Dynamic linker path for ELF targets (e.g.
        /// `/lib64/ld-linux-x86-64.so.2`). `None` on Windows and macOS.
        dynamic_linker: Option<PathBuf>,
        /// Platform-specific flags (e.g. `/NOLOGO`, `/SUBSYSTEM:CONSOLE` on
        /// Windows, `-no-pie` on Linux, `-arch`/`-syslibroot` on macOS).
        extra_args: Vec<String>,
    },
}

impl NativeLinker {
    pub fn discover() -> Result<Self, String> {
        if let Ok(command) = std::env::var("ORI_NATIVE_LINKER") {
            let command = command.trim();
            if command.is_empty() {
                return Err("ORI_NATIVE_LINKER is set but empty".to_string());
            }
            return Ok(Self {
                strategy: NativeLinkerStrategy::RawNativeCommand {
                    command: PathBuf::from(command),
                },
            });
        }

        if env_flag("ORI_USE_BUNDLED_RUST_LLD") {
            match discover_bundled_rust_lld() {
                Ok(strategy) => return Ok(Self { strategy }),
                Err(reason) => {
                    // Opt-in failed: surface the error so users learn why the
                    // bundled path did not engage, instead of silently falling
                    // back to the Rustc driver (which would mask the bug they
                    // are trying to diagnose).
                    return Err(format!(
                        "{NATIVE_LINKER_MISSING}: ORI_USE_BUNDLED_RUST_LLD=1 was set but the bundled rust-lld strategy could not be engaged: {reason}"
                    ));
                }
            }
        }

        if env_flag("ORI_USE_SYSTEM_LINKER") {
            match discover_system_linker() {
                Ok(strategy) => return Ok(Self { strategy }),
                Err(reason) => {
                    return Err(format!(
                        "{NATIVE_LINKER_MISSING}: ORI_USE_SYSTEM_LINKER=1 was set but the system linker strategy could not be engaged: {reason}"
                    ));
                }
            }
        }

        // Default path (LANG-PERF + Rust removal for end users):
        // 1. Bundled `rust-lld` when packaged/discovered — faster AOT than GNU
        //    `ld` on measured Linux (~2.5s vs ~4s for examples/hello) and does
        //    **not** require `rustc` (only the lld binary, staged under
        //    `runtime/bin/` in release packages).
        // 2. System linker (OS toolchain) — always available fallback.
        // 3. Legacy `rustc` driver when neither is available or
        //    `ORI_USE_RUSTC_DRIVER=1`.
        if !env_flag("ORI_USE_RUSTC_DRIVER") {
            if let Ok(strategy) = discover_bundled_rust_lld() {
                return Ok(Self { strategy });
            }
            if let Ok(strategy) = discover_system_linker() {
                return Ok(Self { strategy });
            }
        }

        let command = std::env::var("ORI_RUSTC")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("rustc"));
        let linker_override = rust_lld_override(&command)?;
        Ok(Self {
            strategy: NativeLinkerStrategy::RustcDriver {
                command,
                linker_override,
            },
        })
    }

    pub fn strategy_name(&self) -> &'static str {
        match &self.strategy {
            NativeLinkerStrategy::RustcDriver { .. } => "RustcDriver",
            NativeLinkerStrategy::RawNativeCommand { .. } => "RawNativeCommand",
            NativeLinkerStrategy::BundledRustLld { .. } => "BundledRustLld",
            NativeLinkerStrategy::SystemLinker { .. } => "SystemLinker",
        }
    }

    pub fn link(
        &self,
        obj_path: &Path,
        exe_path: &Path,
        extra_libs: &[PathBuf],
        options: NativeLinkOptions,
    ) -> Result<(), String> {
        self.link_many(&[obj_path.to_owned()], exe_path, extra_libs, options)
    }

    pub fn link_many(
        &self,
        obj_paths: &[PathBuf],
        exe_path: &Path,
        extra_libs: &[PathBuf],
        options: NativeLinkOptions,
    ) -> Result<(), String> {
        if obj_paths.is_empty() {
            return Err(format!(
                "{NATIVE_LINK_FAILED}: no object files were provided"
            ));
        }
        let request = NativeLinkRequest {
            object_paths: obj_paths,
            output_path: exe_path,
            extra_libraries: extra_libs,
            options,
        };
        match &self.strategy {
            NativeLinkerStrategy::RustcDriver {
                command,
                linker_override,
            } => link_with_rustc_driver(
                command,
                linker_override.as_deref(),
                obj_paths,
                exe_path,
                extra_libs,
                options,
            ),
            NativeLinkerStrategy::RawNativeCommand { command } => {
                link_with_raw_native_command(command, obj_paths, exe_path, extra_libs, options)
            }
            NativeLinkerStrategy::BundledRustLld {
                lld,
                flavor,
                lib_dirs,
                crt_pre,
                crt_post,
                dynamic_linker,
                extra_args,
            } => link_with_bundled_rust_lld(
                lld,
                flavor,
                PlatformLinkContext {
                    library_directories: lib_dirs,
                    crt_prefix_objects: crt_pre,
                    crt_suffix_objects: crt_post,
                    dynamic_linker: dynamic_linker.as_deref(),
                    extra_arguments: extra_args,
                },
                request,
            ),
            NativeLinkerStrategy::SystemLinker {
                linker,
                lib_dirs,
                crt_pre,
                crt_post,
                dynamic_linker,
                extra_args,
            } => link_with_system_linker(
                linker,
                PlatformLinkContext {
                    library_directories: lib_dirs,
                    crt_prefix_objects: crt_pre,
                    crt_suffix_objects: crt_post,
                    dynamic_linker: dynamic_linker.as_deref(),
                    extra_arguments: extra_args,
                },
                request,
            ),
        }
    }
}

/// Link one object into an executable at `exe_path`.
/// `extra_libs`: additional static libraries to link, usually `ori-runtime`.
pub fn link(obj_path: &Path, exe_path: &Path, extra_libs: &[PathBuf]) -> Result<(), String> {
    link_with_options(obj_path, exe_path, extra_libs, NativeLinkOptions::default())
}

pub fn link_with_options(
    obj_path: &Path,
    exe_path: &Path,
    extra_libs: &[PathBuf],
    options: NativeLinkOptions,
) -> Result<(), String> {
    NativeLinker::discover()?.link(obj_path, exe_path, extra_libs, options)
}

/// Link multiple object files into an executable or shared library.
pub fn link_many_with_options(
    obj_paths: &[PathBuf],
    exe_path: &Path,
    extra_libs: &[PathBuf],
    options: NativeLinkOptions,
) -> Result<(), String> {
    NativeLinker::discover()?.link_many(obj_paths, exe_path, extra_libs, options)
}

const NATIVE_LINKER_MISSING: &str = "native.linker_missing";
const NATIVE_LINK_FAILED: &str = "native.link_failed";
const NATIVE_RUNTIME_SYMBOL_MISSING: &str = "native.runtime_symbol_missing";

fn link_with_rustc_driver(
    command: &Path,
    linker_override: Option<&Path>,
    obj_paths: &[PathBuf],
    exe_path: &Path,
    extra_libs: &[PathBuf],
    options: NativeLinkOptions,
) -> Result<(), String> {
    static NEXT_LINK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let id = NEXT_LINK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let shim = std::env::temp_dir().join(format!("ori_link_shim_{}_{}.rs", std::process::id(), id));
    std::fs::write(&shim, "#![no_main]\n")
        .map_err(|e| format!("failed to write native linker shim {}: {e}", shim.display()))?;

    let mut cmd = std::process::Command::new(command);
    // Same multiarch trap as SystemLinker: rustup LIBRARY_PATH can hide libc.
    cmd.env_remove("LIBRARY_PATH");
    cmd.env_remove("LIBPATH");
    cmd.arg("--edition=2021")
        .arg("--crate-name")
        .arg(format!("ori_link_shim_{id}"))
        .arg(&shim);
    if options.shared {
        cmd.args(["--crate-type", "cdylib"]);
    }
    cmd.arg("-o").arg(exe_path);
    for obj_path in obj_paths {
        cmd.arg("-C")
            .arg(format!("link-arg={}", obj_path.display()));
    }

    if let Some(linker) = linker_override {
        cmd.arg("-C").arg(format!("linker={}", linker.display()));
    }
    // Force multiarch lib search for the underlying `cc`/`ld` (GitHub Actions).
    if cfg!(target_os = "linux") {
        for dir in linux_multiarch_lib_dirs() {
            cmd.arg("-C").arg(format!("link-arg=-L{dir}"));
        }
    }

    for lib in extra_libs {
        let s = lib.to_string_lossy();
        if options.shared && is_executable_only_link_argument(&s) {
            continue;
        }
        if options.shared && !cfg!(windows) && is_dynamic_library_path(lib) {
            if let Some(parent) = lib.parent() {
                cmd.arg("-C")
                    .arg(format!("link-arg=-Wl,-rpath,{}", parent.display()));
            }
        }
        // Flag-like entries from runtime-link.json (`-lpthread`, `-lc`, …).
        cmd.arg("-C").arg(format!("link-arg={s}"));
    }

    let output = cmd.output().map_err(|e| {
        format!(
            "{NATIVE_LINKER_MISSING}: could not invoke native linker driver `{}`: {e}",
            command.display()
        )
    });
    let _ = std::fs::remove_file(&shim);
    let output = output?;

    if output.status.success() {
        Ok(())
    } else {
        let mut err = format_native_link_failure(
            "driver",
            command,
            output.status,
            &output.stdout,
            &output.stderr,
            options,
        );
        // Living-maintenance note: RustcDriver links a Rust crate (libstd) plus
        // ori-runtime staticlib (also built with Rust std) → duplicate symbols.
        if String::from_utf8_lossy(&output.stderr).contains("rust_eh_personality")
            || String::from_utf8_lossy(&output.stderr).contains("duplicate symbol")
        {
            err.push_str(
                "\nhint: RustcDriver is not suitable for linking against the packaged \
                 ori-runtime staticlib (duplicate libstd symbols). Prefer SystemLinker \
                 (`ORI_USE_SYSTEM_LINKER=1`) or BundledRustLld for AOT.",
            );
        }
        Err(err)
    }
}

fn rust_lld_override(rustc: &Path) -> Result<Option<PathBuf>, String> {
    if std::env::var("ORI_USE_RUST_LLD").is_err() {
        return Ok(None);
    }

    if let Ok(path) = std::env::var("ORI_RUST_LLD") {
        let path = path.trim();
        if path.is_empty() {
            return Err("ORI_RUST_LLD is set but empty".to_string());
        }
        return Ok(Some(PathBuf::from(path)));
    }

    if let Some(path) = discover_rust_lld_from_rustc(rustc) {
        return Ok(Some(path));
    }

    Ok(Some(PathBuf::from(if cfg!(windows) {
        "rust-lld.exe"
    } else {
        "rust-lld"
    })))
}

fn discover_rust_lld_from_rustc(rustc: &Path) -> Option<PathBuf> {
    let sysroot = std::process::Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())?;
    let host = rustc_host_triple(rustc)?;
    let exe = if cfg!(windows) {
        "rust-lld.exe"
    } else {
        "rust-lld"
    };
    let candidate = PathBuf::from(sysroot)
        .join("lib")
        .join("rustlib")
        .join(host)
        .join("bin")
        .join(exe);
    candidate.is_file().then_some(candidate)
}

fn rustc_host_triple(rustc: &Path) -> Option<String> {
    let output = std::process::Command::new(rustc).arg("-vV").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.strip_prefix("host:")
                .map(|host| host.trim().to_string())
        })
}

/// Truthy env-var check: `1`, `true`, `yes`, `on` (case-insensitive) count as set.
fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

/// Resolve the `BundledRustLld` strategy: locate `rust-lld` and the platform
/// CRT library directories, without invoking `rustc` as a link driver.
fn discover_bundled_rust_lld() -> Result<NativeLinkerStrategy, String> {
    let lld = find_bundled_rust_lld()?;
    if cfg!(target_os = "windows") {
        let lib_dirs = discover_msvc_crt_lib_dirs()?;
        Ok(NativeLinkerStrategy::BundledRustLld {
            lld,
            flavor: "link".to_string(),
            lib_dirs,
            crt_pre: Vec::new(),
            crt_post: Vec::new(),
            dynamic_linker: None,
            extra_args: vec!["-subsystem:console".to_string()],
        })
    } else if cfg!(target_os = "linux") {
        let crt = discover_linux_gnu_crt()?;
        Ok(NativeLinkerStrategy::BundledRustLld {
            lld,
            flavor: "gnu".to_string(),
            lib_dirs: crt.lib_dirs,
            crt_pre: crt.crt_pre,
            crt_post: crt.crt_post,
            dynamic_linker: Some(crt.dynamic_linker),
            extra_args: vec!["-no-pie".to_string()],
        })
    } else if cfg!(target_os = "macos") {
        let crt = discover_macos_crt()?;
        Ok(NativeLinkerStrategy::BundledRustLld {
            lld,
            flavor: "darwin".to_string(),
            lib_dirs: Vec::new(),
            crt_pre: Vec::new(),
            crt_post: Vec::new(),
            dynamic_linker: None,
            extra_args: vec![
                "-arch".to_string(),
                crt.arch,
                "-platform_version".to_string(),
                "macos".to_string(),
                crt.deployment_target,
                crt.sdk_version,
                "-syslibroot".to_string(),
                crt.sdk_path.display().to_string(),
            ],
        })
    } else {
        Err(format!(
            "bundled rust-lld strategy is not implemented for target os `{}`",
            std::env::consts::OS
        ))
    }
}

/// Resolve the `SystemLinker` strategy: locate the platform native linker
/// (`link.exe`/`ld`/`ld64`) and CRT library paths, without `rust-lld` or `rustc`.
fn discover_system_linker() -> Result<NativeLinkerStrategy, String> {
    if cfg!(target_os = "windows") {
        let lib_dirs = discover_msvc_crt_lib_dirs()?;
        let linker = find_windows_link_exe(&lib_dirs[0])?;
        Ok(NativeLinkerStrategy::SystemLinker {
            linker,
            lib_dirs,
            crt_pre: Vec::new(),
            crt_post: Vec::new(),
            dynamic_linker: None,
            extra_args: vec!["/NOLOGO".to_string(), "/SUBSYSTEM:CONSOLE".to_string()],
        })
    } else if cfg!(target_os = "linux") {
        let crt = discover_linux_gnu_crt()?;
        let linker = find_linux_ld()?;
        Ok(NativeLinkerStrategy::SystemLinker {
            linker,
            lib_dirs: crt.lib_dirs,
            crt_pre: crt.crt_pre,
            crt_post: crt.crt_post,
            dynamic_linker: Some(crt.dynamic_linker),
            extra_args: vec!["-no-pie".to_string()],
        })
    } else if cfg!(target_os = "macos") {
        let crt = discover_macos_crt()?;
        let linker = find_macos_ld()?;
        Ok(NativeLinkerStrategy::SystemLinker {
            linker,
            lib_dirs: Vec::new(),
            crt_pre: Vec::new(),
            crt_post: Vec::new(),
            dynamic_linker: None,
            extra_args: vec![
                "-arch".to_string(),
                crt.arch,
                "-platform_version".to_string(),
                "macos".to_string(),
                crt.deployment_target,
                crt.sdk_version,
                "-syslibroot".to_string(),
                crt.sdk_path.display().to_string(),
            ],
        })
    } else {
        Err(format!(
            "system linker strategy is not implemented for target os `{}`",
            std::env::consts::OS
        ))
    }
}

/// Explicit system linker path from `ORI_SYSTEM_LINKER`, when set.
fn find_system_linker_override() -> Result<Option<PathBuf>, String> {
    if let Ok(path) = std::env::var("ORI_SYSTEM_LINKER") {
        let path = path.trim();
        if path.is_empty() {
            return Err("ORI_SYSTEM_LINKER is set but empty".to_string());
        }
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        return Err(format!(
            "ORI_SYSTEM_LINKER points to `{}` which does not exist",
            candidate.display()
        ));
    }
    Ok(None)
}

/// Locate MSVC `link.exe` for the `SystemLinker` strategy.
///
/// Discovery order:
/// 1. `ORI_SYSTEM_LINKER` — explicit path.
/// 2. `<VS>\VC\Tools\MSVC\<ver>\bin\Hostx64\<arch>\link.exe` (or `Hostx86`).
fn find_windows_link_exe(msvc_lib: &Path) -> Result<PathBuf, String> {
    if let Some(path) = find_system_linker_override()? {
        return Ok(path);
    }
    let arch = msvc_arch_dir();
    let msvc_ver_dir = msvc_lib
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| format!("cannot derive MSVC tools dir from `{}`", msvc_lib.display()))?;
    for host in ["Hostx64", "Hostx86"] {
        let candidate = msvc_ver_dir
            .join("bin")
            .join(host)
            .join(arch)
            .join("link.exe");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "could not find link.exe under MSVC tools dir `{}`; set ORI_SYSTEM_LINKER to override",
        msvc_ver_dir.display()
    ))
}

/// Locate a system linker for the `SystemLinker` strategy on Linux.
///
/// Discovery order:
/// 1. `ORI_SYSTEM_LINKER` — explicit path.
/// 2. **C compiler driver** (`CC`, then `cc`/`gcc`) — preferred: multiarch
///    `-L` / `-lc` resolution works on Debian/Ubuntu CI (bare `ld` often fails
///    with `cannot find -lc`).
/// 3. Fast bare linkers: `mold`, `ld.lld`, `ld`.
/// 4. `{CC} -print-prog-name=ld` as last resort.
fn find_linux_ld() -> Result<PathBuf, String> {
    if let Some(path) = find_system_linker_override()? {
        return Ok(path);
    }
    if let Ok(cc) = std::env::var("CC") {
        let cc = cc.trim();
        if !cc.is_empty() {
            if Path::new(cc).is_file() {
                return Ok(PathBuf::from(cc));
            }
            if let Some(path) = which_on_path(cc) {
                return Ok(path);
            }
        }
    }
    for name in ["cc", "gcc"] {
        if let Some(path) = which_on_path(name) {
            return Ok(path);
        }
    }
    for name in linux_system_linker_path_candidates() {
        if let Some(path) = which_on_path(name) {
            return Ok(path);
        }
    }
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    cc_print_prog_name(&cc, "ld")
}

/// Ordered bare names tried on `PATH` after the C compiler driver.
fn linux_system_linker_path_candidates() -> &'static [&'static str] {
    &["mold", "ld.lld", "ld"]
}

fn is_unix_cc_link_driver(linker: &Path) -> bool {
    let name = linker
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name == "cc"
        || name == "gcc"
        || name == "clang"
        || name.ends_with("-gcc")
        || name.ends_with("-cc")
        || name.ends_with("-clang")
}

/// Locate macOS `ld` for the `SystemLinker` strategy.
///
/// Discovery order:
/// 1. `ORI_SYSTEM_LINKER` — explicit path.
/// 2. `xcrun --find ld`.
fn find_macos_ld() -> Result<PathBuf, String> {
    if let Some(path) = find_system_linker_override()? {
        return Ok(path);
    }
    xcrun_find_tool("ld")
}

/// Locate `rust-lld` (bundled with Ori, overridden via env, or borrowed from
/// the Rust toolchain as a bootstrap). Discovery order:
///
/// 1. `ORI_RUST_LLD` — explicit path.
/// 2. `<ori exe dir>/runtime/bin/rust-lld[.exe]` — release package layout.
/// 3. `<ori exe dir>/rust-lld[.exe]` — bootstrap/dev layout.
/// 4. `<rustc sysroot>/lib/rustlib/<host>/bin/rust-lld[.exe]` — Rust toolchain.
fn find_bundled_rust_lld() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("ORI_RUST_LLD") {
        let path = path.trim();
        if path.is_empty() {
            return Err("ORI_RUST_LLD is set but empty".to_string());
        }
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            // Explicit path: still require a runnable binary (missing libLLVM → fail early).
            return validate_rust_lld_runs(&candidate).map(|()| candidate);
        }
        return Err(format!(
            "ORI_RUST_LLD points to `{}` which does not exist",
            candidate.display()
        ));
    }

    if let Some(bundled) = discover_bundled_rust_lld_next_to_exe() {
        return Ok(bundled);
    }

    if let Some(sysroot_lld) = discover_rust_lld_from_rustc(Path::new("rustc")) {
        if validate_rust_lld_runs(&sysroot_lld).is_ok() {
            return Ok(sysroot_lld);
        }
    }

    Err(
        "could not locate a runnable rust-lld; set ORI_RUST_LLD or install Rust toolchain"
            .to_string(),
    )
}

fn discover_bundled_rust_lld_next_to_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    discover_bundled_rust_lld_from_exe_dir(dir)
}

fn discover_bundled_rust_lld_from_exe_dir(dir: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "rust-lld.exe"
    } else {
        "rust-lld"
    };
    let candidates = [dir.join("runtime").join("bin").join(name), dir.join(name)];
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file() && validate_rust_lld_runs(candidate).is_ok())
}

/// `rust-lld` from the Rust sysroot is often dynamically linked to `libLLVM` and
/// fails with exit 127 when copied into a release package without that library.
/// Prefer SystemLinker in that case rather than a non-runnable BundledRustLld.
fn validate_rust_lld_runs(lld: &Path) -> Result<(), String> {
    let output = std::process::Command::new(lld)
        .arg("-flavor")
        .arg(if cfg!(windows) {
            "link"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "gnu"
        })
        .arg("--version")
        .output()
        .map_err(|e| format!("could not execute `{}`: {e}", lld.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!(
        "`{} -flavor <target> --version` failed (status {:?}): {}{}",
        lld.display(),
        output.status.code(),
        stdout.trim(),
        stderr.trim()
    ))
}

/// Discover Windows MSVC CRT library directories using `vswhere.exe` and the
/// Windows SDK layout. Does not require `vcvarsall.bat` to be loaded.
///
/// Returns the three lib directories needed to link a `mainCRTStartup`-style
/// console binary against the dynamic CRT (`msvcrt`):
/// - `<VS>\VC\Tools\MSVC\<ver>\lib\<arch>`
/// - `<WindowsKits>\Lib\<sdk>\um\<arch>`
/// - `<WindowsKits>\Lib\<sdk>\ucrt\<arch>`
fn discover_msvc_crt_lib_dirs() -> Result<Vec<PathBuf>, String> {
    let arch = msvc_arch_dir().to_string();
    let vs_install = find_vs_install_via_vswhere()?;
    let msvc_lib = find_latest_msvc_tools_lib(&vs_install, &arch)?;
    let windows_kits = find_windows_kits_root()?;
    let sdk_version = pick_latest_windows_sdk_version(&windows_kits)?;
    let um_lib = windows_kits
        .join("Lib")
        .join(&sdk_version)
        .join("um")
        .join(&arch);
    let ucrt_lib = windows_kits
        .join("Lib")
        .join(&sdk_version)
        .join("ucrt")
        .join(&arch);
    for dir in [&msvc_lib, &um_lib, &ucrt_lib] {
        if !dir.is_dir() {
            return Err(format!(
                "MSVC CRT discovery expected lib directory `{}` but it does not exist",
                dir.display()
            ));
        }
    }
    Ok(vec![msvc_lib, um_lib, ucrt_lib])
}

fn msvc_arch_dir() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "x64"
    } else if cfg!(target_pointer_width = "32") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    }
}

fn find_vs_install_via_vswhere() -> Result<PathBuf, String> {
    let vswhere =
        PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    if !vswhere.is_file() {
        return Err(
            "vswhere.exe not found at the standard install location; install Visual Studio Build Tools"
                .to_string(),
        );
    }
    let output = std::process::Command::new(&vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .map_err(|e| format!("failed to invoke vswhere `{}`: {e}", vswhere.display()))?;
    if !output.status.success() {
        return Err(format!(
            "vswhere exited with status {} while locating Visual Studio install",
            output.status
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(
            "vswhere did not report any Visual Studio install with MSVC tools; install Visual Studio Build Tools"
                .to_string(),
        );
    }
    let install = PathBuf::from(path);
    if !install.is_dir() {
        return Err(format!(
            "vswhere reported install path `{}` which does not exist",
            install.display()
        ));
    }
    Ok(install)
}

fn find_latest_msvc_tools_lib(vs_install: &Path, arch: &str) -> Result<PathBuf, String> {
    let tools_dir = vs_install.join("VC").join("Tools").join("MSVC");
    let mut versions: Vec<_> = std::fs::read_dir(&tools_dir)
        .map_err(|e| format!("cannot read MSVC tools dir `{}`: {e}", tools_dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    versions.sort();
    // Try newest first; pick the first one whose `lib/<arch>` exists.
    for version in versions.iter().rev() {
        let lib = tools_dir.join(version).join("lib").join(arch);
        if lib.is_dir() {
            return Ok(lib);
        }
    }
    Err(format!(
        "no MSVC tools version under `{}` has a `lib/{arch}` directory",
        tools_dir.display()
    ))
}

fn find_windows_kits_root() -> Result<PathBuf, String> {
    if let Ok(kits) = std::env::var("WindowsSdkDir") {
        let kits = kits.trim().trim_end_matches('\\').trim_end_matches('/');
        if !kits.is_empty() {
            let candidate = PathBuf::from(kits);
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }
    let default = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10");
    if default.is_dir() {
        return Ok(default);
    }
    Err("Windows SDK root not found; set WindowsSdkDir or install Windows SDK".to_string())
}

fn pick_latest_windows_sdk_version(kits_root: &Path) -> Result<String, String> {
    let lib_dir = kits_root.join("Lib");
    let mut versions: Vec<_> = std::fs::read_dir(&lib_dir)
        .map_err(|e| {
            format!(
                "cannot read Windows SDK Lib dir `{}`: {e}",
                lib_dir.display()
            )
        })?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    versions.sort();
    // Prefer the newest one that has `um/<arch>` and `ucrt/<arch>`.
    let arch = msvc_arch_dir();
    for version in versions.iter().rev() {
        let um = lib_dir.join(version).join("um").join(arch);
        let ucrt = lib_dir.join(version).join("ucrt").join(arch);
        if um.is_dir() && ucrt.is_dir() {
            return Ok(version.clone());
        }
    }
    Err(format!(
        "no Windows SDK version under `{}` has both `um/{arch}` and `ucrt/{arch}`",
        lib_dir.display()
    ))
}

/// Linux GNU CRT discovery result.
///
/// `crt_pre` holds the CRT objects that must precede the user object + libs
/// (`crt1.o`, `crti.o`); `crt_post` holds the ones that must follow them
/// (`crtn.o`). `dynamic_linker` is the ELF interpreter path
/// (`ld-linux-x86-64.so.2`).
struct LinuxGnuCrt {
    lib_dirs: Vec<PathBuf>,
    crt_pre: Vec<PathBuf>,
    crt_post: Vec<PathBuf>,
    dynamic_linker: PathBuf,
}

struct MacOsCrt {
    sdk_path: PathBuf,
    sdk_version: String,
    deployment_target: String,
    arch: String,
}

/// Discover Linux GNU CRT components using `cc -print-file-name` and
/// `cc -print-search-dirs`, with fallback to common GNU/Linux system paths.
/// Does not require a Rust toolchain.
///
/// The C compiler driver (`cc`) is used only as a discovery tool — the
/// actual link is performed by `rust-lld` directly. If `cc` is not installed,
/// Ori still tries common libc development paths before failing.
fn discover_linux_gnu_crt() -> Result<LinuxGnuCrt, String> {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    match discover_linux_gnu_crt_with_cc(&cc) {
        Ok(crt) => Ok(crt),
        Err(cc_reason) => discover_linux_gnu_crt_from_common_paths().map_err(|fallback_reason| {
            format!(
                "Linux CRT discovery via `{cc}` failed: {cc_reason}; \
                 fallback search in common system paths also failed: {fallback_reason}"
            )
        }),
    }
}

fn discover_linux_gnu_crt_with_cc(cc: &str) -> Result<LinuxGnuCrt, String> {
    Ok(LinuxGnuCrt {
        lib_dirs: cc_print_search_dirs_libpaths(cc)?,
        crt_pre: vec![
            cc_print_file_name(cc, "crt1.o")?,
            cc_print_file_name(cc, "crti.o")?,
        ],
        crt_post: vec![cc_print_file_name(cc, "crtn.o")?],
        dynamic_linker: discover_linux_dynamic_linker(cc)?,
    })
}

fn discover_linux_gnu_crt_from_common_paths() -> Result<LinuxGnuCrt, String> {
    Ok(LinuxGnuCrt {
        lib_dirs: common_linux_lib_dirs(),
        crt_pre: vec![
            find_common_linux_file("crt1.o")?,
            find_common_linux_file("crti.o")?,
        ],
        crt_post: vec![find_common_linux_file("crtn.o")?],
        dynamic_linker: discover_linux_dynamic_linker_from_common_paths()?,
    })
}

fn common_linux_lib_dirs() -> Vec<PathBuf> {
    [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/lib/x86_64-linux-gnu",
        "/lib/aarch64-linux-gnu",
        "/lib64",
        "/lib",
    ]
    .into_iter()
    .map(PathBuf::from)
    .filter(|path| path.is_dir())
    .collect()
}

fn find_common_linux_file(file: &str) -> Result<PathBuf, String> {
    for dir in common_linux_lib_dirs() {
        let candidate = dir.join(file);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "could not locate `{file}` in common Linux library directories; \
         install libc development files such as libc6-dev"
    ))
}

fn cc_print_file_name(cc: &str, file: &str) -> Result<PathBuf, String> {
    let arg = format!("-print-file-name={file}");
    let output = std::process::Command::new(cc)
        .args([arg.as_str()])
        .output()
        .map_err(|e| format!("failed to invoke `{cc} -print-file-name={file}`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{cc} -print-file-name={file}` exited with status {}",
            output.status
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // `cc` echoes the bare filename when the file is not found; a real path
    // contains a separator.
    if path.is_empty() || !path.contains(std::path::MAIN_SEPARATOR) {
        return Err(format!(
            "`{cc} -print-file-name={file}` returned `{path}` (not found); \
             install the C runtime development files (e.g. libc6-dev)"
        ));
    }
    let candidate = PathBuf::from(path);
    if !candidate.is_file() {
        return Err(format!(
            "`{cc} -print-file-name={file}` returned `{}` which does not exist",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn cc_print_prog_name(cc: &str, prog: &str) -> Result<PathBuf, String> {
    let arg = format!("-print-prog-name={prog}");
    let output = std::process::Command::new(cc)
        .args([arg.as_str()])
        .output()
        .map_err(|e| format!("failed to invoke `{cc} -print-prog-name={prog}`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{cc} -print-prog-name={prog}` exited with status {}",
            output.status
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(format!(
            "`{cc} -print-prog-name={prog}` returned empty; \
             install binutils or set ORI_SYSTEM_LINKER"
        ));
    }
    // GCC often prints a bare tool name (`ld`) rather than an absolute path.
    // Resolve via PATH so SystemLinker works for end users without ORI_SYSTEM_LINKER.
    let candidate = if path.contains(std::path::MAIN_SEPARATOR) {
        PathBuf::from(&path)
    } else {
        which_on_path(&path).ok_or_else(|| {
            format!(
                "`{cc} -print-prog-name={prog}` returned `{path}` (not found on PATH); \
                 install binutils or set ORI_SYSTEM_LINKER"
            )
        })?
    };
    if !candidate.is_file() {
        return Err(format!(
            "`{cc} -print-prog-name={prog}` resolved to `{}` which does not exist",
            candidate.display()
        ));
    }
    Ok(candidate)
}

/// Resolve a bare executable name against `PATH` (Unix-style `:` or Windows `;`).
fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let with_exe = dir.join(format!("{name}.exe"));
            if with_exe.is_file() {
                return Some(with_exe);
            }
        }
    }
    None
}

fn discover_linux_dynamic_linker(cc: &str) -> Result<PathBuf, String> {
    // `cc -print-file-name=ld-linux-x86-64.so.2` works on most distros.
    // Fall back to standard paths if `cc` does not know the interpreter.
    let candidates = [
        "ld-linux-x86-64.so.2",
        "ld-linux.so.2",
        "ld-linux-aarch64.so.1",
    ];
    for &name in &candidates {
        let arg = format!("-print-file-name={name}");
        let output = match std::process::Command::new(cc).args([arg.as_str()]).output() {
            Ok(output) => output,
            Err(_) => break,
        };
        if !output.status.success() {
            continue;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.contains(std::path::MAIN_SEPARATOR) {
            let candidate = PathBuf::from(&path);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    discover_linux_dynamic_linker_from_common_paths()
}

fn discover_linux_dynamic_linker_from_common_paths() -> Result<PathBuf, String> {
    let fallbacks = [
        "/lib64/ld-linux-x86-64.so.2",
        "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
        "/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
        "/lib/ld-linux-aarch64.so.1",
    ];
    for &path in &fallbacks {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err("could not locate the Linux dynamic linker (ld-linux*.so); \
         set CC to a C compiler that knows its target triple"
        .to_string())
}

fn cc_print_search_dirs_libpaths(cc: &str) -> Result<Vec<PathBuf>, String> {
    let output = match std::process::Command::new(cc)
        .args(["-print-search-dirs"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Ok(common_linux_lib_dirs()),
    };
    if !output.status.success() {
        return Err(format!(
            "`{cc} -print-search-dirs` exited with status {}",
            output.status
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut dirs = Vec::new();
    for line in text.lines() {
        // Lines look like `libraries: =/usr/lib/gcc/x86_64-linux-gnu/11/:/usr/lib/...`
        if let Some(rest) = line.strip_prefix("libraries:") {
            let rest = rest.trim().trim_start_matches('=');
            for part in rest.split(':') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let candidate = PathBuf::from(part);
                if candidate.is_dir() && !dirs.contains(&candidate) {
                    dirs.push(candidate);
                }
            }
        }
    }
    if dirs.is_empty() {
        dirs = common_linux_lib_dirs();
    }
    if dirs.is_empty() {
        return Err(format!(
            "`{cc} -print-search-dirs` did not expose any library directories \
             and no fallback paths exist"
        ));
    }
    Ok(dirs)
}

/// Discover macOS CRT/SDK components using `xcrun`. Does not require a Rust
/// toolchain — only the Xcode Command Line Tools (which provide `xcrun` and
/// the macOS SDK).
///
/// `xcrun` is used only as a discovery tool — the actual link is performed by
/// `rust-lld -flavor darwin` directly. If Xcode Command Line Tools are not
/// installed, this strategy cannot engage and the caller should fall back to
/// `RustcDriver`.
fn discover_macos_crt() -> Result<MacOsCrt, String> {
    let sdk_path = xcrun_show_sdk_path()?;
    let sdk_version = xcrun_show_sdk_version()?;
    let arch = macos_arch().to_string();
    let deployment_target = std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| {
        if arch == "arm64" {
            "11.0".to_string()
        } else {
            "10.12".to_string()
        }
    });
    Ok(MacOsCrt {
        sdk_path,
        sdk_version,
        deployment_target,
        arch,
    })
}

fn macos_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

fn xcrun_show_sdk_path() -> Result<PathBuf, String> {
    let output = std::process::Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .map_err(|e| {
            format!(
                "could not invoke `xcrun --show-sdk-path` (is Xcode Command Line Tools \
                 installed?): {e}"
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "`xcrun --show-sdk-path` exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout);
    let path = path.trim();
    if path.is_empty() {
        return Err("`xcrun --show-sdk-path` returned an empty path".to_string());
    }
    let candidate = PathBuf::from(path);
    if !candidate.is_dir() {
        return Err(format!(
            "`xcrun --show-sdk-path` returned `{}` which is not a directory",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn xcrun_show_sdk_version() -> Result<String, String> {
    let output = std::process::Command::new("xcrun")
        .args(["--show-sdk-version"])
        .output()
        .map_err(|e| format!("could not invoke `xcrun --show-sdk-version`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`xcrun --show-sdk-version` exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout);
    let version = version.trim();
    if version.is_empty() {
        return Err("`xcrun --show-sdk-version` returned an empty version".to_string());
    }
    Ok(version.to_string())
}

fn xcrun_find_tool(tool: &str) -> Result<PathBuf, String> {
    let output = std::process::Command::new("xcrun")
        .args(["--find", tool])
        .output()
        .map_err(|e| {
            format!(
                "failed to invoke `xcrun --find {tool}`: {e}; \
                 install Xcode Command Line Tools or set ORI_SYSTEM_LINKER"
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`xcrun --find {tool}` exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(format!("`xcrun --find {tool}` returned an empty path"));
    }
    let candidate = PathBuf::from(path);
    if !candidate.is_file() {
        return Err(format!(
            "`xcrun --find {tool}` returned `{}` which does not exist",
            candidate.display()
        ));
    }
    Ok(candidate)
}

/// Invoke `rust-lld` directly with the discovered CRT lib directories.
///
/// On Windows MSVC, `rust-lld -flavor link` accepts `link.exe`-style args.
/// `extra_libs` already contains the runtime static lib plus the
/// `native_static_libs` list (e.g. `kernel32.lib`, `/defaultlib:msvcrt`)
/// coming from `runtime-link.json`, so we only need to add the lib search
/// paths and the subsystem flag.
///
/// On Linux GNU, `rust-lld -flavor gnu` accepts `ld`-style args. We pass
/// the CRT objects (`crt1.o`, `crti.o`) before the user object + libs and
/// `crtn.o` after them, plus the dynamic linker path and `-no-pie`.
///
/// On macOS, `rust-lld -flavor darwin` accepts `ld64`-style args. The SDK
/// is provided via `-syslibroot` (in `extra_args`), the deployment target
/// via `-platform_version`, and the arch via `-arch`. CRT objects are
/// handled implicitly by the darwin flavor (no `crt1.o`/`crti.o`/`crtn.o`
/// passed explicitly). `-lc` is added by the non-Windows branch below.
fn link_with_bundled_rust_lld(
    lld: &Path,
    flavor: &str,
    context: PlatformLinkContext<'_>,
    request: NativeLinkRequest<'_>,
) -> Result<(), String> {
    let PlatformLinkContext {
        library_directories,
        crt_prefix_objects,
        crt_suffix_objects,
        dynamic_linker,
        extra_arguments,
    } = context;
    let NativeLinkRequest {
        object_paths,
        output_path,
        extra_libraries,
        options,
    } = request;
    let mut cmd = std::process::Command::new(lld);
    cmd.arg("-flavor").arg(flavor);
    if cfg!(windows) {
        if options.shared {
            cmd.arg("-DLL");
        }
        cmd.arg(format!("-OUT:{}", output_path.display()))
            .arg("-DEBUG")
            .arg(format!(
                "-PDB:{}",
                output_path.with_extension("pdb").display()
            ));
    } else {
        if options.shared {
            cmd.arg(if flavor == "darwin" {
                "-dylib"
            } else {
                "-shared"
            });
        }
        cmd.arg("-o").arg(output_path);
    }
    if !options.shared {
        if let Some(linker) = dynamic_linker {
            cmd.arg("-dynamic-linker").arg(linker);
        }
    }
    // GNU linkers otherwise inject a content-independent build-id note. The
    // note changes on every link and makes identical Ori sources produce
    // different release archives; package reproducibility requires a stable
    // link image. Darwin and MSVC use their own deterministic identity paths.
    if cfg!(target_os = "linux") {
        cmd.arg("--build-id=none");
    }
    for arg in extra_arguments {
        if options.shared && is_executable_only_link_argument(arg) {
            continue;
        }
        cmd.arg(arg);
    }
    for dir in library_directories {
        if cfg!(windows) {
            cmd.arg(format!("-libpath:{}", dir.display()));
        } else {
            cmd.arg(format!("-L{}", dir.display()));
        }
    }
    // CRT objects that must precede the user object (Linux GNU: crt1.o, crti.o)
    for crt in crt_prefix_objects {
        if options.shared && is_executable_crt_start(crt) {
            continue;
        }
        cmd.arg(crt);
    }
    for obj_path in object_paths {
        cmd.arg(obj_path);
    }
    for lib in extra_libraries {
        if options.shared && is_executable_only_link_argument(&lib.to_string_lossy()) {
            continue;
        }
        if options.shared && !cfg!(windows) && is_dynamic_library_path(lib) {
            if let Some(parent) = lib.parent() {
                cmd.arg("-rpath").arg(parent);
            }
        }
        cmd.arg(lib);
    }
    // Ensure libc is linked on Linux (Windows pulls it via /defaultlib:msvcrt).
    // The bundled rust-lld path bypasses cc, so unwinding and thread helpers
    // that cc would pull implicitly must be linked explicitly. libgcc_s
    // provides _Unwind_Resume referenced by the packaged static runtime.
    if cfg!(target_os = "linux") {
        cmd.arg("-lc")
            .arg("-lm")
            .arg("-lpthread")
            .arg("-ldl")
            .arg("-lgcc_s");
    } else if !cfg!(windows) {
        cmd.arg("-lc");
    }
    // CRT objects that must follow the libs (Linux GNU: crtn.o)
    for crt in crt_suffix_objects {
        cmd.arg(crt);
    }

    let output = cmd.output().map_err(|e| {
        format!(
            "{NATIVE_LINKER_MISSING}: could not invoke bundled rust-lld `{}`: {e}",
            lld.display()
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format_native_link_failure(
            "bundled-rust-lld",
            lld,
            output.status,
            &output.stdout,
            &output.stderr,
            options,
        ))
    }
}

fn is_executable_crt_start(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("crt1.o" | "Scrt1.o" | "rcrt1.o")
    )
}

fn is_executable_only_link_argument(argument: &str) -> bool {
    let normalized = argument.to_ascii_lowercase();
    normalized == "-no-pie"
        || normalized == "-pie"
        || normalized.starts_with("-subsystem:")
        || normalized.starts_with("/subsystem:")
}

fn is_dynamic_library_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("so" | "dylib" | "dll")
    )
}

/// Invoke the platform system linker directly with discovered CRT paths.
///
/// On Windows MSVC, `link.exe` accepts `/OUT:`, `/LIBPATH:`, `/NOLOGO`, and
/// `/SUBSYSTEM:CONSOLE`. On Linux GNU, `ld` accepts `-o`, `-dynamic-linker`,
/// `-L`, CRT objects, and `-no-pie`. On macOS, `ld` accepts `-o`, `-arch`,
/// `-platform_version`, and `-syslibroot`.
fn link_with_system_linker(
    linker: &Path,
    context: PlatformLinkContext<'_>,
    request: NativeLinkRequest<'_>,
) -> Result<(), String> {
    let PlatformLinkContext {
        library_directories,
        crt_prefix_objects,
        crt_suffix_objects,
        dynamic_linker,
        extra_arguments,
    } = context;
    let NativeLinkRequest {
        object_paths,
        output_path,
        extra_libraries,
        options,
    } = request;
    let mut cmd = std::process::Command::new(linker);
    let cc_driver = !cfg!(windows) && is_unix_cc_link_driver(linker);
    let shared = options.shared;

    // Cargo/rustup often leave LIBRARY_PATH pointing at the Rust sysroot. That
    // shadows Debian multiarch paths and makes both `ld` and `cc` fail with
    // `cannot find -lc` on GitHub Actions. Clear it for the system link step.
    cmd.env_remove("LIBRARY_PATH");
    cmd.env_remove("LIBPATH");

    if cfg!(windows) {
        if shared {
            cmd.arg("/DLL");
        }
        cmd.arg("/DEBUG");
        cmd.arg(format!(
            "/PDB:{}",
            output_path.with_extension("pdb").display()
        ));
        cmd.arg(format!("/OUT:{}", output_path.display()));
    } else {
        if shared {
            // Prefer the C driver for shared libraries (handles PIC + crt).
            if cfg!(target_os = "macos") {
                cmd.arg(if cc_driver { "-dynamiclib" } else { "-dylib" });
            } else if cc_driver {
                cmd.arg("-shared");
                cmd.arg("-fPIC");
            } else {
                cmd.arg("-shared");
            }
        }
        cmd.arg("-o").arg(output_path);
    }

    if cfg!(target_os = "linux") {
        if cc_driver {
            cmd.arg("-Wl,--build-id=none");
        } else {
            cmd.arg("--build-id=none");
        }
    }

    // `cc`/`gcc` as link driver: supply multiarch `-L` explicitly (GHA runners
    // have broken default search for `-lc` even for `/usr/bin/cc`), but skip
    // manual CRT objects / -dynamic-linker (the driver owns those).
    // Bare `ld`/`mold`/`ld.lld` need full CRT + -dynamic-linker.
    // Shared libraries do not use crt1.o / -dynamic-linker.
    if !cc_driver && !shared {
        if let Some(dl) = dynamic_linker {
            cmd.arg("-dynamic-linker").arg(dl);
        }
    }
    for arg in extra_arguments {
        // PIE flags are wrong for shared objects.
        if shared && is_executable_only_link_argument(arg) {
            continue;
        }
        cmd.arg(arg);
    }
    for dir in library_directories {
        if cfg!(windows) {
            cmd.arg(format!("/LIBPATH:{}", dir.display()));
        } else {
            cmd.arg(format!("-L{}", dir.display()));
        }
    }
    if cfg!(target_os = "linux") {
        for extra in linux_multiarch_lib_dirs() {
            cmd.arg(format!("-L{extra}"));
        }
        // Also ask the C driver where libc actually lives (more reliable than
        // hard-coded multiarch paths on odd images).
        for dir in linux_cc_library_dirs(linker) {
            cmd.arg(format!("-L{}", dir.display()));
        }
    }
    if !cc_driver && !shared {
        for crt in crt_prefix_objects {
            cmd.arg(crt);
        }
    }
    for obj_path in object_paths {
        cmd.arg(obj_path);
    }
    // `extra_libs` mixes the runtime staticlib/cdylib path with flag-like
    // entries from runtime-link.json (`-lpthread`, `-lc`, `-no-pie`).
    // Shared mode typically links the runtime **cdylib** (see pipeline).
    for lib in extra_libraries {
        let s = lib.to_string_lossy();
        if shared && is_executable_only_link_argument(&s) {
            continue;
        }
        if s.starts_with('-') {
            if cc_driver && (s == "-lc" || s == "-no-pie") {
                continue;
            }
            cmd.arg(s.as_ref());
        } else {
            // When linking a .so/.dll runtime into another shared library,
            // pass `-Ldir -lname` so the dynamic linker resolves it, and set
            // an rpath to the runtime directory for local smoke tests.
            if shared && !cfg!(windows) {
                if let Some(parent) = lib.parent() {
                    cmd.arg(format!("-L{}", parent.display()));
                    if cc_driver {
                        cmd.arg(format!("-Wl,-rpath,{}", parent.display()));
                    } else {
                        cmd.arg("-rpath").arg(parent);
                    }
                }
                if let Some(name) = lib.file_name().and_then(|n| n.to_str()) {
                    if let Some(stem) = name
                        .strip_prefix("lib")
                        .and_then(|n| n.strip_suffix(".so"))
                        .or_else(|| {
                            name.strip_prefix("lib")
                                .and_then(|n| n.strip_suffix(".dylib"))
                        })
                    {
                        cmd.arg(format!("-l{stem}"));
                        continue;
                    }
                }
            }
            cmd.arg(lib);
        }
    }
    if !cfg!(windows) && !cc_driver {
        cmd.arg("-lc");
    }
    if !cc_driver && !shared {
        for crt in crt_suffix_objects {
            cmd.arg(crt);
        }
    }

    let output = cmd.output().map_err(|e| {
        format!(
            "{NATIVE_LINKER_MISSING}: could not invoke system linker `{}`: {e}",
            linker.display()
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format_native_link_failure(
            "system-linker",
            linker,
            output.status,
            &output.stdout,
            &output.stderr,
            options,
        ))
    }
}

fn linux_multiarch_lib_dirs() -> Vec<&'static str> {
    [
        "/usr/lib/x86_64-linux-gnu",
        "/lib/x86_64-linux-gnu",
        "/usr/lib64",
        "/lib64",
        "/usr/lib",
        "/lib",
    ]
    .into_iter()
    .filter(|p| Path::new(p).is_dir())
    .collect()
}

/// Library directories reported by the C compiler (`cc -print-search-dirs` and
/// the directory of `cc -print-file-name=libc.so`).
fn linux_cc_library_dirs(cc: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(output) = std::process::Command::new(cc)
        .arg("-print-file-name=libc.so")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && path != "libc.so" {
                let p = PathBuf::from(&path);
                if let Some(parent) = p.parent() {
                    if parent.is_dir() {
                        dirs.push(parent.to_path_buf());
                    }
                }
                // Also resolve `../x86_64-linux-gnu` relative to gcc lib dirs.
                if let Some(grand) = Path::new(&path).parent().and_then(|p| p.parent()) {
                    let multi = grand.join("x86_64-linux-gnu");
                    if multi.is_dir() {
                        dirs.push(multi);
                    }
                }
            }
        }
    }
    if let Ok(output) = std::process::Command::new(cc)
        .arg("-print-search-dirs")
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("libraries: =") {
                    for part in rest.split(':') {
                        let p = PathBuf::from(part);
                        if p.is_dir() {
                            dirs.push(p);
                        }
                    }
                }
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

fn link_with_raw_native_command(
    command: &Path,
    obj_paths: &[PathBuf],
    exe_path: &Path,
    extra_libs: &[PathBuf],
    options: NativeLinkOptions,
) -> Result<(), String> {
    let mut cmd = std::process::Command::new(command);
    if cfg!(windows) {
        cmd.arg("/NOLOGO");
        if options.shared {
            cmd.arg("/DLL");
        }
        cmd.arg("/DEBUG")
            .arg(format!("/PDB:{}", exe_path.with_extension("pdb").display()))
            .arg(format!("/OUT:{}", exe_path.display()));
    } else {
        if options.shared {
            cmd.arg(if cfg!(target_os = "macos") {
                if is_unix_cc_link_driver(command) {
                    "-dynamiclib"
                } else {
                    "-dylib"
                }
            } else {
                "-shared"
            });
        }
        cmd.arg("-o").arg(exe_path);
    }
    for obj_path in obj_paths {
        cmd.arg(obj_path);
    }
    for lib in extra_libs {
        if options.shared && is_executable_only_link_argument(&lib.to_string_lossy()) {
            continue;
        }
        if options.shared && !cfg!(windows) && is_dynamic_library_path(lib) {
            if let Some(parent) = lib.parent() {
                if is_unix_cc_link_driver(command) {
                    cmd.arg(format!("-Wl,-rpath,{}", parent.display()));
                } else {
                    cmd.arg("-rpath").arg(parent);
                }
            }
        }
        cmd.arg(lib);
    }

    let output = cmd.output().map_err(|e| {
        format!(
            "{NATIVE_LINKER_MISSING}: could not invoke native linker `{}` from ORI_NATIVE_LINKER: {e}",
            command.display()
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format_native_link_failure(
            "",
            command,
            output.status,
            &output.stdout,
            &output.stderr,
            options,
        ))
    }
}

fn format_native_link_failure(
    kind: &str,
    command: &Path,
    status: impl std::fmt::Display,
    stdout: &[u8],
    stderr: &[u8],
    options: NativeLinkOptions,
) -> String {
    let stdout_text = String::from_utf8_lossy(stdout);
    let stderr_text = String::from_utf8_lossy(stderr);
    let stdout_trimmed = stdout_text.trim();
    let stderr_trimmed = stderr_text.trim();
    let label = if kind.trim().is_empty() {
        "native linker".to_string()
    } else {
        format!("native linker {kind}")
    };
    let missing_symbol = looks_like_missing_native_symbol(stdout_trimmed)
        || looks_like_missing_native_symbol(stderr_trimmed);
    let mut message = String::new();
    if missing_symbol {
        message.push_str(
            &format!("{NATIVE_RUNTIME_SYMBOL_MISSING}: native link failed because at least one native symbol was not resolved.\n"),
        );
        message.push_str(
            "Check whether the packaged ori-runtime was staged for the same compiler version, target and ABI, and whether the runtime exports every symbol used by the native backend.\n",
        );
    } else {
        message.push_str(&format!("{NATIVE_LINK_FAILED}: native linker failed.\n"));
    }
    message.push_str(&format!(
        "{label} `{}` failed with status {status}",
        command.display()
    ));
    if let Some(first_error) = first_non_empty_linker_line(stderr_trimmed, stdout_trimmed) {
        message.push_str(&format!("\nfirst linker message: {first_error}"));
    }
    if options.raw_diagnostics {
        message.push_str(&format!(
            "\nstdout:\n{}\nstderr:\n{}",
            stdout_trimmed, stderr_trimmed
        ));
    } else {
        message.push_str("\nuse `ori compile --native-raw` to print full linker stdout/stderr");
    }
    message
}

fn first_non_empty_linker_line<'a>(stderr: &'a str, stdout: &'a str) -> Option<&'a str> {
    let lines = || stderr.lines().chain(stdout.lines()).map(str::trim);
    // Prefer high-signal diagnostics (multiarch -lc, duplicate std symbols, …)
    // over the generic rustc wrapper line "linking with `cc` failed".
    for needle in [
        "duplicate symbol",
        "cannot find -l",
        "undefined reference",
        "undefined symbol",
        "unresolved external",
    ] {
        if let Some(line) = lines().find(|line| line.to_ascii_lowercase().contains(needle)) {
            return Some(line);
        }
    }
    lines().find(|line| !line.is_empty())
}

fn looks_like_missing_native_symbol(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    [
        "unresolved external symbol",
        "undefined reference",
        "undefined symbol",
        "symbol(s) not found",
        "unresolved symbol",
        "lnk2001",
        "lnk2019",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests;
