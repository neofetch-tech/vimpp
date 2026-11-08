//! Rendering: tabs strip, file explorer, editor with syntax spans,
//! status line, command line and the two completion popups.
//!
//! Rendering borrows read-only snapshots of the editor state: no allocation
//! beyond the visible viewport, no `&mut` inside the frame closure.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Mode};
use crate::buffer::Buffer;
use crate::highlight::{highlight_line, style_for};
use crate::theme::Theme;

/// Read-only borrow of the active buffer (rendering never mutates).
fn cur(app: &App) -> &Buffer {
    let i = app.active.min(app.buffers.len().saturating_sub(1));
    &app.buffers[i]
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab strip
            Constraint::Min(3),    // explorer + editor
            Constraint::Length(1), // status line
            Constraint::Length(1), // command line
        ])
        .split(area);

    let title = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(10)])
        .split(chunks[0]);
    draw_tabs(frame, app, title[0]);
    draw_help_button(frame, app, title[1]);

    let body = if app.explorer.open {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Min(20)])
            .split(chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0)])
            .split(chunks[1])
    };

    let editor_area = if app.explorer.open {
        draw_explorer(frame, app, body[0]);
        body[1]
    } else {
        body[0]
    };

    if app.help_open {
        draw_help(frame, app, chunks[1]);
    } else {
        draw_editor(frame, app, editor_area);
    }
    draw_status(frame, app, chunks[2]);
    draw_cmdline(frame, app, chunks[3]);
    draw_popups(frame, app, chunks[3], editor_area);
}

fn draw_help_button(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    frame.render_widget(
        Paragraph::new("[ HELP ]")
            .alignment(ratatui::layout::Alignment::Right)
            .style(
                Style::default()
                    .fg(t.accent_ink)
                    .bg(t.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        area,
    );
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let lines = vec![
        Line::from(Span::styled("QUICK START", Style::default().fg(t.cyan).add_modifier(Modifier::BOLD))),
        Line::from("1. Press i to start typing code. Type normally."),
        Line::from("2. Press Esc to stop typing and return to navigation and commands."),
        Line::from("3. Type :w and press Enter to save. Use :q to quit or :wq to save and quit."),
        Line::from(""),
        Line::from(Span::styled("MODES AND MOVEMENT", Style::default().fg(t.cyan).add_modifier(Modifier::BOLD))),
        Line::from("Normal mode navigates and runs commands. Insert mode types code. Visual mode selects text."),
        Line::from("Arrow keys or h/j/k/l move. w/b move by word; 0/$ go to line start/end; gg/G go to file start/end."),
        Line::from("v selects characters; V selects lines. Esc clears selection; d deletes, y copies, p pastes."),
        Line::from(""),
        Line::from(Span::styled("EDITING AND SEARCH", Style::default().fg(t.cyan).add_modifier(Modifier::BOLD))),
        Line::from("x deletes a character; dd a line; dw a word. yy copies a line; p/P paste; u undo; Ctrl-R redo."),
        Line::from("Type /text to search. Press Enter, then n for the next match or N for the previous one."),
        Line::from("In Insert mode, Enter keeps indentation. Tab accepts a completion or inserts indentation."),
        Line::from(""),
        Line::from(Span::styled("FILES AND PROJECT TREE", Style::default().fg(t.cyan).add_modifier(Modifier::BOLD))),
        Line::from("Ctrl-B opens the file tree. j/k select; Enter/l opens; h goes to the parent folder; q leaves the tree."),
        Line::from(":e path opens a file; :w path saves as. :q! quits without saving."),
        Line::from("gt/gT or Ctrl-N/Ctrl-P switch open files. :bn/:bp do the same; :enew creates a blank buffer."),
        Line::from(""),
        Line::from(Span::styled("SETTINGS", Style::default().fg(t.cyan).add_modifier(Modifier::BOLD))),
        Line::from("Absolute line numbers are on by default. :set relativenumber shows cursor distances; :set norelativenumber turns them off."),
        Line::from(":set number / :set nonumber shows or hides line numbers. :set tabstop=N changes indentation width."),
        Line::from("Syntax highlighting: Rust, JS/TS, Python, C, Go, Lua, HTML, CSS, JSON, TOML, Markdown, SQL, YAML, and Shell."),
        Line::from("Lua plugins load from ~/.vimplus/init.lua. Nordic colors are in config.toml; choose JetBrains Mono in terminal settings."),
        Line::from("F1 or [ HELP ] opens this guide. Esc closes it; Up/Down scroll."),
    ];
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((app.help_scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(t.accent))
                    .title(" Vim++ Help — Up/Down scroll · F1/Esc close "),
            )
            .style(Style::default().fg(t.text).bg(t.bg)),
        area,
    );
}

// ---- tab strip -----------------------------------------------------------------

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let mut spans = vec![Span::styled(" ", Style::default().bg(t.panel))];

    for (i, buf) in app.buffers.iter().enumerate() {
        let active = i == app.active;
        let (fg, bg) = if active {
            (t.accent_ink, t.accent)
        } else {
            (t.text_dim, t.panel)
        };
        let label = format!(" {} ", buf.display_name());
        spans.push(Span::styled(
            label,
            Style::default().fg(fg).bg(bg).add_modifier(if active {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ));
        spans.push(Span::styled("  ", Style::default().bg(t.bg)));
    }
    spans.push(Span::styled(
        " ".repeat(area.width as usize),
        Style::default().bg(t.bg),
    ));

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(t.bg)),
        area,
    );
}

