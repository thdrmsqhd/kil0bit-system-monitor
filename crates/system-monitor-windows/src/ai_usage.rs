//! OpenCode Go's authenticated public quota endpoint and deterministic JSON parser.
//! Requests run on a caller-owned worker thread, never in the Win32 window procedure.

use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use system_monitor_core::{AiUsageSnapshot, AiUsageWindow};

const OPENCODE_GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingKey,
    Unauthorized,
    RateLimited,
    HttpStatus(u16),
    Network,
    MalformedResponse,
}

/// Parse supported current/compatibility response layouts without leaking response contents.
pub fn parse_opencode_usage(
    value: &Value,
    now_epoch_seconds: u64,
) -> Result<AiUsageSnapshot, ProviderError> {
    let root = value.get("usage").unwrap_or(value);
    let rolling = parse_window(root, &["rolling", "rollingUsage"], now_epoch_seconds)?;
    let weekly = parse_optional_window(root, &["weekly", "weeklyUsage"], now_epoch_seconds);
    let monthly = parse_optional_window(root, &["monthly", "monthlyUsage"], now_epoch_seconds);
    Ok(AiUsageSnapshot {
        rolling: Some(rolling),
        weekly,
        monthly,
        updated_at_epoch_seconds: now_epoch_seconds,
        stale: false,
    })
}

fn parse_window(root: &Value, aliases: &[&str], now: u64) -> Result<AiUsageWindow, ProviderError> {
    parse_optional_window(root, aliases, now).ok_or(ProviderError::MalformedResponse)
}

fn parse_optional_window(root: &Value, aliases: &[&str], now: u64) -> Option<AiUsageWindow> {
    let object = aliases
        .iter()
        .find_map(|name| root.get(name).filter(|v| !v.is_null()))?;
    let used = object
        .get("percent")
        .or_else(|| object.get("usagePercent"))?
        .as_f64()?;
    if !(0.0..=100.0).contains(&used) {
        return None;
    }
    let reset_seconds = object
        .get("resetInSec")
        .or_else(|| object.get("reset_in_sec"))
        .and_then(Value::as_u64);
    Some(AiUsageWindow {
        used_percent: used.round() as u8,
        reset_at_epoch_seconds: reset_seconds.map(|seconds| now.saturating_add(seconds)),
    })
}

pub fn fetch_opencode_usage(api_key: &str) -> Result<AiUsageSnapshot, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| ProviderError::Network)?;
    let response = client
        .get(OPENCODE_GO_USAGE_URL)
        .bearer_auth(api_key)
        .send()
        .map_err(|_| ProviderError::Network)?;
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(ProviderError::Unauthorized);
    }
    if status.as_u16() == 429 {
        return Err(ProviderError::RateLimited);
    }
    if !status.is_success() {
        return Err(ProviderError::HttpStatus(status.as_u16()));
    }
    let value: Value = response
        .json()
        .map_err(|_| ProviderError::MalformedResponse)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    parse_opencode_usage(&value, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quota_window_percentages_and_reset_seconds() {
        let value = serde_json::json!({
            "usage": {
                "rolling": { "percent": 45, "resetInSec": 300 },
                "weekly": { "percent": 62, "resetInSec": 7200 },
                "monthly": { "percent": 0 }
            }
        });
        let snapshot = parse_opencode_usage(&value, 10_000).unwrap();
        assert_eq!(snapshot.rolling.as_ref().unwrap().used_percent, 45);
        assert_eq!(snapshot.rolling.as_ref().unwrap().remaining_percent(), 55);
        assert_eq!(
            snapshot.rolling.unwrap().reset_at_epoch_seconds,
            Some(10_300)
        );
        assert_eq!(
            snapshot.weekly.unwrap().reset_at_epoch_seconds,
            Some(17_200)
        );
        assert_eq!(snapshot.monthly.unwrap().used_percent, 0);
    }

    #[test]
    fn parses_codexbar_compatibility_window_names_and_allows_missing_optional_windows() {
        let value = serde_json::json!({
            "rollingUsage": { "usagePercent": 12.4, "resetInSec": 60 }
        });
        let snapshot = parse_opencode_usage(&value, 100).unwrap();
        assert_eq!(snapshot.rolling.unwrap().used_percent, 12);
        assert!(snapshot.weekly.is_none());
        assert!(snapshot.monthly.is_none());
    }

    #[test]
    fn rejects_missing_rolling_window_and_out_of_range_percent() {
        assert_eq!(
            parse_opencode_usage(&serde_json::json!({"weekly": {"percent": 4}}), 0),
            Err(ProviderError::MalformedResponse)
        );
        assert!(
            parse_opencode_usage(&serde_json::json!({"rolling": {"percent": 101}}), 0).is_err()
        );
    }
}
