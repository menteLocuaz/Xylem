# Xylem Project Guidelines

## Project Overview
Xylem is a high-performance incremental parser for Neovim 0.11+, implemented as a Neovim plugin with a Rust-based backend. It leverages Tree-sitter for efficient, incremental code analysis, offloading heavy parsing and query execution from the main Neovim thread to a dedicated Rust process communicating via JSON over RPC (stdin/stdout).

### Core Technologies
- **Rust Backend:**
  - `tree-sitter`: Core parsing engine.
  - `tokio`: Async runtime (used for RPC and task management).
  - `serde` & `serde_json`: Message serialization.
  - `ropey`: Efficient text manipulation via ropes.
  - `parking_lot`: High-performance synchronization primitives.
- **Neovim Frontend:**
  - `Lua`: Plugin logic and Neovim API integration.
  - `RPC`: Custom JSON-based protocol over standard streams.

## Project Structure
- `src/`: Rust source code.
  - `main.rs`: Application entry point and RPC server implementation.
  - `parser/`: Tree-sitter integration, incremental updates, and diffing.
  - `runtime/`: System state management, caching, and scheduling.
  - `editor/`: Editor abstractions (buffers, events) and RPC handling.
  - `features/`: Specific editor features like syntax highlighting and indentation.
- `lua/xylem/`: Neovim Lua plugin.
  - `init.lua`: Plugin initialization and process management.
  - `highlights.lua`: Logic for applying Tree-sitter highlights to Neovim buffers.
  - `rpc.lua`: Utilities for communicating with the Rust backend.
- `queries/`: Tree-sitter query files (`.scm`) for different languages (currently focused on Lua).
- `plugin/`: Neovim plugin entry point.

## Building and Running
### Prerequisites
- Rust toolchain (stable)
- Neovim 0.11+

### Build
```bash
cargo build --release
```

### Development Execution
You can run the Rust backend in standalone mode for debugging:
```bash
cargo run
```
To run it in RPC mode (as the plugin does):
```bash
cargo run -- --rpc
```

### Integration
The plugin expects the binary to be at `./target/release/xylem` or in the Neovim data directory. For development, ensure the binary is built before starting Neovim.

## Development Conventions
- **Incrementalism:** Prioritize incremental updates over full re-parses whenever possible.
- **RPC Protocol:** Communication is asynchronous and JSON-encoded. New methods should be documented in `src/main.rs` and `lua/xylem/init.lua`.
- **Modularity:** Keep editor-specific logic in `src/editor/` and language-specific features in `src/features/`.
- **Performance:** Use `ropey` for text storage to avoid expensive string allocations during edits.

## Current Status & Roadmap
The project is in active early development. Key features like full incremental `InputEdit` support, advanced query caching, and an asynchronous scheduler are planned. Many internal structures are currently under development (indicated by compiler warnings for unused code).
