mod parser;
mod runtime;
mod editor;
mod features;

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use parking_lot::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, stdin, stdout};

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

    async fn process_stdin(&mut self) -> anyhow::Result<()> {
        let mut stdin = BufReader::new(stdin());
        
        while self.running.load(Ordering::SeqCst) {
            let mut header = String::new();
            if stdin.read_line(&mut header).await? == 0 { break; }
            
            if header.starts_with("Content-Length: ") {
                let len: usize = header["Content-Length: ".len()..].trim().parse()?;
                
                // Read until the double newline
                let mut next_line = String::new();
                stdin.read_line(&mut next_line).await?; // Should be \r\n
                
                let mut body = vec![0u8; len];
                stdin.read_exact(&mut body).await?;
                
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&body) {
                    self.handle_message(&msg).await?;
                }
            } else if !header.trim().is_empty() {
                // Fallback for line-based JSON if no Content-Length header
                if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&header) {
                    self.handle_message(&msg).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_message(&mut self, msg: &serde_json::Value) -> anyhow::Result<()> {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params");

        match method {
            "xylem.attach" => {
                if let Some(p) = params {
                    let buffer_id = p.get("buffer_id")
                        .and_then(|b| b.as_u64())
                        .unwrap_or(0);
                    self.runtime.write().set_buffer_id(buffer_id);
                }
            }
            "xylem.detach" => {}
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
                    self.send_highlights(0, highlights).await?;
                }
            }
            "xylem.parse" => {
                let ast = self.get_ast();
                self.send_response("xylem.ast", ast).await?;
            }
            _ => {}
        }
        Ok(())
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

    async fn send_highlights(&self, buffer_id: u64, highlights: Vec<runtime::state::HighlightRange>) -> anyhow::Result<()> {
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

        self.send_message(&response).await
    }

    async fn send_response(&self, method: &str, result: String) -> anyhow::Result<()> {
        let response = serde_json::json!({
            "method": method,
            "result": result,
        });
        self.send_message(&response).await
    }

    async fn send_message(&self, msg: &serde_json::Value) -> anyhow::Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut stdout = stdout();
        stdout.write_all(header.as_bytes()).await?;
        stdout.write_all(body.as_bytes()).await?;
        stdout.flush().await?;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let is_rpc = args.contains(&"--rpc".to_string());
    let is_sync = args.contains(&"--sync".to_string());

    if is_sync {
        let manager = runtime::sync::SyncManager::new()?;
        manager.sync_all().await?;
        return Ok(());
    }

    if is_rpc {
        let mut server = XylemServer::new();
        server.process_stdin().await?;
        server.shutdown();
    } else {
        println!("xylem v0.1.0 - incremental parser for Neovim 0.11+");
        println!("Usage: xylem --rpc | xylem --sync");

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
    Ok(())
}
