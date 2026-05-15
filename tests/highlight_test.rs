use xylem::runtime::state::RuntimeState;
use xylem::parser::queries::types::HighlightKind;

#[test]
fn test_lua_highlighting() {
    let mut state = RuntimeState::new();
    state.set_text(r#"
local x = 10
function hello()
    print(x)
end
"#);

    // Note: We need to register a query for this to work in a real scenario.
    // However, QueryEngine::get currently returns None for all languages because it's not implemented yet.
    // The current implementation of apply_highlights will return an empty vector.
    
    let highlights = state.get_highlights();
    // Since we don't have a way to load queries easily in this test yet,
    // we'll just verify that it doesn't crash and returns a vector.
    assert!(highlights.is_empty()); 
}
