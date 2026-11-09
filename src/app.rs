//! Application state and the modal editing core.
//!
//! Mode flow: Normal ↔ Insert / Visual, with a Command (ex / search) line.
//! Key events go through `handle_key`, which first consults user mappings
//! (from `init.lua` / `config.toml`) and then falls back to built-in keys.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::{is_word_char, Buffer};
use crate::command;
use crate::explorer::Explorer;
use crate::highlight::Language;
use crate::keys;
use crate::lua::LuaHost;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual { line: bool },
    Command { search: bool },
}

#[derive(Clone)]
pub enum Register {
    Chars(String),
    Lines(Vec<String>),
}

pub struct Completion {
    pub prefix: String,
    pub items: Vec<String>,
    pub index: usize,
}

#[derive(Clone)]
pub struct Options {
    pub number: bool,
    pub relativenumber: bool,
    pub tabstop: usize,
    pub expandtab: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            number: true,
            // Absolute numbering is the beginner-friendly default. Relative
            // distances remain available with `:set relativenumber`.
            relativenumber: false,
            tabstop: 4,
            expandtab: true,
        }
    }
}

#[derive(Clone)]
pub struct Mapping {
    pub mode: char, // 'n', 'i', 'v', 'a' (all)
    pub lhs: String,
    pub rhs: String, // "<Cmd>..." or a raw key sequence
}

pub struct App {
    pub buffers: Vec<Buffer>,
    pub active: usize,
    pub mode: Mode,
    pub theme: Theme,

    // Command / search line.
    pub cmd: String,
    pub cmd_completions: Option<Vec<String>>,
    pub cmd_index: usize,

    // Multi-key saga: "gg", "dd", "dw", "gt" ...
    pub pending_g: bool,
    pub pending_d: bool,

    // Injected keys from user mappings.
    pub pending: VecDeque<KeyEvent>,

    pub register: Option<Register>,
    pub visual_anchor: (usize, usize),
    pub completion: Option<Completion>,

