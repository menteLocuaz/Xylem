use tree_sitter::Node;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct IndentEngine {
    indent_size: Arc<RwLock<usize>>,
    use_tabs: Arc<RwLock<bool>>,
}

impl IndentEngine {
    pub fn new() -> Self {
        Self {
            indent_size: Arc::new(RwLock::new(4)),
            use_tabs: Arc::new(RwLock::new(false)),
        }
    }

    pub fn set_indent_size(&self, size: usize) {
        *self.indent_size.write() = size;
    }

    pub fn set_use_tabs(&self, use_tabs: bool) {
        *self.use_tabs.write() = use_tabs;
    }

    pub fn get_indent_size(&self) -> usize {
        *self.indent_size.read()
    }

    pub fn is_use_tabs(&self) -> bool {
        *self.use_tabs.read()
    }

    pub fn compute_indent(&self, node: Node) -> IndentResult {
        let kind = node.kind();

        let base_indent = self.calculate_base_indent(node);
        let delta = self.calculate_delta(kind);

        IndentResult {
            base_indent,
            delta,
            should_outdent: self.should_outdent(kind),
        }
    }

    fn calculate_base_indent(&self, node: Node) -> usize {
        if let Some(parent) = node.parent() {
            if parent.kind() == "chunk" {
                return 0;
            }
        }
        self.get_indent_size()
    }

    fn calculate_delta(&self, kind: &str) -> i32 {
        match kind {
            "function_declaration" | "if_statement" | "for_statement" |
            "while_statement" | "repeat_statement" | "local_function" => 1,
            _ => 0,
        }
    }

    fn should_outdent(&self, kind: &str) -> bool {
        matches!(
            kind,
            "end" | "else" | "elseif" | "until"
        )
    }
}

impl Default for IndentEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct IndentResult {
    pub base_indent: usize,
    pub delta: i32,
    pub should_outdent: bool,
}