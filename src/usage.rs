use serde::Deserialize;
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const CACHE_TTL_SECS: u64 = 300;
const INVALIDATED_TTL_SECS: u64 = 30;
const BACKOFF_TTL_SECS: u64 = 120;
const REQUEST_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageWindow {
    pub label: String,
    pub pct: u8,
    pub reset: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UsageData {
    pub plan_name: Option<String>,
    pub five_hour: Option<u8>,
    pub five_hour_reset: Option<String>,
    pub seven_day: Option<u8>,
    pub seven_day_reset: Option<String>,
    pub weekly_models: Vec<UsageWindow>,
    pub api_available: bool,
}

pub struct UsageFetcher {
    cached: UsageData,
    last_request: Option<Instant>,
    rx: mpsc::Receiver<UsageData>,
    tx: mpsc::Sender<()>,
    inflight: bool,
}

#[derive(Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthData>,
}

#[derive(Deserialize)]
struct OAuthData {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<u64>,
}

#[derive(Deserialize, Default)]
struct ApiResponse {
    five_hour: Option<WindowData>,
    seven_day: Option<WindowData>,
    seven_day_opus: Option<WindowData>,
    seven_day_sonnet: Option<WindowData>,
    #[serde(default)]
    limits: Vec<ScopedLimit>,
}

#[derive(Deserialize, Default)]
struct WindowData {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize, Default)]
struct ScopedLimit {
    kind: Option<String>,
    scope: Option<LimitScope>,
    percent: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize, Default)]
struct LimitScope {
    model: Option<ScopeModel>,
}

#[derive(Deserialize, Default)]
struct ScopeModel {
    display_name: Option<String>,
}

impl ScopedLimit {
    fn weekly_model_name(&self) -> Option<&str> {
        if self.kind.as_deref() != Some("weekly_scoped") {
            return None;
        }
        self.scope
            .as_ref()
            .and_then(|s| s.model.as_ref())
            .and_then(|m| m.display_name.as_deref())
            .filter(|n| !n.is_empty())
    }
}

impl UsageFetcher {
    pub fn new() -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        let (trigger_tx, trigger_rx) = mpsc::channel::<()>();

        std::thread::spawn(move || {
            while trigger_rx.recv().is_ok() {
                let data = fetch_usage();
                if result_tx.send(data).is_err() {
                    break;
                }
            }
        });

        Self {
            cached: UsageData::default(),
            last_request: None,
            rx: result_rx,
            tx: trigger_tx,
            inflight: false,
        }
    }

    pub fn get(&mut self) -> &UsageData {
        while let Ok(data) = self.rx.try_recv() {
            self.inflight = false;
            if data.api_available {
                self.cached = data;
            } else {
                self.last_request = backdated(CACHE_TTL_SECS - BACKOFF_TTL_SECS);
                return &self.cached;
            }
            self.last_request = Some(Instant::now());
        }

        if !self.inflight {
            let should_fetch = match self.last_request {
                Some(t) => t.elapsed().as_secs() >= CACHE_TTL_SECS,
                None => true,
            };
            if should_fetch {
                self.inflight = true;
                let _ = self.tx.send(());
            }
        }

        &self.cached
    }

    pub fn invalidate(&mut self) {
        if self.last_request.is_some() {
            self.last_request = backdated(CACHE_TTL_SECS - INVALIDATED_TTL_SECS);
        }
    }
}

fn backdated(secs: u64) -> Option<Instant> {
    Instant::now().checked_sub(std::time::Duration::from_secs(secs))
}

fn fetch_usage() -> UsageData {
    let creds = match read_credentials() {
        Some(c) => c,
        None => return UsageData::default(),
    };

    let (token, sub_type) = creds;
    let plan_name = plan_from_subscription(&sub_type);

    if plan_name.is_none() {
        return UsageData::default();
    }

    match call_api(&token) {
        Some(resp) => build_usage(plan_name, &resp),
        None => UsageData {
            plan_name,
            api_available: false,
            ..UsageData::default()
        },
    }
}

