pub mod framing;
pub mod protocol;
pub mod server;

pub use framing::{read_frame, write_frame, FrameError, MAX_FRAME_SIZE, PROTOCOL_MAGIC};
pub use protocol::{
    BridgeErrorPayload, CompileModuleRequest, CompileModuleResponse, HandshakeRequest,
    HandshakeResponse, RequestEnvelope, ResponseEnvelope, SerializedBinaryOp, SerializedExpr,
    SerializedExprArm,
    SerializedFunc, SerializedMatchArm, SerializedModule, SerializedParam, SerializedPattern,
    SerializedStmt, SerializedTy,
    CURRENT_PROTOCOL_VERSION,
};
pub const RUNTIME_IO_PRINT: &str = "ori_io_print";
pub const RUNTIME_IO_EPRINT: &str = "ori_io_eprint";

pub use server::BridgeServer;
