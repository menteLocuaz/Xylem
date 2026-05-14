use ropey::Rope;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct Buffer {
    id: u64,
    content: Arc<RwLock<Rope>>,
    version: Arc<RwLock<u64>>,
    is_modified: Arc<RwLock<bool>>,
}

impl Buffer {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            content: Arc::new(RwLock::new(Rope::new())),
            version: Arc::new(RwLock::new(0)),
            is_modified: Arc::new(RwLock::new(false)),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn set_content(&self, text: &str) {
        *self.content.write() = Rope::from_str(text);
        *self.version.write() += 1;
        *self.is_modified.write() = true;
    }

    pub fn get_content(&self) -> String {
        self.content.read().to_string()
    }

    pub fn get_rope(&self) -> Arc<RwLock<Rope>> {
        self.content.clone()
    }

    pub fn insert(&self, pos: usize, text: &str) {
        self.content.write().insert(pos, text);
        *self.version.write() += 1;
        *self.is_modified.write() = true;
    }

    pub fn remove(&self, start: usize, end: usize) {
        self.content.write().remove(start..end);
        *self.version.write() += 1;
        *self.is_modified.write() = true;
    }

    pub fn version(&self) -> u64 {
        *self.version.read()
    }

    pub fn is_modified(&self) -> bool {
        *self.is_modified.write()
    }

    pub fn mark_saved(&self) {
        *self.is_modified.write() = false;
    }
}

pub struct BufferManager {
    buffers: Arc<RwLock<std::collections::HashMap<u64, Arc<Buffer>>>>,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            buffers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn create(&self, id: u64) -> Arc<Buffer> {
        let buffer = Arc::new(Buffer::new(id));
        self.buffers.write().insert(id, buffer.clone());
        buffer
    }

    pub fn get(&self, id: u64) -> Option<Arc<Buffer>> {
        self.buffers.read().get(&id).cloned()
    }

    pub fn remove(&self, id: u64) {
        self.buffers.write().remove(&id);
    }

    pub fn exists(&self, id: u64) -> bool {
        self.buffers.read().contains_key(&id)
    }
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}