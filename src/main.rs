//! Entry point: config → Lua runtime → files → UI loop → restore terminal.
//! Startup is timed end-to-end because the whole point of Vim++ is that it
//! boots before your blink finishes.

mod app;
mod buffer;
mod command;
mod config;
mod explorer;
mod highlight;
mod keys;
mod lua;
mod theme;
mod ui;

use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};

fn main() -> Result<()> {
    let start = Instant::now();

    let cli: Vec<String> = std::env::args().skip(1).collect();
    if cli.iter().any(|a| a == "--version" || a == "-V") {
        println!("Vim++ v{} — NamASTE ✌", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut app = app::App::new();
    config::load(&mut app);
    load_lua(&mut app);

    if cli.iter().any(|a| a == "--bench") {
        // Dry run: everything except the TUI loop. The Lua host + config are
        // the whole "boot cost" story; ratatui init adds single-digit ms.
        println!(
            "⚡ preload (config + Lua host): {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        app.lua = None;
        return Ok(());
    }

    for path in &cli {
        app.open_path(std::path::Path::new(path));
    }
    if app.buffers.is_empty() {
        app.new_buffer();
    }
    app.startup_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut terminal = ratatui::init();
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
    let result = run(&mut terminal, &mut app);
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
    ratatui::restore();

    result?;

    println!(
        "⚡ Vim++ ready — booted in {:.1} ms. NamASTE ✌",
        app.startup_ms
    );
    Ok(())
}

/// Initialize the Lua host, load `~/.vimplus/init.lua` when present, drain
/// its load-time outbox into the editor (messages, options, commands).
fn load_lua(app: &mut app::App) {
    let mut host = match lua::LuaHost::new() {
        Ok(host) => host,
        Err(e) => {
            app.set_message(format!("Lua runtime unavailable: {e}"));
            return;
        }
    };
    if let Err(e) = host.bootstrap(lua::BOOTSTRAP) {
        app.set_message(e);
        return; // API table missing: skip scripts entirely
    }

    let init = config::config_dir().join("init.lua");
    let script_result = match std::fs::read_to_string(&init) {
        Ok(src) => host.run_script(&src, "init.lua"),
        Err(_) => Ok(()), // no init.lua = stock Vim++ behavior
    };

    match script_result.and_then(|_| host.collect()) {
        Ok(result) => {
            if let Some(first) = result.log.first() {
                app.set_message(first.clone());
            }
            for option in result.options {
                config::apply_set(app, &option);
            }
            for deferred in result.commands {
                command::execute(app, deferred.trim().trim_start_matches(':'));
            }
            for error in result.errors {
                app.set_message(error);
            }
        }
        Err(e) => app.set_message(e),
    }

    app.lua = Some(host);
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut app::App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if let Some((_, stamp)) = &app.message {
            if stamp.elapsed() > Duration::from_secs(4) {
                app.message = None;
            }
        }

        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                Event::Mouse(mouse)
                    if mouse.kind == MouseEventKind::Down(MouseButton::Left) && mouse.row == 0 =>
                {
                    if mouse.column >= terminal.size()?.width.saturating_sub(10) {
                        app.help_open = !app.help_open;
                        app.help_scroll = 0;
                    }
                }
                _ => {}
            }
        }

        app.drain_pending();
    }
    Ok(())
}
