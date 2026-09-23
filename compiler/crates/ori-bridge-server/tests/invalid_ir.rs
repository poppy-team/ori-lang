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
fn print_of_an_integer_is_rejected_instead_of_forging_a_string_type() {
    let (res, dir) = compile_with_expr(SerializedExpr::Call {
        callee: "println".to_string(),
        args: vec![SerializedExpr::IntLit(42)],
    });
    assert_eq!(res.status, "error");
    assert_eq!(res.error.unwrap().code, "bridge.unsupported_ir");
    assert!(!dir.path().join("invalid.o").exists());
}
