use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRow {
    pub session_id: String,
    pub attached: bool,
    pub current: bool,
}

#[derive(Debug, Default)]
pub struct PaneState {
    pub open: bool,
    pub cursor: usize,
    pub status: Option<String>,
}

impl PaneState {
    pub fn clamp(&mut self, len: usize) {
        if self.cursor >= len {
            self.cursor = len.saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, len: usize) {
        if self.cursor + 1 < len {
            self.cursor += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamePurpose {
    New,
    Fork,
    Rename,
}

impl NamePurpose {
    pub fn title(self) -> &'static str {
        match self {
            Self::New => " New session name ",
            Self::Fork => " Fork session name ",
            Self::Rename => " Rename session ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameInput {
    pub purpose: NamePurpose,
    pub root: PathBuf,
    pub target: Option<String>,
    pub value: String,
    pub pristine: bool,
}

impl NameInput {
    pub fn new(
        purpose: NamePurpose,
        root: PathBuf,
        target: Option<String>,
        suggestion: String,
    ) -> Self {
        Self {
            purpose,
            root,
            target,
            value: suggestion,
            pristine: true,
        }
    }

    pub fn push(&mut self, c: char) {
        if self.pristine {
            self.value.clear();
            self.pristine = false;
        }
        self.value.push(c);
    }

    pub fn pop(&mut self) {
        self.pristine = false;
        self.value.pop();
    }

    pub fn accepted(&self) -> Option<String> {
        let trimmed = self.value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageInput {
    pub target: String,
    pub target_label: String,
    pub value: String,
}

impl MessageInput {
    pub fn push(&mut self, c: char) {
        self.value.push(c);
    }

    pub fn pop(&mut self) {
        self.value.pop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeoverPrompt {
    pub session_id: String,
    pub project_name: String,
    pub root: PathBuf,
    pub label: String,
    pub verified_pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnhideDialog {
    pub root: PathBuf,
    pub cursor: usize,
}

pub fn message_payload(sender: &str, text: &str) -> Vec<u8> {
    format!("[c9s message from {}]: {}\r", sender, text.trim()).into_bytes()
}

pub fn rename_payload(name: &str) -> Vec<u8> {
    format!("/rename {}\r", name.trim()).into_bytes()
}

pub fn coordinate_prompt(peer_name: &str) -> String {
    format!(
        "Coordinate with my other Claude session @{} (same repository): ask what it is working on and whether it overlaps with our task, then summarize its answer for me.",
        peer_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_input_first_key_replaces_suggestion() {
        let mut input = NameInput::new(
            NamePurpose::New,
            PathBuf::from("/p"),
            None,
            "brave-otter".into(),
        );
        assert_eq!(input.accepted().as_deref(), Some("brave-otter"));
        input.push('a');
        assert_eq!(input.value, "a");
        input.push('b');
        assert_eq!(input.value, "ab");
        input.pop();
        assert_eq!(input.value, "a");
        input.pop();
        assert_eq!(input.accepted(), None);
    }

    #[test]
    fn test_name_input_backspace_edits_suggestion() {
        let mut input = NameInput::new(
            NamePurpose::Rename,
            PathBuf::from("/p"),
            Some("s".into()),
            "c9s-c5".into(),
        );
        input.pop();
        assert_eq!(input.value, "c9s-c");
        input.push('9');
        assert_eq!(input.value, "c9s-c9");
    }

    #[test]
    fn test_payloads() {
        assert_eq!(
            message_payload("c9s-c5", " hello there "),
            b"[c9s message from c9s-c5]: hello there\r".to_vec()
        );
        assert_eq!(
            rename_payload("brave-otter"),
            b"/rename brave-otter\r".to_vec()
        );
        assert!(coordinate_prompt("infra-6a").contains("@infra-6a"));
        assert!(!coordinate_prompt("x").ends_with('\r'));
    }

    #[test]
    fn test_pane_cursor_bounds() {
        let mut pane = PaneState::default();
        pane.move_down(3);
        pane.move_down(3);
        pane.move_down(3);
        assert_eq!(pane.cursor, 2);
        pane.clamp(1);
        assert_eq!(pane.cursor, 0);
        pane.move_up();
        assert_eq!(pane.cursor, 0);
    }
}
