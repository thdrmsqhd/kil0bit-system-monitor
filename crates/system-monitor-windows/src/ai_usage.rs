//! OpenCode Go's authenticated public quota endpoint and deterministic JSON parser.
//! Requests run on a caller-owned worker thread, never in the Win32 window procedure.

use reqwest::blocking::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use system_monitor_core::{ai_retry_delay_seconds, AiUsageSnapshot, AiUsageWindow};

const OPENCODE_GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const DEEPSEEK_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

#[derive(Clone, Debug, PartialEq)]
pub struct CurrencyBalance {
    pub currency: String,
    pub total: f64,
    pub granted: f64,
    pub topped_up: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeepSeekBalanceSnapshot {
    pub is_available: bool,
    pub balances: Vec<CurrencyBalance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingKey,
    Unauthorized,
    RateLimited,
    HttpStatus(u16),
    Network,
    MalformedResponse,
}

#[derive(Clone, Debug, Default)]
pub struct ProviderEvent {
    pub snapshot: Option<AiUsageSnapshot>,
    pub error: Option<ProviderError>,
    pub consecutive_failures: u32,
}

/// Dedicated, single-flight polling worker. HTTP requests are bounded to 15 seconds.
pub struct AiUsageWorker {
    stop: Arc<AtomicBool>,
    interval_seconds: Arc<AtomicU32>,
    thread: Option<JoinHandle<()>>,
}

impl AiUsageWorker {
    pub fn start(
        interval_seconds: u32,
        store: crate::secret_store::SecretStore,
        on_event: impl Fn(ProviderEvent) + Send + Sync + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let interval_seconds = Arc::new(AtomicU32::new(interval_seconds.clamp(60, 3600)));
        let worker_stop = Arc::clone(&stop);
        let worker_interval = Arc::clone(&interval_seconds);
        let thread = thread::spawn(move || {
            let mut last_good: Option<AiUsageSnapshot> = None;
            let mut failures = 0_u32;
            while !worker_stop.load(Ordering::Acquire) {
                let result = store
                    .load()
                    .map_err(|_| ProviderError::MissingKey)
                    .and_then(|key| fetch_opencode_usage(&key));
                let error = result.as_ref().err().cloned();
                match result {
                    Ok(snapshot) => {
                        last_good = Some(snapshot);
                        failures = 0;
                    }
                    Err(_) => {
                        failures = failures.saturating_add(1);
                        if let Some(snapshot) = &mut last_good {
                            snapshot.stale = true;
                        }
                    }
                }
                on_event(ProviderEvent {
                    snapshot: last_good.clone(),
                    error,
                    consecutive_failures: failures,
                });
                let delay =
                    ai_retry_delay_seconds(worker_interval.load(Ordering::Relaxed), failures);
                thread::park_timeout(Duration::from_secs(delay as u64));
            }
        });
        Self {
            stop,
            interval_seconds,
            thread: Some(thread),
        }
    }

    pub fn wake(&self) {
        if let Some(thread) = &self.thread {
            thread.thread().unpark();
        }
    }

    pub fn set_interval(&self, seconds: u32) {
        self.interval_seconds
            .store(seconds.clamp(60, 3600), Ordering::Relaxed);
        self.wake();
    }

    /// Request shutdown without blocking the UI on an in-flight, bounded HTTP call.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
        }
    }
}

impl Drop for AiUsageWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
        }
    }
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

