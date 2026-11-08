//! Key notation in Neovim dialect + translating mapping right-hand sides
//! into synthesized key events. This is what makes user `init.lua` maps
//! behave natively inside Vim++.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Canonical notation for a physical key press, Neovim style: `<C-s>`, `<Esc>`.
pub fn notation_of(key: &KeyEvent) -> Option<String> {
    let base = base_notation(key.code)?;
    let mut mods = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        mods.push('C');
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        if !mods.is_empty() {
            mods.push('-');
        }
        mods.push('A');
    }
    if mods.is_empty() {
        Some(base)
    } else {
        Some(format!("<{mods}-{base}>"))
    }
}

/// Base notation WITHOUT angle brackets. SHIFT is intentionally ignored for
/// characters — uppercase `A` is just "A".
fn base_notation(code: KeyCode) -> Option<String> {
    match code {
        KeyCode::Char(c) => Some(c.to_string()),
        KeyCode::Esc => Some("Esc".into()),
        KeyCode::Enter => Some("CR".into()),
        KeyCode::Tab => Some("Tab".into()),
        KeyCode::BackTab => Some("Tab".into()),
        KeyCode::Backspace => Some("BS".into()),
        KeyCode::Delete => Some("Del".into()),
        KeyCode::Left => Some("Left".into()),
        KeyCode::Right => Some("Right".into()),
        KeyCode::Up => Some("Up".into()),
        KeyCode::Down => Some("Down".into()),
        KeyCode::Home => Some("Home".into()),
        KeyCode::End => Some("End".into()),
        KeyCode::PageUp => Some("PageUp".into()),
        KeyCode::PageDown => Some("PageDown".into()),
        KeyCode::F(n) => Some(format!("F{n}")),
        _ => None,
    }
}

/// Parse a mapping RHS like `ggj<Esc>` into key events.
/// `<Cmd>...<CR>` is resolved by the caller (it means "run ex command").
pub fn rhs_to_keys(rhs: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let chars: Vec<char> = rhs.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            if let Some(end) = (i + 1..chars.len()).find(|&j| chars[j] == '>') {
                let token: String = chars[i + 1..end].iter().collect();
                if let Some(ev) = token_to_event(&token) {
                    out.push(ev);
                    i = end + 1;
                    continue;
                }
            }
        }
        out.push(KeyEvent::new(KeyCode::Char(chars[i]), KeyModifiers::NONE));
        i += 1;
    }
    out
}

fn token_to_event(token: &str) -> Option<KeyEvent> {
    let (mods, rest) = match token.strip_prefix("C-") {
        Some(rest) => (KeyModifiers::CONTROL, rest),
        None => (KeyModifiers::NONE, token),
    };
    let code = match rest.to_ascii_lowercase().as_str() {
        "esc" => KeyCode::Esc,
        "cr" | "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "bs" => KeyCode::Backspace,
        "del" => KeyCode::Delete,
        "space" => KeyCode::Char(' '),
        "lt" => KeyCode::Char('<'),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        _ => {
            let mut cs = rest.chars();
            let c = cs.next()?;
            if cs.next().is_some() || !c.is_ascii_alphanumeric() {
                return None; // unknown token: upstream treats it as literal text
            }
            KeyCode::Char(c)
        }
    };
    Some(KeyEvent::new(code, mods))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_s_notation() {
        let k = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(notation_of(&k).as_deref(), Some("<C-s>"));
    }

    #[test]
    fn plain_char_notation() {
        let k = KeyEvent::new(KeyCode::Char('H'), KeyModifiers::SHIFT);
        assert_eq!(notation_of(&k).as_deref(), Some("H"));
    }

    #[test]
    fn rhs_parsing() {
        let keys = rhs_to_keys("g<Esc>");
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].code, KeyCode::Esc);
        let keys = rhs_to_keys("<C-s>");
        assert!(keys[0].modifiers.contains(KeyModifiers::CONTROL));
    }
}