fn utilization_scale(values: &[f64]) -> f64 {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if !finite.is_empty() && finite.iter().all(|v| *v <= 1.0) {
        100.0
    } else {
        1.0
    }
}

fn window_pct(window: Option<&WindowData>, scale: f64) -> Option<u8> {
    parse_utilization(window.and_then(|w| w.utilization).map(|v| v * scale))
}

fn window_reset(window: Option<&WindowData>) -> Option<String> {
    window
        .and_then(|w| w.resets_at.as_deref())
        .and_then(format_reset_time)
}

fn build_usage(plan_name: Option<String>, resp: &ApiResponse) -> UsageData {
    let raw: Vec<f64> = [
        &resp.five_hour,
        &resp.seven_day,
        &resp.seven_day_opus,
        &resp.seven_day_sonnet,
    ]
    .iter()
    .filter_map(|w| w.as_ref().and_then(|w| w.utilization))
    .chain(resp.limits.iter().filter_map(|l| l.percent))
    .collect();
    let scale = utilization_scale(&raw);

    let mut weekly_models: Vec<UsageWindow> = resp
        .limits
        .iter()
        .filter_map(|limit| {
            let name = limit.weekly_model_name()?;
            let pct = parse_utilization(limit.percent.map(|v| v * scale))?;
            Some(UsageWindow {
                label: format!("Current week ({})", name),
                pct,
                reset: limit.resets_at.as_deref().and_then(format_reset_time),
            })
        })
        .collect();
    for (window, name) in [
        (&resp.seven_day_opus, "Opus"),
        (&resp.seven_day_sonnet, "Sonnet"),
    ] {
        let label = format!("Current week ({})", name);
        if weekly_models.iter().any(|w| w.label == label) {
            continue;
        }
        if let Some(pct) = window_pct(window.as_ref(), scale) {
            weekly_models.push(UsageWindow {
                label,
                pct,
                reset: window_reset(window.as_ref()),
            });
        }
    }

    UsageData {
        plan_name,
        five_hour: window_pct(resp.five_hour.as_ref(), scale),
        five_hour_reset: window_reset(resp.five_hour.as_ref()),
        seven_day: window_pct(resp.seven_day.as_ref(), scale),
        seven_day_reset: window_reset(resp.seven_day.as_ref()),
        weekly_models,
        api_available: true,
    }
}

fn read_credentials() -> Option<(String, String)> {
    let content = read_credentials_json()?;
    let creds: Credentials = serde_json::from_str(&content).ok()?;
    let oauth = creds.claude_ai_oauth?;
    let token = oauth.access_token?;

    if let Some(expires_at) = oauth.expires_at {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if expires_at <= now_ms {
            return None;
        }
    }

    let sub_type = oauth.subscription_type.unwrap_or_default();
    Some((token, sub_type))
}

fn read_credentials_json() -> Option<String> {
    let home = dirs::home_dir()?;
    let path = home.join(".claude").join(".credentials.json");
    if let Ok(content) = std::fs::read_to_string(path) {
        return Some(content);
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "Claude Code-credentials",
                "-w",
            ])
            .output()
            .ok()?;
        if output.status.success() {
            let json = String::from_utf8(output.stdout).ok()?;
            return Some(json.trim().to_string());
        }
    }

    None
}

fn plan_from_subscription(sub_type: &str) -> Option<String> {
    let lower = sub_type.to_lowercase();
    if lower.contains("max") {
        Some("Max".to_string())
    } else if lower.contains("pro") {
        Some("Pro".to_string())
    } else if lower.contains("team") {
        Some("Team".to_string())
    } else if sub_type.is_empty() || lower.contains("api") {
        None
    } else {
        let mut chars = sub_type.chars();
        let first = chars.next()?.to_uppercase().to_string();
        Some(first + chars.as_str())
    }
}

fn parse_utilization(val: Option<f64>) -> Option<u8> {
    val.filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 100.0) as u8)
}

