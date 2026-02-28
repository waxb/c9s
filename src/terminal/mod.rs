mod manager;
mod notifier;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

pub use manager::{TabEntry, TerminalManager};

pub struct EmbeddedTerminal {
    session_id: String,
    project_name: String,
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty>,
    _reader_handle: JoinHandle<()>,
    exited: Arc<AtomicBool>,
    bell: Arc<AtomicBool>,
    bell_blink: Arc<AtomicBool>,
    dirty: Arc<AtomicBool>,
}

impl EmbeddedTerminal {
    pub fn spawn_resume(
        session_id: &str,
        project_name: &str,
        cwd: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        Self::spawn_inner(
            session_id,
            project_name,
            "claude",
            &["--resume", session_id],
            cwd,
            rows,
            cols,
        )
    }

    pub fn spawn_ssh(
        session_id: &str,
        project_name: &str,
        ssh_command: &str,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        Self::spawn_inner(
            session_id,
            project_name,
            "bash",
            &["-c", ssh_command],
            Path::new("/tmp"),
            rows,
            cols,
        )
    }

    pub fn spawn_new(cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        let project_name = cwd
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.to_string_lossy().to_string());

        let id = uuid::Uuid::new_v4().to_string();

        Self::spawn_inner(&id, &project_name, "claude", &[], cwd, rows, cols)
    }

    fn spawn_inner(
        session_id: &str,
        project_name: &str,
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

        let inner_cmd = if args.is_empty() {
            cmd.to_string()
        } else {
            let escaped_args: Vec<String> = args
                .iter()
                .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
                .collect();
            format!("{} {}", cmd, escaped_args.join(" "))
        };
        let mut cmd_builder = CommandBuilder::new("bash");
        cmd_builder.arg("-c");
        cmd_builder.arg(format!("export GPG_TTY=$(tty); exec {}", inner_cmd));
        cmd_builder.cwd(cwd);

        let _child = pair
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

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 10000)));
        let exited = Arc::new(AtomicBool::new(false));
        let bell = Arc::new(AtomicBool::new(false));
        let bell_blink = Arc::new(AtomicBool::new(false));
        let dirty = Arc::new(AtomicBool::new(true));

        let parser_clone = Arc::clone(&parser);
        let exited_clone = Arc::clone(&exited);
        let dirty_clone = Arc::clone(&dirty);

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        exited_clone.store(true, Ordering::Relaxed);
                        dirty_clone.store(true, Ordering::Relaxed);
                        break;
                    }
                    Ok(n) => {
                        parser_clone.lock().unwrap().process(&buf[..n]);
                        dirty_clone.store(true, Ordering::Relaxed);
                    }
                    Err(_) => {
                        exited_clone.store(true, Ordering::Relaxed);
                        dirty_clone.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            session_id: session_id.to_string(),
            project_name: project_name.to_string(),
            parser,
            writer,
            master: pair.master,
            _reader_handle: reader_handle,
            exited,
            bell,
            bell_blink,
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
        self.bell.store(false, Ordering::Relaxed);
        self.bell_blink.store(false, Ordering::Relaxed);
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

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Relaxed)
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

    pub fn clear_bell(&self) {
        self.bell.store(false, Ordering::Relaxed);
        self.bell_blink.store(false, Ordering::Relaxed);
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn project_name(&self) -> &str {
        &self.project_name
    }
}
