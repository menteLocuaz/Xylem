use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum RpcRequest {
    #[serde(rename = "xylem.change")]
    Change {
        buffer_id: u64,
        start_byte: usize,
        old_end_byte: usize,
        new_text: String,
    },
    #[serde(rename = "xylem.attach")]
    Attach { buffer_id: u64 },
    #[serde(rename = "xylem.detach")]
    Detach { buffer_id: u64 },
    #[serde(rename = "xylem.parse")]
    Parse { buffer_id: u64 },
    #[serde(rename = "xylem.install")]
    Install {
        name: String,
        repo: String,
        revision: String,
        queries: Vec<String>,
    },
}

#[derive(Debug)]
pub enum MsgpackRpcIn {
    Request { msgid: u64, method: String, params: rmpv::Value },
    Response { msgid: u64, error: Option<String>, result: Option<rmpv::Value> },
    Notification { method: String, params: rmpv::Value },
}

impl MsgpackRpcIn {
    pub fn from_value(value: rmpv::Value) -> anyhow::Result<Self> {
        let arr = value.as_array().ok_or_else(|| anyhow::anyhow!("expected msgpack array"))?;

        let msg_type = arr.first()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("expected message type byte"))?;

        match msg_type {
            0 => {
                let msgid = arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0);
                let method = arr.get(2)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("expected method string"))?
                    .to_string();
                let params = arr.get(3).cloned().unwrap_or(rmpv::Value::Nil);
                Ok(MsgpackRpcIn::Request { msgid, method, params })
            }
            1 => {
                let msgid = arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0);
                let error = arr.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());
                let result = arr.get(3).cloned();
                Ok(MsgpackRpcIn::Response { msgid, error, result })
            }
            2 => {
                let method = arr.get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("expected method string"))?
                    .to_string();
                let params = arr.get(2).cloned().unwrap_or(rmpv::Value::Nil);
                Ok(MsgpackRpcIn::Notification { method, params })
            }
            t => Err(anyhow::anyhow!("unknown msgpack-rpc message type: {}", t)),
        }
    }

    pub fn into_rpc_request(self) -> anyhow::Result<RpcRequest> {
        let (method, params) = match self {
            MsgpackRpcIn::Request { method, params, .. } => (method, params),
            MsgpackRpcIn::Notification { method, params } => (method, params),
            MsgpackRpcIn::Response { .. } => return Err(anyhow::anyhow!("response cannot become RpcRequest")),
        };

        let params_bytes = rmp_serde::to_vec(&params)?;
        let request: RpcRequest = match method.as_str() {
            "xylem.change" => {
                #[derive(Deserialize)]
                struct P { buffer_id: u64, start_byte: usize, old_end_byte: usize, new_text: String }
                let p: P = rmp_serde::from_slice(&params_bytes)?;
                RpcRequest::Change { buffer_id: p.buffer_id, start_byte: p.start_byte, old_end_byte: p.old_end_byte, new_text: p.new_text }
            }
            "xylem.attach" => {
                #[derive(Deserialize)]
                struct P { buffer_id: u64 }
                let p: P = rmp_serde::from_slice(&params_bytes)?;
                RpcRequest::Attach { buffer_id: p.buffer_id }
            }
            "xylem.detach" => {
                #[derive(Deserialize)]
                struct P { buffer_id: u64 }
                let p: P = rmp_serde::from_slice(&params_bytes)?;
                RpcRequest::Detach { buffer_id: p.buffer_id }
            }
            "xylem.parse" => {
                #[derive(Deserialize)]
                struct P { buffer_id: u64 }
                let p: P = rmp_serde::from_slice(&params_bytes)?;
                RpcRequest::Parse { buffer_id: p.buffer_id }
            }
            "xylem.install" => {
                #[derive(Deserialize)]
                struct P { name: String, repo: String, revision: String, queries: Vec<String> }
                let p: P = rmp_serde::from_slice(&params_bytes)?;
                RpcRequest::Install { name: p.name, repo: p.repo, revision: p.revision, queries: p.queries }
            }
            other => return Err(anyhow::anyhow!("unknown method: {}", other)),
        };

        Ok(request)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum XylemMessage {
    Attach { buffer_id: u64 },
    Detach { buffer_id: u64 },
    Change { buffer_id: u64, text: String },
    Parse { buffer_id: u64 },
}

#[derive(Debug)]
pub enum ServerCommand {
    UpdateState(XylemMessage),
    UpdateStateWithReply {
        event: crate::editor::events::EditorEvent,
        buffer_id: u64,
    },
    SendDelta {
        buffer_id: u64,
        version: u64,
        deltas: Vec<crate::features::highlight::HighlightDelta>,
    },
    Reply {
        buffer_id: u64,
        deltas: Option<Vec<crate::features::highlight::HighlightDelta>>,
    },
    Shutdown,
}