fn format_reset_time(iso: &str) -> Option<String> {
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let local = dt.with_timezone(&chrono::Local);
    Some(local.format("%b %-d at %-H:%M").to_string())
}

fn call_api(token: &str) -> Option<ApiResponse> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS)))
        .build()
        .new_agent();

    let resp = agent
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", &format!("Bearer {}", token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "c9s/0.1")
        .call()
        .ok()?;

    if resp.status() != 200 {
        return None;
    }

    let body = resp.into_body().read_to_string().ok()?;
    serde_json::from_str(&body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_from_subscription() {
        assert_eq!(plan_from_subscription("max_5x"), Some("Max".to_string()));
        assert_eq!(plan_from_subscription("pro"), Some("Pro".to_string()));
        assert_eq!(
            plan_from_subscription("team_enterprise"),
            Some("Team".to_string())
        );
        assert_eq!(plan_from_subscription(""), None);
        assert_eq!(plan_from_subscription("api_key"), None);
    }

    #[test]
    fn test_build_usage_with_scoped_fable_limit_and_percent_scale() {
        let body = r#"{
            "five_hour": {"utilization": 27.0, "resets_at": "2026-09-08T15:00:00Z"},
            "seven_day": {"utilization": 41.5, "resets_at": "2026-09-12T09:00:00Z"},
            "seven_day_sonnet": {"utilization": 12.0, "resets_at": "2026-09-12T09:00:00Z"},
            "seven_day_opus": null,
            "limits": [
                {"kind": "weekly_scoped", "group": "weekly", "percent": 63.2,
                 "resets_at": "2026-09-12T09:00:00Z", "scope": {"model": {"display_name": "Fable"}}},
                {"kind": "other", "percent": 5.0}
            ]
        }"#;
        let resp: ApiResponse = serde_json::from_str(body).unwrap();
        let usage = build_usage(Some("Max".into()), &resp);
        assert!(usage.api_available);
        assert_eq!(usage.five_hour, Some(27));
        assert_eq!(usage.seven_day, Some(41));
        let labels: Vec<&str> = usage
            .weekly_models
            .iter()
            .map(|w| w.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec!["Current week (Fable)", "Current week (Sonnet)"]
        );
        assert_eq!(usage.weekly_models[0].pct, 63);
        assert_eq!(usage.weekly_models[1].pct, 12);
        assert!(usage.weekly_models[0].reset.is_some());
    }

    #[test]
    fn test_build_usage_scales_fractions_to_percent() {
        let body = r#"{
            "five_hour": {"utilization": 0.27, "resets_at": null},
            "seven_day": {"utilization": 0.9},
            "limits": [{"kind": "weekly_scoped", "percent": 0.5, "scope": {"model": {"display_name": "Fable"}}}]
        }"#;
        let resp: ApiResponse = serde_json::from_str(body).unwrap();
        let usage = build_usage(None, &resp);
        assert_eq!(usage.five_hour, Some(27));
        assert_eq!(usage.seven_day, Some(90));
        assert_eq!(usage.weekly_models[0].pct, 50);
    }

    #[test]
    fn test_build_usage_without_extras() {
        let resp: ApiResponse =
            serde_json::from_str(r#"{"five_hour": {"utilization": 3.0}}"#).unwrap();
        let usage = build_usage(None, &resp);
        assert_eq!(usage.five_hour, Some(3));
        assert_eq!(usage.seven_day, None);
        assert!(usage.weekly_models.is_empty());
    }

    #[test]
    fn test_parse_utilization() {
        assert_eq!(parse_utilization(Some(50.0)), Some(50));
        assert_eq!(parse_utilization(Some(0.0)), Some(0));
        assert_eq!(parse_utilization(Some(100.0)), Some(100));
        assert_eq!(parse_utilization(Some(150.0)), Some(100));
        assert_eq!(parse_utilization(Some(f64::NAN)), None);
        assert_eq!(parse_utilization(None), None);
    }
}
