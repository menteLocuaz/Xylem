use std::io::Write;
use std::process::{ChildStdin, Stdio};
use std::process::Command;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::runtime::state::{RuntimeState, HighlightRange};
use crate::editor::events::{EditorEvent, HighlightUpdate, HighlightDef};

pub struct RpcHandler {
    child_stdin: Arc<RwLock<Option<ChildStdin>>>,
    runtime: Arc<RwLock<RuntimeState>>,
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

        let runtime = Arc::new(RwLock::new(RuntimeState::new()));
        let is_running = Arc::new(RwLock::new(true));

        Self {
            child_stdin: Arc::new(RwLock::new(Some(child_stdin))),
            runtime,
            running: is_running,
        }
    }

    pub fn send_notification(&self, method: &str, args: String) -> Result<(), String> {
        let mut stdin = self.child_stdin.write();
        if let Some(ref mut s) = *stdin {
            let msg = format!("notification:{}:{}\n", method, args);
            s.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
            s.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
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
        self.send_notification("xylem_highlights", json)
    }

    pub fn process_event(&self, event: EditorEvent) -> bool {
        self.runtime.write().apply_change(&event)
    }

    pub fn get_highlights(&self) -> Vec<HighlightRange> {
        self.runtime.read().get_highlights()
    }

    pub fn set_text(&self, text: &str) {
        self.runtime.write().set_text(text);
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
        self.handler.process_event(event);
    }

    pub fn get_highlights(&self, buffer_id: u64) {
        let highlights = self.handler.get_highlights();
        let _ = self.handler.send_highlights(buffer_id, highlights);
    }
}