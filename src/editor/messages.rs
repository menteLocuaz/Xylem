use serde::{Deserialize, Serialize};

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