// ---- explorer ------------------------------------------------------------------

fn draw_explorer(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 6 {
        return;
    }
    let t = &app.theme;
    let border_style = if app.explorer.focused {
        t.accent
    } else {
        t.border
    };

    let dir_name = app
        .explorer
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".into());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_style))
        .title(Span::styled(
            format!(" {dir_name} "),
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(t.panel));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = inner.height as usize;
    let items: Vec<Line> = app
        .explorer
        .entries
        .iter()
        .enumerate()
        .skip(visible_start(app.explorer.selected, rows))
        .take(rows)
        .map(|(i, entry)| {
            let selected = i == app.explorer.selected;
            let (icon, fg) = if entry.name == ".." {
                ("⤴ ", t.text_dim)
            } else if entry.is_dir {
                ("▾ ", t.accent)
            } else {
                ("  ", t.text)
            };
            let (bg, weight) = if selected {
                (t.selection, Modifier::BOLD)
            } else {
                (t.panel, Modifier::empty())
            };
            Line::from(vec![
                Span::styled(icon, Style::default().fg(t.mint).bg(bg)),
                Span::styled(
                    entry.name.clone(),
                    Style::default().fg(fg).bg(bg).add_modifier(weight),
                ),
                Span::styled(
                    " ".repeat(
                        (inner.width as usize)
                            .saturating_sub(entry.name.chars().count() + icon.chars().count() + 1),
                    ),
                    Style::default().bg(bg),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(items), inner);
}

fn visible_start(selected: usize, rows: usize) -> usize {
    if rows == 0 || selected < rows {
        0
    } else {
        selected + 1 - rows
    }
}

// ---- editor ----------------------------------------------------------------------

fn draw_editor(frame: &mut Frame, app: &mut App, area: Rect) {
    // 1) Snapshot what drawing needs, then stop touching `app`.
    let (number, relative, visual_active, line_mode) = (
        app.options.number,
        app.options.relativenumber,
        matches!(app.mode, Mode::Visual { .. }),
        matches!(app.mode, Mode::Visual { line: true }),
    );
    let (cursor_fg_select, cursor_line_bg) = (app.theme.selection, app.theme.cursor_line);
    let sel = app.visual_rows();
    let caret_mode = matches!(app.mode, Mode::Insert | Mode::Command { .. });

    // 2) Scroll math via a plain borrow (Buffer scroll fields).
    app.ensure_visible(
        area.height as usize,
        area.width as usize,
        gutter_width(cur(app)),
    );

    // 3) Everything below reads through immutable snapshots.
    let active_idx = app.active;
    let buffer = app.buffers.get_mut(active_idx).expect("buffer exists");

    let gutter = gutter_width(buffer);
    let (cx, cy) = (buffer.cx, buffer.cy);
    let (top, left) = (buffer.top, buffer.left);
    let lang = buffer.lang;
    let theme_snapshot: &Theme = &app.theme;

    let rows = area.height as usize;
    for i in 0..rows {
        let li = top + i;
        if li >= buffer.line_count() {
            break;
        }
        let y = area.y + i as u16;
        let row = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        };
        draw_row(
            frame,
            buffer,
            theme_snapshot,
            row,
            li,
            cx,
            cy,
            left,
            gutter,
            number,
            relative,
            visual_active,
            line_mode,
            sel,
            cursor_line_bg,
            cursor_fg_select,
            lang,
        );
    }

    if caret_mode {
        let col = area.x as usize
            + gutter
            + cx.saturating_sub(left)
                .min(area.width.saturating_sub(gutter as u16) as usize);
        let row = area.y as usize + (cy.saturating_sub(top));
        if col < area.x as usize + area.width as usize
            && row < area.y as usize + area.height as usize
        {
            frame.set_cursor_position((col as u16, row as u16));
        }
    } else {
        mark_cursor_char(frame, cx, left, gutter, area, cy, top);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    frame: &mut Frame,
    b: &Buffer,
    t: &Theme,
    row: Rect,
    li: usize,
    cx: usize,
    cy: usize,
    left: usize,
    gutter: usize,
    number: bool,
    relative: bool,
    visual_active: bool,
    line_mode: bool,
    sel: (usize, usize),
    cursor_line_bg: Color,
    selection_bg: Color,
    lang: crate::highlight::Language,
) {
    let is_cur = li == cy;
    let in_sel = visual_active && sel.0 <= li && li <= sel.1;
    let row_bg = if (is_cur && !visual_active) || (line_mode && in_sel) {
        Some(cursor_line_bg)
    } else {
        None
    };
    let mut spans: Vec<Span> = Vec::new();

    if number || relative {
        let shown = if relative && !is_cur {
            li.abs_diff(cy)
        } else if number {
            li + 1
        } else {
            0
        };
        let num_style = if is_cur {
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.muted)
        };
        spans.push(Span::styled(
            format!("{:>w$} ", shown, w = gutter - 1),
            num_style,
        ));
    }

    let line_text = b.line(li);
    let visible: String = line_text
        .chars()
        .skip(left)
        .take(row.width.saturating_sub(gutter as u16) as usize)
        .collect();

    for token in highlight_line(lang, &visible) {
        let text: String = visible
            .chars()
            .skip(token.start)
            .take(token.end - token.start)
            .collect();
        let mut style = style_for(t, token.kind);
        if in_sel {
            style = style.bg(selection_bg);
        } else if let Some(bg) = row_bg {
            style = style.bg(bg);
        }
        spans.push(Span::styled(text, style));
    }

    let used = gutter + visible.chars().count();
    if (used as u16) < row.width {
        let mut pad_style = Style::default();
        if in_sel {
            pad_style = pad_style.bg(selection_bg);
        } else if let Some(bg) = row_bg {
            pad_style = pad_style.bg(bg);
        }
        spans.push(Span::styled(
            " ".repeat((row.width - used as u16) as usize),
            pad_style,
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), row);
    let _ = cx;
}

fn mark_cursor_char(
    frame: &mut Frame,
    cx: usize,
    left: usize,
    gutter: usize,
    area: Rect,
    cy: usize,
    top: usize,
) {
    if cy < top {
        return;
    }
    let row_y = area.y + (cy - top) as u16;
    if row_y >= area.y + area.height {
        return;
    }
    let off = cx.saturating_sub(left);
    let col = area.x as usize
        + gutter
        + off.min(area.width.saturating_sub(gutter as u16).max(1) as usize);
    let col = col.min(area.x as usize + area.width as usize - 1) as u16;
    let _ = frame.buffer_mut().cell_mut((col, row_y)).map(|cell| {
        cell.set_style(Style::default().add_modifier(Modifier::UNDERLINED));
    });
}

fn gutter_width(b: &Buffer) -> usize {
    let digits = b.line_count().to_string().len().max(3);
    digits + 1
}

// ---- status line ------------------------------------------------------------------

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let b = cur(app);
    let (mode_fg, mode_bg) = match app.mode {
        Mode::Normal => (t.accent_ink, t.accent),
        Mode::Insert => (t.text, t.border_soft),
        Mode::Visual { line: true } => (t.accent_ink, t.mint),
        Mode::Visual { line: false } => (t.accent_ink, t.mint),
        Mode::Command { search: true } => (t.text, t.search),
        Mode::Command { search: false } => (t.text, t.panel_alt),
    };
    let mode_label = match app.mode {
        Mode::Visual { line: true } => " V-LINE ".to_string(),
        _ => format!(" {} ", app.mode_name()),
    };

    let right_text = format!(
        " {} ⚡ {:.1}ms  Ln {}, Col {} ",
        b.lang.badge(),
        app.startup_ms,
        b.cy + 1,
        b.cx + 1
    );
    let right_width = right_text.chars().count() as u16;

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(right_width.min(area.width)),
        ])
        .split(area);

    let left_spans = vec![
        Span::styled(
            mode_label,
            Style::default()
                .fg(mode_fg)
                .bg(mode_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", b.display_name()),
            Style::default().fg(t.text),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).style(Style::default().bg(t.status_bg)),
        chunks[0],
    );
    let right_spans = vec![Span::styled(right_text, Style::default().fg(t.text_dim))];
    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).style(Style::default().bg(t.status_bg)),
        chunks[1],
    );
}

// ---- command line -----------------------------------------------------------------

fn draw_cmdline(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let bg = Style::default().bg(t.bg);

    if let Some((msg, _)) = &app.message {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {}", msg),
                Style::default().fg(t.yellow).bg(t.bg),
            ))),
            area,
        );
        return;
    }

    match app.mode {
        Mode::Command { search } => {
            let prompt = if search { "/" } else { ":" };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        prompt,
                        Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(app.cmd.clone(), Style::default().fg(t.text)),
                ]))
                .style(bg),
                area,
            );
        }
        _ => {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(
                        " Vim++ v{} — type :help or press F1 for help",
                        env!("CARGO_PKG_VERSION")
                    ),
                    Style::default().fg(t.muted),
                )))
                .style(bg),
                area,
            );
        }
    }
}

