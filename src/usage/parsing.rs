use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::pango;

struct RateWindow {
    used_percent: f64,
    resets_at: Option<String>, // ISO 8601 UTC
}

enum Credits {
    ClaudeExtra { used_usd: f64, limit_usd: f64 },
    CodexBalance { balance_usd: f64 },
}

struct ProviderStatus {
    indicator: String,
    description: String,
}

struct ProviderUsage {
    session: Option<RateWindow>,
    weekly: Option<RateWindow>,
    model_weekly: Vec<(String, RateWindow)>,
    credits: Option<Credits>,
    status: Option<ProviderStatus>,
    plan: Option<String>,           // Codex only; Claude API has no plan field
    data_timestamp: Option<String>, // ISO 8601 UTC — when this provider's data was last fetched
    token_expired: bool,
    has_credentials: bool,
    cli_installed: bool,
}

fn parse_status(status: &Value) -> Option<ProviderStatus> {
    let indicator = status["indicator"].as_str()?.to_string();
    let description = status["description"].as_str()?.to_string();
    Some(ProviderStatus { indicator, description })
}

// @NOTE: Actual API uses snake_case field names, not camelCase as CodexBar's Swift source
//   suggested. Verified against live API responses on Mar 8, 2026.
fn parse_claude_entry(provider: &Value) -> ProviderUsage {
    let data = &provider["data"];
    let token_expired = provider["token_expired"].as_bool().unwrap_or(false);
    let has_credentials = provider["has_credentials"].as_bool().unwrap_or(false);
    let cli_installed = provider["cli_installed"].as_bool().unwrap_or(false);
    let data_timestamp = provider["data_timestamp"].as_str().map(String::from);
    let status = parse_status(&provider["status"]);

    let session = data["five_hour"].as_object().map(|w| RateWindow {
        used_percent: w.get("utilization").and_then(|v| v.as_f64()).unwrap_or(0.0),
        resets_at: w.get("resets_at").and_then(|v| v.as_str()).map(String::from),
    });

    let weekly = data["seven_day"].as_object().map(|w| RateWindow {
        used_percent: w.get("utilization").and_then(|v| v.as_f64()).unwrap_or(0.0),
        resets_at: w.get("resets_at").and_then(|v| v.as_str()).map(String::from),
    });

    // @NOTE: Model-specific weekly windows — iterate keys matching seven_day_* that
    //   aren't "seven_day" itself. Strip "seven_day_" prefix for display label.
    //   e.g. "seven_day_sonnet" → "Sonnet", "seven_day_opus" → "Opus"
    let mut model_weekly = Vec::new();
    if let Some(obj) = data.as_object() {
        for (key, val) in obj {
            if key.starts_with("seven_day_") && !val.is_null() {
                let model_name = key.strip_prefix("seven_day_").unwrap_or(key);
                if let Some(w) = val.as_object() {
                    // @NOTE: Split on _ and capitalize each word for multi-word model names
                    //   e.g. "oauth_apps" → "Oauth Apps". Pango-escaped at render time (not here)
                    //   to avoid breaking meter label padding calculation.
                    let display_name = model_name
                        .split('_')
                        .map(pango::capitalize)
                        .collect::<Vec<_>>()
                        .join(" ");
                    model_weekly.push((
                        display_name,
                        RateWindow {
                            used_percent: w.get("utilization").and_then(|v| v.as_f64()).unwrap_or(0.0),
                            resets_at: w.get("resets_at").and_then(|v| v.as_str()).map(String::from),
                        },
                    ));
                }
            }
        }
    }

    // Claude Extra credits — API returns cents, normalize to USD
    let credits = data["extra_usage"].as_object().and_then(|extra| {
        let is_enabled = extra.get("is_enabled")?.as_bool()?;
        if !is_enabled {
            return None;
        }
        let used_cents = extra.get("used_credits")?.as_f64()?;
        let limit_cents = extra.get("monthly_limit")?.as_f64()?;
        Some(Credits::ClaudeExtra {
            used_usd: used_cents / 100.0,
            limit_usd: limit_cents / 100.0,
        })
    });

    ProviderUsage {
        session,
        weekly,
        model_weekly,
        credits,
        status,
        plan: None,
        data_timestamp,
        token_expired,
        has_credentials,
        cli_installed,
    }
}

