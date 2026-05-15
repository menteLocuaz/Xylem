use xylem::runtime::state::RuntimeState;
use xylem::editor::events::EditorEvent;
use xylem::editor::messages::{XylemMessage, ServerCommand};
use xylem::editor::rpc_server::XylemServer;

use std::io::Write;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;

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
