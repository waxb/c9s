use anyhow::{bail, Context, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct LinearConfig {
    pub api_key: String,
    pub refresh_token: Option<String>,
    pub webhook_secret: String,
    pub port: u16,
    pub default_repo: Option<PathBuf>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub repos: std::collections::HashMap<String, String>,
}

impl LinearConfig {
    pub fn needs_auth(&self) -> bool {
        self.api_key.is_empty() && self.client_id.is_some() && self.client_secret.is_some()
    }

    pub fn is_ready(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub fn oauth_authorize_url(&self) -> Option<String> {
        let client_id = self.client_id.as_ref()?;
        let redirect = format!("http://localhost:{}/oauth/callback", self.port);
        Some(format!(
            "https://linear.app/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope=read,write&actor=app",
            client_id,
            urlencoded(&redirect),
        ))
    }

    pub fn load() -> Option<Self> {
        let config_path = dirs::home_dir()?.join(".c9s").join("config.toml");
        let content = std::fs::read_to_string(&config_path).ok()?;
        let table: toml::Table = content.parse().ok()?;
        let linear = table.get("linear")?;

        let api_key = linear
            .get("api_key")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("LINEAR_API_KEY").ok())
            .unwrap_or_default();

        let webhook_secret = linear
            .get("webhook_secret")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("LINEAR_WEBHOOK_SECRET").ok())
            .unwrap_or_default();

        let port = linear
            .get("port")
            .and_then(|v| v.as_integer())
            .map(|p| p as u16)
            .or_else(|| {
                std::env::var("C9S_LINEAR_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
            })
            .unwrap_or(9519);

        let default_repo = linear
            .get("default_repo")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(|| std::env::var("C9S_DEFAULT_REPO").ok().map(PathBuf::from));

        let client_id = linear
            .get("client_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let client_secret = linear
            .get("client_secret")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let refresh_token = linear
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let repos = linear
            .get("repos")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Some(LinearConfig {
            api_key,
            refresh_token,
            webhook_secret,
            port,
            default_repo,
            client_id,
            client_secret,
            repos,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LinearIssue {
    pub identifier: String,
    pub title: String,
    pub description: String,
    pub git_branch_name: Option<String>,
    pub status: String,
    pub team: String,
    pub team_key: String,
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
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
}

impl LinearClient {
    pub fn new(config: &LinearConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            refresh_token: config.refresh_token.clone(),
        }
    }

    fn graphql_request(&mut self, query: &str) -> Result<String> {
        let result = ureq::post("https://api.linear.app/graphql")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send(query.as_bytes());

        match result {
            Ok(response) => {
                response.into_body().read_to_string()
                    .context("failed to read Linear response")
            }
            Err(ureq::Error::StatusCode(401)) => {
                crate::tlog!(info, "Linear: access token expired, attempting refresh...");
                self.refresh_access_token()?;

                let response = ureq::post("https://api.linear.app/graphql")
                    .header("Authorization", &format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .send(query.as_bytes())
                    .context("Linear API call failed after token refresh")?;

                response.into_body().read_to_string()
                    .context("failed to read Linear response")
            }
            Err(e) => Err(anyhow::anyhow!("Linear API call failed: {}", e)),
        }
    }

    fn refresh_access_token(&mut self) -> Result<()> {
        let refresh = self.refresh_token.as_ref()
            .ok_or_else(|| anyhow::anyhow!("no refresh_token available, re-auth required"))?;
        let client_id = self.client_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("no client_id for token refresh"))?;
        let client_secret = self.client_secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("no client_secret for token refresh"))?;

        let body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
            urlencoded(refresh),
            urlencoded(client_id),
            urlencoded(client_secret),
        );

        let response = ureq::post("https://api.linear.app/oauth/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .context("token refresh request failed")?;

        let resp_body = response.into_body().read_to_string()
            .context("failed to read refresh response")?;

        let value: serde_json::Value = serde_json::from_str(&resp_body)
            .context("failed to parse refresh response")?;

        let new_access = value.get("access_token").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("no access_token in refresh response"))?;

        let new_refresh = value.get("refresh_token").and_then(|v| v.as_str());

        self.api_key = new_access.to_string();
        if let Some(rt) = new_refresh {
            self.refresh_token = Some(rt.to_string());
        }

        save_tokens_to_config(&self.api_key, self.refresh_token.as_deref())?;
        crate::tlog!(info, "Linear: token refreshed and saved");

        Ok(())
    }

    pub fn fetch_issue(&mut self, identifier: &str) -> Result<LinearIssue> {
        let query = format!(
            r#"{{ "query": "query {{ issue(id: \"{}\") {{ identifier title description branchName status {{ name }} team {{ name key }} labels {{ nodes {{ name }} }} }} }}" }}"#,
            identifier
        );

        let body = self.graphql_request(&query)?;
        parse_issue_response(&body)
    }

    pub fn update_issue_status(&mut self, identifier: &str, status_name: &str) -> Result<()> {
        let query = format!(
            r#"{{ "query": "mutation {{ issueUpdate(id: \"{}\", input: {{ stateId: \"{}\" }}) {{ success }} }}" }}"#,
            identifier, status_name
        );

        let _ = self.graphql_request(&query)?;

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

    let team_key = issue
        .get("team")
        .and_then(|t| t.get("key"))
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
        team_key,
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

pub fn start_http_listener(port: u16, secret: String, client_id: Option<String>, client_secret: Option<String>, tx: mpsc::Sender<LinearEvent>) {
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
                ("GET", url) if url.starts_with("/oauth/callback") => {
                    let query = url.splitn(2, '?').nth(1).unwrap_or("");
                    let code = query
                        .split('&')
                        .find_map(|pair| {
                            let mut kv = pair.splitn(2, '=');
                            match (kv.next(), kv.next()) {
                                (Some("code"), Some(v)) => Some(v.to_string()),
                                _ => None,
                            }
                        });

                    match (code, client_id.as_ref(), client_secret.as_ref()) {
                        (Some(code), Some(cid), Some(csec)) => {
                            let redirect_uri = format!("http://localhost:{}/oauth/callback", port);
                            match exchange_oauth_token(&code, cid, csec, &redirect_uri) {
                                Ok(tokens) => {
                                    if let Err(e) = save_tokens_to_config(&tokens.access_token, tokens.refresh_token.as_deref()) {
                                        crate::tlog!(error, "Failed to save Linear tokens: {}", e);
                                    } else {
                                        crate::tlog!(info, "Linear OAuth tokens saved to config (access + refresh)");
                                    }
                                    let html = "<html><body><h1>c9s connected to Linear</h1><p>You can close this tab. The API token has been saved to ~/.c9s/config.toml</p></body></html>";
                                    let resp = tiny_http::Response::from_string(html)
                                        .with_header("Content-Type: text/html".parse::<tiny_http::Header>().unwrap());
                                    let _ = request.respond(resp);
                                }
                                Err(e) => {
                                    crate::tlog!(error, "Linear OAuth token exchange failed: {}", e);
                                    let resp = tiny_http::Response::from_string(format!("OAuth failed: {}", e))
                                        .with_status_code(500);
                                    let _ = request.respond(resp);
                                }
                            }
                        }
                        (None, _, _) => {
                            let resp = tiny_http::Response::from_string("missing code parameter")
                                .with_status_code(400);
                            let _ = request.respond(resp);
                        }
                        _ => {
                            let resp = tiny_http::Response::from_string("client_id/client_secret not configured in ~/.c9s/config.toml")
                                .with_status_code(500);
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

fn urlencoded(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

struct OAuthTokens {
    access_token: String,
    refresh_token: Option<String>,
}

fn exchange_oauth_token(code: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Result<OAuthTokens> {
    let body = format!(
        "grant_type=authorization_code&code={}&client_id={}&client_secret={}&redirect_uri={}",
        urlencoded(code),
        urlencoded(client_id),
        urlencoded(client_secret),
        urlencoded(redirect_uri),
    );

    let response = ureq::post("https://api.linear.app/oauth/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body.as_bytes())
        .context("failed to exchange OAuth code")?;

    let resp_body = response.into_body().read_to_string()
        .context("failed to read token response")?;

    let value: serde_json::Value = serde_json::from_str(&resp_body)
        .context("failed to parse token response")?;

    let access_token = value
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no access_token in OAuth response: {}", resp_body))?;

    let refresh_token = value
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(OAuthTokens {
        access_token,
        refresh_token,
    })
}

fn save_tokens_to_config(access_token: &str, refresh_token: Option<&str>) -> Result<()> {
    let config_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
        .join(".c9s")
        .join("config.toml");

    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut table: toml::Table = content.parse().unwrap_or_default();

    let linear = table
        .entry("linear")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));

    if let toml::Value::Table(ref mut t) = linear {
        t.insert("api_key".to_string(), toml::Value::String(access_token.to_string()));
        if let Some(rt) = refresh_token {
            t.insert("refresh_token".to_string(), toml::Value::String(rt.to_string()));
        }
    }

    std::fs::write(&config_path, table.to_string())
        .context("failed to write config.toml")?;

    Ok(())
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
                "team": { "name": "Lumen", "key": "LUM" },
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
        assert_eq!(issue.team_key, "LUM");
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
                    "team": { "name": "Eng", "key": "ENG" },
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
            team_key: "ENG".to_string(),
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
            team_key: "ENG".to_string(),
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
            team_key: "ENG".to_string(),
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
            team_key: "ENG".to_string(),
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
