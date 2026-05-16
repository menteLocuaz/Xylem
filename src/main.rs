use xylem::runtime::state::RuntimeState;
use xylem::editor::messages::{ServerCommand, XylemMessage};
use xylem::editor::rpc_server::XylemServer;
use xylem::runtime::scheduler::{ParseJob, Priority, scheduler_loop};
use xylem::runtime::workers::{parse_worker_loop, ParseResult};
use xylem::features::highlight::HighlightDelta;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;
use tokio::sync::mpsc;

static MSG_ID: AtomicU64 = AtomicU64::new(1);

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

        let state = Arc::new(RwLock::new(RuntimeState::new()));

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
                    StdoutMessage::HighlightDelta { buffer_id, version, deltas } => {
                        let _ = send_highlight_delta_to_neovim(buffer_id, version, deltas);
                    }
                }
            }
        });

        tokio::spawn(dispatch_loop(
            cmd_rx,
            state.clone(),
            schedule_tx,
            stdout_tx,
        ));

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

async fn dispatch_loop(
    mut cmd_rx: mpsc::Receiver<ServerCommand>,
    state: Arc<RwLock<RuntimeState>>,
    schedule_tx: mpsc::UnboundedSender<ParseJob>,
    stdout_tx: mpsc::UnboundedSender<StdoutMessage>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            ServerCommand::UpdateState(XylemMessage::Attach { buffer_id }) => {
                state.write().set_buffer_id(buffer_id);
            }
            ServerCommand::UpdateState(XylemMessage::Detach { buffer_id }) => {
                let _ = state.write().buffers.remove(&buffer_id);
            }
            ServerCommand::UpdateState(_) => {}
            ServerCommand::UpdateStateWithReply { event, buffer_id } => {
                if let xylem::editor::events::EditorEvent::Change {
                    start_byte,
                    end_byte,
                    text,
                    ..
                } = event
                {
                    let _ = schedule_tx.send(ParseJob {
                        buffer_id,
                        priority: Priority::Normal,
                        changes: vec![(start_byte, end_byte, text)],
                        enqueued: tokio::time::Instant::now(),
                    });
                }
            }
            ServerCommand::SendDelta { buffer_id, version, deltas } => {
                let _ = stdout_tx.send(StdoutMessage::HighlightDelta { buffer_id, version, deltas });
            }
            ServerCommand::Reply { .. } | ServerCommand::Shutdown => {}
        }
    }
}

enum StdoutMessage {
    HighlightDelta {
        buffer_id: u64,
        version: u64,
        deltas: Vec<HighlightDelta>,
    },
}

fn rmpv_uint(n: u64) -> rmpv::Value {
    rmpv::Value::Integer(rmpv::Integer::from(n))
}

fn rmpv_str(s: &str) -> rmpv::Value {
    rmpv::Value::String(rmpv::Utf8String::from(s))
}

fn deltas_to_rmpv(deltas: &[HighlightDelta]) -> rmpv::Value {
    rmpv::Value::Array(deltas.iter().map(|d| {
        rmpv::Value::Map(vec![
            (rmpv_str("line"), rmpv_uint(d.line as u64)),
            (rmpv_str("captures"), rmpv::Value::Array(
                d.captures.iter().map(|c| {
                    rmpv::Value::Map(vec![
                        (rmpv_str("start_col"), rmpv_uint(c.start_col as u64)),
                        (rmpv_str("end_col"), rmpv_uint(c.end_col as u64)),
                        (rmpv_str("hl_group"), rmpv::Value::String(rmpv::Utf8String::from(c.hl_group.clone()))),
                    ])
                }).collect()
            )),
        ])
    }).collect())
}

fn send_highlight_delta_to_neovim(
    buffer_id: u64,
    version: u64,
    deltas: Vec<HighlightDelta>,
) -> anyhow::Result<()> {
    let deltas_value = deltas_to_rmpv(&deltas);

    let params = rmpv::Value::Map(vec![
        (rmpv_str("buffer_id"), rmpv_uint(buffer_id)),
        (rmpv_str("version"), rmpv_uint(version)),
        (rmpv_str("deltas"), deltas_value),
    ]);

    call_neovim_lua(
        "require('xylem').apply_highlight_delta(select(1, ...))",
        rmpv::Value::Array(vec![params]),
    )
}

fn call_neovim_lua(code: &str, args: rmpv::Value) -> anyhow::Result<()> {
    use std::io::Write;

    let msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);

    let msg = rmpv::Value::Array(vec![
        rmpv_uint(0),
        rmpv_uint(msgid),
        rmpv_str("nvim_exec_lua"),
        rmpv::Value::Array(vec![
            rmpv::Value::String(rmpv::Utf8String::from(code)),
            args,
        ]),
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &msg)?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&buf)?;
    stdout.flush()?;
    Ok(())
}
