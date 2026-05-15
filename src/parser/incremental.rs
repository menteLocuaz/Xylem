use tree_sitter::{Parser, Point, Tree, InputEdit, Node};
use tree_sitter_lua::LANGUAGE;
use ropey::Rope;

pub struct IncrementalParser {
    parser: Parser,
    tree: Option<Tree>,
}

impl IncrementalParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).expect("Error loading Lua grammar");

        Self {
            parser,
            tree: None,
        }
    }

    /// Perform a full re-parse of the provided rope.
    pub fn parse_full(&mut self, rope: &Rope) {
        self.tree = self.parser.parse_with(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return "";
                }
                let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                &chunk[byte - chunk_byte_idx..]
            },
            None,
        );
    }

    /// Apply an edit and re-parse incrementally.
    pub fn parse_incremental(&mut self, rope: &Rope, edit: InputEdit) {
        if let Some(ref mut tree) = self.tree {
            tree.edit(&edit);

            self.tree = self.parser.parse_with(
                &mut |byte, _| {
                    if byte >= rope.len_bytes() {
                        return "";
                    }
                    let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                    &chunk[byte - chunk_byte_idx..]
                },
                Some(tree),
            );
        } else {
            self.parse_full(rope);
        }
    }

    pub fn root_node(&self) -> Option<Node<'_>> {
        self.tree.as_ref().map(|t| t.root_node())
    }
}

impl Default for IncrementalParser {
    fn default() -> Self {
        Self::new()
    }
}