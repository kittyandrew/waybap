use crate::catppuccin;

const DIRECTIONS: &[&str] = &["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

pub fn wind_direction(degrees: i32) -> &'static str {
    let idx = ((degrees.rem_euclid(360) as f64 / 45.0).round() as usize) % 8;
    DIRECTIONS[idx]
}

pub fn format_conditions(code: i32, precip: i32, cloud: i32, snow: f64, vis: f64) -> String {
    let mut parts = Vec::new();
    if precip > 0 {
        parts.push(format!("Precip {precip}%"));
    }
    if snow > 0.0 {
        parts.push(format!("Snow {snow:.1}cm"));
    }
    if vis < 1000.0 {
        parts.push(format!("Vis {}m", vis.round() as i32));
    }
    let cloud_suffix = if (code == 0 || code == 1 || code == 2) && cloud > 0 {
        format!(" (☁️ {cloud}%)")
    } else {
        String::new()
    };
    if parts.is_empty() {
        cloud_suffix
    } else {
        format!(", {}{cloud_suffix}", parts.join(", "))
    }
}

fn color_temp_fmt(display: String, temp: i32) -> String {
    if temp <= -10 {
        format!("<span foreground=\"{}\">{display}</span>", catppuccin::MUTED)
    } else if temp <= 0 {
        format!("<span foreground=\"{}\">{display}</span>", catppuccin::BLUE)
    } else if temp >= 31 {
        format!("<span foreground=\"{}\">{display}</span>", catppuccin::RED)
    } else if temp >= 16 {
        format!("<span foreground=\"{}\">{display}</span>", catppuccin::PEACH)
    } else {
        display
    }
}

/// Color-code a temperature value without alignment padding.
pub fn color_temp(temp: i32) -> String {
    color_temp_fmt(format!("{temp}°"), temp)
}

/// Color-code a temperature value, right-aligned to 3 chars (for monospace column alignment).
pub fn color_temp_padded(temp: i32) -> String {
    color_temp_fmt(format!("{temp: >3}°"), temp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_direction_wraps_degrees() {
        assert_eq!(wind_direction(0), "N");
        assert_eq!(wind_direction(45), "NE");
        assert_eq!(wind_direction(360), "N");
        assert_eq!(wind_direction(-45), "NW");
    }

    #[test]
    fn format_conditions_includes_only_relevant_parts() {
        assert_eq!(format_conditions(0, 0, 25, 0.0, 10_000.0), " (☁️ 25%)");
        assert_eq!(format_conditions(61, 80, 100, 0.0, 500.0), ", Precip 80%, Vis 500m");
    }
}
