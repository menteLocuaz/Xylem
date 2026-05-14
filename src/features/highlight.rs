use crate::parser::queries::QueryEngine;
use crate::runtime::state::HighlightRange;
use tree_sitter::Node;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct HighlightEngine {
    custom_queries: Arc<RwLock<Vec<String>>>,
}

impl HighlightEngine {
    pub fn new() -> Self {
        Self {
            custom_queries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_query(&self, query: String) {
        self.custom_queries.write().push(query);
    }

    pub fn apply_highlights(&self, root: Node, source: &[u8]) -> Vec<HighlightRange> {
        let mut highlights = Vec::new();
        let language = tree_sitter_lua::LANGUAGE;

        let default_query = r#"
            (function_declaration
              name: (identifier) @function)
            (function_call
              name: (identifier) @function)
            (identifier) @variable
            (number) @number
            (string) @string
            (comment) @comment
            (keyword) @keyword
            (["local" "require" "return" "if" "then" "else" "end"] @keyword)
        "#;

        if let Ok(query) = tree_sitter::Query::new(&language.into(), default_query) {
            let matches = QueryEngine::execute(&query, root, source);
            self.process_matches(&matches, &mut highlights);
        }

        for custom in self.custom_queries.read().iter() {
            if let Ok(query) = tree_sitter::Query::new(&language.into(), custom) {
                let matches = QueryEngine::execute(&query, root, source);
                self.process_matches(&matches, &mut highlights);
            }
        }

        highlights.sort_by_key(|h| h.start_byte);
        highlights
    }

    fn process_matches(&self, matches: &[crate::parser::queries::QueryMatch], highlights: &mut Vec<HighlightRange>) {
        for m in matches {
            for (name, start, end) in &m.captures {
                if let Some(ns) = name.strip_prefix('@') {
                    let hl_group = match ns {
                        "function" => "Function",
                        "variable" => "Variable",
                        "number" => "Number",
                        "string" => "String",
                        "comment" => "Comment",
                        "keyword" => "Keyword",
                        _ => "Normal",
                    };

                    highlights.push(HighlightRange {
                        start_byte: *start,
                        end_byte: *end,
                        highlight: hl_group.to_string(),
                    });
                }
            }
        }
    }
}

impl Default for HighlightEngine {
    fn default() -> Self {
        Self::new()
    }
}