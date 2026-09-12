use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{catppuccin, pango};

struct RateWindow {
    used_percent: f64,
    resets_at: Option<DateTime<Utc>>,
}

enum Credits {
    ClaudeExtra { used_usd: f64, limit_usd: f64 },
    CodexBalance { balance: String },
}

struct ProviderStatus {
    indicator: String,
    description: String,
}

struct ProviderUsage {
    session: Option<RateWindow>,
    weekly: Option<RateWindow>,
    extra_windows: Vec<(String, RateWindow)>,
    credits: Option<Credits>,
    status: Option<ProviderStatus>,
    plan: Option<String>,                  // Codex only (Claude's API has no plan field)
    data_timestamp: Option<DateTime<Utc>>, // when this provider's data was last fetched
    token_expired: bool,
    has_credentials: bool,
    cli_installed: bool,
}

fn parse_status(status: &Value) -> Option<ProviderStatus> {
    let indicator = status["indicator"].as_str()?.to_string();
    let description = status["description"].as_str()?.to_string();
    Some(ProviderStatus { indicator, description })
}

fn parse_claude_entry(provider: &Value) -> ProviderUsage {
    let data = &provider["data"];
    let data_timestamp =
        provider["data_timestamp"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.to_utc());

    let session = data["five_hour"]["utilization"].as_f64().map(|used_percent| RateWindow {
        used_percent,
        resets_at: data["five_hour"]["resets_at"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.to_utc()),
    });

    let weekly = data["seven_day"]["utilization"].as_f64().map(|used_percent| RateWindow {
        used_percent,
        resets_at: data["seven_day"]["resets_at"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.to_utc()),
    });

    // Include any extra weekly limits, such as limits for individual models.
    let mut extra_windows = Vec::new();
    if let Some(obj) = data.as_object() {
        for (key, val) in obj {
            if let Some(model_name) = key.strip_prefix("seven_day_")
                && let Some(used_percent) = val["utilization"].as_f64()
            {
                // Leave labels unescaped until padding (oauth_apps becomes Oauth Apps).
                let display_name = model_name.split('_').map(pango::capitalize).collect::<Vec<_>>().join(" ");
                extra_windows.push((
                    format!("{display_name} weekly:"),
                    RateWindow {
                        used_percent,
                        resets_at: val["resets_at"]
                            .as_str()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.to_utc()),
                    },
                ));
            }
        }
    }

    // Convert the API's cents to dollars for display.
    let credits = data["extra_usage"].as_object().and_then(|extra| {
        if !extra.get("is_enabled")?.as_bool()? {
            return None; // Hide the spending row when extra usage is disabled.
        }
        let used_cents = extra.get("used_credits")?.as_f64()?;
        let limit_cents = extra.get("monthly_limit")?.as_f64()?;
        Some(Credits::ClaudeExtra { used_usd: used_cents / 100.0, limit_usd: limit_cents / 100.0 })
    });

    ProviderUsage {
        session,
        weekly,
        extra_windows,
        credits,
        status: parse_status(&provider["status"]),
        plan: None,
        data_timestamp,
        token_expired: provider["token_expired"].as_bool().unwrap_or(false),
        has_credentials: provider["has_credentials"].as_bool().unwrap_or(false),
        cli_installed: provider["cli_installed"].as_bool().unwrap_or(false),
    }
}

