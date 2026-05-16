# Xylem Command and Grammar Reference

Xylem implements a specialized management system for Tree-sitter grammars and queries, replacing standard Neovim Tree-sitter workflows with a high-performance Rust-based backend. This system enables intelligent synchronization, lazy loading, and deep integration via RPC.

---

## Neovim Commands

Xylem provides several commands with intelligent autocompletion for managing the parsing environment.

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `:XylemInstall` | `<lang>` | Downloads and synchronizes queries for a specific language. |
| `:XylemUpdate` | `[<lang>]` | Updates one specific language or all installed languages if no argument is provided. |
| `:XylemSync` | None | Performs a bulk synchronization of all supported grammars (200+) in the background. |
| `:XylemInfo` | None | Displays current system status, including version, active buffers, and backend state. |

### Intelligent Autocompletion
The `:XylemInstall` and `:XylemUpdate` commands query the Rust backend in real-time. Suggestions are dynamically generated based on the internal grammar registry, ensuring you only try to install supported languages.

---

## Grammar Management

### Installing a Language
To install a specific grammar (e.g., Rust):
```vim
:XylemInstall rust
```
Xylem will notify you of the progress via `vim.notify`. Once complete, the queries are stored in your local data directory (e.g., `~/.local/share/xylem/queries/rust`).

### Bulk Synchronization
For a complete local setup of all 200+ supported grammars:
```vim
:XylemSync
```
This runs as a background process. The `SyncManager` handles cloning and query extraction without blocking the editor.

### Manual Query Management
Queries (`.scm` files) are located in the Xylem data directory.
* **Linux:** `~/.local/share/xylem/queries/`
* **macOS:** `~/Library/Application Support/xylem/queries/`
* **Windows:** `%AppData%\xylem\queries\`

**Note on Modifications:** If you manually edit a `.scm` file in these directories, you must restart Neovim to see the changes. Xylem's `HighlightEngine` caches compiled queries in memory for performance.

---

## The Synchronization Engine

Xylem uses a **Reactive RPC Model** to handle grammars efficiently.

### Lazy Loading Workflow
1.  **Detection:** When you open a file, the Lua plugin identifies the filetype.
2.  **Request:** The plugin checks if the required grammar is available. If not, it sends an `RpcRequest::Install` to the Rust backend.
3.  **Backend Fetch:** The `ParserInstaller` (Rust) downloads the source tarball and compiles only the requested grammar.
4.  **Query Loading:** The `QueryLoader` retrieves the corresponding `.scm` files and populates the `QueryEngine` cache.

### Backend Components
*   **`SyncManager`:** Orchestrates bulk downloads and repository synchronization.
*   **`ParserInstaller`:** Handles the compilation of Tree-sitter grammars into shared objects.
*   **`QueryEngine`:** Manages the in-memory `DashMap` cache of compiled Tree-sitter queries.

---

## Technical Internals

### RPC Message Types
Xylem uses Msgpack-RPC over standard streams. Key message types for grammar management include:

*   `SyncAll`: Triggers a full synchronization of the registry.
*   `SyncOne(lang)`: Synchronizes a specific language.
*   `Info`: Requests backend status and versioning.
*   `GetGrammars`: Returns the list of supported grammars for Lua-side autocompletion.

### Caching and Performance
To minimize overhead, Xylem avoids frequent disk I/O:
- **Compiled Queries:** Cached in `DashMap` within the Rust process.
- **Incremental Updates:** The system is designed to favor incremental `InputEdit` applications over full re-parses whenever possible.

---

## Current Status and Roadmap
*   **File Monitoring:** Automatic reloading of queries when disk changes are detected is currently planned but not implemented.
*   **Registry Expansion:** The list of 200+ grammars is managed in `src/runtime/list.rs`.
