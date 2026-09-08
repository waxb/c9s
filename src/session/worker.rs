use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use super::{Session, SessionDiscovery};
use crate::store::Store;
use crate::tlog;

pub struct DiscoveryWorker {
    request_tx: mpsc::Sender<()>,
    result_rx: mpsc::Receiver<Vec<Session>>,
    _handle: JoinHandle<()>,
}

impl DiscoveryWorker {
    pub fn spawn(interval: Duration) -> Self {
        Self::spawn_with(SessionDiscovery::new(), interval, true)
    }

    fn spawn_with(mut discovery: SessionDiscovery, interval: Duration, checkpoint: bool) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<Vec<Session>>();

        let handle = std::thread::spawn(move || {
            let store = checkpoint.then(open_checkpoint_store).flatten();
            if let Some(store) = &store {
                match store.load_transcript_stats() {
                    Ok(entries) => discovery.seed_cache(entries),
                    Err(e) => tlog!(warn, "transcript checkpoint load failed: {}", e),
                }
            }
            loop {
                let sessions = discovery.discover_all().unwrap_or_default();
                if let Some(store) = &store {
                    let dirty = discovery.take_dirty_cache();
                    if !dirty.is_empty() {
                        if let Err(e) = store.save_transcript_stats(&dirty) {
                            tlog!(warn, "transcript checkpoint save failed: {}", e);
                        }
                    }
                }
                if result_tx.send(sessions).is_err() {
                    break;
                }
                match request_rx.recv_timeout(interval) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                while request_rx.try_recv().is_ok() {}
            }
        });

        Self {
            request_tx,
            result_rx,
            _handle: handle,
        }
    }

    pub fn request_refresh(&self) {
        let _ = self.request_tx.send(());
    }

    pub fn wait_first(&self, timeout: Duration) -> Option<Vec<Session>> {
        self.result_rx.recv_timeout(timeout).ok()
    }

    pub fn take_latest(&self) -> Option<Vec<Session>> {
        let mut latest = None;
        while let Ok(sessions) = self.result_rx.try_recv() {
            latest = Some(sessions);
        }
        latest
    }
}

fn open_checkpoint_store() -> Option<Store> {
    match Store::open() {
        Ok(store) => Some(store),
        Err(e) => {
            tlog!(warn, "transcript checkpoint unavailable: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_delivers_initial_and_requested_scans() {
        let home = tempfile::tempdir().unwrap();
        let project_dir = home.path().join("projects").join("-tmp-w");
        std::fs::create_dir_all(&project_dir).unwrap();
        let sessions_dir = home.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let discovery = SessionDiscovery::with_dirs(home.path(), &sessions_dir);
        let worker = DiscoveryWorker::spawn_with(discovery, Duration::from_secs(3600), false);

        let first = worker
            .wait_first(Duration::from_secs(10))
            .expect("initial scan");
        assert!(first.is_empty());

        std::fs::write(
            project_dir.join("w1.jsonl"),
            "{\"sessionId\":\"w1\",\"cwd\":\"/tmp/w\",\"type\":\"user\",\"timestamp\":\"2026-01-01T00:00:00Z\"}\n",
        )
        .unwrap();
        worker.request_refresh();
        let second = worker
            .wait_first(Duration::from_secs(10))
            .expect("requested scan");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].id, "w1");
        assert!(worker.take_latest().is_none());
    }
}
