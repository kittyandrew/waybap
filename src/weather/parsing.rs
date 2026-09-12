use chrono::{Local, NaiveDate, NaiveDateTime, Timelike};
use serde::Deserialize;
use serde_json::{Value, json, value::from_value};

use crate::{catppuccin, weather::constants::get_description, weather::constants::get_icon, weather::utils::*};

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
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    apparent_temperature_max: Vec<f64>,
    apparent_temperature_min: Vec<f64>,
    precipitation_probability_max: Vec<i32>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
}

/// Use Nerd Font glyphs to avoid extra emoji spacing in the bar. The tooltip still uses emoji via get_icon().
fn bar_icon(code: i32, is_day: bool) -> (&'static str, &'static str) {
    if !is_day && (code == 0 || code == 1) {
        return ("\u{F0594}", catppuccin::MUTED); // 󰖔 nf-md-weather_night
    }
    match code {
        0 => ("\u{F0599}", catppuccin::YELLOW),         // 󰖙 nf-md-weather_sunny
        1 | 2 => ("\u{F0595}", catppuccin::YELLOW),     // 󰖕 nf-md-weather_partly_cloudy
        3 => ("\u{F0590}", catppuccin::MUTED),          // 󰖐 nf-md-weather_cloudy (overcast)
        45 | 48 => ("\u{F0591}", catppuccin::MUTED),    // 󰖑 nf-md-weather_fog
        51..=57 => ("\u{F0597}", catppuccin::BLUE),     // 󰖗 nf-md-weather_rainy (drizzle)
        61..=65 => ("\u{F0597}", catppuccin::BLUE),     // 󰖗 nf-md-weather_rainy
        66 | 67 => ("\u{F067F}", catppuccin::BLUE),     // 󰙿 nf-md-weather_snowy_rainy (freezing rain)
        71..=77 => ("\u{F0598}", catppuccin::LAVENDER), // 󰖘 nf-md-weather_snowy
        80..=82 => ("\u{F0596}", catppuccin::BLUE),     // 󰖖 nf-md-weather_pouring (showers)
        85 | 86 => ("\u{F0F36}", catppuccin::LAVENDER), // 󰼶 nf-md-weather_snowy_heavy
        95..=99 => ("\u{F0593}", catppuccin::PEACH),    // 󰖓 nf-md-weather_lightning
        _ => ("?", catppuccin::MUTED),
    }
}

