use ropey::Rope;
use tree_sitter::{Point, InputEdit};

pub struct EditResult {
    pub input_edit: InputEdit,
    pub new_text: String,
}

pub fn compute_edit(
    old_text: &Rope,
    start_byte: usize,
    end_byte: usize,
    new_text: &str,
) -> EditResult {
    let start_char = old_text.char_to_line(old_text.byte_to_char(start_byte));
    let start_byte_clamped = old_text.char_to_byte(start_char);

    let end_char = old_text.char_to_line(old_text.byte_to_char(end_byte));
    let end_byte_clamped = old_text.char_to_byte(end_char);

    let old_end_position = old_text.char_to_line(end_char);
    let start_position = Point {
        row: start_char,
        column: if start_char == 0 { 0 } else { start_byte - start_byte_clamped },
    };
    let old_end_position = Point {
        row: end_char,
        column: if end_char == 0 { 0 } else { end_byte - end_byte_clamped },
    };

    let old_len = end_byte - start_byte;
    let new_len = new_text.len();
    let new_end_byte = start_byte + new_len;

    let new_end_position = Point {
        row: start_position.row + new_text.matches('\n').count(),
        column: if new_text.contains('\n') {
            new_text.len() - new_text.rfind('\n').unwrap()
        } else {
            new_text.len()
        },
    };

    EditResult {
        input_edit: InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        },
        new_text: new_text.to_string(),
    }
}

pub fn rope_to_string(rope: &Rope) -> String {
    rope.to_string()
}