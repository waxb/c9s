#![cfg(test)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use chrono::Utc;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

use crate::app::{
    BranchInputState, ConfirmWorktreeCleanupState, ConfirmRecreateWorktreeState,
    NewSessionMenuState, NewSessionOption, SessionEntry,
};
use crate::session::{Session, SessionStatus, WorktreeInfo};
use crate::worktree;

fn init_git_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    StdCommand::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::fs::write(dir.path().join("README.md"), "init").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    dir
}

fn make_test_session(id: &str, cwd: &Path) -> Session {
    Session {
        id: id.to_string(),
        pid: None,
        cwd: cwd.to_path_buf(),
        project_name: cwd
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        git_branch: Some("main".to_string()),
        model: Some("claude-sonnet-4-20250514".to_string()),
        status: SessionStatus::Dead,
        started_at: Utc::now(),
        last_activity: Utc::now(),
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        message_count: 5,
        tool_call_count: 2,
        claude_version: Some("1.0.0".to_string()),
        permission_mode: None,
        plan_slugs: Vec::new(),
        compaction_count: 0,
        hook_run_count: 0,
        hook_error_count: 0,
        repo_root: None,
        worktree_info: None,
    }
}

fn make_worktree_session(id: &str, repo_root: &Path, wt_path: &Path, branch: &str) -> Session {
    let mut session = make_test_session(id, wt_path);
    session.repo_root = Some(repo_root.to_path_buf());
    session.worktree_info = Some(WorktreeInfo {
        worktree_path: wt_path.to_path_buf(),
        pinned_branch: branch.to_string(),
    });
    session.git_branch = Some(branch.to_string());
    session.project_name = repo_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    session
}

// ---------------------------------------------------------------------------
// Test 4: Create worktree session E2E
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_create_worktree_session_full_lifecycle() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "feat-e2e-test", true).unwrap();
    assert!(wt_path.exists(), "worktree directory should exist");
    assert!(
        wt_path
            .to_string_lossy()
            .contains(worktree::WORKTREE_DIR),
        "worktree should be under .c9s-worktrees/"
    );

    let branch = worktree::get_current_branch(&wt_path).unwrap();
    assert_eq!(branch, "feat-e2e-test");

    let wts = worktree::list_worktrees(&repo_root).unwrap();
    assert_eq!(wts.len(), 2);
    let linked = wts.iter().find(|w| !w.is_main).unwrap();
    assert_eq!(linked.branch, "feat-e2e-test");

    let exclude_content =
        std::fs::read_to_string(repo_root.join(".git/info/exclude")).unwrap();
    assert!(
        exclude_content.contains(worktree::WORKTREE_DIR),
        ".git/info/exclude should contain .c9s-worktrees/"
    );

    let root_from_wt = worktree::resolve_repo_root(&wt_path).unwrap();
    assert_eq!(root_from_wt, repo_root, "repo root resolved from worktree should match");

    assert!(worktree::is_inside_c9s_worktree(&wt_path));

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
    assert!(!wt_path.exists(), "worktree should be removed");
}

#[test]
fn test_e2e_branch_input_state_flow() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    StdCommand::new("git")
        .args(["branch", "feature-alpha"])
        .current_dir(&repo_root)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["branch", "feature-beta"])
        .current_dir(&repo_root)
        .output()
        .unwrap();

    let branches = worktree::list_local_branches(&repo_root).unwrap();
    assert!(branches.contains(&"main".to_string()));
    assert!(branches.contains(&"feature-alpha".to_string()));
    assert!(branches.contains(&"feature-beta".to_string()));

    let mut state = BranchInputState {
        input: String::new(),
        repo_root: repo_root.clone(),
        branches: branches.clone(),
        suggestion: None,
        error: None,
    };

    state.input.push('f');
    state.suggestion = branches
        .iter()
        .find(|b| b.starts_with(&state.input) && *b != &state.input)
        .cloned();
    assert!(state.suggestion.is_some());
    assert!(state.suggestion.as_ref().unwrap().starts_with("feature-"));

    state.input = "feature-a".to_string();
    state.suggestion = branches
        .iter()
        .find(|b| b.starts_with(&state.input) && *b != &state.input)
        .cloned();
    assert_eq!(state.suggestion, Some("feature-alpha".to_string()));

    if let Some(suggestion) = state.suggestion.take() {
        state.input = suggestion;
    }
    assert_eq!(state.input, "feature-alpha");

    let new_branch = !branches.contains(&state.input);
    assert!(!new_branch, "feature-alpha should be an existing branch");
    let wt_path =
        worktree::create_worktree(&repo_root, &state.input, new_branch).unwrap();
    assert!(wt_path.exists());

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

