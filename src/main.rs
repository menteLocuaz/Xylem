use xylem::runtime::state::RuntimeState;
use xylem::editor::messages::{ServerCommand, XylemMessage};
use xylem::editor::rpc_server::XylemServer;
use xylem::runtime::scheduler::{ParseJob, Priority, scheduler_loop};
use xylem::runtime::workers::{parse_worker_loop, ParseResult};

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
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<ServerCommand>(100);
        let (job_tx, job_rx) = mpsc::channel::<ParseJob>(32);
        let (result_tx, mut result_rx) = mpsc::unbounded_channel::<ParseResult>();
        let (schedule_tx, schedule_rx) = mpsc::unbounded_channel::<ParseJob>();

        let state = Arc::new(RwLock::new(RuntimeState::new()));

        let state_for_workers = state.clone();
        tokio::spawn(async move {
            parse_worker_loop(job_rx, state_for_workers, result_tx).await;
        });

        tokio::spawn(async move {
            scheduler_loop(schedule_rx, job_tx).await;
        });

        tokio::spawn(async move {
            while let Some(result) = result_rx.recv().await {
                let _ = send_delta_to_stdout(result.buffer_id, 0, result.deltas);
            }
        });

        let state_for_dispatch = state.clone();
        let schedule_tx_for_dispatch = schedule_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    ServerCommand::UpdateState(msg) => {
                        match msg {
                            XylemMessage::Attach { buffer_id } => {
                                state_for_dispatch.write().set_buffer_id(buffer_id);
                            }
                            XylemMessage::Detach { buffer_id } => {
                                let _ = state_for_dispatch.write().buffers.remove(&buffer_id);
                            }
                            _ => {}
                        }
                    }
                    ServerCommand::UpdateStateWithReply { event, buffer_id } => {
                        if let xylem::editor::events::EditorEvent::Change {
                            start_byte,
                            end_byte,
                            text,
                            ..
                        } = event
                        {
                            let _ = schedule_tx_for_dispatch.send(ParseJob {
                                buffer_id,
                                priority: Priority::Normal,
                                changes: vec![(start_byte, end_byte, text)],
                                enqueued: tokio::time::Instant::now(),
                            });
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

        let server = XylemServer::new(cmd_tx);
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
