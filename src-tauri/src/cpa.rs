//! Read-only CPA quota adapter. Provider OAuth tokens are substituted by CPA,
//! never downloaded. Only normalized quota data crosses the WebView boundary.
use crate::{state::lk, vault};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const VAULT_KEY: &str = "cpa-quota-connection-v1";
const DEFAULT_URL: &str = "http://127.0.0.1:8317";
const MAX_BODY: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    base_url: String,
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    base_url: String,
    configured: bool,
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    label: String,
    remaining_percent: Option<f64>,
    reset_at: Option<String>,
    // "billing" is a spending-cap rollover, not a model rate-limit reset.
    kind: String,
    window_seconds: Option<f64>,
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuota {
    id: String,
    name: String,
    provider: String,
    plan: String,
    windows: Vec<QuotaWindow>,
    error: Option<String>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    configured: bool,
    accounts: Vec<AccountQuota>,
    updated_at: Option<String>,
    error: Option<String>,
    refreshing: bool,
}

#[derive(Default)]
struct Runtime {
    snapshot: QuotaSnapshot,
    last_attempt: Option<Instant>,
    generation: u64,
}
static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();
// Serialize credential changes without holding the snapshot lock during I/O.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());
fn runtime() -> &'static Mutex<Runtime> {
    RUNTIME.get_or_init(|| Mutex::new(Runtime::default()))
}
fn connection() -> Option<Connection> {
    serde_json::from_str(&vault::get(VAULT_KEY)?).ok()
}

fn normalize_url(raw: &str) -> Result<String, String> {
    let mut url = url::Url::parse(raw.trim()).map_err(|_| "invalid_url")?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("https_required".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid_url".into());
    }
    let path = url
        .path()
        .trim_end_matches('/')
        .trim_end_matches("/v0/management")
        .to_string();
    url.set_path(&path);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn client(base: &str) -> Result<ureq::Agent, String> {
    let url = url::Url::parse(base).map_err(|_| "invalid_url")?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    let builder = if local {
        ureq::AgentBuilder::new()
    } else {
        crate::network::download_agent_builder()
    };
    Ok(builder
        .redirects(0)
        .timeout(Duration::from_secs(12))
        .build())
}

fn management(
    agent: &ureq::Agent,
    conn: &Connection,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let request = agent
        .request(
            if body.is_some() { "POST" } else { "GET" },
            &format!("{}/v0/management/{}", conn.base_url, path),
        )
        .set("Authorization", &format!("Bearer {}", conn.token));
    let response = match body {
        Some(body) => request.send_json(body),
        None => request.call(),
    }
    .map_err(|error| match error {
        ureq::Error::Status(code, _) => format!("management_http_{code}"),
        _ => "connection_failed".into(),
    })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "invalid_response")?;
    if bytes.len() as u64 > MAX_BODY {
        return Err("response_too_large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "invalid_response".into())
}

fn inventory(agent: &ureq::Agent, conn: &Connection) -> Result<Vec<Value>, String> {
    management(agent, conn, "auth-files", None)?["files"]
        .as_array()
        .cloned()
        .ok_or("invalid_inventory".into())
}

#[tauri::command]
pub async fn cpa_config() -> Result<ConfigView, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let conn = connection();
        ConfigView {
            base_url: conn
                .as_ref()
                .map(|c| c.base_url.clone())
                .unwrap_or(DEFAULT_URL.into()),
            configured: conn.is_some(),
        }
    })
    .await
    .map_err(|_| "worker_failed".into())
}

#[tauri::command]
pub async fn save_cpa_config(base_url: String, token: String) -> Result<ConfigView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lk(&CONFIG_LOCK);
        let base_url = normalize_url(&base_url)?;
        let token = if token.trim().is_empty() {
            connection()
                .filter(|c| c.base_url == base_url)
                .map(|c| c.token)
                .ok_or("token_required")?
        } else {
            token.trim().to_string()
        };
        if token.len() > 4096 || token.chars().any(char::is_control) {
            return Err("invalid_token".into());
        }
        let conn = Connection {
            base_url: base_url.clone(),
            token,
        };
        inventory(&client(&base_url)?, &conn)?;
        vault::set(
            VAULT_KEY,
            &serde_json::to_string(&conn).map_err(|_| "credential_save_failed")?,
        )
        .map_err(|_| "credential_save_failed")?;
        let mut state = lk(runtime());
        state.generation += 1;
        state.last_attempt = None;
        state.snapshot = QuotaSnapshot {
            configured: true,
            ..Default::default()
        };
        Ok(ConfigView {
            base_url,
            configured: true,
        })
    })
    .await
    .map_err(|_| "worker_failed")?
}

