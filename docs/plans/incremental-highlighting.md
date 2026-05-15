# Incremental Highlighting Pipeline - Implementation Plan (Corrected)

## Execution Order: Phase 0 → 1 → 2 → 3 → 4 → 5

---

## Phase 0: Wire incremental path in main.rs + unify RPC protocol

**File:** `src/main.rs`

### Problem
`xylem.change` routes to `EditorEvent::Reload` → `set_text()` → full parse. The incremental path (`EditorEvent::Change`) is never used.

### Changes
1. Route `xylem.change` to `EditorEvent::Change` with `start_byte`, `old_end_byte`, `new_text`
2. Add stdout writer for RPC responses (Content-Length + JSON)
3. After processing, send highlight deltas back to Neovim

### Code:
```rust
// In handle_message:
"xylem.change" => {
    if let Some(p) = params {
        let buffer_id = p.get("buffer_id").and_then(|v| v.as_u64()).unwrap_or(0);
        let start_byte = p.get("start_byte").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let old_end_byte = p.get("old_end_byte").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let new_text = p.get("new_text").and_then(|t| t.as_str()).unwrap_or("");

        let event = EditorEvent::Change {
            buffer_id,
            start_byte,
            end_byte: old_end_byte,
            text: new_text.to_string(),
        };
        runtime.write().apply_change(&event);

        let deltas = runtime.write().compute_highlights_for_buffer(buffer_id);
        // send deltas to stdout
    }
}
```

---

## Phase 1: IncrementalParser — save old tree, changed_ranges, has_parsed

**File:** `src/parser/incremental.rs`

### Changes
1. Rename `tree` → `current_tree`, add `previous_tree: Option<Tree>`
2. Add `has_parsed: bool` flag (explicit, not derived from previous_tree)
3. In `parse_full()` and `parse_incremental()`: save old tree before parsing, set `has_parsed = true`
4. Add `changed_ranges() -> Vec<Range>`
5. Add `is_first_parse() -> bool` using `has_parsed` flag

### Code:
```rust
use tree_sitter::{Parser, Point, Tree, InputEdit, Node, Range};
use tree_sitter_lua::LANGUAGE;
use ropey::Rope;

pub struct IncrementalParser {
    parser: Parser,
    current_tree: Option<Tree>,
    previous_tree: Option<Tree>,
    has_parsed: bool,
}

impl IncrementalParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).expect("Error loading Lua grammar");

        Self {
            parser,
            current_tree: None,
            previous_tree: None,
            has_parsed: false,
        }
    }

    pub fn parse_full(&mut self, rope: &Rope) {
        self.previous_tree = self.current_tree.take();
        self.current_tree = self.parser.parse_with(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return "";
                }
                let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                &chunk[byte - chunk_byte_idx..]
            },
            None,
        );
        self.has_parsed = true;
    }

    pub fn parse_incremental(&mut self, rope: &Rope, edit: InputEdit) {
        if let Some(ref mut tree) = self.current_tree {
            tree.edit(&edit);
        }

        self.previous_tree = self.current_tree.take();

        self.current_tree = self.parser.parse_with(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return "";
                }
                let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
                &chunk[byte - chunk_byte_idx..]
            },
            self.previous_tree.as_ref(),
        );
        self.has_parsed = true;
    }

    pub fn root_node(&self) -> Option<Node<'_>> {
        self.current_tree.as_ref().map(|t| t.root_node())
    }

    pub fn changed_ranges(&self) -> Vec<Range> {
        match (&self.previous_tree, &self.current_tree) {
            (Some(old), Some(new)) => old.changed_ranges(new).collect(),
            _ => vec![],
        }
    }

    pub fn is_first_parse(&self) -> bool {
        !self.has_parsed
    }
}
```

---

## Phase 2: HighlightEngine — line-keyed cache, repaint_ranges, full_repaint

**File:** `src/features/highlight.rs`

### Design decision: cache keyed by line number
- Invalidated by clearing the range of lines in `changed_range`
- `CaptureEntry` stores both byte and line/col for RPC conversion
- No byte→line lookup needed at cache access time

