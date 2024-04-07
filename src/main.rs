use std::env;
use std::process::ExitCode;

mod scheduler;
mod server;
mod weather;

fn usage(program: &str) {
    eprintln!("Usage: {program} [SUBCOMMAND] [OPTIONS]");
    eprintln!("Subcommands:");
    eprintln!("    serve [address]       @TODO: start local HTTP server");
}

fn start_scheduler() -> Result<(), ()> {
    let weather_job = scheduler::Job::new("weather", 60 * 10, weather::query);
    weather_job.run();

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
            let _ = start_scheduler();

            let address = args.next().unwrap_or("127.0.0.1:6969".to_string());
            server::start(&address)
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