fn parse_codex_entry(provider: &Value) -> ProviderUsage {
    let data = &provider["data"];
    let token_expired = provider["token_expired"].as_bool().unwrap_or(false);
    let has_credentials = provider["has_credentials"].as_bool().unwrap_or(false);
    let cli_installed = provider["cli_installed"].as_bool().unwrap_or(false);
    let data_timestamp = provider["data_timestamp"].as_str().map(String::from);
    let status = parse_status(&provider["status"]);

    let rate_limit = &data["rate_limit"];

    let session = rate_limit["primary_window"].as_object().map(|w| RateWindow {
        used_percent: w.get("used_percent").and_then(|v| v.as_f64()).unwrap_or(0.0),
        // @NOTE: Codex uses unix seconds — convert to ISO 8601 for consistent countdown handling
        resets_at: w
            .get("reset_at")
            .and_then(|v| v.as_i64())
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.to_rfc3339()),
    });

    let weekly = rate_limit["secondary_window"].as_object().map(|w| RateWindow {
        used_percent: w.get("used_percent").and_then(|v| v.as_f64()).unwrap_or(0.0),
        resets_at: w
            .get("reset_at")
            .and_then(|v| v.as_i64())
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.to_rfc3339()),
    });

    let credits = data["credits"].as_object().and_then(|c| {
        let has_credits = c.get("has_credits")?.as_bool()?;
        if !has_credits {
            return None;
        }
        // @NOTE: Codex API returns balance as a string, not a number
        let balance = c
            .get("balance")?
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| c.get("balance")?.as_f64())?;
        Some(Credits::CodexBalance { balance_usd: balance })
    });

    let plan = data["plan_type"]
        .as_str()
        .map(|s| s.split('_').map(pango::capitalize).collect::<Vec<_>>().join(" "));

    ProviderUsage {
        session,
        weekly,
        model_weekly: Vec::new(),
        credits,
        status,
        plan,
        data_timestamp,
        token_expired,
        has_credentials,
        cli_installed,
    }
}

// --- Formatting helpers ---

fn usage_color(used_percent: f64) -> &'static str {
    let remaining = 100.0 - used_percent;
    if remaining > 50.0 {
        "#a6d189" // green
    } else if remaining > 25.0 {
        "#e5c890" // yellow
    } else if remaining > 10.0 {
        "#ef9f76" // peach
    } else {
        "#e78284" // red
    }
}

fn format_countdown(resets_at: &Option<String>) -> String {
    let resets_at = match resets_at {
        Some(s) => s,
        None => return String::new(),
    };
    let reset_time = match DateTime::parse_from_rfc3339(resets_at) {
        Ok(dt) => dt,
        Err(_) => return String::new(),
    };
    let diff = reset_time.signed_duration_since(Utc::now());
    let total_secs = diff.num_seconds();
    if total_secs <= 0 {
        return "<span foreground=\"#949cbb\">resetting...</span>".to_string();
    }
    let total_mins = diff.num_minutes();
    let total_hours = diff.num_hours();
    let total_days = diff.num_days();

    if total_mins < 1 {
        format!("resets in {total_secs}s")
    } else if total_hours < 1 {
        format!("resets in {total_mins}m")
    } else if total_hours < 24 {
        let mins = total_mins - total_hours * 60;
        format!("resets in {total_hours}h {mins}m")
    } else {
        let hours = total_hours - total_days * 24;
        format!("resets in {total_days}d {hours}h")
    }
}

