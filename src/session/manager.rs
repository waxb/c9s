use std::process::Command;

pub struct SessionManager;

impl SessionManager {
    pub fn is_claude_installed() -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
