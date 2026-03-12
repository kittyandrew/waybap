use chrono::Utc;
use core::time::Duration;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::fs::read_to_string;

use crate::scheduler::get_cache_fp;

struct OAuthCredentials {
    access_token: String,
    expires_at: Option<u64>,    // ms since epoch (Claude only; Codex has no expiry field)
    account_id: Option<String>, // Codex only, for ChatGPT-Account-Id header
}

fn load_claude_credentials() -> Option<OAuthCredentials> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.claude/.credentials.json");
    let text = match read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return None, // Not installed or not logged in
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse Claude credentials: {e}");
            return None;
        }
    };
    // @NOTE: Claude Code nests credentials under "claudeAiOauth" (verified on this machine).
    //   CodexBar's source assumed a flat structure — actual file wraps it.
    let oauth = &v["claudeAiOauth"];
    let access_token = oauth["accessToken"].as_str()?.to_string();
    let expires_at = oauth["expiresAt"].as_u64();
    Some(OAuthCredentials {
        access_token,
        expires_at,
        account_id: None,
    })
}

// @NOTE: Codex nests credentials under a "tokens" key. Verified against actual
//   ~/.codex/auth.json on this Linux machine — format matches CodexBar's macOS source.
fn load_codex_credentials() -> Option<OAuthCredentials> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.codex/auth.json");
    let text = match read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return None,
    };
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
    Some(OAuthCredentials {
        access_token,
        expires_at: None,
        account_id,
    })
}

fn is_token_expired(creds: &OAuthCredentials) -> bool {
    match creds.expires_at {
        Some(expires_ms) => {
            let now_ms = Utc::now().timestamp_millis().max(0) as u64;
            now_ms >= expires_ms
        }
        None => false, // Codex has no expiry field — treat as valid
    }
}

/// Check if a CLI binary exists on PATH without spawning a subprocess.
fn cli_on_path(name: &str) -> bool {
    let path_var = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) => return false,
    };
    std::env::split_paths(&path_var).any(|dir| {
        let path = dir.join(name);
        path.is_file()
    })
}

enum FetchResult {
    Ok(Value),
    TokenExpired, // 401/403 from API — server-side expiry, revocation, or disabled access
    Failed,       // network error, 5xx, etc.
}

/// Send an authenticated GET request and parse JSON response. Shared by Claude and Codex fetchers.
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
        return FetchResult::TokenExpired;
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
        // @WARNING: Beta header will eventually break when Anthropic graduates OAuth out of beta.
        //   Monitor for 4xx errors or missing data after Anthropic API updates.
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
    Some(json!({
        "indicator": v["status"]["indicator"],
        "description": v["status"]["description"],
    }))
}

/// Fetch usage data + status for a single provider. Returns a JSON object with
/// `data`, `token_expired`, `has_credentials`, `status`, `cli_installed` fields.
fn fetch_provider(
    client: &Client,
    creds: Option<OAuthCredentials>,
    cli_name: &str,
    status_url: &str,
    fetch_fn: fn(&Client, &OAuthCredentials) -> FetchResult,
) -> Value {
    let cli_installed = cli_on_path(cli_name);
    let status = fetch_status(client, status_url);
    let has_credentials = creds.is_some();

    let creds = match creds {
        Some(c) => c,
        None => {
            return json!({
                "data": null,
                "data_timestamp": null,
                "token_expired": false,
                "has_credentials": false,
                "status": status,
                "cli_installed": cli_installed,
            });
        }
    };

    let locally_expired = is_token_expired(&creds);
    let (data, token_expired, data_timestamp) = if locally_expired {
        (None::<Value>, true, None::<String>)
    } else {
        match fetch_fn(client, &creds) {
            FetchResult::Ok(v) => (Some(v), false, Some(Utc::now().to_rfc3339())),
            FetchResult::TokenExpired => (None, true, None),
            FetchResult::Failed => (None, false, None),
        }
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
    let client = match Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("waybap/0.1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client for usage: {e}");
            return None;
        }
    };

    // @NOTE: Read existing cache for partial failure carry-forward (D14).
    //   Novel pattern — no other module's query() reads its own cache.
    //   Thread-safe because scheduler runs query() → write sequentially within tick().
    let prev_cache: Option<Value> = read_to_string(get_cache_fp("usage"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let claude_creds = load_claude_credentials();
    let codex_creds = load_codex_credentials();
    let has_claude_creds = claude_creds.is_some();
    let has_codex_creds = codex_creds.is_some();

    let mut claude = fetch_provider(
        &client,
        claude_creds,
        "claude",
        "https://status.anthropic.com/api/v2/status.json",
        fetch_usage_claude,
    );
    let mut codex = fetch_provider(
        &client,
        codex_creds,
        "codex",
        "https://status.openai.com/api/v2/status.json",
        fetch_usage_codex,
    );

    // Carry forward stale data + data_timestamp on partial failure (D14).
    // @NOTE: token_expired is NOT carried forward — if a server-side revocation (401/403)
    //   is followed by a network failure, the stale data reappears without the "token expired"
    //   warning until the next successful fetch or local expiry check. Accepted trade-off:
    //   self-heals within 120s and avoids complexity of merging token states.
    if claude["data"].is_null() && has_claude_creds {
        if let Some(ref prev) = prev_cache {
            if !prev["claude"]["data"].is_null() {
                claude["data"] = prev["claude"]["data"].clone();
                claude["data_timestamp"] = prev["claude"]["data_timestamp"].clone();
            }
        }
    }
    if codex["data"].is_null() && has_codex_creds {
        if let Some(ref prev) = prev_cache {
            if !prev["codex"]["data"].is_null() {
                codex["data"] = prev["codex"]["data"].clone();
                codex["data_timestamp"] = prev["codex"]["data_timestamp"].clone();
            }
        }
    }

    // @NOTE: Return Some() even when both providers are unconfigured — parsing handles
    //   the "not configured" display. None is only for transient failures worth retrying.
    let has_any_data = !claude["data"].is_null() || !codex["data"].is_null();
    let has_any_creds = has_claude_creds || has_codex_creds;
    if has_any_creds && !has_any_data {
        return None;
    }

    let result = json!({
        "timestamp": Utc::now().to_rfc3339(),
        "claude": claude,
        "codex": codex,
    });

    Some(result.to_string())
}
