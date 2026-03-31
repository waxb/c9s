use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const WORKTREE_DIR: &str = ".c9s-worktrees";

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub is_main: bool,
}

pub fn resolve_repo_root(cwd: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "rev-parse", "--git-common-dir"])
        .output()
        .context("failed to run git rev-parse --git-common-dir")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("not a git repository: {}", stderr.trim());
    }

    let git_common = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let git_path = PathBuf::from(&git_common);

    let absolute = if git_path.is_relative() {
        cwd.join(&git_path)
    } else {
        git_path
    };

    let canonical = absolute
        .canonicalize()
        .unwrap_or(absolute);

    let repo_root = if canonical.ends_with(".git") {
        canonical.parent().unwrap_or(&canonical).to_path_buf()
    } else {
        canonical
    };

    Ok(repo_root)
}

pub fn list_worktrees(repo_root: &Path) -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "worktree", "list", "--porcelain"])
        .output()
        .context("failed to run git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree list failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in stdout.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                if !is_bare {
                    let is_main = worktrees.is_empty();
                    worktrees.push(Worktree { path, branch, is_main });
                }
            }
            current_path = Some(PathBuf::from(path_str));
            current_branch = None;
            is_bare = false;
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            current_branch = Some(
                branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string(),
            );
        } else if line == "bare" {
            is_bare = true;
        } else if line == "detached" {
            current_branch = Some("(detached)".to_string());
        } else if line.is_empty() {
            if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                if !is_bare {
                    let is_main = worktrees.is_empty();
                    worktrees.push(Worktree { path, branch, is_main });
                }
            }
            current_path = None;
            current_branch = None;
            is_bare = false;
        }
    }

    if let (Some(path), Some(branch)) = (current_path, current_branch) {
        if !is_bare {
            let is_main = worktrees.is_empty();
            worktrees.push(Worktree { path, branch, is_main });
        }
    }

    Ok(worktrees)
}

pub fn create_worktree(repo_root: &Path, branch: &str, new_branch: bool) -> Result<PathBuf> {
    let sanitized = sanitize_branch_name(branch);
    let wt_path = repo_root.join(WORKTREE_DIR).join(&sanitized);

    let _ = prune_worktrees(repo_root);
    ensure_git_info_exclude(repo_root)?;

    let output = if new_branch {
        Command::new("git")
            .args([
                "-C",
                &repo_root.to_string_lossy(),
                "worktree",
                "add",
                "-b",
                branch,
                &wt_path.to_string_lossy(),
            ])
            .output()
            .context("failed to run git worktree add -b")?
    } else {
        Command::new("git")
            .args([
                "-C",
                &repo_root.to_string_lossy(),
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                branch,
            ])
            .output()
            .context("failed to run git worktree add")?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree add failed: {}", stderr.trim());
    }

    let canonical = wt_path.canonicalize().unwrap_or(wt_path);
    Ok(canonical)
}

pub fn remove_worktree(repo_root: &Path, worktree_path: &Path, force: bool) -> Result<()> {
    let repo_str = repo_root.to_string_lossy();
    let wt_str = worktree_path.to_string_lossy();

    let mut cmd = Command::new("git");
    cmd.args(["-C", &repo_str, "worktree", "remove"]);
    if force {
        cmd.arg("--force");
    }
    cmd.arg(&*wt_str);

    let output = cmd.output().context("failed to run git worktree remove")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree remove failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn prune_worktrees(repo_root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "worktree", "prune"])
        .output()
        .context("failed to run git worktree prune")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree prune failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn is_dirty(worktree_path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["-C", &worktree_path.to_string_lossy(), "status", "--porcelain"])
        .output()
        .context("failed to run git status")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git status failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