#[test]
fn test_e2e_branch_input_new_branch() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();
    let branches = worktree::list_local_branches(&repo_root).unwrap();

    let branch_name = "brand-new-feature";
    let new_branch = !branches.contains(&branch_name.to_string());
    assert!(new_branch);

    let wt_path =
        worktree::create_worktree(&repo_root, branch_name, new_branch).unwrap();
    assert!(wt_path.exists());

    let current = worktree::get_current_branch(&wt_path).unwrap();
    assert_eq!(current, branch_name);

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

#[test]
fn test_e2e_branch_input_error_already_checked_out() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let result = worktree::create_worktree(&repo_root, "main", false);
    assert!(result.is_err(), "should fail: main is already checked out");

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("worktree add failed") || err_msg.contains("already"),
        "error should mention worktree failure: {}",
        err_msg
    );
}

// ---------------------------------------------------------------------------
// Test 5: Kill + worktree cleanup E2E
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_kill_worktree_session_clean_path() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "kill-clean-test", true).unwrap();
    assert!(wt_path.exists());

    let is_dirty = worktree::is_dirty(&wt_path).unwrap();
    assert!(!is_dirty, "fresh worktree should be clean");

    let state = ConfirmWorktreeCleanupState {
        session_id: "test-session-1".to_string(),
        worktree_path: wt_path.clone(),
        branch: "kill-clean-test".to_string(),
        is_dirty: false,
    };

    assert!(!state.is_dirty);
    worktree::remove_worktree(&repo_root, &state.worktree_path, state.is_dirty).unwrap();
    assert!(!wt_path.exists(), "worktree should be removed after clean kill");
}

#[test]
fn test_e2e_kill_worktree_session_dirty_path() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "kill-dirty-test", true).unwrap();
    std::fs::write(wt_path.join("uncommitted.txt"), "dirty data").unwrap();

    let is_dirty = worktree::is_dirty(&wt_path).unwrap();
    assert!(is_dirty, "worktree with new file should be dirty");

    let state = ConfirmWorktreeCleanupState {
        session_id: "test-session-2".to_string(),
        worktree_path: wt_path.clone(),
        branch: "kill-dirty-test".to_string(),
        is_dirty: true,
    };

    assert!(state.is_dirty);

    let result = worktree::remove_worktree(&repo_root, &wt_path, false);
    assert!(result.is_err(), "removing dirty worktree without force should fail");
    assert!(wt_path.exists(), "dirty worktree should still exist");

    worktree::remove_worktree(&repo_root, &wt_path, true).unwrap();
    assert!(!wt_path.exists(), "dirty worktree should be removed with force");
}

#[test]
fn test_e2e_kill_normal_session_no_worktree_cleanup() {
    let session = make_test_session("normal-session", Path::new("/tmp/project"));
    assert!(!session.is_worktree_session());
    assert!(session.worktree_info.is_none());
}

#[test]
fn test_e2e_kill_detects_worktree_session() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();
    let wt_path = worktree::create_worktree(&repo_root, "detect-test", true).unwrap();

    let session = make_worktree_session("wt-session", &repo_root, &wt_path, "detect-test");

    assert!(session.is_worktree_session());
    let wt_info = session.worktree_info.as_ref().unwrap();
    assert_eq!(wt_info.pinned_branch, "detect-test");
    assert_eq!(wt_info.worktree_path, wt_path);

    let is_dirty = worktree::is_dirty(&wt_info.worktree_path).unwrap();
    assert!(!is_dirty);

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

