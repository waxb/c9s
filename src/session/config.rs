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
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigItemKind {
    SectionHeader,
    Category,
    FileExists,
    FileMissing,
    MemoryFile,
}

pub fn build_config_items(cfg: &SessionConfig, cwd: &Path) -> Vec<ConfigItem> {
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".claude");
    let encoded_cwd = cwd.to_string_lossy().replace('/', "-");
    let memory_dir = claude_dir.join("projects").join(&encoded_cwd).join("memory");

    let mut items = Vec::new();

    items.push(ConfigItem {
        label: "Global (~/.claude/)".to_string(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
    });

    let global_md = claude_dir.join("CLAUDE.md");
    items.push(ConfigItem {
        label: "CLAUDE.md".to_string(),
        path: if cfg.global_claude_md { Some(global_md) } else { None },
        kind: if cfg.global_claude_md { ConfigItemKind::FileExists } else { ConfigItemKind::FileMissing },
    });

    if cfg.global_rules.is_empty() {
        items.push(ConfigItem {
            label: "rules/  [empty]".to_string(),
            path: None,
            kind: ConfigItemKind::FileMissing,
        });
    } else {
        let mut current_cat = String::new();
        for entry in &cfg.global_rules {
            if entry.category != current_cat {
                current_cat = entry.category.clone();
                items.push(ConfigItem {
                    label: format!("rules/{}/", current_cat),
                    path: None,
                    kind: ConfigItemKind::Category,
                });
            }
            items.push(ConfigItem {
                label: format!("  {}", entry.name),
                path: Some(claude_dir.join("rules").join(&current_cat).join(&entry.name)),
                kind: ConfigItemKind::FileExists,
            });
        }
    }

    if !cfg.global_agents.is_empty() {
        let mut current_cat = String::new();
        for entry in &cfg.global_agents {
            if entry.category != current_cat {
                current_cat = entry.category.clone();
                items.push(ConfigItem {
                    label: format!("agents/{}/", current_cat),
                    path: None,
                    kind: ConfigItemKind::Category,
                });
            }
            items.push(ConfigItem {
                label: format!("  {}", entry.name),
                path: Some(claude_dir.join("agents").join(&current_cat).join(&entry.name)),
                kind: ConfigItemKind::FileExists,
            });
        }
    }

    items.push(ConfigItem {
        label: String::new(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
    });

    items.push(ConfigItem {
        label: "Project (.claude/)".to_string(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
    });

    let project_md = cwd.join("CLAUDE.md");
    items.push(ConfigItem {
        label: "CLAUDE.md".to_string(),
        path: if cfg.project_claude_md { Some(project_md) } else { None },
        kind: if cfg.project_claude_md { ConfigItemKind::FileExists } else { ConfigItemKind::FileMissing },
    });

    if !cfg.project_rules.is_empty() {
        let mut current_cat = String::new();
        for entry in &cfg.project_rules {
            if entry.category != current_cat {
                current_cat = entry.category.clone();
                items.push(ConfigItem {
                    label: format!("rules/{}/", current_cat),
                    path: None,
                    kind: ConfigItemKind::Category,
                });
            }
            items.push(ConfigItem {
                label: format!("  {}", entry.name),
                path: Some(cwd.join(".claude").join("rules").join(&current_cat).join(&entry.name)),
                kind: ConfigItemKind::FileExists,
            });
        }
    }

    let settings_path = cwd.join(".claude").join("settings.local.json");
    items.push(ConfigItem {
        label: "settings.local.json".to_string(),
        path: if cfg.project_settings { Some(settings_path) } else { None },
        kind: if cfg.project_settings { ConfigItemKind::FileExists } else { ConfigItemKind::FileMissing },
    });

    if !cfg.project_commands.is_empty() {
        items.push(ConfigItem {
            label: "commands/".to_string(),
            path: None,
            kind: ConfigItemKind::Category,
        });
        for cmd in &cfg.project_commands {
            items.push(ConfigItem {
                label: format!("  {}", cmd),
                path: Some(cwd.join(".claude").join("commands").join(cmd)),
                kind: ConfigItemKind::FileExists,
            });
        }
    }

    items.push(ConfigItem {
        label: String::new(),
        path: None,
        kind: ConfigItemKind::SectionHeader,
    });

    items.push(ConfigItem {
        label: format!("Memory ({})", cfg.project_memories.len()),
        path: None,
        kind: ConfigItemKind::SectionHeader,
    });

    if cfg.project_memories.is_empty() {
        items.push(ConfigItem {
            label: "  (no memories)".to_string(),
            path: None,
            kind: ConfigItemKind::FileMissing,
        });
    } else {
        for mem in &cfg.project_memories {
            items.push(ConfigItem {
                label: mem.clone(),
                path: Some(memory_dir.join(mem)),
                kind: ConfigItemKind::MemoryFile,
            });
        }
    }

    items
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
    let memory_dir = claude_dir.join("projects").join(&encoded_cwd).join("memory");
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

    let mut cats: Vec<_> = categories
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();
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
