use anyhow::Result;
use ratatui::layout::Rect;
use std::path::PathBuf;

use crate::app::{
    message_payload, rename_payload, App, MessageInput, NameInput, NamePurpose, TakeoverPrompt,
    UnhideDialog, ViewMode,
};
use crate::input::Action;
use crate::session::{self, SessionStatus};
use crate::terminal::kill_process;
use crate::ui;
use crate::{log, tlog, Term};

pub fn screen_area(terminal: &Term) -> Result<Rect> {
    let size = terminal.size()?;
    Ok(Rect::new(0, 0, size.width, size.height))
}

fn pty_dims(terminal: &Term, app: &App) -> Result<(u16, u16)> {
    Ok(ui::pty_size(screen_area(terminal)?, app.panes()))
}

pub fn resize_panes(app: &App, area: Rect) {
    let panes = app.panes();
    if app.in_terminal_context() {
        let (pty_rows, pty_cols) = ui::pty_size(area, panes);
        let _ = app.terminal_manager().resize_active(pty_rows, pty_cols);
    }
    if app.is_side_panel_open() {
        let (panel_rows, panel_cols) = ui::shell_pty_size(area, panes);
        if let Some(st) = app.side_terminal() {
            let _ = st.resize(panel_rows, panel_cols);
        }
    }
}

