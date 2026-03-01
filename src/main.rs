use std::env;
use std::fs::read_to_string;
use std::process::ExitCode;

mod crypto;
mod scheduler;
mod sensors;
mod server;
mod weather;

fn usage(program: &str) {
    eprintln!("Usage: {program} [SUBCOMMAND] [OPTIONS]");
    eprintln!("Subcommands:");
    eprintln!("    serve [address]       start the daemon (default: 127.0.0.1:6969)");
    eprintln!("    test <weather|crypto|sensors> [--cache] fetch and parse live data (or cached)");
}

fn start_scheduler() {
    scheduler::Job::new("weather", 60 * 10, weather::query).run();
    scheduler::Job::new("crypto", 60 * 15, crypto::query).run();
    scheduler::Job::new("sensors", 10, sensors::query).run();
}

fn run_query(
    name: &str,
    use_cache: bool,
    query: fn() -> Option<String>,
    parse: fn(serde_json::Value) -> Result<String, Box<dyn std::error::Error>>,
) -> Result<(), ()> {
    let raw = if use_cache {
        let cache_fp = scheduler::get_cache_fp(name);
        read_to_string(&cache_fp).map_err(|err| {
            eprintln!("ERROR: failed to read cache file '{cache_fp}': {err}");
        })?
    } else {
        query().ok_or_else(|| {
            eprintln!("ERROR: {name} query failed");
        })?
    };
    let value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|err| {
        eprintln!("ERROR: failed to parse {name} response JSON: {err}");
    })?;
    let result = parse(value).map_err(|err| {
        eprintln!("ERROR: {name} parsing failed: {err}");
    })?;
    println!("{result}");
    Ok(())
}

fn entry() -> Result<(), ()> {
    let mut args = env::args();
    let program = args.next().expect("path to program is provided");

    let subcommand = args.next().ok_or_else(|| {
        usage(&program);
        eprintln!("ERROR: no subcommand is provided");
    })?;
    match subcommand.as_str() {
        "serve" => {
            start_scheduler();

            let address = args.next().unwrap_or("127.0.0.1:6969".to_string());
            server::start(&address)
        }

        "test" => {
            let target = args.next().ok_or_else(|| {
                usage(&program);
                eprintln!("ERROR: 'test' requires a target: weather, crypto, or sensors");
            })?;
            let use_cache = args.next().map(|a| a == "--cache").unwrap_or(false);
            match target.as_str() {
                "weather" => run_query("weather", use_cache, weather::query, weather::parse_data),
                "crypto" => run_query("crypto", use_cache, crypto::query, crypto::parse_data),
                "sensors" => run_query("sensors", use_cache, sensors::query, sensors::parse_data),
                _ => {
                    usage(&program);
                    eprintln!("ERROR: unknown test target '{target}'");
                    Err(())
                }
            }
        }

        _ => {
            usage(&program);
            eprintln!("ERROR: unknown subcommand {subcommand}");
            Err(())
        }
    }
}

fn main() -> ExitCode {
    match entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}
