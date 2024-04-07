use chrono::NaiveTime;
use std::collections::HashMap;

pub fn format_day_time(day: &serde_json::Value, key: &str) -> String {
    NaiveTime::parse_from_str(day["astronomy"][0][key].as_str().unwrap(), "%I:%M %p")
        .unwrap()
        .format("%H:%M")
        .to_string()
}

pub fn format_time(time: &str) -> String {
    let hour = time.replace("00", "").parse::<i32>().unwrap();
    format!("{:02}", hour)
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
