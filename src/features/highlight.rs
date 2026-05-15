use crate::parser::queries::engine::{QueryEngine, CachedQuery};
use crate::parser::queries::QueryType;
use crate::runtime::state::HighlightRange;
use tree_sitter::{Node, Language};
use parking_lot::RwLock;
use std::sync::Arc;
use fxhash::FxHashSet;

pub struct HighlightEngine {
    custom_queries: Arc<RwLock<Vec<Arc<CachedQuery>>>>,
    engine: QueryEngine,
}

impl HighlightEngine {
    pub fn new() -> Self {
        Self {
            custom_queries: Arc::new(RwLock::new(Vec::new())),
            engine: QueryEngine::new(),
        }
    }

    pub fn add_query(&self, query: String, language: &Language) {
        if let Ok(ts_query) = tree_sitter::Query::new(language, &query) {
            self.custom_queries.write().push(Arc::new(CachedQuery::new(ts_query)));
        }
    }

    pub fn apply_highlights(&self, root: Node, source: &[u8], lang: &str, language: Language) -> Vec<HighlightRange> {
        let mut highlights_set = FxHashSet::default();

        if let Some(cached_query) = self.engine.get(lang, QueryType::Highlights, &language) {
            QueryEngine::execute(&cached_query.query, root, source, |m| {
                self.process_matches(&cached_query, m, &mut highlights_set);
            });
        }

        for cached_query in self.custom_queries.read().iter() {
            QueryEngine::execute(&cached_query.query, root, source, |m| {
                self.process_matches(cached_query, m, &mut highlights_set);
            });
        }

        let mut highlights: Vec<HighlightRange> = highlights_set.into_iter().collect();
        highlights.sort_by_key(|h| h.start_byte);
        highlights
    }

    fn process_matches(&self, cached_query: &CachedQuery, m: &crate::parser::queries::QueryMatch, highlights: &mut FxHashSet<HighlightRange>) {
        for capture in m.captures {
            let highlight = cached_query.capture_mappings[capture.index as usize];
            highlights.insert(HighlightRange {
                start_byte: capture.node.start_byte(),
                end_byte: capture.node.end_byte(),
                highlight,
            });
        }
    }
}

impl Default for HighlightEngine {
    fn default() -> Self {
        Self::new()
    }
}