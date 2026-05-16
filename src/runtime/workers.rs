use tokio::sync::mpsc;
use tokio::task;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::runtime::scheduler::ParseJob;
use crate::runtime::state::RuntimeState;
use crate::features::highlight::HighlightDelta;

pub struct ParseResult {
    pub buffer_id: u64,
    pub deltas: Vec<HighlightDelta>,
}

pub async fn parse_worker_loop(
    mut job_rx: mpsc::Receiver<ParseJob>,
    state: Arc<RwLock<RuntimeState>>,
    result_tx: mpsc::UnboundedSender<ParseResult>,
) {
    while let Some(job) = job_rx.recv().await {
        let state = state.clone();
        let tx = result_tx.clone();
        let buffer_id = job.buffer_id;
        let changes = job.changes;

        task::spawn_blocking(move || {
            let mut s = state.write();

            let deltas = if changes.is_empty() {
                s.full_parse(buffer_id)
            } else {
                s.apply_changes_and_parse(buffer_id, &changes)
            };

            let _ = tx.send(ParseResult {
                buffer_id,
                deltas,
            });
        });
    }
}
