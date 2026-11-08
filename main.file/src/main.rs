use std::{
    cell::RefCell,
    env, fs, io,
    path::{Path, PathBuf},
    ptr::NonNull,
    rc::Rc,
    time::Duration,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mlua::{Function, Lua, Table};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};
use walkdir::WalkDir;

unsafe extern "C" {
    fn vimpp_buffer_new() -> *mut CoreBuffer;
    fn vimpp_buffer_free(buffer: *mut CoreBuffer);
    fn vimpp_buffer_data(buffer: *const CoreBuffer) -> *const std::ffi::c_char;
    fn vimpp_buffer_len(buffer: *const CoreBuffer) -> usize;
    fn vimpp_buffer_set(buffer: *mut CoreBuffer, data: *const std::ffi::c_char, len: usize) -> i32;
    fn vimpp_buffer_insert(
        buffer: *mut CoreBuffer,
        at: usize,
        data: *const std::ffi::c_char,
        len: usize,
    ) -> i32;
    fn vimpp_buffer_delete(buffer: *mut CoreBuffer, at: usize, len: usize) -> i32;
}

#[repr(C)]
struct CoreBuffer {
    _private: [u8; 0],
}

struct Buffer(NonNull<CoreBuffer>);
impl Buffer {
    fn new() -> Self {
        Self(NonNull::new(unsafe { vimpp_buffer_new() }).expect("C core buffer allocation failed"))
    }
    fn text(&self) -> String {
        unsafe {
            let ptr = vimpp_buffer_data(self.0.as_ptr());
            let len = vimpp_buffer_len(self.0.as_ptr());
            String::from_utf8_lossy(std::slice::from_raw_parts(ptr.cast::<u8>(), len)).into_owned()
        }
    }
    fn set(&mut self, value: &str) {
        unsafe {
            vimpp_buffer_set(self.0.as_ptr(), value.as_ptr().cast(), value.len());
        }
    }
    fn insert(&mut self, at: usize, value: &str) {
        unsafe {
            vimpp_buffer_insert(self.0.as_ptr(), at, value.as_ptr().cast(), value.len());
        }
    }
    fn delete(&mut self, at: usize, len: usize) {
        unsafe {
            vimpp_buffer_delete(self.0.as_ptr(), at, len);
        }
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            vimpp_buffer_free(self.0.as_ptr());
        }
    }
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
    Command,
}

struct App {
    buffer: Buffer,
    cursor: usize,
    mode: Mode,
    command: String,
    message: String,
    path: Option<PathBuf>,
    dirty: bool,
    tree: Vec<PathBuf>,
    selected: usize,
    tree_open: bool,
    root: PathBuf,
    exit_requested: bool,
    help_open: bool,
    history: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
    search_pattern: String,
    lua: Lua,
    lua_notice: Rc<RefCell<Option<String>>>,
}

impl App {
    fn new(arg: Option<PathBuf>) -> Self {
        let root = arg
            .as_ref()
            .and_then(|p| {
                if p.is_dir() {
                    Some(p.clone())
                } else {
                    p.parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .map(Path::to_path_buf)
                }
            })
            .unwrap_or_else(|| env::current_dir().unwrap_or_default());
        let mut app = Self {
            buffer: Buffer::new(),
            cursor: 0,
            mode: Mode::Normal,
            command: String::new(),
            message: "Vim++  ·  i insert  ·  :w save  ·  Ctrl-N project tree".into(),
            path: arg.clone().filter(|p| !p.is_dir()),
            dirty: false,
            tree: Vec::new(),
            selected: 0,
            tree_open: true,
            root,
            exit_requested: false,
            help_open: false,
            history: Vec::new(),
            redo: Vec::new(),
            search_pattern: String::new(),
            lua: Lua::new(),
            lua_notice: Rc::new(RefCell::new(None)),
        };
        if let Some(p) = arg {
            if p.is_dir() {
                app.root = p;
            } else if let Ok(text) = fs::read_to_string(&p) {
                app.buffer.set(&text);
            }
        }
        app.refresh_tree();
        app
    }

