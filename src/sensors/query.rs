use std::fs;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use super::{SensorData, SensorGroup, SensorReading};

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

    // NVIDIA GPU via nvidia-smi (no hwmon available for NVIDIA).
    // Runs in a thread with a 5-second timeout to avoid blocking the scheduler
    // forever if nvidia-smi hangs (driver bug, GPU lockup, etc.).
    let nvidia = {
        let (tx, rx) = mpsc::channel();
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
        rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default()
    };

    let data = SensorData { sensors, nvidia };
    serde_json::to_string(&data).ok()
}
