use std::{fs, process::Command, sync::Mutex, sync::Once, thread, time::Duration, time::Instant};

use super::{SensorData, SensorGroup, SensorReading};

// nvidia-smi is expensive, so refresh it less often than the once-per-second hwmon reads.
const NVIDIA_INTERVAL: Duration = Duration::from_secs(10);
const NVIDIA_TIMEOUT: Duration = Duration::from_secs(5);

static NVIDIA_CACHE: Mutex<(Vec<f64>, Option<Instant>)> = Mutex::new((Vec::new(), None));
static NVIDIA_WORKER: Once = Once::new();

fn query_nvidia() -> Vec<f64> {
    NVIDIA_WORKER.call_once(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        // @NOTE: Use one worker so a stuck driver cannot leave a growing pile of nvidia-smi processes. - Sep 12, 2026
        thread::spawn(move || {
            loop {
                let temps = Command::new("nvidia-smi")
                    .args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
                    .output()
                    .ok()
                    .filter(|out| out.status.success())
                    .and_then(|out| String::from_utf8(out.stdout).ok())
                    .map(|stdout| {
                        stdout
                            .lines()
                            .filter_map(|l| l.trim().parse().ok())
                            .filter(|t| *t != 0.0 && (-40.0..=150.0).contains(t))
                            .collect()
                    })
                    .unwrap_or_default();
                *NVIDIA_CACHE.lock().expect("NVIDIA cache lock poisoned") = (temps, Some(Instant::now()));
                let _ = tx.send(()); // Only startup waits here. Later sends can fail because the receiver is gone.
                thread::sleep(NVIDIA_INTERVAL);
            }
        });
        let _ = rx.recv_timeout(NVIDIA_TIMEOUT); // Wait briefly for the test command's first reading.
    });
    let cache = NVIDIA_CACHE.lock().expect("NVIDIA cache lock poisoned");
    // Hide old readings if a refresh gets stuck. Show temperatures again when the worker gets fresh readings.
    if cache.1.is_some_and(|at| at.elapsed() <= NVIDIA_INTERVAL + NVIDIA_TIMEOUT) { cache.0.clone() } else { Vec::new() }
}

pub fn query() -> Option<String> {
    let mut sensors = Vec::new();

    let entries =
        fs::read_dir("/sys/class/hwmon").inspect_err(|err| eprintln!("ERROR: failed to read /sys/class/hwmon: {err}")).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(name) = fs::read_to_string(path.join("name")) else { continue }; // Keep going if one device can't be read.
        let name = name.trim().to_string();
        let Ok(files) = fs::read_dir(&path) else { continue }; // The device may have disappeared since we listed it.
        let mut inputs: Vec<u32> = files
            .flatten()
            .filter_map(|file| file.file_name().to_str()?.strip_prefix("temp")?.strip_suffix("_input")?.parse().ok())
            .collect();
        inputs.sort_unstable(); // Keep numeric order because the bar uses coretemp's first reading.

        let mut readings = Vec::new();
        for i in inputs {
            let Ok(temp_str) = fs::read_to_string(path.join(format!("temp{i}_input"))) else { continue }; // Keep other readings.
            let Ok(temp_mc) = temp_str.trim().parse::<f64>() else { continue }; // One bad reading shouldn't hide the others.
            let temp = temp_mc / 1000.0;

            // @NOTE: Unconnected motherboard inputs can report zero (e.g. PCH on nct6799).
            // Skipping them also hides real 0°C readings. - Sep 12, 2026
            if temp_mc == 0.0 || !(-40.0..=150.0).contains(&temp) {
                continue; // Skip disconnected sensors, out-of-range temperatures, NaN and infinity.
            }

            let label = fs::read_to_string(path.join(format!("temp{i}_label")))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| format!("temp{i}"));

            readings.push(SensorReading { label, temp });
        }

        if !readings.is_empty() {
            sensors.push(SensorGroup { name, readings });
        }
    }

    sensors.sort_by(|a, b| a.name.cmp(&b.name)); // Keep the sysfs order for chips with the same name.

    serde_json::to_string(&SensorData { sensors, nvidia: query_nvidia() }).ok()
}
