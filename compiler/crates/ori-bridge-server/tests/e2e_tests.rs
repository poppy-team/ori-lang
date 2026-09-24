use ori_bridge_server::{
    BridgeServer, CompileModuleRequest, RequestEnvelope, SerializedFunc, SerializedModule,
    CURRENT_PROTOCOL_VERSION,
};
use ori_codegen::link;
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[test]
fn test_bridge_real_codegen_and_run_end_to_end() {
    let server = BridgeServer::new();
    let tmp_obj = NamedTempFile::new().expect("failed to create temp file");
    let obj_path = tmp_obj.path().to_string_lossy().to_string();
    let exe_path = tmp_obj.path().with_extension("exe");

    // 1. Ori frontend emits: fn main() -> int { let x: int = 40 + 2; return x }
    let module = SerializedModule {
        namespace: "".to_string(),
        funcs: vec![SerializedFunc {
            name: "main".to_string(),
            params: vec![],
            return_ty: ori_bridge_server::SerializedTy::Int,
            body_stmts: vec![
                ori_bridge_server::SerializedStmt::Let {
                    mutable: false,
                    name: "x".to_string(),
                    ty: ori_bridge_server::SerializedTy::Int,
                    value: ori_bridge_server::SerializedExpr::Add(
                        Box::new(ori_bridge_server::SerializedExpr::IntLit(40)),
                        Box::new(ori_bridge_server::SerializedExpr::IntLit(2)),
                    ),
                },
                ori_bridge_server::SerializedStmt::Return(Some(
                    ori_bridge_server::SerializedExpr::Var("x".to_string()),
                )),
            ],
            is_public: true,
        }],
    };

    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 9001,
        command: "compile_module".to_string(),
        payload: serde_json::to_value(CompileModuleRequest {
            module,
            output_path: obj_path.clone(),
            lib_mode: false,
        })
        .unwrap(),
    };

    // 2. Bridge lowers the user's function to real Cranelift IR and emits a native object
    let res = server.handle_request(req);
    assert_eq!(res.status, "ok", "bridge failed: {:?}", res.error);

    // 3. Staged runtime library
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().unwrap().parent().unwrap().parent().unwrap();
    let staged_runtime = repo_root
        .join("runtime")
        .join("x86_64-unknown-linux-gnu")
        .join("libori_runtime.a");

    let extra_libs = vec![
        staged_runtime,
        PathBuf::from("-lpthread"),
        PathBuf::from("-ldl"),
        PathBuf::from("-lm"),
        PathBuf::from("-lc"),
    ];

    link(
        &PathBuf::from(&obj_path),
        &exe_path,
        &extra_libs,
    )
    .expect("native link failed");

    // 4. Execute: the native binary runs cleanly with success exit code 0
    let output = std::process::Command::new(&exe_path)
        .output()
        .expect("run failed");
    assert!(
        output.status.success(),
        "expected success exit code 0, got {:?}",
        output.status
    );
}
