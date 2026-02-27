mod app;
mod input;
mod session;
mod store;
mod terminal;
mod ui;

use anyhow::Result;
use app::{App, ViewMode};
use crossterm::event;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use input::{handle_event, Action};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use session::SessionManager;
use std::io::{stdout, IsTerminal};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    stdout().execute(DisableMouseCapture)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let refresh_interval = Duration::from_secs(5);
    let mut last_refresh = Instant::now();
    let mut needs_draw = true;
    let mut mouse_captured = true;

    loop {
        if matches!(app.view_mode(), ViewMode::Terminal | ViewMode::TerminalHarpoon) {
            if let Some(term) = app.terminal_manager().active_terminal() {
                if term.take_dirty() {
                    needs_draw = true;
                }
            }
        }

        if needs_draw {
            terminal.draw(|f| {
                let area = f.area();
                match app.view_mode() {
                    ViewMode::List | ViewMode::Filter => {
                        ui::render_session_list(f, app, area);
                    }
                    ViewMode::Detail => {
                        if let Some(session) = app.selected_session() {
                            let session = session.clone();
                            let items = app.detail_items().to_vec();
                            let cursor = app.detail_cursor();
                            let preview = app.detail_preview().cloned();
                            let preview_scroll = app.detail_preview_scroll();
                            ui::render_session_detail(
                                f, &session, &items, cursor,
                                preview.as_ref(), preview_scroll, area,
                            );
                        }
                    }
                    ViewMode::Harpoon => {
                        ui::render_session_list(f, app, area);
                        ui::render_harpoon(f, app, area);
                    }
                    ViewMode::Help => {
                        ui::render_session_list(f, app, area);
                        ui::render_help(f, area);
                    }
                    ViewMode::Terminal => {
                        render_terminal_view(app, f, area);
                    }
                    ViewMode::TerminalHarpoon => {
                        render_terminal_view(app, f, area);
                        ui::render_harpoon(f, app, area);
                    }
                    ViewMode::Command => {
                        ui::render_session_list(f, app, area);
                        ui::render_command_input(f, app.command_input(), area);
                    }
                    ViewMode::ConfirmQuit => {
                        ui::render_session_list(f, app, area);
                        let active = app.active_attached_sessions();
                        ui::render_confirm_quit(f, &active, area);
                    }
                }
            })?;
            needs_draw = false;
        }

        if event::poll(Duration::from_millis(16))? {
            loop {
                let ev = event::read()?;

                if let event::Event::Resize(cols, rows) = ev {
                    if matches!(app.view_mode(), ViewMode::Terminal | ViewMode::TerminalHarpoon) {
                        let _ = app.terminal_manager().resize_active(rows.saturating_sub(2), cols);
                    }
                    needs_draw = true;
                }

                let action = handle_event(&ev, app.view_mode());
                let is_noop = matches!(action, Action::None | Action::TerminalInput(_));
                process_action(app, action, terminal)?;
                if !is_noop {
                    needs_draw = true;
                }

                if app.should_quit() {
                    break;
                }
                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
            }
        }

        let in_terminal = matches!(app.view_mode(), ViewMode::Terminal | ViewMode::TerminalHarpoon);
        if in_terminal && mouse_captured {
            stdout().execute(DisableMouseCapture)?;
            mouse_captured = false;
        } else if !in_terminal && !mouse_captured {
            stdout().execute(EnableMouseCapture)?;
            mouse_captured = true;
        }

        app.terminal_manager_mut().check_and_forward_notifications(in_terminal);

        if matches!(app.view_mode(), ViewMode::Terminal | ViewMode::TerminalHarpoon) {
            app.terminal_manager_mut().cleanup_inactive_exited();
        }

        if !matches!(app.view_mode(), ViewMode::Terminal | ViewMode::TerminalHarpoon) {
            if last_refresh.elapsed() >= refresh_interval {
                app.refresh()?;
                last_refresh = Instant::now();
                needs_draw = true;
            }
        }

        if app.should_quit() {
            break;
        }
    }

    Ok(())
}

fn render_terminal_view(
    app: &App,
    f: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
) {
    if let Some(term) = app.terminal_manager().active_terminal() {
        let guard = term.lock_parser();
        let screen = guard.screen();
        let scrolled = screen.scrollback() > 0;
        let exited = term.is_exited();
        let tabs = app.terminal_manager().tab_info();
        ui::render_terminal(f, screen, &tabs, exited, scrolled, area);
    }
}

