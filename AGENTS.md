# Xylem - AGENTS.md

## Build Commands
- `cargo build --release` - Build the Rust binary to `target/release/xylem`
- `cargo run` - Run standalone demo (prints AST and highlights)
- `cargo run -- --rpc` - Run in RPC server mode (how Neovim uses it)
- `cargo run -- --sync` - Run sync mode (alternative RPC mode)

## Neovim Plugin Integration
- **Requires**: Neovim 0.11+ (uses `vim.uv`, modern Lua APIs)
- **Binary path**: `vim.fn.stdpath("data")/xylem/target/release/xylem`, falls back to `./target/release/xylem`
- **File filter**: Only activates on `*.lua` buffers (autocmds in `lua/xylem/init.lua:105`)
- **lazy.nvim config**: `build = "cargo build --release"` triggers on plugin load

## RPC Protocol (LSP-style)
- JSON messages over stdin/stdout with `Content-Length` header
- **Inbound methods**: `xylem.attach`, `xylem.detach`, `xylem.change`, `xylem.parse`
- **Outbound methods**: `xylem.highlights` (returns syntax highlight ranges)
- Protocol defined in `src/main.rs:26-54` and `lua/xylem/init.lua:92-100`

## Key Files
- `src/main.rs` - Rust entry point, RPC server, message dispatch
- `lua/xylem/init.lua` - Neovim plugin entry, process management
- `lua/xylem/rpc.lua` - RPC utilities (not yet imported by init.lua)
- `queries/lua/highlights.scm` - Tree-sitter highlight queries
- `lazy.lua` - Lazy.nvim plugin spec for local development

## Architecture Notes
- Rust uses `tokio` for async but actual I/O uses `BufReader` on stdin/stdout
- `ropey` for efficient text storage in Rust
- `parking_lot::RwLock` for thread-safe state access
- Incremental parsing goal: `InputEdit` with correct byte positions (not fully implemented)

## Uncommitted Files
The `.agents/` directory and `skills-lock.json` are gitignored. Do not commit agent configurations.