    fn refresh_tree(&mut self) {
        self.tree = WalkDir::new(&self.root).max_depth(6).into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.path() != self.root && !entry.path().components().any(|part| {
                matches!(part, std::path::Component::Normal(name) if name == ".git" || name == "target" || name == ".idea")
            }))
            .map(|entry| entry.path().to_path_buf()).collect();
        self.tree.sort();
        self.selected = self.selected.min(self.tree.len().saturating_sub(1));
    }

    fn open(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.root = path;
            self.refresh_tree();
            self.message = format!("Project: {}", self.root.display());
        } else if let Ok(text) = fs::read_to_string(&path) {
            self.buffer.set(&text);
            self.cursor = 0;
            self.path = Some(path.clone());
            self.dirty = false;
            self.message = format!("Opened {}", path.display());
        } else {
            self.message = format!("Could not open {}", path.display());
        }
    }

    fn save(&mut self) {
        if let Some(path) = self.path.clone() {
            match fs::write(&path, self.buffer.text()) {
                Ok(()) => {
                    self.dirty = false;
                    self.message = format!("Saved {}", path.display());
                }
                Err(err) => self.message = format!("Save error: {err}"),
            }
        } else {
            self.message = "Use :w path/to/file to save this buffer".into();
        }
    }

    fn checkpoint(&mut self) {
        self.history.push((self.buffer.text(), self.cursor));
        self.redo.clear();
    }

    fn undo(&mut self, reverse: bool) {
        let (from, to) = if reverse {
            (&mut self.redo, &mut self.history)
        } else {
            (&mut self.history, &mut self.redo)
        };
        if let Some((text, cursor)) = from.pop() {
            to.push((self.buffer.text(), self.cursor));
            self.buffer.set(&text);
            self.cursor = cursor.min(text.len());
            self.dirty = true;
            self.message = if reverse { "Redo" } else { "Undo" }.into();
        } else {
            self.message = if reverse {
                "Nothing to redo"
            } else {
                "Nothing to undo"
            }
            .into();
        }
    }

    fn search(&mut self, forward: bool) {
        if self.search_pattern.is_empty() {
            self.message = "No search pattern; use /text".into();
            return;
        }
        let text = self.buffer.text();
        let found = if forward {
            text.get(self.cursor.saturating_add(1)..)
                .and_then(|part| part.find(&self.search_pattern).map(|n| self.cursor + 1 + n))
                .or_else(|| text.find(&self.search_pattern))
        } else {
            text.get(..self.cursor)
                .and_then(|part| part.rfind(&self.search_pattern))
                .or_else(|| text.rfind(&self.search_pattern))
        };
        if let Some(at) = found {
            self.cursor = at;
            self.message = format!("Search: {}", self.search_pattern);
        } else {
            self.message = format!("Pattern not found: {}", self.search_pattern);
        }
    }

    fn execute_command(&mut self) -> bool {
        let command = self.command.trim().to_owned();
        self.mode = Mode::Normal;
        self.command.clear();
        if command == "help" {
            self.help_open = true;
        } else if command == "tree" {
            self.tree_open = !self.tree_open;
        } else if let Some(pattern) = command.strip_prefix('/') {
            self.search_pattern = pattern.to_owned();
            self.search(true);
        } else if command == "w" || command == "write" {
            self.save();
        } else if command == "q" || command == "quit" {
            if self.dirty {
                self.message = "Unsaved changes; use :q! to discard".into();
            } else {
                self.exit_requested = true;
            }
        } else if command == "q!" || command == "quit!" {
            self.exit_requested = true;
        } else if command == "wq" || command == "x" {
            self.save();
            if !self.dirty {
                self.exit_requested = true;
            }
        } else if let Some(path) = command
            .strip_prefix("e ")
            .or_else(|| command.strip_prefix("edit "))
        {
            self.open(self.root.join(path));
        } else if let Some(path) = command
            .strip_prefix("w ")
            .or_else(|| command.strip_prefix("write "))
        {
            self.path = Some(self.root.join(path));
            self.save();
        } else {
            self.message = format!("Unknown command: :{command}");
        }
        self.exit_requested
    }

    fn key(&mut self, key: KeyEvent) -> bool {
        let consumed = self.lua_key_hook(&key);
        if let Some(notice) = self.lua_notice.borrow_mut().take() {
            self.message = notice;
        }
        if consumed {
            return false;
        }
        if key.code == KeyCode::F(1) {
            self.help_open = !self.help_open;
            return false;
        }
        if self.help_open {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.help_open = false;
            }
            return false;
        }
        if self.mode == Mode::Command {
            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    self.command.clear();
                }
                KeyCode::Enter => {
                    self.execute_command();
                    if self.exit_requested {
                        return true;
                    }
                }
                KeyCode::Backspace => {
                    self.command.pop();
                }
                KeyCode::Char(ch) => self.command.push(ch),
                _ => {}
            }
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
            self.tree_open = !self.tree_open;
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('z') {
            self.undo(false);
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            self.undo(true);
            return false;
        }
        if self.mode == Mode::Insert {
            match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Enter => {
                    let text = self.buffer.text();
                    let line_start = text[..self.cursor].rfind('\n').map_or(0, |n| n + 1);
                    let indent: String = text[line_start..self.cursor]
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect();
                    let insertion = format!("\n{indent}");
                    self.checkpoint();
                    self.buffer.insert(self.cursor, &insertion);
                    self.cursor += insertion.len();
                    self.dirty = true;
                }
                KeyCode::Backspace if self.cursor > 0 => {
                    let text = self.buffer.text();
                    let previous = prev_boundary(&text, self.cursor);
                    self.checkpoint();
                    self.buffer.delete(previous, self.cursor - previous);
                    self.cursor = previous;
                    self.dirty = true;
                }
                KeyCode::Tab => {
                    self.checkpoint();
                    self.buffer.insert(self.cursor, "    ");
                    self.cursor += 4;
                    self.dirty = true;
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let value = ch.to_string();
                    self.checkpoint();
                    self.buffer.insert(self.cursor, &value);
                    self.cursor += value.len();
                    self.dirty = true;
                }
                _ => {}
            }
            return false;
        }
        let text = self.buffer.text();
        if self.tree_open && key.code == KeyCode::Down {
            self.selected = (self.selected + 1).min(self.tree.len().saturating_sub(1));
            return false;
        }
        if self.tree_open && key.code == KeyCode::Up {
            self.selected = self.selected.saturating_sub(1);
            return false;
        }
        match key.code {
            KeyCode::Char('i') => self.mode = Mode::Insert,
            KeyCode::Char('a') => {
                self.cursor = next_boundary(&text, self.cursor);
                self.mode = Mode::Insert;
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command.clear();
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Command;
                self.command = "/".into();
            }
            KeyCode::Char('?') => self.help_open = true,
            KeyCode::Char('u') => self.undo(false),
            KeyCode::Char('n') => self.search(true),
            KeyCode::Char('N') => self.search(false),
            KeyCode::Char('h') | KeyCode::Left => self.cursor = prev_boundary(&text, self.cursor),
            KeyCode::Char('l') | KeyCode::Right => self.cursor = next_boundary(&text, self.cursor),
            KeyCode::Char('j') | KeyCode::Down => self.move_vertical(&text, 1),
            KeyCode::Char('k') | KeyCode::Up => self.move_vertical(&text, -1),
            KeyCode::Char('x') if self.cursor < text.len() => {
                let end = next_boundary(&text, self.cursor);
                self.checkpoint();
                self.buffer.delete(self.cursor, end - self.cursor);
                self.dirty = true;
            }
            KeyCode::Char('o') => {
                let line_start = text[..self.cursor].rfind('\n').map_or(0, |n| n + 1);
                let indent: String = text[line_start..self.cursor]
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                let end = text[self.cursor..]
                    .find('\n')
                    .map(|n| self.cursor + n)
                    .unwrap_or(text.len());
                self.cursor = end;
                self.checkpoint();
                self.buffer.insert(self.cursor, &format!("\n{indent}"));
                self.cursor += indent.len() + 1;
                self.mode = Mode::Insert;
                self.dirty = true;
            }
            KeyCode::Enter if self.tree_open => {
                if let Some(path) = self.tree.get(self.selected).cloned() {
                    self.open(path);
                }
            }
            _ => {}
        }
        false
    }

    fn lua_key_hook(&self, key: &KeyEvent) -> bool {
        let name = match key.code {
            KeyCode::Char(ch) => ch.to_string(),
            KeyCode::Enter => "Enter".into(),
            KeyCode::Esc => "Esc".into(),
            KeyCode::Tab => "Tab".into(),
            KeyCode::Backspace => "Backspace".into(),
            KeyCode::Up => "Up".into(),
            KeyCode::Down => "Down".into(),
            KeyCode::Left => "Left".into(),
            KeyCode::Right => "Right".into(),
            KeyCode::F(n) => format!("F{n}"),
            _ => return false,
        };
        let globals = self.lua.globals();
        let api = match globals.get::<Table>("vimpp") {
            Ok(api) => api,
            Err(_) => return false,
        };
        let hook = match api.get::<Function>("on_key") {
            Ok(hook) => hook,
            Err(_) => return false,
        };
        hook.call::<bool>(name).unwrap_or(false)
    }

    fn move_vertical(&mut self, text: &str, direction: isize) {
        let line_start = text[..self.cursor].rfind('\n').map_or(0, |p| p + 1);
        let column = self.cursor - line_start;
        let target = if direction < 0 {
            if line_start == 0 {
                return;
            }
            let end = line_start - 1;
            let start = text[..end].rfind('\n').map_or(0, |p| p + 1);
            Some((start, end))
        } else {
            text[self.cursor..]
                .find('\n')
                .map(|p| self.cursor + p + 1)
                .map(|start| {
                    let end = text[start..].find('\n').map_or(text.len(), |p| start + p);
                    (start, end)
                })
        };
        if let Some((start, end)) = target {
            self.cursor = start + column.min(end - start);
        }
    }
}