fn process_action(
    app: &mut App,
    action: Action,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    match action {
        Action::Quit => {
            let active = app.active_attached_sessions();
            if active.is_empty() {
                app.quit();
            } else {
                app.set_view_mode(ViewMode::ConfirmQuit);
            }
        }
        Action::ConfirmQuit => app.quit(),
        Action::CancelQuit => app.set_view_mode(ViewMode::List),
        Action::MoveUp => app.move_up(),
        Action::MoveDown => app.move_down(),
        Action::MoveToTop => app.move_to_top(),
        Action::MoveToBottom => app.move_to_bottom(),
        Action::Select => match app.view_mode() {
            ViewMode::List => {
                attach_selected(app, terminal)?;
            }
            ViewMode::Detail => {
                app.detail_open_preview();
            }
            _ => {}
        },
        Action::AttachSession => {
            attach_selected(app, terminal)?;
        }
        Action::AttachByIndex(idx) => {
            attach_by_index(app, idx, terminal)?;
            if matches!(
                app.view_mode(),
                ViewMode::Harpoon | ViewMode::TerminalHarpoon
            ) {
                app.set_view_mode(ViewMode::Terminal);
            }
        }
        Action::ToggleHarpoon => {
            if *app.view_mode() == ViewMode::Harpoon {
                app.set_view_mode(ViewMode::List);
            } else {
                app.set_view_mode(ViewMode::Harpoon);
            }
        }
        Action::TerminalHarpoon => {
            app.set_view_mode(ViewMode::TerminalHarpoon);
        }
        Action::Detach => {
            if app.terminal_manager().active_is_exited() {
                app.terminal_manager_mut().remove_active();
            } else {
                app.terminal_manager_mut().detach();
            }
            app.set_view_mode(ViewMode::List);
            let _ = app.refresh();
        }
        Action::TerminalInput(bytes) => {
            let _ = app.terminal_manager_mut().write_to_active(&bytes);
        }
        Action::CycleNextSession => {
            app.terminal_manager_mut().cycle_next();
        }
        Action::CyclePrevSession => {
            app.terminal_manager_mut().cycle_prev();
        }
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
        Action::Back => match app.view_mode() {
            ViewMode::Detail => {
                if app.detail_preview().is_some() {
                    app.detail_close_preview();
                } else {
                    app.set_view_mode(ViewMode::List);
                }
            }
            ViewMode::Help | ViewMode::Harpoon => app.set_view_mode(ViewMode::List),
            ViewMode::TerminalHarpoon => app.set_view_mode(ViewMode::Terminal),
            ViewMode::Filter => {
                app.set_view_mode(ViewMode::List);
            }
            ViewMode::List => {
                if app.has_active_filter() {
                    app.clear_filter();
                }
            }
            _ => {}
        },
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
        Action::LaunchNew => {
            app.set_view_mode(ViewMode::Command);
        }
        Action::CommandInput(c) => app.command_push(c),
        Action::CommandBackspace => app.command_pop(),
        Action::CommandSubmit => {
            let input = app.command_take();
            let path = input.trim().to_string();
            if !path.is_empty() {
                let area = terminal.size()?;
                let rows = area.height.saturating_sub(1);
                let cols = area.width;
                let cwd = PathBuf::from(&path);
                if cwd.is_dir() {
                    let _ = app.terminal_manager_mut().attach_new(&cwd, rows, cols);
                    app.set_view_mode(ViewMode::Terminal);
                } else {
                    app.set_view_mode(ViewMode::List);
                }
            } else {
                app.set_view_mode(ViewMode::List);
            }
        }
        Action::CommandCancel => {
            app.command_take();
            app.set_view_mode(ViewMode::List);
        }
        Action::None => {}
    }
    Ok(())
}

fn attach_selected(
    app: &mut App,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    if let Some(session) = app.selected_session() {
        let id = session.id.clone();
        let name = session.project_name.clone();
        let cwd = session.cwd.clone();
        let pid = session.pid;
        let area = terminal.size()?;
        let rows = area.height.saturating_sub(1);
        let cols = area.width;
        app.terminal_manager_mut()
            .attach(&id, &name, &cwd, pid, rows, cols)?;
        app.set_view_mode(ViewMode::Terminal);
    }
    Ok(())
}

fn attach_by_index(
    app: &mut App,
    idx: usize,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    let sessions: Vec<_> = app
        .filtered_sessions()
        .iter()
        .map(|s| (s.id.clone(), s.project_name.clone(), s.cwd.clone(), s.pid))
        .collect();

    if let Some((id, name, cwd, pid)) = sessions.get(idx) {
        let area = terminal.size()?;
        let rows = area.height.saturating_sub(1);
        let cols = area.width;
        app.terminal_manager_mut()
            .attach(id, name, cwd, *pid, rows, cols)?;
        app.set_view_mode(ViewMode::Terminal);
    }
    Ok(())
}
