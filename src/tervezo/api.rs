use crate::tlog;

use super::config::TervezoConfig;
use super::models::{
    AnalysisResponse, ChangesResponse, CreatePrResponse, FileChange, Implementation, ListResponse,
    PlanResponse, PrDetails, PromptRequest, PromptResponse, RestartResponse, SshCredentials,
    StatusResponse, Step, StepsResponse, SuccessResponse, TestOutputResponse, TestReport,
    TimelineMessage,
};

fn simple_percent_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &str, label: &str) -> Result<T, String> {
    serde_json::from_str(body).map_err(|e| {
        tlog!(error, "{} parse error: {} — body: {}", label, e, body);
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
            url.push_str(&format!("?status={}", simple_percent_encode(status)));
        }

        let resp = self.get(&url)?;
        let list: ListResponse = parse_json(&resp, "list_implementations")?;
        tlog!(info, "parsed {} implementations", list.items.len());
        Ok(list.items)
    }

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

        // Parse the envelope, then deserialize each message individually
        // so one bad message doesn't kill the entire timeline.
        let envelope: serde_json::Value = parse_json(&resp, "get_timeline")?;

        let raw_msgs = envelope
            .get("messages")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut messages = Vec::with_capacity(raw_msgs.len());
        let mut skipped = 0;

        for raw in raw_msgs {
            if raw.is_null() {
                continue;
            }
            match serde_json::from_value::<TimelineMessage>(raw) {
                Ok(msg) => messages.push(msg),
                Err(e) => {
                    skipped += 1;
                    if skipped <= 3 {
                        tlog!(warn, "timeline msg parse skip: {}", e);
                    }
                }
            }
        }

        tlog!(
            info,
            "parsed {} timeline messages (skipped {})",
            messages.len(),
            skipped
        );
        if let Some(first) = messages.first() {
            tlog!(
                info,
                "first parsed msg: type={:?} text='{}'",
                first.msg_type,
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

    pub fn get_test_output(&self, id: &str) -> Result<Vec<TestReport>, String> {
        let url = format!("{}/implementations/{}/test-output", self.base_url, id);
        let resp = self.get(&url)?;
        let tests: TestOutputResponse = parse_json(&resp, "get_test_output")?;

        let mut reports = Vec::with_capacity(tests.test_reports.len());
        let mut skipped = 0;
        for raw in tests.test_reports {
            match serde_json::from_value::<TestReport>(raw) {
                Ok(report) => reports.push(report),
                Err(e) => {
                    skipped += 1;
                    if skipped <= 3 {
                        tlog!(warn, "test report parse skip: {}", e);
                    }
                }
            }
        }

        tlog!(
            info,
            "parsed {} test reports (skipped {})",
            reports.len(),
            skipped
        );
        Ok(reports)
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

    pub fn get_status(&self, id: &str) -> Result<StatusResponse, String> {
        let url = format!("{}/implementations/{}/status", self.base_url, id);
        let resp = self.get(&url)?;
        parse_json(&resp, "get_status")
    }

    pub fn get_pr_details(&self, id: &str) -> Result<PrDetails, String> {
        let url = format!("{}/implementations/{}/pr", self.base_url, id);
        let resp = self.get(&url)?;
        parse_json(&resp, "get_pr_details")
    }

    pub fn create_pr(&self, id: &str) -> Result<CreatePrResponse, String> {
        let url = format!("{}/implementations/{}/pr", self.base_url, id);
        let resp = self.post(&url, "{}")?;
        parse_json(&resp, "create_pr")
    }

    pub fn merge_pr(&self, id: &str) -> Result<SuccessResponse, String> {
        let url = format!("{}/implementations/{}/pr/merge", self.base_url, id);
        let resp = self.post(&url, "{}")?;
        parse_json(&resp, "merge_pr")
    }

    pub fn close_pr(&self, id: &str) -> Result<SuccessResponse, String> {
        let url = format!("{}/implementations/{}/pr/close", self.base_url, id);
        let resp = self.post(&url, "{}")?;
        parse_json(&resp, "close_pr")
    }

    pub fn reopen_pr(&self, id: &str) -> Result<SuccessResponse, String> {
        let url = format!("{}/implementations/{}/pr/reopen", self.base_url, id);
        let resp = self.post(&url, "{}")?;
        parse_json(&resp, "reopen_pr")
    }

    pub fn restart(&self, id: &str) -> Result<RestartResponse, String> {
        let url = format!("{}/implementations/{}/restart", self.base_url, id);
        let resp = self.post(&url, "{}")?;
        parse_json(&resp, "restart")
    }

    pub fn send_prompt(&self, id: &str, message: &str) -> Result<PromptResponse, String> {
        let url = format!("{}/implementations/{}/prompt", self.base_url, id);
        let body = serde_json::to_string(&PromptRequest {
            message: message.to_string(),
        })
        .map_err(|e| format!("serialize prompt failed: {}", e))?;
        let resp = self.post(&url, &body)?;
        parse_json(&resp, "send_prompt")
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
            tlog!(error, "HTTP {}: {}", status, body);
            return Err(format!("HTTP {}: {}", status, body));
        }

        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("read body failed: {}", e))?;

        tlog!(info, "response body: {} bytes", body.len());

        Ok(body)
    }

    fn post(&self, url: &str, json_body: &str) -> Result<String, String> {
        tlog!(info, "POST {}", url);
        let resp = self
            .agent
            .post(url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("User-Agent", "c9s/0.1")
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .send(json_body.as_bytes())
            .map_err(|e| {
                tlog!(error, "POST request error: {}", e);
                format!("POST request failed: {}", e)
            })?;

        let status = resp.status();
        tlog!(info, "POST response: HTTP {}", status);

        if status != 200 && status != 201 {
            let body = resp
                .into_body()
                .read_to_string()
                .unwrap_or_else(|_| "(unreadable body)".to_string());
            tlog!(error, "POST HTTP {}: {}", status, body);
            return Err(format!("HTTP {}: {}", status, body));
        }

        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("read body failed: {}", e))?;

        tlog!(info, "POST response body: {} bytes", body.len());

        Ok(body)
    }
}