### New types:
```rust
#[derive(Clone, PartialEq, Debug)]
pub struct CaptureEntry {
    pub start_col: u32,
    pub end_col: u32,
    pub hl_group: String,
}

#[derive(Clone, Debug)]
pub struct HighlightDelta {
    pub line: u32,
    pub captures: Vec<CaptureEntry>,
}
```

### HighlightEngine changes:
```rust
pub struct HighlightEngine {
    custom_queries: Arc<RwLock<Vec<Arc<CachedQuery>>>>,
    engine: QueryEngine,
    capture_cache: HashMap<u32, Vec<CaptureEntry>>, // line → captures
}
```

### Methods:
- `full_repaint(root, source, lang, language) -> Vec<HighlightDelta>`:
  1. Clear cache entirely
  2. Run queries on full tree
  3. Group captures by line
  4. Return all lines as deltas, populate cache

- `repaint_ranges(root, source, lang, language, changed_ranges) -> Vec<HighlightDelta>`:
  1. For each `Range` in `changed_ranges`:
     - Compute `start_line`..`end_line` from `Range.start_point.row` / `end_point.row`
     - Clear cache entries for those lines
     - Run query cursor with `set_byte_range(range.start_byte..range.end_byte)`
     - Collect captures, group by line
     - Compare with old cache (already cleared, so always "changed")
     - Update cache, add to deltas
  2. Return merged deltas

- Helper `collect_captures(cursor, query, root, source, start_line..=end_line) -> HashMap<u32, Vec<CaptureEntry>>`

### Note on Rope:
Do NOT call `rope.to_string()`. The `apply_highlights` method already takes `source: &[u8]`. The caller (`BufferState`) will provide a slice from the Rope. For incremental repaint, we can pass the full source — the query cursor's `set_byte_range` limits what it scans.

---

## Phase 3: Delta types + RPC (stdout, not stdin)

**Files:** `src/editor/events.rs`, `src/editor/rpc.rs`

### In `src/editor/events.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightDeltaRpc {
    pub line: u32,
    pub captures: Vec<CaptureEntryRpc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureEntryRpc {
    pub start_col: u32,
    pub end_col: u32,
    pub hl_group: String,
}
```

### In `src/editor/rpc.rs`:
1. Replace `send_notification()` with LSP-style Content-Length + JSON to **stdout**
2. Add `send_highlight_delta(buffer_id, version, deltas)`:
```rust
pub fn send_highlight_delta(
    &self,
    buffer_id: u64,
    version: u64,
    deltas: Vec<HighlightDeltaRpc>,
) -> Result<(), String> {
    let msg = serde_json::json!({
        "method": "xylem.highlights.delta",
        "params": {
            "buffer_id": buffer_id,
            "version": version,
            "deltas": deltas,
        }
    });
    self.send_json(&msg)
}
```

3. Add `send_json()` helper — writes to **stdout**:
```rust
fn send_json(&self, msg: &serde_json::Value) -> Result<(), String> {
    let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
    handle.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    handle.flush().map_err(|e| e.to_string())?;
    Ok(())
}
```

---

## Phase 4: Connect pipeline (no Rope::to_string)

**File:** `src/runtime/state.rs`

### BufferState changes:
1. Add `compute_highlights(&mut self) -> Vec<HighlightDelta>`:
   - Uses `self.buffer.to_vec()` or iterates chunks — NOT `to_string()`
   - Actually, Rope's `to_vec()` also allocates. Better: collect chunks into a Vec<u8> once, reuse.
   - Or: add a `source_bytes: Vec<u8>` field to BufferState, keep it in sync with Rope

### Option: Keep a `Vec<u8>` alongside Rope
```rust
pub struct BufferState {
    pub buffer: Rope,
    source_bytes: Vec<u8>,  // kept in sync with buffer
    parser: IncrementalParser,
    highlight_engine: HighlightEngine,
    is_dirty: bool,
    version: u64,
    dirty_regions: Vec<DirtyRegion>,
}
```

On `apply_change`: update both Rope and `source_bytes`.
On `set_text`: update both.

This avoids O(n) allocation on every highlight call.

### compute_highlights:
```rust
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
```

### RuntimeState changes:
`apply_change()` returns `Option<Vec<HighlightDelta>>`:
```rust
pub fn apply_change(&mut self, change: &EditorEvent) -> Option<Vec<HighlightDelta>> {
    // ... existing logic ...
    Some(state.compute_highlights())
}
```

---

## Phase 5: Lua — nvim_buf_attach + delta handler

**File:** `lua/xylem/init.lua`

### Use nvim_buf_attach with on_bytes
Instead of computing diff from TextChanged, use Neovim's byte-level change notifications:

```lua
local attached_buffers = {}

