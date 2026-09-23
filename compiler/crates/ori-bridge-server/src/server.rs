use crate::protocol::{
    BridgeErrorPayload, CompileModuleRequest, CompileModuleResponse, HandshakeRequest,
    HandshakeResponse, RequestEnvelope, ResponseEnvelope, SerializedBinaryOp, SerializedExpr,
    SerializedFunc, SerializedModule, SerializedStmt, SerializedTy, CURRENT_PROTOCOL_VERSION,
};
use ori_ast::expr::BinaryOp;
use ori_codegen::{emit_native_with_options, NativeEmitOptions};
use ori_diagnostics::Span;
use ori_hir::hir::{
    HirArg, HirBlock, HirExpr, HirExprKind, HirFunc, HirModule, HirParam, HirStmt,
};
use ori_types::{DefId, Ty};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::path::Path;

pub struct BridgeServer;

impl Default for BridgeServer {
    fn default() -> Self {
        Self::new()
    }
}

impl BridgeServer {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_request(&self, req: RequestEnvelope) -> ResponseEnvelope {
        if req.protocol_version != CURRENT_PROTOCOL_VERSION {
            return ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.unsupported_version".to_string(),
                    message: format!(
                        "protocol version {} not supported (expected {})",
                        req.protocol_version, CURRENT_PROTOCOL_VERSION
                    ),
                }),
            };
        }

        match req.command.as_str() {
            "handshake" => self.handle_handshake(req),
            "compile_module" | "compile_empty_module" => self.handle_compile_module(req),
            unknown => ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.unknown_command".to_string(),
                    message: format!("unknown command: {}", unknown),
                }),
            },
        }
    }

    fn handle_handshake(&self, req: RequestEnvelope) -> ResponseEnvelope {
        let _handshake: HandshakeRequest = match serde_json::from_value(req.payload) {
            Ok(h) => h,
            Err(e) => {
                return ResponseEnvelope {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id: req.request_id,
                    status: "error".to_string(),
                    data: None,
                    error: Some(BridgeErrorPayload {
                        code: "bridge.invalid_payload".to_string(),
                        message: e.to_string(),
                    }),
                }
            }
        };

        let response_data = HandshakeResponse {
            server_version: "0.3.8".to_string(),
            protocol_version: CURRENT_PROTOCOL_VERSION,
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            features: vec![
                "cranelift".to_string(),
                "native_aot".to_string(),
                "system_linker".to_string(),
            ],
        };

        ResponseEnvelope {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: req.request_id,
            status: "ok".to_string(),
            data: Some(serde_json::to_value(response_data).unwrap()),
            error: None,
        }
    }

    fn handle_compile_module(&self, req: RequestEnvelope) -> ResponseEnvelope {
        let compile_req: CompileModuleRequest = match serde_json::from_value(req.payload) {
            Ok(c) => c,
            Err(e) => {
                return ResponseEnvelope {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id: req.request_id,
                    status: "error".to_string(),
                    data: None,
                    error: Some(BridgeErrorPayload {
                        code: "bridge.invalid_payload".to_string(),
                        message: e.to_string(),
                    }),
                }
            }
        };

        if let Err(message) = validate_module(&compile_req.module) {
            return ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.unsupported_ir".to_string(),
                    message,
                }),
            };
        }

        let hir = lower_serialized_module(&compile_req.module);
        let out_path = Path::new(&compile_req.output_path);

        let emit_opts = NativeEmitOptions {
            lib: compile_req.lib_mode,
        };

        match emit_native_with_options(&hir, out_path, emit_opts) {
            Ok(()) => {
                let bytes = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
                let res = CompileModuleResponse {
                    output_path: compile_req.output_path,
                    bytes_written: bytes,
                };
                ResponseEnvelope {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id: req.request_id,
                    status: "ok".to_string(),
                    data: Some(serde_json::to_value(res).unwrap()),
                    error: None,
                }
            }
            Err(e) => ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.compilation_failed".to_string(),
                    message: e,
                }),
            },
        }
    }
}

