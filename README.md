# Xylem - Incremental Parser for Neovim 0.11+

Plugin Lua mínimo que se integra con un parser incremental en Rust via RPC/stdio.

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
Tree-sitter incremental parser
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
│   │   └── queries.rs       # Tree-sitter query engine
│   ├── runtime/
│   │   ├── state.rs         # Buffer state management
│   │   ├── scheduler.rs     # Task scheduler
│   │   └── cache.rs         # Query and buffer cache
│   ├── editor/
│   │   ├── events.rs        # Editor events (change, save, etc)
│   │   ├── buffer.rs        # Buffer abstraction
│   │   └── rpc.rs           # RPC handler
│   └── features/
│       ├── highlight.rs     # Syntax highlighting
│       └── indent.rs        # Indentation engine
├── lua/xylem/
│   ├── init.lua             # Plugin entry point
│   ├── rpc.lua              # RPC utilities
│   └── highlights.lua       # Highlight management
├── plugin/xylem.lua       # Neovim plugin loader
├── queries/lua/
│   └── highlights.scm       # Tree-sitter queries para Lua
└── lazy.lua                 # Lazy.nvim config
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

## Features Implementadas

- [x] Parser incremental con tree-sitter
- [x] Highlights basicos (Function, Variable, Number, etc)
- [x] RPC via stdin/stdout
- [x] Buffer state management
- [x] Autocmds para TextChanged/TextChangedI

## Features Pendientes

- [ ] Incremental real (InputEdit con posiciones correctas)
- [ ] Extmarks para highlights reales en Neovim
- [ ] Query cache
- [ ] Scheduler async
- [ ] Rope incremental (evitar to_string())