pub fn parse_data(raw_weather: Value) -> Result<String, Box<dyn std::error::Error>> {
    let wrapper = from_value::<QueryWrapper>(raw_weather)?;
    let current = &wrapper.data.current;
    let hourly = &wrapper.data.hourly;
    let daily = &wrapper.data.daily;

    // Serde checks the field types, but we also need matching column lengths to safely index the rows.
    if [
        daily.temperature_2m_max.len(),
        daily.temperature_2m_min.len(),
        daily.apparent_temperature_max.len(),
        daily.apparent_temperature_min.len(),
        daily.precipitation_probability_max.len(),
        daily.sunrise.len(),
        daily.sunset.len(),
    ]
    .iter()
    .any(|&len| len != daily.time.len())
    {
        return Err("daily weather columns have unequal lengths".into());
    }
    if [
        hourly.temperature_2m.len(),
        hourly.apparent_temperature.len(),
        hourly.weather_code.len(),
        hourly.precipitation_probability.len(),
        hourly.cloud_cover.len(),
        hourly.snowfall.len(),
        hourly.visibility.len(),
        hourly.is_day.len(),
    ]
    .iter()
    .any(|&len| len != hourly.time.len())
    {
        return Err("hourly weather columns have unequal lengths".into());
    }

    let is_day = current.is_day != 0;
    let icon = get_icon(current.weather_code, is_day);
    let feels_colored = color_temp(current.apparent_temperature.round() as i32);

    let (bar_glyph, bar_glyph_color) = bar_icon(current.weather_code, is_day);
    let text = format!("<span size=\"x-small\"><span foreground=\"{bar_glyph_color}\">{bar_glyph}</span> {feels_colored}</span>");

    let mut tooltip = String::new();

    if let Some(ref name) = wrapper.location_name {
        tooltip += &format!("<span size=\"large\">{}</span>\n\n", crate::pango::escape(name));
    }

    let temp = current.temperature_2m.round() as i32;
    let desc = get_description(current.weather_code);
    tooltip += &format!("{icon} <b>{desc}</b> {}({feels_colored})\n", color_temp(temp));
    tooltip += &format!(
        "Wind: {} km/h {:>2}   |   Humidity: {}%\n",
        current.wind_speed_10m.round() as i32, // Wind speed in km/h.
        wind_direction(current.wind_direction_10m),
        current.relative_humidity_2m // Relative humidity (%).
    );

    // timezone=auto in query.rs gives us the forecast location's time, which may differ from this machine's time.
    let now = NaiveDateTime::parse_from_str(&current.time, "%Y-%m-%dT%H:%M")?;

    let system_today = Local::now().date_naive(); // Use today's date for labels even when the cache is stale.

    for (day_i, time) in daily.time.iter().enumerate() {
        let date = NaiveDate::parse_from_str(time, "%Y-%m-%d")?;
        if date < now.date() {
            continue; // Skip any older days included in the response so the forecast starts on the API's current date.
        }

        tooltip += "\n<b>";
        if date == system_today {
            tooltip += "Today, ";
        } else if date == system_today.succ_opt().unwrap_or(system_today) {
            tooltip += "Tomorrow, ";
        }
        tooltip += &format!("{}</b>\n", date.format("%-d %B %Y"));

        let precip_max = daily.precipitation_probability_max[day_i];
        let sunrise = NaiveDateTime::parse_from_str(&daily.sunrise[day_i], "%Y-%m-%dT%H:%M")?.format("%H:%M");
        let sunset = NaiveDateTime::parse_from_str(&daily.sunset[day_i], "%Y-%m-%dT%H:%M")?.format("%H:%M");
        tooltip += &format!(
            "🌡️↑ {}({}) 🌡️↓ {}({})  🌧️{precip_max}%  🌅{sunrise} 🌇{sunset}\n",
            color_temp(daily.temperature_2m_max[day_i].round() as i32), // Day's high.
            color_temp(daily.apparent_temperature_max[day_i].round() as i32), // Feels-like high.
            color_temp(daily.temperature_2m_min[day_i].round() as i32), // Day's low.
            color_temp(daily.apparent_temperature_min[day_i].round() as i32), // Feels-like low.
        );

        // Open-Meteo gives us 24 hourly entries per day.
        for h in day_i * 24..((day_i + 1) * 24).min(hourly.time.len()) {
            let time = NaiveDateTime::parse_from_str(&hourly.time[h], "%Y-%m-%dT%H:%M")?;
            if time.date() != date {
                return Err("hourly weather date does not match daily forecast".into()); // Don't show readings under the wrong day.
            }
            let hour_num = time.hour();
            if !hour_num.is_multiple_of(3) {
                continue; // Show three-hour samples to keep the tooltip short.
            }

            if date == now.date() && hour_num < now.hour().saturating_sub(2) {
                continue; // Keep the latest three-hour sample and skip the older hours on the API's current day.
            }

            let h_code = hourly.weather_code[h];
            let conditions = format_conditions(
                h_code, hourly.precipitation_probability[h], hourly.cloud_cover[h], hourly.snowfall[h], hourly.visibility[h],
            );

            tooltip += &format!(
                "{:02} {} {}({}) {}{}\n",
                hour_num, // Hour (00-23) at the forecast location.
                get_icon(h_code, hourly.is_day[h] != 0),
                color_temp_padded(hourly.temperature_2m[h].round() as i32), // Temperature.
                color_temp(hourly.apparent_temperature[h].round() as i32),  // Feels-like temperature.
                get_description(h_code),
                conditions
            );
        }
    }

    Ok(serde_json::to_string(&json!({"text": text, "tooltip": format!("<tt>{tooltip}</tt>")}))?)
}
