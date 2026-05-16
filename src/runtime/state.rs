use ropey::Rope;
use std::sync::Arc;
use std::ops::Range;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use crate::parser::{IncrementalParser, diff::compute_edit_positions};
use crate::editor::events::EditorEvent;
use crate::parser::queries::types::HighlightKind;

use crate::features::highlight::{HighlightEngine, HighlightDelta};

#[derive(Debug, Clone)]
pub struct DirtyRegion {
    pub byte_range: Range<usize>,
}

pub struct BufferState {
    pub buffer: Rope,
    source_bytes: Vec<u8>,
    source_bytes_dirty: bool,
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
            source_bytes_dirty: false,
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
        self.source_bytes_dirty = false;
        self.parser.parse_full(&self.buffer);
        self.is_dirty = false;
        self.version += 1;
        self.dirty_regions.clear();
        self.dirty_regions.push(DirtyRegion { byte_range: 0..self.buffer.len_bytes() });
    }

    pub fn ensure_source_bytes(&mut self) {
        if !self.source_bytes_dirty {
            return;
        }
        let mut bytes = Vec::with_capacity(self.buffer.len_bytes());
        for chunk in self.buffer.chunks() {
            bytes.extend_from_slice(chunk.as_bytes());
        }
        self.source_bytes = bytes;
        self.source_bytes_dirty = false;
    }

    pub fn apply_change(&mut self, start_byte: usize, end_byte: usize, text: &str) {
        let old_text = self.buffer.clone();

        let start_char = self.buffer.byte_to_char(start_byte);
        let end_char = self.buffer.byte_to_char(end_byte);

        self.buffer.remove(start_char..end_char);
        self.buffer.insert(start_char, text);

        self.source_bytes_dirty = true;

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
        self.source_bytes_dirty = true;
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
        self.ensure_source_bytes();
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
    pub buffers: DashMap<u64, Arc<RwLock<BufferState>>>,
    pub current_buffer_id: Arc<RwLock<u64>>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            buffers: DashMap::new(),
            current_buffer_id: Arc::new(RwLock::new(0)),
        }
    }

    pub fn set_buffer_id(&self, id: u64) {
        *self.current_buffer_id.write() = id;
        self.buffers.entry(id).or_insert_with(|| Arc::new(RwLock::new(BufferState::new())));
    }

    pub fn set_text(&self, text: &str) {
        let id = *self.current_buffer_id.read();
        let state = self.buffers.entry(id).or_insert_with(|| Arc::new(RwLock::new(BufferState::new())));
        state.write().set_text(text);
    }

    pub fn full_parse(&self, buffer_id: u64) -> Vec<HighlightDelta> {
        if let Some(state) = self.buffers.get(&buffer_id) {
            state.write().full_reparse()
        } else {
            vec![]
        }
    }

    pub fn apply_changes_and_parse(&self, buffer_id: u64, changes: &[(usize, usize, String)]) -> Vec<HighlightDelta> {
        let state = self.buffers.entry(buffer_id).or_insert_with(|| Arc::new(RwLock::new(BufferState::new())));
        state.write().apply_multiple_changes(changes)
    }

    pub fn apply_change(&self, change: &EditorEvent) -> Option<Vec<HighlightDelta>> {
        match change {
            EditorEvent::Change { buffer_id, start_byte, end_byte, text } => {
                let id = if *buffer_id == 0 { *self.current_buffer_id.read() } else { *buffer_id };
                let state = self.buffers.entry(id).or_insert_with(|| Arc::new(RwLock::new(BufferState::new())));
                let mut state_guard = state.write();
                state_guard.apply_change(*start_byte, *end_byte, text);
                Some(state_guard.compute_highlights())
            }
            EditorEvent::Reload { buffer_id, text } => {
                let id = if *buffer_id == 0 { *self.current_buffer_id.read() } else { *buffer_id };
                let state = self.buffers.entry(id).or_insert_with(|| Arc::new(RwLock::new(BufferState::new())));
                let mut state_guard = state.write();
                state_guard.set_text(text);
                Some(state_guard.compute_highlights())
            }
            EditorEvent::Save { buffer_id } => {
                let id = if *buffer_id == 0 { *self.current_buffer_id.read() } else { *buffer_id };
                if let Some(state) = self.buffers.get(&id) {
                    let mut state_guard = state.write();
                    state_guard.is_dirty = false;
                    state_guard.dirty_regions.clear();
                }
                None
            }
            _ => None,
        }
    }

    pub fn get_highlights(&self) -> Vec<HighlightRange> {
        let id = *self.current_buffer_id.read();
        self.get_highlights_for_buffer(id)
    }

    pub fn get_highlights_for_buffer(&self, buffer_id: u64) -> Vec<HighlightRange> {
        let state = match self.buffers.get(&buffer_id) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let mut state_guard = state.write();
        state_guard.ensure_source_bytes();
        if let Some(root) = state_guard.parser.root_node() {
            let source = &state_guard.source_bytes;
            state_guard.highlight_engine.apply_highlights(
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HighlightRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub highlight: HighlightKind,
}
