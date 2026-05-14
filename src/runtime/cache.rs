use std::collections::HashMap;
use std::num::NonZeroUsize;
use parking_lot::RwLock;
use std::sync::Arc;
use lru::LruCache;
use tree_sitter::Query;

pub struct QueryCache {
    cache: Arc<RwLock<HashMap<String, Arc<Query>>>>,
    lru: Arc<RwLock<LruCache<String, Arc<Query>>>>,
    max_size: usize,
}

impl QueryCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(max_size).unwrap()
            ))),
            max_size,
        }
    }

    pub fn get(&self, key: &str) -> Option<Arc<Query>> {
        let mut lru = self.lru.write();
        lru.get(key).map(|q| (*q).clone())
    }

    pub fn insert(&self, key: String, query: Arc<Query>) {
        let mut lru = self.lru.write();
        lru.put(key, query);
    }

    pub fn clear(&self) {
        let mut lru = self.lru.write();
        lru.clear();
        self.cache.write().clear();
    }
}

pub struct BufferCache {
    cache: Arc<RwLock<HashMap<u64, CachedBuffer>>>,
}

impl BufferCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, buffer_id: u64) -> Option<CachedBuffer> {
        self.cache.read().get(&buffer_id).cloned()
    }

    pub fn insert(&self, buffer_id: u64, buffer: CachedBuffer) {
        self.cache.write().insert(buffer_id, buffer);
    }

    pub fn remove(&self, buffer_id: u64) {
        self.cache.write().remove(&buffer_id);
    }
}

impl Default for BufferCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct CachedBuffer {
    pub text: String,
    pub tree_hash: u64,
    pub version: u64,
}