use chrono::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

use crate::weather::constants::WEATHER_CODES;
use crate::weather::utils::*;

pub fn parse_data(weather: Value) -> String {
    let current = &weather["current_condition"][0];
    let feels_like = current["FeelsLikeC"].as_str().unwrap();
    let weather_code = current["weatherCode"].as_str().unwrap();
    let weather_icon = WEATHER_CODES
        .iter()
        .find(|(code, _)| *code == weather_code.parse::<i32>().unwrap())
        .map(|(_, symbol)| symbol)
        .unwrap();

    // Display 'Feels like' on the sidebar.
    let text = format!("<span size=\"small\"> {}\n {}°</span>", weather_icon, feels_like);

    let mut result = HashMap::new();
    result.insert("text", text);

    let area = &weather["nearest_area"][0];
    let mut tooltip = format!(
        "<span size=\"large\">{}, {}, {}</span>\n\n",
        area["areaName"][0]["value"].as_str().unwrap(),
        area["region"][0]["value"].as_str().unwrap(),
        area["country"][0]["value"].as_str().unwrap()
    );
    tooltip += &format!(
        "<b>{}</b> {}°\n",
        current["weatherDesc"][0]["value"].as_str().unwrap(),
        current["temp_C"].as_str().unwrap()
    );
    tooltip += &format!("Feels like: {}°\n", feels_like);
    tooltip += &format!("Wind: {}Km/h\n", current["windspeedKmph"].as_str().unwrap());
    tooltip += &format!("Humidity: {}%\n", current["humidity"].as_str().unwrap());

    let today = Local::now().date_naive();
    let mut forecast = weather["weather"].as_array().unwrap().clone();
    forecast.retain(|item| {
        let item_date =
            NaiveDate::parse_from_str(item["date"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        item_date >= today
    });

    let now = Local::now();

    for (i, day) in forecast.iter().enumerate() {
        tooltip += "\n<b>";
        if i == 0 {
            tooltip += "Today, ";
        }
        if i == 1 {
            tooltip += "Tomorrow, ";
        }
        let date = NaiveDate::parse_from_str(day["date"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        tooltip += &format!("{}</b>\n", date.format("%d.%m %Y"));

        tooltip += &format!(
            "⬆️ {}° ⬇️ {}° ",
            day["maxtempC"].as_str().unwrap(),
            day["mintempC"].as_str().unwrap(),
        );

        tooltip += &format!(
            "🌅 {} 🌇 {}\n",
            format_day_time(day, "sunrise"),
            format_day_time(day, "sunset"),
        );

        for hour in day["hourly"].as_array().unwrap() {
            let hour_time = hour["time"].as_str().unwrap();
            let formatted_hour_time = if hour_time.len() >= 2 {
                hour_time[..hour_time.len() - 2].to_string()
            } else {
                hour_time.to_string()
            };
            if i == 0
                && now.hour() >= 2
                && formatted_hour_time.parse::<u32>().unwrap() < now.hour() - 2
            {
                continue;
            }

            let mut tooltip_line = format!(
                "{} {} {} {}",
                format_time(hour["time"].as_str().unwrap()),
                WEATHER_CODES
                    .iter()
                    .find(|(code, _)| *code
                        == hour["weatherCode"]
                            .as_str()
                            .unwrap()
                            .parse::<i32>()
                            .unwrap())
                    .map(|(_, symbol)| symbol)
                    .unwrap(),
                format!("{: >3}°", hour["FeelsLikeC"].as_str().unwrap()),
                hour["weatherDesc"][0]["value"].as_str().unwrap(),
            );
            tooltip_line += format!(", {}", format_chances(hour)).as_str();
            tooltip_line += "\n";
            tooltip += &tooltip_line;
        }
    }
    result.insert("tooltip", tooltip);
    serde_json::to_string(&result).unwrap()
}
