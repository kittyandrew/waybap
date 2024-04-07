use core::time::Duration;
use reqwest::blocking::Client;

pub fn query() -> Option<String> {
    let timeout = 5;
    let client = Client::builder()
        .timeout(Duration::new(timeout, 0))
        .build()
        .unwrap();

    // @TODO: We need some way to detect that address changed and invalidate the cache.
    match client.get("https://wttr.in/Shkarivka?format=j1").send() {
        Ok(response) => Some(response.text().unwrap()),
        Err(err) => {
            println!("Request failed!: {err} (timeout was {timeout})!");
            None
        }
    }
}