fn parse_codex_entry(provider: &Value) -> ProviderUsage {
    let data = &provider["data"];
    let data_timestamp =
        provider["data_timestamp"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.to_utc());

    let rate_limit = &data["rate_limit"];

    let (mut session, mut weekly) = (None, None);
    let mut extra_windows = Vec::new();
    // @NOTE: The weekly window can be in either slot, so identify it by its duration. - Sep 12, 2026
    for key in ["primary_window", "secondary_window"] {
        let w = &rate_limit[key];
        let Some(used_percent) = w["used_percent"].as_f64() else { continue }; // Missing data doesn't mean 0% used.
        let window = RateWindow {
            used_percent,
            resets_at: w["reset_at"].as_i64().and_then(|ts| DateTime::from_timestamp(ts, 0)), // Unix seconds
        };
        match w["limit_window_seconds"].as_i64() {
            Some(604800) => weekly = Some(window),
            Some(18000) => session = Some(window),
            duration => {
                let label = match duration {
                    Some(s) if s > 0 && s % 86400 == 0 => format!("Rate ({}d):", s / 86400),
                    Some(s) if s > 0 && s % 3600 == 0 => format!("Rate ({}h):", s / 3600),
                    Some(s) if s > 0 => format!("Rate ({s}s):"),
                    _ => "Rate:".to_string(),
                };
                extra_windows.push((label, window)); // Show other durations under their own labels.
            }
        }
    }

    // @NOTE: Codex sends the balance as text. It's a credit count, not a dollar amount. - Sep 12, 2026
    let c = &data["credits"];
    let balance = if c["unlimited"] == true {
        Some("Unlimited")
    } else if c["has_credits"] == true {
        Some(c["balance"].as_str().unwrap_or("Available"))
    } else {
        None
    };
    ProviderUsage {
        session,
        weekly,
        extra_windows,
        credits: balance.map(|balance| Credits::CodexBalance { balance: balance.to_string() }),
        status: parse_status(&provider["status"]),
        plan: data["plan_type"].as_str().map(|s| s.split('_').map(pango::capitalize).collect::<Vec<_>>().join(" ")),
        data_timestamp,
        token_expired: provider["token_expired"].as_bool().unwrap_or(false),
        has_credentials: provider["has_credentials"].as_bool().unwrap_or(false),
        cli_installed: provider["cli_installed"].as_bool().unwrap_or(false),
    }
}

fn usage_color(used_percent: f64) -> &'static str {
    match used_percent {
        ..50.0 => catppuccin::GREEN,
        ..=75.0 => catppuccin::YELLOW,
        ..=90.0 => catppuccin::PEACH,
        _ => catppuccin::RED,
    }
}

fn format_countdown(resets_at: &Option<DateTime<Utc>>) -> String {
    let reset_time = match resets_at {
        Some(dt) => dt,
        None => return String::new(), // No valid reset time to count down to.
    };
    let total_secs = reset_time.signed_duration_since(Utc::now()).num_seconds();
    match total_secs {
        ..=0 => format!("<span foreground=\"{}\">resetting...</span>", catppuccin::MUTED), // Wait for the API's next reset time.
        1..=59 => format!("resets in {total_secs}s"),
        60..=3599 => format!("resets in {}m", total_secs / 60),
        3600..=86399 => format!("resets in {}h {}m", total_secs / 3600, total_secs % 3600 / 60),
        _ => format!("resets in {}d {}h", total_secs / 86400, total_secs % 86400 / 3600),
    }
}

