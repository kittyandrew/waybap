use chrono::{Local, NaiveDate};
use serde::Deserialize;
use serde_json::{json, value::from_value, Value};

use crate::weather::constants::{get_description, get_icon};
use crate::weather::utils::*;

#[derive(Deserialize)]
struct QueryWrapper {
    location_name: Option<String>,
    data: OpenMeteoResponse,
}

#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: CurrentWeather,
    hourly: HourlyWeather,
    daily: DailyWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    time: String,
    temperature_2m: f64,
    apparent_temperature: f64,
    weather_code: i32,
    wind_speed_10m: f64,
    wind_direction_10m: i32,
    relative_humidity_2m: i32,
    is_day: i32,
}

#[derive(Deserialize)]
struct HourlyWeather {
    time: Vec<String>,
    temperature_2m: Vec<f64>,
    apparent_temperature: Vec<f64>,
    weather_code: Vec<i32>,
    precipitation_probability: Vec<i32>,
    cloud_cover: Vec<i32>,
    snowfall: Vec<f64>,
    visibility: Vec<f64>,
    is_day: Vec<i32>,
}

#[derive(Deserialize)]
struct DailyWeather {
    time: Vec<String>,
    #[allow(dead_code)]
    weather_code: Vec<i32>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    apparent_temperature_max: Vec<f64>,
    apparent_temperature_min: Vec<f64>,
    precipitation_probability_max: Vec<i32>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
}

fn bar_icon_color(code: i32, is_day: bool) -> &'static str {
    if !is_day && (code == 0 || code == 1) {
        return "#949cbb"; // muted — night
    }
    match code {
        0 => "#e5c890",       // yellow — sunny
        1 | 2 => "#e5c890",   // yellow — partly cloudy
        3 => "#949cbb",       // muted — overcast
        45 | 48 => "#949cbb", // muted — fog
        51..=57 => "#8caaee", // blue — drizzle
        61..=65 => "#8caaee", // blue — rain
        66 | 67 => "#8caaee", // blue — freezing rain
        71..=77 => "#babbf1", // lavender — snow
        80..=82 => "#8caaee", // blue — showers
        85 | 86 => "#babbf1", // lavender — snow showers
        95..=99 => "#ef9f76", // peach — thunderstorm
        _ => "#949cbb",       // muted fallback
    }
}

/// Nerd Font weather glyph for bar text — compact, no emoji padding issues.
/// Tooltip still uses emoji via get_icon().
fn bar_icon(code: i32, is_day: bool) -> &'static str {
    if !is_day && (code == 0 || code == 1) {
        return "\u{F0F36}"; // 󰼶 nf-md-weather_night
    }
    match code {
        0 => "\u{F0599}",       // 󰖙 nf-md-weather_sunny
        1 => "\u{F0595}",       // 󰖕 nf-md-weather_partly_cloudy
        2 => "\u{F0595}",       // 󰖕 nf-md-weather_partly_cloudy
        3 => "\u{F0590}",       // 󰖐 nf-md-weather_cloudy
        45 | 48 => "\u{F0591}", // 󰖑 nf-md-weather_fog
        51..=57 => "\u{F0597}", // 󰖗 nf-md-weather_rainy
        61..=65 => "\u{F0597}", // 󰖗 nf-md-weather_rainy
        66 | 67 => "\u{F0598}", // 󰖘 nf-md-weather_snowy_rainy
        71..=77 => "\u{F0598}", // 󰖘 nf-md-weather_snowy
        80..=82 => "\u{F0596}", // 󰖖 nf-md-weather_pouring
        85 | 86 => "\u{F0F36}", // 󰼶 nf-md-weather_snowy_heavy
        95..=99 => "\u{F0593}", // 󰖓 nf-md-weather_lightning
        _ => "?",
    }
}

