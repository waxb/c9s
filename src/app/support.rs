use std::path::{Path, PathBuf};

use super::CiStatus;

pub fn expand_home(input: &str) -> String {
    match (input.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.to_string_lossy().to_string() + rest,
        _ => input.to_string(),
    }
}

pub fn display_path(path: &Path) -> String {
    contract_home(&path.to_string_lossy())
}

pub fn resolve_new_session_dir(input: &str, fallback: Option<&Path>) -> Result<PathBuf, String> {
    let typed = input.trim();
    let candidate = if typed.is_empty() {
        fallback
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "enter a directory path".to_string())?
    } else {
        PathBuf::from(expand_home(typed))
    };
    if !candidate.is_dir() {
        return Err("not a directory".to_string());
    }
    Ok(std::fs::canonicalize(&candidate).unwrap_or(candidate))
}

fn contract_home(path: &str) -> String {
    match dirs::home_dir() {
        Some(home) => {
            let home_str = home.to_string_lossy().to_string();
            match path.strip_prefix(&home_str) {
                Some(rest) => format!("~{}", rest),
                None => path.to_string(),
            }
        }
        None => path.to_string(),
    }
}

pub fn complete_directory(input: &str) -> Option<String> {
    let expanded = expand_home(input);
    let path = Path::new(&expanded);
    let (dir, prefix) = if path.is_dir() && expanded.ends_with('/') {
        (path.to_path_buf(), String::new())
    } else {
        let dir = path.parent().unwrap_or(Path::new("."));
        let prefix = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (dir.to_path_buf(), prefix)
    };

    let entries = std::fs::read_dir(&dir).ok()?;
    let mut matches: Vec<String> = entries
        .flatten()
        .filter(|e| {
            e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && e.file_name().to_string_lossy().starts_with(&prefix)
        })
        .map(|e| {
            let full = format!("{}/", dir.join(e.file_name()).to_string_lossy());
            if input.starts_with('~') {
                contract_home(&full)
            } else {
                full
            }
        })
        .collect();
    matches.sort();

    match matches.len() {
        0 => None,
        1 => matches.pop(),
        _ => {
            let common = longest_common_prefix(&matches);
            (common.len() > input.len()).then_some(common)
        }
    }
}

fn longest_common_prefix(strings: &[String]) -> String {
    let Some(first) = strings.first() else {
        return String::new();
    };
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.chars().zip(s.chars()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}

pub fn check_ci_for_local(cwd: &Path, branch: &str) -> CiStatus {
    let output = std::process::Command::new("gh")
        .args([
            "run",
            "list",
            "--branch",
            branch,
            "--limit",
            "1",
            "--json",
            "conclusion,status",
        ])
        .current_dir(cwd)
        .output();
    match output {
        Ok(o) if o.status.success() => parse_gh_runs_json(&String::from_utf8_lossy(&o.stdout)),
        _ => CiStatus::None,
    }
}

fn parse_gh_runs_json(text: &str) -> CiStatus {
    let runs: Vec<serde_json::Value> = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return CiStatus::None,
    };
    let Some(run) = runs.first() else {
        return CiStatus::None;
    };
    let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let conclusion = run.get("conclusion").and_then(|v| v.as_str()).unwrap_or("");

    if status == "in_progress" || status == "queued" {
        CiStatus::Running
    } else if conclusion == "success" {
        CiStatus::Passing
    } else if conclusion == "failure" {
        CiStatus::Failing
    } else {
        CiStatus::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest_common_prefix() {
        let items = vec!["/a/foo/".to_string(), "/a/foobar/".to_string()];
        assert_eq!(longest_common_prefix(&items), "/a/foo");
        assert_eq!(longest_common_prefix(&[]), "");
        assert_eq!(longest_common_prefix(&["/x/".to_string()]), "/x/");
    }

    #[test]
    fn test_parse_gh_runs_json() {
        assert_eq!(
            parse_gh_runs_json(r#"[{"status":"completed","conclusion":"success"}]"#),
            CiStatus::Passing
        );
        assert_eq!(
            parse_gh_runs_json(r#"[{"status":"completed","conclusion":"failure"}]"#),
            CiStatus::Failing
        );
        assert_eq!(
            parse_gh_runs_json(r#"[{"status":"in_progress","conclusion":""}]"#),
            CiStatus::Running
        );
        assert_eq!(parse_gh_runs_json("[]"), CiStatus::None);
        assert_eq!(parse_gh_runs_json("not json"), CiStatus::None);
    }

    #[test]
    fn test_complete_directory_single_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("alpha")).unwrap();
        std::fs::create_dir(dir.path().join("beta")).unwrap();
        let input = format!("{}/al", dir.path().display());
        let completed = complete_directory(&input).unwrap();
        assert!(completed.ends_with("/alpha/"));
    }

    #[test]
    fn test_complete_directory_common_prefix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("proj-one")).unwrap();
        std::fs::create_dir(dir.path().join("proj-two")).unwrap();
        let input = format!("{}/p", dir.path().display());
        let completed = complete_directory(&input).unwrap();
        assert!(completed.ends_with("/proj-"));
        assert_eq!(complete_directory(&completed), None);
    }

    #[test]
    fn test_resolve_new_session_dir_falls_back_to_selected_root() {
        let dir = tempfile::tempdir().unwrap();
        let want = std::fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            resolve_new_session_dir("", Some(dir.path())),
            Ok(want.clone())
        );
        assert_eq!(resolve_new_session_dir("   ", Some(dir.path())), Ok(want));
    }

    #[test]
    fn test_resolve_new_session_dir_requires_path_without_fallback() {
        let err = resolve_new_session_dir("", None).unwrap_err();
        assert!(err.contains("directory"), "{}", err);
    }

    #[test]
    fn test_resolve_new_session_dir_rejects_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = format!("{}/does-not-exist", dir.path().display());
        let err = resolve_new_session_dir(&bogus, Some(dir.path())).unwrap_err();
        assert!(err.contains("not a directory"), "{}", err);
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(resolve_new_session_dir(&file.to_string_lossy(), None).is_err());
    }

    #[test]
    fn test_resolve_new_session_dir_accepts_typed_path() {
        let dir = tempfile::tempdir().unwrap();
        let want = std::fs::canonicalize(dir.path()).unwrap();
        let typed = format!(" {} ", dir.path().display());
        assert_eq!(resolve_new_session_dir(&typed, None), Ok(want));
    }

    #[test]
    fn test_display_path_contracts_home() {
        let home = dirs::home_dir().unwrap();
        let shown = display_path(&home.join("work").join("c9s"));
        assert_eq!(shown, "~/work/c9s");
        assert_eq!(display_path(Path::new("/opt/x")), "/opt/x");
    }
}