fn lower_serialized_module(sm: &SerializedModule) -> HirModule {
    let mut funcs = Vec::new();

    for (i, f) in sm.funcs.iter().enumerate() {
        funcs.push(lower_func(f, &sm.namespace, DefId(i as u32 + 1)));
    }

    HirModule {
        namespace: SmolStr::new(&sm.namespace),
        structs: vec![],
        enums: vec![],
        traits: vec![],
        trait_impls: vec![],
        funcs,
        consts: vec![],
        externs: vec![],
    }
}

// Reject calls whose signatures are not represented by protocol v1. Fabricating
// zero-argument C externs would pass an incorrect ABI to the native backend.
fn validate_module(module: &SerializedModule) -> Result<(), String> {
    let mut names = std::collections::HashSet::new();
    for func in &module.funcs {
        if !names.insert(&func.name) {
            return Err(format!("duplicate function: {}", func.name));
        }
        let mut locals = HashMap::new();
        for param in &func.params {
            if locals.insert(param.name.clone(), param.ty.clone()).is_some() {
                return Err(format!("duplicate parameter: {}", param.name));
            }
        }
        for stmt in &func.body_stmts {
            validate_stmt(stmt, &mut locals, &func.return_ty)?;
        }
    }
    Ok(())
}

fn validate_stmt(
    s: &SerializedStmt,
    locals: &mut HashMap<String, SerializedTy>,
    return_ty: &SerializedTy,
) -> Result<(), String> {
    match s {
        SerializedStmt::Let { name, ty, value } => {
            let actual = validate_expr(value, locals)?;
            if actual != *ty || locals.contains_key(name) {
                return Err(format!("invalid or duplicate binding: {name}"));
            }
            locals.insert(name.clone(), ty.clone());
        }
        SerializedStmt::Return(Some(e)) => {
            if validate_expr(e, locals)? != *return_ty {
                return Err("return type does not match function signature".to_string());
            }
        }
        SerializedStmt::Return(None) => {
            if *return_ty != SerializedTy::Void {
                return Err("value required by function signature".to_string());
            }
        }
        SerializedStmt::Expr(e) => {
            validate_expr(e, locals)?;
        }
        SerializedStmt::If {
            cond,
            then_stmts,
            else_stmts,
        } => {
            if validate_expr(cond, locals)? != SerializedTy::Bool {
                return Err("if condition must be bool".to_string());
            }
            let mut then_locals = locals.clone();
            let mut else_locals = locals.clone();
            for ts in then_stmts {
                validate_stmt(ts, &mut then_locals, return_ty)?;
            }
            for es in else_stmts {
                validate_stmt(es, &mut else_locals, return_ty)?;
            }
        }
    }
    Ok(())
}

fn validate_expr(
    e: &SerializedExpr,
    locals: &HashMap<String, SerializedTy>,
) -> Result<SerializedTy, String> {
    match e {
        SerializedExpr::Call { callee, args } => {
            if !matches!(callee.as_str(), "println" | "io.println" | "ori.io.println") {
                return Err(format!("call to {callee} requires a typed function signature"));
            }
            if args.len() > 1
                || args
                    .iter()
                    .any(|arg| !matches!(arg, SerializedExpr::StrLit(_)))
            {
                return Err("println requires zero or one string literal in protocol v1".to_string());
            }
            Ok(SerializedTy::Void)
        }
        SerializedExpr::Add(l, r) => validate_int_pair(l, r, locals),
        SerializedExpr::Binary { op, left, right } => {
            validate_int_pair(left, right, locals)?;
            if matches!(
                op,
                SerializedBinaryOp::Eq
                    | SerializedBinaryOp::Ne
                    | SerializedBinaryOp::Lt
                    | SerializedBinaryOp::Le
                    | SerializedBinaryOp::Gt
                    | SerializedBinaryOp::Ge
            ) {
                Ok(SerializedTy::Bool)
            } else {
                Ok(SerializedTy::Int)
            }
        }
        SerializedExpr::IntLit(_) => Ok(SerializedTy::Int),
        SerializedExpr::StrLit(_) => Ok(SerializedTy::String),
        SerializedExpr::BoolLit(_) => Ok(SerializedTy::Bool),
        SerializedExpr::Var(name) => match locals.get(name) {
            Some(SerializedTy::Int) => Ok(SerializedTy::Int),
            Some(_) => Err(format!("variable {name} requires typed lowering")),
            None => Err(format!("undefined variable: {name}")),
        },
    }
}

