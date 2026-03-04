use core::time::Duration;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::sync::OnceLock;

struct Location {
    lat: f64,
    lon: f64,
    city: Option<String>,
    country: Option<String>,
}

static LOCATION: OnceLock<Location> = OnceLock::new();

fn try_resolve() -> Option<Location> {
    // Try env vars first
    let lat_env = std::env::var("WAYBAP_LAT").ok();
    let lon_env = std::env::var("WAYBAP_LON").ok();
    match (lat_env, lon_env) {
        (Some(lat_s), Some(lon_s)) => {
            let lat: f64 = match lat_s.parse() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("WAYBAP_LAT is not a valid number: {e}");
                    return None;
                }
            };
            let lon: f64 = match lon_s.parse() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("WAYBAP_LON is not a valid number: {e}");
                    return None;
                }
            };
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                eprintln!("WAYBAP_LAT/LON out of range (lat: -90..90, lon: -180..180): {lat}, {lon}");
                return None;
            }
            return Some(Location {
                lat,
                lon,
                city: None,
                country: None,
            });
        }
        (Some(_), None) | (None, Some(_)) => {
            eprintln!("WAYBAP_LAT and WAYBAP_LON must both be set, ignoring partial config");
            return None;
        }
        _ => {}
    }

    // Fallback: IP geolocation
    let client = match Client::builder()
        .timeout(Duration::from_secs(3))
        .user_agent("waybap/0.1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client for geolocation: {e}");
            return None;
        }
    };
    let response = match client.get("https://ipwho.is/").send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Geolocation request failed: {e}");
            return None;
        }
    };
    let text = match response.text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read geolocation response: {e}");
            return None;
        }
    };
    let geo: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse geolocation JSON: {e}");
            return None;
        }
    };
    if geo["success"].as_bool() != Some(true) {
        let msg = geo["message"].as_str().unwrap_or("unknown error");
        eprintln!("Geolocation failed: {msg}");
        return None;
    }
    let lat = geo["latitude"].as_f64()?;
    let lon = geo["longitude"].as_f64()?;
    let city = geo["city"].as_str().map(String::from);
    let country = geo["country"].as_str().map(String::from);
    Some(Location {
        lat,
        lon,
        city,
        country,
    })
}

fn resolve_location() -> Option<&'static Location> {
    // Return cached location if available
    if let Some(loc) = LOCATION.get() {
        return Some(loc);
    }
    // Try to resolve; only cache on success so failures retry next cycle
    let loc = try_resolve()?;
    let _ = LOCATION.set(loc);
    LOCATION.get()
}

pub fn query() -> Option<String> {
    let loc = resolve_location()?;

    let location_name: Option<String> =
        std::env::var("WAYBAP_LOCATION_NAME")
            .ok()
            .or_else(|| match (&loc.city, &loc.country) {
                (Some(city), Some(country)) => Some(format!("{city}, {country}")),
                _ => None,
            });

    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
         ?latitude={lat}&longitude={lon}\
         &current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,wind_direction_10m,relative_humidity_2m,is_day\
         &hourly=temperature_2m,apparent_temperature,weather_code,precipitation_probability,cloud_cover,snowfall,visibility,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,apparent_temperature_max,apparent_temperature_min,precipitation_probability_max,sunrise,sunset\
         &timezone=auto&forecast_days=3",
        lat = loc.lat,
        lon = loc.lon,
    );

    let client = match Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("waybap/0.1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client for Open-Meteo: {e}");
            return None;
        }
    };
    match client.get(&url).send() {
        Ok(response) => {
            let status = response.status();
            let text = match response.text() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to read Open-Meteo response: {e}");
                    return None;
                }
            };
            if !status.is_success() {
                // Try to extract the "reason" field from error responses (e.g. HTTP 400)
                let reason = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v["reason"].as_str().map(String::from))
                    .unwrap_or_else(|| format!("HTTP {status}"));
                eprintln!("Open-Meteo API error: {reason}");
                return None;
            }
            let data: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Failed to parse Open-Meteo JSON: {e}");
                    return None;
                }
            };
            if data["error"].as_bool() == Some(true) {
                let reason = data["reason"].as_str().unwrap_or("unknown error");
                eprintln!("Open-Meteo API error: {reason}");
                return None;
            }
            let wrapped = json!({
                "location_name": location_name,
                "data": data,
            });
            Some(wrapped.to_string())
        }
        Err(err) => {
            eprintln!("Open-Meteo request failed: {err}!");
            None
        }
    }
}
