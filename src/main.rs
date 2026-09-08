mod app;
mod input;
mod log;
mod session;
mod store;
mod terminal;
mod ui;
mod usage;

use anyhow::Result;
use app::{App, ViewMode};
use crossterm::event;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use input::{handle_event, Action};
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::{Frame, Terminal};
use session::SessionManager;
use std::io::{stdout, IsTerminal};
use std::path::PathBuf;
use std::time::{Duration, Instant};

type Term = Terminal<CrosstermBackend<std::io::Stdout>>;

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
                println!("  c9s           Launch the TUI dashboard");
                println!("  c9s version   Show version");
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

fn is_terminal_view(mode: &ViewMode) -> bool {
    matches!(mode, ViewMode::Terminal | ViewMode::TerminalQSwitcher)
}

fn run_loop(terminal: &mut Term, app: &mut App) -> Result<()> {
    let mut last_refresh = Instant::now();
    let mut needs_draw = true;
    let mut mouse_captured = true;

    loop {
        needs_draw |= poll_background(app);

        if needs_draw {
            terminal.draw(|f| draw_frame(f, app))?;
            needs_draw = false;
        }

        if event::poll(POLL_INTERVAL)? {
            needs_draw |= handle_events(terminal, app)?;
            if app.should_quit() {
                break;
            }
        }

        let needs_native_mouse =
            is_terminal_view(app.view_mode()) || *app.view_mode() == ViewMode::Log;
        if needs_native_mouse && mouse_captured {
            stdout().execute(DisableMouseCapture)?;
            mouse_captured = false;
        } else if !needs_native_mouse && !mouse_captured {
            stdout().execute(EnableMouseCapture)?;
            mouse_captured = true;
        }

        let viewing_active = is_terminal_view(app.view_mode());
        if app
            .terminal_manager_mut()
            .check_and_forward_notifications(viewing_active)
        {
            app.invalidate_usage();
        }

        app.terminal_manager_mut().cleanup_inactive_exited();

        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            if is_terminal_view(app.view_mode()) {
                app.refresh_usage();
            } else {
                app.refresh()?;
            }
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

fn poll_background(app: &mut App) -> bool {
    let mut dirty = false;

    if is_terminal_view(app.view_mode()) {
        if let Some(term) = app.terminal_manager().active_terminal() {
            dirty |= term.take_dirty();
        }
    }

    if app.is_side_panel_open() {
        if let Some(st) = app.side_terminal() {
            dirty |= st.take_dirty();
        }
    }

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
            handle_resize(app, cols, rows);
            needs_draw = true;
        }

        let action = handle_event(&ev, app.view_mode(), app.is_side_panel_focused());
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

fn handle_resize(app: &mut App, cols: u16, rows: u16) {
    if is_terminal_view(app.view_mode()) {
        let term_cols = if app.is_side_panel_open() {
            cols * 60 / 100
        } else {
            cols
        };
        let _ = app
            .terminal_manager()
            .resize_active(rows.saturating_sub(2), term_cols);
    }
    if app.is_side_panel_open() {
        let panel_cols = cols * 40 / 100;
        let panel_rows = rows.saturating_sub(1);
        if let Some(st) = app.side_terminal() {
            let _ = st.resize(panel_rows, panel_cols);
        }
    }
}

fn draw_frame(f: &mut Frame, app: &App) {
    let full_area = f.area();
    let (area, side_area) = if app.is_side_panel_open() {
        ui::split_with_side_panel(full_area)
    } else {
        (full_area, Rect::default())
    };

    match app.view_mode() {
        ViewMode::List | ViewMode::Filter => ui::render_session_list(f, app, area),
        ViewMode::Detail => render_detail_view(app, f, area),
        ViewMode::SessionFilePicker => {
            render_detail_view(app, f, area);
            ui::render_session_file_picker(f, &app.session_files, app.session_file_cursor, area);
        }
        ViewMode::QSwitcher => {
            ui::render_session_list(f, app, area);
            ui::render_qswitcher(f, app, area);
        }
        ViewMode::Help => {
            ui::render_session_list(f, app, area);
            ui::render_help(f, area);
        }
        ViewMode::Terminal => render_terminal_view(app, f, area),
        ViewMode::TerminalQSwitcher => {
            render_terminal_view(app, f, area);
            ui::render_qswitcher(f, app, area);
        }
        ViewMode::Command => {
            ui::render_session_list(f, app, area);
            ui::render_command_input(f, app.command_input(), area);
        }
        ViewMode::ConfirmKill => {
            ui::render_session_list(f, app, area);
            let name = app
                .confirm_kill_session_id
                .as_ref()
                .and_then(|id| app.all_sessions().iter().find(|s| &s.id == id))
                .map(|s| s.project_name.clone())
                .unwrap_or_else(|| "unknown".to_string());
            ui::render_confirm_kill(f, &name, area);
        }
        ViewMode::ConfirmQuit => {
            ui::render_session_list(f, app, area);
            let active = app.active_attached_sessions();
            ui::render_confirm_quit(f, &active, area);
        }
        ViewMode::Log => {
            let entries = log::entries();
            ui::render_log_panel(f, &entries, app.log_scroll(), area);
        }
    }

    if app.is_side_panel_open() {
        if let Some(st) = app.side_terminal() {
            ui::render_side_panel(f, st, app.is_side_panel_focused(), side_area);
        }
    }
}

fn render_detail_view(app: &App, f: &mut Frame, area: Rect) {
    if let Some(session) = app.selected_session() {
        ui::render_session_detail(
            f,
            session,
            app.detail_items(),
            app.detail_cursor(),
            app.detail_preview(),
            app.detail_preview_scroll(),
            area,
        );
    }
}

fn render_terminal_view(app: &App, f: &mut Frame, area: Rect) {
    if let Some(term) = app.terminal_manager().active_terminal() {
        let guard = term.lock_parser();
        let screen = guard.screen();
        let scrolled = screen.scrollback() > 0;
        let exited = term.is_exited();
        let tabs = app.terminal_manager().tab_info();
        ui::render_terminal(f, screen, &tabs, exited, scrolled, app.usage(), area);
    }
}

fn pty_dims(terminal: &Term) -> Result<(u16, u16)> {
    let area = terminal.size()?;
    Ok((area.height.saturating_sub(1), area.width))
}

fn process_action(app: &mut App, action: Action, terminal: &Term) -> Result<()> {
    match action {
        Action::Quit => {
            if app.active_attached_sessions().is_empty() {
                app.quit();
            } else {
                app.set_view_mode(ViewMode::ConfirmQuit);
            }
        }
        Action::ConfirmQuit => app.quit(),
        Action::CancelQuit => app.set_view_mode(ViewMode::List),
        Action::KillSession => {
            if let Some(session) = app.selected_session() {
                if session.pid.is_some() {
                    app.confirm_kill_session_id = Some(session.id.clone());
                    app.set_view_mode(ViewMode::ConfirmKill);
                }
            }
        }
        Action::ConfirmKill => {
            if let Some(session_id) = app.confirm_kill_session_id.take() {
                if let Some(pid) = app
                    .all_sessions()
                    .iter()
                    .find(|s| s.id == session_id)
                    .and_then(|s| s.pid)
                {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }
            app.set_view_mode(ViewMode::List);
            let _ = app.refresh();
        }
        Action::CancelKill => {
            app.confirm_kill_session_id = None;
            app.set_view_mode(ViewMode::List);
        }
        Action::OpenSessionFiles => open_session_files(app),
        Action::SessionFileUp => {
            app.session_file_cursor = app.session_file_cursor.saturating_sub(1);
        }
        Action::SessionFileDown => {
            if app.session_file_cursor + 1 < app.session_files.len() {
                app.session_file_cursor += 1;
            }
        }
        Action::SessionFileSelect => select_session_file(app, terminal)?,
        Action::SessionFileClose => {
            app.session_files.clear();
            app.set_view_mode(ViewMode::Detail);
        }
        Action::UnfollowSession => {
            if let Some(id) = app.selected_session().map(|s| s.id.clone()) {
                app.unfollow_session(&id);
            }
        }
        Action::ResumeSessionPicker => {
            if let Some(cwd) = app.selected_session().map(|s| s.cwd.clone()) {
                let (rows, cols) = pty_dims(terminal)?;
                let _ = app
                    .terminal_manager_mut()
                    .attach_resume_picker(&cwd, rows, cols);
                app.set_view_mode(ViewMode::Terminal);
            }
        }
        Action::MoveUp => app.move_up(),
        Action::MoveDown => app.move_down(),
        Action::MoveToTop => app.move_to_top(),
        Action::MoveToBottom => app.move_to_bottom(),
        Action::Select => match app.view_mode() {
            ViewMode::List => attach_selected(app, terminal)?,
            ViewMode::Detail => app.detail_open_preview(),
            _ => {}
        },
        Action::AttachSession => attach_selected(app, terminal)?,
        Action::AttachByIndex(idx) => attach_by_index(app, idx, terminal)?,
        Action::ToggleQSwitcher => match app.view_mode() {
            ViewMode::QSwitcher => app.set_view_mode(ViewMode::List),
            _ => app.set_view_mode(ViewMode::QSwitcher),
        },
        Action::TerminalQSwitcher => app.set_view_mode(ViewMode::TerminalQSwitcher),
        Action::Detach => {
            if app.terminal_manager().active_is_exited() {
                app.terminal_manager_mut().remove_active();
            } else {
                app.terminal_manager_mut().detach();
            }
            app.set_view_mode(ViewMode::List);
        }
        Action::TerminalInput(bytes) => {
            let _ = app.terminal_manager_mut().write_to_active(&bytes);
        }
        Action::CycleNextSession => app.terminal_manager_mut().cycle_next(),
        Action::CyclePrevSession => app.terminal_manager_mut().cycle_prev(),
        Action::ScrollUp(n) => {
            if let Some(term) = app.terminal_manager().active_terminal() {
                term.scroll_up(n);
            }
        }
        Action::ScrollDown(n) => {
            if let Some(term) = app.terminal_manager().active_terminal() {
                term.scroll_down(n);
            }
        }
        Action::Back => handle_back(app),
        Action::ShowDetail => {
            if app.selected_session().is_some() {
                app.set_view_mode(ViewMode::Detail);
            }
        }
        Action::ShowHelp => {
            if *app.view_mode() == ViewMode::Help {
                app.set_view_mode(ViewMode::List);
            } else {
                app.set_view_mode(ViewMode::Help);
            }
        }
        Action::ToggleFilter => app.set_view_mode(ViewMode::Filter),
        Action::FilterInput(c) => app.filter_push(c),
        Action::FilterBackspace => app.filter_pop(),
        Action::FilterSubmit => app.set_view_mode(ViewMode::List),
        Action::CycleSort => app.cycle_sort(),
        Action::Refresh => {
            let _ = app.refresh();
        }
        Action::LaunchNew => app.set_view_mode(ViewMode::Command),
        Action::CommandInput(c) => app.command_push(c),
        Action::CommandBackspace => app.command_pop(),
        Action::CommandSubmit => submit_new_session(app, terminal)?,
        Action::CommandTab => app.command_tab_complete(),
        Action::CommandCancel => {
            app.command_take();
            app.set_view_mode(ViewMode::List);
        }
        Action::ToggleLog => {
            if *app.view_mode() == ViewMode::Log {
                app.set_view_mode(ViewMode::List);
            } else {
                app.log_scroll_to_bottom();
                app.set_view_mode(ViewMode::Log);
            }
        }
        Action::ClearLog => app.clear_log(),
        Action::ToggleSideTerminal => {
            if app.is_side_panel_open() {
                app.close_side_panel();
            } else {
                let area = terminal.size()?;
                let panel_cols = area.width * 40 / 100;
                let panel_rows = area.height.saturating_sub(1);
                app.open_side_panel(panel_rows, panel_cols);
            }
        }
        Action::SideTerminalInput(bytes) => {
            if let Some(st) = app.side_terminal_mut() {
                let _ = st.write_input(&bytes);
            }
        }
        Action::None => {}
    }
    Ok(())
}

fn handle_back(app: &mut App) {
    match app.view_mode() {
        ViewMode::Detail => {
            if app.detail_preview().is_some() {
                app.detail_close_preview();
            } else {
                app.set_view_mode(ViewMode::List);
            }
        }
        ViewMode::Log | ViewMode::Help | ViewMode::QSwitcher | ViewMode::Filter => {
            app.set_view_mode(ViewMode::List)
        }
        ViewMode::TerminalQSwitcher => app.set_view_mode(ViewMode::Terminal),
        ViewMode::List if app.has_active_filter() => app.clear_filter(),
        _ => {}
    }
}

fn open_session_files(app: &mut App) {
    if *app.view_mode() != ViewMode::Detail {
        return;
    }
    let Some((cwd, id)) = app
        .selected_session()
        .map(|s| (s.cwd.clone(), s.id.clone()))
    else {
        return;
    };
    let files = session::list_session_files(&cwd, &id);
    if files.is_empty() {
        return;
    }
    let current_idx = files.iter().position(|f| f.is_current).unwrap_or(0);
    app.session_files = files;
    app.session_file_cursor = current_idx;
    app.set_view_mode(ViewMode::SessionFilePicker);
}

fn select_session_file(app: &mut App, terminal: &Term) -> Result<()> {
    let picked = app
        .session_files
        .get(app.session_file_cursor)
        .map(|f| f.session_id.clone());
    let selected = app.selected_session().map(|s| {
        (
            s.id.clone(),
            s.project_name.clone(),
            s.cwd.clone(),
            s.takeover_pid(),
        )
    });
    if let (Some(session_id), Some((selected_id, name, cwd, pid))) = (picked, selected) {
        let takeover = pid.filter(|_| selected_id == session_id);
        let (rows, cols) = pty_dims(terminal)?;
        app.terminal_manager_mut()
            .attach(&session_id, &name, &cwd, takeover, rows, cols)?;
        app.set_view_mode(ViewMode::Terminal);
    }
    app.session_files.clear();
    Ok(())
}

fn submit_new_session(app: &mut App, terminal: &Term) -> Result<()> {
    let input = app.command_take();
    let path = input.trim().to_string();
    if path.is_empty() {
        app.set_view_mode(ViewMode::List);
        return Ok(());
    }
    let expanded = match (path.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.to_string_lossy().to_string() + rest,
        _ => path,
    };
    let cwd = std::fs::canonicalize(&expanded).unwrap_or_else(|_| PathBuf::from(&expanded));
    if cwd.is_dir() {
        let (rows, cols) = pty_dims(terminal)?;
        let _ = app.terminal_manager_mut().attach_new(&cwd, rows, cols);
        app.set_view_mode(ViewMode::Terminal);
    } else {
        tlog!(warn, "new session: not a directory: {}", cwd.display());
        app.set_view_mode(ViewMode::List);
    }
    Ok(())
}

fn attach_selected(app: &mut App, terminal: &Term) -> Result<()> {
    let target = app.selected_session().map(|s| {
        (
            s.id.clone(),
            s.project_name.clone(),
            s.cwd.clone(),
            s.takeover_pid(),
        )
    });
    attach_target(app, target, terminal)
}

fn attach_by_index(app: &mut App, idx: usize, terminal: &Term) -> Result<()> {
    let target = app.filtered_sessions().get(idx).map(|s| {
        (
            s.id.clone(),
            s.project_name.clone(),
            s.cwd.clone(),
            s.takeover_pid(),
        )
    });
    attach_target(app, target, terminal)
}

fn attach_target(
    app: &mut App,
    target: Option<(String, String, PathBuf, Option<u32>)>,
    terminal: &Term,
) -> Result<()> {
    let Some((id, name, cwd, pid)) = target else {
        return Ok(());
    };
    let (rows, cols) = pty_dims(terminal)?;
    app.terminal_manager_mut()
        .attach(&id, &name, &cwd, pid, rows, cols)?;
    app.set_view_mode(ViewMode::Terminal);
    Ok(())
}