/// Convert non-negative seconds to a human-readable age string: "just now", "42s ago", "5m ago", etc.
fn format_age_text(secs: i64) -> String {
    if secs == 0 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Format data age suffix for "Last data" line: " (12m ago)" or empty if unknown.
fn format_data_age(data_timestamp: &Option<String>) -> String {
    let ts = match data_timestamp {
        Some(s) => s,
        None => return String::new(),
    };
    let dt = match DateTime::parse_from_rfc3339(ts) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return String::new(),
    };
    let secs = Utc::now().signed_duration_since(dt).num_seconds().max(0);
    format!(" ({})", format_age_text(secs))
}

fn format_meter_line(label: &str, window: &RateWindow, pad_to: usize) -> String {
    let color = usage_color(window.used_percent);
    let bar = pango::meter_bar(window.used_percent, 10, color, "#949cbb");
    let countdown = format_countdown(&window.resets_at);
    let pct = format!("{:.0}%", window.used_percent.clamp(0.0, 100.0));
    // @NOTE: Pad first (using visual char width), then escape for Pango safety.
    //   Pango renders escaped entities (e.g. &amp;) as single chars, so padding
    //   on the unescaped string gives correct visual alignment.
    let padded_label = format!("{label:pad_to$}");
    let escaped_label = pango::escape(&padded_label);
    format!("{escaped_label} {bar}  {pct:>4}  {countdown}")
}

fn format_status_line(status: &ProviderStatus) -> String {
    let desc = pango::escape(&status.description);
    match status.indicator.as_str() {
        "none" => format!("<span foreground=\"#a6d189\">✓ {desc}</span>"),
        "minor" => format!("<span foreground=\"#e5c890\">⚠ {desc}</span>"),
        "major" | "critical" => format!("<span foreground=\"#e78284\">✗ {desc}</span>"),
        "maintenance" => format!("<span foreground=\"#949cbb\">⚙ {desc}</span>"),
        _ => format!("<span foreground=\"#949cbb\">? {desc}</span>"),
    }
}

fn format_credits(credits: &Credits) -> String {
    match credits {
        Credits::ClaudeExtra { used_usd, limit_usd } => format!("Extra: ${used_usd:.2} / ${limit_usd:.2}"),
        Credits::CodexBalance { balance_usd } => format!("Credits: ${balance_usd:.2}"),
    }
}

fn format_provider_section(name: &str, usage: &ProviderUsage) -> String {
    let separator = match &usage.plan {
        Some(plan) => format!("━━━ {} ({}) ━━━", name, pango::escape(plan)),
        None => format!("━━━ {name} ━━━"),
    };
    let mut lines: Vec<String> = vec![separator];

    // Not configured: no credentials at all
    if !usage.has_credentials {
        if !usage.cli_installed {
            let url = if name == "Claude" {
                "claude.ai/cli"
            } else {
                "github.com/openai/codex"
            };
            lines.push(format!("Not installed — see {url}"));
        } else {
            let cmd = if name == "Claude" {
                "claude login"
            } else {
                "codex login"
            };
            lines.push(format!("Not logged in — run: {cmd}"));
        }
        if let Some(ref status) = usage.status {
            lines.push(format_status_line(status));
        }
        return lines.join("\n");
    }

    // Token expired state
    if usage.token_expired {
        let cmd = if name == "Claude" {
            "claude login"
        } else {
            "codex login"
        };
        lines.push(format!("Token expired — run: {cmd}"));
        if usage.session.is_some() || usage.weekly.is_some() || !usage.model_weekly.is_empty() || usage.credits.is_some()
        {
            let age_suffix = format_data_age(&usage.data_timestamp);
            lines.push(format!("Last data{age_suffix}:"));
        }
    }

    // Calculate max label width for meter alignment
    let mut all_labels: Vec<String> = Vec::new();
    if usage.session.is_some() {
        all_labels.push("Rate (5h):".to_string());
    }
    if usage.weekly.is_some() {
        all_labels.push("Weekly:".to_string());
    }
    for (model, _) in &usage.model_weekly {
        all_labels.push(format!("{model} weekly:"));
    }
    let max_label = all_labels.iter().map(|l| l.len()).max().unwrap_or(0);

    if let Some(ref w) = usage.session {
        lines.push(format_meter_line("Rate (5h):", w, max_label));
    }
    if let Some(ref w) = usage.weekly {
        lines.push(format_meter_line("Weekly:", w, max_label));
    }
    for (model, w) in &usage.model_weekly {
        let label = format!("{model} weekly:");
        lines.push(format_meter_line(&label, w, max_label));
    }

    if let Some(ref credits) = usage.credits {
        lines.push(format_credits(credits));
    }
    if let Some(ref status) = usage.status {
        lines.push(format_status_line(status));
    }

    lines.join("\n")
}

