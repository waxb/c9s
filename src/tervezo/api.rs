use crate::tlog;

use super::config::TervezoConfig;
use super::models::{
    AnalysisResponse, ChangesResponse, FileChange, Implementation, ListResponse, PlanResponse,
    SshCredentials, Step, StepsResponse, TestOutputResponse, TimelineMessage, TimelineResponse,
};

/// Helper to parse JSON with detailed logging on failure.
fn parse_json<T: serde::de::DeserializeOwned>(body: &str, label: &str) -> Result<T, String> {
    serde_json::from_str(body).map_err(|e| {
        tlog!(
            error,
            "{} parse error: {} — body: {}",
            label,
            e,
            &body[..500.min(body.len())]
        );
        format!("{} parse error: {}", label, e)
    })
}

const REQUEST_TIMEOUT_SECS: u64 = 10;

pub struct TervezoClient {
    agent: ureq::Agent,
    base_url: String,
    api_key: String,
}

impl TervezoClient {
    pub fn new(config: &TervezoConfig) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS)))
            .http_status_as_error(false)
            .build()
            .new_agent();

        Self {
            agent,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        }
    }

    pub fn list_implementations(
        &self,
        status_filter: Option<&str>,
    ) -> Result<Vec<Implementation>, String> {
        let mut url = format!("{}/implementations", self.base_url);
        if let Some(status) = status_filter {
            url.push_str(&format!("?status={}", status));
        }

        let resp = self.get(&url)?;
        let list: ListResponse = parse_json(&resp, "list_implementations")?;
        tlog!(info, "parsed {} implementations", list.items.len());
        Ok(list.items)
    }

    #[allow(dead_code)]
    pub fn get_implementation(&self, id: &str) -> Result<Implementation, String> {
        let url = format!("{}/implementations/{}", self.base_url, id);
        let resp = self.get(&url)?;
        parse_json(&resp, "get_implementation")
    }

    pub fn get_timeline(
        &self,
        id: &str,
        after_cursor: Option<&str>,
    ) -> Result<Vec<TimelineMessage>, String> {
        let mut url = format!("{}/implementations/{}/timeline", self.base_url, id);
        if let Some(cursor) = after_cursor {
            url.push_str(&format!("?after={}", cursor));
        }

        let resp = self.get(&url)?;

        // Log raw JSON structure of first message for debugging
        if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&resp) {
            if let Some(first) = raw
                .get("messages")
                .and_then(|m| m.as_array())
                .and_then(|a| a.first())
            {
                tlog!(
                    info,
                    "timeline first msg keys: {}",
                    first
                        .as_object()
                        .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
                        .unwrap_or_default()
                );
                let sample = serde_json::to_string(first).unwrap_or_default();
                tlog!(
                    info,
                    "timeline first msg: {}",
                    &sample[..500.min(sample.len())]
                );
            }
        }

        let timeline: TimelineResponse = parse_json(&resp, "get_timeline")?;
        let messages: Vec<TimelineMessage> = timeline.messages.into_iter().flatten().collect();
        tlog!(info, "parsed {} timeline messages", messages.len());
        if let Some(first) = messages.first() {
            tlog!(
                info,
                "first parsed msg: type={:?} status={:?} message={:?} content={:?} text='{}'",
                first.msg_type,
                first.status,
                first.message,
                first.content,
                first.display_text()
            );
        }
        Ok(messages)
    }

    pub fn get_plan(&self, id: &str) -> Result<String, String> {
        let url = format!("{}/implementations/{}/plan", self.base_url, id);
        let resp = self.get(&url)?;
        let plan: PlanResponse = parse_json(&resp, "get_plan")?;
        tlog!(info, "parsed plan: {} chars", plan.plan.len());
        Ok(plan.plan)
    }

    pub fn get_analysis(&self, id: &str) -> Result<String, String> {
        let url = format!("{}/implementations/{}/analysis", self.base_url, id);
        let resp = self.get(&url)?;
        let analysis: AnalysisResponse = parse_json(&resp, "get_analysis")?;
        tlog!(info, "parsed analysis: {} chars", analysis.analysis.len());
        Ok(analysis.analysis)
    }

    pub fn get_changes(&self, id: &str) -> Result<Vec<FileChange>, String> {
        let url = format!("{}/implementations/{}/changes", self.base_url, id);
        let resp = self.get(&url)?;
        let changes: ChangesResponse = parse_json(&resp, "get_changes")?;
        let files: Vec<FileChange> = changes.files.into_iter().flatten().collect();
        tlog!(info, "parsed {} file changes", files.len());
        Ok(files)
    }

    pub fn get_test_output(&self, id: &str) -> Result<String, String> {
        let url = format!("{}/implementations/{}/test-output", self.base_url, id);
        let resp = self.get(&url)?;
        let tests: TestOutputResponse = parse_json(&resp, "get_test_output")?;
        // Render test reports as pretty JSON since the schema is loosely typed
        let output = serde_json::to_string_pretty(&tests.test_reports)
            .unwrap_or_else(|_| "(no test data)".to_string());
        tlog!(info, "parsed {} test reports", tests.test_reports.len());
        Ok(output)
    }

    pub fn get_ssh(&self, id: &str) -> Result<SshCredentials, String> {
        let url = format!("{}/implementations/{}/ssh", self.base_url, id);
        let resp = self.get(&url)?;
        let creds: SshCredentials = parse_json(&resp, "get_ssh")?;
        tlog!(info, "parsed SSH creds: host={}", creds.host);
        Ok(creds)
    }

    #[allow(dead_code)]
    pub fn get_steps(&self, id: &str) -> Result<Vec<Step>, String> {
        let url = format!("{}/implementations/{}/steps", self.base_url, id);
        let resp = self.get(&url)?;
        let steps: StepsResponse = parse_json(&resp, "get_steps")?;
        tlog!(info, "parsed {} steps", steps.steps.len());
        Ok(steps.steps)
    }

    fn get(&self, url: &str) -> Result<String, String> {
        tlog!(info, "GET {}", url);
        let resp = self
            .agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("User-Agent", "c9s/0.1")
            .header("Accept", "application/json")
            .call()
            .map_err(|e| {
                tlog!(error, "request error: {}", e);
                format!("request failed: {}", e)
            })?;

        let status = resp.status();
        tlog!(info, "response: HTTP {}", status);

        if status != 200 {
            let body = resp
                .into_body()
                .read_to_string()
                .unwrap_or_else(|_| "(unreadable body)".to_string());
            tlog!(error, "HTTP {}: {}", status, &body[..200.min(body.len())]);
            return Err(format!("HTTP {}: {}", status, &body[..200.min(body.len())]));
        }

        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("read body failed: {}", e))?;

        tlog!(info, "body: {}...", &body[..200.min(body.len())]);

        Ok(body)
    }
}
