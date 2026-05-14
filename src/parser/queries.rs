use tree_sitter::{Query, QueryCursor, Node};
use streaming_iterator::StreamingIterator;

pub struct QueryEngine;

impl QueryEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        query: &Query,
        root: Node,
        source: &[u8],
    ) -> Vec<QueryMatch> {
        let mut cursor = QueryCursor::new();
        let mut matches = Vec::new();

        let mut cursor = cursor.matches(query, root, source);
        while let Some(m) = cursor.next() {
            let captured: Vec<(String, usize, usize)> = m.captures.iter().map(|c| {
                let name = query.capture_names()[c.index as usize].to_string();
                (name, c.node.start_byte(), c.node.end_byte())
            }).collect();

            matches.push(QueryMatch { captures: captured });
        }

        matches
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone)]
pub struct QueryMatch {
    pub captures: Vec<(String, usize, usize)>,
}