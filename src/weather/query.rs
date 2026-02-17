use core::time::Duration;
use reqwest::blocking::Client;

pub fn query() -> Option<String> {
    let timeout = 5;
    let client = Client::builder().timeout(Duration::new(timeout, 0)).build().unwrap();

    // @TODO: Make ipinfo request to get local addr.
    // @TODO: Add option to hardcode your location.
    // @TODO: We need some way to detect that address changed and invalidate the cache.
    match client.get("https://wttr.in/Shkarivka?format=j1").send() {
        Ok(response) => {
            if !response.status().is_success() {
                eprintln!("Request returned non-success status: {}!", response.status());
                return None;
            }
            match response.text() {
                Ok(text) => Some(text),
                Err(err) => {
                    eprintln!("Request text read failed: {err}!");
                    None
                }
            }
        }
        Err(err) => {
            eprintln!("Request failed: {err} (timeout was {timeout})!");
            None
        }
    }
}