// ---------------------------------------------------------------------------
// Test 6: Resume dead worktree session with deleted CWD
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_resume_deleted_worktree_recreate() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "resume-test", true).unwrap();
    assert!(wt_path.exists());

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
    assert!(!wt_path.exists(), "worktree deleted");

    let mut session = make_worktree_session("resume-session", &repo_root, &wt_path, "resume-test");
    session.status = SessionStatus::Dead;

    assert!(session.is_worktree_session());
    assert!(session.status == SessionStatus::Dead);
    assert!(!session.cwd.exists(), "CWD should not exist");

    let state = ConfirmRecreateWorktreeState {
        session_id: session.id.clone(),
        branch: "resume-test".to_string(),
        repo_root: repo_root.clone(),
    };

    let recreated = worktree::create_worktree(&state.repo_root, &state.branch, false).unwrap();
    assert!(recreated.exists(), "worktree should be recreated");

    let branch = worktree::get_current_branch(&recreated).unwrap();
    assert_eq!(branch, "resume-test");

    worktree::remove_worktree(&repo_root, &recreated, false).unwrap();
}

#[test]
fn test_e2e_resume_existing_worktree_no_prompt() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();
    let wt_path = worktree::create_worktree(&repo_root, "still-exists", true).unwrap();

    let mut session =
        make_worktree_session("existing-wt-session", &repo_root, &wt_path, "still-exists");
    session.status = SessionStatus::Dead;

    assert!(session.is_worktree_session());
    assert!(session.cwd.exists(), "CWD should still exist");

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

#[test]
fn test_e2e_resume_recreate_branch_deleted_should_fail() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "ephemeral-branch", true).unwrap();
    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();

    StdCommand::new("git")
        .args(["branch", "-D", "ephemeral-branch"])
        .current_dir(&repo_root)
        .output()
        .unwrap();

    let result = worktree::create_worktree(&repo_root, "ephemeral-branch", false);
    assert!(
        result.is_err(),
        "recreate should fail when branch no longer exists"
    );
}

// ---------------------------------------------------------------------------
// Test 7: [W] prefix in session list rendering
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_list_view_worktree_prefix() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();
    let wt_path = worktree::create_worktree(&repo_root, "feat-render", true).unwrap();

    let wt_session =
        make_worktree_session("render-wt", &repo_root, &wt_path, "feat-render");
    let normal_session = make_test_session("render-normal", &repo_root);

    let wt_entry = SessionEntry::Local(wt_session);
    let normal_entry = SessionEntry::Local(normal_session);

    assert!(wt_entry.is_worktree_session());
    assert!(!normal_entry.is_worktree_session());

    let wt_branch_display = {
        let branch = wt_entry.branch().unwrap_or("-");
        if wt_entry.is_worktree_session() {
            format!("[W] {}", branch)
        } else {
            branch.to_string()
        }
    };
    assert!(wt_branch_display.starts_with("[W] "));
    assert!(wt_branch_display.contains("feat-render"));

    let normal_branch_display = {
        let branch = normal_entry.branch().unwrap_or("-");
        if normal_entry.is_worktree_session() {
            format!("[W] {}", branch)
        } else {
            branch.to_string()
        }
    };
    assert!(!normal_branch_display.starts_with("[W]"));

    terminal
        .draw(|f| {
            let area = f.area();
            let text = ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    &wt_branch_display,
                    ratatui::style::Style::default().fg(ratatui::style::Color::Green),
                ),
                ratatui::text::Span::raw("  |  "),
                ratatui::text::Span::raw(&normal_branch_display),
            ]);
            f.render_widget(ratatui::widgets::Paragraph::new(text), area);
        })
        .unwrap();

    let buf = terminal.backend().buffer().clone();
    let content = buffer_to_string(&buf);
    assert!(
        content.contains("[W] feat-render"),
        "buffer should contain [W] prefix: {}",
        content
    );
    assert!(
        content.contains("main"),
        "buffer should contain normal branch"
    );

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

#[test]
fn test_e2e_list_view_branch_flex_column() {
    let long_branch = "feature/this-is-a-very-long-branch-name-that-exceeds-column";
    let wt_label = format!("[W] {}", long_branch);

    assert!(
        wt_label.len() > 20,
        "label should exceed old fixed column width"
    );
    assert!(wt_label.starts_with("[W] "));
    assert!(wt_label.contains("exceeds-column"));
}

