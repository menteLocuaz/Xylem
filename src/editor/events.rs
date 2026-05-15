use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EditorEvent {
    Change {
        buffer_id: u64,
        start_byte: usize,
        end_byte: usize,
        text: String,
    },
    Save { buffer_id: u64 },
    Reload { buffer_id: u64, text: String },
    Create { buffer_id: u64, text: String },
    Delete { buffer_id: u64 },
}

impl EditorEvent {
    pub fn from_on_lines(
        buffer_id: u64,
        _start_row: usize,
        _end_row: usize,
        old_num_lines: usize,
        _new_num_lines: usize,
        lines: &str,
    ) -> Self {
        let start_byte = 0;
        let end_byte = old_num_lines * 100;

        EditorEvent::Change {
            buffer_id,
            start_byte,
            end_byte,
            text: lines.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightUpdate {
    pub buffer_id: u64,
    pub highlights: Vec<HighlightDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightDef {
    pub start_byte: usize,
    pub end_byte: usize,
    pub hl_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub buffer_id: u64,
    pub ast: String,
    pub highlights: Vec<HighlightDef>,
}