use ropey::Rope;

pub struct DiffResult {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_text: String,
}

pub fn compute_diff(old_text: &Rope, new_text: &str) -> Option<DiffResult> {
    let old_str = old_text.to_string();
    let old_bytes = old_str.as_bytes();
    let new_bytes = new_text.as_bytes();

    let mut start = 0;
    let old_len = old_bytes.len();
    let new_len = new_bytes.len();

    while start < old_len && start < new_len && old_bytes[start] == new_bytes[start] {
        start += 1;
    }

    if start == old_len && start == new_len {
        return None;
    }

    let mut end_old = old_len;
    let mut end_new = new_len;

    while end_old > start && end_new > start && old_bytes[end_old - 1] == new_bytes[end_new - 1] {
        end_old -= 1;
        end_new -= 1;
    }

    Some(DiffResult {
        start_byte: start,
        old_end_byte: end_old,
        new_text: new_text[start..new_len].to_string(),
    })
}

pub fn compute_edit_positions(
    text: &Rope,
    start_byte: usize,
    end_byte: usize,
) -> (tree_sitter::Point, tree_sitter::Point) {
    let start_char = text.byte_to_char(start_byte);
    let end_char = text.byte_to_char(end_byte);

    let start_row = text.char_to_line(start_char);
    let end_row = text.char_to_line(end_char);

    let start_col = if start_row == 0 {
        start_byte
    } else {
        let row_start_byte = text.char_to_byte(start_row);
        start_byte - row_start_byte
    };

    let end_col = if end_row == 0 {
        end_byte
    } else {
        let row_start_byte = text.char_to_byte(end_row);
        end_byte - row_start_byte
    };

    (tree_sitter::Point { row: start_row, column: start_col },
     tree_sitter::Point { row: end_row, column: end_col })
}