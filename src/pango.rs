/// Escape XML special characters for safe use in Pango markup.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Capitalize first letter of a string: "sonnet" → "Sonnet"
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// 10-character block meter bar with Pango color markup.
/// `used_percent` is 0-100. Filled blocks (█) use `filled_color`, empty blocks (░) use `empty_color`.
pub fn meter_bar(used_percent: f64, width: usize, filled_color: &str, empty_color: &str) -> String {
    let clamped = used_percent.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let empty = width - filled;
    format!(
        "<span foreground=\"{filled_color}\">{}</span><span foreground=\"{empty_color}\">{}</span>",
        "█".repeat(filled),
        "░".repeat(empty),
    )
}
