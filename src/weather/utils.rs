use chrono::NaiveTime;
use std::collections::HashMap;

pub fn format_day_time(astronomy: &serde_json::Value, key: &str) -> Result<String, String> {
    Ok(NaiveTime::parse_from_str(
        astronomy[key].as_str().ok_or("Time of the day was not present!")?,
        "%I:%M %p",
    )
    .map_err(|err| format!("Failed to parse time from string: {err}"))?
    .format("%H:%M")
    .to_string())
}

pub fn format_chances(hour: &serde_json::Value) -> String {
    let chances: HashMap<&str, String> = [
        ("chanceoffog", "Fog".to_string()),
        ("chanceoffrost", "Frost".to_string()),
        ("chanceofovercast", "Overcast".to_string()),
        ("chanceofrain", "Rain".to_string()),
        ("chanceofsnow", "Snow".to_string()),
        ("chanceofsunshine", "Sunshine".to_string()),
        ("chanceofthunder", "Thunder".to_string()),
        ("chanceofwindy", "Windy".to_string()),
    ]
    .iter()
    .cloned()
    .collect();

    let mut conditions = vec![];
    for (event, name) in chances.iter() {
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
