use ori_bridge_server::{
    CompileModuleRequest, SerializedBinaryOp, SerializedExpr, SerializedFunc, SerializedModule,
    SerializedStmt, SerializedTy,
    read_frame, write_frame, BridgeServer, HandshakeRequest, RequestEnvelope, CURRENT_PROTOCOL_VERSION,
};
use std::io::Cursor;

#[test]
fn test_framing_round_trip() {
    let payload = b"{\"command\":\"handshake\"}";
    let mut buffer = Vec::new();

    write_frame(&mut buffer, payload).expect("write frame failed");

    let mut cursor = Cursor::new(buffer);
    let received = read_frame(&mut cursor).expect("read frame failed");

    assert_eq!(received, payload);
}

#[test]
fn test_bridge_handshake_flow() {
    let server = BridgeServer::new();

    let handshake_req = HandshakeRequest {
        client_version: "0.3.8".to_string(),
        supported_protocol: 1,
    };

    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 1001,
        command: "handshake".to_string(),
        payload: serde_json::to_value(handshake_req).unwrap(),
    };

    let res = server.handle_request(req);
    assert_eq!(res.status, "ok");
    assert_eq!(res.request_id, 1001);
    assert!(res.data.is_some());
    assert!(res.error.is_none());
}

#[test]
fn test_bridge_rejects_incompatible_protocol_version() {
    let server = BridgeServer::new();

    let req = RequestEnvelope {
        protocol_version: 999, // Incompatible version
        request_id: 1002,
        command: "handshake".to_string(),
        payload: serde_json::json!({}),
    };

    let res = server.handle_request(req);
    assert_eq!(res.status, "error");
    let err = res.error.expect("expected error payload");
    assert_eq!(err.code, "bridge.unsupported_version");
}

#[test]
fn integer_comparison_lowers_to_a_boolean_if_condition() {
    let dir = tempfile::tempdir().unwrap();
    let obj = dir.path().join("comparison.o");
    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 1003,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(CompileModuleRequest {
            module: SerializedModule {
                namespace: String::new(),
                funcs: vec![SerializedFunc {
                    name: "main".to_string(),
                    params: vec![],
                    return_ty: SerializedTy::Void,
                    body_stmts: vec![SerializedStmt::If {
                        cond: SerializedExpr::Binary {
                            op: SerializedBinaryOp::Lt,
                            left: Box::new(SerializedExpr::IntLit(2)),
                            right: Box::new(SerializedExpr::IntLit(3)),
                        },
                        then_stmts: vec![SerializedStmt::Return(None)],
                        else_stmts: vec![SerializedStmt::Return(None)],
                    }],
                    is_public: true,
                }],
            },
            output_path: obj.to_string_lossy().into_owned(),
            lib_mode: false,
        })
        .unwrap(),
    };
    let res = BridgeServer::new().handle_request(req);
    assert_eq!(res.status, "ok", "{:?}", res.error);
    assert!(obj.exists());
}