pub fn parse_data(raw_weather: Value) -> Result<String, Box<dyn std::error::Error>> {
    let wrapper = from_value::<QueryWrapper>(raw_weather)?;
    let current = &wrapper.data.current;
    let hourly = &wrapper.data.hourly;
    let daily = &wrapper.data.daily;

    let is_day = current.is_day != 0;
    let icon = get_icon(current.weather_code, is_day);
    let feels = current.apparent_temperature.round() as i32;
    let feels_colored = color_temp(feels);

    let bar_glyph = bar_icon(current.weather_code, is_day);
    let bar_glyph_color = bar_icon_color(current.weather_code, is_day);
    let text = format!(
        "<span size=\"x-small\"><span foreground=\"{bar_glyph_color}\">{bar_glyph}</span> {feels_colored}</span>"
    );

    let mut tooltip = String::new();

    // Location header
    if let Some(ref name) = wrapper.location_name {
        tooltip += &format!("<span size=\"large\">{}</span>\n\n", crate::pango::escape(name));
    }

    // Current conditions
    let temp = current.temperature_2m.round() as i32;
    let desc = get_description(current.weather_code);
    tooltip += &format!("{icon} <b>{desc}</b> {}({feels_colored})\n", color_temp(temp));
    tooltip += &format!(
        "Wind: {} km/h {}\n",
        current.wind_speed_10m.round() as i32,
        wind_direction(current.wind_direction_10m)
    );
    tooltip += &format!("Humidity: {}%\n", current.relative_humidity_2m);

    // Parse location-local date and hour from the API response
    let (today_str, now_hour) = {
        let parts: Vec<&str> = current.time.split('T').collect();
        let date = parts.first().ok_or("missing date in current.time")?;
        let hour: u32 = parts
            .get(1)
            .and_then(|t| t.split(':').next())
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        (date.to_string(), hour)
    };
    let today = NaiveDate::parse_from_str(&today_str, "%Y-%m-%d")?;
    let start_idx = daily.time.iter().position(|d| d == &today_str).unwrap_or(0);

    // Use the system clock for Today/Tomorrow labels so stale cache data
    // (e.g. network down for days) doesn't misleadingly label old dates.
    let system_today = Local::now().date_naive();

    for day_i in start_idx..daily.time.len() {
        let date = NaiveDate::parse_from_str(&daily.time[day_i], "%Y-%m-%d")?;

        tooltip += "\n<b>";
        if date == system_today {
            tooltip += "Today, ";
        } else if date == system_today.succ_opt().unwrap_or(system_today) {
            tooltip += "Tomorrow, ";
        }
        tooltip += &format!("{}</b>\n", date.format("%-d %B %Y"));

        let max_temp = daily.temperature_2m_max[day_i].round() as i32;
        let min_temp = daily.temperature_2m_min[day_i].round() as i32;
        let max_feels = daily.apparent_temperature_max[day_i].round() as i32;
        let min_feels = daily.apparent_temperature_min[day_i].round() as i32;
        let precip_max = daily.precipitation_probability_max[day_i];
        let sunrise = daily.sunrise[day_i].split('T').nth(1).unwrap_or("??:??");
        let sunset = daily.sunset[day_i].split('T').nth(1).unwrap_or("??:??");
        tooltip += &format!(
            "🌡️↑ {}({}) 🌡️↓ {}({})  🌧️{precip_max}%  🌅{sunrise} 🌇{sunset}\n",
            color_temp(max_temp),
            color_temp(max_feels),
            color_temp(min_temp),
            color_temp(min_feels),
        );

        // Hourly entries for this day
        let h_start = day_i * 24;
        let h_end = ((day_i + 1) * 24).min(hourly.time.len());

        for h in h_start..h_end {
            // Extract hour from time string "YYYY-MM-DDTHH:MM"
            let hour_str = hourly.time[h].split('T').nth(1).unwrap_or("00:00");
            let hour_num: u32 = hour_str.split(':').next().unwrap_or("0").parse().unwrap_or(0);

            // Filter to 3-hour intervals
            if !hour_num.is_multiple_of(3) {
                continue;
            }

            // Skip past hours on today (>2 hours ago)
            if date == today && now_hour >= 2 && hour_num < now_hour - 2 {
                continue;
            }

            let h_is_day = hourly.is_day[h] != 0;
            let h_code = hourly.weather_code[h];
            let h_icon = get_icon(h_code, h_is_day);
            let h_temp = hourly.temperature_2m[h].round() as i32;
            let h_feels = hourly.apparent_temperature[h].round() as i32;
            let h_desc = get_description(h_code);
            let conditions = format_conditions(
                h_code,
                hourly.precipitation_probability[h],
                hourly.cloud_cover[h],
                hourly.snowfall[h],
                hourly.visibility[h],
            );

            tooltip += &format!(
                "{:02} {} {}({}) {}{}\n",
                hour_num,
                h_icon,
                color_temp_padded(h_temp),
                color_temp(h_feels),
                h_desc,
                conditions
            );
        }
    }

    Ok(serde_json::to_string(&json!({
        "text": text,
        "tooltip": format!("<tt>{tooltip}</tt>"),
    }))?)
}
