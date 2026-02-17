use serde::Deserialize;
use serde_aux::prelude::*;
use serde_json::{value::from_value, Value};
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct Coin {
    name: String,
    symbol: String,
    #[serde(rename = "current_price")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    price: f64,
    #[serde(rename = "price_change_percentage_24h")]
    change: Option<f64>,
}

pub fn parse_data(raw_crypto: Value) -> Result<String, Box<dyn std::error::Error>> {
    let coins = from_value::<Vec<Coin>>(raw_crypto)?;

    // @NOTE: You can't put 'class' on the span here for some reason, but you
    //        can change a bunch of things directly with this special subset
    //        of html (bruh): https://docs.gtk.org/Pango/pango_markup.html
    let mut text = "<span size=\"large\" color=\"#F7931A\"> 󰠓</span>\n".to_string(); // Using bitcoin orange.
    let mut tooltip = "<span size=\"xx-large\">Crypto</span>\n".to_string();
    let max_name_len = coins.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for (i, coin) in coins.iter().enumerate() {
        let change = coin.change.unwrap_or(0.0);
        let color = if change < 0.0 { "#e78284" } else { "#a6d189" };
        // @NOTE: Store bitcoin price to display in the sidebar.
        if coin.symbol == "btc" {
            // @TODO: We have to do this, because of hardcoded color/emoji.
            if i != 0 {
                return Err("Bitcoin has to be at the very top for this to work...".into());
            }
            text = format!(
                "{text}<span foreground=\"{color}\" size=\"x-small\">{price:.1}k</span>",
                price = coin.price / 1000.0
            );
        }
        let coin_name = format!("  <b>{name}</b>:", name = coin.name);
        let price_value = format!(
            "$<span foreground=\"{color}\">{price:.precision$}</span>",
            price = coin.price,
            precision = 7_usize.saturating_sub(format!("${price}", price = coin.price.round()).len()),
        );
        let change_text = match coin.change {
            Some(c) => format!(
                "<span foreground=\"{color}\">{space}{c:.1}%</span>",
                space = if c < 0.0 { "" } else { " " },
            ),
            None => "<span foreground=\"#949cbb\"> N/A</span>".to_string(),
        };
        tooltip += format!(
            "{coin_name: <cname_len$}{price_value: <45}{change_text}\n",
            cname_len = max_name_len + 10 + 3, // Adapt to coin name + markdown formatting + 3.
        )
        .as_ref();
    }

    // We probably want a proper return type.
    let mut result = HashMap::new();
    result.insert("text", text);
    result.insert("tooltip", format!("<tt>{tooltip}</tt>")); // We want to wrap in monofont.
    Ok(serde_json::to_string(&result)?)
}