#[tauri::command]
pub async fn disconnect_cpa() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = lk(&CONFIG_LOCK);
        vault::delete(VAULT_KEY).map_err(|_| "credential_delete_failed")?;
        let mut state = lk(runtime());
        state.generation += 1;
        state.last_attempt = None;
        state.snapshot = QuotaSnapshot::default();
        Ok(())
    })
    .await
    .map_err(|_| "worker_failed")?
}

/// Return cached data immediately, refreshing on a worker. Repeated opens never
/// fan out duplicate probes; even manual refresh has a 15-second cooldown.
#[tauri::command]
pub async fn cpa_quota(force: bool) -> Result<QuotaSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lk(&CONFIG_LOCK);
        let Some(conn) = connection() else {
            return QuotaSnapshot::default();
        };
        let mut state = lk(runtime());
        state.snapshot.configured = true;
        let ttl = if force { 15 } else { 120 };
        if !state.snapshot.refreshing
            && state
                .last_attempt
                .map(|t| t.elapsed() >= Duration::from_secs(ttl))
                .unwrap_or(true)
        {
            state.snapshot.refreshing = true;
            state.last_attempt = Some(Instant::now());
            let generation = state.generation;
            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(|| fetch_accounts(&conn))
                    .unwrap_or_else(|_| Err("worker_failed".into()));
                let mut state = lk(runtime());
                if generation != state.generation {
                    return;
                }
                state.snapshot.refreshing = false;
                match result {
                    Ok(accounts) => {
                        state.snapshot.accounts = accounts;
                        state.snapshot.updated_at = Some(Utc::now().to_rfc3339());
                        state.snapshot.error = None;
                    }
                    Err(error) => state.snapshot.error = Some(error),
                }
            });
        }
        state.snapshot.clone()
    })
    .await
    .map_err(|_| "worker_failed".into())
}

fn field<'a>(value: &'a Value, keys: &[&str]) -> &'a Value {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find(|v| !v.is_null())
        .unwrap_or(&Value::Null)
}
fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}
fn reset(value: &Value) -> Option<String> {
    if let Some(n) = number(value).filter(|n| *n > 0.0) {
        let secs = if n > 1e12 { n / 1000.0 } else { n };
        return DateTime::from_timestamp(secs as i64, 0).map(|d| d.to_rfc3339());
    }
    DateTime::parse_from_rfc3339(&text(value))
        .ok()
        .map(|d| d.to_rfc3339())
}
fn object(value: &Value) -> Value {
    if let Some(s) = value.as_str() {
        serde_json::from_str(s).unwrap_or(Value::Null)
    } else {
        value.clone()
    }
}
fn metadata(file: &Value, keys: &[&str]) -> String {
    for source in [
        file.clone(),
        object(&file["id_token"]),
        file["metadata"].clone(),
        file["attributes"].clone(),
        file["oauth"].clone(),
        file["user"].clone(),
    ] {
        let value = text(field(&source, keys));
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}
fn provider(file: &Value) -> String {
    let name = text(field(file, &["type", "provider"])).to_lowercase();
    if name == "xai" || name == "grok" {
        "grok".into()
    } else {
        name
    }
}

fn proxy_call(
    agent: &ureq::Agent,
    conn: &Connection,
    file: &Value,
    url: &str,
    data: Option<Value>,
    mut headers: Value,
) -> Result<Value, String> {
    let index = text(field(file, &["auth_index", "authIndex"]));
    if index.is_empty() {
        return Err("missing_auth_index".into());
    }
    headers["Authorization"] = json!("Bearer $TOKEN$");
    headers["Content-Type"] = json!("application/json");
    let mut payload = json!({"authIndex":index,"method":if data.is_some(){"POST"}else{"GET"},"url":url,"header":headers});
    if let Some(data) = data {
        payload["data"] = json!(data.to_string());
    }
    let response = management(agent, conn, "api-call", Some(payload))?;
    let status = number(&response["status_code"]).ok_or("invalid_response")? as u16;
    if !(200..300).contains(&status) {
        return Err(format!("provider_http_{status}"));
    }
    let body = object(&response["body"]);
    if !body.is_object() {
        return Err("invalid_response".into());
    }
    Ok(body)
}

fn fetch_accounts(conn: &Connection) -> Result<Vec<AccountQuota>, String> {
    let agent = client(&conn.base_url)?;
    let files = inventory(&agent, conn)?;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let output = Mutex::new(Vec::new());
    let deadline = Instant::now() + Duration::from_secs(45);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(file) = files.get(i) else {
                    break;
                };
                let mut account = AccountQuota {
                    id: format!(
                        "{}-{i}",
                        text(field(file, &["auth_index", "authIndex", "name"]))
                    ),
                    name: text(field(file, &["email", "name", "account", "id"])),
                    provider: provider(file),
                    plan: metadata(file, &["plan_type", "planType"]),
                    windows: Vec::new(),
                    error: None,
                };
                let result = if file["disabled"].as_bool() == Some(true) {
                    Err("account_disabled".into())
                } else if !matches!(account.provider.as_str(), "codex" | "antigravity" | "grok") {
                    Err("unsupported_provider".into())
                } else if Instant::now() >= deadline {
                    Err("refresh_budget_exceeded".into())
                } else {
                    fetch_account(&agent, conn, file, &mut account)
                };
                if let Err(error) = result {
                    account.error = Some(error);
                }
                lk(&output).push((i, account));
            });
        }
    });
    let mut result = output.into_inner().unwrap_or_default();
    result.sort_by_key(|(i, _)| *i);
    Ok(result.into_iter().map(|(_, a)| a).collect())
}