fn format_bar_line(prefix: &str, usage: &ProviderUsage) -> String {
    let session = match usage.session.as_ref() {
        Some(s) => s,
        // Active provider but no session data — show muted placeholder
        None => return format!("<span foreground=\"#949cbb\">{prefix} —</span>"),
    };
    let clamped = session.used_percent.clamp(0.0, 100.0);
    let pct = clamped.round() as i64;
    if usage.token_expired {
        // Muted color with ? suffix — signals data staleness (D17)
        format!("<span foreground=\"#949cbb\">{prefix} {pct}?</span>")
    } else {
        let color = usage_color(clamped);
        format!("<span foreground=\"{color}\">{prefix} {pct}</span>")
    }
}

fn format_freshness(timestamp: &str) -> String {
    let cache_time = match DateTime::parse_from_rfc3339(timestamp) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return String::new(),
    };
    let age = Utc::now().signed_duration_since(cache_time);
    let secs = age.num_seconds().max(0);

    let age_text = format_age_text(secs);

    // Color by cache age: muted (<4m), yellow (4-10m), peach (>10m)
    let color = if secs < 240 {
        "#949cbb"
    } else if secs < 600 {
        "#e5c890"
    } else {
        "#ef9f76"
    };

    format!("<span foreground=\"{color}\">Updated {age_text}</span>")
}

pub fn parse_data(data: Value) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = data["timestamp"].as_str().unwrap_or("");
    let claude_entry = &data["claude"];
    let codex_entry = &data["codex"];

    let claude = parse_claude_entry(claude_entry);
    let codex = parse_codex_entry(codex_entry);

    // Show a provider if its CLI is installed OR it has credentials
    let show_claude = claude.cli_installed || claude.has_credentials;
    let show_codex = codex.cli_installed || codex.has_credentials;

    // Bar text: one line per visible provider with session data
    let mut bar_lines: Vec<String> = Vec::new();
    if show_claude {
        bar_lines.push(format_bar_line("C", &claude));
    }
    if show_codex {
        bar_lines.push(format_bar_line("X", &codex));
    }

    let bar_text = if bar_lines.is_empty() {
        // Both providers disabled — single muted icon
        "<span foreground=\"#949cbb\">\u{F0EC0}</span>".to_string() // 󰻀 nf-md-head_cog
    } else {
        bar_lines.join("\n")
    };

    // Tooltip
    let mut tooltip_parts: Vec<String> = vec!["<span size=\"xx-large\">AI Usage</span>".to_string()];
    if show_claude {
        tooltip_parts.push(format_provider_section("Claude", &claude));
    }
    if show_codex {
        tooltip_parts.push(format_provider_section("Codex", &codex));
    }
    if !timestamp.is_empty() {
        tooltip_parts.push(format_freshness(timestamp));
    }

    let tooltip = format!("<tt>{}</tt>", tooltip_parts.join("\n\n"));

    Ok(serde_json::to_string(&json!({
        "text": bar_text,
        "tooltip": tooltip,
    }))?)
}
