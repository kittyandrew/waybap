use std::fs::read_to_string;
use std::io;

use crate::crypto;
use crate::scheduler::get_cache_fp;
use crate::weather;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

fn serve_404(request: Request) -> io::Result<()> {
    request.respond(Response::from_string("404").with_status_code(StatusCode(404)))
}

fn serve_json(request: Request, bytes: &[u8]) -> io::Result<()> {
    let content_type_json = "application/json; charset=utf-8";
    let content_type_header = Header::from_bytes("Content-Type", content_type_json).expect("valid header passed");
    request.respond(Response::from_data(bytes).with_header(content_type_header))
}

fn serve_error_json(request: Request, err_message: String) -> io::Result<()> {
    let err_res = serde_json::json!({
        "text": "⛓️‍💥",
        "tooltip": err_message
    });
    serve_json(request, err_res.to_string().as_bytes())
}

fn serve_api_weather(request: Request) -> io::Result<()> {
    let cache_fp = get_cache_fp("weather");
    let raw_data = read_to_string(cache_fp)?;
    let raw_data = serde_json::from_str::<serde_json::Value>(&raw_data)?;

    match weather::parse_data(raw_data) {
        Ok(result) => serve_json(request, result.as_bytes()),
        Err(err) => serve_error_json(request, format!("Weather service failed: {err}!")),
    }
}

fn serve_api_crypto(request: Request) -> io::Result<()> {
    let cache_fp = get_cache_fp("crypto");
    let raw_data = read_to_string(cache_fp)?;
    let raw_data = serde_json::from_str::<serde_json::Value>(&raw_data)?;

    match crypto::parse_data(raw_data) {
        Ok(result) => serve_json(request, result.as_bytes()),
        Err(err) => serve_error_json(request, format!("Crypto service failed: {err}!")),
    }
}

fn serve_request(request: Request) -> io::Result<()> {
    #[cfg(debug_assertions)] // @TODO: only in debug mode, use proper log crate later
    println!(
        "INFO: received request! method: {:?}, url: {:?}",
        request.method(),
        request.url()
    );

    match (request.method(), request.url()) {
        (Method::Get, "/api/weather") => serve_api_weather(request),
        (Method::Get, "/api/crypto") => serve_api_crypto(request),
        _ => serve_404(request),
    }
}

pub fn start(address: &str) -> Result<(), ()> {
    let server = Server::http(address).map_err(|err| {
        eprintln!("ERROR: could not start HTTP server at {address}: {err}");
    })?;

    println!("INFO: listening at http://{address}/");

    for request in server.incoming_requests() {
        serve_request(request)
            .map_err(|err| {
                eprintln!("ERROR: could not serve the response: {err}");
            })
            .ok(); // <- don't stop on errors, keep serving
    }

    eprintln!("ERROR: the server socket has shutdown");
    Err(())
}
