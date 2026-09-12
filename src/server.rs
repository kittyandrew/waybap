use std::{fs::read_to_string, io};

use crate::{crypto, pango, scheduler::get_cache_fp, sensors, usage, weather};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

fn serve_404(request: Request) -> io::Result<()> {
    request.respond(Response::from_string("404").with_status_code(StatusCode(404)))
}

fn serve_json(request: Request, bytes: &[u8]) -> io::Result<()> {
    let content_type_header = Header::from_bytes("Content-Type", "application/json; charset=utf-8")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid Content-Type header"))?;
    request.respond(Response::from_data(bytes).with_header(content_type_header))
}

fn serve_error_json(request: Request, err_message: String) -> io::Result<()> {
    let err_res = serde_json::json!({"text": "⛓️‍💥", "tooltip": format!("<tt>{}</tt>", pango::escape(&err_message))});
    serve_json(request, err_res.to_string().as_bytes()) // Give Waybar the usual JSON so it can show the error in a tooltip.
}

fn serve_cached_api<F>(request: Request, name: &str, parse: F) -> io::Result<()>
where
    F: FnOnce(serde_json::Value) -> Result<String, Box<dyn std::error::Error>>,
{
    let display = pango::capitalize(name);
    let raw_data = match read_to_string(get_cache_fp(name)) {
        Ok(s) => s,
        Err(err) => return serve_error_json(request, format!("{display} data not available: {err}")),
    };
    let raw_data = match serde_json::from_str::<serde_json::Value>(&raw_data) {
        Ok(v) => v,
        Err(err) => return serve_error_json(request, format!("{display} cache corrupted: {err}")),
    };
    match parse(raw_data) {
        Ok(result) => serve_json(request, result.as_bytes()),
        Err(err) => serve_error_json(request, format!("{display} service failed: {err}!")),
    }
}

fn serve_request(request: Request) -> io::Result<()> {
    #[cfg(debug_assertions)]
    println!("INFO: received request! method: {:?}, url: {:?}", request.method(), request.url());

    match (request.method(), request.url()) {
        (Method::Get, "/api/weather") => serve_cached_api(request, "weather", weather::parse_data),
        (Method::Get, "/api/crypto") => serve_cached_api(request, "crypto", crypto::parse_data),
        (Method::Get, "/api/sensors") => serve_cached_api(request, "sensors", sensors::parse_data),
        (Method::Get, "/api/usage") => serve_cached_api(request, "usage", usage::parse_data),
        _ => serve_404(request),
    }
}

pub fn start(address: &str) -> Result<(), ()> {
    let server = Server::http(address).map_err(|err| eprintln!("ERROR: could not start HTTP server at {address}: {err}"))?;

    println!("INFO: listening at http://{address}/");

    for request in server.incoming_requests() {
        // Keep serving even if one response fails.
        serve_request(request).map_err(|err| eprintln!("ERROR: could not serve the response: {err}")).ok();
    }

    eprintln!("ERROR: the server socket has shutdown");
    Err(())
}
