//! Simplified weather (wttr.in) querying and parsing implementation,
//! borrowed from: https://github.com/bjesus/wttrbar/tree/main/src
mod constants;
mod parsing;
mod query;
mod utils;

pub use parsing::parse_data;
pub use query::query;
