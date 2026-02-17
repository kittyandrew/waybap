//! # JobScheduler
//! Stripped down and simplified version of the scheduler from:
//! https://github.com/BlackDex/job_scheduler/blob/master/src/lib.rs

use chrono::{DateTime, TimeZone, Utc};
use std::fs;
use std::fs::File;
use std::io::prelude::*;

use std::thread;

pub fn get_cache_fp(name: &str) -> String {
    let home_dir = std::env::var("HOME").expect("Home directory needs to exist!");
    let cache_dir = format!("{home_dir}/.cache/waybap");
    let _ = fs::create_dir_all(&cache_dir);
    format!("{cache_dir}/{name}.json")
}

fn get_last_modified_or_default(filepath: &str) -> DateTime<Utc> {
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
                match (self.run)() {
                    Some(output) => {
                        let cachefile = get_cache_fp(&self.name);
                        let tmp_file = format!("{cachefile}.tmp");
                        let result = File::create(&tmp_file)
                            .and_then(|mut f| {
                                f.write_all(output.as_bytes())?;
                                f.sync_all()
                            })
                            .and_then(|_| fs::rename(&tmp_file, &cachefile));
                        if let Err(err) = result {
                            eprintln!("ERROR: failed to write cache file '{cachefile}': {err}");
                        }
                        break;
                    }
                    None => {
                        iterations += 1;
                        thread::sleep(std::time::Duration::from_secs(iterations));

                        if iterations == self.retries {
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

    fn time_till_next_run(&self) -> std::time::Duration {
        let now = Utc::now();
        let next_time = self.last_run.timestamp() + self.interval as i64;
        let next_in_secs = next_time - now.timestamp();
        let duration = if next_in_secs > 0 { next_in_secs as u64 } else { 0 };
        std::time::Duration::new(duration, 0)
    }

    pub fn run(mut self) {
        let job_name = format!("{name}-job", name = self.name);

        std::thread::Builder::new()
            .name(job_name.clone())
            .spawn(move || {
                println!("[{job_name}]: started thread - {:?}!", chrono::Utc::now());
                loop {
                    self.tick();
                    let sleep_for = self.time_till_next_run();
                    println!("[{job_name}]: sleeping for {:?} ...", sleep_for);
                    std::thread::sleep(sleep_for);
                }
            })
            .expect("Error spawning job-scheduler thread");
    }
}
