use ropey::Rope;
use std::collections::HashMap;
use std::ops::Range;
use crate::parser::{IncrementalParser, diff::compute_edit_positions};
use crate::editor::events::EditorEvent;

use crate::features::highlight::{HighlightEngine, HighlightDelta};

#[derive(Debug, Clone)]
pub struct DirtyRegion {
    pub byte_range: Range<usize>,
}

pub struct BufferState {
    pub buffer: Rope,
    source_bytes: Vec<u8>,
    pub parser: IncrementalParser,
    pub highlight_engine: HighlightEngine,
    pub is_dirty: bool,
    pub version: u64,
    pub dirty_regions: Vec<DirtyRegion>,
}

impl BufferState {
    pub fn new() -> Self {
        Self {
            buffer: Rope::new(),
            source_bytes: Vec::new(),
            parser: IncrementalParser::new(),
            highlight_engine: HighlightEngine::new(),
            is_dirty: false,
            version: 0,
            dirty_regions: Vec::new(),
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer = Rope::from_str(text);
        self.source_bytes = text.as_bytes().to_vec();
        self.parser.parse_full(&self.buffer);
        self.is_dirty = false;
        self.version += 1;
        self.dirty_regions.clear();
        self.dirty_regions.push(DirtyRegion { byte_range: 0..self.buffer.len_bytes() });
    }

    pub fn apply_change(&mut self, start_byte: usize, end_byte: usize, text: &str) {
        let old_text = self.buffer.clone();

        let start_char = self.buffer.byte_to_char(start_byte);
        let end_char = self.buffer.byte_to_char(end_byte);

        self.buffer.remove(start_char..end_char);
        self.buffer.insert(start_char, text);

        let mut bytes = Vec::with_capacity(self.buffer.len_bytes());
        for chunk in self.buffer.chunks() {
            bytes.extend_from_slice(chunk.as_bytes());
        }
        self.source_bytes = bytes;

        let (start_pos, old_end_pos) = compute_edit_positions(&old_text, start_byte, end_byte);
        let new_end_pos = compute_edit_positions(
            &self.buffer,
            start_byte,
            start_byte + text.len(),
        ).1;

        let edit = tree_sitter::InputEdit {
            start_byte,
            old_end_byte: end_byte,
            new_end_byte: start_byte + text.len(),
            start_position: start_pos,
            old_end_position: old_end_pos,
            new_end_position: new_end_pos,
        };

        self.parser.parse_incremental(&self.buffer, edit);

        self.is_dirty = true;
        self.version += 1;
        let new_end = start_byte + text.len();
        self.dirty_regions.push(DirtyRegion { byte_range: start_byte..end_byte.max(new_end) });
    }

    pub fn full_reparse(&mut self) -> Vec<HighlightDelta> {
        self.parser.parse_full(&self.buffer);
        self.source_bytes = {
            let mut bytes = Vec::with_capacity(self.buffer.len_bytes());
            for chunk in self.buffer.chunks() {
                bytes.extend_from_slice(chunk.as_bytes());
            }
            bytes
        };
        self.is_dirty = false;
        self.version += 1;
        self.compute_highlights()
    }

    pub fn apply_multiple_changes(&mut self, changes: &[(usize, usize, String)]) -> Vec<HighlightDelta> {
        for (start_byte, end_byte, text) in changes {
            self.apply_change(*start_byte, *end_byte, text);
        }
        self.compute_highlights()
    }

    pub fn compute_highlights(&mut self) -> Vec<HighlightDelta> {
        let root = match self.parser.root_node() {
            Some(r) => r,
            None => return vec![],
        };

        if self.parser.is_first_parse() {
            self.highlight_engine.full_repaint(
                &self.source_bytes,
                root,
                "lua",
                tree_sitter_lua::LANGUAGE.into(),
            )
        } else {
            let changed = self.parser.changed_ranges();
            self.highlight_engine.repaint_ranges(
                &self.source_bytes,
                root,
                "lua",
                tree_sitter_lua::LANGUAGE.into(),
                &changed,
            )
        }
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

    pub fn full_parse(&mut self, buffer_id: u64) -> Vec<HighlightDelta> {
        if let Some(state) = self.buffers.get_mut(&buffer_id) {
            state.full_reparse()
        } else {
            vec![]
        }
    }

    pub fn apply_changes_and_parse(&mut self, buffer_id: u64, changes: &[(usize, usize, String)]) -> Vec<HighlightDelta> {
        let state = self.buffers.entry(buffer_id).or_insert_with(BufferState::new);
        state.apply_multiple_changes(changes)
    }

    pub fn apply_change(&mut self, change: &EditorEvent) -> Option<Vec<HighlightDelta>> {
        match change {
            EditorEvent::Change { buffer_id, start_byte, end_byte, text } => {
                let id = if *buffer_id == 0 { self.current_buffer_id } else { *buffer_id };
                let state = self.buffers.entry(id).or_insert_with(BufferState::new);
                state.apply_change(*start_byte, *end_byte, text);
                Some(state.compute_highlights())
            }
            EditorEvent::Reload { buffer_id, text } => {
                let id = if *buffer_id == 0 { self.current_buffer_id } else { *buffer_id };
                let state = self.buffers.entry(id).or_insert_with(BufferState::new);
                state.set_text(text);
                Some(state.compute_highlights())
            }
            EditorEvent::Save { buffer_id } => {
                let id = if *buffer_id == 0 { self.current_buffer_id } else { *buffer_id };
                if let Some(state) = self.buffers.get_mut(&id) {
                    state.is_dirty = false;
                    state.dirty_regions.clear();
                }
                None
            }
            _ => None,
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
            let source = &state.source_bytes;
            state.highlight_engine.apply_highlights(
                root,
                source,
                "lua",
                tree_sitter_lua::LANGUAGE.into(),
            )
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

use serde::{Serialize, Deserialize};
use crate::parser::queries::types::HighlightKind;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HighlightRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub highlight: HighlightKind,
}
