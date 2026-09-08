mod actions;
mod app;
mod config;
mod input;
mod log;
mod render;
mod session;
mod store;
mod terminal;
mod ui;
mod usage;

use anyhow::Result;
use app::App;
use crossterm::event;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use input::{handle_event, Action};
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use session::SessionManager;
use std::io::{stdout, IsTerminal};
use std::time::{Duration, Instant};

use actions::{process_action, resize_panes, screen_area};
use app::ViewMode;
use ui::Panes;

pub type Term = Terminal<CrosstermBackend<std::io::Stdout>>;

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(16);

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "version" | "--version" | "-v" => {
                println!("c9s {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "help" | "--help" | "-h" => {
                println!("c9s - Claude Code Session Manager");
                println!();
                println!("Usage:");
                println!("  c9s              Launch the TUI dashboard");
                println!("  c9s version      Show version");
                println!("  c9s usage-log    Print recent usage samples and keep-warm pings");
                return Ok(());
            }
            "usage-log" => {
                let limit = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(60);
                print_usage_log(limit)?;
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                eprintln!("Run 'c9s help' for usage.");
                std::process::exit(1);
            }
        }
    }

    if !stdout().is_terminal() {
        eprintln!("Error: c9s requires an interactive terminal (TTY).");
        std::process::exit(1);
    }

    if !SessionManager::is_claude_installed() {
        eprintln!("Error: claude CLI not found. Install it first: https://docs.anthropic.com/en/docs/claude-code");
        std::process::exit(1);
    }

    let mut app = App::new()?;

    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let msg = format!("PANIC: {}\n{}", info, bt);
        tlog!(error, "{}", msg);
        eprintln!("{}", msg);
    }));

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    if let Err(ref e) = result {
        tlog!(error, "run_loop returned error: {}", e);
    }

    stdout().execute(DisableMouseCapture)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn print_usage_log(limit: usize) -> Result<()> {
    let store = store::Store::open()?;
    let samples = store.recent_usage_samples(limit)?;
    println!(
        "{:<20} {:>4} {:>4}  {:<5} {:<9} {}",
        "time", "5h%", "7d%", "event", "session", "context"
    );
    for s in samples {
        let time = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(s.ts_ms)
            .map(|t| {
                t.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default();
        let pct = |v: Option<u8>| v.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
        let session = s
            .session_id
            .as_deref()
            .map(|id| id[..8.min(id.len())].to_string())
            .unwrap_or_default();
        let context = s
            .context_tokens
            .map(|c| format!("{}K", c / 1000))
            .unwrap_or_default();
        println!(
            "{:<20} {:>4} {:>4}  {:<5} {:<9} {}",
            time,
            pct(s.five_hour),
            pct(s.seven_day),
            s.event,
            session,
            context
        );
    }
    Ok(())
}

fn run_loop(terminal: &mut Term, app: &mut App) -> Result<()> {
    let mut last_refresh = Instant::now();
    let mut needs_draw = true;
    let mut mouse_captured = true;
    let mut layout: Option<LayoutKey> = None;

    loop {
        needs_draw |= poll_background(app);
        needs_draw |= sync_pty_layout(terminal, app, &mut layout)?;

        if needs_draw {
            terminal.draw(|f| render::draw_frame(f, app))?;
            needs_draw = false;
        }

        if event::poll(POLL_INTERVAL)? {
            needs_draw |= handle_events(terminal, app)?;
            if app.should_quit() {
                break;
            }
        }

        let needs_native_mouse = app.in_terminal_context() || *app.view_mode() == ViewMode::Log;
        if needs_native_mouse && mouse_captured {
            stdout().execute(DisableMouseCapture)?;
            mouse_captured = false;
        } else if !needs_native_mouse && !mouse_captured {
            stdout().execute(EnableMouseCapture)?;
            mouse_captured = true;
        }

        let viewing_active = app.in_terminal_context();
        let notified = app
            .terminal_manager_mut()
            .check_and_forward_notifications(viewing_active)
            | app.poll_registry(viewing_active);
        if notified {
            app.invalidate_usage();
            needs_draw = true;
        }

        app.terminal_manager_mut().cleanup_inactive_exited();

        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh_usage();
            last_refresh = Instant::now();
            needs_draw = true;
        }

        if app.should_quit() {
            tlog!(info, "run_loop exiting, view_mode={:?}", app.view_mode());
            break;
        }
    }

    Ok(())
}

type LayoutKey = (Panes, Option<String>);

fn sync_pty_layout(terminal: &Term, app: &App, layout: &mut Option<LayoutKey>) -> Result<bool> {
    let key = (app.panes(), app.attached_session_id().map(str::to_string));
    let changed = layout.as_ref() != Some(&key);
    *layout = Some(key);
    if changed && app.in_terminal_context() {
        resize_panes(app, screen_area(terminal)?);
        return Ok(true);
    }
    Ok(false)
}

fn poll_background(app: &mut App) -> bool {
    let mut dirty = false;

    if app.in_terminal_context() {
        if let Some(term) = app.terminal_manager().active_terminal() {
            dirty |= term.take_dirty();
        }
    }

    if app.is_side_panel_open() {
        if let Some(st) = app.side_terminal() {
            dirty |= st.take_dirty();
        }
    }

    dirty |= app.poll_discovery();
    dirty |= app.poll_keep_warm();
    app.drain_ci_statuses();
    app.check_ci_statuses();

    if *app.view_mode() == ViewMode::Log && log::take_dirty() {
        dirty = true;
    }

    dirty
}

fn handle_events(terminal: &Term, app: &mut App) -> Result<bool> {
    let mut needs_draw = false;
    loop {
        let ev = event::read()?;

        if let event::Event::Resize(cols, rows) = ev {
            resize_panes(app, Rect::new(0, 0, cols, rows));
            needs_draw = true;
        }

        let action = handle_event(
            &ev,
            app.view_mode(),
            app.is_side_panel_focused(),
            &app.config.keys,
        );
        let is_noop = matches!(
            action,
            Action::None | Action::TerminalInput(_) | Action::SideTerminalInput(_)
        );
        if let Err(e) = process_action(app, action, terminal) {
            tlog!(
                error,
                "process_action error: {} (view={:?})",
                e,
                app.view_mode()
            );
            return Err(e);
        }
        needs_draw |= !is_noop;

        if app.should_quit() || !event::poll(Duration::from_millis(0))? {
            break;
        }
    }
    Ok(needs_draw)
}
