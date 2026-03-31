use anyhow::{bail, Context, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::io::Read;
use std::sync::mpsc;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct LinearIssue {
    pub identifier: String,
    pub title: String,
    pub description: String,
    pub git_branch_name: Option<String>,
    pub status: String,
    pub team: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LinearEvent {
    pub issue_id: String,
    pub prompt_context: Option<String>,
    pub comment: Option<String>,
}

pub struct LinearClient {
    api_key: String,
}

impl LinearClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
        }
    }

    pub fn fetch_issue(&self, identifier: &str) -> Result<LinearIssue> {
        let query = format!(
            r#"{{ "query": "query {{ issue(id: \"{}\") {{ identifier title description branchName status {{ name }} team {{ name }} labels {{ nodes {{ name }} }} }} }}" }}"#,
            identifier
        );

        let response = ureq::post("https://api.linear.app/graphql")
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .send(query.as_bytes())
            .context("failed to call Linear API")?;

        let body = response.into_body().read_to_string()
            .context("failed to read Linear response")?;

        parse_issue_response(&body)
    }

    pub fn update_issue_status(&self, identifier: &str, status_name: &str) -> Result<()> {
        let query = format!(
            r#"{{ "query": "mutation {{ issueUpdate(id: \"{}\", input: {{ stateId: \"{}\" }}) {{ success }} }}" }}"#,
            identifier, status_name
        );

        let _response = ureq::post("https://api.linear.app/graphql")
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .send(query.as_bytes())
            .context("failed to update Linear issue status")?;

        Ok(())
    }
}

