use crate::parser::queries::types::{QueryKey, QueryType, HighlightKind};
use dashmap::DashMap;
use std::sync::Arc;
use tree_sitter::{Query, QueryCursor, QueryMatch, Node, Language};
use streaming_iterator::StreamingIterator;

pub struct CachedQuery {
    pub query: Query,
    pub capture_mappings: Vec<HighlightKind>,
}

impl CachedQuery {
    pub fn new(query: Query) -> Self {
        let capture_mappings = query
            .capture_names()
            .iter()
            .map(|name| HighlightKind::from_name(name))
            .collect();

        Self {
            query,
            capture_mappings,
        }
    }
}

pub struct QueryEngine {
    cache: DashMap<QueryKey, Arc<CachedQuery>>,
}

impl QueryEngine {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    pub fn execute<F>(
        query: &Query,
        root_node: Node,
        source: &[u8],
        mut f: F,
    ) where F: for<'a> FnMut(&QueryMatch<'a, '_>) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, root_node, source);
        while let Some(m) = matches.next() {
            f(m);
        }
    }

    pub fn get(&self, lang: &str, query_type: QueryType, _language: &Language) -> Option<Arc<CachedQuery>> {
        let key = QueryKey {
            lang: lang.to_string(),
            query_type,
        };

        if let Some(query) = self.cache.get(&key) {
            return Some(query.value().clone());
        }

        // In a real implementation, we would load the query from disk here if not cached.
        // For now, we'll just return None if not found, as before.
        None
    }

    pub fn add(&self, lang: &str, query_type: QueryType, query: Query) {
        let key = QueryKey {
            lang: lang.to_string(),
            query_type,
        };
        self.cache.insert(key, Arc::new(CachedQuery::new(query)));
    }
}
