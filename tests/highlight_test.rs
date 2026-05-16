use xylem::runtime::state::RuntimeState;

#[test]
fn test_edit_consistency() {
    let state = RuntimeState::new();
    let buffer_id = 1;
    state.set_buffer_id(buffer_id);
    state.set_text("function main()\nend");

    // Perform many small edits
    for i in 0..100 {
        state.apply_changes_and_parse(buffer_id, &[(9, 9, format!("-- edit {}\n", i))]);
    }

    // This will trigger ensure_source_bytes
    let highlights = state.get_highlights_for_buffer(buffer_id);
    let _ = highlights.len();

    // Verify buffer content consistency
    let buffer_ref = state.buffers.get(&buffer_id).unwrap();
    let state_guard = buffer_ref.read();
    assert!(state_guard.buffer.len_bytes() > 20);
}
