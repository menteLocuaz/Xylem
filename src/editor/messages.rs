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
