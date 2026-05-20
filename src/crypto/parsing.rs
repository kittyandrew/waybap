use serde::Deserialize;
use serde_aux::prelude::*;
use serde_json::{Value, json, value::from_value};

use crate::catppuccin;

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

fn display_change(change: f64) -> (&'static str, f64) {
    let rounded = (change * 10.0).round() / 10.0;
    if rounded == 0.0 {
        (catppuccin::MUTED, 0.0)
    } else if rounded < 0.0 {
        (catppuccin::RED, rounded)
    } else {
        (catppuccin::GREEN, rounded)
    }
}

pub fn parse_data(raw_crypto: Value) -> Result<String, Box<dyn std::error::Error>> {
    let coins = from_value::<Vec<Coin>>(raw_crypto)?;

    // @NOTE: You can't put 'class' on the span here for some reason, but you
    //        can change a bunch of things directly with this special subset
    //        of html (bruh): https://docs.gtk.org/Pango/pango_markup.html
    let mut text = format!(
        "<span size=\"large\" foreground=\"{}\"> 󰠓</span>\n",
        catppuccin::BITCOIN_ORANGE
    );
    let mut tooltip = "<span size=\"xx-large\">Crypto</span>\n".to_string();
    let max_name_len = coins
        .iter()
        .map(|c| crate::pango::escape(&c.name).len())
        .max()
        .unwrap_or(0);
    for coin in &coins {
        let change = coin.change.unwrap_or(0.0);
        let (color, displayed_change) = display_change(change);
        // @NOTE: Store bitcoin price to display in the sidebar.
        if coin.symbol == "btc" {
            text = format!(
                "{text}<span foreground=\"{color}\" size=\"x-small\">{price:.1}k</span>",
                price = coin.price / 1000.0
            );
        }
        let coin_name = format!("  <b>{name}</b>:", name = crate::pango::escape(&coin.name));
        let price_value = format!(
            "$<span foreground=\"{color}\">{price:.precision$}</span>",
            price = coin.price,
            precision = 7_usize.saturating_sub(format!("${price}", price = coin.price.round()).len()),
        );
        let change_text = match coin.change {
            Some(_) => format!(
                "<span foreground=\"{color}\">{space}{displayed_change:.1}%</span>",
                space = if displayed_change < 0.0 { "" } else { " " },
            ),
            None => format!("<span foreground=\"{}\"> N/A</span>", catppuccin::MUTED),
        };
        tooltip += format!(
            "{coin_name: <cname_len$}{price_value: <45}{change_text}\n",
            cname_len = max_name_len + 10 + 3, // Adapt to coin name + markdown formatting + 3.
        )
        .as_ref();
    }

    Ok(serde_json::to_string(&json!({
        "text": text,
        "tooltip": format!("<tt>{tooltip}</tt>"),
    }))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_btc_price_even_when_btc_is_not_first() {
        let rendered = parse_data(json!([
            {
                "name": "Ethereum",
                "symbol": "eth",
                "current_price": 3500.0,
                "price_change_percentage_24h": 1.2
            },
            {
                "name": "Bitcoin",
                "symbol": "btc",
                "current_price": 101000.0,
                "price_change_percentage_24h": -0.5
            }
        ]))
        .expect("crypto data renders");

        assert!(rendered.contains("101.0k"));
        assert!(rendered.contains("Ethereum"));
        assert!(rendered.contains("Bitcoin"));
    }

    #[test]
    fn renders_display_rounded_zero_change_as_muted_zero() {
        let rendered = parse_data(json!([{
            "name": "Bitcoin",
            "symbol": "btc",
            "current_price": 101000.0,
            "price_change_percentage_24h": -0.04
        }]))
        .expect("crypto data renders");

        assert!(!rendered.contains("-0.0%"));
        assert!(rendered.contains("foreground=\\\"#949cbb\\\"> 0.0%"));
    }
}
