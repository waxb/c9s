mod app;
mod input;
mod log;
mod session;
mod store;
mod terminal;
mod tervezo;
mod ui;
mod usage;

use anyhow::Result;
use app::{App, SessionEntry, TervezoAction, TervezoDetailMsg, TervezoTab, ViewMode};
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
use tervezo::TervezoClient;

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
    let mut last_refresh = Instant::now();
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

        if *app.view_mode() == ViewMode::Log && log::take_dirty() {
            needs_draw = true;
        }

        if needs_draw {
            terminal.draw(|f| {
                let area = f.area();
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
                        let _ = app
                            .terminal_manager()
                            .resize_active(rows.saturating_sub(2), cols);
                    }
                    needs_draw = true;
                }

                let action = handle_event(&ev, app.view_mode());
                let is_noop = matches!(action, Action::None | Action::TerminalInput(_));
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

        let in_terminal = matches!(
            app.view_mode(),
            ViewMode::Terminal | ViewMode::TerminalQSwitcher
        );
        app.terminal_manager_mut()
            .check_and_forward_notifications(in_terminal);

        if matches!(
            app.view_mode(),
            ViewMode::Terminal | ViewMode::TerminalQSwitcher
        ) {
            app.terminal_manager_mut().cleanup_inactive_exited();
        }

        if !matches!(
            app.view_mode(),
            ViewMode::Terminal | ViewMode::TerminalQSwitcher
        ) && last_refresh.elapsed() >= refresh_interval
        {
            app.refresh()?;
            last_refresh = Instant::now();
            needs_draw = true;
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
        ui::render_terminal(f, screen, &tabs, exited, scrolled, area);
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
                ViewMode::QSwitcher | ViewMode::TerminalQSwitcher
            ) {
                app.set_view_mode(ViewMode::Terminal);
            }
        }
        Action::ToggleQSwitcher => {
            if *app.view_mode() == ViewMode::QSwitcher {
                app.set_view_mode(ViewMode::List);
            } else {
                app.set_view_mode(ViewMode::QSwitcher);
            }
        }
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
            ViewMode::TervezoDetail => {
                tlog!(info, "DIAG: Back action from TervezoDetail → List");
                app.set_view_mode(ViewMode::List);
            }
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
        Action::None => {}
    }
    Ok(())
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
    }

    // Fetch timeline + plan + status on background threads
    let tx_timeline = tx.clone();
    let tx_plan = tx.clone();
    let tx_status = tx.clone();
    let config_timeline = config.clone();
    let config_status = config.clone();
    let id_timeline = impl_id.clone();
    let id_plan = impl_id.clone();
    let id_status = impl_id.clone();

    std::thread::spawn(move || {
        let client = TervezoClient::new(&config_timeline);
        match client.get_timeline(&id_timeline, None) {
            Ok(msgs) => {
                let _ = tx_timeline.send(TervezoDetailMsg::Timeline(msgs));
            }
            Err(e) => {
                let _ = tx_timeline.send(TervezoDetailMsg::Error(TervezoTab::Plan, e));
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
    let (impl_id, tab, already_loaded) = match app.tervezo_detail.as_ref() {
        Some(state) => {
            let loaded = match state.active_tab {
                TervezoTab::Plan => state.plan_content.is_some(),
                TervezoTab::Changes => state.changes.is_some(),
                TervezoTab::TestOutput => state.test_output.is_some(),
                TervezoTab::Analysis => state.analysis_content.is_some(),
            };
            (state.implementation_id.clone(), state.active_tab, loaded)
        }
        None => return,
    };

    if already_loaded {
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
