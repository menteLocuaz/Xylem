use tree_sitter::{Parser, Tree, InputEdit, Node, Range};
use tree_sitter_lua::LANGUAGE;
use ropey::Rope;

pub struct IncrementalParser {
    parser: Parser,
    current_tree: Option<Tree>,
    previous_tree: Option<Tree>,
    has_parsed: bool,
}

impl IncrementalParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).expect("Error loading Lua grammar");

        Self {
            parser,
            current_tree: None,
            previous_tree: None,
            has_parsed: false,
        }
    }

    /// Perform a full re-parse of the provided rope.
    pub fn parse_full(&mut self, rope: &Rope) {
        self.previous_tree = self.current_tree.take();
        self.current_tree = self.parser.parse_with(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return "";
                }
                let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                &chunk[byte - chunk_byte_idx..]
            },
            None,
        );
        self.has_parsed = true;
    }

    /// Apply an edit and re-parse incrementally.
    pub fn parse_incremental(&mut self, rope: &Rope, edit: InputEdit) {
        if let Some(ref mut tree) = self.current_tree {
            tree.edit(&edit);
        }

        self.previous_tree = self.current_tree.take();

        self.current_tree = self.parser.parse_with(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return "";
                }
                let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                &chunk[byte - chunk_byte_idx..]
            },
            self.previous_tree.as_ref(),
        );
        self.has_parsed = true;
    }

    pub fn root_node(&self) -> Option<Node<'_>> {
        self.current_tree.as_ref().map(|t| t.root_node())
    }

    /// Return the ranges that changed between the previous and current tree.
    pub fn changed_ranges(&self) -> Vec<Range> {
        match (&self.previous_tree, &self.current_tree) {
            (Some(old), Some(new)) => old.changed_ranges(new).collect(),
            _ => vec![],
        }
    }

    /// Returns true if no parse has been performed yet.
    pub fn is_first_parse(&self) -> bool {
        !self.has_parsed
    }
}

impl Default for IncrementalParser {
    fn default() -> Self {
        Self::new()
    }
}
