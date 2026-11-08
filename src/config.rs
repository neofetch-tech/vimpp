//! `~/.vimplus/config.toml` — theme overrides, editor options, no bloat.
//! A deliberate tiny TOML subset: `key = value` lines and `[section]` headers.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::app::App;

pub fn config_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        PathBuf::from(".vimplus")
    } else {
        PathBuf::from(home).join(".vimplus")
    }
}

/// Load config.toml if present; overrides land on the Theme + Options.
pub fn load(app: &mut App) {
    let path = config_dir().join("config.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return; // zero config is a perfectly valid config
    };
    let mut overrides = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim().trim_matches('"').to_string();

        match key.as_str() {
            "number" | "relativenumber" | "expandtab" => {
                if let Some(b) = parse_bool(&value) {
                    match key.as_str() {
                        "number" => app.options.number = b,
                        "relativenumber" => app.options.relativenumber = b,
                        "expandtab" => app.options.expandtab = b,
                        _ => {}
                    }
                }
            }
            "tabstop" | "ts" => {
                if let Ok(n) = value.parse::<usize>() {
                    app.options.tabstop = n.clamp(1, 16);
                }
            }
            _ => {
                overrides.insert(key, value);
            }
        }
    }
    app.theme.apply_overrides(&overrides);
}

/// Shared parsing for `:set` and config-file values.
pub fn apply_set(app: &mut App, args: &str) {
    for token in args.split_whitespace() {
        let token = token.trim_end_matches(',').trim();
        if token.is_empty() {
            continue;
        }
        // form: name, name=value, name:value
        let (key, value) = match token.split_once(['=', ':']) {
            Some((k, v)) => (k.trim().to_lowercase(), Some(v.trim().to_string())),
            None => (token.to_lowercase(), None),
        };
        let negated = key.starts_with("no");
        if negated {
            let stripped = key.trim_start_matches("no");
            // only strip if the remainder is a known bool option
            if matches!(
                stripped,
                "number" | "nu" | "relativenumber" | "rnu" | "expandtab" | "et"
            ) {
                set_bool(app, alias(stripped), false);
                continue;
            }
        }
        match key.as_str() {
            "number" | "nu" => {
                app.options.number = value.as_deref().and_then(parse_bool).unwrap_or(true)
            }
            "relativenumber" | "rnu" => {
                app.options.relativenumber = value.as_deref().and_then(parse_bool).unwrap_or(true)
            }
            "expandtab" | "et" => {
                app.options.expandtab = value.as_deref().and_then(parse_bool).unwrap_or(true)
            }
            "tabstop" | "ts" => {
                let parsed = value
                    .clone()
                    .or_else(|| token.split_once('=').map(|(_, v)| v.to_string()))
                    .and_then(|v| v.parse::<usize>().ok());
                match parsed {
                    Some(n) => app.options.tabstop = n.clamp(1, 16),
                    None => app.set_message(format!("E521: Number required after ={key}")),
                }
            }
            _ => app.set_message(format!("E518: Unknown option: {key}")),
        }
    }
}

fn alias(key: &str) -> &str {
    match key {
        "nu" => "number",
        "rnu" => "relativenumber",
        "et" => "expandtab",
        other => other,
    }
}

fn set_bool(app: &mut App, key: &str, b: bool) {
    match key {
        "number" => app.options.number = b,
        "relativenumber" => app.options.relativenumber = b,
        "expandtab" => app.options.expandtab = b,
        _ => app.set_message(format!("E518: Unknown option: {key}")),
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}
