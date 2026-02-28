use crate::tlog;

use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::config::TervezoConfig;
use super::models::TimelineMessage;

const MAX_BACKOFF_SECS: u64 = 30;
const SSE_TIMEOUT_SECS: u64 = 120;

#[allow(dead_code)]
pub enum SseMessage {
    Event(Box<TimelineMessage>),
    Error(String),
}

pub struct SseStream {
    stop: Arc<AtomicBool>,
    _handle: JoinHandle<()>,
}

impl SseStream {
    pub fn connect(
        config: &TervezoConfig,
        implementation_id: &str,
        last_cursor: Option<String>,
        tx: mpsc::Sender<SseMessage>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);

        let base_url = config.base_url.trim_end_matches('/').to_string();
        let api_key = config.api_key.clone();
        let impl_id = implementation_id.to_string();

        let handle = std::thread::spawn(move || {
            Self::stream_loop(base_url, api_key, impl_id, last_cursor, stop_clone, tx);
        });

        Self {
            stop,
            _handle: handle,
        }
    }

    fn stream_loop(
        base_url: String,
        api_key: String,
        impl_id: String,
        initial_cursor: Option<String>,
        stop: Arc<AtomicBool>,
        tx: mpsc::Sender<SseMessage>,
    ) {
        let mut cursor = initial_cursor;
        let mut backoff_secs = 1u64;

        loop {
            if stop.load(Ordering::Relaxed) {
                return;
            }

            let mut url = format!("{}/implementations/{}/stream", base_url, impl_id);
            if let Some(ref c) = cursor {
                url.push_str(&format!("?after={}", c));
            }

            tlog!(info, "SSE connecting: {}", url);
            match Self::open_sse(&url, &api_key) {
                Ok(reader) => {
                    tlog!(info, "SSE connected, reading events...");
                    backoff_secs = 1;
                    Self::read_events(reader, &stop, &tx, &mut cursor);
                    tlog!(info, "SSE stream ended, will reconnect");
                }
                Err(e) => {
                    tlog!(error, "SSE error: {}", e);
                    let _ = tx.send(SseMessage::Error(e));
                }
            }

            if stop.load(Ordering::Relaxed) {
                return;
            }

            for _ in 0..(backoff_secs * 10) {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
        }
    }

    fn open_sse(url: &str, api_key: &str) -> Result<Box<dyn BufRead + Send>, String> {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(SSE_TIMEOUT_SECS)))
            .build()
            .new_agent();

        let resp = agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", api_key))
            .header("Accept", "text/event-stream")
            .header("User-Agent", "c9s/0.1")
            .call()
            .map_err(|e| format!("SSE connect failed: {}", e))?;

        let reader = resp.into_body().into_reader();
        Ok(Box::new(std::io::BufReader::new(reader)))
    }

    fn read_events(
        reader: Box<dyn BufRead + Send>,
        stop: &Arc<AtomicBool>,
        tx: &mpsc::Sender<SseMessage>,
        cursor: &mut Option<String>,
    ) {
        let mut data_buf = String::new();
        let mut event_id: Option<String> = None;

        for line_result in reader.lines() {
            if stop.load(Ordering::Relaxed) {
                return;
            }

            let line = match line_result {
                Ok(l) => l,
                Err(_) => return,
            };

            if line.is_empty() {
                if !data_buf.is_empty() {
                    tlog!(
                        info,
                        "SSE raw data: {}",
                        &data_buf[..300.min(data_buf.len())]
                    );
                    // SSE events are envelopes: {"messages":[...]}, {"plan":"..."}, etc.
                    // Extract timeline messages from the "messages" array.
                    match serde_json::from_str::<serde_json::Value>(&data_buf) {
                        Ok(envelope) => {
                            if let Some(msgs) = envelope.get("messages").and_then(|v| v.as_array())
                            {
                                for raw_msg in msgs {
                                    if raw_msg.is_null() {
                                        continue;
                                    }
                                    match serde_json::from_value::<TimelineMessage>(raw_msg.clone())
                                    {
                                        Ok(msg) => {
                                            let dt = msg.display_text();
                                            if dt.is_empty() {
                                                // Log full raw JSON for messages with no display text
                                                let raw_str = serde_json::to_string(raw_msg)
                                                    .unwrap_or_default();
                                                tlog!(
                                                    info,
                                                    "SSE msg (no text): type={:?} raw={}",
                                                    msg.msg_type,
                                                    &raw_str[..300.min(raw_str.len())]
                                                );
                                            } else {
                                                tlog!(
                                                    info,
                                                    "SSE msg: type={:?} text={}",
                                                    msg.msg_type,
                                                    &dt[..100.min(dt.len())]
                                                );
                                            }
                                            if let Some(ref id) = msg.id {
                                                *cursor = Some(id.clone());
                                            }
                                            let _ = tx.send(SseMessage::Event(Box::new(msg)));
                                        }
                                        Err(e) => {
                                            tlog!(warn, "SSE msg parse failed: {}", e);
                                        }
                                    }
                                }
                            } else {
                                // Non-message envelope (plan update, etc.) — log and skip
                                let keys = envelope
                                    .as_object()
                                    .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
                                    .unwrap_or_default();
                                tlog!(info, "SSE non-message envelope: keys=[{}]", keys);
                            }
                            // Update cursor from event id if no message had one
                            if let Some(ref eid) = event_id {
                                if cursor.is_none() {
                                    *cursor = Some(eid.clone());
                                }
                            }
                        }
                        Err(e) => {
                            tlog!(
                                warn,
                                "SSE JSON parse failed: {} — raw: {}",
                                e,
                                &data_buf[..200.min(data_buf.len())]
                            );
                        }
                    }
                    data_buf.clear();
                    event_id = None;
                }
            } else if let Some(data) = line.strip_prefix("data: ") {
                data_buf.push_str(data);
            } else if let Some(id) = line.strip_prefix("id: ") {
                event_id = Some(id.to_string());
            }
        }
    }
}

impl Drop for SseStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