/// Expects a non-negative age in seconds.
fn format_age_text(secs: i64) -> String {
    match secs {
        0 => "just now".to_string(),
        ..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

fn format_meter_line(label: &str, window: &RateWindow, pad_to: usize) -> String {
    let bar = pango::meter_bar(window.used_percent, 10, usage_color(window.used_percent), catppuccin::MUTED);
    let countdown = format_countdown(&window.resets_at);
    let pct = format!("{:.0}%", window.used_percent.clamp(0.0, 100.0));
    let escaped_label = pango::escape(&format!("{label:pad_to$}")); // Pad before escaping so &amp; counts as one char.
    format!("{escaped_label} {bar}  {pct:>4}  {countdown}")
}

fn format_status_line(status: &ProviderStatus) -> String {
    let desc = pango::escape(&status.description);
    match status.indicator.as_str() {
        "none" => format!("<span foreground=\"{}\">✓ {desc}</span>", catppuccin::GREEN),
        "minor" => format!("<span foreground=\"{}\">⚠ {desc}</span>", catppuccin::YELLOW),
        "major" | "critical" => format!("<span foreground=\"{}\">✗ {desc}</span>", catppuccin::RED),
        "maintenance" => format!("<span foreground=\"{}\">⚙ {desc}</span>", catppuccin::MUTED),
        _ => format!("<span foreground=\"{}\">? {desc}</span>", catppuccin::MUTED),
    }
}

fn format_credits(credits: &Credits) -> String {
    match credits {
        Credits::ClaudeExtra { used_usd, limit_usd } => format!("Extra: ${used_usd:.2} / ${limit_usd:.2}"),
        Credits::CodexBalance { balance } => format!("Credits: {}", pango::escape(balance)),
    }
}

fn format_provider_section(name: &str, usage: &ProviderUsage) -> String {
    let separator = match &usage.plan {
        Some(plan) => format!("━━━ {} ({}) ━━━", name, pango::escape(plan)),
        None => format!("━━━ {name} ━━━"),
    };
    let mut lines: Vec<String> = vec![separator];

    if !usage.has_credentials {
        let cmd = if name == "Claude" { "claude login" } else { "codex login" }; // We only get here if the CLI is installed.
        lines.push(format!("Not logged in — run: {cmd}"));
        if let Some(ref status) = usage.status {
            lines.push(format_status_line(status));
        }
        return lines.join("\n"); // Show login help and service status, but no usage while logged out.
    }

    if usage.token_expired {
        let cmd = if name == "Claude" { "claude login" } else { "codex login" };
        lines.push(format!("Token expired — run: {cmd}"));
    }
    if let Some(timestamp) = usage.data_timestamp {
        lines.push(format_freshness(timestamp));
    }

    let mut windows = Vec::new();
    if let Some(w) = &usage.session {
        windows.push(("Rate (5h):", w));
    }
    if let Some(w) = &usage.weekly {
        windows.push(("Weekly:", w));
    }
    for (label, w) in &usage.extra_windows {
        windows.push((label.as_str(), w));
    }
    let max_label = windows.iter().map(|(label, _)| label.chars().count()).max().unwrap_or(0);
    for (label, w) in windows {
        lines.push(format_meter_line(label, w, max_label));
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
    // Use weekly usage for a steadier reading in the bar than session usage.
    let weekly = match usage.weekly.as_ref() {
        Some(w) => w,
        None => return format!("<span foreground=\"{}\">{prefix} —</span>", catppuccin::MUTED),
    };
    let clamped = weekly.used_percent.clamp(0.0, 100.0);
    let pct = clamped.round() as i64;
    if usage.token_expired || usage.data_timestamp.is_none_or(|ts| (Utc::now() - ts).num_seconds() >= 240) {
        format!("<span foreground=\"{}\">{prefix} {pct}?</span>", catppuccin::MUTED) // Show that this reading may be out of date.
    } else {
        let color = usage_color(clamped);
        format!("<span foreground=\"{color}\">{prefix} {pct}</span>")
    }
}

fn format_freshness(timestamp: DateTime<Utc>) -> String {
    // Don't show a negative age if the clocks disagree or ours moves back.
    let secs = Utc::now().signed_duration_since(timestamp).num_seconds().max(0);

    let age_text = format_age_text(secs);

    let color = match secs {
        ..=239 => catppuccin::MUTED,
        240..=599 => catppuccin::YELLOW,
        _ => catppuccin::PEACH,
    };

    format!("<span foreground=\"{color}\">Data fetched {age_text}</span>")
}

pub fn parse_data(data: Value) -> Result<String, Box<dyn std::error::Error>> {
    let claude = parse_claude_entry(&data["claude"]);
    let codex = parse_codex_entry(&data["codex"]);

    let show_claude = claude.cli_installed || claude.has_credentials;
    let show_codex = codex.cli_installed || codex.has_credentials;

    let mut bar_lines: Vec<String> = Vec::new();
    if show_claude {
        bar_lines.push(format_bar_line("C", &claude));
    }
    if show_codex {
        bar_lines.push(format_bar_line("X", &codex));
    }

    let bar_text = if bar_lines.is_empty() {
        format!("<span foreground=\"{}\">\u{F0EC0}</span>", catppuccin::MUTED) // 󰻀 nf-md-head_cog
    } else {
        format!("<span size=\"x-small\">{}</span>", bar_lines.join("\n"))
    };

    let mut tooltip_parts: Vec<String> = vec!["<span size=\"xx-large\">AI Usage</span>".to_string()];
    if show_claude {
        tooltip_parts.push(format_provider_section("Claude", &claude));
    }
    if show_codex {
        tooltip_parts.push(format_provider_section("Codex", &codex));
    }

    let tooltip = format!("<tt>{}</tt>", tooltip_parts.join("\n\n"));

    Ok(serde_json::to_string(&json!({"text": bar_text, "tooltip": tooltip}))?)
}
