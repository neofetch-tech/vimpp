//! A tiny, dependency-free lexer.
//!
//! Vim++ deliberately avoids shipping a full tree-sitter grammar set in the
//! startup path. This lexer is good enough for reading code, costs microseconds
//! per line, and keeps the first frame fast. Tree-sitter can be layered on top
//! later without changing the renderer: it already consumes `TokenKind` spans.

use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use std::path::Path;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Html,
    Css,
    Json,
    Toml,
    Markdown,
    Python,
    Shell,
    Go,
    C,
    Lua,
    Sql,
    Yaml,
    Other,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "rs" => Self::Rust,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "html" | "htm" | "xhtml" => Self::Html,
            "css" | "scss" | "sass" | "less" => Self::Css,
            "json" | "jsonc" => Self::Json,
            "toml" => Self::Toml,
            "md" | "markdown" | "mdx" => Self::Markdown,
            "py" | "pyi" => Self::Python,
            "sh" | "bash" | "zsh" | "fish" => Self::Shell,
            "go" => Self::Go,
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => Self::C,
            "lua" => Self::Lua,
            "sql" => Self::Sql,
            "yaml" | "yml" => Self::Yaml,
            _ => Self::Other,
        }
    }

    #[allow(dead_code)] // part of the lexer's documented surface
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Toml => "TOML",
            Self::Markdown => "Markdown",
            Self::Python => "Python",
            Self::Shell => "Shell",
            Self::Go => "Go",
            Self::C => "C",
            Self::Lua => "Lua",
            Self::Sql => "SQL",
            Self::Yaml => "YAML",
            Self::Other => "Plain text",
        }
    }

    /// Short label shown in the status bar.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::JavaScript => "js",
            Self::TypeScript => "ts",
            Self::Html => "html",
            Self::Css => "css",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "md",
            Self::Python => "py",
            Self::Shell => "sh",
            Self::Go => "go",
            Self::C => "c",
            Self::Lua => "lua",
            Self::Sql => "sql",
            Self::Yaml => "yaml",
            Self::Other => "txt",
        }
    }

    #[allow(dead_code)] // exposed for the future tree-sitter layering
    pub fn color(self, theme: &Theme) -> ratatui::style::Color {
        match self {
            Self::Rust => theme.orange,
            Self::JavaScript => theme.yellow,
            Self::TypeScript => theme.blue,
            Self::Html => theme.red,
            Self::Css => theme.cyan,
            Self::Json => theme.yellow,
            Self::Toml => theme.red,
            Self::Markdown => theme.blue,
            Self::Python => theme.blue,
            Self::Shell => theme.mint,
            Self::Go => theme.cyan,
            Self::C => theme.blue,
            Self::Lua => theme.purple,
            Self::Sql => theme.orange,
            Self::Yaml => theme.red,
            Self::Other => theme.muted,
        }
    }

    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "as",
                "async",
                "await",
                "break",
                "const",
                "continue",
                "crate",
                "dyn",
                "else",
                "enum",
                "extern",
                "fn",
                "for",
                "if",
                "impl",
                "in",
                "let",
                "loop",
                "match",
                "mod",
                "move",
                "mut",
                "pub",
                "ref",
                "return",
                "self",
                "Self",
                "static",
                "struct",
                "super",
                "trait",
                "type",
                "unsafe",
                "use",
                "where",
                "while",
                "union",
                "macro_rules",
            ],
            Self::JavaScript | Self::TypeScript => &[
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "debugger",
                "default",
                "delete",
                "do",
                "else",
                "export",
                "extends",
                "finally",
                "for",
                "function",
                "if",
                "import",
                "in",
                "instanceof",
                "let",
                "new",
                "of",
                "return",
                "static",
                "super",
                "switch",
                "this",
                "throw",
                "try",
                "typeof",
                "var",
                "void",
                "while",
                "with",
                "yield",
                "async",
                "await",
                "interface",
                "type",
                "enum",
                "implements",
                "readonly",
                "private",
                "public",
                "protected",
                "as",
                "from",
                "satisfies",
                "declare",
                "namespace",
                "abstract",
            ],
            Self::Python => &[
                "def", "class", "if", "elif", "else", "for", "while", "return", "import", "from",
                "as", "pass", "break", "continue", "with", "try", "except", "finally", "raise",
                "in", "is", "not", "and", "or", "lambda", "yield", "global", "nonlocal", "assert",
                "del", "async", "await", "match", "case",
            ],
            Self::Shell => &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
                "function", "in", "local", "export", "return",
            ],
            Self::Go => &[
                "package",
                "import",
                "func",
                "var",
                "const",
                "type",
                "struct",
                "interface",
                "map",
                "chan",
                "go",
                "defer",
                "if",
                "else",
                "for",
                "range",
                "return",
                "switch",
                "case",
                "default",
                "break",
                "continue",
                "select",
                "fallthrough",
                "goto",
            ],
            Self::C => &[
                "int", "char", "void", "struct", "enum", "union", "typedef", "static", "const",
                "unsigned", "signed", "long", "short", "float", "double", "if", "else", "for",
                "while", "do", "switch", "case", "default", "break", "continue", "return", "goto",
                "sizeof", "extern", "volatile", "register", "inline",
            ],
            Self::Lua => &[
                "local", "function", "end", "if", "then", "else", "elseif", "for", "while", "do",
                "repeat", "until", "return", "break", "in", "and", "or", "not", "goto",
            ],
            Self::Sql => &[
                "select", "from", "where", "insert", "into", "values", "update", "set", "delete",
                "create", "table", "drop", "alter", "join", "on", "group", "by", "order", "limit",
                "as", "and", "or", "not", "is", "distinct", "having", "left", "right", "inner",
                "outer", "null",
            ],
            _ => &[],
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Function,
    Constant,
    Operator,
    Property,
    Tag,
    Attribute,
    Macro,
    Heading,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

