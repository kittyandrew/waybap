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
    if (code == 0 || code == 1 || code == 2) && cloud > 0 {
        parts.push(format!("Clouds {cloud}%"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(", {}", parts.join(", "))
    }
}

pub fn color_temp(temp: i32) -> String {
    if temp <= -10 {
        format!("<span color=\"#949cbb\">{temp: >3}°</span>")
    } else if temp <= 0 {
        format!("<span color=\"#8caaee\">{temp: >3}°</span>")
    } else if temp >= 31 {
        format!("<span color=\"#e78284\">{temp: >3}°</span>")
    } else if temp >= 16 {
        format!("<span color=\"#ef9f76\">{temp: >3}°</span>")
    } else {
        format!("{temp: >3}°")
    }
}