// ---- popups -----------------------------------------------------------------------

fn draw_popups(frame: &mut Frame, app: &App, cmdline: Rect, editor: Rect) {
    match app.mode {
        Mode::Command { search: false } => {
            let Some(items) = app.cmd_completions.clone() else {
                return;
            };
            if items.is_empty() {
                return;
            }
            let width = (items.iter().map(|s| s.chars().count()).max().unwrap_or(8) as u16 + 6)
                .min(editor.width.saturating_sub(4));
            let height = (items.len() as u16).min(8);
            let x = if cmdline.width > width + 4 {
                cmdline.x + cmdline.width - width - 2
            } else {
                cmdline.x
            };
            let y = cmdline.y.saturating_sub(height);
            render_popup(
                frame,
                &app.theme,
                Rect {
                    x,
                    y,
                    width,
                    height,
                },
                &items,
                app.cmd_index % items.len(),
            );
        }
        _ => {
            let Some(comp) = &app.completion else { return };
            if comp.items.is_empty() {
                return;
            }
            let shown: Vec<String> = comp
                .items
                .iter()
                .map(|s| format!("{}  [buffer]", s))
                .collect();
            let width = (shown.iter().map(|s| s.chars().count()).max().unwrap_or(10) as u16 + 4)
                .min(editor.width.saturating_sub(8));
            let height = (comp.items.len() as u16).min(8);
            let rect = Rect {
                x: editor.x.saturating_add(4),
                y: cmdline.y.saturating_sub(height),
                width,
                height,
            };
            render_popup(
                frame,
                &app.theme,
                rect,
                &shown,
                comp.index % comp.items.len(),
            );
        }
    }
}

fn render_popup(frame: &mut Frame, t: &Theme, rect: Rect, items: &[String], selected: usize) {
    if rect.width == 0 || rect.height == 0 || items.is_empty() {
        return;
    }
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent_dim))
        .style(Style::default().bg(t.completion_bg));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let sel = i == selected;
            let bg = if sel { t.accent } else { t.completion_bg };
            let fg = if sel { t.accent_ink } else { t.text };
            let label = format!(" {}", item);
            let pad = (rect.width as usize)
                .saturating_sub(label.chars().count() + 1)
                .max(1);
            Line::from(vec![
                Span::styled(
                    label,
                    Style::default().fg(fg).bg(bg).add_modifier(if sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                ),
                Span::styled(" ".repeat(pad), Style::default().fg(fg).bg(bg)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
