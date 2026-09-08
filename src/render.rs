use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::{App, ViewMode};
use crate::{log, ui};

pub fn draw_frame(f: &mut Frame, app: &App) {
    let full_area = f.area();
    let areas = ui::split_areas(full_area, app.panes());
    let area = areas.main;

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
        ViewMode::Terminal | ViewMode::SessionPane => render_terminal_view(app, f, area),
        ViewMode::TerminalQSwitcher => {
            render_terminal_view(app, f, area);
            ui::render_qswitcher(f, app, area);
        }
        ViewMode::Command => {
            ui::render_session_list(f, app, area);
            ui::render_command_input(f, app.command_input(), area);
        }
        ViewMode::ConfirmKill => {
            render_base(app, f, area);
            let name = app
                .confirm_kill_session_id
                .as_ref()
                .and_then(|id| app.find_session(id))
                .map(ui::session_title)
                .unwrap_or_else(|| "unknown".to_string());
            ui::render_confirm_kill(f, &name, area);
        }
        ViewMode::ConfirmQuit => {
            ui::render_session_list(f, app, area);
            let active = app.active_attached_sessions();
            ui::render_confirm_quit(f, &active, area);
        }
        ViewMode::NameInput => {
            render_base(app, f, area);
            if let Some(input) = app.name_input() {
                let hint = if input.pristine {
                    "type to replace the suggestion, Enter accepts, Esc cancels"
                } else {
                    "Enter accepts, Esc cancels"
                };
                ui::render_input_dialog(
                    f,
                    input.purpose.title(),
                    "name:",
                    &input.value,
                    hint,
                    area,
                );
            }
        }
        ViewMode::MessageInput => {
            render_base(app, f, area);
            if let Some(input) = app.message_input() {
                let title = format!(" Message to {} ", input.target_label);
                ui::render_input_dialog(
                    f,
                    &title,
                    "text:",
                    &input.value,
                    "Enter sends into that session's input, Esc cancels",
                    area,
                );
            }
        }
        ViewMode::ConfirmTakeover => {
            render_base(app, f, area);
            if let Some(prompt) = app.takeover() {
                ui::render_confirm_takeover(f, prompt, area);
            }
        }
        ViewMode::UnhideDialog => {
            render_base(app, f, area);
            if let Some(dialog) = app.unhide() {
                let hidden = app.hidden_sessions(&dialog.root);
                ui::render_unhide_dialog(f, &hidden, dialog.cursor, area);
            }
        }
        ViewMode::Log => {
            let entries = log::entries();
            ui::render_log_panel(f, &entries, app.log_scroll(), area);
        }
    }

    if let Some(pane_area) = areas.session_pane {
        if app.in_terminal_context() {
            ui::render_session_pane(f, app, pane_area);
        }
    }

    if let (Some(side_area), Some(st)) = (areas.shell_pane, app.side_terminal()) {
        ui::render_side_panel(f, st, app.is_side_panel_focused(), side_area);
    }
}

fn render_base(app: &App, f: &mut Frame, area: Rect) {
    if app.in_terminal_context() {
        render_terminal_view(app, f, area);
    } else {
        ui::render_session_list(f, app, area);
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
    let Some(term) = app.terminal_manager().active_terminal() else {
        ui::render_session_list(f, app, area);
        return;
    };
    let guard = term.lock_parser();
    let screen = guard.screen();
    let scrolled = screen.scrollback() > 0;
    let exited = term.is_exited();
    let tabs = app.terminal_manager().tab_info();
    let pane_key = app.config.keys.session_pane.label();
    let data = ui::TerminalViewData {
        tabs: &tabs,
        exited,
        scrolled,
        usage: app.usage(),
        pane_key: &pane_key,
    };
    ui::render_terminal(f, screen, &data, area);
}
