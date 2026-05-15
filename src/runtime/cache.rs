use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::{Arc, OnceLock};
use tree_sitter::Query;
use std::path::Path;
use std::fs;

static QUERY_CACHE: OnceLock<RwLock<HashMap<String, Arc<Query>>>> = OnceLock::new();

pub fn get_or_load_query(lang: &str, query_name: &str, scm_path: &Path, fallback: Option<&str>) -> Option<Arc<Query>> {
    let cache_key = format!("{}:{}", lang, query_name);
    let cache = QUERY_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    
    // Fast path - read lock
    {
        let read_lock = cache.read();
        if let Some(q) = read_lock.get(&cache_key) {
            return Some(Arc::clone(q));
        }
    }
    
    // Slow path - load and compile
    let language = match lang {
        "lua" => tree_sitter_lua::LANGUAGE,
        _ => return None,
    };

    let source = fs::read_to_string(scm_path).ok()
        .or_else(|| fallback.map(|s| s.to_string()))?;

    if let Ok(query) = Query::new(&language.into(), &source) {
        let arc_query = Arc::new(query);
        let mut write_lock = cache.write();
        write_lock.insert(cache_key, Arc::clone(&arc_query));
        Some(arc_query)
    } else {
        None
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
