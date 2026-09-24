use ori_bridge_server::{
    BridgeServer, CompileModuleRequest, RequestEnvelope, SerializedFunc, SerializedModule,
    SerializedParam, SerializedStmt, SerializedTy, CURRENT_PROTOCOL_VERSION,
};
use tempfile::NamedTempFile;

#[test]
fn test_bridge_real_codegen_round_trip() {
    let server = BridgeServer::new();
    let tmp_obj = NamedTempFile::new().expect("failed to create temp file");
    let obj_path = tmp_obj.path().to_string_lossy().to_string();

    let module = SerializedModule {
        namespace: "test.bridge.module".to_string(),
        funcs: vec![SerializedFunc {
            name: "add_two".to_string(),
            params: vec![
                SerializedParam {
                    name: "a".to_string(),
                    ty: SerializedTy::Int,
                },
                SerializedParam {
                    name: "b".to_string(),
                    ty: SerializedTy::Int,
                },
            ],
            return_ty: SerializedTy::Int,
            body_stmts: vec![SerializedStmt::Let {
                name: "x".to_string(),
                ty: SerializedTy::Int,
                value: ori_bridge_server::protocol::SerializedExpr::IntLit(42),
            }],
            is_public: true,
        }],
    };

    let compile_req = CompileModuleRequest {
        module,
        output_path: obj_path.clone(),
        lib_mode: true,
    };

    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 2001,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(compile_req).unwrap(),
    };

    let res = server.handle_request(req);
    assert_eq!(res.status, "ok", "compilation failed: {:?}", res.error);
    assert_eq!(res.request_id, 2001);

    let meta = std::fs::metadata(&obj_path).expect("failed to read emitted file metadata");
    assert!(meta.len() > 0, "emitted object file should not be empty");
}

#[test]
fn test_bridge_forward_local_call_keeps_integer_signature() {
    let server = BridgeServer::new();
    let tmp_obj = NamedTempFile::new().expect("failed to create temp file");
    let obj_path = tmp_obj.path().to_string_lossy().to_string();

    let module = SerializedModule {
        namespace: "test.bridge.calls".to_string(),
        funcs: vec![
            SerializedFunc {
                name: "main".to_string(),
                params: vec![],
                return_ty: SerializedTy::Int,
                body_stmts: vec![SerializedStmt::Return(Some(
                    ori_bridge_server::SerializedExpr::Call {
                        callee: "answer".to_string(),
                        args: vec![],
                    },
                ))],
                is_public: true,
            },
            SerializedFunc {
                name: "answer".to_string(),
                params: vec![],
                return_ty: SerializedTy::Int,
                body_stmts: vec![SerializedStmt::Return(Some(
                    ori_bridge_server::SerializedExpr::IntLit(42),
                ))],
                is_public: false,
            },
        ],
    };

    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 2002,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(CompileModuleRequest {
            module,
            output_path: obj_path.clone(),
            lib_mode: false,
        })
        .unwrap(),
    };

    let res = server.handle_request(req);
    assert_eq!(res.status, "ok", "compilation failed: {:?}", res.error);
    assert!(std::fs::metadata(&obj_path).unwrap().len() > 0);
}

#[test]
fn test_bridge_forward_local_call_keeps_integer_parameter() {
    let server = BridgeServer::new();
    let tmp_obj = NamedTempFile::new().expect("failed to create temp file");
    let obj_path = tmp_obj.path().to_string_lossy().to_string();
    let module = SerializedModule {
        namespace: "test.bridge.parameters".to_string(),
        funcs: vec![
            SerializedFunc {
                name: "main".to_string(),
                params: vec![],
                return_ty: SerializedTy::Int,
                body_stmts: vec![SerializedStmt::Return(Some(
                    ori_bridge_server::SerializedExpr::Call {
                        callee: "answer".to_string(),
                        args: vec![ori_bridge_server::SerializedExpr::IntLit(42)],
                    },
                ))],
                is_public: true,
            },
            SerializedFunc {
                name: "answer".to_string(),
                params: vec![SerializedParam {
                    name: "value".to_string(),
                    ty: SerializedTy::Int,
                }],
                return_ty: SerializedTy::Int,
                body_stmts: vec![SerializedStmt::Return(Some(
                    ori_bridge_server::SerializedExpr::Var("value".to_string()),
                ))],
                is_public: false,
            },
        ],
    };
    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 2003,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(CompileModuleRequest {
            module,
            output_path: obj_path.clone(),
            lib_mode: false,
        })
        .unwrap(),
    };
    let res = server.handle_request(req);
    assert_eq!(res.status, "ok", "compilation failed: {:?}", res.error);
    assert!(std::fs::metadata(&obj_path).unwrap().len() > 0);
}