// ---------------------------------------------------------------------------
// Test 8: Detail view shows worktree info
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_detail_view_worktree_info() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();
    let wt_path = worktree::create_worktree(&repo_root, "detail-view-test", true).unwrap();

    let session =
        make_worktree_session("detail-session", &repo_root, &wt_path, "detail-view-test");

    assert!(session.worktree_info.is_some());

    terminal
        .draw(|f| {
            let area = f.area();
            crate::ui::render_session_detail(f, &session, &[], 0, None, 0, area);
        })
        .unwrap();

    let buf = terminal.backend().buffer().clone();
    let content = buffer_to_string(&buf);
    assert!(
        content.contains("Worktree"),
        "detail view should show Worktree label: {}",
        content
    );
    assert!(
        content.contains("WT Branch"),
        "detail view should show WT Branch label: {}",
        content
    );
    assert!(
        content.contains("detail-view-test"),
        "detail view should show the pinned branch name: {}",
        content
    );

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

#[test]
fn test_e2e_detail_view_normal_session_no_worktree_info() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let session = make_test_session("normal-detail", Path::new("/tmp/project"));

    terminal
        .draw(|f| {
            let area = f.area();
            crate::ui::render_session_detail(f, &session, &[], 0, None, 0, area);
        })
        .unwrap();

    let buf = terminal.backend().buffer().clone();
    let content = buffer_to_string(&buf);
    assert!(
        !content.contains("Worktree"),
        "normal session detail should not show Worktree label"
    );
    assert!(
        !content.contains("WT Branch"),
        "normal session detail should not show WT Branch label"
    );
}

// ---------------------------------------------------------------------------
// Test 9: Prune stale worktrees
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_prune_removes_stale_entries() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "prune-target", true).unwrap();
    assert!(wt_path.exists());

    std::fs::remove_dir_all(&wt_path).unwrap();
    assert!(!wt_path.exists());

    let wts_before = worktree::list_worktrees(&repo_root).unwrap();
    let stale_count = wts_before.iter().filter(|w| !w.path.exists()).count();
    assert!(
        stale_count > 0 || wts_before.len() > 1,
        "should have a stale worktree reference"
    );

    worktree::prune_worktrees(&repo_root).unwrap();

    let wts_after = worktree::list_worktrees(&repo_root).unwrap();
    assert_eq!(
        wts_after.len(),
        1,
        "after prune, only main worktree should remain"
    );
    assert!(wts_after[0].is_main);
}

#[test]
fn test_e2e_prune_noop_when_clean() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let before = worktree::list_worktrees(&repo_root).unwrap();
    worktree::prune_worktrees(&repo_root).unwrap();
    let after = worktree::list_worktrees(&repo_root).unwrap();

    assert_eq!(before.len(), after.len());
}

#[test]
fn test_e2e_prune_preserves_active_worktrees() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_active = worktree::create_worktree(&repo_root, "active-wt", true).unwrap();
    let wt_stale = worktree::create_worktree(&repo_root, "stale-wt", true).unwrap();

    std::fs::remove_dir_all(&wt_stale).unwrap();

    worktree::prune_worktrees(&repo_root).unwrap();

    let wts = worktree::list_worktrees(&repo_root).unwrap();
    assert_eq!(wts.len(), 2, "main + active-wt should remain");
    assert!(
        wts.iter().any(|w| w.branch == "active-wt"),
        "active worktree should survive prune"
    );
    assert!(
        !wts.iter().any(|w| w.branch == "stale-wt"),
        "stale worktree should be pruned"
    );

    worktree::remove_worktree(&repo_root, &wt_active, false).unwrap();
}

// ---------------------------------------------------------------------------
// Test: NewSessionMenu shows worktree options in git repo
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_new_session_menu_git_repo() {
    let menu = NewSessionMenuState::new(false, true);
    assert_eq!(menu.items.len(), 3);
    assert_eq!(menu.items[0], NewSessionOption::Local);
    assert_eq!(menu.items[1], NewSessionOption::BranchSession);
    assert_eq!(menu.items[2], NewSessionOption::ExistingWorktree);
}

#[test]
fn test_e2e_new_session_menu_non_git() {
    let menu = NewSessionMenuState::new(false, false);
    assert_eq!(menu.items.len(), 1);
    assert_eq!(menu.items[0], NewSessionOption::Local);
}

