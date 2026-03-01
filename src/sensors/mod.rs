mod parsing;
mod query;

pub use parsing::parse_data;
pub use query::query;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SensorReading {
    pub label: String,
    pub temp: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SensorGroup {
    pub name: String,
    pub readings: Vec<SensorReading>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SensorData {
    pub sensors: Vec<SensorGroup>,
    pub nvidia: Vec<f64>,
}
