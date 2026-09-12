use core::time::Duration;
use reqwest::blocking::Client;

pub fn query() -> Option<String> {
    // Use curl's user agent because Cloudflare blocks requests with the default one.
    let client = match Client::builder().timeout(Duration::from_secs(10)).user_agent("curl/8.6.0").build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client for CoinGecko: {e}");
            return None;
        }
    };

    let params = "vs_currency=usd&order=market_cap_desc&per_page=10&price_change_percentage=24h";
    match client.get(format!("https://api.coingecko.com/api/v3/coins/markets?{params}")).send() {
        Ok(response) => match response.status().is_success() {
            true => match response.text() {
                Ok(text) => Some(text),
                Err(err) => {
                    eprintln!("Request text read failed: {err}!");
                    None
                }
            },
            false => {
                eprintln!("Request returned non-success status: {}!", response.status());
                None
            }
        },
        Err(err) => {
            eprintln!("CoinGecko request failed: {err}!");
            None
        }
    }
}
