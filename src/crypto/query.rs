use core::time::Duration;
use reqwest::blocking::Client;

pub fn query() -> Option<String> {
    let timeout = 5;
    let client = Client::builder()
        .timeout(Duration::new(timeout, 0))
        // We have to use this custom header, because cloudflare blocks default one.
        .user_agent("curl/8.6.0")
        .build()
        .expect("reqwest client to build successfully");

    let params = "vs_currency=usd&order=market_cap_desc&per_page=10&price_change_percentage=24h";
    let crypto_url = format!("https://api.coingecko.com/api/v3/coins/markets?{params}");
    match client.get(crypto_url).send() {
        Ok(response) => match response.text() {
            Ok(text) => Some(text),
            Err(err) => {
                eprintln!("Request text read failed: {err}!");
                None
            }
        },
        Err(err) => {
            eprintln!("Request failed: {err} (timeout was {timeout})!");
            None
        }
    }
}