    pub explorer: Explorer,
    pub message: Option<(String, Instant)>,
    pub startup_ms: f64,
    pub should_quit: bool,
    pub last_search: Option<String>,
    pub options: Options,
    pub maps: Vec<Mapping>,
    pub lua: Option<LuaHost>,
    pub help_open: bool,
    pub help_scroll: u16,
    pub welcome_open: bool,
    pub pending_ctrl_k: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            active: 0,
            mode: Mode::Normal,
            theme: Theme::default(),
            cmd: String::new(),
            cmd_completions: None,
            cmd_index: 0,
            pending_g: false,
            pending_d: false,
            pending: VecDeque::new(),
            register: None,
            visual_anchor: (0, 0),
            completion: None,
            explorer: Explorer::new(),
            message: None,
            startup_ms: 0.0,
            should_quit: false,
            last_search: None,
            options: Options::default(),
            maps: Vec::new(),
            lua: None,
            help_open: false,
            help_scroll: 0,
            welcome_open: false,
            pending_ctrl_k: false,
        }
    }

    // ---- shorthands ------------------------------------------------------

    pub fn buf(&mut self) -> &mut Buffer {
        let active = self.active.min(self.buffers.len().saturating_sub(1));
        self.active = active;
        &mut self.buffers[active]
    }

    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), Instant::now()));
    }

    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual { line: false } => "VISUAL",
            Mode::Visual { line: true } => "V-LINE",
            Mode::Command { search: false } => "COMMAND",
            Mode::Command { search: true } => "SEARCH",
        }
    }

    // ---- buffers & files ---------------------------------------------------

    pub fn new_buffer(&mut self) {
        self.buffers.push(Buffer::untitled());
        self.active = self.buffers.len() - 1;
    }

    pub fn open_path(&mut self, path: &Path) {
        if path.is_dir() {
            self.explorer.dir = path.to_path_buf();
            self.explorer.show();
            return;
        }
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };
        let exists = absolute.exists();
        let canonical = if exists {
            std::fs::canonicalize(&absolute).unwrap_or(absolute.clone())
        } else {
            absolute
        };
        if let Some(i) = self
            .buffers
            .iter()
            .position(|b| b.path.as_deref() == Some(canonical.as_path()))
        {
            self.active = i;
            return;
        }
        if exists {
            self.buffers.push(Buffer::open(&canonical));
        } else {
            let mut b = Buffer::untitled();
            b.dirty = true;
            b.path = Some(canonical.clone());
            b.lang = Language::from_path(&canonical);
            self.buffers.push(b);
            self.set_message(format!("New file: {}", canonical.display()));
        }
        self.active = self.buffers.len() - 1;
    }

    pub fn save(&mut self) {
        let name = self.buf().name();
        let written = self.buf().save();
        match written {
            Ok(n) => self.set_message(format!(
                "\"{}\" {} bytes written",
                name,
                n.saturating_sub(1) // minus the trailing newline
            )),
            Err(e) => self.set_message(format!("Error saving: {e}")),
        }
    }

    pub fn save_succeeded(&mut self) -> bool {
        let result = self.buf().save();
        match result {
            Ok(n) => {
                let name = self.buf().name();
                self.set_message(format!("\"{}\" {} bytes written", name, n.saturating_sub(1)));
                true
            }
            Err(e) => {
                self.set_message(format!("Error saving: {e}"));
                false
            }
        }
    }

    pub fn save_as(&mut self, filename: &str) {
        if filename.is_empty() {
            self.set_message("E32: No file name");
            return;
        }
        let path = PathBuf::from(filename);
        let lang = Language::from_path(&path);
        self.buf().path = Some(path);
        self.buf().lang = lang;
        self.save();
    }

    /// Close the active buffer. `force` ignores unsaved changes.
    pub fn close_active(&mut self, force: bool) {
        if self.buf().dirty && !force {
            self.set_message("E37: No write since last change (add ! to override)");
            return;
        }
        self.buffers.remove(self.active);
        if self.buffers.is_empty() {
            self.should_quit = true;
            return;
        }
        self.active = self.active.saturating_sub(1).min(self.buffers.len() - 1);
        self.mode = Mode::Normal;
    }

    pub fn cycle_buffer(&mut self, forward: bool) {
        let n = self.buffers.len();
        if n < 2 {
            return;
        }
        self.active = if forward {
            (self.active + 1) % n
        } else {
            (self.active + n - 1) % n
        };
        self.mode = Mode::Normal;
    }

    // ---- dispatch --------------------------------------------------------

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.welcome_open {
            self.welcome_open = false;
        }
        if self.mode == Mode::Normal && !self.explorer.focused {
            if self.pending_ctrl_k {
                self.pending_ctrl_k = false;
                if key.code == KeyCode::Char('o') || key.code == KeyCode::Char('O') {
                    self.mode = Mode::Command { search: false };
                    self.cmd = "e ".into();
                    return;
                }
            } else if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('k') | KeyCode::Char('K'))
            {
                self.pending_ctrl_k = true;
                self.set_message("Press O to open a directory (Ctrl-K, then O)");
                return;
            }
        }
        if key.code == KeyCode::F(1) {
            self.help_open = !self.help_open;
            self.help_scroll = 0;
            return;
        }
        if self.help_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.help_open = false,
                KeyCode::Down | KeyCode::PageDown => {
                    self.help_scroll = self.help_scroll.saturating_add(8)
                }
                KeyCode::Up | KeyCode::PageUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(8)
                }
                _ => {}
            }
            return;
        }
        if self.mode == Mode::Normal && key.code == KeyCode::Char('?') {
            self.help_open = true;
            self.help_scroll = 0;
            return;
        }
        if !matches!(self.mode, Mode::Command { .. }) && !self.explorer.focused {
            if let Some(notation) = keys::notation_of(&key) {
                let mode_char = match self.mode {
                    Mode::Normal => 'n',
                    Mode::Insert => 'i',
                    Mode::Visual { .. } => 'v',
                    _ => 'n',
                };
                // 1) Lua function mappings (from init.lua).
                if let Some(host) = &mut self.lua {
                    if let Some(action) = host.map_lookup(mode_char, &notation) {
                        self.run_action(&action);
                        self.after_key();
                        return;
                    }
                }
                // 2) String mappings.
                if let Some(rhs) = lookup_map(&self.maps, mode_char, &notation) {
                    self.run_mapping(&rhs);
                    self.after_key();
                    return;
                }
                // 3) Built-in behavior for <C-s>, <C-b>, <C-n>, <C-p>.
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let KeyCode::Char(c) = key.code {
                        match c {
                            's' | 'S' => {
                                self.mode = Mode::Normal;
                                self.save();
                                self.after_key();
                                return;
                            }
                            'b' | 'B' => {
                                self.toggle_focus_explorer();
                                self.after_key();
                                return;
                            }
                            'n' | 'N' if self.mode == Mode::Normal => {
                                self.cycle_buffer(true);
                                self.after_key();
                                return;
                            }
                            'p' | 'P' if self.mode == Mode::Normal => {
                                self.cycle_buffer(false);
                                self.after_key();
                                return;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Visual { line } => self.handle_visual(key, line),
            Mode::Command { search } => self.handle_command(key, search),
        }
        self.after_key();
    }

    fn after_key(&mut self) {
        // Closing the last buffer sets `should_quit`; the event handler still
        // reaches its usual post-key cursor clamp before returning.
        if self.should_quit || self.buffers.is_empty() {
            return;
        }
        if self.mode == Mode::Insert {
            self.buf().clamp_cursor_insert();
        } else if matches!(self.mode, Mode::Normal | Mode::Visual { .. }) {
            self.buf().clamp_cursor_normal();
        }
    }

    /// Execute the right-hand side of a string mapping.
    fn run_mapping(&mut self, rhs: &str) {
        let rhs = rhs.trim();
        if let Some(rest) = rhs.strip_prefix("<Cmd>") {
            let cmd = rest
                .trim_end_matches("<CR>")
                .trim()
                .trim_start_matches(':')
                .to_string();
            command::execute(self, &cmd);
            return;
        }
        for k in keys::rhs_to_keys(rhs) {
            if self.pending.len() > 2048 {
                break; // mapping gone recursive; abort instead of wedging
            }
            self.pending.push_back(k);
        }
    }

    /// Execute an action string returned by a Lua handler.
    /// `":cmd"` → ex command; `"!text"` → echo; otherwise plain keys.
    pub fn run_action(&mut self, action: &str) {
        let action = action.trim();
        if let Some(msg) = action.strip_prefix('!') {
            self.set_message(msg.trim().to_string());
        } else if let Some(cmd) = action.strip_prefix(':') {
            command::execute(self, cmd.trim());
        } else if !action.is_empty() {
            self.run_mapping(action);
        }
    }

    pub fn drain_pending(&mut self) {
        let mut steps = 0usize;
        while !self.pending.is_empty() && steps < 5000 {
            steps += 1;
            let Some(key) = self.pending.pop_front() else {
                break;
            };
            self.handle_key(key);
            if self.should_quit {
                self.pending.clear();
                break;
            }
        }
        if steps >= 5000 {
            self.pending.clear();
            self.set_message("Recursive mapping was stopped (E170)");
        }
    }

    // ---- normal mode -------------------------------------------------------

    fn handle_normal(&mut self, key: KeyEvent) {
        if self.explorer.focused {
            self.handle_explorer(key);
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            if let KeyCode::Char(c) = key.code {
                match c {
                    'r' | 'R' => {
                        if self.buf().redo() {
                            self.set_message("redo");
                        } else {
                            self.set_message("Nothing to redo");
                        }
                        return;
                    }
                    'd' | 'D' => {
                        let count = self.buf().line_count().max(1);
                        let target = self.buf().cy + 12;
                        self.buf().cy = target.min(count - 1);
                        self.buf().clamp_cursor_normal();
                        return;
                    }
                    'u' | 'U' => {
                        self.buf().cy = self.buf().cy.saturating_sub(12);
                        self.buf().clamp_cursor_normal();
                        return;
                    }
                    _ => {}
                }
            }
        }
        let c = match key.code {
            KeyCode::Char(c) => c,
            KeyCode::Up => return self.buf().move_up(),
            KeyCode::Down => return self.buf().move_down(),
            KeyCode::Left => return self.buf().move_left(),
            KeyCode::Right => return self.buf().move_right(),
            KeyCode::Home => return self.buf().line_start(),
            KeyCode::End => return self.buf().line_end(),
            _ => return,
        };

        if self.pending_g {
            self.pending_g = false;
            match c {
                'g' => self.buf().cy = 0,
                't' => return self.cycle_buffer(true),
                'T' => return self.cycle_buffer(false),
                _ => {}
            }
            return;
        }
        if self.pending_d {
            self.pending_d = false;
            match c {
                'd' => {
                    let text = self.buf().delete_line();
                    self.register = Some(Register::Lines(vec![text]));
                }
                'w' => self.buf().delete_through_word_forward(),
                _ => {}
            }
            return;
        }

        match c {
            'h' => self.buf().move_left(),
            'j' => self.buf().move_down(),
            'k' => self.buf().move_up(),
            'l' => self.buf().move_right(),
            'w' => self.buf().word_forward(),
            'b' => self.buf().word_backward(),
            'e' => self.buf().word_end(),
            '0' => self.buf().line_start(),
            '$' => self.buf().line_end(),
            '^' => self.buf().first_non_blank(),
            'G' => {
                self.buf().cy = self.buf().line_count() - 1;
                self.buf().line_end();
            }
            'g' => self.pending_g = true,
            'd' => self.pending_d = true,
            'x' => {
                let removed = self.buf().delete_char();
                if !removed.is_empty() {
                    self.register = Some(Register::Chars(removed));
                }
            }
            'y' => {
                let cy = self.buf().cy;
                let line = self.buf().line(cy);
                self.register = Some(Register::Lines(vec![line]));
                self.set_message("1 line yanked");
            }
            'p' => {
                let (cx, cy) = (self.buf().cx, self.buf().cy);
                match self.register.clone() {
                    Some(Register::Lines(lines)) => self.buf().paste_lines(&lines),
                    Some(Register::Chars(text)) => {
                        let len = self.buf().line_len(cy);
                        self.buf().cx = (cx + 1).min(len);
                        self.buf().paste_text(&text);
                    }
                    None => self.set_message("Register is empty"),
                }
            }
            'P' => match self.register.clone() {
                Some(Register::Lines(lines)) => self.buf().paste_lines(&lines),
                Some(Register::Chars(text)) => self.buf().paste_text(&text),
                None => self.set_message("Register is empty"),
            },
            'J' => {
                let (cy, n) = (self.buf().cy, self.buf().line_count());
                if cy + 1 < n {
                    let next = self.buf().line(cy + 1);
                    let trimmed = next.trim_start().to_string();
                    self.buf().cy = cy + 1;
                    self.buf().cx = 0;
                    self.buf().delete_line();
                    let count = self.buf().line_count();
                    self.buf().cy = cy.min(count.saturating_sub(1));
                    self.buf().line_end();
                    if !trimmed.is_empty() {
                        self.buf().insert_text_raw(&format!(" {}", trimmed));
                    }
                    self.buf().clamp_cursor_normal();
                }
            }
            'i' => self.mode = Mode::Insert,
            'I' => {
                self.buf().first_non_blank();
                self.mode = Mode::Insert;
            }
            'a' => {
                let cy = self.buf().cy;
                if self.buf().line_len(cy) > 0 {
                    self.buf().cx += 1;
                }
                self.mode = Mode::Insert;
            }
            'A' => {
                self.buf().line_end();
                self.mode = Mode::Insert;
            }
            'o' => {
                self.buf().line_end();
                self.buf().newline();
                self.mode = Mode::Insert;
            }
            'O' => {
                self.buf().snapshot();
                let (cy, indent) = {
                    let b = self.buf();
                    let indent: String = b
                        .line(b.cy)
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect();
                    (b.cy, indent)
                };
                let g = self.buf().rope.line_to_char(cy);
                self.buf().rope.insert(g, &format!("{}\n", indent));
                self.buf().dirty = true;
                self.buf().cy = cy;
                self.buf().cx = indent.chars().count();
                self.mode = Mode::Insert;
            }
            'v' => {
                self.visual_anchor = (self.buf().cx, self.buf().cy);
                self.mode = Mode::Visual { line: false };
            }
            'V' => {
                self.visual_anchor = (self.buf().cx, self.buf().cy);
                self.mode = Mode::Visual { line: true };
            }
            'u' => {
                if self.buf().undo() {
                    self.set_message("undo");
                } else {
                    self.set_message("Nothing to undo");
                }
            }
            ':' => {
                self.cmd.clear();
                self.cmd_completions = None;
                self.cmd_index = 0;
                self.mode = Mode::Command { search: false };
            }
            '/' => {
                self.cmd.clear();
                self.cmd_completions = None;
                self.mode = Mode::Command { search: true };
            }
            'n' => self.jump_search(true),
            'N' => self.jump_search(false),
            'z' => {} // reserved
            _ => {}
        }
    }

    // ---- insert mode ---------------------------------------------------------

    fn handle_insert(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.completion.is_some() && !is_word_char(c) {
                    self.completion = None;
                }
                if self.options.expandtab && c == '\t' {
                    let spaces = " ".repeat(self.options.tabstop);
                    self.buf().insert_text_raw(&spaces);
                } else {
                    self.buf().insert_char(c);
                }
                if is_word_char(c) {
                    self.update_insert_completion();
                }
            }
            KeyCode::Tab => {
                if self.accept_completion() {
                    return;
                }
                if self.options.expandtab {
                    let spaces = " ".repeat(self.options.tabstop);
                    self.buf().insert_text_raw(&spaces);
                } else {
                    self.buf().insert_char('\t');
                }
            }
            KeyCode::Backspace => {
                if self.completion.take().is_some() {
                    self.buf().backspace();
                    self.update_insert_completion();
                    return;
                }
                self.buf().backspace();
            }
            KeyCode::Enter => {
                self.completion = None;
                self.buf().newline();
            }
            KeyCode::Esc => {
                if self.completion.take().is_some() {
                    return; // first Esc just closes the popup
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Up => self.buf().move_up(),
            KeyCode::Down => self.buf().move_down(),
            KeyCode::Left => {
                if self.buf().cx > 0 {
                    self.buf().cx -= 1;
                }
            }
            KeyCode::Right => {
                let cy = self.buf().cy;
                if self.buf().cx < self.buf().line_len(cy) {
                    self.buf().cx += 1;
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_completion(1);
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_completion(-1);
            }
            _ => {}
        }
    }

    // ---- visual mode ---------------------------------------------------------

    fn handle_visual(&mut self, key: KeyEvent, line_mode: bool) {
        let c = match key.code {
            KeyCode::Char(c) => c,
            KeyCode::Up => {
                self.buf().move_up();
                return;
            }
            KeyCode::Down => {
                self.buf().move_down();
                return;
            }
            KeyCode::Left => {
                self.buf().move_left();
                return;
            }
            KeyCode::Right => {
                self.buf().move_right();
                return;
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                return;
            }
            _ => return,
        };

        if self.pending_g {
            self.pending_g = false;
            match c {
                'g' => self.buf().cy = 0,
                't' => return self.cycle_buffer(true),
                'T' => return self.cycle_buffer(false),
                _ => {}
            }
            return;
        }

        match c {
            'h' => self.buf().move_left(),
            'j' => self.buf().move_down(),
            'k' => self.buf().move_up(),
            'l' => self.buf().move_right(),
            'w' => self.buf().word_forward(),
            'b' => self.buf().word_backward(),
            'e' => self.buf().word_end(),
            '0' => self.buf().line_start(),
            '$' => self.buf().line_end(),
            '^' => self.buf().first_non_blank(),
            'G' => {
                self.buf().cy = self.buf().line_count() - 1;
                self.buf().line_end();
            }
            'g' => self.pending_g = true,
            'v' | 'V' => {
                self.mode = Mode::Visual { line: !line_mode };
                self.visual_anchor = (self.buf().cx, self.buf().cy);
            }
            'd' | 'x' => self.visual_delete(line_mode),
            'y' => {
                if line_mode {
                    let (y0, y1) = self.visual_rows();
                    let lines: Vec<String> = (y0..=y1)
                        .map(|y| {
                            let b = self.buf();
                            b.line(y)
                        })
                        .collect();
                    self.register = Some(Register::Lines(lines));
                } else {
                    let ((sy, sx), (ey, ex)) = self.sel_range(false);
                    let text = self.buf().copy_span((sy, sx), (ey, ex));
                    self.register = Some(Register::Chars(text));
                }
                self.mode = Mode::Normal;
            }
            'c' => {
                self.visual_delete(line_mode);
                self.mode = Mode::Insert;
            }
            _ => {}
        }
    }

    /// Selection range ordered start..end. Line mode ignores columns.
    pub fn sel_range(&self, line_mode: bool) -> ((usize, usize), (usize, usize)) {
        let a = self.visual_anchor;
        let b = self
            .buffers
            .get(self.active)
            .map(|b| (b.cx, b.cy))
            .unwrap_or((0, 0));
        if line_mode {
            if a.1 <= b.1 {
                ((0, a.1), (usize::MAX, b.1))
            } else {
                ((0, b.1), (usize::MAX, a.1))
            }
        } else if (a.1, a.0) <= (b.1, b.0) {
            (a, b)
        } else {
            (b, a)
        }
    }

    pub fn visual_rows(&self) -> (usize, usize) {
        let a = self.visual_anchor.1;
        let b = self.buffers.get(self.active).map(|buf| buf.cy).unwrap_or(0);
        (a.min(b), a.max(b))
    }

    fn visual_delete(&mut self, line_mode: bool) {
        if line_mode {
            let (y0, y1) = self.visual_rows();
            let removed = self.buf().delete_lines_range(y0, y1);
            self.register = Some(Register::Lines(removed));
            self.set_message(format!("{} lines deleted", self.register_lines_len()));
        } else {
            let ((sy, sx), (ey, ex)) = self.sel_range(false);
            let removed = self.buf().delete_span((sy, sx), (ey, ex));
            self.register = Some(Register::Chars(removed));
        }
        self.mode = Mode::Normal;
    }

    fn register_lines_len(&self) -> usize {
        match &self.register {
            Some(Register::Lines(lines)) => lines.len(),
            _ => 0,
        }
    }

    // ---- command / search mode ----------------------------------------------

    fn handle_command(&mut self, key: KeyEvent, search: bool) {
        match key.code {
            KeyCode::Esc => {
                self.cmd_completions = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.cmd.pop();
                if self.cmd.is_empty() {
                    self.mode = Mode::Normal;
                }
                self.cmd_completions = None;
            }
            KeyCode::Char(c) => {
                self.cmd.push(c);
                self.cmd_completions = None;
                self.cmd_index = 0;
            }
            KeyCode::Tab if !search => {
                self.command_complete();
            }
            KeyCode::Enter => {
                let text = self.cmd.clone();
                self.mode = Mode::Normal;
                if search {
                    if !text.is_empty() {
                        self.last_search = Some(text.clone());
                        self.jump_search(true);
                    }
                } else {
                    command::execute(self, &text);
                }
            }
            _ => {}
        }
    }

    fn command_complete(&mut self) {
        if self.cmd_completions.is_none() {
            let items = command::candidates(&self.cmd);
            if items.is_empty() {
                self.set_message("No matches");
                return;
            }
            self.cmd_completions = Some(items);
            self.cmd_index = 0;
        }
        let list = self.cmd_completions.clone().unwrap();
        if list.is_empty() {
            self.cmd_completions = None;
            return;
        }
        let choice = list[self.cmd_index % list.len()].clone();
        self.cmd_index = (self.cmd_index + 1) % list.len();
        self.cmd = choice;
    }

    // ---- completion (insert mode, from buffer words) --------------------------

    fn update_insert_completion(&mut self) {
        let (cx, cy) = (self.buf().cx, self.buf().cy);
        let line = self.buf().line(cy);
        let chars: Vec<char> = line.chars().collect();
        let mut start = cx.min(chars.len());
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }
        let prefix: String = chars[start..cx.min(chars.len())].iter().collect();
        if prefix.chars().count() < 2 {
            self.completion = None;
            return;
        }
        let mut items: Vec<String> = Vec::new();
        'outer: for l in 0..self.buf().line_count().min(4000) {
            for tok in self
                .buf()
                .line(l)
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            {
                if tok.len() > prefix.len()
                    && tok.starts_with(&prefix)
                    && !items.iter().any(|i| i == tok)
                {
                    items.push(tok.to_string());
                    if items.len() >= 60 {
                        break 'outer;
                    }
                }
            }
        }
        if items.is_empty() {
            self.completion = None;
        } else {
            self.completion = Some(Completion {
                prefix,
                items,
                index: 0,
            });
        }
    }

    fn cycle_completion(&mut self, dir: i32) {
        if let Some(comp) = &mut self.completion {
            let n = comp.items.len() as i32;
            if n > 0 {
                comp.index = ((comp.index as i32 + dir).rem_euclid(n)) as usize;
            }
        }
    }

    /// Tab in insert mode: replace the prefix with the selected candidate.
    /// Returns true if a completion was accepted.
    fn accept_completion(&mut self) -> bool {
        let Some(comp) = self.completion.take() else {
            return false;
        };
        let Some(word) = comp.items.get(comp.index % comp.items.len()).cloned() else {
            return false;
        };
        let (cx, cy) = (self.buf().cx, self.buf().cy);
        if cx >= comp.prefix.chars().count() {
            let start = cx - comp.prefix.chars().count();
            let b = self.buf();
            b.delete_span((start, cy), (cx - 1, cy));
            b.insert_text_raw(&word);
        }
        true
    }

    // ---- search ----------------------------------------------------------------

    pub fn jump_search(&mut self, forward: bool) {
        let Some(pattern) = self.last_search.clone() else {
            self.set_message("E35: No previous regular expression");
            return;
        };
        let (start_x, start_y) = (self.buf().cx, self.buf().cy);
        let n = self.buf().line_count();
        if n == 0 {
            return;
        }
        let mut hit: Option<(usize, usize)> = None;
        if forward {
            if let Some(x) = self.buf().search_in_line(start_y, &pattern, start_x + 1) {
                hit = Some((x, start_y));
            }
            if hit.is_none() {
                for step in 1..=n {
                    let y = (start_y + step) % n;
                    if let Some(x) = self.buf().search_in_line(y, &pattern, 0) {
                        hit = Some((x, y));
                        break;
                    }
                }
            }
        } else if start_x > 0 {
            // last occurrence before the cursor on this line
            let hay = self.buf().line(start_y).to_lowercase();
            let nd = pattern.to_lowercase();
            let mut best: Option<usize> = None;
            for (bi, _) in hay.match_indices(&nd) {
                let ci = hay[..bi].chars().count();
                if ci < start_x {
                    best = Some(ci);
                }
            }
            hit = best.map(|x| (x, start_y));
        }
        if hit.is_none() {
            for step in 1..=n {
                let y = (start_y + n - (step % n)) % n;
                let hay = self.buf().line(y).to_lowercase();
                let nd = pattern.to_lowercase();
                let mut last: Option<usize> = None;
                for (bi, _) in hay.match_indices(&nd) {
                    last = Some(hay[..bi].chars().count());
                }
                if let Some(x) = last {
                    hit = Some((x, y));
                    break;
                }
            }
        }
        match hit {
            Some((x, y)) => {
                self.buf().cy = y;
                self.buf().cx = x;
                self.buf().clamp_cursor_normal();
            }
            None => self.set_message(format!("E486: Pattern not found: {}", pattern)),
        }
    }

    // ---- explorer ----------------------------------------------------------------

    pub fn toggle_focus_explorer(&mut self) {
        if self.explorer.open {
            self.explorer.focused = !self.explorer.focused;
        } else {
            self.explorer.show();
        }
    }

    fn handle_explorer(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') => {
                self.explorer.selected =
                    (self.explorer.selected + 1).min(self.explorer.entries.len().saturating_sub(1));
            }
            KeyCode::Char('k') => self.explorer.selected = self.explorer.selected.saturating_sub(1),
            KeyCode::Char('h') => {
                if self.explorer.dir.parent().is_some() {
                    self.explorer.dir.pop();
                    self.explorer.refresh();
                    self.explorer.selected = 0;
                }
            }
            KeyCode::Enter | KeyCode::Char('l') => self.explorer_open_current(),
            KeyCode::Esc => {
                self.explorer.open = false;
                self.explorer.focused = false;
            }
            KeyCode::Char('q') => self.explorer.focused = false,
            _ => {}
        }
    }

    pub fn explorer_open_current(&mut self) {
        let Some(entry) = self
            .explorer
            .entries
            .get(self.explorer.selected)
            .cloned()
            .map(|e| e.name)
        else {
            return;
        };
        if entry == ".." {
            self.explorer.dir.pop();
            self.explorer.refresh();
            self.explorer.selected = 0;
            return;
        }
        let path = self.explorer.dir.join(&entry);
        if path.is_dir() {
            self.explorer.dir = path;
            self.explorer.refresh();
            self.explorer.selected = 0;
        } else {
            self.open_path(&path);
            self.explorer.focused = false;
        }
    }

    // ---- one-off helpers ------------------------------------------------------------

    /// Buffer top-of-window scrolling shared by UI (kept off render code).
    pub fn ensure_visible(&mut self, rows: usize, cols: usize, gutter: usize) {
        let b = self.buf();
        if rows > 0 {
            if b.cy < b.top {
                b.top = b.cy;
            }
            if b.cy >= b.top + rows {
                b.top = b.cy + 1 - rows;
            }
        }
        let text_cols = cols.saturating_sub(gutter);
        if text_cols > 4 {
            if b.cx < b.left {
                b.left = b.cx;
            } else if b.cx >= b.left + text_cols - 2 {
                b.left = (b.cx + 2).saturating_sub(text_cols);
            }
        }
    }
}

fn lookup_map(maps: &[Mapping], mode: char, notation: &str) -> Option<String> {
    maps.iter()
        .rev()
        .find(|m| m.lhs == notation && (m.mode == mode || m.mode == 'a'))
        .map(|m| m.rhs.clone())
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