impl Token {
    #[allow(dead_code)] // kept for downstream token consumers
    pub fn contains(&self, index: usize) -> bool {
        index >= self.start && index < self.end
    }
}

pub fn style_for(theme: &Theme, kind: TokenKind) -> Style {
    let (color, modifier) = match kind {
        TokenKind::Plain => (theme.text, Modifier::empty()),
        TokenKind::Keyword => (theme.accent, Modifier::BOLD),
        TokenKind::Type => (theme.cyan, Modifier::empty()),
        TokenKind::String => (theme.mint, Modifier::empty()),
        TokenKind::Number => (theme.orange, Modifier::empty()),
        TokenKind::Comment => (theme.muted, Modifier::ITALIC),
        TokenKind::Function => (theme.blue, Modifier::empty()),
        TokenKind::Constant => (theme.purple, Modifier::empty()),
        TokenKind::Operator => (theme.text_dim, Modifier::empty()),
        TokenKind::Property => (theme.yellow, Modifier::empty()),
        TokenKind::Tag => (theme.red, Modifier::empty()),
        TokenKind::Attribute => (theme.yellow, Modifier::empty()),
        TokenKind::Macro => (theme.purple, Modifier::empty()),
        TokenKind::Heading => (theme.accent, Modifier::BOLD),
    };
    Style::default().fg(color).add_modifier(modifier)
}

/// Highlight one logical line. `Token` offsets are character indices.
pub fn highlight_line(language: Language, text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    match language {
        Language::Markdown => markdown(&chars, &mut tokens),
        Language::Html => html(&chars, &mut tokens),
        Language::Css => css(&chars, &mut tokens),
        Language::Json => json(&chars, &mut tokens),
        _ => code(language, &chars, &mut tokens),
    }
    merge(tokens)
}

fn merge(tokens: Vec<Token>) -> Vec<Token> {
    let mut merged: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens {
        if token.start >= token.end {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.end == token.start && last.kind == token.kind => {
                last.end = token.end;
            }
            _ => merged.push(token),
        }
    }
    merged
}

fn push(out: &mut Vec<Token>, start: usize, end: usize, kind: TokenKind) {
    if end > start {
        out.push(Token { start, end, kind });
    }
}

