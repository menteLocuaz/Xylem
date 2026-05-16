use xylem::editor::handler::{StdoutMessage, dispatch_loop, send_highlight_delta_to_neovim};
use xylem::editor::messages::ServerCommand;
use xylem::editor::rpc_server::XylemServer;
use xylem::runtime::scheduler::{ParseJob, scheduler_loop};
use xylem::runtime::state::RuntimeState;
use xylem::runtime::workers::{ParseResult, parse_worker_loop};

use std::sync::Arc;
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
        let (cmd_tx, cmd_rx) = mpsc::channel::<ServerCommand>(100);
        let (job_tx, job_rx) = mpsc::channel::<ParseJob>(32);
        let (result_tx, mut result_rx) = mpsc::unbounded_channel::<ParseResult>();
        let (schedule_tx, schedule_rx) = mpsc::unbounded_channel::<ParseJob>();
        let (stdout_tx, mut stdout_rx) = mpsc::unbounded_channel::<StdoutMessage>();

        let state = Arc::new(RuntimeState::new());

        let state_for_workers = state.clone();
        tokio::spawn(async move {
            parse_worker_loop(job_rx, state_for_workers, result_tx).await;
        });

        tokio::spawn(async move {
            scheduler_loop(schedule_rx, job_tx).await;
        });

        let stdout_tx_for_results = stdout_tx.clone();
        tokio::spawn(async move {
            while let Some(result) = result_rx.recv().await {
                let _ = stdout_tx_for_results.send(StdoutMessage::HighlightDelta {
                    buffer_id: result.buffer_id,
                    version: 0,
                    deltas: result.deltas,
                });
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = stdout_rx.recv().await {
                match msg {
                    StdoutMessage::HighlightDelta {
                        buffer_id,
                        version,
                        deltas,
                    } => {
                        let _ = send_highlight_delta_to_neovim(buffer_id, version, deltas);
                    }
                }
            }
        });

        tokio::spawn(dispatch_loop(cmd_rx, state.clone(), schedule_tx, stdout_tx));
        let server = XylemServer::new(cmd_tx);
        server.process_stdin().await?;
        server.shutdown();
    } else {
        println!("xylem v0.1.0 - incremental parser for Neovim 0.11+");
        println!("Usage: xylem --rpc | xylem --sync");

        let runtime = Arc::new(RuntimeState::new());
        runtime.set_text(
            r#"
local x = 10

function hello()
    print(x)
end
            "#,
        );

        let ast = {
            let id = *runtime.current_buffer_id.read();
            if let Some(state) = runtime.buffers.get(&id) {
                let state_guard = state.read();
                if let Some(root) = state_guard.parser.root_node() {
                    root.to_sexp()
                } else {
                    "null".to_string()
                }
            } else {
                "null".to_string()
            }
        };
        println!("AST:\n{}", ast);

        let highlights = runtime.get_highlights();
        println!("\nHighlights:");
        for hl in highlights {
            println!("  {}: {}..{}", hl.highlight, hl.start_byte, hl.end_byte);
        }
    }
    Ok(())
}