fn validate_int_pair(
    left: &SerializedExpr,
    right: &SerializedExpr,
    locals: &HashMap<String, SerializedTy>,
) -> Result<SerializedTy, String> {
    if validate_expr(left, locals)? != SerializedTy::Int
        || validate_expr(right, locals)? != SerializedTy::Int
    {
        return Err("arithmetic operands must be int".to_string());
    }
    Ok(SerializedTy::Int)
}

fn lower_ty(ty: &SerializedTy) -> Ty {
    match ty {
        SerializedTy::Int => Ty::Int,
        SerializedTy::Float => Ty::Float,
        SerializedTy::Bool => Ty::Bool,
        SerializedTy::String => Ty::String,
        SerializedTy::Void => Ty::Void,
    }
}

fn lower_func(sf: &SerializedFunc, ns: &str, def_id: DefId) -> HirFunc {
    let mut params = Vec::new();
    for p in &sf.params {
        params.push(HirParam {
            name: SmolStr::new(&p.name),
            ty: lower_ty(&p.ty),
            default: None,
            contract: None,
            variadic: false,
            span: Span::DUMMY,
        });
    }

    let mut stmts = Vec::new();
    for s in &sf.body_stmts {
        stmts.push(lower_stmt(s));
    }

    // Qualify the entrypoint name the same way `is_entry_main` expects:
    // when the module has a namespace, the entry is `<ns>.main`.
    let name = if sf.name == "main" && !ns.is_empty() {
        SmolStr::new(format!("{ns}.main"))
    } else {
        SmolStr::new(&sf.name)
    };

    HirFunc {
        def_id,
        name,
        params,
        return_ty: lower_ty(&sf.return_ty),
        body: HirBlock {
            stmts,
            span: Span::DUMMY,
        },
        closure_captures: vec![],
        is_public: sf.is_public,
        is_async: false,
        is_mut: false,
        is_inline: false,
        is_no_inline: false,
        c_export_name: None,
        span: Span::DUMMY,
    }
}

fn lower_stmt(ss: &SerializedStmt) -> HirStmt {
    match ss {
        SerializedStmt::Let { name, ty, value } => HirStmt::Let {
            name: SmolStr::new(name),
            ty: lower_ty(ty),
            mutable: false,
            value: lower_expr(value),
            span: Span::DUMMY,
        },
        SerializedStmt::Return(maybe_expr) => {
            HirStmt::Return(maybe_expr.as_ref().map(lower_expr), Span::DUMMY)
        }
        SerializedStmt::Expr(expr) => HirStmt::Expr(lower_expr(expr)),
        SerializedStmt::If {
            cond,
            then_stmts,
            else_stmts,
        } => HirStmt::If {
            cond: lower_expr(cond),
            then: HirBlock {
                stmts: then_stmts.iter().map(lower_stmt).collect(),
                span: Span::DUMMY,
            },
            else_ifs: vec![],
            else_: if else_stmts.is_empty() {
                None
            } else {
                Some(HirBlock {
                    stmts: else_stmts.iter().map(lower_stmt).collect(),
                    span: Span::DUMMY,
                })
            },
            span: Span::DUMMY,
        },
    }
}