pub fn parse_issue_response(json: &str) -> Result<LinearIssue> {
    let value: serde_json::Value =
        serde_json::from_str(json).context("failed to parse Linear API response JSON")?;

    let issue = value
        .get("data")
        .and_then(|d| d.get("issue"))
        .ok_or_else(|| anyhow::anyhow!("missing data.issue in Linear response"))?;

    let identifier = issue
        .get("identifier")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing identifier"))?
        .to_string();

    let title = issue
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let description = issue
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let git_branch_name = issue
        .get("branchName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let status = issue
        .get("status")
        .and_then(|s| s.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let team = issue
        .get("team")
        .and_then(|t| t.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let labels = issue
        .get("labels")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l.get("name").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(LinearIssue {
        identifier,
        title,
        description,
        git_branch_name,
        status,
        team,
        labels,
    })
}

pub fn build_prompt(issue: &LinearIssue, event: &LinearEvent) -> String {
    let mut prompt = format!(
        "Implement {}: {}\n\n{}",
        issue.identifier, issue.title, issue.description
    );

    if let Some(ref ctx) = event.prompt_context {
        prompt.push_str("\n\n--- Linear Context ---\n");
        prompt.push_str(ctx);
    }
    if let Some(ref comment) = event.comment {
        prompt.push_str("\n\n--- Additional Instructions ---\n");
        prompt.push_str(comment);
    }

    prompt
}

pub fn verify_webhook_signature(body: &[u8], signature: &str, secret: &str) -> bool {
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);

    let expected = match hex::decode(signature) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    mac.verify_slice(&expected).is_ok()
}

pub fn start_http_listener(port: u16, secret: String, tx: mpsc::Sender<LinearEvent>) {
    std::thread::spawn(move || {
        let addr = format!("0.0.0.0:{}", port);
        let server = match tiny_http::Server::http(&addr) {
            Ok(s) => s,
            Err(e) => {
                crate::tlog!(error, "Linear HTTP listener failed to start on {}: {}", addr, e);
                return;
            }
        };
        crate::tlog!(info, "Linear HTTP listener started on :{}", port);

        for mut request in server.incoming_requests() {
            let method = request.method().to_string();
            let url = request.url().to_string();

            match (method.as_str(), url.as_str()) {
                ("GET", "/health") => {
                    let response = tiny_http::Response::from_string("ok");
                    let _ = request.respond(response);
                }
                ("POST", "/webhook/linear") => {
                    let mut body = Vec::new();
                    if request.as_reader().read_to_end(&mut body).is_err() {
                        let resp = tiny_http::Response::from_string("bad request")
                            .with_status_code(400);
                        let _ = request.respond(resp);
                        continue;
                    }

                    let sig = request
                        .headers()
                        .iter()
                        .find(|h| h.field.as_str() == "Linear-Signature" || h.field.as_str() == "linear-signature")
                        .map(|h| h.value.as_str().to_string());

                    match sig {
                        Some(ref s) if verify_webhook_signature(&body, s, &secret) => {}
                        _ => {
                            crate::tlog!(warn, "Linear webhook: invalid or missing signature");
                            let resp = tiny_http::Response::from_string("unauthorized")
                                .with_status_code(401);
                            let _ = request.respond(resp);
                            continue;
                        }
                    }

                    match parse_webhook_body(&body) {
                        Ok(event) => {
                            crate::tlog!(info, "Linear webhook received for issue: {}", event.issue_id);
                            let _ = tx.send(event);
                            let resp = tiny_http::Response::from_string("ok");
                            let _ = request.respond(resp);
                        }
                        Err(e) => {
                            crate::tlog!(warn, "Linear webhook parse error: {}", e);
                            let resp = tiny_http::Response::from_string("bad request")
                                .with_status_code(400);
                            let _ = request.respond(resp);
                        }
                    }
                }
                ("POST", "/queue") => {
                    let remote_addr = request.remote_addr().map(|a| a.ip().to_string());
                    let is_local = remote_addr
                        .as_deref()
                        .map(|ip| ip == "127.0.0.1" || ip == "::1")
                        .unwrap_or(false);

                    if !is_local {
                        let resp = tiny_http::Response::from_string("forbidden")
                            .with_status_code(403);
                        let _ = request.respond(resp);
                        continue;
                    }

                    let mut body = Vec::new();
                    if request.as_reader().read_to_end(&mut body).is_err() {
                        let resp = tiny_http::Response::from_string("bad request")
                            .with_status_code(400);
                        let _ = request.respond(resp);
                        continue;
                    }

                    match parse_queue_body(&body) {
                        Ok(event) => {
                            crate::tlog!(info, "Linear queue request for issue: {}", event.issue_id);
                            let _ = tx.send(event);
                            let resp = tiny_http::Response::from_string("queued");
                            let _ = request.respond(resp);
                        }
                        Err(e) => {
                            let resp =
                                tiny_http::Response::from_string(format!("bad request: {}", e))
                                    .with_status_code(400);
                            let _ = request.respond(resp);
                        }
                    }
                }
                _ => {
                    let resp = tiny_http::Response::from_string("not found")
                        .with_status_code(404);
                    let _ = request.respond(resp);
                }
            }
        }
    });
}

fn parse_webhook_body(body: &[u8]) -> Result<LinearEvent> {
    let value: serde_json::Value =
        serde_json::from_slice(body).context("invalid webhook JSON")?;

    let issue_id = value
        .get("data")
        .and_then(|d| d.get("agentSession"))
        .and_then(|s| s.get("issue"))
        .and_then(|i| i.get("identifier"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            value
                .get("issue_id")
                .or_else(|| value.get("issueId"))
                .and_then(|v| v.as_str())
        })
        .ok_or_else(|| anyhow::anyhow!("missing issue identifier in webhook payload"))?
        .to_string();

    let prompt_context = value
        .get("data")
        .and_then(|d| d.get("agentSession"))
        .and_then(|s| s.get("promptContext"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let comment = value
        .get("data")
        .and_then(|d| d.get("agentSession"))
        .and_then(|s| s.get("comment"))
        .and_then(|c| c.get("body"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(LinearEvent {
        issue_id,
        prompt_context,
        comment,
    })
}

fn parse_queue_body(body: &[u8]) -> Result<LinearEvent> {
    let value: serde_json::Value =
        serde_json::from_slice(body).context("invalid queue JSON")?;

    let issue_id = value
        .get("issue_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing issue_id field"))?
        .to_string();

    let comment = value
        .get("comment")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(LinearEvent {
        issue_id,
        prompt_context: None,
        comment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RESPONSE: &str = r#"{
        "data": {
            "issue": {
                "identifier": "LUM-631",
                "title": "restyle validation review readonly renderer",
                "description": "the review readonly renderer is looking very barebones",
                "branchName": "feature/lum-631-restyle-validation-review-readonly-renderer",
                "status": { "name": "In Review" },
                "team": { "name": "Lumen" },
                "labels": { "nodes": [{ "name": "UI" }, { "name": "Improvement" }] }
            }
        }
    }"#;

    #[test]
    fn test_parse_issue_response_happy_path() {
        let issue = parse_issue_response(SAMPLE_RESPONSE).unwrap();
        assert_eq!(issue.identifier, "LUM-631");
        assert_eq!(issue.title, "restyle validation review readonly renderer");
        assert!(issue.description.contains("barebones"));
        assert_eq!(
            issue.git_branch_name.as_deref(),
            Some("feature/lum-631-restyle-validation-review-readonly-renderer")
        );
        assert_eq!(issue.status, "In Review");
        assert_eq!(issue.team, "Lumen");
        assert_eq!(issue.labels, vec!["UI", "Improvement"]);
    }

    #[test]
    fn test_parse_issue_response_missing_branch() {
        let json = r#"{
            "data": {
                "issue": {
                    "identifier": "TEST-1",
                    "title": "Test",
                    "description": "",
                    "status": { "name": "Todo" },
                    "team": { "name": "Eng" },
                    "labels": { "nodes": [] }
                }
            }
        }"#;
        let issue = parse_issue_response(json).unwrap();
        assert!(issue.git_branch_name.is_none());
    }

    #[test]
    fn test_parse_issue_response_malformed_json() {
        let result = parse_issue_response("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_issue_response_missing_data() {
        let result = parse_issue_response(r#"{"errors": [{"message": "not found"}]}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_webhook_signature_valid() {
        let body = b"test body";
        let secret = "test_secret";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(verify_webhook_signature(body, &signature, secret));
    }

    #[test]
    fn test_verify_webhook_signature_invalid_body() {
        let secret = "test_secret";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(b"original body");
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(!verify_webhook_signature(b"tampered body", &signature, secret));
    }

    #[test]
    fn test_verify_webhook_signature_wrong_secret() {
        let body = b"test body";

        let mut mac = HmacSha256::new_from_slice(b"correct_secret").unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(!verify_webhook_signature(body, &signature, "wrong_secret"));
    }

    #[test]
    fn test_verify_webhook_signature_empty() {
        assert!(!verify_webhook_signature(b"body", "", "secret"));
    }

    #[test]
    fn test_build_prompt_basic() {
        let issue = LinearIssue {
            identifier: "LUM-1".to_string(),
            title: "Fix the bug".to_string(),
            description: "It crashes on startup".to_string(),
            git_branch_name: None,
            status: "Todo".to_string(),
            team: "Eng".to_string(),
            labels: vec![],
        };
        let event = LinearEvent {
            issue_id: "LUM-1".to_string(),
            prompt_context: None,
            comment: None,
        };
        let prompt = build_prompt(&issue, &event);
        assert!(prompt.contains("Implement LUM-1: Fix the bug"));
        assert!(prompt.contains("It crashes on startup"));
        assert!(!prompt.contains("Linear Context"));
        assert!(!prompt.contains("Additional Instructions"));
    }

    #[test]
    fn test_build_prompt_with_context() {
        let issue = LinearIssue {
            identifier: "LUM-2".to_string(),
            title: "Add feature".to_string(),
            description: "Add the thing".to_string(),
            git_branch_name: None,
            status: "Todo".to_string(),
            team: "Eng".to_string(),
            labels: vec![],
        };
        let event = LinearEvent {
            issue_id: "LUM-2".to_string(),
            prompt_context: Some("<context>issue details here</context>".to_string()),
            comment: None,
        };
        let prompt = build_prompt(&issue, &event);
        assert!(prompt.contains("--- Linear Context ---"));
        assert!(prompt.contains("issue details here"));
    }

    #[test]
    fn test_build_prompt_with_comment() {
        let issue = LinearIssue {
            identifier: "LUM-3".to_string(),
            title: "Style update".to_string(),
            description: "Restyle the page".to_string(),
            git_branch_name: None,
            status: "Todo".to_string(),
            team: "Eng".to_string(),
            labels: vec![],
        };
        let event = LinearEvent {
            issue_id: "LUM-3".to_string(),
            prompt_context: None,
            comment: Some("Focus on the header area only".to_string()),
        };
        let prompt = build_prompt(&issue, &event);
        assert!(prompt.contains("--- Additional Instructions ---"));
        assert!(prompt.contains("Focus on the header area only"));
    }

    #[test]
    fn test_build_prompt_with_all() {
        let issue = LinearIssue {
            identifier: "LUM-4".to_string(),
            title: "Full feature".to_string(),
            description: "Build everything".to_string(),
            git_branch_name: None,
            status: "Todo".to_string(),
            team: "Eng".to_string(),
            labels: vec![],
        };
        let event = LinearEvent {
            issue_id: "LUM-4".to_string(),
            prompt_context: Some("xml context".to_string()),
            comment: Some("do it fast".to_string()),
        };
        let prompt = build_prompt(&issue, &event);
        assert!(prompt.contains("Implement LUM-4"));
        assert!(prompt.contains("--- Linear Context ---"));
        assert!(prompt.contains("xml context"));
        assert!(prompt.contains("--- Additional Instructions ---"));
        assert!(prompt.contains("do it fast"));
    }

    #[test]
    fn test_parse_webhook_body_agent_session() {
        let body = r#"{
            "data": {
                "agentSession": {
                    "issue": { "identifier": "LUM-100" },
                    "promptContext": "<issue>context here</issue>",
                    "comment": { "body": "please fix this fast" }
                }
            }
        }"#;
        let event = parse_webhook_body(body.as_bytes()).unwrap();
        assert_eq!(event.issue_id, "LUM-100");
        assert_eq!(event.prompt_context.as_deref(), Some("<issue>context here</issue>"));
        assert_eq!(event.comment.as_deref(), Some("please fix this fast"));
    }

    #[test]
    fn test_parse_webhook_body_simple() {
        let body = r#"{"issue_id": "LUM-200"}"#;
        let event = parse_webhook_body(body.as_bytes()).unwrap();
        assert_eq!(event.issue_id, "LUM-200");
        assert!(event.prompt_context.is_none());
        assert!(event.comment.is_none());
    }

    #[test]
    fn test_parse_queue_body() {
        let body = r#"{"issue_id": "LUM-300", "comment": "focus on tests"}"#;
        let event = parse_queue_body(body.as_bytes()).unwrap();
        assert_eq!(event.issue_id, "LUM-300");
        assert_eq!(event.comment.as_deref(), Some("focus on tests"));
    }

    #[test]
    fn test_parse_queue_body_minimal() {
        let body = r#"{"issue_id": "LUM-400"}"#;
        let event = parse_queue_body(body.as_bytes()).unwrap();
        assert_eq!(event.issue_id, "LUM-400");
        assert!(event.comment.is_none());
    }

    #[test]
    fn test_parse_queue_body_missing_id() {
        let result = parse_queue_body(b"{}");
        assert!(result.is_err());
    }
}
