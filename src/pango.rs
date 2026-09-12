/// Escape XML special characters for safe use in Pango markup.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Capitalize the first letter: "sonnet" -> "Sonnet".
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// Draw a Pango meter `width` characters wide, keeping `used_percent` within 0-100.
/// Use `filled_color` for filled blocks (█) and `empty_color` for empty blocks (░).
pub fn meter_bar(used_percent: f64, width: usize, filled_color: &str, empty_color: &str) -> String {
    let filled = ((used_percent.clamp(0.0, 100.0) / 100.0) * width as f64).round() as usize;
    let empty = width - filled;
    format!(
        "<span foreground=\"{filled_color}\">{}</span><span foreground=\"{empty_color}\">{}</span>",
        "█".repeat(filled),
        "░".repeat(empty),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_special_characters() {
        assert_eq!(escape("Tom & <Jerry> \"cat'"), "Tom &amp; &lt;Jerry&gt; &quot;cat&apos;");
    }

    #[test]
    fn meter_bar_clamps_percentages() {
        assert_eq!(meter_bar(125.0, 4, "#fff", "#000"), "<span foreground=\"#fff\">████</span><span foreground=\"#000\"></span>");
        assert_eq!(meter_bar(-10.0, 4, "#fff", "#000"), "<span foreground=\"#fff\"></span><span foreground=\"#000\">░░░░</span>");
    }
}
