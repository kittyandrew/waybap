//! Weather data from Open-Meteo API (current conditions + 3-day forecast).
mod constants;
mod parsing;
mod query;
mod utils;

pub use parsing::parse_data;
pub use query::query;
