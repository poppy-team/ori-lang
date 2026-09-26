use ori_bridge_server::{BridgeServer, RequestEnvelope, ResponseEnvelope, CURRENT_PROTOCOL_VERSION};
use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args[1] != "--request-file" {
        eprintln!("Usage: ori-bridge-server --request-file <path>");
        exit(1);
    }

    let req_path = &args[2];
    let payload_str = match fs::read_to_string(req_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ori-bridge-server: failed to read {}: {}", req_path, e);
            exit(1);
        }
    };

    let envelope = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 1,
        command: "compile_module".to_string(),
        payload: match serde_json::from_str(&payload_str) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("ori-bridge-server: invalid JSON in {}: {}", req_path, e);
                exit(1);
            }
        },
    };

    let server = BridgeServer::new();
    let res: ResponseEnvelope = server.handle_request(envelope);

    if res.status != "ok" {
        if let Some(err) = res.error {
            eprintln!("ori-bridge-server error [{}]: {}", err.code, err.message);
        } else {
            eprintln!("ori-bridge-server: unknown compilation error");
        }
        exit(1);
    }
}