fn fetch_account(
    agent: &ureq::Agent,
    conn: &Connection,
    file: &Value,
    account: &mut AccountQuota,
) -> Result<(), String> {
    match account.provider.as_str() {
        "codex" => {
            let mut headers = json!({"User-Agent":"codex-tui/0.149.1"});
            let id = metadata(
                file,
                &[
                    "chatgpt_account_id",
                    "chatgptAccountId",
                    "account_id",
                    "accountId",
                ],
            );
            if !id.is_empty() {
                headers["Chatgpt-Account-Id"] = json!(id);
            }
            let body = proxy_call(
                agent,
                conn,
                file,
                "https://chatgpt.com/backend-api/wham/usage",
                None,
                headers,
            )?;
            let plan = text(field(&body, &["plan_type", "planType"]));
            if !plan.is_empty() {
                account.plan = plan;
            }
            account.windows = codex_windows(&body, Utc::now().timestamp());
        }
        "antigravity" => {
            let headers = json!({"User-Agent":"antigravity/cli/1.0.13 (aidev_client; os_type=windows; arch=amd64)"});
            let mut project =
                metadata(file, &["project_id", "projectId", "gemini_virtual_project"]);
            if project.is_empty() {
                let body = proxy_call(
                    agent,
                    conn,
                    file,
                    "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
                    Some(json!({"metadata":{"ideType":"ANTIGRAVITY"}})),
                    headers.clone(),
                )?;
                project = text(&body["cloudaicompanionProject"]);
                if project.is_empty() {
                    project = text(&body["cloudaicompanionProject"]["id"]);
                }
            }
            if project.is_empty() {
                return Err("missing_project_id".into());
            }
            let mut last_error = "no_quota_data".to_string();
            for host in [
                "daily-cloudcode-pa.googleapis.com",
                "cloudcode-pa.googleapis.com",
            ] {
                match proxy_call(
                    agent,
                    conn,
                    file,
                    &format!("https://{host}/v1internal:retrieveUserQuotaSummary"),
                    Some(json!({"project":project})),
                    headers.clone(),
                ) {
                    Ok(body) => {
                        account.windows = antigravity_windows(&body);
                        if !account.windows.is_empty() {
                            break;
                        }
                    }
                    Err(error) => last_error = error,
                }
            }
            // Older CPA/Antigravity releases expose per-model quotaInfo instead.
            if account.windows.is_empty() {
                match proxy_call(
                    agent,
                    conn,
                    file,
                    "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
                    Some(json!({"project":project})),
                    headers,
                ) {
                    Ok(body) => account.windows = antigravity_windows(&body),
                    Err(error) => last_error = error,
                }
            }
            if account.windows.is_empty() {
                return Err(last_error);
            }
        }
        "grok" => {
            let mut headers = json!({"x-xai-token-auth":"xai-grok-cli","x-grok-client-version":"0.2.91","User-Agent":"grok-pager/0.2.91 grok-shell/0.2.91 (windows; x86_64)"});
            let id = metadata(file, &["sub", "subject", "user_id", "userId"]);
            if !id.is_empty() {
                headers["x-userid"] = json!(id);
            }
            let mut errors = Vec::new();
            for (url, monthly) in [
                (
                    "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
                    false,
                ),
                ("https://cli-chat-proxy.grok.com/v1/billing", true),
            ] {
                match proxy_call(agent, conn, file, url, None, headers.clone()) {
                    Ok(body) => account.windows.extend(grok_windows(&body, monthly)),
                    Err(error) => errors.push(error),
                }
            }
            if !errors.is_empty() {
                account.error = Some(format!("grok_billing_unavailable:{}", errors.join(",")));
            }
            // Do not perform a paid chat/ping as a quota fallback: availability
            // is not remaining quota, and that probe can incur a charge.
        }
        _ => return Err("unsupported_provider".into()),
    }
    if account.windows.is_empty() && account.error.is_none() {
        return Err("no_quota_data".into());
    }
    Ok(())
}

