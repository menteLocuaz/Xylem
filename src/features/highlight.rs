use crate::parser::queries::engine::{QueryEngine, CachedQuery};
use crate::parser::queries::QueryType;
use tree_sitter::{Node, Language, Query, QueryCursor, Range};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use fxhash::FxHashSet;
use streaming_iterator::StreamingIterator;

#[derive(Clone, PartialEq, Debug)]
pub struct CaptureEntry {
    pub start_col: u32,
    pub end_col: u32,
    pub hl_group: String,
}

#[derive(Clone, Debug)]
pub struct HighlightDelta {
    pub line: u32,
    pub captures: Vec<CaptureEntry>,
}

pub struct HighlightEngine {
    custom_queries: Arc<RwLock<Vec<Arc<CachedQuery>>>>,
    engine: QueryEngine,
    capture_cache: HashMap<u32, Vec<CaptureEntry>>,
}

impl HighlightEngine {
    pub fn new() -> Self {
        Self {
            custom_queries: Arc::new(RwLock::new(Vec::new())),
            engine: QueryEngine::new(),
            capture_cache: HashMap::new(),
        }
    }

    pub fn add_query(&self, query: String, language: &Language) {
        if let Ok(ts_query) = tree_sitter::Query::new(language, &query) {
            self.custom_queries.write().push(Arc::new(CachedQuery::new(ts_query)));
        }
    }

    pub fn apply_highlights(&self, root: Node, source: &[u8], lang: &str, language: Language) -> Vec<crate::runtime::state::HighlightRange> {
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

        let mut highlights: Vec<crate::runtime::state::HighlightRange> = highlights_set.into_iter().collect();
        highlights.sort_by_key(|h| h.start_byte);
        highlights
    }

    fn process_matches(&self, cached_query: &CachedQuery, m: &crate::parser::queries::QueryMatch, highlights: &mut FxHashSet<crate::runtime::state::HighlightRange>) {
        for capture in m.captures {
            let highlight = cached_query.capture_mappings[capture.index as usize];
            highlights.insert(crate::runtime::state::HighlightRange {
                start_byte: capture.node.start_byte(),
                end_byte: capture.node.end_byte(),
                highlight,
            });
        }
    }

    /// Full repaint: clear cache, highlight entire buffer, return all deltas.
    pub fn full_repaint(
        &mut self,
        source: &[u8],
        root: Node,
        lang: &str,
        language: Language,
    ) -> Vec<HighlightDelta> {
        self.capture_cache.clear();

        let mut captures_by_line: HashMap<u32, Vec<CaptureEntry>> = HashMap::new();

        self.collect_all_captures(root, source, lang, language, &mut captures_by_line);

        self.capture_cache = captures_by_line.clone();

        let mut deltas: Vec<HighlightDelta> = captures_by_line
            .into_iter()
            .map(|(line, captures)| HighlightDelta { line, captures })
            .collect();
        deltas.sort_by_key(|d| d.line);
        deltas
    }

    /// Incremental repaint: only re-query changed ranges, return delta for affected lines.
    pub fn repaint_ranges(
        &mut self,
        source: &[u8],
        root: Node,
        lang: &str,
        language: Language,
        changed_ranges: &[Range],
    ) -> Vec<HighlightDelta> {
        let mut deltas: Vec<HighlightDelta> = Vec::new();

        for range in changed_ranges {
            let start_line = range.start_point.row as u32;
            let end_line = range.end_point.row as u32;

            for line in start_line..=end_line {
                self.capture_cache.remove(&line);
            }

            let mut captures_by_line: HashMap<u32, Vec<CaptureEntry>> = HashMap::new();
            self.collect_captures_in_range(
                root,
                source,
                lang,
                &language,
                range.start_byte..range.end_byte,
                start_line..=end_line,
                &mut captures_by_line,
            );

            for (line, captures) in &captures_by_line {
                self.capture_cache.insert(*line, captures.clone());
            }

            for (line, captures) in captures_by_line {
                deltas.push(HighlightDelta { line, captures });
            }
        }

        deltas.sort_by_key(|d| d.line);
        deltas.dedup_by(|a, b| {
            if a.line == b.line {
                b.captures.extend(a.captures.iter().cloned());
                true
            } else {
                false
            }
        });
        deltas
    }

    fn collect_all_captures(
        &self,
        root: Node,
        source: &[u8],
        lang: &str,
        language: Language,
        captures_by_line: &mut HashMap<u32, Vec<CaptureEntry>>,
    ) {
        let mut add_captures = |query: &Query| {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, root, source);
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let node = capture.node;
                    let start_line = node.start_position().row as u32;
                    let end_line = node.end_position().row as u32;
                    let start_col = node.start_position().column as u32;
                    let end_col = node.end_position().column as u32;
                    let hl_group = query.capture_names()[capture.index as usize].to_string();

                    let entry = CaptureEntry {
                        start_col,
                        end_col,
                        hl_group,
                    };

                    for line in start_line..=end_line {
                        captures_by_line
                            .entry(line)
                            .or_insert_with(Vec::new)
                            .push(entry.clone());
                    }
                }
            }
        };

        if let Some(cached_query) = self.engine.get(lang, QueryType::Highlights, &language) {
            add_captures(&cached_query.query);
        }

        for cached_query in self.custom_queries.read().iter() {
            add_captures(&cached_query.query);
        }
    }

    fn collect_captures_in_range(
        &self,
        root: Node,
        source: &[u8],
        lang: &str,
        language: &Language,
        byte_range: std::ops::Range<usize>,
        line_range: std::ops::RangeInclusive<u32>,
        captures_by_line: &mut HashMap<u32, Vec<CaptureEntry>>,
    ) {
        let mut add_captures = |query: &Query| {
            let mut cursor = QueryCursor::new();
            cursor.set_byte_range(byte_range.clone());
            let mut matches = cursor.matches(query, root, source);
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let node = capture.node;
                    let start_line = node.start_position().row as u32;
                    let end_line = node.end_position().row as u32;
                    let start_col = node.start_position().column as u32;
                    let end_col = node.end_position().column as u32;
                    let hl_group = query.capture_names()[capture.index as usize].to_string();

                    let entry = CaptureEntry {
                        start_col,
                        end_col,
                        hl_group,
                    };

                    for line in start_line..=end_line {
                        if line_range.contains(&line) {
                            captures_by_line
                                .entry(line)
                                .or_insert_with(Vec::new)
                                .push(entry.clone());
                        }
                    }
                }
            }
        };

        if let Some(cached_query) = self.engine.get(lang, QueryType::Highlights, &language) {
            add_captures(&cached_query.query);
        }

        for cached_query in self.custom_queries.read().iter() {
            add_captures(&cached_query.query);
        }
    }
}

impl Default for HighlightEngine {
    fn default() -> Self {
        Self::new()
    }
}
