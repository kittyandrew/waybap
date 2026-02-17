use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;
use serde_json::{value::from_value, value::to_value, Value};
use std::collections::HashMap;

use crate::weather::constants::get_icon_by_code;
use crate::weather::utils::*;

#[derive(Deserialize, Serialize, Debug)]
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
    let current = weather.current.first().ok_or("No current weather data!")?;
    let icon = get_icon_by_code(current.code)?;

    let mut result = HashMap::new();

    // Display 'Feels like' on the sidebar.
    let feels = &current.feels;
    result.insert("text", format!("<span size=\"small\"> {icon}\n {feels}°</span>"));

    let area = weather.areas.first().ok_or("No area data!")?;
    let mut tooltip = format!(
        "<span size=\"large\">{}, {}, {}</span>\n\n",
        area["areaName"][0]["value"].as_str().ok_or("Area name was empty!")?,
        area["region"][0]["value"].as_str().ok_or("Area region was empty!")?,
        area["country"][0]["value"].as_str().ok_or("Area country was empty!")?
    );

    let weather_desc = current
        .desc
        .first()
        .and_then(|d| d["value"].as_str())
        .ok_or("Weather description empty!")?;
    let temp = current.temp.as_deref().ok_or("Temperature was not present!")?;
    tooltip += &format!("{icon} <b>{weather_desc}</b> {temp}°\n");
    tooltip += &format!("Feels like: {feels}°\n");
    tooltip += &format!("Wind: {}Km/h\n", current.windspeed);
    tooltip += &format!("Humidity: {}%\n", current.humidity);

    let today = Local::now().date_naive();
    let mut forecast = weather.forecasts;
    forecast.retain(|day| {
        NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
            .map(|date| date >= today)
            .unwrap_or(false)
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

        let date = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")?;
        tooltip += &format!("{}</b>\n", date.format("%d.%m %Y"));
        tooltip += &format!("⬆️ {max}° ⬇️ {min}° ", max = &day.max, min = &day.min);

        let astronomy = day.astronomy.first().ok_or("No astronomy data!")?;
        let tt_sunrise = format_day_time(astronomy, "sunrise")?;
        let tt_sunset = format_day_time(astronomy, "sunset")?;
        tooltip += &format!("🌅 {tt_sunrise} 🌇 {tt_sunset}\n");

        for hour in day.hourly.iter() {
            let hour_time_raw: i32 = hour.time.as_deref().ok_or("Hour time was not present!")?.parse()?;
            let hour_num = hour_time_raw / 100;

            if i == 0 && now.hour() >= 2 && (hour_num as u32) < now.hour() - 2 {
                continue;
            }

            let hour_desc = hour
                .desc
                .first()
                .and_then(|d| d["value"].as_str())
                .ok_or("Hour weather description empty!")?;

            tooltip += &format!(
                "{:02} {} {: >3}° {}",
                hour_num,
                get_icon_by_code(hour.code)?,
                hour.feels,
                hour_desc,
            );

            let raw_hour = to_value(hour)?;
            tooltip += format!(", {}\n", format_chances(&raw_hour)).as_str();
        }
    }
    result.insert("tooltip", tooltip);
    Ok(serde_json::to_string(&result)?)
}
