<div align="center">

<img src="assets/logo.png" alt="Vim++ logo" width="220" />

# Vim++

**The millisecond modal editor.** Rust + ratatui, native Lua plugins, Nordic theme.

[![Rust](https://img.shields.io/badge/rust-1.78%2B-%23b6ff3b?style=flat-square)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-%23b6ff3b?style=flat-square)](LICENSE)
[![Startup](https://img.shields.io/badge/startup-%3C%2015%20ms-%23b6ff3b?style=flat-square)](#-startup)

*Українська версія: [README.uk.md](README.uk.md)*

</div>

---

## Why Vim++

Vim++ boots in **single-digit milliseconds** — the entire editor is one Rust binary; there is no plugin host process to spawn, no daemon handshake, no autoload soup. When you run `vimpp src/main.rs`, you are editing before Vim would have finished sourcing its runtime.

Yet you keep the Neovim workflow: **modal editing, tabs, `:commands`, and plugins — natively scripted in Lua inside the editor process** (via [mlua](https://crates.io/crates/mlua)). No RPC, no bridge, no latency.

| | Vim++ | Vim | Neovim |
|---|---|---|---|
| Startup | **~5–15 ms** | 30–150 ms | 40–300 ms |
| scripting     | Lua, in-process | Vimscript | Lua via RPC |
| UI            | ratatui (GPU-free, everywhere) | curses | curses |
| Memory idle   | ~10 MB | ~10 MB | ~30 MB |

## Features

- **Modal core** — Normal / Insert / Visual (char & line) / Command, with `hjkl w b e 0 $ ^ G gg`, `x dd dw yy p P i a A o O I`, `J` join, `u` / `<C-r>` (300-deep undo tree), `/search` + `n` / `N`.
- **Line numbers** — absolute line numbers are the default; `:set relativenumber` switches other lines to cursor-relative distances.
- **Tabs → buffers**, `gt` / `gT`, `<C-n>` / `<C-p>`, `:b[!]`, `:bd`, `:bn`, `:bp`, `:e file`.
- **File explorer** — `Ctrl-b` toggles/focuses; `j/k` navigate, `Enter`/`l` opens files and dives into dirs, `h` climbs.
- **Syntax highlighting** — built-in zero-dependency lexer for Rust, JS/TS, Python, C, Go, Lua, HTML, CSS, JSON, TOML, Markdown, SQL, YAML, Shell. Token offsets are Unicode-safe.
- **Insert-mode word completion** — buffer-scoped popup, `<Tab>` accept (as-is), `<C-n>`/`<C-p>` cycle — like the reference screenshot.
- **Command-line completion popup** — `:set <Tab>` cycles `number`, `relativenumber`, `tabstop`, ...
- **Theming** — Nordic palette; colors remain overridable from `config.toml`. Set your terminal font to **JetBrains Mono** for the intended typography.
- **Help** — click `[ HELP ]`, press `F1`, or type `:help` for the full in-app guide.
- **Native Lua plugins** — see below.

## Startup

`vimpp` times `main()` → first frame and reports it on exit:

```
⚡ Vim++ ready — booted in 6.1 ms. NamASTE ✌
```

No plugins on disk = no plugin cost. Lua is vendored into the binary; `Lua::new()` costs microseconds.

## Install

```sh
# build from source (Rust 1.78+)
git clone https://github.com/your-handle/vimpp && cd vimpp
cargo build --release
./target/release/vimpp src/main.rs
```

## Keys (v0.2 cheat sheet)

| Mode | Keys |
|---|---|
| Enter modes | `i`, `I`, `a`, `A`, `o`, `O`, `v`, `V`, `:`, `/` |
| Move | `h j k l`, `w b e`, `0 $ ^`, `gg G`, arrows |
| Edit | `x dd dw`, `y p P`, `J` (join), `u <C-r>` |
| Visual | `v`/`V`, `d`, `y`, `c`, `o` types toggle via `v`/`V` |
| Search | `/text⏎`, then `n` / `N` |
| Buffers | `gt`, `gT`, `<C-n>`, `<C-p>`, `:bn`, `:bp`, `:b 2`, `:bd!` |
| Files | `<C-b>` (explorer), `:e path`, `:w [file]`, `:wq`, `:x` |
| Options | `:set number`, `:set nonumber`, `:set tabstop=2`, `:set expandtab!` |
| Completion | `<Tab>`, `<C-n>`, `<C-p>` (insert mode); `<Tab>` on `:` line |

## Native Lua plugins

Everything you'd script in Neovim day-to-day works **in-process**:

```lua
-- ~/.vimplus/init.lua
vimpp.echo("my config is loading…")

vimpp.keymap.set("n", "Y", "y$")            -- string RHS = raw key feed
vimpp.keymap.set("n", "<C-t>", function()   -- function RHS = return an action
  return ":set nu!"                          -- ":cmd" — run ex command
end)

vimpp.command("Greet", function(args)       -- your own ex command
  return "!Hello, " .. (args or "world")    -- "!text" = echo message
end)

vim.cmd(":set relativenumber")              -- deferred until init.lua returns
vimpp.set_option("tabstop", 2)
```

**Action protocol**: a handler returns `":ex command"`, `!"echo text"`, or a raw key
sequence (`"gg"`, `"iHello"`, `"<Esc>"`...). Return `nil` = "handled, no action".
At load time `vimpp.echo/set_option/exec` & `vim.cmd` are **buffered** and applied
right after your script returns — so plugins can compose without re-entrancy guts.
`vim.api.nvim_set_keymap`, `vim.o`, `vim.g` are shimmed for drop-in familiarity.

Settings live in two happy places:

```
~/.vimplus/config.toml   # Rust side: theme colors, number/tabstop/expandtab
~/.vimplus/init.lua      # Lua side: keymaps, commands, your wildest ideas
```

## Architecture

```
src/
├── main.rs              # boot timer, event loop (poll 120 ms, zero wake-storms)
├── buffer.rs            # rope (Ropey) + cursor model + undo/redo + tests
├── app.rs               # modes, dispatch, registers, completion, selections
├── command.rs           # ex commands + ':' popup candidates
├── keys.rs              # Neovim notation (<C-s>) + RHS → synthesized events
├── config.rs            # tiny TOML for theme/options
├── lua.rs               # Lua host (vendored Lua 5.1, single process)
├── explorer.rs          # sidebar file tree
├── highlight.rs         # Unicode-safe token lexer, 14 languages
├── theme.rs             # Nordic palette + user overrides
├── ui.rs                # ratatui rendering, tabs, popups, status line
└── vimplus_bootstrap.lua # the `vimpp` API exposed to user scripts
```

Design rules: **no allocation in the render path**, **no blocking I/O on keys**,
**no plugin process** — the three things that make editors slow.

## Roadmap

- [ ] 0.2 — splits (`:vsplit`), marks, `>{`-style text objects, registers view
- [ ] 0.3 — tree-sitter layer on top of the existing token renderer
- [ ] 0.4 — LSP client in-process, fuzzy finder (`:find`), internal
   separator-less line cache for 1M-line buffers (already rope-ready)

## Try it

Install Rust and a native C toolchain used by the vendored Lua dependency, then run:

```sh
cargo run -- src/main.rs
```

Press `F1` (or click `[ HELP ]`) to see all controls. A quick editor walkthrough: use `Ctrl-B` to open the project tree, open a file with `Enter`, press `i`, type a short comment, press `Esc`, then `:w` to save. Try `/fn main` and `n` to search. `:q` closes the buffer.

## GitHub builds and releases

`.github/workflows/build.yml` builds Windows and Linux binaries on every push. To publish downloadable assets, push a version tag after updating the package version in `Cargo.toml`:

```sh
git tag v0.2.0
git push origin v0.2.0
```

The workflow creates a GitHub Release with `vimpp-linux-x86_64` and `vimpp-windows-x86_64.exe`. View build logs in the repository's **Actions** tab.

## License

MIT — see [LICENSE](LICENSE). NamASTE ✌
