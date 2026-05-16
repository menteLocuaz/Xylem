Based on the current implementation in src/runtime/ and the project roadmap, here are the answers to your questions about managing .scm files (Tree-sitter queries):

1. How do I download a specific .scm file without modifying anything?

Currently, Xylem offers two ways to "download" or synchronize queries:

* Bulk synchronization: Running `xylem --sync` from the CLI activates `SyncManager::sync_all` (in src/runtime/sync.rs). This clones the repositories of the 200+ grammars defined

in src/runtime/list.rs and copies their .scm files to your local data directory (typically ~/.local/share/xylem/queries/).

* On-demand (RPC): The backend supports an RPC installation request (managed in src/editor/rpc_server.rs). When the Neovim plugin requests a specific language,

the ParserInstaller (in src/parser/installer.rs) downloads the source tarball and compiles only that specific grammar and its queries.

If you want to manually download the queries for a specific language without performing a full synchronization, you can use the logic of SyncResult or trigger it with a Lua call in Neovim:

1 -- Example: Download only PHP

2 require('xylem').install('php', 'https://github.com/tree-sitter/tree-sitter-php', 'master', {})

2. If I modify one of these .scm files in the folder, will the change be applied?

No, not immediately.

* The HighlightEngine (in src/features/highlight.rs) uses a QueryEngine that caches the compiled queries in a DashMap (src/parser/queries/engine.rs).

* Once Tree-sitter loads and compiles a query, it remains in memory. Currently, there is no file monitoring system to detect changes on disk.

* To apply changes, you must restart the Xylem process (usually by restarting Neovim), which clears the in-memory cache and forces a reload from disk.

3. How does Neovim use lazy loading to determine which language to download?

Xylem uses a reactive RPC model for lazy loading:

1. Detection: When a file is opened, the Neovim Lua plugin detects the file type.

2. Request: The plugin checks if Xylem has the parser. If not, it sends an RpcRequest::Install request to the Rust backend containing the GrammarSpec (name, repository URL, and revision).

3. Backend Fetch: The backend's ParserInstaller downloads and compiles only that specific grammar. 4. Query Loading: The QueryLoader (located in src/parser/queries/loader.rs) looks for .scm files in the runtime path or the Xylem data directory specifically for that language.

This prevents the backend from having to download or load the entire catalog of over 200 languages ​​on startup. Note that the large src/runtime/queries/ folder in the source code tree is a fallback option; in production, Xylem prefers queries downloaded to its data directory or provided by your Neovim configuration.

Current Status: As indicated in the project roadmap, formalizing the integration between QueryLoader and QueryEngine is still pending, so some of this logic (specifically the automatic disk-to-cache reloading) is still being finalized.