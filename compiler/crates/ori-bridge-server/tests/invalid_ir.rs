use ori_bridge_server::{
    BridgeServer, CompileModuleRequest, RequestEnvelope, SerializedExpr, SerializedFunc,
    SerializedModule, SerializedStmt, SerializedTy, CURRENT_PROTOCOL_VERSION,
};

fn compile_with_expr(expr: SerializedExpr) -> (ori_bridge_server::ResponseEnvelope, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 17,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(CompileModuleRequest {
            module: SerializedModule {
                namespace: String::new(),
                funcs: vec![SerializedFunc {
                    name: "main".to_string(),
                    params: vec![],
                    return_ty: SerializedTy::Void,
                    body_stmts: vec![SerializedStmt::Expr(expr)],
                    is_public: true,
                }],
            },
            output_path: dir.path().join("invalid.o").to_string_lossy().into_owned(),
            lib_mode: false,
        })
        .unwrap(),
    };
    (BridgeServer::new().handle_request(req), dir)
}

#[test]
fn unknown_callee_is_rejected_before_writing_an_object() {
    let (res, dir) = compile_with_expr(SerializedExpr::Call {
        callee: "missing_function".to_string(),
        args: vec![],
    });
    assert_eq!(res.status, "error");
    assert_eq!(res.request_id, 17);
    assert_eq!(res.error.unwrap().code, "bridge.unsupported_ir");
    assert!(!dir.path().join("invalid.o").exists());
}

#[test]
fn local_call_with_wrong_arity_is_rejected_before_writing_an_object() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("invalid.o");
    let module = SerializedModule {
        namespace: "test.calls".to_string(),
        funcs: vec![
            SerializedFunc {
                name: "main".to_string(),
                params: vec![],
                return_ty: SerializedTy::Int,
                body_stmts: vec![SerializedStmt::Return(Some(SerializedExpr::Call {
                    callee: "answer".to_string(),
                    args: vec![SerializedExpr::IntLit(3)],
                }))],
                is_public: true,
            },
            SerializedFunc {
                name: "answer".to_string(),
                params: vec![],
                return_ty: SerializedTy::Int,
                body_stmts: vec![SerializedStmt::Return(Some(SerializedExpr::IntLit(42)))],
                is_public: false,
            },
        ],
    };
    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 18,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(CompileModuleRequest {
            module,
            output_path: output.to_string_lossy().into_owned(),
            lib_mode: false,
        })
        .unwrap(),
    };
    let res = BridgeServer::new().handle_request(req);
    assert_eq!(res.error.unwrap().code, "bridge.unsupported_ir");
    assert!(!output.exists());
}

#[test]
fn print_of_an_integer_is_rejected_instead_of_forging_a_string_type() {
    let (res, dir) = compile_with_expr(SerializedExpr::Call {
        callee: "println".to_string(),
        args: vec![SerializedExpr::IntLit(42)],
    });
    assert_eq!(res.status, "error");
    assert_eq!(res.error.unwrap().code, "bridge.unsupported_ir");
    assert!(!dir.path().join("invalid.o").exists());
}

#[test]
fn undefined_variable_is_rejected_before_codegen() {
    let (res, dir) = compile_with_expr(SerializedExpr::Var("not_declared".to_string()));
    assert_eq!(res.status, "error");
    assert!(res.error.unwrap().message.contains("undefined variable"));
    assert!(!dir.path().join("invalid.o").exists());
}

#[test]
fn mixed_type_arithmetic_is_rejected_before_codegen() {
    let (res, dir) = compile_with_expr(SerializedExpr::Add(
        Box::new(SerializedExpr::IntLit(1)),
        Box::new(SerializedExpr::StrLit("2".to_string())),
    ));
    assert_eq!(res.status, "error");
    assert!(res.error.unwrap().message.contains("arithmetic operands"));
    assert!(!dir.path().join("invalid.o").exists());
}
