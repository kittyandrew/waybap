use chrono::prelude::*;
use serde::Deserialize;
use serde_aux::prelude::*;
use serde_json::{value::from_value, value::to_value, Value};
use std::collections::HashMap;

use crate::weather::constants::get_icon_by_code;
use crate::weather::utils::*;

#[derive(Deserialize, Debug)]
pub struct Current {
    #[serde(rename = "FeelsLikeC")]
    feels: String,
    #[serde(rename = "weatherCode")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    code: i32,
    #[serde(rename = "weatherDesc")]
    desc: Vec<Value>,
    #[serde(rename = "temp_C")]
    temp: Option<String>, // Only present inside 'current_condition'.
    #[serde(rename = "windspeedKmph")]
    windspeed: String,
    humidity: String,
    time: Option<String>, // Only present inside 'Day'.
}

#[derive(Deserialize, Debug)]
struct Day {
    date: String,
    astronomy: Vec<Value>,
    #[serde(rename = "maxtempC")]
    max: String,
    #[serde(rename = "mintempC")]
    min: String,
    hourly: Vec<Current>,
}

#[derive(Deserialize, Debug)]
struct Weather {
    #[serde(rename = "current_condition")]
    current: Vec<Current>,
    #[serde(rename = "weather")]
    forecasts: Vec<Day>,
    #[serde(rename = "nearest_area")]
    areas: Vec<Value>,
}

pub fn parse_data(raw_weather: Value) -> Result<String, Box<dyn std::error::Error>> {
    let weather = from_value::<Weather>(raw_weather)?;
    let current = &weather.current[0];
    let icon = get_icon_by_code(current.code);

    let mut result = HashMap::new();

    // Display 'Feels like' on the sidebar.
    let feels = &current.feels;
    result.insert("text", format!("<span size=\"small\"> {icon}\n {feels}°</span>"));

    let area = &weather.areas[0];
    let mut tooltip = format!(
        "<span size=\"large\">{}, {}, {}</span>\n\n",
        area["areaName"][0]["value"].as_str().ok_or("Area name failed!")?,
        area["region"][0]["value"].as_str().unwrap(),
        area["country"][0]["value"].as_str().unwrap()
    );

    let weather_desc = current.desc[0]["value"].as_str().unwrap();
    let temp = current.temp.clone().unwrap();
    tooltip += &format!("{icon} <b>{weather_desc}</b> {temp}°\n");
    tooltip += &format!("Feels like: {feels}°\n");
    tooltip += &format!("Wind: {}Km/h\n", current.windspeed);
    tooltip += &format!("Humidity: {}%\n", current.humidity);

    let today = Local::now().date_naive();
    let mut forecast = weather.forecasts;
    forecast.retain(|day| NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").unwrap() >= today);

    let now = Local::now();

    for (i, day) in forecast.iter().enumerate() {
        tooltip += "\n<b>";
        if i == 0 {
            tooltip += "Today, ";
        }
        if i == 1 {
            tooltip += "Tomorrow, ";
        }

        let date = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").unwrap();
        tooltip += &format!("{}</b>\n", date.format("%d.%m %Y"));
        tooltip += &format!("⬆️ {max}° ⬇️ {min}° ", max = &day.max, min = &day.min);

        let tt_sunrise = format_day_time(&day.astronomy[0], "sunrise");
        let tt_sunset = format_day_time(&day.astronomy[0], "sunset");
        tooltip += &format!("🌅 {tt_sunrise} 🌇 {tt_sunset}\n");

        for hour in day.hourly.iter() {
            let hour_time = hour.time.clone().unwrap();
            let formatted_hour_time = if hour_time.len() >= 2 {
                hour_time[..hour_time.len() - 2].to_string()
            } else {
                hour_time.to_string()
            };
            if i == 0 && now.hour() >= 2 && formatted_hour_time.parse::<u32>().unwrap() < now.hour() - 2 {
                continue;
            }

            tooltip += &format!(
                "{} {} {} {}",
                format!("{:02}", hour_time.replace("00", "").parse::<i32>().unwrap()),
                get_icon_by_code(hour.code),
                format!("{: >3}°", hour.feels),
                hour.desc[0]["value"].as_str().unwrap(),
            );

            let raw_hour = to_value::<Value>(&hour).unwrap();
            tooltip += &format!(", {}\n", format_chances(&raw_hour)).as_str();
        }
    }
    result.insert("tooltip", tooltip);
    Ok(serde_json::to_string(&result)?)
}
