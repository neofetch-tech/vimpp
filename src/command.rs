//! Ex commands (`:w`, `:q`, `:bd`, `:set`, `:nnoremap`, ...) and the
//! command-line completion candidates for the popup.

use std::path::Path;

use crate::app::{App, Mapping, Mode};
use crate::config::apply_set;

pub fn execute(app: &mut App, raw: &str) {
    let s = raw.trim();
    if s.is_empty() {
        return;
    }
    let (head, rest) = match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim()),
        None => (s, ""),
    };

    // 1) Lua-registered commands win (users extend Vim++ natively).
    if let Some(host) = app.lua.as_mut() {
        if let Some(action) = host.run_command(head, rest) {
            if !action.is_empty() {
                app.run_action(&action);
            }
            return;
        }
    }

    // 2) Built-ins.
    match head {
        "w" | "write" => {
            if rest.is_empty() {
                app.save();
            } else {
                app.save_as(rest);
            }
        }
        "q" | "quit" => app.close_active(false),
        "q!" | "quit!" => app.close_active(true),
        "x" | "exit" | "wq" => {
            let saved = if app.buf().dirty {
                if rest.is_empty() {
                    app.save_succeeded()
                } else {
                    app.save_as(rest);
                    !app.buf().dirty
                }
            } else {
                true
            };
            if saved {
                app.close_active(true);
            }
        }
        "e" | "edit" | "open" => {
            if rest.is_empty() {
                app.set_message("E32: No file name");
            } else {
                app.open_path(Path::new(rest));
                app.mode = Mode::Normal;
            }
        }
        "enew" => app.new_buffer(),
        "bd" | "bdelete" => {
            if rest.is_empty() {
                app.close_active(false);
            } else if let Ok(n) = rest.trim_end_matches('!').parse::<usize>() {
                if n > 0 && n <= app.buffers.len() {
                    let dirty = app.buffers[n - 1].dirty;
                    if dirty && !rest.ends_with('!') {
                        app.set_message("E37: No write since last change (add ! to override)");
                    } else {
                        app.buffers.remove(n - 1);
                        if app.buffers.is_empty() {
                            app.should_quit = true;
                        } else {
                            app.active = app.active.min(app.buffers.len() - 1).saturating_sub(1);
                        }
                    }
                } else {
                    app.set_message("E86: Invalid buffer number");
                }
            }
        }
        "bn" | "bnext" => app.cycle_buffer(true),
        "bp" | "bprev" | "bprevious" => app.cycle_buffer(false),
        "b" | "buffer" => {
            if let Ok(n) = rest.parse::<usize>() {
                if n > 0 && n <= app.buffers.len() {
                    app.active = n - 1;
                } else {
                    app.set_message("E86: Invalid buffer number");
                }
            } else if let Some(i) = app
                .buffers
                .iter()
                .position(|b| b.name().starts_with(rest) && !rest.is_empty())
            {
                app.active = i;
            } else {
                app.set_message("E94: No matching buffer");
            }
        }
        "set" | "se" | "setlocal" => apply_set(app, rest),
        "nnoremap" | "nmap" => add_mapping(app, 'n', rest),
        "inoremap" | "imap" => add_mapping(app, 'i', rest),
        "vnoremap" | "vmap" => add_mapping(app, 'v', rest),
        "map" | "noremap" => add_mapping(app, 'a', rest),
        "unmap" => {
            app.maps.retain(|m| m.lhs != rest.trim());
            app.set_message("Mapping removed");
        }
        "colorscheme" | "colo" => {
            app.set_message("Vim++ uses the Nordic color palette");
        }
        "split" | "sp" | "vsplit" | "vs" => {
            app.set_message("vsplit arrives in 0.2 — buffers & tabs carry the load for now");
        }
        "help" | "h" | "tutor" | "Tutor" => {
            app.help_open = true;
            app.help_scroll = 0;
        }
        "lua" => {
            let Some(host) = app.lua.as_mut() else {
                app.set_message("Lua runtime unavailable");
                return;
            };
            match host.eval(rest) {
                Ok(Some(action)) => {
                    if !action.is_empty() {
                        app.run_action(&action);
                    }
                }
                Ok(None) => {}
                Err(e) => app.set_message(format!("lua: {e}")),
            }
        }
        "!" | "terminal" => {
            app.set_message("Shell passthrough lands with 0.2");
        }
        _ => {
            app.set_message(format!("E492: Not an editor command: {head}"));
        }
    }
}

fn add_mapping(app: &mut App, mode: char, rest: &str) {
    let mut parts = rest.split_whitespace();
    let Some(lhs) = parts.next() else {
        app.set_message("Usage: :nnoremap keys command");
        return;
    };
    let rhs = parts.collect::<Vec<_>>().join(" ");
    if rhs.is_empty() {
        app.set_message("Usage: :nnoremap keys command");
        return;
    }
    app.maps
        .retain(|m: &Mapping| !(m.lhs == lhs && m.mode == mode));
    app.maps.push(Mapping {
        mode,
        lhs: lhs.to_string(),
        rhs,
    });
    app.set_message("Mapping added");
}

/// Candidates for the `:` popup. Returns FULL command strings (the whole
/// typed line may be replaced by a choice).
pub fn candidates(cmd: &str) -> Vec<String> {
    const ROOTS: &[&str] = &[
        "w",
        "wq",
        "q",
        "q!",
        "x",
        "e",
        "enew",
        "bd",
        "bn",
        "bp",
        "b",
        "set",
        "nnoremap",
        "inoremap",
        "vnoremap",
        "map",
        "unmap",
        "colorscheme",
        "split",
        "vsplit",
        "help",
        "lua",
    ];
    let cmd = cmd.trim_start();
    if cmd.is_empty() {
        return Vec::new();
    }
    let head = cmd.split_whitespace().next().unwrap_or("");
    let has_space = cmd.contains(' ');

    if has_space {
        let arg = cmd[head.len()..].trim_start();
        match head {
            "set" | "se" | "setlocal" => arg_options(head, arg),
            "colorscheme" | "colo" => vec!["colorscheme nordic".to_string()],
            _ => Vec::new(),
        }
    } else {
        ROOTS
            .iter()
            .filter(|root| root.starts_with(head) && !head.is_empty())
            .map(|root| root.to_string())
            .collect()
    }
}

fn arg_options(head: &str, arg: &str) -> Vec<String> {
    const OPTS: &[&str] = &[
        "number",
        "relativenumber",
        "tabstop",
        "expandtab",
        "nonumber",
        "norelativenumber",
        "noexpandtab",
    ];
    OPTS.iter()
        .filter(|o| o.starts_with(arg))
        .map(|o| format!("{} {}", head, o))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_completion() {
        let c = candidates("w");
        assert!(c.contains(&"w".to_string()));
        assert!(c.contains(&"wq".to_string()));
        assert!(!c.iter().any(|s| s.contains("bd")));
    }

    #[test]
    fn set_completion() {
        let c = candidates("set rel");
        assert_eq!(c, vec!["set relativenumber".to_string()]);
    }
}
