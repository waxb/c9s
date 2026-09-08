mod manager;
mod notifier;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::session::registry::process_alive;

pub use manager::{kill_process, TabEntry, TerminalManager};

const SCROLLBACK_LINES: usize = 10_000;
const READ_BUF_SIZE: usize = 8192;
const TERM_GRACE: Duration = Duration::from_millis(300);

pub struct EmbeddedTerminal {
    session_id: String,
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    _reader_handle: JoinHandle<()>,
    exited: Arc<AtomicBool>,
    bell: Arc<AtomicBool>,
    bell_blink: Arc<AtomicBool>,
    dirty: Arc<AtomicBool>,
}

pub(crate) fn project_name_for(cwd: &Path) -> String {
    cwd.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| cwd.to_string_lossy().to_string())
}

impl EmbeddedTerminal {
    pub fn spawn_resume(session_id: &str, cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        Self::spawn_inner(
            session_id,
            "claude",
            &["--resume", session_id],
            cwd,
            rows,
            cols,
        )
    }

    pub(crate) fn spawn_shell(cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        Self::spawn_inner(&id, "bash", &["--login"], cwd, rows, cols)
    }

    pub fn spawn_resume_picker(cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        Self::spawn_inner(&id, "claude", &["--resume"], cwd, rows, cols)
    }

    pub fn spawn_new(cwd: &Path, name: &str, rows: u16, cols: u16) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        Self::spawn_inner(
            &id,
            "claude",
            &["--session-id", &id, "-n", name],
            cwd,
            rows,
            cols,
        )
    }

    pub fn spawn_fork(
        cwd: &Path,
        source_id: &str,
        name: &str,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        Self::spawn_inner(
            &id,
            "claude",
            &[
                "--session-id",
                &id,
                "--fork-session",
                "--resume",
                source_id,
                "-n",
                name,
            ],
            cwd,
            rows,
            cols,
        )
    }

    #[cfg(test)]
    pub(crate) fn spawn_for_test(session_id: &str, cmd: &str, args: &[&str], cwd: &Path) -> Self {
        Self::spawn_inner(session_id, cmd, args, cwd, 5, 20).expect("spawn")
    }

    fn spawn_inner(
        session_id: &str,
        cmd: &str,
        args: &[&str],
        cwd: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut cmd_builder = CommandBuilder::new("bash");
        cmd_builder.arg("-c");
        cmd_builder.arg(format!(
            "export GPG_TTY=$(tty); exec {}",
            shell_command(cmd, args)
        ));
        cmd_builder.cwd(cwd);

        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK_LINES)));
        let exited = Arc::new(AtomicBool::new(false));
        let dirty = Arc::new(AtomicBool::new(true));

        let parser_clone = Arc::clone(&parser);
        let exited_clone = Arc::clone(&exited);
        let dirty_clone = Arc::clone(&dirty);

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; READ_BUF_SIZE];
            loop {
                match reader.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        parser_clone.lock().unwrap().process(&buf[..n]);
                        dirty_clone.store(true, Ordering::Relaxed);
                    }
                    _ => {
                        exited_clone.store(true, Ordering::Relaxed);
                        dirty_clone.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            session_id: session_id.to_string(),
            parser,
            writer,
            master: pair.master,
            child: Mutex::new(child),
            _reader_handle: reader_handle,
            exited,
            bell: Arc::new(AtomicBool::new(false)),
            bell_blink: Arc::new(AtomicBool::new(false)),
            dirty,
        })
    }

    pub fn lock_parser(&self) -> MutexGuard<'_, vt100::Parser> {
        self.parser.lock().unwrap()
    }

    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.clear_bell();
        self.parser.lock().unwrap().screen_mut().set_scrollback(0);
        self.dirty.store(true, Ordering::Relaxed);
        self.writer
            .write_all(bytes)
            .context("failed to write to PTY")?;
        self.writer.flush().context("failed to flush PTY")?;
        Ok(())
    }

    pub fn scroll_up(&self, lines: usize) {
        let mut parser = self.parser.lock().unwrap();
        let current = parser.screen().scrollback();
        parser.screen_mut().set_scrollback(current + lines);
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn scroll_down(&self, lines: usize) {
        let mut parser = self.parser.lock().unwrap();
        let current = parser.screen().scrollback();
        parser
            .screen_mut()
            .set_scrollback(current.saturating_sub(lines));
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        self.parser
            .lock()
            .unwrap()
            .screen_mut()
            .set_size(rows, cols);
        self.dirty.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|c| c.process_id())
    }

    fn reap(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.try_wait();
        }
    }

    pub fn is_exited(&self) -> bool {
        if !self.exited.load(Ordering::Relaxed) {
            return false;
        }
        self.reap();
        true
    }

    pub fn terminate(&self) {
        if let Some(pid) = self.pid().filter(|&pid| process_alive(pid)) {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
            let start = Instant::now();
            while process_alive(pid) && start.elapsed() < TERM_GRACE {
                std::thread::sleep(Duration::from_millis(25));
            }
            if process_alive(pid) {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }
        self.reap();
    }

    pub fn has_bell(&self) -> bool {
        self.bell.load(Ordering::Relaxed)
    }

    pub fn has_bell_blink(&self) -> bool {
        self.bell_blink.load(Ordering::Relaxed)
    }

    pub fn set_bell(&self) {
        self.bell.store(true, Ordering::Relaxed);
        self.bell_blink.store(true, Ordering::Relaxed);
    }

    pub fn clear_bell_blink(&self) {
        self.bell_blink.store(false, Ordering::Relaxed);
    }

    pub fn clear_bell(&self) {
        self.bell.store(false, Ordering::Relaxed);
        self.bell_blink.store(false, Ordering::Relaxed);
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn set_session_id(&mut self, session_id: &str) {
        self.session_id = session_id.to_string();
    }
}

impl Drop for EmbeddedTerminal {
    fn drop(&mut self) {
        if !self.exited.load(Ordering::Relaxed) {
            self.terminate();
        }
    }
}

fn shell_command(cmd: &str, args: &[&str]) -> String {
    if args.is_empty() {
        return cmd.to_string();
    }
    let escaped: Vec<String> = args
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
        .collect();
    format!("{} {}", cmd, escaped.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_command_quotes_arguments() {
        assert_eq!(shell_command("claude", &[]), "claude");
        assert_eq!(
            shell_command("claude", &["--resume", "abc"]),
            "claude '--resume' 'abc'"
        );
        assert_eq!(shell_command("x", &["it's"]), "x 'it'\\''s'");
    }

    #[test]
    fn test_project_name_for() {
        assert_eq!(project_name_for(Path::new("/a/b/c9s")), "c9s");
        assert_eq!(project_name_for(Path::new("/")), "/");
    }

    #[test]
    fn test_spawned_shell_reports_pid_and_exit() {
        let dir = tempfile::tempdir().unwrap();
        let term =
            EmbeddedTerminal::spawn_inner("t", "true", &[], dir.path(), 5, 20).expect("spawn");
        let start = Instant::now();
        while !term.is_exited() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(term.is_exited());
    }

    #[test]
    fn test_terminate_kills_long_running_child() {
        let dir = tempfile::tempdir().unwrap();
        let term =
            EmbeddedTerminal::spawn_inner("t", "sleep", &["30"], dir.path(), 5, 20).expect("spawn");
        std::thread::sleep(Duration::from_millis(100));
        let pid = term.pid().expect("pid");
        assert!(process_alive(pid));
        term.terminate();
        assert!(!process_alive(pid));
    }
}
