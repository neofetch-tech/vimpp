//! The native Lua plugin host (Lua 5.1 via mlua, vendored).
//!
//! User code lives in `~/.vimplus/init.lua` and hooks into editor events
//! WITHOUT crossing process boundaries — a full Neovim-style script host,
//! single-process, zero startup cost beyond a `Lua::new()` (~50 µs).
//!
//! Contract (documented in README):
//!  * `vimpp.keymap.set(mode, lhs, rhsFn)` / `vim.keymap.set(...)`
//!  * `vimpp.command(name, fn)` — custom ex commands, `fn(args) -> action`
//!  * `vimpp.echo(msg)` / `vimpp.set_option(k, v)` / `vim.cmd(":w")`
//!    are buffered at load time and applied after the script returns.
//!  * Handlers and commands RETURN an action string that the editor runs:
//!      ":bd" → ex command; "!text" → echo message; "gg" → raw keys.
//!    Returning nil means "handled, do nothing".

use mlua::{Lua, Table, Value};

pub struct LuaHost {
    lua: Lua,
}

/// Everything the bootstrap collected while scripts were running.
pub struct LoadResult {
    pub log: Vec<String>,
    pub options: Vec<String>,  // "key=value" lines
    pub commands: Vec<String>, // deferred ex commands (vim.cmd)
    pub errors: Vec<String>,
}

impl LuaHost {
    pub fn new() -> Result<Self, String> {
        Ok(Self { lua: Lua::new() })
    }

    pub fn bootstrap(&mut self, src: &str) -> Result<(), String> {
        self.lua
            .load(src)
            .exec()
            .map_err(|e| format!("bootstrap: {}", e))
    }

    /// Run a user script (`init.lua`).
    pub fn run_script(&mut self, src: &str, label: &str) -> Result<(), String> {
        self.lua
            .load(src)
            .exec()
            .map_err(|e| format!("{label}: {e}"))
    }

    /// Collect `vimpp._out` accumulated during the last script execution.
    pub fn collect(&self) -> Result<LoadResult, String> {
        let globals = self.lua.globals();
        let read_list = |name: &str| -> Vec<String> {
            let Ok(vimpp) = globals.get::<Table>("vimpp") else {
                return Vec::new();
            };
            let Ok(out) = vimpp.get::<Table>("_out") else {
                return Vec::new();
            };
            let Ok(list) = out.get::<Table>(name) else {
                return Vec::new();
            };
            list.sequence_values::<String>()
                .filter_map(|v| v.ok())
                .collect()
        };
        let mut errors = Vec::new();
        if let Ok(Value::String(s)) = globals.get::<Value>("__vimpp_err") {
            if let Ok(text) = s.to_str() {
                errors.push(text.to_string());
            }
        }
        Ok(LoadResult {
            log: read_list("log"),
            options: read_list("options"),
            commands: read_list("commands"),
            errors,
        })
    }

    /// Find a Lua mapping for (mode, notation) and run it.
    /// Returns Some("") when handled with no resulting action.
    pub fn map_lookup(&self, mode: char, notation: &str) -> Option<String> {
        let globals = self.lua.globals();
        let keymaps = globals
            .get::<Table>("vimpp")
            .ok()?
            .get::<Table>("_keymaps")
            .ok()?;
        for m in [mode.to_string(), "a".to_string()] {
            let Ok(value) = keymaps.get::<Value>(format!("{m}|{notation}")) else {
                continue;
            };
            match value {
                Value::Function(f) => return action_of(f),
                Value::String(s) => return Some(s.to_string_lossy()),
                Value::Nil => continue,
                _ => continue,
            }
        }
        None
    }

    #[allow(dead_code)] // exercised by the Lua host tests below
    pub fn has_command(&self, name: &str) -> bool {
        let Ok(vimpp) = self.lua.globals().get::<Table>("vimpp") else {
            return false;
        };
        let Ok(cmds) = vimpp.get::<Table>("_commands") else {
            return false;
        };
        matches!(cmds.get::<Value>(name), Ok(Value::Function(_)))
    }

    /// Run a user-defined ex command with the typed args.
    pub fn run_command(&self, name: &str, args: &str) -> Option<String> {
        let vimpp = self.lua.globals().get::<Table>("vimpp").ok()?;
        let cmds = vimpp.get::<Table>("_commands").ok()?;
        let f = cmds.get::<Value>(name).ok()?;
        match f {
            Value::Function(function) => match function.call::<Value>(args) {
                Ok(Value::Nil) => Some(String::new()),
                Ok(Value::String(s)) => Some(s.to_string_lossy()),
                Ok(_) => Some(String::new()),
                Err(e) => Some(format!("!lua command error: {e}")),
            },
            _ => None,
        }
    }

    /// `:lua <code>` — evaluate and optionally return an action.
    pub fn eval(&self, code: &str) -> Result<Option<String>, String> {
        let value: Value = self.lua.load(code).eval().map_err(|e| e.to_string())?;
        match value {
            Value::Nil => Ok(None),
            Value::String(s) => Ok(Some(s.to_string_lossy())),
            _ => Ok(Some(String::new())),
        }
    }
}

fn action_of(function: mlua::Function) -> Option<String> {
    match function.call::<Value>(()) {
        Ok(Value::String(s)) => Some(s.to_string_lossy()),
        Ok(_) => Some(String::new()), // handled, no action
        Err(e) => Some(format!("!lua error: {e}")),
    }
}

/// The API surface exposed to every user script. Pure Lua — nothing to
/// synchronize at runtime, which keeps dispatch allocation-free.
pub const BOOTSTRAP: &str = include_str!("vimplus_bootstrap.lua");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keymaps_and_commands_roundtrip() {
        let mut host = LuaHost::new().unwrap();
        host.bootstrap(BOOTSTRAP).unwrap();
        host.run_script(
            r#"
            vimpp.keymap.set("n", "<C-t>", function() return "!tagged" end)
            vimpp.keymap.set("n", "Y", "yy")
            vimpp.command("Greet", function(_) return "!hello from lua" end)
            vimpp.echo("loaded demo")
            vimpp.set_option("number", "true")
            vim.cmd("wq")
            "#,
            "test",
        )
        .unwrap();
        assert_eq!(host.map_lookup('n', "<C-t>").as_deref(), Some("!tagged"));
        assert_eq!(host.map_lookup('n', "Y").as_deref(), Some("yy"));
        assert_eq!(host.map_lookup('n', "zz"), None);
        assert!(host.has_command("Greet"));
        assert_eq!(
            host.run_command("Greet", "").as_deref(),
            Some("!hello from lua")
        );
        let result = host.collect().unwrap();
        assert!(result.log.contains(&"loaded demo".to_string()));
        assert!(result.options.contains(&"number=true".to_string()));
        assert!(result.commands.contains(&"wq".to_string()));
    }
}
