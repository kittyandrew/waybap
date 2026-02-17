use chrono::NaiveTime;

pub fn format_day_time(astronomy: &serde_json::Value, key: &str) -> Result<String, String> {
    Ok(NaiveTime::parse_from_str(
        astronomy[key].as_str().ok_or("Time of the day was not present!")?,
        "%I:%M %p",
    )
    .map_err(|err| format!("Failed to parse time from string: {err}"))?
    .format("%H:%M")
    .to_string())
}

const CHANCES: &[(&str, &str)] = &[
    ("chanceoffog", "Fog"),
    ("chanceoffrost", "Frost"),
    ("chanceofovercast", "Overcast"),
    ("chanceofrain", "Rain"),
    ("chanceofsnow", "Snow"),
    ("chanceofsunshine", "Sunshine"),
    ("chanceofthunder", "Thunder"),
    ("chanceofwindy", "Windy"),
];

pub fn format_chances(hour: &serde_json::Value) -> String {
    let mut conditions = vec![];
    for &(event, name) in CHANCES {
        if let Some(chance) = hour[event].as_str() {
            if let Ok(chance_value) = chance.parse::<u32>() {
                if chance_value > 0 {
                    conditions.push((name, chance_value));
                }
            }
        }
    }
    conditions.sort_by_key(|&(_, chance_value)| std::cmp::Reverse(chance_value));
    conditions
        .iter()
        .map(|&(name, chance_value)| format!("{} {}%", name, chance_value))
        .collect::<Vec<_>>()
        .join(", ")
}
