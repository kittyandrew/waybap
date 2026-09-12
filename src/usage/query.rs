use chrono::Utc;
use core::time::Duration;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::fs::read_to_string;

use crate::scheduler::get_cache_fp;

struct OAuthCredentials {
    access_token: String,
    expires_at: Option<u64>,    // ms since epoch (Claude only; Codex has no expiry field)
    account_id: Option<String>, // Codex only, for ChatGPT-Account-Id header
}

fn load_claude_credentials() -> Option<OAuthCredentials> {
    let home = std::env::var("HOME").ok()?;
    let text = read_to_string(format!("{home}/.claude/.credentials.json")).ok()?;
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse Claude credentials: {e}");
            return None;
        }
    };
    let oauth = &v["claudeAiOauth"];
    let access_token = oauth["accessToken"].as_str()?.to_string();
    let expires_at = oauth["expiresAt"].as_u64();
    Some(OAuthCredentials { access_token, expires_at, account_id: None })
}

fn load_codex_credentials() -> Option<OAuthCredentials> {
    let home = std::env::var("HOME").ok()?;
    let text = read_to_string(format!("{home}/.codex/auth.json")).ok()?;
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse Codex credentials: {e}");
            return None;
        }
    };
    let tokens = &v["tokens"];
    let access_token = tokens["access_token"].as_str()?.to_string();
    let account_id = tokens["account_id"].as_str().map(String::from);
    Some(OAuthCredentials { access_token, expires_at: None, account_id })
}

fn is_token_expired(creds: &OAuthCredentials) -> bool {
    match creds.expires_at {
        Some(expires_ms) => Utc::now().timestamp_millis().max(0) as u64 >= expires_ms,
        None => false, // Codex has no expiry field, so let the API decide if the token is valid.
    }
}

/// Check if a CLI binary exists on PATH without spawning a subprocess.
fn cli_on_path(name: &str) -> bool {
    let path_var = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) => return false,
    };
    std::env::split_paths(&path_var).any(|dir| dir.join(name).is_file())
}

enum FetchResult {
    Ok(Value),
    TokenExpired, // API 401/403: expiry, revocation, or disabled access
    Failed,       // network error, 5xx, etc.
}

fn fetch_usage(req: reqwest::blocking::RequestBuilder, label: &str) -> FetchResult {
    let response = match req.send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{label} usage API request failed: {e}");
            return FetchResult::Failed;
        }
    };
    let status = response.status();
    if matches!(status.as_u16(), 401 | 403) {
        eprintln!("{label} usage API returned {status} — token expired or access revoked");
        return FetchResult::TokenExpired; // Ask the user to log in again rather than treating this as a temporary outage.
    }
    if !status.is_success() {
        eprintln!("{label} usage API error: HTTP {status}");
        return FetchResult::Failed;
    }
    let text = match response.text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {label} usage response: {e}");
            return FetchResult::Failed;
        }
    };
    match serde_json::from_str(&text) {
        Ok(v) => FetchResult::Ok(v),
        Err(e) => {
            eprintln!("Failed to parse {label} usage JSON: {e}");
            FetchResult::Failed
        }
    }
}

fn fetch_usage_claude(client: &Client, creds: &OAuthCredentials) -> FetchResult {
    let req = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {}", creds.access_token))
        .header("Accept", "application/json")
        // @NOTE: If requests break after an API update, check this OAuth beta header. - Sep 12, 2026
        .header("anthropic-beta", "oauth-2025-04-20");
    fetch_usage(req, "Claude")
}

fn fetch_usage_codex(client: &Client, creds: &OAuthCredentials) -> FetchResult {
    let mut req = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("Authorization", format!("Bearer {}", creds.access_token))
        .header("Accept", "application/json");
    if let Some(ref account_id) = creds.account_id {
        req = req.header("ChatGPT-Account-Id", account_id);
    }
    fetch_usage(req, "Codex")
}

fn fetch_status(client: &Client, url: &str) -> Option<Value> {
    let response = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Status page request failed for {url}: {e}");
            return None;
        }
    };
    if !response.status().is_success() {
        eprintln!("Status page returned HTTP {}: {url}", response.status());
        return None;
    }
    let text = match response.text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read status page response from {url}: {e}");
            return None;
        }
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse status page JSON from {url}: {e}");
            return None;
        }
    };
    Some(json!({"indicator": v["status"]["indicator"], "description": v["status"]["description"]}))
}

fn fetch_provider(
    client: &Client, creds: Option<OAuthCredentials>, cli_name: &str, status_url: &str,
    fetch_fn: fn(&Client, &OAuthCredentials) -> FetchResult,
) -> Value {
    let cli_installed = cli_on_path(cli_name);
    let status = fetch_status(client, status_url); // Keep fetching usage even if the status page is down.
    let has_credentials = creds.is_some();

    let (data, token_expired, data_timestamp) = match creds {
        None => (None, false, None),
        Some(creds) if is_token_expired(&creds) => (None, true, None), // Don't send a request with a token we know has expired.
        Some(creds) => match fetch_fn(client, &creds) {
            FetchResult::Ok(v) => (Some(v), false, Some(Utc::now().to_rfc3339())),
            FetchResult::TokenExpired => (None, true, None),
            FetchResult::Failed => (None, false, None),
        },
    };

    json!({
        "data": data,
        "data_timestamp": data_timestamp,
        "token_expired": token_expired,
        "has_credentials": has_credentials,
        "status": status,
        "cli_installed": cli_installed,
    })
}

pub fn query() -> Option<String> {
    let client = match Client::builder().timeout(Duration::from_secs(10)).user_agent("waybap/0.1.0").build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client for usage: {e}");
            return None;
        }
    };

    let prev_cache: Option<Value> = read_to_string(get_cache_fp("usage")).ok().and_then(|s| serde_json::from_str(&s).ok());

    let claude_creds = load_claude_credentials();
    let claude_status_url = "https://status.anthropic.com/api/v2/status.json";
    let mut claude = fetch_provider(&client, claude_creds, "claude", claude_status_url, fetch_usage_claude);

    let codex_creds = load_codex_credentials();
    let codex_status_url = "https://status.openai.com/api/v2/status.json";
    let mut codex = fetch_provider(&client, codex_creds, "codex", codex_status_url, fetch_usage_codex);

    // Keep the last usage data and its timestamp if a provider's request fails.
    // @NOTE: Don't keep an old token_expired warning after the user logs in again. A network failure also clears
    // that warning, but the old timestamp still shows that the data hasn't been refreshed. - Sep 12, 2026
    for (name, provider) in [("claude", &mut claude), ("codex", &mut codex)] {
        if provider["data"].is_null()
            && provider["has_credentials"] == true
            && let Some(ref prev) = prev_cache
            && !prev[name]["data"].is_null()
        {
            provider["data"] = prev[name]["data"].clone();
            provider["data_timestamp"] = prev[name]["data_timestamp"].clone();
        }
    }

    // Show login errors even if we've never fetched usage successfully.
    let has_any_data = !claude["data"].is_null() || !codex["data"].is_null();
    let has_any_creds = claude["has_credentials"] == true || codex["has_credentials"] == true;
    if has_any_creds && !has_any_data && claude["token_expired"] != true && codex["token_expired"] != true {
        return None; // No usage data or login warning to show, so let the scheduler retry.
    }

    Some(json!({"timestamp": Utc::now().to_rfc3339(), "claude": claude, "codex": codex}).to_string())
}