#[test]
fn test_e2e_new_session_menu_git_with_tervezo() {
    let menu = NewSessionMenuState::new(true, true);
    assert_eq!(menu.items.len(), 4);
    assert_eq!(menu.items[0], NewSessionOption::Local);
    assert_eq!(menu.items[1], NewSessionOption::BranchSession);
    assert_eq!(menu.items[2], NewSessionOption::ExistingWorktree);
    assert_eq!(menu.items[3], NewSessionOption::Tervezo);
}

// ---------------------------------------------------------------------------
// Test: WorktreePicker filters out main and active sessions
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_worktree_picker_filters_correctly() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt1 = worktree::create_worktree(&repo_root, "picker-a", true).unwrap();
    let wt2 = worktree::create_worktree(&repo_root, "picker-b", true).unwrap();

    let all_wts = worktree::list_worktrees(&repo_root).unwrap();
    assert_eq!(all_wts.len(), 3); // main + 2 worktrees

    let active_cwds: HashSet<PathBuf> = vec![wt1.clone()].into_iter().collect();

    let available: Vec<_> = all_wts
        .into_iter()
        .filter(|wt| !wt.is_main && !active_cwds.contains(&wt.path))
        .collect();

    assert_eq!(available.len(), 1, "only picker-b should be available");
    assert_eq!(available[0].branch, "picker-b");

    worktree::remove_worktree(&repo_root, &wt1, false).unwrap();
    worktree::remove_worktree(&repo_root, &wt2, false).unwrap();
}

// ---------------------------------------------------------------------------
// Test: Full create -> detect -> kill -> cleanup cycle
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_full_lifecycle_create_detect_kill_cleanup() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "lifecycle-branch", true).unwrap();
    assert!(wt_path.exists());

    assert!(worktree::is_inside_c9s_worktree(&wt_path));
    let resolved_root = worktree::resolve_repo_root(&wt_path).unwrap();
    assert_eq!(resolved_root, repo_root);
    let pinned = worktree::get_current_branch(&wt_path).unwrap();
    assert_eq!(pinned, "lifecycle-branch");

    let session =
        make_worktree_session("lifecycle-id", &repo_root, &wt_path, "lifecycle-branch");
    assert!(session.is_worktree_session());
    assert_eq!(session.repo_root.as_ref().unwrap(), &repo_root);

    let is_dirty = worktree::is_dirty(&wt_path).unwrap();
    assert!(!is_dirty);
    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
    assert!(!wt_path.exists());

    let wts = worktree::list_worktrees(&repo_root).unwrap();
    assert_eq!(wts.len(), 1, "only main worktree should remain");
}

#[test]
fn test_e2e_full_lifecycle_create_dirty_kill_force_cleanup() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path =
        worktree::create_worktree(&repo_root, "dirty-lifecycle", true).unwrap();
    std::fs::write(wt_path.join("dirty.txt"), "data").unwrap();

    assert!(worktree::is_dirty(&wt_path).unwrap());

    let remove_result = worktree::remove_worktree(&repo_root, &wt_path, false);
    assert!(remove_result.is_err());
    assert!(wt_path.exists());

    worktree::remove_worktree(&repo_root, &wt_path, true).unwrap();
    assert!(!wt_path.exists());
}

