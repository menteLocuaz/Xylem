mod parser;
mod runtime;
mod editor;
mod features;
mod logger;

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::sync::OnceLock;
use parking_lot::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, stdin, stdout};
use tokio::sync::mpsc;

use runtime::state::RuntimeState;
use runtime::sync::{SyncResult};
use editor::events::EditorEvent;
use editor::messages::{XylemMessage, ServerCommand};

static STDOUT_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

async fn send_json(msg: &serde_json::Value) {
    let guard = STDOUT_MUTEX.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let body = serde_json::to_string(msg).unwrap_or_default();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut stdout = stdout();
    let _ = stdout.write_all(header.as_bytes()).await;
    let _ = stdout.write_all(body.as_bytes()).await;
    let _ = stdout.flush().await;
    drop(guard);
}

fn build_result(id: &str, lang: &str, result: &SyncResult) -> serde_json::Value {
    serde_json::json!({
        "method": "xylem.sync.result",
        "id": id,
        "params": {
            "lang": lang,
            "success": result.success,
            "path": result.path,
            "message": result.message,
        }
    })
}

fn build_complete(id: &str, total: u32, failed_langs: &[String]) -> serde_json::Value {
    serde_json::json!({
        "method": "xylem.sync.complete",
        "id": id,
        "params": {
            "total": total,
            "failed": failed_langs.len(),
            "failed_langs": failed_langs,
        }
    })
}

struct XylemServer {
    tx: mpsc::Sender<ServerCommand>,
    running: Arc<AtomicBool>,
}

impl XylemServer {
    fn new(tx: mpsc::Sender<ServerCommand>) -> Self {
        Self {
            tx,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    async fn process_stdin(&self) -> anyhow::Result<()> {
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

    async fn handle_message(&self, msg: &serde_json::Value) -> anyhow::Result<()> {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params");

        match method {
            "xylem.change" => {
                if let Some(p) = params {
                    let text = p.get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Change {
                        buffer_id: 0,
                        text: text.to_string(),
                    })).await;
                }
            }
            _ => {}
        }
        Ok(())
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

    fn shutdown(&self) {
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
        let (tx, mut rx) = mpsc::channel::<ServerCommand>(100);
        let runtime = Arc::new(RwLock::new(RuntimeState::new()));
        let runtime_clone = runtime.clone();
        
        // Spawn command processor
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    ServerCommand::UpdateState(msg) => {
                        if let XylemMessage::Change { buffer_id, text } = msg {
                            let event = EditorEvent::Reload {
                                buffer_id,
                                text,
                            };
                            runtime_clone.write().apply_change(&event);
                        }
                    }
                    ServerCommand::Shutdown => break,
                }
            }
        });

        let server = XylemServer::new(tx);
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
