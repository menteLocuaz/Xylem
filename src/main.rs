mod parser;
mod runtime;
mod editor;
mod features;

use std::io::{BufRead, stdin};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use parking_lot::RwLock;

use runtime::state::RuntimeState;
use editor::events::EditorEvent;

struct XylemServer {
    runtime: Arc<RwLock<RuntimeState>>,
    running: Arc<AtomicBool>,
}

impl XylemServer {
    fn new() -> Self {
        Self {
            runtime: Arc::new(RwLock::new(RuntimeState::new())),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    fn process_stdin(&mut self) {
        let stdin = stdin();
        let mut handle = stdin.lock();
        let mut buffer = String::new();

        while self.running.load(Ordering::SeqCst) {
            buffer.clear();
            match handle.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&buffer) {
                        self.handle_message(&msg);
                    }
                }
                Err(_) => break,
            }
        }
    }

    fn handle_message(&mut self, msg: &serde_json::Value) {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params");

        match method {
            "xylem.attach" => {
                if let Some(p) = params {
                    let buffer_id = p.get("buffer_id")
                        .and_then(|b| b.as_u64())
                        .unwrap_or(0);
                    self.runtime.write().set_buffer_id(buffer_id);
                    println!("Attached to buffer {}", buffer_id);
                }
            }
            "xylem.detach" => {
                println!("Detached from buffer");
            }
            "xylem.change" => {
                if let Some(p) = params {
                    let text = p.get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    let event = EditorEvent::Reload {
                        buffer_id: 0,
                        text: text.to_string(),
                    };
                    self.runtime.write().apply_change(&event);

                    let highlights = self.runtime.read().get_highlights();
                    self.send_highlights(0, highlights);
                }
            }
            "xylem.parse" => {
                let ast = self.get_ast();
                self.send_response("xylem.ast", ast);
            }
            _ => {}
        }
    }

    fn get_ast(&self) -> String {
        let runtime = self.runtime.read();
        if let Some(state) = runtime.buffers.get(&runtime.current_buffer_id) {
            if let Some(root) = state.parser.root_node() {
                root.to_sexp()
            } else {
                "null".to_string()
            }
        } else {
            "null".to_string()
        }
    }

    fn send_highlights(&self, buffer_id: u64, highlights: Vec<runtime::state::HighlightRange>) {
        let hl_json: Vec<serde_json::Value> = highlights.iter().map(|h| {
            serde_json::json!({
                "start_byte": h.start_byte,
                "end_byte": h.end_byte,
                "hl_group": h.highlight,
            })
        }).collect();

        let response = serde_json::json!({
            "method": "xylem.highlights",
            "params": {
                "buffer_id": buffer_id,
                "highlights": hl_json,
            }
        });

        println!("{}", response);
    }

    fn send_response(&self, method: &str, result: String) {
        let response = serde_json::json!({
            "method": method,
            "result": result,
        });
        println!("{}", response);
    }

    fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_rpc = args.contains(&"--rpc".to_string());

    if is_rpc {
        let mut server = XylemServer::new();
        server.process_stdin();
        server.shutdown();
    } else {
        println!("xylem v0.1.0 - incremental parser for Neovim 0.11+");
        println!("Usage: xylem --rpc");

        let runtime = Arc::new(RwLock::new(RuntimeState::new()));
        runtime.write().set_text(
            r#"
local x = 10

function hello()
    print(x)
end
            "#
        );

        let ast = {
            let runtime = runtime.read();
            if let Some(state) = runtime.buffers.get(&runtime.current_buffer_id) {
                if let Some(root) = state.parser.root_node() {
                    root.to_sexp()
                } else {
                    "null".to_string()
                }
            } else {
                "null".to_string()
            }
        };
        println!("AST:\n{}", ast);

        let highlights = runtime.read().get_highlights();
        println!("\nHighlights:");
        for hl in highlights {
            println!("  {}: {}..{}", hl.highlight, hl.start_byte, hl.end_byte);
        }
    }
}