/// Read the Codex CLI token without changing the file or refreshing credentials.
pub fn read_codex_access_token(
    path: &std::path::Path,
) -> Result<zeroize::Zeroizing<String>, ProviderError> {
    let bytes = std::fs::read(path).map_err(|_| ProviderError::MissingKey)?;
    let json: Value =
        serde_json::from_slice(&bytes).map_err(|_| ProviderError::MalformedResponse)?;
    let token = json
        .pointer("/tokens/access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.trim().is_empty())
        .ok_or(ProviderError::MissingKey)?;
    Ok(zeroize::Zeroizing::new(token.to_owned()))
}

pub fn codex_auth_path() -> Result<std::path::PathBuf, ProviderError> {
    let profile = std::env::var_os("USERPROFILE").ok_or(ProviderError::MissingKey)?;
    Ok(std::path::PathBuf::from(profile)
        .join(".codex")
        .join("auth.json"))
}

pub fn parse_codex_usage(value: &Value, now: u64) -> Result<AiUsageSnapshot, ProviderError> {
    let rate = value
        .get("rate_limit")
        .ok_or(ProviderError::MalformedResponse)?;
    let rolling = parse_codex_window(
        rate.get("primary_window")
            .ok_or(ProviderError::MalformedResponse)?,
        now,
    )?;
    let weekly = rate
        .get("secondary_window")
        .map(|v| parse_codex_window(v, now))
        .transpose()?;
    Ok(AiUsageSnapshot {
        rolling: Some(rolling),
        weekly,
        monthly: None,
        updated_at_epoch_seconds: now,
        stale: false,
    })
}

fn parse_codex_window(value: &Value, now: u64) -> Result<AiUsageWindow, ProviderError> {
    let used = value
        .get("used_percent")
        .and_then(Value::as_f64)
        .ok_or(ProviderError::MalformedResponse)?;
    if !(0.0..=100.0).contains(&used) {
        return Err(ProviderError::MalformedResponse);
    }
    let reset = value.get("reset_at").and_then(Value::as_u64).or_else(|| {
        value
            .get("reset_in_seconds")
            .and_then(Value::as_u64)
            .map(|s| now.saturating_add(s))
    });
    Ok(AiUsageWindow {
        used_percent: used.round() as u8,
        reset_at_epoch_seconds: reset,
    })
}

pub fn fetch_codex_usage(token: &str) -> Result<AiUsageSnapshot, ProviderError> {
    if token.trim().is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| ProviderError::Network)?;
    let response = client
        .get(CODEX_USAGE_URL)
        .bearer_auth(token)
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
    parse_codex_usage(&value, now)
}

pub fn parse_deepseek_balance(value: &Value) -> Result<DeepSeekBalanceSnapshot, ProviderError> {
    let is_available = value
        .get("is_available")
        .and_then(Value::as_bool)
        .ok_or(ProviderError::MalformedResponse)?;
    let infos = value
        .get("balance_infos")
        .and_then(Value::as_array)
        .ok_or(ProviderError::MalformedResponse)?;
    let mut balances = Vec::with_capacity(infos.len());
    for item in infos {
        let currency = item
            .get("currency")
            .and_then(Value::as_str)
            .ok_or(ProviderError::MalformedResponse)?;
        let amount = |key: &str| -> Result<f64, ProviderError> {
            item.get(key)
                .and_then(|v| {
                    v.as_str()
                        .and_then(|s| s.parse().ok())
                        .or_else(|| v.as_f64())
                })
                .ok_or(ProviderError::MalformedResponse)
        };
        balances.push(CurrencyBalance {
            currency: currency.to_owned(),
            total: amount("total_balance")?,
            granted: amount("granted_balance")?,
            topped_up: amount("topped_up_balance")?,
        });
    }
    Ok(DeepSeekBalanceSnapshot {
        is_available,
        balances,
    })
}

pub fn fetch_deepseek_balance(api_key: &str) -> Result<DeepSeekBalanceSnapshot, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| ProviderError::Network)?;
    let response = client
        .get(DEEPSEEK_BALANCE_URL)
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
    parse_deepseek_balance(&value)
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

    #[test]
    fn parses_codex_primary_and_secondary_windows() {
        let value = serde_json::json!({"rate_limit":{"primary_window":{"used_percent":41,"reset_at":200},"secondary_window":{"used_percent":13,"reset_in_seconds":60}}});
        let snapshot = parse_codex_usage(&value, 100).unwrap();
        assert_eq!(snapshot.rolling.unwrap().used_percent, 41);
        assert_eq!(snapshot.weekly.unwrap().reset_at_epoch_seconds, Some(160));
    }

    #[test]
    fn reads_codex_token_without_modifying_auth_file() {
        let path =
            std::env::temp_dir().join(format!("codex-auth-readonly-{}.json", std::process::id()));
        let source = br#"{"tokens":{"access_token":"fixture-token"}}"#;
        std::fs::write(&path, source).unwrap();
        let token = read_codex_access_token(&path).unwrap();
        assert_eq!(&*token, "fixture-token");
        assert_eq!(std::fs::read(&path).unwrap(), source);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn parses_deepseek_currency_balances() {
        let value = serde_json::json!({"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"12.5","granted_balance":"10","topped_up_balance":"2.5"}]});
        let balance = parse_deepseek_balance(&value).unwrap();
        assert!(balance.is_available);
        assert_eq!(balance.balances[0].total, 12.5);
        assert_eq!(balance.balances[0].topped_up, 2.5);
    }
}
