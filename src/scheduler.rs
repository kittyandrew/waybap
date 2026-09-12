//! # JobScheduler
//! Stripped down and simplified version of the scheduler from:
//! https://github.com/BlackDex/job_scheduler/blob/master/src/lib.rs

use chrono::Utc;
use std::{fs, fs::File, io::Write, thread, time::Duration, time::SystemTime};

pub fn get_cache_fp(name: &str) -> String {
    let home_dir = match std::env::var("HOME") {
        Ok(home) => home,
        Err(err) => {
            eprintln!("ERROR: HOME is not set; cannot resolve ~/.cache/waybap: {err}");
            std::process::exit(1);
        }
    };
    let cache_dir = format!("{home_dir}/.cache/waybap");
    let _ = fs::create_dir_all(&cache_dir);
    format!("{cache_dir}/{name}.json")
}

pub fn start(name: &'static str, interval: u64, query: fn() -> Option<String>) -> std::io::Result<()> {
    let interval = Duration::from_secs(interval);
    let cachefile = get_cache_fp(name);
    thread::Builder::new().name(format!("{name}-job")).spawn(move || {
        // Fetch right away if the cache is missing or its timestamp is in the future.
        let age = fs::metadata(&cachefile).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        thread::sleep(interval.saturating_sub(age.unwrap_or(interval)));
        loop {
            let started = SystemTime::now();
            println!("[{:?}]: Running {name}!", Utc::now());
            for retry in 0..=3 {
                thread::sleep(Duration::from_secs(retry));
                if let Some(output) = query() {
                    let cachefile = get_cache_fp(name);
                    // Give each daemon its own temp file so simultaneous writes don't corrupt the cache.
                    let tmp_file = format!("{cachefile}.{}.tmp", std::process::id());
                    let result = File::create(&tmp_file)
                        .and_then(|mut f| {
                            f.write_all(output.as_bytes())?;
                            f.sync_all()
                        })
                        .and_then(|_| fs::rename(&tmp_file, &cachefile));
                    if let Err(err) = result {
                        eprintln!("ERROR: failed to write cache file '{cachefile}': {err}");
                    }
                    break; // Only retry the fetch. If writing fails, try again on the next scheduled run.
                }
            }
            // Count time spent fetching and suspended. If the clock moves back, wait at most one interval.
            let delay = interval.saturating_sub(started.elapsed().unwrap_or_default());
            println!("[{name}-job]: sleeping for {delay:?} ...");
            thread::sleep(delay);
        }
    })?;
    Ok(())
}