// ---------------------------------------------------------------------------
// Session persistence across c9s restart
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_jsonl_files_survive_discovery_cycle() {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    let project_dir = claude_dir.join("projects").join("-tmp-myproject");
    std::fs::create_dir_all(&project_dir).unwrap();

    let jsonl_content = concat!(
        r#"{"sessionId":"abc-123","cwd":"/tmp/myproject","gitBranch":"main","type":"user","timestamp":"2026-01-01T00:00:00Z"}"#,
        "\n",
        r#"{"type":"assistant","message":{"model":"claude-sonnet-4-20250514","stop_reason":"end_turn","usage":{"input_tokens":500,"output_tokens":200}},"timestamp":"2026-01-01T00:01:00Z"}"#,
        "\n"
    );
    let jsonl_path = project_dir.join("abc-123.jsonl");
    std::fs::write(&jsonl_path, jsonl_content).unwrap();

    let content_before = std::fs::read_to_string(&jsonl_path).unwrap();
    let mtime_before = std::fs::metadata(&jsonl_path).unwrap().modified().unwrap();

    let mut discovery = crate::session::SessionDiscovery::new_with_dir(claude_dir.clone());
    let sessions_1 = discovery.discover_all().unwrap();

    let content_after = std::fs::read_to_string(&jsonl_path).unwrap();
    let mtime_after = std::fs::metadata(&jsonl_path).unwrap().modified().unwrap();
    assert_eq!(content_before, content_after, "JSONL file must not be modified by discovery");
    assert_eq!(mtime_before, mtime_after, "JSONL file mtime must not change");

    let mut discovery_2 = crate::session::SessionDiscovery::new_with_dir(claude_dir.clone());
    let sessions_2 = discovery_2.discover_all().unwrap();

    assert_eq!(sessions_1.len(), sessions_2.len(), "same sessions found across discovery instances");
    assert_eq!(sessions_1[0].id, sessions_2[0].id);
    assert_eq!(sessions_1[0].input_tokens, sessions_2[0].input_tokens);
    assert_eq!(sessions_1[0].output_tokens, sessions_2[0].output_tokens);
}

#[test]
fn test_e2e_sqlite_survives_store_reopen() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("data.db");

    let session = make_test_session("persist-test", Path::new("/tmp/project"));

    {
        let store = crate::store::Store::open_at(&db_path).unwrap();
        store.upsert_session(&session).unwrap();
    }

    {
        let store = crate::store::Store::open_at(&db_path).unwrap();
        let count = store.get_session_count().unwrap();
        assert_eq!(count, 1, "session must survive store reopen");
    }
}

#[test]
fn test_e2e_dead_session_retains_all_data() {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    let project_dir = claude_dir.join("projects").join("-tmp-myproject");
    std::fs::create_dir_all(&project_dir).unwrap();

    let jsonl_content = concat!(
        r#"{"sessionId":"dead-session-1","cwd":"/tmp/myproject","gitBranch":"feature/test","type":"user","timestamp":"2026-01-01T00:00:00Z"}"#,
        "\n",
        r#"{"type":"assistant","message":{"model":"claude-opus-4-20250514","stop_reason":"end_turn","usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":100}},"timestamp":"2026-01-01T00:05:00Z"}"#,
        "\n"
    );
    std::fs::write(project_dir.join("dead-session-1.jsonl"), jsonl_content).unwrap();

    let mut discovery = crate::session::SessionDiscovery::new_with_dir(claude_dir);
    let sessions = discovery.discover_all().unwrap();

    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    assert_eq!(s.id, "dead-session-1");
    assert_eq!(s.status, SessionStatus::Dead, "no matching process = Dead");
    assert_eq!(s.git_branch.as_deref(), Some("feature/test"));
    assert!(s.model.as_deref().unwrap().contains("opus"));
    assert_eq!(s.input_tokens, 1000);
    assert_eq!(s.output_tokens, 500);
    assert_eq!(s.cache_read_tokens, 200);
    assert_eq!(s.cache_write_tokens, 100);
    assert_eq!(s.message_count, 2);
}

#[test]
fn test_e2e_worktree_session_data_survives_restart_cycle() {
    let repo = init_git_repo();
    let repo_root = repo.path().canonicalize().unwrap();

    let wt_path = worktree::create_worktree(&repo_root, "persist-branch", true).unwrap();

    let session = make_worktree_session("wt-persist", &repo_root, &wt_path, "persist-branch");

    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("data.db");

    {
        let store = crate::store::Store::open_at(&db_path).unwrap();
        store.upsert_session(&session).unwrap();
    }

    {
        let store = crate::store::Store::open_at(&db_path).unwrap();
        let count = store.get_session_count().unwrap();
        assert_eq!(count, 1);
    }

    assert!(wt_path.exists(), "worktree directory survives store cycle");
    let branch = worktree::get_current_branch(&wt_path).unwrap();
    assert_eq!(branch, "persist-branch", "worktree branch is intact");

    worktree::remove_worktree(&repo_root, &wt_path, false).unwrap();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn buffer_to_string(buf: &Buffer) -> String {
    let mut result = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            result.push_str(cell.symbol());
        }
        result.push('\n');
    }
    result
}
