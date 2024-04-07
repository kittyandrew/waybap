use std::fs::read_to_string;
use std::io;
use std::str;

use crate::scheduler::get_cache_fp;
use crate::weather;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

fn serve_404(request: Request) -> io::Result<()> {
    request.respond(Response::from_string("404").with_status_code(StatusCode(404)))
}

fn serve_500(request: Request) -> io::Result<()> {
    request.respond(Response::from_string("500").with_status_code(StatusCode(500)))
}

fn serve_json(request: Request, bytes: &[u8]) -> io::Result<()> {
    let content_type_json = "application/json; charset=utf-8";
    let content_type_header = Header::from_bytes("Content-Type", content_type_json)
        .expect("That we didn't put any garbage in the headers");
    request.respond(Response::from_data(bytes).with_header(content_type_header))
}

// TODO: the errors
fn serve_api_stats(request: Request) -> io::Result<()> {
    use serde::Serialize;

    #[derive(Default, Serialize)]
    struct Stats {
        served_requests: usize,
    }

    let stats: Stats = Default::default();

    let json = match serde_json::to_string(&stats) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("ERROR: could not convert stats results to JSON: {err}");
            return serve_500(request);
        }
    };

    let content_type_header = Header::from_bytes("Content-Type", "application/json")
        .expect("That we didn't put any garbage in the headers");
    request.respond(Response::from_string(&json).with_header(content_type_header))
}

fn serve_api_weather(request: Request) -> io::Result<()> {
    let cache_fp = get_cache_fp("weather");
    let raw_data = read_to_string(cache_fp)?;
    let raw_data = serde_json::from_str::<serde_json::Value>(&raw_data).unwrap();

    let result = weather::parse_data(raw_data);
    serve_json(request, result.as_bytes())
}

fn serve_request(request: Request) -> io::Result<()> {
    println!(
        "INFO: received request! method: {:?}, url: {:?}",
        request.method(),
        request.url()
    );

    match (request.method(), request.url()) {
        (Method::Get, "/admin/stats") => serve_api_stats(request),
        (Method::Get, "/api/weather") => serve_api_weather(request),
        _ => serve_404(request),
    }
}

pub fn start(address: &str) -> Result<(), ()> {
    let server = Server::http(&address).map_err(|err| {
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
