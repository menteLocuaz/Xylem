use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::VecDeque;

pub struct Scheduler {
    tasks: Arc<RwLock<VecDeque<ScheduledTask>>>,
    shutdown: Arc<RwLock<bool>>,
}

pub struct ScheduledTask {
    pub id: u64,
    pub priority: u8,
    pub payload: String,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(VecDeque::new())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    pub fn schedule(&self, task: ScheduledTask) {
        let mut queue = self.tasks.write();
        let pos = queue.iter().position(|t| t.priority < task.priority)
            .unwrap_or(queue.len());
        queue.insert(pos, task);
    }

    pub fn poll(&self) -> Option<ScheduledTask> {
        self.tasks.write().pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.read().is_empty()
    }

    pub fn shutdown(&self) {
        *self.shutdown.write() = true;
        self.tasks.write().clear();
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}