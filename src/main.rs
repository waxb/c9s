mod app;
mod input;
mod log;
mod session;
mod store;
mod terminal;
mod tervezo;
mod ui;
mod usage;
mod worktree;
#[cfg(test)]
mod worktree_integration_tests;

use anyhow::Result;
use app::{
    App, SessionEntry, TervezoAction, TervezoCreateMsg, TervezoDetailMsg, TervezoTab, ViewMode,
    WorkspaceMsg,
};
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
use tervezo::{CreateImplementationRequest, TervezoClient};

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

    if let Some(store) = app.store_ref() {
        let _ = store.backfill_repo_roots();
    }

    // Install panic hook that logs to c9s.log before printing to stderr
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
        tlog!(error, "DIAG: run_loop returned error: {}", e);
    }

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
    let detail_refresh_interval = Duration::from_secs(10);
    let mut last_refresh = Instant::now();
    let mut last_detail_refresh = Instant::now();
    let mut needs_draw = true;
    let mut mouse_captured = true;

    loop {
        if matches!(
            app.view_mode(),
            ViewMode::Terminal | ViewMode::TerminalQSwitcher
        ) {
            if let Some(term) = app.terminal_manager().active_terminal() {
                if term.take_dirty() {
                    needs_draw = true;
                }
            }
        }

        if app.is_side_panel_open() {
            if let Some(st) = app.side_terminal() {
                if st.take_dirty() {
                    needs_draw = true;
                }
            }
        }

        if app.check_tervezo_dirty() {
            needs_draw = true;
        }

        if app.drain_tervezo_detail_messages() {
            needs_draw = true;

            // Handle navigate_to_impl from restart action
            let nav_target = app.tervezo_detail.as_ref().and_then(|s| {
                s.action_result.as_ref().and_then(|r| {
                    r.as_ref()
                        .ok()
                        .and_then(|msg| msg.strip_prefix("NAVIGATE:").map(|id| id.to_string()))
                })
            });
            if let Some(new_id) = nav_target {
                // Fetch the new implementation and re-initialize the detail view
                if let Some(config) = app.tervezo_config() {
                    let client = TervezoClient::new(config);
                    if let Ok(new_impl) = client.get_implementation(&new_id) {
                        if let Some(ref mut state) = app.tervezo_detail {
                            state.action_result = Some(Ok(format!("Restarted → {}", new_id)));
                        }
                        // Re-initialize state with the new implementation
                        let mut new_state = app::TervezoDetailState::new(new_impl);
                        new_state.action_result =
                            Some(Ok("Restarted (new implementation)".to_string()));
                        app.tervezo_detail = Some(new_state);
                        trigger_tervezo_initial_fetch(app);
                    }
                }
            }
        }

        if app.drain_sse_messages() {
            needs_draw = true;
        }

        if let Some(msg) = app.drain_workspace_messages() {
            match msg {
                WorkspaceMsg::Loaded(workspaces) => {
                    if let Some(ref mut state) = app.tervezo_create {
                        state.workspaces_loading = false;
                        state.workspaces = workspaces;
                        state.workspaces_error = None;
                    }
                }
                WorkspaceMsg::Error(e) => {
                    if let Some(ref mut state) = app.tervezo_create {
                        state.workspaces_loading = false;
                        state.workspaces_error = Some(e);
                    }
                }
            }
            needs_draw = true;
        }

        if let Some(msg) = app.drain_tervezo_create_messages() {
            match msg {
                TervezoCreateMsg::Success(_impl) => {
                    app.set_view_mode(ViewMode::List);
                    if let Some(fetcher) = app.tervezo_fetcher_ref() {
                        fetcher.mark_dirty();
                    }
                    let _ = app.refresh();
                }
                TervezoCreateMsg::Error(e) => {
                    if let Some(ref mut state) = app.tervezo_create {
                        state.submitting = false;
                        state.error = Some(e);
                    }
                }
            }
            needs_draw = true;
        }

        app.drain_ci_statuses();
        app.check_ci_statuses();

        if *app.view_mode() == ViewMode::Log && log::take_dirty() {
            needs_draw = true;
        }

        if needs_draw {
            terminal.draw(|f| {
                let full_area = f.area();
                let (main_area, side_area) = if app.is_side_panel_open() {
                    ui::split_with_side_panel(full_area)
                } else {
                    (full_area, ratatui::layout::Rect::default())
                };
                let area = main_area;

                match app.view_mode() {
                    ViewMode::List | ViewMode::Filter => {
                        ui::render_session_list(f, app, area);
                    }
                    ViewMode::Detail => {
                        if let Some(entry) = app.selected_session() {
                            if let Some(session) = entry.as_local() {
                                let session = session.clone();
                                let items = app.detail_items().to_vec();
                                let cursor = app.detail_cursor();
                                let preview = app.detail_preview().cloned();
                                let preview_scroll = app.detail_preview_scroll();
                                ui::render_session_detail(
                                    f,
                                    &session,
                                    &items,
                                    cursor,
                                    preview.as_ref(),
                                    preview_scroll,
                                    area,
                                );
                            }
                        }
                    }
                    ViewMode::SessionFilePicker => {
                        if let Some(entry) = app.selected_session() {
                            if let Some(session) = entry.as_local() {
                                let session = session.clone();
                                let items = app.detail_items().to_vec();
                                let cursor = app.detail_cursor();
                                let preview = app.detail_preview().cloned();
                                let preview_scroll = app.detail_preview_scroll();
                                ui::render_session_detail(
                                    f,
                                    &session,
                                    &items,
                                    cursor,
                                    preview.as_ref(),
                                    preview_scroll,
                                    area,
                                );
                            }
                        }
                        ui::render_session_file_picker(
                            f,
                            &app.session_files,
                            app.session_file_cursor,
                            area,
                        );
                    }
                    ViewMode::TervezoDetail => {
                        if let Some(ref state) = app.tervezo_detail {
                            ui::render_tervezo_detail(f, state, area);
                        }
                    }
                    ViewMode::TervezoActionMenu => {
                        if let Some(ref state) = app.tervezo_detail {
                            ui::render_tervezo_detail(f, state, area);
                            ui::render_tervezo_action_menu(f, state, area);
                        }
                    }
                    ViewMode::TervezoConfirm => {
                        if let Some(ref state) = app.tervezo_detail {
                            ui::render_tervezo_detail(f, state, area);
                            ui::render_tervezo_confirm(f, state, area);
                        }
                    }
                    ViewMode::TervezoPromptInput => {
                        if let Some(ref state) = app.tervezo_detail {
                            ui::render_tervezo_detail_with_prompt(f, state, area);
                        }
                    }
                    ViewMode::TervezoQSwitcher => {
                        if let Some(ref state) = app.tervezo_detail {
                            ui::render_tervezo_detail(f, state, area);
                        }
                        ui::render_qswitcher(f, app, area);
                    }
                    ViewMode::TervezoCreateDialog => {
                        ui::render_session_list(f, app, area);
                        ui::render_tervezo_create_dialog(f, &app.tervezo_create, area);
                    }
                    ViewMode::NewSessionMenu => {
                        ui::render_session_list(f, app, area);
                        ui::render_new_session_menu(f, &app.new_session_menu, area);
                    }
                    ViewMode::QSwitcher => {
                        ui::render_session_list(f, app, area);
                        ui::render_qswitcher(f, app, area);
                    }
                    ViewMode::Help => {
                        ui::render_session_list(f, app, area);
                        ui::render_help(f, area);
                    }
                    ViewMode::Terminal => {
                        render_terminal_view(app, f, area);
                    }
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
                            .and_then(|id| {
                                app.filtered_sessions()
                                    .iter()
                                    .find(|e| e.id() == id)
                                    .map(|e| e.display_name().to_string())
                            })
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
                    ViewMode::BranchInput => {
                        ui::render_session_list(f, app, area);
                        if let Some(ref state) = app.branch_input {
                            ui::render_branch_input(f, state, area);
                        }
                    }
                    ViewMode::WorktreePicker => {
                        ui::render_session_list(f, app, area);
                        if let Some(ref state) = app.worktree_picker {
                            ui::render_worktree_picker(f, state, area);
                        }
                    }
                    ViewMode::ConfirmWorktreeCleanup => {
                        ui::render_session_list(f, app, area);
                        if let Some(ref state) = app.confirm_worktree_cleanup {
                            ui::render_confirm_worktree_cleanup(f, state, area);
                        }
                    }
                    ViewMode::ConfirmRecreateWorktree => {
                        ui::render_session_list(f, app, area);
                        if let Some(ref state) = app.confirm_recreate_worktree {
                            ui::render_confirm_recreate_worktree(f, state, area);
                        }
                    }
                }

                if app.is_side_panel_open() {
                    if let Some(st) = app.side_terminal() {
                        let focused = app.is_side_panel_focused();
                        ui::render_side_panel(f, st, focused, side_area);
                    }
                }
            })?;
            needs_draw = false;
        }

        if event::poll(Duration::from_millis(16))? {
            loop {
                let ev = event::read()?;

                if let event::Event::Resize(cols, rows) = ev {
                    if matches!(
                        app.view_mode(),
                        ViewMode::Terminal | ViewMode::TerminalQSwitcher
                    ) {
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
                        "DIAG: process_action error: {} (view={:?})",
                        e,
                        app.view_mode()
                    );
                    return Err(e);
                }
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

        let needs_native_mouse = matches!(
            app.view_mode(),
            ViewMode::Terminal | ViewMode::TerminalQSwitcher | ViewMode::Log
        );
        if needs_native_mouse && mouse_captured {
            stdout().execute(DisableMouseCapture)?;
            mouse_captured = false;
        } else if !needs_native_mouse && !mouse_captured {
            stdout().execute(EnableMouseCapture)?;
            mouse_captured = true;
        }

        let viewing_active = matches!(
            app.view_mode(),
            ViewMode::Terminal | ViewMode::TerminalQSwitcher
        );
        let notified = app
            .terminal_manager_mut()
            .check_and_forward_notifications(viewing_active);

        if notified {
            app.invalidate_usage();
        }

        app.terminal_manager_mut().cleanup_inactive_exited();

        if last_refresh.elapsed() >= refresh_interval {
            if matches!(
                app.view_mode(),
                ViewMode::Terminal | ViewMode::TerminalQSwitcher
            ) {
                app.refresh_usage();
            } else {
                app.refresh()?;
            }
            last_refresh = Instant::now();
            needs_draw = true;
        }

        // Periodic re-fetch of detail panel data for running implementations
        let in_detail_view = matches!(
            app.view_mode(),
            ViewMode::TervezoDetail
                | ViewMode::TervezoActionMenu
                | ViewMode::TervezoConfirm
                | ViewMode::TervezoPromptInput
        );
        let is_running = app
            .tervezo_detail
            .as_ref()
            .map(|s| s.implementation.status.is_running())
            .unwrap_or(false);
        if in_detail_view && is_running && last_detail_refresh.elapsed() >= detail_refresh_interval
        {
            trigger_tervezo_panel_refresh(app);
            last_detail_refresh = Instant::now();
        }

        if app.should_quit() {
            tlog!(
                info,
                "DIAG: run_loop exiting (should_quit=true), view_mode={:?}",
                app.view_mode()
            );
            break;
        }
    }

    Ok(())
}

fn render_terminal_view(app: &App, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    if let Some(term) = app.terminal_manager().active_terminal() {
        let guard = term.lock_parser();
        let screen = guard.screen();
        let scrolled = screen.scrollback() > 0;
        let exited = term.is_exited();
        let tabs = app.terminal_manager().tab_info();
        ui::render_terminal(f, screen, &tabs, exited, scrolled, app.usage(), area);
    }
}

fn process_action(
    app: &mut App,
    action: Action,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    // Clear flash message on any keypress in tervezo detail view
    if !matches!(action, Action::None) {
        let in_tzv = matches!(
            app.view_mode(),
            ViewMode::TervezoDetail
                | ViewMode::TervezoActionMenu
                | ViewMode::TervezoConfirm
                | ViewMode::TervezoPromptInput
                | ViewMode::TervezoQSwitcher
        );
        if in_tzv {
            if let Some(ref mut state) = app.tervezo_detail {
                if state.action_result.is_some() {
                    state.action_result = None;
                }
            }
        }
    }

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
        Action::KillSession => {
            if let Some(entry) = app.selected_session() {
                if let Some(session) = entry.as_local() {
                    if session.pid.is_some() {
                        app.confirm_kill_session_id = Some(session.id.clone());
                        app.set_view_mode(ViewMode::ConfirmKill);
                    }
                }
            }
        }
        Action::ConfirmKill => {
            if let Some(session_id) = app.confirm_kill_session_id.take() {
                let session = app
                    .all_sessions()
                    .iter()
                    .find(|s| s.id == session_id)
                    .cloned();

                if let Some(ref s) = session {
                    if let Some(pid) = s.pid {
                        unsafe {
                            libc::kill(pid as i32, libc::SIGTERM);
                        }
                    }
                }

                if let Some(ref s) = session {
                    if let Some(ref wt_info) = s.worktree_info {
                        let is_dirty =
                            worktree::is_dirty(&wt_info.worktree_path).unwrap_or(true);
                        app.confirm_worktree_cleanup =
                            Some(app::ConfirmWorktreeCleanupState {
                                session_id: session_id.clone(),
                                worktree_path: wt_info.worktree_path.clone(),
                                branch: wt_info.pinned_branch.clone(),
                                is_dirty,
                            });
                        app.set_view_mode(ViewMode::ConfirmWorktreeCleanup);
                    } else {
                        app.set_view_mode(ViewMode::List);
                        let _ = app.refresh();
                    }
                } else {
                    app.set_view_mode(ViewMode::List);
                    let _ = app.refresh();
                }
            } else {
                app.set_view_mode(ViewMode::List);
                let _ = app.refresh();
            }
        }
        Action::CancelKill => {
            app.confirm_kill_session_id = None;
            app.set_view_mode(ViewMode::List);
        }
        Action::OpenSessionFiles => {
            if *app.view_mode() == ViewMode::Detail {
                if let Some(entry) = app.selected_session() {
                    if let Some(session) = entry.as_local() {
                        let files =
                            session::list_session_files(&session.cwd, &session.id);
                        if !files.is_empty() {
                            let current_idx = files.iter().position(|f| f.is_current).unwrap_or(0);
                            app.session_files = files;
                            app.session_file_cursor = current_idx;
                            app.set_view_mode(ViewMode::SessionFilePicker);
                        }
                    }
                }
            }
        }
        Action::SessionFileUp => {
            if app.session_file_cursor > 0 {
                app.session_file_cursor -= 1;
            }
        }
        Action::SessionFileDown => {
            if app.session_file_cursor + 1 < app.session_files.len() {
                app.session_file_cursor += 1;
            }
        }
        Action::SessionFileSelect => {
            if let Some(file) = app.session_files.get(app.session_file_cursor) {
                let session_id = file.session_id.clone();
                if let Some(entry) = app.selected_session() {
                    if let Some(session) = entry.as_local() {
                        let cwd = session.cwd.clone();
                        let pid = session.pid;
                        let area = terminal.size()?;
                        let rows = area.height.saturating_sub(1);
                        let cols = area.width;
                        let name = session.project_name.clone();
                        app.terminal_manager_mut().attach(
                            &session_id,
                            &name,
                            &cwd,
                            pid,
                            rows,
                            cols,
                        )?;
                        app.set_view_mode(ViewMode::Terminal);
                    }
                }
            }
            app.session_files.clear();
        }
        Action::SessionFileClose => {
            app.session_files.clear();
            app.set_view_mode(ViewMode::Detail);
        }
        Action::UnfollowSession => {
            if let Some(entry) = app.selected_session() {
                let id = entry.id().to_string();
                app.unfollow_session(&id);
                app.merge_and_refilter();
            }
        }
        Action::ResumeSessionPicker => {
            if let Some(entry) = app.selected_session() {
                if let Some(session) = entry.as_local() {
                    let cwd = session.cwd.clone();
                    let area = terminal.size()?;
                    let rows = area.height.saturating_sub(1);
                    let cols = area.width;
                    let _ = app
                        .terminal_manager_mut()
                        .attach_resume_picker(&cwd, rows, cols);
                    app.set_view_mode(ViewMode::Terminal);
                }
            }
        }
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
                ViewMode::QSwitcher | ViewMode::TerminalQSwitcher | ViewMode::TervezoQSwitcher
            ) {
                // attach_by_index already sets TervezoDetail for remote sessions;
                // only force Terminal for local session attachments
                if !matches!(app.view_mode(), ViewMode::TervezoDetail) {
                    app.set_view_mode(ViewMode::Terminal);
                }
            }
        }
        Action::ToggleQSwitcher => match app.view_mode() {
            ViewMode::QSwitcher => app.set_view_mode(ViewMode::List),
            ViewMode::TervezoQSwitcher => app.set_view_mode(ViewMode::TervezoDetail),
            ViewMode::TervezoDetail => app.set_view_mode(ViewMode::TervezoQSwitcher),
            _ => app.set_view_mode(ViewMode::QSwitcher),
        },
        Action::TerminalQSwitcher => {
            app.set_view_mode(ViewMode::TerminalQSwitcher);
        }
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
            ViewMode::TervezoDetail => {
                tlog!(info, "DIAG: Back action from TervezoDetail → List");
                app.set_view_mode(ViewMode::List);
            }
            ViewMode::TervezoQSwitcher => app.set_view_mode(ViewMode::TervezoDetail),
            ViewMode::Log | ViewMode::Help | ViewMode::QSwitcher => {
                app.set_view_mode(ViewMode::List)
            }
            ViewMode::TerminalQSwitcher => app.set_view_mode(ViewMode::Terminal),
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
            if let Some(entry) = app.selected_session() {
                match entry {
                    SessionEntry::Local(_) => {
                        app.set_view_mode(ViewMode::Detail);
                    }
                    SessionEntry::Remote(_) => {
                        app.set_view_mode(ViewMode::TervezoDetail);
                        trigger_tervezo_initial_fetch(app);
                    }
                }
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
            let in_git_repo = app
                .selected_session()
                .and_then(|e| e.as_local())
                .map(|s| s.cwd.clone())
                .or_else(|| std::env::current_dir().ok())
                .map(|cwd| worktree::resolve_repo_root(&cwd).is_ok())
                .unwrap_or(false);

            if app.has_tervezo() || in_git_repo {
                app.open_new_session_menu(in_git_repo);
                app.set_view_mode(ViewMode::NewSessionMenu);
            } else {
                app.set_view_mode(ViewMode::Command);
            }
        }
        Action::CommandInput(c) => app.command_push(c),
        Action::CommandBackspace => app.command_pop(),
        Action::CommandSubmit => {
            let input = app.command_take();
            let path = input.trim().to_string();
            if !path.is_empty() {
                let expanded = if let Some(rest) = path.strip_prefix('~') {
                    dirs::home_dir()
                        .map(|h| h.to_string_lossy().to_string() + rest)
                        .unwrap_or(path)
                } else {
                    path
                };
                let cwd =
                    std::fs::canonicalize(&expanded).unwrap_or_else(|_| PathBuf::from(&expanded));
                if cwd.is_dir() {
                    let area = terminal.size()?;
                    let rows = area.height.saturating_sub(1);
                    let cols = area.width;
                    let _ = app.terminal_manager_mut().attach_new(&cwd, rows, cols);
                    app.set_view_mode(ViewMode::Terminal);
                } else {
                    app.set_view_mode(ViewMode::List);
                }
            } else {
                app.set_view_mode(ViewMode::List);
            }
        }
        Action::CommandTab => {
            app.command_tab_complete();
        }
        Action::CommandCancel => {
            app.command_take();
            app.set_view_mode(ViewMode::List);
        }
        Action::TervezoTabNext => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.active_tab = state.active_tab.next();
                trigger_tervezo_tab_fetch(app);
            }
        }
        Action::TervezoTabPrev => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.active_tab = state.active_tab.prev();
                trigger_tervezo_tab_fetch(app);
            }
        }
        Action::TervezoScrollUp => {
            if let Some(ref mut state) = app.tervezo_detail {
                if state.timeline_at_bottom {
                    // Sync scroll position from render before leaving autoscroll
                    state.timeline_scroll = state.timeline_rendered_scroll.get();
                }
                state.timeline_scroll = state.timeline_scroll.saturating_sub(1);
                state.timeline_at_bottom = false;
            }
        }
        Action::TervezoScrollDown => {
            if let Some(ref mut state) = app.tervezo_detail {
                if state.timeline_at_bottom {
                    // Already at the bottom — nothing to scroll down to
                } else {
                    state.timeline_scroll += 1;
                }
            }
        }
        Action::TervezoScrollHalfPageUp => {
            if let Some(ref mut state) = app.tervezo_detail {
                if state.timeline_at_bottom {
                    state.timeline_scroll = state.timeline_rendered_scroll.get();
                }
                let half = state.timeline_visible_height.get() / 2;
                state.timeline_scroll = state.timeline_scroll.saturating_sub(half.max(1));
                state.timeline_at_bottom = false;
            }
        }
        Action::TervezoScrollHalfPageDown => {
            if let Some(ref mut state) = app.tervezo_detail {
                if state.timeline_at_bottom {
                    // Already at the bottom — nothing to scroll down to
                } else {
                    let half = state.timeline_visible_height.get() / 2;
                    state.timeline_scroll += half.max(1);
                }
            }
        }
        Action::TervezoScrollToTop => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.timeline_scroll = 0;
                state.timeline_at_bottom = false;
            }
        }
        Action::TervezoScrollToBottom => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.timeline_at_bottom = true;
            }
        }
        Action::TervezoSsh => {
            if let Some(ref state) = app.tervezo_detail {
                if state.implementation.status.is_running() {
                    if let Some(creds) = state.ssh_creds.clone() {
                        let area = terminal.size()?;
                        let rows = area.height.saturating_sub(1);
                        let cols = area.width;
                        let id = state.implementation_id.clone();
                        let name = state.implementation.display_name().to_string();
                        let _ = app.terminal_manager_mut().attach_ssh(
                            &id,
                            &name,
                            &creds.ssh_command,
                            rows,
                            cols,
                        );
                        app.set_view_mode(ViewMode::Terminal);
                    }
                }
            }
        }
        Action::TervezoRefreshDetail => {
            trigger_tervezo_initial_fetch(app);
        }
        Action::TervezoToggleRaw => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.raw_markdown = !state.raw_markdown;
            }
        }
        Action::TervezoToggleExpand => {
            if let Some(ref mut state) = app.tervezo_detail {
                if state.active_tab == TervezoTab::Changes {
                    state.toggle_changes_expand();
                }
            }
        }
        Action::TervezoToggleSteps => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.steps_expanded = !state.steps_expanded;
            }
        }
        Action::TervezoOpenActionMenu => {
            // Clear any previous flash message
            if let Some(ref mut state) = app.tervezo_detail {
                state.action_result = None;
                let items = state.compute_available_actions();
                if !items.is_empty() {
                    state.action_menu_items = items;
                    state.action_menu_cursor = 0;
                    app.set_view_mode(ViewMode::TervezoActionMenu);
                }
            }
        }
        Action::TervezoActionMenuUp => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.action_menu_cursor = state.action_menu_cursor.saturating_sub(1);
            }
        }
        Action::TervezoActionMenuDown => {
            if let Some(ref mut state) = app.tervezo_detail {
                let max = state.action_menu_items.len().saturating_sub(1);
                if state.action_menu_cursor < max {
                    state.action_menu_cursor += 1;
                }
            }
        }
        Action::TervezoActionMenuSelect => {
            let selected_action = app
                .tervezo_detail
                .as_ref()
                .and_then(|s| s.action_menu_items.get(s.action_menu_cursor).copied());
            if let Some(action) = selected_action {
                if action == TervezoAction::SendPrompt {
                    // Open prompt input instead
                    app.set_view_mode(ViewMode::TervezoPromptInput);
                } else if action == TervezoAction::ViewPrInBrowser {
                    // Open PR URL in default browser (local operation, no API call)
                    if let Some(url) = app.tervezo_detail.as_ref().and_then(|s| {
                        s.pr_details
                            .as_ref()
                            .and_then(|pr| pr.url.clone())
                            .or_else(|| s.implementation.pr_url.clone())
                    }) {
                        let _ = open::that(&url);
                    }
                    app.set_view_mode(ViewMode::TervezoDetail);
                } else if action.is_destructive() {
                    if let Some(ref mut state) = app.tervezo_detail {
                        state.confirm_action = Some(action);
                    }
                    app.set_view_mode(ViewMode::TervezoConfirm);
                } else {
                    // Non-destructive: execute immediately
                    app.set_view_mode(ViewMode::TervezoDetail);
                    execute_tervezo_action(app, action);
                }
            }
        }
        Action::TervezoActionMenuClose => {
            app.set_view_mode(ViewMode::TervezoDetail);
        }
        Action::TervezoConfirmYes => {
            let action = app
                .tervezo_detail
                .as_mut()
                .and_then(|s| s.confirm_action.take());
            app.set_view_mode(ViewMode::TervezoDetail);
            if let Some(action) = action {
                execute_tervezo_action(app, action);
            }
        }
        Action::TervezoConfirmNo => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.confirm_action = None;
            }
            app.set_view_mode(ViewMode::TervezoDetail);
        }
        Action::TervezoOpenPrompt => {
            let can_prompt = app
                .tervezo_detail
                .as_ref()
                .map(|s| {
                    let waiting = s
                        .status_info
                        .as_ref()
                        .map(|si| si.waiting_for_input)
                        .unwrap_or(false);
                    waiting || s.implementation.status.is_terminal()
                })
                .unwrap_or(false);
            if can_prompt {
                if let Some(ref mut state) = app.tervezo_detail {
                    state.action_result = None;
                    state.prompt_input.clear();
                }
                app.set_view_mode(ViewMode::TervezoPromptInput);
            }
        }
        Action::TervezoPromptChar(c) => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.prompt_input.push(c);
            }
        }
        Action::TervezoPromptBackspace => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.prompt_input.pop();
            }
        }
        Action::TervezoPromptSubmit => {
            let prompt_data = app
                .tervezo_detail
                .as_ref()
                .map(|s| (s.implementation_id.clone(), s.prompt_input.clone()));
            if let Some((impl_id, message)) = prompt_data {
                if !message.trim().is_empty() {
                    if let Some(ref mut state) = app.tervezo_detail {
                        state.prompt_sending = true;
                    }
                    app.set_view_mode(ViewMode::TervezoDetail);
                    if let Some(config) = app.tervezo_config() {
                        let config = config.clone();
                        let tx = app.tervezo_detail_tx.clone();
                        std::thread::spawn(move || {
                            let client = TervezoClient::new(&config);
                            match client.send_prompt(&impl_id, &message) {
                                Ok(resp) => {
                                    let msg = if resp.sent {
                                        if let Some(ref fid) = resp.follow_up_id {
                                            format!("Prompt sent (follow-up: {})", fid)
                                        } else {
                                            "Prompt sent".to_string()
                                        }
                                    } else {
                                        "Prompt not sent".to_string()
                                    };
                                    if let Some(tx) = tx {
                                        let _ = tx.send(TervezoDetailMsg::PromptSent(msg));
                                    }
                                }
                                Err(e) => {
                                    if let Some(tx) = tx {
                                        let _ = tx.send(TervezoDetailMsg::PromptError(e));
                                    }
                                }
                            }
                        });
                    }
                }
            }
        }
        Action::TervezoPromptCancel => {
            if let Some(ref mut state) = app.tervezo_detail {
                state.prompt_input.clear();
            }
            app.set_view_mode(ViewMode::TervezoDetail);
        }
        Action::TervezoCreateClose => {
            app.set_view_mode(ViewMode::List);
        }
        Action::TervezoCreateFieldNext => {
            if let Some(ref mut state) = app.tervezo_create {
                state.active_field = state.active_field.next();
            }
        }
        Action::TervezoCreateFieldPrev => {
            if let Some(ref mut state) = app.tervezo_create {
                state.active_field = state.active_field.prev();
            }
        }
        Action::TervezoCreateToggleMode => {
            if let Some(ref mut state) = app.tervezo_create {
                match state.active_field {
                    app::TervezoCreateField::Mode => {
                        state.mode = state.mode.toggle();
                    }
                    app::TervezoCreateField::Workspace => {
                        if !state.workspaces.is_empty() {
                            state.selected_workspace =
                                (state.selected_workspace + 1) % state.workspaces.len();
                        }
                    }
                    app::TervezoCreateField::BaseBranch => {
                        submit_tervezo_create(app);
                    }
                    _ => {}
                }
            }
        }
        Action::TervezoCreateChar(c) => {
            if let Some(ref mut state) = app.tervezo_create {
                if state.submitting {
                } else {
                    match state.active_field {
                        app::TervezoCreateField::Prompt => state.prompt.push(c),
                        app::TervezoCreateField::RepoUrl => state.repo_url.push(c),
                        app::TervezoCreateField::BaseBranch => state.base_branch.push(c),
                        app::TervezoCreateField::Mode | app::TervezoCreateField::Workspace => {}
                    }
                    state.error = None;
                }
            }
        }
        Action::TervezoCreateBackspace => {
            if let Some(ref mut state) = app.tervezo_create {
                if !state.submitting {
                    match state.active_field {
                        app::TervezoCreateField::Prompt => {
                            state.prompt.pop();
                        }
                        app::TervezoCreateField::RepoUrl => {
                            state.repo_url.pop();
                        }
                        app::TervezoCreateField::BaseBranch => {
                            state.base_branch.pop();
                        }
                        app::TervezoCreateField::Mode | app::TervezoCreateField::Workspace => {}
                    }
                    state.error = None;
                }
            }
        }
        Action::TervezoCreateSubmit => {
            submit_tervezo_create(app);
        }
        Action::ToggleLog => {
            if *app.view_mode() == ViewMode::Log {
                app.set_view_mode(ViewMode::List);
            } else {
                app.log_scroll_to_bottom();
                app.set_view_mode(ViewMode::Log);
            }
        }
        Action::ClearLog => {
            app.clear_log();
        }
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
        Action::NewSessionMenuClose => {
            app.set_view_mode(ViewMode::List);
        }
        Action::NewSessionMenuUp => {
            if let Some(ref mut state) = app.new_session_menu {
                if state.cursor > 0 {
                    state.cursor -= 1;
                }
            }
        }
        Action::NewSessionMenuDown => {
            if let Some(ref mut state) = app.new_session_menu {
                if state.cursor + 1 < state.items.len() {
                    state.cursor += 1;
                }
            }
        }
        Action::NewSessionMenuSelect => {
            if let Some(ref state) = app.new_session_menu {
                match state.items[state.cursor] {
                    app::NewSessionOption::Local => {
                        app.set_view_mode(ViewMode::Command);
                    }
                    app::NewSessionOption::BranchSession => {
                        let cwd = app
                            .selected_session()
                            .and_then(|e| e.as_local())
                            .map(|s| s.cwd.clone())
                            .or_else(|| std::env::current_dir().ok())
                            .unwrap_or_default();
                        match worktree::resolve_repo_root(&cwd) {
                            Ok(repo_root) => {
                                let branches =
                                    worktree::list_local_branches(&repo_root).unwrap_or_default();
                                app.branch_input = Some(app::BranchInputState {
                                    input: String::new(),
                                    repo_root,
                                    branches,
                                    suggestion: None,
                                    error: None,
                                });
                                app.set_view_mode(ViewMode::BranchInput);
                            }
                            Err(_) => {
                                app.set_view_mode(ViewMode::List);
                            }
                        }
                    }
                    app::NewSessionOption::ExistingWorktree => {
                        let cwd = app
                            .selected_session()
                            .and_then(|e| e.as_local())
                            .map(|s| s.cwd.clone())
                            .or_else(|| std::env::current_dir().ok())
                            .unwrap_or_default();
                        match worktree::resolve_repo_root(&cwd) {
                            Ok(repo_root) => {
                                let worktrees =
                                    worktree::list_worktrees(&repo_root).unwrap_or_default();
                                let active_cwds: std::collections::HashSet<std::path::PathBuf> =
                                    app.all_sessions()
                                        .iter()
                                        .filter(|s| s.pid.is_some())
                                        .map(|s| s.cwd.clone())
                                        .collect();
                                let available: Vec<_> = worktrees
                                    .into_iter()
                                    .filter(|wt| !wt.is_main && !active_cwds.contains(&wt.path))
                                    .collect();
                                if available.is_empty() {
                                    app.set_view_mode(ViewMode::List);
                                } else {
                                    app.worktree_picker = Some(app::WorktreePickerState {
                                        worktrees: available,
                                        cursor: 0,
                                        repo_root,
                                    });
                                    app.set_view_mode(ViewMode::WorktreePicker);
                                }
                            }
                            Err(_) => app.set_view_mode(ViewMode::List),
                        }
                    }
                    app::NewSessionOption::Tervezo => {
                        app.set_view_mode(ViewMode::TervezoCreateDialog);
                    }
                }
            }
        }
        Action::FixCi => {
            if let Some(entry) = app.selected_session().cloned() {
                if let Some(imp) = entry.as_remote() {
                    let impl_id = imp.id.clone();
                    let ci_status = app.ci_statuses.get(&impl_id).cloned();
                    if matches!(ci_status, Some(app::CiStatus::Failing)) {
                        let branch = imp.branch.clone().unwrap_or_else(|| "unknown".to_string());
                        let prompt = format!(
                            "The CI pipeline is failing on branch `{}`. \
                            Investigate the GitHub Actions CI failure logs, identify the root cause, \
                            and implement proper fixes. If there are merge conflicts with the base branch, \
                            resolve them. Do not disable checks, skip tests, or apply superficial workarounds \
                            just to make CI green -- address the underlying issue. \
                            After pushing your fix, monitor whether CI passes. If it still fails, \
                            read the new logs and iterate until all checks pass.",
                            branch
                        );
                        if let Some(config) = app.tervezo_config() {
                            let config = config.clone();
                            let status_id = impl_id.clone();
                            app.ci_statuses.insert(status_id, app::CiStatus::Fixing);
                            std::thread::spawn(move || {
                                let client = TervezoClient::new(&config);
                                match client.send_prompt(&impl_id, &prompt) {
                                    Ok(resp) => {
                                        if resp.sent {
                                            tlog!(
                                                info,
                                                "CI fix prompt sent for {} (branch: {})",
                                                impl_id,
                                                branch
                                            );
                                        } else {
                                            tlog!(
                                                warn,
                                                "CI fix prompt was not accepted for {}",
                                                impl_id
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tlog!(error, "Failed to send CI fix prompt: {}", e);
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
        Action::None => {}
        Action::BranchInputChar(c) => {
            if let Some(ref mut state) = app.branch_input {
                state.input.push(c);
                state.error = None;
                state.suggestion = state
                    .branches
                    .iter()
                    .find(|b| b.starts_with(&state.input) && *b != &state.input)
                    .cloned();
            }
        }
        Action::BranchInputBackspace => {
            if let Some(ref mut state) = app.branch_input {
                state.input.pop();
                state.error = None;
                state.suggestion = if state.input.is_empty() {
                    None
                } else {
                    state
                        .branches
                        .iter()
                        .find(|b| b.starts_with(&state.input) && *b != &state.input)
                        .cloned()
                };
            }
        }
        Action::BranchInputTab => {
            if let Some(ref mut state) = app.branch_input {
                if let Some(suggestion) = state.suggestion.take() {
                    state.input = suggestion;
                }
            }
        }
        Action::BranchInputSubmit => {
            if let Some(mut state) = app.branch_input.take() {
                let branch = state.input.trim().to_string();
                if branch.is_empty() {
                    app.set_view_mode(ViewMode::List);
                } else {
                    let new_branch = !state.branches.contains(&branch);
                    match worktree::create_worktree(&state.repo_root, &branch, new_branch) {
                        Ok(wt_path) => {
                            let repo_name = state.repo_root.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let tab_label = format!("{}:{}", repo_name, branch);
                            let area = terminal.size()?;
                            let rows = area.height.saturating_sub(1);
                            let cols = area.width;
                            let _ =
                                app.terminal_manager_mut()
                                    .attach_new_named(&wt_path, Some(&tab_label), rows, cols);
                            app.set_view_mode(ViewMode::Terminal);
                        }
                        Err(e) => {
                            state.error = Some(format!("{}", e));
                            state.input = branch;
                            app.branch_input = Some(state);
                        }
                    }
                }
            }
        }
        Action::BranchInputCancel => {
            app.branch_input = None;
            app.set_view_mode(ViewMode::List);
        }
        Action::WorktreePickerUp => {
            if let Some(ref mut state) = app.worktree_picker {
                if state.cursor > 0 {
                    state.cursor -= 1;
                }
            }
        }
        Action::WorktreePickerDown => {
            if let Some(ref mut state) = app.worktree_picker {
                if state.cursor + 1 < state.worktrees.len() {
                    state.cursor += 1;
                }
            }
        }
        Action::WorktreePickerSelect => {
            if let Some(state) = app.worktree_picker.take() {
                if let Some(wt) = state.worktrees.get(state.cursor) {
                    let repo_name = state.repo_root.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let tab_label = format!("{}:{}", repo_name, wt.branch);
                    let area = terminal.size()?;
                    let rows = area.height.saturating_sub(1);
                    let cols = area.width;
                    let _ = app
                        .terminal_manager_mut()
                        .attach_new_named(&wt.path, Some(&tab_label), rows, cols);
                    app.set_view_mode(ViewMode::Terminal);
                }
            }
        }
        Action::WorktreePickerClose => {
            app.worktree_picker = None;
            app.set_view_mode(ViewMode::List);
        }
        Action::ConfirmWorktreeCleanupYes => {
            if let Some(state) = app.confirm_worktree_cleanup.take() {
                if let Some(repo_root) = app
                    .all_sessions()
                    .iter()
                    .find(|s| s.id == state.session_id)
                    .and_then(|s| s.repo_root.clone())
                {
                    let _ =
                        worktree::remove_worktree(&repo_root, &state.worktree_path, state.is_dirty);
                }
            }
            app.set_view_mode(ViewMode::List);
            let _ = app.refresh();
        }
        Action::ConfirmWorktreeCleanupNo => {
            app.confirm_worktree_cleanup = None;
            app.set_view_mode(ViewMode::List);
            let _ = app.refresh();
        }
        Action::ConfirmRecreateYes => {
            if let Some(state) = app.confirm_recreate_worktree.take() {
                match worktree::create_worktree(&state.repo_root, &state.branch, false) {
                    Ok(wt_path) => {
                        let area = terminal.size()?;
                        let rows = area.height.saturating_sub(1);
                        let cols = area.width;
                        app.terminal_manager_mut().attach(
                            &state.session_id,
                            &state.branch,
                            &wt_path,
                            None,
                            rows,
                            cols,
                        )?;
                        app.set_view_mode(ViewMode::Terminal);
                    }
                    Err(_) => {
                        app.set_view_mode(ViewMode::List);
                    }
                }
            } else {
                app.set_view_mode(ViewMode::List);
            }
        }
        Action::ConfirmRecreateNo => {
            app.confirm_recreate_worktree = None;
            app.set_view_mode(ViewMode::List);
        }
        Action::PruneWorktrees => {
            let cwd = app
                .selected_session()
                .and_then(|e| e.as_local())
                .map(|s| s.cwd.clone())
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();
            if let Ok(repo_root) = worktree::resolve_repo_root(&cwd) {
                let _ = worktree::prune_worktrees(&repo_root);
            }
            let _ = app.refresh();
        }
    }
    Ok(())
}

fn trigger_tervezo_panel_refresh(app: &mut App) {
    let config = match app.tervezo_config() {
        Some(c) => c.clone(),
        None => return,
    };
    let tx = match app.tervezo_detail_tx.clone() {
        Some(tx) => tx,
        None => return,
    };
    let (impl_id, loading) = match app.tervezo_detail.as_ref() {
        Some(state) => (state.implementation_id.clone(), state.loading.clone()),
        None => return,
    };

    // Fetch all four panels + status, skipping any already in-flight
    let tabs = [
        TervezoTab::Plan,
        TervezoTab::Analysis,
        TervezoTab::Changes,
        TervezoTab::TestOutput,
    ];
    for &tab in &tabs {
        if loading.contains(&tab) {
            continue;
        }
        if let Some(ref mut state) = app.tervezo_detail {
            state.loading.insert(tab);
        }
        let tx = tx.clone();
        let config = config.clone();
        let impl_id = impl_id.clone();
        std::thread::spawn(move || {
            let client = TervezoClient::new(&config);
            match tab {
                TervezoTab::Plan => match client.get_plan(&impl_id) {
                    Ok(plan) => {
                        let _ = tx.send(TervezoDetailMsg::Plan(plan));
                    }
                    Err(e) => {
                        let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                    }
                },
                TervezoTab::Analysis => match client.get_analysis(&impl_id) {
                    Ok(analysis) => {
                        let _ = tx.send(TervezoDetailMsg::Analysis(analysis));
                    }
                    Err(e) => {
                        let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                    }
                },
                TervezoTab::Changes => match client.get_changes(&impl_id) {
                    Ok(changes) => {
                        let _ = tx.send(TervezoDetailMsg::Changes(changes));
                    }
                    Err(e) => {
                        let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                    }
                },
                TervezoTab::TestOutput => match client.get_test_output(&impl_id) {
                    Ok(reports) => {
                        let _ = tx.send(TervezoDetailMsg::TestOutput(reports));
                    }
                    Err(e) => {
                        let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                    }
                },
            }
        });
    }

    // Also refresh status
    {
        let tx = tx.clone();
        let config = config.clone();
        let impl_id = impl_id.clone();
        std::thread::spawn(move || {
            let client = TervezoClient::new(&config);
            if let Ok(status) = client.get_status(&impl_id) {
                let _ = tx.send(TervezoDetailMsg::Status(status));
            }
        });
    }
}

fn trigger_tervezo_initial_fetch(app: &mut App) {
    let config = match app.tervezo_config() {
        Some(c) => c.clone(),
        None => return,
    };
    let tx = match app.tervezo_detail_tx.clone() {
        Some(tx) => tx,
        None => return,
    };
    let impl_id = match app.tervezo_detail.as_ref() {
        Some(state) => state.implementation_id.clone(),
        None => return,
    };

    if let Some(ref mut state) = app.tervezo_detail {
        state.loading.insert(TervezoTab::Plan);
        state.loading.insert(TervezoTab::Analysis);
        state.loading.insert(TervezoTab::Changes);
        state.loading.insert(TervezoTab::TestOutput);
        state.timeline_error = None;
    }

    // Fetch timeline + plan + status + analysis + changes + test output on background threads
    let tx_timeline = tx.clone();
    let tx_plan = tx.clone();
    let tx_status = tx.clone();
    let tx_analysis = tx.clone();
    let tx_changes = tx.clone();
    let tx_test = tx.clone();
    let config_timeline = config.clone();
    let config_status = config.clone();
    let config_analysis = config.clone();
    let config_changes = config.clone();
    let config_test = config.clone();
    let id_timeline = impl_id.clone();
    let id_plan = impl_id.clone();
    let id_status = impl_id.clone();
    let id_analysis = impl_id.clone();
    let id_changes = impl_id.clone();
    let id_test = impl_id.clone();

    std::thread::spawn(move || {
        let client = TervezoClient::new(&config_timeline);
        match client.get_timeline(&id_timeline, None) {
            Ok(msgs) => {
                let _ = tx_timeline.send(TervezoDetailMsg::Timeline(msgs));
            }
            Err(e) => {
                let _ = tx_timeline.send(TervezoDetailMsg::TimelineError(e));
            }
        }
    });

    std::thread::spawn(move || {
        let client = TervezoClient::new(&config);
        match client.get_plan(&id_plan) {
            Ok(plan) => {
                let _ = tx_plan.send(TervezoDetailMsg::Plan(plan));
            }
            Err(e) => {
                let _ = tx_plan.send(TervezoDetailMsg::Error(TervezoTab::Plan, e));
            }
        }
    });

    // Fetch status (steps info)
    std::thread::spawn(move || {
        let client = TervezoClient::new(&config_status);
        if let Ok(status) = client.get_status(&id_status) {
            let _ = tx_status.send(TervezoDetailMsg::Status(status));
        }
    });

    // Fetch analysis
    std::thread::spawn(move || {
        let client = TervezoClient::new(&config_analysis);
        match client.get_analysis(&id_analysis) {
            Ok(analysis) => {
                let _ = tx_analysis.send(TervezoDetailMsg::Analysis(analysis));
            }
            Err(e) => {
                let _ = tx_analysis.send(TervezoDetailMsg::Error(TervezoTab::Analysis, e));
            }
        }
    });

    // Fetch changes
    std::thread::spawn(move || {
        let client = TervezoClient::new(&config_changes);
        match client.get_changes(&id_changes) {
            Ok(changes) => {
                let _ = tx_changes.send(TervezoDetailMsg::Changes(changes));
            }
            Err(e) => {
                let _ = tx_changes.send(TervezoDetailMsg::Error(TervezoTab::Changes, e));
            }
        }
    });

    // Fetch test output
    std::thread::spawn(move || {
        let client = TervezoClient::new(&config_test);
        match client.get_test_output(&id_test) {
            Ok(reports) => {
                let _ = tx_test.send(TervezoDetailMsg::TestOutput(reports));
            }
            Err(e) => {
                let _ = tx_test.send(TervezoDetailMsg::Error(TervezoTab::TestOutput, e));
            }
        }
    });

    // Fetch PR details if implementation has a PR
    let has_pr = app
        .tervezo_detail
        .as_ref()
        .map(|s| s.implementation.pr_url.is_some())
        .unwrap_or(false);
    if has_pr {
        let pr_config = match app.tervezo_config() {
            Some(c) => c.clone(),
            None => return,
        };
        let pr_tx = match app.tervezo_detail_tx.clone() {
            Some(tx) => tx,
            None => return,
        };
        let pr_id = impl_id;
        std::thread::spawn(move || {
            let client = TervezoClient::new(&pr_config);
            if let Ok(pr) = client.get_pr_details(&pr_id) {
                let _ = pr_tx.send(TervezoDetailMsg::PrDetails(pr));
            }
        });
    }

    // For running implementations: start SSE stream + fetch SSH creds
    let running_info = app.tervezo_detail.as_ref().and_then(|s| {
        if s.implementation.status.is_running() {
            Some(s.implementation_id.clone())
        } else {
            None
        }
    });

    if let Some(impl_id_sse) = running_info {
        app.start_sse_stream(&impl_id_sse);

        // Fetch SSH credentials in background
        if let Some(config) = app.tervezo_config() {
            let ssh_config = config.clone();
            let ssh_id = impl_id_sse;
            let ssh_tx = match app.tervezo_detail_tx.clone() {
                Some(tx) => tx,
                None => return,
            };

            std::thread::spawn(move || {
                let client = TervezoClient::new(&ssh_config);
                if let Ok(creds) = client.get_ssh(&ssh_id) {
                    let _ = ssh_tx.send(TervezoDetailMsg::SshCreds(creds));
                }
            });
        }
    }
}

fn trigger_tervezo_tab_fetch(app: &mut App) {
    let config = match app.tervezo_config() {
        Some(c) => c.clone(),
        None => return,
    };
    let tx = match app.tervezo_detail_tx.clone() {
        Some(tx) => tx,
        None => return,
    };
    let (impl_id, tab, skip_fetch) = match app.tervezo_detail.as_ref() {
        Some(state) => {
            let already_loaded = match state.active_tab {
                TervezoTab::Plan => state.plan_content.is_some(),
                TervezoTab::Changes => state.changes.is_some(),
                TervezoTab::TestOutput => state.test_output.is_some(),
                TervezoTab::Analysis => state.analysis_content.is_some(),
            };
            // Always re-fetch for running implementations; lazy-load for completed ones
            let skip = if state.implementation.status.is_running() {
                false
            } else {
                already_loaded
            };
            (state.implementation_id.clone(), state.active_tab, skip)
        }
        None => return,
    };

    if skip_fetch {
        return;
    }

    if let Some(ref mut state) = app.tervezo_detail {
        if state.loading.contains(&tab) {
            return;
        }
        state.loading.insert(tab);
    }

    std::thread::spawn(move || {
        let client = TervezoClient::new(&config);
        match tab {
            TervezoTab::Plan => match client.get_plan(&impl_id) {
                Ok(plan) => {
                    let _ = tx.send(TervezoDetailMsg::Plan(plan));
                }
                Err(e) => {
                    let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                }
            },
            TervezoTab::Changes => match client.get_changes(&impl_id) {
                Ok(changes) => {
                    let _ = tx.send(TervezoDetailMsg::Changes(changes));
                }
                Err(e) => {
                    let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                }
            },
            TervezoTab::TestOutput => match client.get_test_output(&impl_id) {
                Ok(reports) => {
                    let _ = tx.send(TervezoDetailMsg::TestOutput(reports));
                }
                Err(e) => {
                    let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                }
            },
            TervezoTab::Analysis => match client.get_analysis(&impl_id) {
                Ok(analysis) => {
                    let _ = tx.send(TervezoDetailMsg::Analysis(analysis));
                }
                Err(e) => {
                    let _ = tx.send(TervezoDetailMsg::Error(tab, e));
                }
            },
        }
    });
}

fn execute_tervezo_action(app: &mut App, action: TervezoAction) {
    let config = match app.tervezo_config() {
        Some(c) => c.clone(),
        None => return,
    };
    let tx = match app.tervezo_detail_tx.clone() {
        Some(tx) => tx,
        None => return,
    };
    let impl_id = match app.tervezo_detail.as_ref() {
        Some(state) => state.implementation_id.clone(),
        None => return,
    };

    if let Some(ref mut state) = app.tervezo_detail {
        state.action_loading = true;
    }

    std::thread::spawn(move || {
        let client = TervezoClient::new(&config);
        let result: Result<String, String> = match action {
            TervezoAction::CreatePr => client.create_pr(&impl_id).map(|r| {
                let url = r.pr_url.unwrap_or_default();
                format!("PR created: {}", url)
            }),
            TervezoAction::MergePr => client.merge_pr(&impl_id).map(|_| "PR merged".to_string()),
            TervezoAction::ClosePr => client.close_pr(&impl_id).map(|_| "PR closed".to_string()),
            TervezoAction::ReopenPr => client
                .reopen_pr(&impl_id)
                .map(|_| "PR reopened".to_string()),
            TervezoAction::Restart => client.restart(&impl_id).map(|r| {
                if r.is_new_implementation {
                    let new_id = r.implementation_id.unwrap_or_default();
                    format!("NAVIGATE:{}", new_id)
                } else {
                    "Restarted".to_string()
                }
            }),
            TervezoAction::SendPrompt => {
                // Should not reach here — handled via prompt input mode
                Ok("(use prompt input)".to_string())
            }
            TervezoAction::ViewPrInBrowser => {
                // Handled synchronously in action menu select
                Ok("(handled locally)".to_string())
            }
        };

        match result {
            Ok(msg) => {
                let _ = tx.send(TervezoDetailMsg::ActionSuccess(msg));
            }
            Err(e) => {
                let _ = tx.send(TervezoDetailMsg::ActionError(e));
            }
        }
    });
}

fn submit_tervezo_create(app: &mut App) {
    let (prompt, mode, workspace_id, repo_name, base_branch) = match app.tervezo_create.as_ref() {
        Some(state) => {
            if state.submitting {
                return;
            }
            let ws_id = state
                .workspaces
                .get(state.selected_workspace)
                .map(|w| w.id.clone());
            (
                state.prompt.clone(),
                state.mode.api_value().to_string(),
                ws_id,
                state.repo_url.clone(),
                state.base_branch.clone(),
            )
        }
        None => return,
    };

    if prompt.trim().is_empty() {
        if let Some(ref mut state) = app.tervezo_create {
            state.error = Some("Prompt cannot be empty".to_string());
        }
        return;
    }
    let workspace_id = match workspace_id {
        Some(id) => id,
        None => {
            if let Some(ref mut state) = app.tervezo_create {
                state.error = Some("No workspace selected".to_string());
            }
            return;
        }
    };

    if let Some(ref mut state) = app.tervezo_create {
        state.submitting = true;
        state.error = None;
    }

    let config = match app.tervezo_config() {
        Some(c) => c.clone(),
        None => return,
    };
    let tx = match app.tervezo_create_tx() {
        Some(tx) => tx,
        None => return,
    };

    let repository_name = if repo_name.trim().is_empty() {
        None
    } else {
        Some(repo_name)
    };
    let request = CreateImplementationRequest {
        prompt,
        mode,
        workspace_id,
        repository_name,
        base_branch: if base_branch.trim().is_empty() {
            None
        } else {
            Some(base_branch)
        },
    };

    std::thread::spawn(move || {
        let client = TervezoClient::new(&config);
        match client.create_implementation(&request) {
            Ok(implementation) => {
                let _ = tx.send(TervezoCreateMsg::Success(implementation));
            }
            Err(e) => {
                let _ = tx.send(TervezoCreateMsg::Error(e));
            }
        }
    });
}

fn attach_selected(
    app: &mut App,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    if let Some(entry) = app.selected_session() {
        match entry {
            SessionEntry::Local(session) => {
                let id = session.id.clone();
                let name = session.project_name.clone();
                let cwd = session.cwd.clone();
                let pid = session.pid;

                if session.is_worktree_session()
                    && session.status == crate::session::SessionStatus::Dead
                    && !cwd.exists()
                {
                    if let Some(ref wt_info) = session.worktree_info {
                        app.confirm_recreate_worktree =
                            Some(app::ConfirmRecreateWorktreeState {
                                session_id: id,
                                branch: wt_info.pinned_branch.clone(),
                                repo_root: session
                                    .repo_root
                                    .clone()
                                    .unwrap_or_default(),
                            });
                        app.set_view_mode(ViewMode::ConfirmRecreateWorktree);
                        return Ok(());
                    }
                }

                let area = terminal.size()?;
                let rows = area.height.saturating_sub(1);
                let cols = area.width;
                app.terminal_manager_mut()
                    .attach(&id, &name, &cwd, pid, rows, cols)?;
                app.set_view_mode(ViewMode::Terminal);
            }
            SessionEntry::Remote(_) => {
                tlog!(info, "DIAG: attach_selected → switching to TervezoDetail");
                app.set_view_mode(ViewMode::TervezoDetail);
                tlog!(
                    info,
                    "DIAG: tervezo_detail present={}",
                    app.tervezo_detail.is_some()
                );
                trigger_tervezo_initial_fetch(app);
            }
        }
    }
    Ok(())
}

type EntryData = (
    String,
    String,
    Option<std::path::PathBuf>,
    Option<u32>,
    bool,
);

fn attach_by_index(
    app: &mut App,
    idx: usize,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    let entry_data: Option<EntryData> = app.filtered_sessions().get(idx).map(|e| match e {
        SessionEntry::Local(s) => (
            s.id.clone(),
            s.project_name.clone(),
            Some(s.cwd.clone()),
            s.pid,
            false,
        ),
        SessionEntry::Remote(i) => (i.id.clone(), i.display_name().to_string(), None, None, true),
    });

    if let Some((id, name, cwd, pid, is_remote)) = entry_data {
        if is_remote {
            app.set_selected(idx);
            app.set_view_mode(ViewMode::TervezoDetail);
            trigger_tervezo_initial_fetch(app);
        } else if let Some(cwd) = cwd {
            let area = terminal.size()?;
            let rows = area.height.saturating_sub(1);
            let cols = area.width;
            app.terminal_manager_mut()
                .attach(&id, &name, &cwd, pid, rows, cols)?;
            app.set_view_mode(ViewMode::Terminal);
        }
    }
    Ok(())
}
