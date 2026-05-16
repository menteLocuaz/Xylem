use std::io::{self, Write};
use std::process::{ChildStdin, Stdio};
use std::process::Command;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::runtime::state::{RuntimeState, HighlightRange};
use crate::editor::events::{EditorEvent, HighlightUpdate, HighlightDef, HighlightDeltaRpc, CaptureEntryRpc};
use crate::features::highlight::HighlightDelta;

pub struct RpcHandler {
    child_stdin: Arc<RwLock<Option<ChildStdin>>>,
    runtime: Arc<RuntimeState>,
    running: Arc<RwLock<bool>>,
}

impl RpcHandler {
    pub fn start(binary_path: &str) -> Self {
        let mut child = Command::new(binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start subprocess");

        let child_stdin = child.stdin.take().unwrap();

        let runtime = Arc::new(RuntimeState::new());
        let is_running = Arc::new(RwLock::new(true));

        Self {
            child_stdin: Arc::new(RwLock::new(Some(child_stdin))),
            runtime,
            running: is_running,
        }
    }

    pub fn send_json(&self, msg: &serde_json::Value) -> Result<(), String> {
        let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
        handle.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
        handle.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn send_notification(&self, method: &str, args: String) -> Result<(), String> {
        let msg = serde_json::json!({
            "method": method,
            "params": serde_json::from_str::<serde_json::Value>(&args).unwrap_or(serde_json::Value::String(args)),
        });
        self.send_json(&msg)
    }

    pub fn send_highlights(&self, buffer_id: u64, highlights: Vec<HighlightRange>) -> Result<(), String> {
        let hl_defs: Vec<HighlightDef> = highlights.into_iter().map(|h| HighlightDef {
            start_byte: h.start_byte,
            end_byte: h.end_byte,
            hl_group: h.highlight,
        }).collect();

        let update = HighlightUpdate {
            buffer_id,
            highlights: hl_defs,
        };

        let json = serde_json::to_string(&update).map_err(|e| e.to_string())?;
        self.send_notification("xylem.highlights", json)
    }

    pub fn send_highlight_delta(
        &self,
        buffer_id: u64,
        version: u64,
        deltas: Vec<HighlightDelta>,
    ) -> Result<(), String> {
        let rpc_deltas: Vec<HighlightDeltaRpc> = deltas.into_iter().map(|d| {
            HighlightDeltaRpc {
                line: d.line,
                captures: d.captures.into_iter().map(|c| CaptureEntryRpc {
                    start_col: c.start_col,
                    end_col: c.end_col,
                    hl_group: c.hl_group.to_string(),
                }).collect(),
            }
        }).collect();

        let msg = serde_json::json!({
            "method": "xylem.highlights.delta",
            "params": {
                "buffer_id": buffer_id,
                "version": version,
                "deltas": rpc_deltas,
            }
        });
        self.send_json(&msg)
    }

    pub fn process_event(&self, event: EditorEvent) -> Option<Vec<HighlightDelta>> {
        self.runtime.apply_change(&event)
    }

    pub fn get_highlights(&self) -> Vec<HighlightRange> {
        self.runtime.get_highlights()
    }

    pub fn set_text(&self, text: &str) {
        self.runtime.set_text(text);
    }

    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    pub fn stop(&mut self) {
        *self.running.write() = false;
        if let Some(stdin) = self.child_stdin.write().take() {
            drop(stdin);
        }
    }
}

pub struct NeovimRpc {
    pub handler: RpcHandler,
}

impl NeovimRpc {
    pub fn new(binary_path: &str) -> Self {
        Self {
            handler: RpcHandler::start(binary_path),
        }
    }

    pub fn apply_change(&self, buffer_id: u64, start_byte: usize, end_byte: usize, text: &str) {
        let event = EditorEvent::Change {
            buffer_id,
            start_byte,
            end_byte,
            text: text.to_string(),
        };
        if let Some(deltas) = self.handler.process_event(event) {
            let version = self.handler.runtime.buffers.get(&buffer_id).map(|b| b.read().version).unwrap_or(0);
            let _ = self.handler.send_highlight_delta(buffer_id, version, deltas);
        }
    }

    pub fn get_highlights(&self, buffer_id: u64) {
        let highlights = self.handler.get_highlights();
        let _ = self.handler.send_highlights(buffer_id, highlights);
    }
}
