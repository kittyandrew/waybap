//! # JobScheduler
//! Stripped down and simplified version of the scheduler from:
//! https://github.com/BlackDex/job_scheduler/blob/master/src/lib.rs

use chrono::{DateTime, TimeZone, Utc};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

use std::thread;

pub fn get_cache_fp(name: &str) -> String {
    let home_dir = std::env::var("HOME").expect("Home directory needs to exist!");
    let cache_dir = format!("{home_dir}/.cache/waybar-data-provider");
    let _ = fs::create_dir_all(&cache_dir);
    format!("{cache_dir}/{name}.json")
}

pub fn get_last_modified_or_default(filepath: &str) -> DateTime<Utc> {
    if let Ok(metadata) = fs::metadata(filepath) {
        if let Ok(last_modified) = metadata.modified() {
            return last_modified.into();
        }
    }
    // Return an oldest possible time so we are instantly running.
    Utc.timestamp_opt(1, 0).unwrap()
}

pub struct Job {
    name: String,
    interval: usize,
    run: Box<dyn (FnMut() -> Option<String>) + Send + Sync + 'static>,
    last_run: DateTime<Utc>,
    retries: u64,
}

impl Job {
    pub fn new<T>(name: &str, interval: usize, run: T) -> Job
    where
        T: 'static,
        T: FnMut() -> Option<String>,
        T: FnMut() -> Option<String> + Send + Sync,
    {
        let cache_fp = get_cache_fp(name);
        let last_run = get_last_modified_or_default(&cache_fp);

        Job {
            name: name.to_string(),
            interval,
            run: Box::new(run),
            // @TODO: For now last tick was forever ago.
            last_run,
            retries: 3,
        }
    }

    fn tick(&mut self) {
        let now = Utc::now();

        let next_time = self.last_run.timestamp() + self.interval as i64;
        if next_time <= now.timestamp() {
            println!("[{:?}]: Running {name}!", chrono::Utc::now(), name = self.name);

            let mut iterations = 0;
            loop {
                // @TODO: Make ipinfo request to get local addr.
                // @TODO: Add option to hardcode your location.
                match (self.run)() {
                    Some(output) => {
                        let cachefile = get_cache_fp(&self.name);
                        let mut f = File::create(cachefile).expect("A");
                        let _ = f.write_all(output.as_bytes());
                        break;
                    }
                    None => {
                        iterations += 1;
                        thread::sleep(std::time::Duration::from_secs(iterations));

                        if iterations == self.retries {
                            // @TODO: Propagate errors..
                            eprintln!("Failed running '{name}' after retries!", name = &self.name);
                            break;
                        }
                    }
                }
            }

            println!("[{:?}]: Finished {name}!", chrono::Utc::now(), name = self.name);

            self.last_run = now;
        }
    }

    pub fn time_till_next_run(&self) -> std::time::Duration {
        let mut duration = 0;
        let now = Utc::now();

        let next_time = self.last_run.timestamp() + self.interval as i64;
        let next_in_secs = next_time - now.timestamp();
        if next_in_secs > 0 && (duration == 0 || next_in_secs < duration) {
            duration = next_in_secs
        }
        std::time::Duration::new(duration as u64, 0)
    }

    pub fn run(self) {
        let job_name = format!("{name}-job", name = self.name);
        // Honestly I do not know how to Rust, so here we go..
        let meme = Arc::new(Mutex::new(self));

        std::thread::Builder::new()
            .name(job_name.clone())
            .spawn(move || {
                println!("[{job_name}]: started thread - {:?}!", chrono::Utc::now());
                loop {
                    meme.lock().unwrap().tick();
                    let sleep_for = meme.lock().unwrap().time_till_next_run();
                    println!("[{job_name}]: sleeping for {:?} ...", sleep_for);
                    std::thread::sleep(sleep_for);
                }
            })
            .expect("Error spawning job-scheduler thread");
    }
}
