use chrono::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

use crate::weather::constants::WEATHER_CODES;
use crate::weather::utils::*;

pub fn parse_data(weather: Value) -> String {
    let current = &weather["current_condition"][0];
    let like = current["FeelsLikeC"].as_str().unwrap();
    let weather_code = current["weatherCode"].as_str().unwrap();
    let icon = WEATHER_CODES
        .iter()
        .find(|(code, _)| *code == weather_code.parse::<i32>().unwrap())
        .map(|(_, symbol)| symbol)
        .unwrap();

    let mut result = HashMap::new();

    // Display 'Feels like' on the sidebar.
    result.insert(
        "text",
        format!("<span size=\"small\"> {icon}\n {like}°</span>"),
    );

    let area = &weather["nearest_area"][0];
    let mut tooltip = format!(
        "<span size=\"large\">{}, {}, {}</span>\n\n",
        area["areaName"][0]["value"].as_str().unwrap(),
        area["region"][0]["value"].as_str().unwrap(),
        area["country"][0]["value"].as_str().unwrap()
    );

    let weather_desc = current["weatherDesc"][0]["value"].as_str().unwrap();
    let temp_c = current["temp_C"].as_str().unwrap();
    tooltip += &format!("{icon} <b>{weather_desc}</b> {temp_c}°\n");
    tooltip += &format!("Feels like: {like}°\n");
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

        let day_max_temp = day["maxtempC"].as_str().unwrap();
        let day_min_temp = day["mintempC"].as_str().unwrap();
        tooltip += &format!("⬆️ {day_max_temp}° ⬇️ {day_min_temp}° ",);

        let tt_sunrise = format_day_time(day, "sunrise");
        let tt_sunset = format_day_time(day, "sunset");
        tooltip += &format!("🌅 {tt_sunrise} 🌇 {tt_sunset}\n");

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

            let hour_code = hour["weatherCode"]
                .as_str()
                .unwrap()
                .parse::<i32>()
                .unwrap();
            tooltip += &format!(
                "{} {} {} {}",
                format_time(hour["time"].as_str().unwrap()),
                WEATHER_CODES
                    .iter()
                    .find(|(code, _)| *code == hour_code)
                    .map(|(_, symbol)| symbol)
                    .unwrap(),
                format!("{: >3}°", hour["FeelsLikeC"].as_str().unwrap()),
                hour["weatherDesc"][0]["value"].as_str().unwrap(),
            );
            tooltip += &format!(", {}\n", format_chances(hour)).as_str();
        }
    }
    result.insert("tooltip", tooltip);
    serde_json::to_string(&result).unwrap()
}
