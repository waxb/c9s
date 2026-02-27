use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub category: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ConfigItem {
    pub label: String,
    pub path: Option<PathBuf>,
    pub kind: ConfigItemKind,
    pub tokens: Option<u32>,
    pub always_loaded: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigItemKind {
    SectionHeader,
    Category,
    FileExists,
    FileMissing,
    MemoryFile,
    SectionTotal,
}

fn estimate_file_tokens(path: &Path) -> (Option<u32>, Option<bool>) {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return (None, None),
    };
    let tokens = (meta.len() / 4) as u32;

    let mut buf = [0u8; 512];
    let frontmatter = match std::fs::File::open(path) {
        Ok(mut f) => {
            let n = f.read(&mut buf).unwrap_or(0);
            String::from_utf8_lossy(&buf[..n]).to_string()
        }
        Err(_) => return (Some(tokens), Some(true)),
    };

    if !frontmatter.starts_with("---") {
        return (Some(tokens), Some(true));
    }

    let has_paths = frontmatter.contains("\npaths:") || frontmatter.contains("\npaths :");
    let has_always_apply_false =
        frontmatter.contains("alwaysApply: false") || frontmatter.contains("alwaysApply:false");

    if has_paths || has_always_apply_false {
        (Some(tokens), Some(false))
    } else {
        (Some(tokens), Some(true))
    }
}

fn make_item(label: String, path: Option<PathBuf>, kind: ConfigItemKind) -> ConfigItem {
    let (tokens, always_loaded) = match &path {
        Some(p)
            if matches!(
                kind,
                ConfigItemKind::FileExists | ConfigItemKind::MemoryFile
            ) =>
        {
            estimate_file_tokens(p)
        }
        _ => (None, None),
    };
    ConfigItem {
        label,
        path,
        kind,
        tokens,
        always_loaded,
    }
}

