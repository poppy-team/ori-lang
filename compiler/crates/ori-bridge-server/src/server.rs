use crate::protocol::{
    BridgeErrorPayload, CompileModuleRequest, CompileModuleResponse, HandshakeRequest,
    HandshakeResponse, RequestEnvelope, ResponseEnvelope, SerializedBinaryOp, SerializedExpr,
    SerializedFunc, SerializedModule, SerializedPattern, SerializedStmt, SerializedTy,
    CURRENT_PROTOCOL_VERSION,
};
use ori_ast::expr::BinaryOp;
use ori_codegen::{emit_native_with_options, NativeEmitOptions};
use ori_diagnostics::Span;
use ori_hir::hir::{
    HirArg, HirArm, HirBlock, HirExpr, HirExprArm, HirExprKind, HirFunc, HirLValue, HirModule, HirParam,
    HirPattern, HirStmt,
};
use ori_types::{DefId, Ty};
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
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
    let callable = callable_functions(sm);

    for (i, f) in sm.funcs.iter().enumerate() {
        funcs.push(lower_func(f, &sm.namespace, DefId(i as u32 + 1), &callable));
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

// The scalar subset supports a fixed, explicitly checked number of Int arguments.
// Build the signature table before visiting bodies so forward calls work.
type CallableSignatures = HashMap<String, (Vec<SerializedTy>, SerializedTy)>;

fn callable_functions(module: &SerializedModule) -> CallableSignatures {
    module
        .funcs
        .iter()
        .filter(|f| {
            f.name != "main"
                && f.name != "println"
                && f.params.len() <= 8
                && f.params.iter().all(|p| p.ty == SerializedTy::Int)
                && matches!(&f.return_ty, SerializedTy::Int | SerializedTy::Bool | SerializedTy::String)
        })
        .map(|f| {
            (
                f.name.clone(),
                (f.params.iter().map(|p| p.ty.clone()).collect(), f.return_ty.clone()),
            )
        })
        .collect()
}

// Reject calls whose signatures are not represented by protocol v1. Fabricating
// zero-argument C externs would pass an incorrect ABI to the native backend.
fn validate_module(module: &SerializedModule) -> Result<(), String> {
    let mut names = std::collections::HashSet::new();
    let callable = callable_functions(module);
    for func in &module.funcs {
        if !names.insert(&func.name) {
            return Err(format!("duplicate function: {}", func.name));
        }
        let mut locals = HashMap::new();
        let mut mutable_names = HashSet::new();
        for param in &func.params {
            if locals.insert(param.name.clone(), param.ty.clone()).is_some() {
                return Err(format!("duplicate parameter: {}", param.name));
            }
        }
        for stmt in &func.body_stmts {
            validate_stmt(stmt, &mut locals, &mut mutable_names, &func.return_ty, &callable, false)?;
        }
    }
    Ok(())
}

fn validate_stmt(
    s: &SerializedStmt,
    locals: &mut HashMap<String, SerializedTy>,
    mutable_names: &mut HashSet<String>,
    return_ty: &SerializedTy,
    callable: &CallableSignatures,
    in_loop: bool,
) -> Result<(), String> {
    match s {
        SerializedStmt::Let { name, ty, value, mutable } => {
            let actual = validate_expr(value, locals, callable)?;
            if actual != *ty || locals.contains_key(name) {
                return Err(format!("invalid or duplicate binding: {name}"));
            }
            locals.insert(name.clone(), ty.clone());
            if *mutable {
                mutable_names.insert(name.clone());
            }
        }
        SerializedStmt::Assign { name, value } => {
            if !mutable_names.contains(name) {
                return Err(format!("assignment requires a mutable local: {name}"));
            }
            if Some(&validate_expr(value, locals, callable)?) != locals.get(name) {
                return Err(format!("assignment type does not match local: {name}"));
            }
        }
        SerializedStmt::Return(Some(e)) => {
            if validate_expr(e, locals, callable)? != *return_ty {
                return Err("return type does not match function signature".to_string());
            }
        }
        SerializedStmt::Return(None) => {
            if *return_ty != SerializedTy::Void {
                return Err("value required by function signature".to_string());
            }
        }
        SerializedStmt::Expr(e) => {
            validate_expr(e, locals, callable)?;
        }
        SerializedStmt::Break | SerializedStmt::Continue => {
            if !in_loop {
                return Err("break and continue require a loop".to_string());
            }
        }
        SerializedStmt::If {
            cond,
            then_stmts,
            else_stmts,
        } => {
            if validate_expr(cond, locals, callable)? != SerializedTy::Bool {
                return Err("if condition must be bool".to_string());
            }
            let mut then_locals = locals.clone();
            let mut else_locals = locals.clone();
            let mut then_mutable = mutable_names.clone();
            let mut else_mutable = mutable_names.clone();
            for ts in then_stmts {
                validate_stmt(ts, &mut then_locals, &mut then_mutable, return_ty, callable, in_loop)?;
            }
            for es in else_stmts {
                validate_stmt(es, &mut else_locals, &mut else_mutable, return_ty, callable, in_loop)?;
            }
        }
        SerializedStmt::While { cond, body_stmts } => {
            if validate_expr(cond, locals, callable)? != SerializedTy::Bool {
                return Err("while condition must be bool".to_string());
            }
            let mut body_locals = locals.clone();
            let mut body_mutable = mutable_names.clone();
            for bs in body_stmts {
                validate_stmt(bs, &mut body_locals, &mut body_mutable, return_ty, callable, true)?;
            }
        }
        SerializedStmt::Match { scrutinee, arms } => {
            let ty = validate_expr(scrutinee, locals, callable)?;
            if !matches!(ty, SerializedTy::Int | SerializedTy::Bool) || arms.is_empty() {
                return Err("match requires a scalar scrutinee and at least one arm".to_string());
            }
            let mut ints = HashSet::new();
            let mut bools = HashSet::new();
            let mut wildcard = false;
            for (index, arm) in arms.iter().enumerate() {
                if wildcard {
                    return Err("match wildcard must be the last arm".to_string());
                }
                match &arm.pattern {
                    SerializedPattern::IntLit(n) if ty == SerializedTy::Int => {
                        if !ints.insert(*n) {
                            return Err("duplicate integer match pattern".to_string());
                        }
                    }
                    SerializedPattern::BoolLit(value) if ty == SerializedTy::Bool => {
                        if !bools.insert(*value) {
                            return Err("duplicate boolean match pattern".to_string());
                        }
                    }
                    SerializedPattern::Wildcard => {
                        wildcard = true;
                        if index + 1 != arms.len() {
                            return Err("match wildcard must be the last arm".to_string());
                        }
                    }
                    _ => return Err("match pattern type does not match scrutinee".to_string()),
                }
                let mut arm_locals = locals.clone();
                let mut arm_mutable = mutable_names.clone();
                for stmt in &arm.body_stmts {
                    validate_stmt(stmt, &mut arm_locals, &mut arm_mutable, return_ty, callable, in_loop)?;
                }
            }
            if !wildcard && (ty == SerializedTy::Int || bools.len() != 2) {
                return Err("match is not exhaustive".to_string());
            }
        }
    }
    Ok(())
}

fn validate_expr(
    e: &SerializedExpr,
    locals: &HashMap<String, SerializedTy>,
    callable: &CallableSignatures,
) -> Result<SerializedTy, String> {
    match e {
        SerializedExpr::Call { callee, args } => {
            if let Some((param_tys, result_ty)) = callable.get(callee) {
                if args.len() != param_tys.len() {
                    return Err(format!("call to {callee} expects {} argument(s)", param_tys.len()));
                }
                for (arg, expected) in args.iter().zip(param_tys) {
                    if validate_expr(arg, locals, callable)? != *expected {
                        return Err(format!("call to {callee} has an argument with the wrong type"));
                    }
                }
                return Ok(result_ty.clone());
            }
            if !matches!(callee.as_str(), "println" | "io.println" | "ori.io.println") {
                return Err(format!("call to {callee} requires a typed function signature"));
            }
            if args.len() > 1 {
                return Err("println requires zero or one string argument".to_string());
            }
            if let Some(arg) = args.first() {
                if validate_expr(arg, locals, callable)? != SerializedTy::String {
                    return Err("println argument must be a string".to_string());
                }
            }
            Ok(SerializedTy::Void)
        }
        SerializedExpr::IfExpr { cond, then_expr, else_expr } => {
            if validate_expr(cond, locals, callable)? != SerializedTy::Bool {
                return Err("if-expression condition must be bool".to_string());
            }
            let then_ty = validate_expr(then_expr, locals, callable)?;
            if then_ty != validate_expr(else_expr, locals, callable)?
                || !matches!(&then_ty, SerializedTy::Int | SerializedTy::Bool | SerializedTy::String)
            {
                return Err("if-expression branches must have the same supported type".to_string());
            }
            Ok(then_ty)
        }
        SerializedExpr::MatchExpr { scrutinee, arms } => {
            let scrutinee_ty = validate_expr(scrutinee, locals, callable)?;
            if !matches!(scrutinee_ty, SerializedTy::Int | SerializedTy::Bool) || arms.is_empty() {
                return Err("match expression requires a scalar scrutinee and arms".to_string());
            }
            let mut ints = HashSet::new();
            let mut bools = HashSet::new();
            let mut wildcard = false;
            let mut result_ty = None;
            for arm in arms {
                if wildcard {
                    return Err("match expression wildcard must be last".to_string());
                }
                match &arm.pattern {
                    SerializedPattern::IntLit(n) if scrutinee_ty == SerializedTy::Int => {
                        if !ints.insert(*n) {
                            return Err("duplicate integer match pattern".to_string());
                        }
                    }
                    SerializedPattern::BoolLit(value) if scrutinee_ty == SerializedTy::Bool => {
                        if !bools.insert(*value) {
                            return Err("duplicate boolean match pattern".to_string());
                        }
                    }
                    SerializedPattern::Wildcard => wildcard = true,
                    _ => return Err("match expression pattern has the wrong type".to_string()),
                }
                let arm_ty = validate_expr(&arm.body, locals, callable)?;
                if !matches!(arm_ty, SerializedTy::Int | SerializedTy::Bool | SerializedTy::String)
                    || result_ty.as_ref().is_some_and(|ty| *ty != arm_ty)
                {
                    return Err("match expression arms must have the same supported type".to_string());
                }
                result_ty = Some(arm_ty);
            }
            if !wildcard && (scrutinee_ty == SerializedTy::Int || bools.len() != 2) {
                return Err("match expression is not exhaustive".to_string());
            }
            result_ty.ok_or_else(|| "match expression has no value".to_string())
        }
        SerializedExpr::Add(l, r) => validate_int_pair(l, r, locals, callable),
        SerializedExpr::Binary { op, left, right } => {
            if matches!(op, SerializedBinaryOp::And | SerializedBinaryOp::Or) {
                if validate_expr(left, locals, callable)? != SerializedTy::Bool
                    || validate_expr(right, locals, callable)? != SerializedTy::Bool
                {
                    return Err("logical operands must be bool".to_string());
                }
                return Ok(SerializedTy::Bool);
            }
            validate_int_pair(left, right, locals, callable)?;
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
            Some(ty) if matches!(ty, SerializedTy::Int | SerializedTy::Bool | SerializedTy::String) => Ok(ty.clone()),
            Some(_) => Err(format!("variable {name} requires typed lowering")),
            None => Err(format!("undefined variable: {name}")),
        },
    }
}

fn validate_int_pair(
    left: &SerializedExpr,
    right: &SerializedExpr,
    locals: &HashMap<String, SerializedTy>,
    callable: &CallableSignatures,
) -> Result<SerializedTy, String> {
    if validate_expr(left, locals, callable)? != SerializedTy::Int
        || validate_expr(right, locals, callable)? != SerializedTy::Int
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

fn lower_func(
    sf: &SerializedFunc,
    ns: &str,
    def_id: DefId,
    callable: &CallableSignatures,
) -> HirFunc {
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
    let mut locals: HashMap<String, SerializedTy> = sf
        .params
        .iter()
        .map(|p| (p.name.clone(), p.ty.clone()))
        .collect();
    for s in &sf.body_stmts {
        stmts.push(lower_stmt(s, callable, &mut locals));
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

fn lower_stmt(
    ss: &SerializedStmt,
    callable: &CallableSignatures,
    locals: &mut HashMap<String, SerializedTy>,
) -> HirStmt {
    match ss {
        SerializedStmt::Let { name, ty, value, mutable } => {
            let lowered_value = lower_expr(value, callable, locals);
            locals.insert(name.clone(), ty.clone());
            HirStmt::Let {
                name: SmolStr::new(name),
                ty: lower_ty(ty),
                mutable: *mutable,
                value: lowered_value,
                span: Span::DUMMY,
            }
        }
        SerializedStmt::Assign { name, value } => HirStmt::Assign {
            lvalue: HirLValue::Var(SmolStr::new(name)),
            value: lower_expr(value, callable, locals),
            span: Span::DUMMY,
        },
        SerializedStmt::Return(maybe_expr) => {
            HirStmt::Return(maybe_expr.as_ref().map(|e| lower_expr(e, callable, locals)), Span::DUMMY)
        }
        SerializedStmt::Expr(expr) => HirStmt::Expr(lower_expr(expr, callable, locals)),
        SerializedStmt::Break => HirStmt::Break(Span::DUMMY),
        SerializedStmt::Continue => HirStmt::Continue(Span::DUMMY),
        SerializedStmt::If {
            cond,
            then_stmts,
            else_stmts,
        } => {
            let lowered_cond = lower_expr(cond, callable, locals);
            let mut then_locals = locals.clone();
            let mut else_locals = locals.clone();
            HirStmt::If {
                cond: lowered_cond,
                then: HirBlock {
                    stmts: then_stmts
                        .iter()
                        .map(|s| lower_stmt(s, callable, &mut then_locals))
                        .collect(),
                    span: Span::DUMMY,
                },
                else_ifs: vec![],
                else_: if else_stmts.is_empty() {
                    None
                } else {
                    Some(HirBlock {
                        stmts: else_stmts
                            .iter()
                            .map(|s| lower_stmt(s, callable, &mut else_locals))
                            .collect(),
                        span: Span::DUMMY,
                    })
                },
                span: Span::DUMMY,
            }
        }
        SerializedStmt::While { cond, body_stmts } => {
            let lowered_cond = lower_expr(cond, callable, locals);
            let mut body_locals = locals.clone();
            HirStmt::While {
                cond: lowered_cond,
                body: HirBlock {
                    stmts: body_stmts
                        .iter()
                        .map(|s| lower_stmt(s, callable, &mut body_locals))
                        .collect(),
                    span: Span::DUMMY,
                },
                span: Span::DUMMY,
            }
        }
        SerializedStmt::Match { scrutinee, arms } => {
            let lowered_scrutinee = lower_expr(scrutinee, callable, locals);
            HirStmt::Match {
                scrutinee: lowered_scrutinee,
                arms: arms.iter().map(|arm| {
                    let mut arm_locals = locals.clone();
                    HirArm {
                        pattern: match &arm.pattern {
                            SerializedPattern::IntLit(value) => HirPattern::IntLit(*value),
                            SerializedPattern::BoolLit(value) => HirPattern::BoolLit(*value),
                            SerializedPattern::Wildcard => HirPattern::Wildcard,
                        },
                        guard: None,
                        body: arm.body_stmts.iter().map(|stmt| lower_stmt(stmt, callable, &mut arm_locals)).collect(),
                        span: Span::DUMMY,
                    }
                }).collect(),
                span: Span::DUMMY,
            }
        }
    }
}

fn lower_binary_op(op: &SerializedBinaryOp) -> BinaryOp {
    match op {
        SerializedBinaryOp::Add => BinaryOp::Add,
        SerializedBinaryOp::Sub => BinaryOp::Sub,
        SerializedBinaryOp::Mul => BinaryOp::Mul,
        SerializedBinaryOp::Div => BinaryOp::Div,
        SerializedBinaryOp::Mod => BinaryOp::Rem,
        SerializedBinaryOp::And => BinaryOp::And,
        SerializedBinaryOp::Or => BinaryOp::Or,
        SerializedBinaryOp::Eq => BinaryOp::Eq,
        SerializedBinaryOp::Ne => BinaryOp::Ne,
        SerializedBinaryOp::Lt => BinaryOp::Lt,
        SerializedBinaryOp::Le => BinaryOp::Le,
        SerializedBinaryOp::Gt => BinaryOp::Gt,
        SerializedBinaryOp::Ge => BinaryOp::Ge,
    }
}

fn lower_expr(
    se: &SerializedExpr,
    callable: &CallableSignatures,
    locals: &HashMap<String, SerializedTy>,
) -> HirExpr {
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
            ty: lower_ty(locals.get(name).expect("validated bridge variable is in scope")),
            span: Span::DUMMY,
        },
        SerializedExpr::Add(left, right) => HirExpr {
            kind: HirExprKind::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(lower_expr(left, callable, locals)),
                rhs: Box::new(lower_expr(right, callable, locals)),
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        },
        SerializedExpr::Binary { op, left, right } => HirExpr {
            kind: HirExprKind::Binary {
                op: lower_binary_op(op),
                lhs: Box::new(lower_expr(left, callable, locals)),
                rhs: Box::new(lower_expr(right, callable, locals)),
            },
            ty: if matches!(
                op,
                SerializedBinaryOp::Eq
                    | SerializedBinaryOp::And
                    | SerializedBinaryOp::Or
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
            let local_signature = callable.get(callee);
            let local_result_ty = local_signature.map(|(_, result)| lower_ty(result));
            let (actual_callee, callee_ty) = if is_print {
                (
                    SmolStr::new("ori_io_print"),
                    Ty::Func {
                        params: vec![Ty::String],
                        ret: Box::new(Ty::Void),
                    },
                )
            } else if let Some((param_tys, result_ty)) = local_signature {
                (
                    SmolStr::new(callee),
                    Ty::Func {
                        params: param_tys.iter().map(lower_ty).collect(),
                        ret: Box::new(lower_ty(result_ty)),
                    },
                )
            } else {
                unreachable!("validated bridge call has an unknown signature")
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
                    let mut le = lower_expr(a, callable, locals);
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
                ty: local_result_ty.unwrap_or(Ty::Void),
                span: Span::DUMMY,
            }
        },
        SerializedExpr::IfExpr { cond, then_expr, else_expr } => {
            let then_lowered = lower_expr(then_expr, callable, locals);
            let ty = then_lowered.ty.clone();
            HirExpr {
                kind: HirExprKind::IfExpr {
                    cond: Box::new(lower_expr(cond, callable, locals)),
                    then: Box::new(then_lowered),
                    else_: Box::new(lower_expr(else_expr, callable, locals)),
                },
                ty,
                span: Span::DUMMY,
            }
        }
        SerializedExpr::MatchExpr { scrutinee, arms } => {
            let lowered_arms: Vec<HirExprArm> = arms.iter().map(|arm| HirExprArm {
                pattern: match &arm.pattern {
                    SerializedPattern::IntLit(value) => HirPattern::IntLit(*value),
                    SerializedPattern::BoolLit(value) => HirPattern::BoolLit(*value),
                    SerializedPattern::Wildcard => HirPattern::Wildcard,
                },
                guard: None,
                body: lower_expr(&arm.body, callable, locals),
                span: Span::DUMMY,
            }).collect();
            let ty = lowered_arms[0].body.ty.clone();
            HirExpr {
                kind: HirExprKind::MatchExpr {
                    scrutinee: Box::new(lower_expr(scrutinee, callable, locals)),
                    arms: lowered_arms,
                },
                ty,
                span: Span::DUMMY,
            }
        }
    }
}
