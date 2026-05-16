use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::cmp::Ordering;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Priority {
    High = 3,
    Normal = 2,
    Background = 1,
}

pub type TextChange = (usize, usize, String);

#[derive(Debug, Clone)]
pub struct ParseJob {
    pub buffer_id: u64,
    pub priority: Priority,
    pub changes: Vec<TextChange>,
    pub enqueued: Instant,
}

impl PartialOrd for ParseJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ParseJob {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.priority.clone() as u8).cmp(&(other.priority.clone() as u8))
    }
}

impl PartialEq for ParseJob {
    fn eq(&self, other: &Self) -> bool {
        self.buffer_id == other.buffer_id
    }
}

impl Eq for ParseJob {}

pub struct Scheduler {
    heap: BinaryHeap<ParseJob>,
    job_tx: mpsc::Sender<ParseJob>,
    pending: HashMap<u64, (ParseJob, Instant)>,
}

impl Scheduler {
    const DEBOUNCE_MS: u64 = 150;

    pub fn new(job_tx: mpsc::Sender<ParseJob>) -> Self {
        Self {
            heap: BinaryHeap::new(),
            job_tx,
            pending: HashMap::new(),
        }
    }

    pub fn push_edit(&mut self, job: ParseJob) {
        let now = Instant::now();
        if let Some((existing, _)) = self.pending.get_mut(&job.buffer_id) {
            existing.changes.extend(job.changes);
        } else {
            self.pending.insert(job.buffer_id, (job, now));
        }
    }

    pub async fn tick(&mut self) {
        let deadline = Duration::from_millis(Self::DEBOUNCE_MS);
        let now = Instant::now();

        let expired: Vec<u64> = self.pending
            .iter()
            .filter(|(_, (_, t))| now.duration_since(*t) >= deadline)
            .map(|(id, _)| *id)
            .collect();

        for id in expired {
            if let Some((mut job, _)) = self.pending.remove(&id) {
                job.enqueued = now;
                self.heap.push(job);
            }
        }

        while let Some(job) = self.heap.pop() {
            if self.job_tx.send(job).await.is_err() {
                break;
            }
        }

        sleep(Duration::from_millis(10)).await;
    }
}

pub async fn scheduler_loop(
    mut schedule_rx: mpsc::UnboundedReceiver<ParseJob>,
    job_tx: mpsc::Sender<ParseJob>,
) {
    let mut scheduler = Scheduler::new(job_tx);

    loop {
        tokio::select! {
            Some(job) = schedule_rx.recv() => {
                scheduler.push_edit(job);
            }
            _ = scheduler.tick() => {}
        }
    }
}
