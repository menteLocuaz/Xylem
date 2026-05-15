use xylem::runtime::state::RuntimeState;
use xylem::editor::events::EditorEvent;
use xylem::editor::messages::{XylemMessage, ServerCommand};

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader, stdin};
use tokio::sync::mpsc;

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

                let mut next_line = String::new();
                stdin.read_line(&mut next_line).await?;

                let mut body = vec![0u8; len];
                stdin.read_exact(&mut body).await?;

                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&body) {
                    self.handle_message(&msg).await?;
                }
            } else if !header.trim().is_empty() {
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
                    let buffer_id = p.get("buffer_id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let start_byte = p.get("start_byte").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let old_end_byte = p.get("old_end_byte").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let new_text = p.get("new_text").and_then(|t| t.as_str()).unwrap_or("");

                    let event = EditorEvent::Change {
                        buffer_id,
                        start_byte,
                        end_byte: old_end_byte,
                        text: new_text.to_string(),
                    };

                    let _ = self.tx.send(ServerCommand::UpdateStateWithReply {
                        event,
                        buffer_id,
                    }).await;
                }
            }
            "xylem.attach" => {
                if let Some(p) = params {
                    let buffer_id = p.get("buffer_id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Attach { buffer_id })).await;
                }
            }
            "xylem.detach" => {
                if let Some(p) = params {
                    let buffer_id = p.get("buffer_id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Detach { buffer_id })).await;
                }
            }
            "xylem.parse" => {
                if let Some(p) = params {
                    let buffer_id = p.get("buffer_id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Parse { buffer_id })).await;
                }
            }
            _ => {}
        }
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
        let manager = xylem::runtime::sync::SyncManager::new()?;
        manager.sync_all().await?;
        return Ok(());
    }

    if is_rpc {
        let (tx, mut rx) = mpsc::channel::<ServerCommand>(100);
        let runtime = Arc::new(RwLock::new(RuntimeState::new()));
        let runtime_clone = runtime.clone();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    ServerCommand::UpdateState(msg) => {
                        match msg {
                            XylemMessage::Change { buffer_id, text } => {
                                let event = EditorEvent::Reload { buffer_id, text };
                                runtime_clone.write().apply_change(&event);
                            }
                            XylemMessage::Attach { buffer_id } => {
                                runtime_clone.write().set_buffer_id(buffer_id);
                            }
                            _ => {}
                        }
                    }
                    ServerCommand::UpdateStateWithReply { event, buffer_id } => {
                        let deltas = runtime_clone.write().apply_change(&event);
                        let version = runtime_clone.read().buffers.get(&buffer_id).map(|b| b.version).unwrap_or(0);
                        if let Some(deltas) = deltas {
                            let _ = tx_clone.send(ServerCommand::SendDelta { buffer_id, version, deltas }).await;
                        }
                    }
                    ServerCommand::SendDelta { buffer_id, version, deltas } => {
                        let _ = send_delta_to_stdout(buffer_id, version, deltas);
                    }
                    ServerCommand::Reply { .. } => {}
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

fn send_delta_to_stdout(buffer_id: u64, version: u64, deltas: Vec<xylem::features::highlight::HighlightDelta>) -> Result<(), String> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct DeltaRpc {
        line: u32,
        captures: Vec<CaptureRpc>,
    }

    #[derive(Serialize)]
    struct CaptureRpc {
        start_col: u32,
        end_col: u32,
        hl_group: String,
    }

    let rpc_deltas: Vec<DeltaRpc> = deltas.into_iter().map(|d| {
        DeltaRpc {
            line: d.line,
            captures: d.captures.into_iter().map(|c| CaptureRpc {
                start_col: c.start_col,
                end_col: c.end_col,
                hl_group: c.hl_group,
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

    let body = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
    handle.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    handle.flush().map_err(|e| e.to_string())?;
    Ok(())
}