fn starts_with(chars: &[char], at: usize, needle: &str) -> bool {
    let needle: Vec<char> = needle.chars().collect();
    if at + needle.len() > chars.len() {
        return false;
    }
    chars[at..at + needle.len()] == needle[..]
}

fn find_from(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() || from >= chars.len() {
        return None;
    }
    (from..=chars.len().saturating_sub(needle.len()))
        .find(|&i| chars[i..i + needle.len()] == needle[..])
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn next_non_space(chars: &[char], from: usize) -> Option<char> {
    chars[from.min(chars.len())..]
        .iter()
        .copied()
        .find(|c| !c.is_whitespace())
}

fn scan_string(chars: &[char], start: usize, quote: char) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

fn scan_number(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            i += 1;
        } else if (c == '+' || c == '-') && i > start && matches!(chars[i - 1], 'e' | 'E') {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn classify_ident(language: Language, word: &str, next: Option<char>) -> TokenKind {
    if language.keywords().contains(&word) {
        return TokenKind::Keyword;
    }
    if matches!(
        word,
        "true" | "false" | "null" | "None" | "nil" | "True" | "False" | "NULL"
    ) {
        return TokenKind::Constant;
    }
    if word.chars().next().is_some_and(|c| c.is_uppercase()) {
        return TokenKind::Type;
    }
    if next == Some('(') {
        return TokenKind::Function;
    }
    TokenKind::Plain
}

fn code(language: Language, chars: &[char], out: &mut Vec<Token>) {
    let line_comment = match language {
        Language::Rust
        | Language::JavaScript
        | Language::TypeScript
        | Language::Go
        | Language::C => Some("//"),
        Language::Python | Language::Shell | Language::Toml | Language::Yaml => Some("#"),
        Language::Lua | Language::Sql => Some("--"),
        _ => None,
    };
    let has_block_comments = matches!(
        language,
        Language::Rust | Language::JavaScript | Language::TypeScript | Language::Go | Language::C
    );

    let mut i = 0;
    while i < chars.len() {
        if let Some(marker) = line_comment {
            if starts_with(chars, i, marker) {
                push(out, i, chars.len(), TokenKind::Comment);
                break;
            }
        }

        if has_block_comments && starts_with(chars, i, "/*") {
            let end = find_from(chars, i + 2, "*/")
                .map(|pos| pos + 2)
                .unwrap_or(chars.len());
            push(out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }

        let c = chars[i];
        if c == '"'
            || c == '\''
            || (c == '`' && matches!(language, Language::JavaScript | Language::TypeScript))
        {
            let end = scan_string(chars, i, c);
            push(out, i, end, TokenKind::String);
            i = end;
            continue;
        }

        if c.is_ascii_digit() {
            let end = scan_number(chars, i);
            push(out, i, end, TokenKind::Number);
            i = end;
            continue;
        }

        if is_ident_start(c) {
            let mut end = i + 1;
            while end < chars.len() && is_ident_char(chars[end]) {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            let kind = classify_ident(language, &word, next_non_space(chars, end));
            push(out, i, end, kind);
            i = end;
            continue;
        }

        if is_operator(c) {
            push(out, i, i + 1, TokenKind::Operator);
        }
        i += 1;
    }
}

fn is_operator(c: char) -> bool {
    matches!(
        c,
        '+' | '-'
            | '*'
            | '/'
            | '%'
            | '='
            | '<'
            | '>'
            | '!'
            | '&'
            | '|'
            | '^'
            | '~'
            | '?'
            | ':'
            | ';'
            | ','
            | '.'
    )
}

fn json(chars: &[char], out: &mut Vec<Token>) {
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            let end = scan_string(chars, i, c);
            let mut after = end;
            while after < chars.len() && chars[after].is_whitespace() {
                after += 1;
            }
            let kind = if after < chars.len() && chars[after] == ':' {
                TokenKind::Property
            } else {
                TokenKind::String
            };
            push(out, i, end, kind);
            i = end;
            continue;
        }
        if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        {
            let end = scan_number(chars, i + usize::from(c == '-'));
            push(out, i, end, TokenKind::Number);
            i = end;
            continue;
        }
        if is_ident_start(c) {
            let mut end = i + 1;
            while end < chars.len() && is_ident_char(chars[end]) {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            let kind = if matches!(word.as_str(), "true" | "false" | "null") {
                TokenKind::Constant
            } else {
                TokenKind::Plain
            };
            push(out, i, end, kind);
            i = end;
            continue;
        }
        if matches!(c, ':' | ',' | '{' | '}' | '[' | ']') {
            push(out, i, i + 1, TokenKind::Operator);
        }
        i += 1;
    }
}

fn css(chars: &[char], out: &mut Vec<Token>) {
    let mut i = 0;
    while i < chars.len() {
        if starts_with(chars, i, "/*") {
            let end = find_from(chars, i + 2, "*/")
                .map(|pos| pos + 2)
                .unwrap_or(chars.len());
            push(out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }
        let c = chars[i];
        if c == '"' || c == '\'' {
            let end = scan_string(chars, i, c);
            push(out, i, end, TokenKind::String);
            i = end;
            continue;
        }
        if c == '#' && chars.get(i + 1).is_some_and(|c| c.is_ascii_hexdigit()) {
            let mut end = i + 1;
            while end < chars.len() && chars[end].is_ascii_hexdigit() {
                end += 1;
            }
            push(out, i, end, TokenKind::Number);
            i = end;
            continue;
        }
        if c == '@' {
            let mut end = i + 1;
            while end < chars.len() && (is_ident_char(chars[end]) || chars[end] == '-') {
                end += 1;
            }
            push(out, i, end, TokenKind::Macro);
            i = end;
            continue;
        }
        if is_ident_start(c) || c == '-' {
            let mut end = i + 1;
            while end < chars.len() && (is_ident_char(chars[end]) || chars[end] == '-') {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            let next = next_non_space(chars, end);
            let kind = if match next {
                Some(':') => true,
                _ => is_css_property(&word),
            } {
                TokenKind::Property
            } else if next == Some('(') {
                TokenKind::Function
            } else if is_css_value(&word) {
                TokenKind::Constant
            } else {
                TokenKind::Plain
            };
            push(out, i, end, kind);
            i = end;
            continue;
        }
        if c.is_ascii_digit() {
            let end = scan_number(chars, i);
            push(out, i, end, TokenKind::Number);
            i = end;
            continue;
        }
        if matches!(c, ':' | ';' | '{' | '}' | '(' | ')' | ',' | '.') {
            push(out, i, i + 1, TokenKind::Operator);
        }
        i += 1;
    }
}

fn is_css_property(word: &str) -> bool {
    matches!(
        word,
        "color"
            | "background"
            | "background-color"
            | "display"
            | "margin"
            | "padding"
            | "border"
            | "font"
            | "font-size"
            | "font-family"
            | "width"
            | "height"
            | "flex"
            | "grid"
            | "position"
            | "top"
            | "left"
            | "right"
            | "bottom"
            | "opacity"
            | "transform"
            | "transition"
            | "gap"
            | "align-items"
            | "justify-content"
            | "border-radius"
            | "box-shadow"
    )
}

fn is_css_value(word: &str) -> bool {
    matches!(
        word,
        "none"
            | "block"
            | "flex"
            | "grid"
            | "inline"
            | "absolute"
            | "relative"
            | "fixed"
            | "solid"
            | "auto"
            | "center"
            | "bold"
            | "italic"
            | "transparent"
            | "inherit"
    )
}

fn html(chars: &[char], out: &mut Vec<Token>) {
    let mut i = 0;
    while i < chars.len() {
        if starts_with(chars, i, "<!--") {
            let end = find_from(chars, i + 4, "-->")
                .map(|pos| pos + 3)
                .unwrap_or(chars.len());
            push(out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }
        if chars[i] != '<' {
            i += 1;
            continue;
        }

        let tag_end = find_from(chars, i, ">").unwrap_or(chars.len() - 1);
        push(out, i, i + 1, TokenKind::Operator);
        let mut j = i + 1;
        if j < chars.len() && chars[j] == '/' {
            push(out, j, j + 1, TokenKind::Operator);
            j += 1;
        }
        let name_start = j;
        while j < tag_end && is_ident_char(chars[j]) {
            j += 1;
        }
        push(out, name_start, j, TokenKind::Tag);

        while j < tag_end {
            let c = chars[j];
            if c.is_whitespace() {
                j += 1;
            } else if c == '"' || c == '\'' {
                let end = scan_string(chars, j, c).min(tag_end + 1);
                push(out, j, end, TokenKind::String);
                j = end;
            } else if c == '=' {
                push(out, j, j + 1, TokenKind::Operator);
                j += 1;
            } else if is_ident_start(c) || c == '-' || c == ':' {
                let start = j;
                j += 1;
                while j < tag_end && (is_ident_char(chars[j]) || chars[j] == '-' || chars[j] == ':')
                {
                    j += 1;
                }
                push(out, start, j, TokenKind::Attribute);
            } else {
                j += 1;
            }
        }
        push(out, tag_end, tag_end + 1, TokenKind::Operator);
        i = tag_end + 1;
    }
}

fn markdown(chars: &[char], out: &mut Vec<Token>) {
    if chars.is_empty() {
        return;
    }
    let line: String = chars.iter().collect();
    let trimmed = line.trim_start();

    if trimmed.starts_with('#') {
        push(out, 0, chars.len(), TokenKind::Heading);
        return;
    }
    if trimmed.starts_with('>') {
        push(out, 0, chars.len(), TokenKind::Comment);
        return;
    }
    if trimmed.starts_with("```") {
        push(out, 0, chars.len(), TokenKind::Macro);
        return;
    }

    let mut i = 0;
    while i < chars.len() {
        if starts_with(chars, i, "<!--") {
            let end = find_from(chars, i + 4, "-->")
                .map(|pos| pos + 3)
                .unwrap_or(chars.len());
            push(out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }
        if chars[i] == '`' {
            let end = scan_string(chars, i, '`');
            push(out, i, end, TokenKind::String);
            i = end;
            continue;
        }
        if starts_with(chars, i, "**") {
            let end = find_from(chars, i + 2, "**")
                .map(|pos| pos + 2)
                .unwrap_or(chars.len());
            push(out, i, end, TokenKind::Keyword);
            i = end;
            continue;
        }
        if chars[i] == '[' {
            if let Some(close) = find_from(chars, i + 1, "](") {
                let end = find_from(chars, close + 2, ")")
                    .map(|pos| pos + 1)
                    .unwrap_or(chars.len());
                push(out, i, end, TokenKind::Function);
                i = end;
                continue;
            }
        }
        if matches!(chars[i], '-' | '*' | '+') && i + 1 < chars.len() && chars[i + 1] == ' ' {
            if i == 0 || chars[i - 1].is_whitespace() {
                push(out, i, i + 1, TokenKind::Keyword);
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<TokenKind> {
        highlight_line(Language::Rust, text)
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn detects_rust_tokens() {
        let tokens = kinds("pub fn main() { let x = 42; // hi");
        assert!(tokens.contains(&TokenKind::Keyword));
        assert!(tokens.contains(&TokenKind::Function));
        assert!(tokens.contains(&TokenKind::Number));
        assert!(tokens.contains(&TokenKind::Comment));
    }

    #[test]
    fn strings_can_contain_comment_markers() {
        let tokens = highlight_line(Language::Rust, "let url = \"http://x\";");
        assert!(tokens.iter().any(|token| token.kind == TokenKind::String));
        assert!(!tokens.iter().any(|token| token.kind == TokenKind::Comment));
    }

    #[test]
    fn markdown_headings_are_highlighted() {
        let tokens = highlight_line(Language::Markdown, "## Hello");
        assert_eq!(tokens[0].kind, TokenKind::Heading);
    }

    #[test]
    fn unicode_offsets_stay_in_characters() {
        let tokens = highlight_line(Language::Rust, "let s = \"привіт\";");
        let end = tokens.last().map(|t| t.end).unwrap_or(0);
        assert_eq!(end, "let s = \"привіт\";".chars().count());
    }
}