function M.attach_buffer(buf_id)
    M.send_message({
        method = "xylem.attach",
        params = { buffer_id = buf_id },
    })

    if not attached_buffers[buf_id] then
        attached_buffers[buf_id] = true
        vim.api.nvim_buf_attach(buf_id, false, {
            on_bytes = function(_, buf, changedtick, start_row, start_col, old_end_row, old_end_col, new_end_row, new_end_col, byte_extent)
                -- byte_extent = { old_bytes, new_bytes } or similar
                -- Actually on_bytes callback signature:
                -- (bufnr, changedtick, start_row, start_col, old_end_row, old_end_col, new_end_row, new_end_col)
                -- We compute byte positions using nvim_buf_get_offset
                local start_byte = vim.api.nvim_buf_get_offset(buf, start_row) + start_col
                local old_end_byte = vim.api.nvim_buf_get_offset(buf, old_end_row) + old_end_col
                -- Get the new text
                local new_text = table.concat(vim.api.nvim_buf_get_text(buf, start_row, start_col, new_end_row, new_end_col, {}), "\n")
                if new_end_row ~= start_row then
                    new_text = new_text .. "\n"
                end

                M.send_message({
                    method = "xylem.change",
                    params = {
                        buffer_id = buf,
                        start_byte = start_byte,
                        old_end_byte = old_end_byte,
                        new_text = new_text,
                    },
                })
            end,
        })
    end
end
```

### Delta handler:
```lua
elseif method == "xylem.highlights.delta" then
    M.apply_highlight_delta(params)
```

```lua
function M.apply_highlight_delta(params)
    local buf = params.buffer_id
    if not vim.api.nvim_buf_is_loaded(buf) then
        return
    end

    local ns = M.hl_ns or vim.api.nvim_create_namespace("xylem")
    M.hl_ns = ns

    for _, delta in ipairs(params.deltas) do
        vim.api.nvim_buf_clear_namespace(buf, ns, delta.line, delta.line + 1)
        for _, cap in ipairs(delta.captures) do
            vim.api.nvim_buf_add_highlight(buf, ns, cap.hl_group,
                delta.line, cap.start_col, cap.end_col)
        end
    end
end
```

### Remove old TextChanged autocmd
Replace the `TextChanged`/`TextChangedI` autocmd with just `BufEnter` for attachment (which triggers `nvim_buf_attach`).

---

## File Change Summary

| File | Changes |
|------|---------|
| `src/parser/incremental.rs` | previous_tree, has_parsed, changed_ranges(), is_first_parse() |
| `src/features/highlight.rs` | CaptureEntry, HighlightDelta, capture_cache, repaint_ranges(), full_repaint() |
| `src/editor/events.rs` | HighlightDeltaRpc, CaptureEntryRpc |
| `src/editor/rpc.rs` | send_json() to stdout, send_highlight_delta(), replace send_notification |
| `src/runtime/state.rs` | source_bytes field, compute_highlights(), apply_change returns deltas |
| `src/main.rs` | Route xylem.change to EditorEvent::Change, send deltas to stdout |
| `lua/xylem/init.lua` | nvim_buf_attach with on_bytes, apply_highlight_delta, remove TextChanged |
