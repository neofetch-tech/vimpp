//! The buffer: a Rope-backed text container with modal cursor state.
//!
//! Cursor model: `(cx, cy)` where `cx` is a CHARACTER index inside line `cy`
//! (no byte-offset headaches with Ukrainian/any-Unicode text). The rope does
//! the heavy lifting; these helpers expose Vim-like primitives.

use std::path::{Path, PathBuf};

use ropey::Rope;

use crate::highlight::Language;

#[derive(Clone)]
pub struct Buffer {
    pub path: Option<PathBuf>,
    pub rope: Rope,
    pub cx: usize,
    pub cy: usize,
    pub dirty: bool,
    pub top: usize,
    pub left: usize,
    pub undo: Vec<Rope>,
    pub redo: Vec<Rope>,
    pub lang: Language,
}

pub fn char_to_byte(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map(|(b, _)| b).unwrap_or(s.len())
}

pub fn class_of(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Buffer {
    pub fn untitled() -> Self {
        Self {
            path: None,
            rope: Rope::new(),
            cx: 0,
            cy: 0,
            dirty: false,
            top: 0,
            left: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            lang: Language::Other,
        }
    }

    /// Open a file. Missing files become a new (dirty) buffer — like `:e new.txt`.
    /// Non-UTF8 bytes are replaced, never fatal.
    pub fn open(path: &Path) -> Self {
        let mut buf = Self::untitled();
        buf.lang = Language::from_path(path);
        match std::fs::read(path) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                // Normalize: we keep lines WITHOUT trailing newline characters.
                let trimmed = text.trim_end_matches(['\n', '\r']);
                buf.rope = Rope::from_str(trimmed);
            }
            Err(_) => {
                buf.dirty = true;
            }
        }
        buf.path = Some(path.to_path_buf());
        buf
    }

    pub fn name(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into()),
            None => "[No Name]".into(),
        }
    }

    pub fn display_name(&self) -> String {
        if self.dirty {
            format!("{} ●", self.name())
        } else {
            self.name()
        }
    }

    // ---- rope helpers -------------------------------------------------

    pub fn line_count(&self) -> usize {
        // Our normalized model: an empty rope still has one (empty) line.
        self.rope.len_lines().max(1)
    }

    pub fn line_len(&self, y: usize) -> usize {
        if y >= self.line_count() {
            return 0;
        }
        line_of(&self.rope, y).chars().count()
    }

    pub fn line(&self, y: usize) -> String {
        if y >= self.line_count() {
            String::new()
        } else {
            line_of(&self.rope, y)
        }
    }

    pub fn char_at(&self, x: usize, y: usize) -> Option<char> {
        if y >= self.line_count() || x >= self.line_len(y) {
            return None;
        }
        let g = self.rope.line_to_char(y) + x;
        self.rope.get_char(g)
    }

    fn prev_pos(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        if x > 0 {
            Some((x - 1, y))
        } else if y > 0 {
            Some((self.line_len(y - 1), y - 1))
        } else {
            None
        }
    }

    fn global(&self, x: usize, y: usize) -> usize {
        let y = y.min(self.line_count().saturating_sub(1));
        self.rope.line_to_char(y) + x.min(self.line_len(y))
    }

    /// Clamp cursor after a NORMAL/VISUAL movement: Vim keeps it ON a character.
    pub fn clamp_cursor_normal(&mut self) {
        self.cy = self.cy.min(self.line_count().saturating_sub(1));
        if self.line_len(self.cy) == 0 {
            self.cx = 0;
        } else {
            self.cx = self.cx.min(self.line_len(self.cy).saturating_sub(1));
        }
    }

    /// Clamp after an INSERT movement: the caret may rest at end-of-line.
    pub fn clamp_cursor_insert(&mut self) {
        self.cy = self.cy.min(self.line_count().saturating_sub(1));
        self.cx = self.cx.min(self.line_len(self.cy));
    }

    // ---- undo/redo -----------------------------------------------------

    pub fn snapshot(&mut self) {
        self.undo.push(self.rope.clone());
        if self.undo.len() > 300 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(self.rope.clone());
                self.rope = prev;
                self.dirty = true;
                self.clamp_cursor_insert();
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(self.rope.clone());
                self.rope = next;
                self.dirty = true;
                self.clamp_cursor_insert();
                true
            }
            None => false,
        }
    }

    // ---- editing primitives --------------------------------------------

    /// Insert `text` (may contain '\n') at the cursor; cursor lands after it.
    pub fn insert_text_raw(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.snapshot();
        let g = self.global(self.cx, self.cy);
        self.rope.insert(g, text);
        let parts = text.split('\n').count();
        if parts > 1 {
            self.cy += parts - 1;
            let tail = text.rsplit('\n').next().unwrap_or("");
            self.cx = tail.chars().count();
        } else {
            self.cx += text.chars().count();
        }
        self.dirty = true;
    }

    pub fn insert_char(&mut self, c: char) {
        self.insert_text_raw(&c.to_string());
    }

    /// Delete the character under the cursor (like `x`). Returns removed text.
    pub fn delete_char(&mut self) -> String {
        if self.cx >= self.line_len(self.cy) {
            return String::new();
        }
        self.snapshot();
        let g = self.global(self.cx, self.cy);
        let removed = self
            .rope
            .get_char(g)
            .map(|c| c.to_string())
            .unwrap_or_default();
        self.rope.remove(g..g + removed.chars().count());
        self.dirty = true;
        removed
    }

    /// Delete the character before the cursor (like `<BS>`), joining lines.
    pub fn backspace(&mut self) {
        if self.cx > 0 {
            self.snapshot();
            let g = self.global(self.cx, self.cy);
            self.rope.remove(g - 1..g);
            self.cx -= 1;
            self.dirty = true;
        } else if self.cy > 0 {
            self.snapshot();
            let prev_len = self.line_len(self.cy - 1);
            let g = self.global(0, self.cy);
            self.rope.remove(g - 1..g); // the '\n'
            self.cy -= 1;
            self.cx = prev_len;
            self.dirty = true;
        }
    }

    /// `Enter` with auto-indent from the current line.
    pub fn newline(&mut self) {
        let indent: String = self
            .line(self.cy)
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        self.insert_text_raw(&format!("\n{}", indent));
        self.cx = indent.chars().count();
    }

    /// `dd` — remove a whole line, returning its text for the register.
    pub fn delete_line(&mut self) -> String {
        if self.line_count() == 1 {
            let text = self.line(0);
            self.snapshot();
            self.rope = Rope::new();
            self.cx = 0;
            self.cy = 0;
            self.dirty = true;
            return text;
        }
        self.snapshot();
        let y = self.cy.min(self.line_count() - 1);
        let text = self.line(y);
        let a = self.rope.line_to_char(y);
        let b = self.rope.line_to_char(y + 1).min(self.rope.len_chars());
        self.rope.remove(a..b);
        self.cy = y.min(self.line_count().saturating_sub(1));
        self.cx = self.cx.min(self.line_len(self.cy).saturating_sub(1));
        self.dirty = true;
        text
    }

    /// Charwise span delete (inclusive of the end char). Assumes start <= end.
    /// Coordinates are `(x, y)` pairs — columns first, like the cursor state.
    pub fn delete_span(&mut self, start: (usize, usize), end: (usize, usize)) -> String {
        let (sx, sy) = start;
        let (ex, ey) = end;
        if sy == ey {
            let g0 = self.global(sx.min(self.line_len(sy)), sy);
            let g1 = self.global((ex + 1).min(self.line_len(ey)), ey);
            if g1 > g0 {
                self.snapshot();
                let removed = self.rope.slice(g0..g1).to_string();
                self.rope.remove(g0..g1);
                self.cx = sx;
                self.cy = sy;
                self.dirty = true;
                return removed;
            }
            return String::new();
        }
        self.snapshot();
        let removed = self.span_text(start, end);
        let ga = self.global(sx, sy);
        let gb = self.global(ex + 1, ey);
        if gb > ga {
            self.rope.remove(ga..gb);
        }
        self.cy = sy;
        self.cx = sx;
        self.dirty = true;
        removed
    }

    /// Charwise span text (`(x, y)` pairs, inclusive of the end char). No mutation.
    fn span_text(&self, start: (usize, usize), end: (usize, usize)) -> String {
        let (sx, sy) = start;
        let (ex, ey) = end;
        let mut out = String::new();
        if sy == ey {
            return self.line(sy).chars().skip(sx).take(ex - sx + 1).collect();
        }
        out.push_str(&self.line(sy).chars().skip(sx).collect::<String>());
        for y in sy + 1..=ey {
            out.push('\n');
            if y == ey {
                out.push_str(&self.line(ey).chars().take(ex + 1).collect::<String>());
            } else {
                out.push_str(&self.line(y));
            }
        }
        out
    }

    /// Linewise delete of `[y0, y1]` inclusive (used by `Vd`).
    pub fn delete_lines_range(&mut self, y0: usize, y1: usize) -> Vec<String> {
        let y0 = y0.min(self.line_count() - 1);
        let y1 = y1.min(self.line_count() - 1).max(y0);
        self.snapshot();
        let removed: Vec<String> = (y0..=y1).map(|y| self.line(y)).collect();
        let a = self.rope.line_to_char(y0);
        let b = self
            .rope
            .line_to_char((y1 + 1).min(self.line_count()))
            .min(self.rope.len_chars());
        if b > a {
            self.rope.remove(a..b);
        }
        if self.line_count() == 0 {
            self.rope = Rope::new();
        }
        self.cy = y0.min(self.line_count().saturating_sub(1));
        self.cx = 0;
        self.dirty = true;
        removed
    }

    /// Charwise copy of a span WITHOUT deleting (visual `y`).
    pub fn copy_span(&self, start: (usize, usize), end: (usize, usize)) -> String {
        self.span_text(start, end)
    }

    /// Linewise paste (like `p` after `dd`).
    pub fn paste_lines(&mut self, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let text = format!("\n{}", lines.join("\n"));
        let g = self.global(self.line_len(self.cy), self.cy);
        self.rope.insert(g, &text);
        self.cy += 1;
        self.cx = 0;
        self.dirty = true;
    }

    pub fn paste_text(&mut self, text: &str) {
        self.insert_text_raw(text);
    }

    // ---- motions ---------------------------------------------------------

    pub fn move_up(&mut self) {
        if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.cx.min(self.line_len(self.cy));
        }
    }

    pub fn move_down(&mut self) {
        if self.cy + 1 < self.line_count() {
            self.cy += 1;
            self.cx = self.cx.min(self.line_len(self.cy));
        }
    }

    pub fn move_left(&mut self) {
        if self.cx > 0 {
            self.cx -= 1;
        } else if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.line_len(self.cy);
        }
    }

    pub fn move_right(&mut self) {
        if self.cx < self.line_len(self.cy) {
            self.cx += 1;
        } else if self.cy + 1 < self.line_count() {
            self.cy += 1;
            self.cx = 0;
        }
    }

    pub fn line_start(&mut self) {
        self.cx = 0;
    }

    pub fn line_end(&mut self) {
        self.cx = self.line_len(self.cy);
    }

    pub fn first_non_blank(&mut self) {
        self.cx = self
            .line(self.cy)
            .chars()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);
    }

    /// `w` — start of the next word.
    pub fn word_forward(&mut self) {
        let (mut x, mut y) = (self.cx, self.cy);
        if x >= self.line_len(y) {
            if y + 1 < self.line_count() {
                y += 1;
                x = 0;
            } else {
                return;
            }
        } else {
            let cls = class_of(self.char_at(x, y).unwrap_or(' '));
            while let Some(c) = self.char_at(x, y) {
                if class_of(c) == cls {
                    x += 1;
                } else {
                    break;
                }
            }
        }
        // Skip whitespace (possibly across empty lines).
        loop {
            match self.char_at(x, y) {
                Some(c) if c.is_whitespace() => x += 1,
                Some(_) => break,
                None => {
                    if y + 1 < self.line_count() {
                        y += 1;
                        x = 0;
                    } else {
                        break;
                    }
                }
            }
        }
        self.cx = x;
        self.cy = y;
    }

    /// `b` — start of the previous word.
    pub fn word_backward(&mut self) {
        let Some((mut x, mut y)) = self.prev_pos(self.cx, self.cy) else {
            return;
        };
        // If we stepped into whitespace, walk back to the first non-space.
        loop {
            match self.char_at(x, y) {
                Some(c) if c.is_whitespace() => match self.prev_pos(x, y) {
                    Some((px, py)) => {
                        x = px;
                        y = py;
                    }
                    None => {
                        self.cx = x;
                        self.cy = y;
                        return;
                    }
                },
                _ => break,
            }
        }
        // Walk to the start of this class run.
        while let Some((px, py)) = self.prev_pos(x, y) {
            let pc = self.char_at(px, py).unwrap_or(' ');
            let cc = self.char_at(x, y).unwrap_or(' ');
            if class_of(pc) == class_of(cc) {
                x = px;
                y = py;
            } else {
                break;
            }
        }
        self.cx = x;
        self.cy = y;
    }

    /// `e` — end of the current/next word.
    pub fn word_end(&mut self) {
        let (mut x, mut y) = (self.cx, self.cy);
        // At minimum, move one char right.
        if x + 1 <= self.line_len(y) && x < self.line_len(y) {
            x += 1;
        } else if y + 1 < self.line_count() {
            y += 1;
            x = 0;
        } else {
            return;
        }
        loop {
            match self.char_at(x, y) {
                Some(c) if c.is_whitespace() => x += 1,
                Some(_) => break,
                None => {
                    if y + 1 < self.line_count() {
                        y += 1;
                        x = 0;
                    } else {
                        return;
                    }
                }
            }
        }
        while x + 1 < self.line_len(y) {
            let cur = self.char_at(x, y).unwrap_or(' ');
            let next = self.char_at(x + 1, y).unwrap_or(' ');
            if class_of(next) == class_of(cur) && class_of(cur) != 0 {
                x += 1;
            } else {
                break;
            }
        }
        self.cx = x;
        self.cy = y;
    }

    /// `dw` (with pending `d`): delete up to (not including) next word start.
    pub fn delete_through_word_forward(&mut self) {
        let (sx, sy) = (self.cx, self.cy);
        self.word_forward();
        let (ex, ey) = (self.cx, self.cy);
        // Exclusive end: one char before the next word start.
        let end = if ex > 0 {
            (ex - 1, ey)
        } else if ey > 0 {
            (self.line_len(ey - 1), ey - 1)
        } else {
            (self.cx, self.cy)
        };
        if end.1 > sy || (end.1 == sy && end.0 >= sx) {
            self.delete_span((sx, sy), end);
        }
        self.clamp_cursor_normal();
    }

    // ---- io ---------------------------------------------------------------

    pub fn save(&mut self) -> std::io::Result<usize> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no file name",
                ))
            }
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let mut text = self.rope.to_string();
        text.push('\n');
        std::fs::write(&path, text.as_bytes())?;
        self.dirty = false;
        Ok(text.len())
    }

    pub fn search_in_line(&self, y: usize, needle: &str, from: usize) -> Option<usize> {
        let hay = self.line(y).to_lowercase();
        let nd = needle.to_lowercase();
        let from_byte = char_to_byte(&hay, from);
        let idx = hay[from_byte.min(hay.len())..].find(&nd)?;
        Some(hay[..from_byte + idx].chars().count())
    }
}

