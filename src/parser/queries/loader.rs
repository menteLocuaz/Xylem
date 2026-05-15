use crate::parser::queries::types::QueryType;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tree_sitter::{Language, Query};

pub struct QueryLoader {
    search_paths: Vec<PathBuf>,
}

impl QueryLoader {
    pub fn new(search_paths: Vec<PathBuf>) -> Self {
        Self { search_paths }
    }

    /// Load query files from Neovim runtime paths
    pub fn load_from_runtimepath(runtimepaths: Vec<String>) -> Self {
        let mut search_paths = Vec::new();
        for path in runtimepaths {
            let p = PathBuf::from(path).join("queries");
            if p.exists() {
                search_paths.push(p);
            }
        }
        Self::new(search_paths)
    }

    pub fn load_query(&self, lang: &str, query_type: QueryType, language: &Language) -> Option<Arc<Query>> {
        let filename = format!("{}.scm", query_type.as_str());
        
        // Iterate in reverse for priority (last path wins)
        for path in self.search_paths.iter().rev() {
            let full_path = path.join(lang).join(&filename);
            if full_path.exists() {
                if let Ok(source) = std::fs::read_to_string(&full_path) {
                    if let Ok(query) = Query::new(language, &source) {
                        return Some(Arc::new(query));
                    }
                }
            }
        }
        None
    }
}
