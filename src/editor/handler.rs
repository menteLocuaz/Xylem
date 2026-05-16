use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use crate::runtime::state::RuntimeState;
use crate::editor::messages::{ServerCommand, XylemMessage};
use crate::runtime::scheduler::{ParseJob, Priority};
use crate::features::highlight::HighlightDelta;

static MSG_ID: AtomicU64 = AtomicU64::new(1);

pub enum StdoutMessage {
    HighlightDelta {
        buffer_id: u64,
        version: u64,
        deltas: Vec<HighlightDelta>,
    },
}

pub async fn dispatch_loop(
    mut cmd_rx: mpsc::Receiver<ServerCommand>,
    state: Arc<RuntimeState>,
    schedule_tx: mpsc::UnboundedSender<ParseJob>,
    stdout_tx: mpsc::UnboundedSender<StdoutMessage>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            ServerCommand::UpdateState(XylemMessage::Attach { buffer_id }) => {
                state.set_buffer_id(buffer_id);
            }
            ServerCommand::UpdateState(XylemMessage::Detach { buffer_id }) => {
                let _ = state.buffers.remove(&buffer_id);
            }
            ServerCommand::UpdateState(_) => {}
            ServerCommand::UpdateStateWithReply { event, buffer_id } => {
                if let crate::editor::events::EditorEvent::Change {
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

pub fn send_highlight_delta_to_neovim(
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

fn deltas_to_rmpv(deltas: &[HighlightDelta]) -> rmpv::Value {
    rmpv::Value::Array(deltas.iter().map(|d| {
        rmpv::Value::Map(vec![
            (rmpv_str("line"), rmpv_uint(d.line as u64)),
            (rmpv_str("captures"), rmpv::Value::Array(
                d.captures.iter().map(|c| {
                    rmpv::Value::Map(vec![
                        (rmpv_str("start_col"), rmpv_uint(c.start_col as u64)),
                        (rmpv_str("end_col"), rmpv_uint(c.end_col as u64)),
                        (rmpv_str("hl_group"), rmpv::Value::String(rmpv::Utf8String::from(c.hl_group.to_string()))),
                    ])
                }).collect()
            )),
        ])
    }).collect())
}

fn rmpv_uint(n: u64) -> rmpv::Value {
    rmpv::Value::Integer(rmpv::Integer::from(n))
}

fn rmpv_str(s: &str) -> rmpv::Value {
    rmpv::Value::String(rmpv::Utf8String::from(s))
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
