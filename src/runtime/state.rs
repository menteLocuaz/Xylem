use ropey::Rope;
use std::collections::HashMap;
use crate::parser::{IncrementalParser, diff::compute_edit_positions};
use crate::editor::events::EditorEvent;

use crate::features::highlight::HighlightEngine;

pub struct BufferState {
    pub buffer: Rope,
    pub parser: IncrementalParser,
    pub highlight_engine: HighlightEngine,
    pub is_dirty: bool,
}

impl BufferState {
    pub fn new() -> Self {
        Self {
            buffer: Rope::new(),
            parser: IncrementalParser::new(),
            highlight_engine: HighlightEngine::new(),
            is_dirty: false,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer = Rope::from_str(text);
        self.parser.parse_full(text);
        self.is_dirty = false;
    }

    pub fn apply_change(&mut self, start_byte: usize, end_byte: usize, text: &str) {
        let old_text = self.buffer.clone();

        let start_char = self.buffer.byte_to_char(start_byte);
        let end_char = self.buffer.byte_to_char(end_byte);

        self.buffer.remove(start_char..end_char);
        self.buffer.insert(start_char, text);

        let (start_pos, old_end_pos) = compute_edit_positions(&old_text, start_byte, end_byte);
        let new_end_pos = compute_edit_positions(
            &self.buffer,
            start_byte,
            start_byte + text.len(),
        ).1;

        self.parser.apply_edit(&tree_sitter::InputEdit {
            start_byte,
            old_end_byte: end_byte,
            new_end_byte: start_byte + text.len(),
            start_position: start_pos,
            old_end_position: old_end_pos,
            new_end_position: new_end_pos,
        });
        
        self.parser.parse_rope(&self.buffer);

        self.is_dirty = true;
    }
}

pub struct RuntimeState {
    pub buffers: HashMap<u64, BufferState>,
    pub current_buffer_id: u64,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            current_buffer_id: 0,
        }
    }

    pub fn set_buffer_id(&mut self, id: u64) {
        self.current_buffer_id = id;
        self.buffers.entry(id).or_insert_with(BufferState::new);
    }

    pub fn set_text(&mut self, text: &str) {
        let state = self.buffers.entry(self.current_buffer_id).or_insert_with(BufferState::new);
        state.set_text(text);
    }

    pub fn apply_change(&mut self, change: &EditorEvent) -> bool {
        match change {
            EditorEvent::Change { buffer_id, start_byte, end_byte, text } => {
                let id = if *buffer_id == 0 { self.current_buffer_id } else { *buffer_id };
                let state = self.buffers.entry(id).or_insert_with(BufferState::new);
                state.apply_change(*start_byte, *end_byte, text);
                true
            }
            EditorEvent::Reload { buffer_id, text } => {
                let id = if *buffer_id == 0 { self.current_buffer_id } else { *buffer_id };
                let state = self.buffers.entry(id).or_insert_with(BufferState::new);
                state.set_text(text);
                true
            }
            EditorEvent::Save { buffer_id } => {
                let id = if *buffer_id == 0 { self.current_buffer_id } else { *buffer_id };
                if let Some(state) = self.buffers.get_mut(&id) {
                    state.is_dirty = false;
                }
                true
            }
            _ => false,
        }
    }

    pub fn get_highlights(&self) -> Vec<HighlightRange> {
        self.get_highlights_for_buffer(self.current_buffer_id)
    }

    pub fn get_highlights_for_buffer(&self, buffer_id: u64) -> Vec<HighlightRange> {
        let state = match self.buffers.get(&buffer_id) {
            Some(s) => s,
            None => return Vec::new(),
        };

        if let Some(root) = state.parser.root_node() {
            // TODO: Avoid to_string() here by updating HighlightEngine to support Rope
            let source = state.buffer.to_string();
            state.highlight_engine.apply_highlights(root, source.as_bytes())
        } else {
            Vec::new()
        }
    }
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct HighlightRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub highlight: String,
}
