use serde::Deserialize;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const CACHE_TTL_SECS: u64 = 60;
const REQUEST_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone, Default)]
pub struct UsageData {
    pub plan_name: Option<String>,
    pub five_hour: Option<u8>,
    pub five_hour_reset: Option<String>,
    pub seven_day: Option<u8>,
    pub seven_day_reset: Option<String>,
    pub api_available: bool,
}

pub struct UsageFetcher {
    cached: UsageData,
    last_fetch: Option<Instant>,
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

#[derive(Deserialize)]
struct ApiResponse {
    five_hour: Option<WindowData>,
    seven_day: Option<WindowData>,
}

#[derive(Deserialize)]
struct WindowData {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

impl UsageFetcher {
    pub fn new() -> Self {
        Self {
            cached: UsageData::default(),
            last_fetch: None,
        }
    }

    pub fn get(&mut self) -> &UsageData {
        let should_fetch = match self.last_fetch {
            Some(t) => t.elapsed().as_secs() >= CACHE_TTL_SECS,
            None => true,
        };

        if should_fetch {
            self.cached = fetch_usage();
            self.last_fetch = Some(Instant::now());
        }

        &self.cached
    }
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
        Some(resp) => UsageData {
            plan_name,
            five_hour: parse_utilization(resp.five_hour.as_ref().and_then(|w| w.utilization)),
            five_hour_reset: resp
                .five_hour
                .as_ref()
                .and_then(|w| w.resets_at.as_ref())
                .and_then(|s| format_reset_time(s)),
            seven_day: parse_utilization(resp.seven_day.as_ref().and_then(|w| w.utilization)),
            seven_day_reset: resp
                .seven_day
                .as_ref()
                .and_then(|w| w.resets_at.as_ref())
                .and_then(|s| format_reset_time(s)),
            api_available: true,
        },
        None => UsageData {
            plan_name,
            api_available: false,
            ..UsageData::default()
        },
    }
}

fn read_credentials() -> Option<(String, String)> {
    let home = dirs::home_dir()?;
    let path = home.join(".claude").join(".credentials.json");
    let content = std::fs::read_to_string(path).ok()?;
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
    fn test_parse_utilization() {
        assert_eq!(parse_utilization(Some(50.0)), Some(50));
        assert_eq!(parse_utilization(Some(0.0)), Some(0));
        assert_eq!(parse_utilization(Some(100.0)), Some(100));
        assert_eq!(parse_utilization(Some(150.0)), Some(100));
        assert_eq!(parse_utilization(Some(f64::NAN)), None);
        assert_eq!(parse_utilization(None), None);
    }
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