pub fn build_config_items(cfg: &SessionConfig, cwd: &Path) -> Vec<ConfigItem> {
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".claude");
    let encoded_cwd = cwd.to_string_lossy().replace('/', "-");
    let memory_dir = claude_dir
        .join("projects")
        .join(&encoded_cwd)
        .join("memory");

    let mut items = Vec::new();
    let mut global_total: u32 = 0;
    let mut global_always: u32 = 0;

    items.push(ConfigItem {
        label: "Global (~/.claude/)".to_string(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
        tokens: None,
        always_loaded: None,
    });

    let global_md = claude_dir.join("CLAUDE.md");
    let item = if cfg.global_claude_md {
        make_item(
            "CLAUDE.md".to_string(),
            Some(global_md),
            ConfigItemKind::FileExists,
        )
    } else {
        make_item("CLAUDE.md".to_string(), None, ConfigItemKind::FileMissing)
    };
    accumulate(&item, &mut global_total, &mut global_always);
    items.push(item);

    if cfg.global_rules.is_empty() {
        items.push(make_item(
            "rules/  [empty]".to_string(),
            None,
            ConfigItemKind::FileMissing,
        ));
    } else {
        let mut current_cat = String::new();
        for entry in &cfg.global_rules {
            if entry.category != current_cat {
                current_cat = entry.category.clone();
                items.push(make_item(
                    format!("rules/{}/", current_cat),
                    None,
                    ConfigItemKind::Category,
                ));
            }
            let path = claude_dir
                .join("rules")
                .join(&current_cat)
                .join(&entry.name);
            let item = make_item(
                format!("  {}", entry.name),
                Some(path),
                ConfigItemKind::FileExists,
            );
            accumulate(&item, &mut global_total, &mut global_always);
            items.push(item);
        }
    }

    if !cfg.global_agents.is_empty() {
        let mut current_cat = String::new();
        for entry in &cfg.global_agents {
            if entry.category != current_cat {
                current_cat = entry.category.clone();
                items.push(make_item(
                    format!("agents/{}/", current_cat),
                    None,
                    ConfigItemKind::Category,
                ));
            }
            let path = claude_dir
                .join("agents")
                .join(&current_cat)
                .join(&entry.name);
            let item = make_item(
                format!("  {}", entry.name),
                Some(path),
                ConfigItemKind::FileExists,
            );
            accumulate(&item, &mut global_total, &mut global_always);
            items.push(item);
        }
    }

    items.push(ConfigItem {
        label: format_total("Global", global_total, global_always),
        path: None,
        kind: ConfigItemKind::SectionTotal,
        tokens: Some(global_total),
        always_loaded: None,
    });

    items.push(ConfigItem {
        label: String::new(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
        tokens: None,
        always_loaded: None,
    });

    items.push(ConfigItem {
        label: "Project (.claude/)".to_string(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
        tokens: None,
        always_loaded: None,
    });

    let mut project_total: u32 = 0;
    let mut project_always: u32 = 0;

    let project_md = cwd.join("CLAUDE.md");
    let item = if cfg.project_claude_md {
        make_item(
            "CLAUDE.md".to_string(),
            Some(project_md),
            ConfigItemKind::FileExists,
        )
    } else {
        make_item("CLAUDE.md".to_string(), None, ConfigItemKind::FileMissing)
    };
    accumulate(&item, &mut project_total, &mut project_always);
    items.push(item);

    if !cfg.project_rules.is_empty() {
        let mut current_cat = String::new();
        for entry in &cfg.project_rules {
            if entry.category != current_cat {
                current_cat = entry.category.clone();
                items.push(make_item(
                    format!("rules/{}/", current_cat),
                    None,
                    ConfigItemKind::Category,
                ));
            }
            let path = cwd
                .join(".claude")
                .join("rules")
                .join(&current_cat)
                .join(&entry.name);
            let item = make_item(
                format!("  {}", entry.name),
                Some(path),
                ConfigItemKind::FileExists,
            );
            accumulate(&item, &mut project_total, &mut project_always);
            items.push(item);
        }
    }

    let settings_path = cwd.join(".claude").join("settings.local.json");
    items.push(if cfg.project_settings {
        make_item(
            "settings.local.json".to_string(),
            Some(settings_path),
            ConfigItemKind::FileExists,
        )
    } else {
        make_item(
            "settings.local.json".to_string(),
            None,
            ConfigItemKind::FileMissing,
        )
    });

    if !cfg.project_commands.is_empty() {
        items.push(make_item(
            "commands/".to_string(),
            None,
            ConfigItemKind::Category,
        ));
        for cmd in &cfg.project_commands {
            let path = cwd.join(".claude").join("commands").join(cmd);
            let item = make_item(format!("  {}", cmd), Some(path), ConfigItemKind::FileExists);
            accumulate(&item, &mut project_total, &mut project_always);
            items.push(item);
        }
    }

    items.push(ConfigItem {
        label: format_total("Project", project_total, project_always),
        path: None,
        kind: ConfigItemKind::SectionTotal,
        tokens: Some(project_total),
        always_loaded: None,
    });

    items.push(ConfigItem {
        label: String::new(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
        tokens: None,
        always_loaded: None,
    });

    let mut mem_total: u32 = 0;

    items.push(ConfigItem {
        label: format!("Memory ({})", cfg.project_memories.len()),
        path: None,
        kind: ConfigItemKind::SectionHeader,
        tokens: None,
        always_loaded: None,
    });

    if cfg.project_memories.is_empty() {
        items.push(make_item(
            "  (no memories)".to_string(),
            None,
            ConfigItemKind::FileMissing,
        ));
    } else {
        for mem in &cfg.project_memories {
            let item = make_item(
                mem.clone(),
                Some(memory_dir.join(mem)),
                ConfigItemKind::MemoryFile,
            );
            if let Some(t) = item.tokens {
                mem_total += t;
            }
            items.push(item);
        }
    }

    items.push(ConfigItem {
        label: format!("  ~{}tk total", format_tokens(mem_total)),
        path: None,
        kind: ConfigItemKind::SectionTotal,
        tokens: Some(mem_total),
        always_loaded: None,
    });

    let grand_always = global_always + project_always;
    let grand_total = global_total + project_total + mem_total;
    items.push(ConfigItem {
        label: String::new(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
        tokens: None,
        always_loaded: None,
    });
    items.push(ConfigItem {
        label: format!(
            "Grand total: ~{}tk  (always-loaded: ~{}tk)",
            format_tokens(grand_total),
            format_tokens(grand_always),
        ),
        path: None,
        kind: ConfigItemKind::SectionTotal,
        tokens: Some(grand_total),
        always_loaded: None,
    });

    items
}

fn accumulate(item: &ConfigItem, total: &mut u32, always: &mut u32) {
    if let Some(t) = item.tokens {
        *total += t;
        if item.always_loaded.unwrap_or(true) {
            *always += t;
        }
    }
}

fn format_total(section: &str, total: u32, always: u32) -> String {
    if total == always || always == 0 {
        format!("  {} total: ~{}tk", section, format_tokens(total))
    } else {
        format!(
            "  {} total: ~{}tk (always: ~{}tk)",
            section,
            format_tokens(total),
            format_tokens(always),
        )
    }
}

fn format_tokens(t: u32) -> String {
    if t >= 1000 {
        format!("{:.1}K", t as f64 / 1000.0)
    } else {
        t.to_string()
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SessionConfig {
    pub global_claude_md: bool,
    pub global_rules: Vec<ConfigEntry>,
    pub global_skills: Vec<String>,
    pub global_agents: Vec<ConfigEntry>,
    pub project_claude_md: bool,
    pub project_rules: Vec<ConfigEntry>,
    pub project_settings: bool,
    pub project_commands: Vec<String>,
    pub project_memories: Vec<String>,
}

pub fn scan_session_config(cwd: &Path) -> SessionConfig {
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".claude");

    let global_claude_md = claude_dir.join("CLAUDE.md").is_file();
    let global_rules = scan_categorized_dir(&claude_dir.join("rules"));
    let global_skills = scan_dir_names(&claude_dir.join("skills"));
    let global_agents = scan_categorized_dir(&claude_dir.join("agents"));

    let project_claude_md = cwd.join("CLAUDE.md").is_file();
    let project_rules = scan_categorized_dir(&cwd.join(".claude").join("rules"));
    let project_settings = cwd.join(".claude").join("settings.local.json").is_file();
    let project_commands = scan_flat_files(&cwd.join(".claude").join("commands"));

    let encoded_cwd = cwd.to_string_lossy().replace('/', "-");
    let memory_dir = claude_dir
        .join("projects")
        .join(&encoded_cwd)
        .join("memory");
    let project_memories = scan_flat_files(&memory_dir);

    SessionConfig {
        global_claude_md,
        global_rules,
        global_skills,
        global_agents,
        project_claude_md,
        project_rules,
        project_settings,
        project_commands,
        project_memories,
    }
}

fn scan_categorized_dir(dir: &Path) -> Vec<ConfigEntry> {
    let mut entries = Vec::new();
    let Ok(categories) = std::fs::read_dir(dir) else {
        return entries;
    };

    let mut cats: Vec<_> = categories.flatten().filter(|e| e.path().is_dir()).collect();
    cats.sort_by_key(|e| e.file_name());

    for cat in cats {
        let cat_name = cat.file_name().to_string_lossy().to_string();
        if cat_name.starts_with('.') {
            continue;
        }
        let Ok(files) = std::fs::read_dir(cat.path()) else {
            continue;
        };
        let mut file_names: Vec<String> = files
            .flatten()
            .filter(|f| f.path().is_file())
            .map(|f| f.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.'))
            .collect();
        file_names.sort();
        for name in file_names {
            entries.push(ConfigEntry {
                category: cat_name.clone(),
                name,
            });
        }
    }

    entries
}

fn scan_dir_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

fn scan_flat_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_estimate_file_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(&file, "a".repeat(400)).unwrap();
        let (tokens, always) = estimate_file_tokens(&file);
        assert_eq!(tokens, Some(100));
        assert_eq!(always, Some(true));
    }

    #[test]
    fn test_conditional_detection() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("rule.md");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "paths:").unwrap();
        writeln!(f, "  - src/**/*.rs").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "Some rule content here").unwrap();
        let (tokens, always) = estimate_file_tokens(&file);
        assert!(tokens.is_some());
        assert_eq!(always, Some(false));
    }

    #[test]
    fn test_always_loaded_detection() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("rule.md");
        std::fs::write(&file, "Just plain content without frontmatter").unwrap();
        let (tokens, always) = estimate_file_tokens(&file);
        assert!(tokens.is_some());
        assert_eq!(always, Some(true));
    }
}
