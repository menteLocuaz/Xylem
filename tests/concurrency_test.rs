use std::sync::Arc;
use std::thread;
use xylem::runtime::state::RuntimeState;

#[test]
fn test_concurrent_buffer_access() {
    let state = Arc::new(RuntimeState::new());
    let mut handles = vec![];

    for i in 0..10 {
        let state = state.clone();
        let handle = thread::spawn(move || {
            let buffer_id = i as u64;
            state.set_buffer_id(buffer_id);
            state.set_text(&format!("local x{} = {}", i, i));

            // Perform some edits
            for j in 0..100 {
                state.apply_changes_and_parse(buffer_id, &[(0, 0, format!("-- comment {}\n", j))]);
            }

            let highlights = state.get_highlights_for_buffer(buffer_id);
            // Even if empty (due to missing queries), it should not crash
            let _ = highlights.len();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(state.buffers.len(), 10);
}

#[test]
fn test_interleaved_buffer_access() {
    let state = Arc::new(RuntimeState::new());

    state.set_buffer_id(1);
    state.set_text("local a = 1");

    state.set_buffer_id(2);
    state.set_text("local b = 2");

    let s1 = state.clone();
    let h1 = thread::spawn(move || {
        for _ in 0..100 {
            s1.apply_changes_and_parse(1, &[(0, 0, "-- edit 1\n".to_string())]);
        }
    });

    let s2 = state.clone();
    let h2 = thread::spawn(move || {
        for _ in 0..100 {
            s2.apply_changes_and_parse(2, &[(0, 0, "-- edit 2\n".to_string())]);
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert!(state.buffers.get(&1).is_some());
    assert!(state.buffers.get(&2).is_some());
}
