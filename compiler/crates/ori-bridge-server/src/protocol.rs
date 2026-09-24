use serde::{Deserialize, Serialize};

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestEnvelope {
    pub protocol_version: u32,
    pub request_id: u64,
    pub command: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseEnvelope {
    pub protocol_version: u32,
    pub request_id: u64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BridgeErrorPayload>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BridgeErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeRequest {
    pub client_version: String,
    pub supported_protocol: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub server_version: String,
    pub protocol_version: u32,
    pub target_triple: String,
    pub features: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum SerializedTy {
    Int,
    Float,
    Bool,
    String,
    Void,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SerializedBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SerializedExpr {
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
    Var(String),
    Binary {
        op: SerializedBinaryOp,
        left: Box<SerializedExpr>,
        right: Box<SerializedExpr>,
    },
    Add(Box<SerializedExpr>, Box<SerializedExpr>),
    Call {
        callee: String,
        args: Vec<SerializedExpr>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SerializedStmt {
    Let {
        name: String,
        ty: SerializedTy,
        value: SerializedExpr,
        #[serde(default)]
        mutable: bool,
    },
    Assign { name: String, value: SerializedExpr },
    Return(Option<SerializedExpr>),
    Expr(SerializedExpr),
    Break,
    Continue,
    If {
        cond: SerializedExpr,
        then_stmts: Vec<SerializedStmt>,
        else_stmts: Vec<SerializedStmt>,
    },
    While { cond: SerializedExpr, body_stmts: Vec<SerializedStmt> },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedParam {
    pub name: String,
    pub ty: SerializedTy,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedFunc {
    pub name: String,
    pub params: Vec<SerializedParam>,
    pub return_ty: SerializedTy,
    pub body_stmts: Vec<SerializedStmt>,
    pub is_public: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedModule {
    pub namespace: String,
    pub funcs: Vec<SerializedFunc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompileModuleRequest {
    pub module: SerializedModule,
    pub output_path: String,
    pub lib_mode: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompileModuleResponse {
    pub output_path: String,
    pub bytes_written: u64,
}
