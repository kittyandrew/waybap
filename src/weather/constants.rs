pub const WEATHER_CODES: &[(i32, &str)] = &[
    (0, "☀️"),
    (1, "🌤️"),
    (2, "⛅"),
    (3, "☁️"),
    (45, "🌫️"),
    (48, "🌫️"),
    (51, "🌧️"),
    (53, "🌧️"),
    (55, "🌧️"),
    (56, "🌧️"),
    (57, "🌧️"),
    (61, "🌦️"),
    (63, "🌧️"),
    (65, "🌧️"),
    (66, "🌨️"),
    (67, "🌨️"),
    (71, "🌨️"),
    (73, "🌨️"),
    (75, "🌨️"),
    (77, "🌨️"),
    (80, "🌦️"),
    (81, "🌧️"),
    (82, "🌧️"),
    (85, "🌨️"),
    (86, "🌨️"),
    (95, "🌩️"),
    (96, "🌩️"),
    (99, "🌩️"),
];

pub const WEATHER_DESCRIPTIONS: &[(i32, &str)] = &[
    (0, "Clear sky"),
    (1, "Mainly clear"),
    (2, "Partly cloudy"),
    (3, "Overcast"),
    (45, "Fog"),
    (48, "Depositing rime fog"),
    (51, "Light drizzle"),
    (53, "Moderate drizzle"),
    (55, "Dense drizzle"),
    (56, "Light freezing drizzle"),
    (57, "Dense freezing drizzle"),
    (61, "Slight rain"),
    (63, "Moderate rain"),
    (65, "Heavy rain"),
    (66, "Light freezing rain"),
    (67, "Heavy freezing rain"),
    (71, "Slight snowfall"),
    (73, "Moderate snowfall"),
    (75, "Heavy snowfall"),
    (77, "Snow grains"),
    (80, "Slight rain showers"),
    (81, "Moderate rain showers"),
    (82, "Violent rain showers"),
    (85, "Slight snow showers"),
    (86, "Heavy snow showers"),
    (95, "Thunderstorm"),
    (96, "Thunderstorm with slight hail"),
    (99, "Thunderstorm with heavy hail"),
];

pub fn get_icon(code: i32, is_day: bool) -> &'static str {
    if !is_day && (code == 0 || code == 1) {
        return "🌙";
    }
    WEATHER_CODES
        .binary_search_by(|(k, _)| k.cmp(&code))
        .map(|i| WEATHER_CODES[i].1)
        .unwrap_or("?")
}

pub fn get_description(code: i32) -> &'static str {
    WEATHER_DESCRIPTIONS
        .binary_search_by(|(k, _)| k.cmp(&code))
        .map(|i| WEATHER_DESCRIPTIONS[i].1)
        .unwrap_or("Unknown")
}