pub fn process_action(app: &mut App, action: Action, terminal: &Term) -> Result<()> {
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
            if let Some(id) = target_session_id(app) {
                app.confirm_kill_session_id = Some(id);
                app.open_dialog(ViewMode::ConfirmKill);
            }
        }
        Action::ConfirmKill => {
            if let Some(session_id) = app.confirm_kill_session_id.take() {
                discard_session(app, &session_id);
            }
            app.close_dialog();
            app.request_refresh();
        }
        Action::CancelKill => {
            app.confirm_kill_session_id = None;
            app.close_dialog();
        }
        Action::ForkSession => open_fork_dialog(app),
        Action::RenameSession => open_rename_dialog(app),
        Action::MessageSession => open_message_dialog(app),
        Action::CoordinateSession => coordinate_with_selected(app),
        Action::OpenUnhideDialog => open_unhide_dialog(app),
        Action::UnhideSelect => {
            if let Some(id) = app.unhide_selected_id() {
                app.unhide_session(&id);
            }
            app.close_dialog();
        }
        Action::UnhideClose => app.close_dialog(),
        Action::NameChar(c) => {
            if let Some(input) = app.name_input_mut() {
                input.push(c);
            }
        }
        Action::NameBackspace => {
            if let Some(input) = app.name_input_mut() {
                input.pop();
            }
        }
        Action::NameSubmit => submit_name(app, terminal)?,
        Action::NameCancel => {
            app.take_name_input();
            app.close_dialog();
        }
        Action::MessageChar(c) => {
            if let Some(input) = app.message_input_mut() {
                input.push(c);
            }
        }
        Action::MessageBackspace => {
            if let Some(input) = app.message_input_mut() {
                input.pop();
            }
        }
        Action::MessageSubmit => submit_message(app),
        Action::MessageCancel => {
            app.take_message_input();
            app.close_dialog();
        }
        Action::TakeoverConfirm => {
            if let Some(prompt) = app.take_takeover() {
                app.close_dialog();
                if let Some(pid) = prompt.verified_pid {
                    attach_now(
                        app,
                        &prompt.session_id,
                        &prompt.project_name,
                        &prompt.root,
                        Some(pid),
                        terminal,
                    )?;
                }
            }
        }
        Action::TakeoverFork => {
            if let Some(prompt) = app.take_takeover() {
                app.close_dialog();
                let suggestion = app.generate_session_name();
                app.open_name_input(NameInput::new(
                    NamePurpose::Fork,
                    prompt.root,
                    Some(prompt.session_id),
                    suggestion,
                ));
            }
        }
        Action::TakeoverCancel => {
            app.take_takeover();
            app.close_dialog();
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
            if let Some(id) = target_session_id(app) {
                app.hide_session(&id);
            }
        }
        Action::ToggleExpand => app.toggle_expand_selected(),
        Action::ExpandRow => match app.view_mode() {
            ViewMode::QSwitcher | ViewMode::TerminalQSwitcher => app.quick_switch_expand(),
            _ => app.expand_selected(),
        },
        Action::CollapseRow => match app.view_mode() {
            ViewMode::QSwitcher | ViewMode::TerminalQSwitcher => app.quick_switch_collapse(),
            _ => app.collapse_selected(),
        },
        Action::ToggleSessionPane => toggle_session_pane(app, terminal)?,
        Action::ResumeSessionPicker => {
            if let Some(cwd) = app.selected_session().map(|s| s.cwd.clone()) {
                let (rows, cols) = pty_dims(terminal, app)?;
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
            ViewMode::SessionPane => attach_selected(app, terminal)?,
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
            app.close_session_pane();
            app.set_view_mode(ViewMode::List);
        }
        Action::TerminalInput(bytes) => {
            let _ = app.terminal_manager_mut().write_to_active(&bytes);
        }
        Action::CycleNextSession => {
            app.terminal_manager_mut().cycle_next();
            app.sync_pane_cursor();
        }
        Action::CyclePrevSession => {
            app.terminal_manager_mut().cycle_prev();
            app.sync_pane_cursor();
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
        Action::Back => handle_back(app),
        Action::ShowDetail => {
            if *app.view_mode() == ViewMode::List && app.selected_session().is_some() {
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
        Action::Refresh => app.request_refresh(),
        Action::LaunchNew => launch_new(app),
        Action::CommandInput(c) => app.command_push(c),
        Action::CommandBackspace => app.command_pop(),
        Action::CommandSubmit => submit_new_session_path(app),
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
            let area = screen_area(terminal)?;
            if app.is_side_panel_open() {
                app.close_side_panel();
            } else {
                let (panel_rows, panel_cols) = ui::shell_pty_size(area, app.panes());
                app.open_side_panel(panel_rows, panel_cols);
            }
            resize_panes(app, area);
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
        ViewMode::SessionPane => {
            app.close_session_pane();
            app.set_view_mode(ViewMode::Terminal);
        }
        ViewMode::List if app.has_active_filter() => app.clear_filter(),
        _ => {}
    }
}

fn target_session_id(app: &App) -> Option<String> {
    if *app.view_mode() == ViewMode::SessionPane {
        app.pane_selected_id()
    } else {
        app.selected_session().map(|s| s.id.clone())
    }
}

fn toggle_session_pane(app: &mut App, terminal: &Term) -> Result<()> {
    if !app.in_terminal_context() {
        return Ok(());
    }
    if app.is_session_pane_open() && *app.view_mode() == ViewMode::SessionPane {
        app.close_session_pane();
        app.set_view_mode(ViewMode::Terminal);
    } else {
        app.open_session_pane();
        app.set_view_mode(ViewMode::SessionPane);
    }
    resize_panes(app, screen_area(terminal)?);
    Ok(())
}

fn launch_new(app: &mut App) {
    if *app.view_mode() == ViewMode::SessionPane {
        if let Some(root) = app
            .terminal_manager()
            .active_root()
            .map(|r| r.to_path_buf())
        {
            let suggestion = app.generate_session_name();
            app.open_name_input(NameInput::new(NamePurpose::New, root, None, suggestion));
        }
    } else {
        app.set_view_mode(ViewMode::Command);
    }
}

fn submit_new_session_path(app: &mut App) {
    let input = app.command_take();
    let path = input.trim().to_string();
    if path.is_empty() {
        app.set_view_mode(ViewMode::List);
        return;
    }
    let expanded = crate::app::expand_home(&path);
    let cwd = std::fs::canonicalize(&expanded).unwrap_or_else(|_| PathBuf::from(&expanded));
    if cwd.is_dir() {
        app.set_view_mode(ViewMode::List);
        let suggestion = app.generate_session_name();
        app.open_name_input(NameInput::new(NamePurpose::New, cwd, None, suggestion));
    } else {
        tlog!(warn, "new session: not a directory: {}", cwd.display());
        app.set_view_mode(ViewMode::List);
    }
}

fn open_fork_dialog(app: &mut App) {
    let Some((id, root)) = target_session_id(app)
        .and_then(|id| app.find_session(&id).map(|s| (s.id.clone(), s.cwd.clone())))
    else {
        return;
    };
    let suggestion = app.generate_session_name();
    app.open_name_input(NameInput::new(
        NamePurpose::Fork,
        root,
        Some(id),
        suggestion,
    ));
}

fn open_rename_dialog(app: &mut App) {
    let Some(id) = target_session_id(app) else {
        return;
    };
    if !app.terminal_manager().is_attached(&id) {
        app.set_pane_status("rename needs an attached session");
        return;
    }
    let current = app
        .find_session(&id)
        .and_then(|s| s.session_name.clone())
        .unwrap_or_else(|| app.generate_session_name());
    let root = app
        .find_session(&id)
        .map(|s| s.cwd.clone())
        .unwrap_or_default();
    app.open_name_input(NameInput::new(NamePurpose::Rename, root, Some(id), current));
}

fn submit_name(app: &mut App, terminal: &Term) -> Result<()> {
    let Some(input) = app.take_name_input() else {
        return Ok(());
    };
    let Some(name) = input.accepted() else {
        app.open_name_input(input);
        return Ok(());
    };
    app.close_dialog();
    match input.purpose {
        NamePurpose::New => {
            let (rows, cols) = pty_dims(terminal, app)?;
            let id = app
                .terminal_manager_mut()
                .attach_new(&input.root, &name, rows, cols)?;
            app.set_project_current(&input.root, &id);
            app.enter_terminal();
        }
        NamePurpose::Fork => {
            let Some(source) = input.target else {
                return Ok(());
            };
            let (rows, cols) = pty_dims(terminal, app)?;
            let id =
                app.terminal_manager_mut()
                    .attach_fork(&input.root, &source, &name, rows, cols)?;
            app.set_project_current(&input.root, &id);
            app.enter_terminal();
        }
        NamePurpose::Rename => {
            if let Some(target) = input.target {
                let sent = app
                    .terminal_manager_mut()
                    .write_to_session(&target, &rename_payload(&name))?;
                if sent {
                    app.set_pane_status(&format!("renamed to {}", name));
                }
            }
        }
    }
    Ok(())
}

fn open_message_dialog(app: &mut App) {
    let Some(id) = target_session_id(app) else {
        return;
    };
    if !app.terminal_manager().is_attached(&id) {
        app.set_pane_status("message needs an attached session");
        return;
    }
    let label = app
        .find_session(&id)
        .map(ui::session_title)
        .unwrap_or_else(|| id[..8.min(id.len())].to_string());
    app.open_message_input(MessageInput {
        target: id,
        target_label: label,
        value: String::new(),
    });
}

fn submit_message(app: &mut App) {
    let Some(input) = app.take_message_input() else {
        return;
    };
    app.close_dialog();
    if input.value.trim().is_empty() {
        return;
    }
    let sender = app.sender_name();
    let payload = message_payload(&sender, &input.value);
    match app
        .terminal_manager_mut()
        .write_to_session(&input.target, &payload)
    {
        Ok(true) => app.set_pane_status(&format!("sent to {}", input.target_label)),
        Ok(false) => app.set_pane_status("target is no longer attached"),
        Err(e) => app.set_pane_status(&format!("send failed: {}", e)),
    }
}

fn coordinate_with_selected(app: &mut App) {
    let Some(id) = target_session_id(app) else {
        return;
    };
    if Some(id.as_str()) == app.attached_session_id() {
        app.set_pane_status("select another session to coordinate with");
        return;
    }
    let Some(peer) = app.find_session(&id).and_then(|s| s.session_name.clone()) else {
        app.set_pane_status("peer has no registry name (not live)");
        return;
    };
    let prompt = crate::app::coordinate_prompt(&peer);
    if app
        .terminal_manager_mut()
        .write_to_active(prompt.as_bytes())
        .is_ok()
    {
        app.set_pane_status("prompt typed into current session, edit and submit");
    }
}

fn open_unhide_dialog(app: &mut App) {
    let Some(root) = app.selected_project().map(|p| p.root.clone()) else {
        return;
    };
    app.open_unhide(UnhideDialog { root, cursor: 0 });
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
    app.session_files.clear();
    let Some(session_id) = picked else {
        app.set_view_mode(ViewMode::Detail);
        return Ok(());
    };
    let Some((project_name, cwd)) = app
        .selected_session()
        .map(|s| (s.project_name.clone(), s.cwd.clone()))
    else {
        app.set_view_mode(ViewMode::Detail);
        return Ok(());
    };
    app.set_view_mode(ViewMode::List);
    attach_target(app, &session_id, &project_name, &cwd, terminal)
}

fn discard_session(app: &mut App, session_id: &str) {
    let discarded = app.terminal_manager_mut().discard(session_id);
    if !discarded {
        if let Some(pid) = app.find_session(session_id).and_then(|s| s.takeover_pid()) {
            kill_process(pid);
        }
    }
    app.hide_session(session_id);
    app.sync_pane_cursor();
    if app.terminal_manager().active_terminal().is_none() && app.in_terminal_context() {
        app.close_session_pane();
        app.set_view_mode(ViewMode::List);
    }
}

fn attach_selected(app: &mut App, terminal: &Term) -> Result<()> {
    let target = match app.view_mode() {
        ViewMode::QSwitcher | ViewMode::TerminalQSwitcher => app.quick_switch_selected_session(),
        ViewMode::SessionPane => app.pane_selected_id().and_then(|id| app.find_session(&id)),
        _ => app.selected_session(),
    }
    .map(|s| (s.id.clone(), s.project_name.clone(), s.cwd.clone()));
    if *app.view_mode() == ViewMode::SessionPane && target.is_none() {
        if let Some(id) = app.pane_selected_id() {
            app.terminal_manager_mut().switch_to(&id);
            app.enter_terminal();
        }
        return Ok(());
    }
    let Some((id, name, cwd)) = target else {
        return Ok(());
    };
    attach_target(app, &id, &name, &cwd, terminal)
}

fn attach_by_index(app: &mut App, idx: usize, terminal: &Term) -> Result<()> {
    let target = match app.view_mode() {
        ViewMode::QSwitcher | ViewMode::TerminalQSwitcher => {
            app.quick_switch_nth_project_session(idx)
        }
        ViewMode::SessionPane => app
            .pane_rows()
            .get(idx)
            .and_then(|row| app.find_session(&row.session_id)),
        _ => app.nth_project_session(idx),
    }
    .map(|s| (s.id.clone(), s.project_name.clone(), s.cwd.clone()));
    let Some((id, name, cwd)) = target else {
        return Ok(());
    };
    attach_target(app, &id, &name, &cwd, terminal)
}

fn attach_target(
    app: &mut App,
    id: &str,
    project_name: &str,
    root: &std::path::Path,
    terminal: &Term,
) -> Result<()> {
    if app.terminal_manager().is_attached(id) {
        app.terminal_manager_mut().switch_to(id);
        app.set_project_current(root, id);
        app.enter_terminal();
        return Ok(());
    }
    let live_elsewhere = app
        .find_session(id)
        .is_some_and(|s| s.status != SessionStatus::Dead && s.pid.is_some());
    if live_elsewhere {
        let verified_pid = app.find_session(id).and_then(|s| s.takeover_pid());
        let label = app
            .find_session(id)
            .map(ui::session_title)
            .unwrap_or_else(|| id.to_string());
        app.open_takeover(TakeoverPrompt {
            session_id: id.to_string(),
            project_name: project_name.to_string(),
            root: root.to_path_buf(),
            label,
            verified_pid,
        });
        return Ok(());
    }
    attach_now(app, id, project_name, root, None, terminal)
}

fn attach_now(
    app: &mut App,
    id: &str,
    project_name: &str,
    root: &std::path::Path,
    takeover_pid: Option<u32>,
    terminal: &Term,
) -> Result<()> {
    let (rows, cols) = pty_dims(terminal, app)?;
    app.terminal_manager_mut()
        .attach(id, project_name, root, takeover_pid, rows, cols)?;
    app.set_project_current(root, id);
    app.enter_terminal();
    log::take_dirty();
    Ok(())
}
