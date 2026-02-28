use crate::tlog;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::api::TervezoClient;
use super::config::TervezoConfig;
use super::models::Implementation;

#[derive(Debug, Default)]
struct FetcherState {
    implementations: Vec<Implementation>,
    error: Option<String>,
    dirty: bool,
}

pub struct TervezoFetcher {
    state: Arc<Mutex<FetcherState>>,
    stop: Arc<AtomicBool>,
    _handle: JoinHandle<()>,
}

impl TervezoFetcher {
    pub fn spawn(config: &TervezoConfig) -> Self {
        let state = Arc::new(Mutex::new(FetcherState::default()));
        let stop = Arc::new(AtomicBool::new(false));

        let client = TervezoClient::new(config);
        let poll_interval = config.poll_interval;
        let state_clone = Arc::clone(&state);
        let stop_clone = Arc::clone(&stop);

        let handle = std::thread::spawn(move || {
            Self::poll_loop(client, poll_interval, state_clone, stop_clone);
        });

        Self {
            state,
            stop,
            _handle: handle,
        }
    }

    fn poll_loop(
        client: TervezoClient,
        interval_secs: u64,
        state: Arc<Mutex<FetcherState>>,
        stop: Arc<AtomicBool>,
    ) {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            tlog!(info, "fetching implementations...");
            match client.list_implementations(None) {
                Ok(impls) => {
                    tlog!(info, "fetched {} implementations", impls.len());
                    let mut s = state.lock().unwrap();
                    s.implementations = impls;
                    s.error = None;
                    s.dirty = true;
                }
                Err(e) => {
                    tlog!(error, "fetch error: {}", e);
                    let mut s = state.lock().unwrap();
                    s.error = Some(e);
                    s.dirty = true;
                }
            }

            for _ in 0..(interval_secs * 10) {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    pub fn take_dirty(&self) -> bool {
        let mut s = self.state.lock().unwrap();
        let was_dirty = s.dirty;
        s.dirty = false;
        was_dirty
    }

    pub fn implementations(&self) -> Vec<Implementation> {
        self.state.lock().unwrap().implementations.clone()
    }

    #[allow(dead_code)]
    pub fn error(&self) -> Option<String> {
        self.state.lock().unwrap().error.clone()
    }
}

impl Drop for TervezoFetcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