fn codex_windows(body: &Value, now: i64) -> Vec<QuotaWindow> {
    let mut windows = Vec::new();
    let mut add = |label: &str, rate: &Value| {
        for keys in [
            ["primary_window", "primaryWindow"],
            ["secondary_window", "secondaryWindow"],
        ] {
            let window = field(rate, &keys);
            if !window.is_object() {
                continue;
            }
            let used = number(field(window, &["used_percent", "usedPercent"]));
            let reset_at = reset(field(window, &["reset_at", "resetAt"])).or_else(|| {
                let delta = number(field(window, &["reset_after_seconds", "resetAfterSeconds"]))?;
                if !(0.0..=31_536_000.0).contains(&delta) {
                    return None;
                }
                reset(&json!(now + delta as i64))
            });
            windows.push(QuotaWindow {
                label: label.into(),
                kind: "quota".into(),
                // used_percent is already 0..100; 0.5 means half a percent.
                remaining_percent: used.map(|n| (100.0 - n).clamp(0.0, 100.0)),
                reset_at,
                window_seconds: number(field(
                    window,
                    &["limit_window_seconds", "limitWindowSeconds"],
                )),
            });
        }
    };
    add("Codex · shared", field(body, &["rate_limit", "rateLimit"]));
    add(
        "Code review",
        field(body, &["code_review_rate_limit", "codeReviewRateLimit"]),
    );
    if let Some(limits) =
        field(body, &["additional_rate_limits", "additionalRateLimits"]).as_array()
    {
        for (i, limit) in limits.iter().enumerate() {
            let name = text(field(
                limit,
                &[
                    "limit_name",
                    "limitName",
                    "metered_feature",
                    "meteredFeature",
                ],
            ));
            add(
                &if name.is_empty() {
                    format!("Additional {}", i + 1)
                } else {
                    name
                },
                field(limit, &["rate_limit", "rateLimit"]),
            );
        }
    }
    windows
}

fn antigravity_windows(body: &Value) -> Vec<QuotaWindow> {
    let mut windows = Vec::new();
    if let Some(groups) = body["groups"].as_array() {
        for group in groups {
            if let Some(buckets) = group["buckets"].as_array() {
                for bucket in buckets {
                    let window = text(&bucket["window"]);
                    windows.push(QuotaWindow {
                        label: format!("Antigravity · quota {}", windows.len() + 1),
                        kind: "quota".into(),
                        remaining_percent: number(field(
                            bucket,
                            &["remainingFraction", "remaining_fraction"],
                        ))
                        .map(|n| (n * 100.0).clamp(0.0, 100.0)),
                        reset_at: reset(field(bucket, &["resetTime", "reset_time"])),
                        window_seconds: match window.as_str() {
                            "5h" | "five-hour" | "five_hour" => Some(18000.0),
                            "weekly" | "week" => Some(604800.0),
                            _ => None,
                        },
                    });
                }
            }
        }
    } else if let Some(models) = body["models"].as_object() {
        for model in models.values() {
            let quota = &model["quotaInfo"];
            if !quota.is_object() {
                continue;
            }
            windows.push(QuotaWindow {
                label: format!("Antigravity · quota {}", windows.len() + 1),
                kind: "quota".into(),
                remaining_percent: number(&quota["remainingFraction"])
                    .map(|n| (n * 100.0).clamp(0.0, 100.0)),
                reset_at: reset(&quota["resetTime"]),
                window_seconds: None,
            });
        }
    }
    windows
}