pub fn get_current_branch(cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("failed to run git rev-parse --abbrev-ref HEAD")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-parse --abbrev-ref HEAD failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn sanitize_branch_name(branch: &str) -> String {
    let mut result: String = branch
        .chars()
        .map(|c| {
            if c == '/' {
                '-'
            } else if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    while result.contains("--") {
        result = result.replace("--", "-");
    }

    result = result.trim_matches(|c| c == '.' || c == '-').to_string();

    if result.is_empty() {
        result = "worktree".to_string();
    }

    result
}

pub fn ensure_git_info_exclude(repo_root: &Path) -> Result<()> {
    let info_dir = repo_root.join(".git").join("info");
    std::fs::create_dir_all(&info_dir)
        .context("failed to create .git/info directory")?;

    let exclude_path = info_dir.join("exclude");
    let exclude_entry = format!("/{}/", WORKTREE_DIR);

    let content = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    if content.contains(&exclude_entry) {
        return Ok(());
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&exclude_path)
        .context("failed to open .git/info/exclude")?;

    if !content.is_empty() && !content.ends_with('\n') {
        writeln!(file)?;
    }
    writeln!(file, "{}", exclude_entry)?;

    Ok(())
}

pub fn list_local_branches(repo_root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "-C",
            &repo_root.to_string_lossy(),
            "branch",
            "--format=%(refname:short)",
        ])
        .output()
        .context("failed to run git branch")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

pub fn is_inside_c9s_worktree(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == WORKTREE_DIR)
}

pub fn ensure_local_clone(remote_url: &str) -> Result<PathBuf> {
    let worktrees_base = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
        .join(".c9s")
        .join("worktrees");
    std::fs::create_dir_all(&worktrees_base)
        .context("failed to create ~/.c9s/worktrees/")?;

    let repo_name = remote_url
        .rsplit('/')
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git");
    let clone_path = worktrees_base.join(repo_name);

    if clone_path.join(".git").exists() {
        let output = Command::new("git")
            .args(["-C", &clone_path.to_string_lossy(), "fetch", "origin"])
            .output()
            .context("failed to run git fetch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git fetch failed: {}", stderr.trim());
        }
    } else {
        let output = Command::new("git")
            .args(["clone", remote_url, &clone_path.to_string_lossy()])
            .output()
            .context("failed to run git clone")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git clone failed: {}", stderr.trim());
        }
    }

    Ok(clone_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    fn init_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
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

    #[test]
    fn test_sanitize_branch_simple() {
        assert_eq!(sanitize_branch_name("main"), "main");
    }

    #[test]
    fn test_sanitize_branch_slashes() {
        assert_eq!(sanitize_branch_name("feature/foo/bar"), "feature-foo-bar");
    }

    #[test]
    fn test_sanitize_branch_special_chars() {
        assert_eq!(sanitize_branch_name("feat@#$/x"), "feat-x");
    }

    #[test]
    fn test_sanitize_branch_leading_trailing() {
        assert_eq!(sanitize_branch_name(".foo-"), "foo");
    }

    #[test]
    fn test_sanitize_branch_consecutive_dashes() {
        assert_eq!(sanitize_branch_name("a///b"), "a-b");
    }

    #[test]
    fn test_sanitize_branch_empty() {
        assert_eq!(sanitize_branch_name("..."), "worktree");
    }

    #[test]
    fn test_resolve_repo_root_main_checkout() {
        let dir = init_git_repo();
        let root = resolve_repo_root(dir.path()).unwrap();
        assert_eq!(root.canonicalize().unwrap(), dir.path().canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_repo_root_non_git_dir() {
        let dir = TempDir::new().unwrap();
        assert!(resolve_repo_root(dir.path()).is_err());
    }

    #[test]
    fn test_list_worktrees_main_only() {
        let dir = init_git_repo();
        let wts = list_worktrees(dir.path()).unwrap();
        assert_eq!(wts.len(), 1);
        assert!(wts[0].is_main);
        assert_eq!(wts[0].branch, "main");
    }

    #[test]
    fn test_create_and_remove_worktree() {
        let dir = init_git_repo();
        let wt_path = create_worktree(dir.path(), "test-branch", true).unwrap();
        assert!(wt_path.exists());

        let wts = list_worktrees(dir.path()).unwrap();
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[1].branch, "test-branch");

        remove_worktree(dir.path(), &wt_path, false).unwrap();
        assert!(!wt_path.exists());
    }

    #[test]
    fn test_create_worktree_existing_branch() {
        let dir = init_git_repo();
        StdCommand::new("git")
            .args(["branch", "existing-branch"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let wt_path = create_worktree(dir.path(), "existing-branch", false).unwrap();
        assert!(wt_path.exists());

        let branch = get_current_branch(&wt_path).unwrap();
        assert_eq!(branch, "existing-branch");

        remove_worktree(dir.path(), &wt_path, false).unwrap();
    }

    #[test]
    fn test_is_dirty_clean() {
        let dir = init_git_repo();
        assert!(!is_dirty(dir.path()).unwrap());
    }

    #[test]
    fn test_is_dirty_with_changes() {
        let dir = init_git_repo();
        std::fs::write(dir.path().join("new_file.txt"), "dirty").unwrap();
        assert!(is_dirty(dir.path()).unwrap());
    }

    #[test]
    fn test_ensure_git_info_exclude_idempotent() {
        let dir = init_git_repo();
        ensure_git_info_exclude(dir.path()).unwrap();
        ensure_git_info_exclude(dir.path()).unwrap();

        let content = std::fs::read_to_string(dir.path().join(".git/info/exclude")).unwrap();
        let count = content.matches(&format!("/{}/", WORKTREE_DIR)).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_local_branches() {
        let dir = init_git_repo();
        let branches = list_local_branches(dir.path()).unwrap();
        assert!(branches.contains(&"main".to_string()));
    }

    #[test]
    fn test_resolve_repo_root_in_worktree() {
        let dir = init_git_repo();
        let wt_path = create_worktree(dir.path(), "wt-test", true).unwrap();

        let root_from_wt = resolve_repo_root(&wt_path).unwrap();
        let root_from_main = resolve_repo_root(dir.path()).unwrap();
        assert_eq!(root_from_wt, root_from_main);

        remove_worktree(dir.path(), &wt_path, false).unwrap();
    }

    #[test]
    fn test_is_inside_c9s_worktree() {
        assert!(is_inside_c9s_worktree(Path::new("/repo/.c9s-worktrees/feat-branch/src")));
        assert!(!is_inside_c9s_worktree(Path::new("/repo/src/main.rs")));
    }

    #[test]
    fn test_create_worktree_branch_already_checked_out() {
        let dir = init_git_repo();
        let result = create_worktree(dir.path(), "main", false);
        assert!(result.is_err());
    }
}
