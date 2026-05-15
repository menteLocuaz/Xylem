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
    Shutdown,
}