fn grok_windows(body: &Value, monthly: bool) -> Vec<QuotaWindow> {
    let config = &body["config"];
    if !config.is_object() {
        return Vec::new();
    }
    let mut windows = Vec::new();
    if monthly {
        let cents = |v: &Value| number(v).or_else(|| number(&v["val"]));
        let cap = cents(field(config, &["monthlyLimit", "monthly_limit"]));
        let used = cents(&config["used"]);
        let end = reset(field(config, &["billingPeriodEnd", "billing_period_end"]));
        if cap.is_some() || used.is_some() || end.is_some() {
            windows.push(QuotaWindow {
                label: "Grok · monthly spending cap".into(),
                kind: "billing".into(),
                remaining_percent: cap
                    .zip(used)
                    .filter(|(c, _)| *c > 0.0)
                    .map(|(c, u)| ((c - u) / c * 100.0).clamp(0.0, 100.0)),
                reset_at: end,
                window_seconds: None,
            });
        }
    } else {
        let period = field(config, &["currentPeriod", "current_period"]);
        let end = reset(&period["end"]);
        let used = number(field(
            config,
            &["creditUsagePercent", "credit_usage_percent"],
        ));
        if used.is_some() || period.is_object() {
            windows.push(QuotaWindow {
                label: "Grok · shared credits".into(),
                kind: "quota".into(),
                remaining_percent: used.map(|u| (100.0 - u).clamp(0.0, 100.0)),
                reset_at: end.clone(),
                window_seconds: None,
            });
        }
        // Product usage is a share of total spending, not a per-model capacity.
        // Do not turn productUsage percentages into fictitious remaining quotas.
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    #[test]
    fn validates_destination_and_normalizes_management_suffix() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8317/v0/management/").unwrap(),
            DEFAULT_URL
        );
        assert_eq!(
            normalize_url("https://example.com/cpa/").unwrap(),
            "https://example.com/cpa"
        );
        for url in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com?key=x",
            "file:///tmp/foo",
        ] {
            assert!(normalize_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn codex_percentage_units_missing_values_and_windows_are_preserved() {
        let body = json!({"rate_limit":{"primary_window":{"used_percent":0.5,"limit_window_seconds":18000,"reset_after_seconds":60},"secondary_window":{"reset_at":2000000000}},
            "additional_rate_limits":[{"limit_name":"gpt-spark","rate_limit":{"primary_window":{"used_percent":"100","reset_at":"2030-01-01T00:00:00Z"}}}]});
        let windows = codex_windows(&body, 1900000000);
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].remaining_percent, Some(99.5));
        assert_eq!(windows[0].window_seconds, Some(18000.0));
        assert_eq!(windows[0].reset_at, reset(&json!(1900000060)));
        assert_eq!(windows[1].remaining_percent, None);
        assert_eq!(windows[2].remaining_percent, Some(0.0));
        assert_eq!(windows[2].label, "gpt-spark");
        assert_eq!(reset(&json!(2000000000000_i64)), reset(&json!(2000000000)));
        assert!(reset(&json!("not a timestamp")).is_none());
    }

    #[test]
    fn antigravity_supports_group_and_legacy_model_responses() {
        let grouped = antigravity_windows(
            &json!({"groups":[{"displayName":"Claude","buckets":[{"window":"5h","remainingFraction":0.25,"resetTime":"2030-01-01T00:00:00Z"},{"window":"weekly","remainingFraction":0}]}]}),
        );
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].label, "Antigravity · quota 1");
        assert_eq!(grouped[1].label, "Antigravity · quota 2");
        assert_eq!(grouped[0].remaining_percent, Some(25.0));
        assert_eq!(grouped[1].remaining_percent, Some(0.0));
        let legacy = antigravity_windows(
            &json!({"models":{"gemini-pro":{"quotaInfo":{"remainingFraction":0.8}},"claude-sonnet":{"quotaInfo":{"resetTime":"2030-01-01T00:00:00Z"}}}}),
        );
        assert_eq!(legacy.len(), 2);
        assert!(legacy.iter().any(|w| w.remaining_percent == Some(80.0)));
        assert!(legacy.iter().any(|w| w.remaining_percent.is_none()));
        assert!(legacy
            .iter()
            .all(|w| w.label.starts_with("Antigravity · quota ")));
    }

    #[test]
    fn grok_separates_billing_rollover_and_does_not_invent_model_quota() {
        let body = json!({"config":{"creditUsagePercent":30,"currentPeriod":{"type":"weekly","end":"2030-01-01T00:00:00Z"},"productUsage":[{"product":"grok-4","usagePercent":80}],"monthlyLimit":{"val":"10000"},"used":{"val":"2500"},"billingPeriodEnd":"2030-02-01T00:00:00Z"}});
        let quota = grok_windows(&body, false);
        let billing = grok_windows(&body, true);
        assert_eq!(quota.len(), 1);
        assert_eq!(quota[0].remaining_percent, Some(70.0));
        assert_eq!(billing[0].remaining_percent, Some(75.0));
        assert_eq!(billing[0].kind, "billing");
        assert_ne!(quota[0].reset_at, billing[0].reset_at);
        assert!(grok_windows(&json!({"config":{}}), false).is_empty());
    }

    fn mock_server(
        replies: Vec<(u16, Value)>,
    ) -> (Connection, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let conn = Connection {
            base_url: format!("http://{}", listener.local_addr().unwrap()),
            token: "test-management-key".into(),
        };
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut request = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.to_lowercase().starts_with("content-length:") {
                        length = line
                            .split(':')
                            .nth(1)
                            .unwrap()
                            .trim()
                            .parse::<usize>()
                            .unwrap();
                    }
                    request.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).unwrap();
                request.push_str(&String::from_utf8(bytes).unwrap());
                requests.push(request);
                let body = body.to_string();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }
            requests
        });
        (conn, worker)
    }

    #[test]
    fn management_proxy_contract_and_token_substitution_roundtrip() {
        let usage = json!({"plan_type":"plus","rate_limit":{"primary_window":{"used_percent":42,"reset_at":2000000000}}});
        let (conn, server) = mock_server(vec![
            (
                200,
                json!({"files":[{"type":"codex","auth_index":"account-1","email":"fixture@example.test","id_token":{"chatgpt_account_id":"workspace-1"}}]}),
            ),
            (200, json!({"status_code":200,"body":usage.to_string()})),
        ]);
        let result = fetch_accounts(&conn).unwrap();
        assert_eq!(result[0].windows[0].remaining_percent, Some(58.0));
        assert_eq!(result[0].plan, "plus");
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /v0/management/auth-files"));
        assert!(requests[1].starts_with("POST /v0/management/api-call"));
        assert!(requests[1].contains("Bearer $TOKEN$"));
        assert!(requests[1].contains("workspace-1"));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("test-management-key"));
    }

    #[test]
    fn errors_redact_upstream_bodies_and_preserve_unknown_quotas() {
        let (conn, server) = mock_server(vec![(401, json!({"error":"sensitive upstream text"}))]);
        assert_eq!(
            inventory(&client(&conn.base_url).unwrap(), &conn).unwrap_err(),
            "management_http_401"
        );
        server.join().unwrap();
        let (conn, server) = mock_server(vec![
            (
                200,
                json!({"files":[{"type":"codex","auth_index":"1"},{"type":"grok","disabled":true}]}),
            ),
            (
                200,
                json!({"status_code":401,"body":"private provider data"}),
            ),
        ]);
        let result = fetch_accounts(&conn).unwrap();
        assert_eq!(result[0].error.as_deref(), Some("provider_http_401"));
        assert!(result[0].windows.is_empty());
        assert_eq!(result[1].error.as_deref(), Some("account_disabled"));
        server.join().unwrap();
    }
}
