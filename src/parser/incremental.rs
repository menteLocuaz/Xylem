use tree_sitter::{Parser, Point, Tree, InputEdit, Node};
use tree_sitter_lua::LANGUAGE;

pub struct IncrementalParser {
    parser: Parser,
    tree: Option<Tree>,
}

impl IncrementalParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).ok();

        Self {
            parser,
            tree: None,
        }
    }

    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, self.tree.as_ref())
    }

    pub fn parse_rope(&mut self, rope: &ropey::Rope) -> Option<Tree> {
        self.parser.parse_with(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return "";
                }
                let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                &chunk[byte - chunk_byte_idx..]
            },
            self.tree.as_ref(),
        )
    }

    pub fn parse_full(&mut self, source: &str) {
        if let Some(tree) = self.parse(source) {
            self.tree = Some(tree);
        }
    }

    pub fn apply_edit(&mut self, edit: &InputEdit) {
        if let Some(ref mut t) = self.tree {
            t.edit(edit);
        }
    }

    pub fn edit(
        &mut self,
        start_byte: usize,
        old_end_byte: usize,
        new_end_byte: usize,
        start_position: Point,
        old_end_position: Point,
        new_end_position: Point,
        source: &str,
    ) {
        self.apply_edit(&InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        });

        if !source.is_empty() {
            if let Some(new_tree) = self.parse(source) {
                self.tree = Some(new_tree);
            }
        }
    }

    pub fn root_node(&self) -> Option<Node<'_>> {
        self.tree.as_ref().map(|t| t.root_node())
    }

    pub fn get_tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }
}

impl Default for IncrementalParser {
    fn default() -> Self {
        Self::new()
    }
}