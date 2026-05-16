use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task;

use crate::features::highlight::HighlightDelta;
use crate::runtime::scheduler::ParseJob;
use crate::runtime::state::RuntimeState;

pub struct ParseResult {
    pub buffer_id: u64,
    pub deltas: Vec<HighlightDelta>,
}

pub async fn parse_worker_loop(
    mut job_rx: mpsc::Receiver<ParseJob>,
    state: Arc<RuntimeState>,
    result_tx: mpsc::UnboundedSender<ParseResult>,
) {
    while let Some(job) = job_rx.recv().await {
        let state = state.clone();
        let tx = result_tx.clone();
        let buffer_id = job.buffer_id;
        let changes = job.changes;

        task::spawn_blocking(move || {
            let deltas = if changes.is_empty() {
                state.full_parse(buffer_id)
            } else {
                state.apply_changes_and_parse(buffer_id, &changes)
            };

            let _ = tx.send(ParseResult { buffer_id, deltas });
        });
    }
}
