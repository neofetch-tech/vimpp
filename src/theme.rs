//! The Vim++ palette: Nordic polar night, snow storm, frost and aurora.
//!
//! Everything the UI paints comes from this struct, so a theme override in
//! `~/.vimplus/config.toml` can change the whole editor without touching UI code.

use ratatui::style::Color;
use std::collections::HashMap;

// Core surfaces.
pub const BG: Color = Color::Rgb(46, 52, 64);
pub const BG_ALT: Color = Color::Rgb(59, 66, 82);
pub const PANEL: Color = Color::Rgb(59, 66, 82);
pub const PANEL_ALT: Color = Color::Rgb(67, 76, 94);
pub const BORDER: Color = Color::Rgb(76, 86, 106);
pub const BORDER_SOFT: Color = Color::Rgb(67, 76, 94);

// Text.
pub const TEXT: Color = Color::Rgb(216, 222, 233);
pub const TEXT_DIM: Color = Color::Rgb(229, 233, 240);
pub const MUTED: Color = Color::Rgb(129, 141, 162);

// Brand.
pub const ACCENT: Color = Color::Rgb(163, 190, 140);
pub const ACCENT_DIM: Color = Color::Rgb(126, 158, 110);
pub const ACCENT_INK: Color = Color::Rgb(46, 52, 64);
pub const MINT: Color = Color::Rgb(136, 192, 208);

// Semantic.
pub const RED: Color = Color::Rgb(191, 97, 106);
pub const ORANGE: Color = Color::Rgb(208, 135, 112);
pub const YELLOW: Color = Color::Rgb(235, 203, 139);
pub const BLUE: Color = Color::Rgb(94, 129, 172);
pub const CYAN: Color = Color::Rgb(136, 192, 208);
pub const PURPLE: Color = Color::Rgb(180, 142, 173);

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub bg_alt: Color,
    pub panel: Color,
    pub panel_alt: Color,
    pub border: Color,
    pub border_soft: Color,

    pub text: Color,
    pub text_dim: Color,
    pub muted: Color,

    pub accent: Color,
    pub accent_dim: Color,
    pub accent_ink: Color,
    pub mint: Color,

    pub red: Color,
    pub orange: Color,
    pub yellow: Color,
    pub blue: Color,
    pub cyan: Color,
    pub purple: Color,

    pub cursor_line: Color,
    pub selection: Color,
    pub search: Color,
    pub status_bg: Color,
    pub completion_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: BG,
            bg_alt: BG_ALT,
            panel: PANEL,
            panel_alt: PANEL_ALT,
            border: BORDER,
            border_soft: BORDER_SOFT,
            text: TEXT,
            text_dim: TEXT_DIM,
            muted: MUTED,
            accent: ACCENT,
            accent_dim: ACCENT_DIM,
            accent_ink: ACCENT_INK,
            mint: MINT,
            red: RED,
            orange: ORANGE,
            yellow: YELLOW,
            blue: BLUE,
            cyan: CYAN,
            purple: PURPLE,
            cursor_line: Color::Rgb(59, 66, 82),
            selection: Color::Rgb(67, 76, 94),
            search: Color::Rgb(78, 92, 76),
            status_bg: Color::Rgb(59, 66, 82),
            completion_bg: Color::Rgb(67, 76, 94),
        }
    }
}

impl Theme {
    /// Apply `key = "#rrggbb"` overrides from the config file.
    pub fn apply_overrides(&mut self, values: &HashMap<String, String>) {
        for (key, value) in values {
            let color = match parse_hex_color(value) {
                Some(color) => color,
                None => continue,
            };
            match key.as_str() {
                "bg" => self.bg = color,
                "bg_alt" => self.bg_alt = color,
                "panel" => self.panel = color,
                "panel_alt" => self.panel_alt = color,
                "border" => self.border = color,
                "text" => self.text = color,
                "text_dim" => self.text_dim = color,
                "muted" => self.muted = color,
                "accent" => self.accent = color,
                "accent_dim" => self.accent_dim = color,
                "mint" => self.mint = color,
                "red" => self.red = color,
                "orange" => self.orange = color,
                "yellow" => self.yellow = color,
                "blue" => self.blue = color,
                "cyan" => self.cyan = color,
                "purple" => self.purple = color,
                "cursor_line" => self.cursor_line = color,
                "selection" => self.selection = color,
                "search" => self.search = color,
                "status_bg" => self.status_bg = color,
                "completion_bg" => self.completion_bg = color,
                _ => {}
            }
        }
    }
}

/// Parse `#rrggbb` or `rrggbb` (and the short `#rgb` form) into a terminal RGB.
pub fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim().trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            (r * 17, g * 17, b * 17)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colors() {
        assert_eq!(parse_hex_color("#b6ff3b"), Some(Color::Rgb(182, 255, 59)));
        assert_eq!(parse_hex_color("0a1024"), Some(Color::Rgb(10, 16, 36)));
        assert_eq!(parse_hex_color("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(parse_hex_color("#xyzxyz"), None);
    }

    #[test]
    fn overrides_only_known_keys() {
        let mut theme = Theme::default();
        let values = HashMap::from([
            ("accent".to_string(), "#ff0000".to_string()),
            ("nonsense".to_string(), "#ffffff".to_string()),
        ]);
        theme.apply_overrides(&values);
        assert_eq!(theme.accent, Color::Rgb(255, 0, 0));
    }
}