/// Rope line without its trailing '\n' (or '\r\n').
fn line_of(rope: &Rope, y: usize) -> String {
    if y >= rope.len_lines() {
        return String::new();
    }
    let slice = rope.line(y);
    let s = slice.to_string();
    s.trim_end_matches(['\n', '\r']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        let mut b = Buffer::untitled();
        b.rope = Rope::from_str(text.trim_end_matches(['\n', '\r']));
        b
    }

    #[test]
    fn insert_and_lines() {
        let mut b = buf("hello\nworld");
        b.cx = 5;
        b.insert_text_raw(" there");
        assert_eq!(b.line(0), "hello there");
        assert_eq!(b.cx, 11);
    }

    #[test]
    fn newline_with_indent() {
        let mut b = buf("    fn a() {}");
        b.cx = 13;
        b.newline();
        assert_eq!(b.line(1), "    ");
        assert_eq!(b.cy, 1);
        assert_eq!(b.cx, 4);
    }

    #[test]
    fn delete_span_across_lines() {
        let mut b = buf("abcdef\nghijkl\nmnopqr");
        let removed = b.delete_span((3, 0), (2, 1));
        assert_eq!(removed, "def\nghi");
        assert_eq!(b.line(0), "abcjkl");
        assert_eq!(b.cx, 3);
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut b = buf("abc");
        b.cx = 3;
        b.insert_char('d');
        assert_eq!(b.line(0), "abcd");
        assert!(b.undo());
        assert_eq!(b.line(0), "abc");
        assert!(b.redo());
        assert_eq!(b.line(0), "abcd");
    }

    #[test]
    fn word_motions() {
        let mut b = buf("let some_var = fn_call(12);");
        b.word_forward();
        assert_eq!((b.cx, b.cy), (4, 0));
        b.word_end();
        assert_eq!(b.cx, 11);
        b.word_backward();
        assert_eq!(b.cx, 4);
    }

    #[test]
    fn unicode_line_ops() {
        let mut b = buf("привіт світ");
        b.cx = 7;
        b.delete_char(); // removes 'с'
        assert_eq!(b.line(0), "привіт віт");
    }
}