fn prev_boundary(text: &str, pos: usize) -> usize {
    text[..pos.min(text.len())]
        .char_indices()
        .last()
        .map_or(0, |(i, _)| i)
}
fn next_boundary(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        text.len()
    } else {
        pos + text[pos..].chars().next().map_or(0, char::len_utf8)
    }
}

fn main() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    if let Some(option) = args.next() {
        if option == "--version" || option == "-V" {
            println!("Vim++ {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        if option == "--help" || option == "-h" {
            println!(
                "Vim++ {}\nUsage: vimpp [file-or-project-directory]\n\nPass a file to edit it, a directory to open it as the project, or no path to open the current directory.",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(());
        }
        let path = PathBuf::from(option);
        let mut app = App::new(Some(path));
        load_lua(&mut app);
        return run_terminal(&mut app);
    }
    let mut app = App::new(None);
    load_lua(&mut app);
    run_terminal(&mut app)
}

fn run_terminal(app: &mut App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal, app);
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    result
}

fn load_lua(app: &mut App) {
    let globals = app.lua.globals();
    let api = app.lua.create_table().unwrap();
    api.set("version", env!("CARGO_PKG_VERSION")).unwrap();
    let notice = Rc::clone(&app.lua_notice);
    let notify = app
        .lua
        .create_function(move |_, text: String| {
            *notice.borrow_mut() = Some(text);
            Ok(())
        })
        .unwrap();
    api.set("notify", notify).unwrap();
    globals.set("vimpp", api).unwrap();
    let lua_path = Path::new("lua/init.lua");
    let source = fs::read_to_string(lua_path).or_else(|_| {
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("lua/init.lua"))
    });
    if let Ok(source) = source {
        if let Err(err) = app
            .lua
            .load(&source)
            .set_name(lua_path.display().to_string())
            .exec()
        {
            app.message = format!("Lua error: {err}");
        }
    }
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if app.key(key) {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse)
                    if mouse.kind == MouseEventKind::Down(crossterm::event::MouseButton::Left)
                        && mouse.row == 0
                        && mouse.column >= terminal.size()?.width.saturating_sub(10) =>
                {
                    app.help_open = !app.help_open;
                }
                _ => {}
            }
        }
    }
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    let header = Line::from(vec![
        Span::styled(
            " Vim++ ",
            Style::default()
                .fg(Color::Rgb(46, 52, 64))
                .bg(Color::Rgb(163, 190, 140))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " {} ",
                match app.mode {
                    Mode::Normal => "NORMAL",
                    Mode::Insert => "INSERT",
                    Mode::Command => "COMMAND",
                }
            ),
            Style::default()
                .fg(Color::Rgb(46, 52, 64))
                .bg(Color::Rgb(136, 192, 208)),
        ),
        Span::raw("  "),
        Span::styled(
            app.path
                .as_ref()
                .map_or("[No Name]".into(), |p| p.display().to_string()),
            Style::default().fg(Color::Rgb(216, 222, 233)),
        ),
        Span::raw(if app.dirty { " [+]" } else { "" }),
    ]);
    let header_parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(10)])
        .split(rows[0]);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(Color::Rgb(59, 66, 82))),
        header_parts[0],
    );
    frame.render_widget(
        Paragraph::new("[ HELP ]")
            .alignment(Alignment::Right)
            .style(
                Style::default()
                    .fg(Color::Rgb(46, 52, 64))
                    .bg(Color::Rgb(163, 190, 140))
                    .add_modifier(Modifier::BOLD),
            ),
        header_parts[1],
    );
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if app.tree_open {
            vec![Constraint::Length(30), Constraint::Min(1)]
        } else {
            vec![Constraint::Length(0), Constraint::Min(1)]
        })
        .split(rows[1]);
    if app.tree_open {
        let entries = app
            .tree
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let relative = path.strip_prefix(&app.root).unwrap_or(path);
                let indent = "  ".repeat(relative.components().count().saturating_sub(1));
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let icon = if path.is_dir() { "▸ " } else { "  " };
                let style = if i == app.selected {
                    Style::default()
                        .fg(Color::Rgb(46, 52, 64))
                        .bg(Color::Rgb(136, 192, 208))
                } else {
                    Style::default().fg(Color::Rgb(216, 222, 233))
                };
                Line::from(format!("{indent}{icon}{name}")).style(style)
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(entries).block(
                Block::default()
                    .title(" Project ")
                    .borders(Borders::RIGHT)
                    .border_style(Style::default().fg(Color::Rgb(76, 86, 106))),
            ),
            body[0],
        );
    }
    let text = app.buffer.text();
    let (line, column) = cursor_position(&text, app.cursor);
    if app.help_open {
        let help = vec![
            Line::from(Span::styled(
                "Vim++ Help",
                Style::default()
                    .fg(Color::Rgb(163, 190, 140))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "MODES",
                Style::default()
                    .fg(Color::Rgb(136, 192, 208))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(
                "Normal mode: navigate and run editor commands. Press i to insert, a to insert after cursor, or o to open a line below.",
            ),
            Line::from(
                "Insert mode: type code; Enter keeps the current indentation, Tab inserts four spaces, Backspace deletes. Esc returns to Normal.",
            ),
            Line::from("Command mode: press : and type a command. Press /text to search for text."),
            Line::from(""),
            Line::from(Span::styled(
                "EDITING",
                Style::default()
                    .fg(Color::Rgb(136, 192, 208))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(
                "h j k l or arrow keys move the cursor. x deletes a character. u / Ctrl-Z undo; Ctrl-R redo.",
            ),
            Line::from("/text searches forward. n repeats forward; N repeats backward."),
            Line::from(
                "n finds the next match and N finds the previous match for the latest /search.",
            ),
            Line::from(
                "Syntax colors are selected by file extension for Rust, C/C++, Lua, Python, JavaScript, and TypeScript.",
            ),
            Line::from(""),
            Line::from(Span::styled(
                "FILES AND PROJECT TREE",
                Style::default()
                    .fg(Color::Rgb(136, 192, 208))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(
                "Start with vimpp [path]. A file opens in the buffer; a directory becomes the project root.",
            ),
            Line::from(
                "Ctrl-N toggles the tree. Use Up/Down to choose an item and Enter to open it or enter a directory.",
            ),
            Line::from(
                ":e path opens a file relative to the project root. :w saves; :w path saves as.",
            ),
            Line::from(
                ":q exits when there are no unsaved edits; :q! discards edits; :wq or :x saves and exits.",
            ),
            Line::from(""),
            Line::from(Span::styled(
                "EXTENSIONS AND APPEARANCE",
                Style::default()
                    .fg(Color::Rgb(136, 192, 208))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(
                "Lua startup extensions load from lua/init.lua. The Lua global vimpp.version contains the version.",
            ),
            Line::from(
                "Nordic colors are built in. Choose JetBrains Mono in your terminal settings for the intended font.",
            ),
            Line::from(""),
            Line::from(Span::styled(
                "Press F1, click [ HELP ], or press Esc to return to the editor.",
                Style::default().fg(Color::Rgb(235, 203, 139)),
            )),
        ];
        frame.render_widget(
            Paragraph::new(help)
                .wrap(ratatui::widgets::Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::LEFT)
                        .title(" Help ")
                        .border_style(Style::default().fg(Color::Rgb(46, 52, 64)))
                        .padding(Padding::horizontal(1)),
                ),
            body[1],
        );
    } else {
        let extension = app
            .path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let lines = text
            .split('\n')
            .enumerate()
            .map(|(i, row)| {
                let mut spans = vec![Span::styled(
                    format!("{:>4} ", i + 1),
                    Style::default().fg(Color::Rgb(76, 86, 106)),
                )];
                spans.extend(highlight_line(row, extension));
                Line::from(spans)
            })
            .collect::<Vec<_>>();
        let pane = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(Color::Rgb(46, 52, 64)))
                .padding(Padding::horizontal(1)),
        );
        frame.render_widget(pane, body[1]);
        let x = body[1]
            .x
            .saturating_add(6)
            .saturating_add(column.saturating_sub(1) as u16);
        let y = body[1].y.saturating_add(line.saturating_sub(1) as u16);
        if x < body[1].right() && y < body[1].bottom() {
            frame.set_cursor_position((x, y));
        }
    }
    let status = if app.mode == Mode::Command {
        format!(":{}", app.command)
    } else {
        app.message.clone()
    };
    frame.render_widget(
        Paragraph::new(status).style(
            Style::default()
                .fg(Color::Rgb(216, 222, 233))
                .bg(Color::Rgb(59, 66, 82)),
        ),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(format!(
            " {}:{}   UTF-8   {}",
            line,
            column,
            app.root.display()
        ))
        .style(
            Style::default()
                .fg(Color::Rgb(163, 190, 140))
                .bg(Color::Rgb(46, 52, 64)),
        ),
        rows[3],
    );
}

fn cursor_position(text: &str, pos: usize) -> (usize, usize) {
    let before = &text[..pos.min(text.len())];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before.rfind('\n').map_or(before.chars().count(), |at| {
        before[at + 1..].chars().count()
    }) + 1;
    (line, column)
}

fn highlight_line(row: &str, extension: &str) -> Vec<Span<'static>> {
    let bytes = row.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    let comment_mark = if extension == "lua" {
        "--"
    } else if matches!(extension, "py" | "sh" | "yaml" | "yml") {
        "#"
    } else {
        "//"
    };
    while i < bytes.len() {
        let start = i;
        let style;
        if row[i..].starts_with(comment_mark) {
            spans.push(Span::styled(
                row[i..].to_owned(),
                Style::default().fg(Color::Rgb(97, 110, 128)),
            ));
            break;
        } else if bytes[i] == b'"'
            || bytes[i] == b'\''
            || (bytes[i] == b'`' && matches!(extension, "js" | "ts" | "jsx" | "tsx"))
        {
            let quote = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += row[i..].chars().next().unwrap().len_utf8();
                }
            }
            style = Style::default().fg(Color::Rgb(163, 190, 140));
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &row[start..i];
            style = if is_keyword(word) {
                Style::default()
                    .fg(Color::Rgb(180, 142, 173))
                    .add_modifier(Modifier::BOLD)
            } else if matches!(
                word,
                "true" | "false" | "None" | "nil" | "null" | "self" | "this"
            ) {
                Style::default().fg(Color::Rgb(208, 135, 112))
            } else {
                Style::default().fg(Color::Rgb(216, 222, 233))
            };
        } else if bytes[i].is_ascii_digit() {
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'.' | b'_'))
            {
                i += 1;
            }
            style = Style::default().fg(Color::Rgb(208, 135, 112));
        } else {
            i += row[i..].chars().next().unwrap().len_utf8();
            style = if bytes[start].is_ascii_punctuation() {
                Style::default().fg(Color::Rgb(136, 192, 208))
            } else {
                Style::default().fg(Color::Rgb(216, 222, 233))
            };
        }
        spans.push(Span::styled(row[start..i].to_owned(), style));
    }
    spans
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "let"
            | "mut"
            | "pub"
            | "use"
            | "mod"
            | "struct"
            | "enum"
            | "impl"
            | "trait"
            | "type"
            | "where"
            | "match"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "return"
            | "break"
            | "continue"
            | "async"
            | "await"
            | "move"
            | "const"
            | "static"
            | "unsafe"
            | "extern"
            | "class"
            | "function"
            | "var"
            | "new"
            | "import"
            | "from"
            | "export"
            | "default"
            | "try"
            | "catch"
            | "throw"
            | "def"
            | "pass"
            | "raise"
            | "with"
            | "as"
            | "lambda"
            | "local"
            | "then"
            | "end"
            | "do"
            | "repeat"
            | "until"
            | "int"
            | "char"
            | "void"
            | "include"
            | "define"
            | "sizeof"
    )
}
