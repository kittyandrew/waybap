use std::fs;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{SensorData, SensorGroup, SensorReading};

// nvidia-smi is expensive (~100ms per call), so we cache its result and only
// re-query every NVIDIA_INTERVAL seconds. The hwmon sysfs reads are virtually
// free (kernel virtual filesystem) and run every time.
const NVIDIA_INTERVAL: Duration = Duration::from_secs(10);

static NVIDIA_CACHE: Mutex<(Vec<f64>, Option<Instant>)> = Mutex::new((Vec::new(), None));

fn query_nvidia() -> Vec<f64> {
    let mut cache = NVIDIA_CACHE.lock().unwrap();
    let stale = cache.1.map(|t| t.elapsed() >= NVIDIA_INTERVAL).unwrap_or(true);
    if !stale {
        return cache.0.clone();
    }

    // Run nvidia-smi in a thread with a 5-second timeout to avoid blocking
    // the scheduler forever if nvidia-smi hangs (driver bug, GPU lockup).
    let (tx, rx) = std::sync::mpsc::channel::<Vec<f64>>();
    std::thread::spawn(move || {
        let result = Command::new("nvidia-smi")
            .args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let stdout = String::from_utf8(out.stdout).ok()?;
                    Some(stdout.lines().filter_map(|l| l.trim().parse().ok()).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let _ = tx.send(result);
    });

    let temps = rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default();
    *cache = (temps.clone(), Some(Instant::now()));
    temps
}

pub fn query() -> Option<String> {
    let mut sensors = Vec::new();

    let entries = fs::read_dir("/sys/class/hwmon")
        .map_err(|err| {
            eprintln!("ERROR: failed to read /sys/class/hwmon: {err}");
        })
        .ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match fs::read_to_string(path.join("name")) {
            Ok(n) => n.trim().to_string(),
            Err(_) => continue,
        };

        let mut readings = Vec::new();
        for i in 1..=24 {
            let temp_path = path.join(format!("temp{i}_input"));
            let temp_str = match fs::read_to_string(&temp_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let temp_mc: f64 = match temp_str.trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let temp = temp_mc / 1000.0;

            // Filter out disconnected/inactive sensors: sysfs reports exactly 0 millidegrees
            // for unconnected motherboard inputs (e.g. PCH virtual sensors on nct6799).
            // This is safe because no component in a running PC will be at exactly 0.000°C.
            if temp_mc == 0.0 || !(-40.0..=150.0).contains(&temp) {
                continue;
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

    // Sort by name for consistent ordering across reboots
    sensors.sort_by(|a, b| a.name.cmp(&b.name));

    let nvidia = query_nvidia();

    let data = SensorData { sensors, nvidia };
    serde_json::to_string(&data).ok()
}
