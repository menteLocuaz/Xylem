# Xylem - Incremental Parser for Neovim 0.11+

Minimal Lua plugin that integrates with a Rust-based incremental parser via RPC/stdio. 

## Arquitectura

```
Neovim 0.11
   ↓
Lua plugin (init.lua)
   ↓
JSON via stdin/stdout
   ↓
Rust runtime (xylem)
   ↓
Tree-sitter incremental parser + Automated Grammar Installer
```

## Estructura del Proyecto

```
xylem/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point + RPC server
│   ├── parser/
│   │   ├── incremental.rs   # Incremental tree-sitter parser
│   │   ├── diff.rs          # Diff computation para edits
│   │   ├── registry.rs      # Grammar registry
│   │   ├── installer.rs     # Async parser installer
│   │   └── queries.rs       # Tree-sitter query engine
│   ├── runtime/
│   │   ├── state.rs         # Buffer state management
│   │   └── ...
│   ├── editor/
│   │   ├── events.rs        # Editor events
│   │   └── rpc_server.rs    # RPC handler
│   └── ...
├── lua/xylem/
│   ├── init.lua             # Plugin initialization
│   └── ...
└── ...
```

## Building

```bash
cargo build --release
```

## Usage con Lazy.nvim

En tu config de plugins (~/.config/nvim/lua/plugins/xylem.lua):

```lua
return {
    dir = "~/projects/xylem",
    lazy = false,
    config = function()
        require("xylem").start()
    end,
}
```

## Automated Parser Installation

Xylem replaces `:TSInstall` and `:TSUpdate`. You can manage language grammars directly through the Rust backend.

### API
- `xylem.install`: Install a language grammar asynchronously.
  - Arguments: `name`, `repo`, `revision`, `queries`.

The system automatically downloads, compiles, and installs dynamic libraries (`.so`/`.dylib`/`.dll`) into your Neovim data directory (`stdpath("data")/site/xylem/parsers`).

## Features Implementadas

- [x] Parser incremental con tree-sitter
- [x] Highlights basicos (Function, Variable, Number, etc)
- [x] RPC via stdin/stdout
- [x] Automated Async Grammar Installer
- [x] Buffer state management
- [x] Autocmds para TextChanged/TextChangedI

## Features Pendientes

- [ ] Incremental real (InputEdit con posiciones correctas)
- [ ] Extmarks para highlights reales en Neovim
- [ ] Query cache
- [ ] Scheduler async
- [ ] Rope incremental (evitar to_string())