fn lower_binary_op(op: &SerializedBinaryOp) -> BinaryOp {
    match op {
        SerializedBinaryOp::Add => BinaryOp::Add,
        SerializedBinaryOp::Sub => BinaryOp::Sub,
        SerializedBinaryOp::Mul => BinaryOp::Mul,
        SerializedBinaryOp::Div => BinaryOp::Div,
        SerializedBinaryOp::Mod => BinaryOp::Rem,
        SerializedBinaryOp::Eq => BinaryOp::Eq,
        SerializedBinaryOp::Ne => BinaryOp::Ne,
        SerializedBinaryOp::Lt => BinaryOp::Lt,
        SerializedBinaryOp::Le => BinaryOp::Le,
        SerializedBinaryOp::Gt => BinaryOp::Gt,
        SerializedBinaryOp::Ge => BinaryOp::Ge,
    }
}

fn lower_expr(se: &SerializedExpr) -> HirExpr {
    match se {
        SerializedExpr::IntLit(val) => HirExpr {
            kind: HirExprKind::IntLit(*val),
            ty: Ty::Int,
            span: Span::DUMMY,
        },
        SerializedExpr::StrLit(val) => HirExpr {
            kind: HirExprKind::StrLit(SmolStr::new(val)),
            ty: Ty::String,
            span: Span::DUMMY,
        },
        SerializedExpr::BoolLit(val) => HirExpr {
            kind: HirExprKind::BoolLit(*val),
            ty: Ty::Bool,
            span: Span::DUMMY,
        },
        SerializedExpr::Var(name) => HirExpr {
            kind: HirExprKind::Var(SmolStr::new(name)),
            ty: Ty::Int, // Fallback scalar
            span: Span::DUMMY,
        },
        SerializedExpr::Add(left, right) => HirExpr {
            kind: HirExprKind::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(lower_expr(left)),
                rhs: Box::new(lower_expr(right)),
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        },
        SerializedExpr::Binary { op, left, right } => HirExpr {
            kind: HirExprKind::Binary {
                op: lower_binary_op(op),
                lhs: Box::new(lower_expr(left)),
                rhs: Box::new(lower_expr(right)),
            },
            ty: if matches!(
                op,
                SerializedBinaryOp::Eq
                    | SerializedBinaryOp::Ne
                    | SerializedBinaryOp::Lt
                    | SerializedBinaryOp::Le
                    | SerializedBinaryOp::Gt
                    | SerializedBinaryOp::Ge
            ) {
                Ty::Bool
            } else {
                Ty::Int
            },
            span: Span::DUMMY,
        },
        SerializedExpr::Call { callee, args } => {
            let is_print = callee == "println" || callee == "io.println" || callee == "ori.io.println";
            let (actual_callee, callee_ty) = if is_print {
                (
                    SmolStr::new("ori_io_print"),
                    Ty::Func {
                        params: vec![Ty::String],
                        ret: Box::new(Ty::Void),
                    },
                )
            } else {
                (SmolStr::new(callee), Ty::Void)
            };

            let mut final_args = args.clone();
            // If it's a print call with 0 arguments, provide an empty string so Cranelift
            // receives the required (ptr, len) pair instead of 0 arguments.
            if is_print && final_args.is_empty() {
                final_args.push(SerializedExpr::StrLit(String::new()));
            }

            let lowered_args = final_args
                .iter()
                .map(|a| {
                    let mut le = lower_expr(a);
                    if is_print {
                        le.ty = Ty::String;
                    }
                    HirArg {
                        value: le,
                        label: None,
                        spread: false,
                    }
                })
                .collect();

            HirExpr {
                kind: HirExprKind::Call {
                    callee: Box::new(HirExpr {
                        kind: HirExprKind::Var(actual_callee),
                        ty: callee_ty,
                        span: Span::DUMMY,
                    }),
                    args: lowered_args,
                },
                ty: Ty::Void,
                span: Span::DUMMY,
            }
        },
    }
}
