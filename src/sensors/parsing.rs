use serde_json::{value::from_value, Value};
use std::collections::HashMap;

use super::SensorData;

// Sensor category determines temperature color thresholds
#[derive(Clone, Copy)]
enum SensorKind {
    CpuGpu,      // CPU/GPU: 50/70/85
    Nvme,        // NVMe drives throttle at ~70°C
    Ram,         // DDR5: normal 30-50, concerning at 50+
    Motherboard, // Mixed sensors, generous thresholds
}

// Catppuccin Frappe palette for temperature color coding
fn temp_color(temp: f64, kind: SensorKind) -> &'static str {
    match kind {
        SensorKind::CpuGpu => {
            if temp >= 85.0 {
                "#e78284" // Red - critical
            } else if temp >= 70.0 {
                "#ef9f76" // Peach - hot
            } else if temp >= 50.0 {
                "#e5c890" // Yellow - warm
            } else {
                "#a6d189" // Green - cool
            }
        }
        SensorKind::Nvme => {
            if temp >= 70.0 {
                "#e78284" // Red - throttling
            } else if temp >= 55.0 {
                "#ef9f76" // Peach - hot
            } else if temp >= 40.0 {
                "#e5c890" // Yellow - warm
            } else {
                "#a6d189" // Green - cool
            }
        }
        SensorKind::Ram => {
            if temp >= 60.0 {
                "#e78284" // Red - critical for DDR5
            } else if temp >= 50.0 {
                "#ef9f76" // Peach - hot
            } else if temp >= 40.0 {
                "#e5c890" // Yellow - warm
            } else {
                "#a6d189" // Green - cool
            }
        }
        SensorKind::Motherboard => {
            if temp >= 85.0 {
                "#e78284" // Red - critical
            } else if temp >= 70.0 {
                "#ef9f76" // Peach - hot
            } else if temp >= 50.0 {
                "#e5c890" // Yellow - warm
            } else {
                "#a6d189" // Green - cool
            }
        }
    }
}

fn format_temp(temp: f64, kind: SensorKind) -> String {
    let color = temp_color(temp, kind);
    format!("<span foreground=\"{color}\">{temp:>5.1}°C</span>")
}

// Known hwmon sensor names -> (display title, nerd font icon, sensor kind, match mode)
// Icons: 󰻠 cpu(F0EE0), 󰢮 expansion_card(F08AE), 󰋊 harddisk(F02CA), 󰘚 chip(F061A), 󰍛 memory(F035B)
const KNOWN_SENSORS: &[(&str, &str, &str, SensorKind, bool)] = &[
    ("k10temp", "\u{F0EE0} CPU", "k10temp", SensorKind::CpuGpu, false),
    ("coretemp", "\u{F0EE0} CPU", "coretemp", SensorKind::CpuGpu, false),
    ("amdgpu", "\u{F08AE} GPU AMD", "amdgpu", SensorKind::CpuGpu, false),
    ("nvme", "\u{F02CA} NVMe", "nvme", SensorKind::Nvme, true), // prefix match
    (
        "nct6799",
        "\u{F061A} Motherboard",
        "nct6799",
        SensorKind::Motherboard,
        false,
    ),
    ("spd5118", "\u{F035B} RAM", "spd5118", SensorKind::Ram, false),
];

fn sensor_matches(hwmon_name: &str, pattern: &str, prefix: bool) -> bool {
    if prefix {
        hwmon_name.starts_with(pattern)
    } else {
        hwmon_name == pattern
    }
}

fn is_known_sensor(hwmon_name: &str) -> bool {
    KNOWN_SENSORS
        .iter()
        .any(|(_, _, pat, _, pfx)| sensor_matches(hwmon_name, pat, *pfx))
}

fn render_section(tooltip: &mut String, header: &str, labels: &[(&str, f64)], kind: SensorKind, pad_width: usize) {
    tooltip.push_str(&format!("\n<b>{header}</b>\n"));
    for &(label, temp) in labels {
        tooltip.push_str(&format!(
            "  {: <pad$} {}\n",
            label,
            format_temp(temp, kind),
            pad = pad_width
        ));
    }
}

pub fn parse_data(raw_data: Value) -> Result<String, Box<dyn std::error::Error>> {
    let data = from_value::<SensorData>(raw_data)?;

    // Find CPU temp for bar text (k10temp Tctl for AMD, or first coretemp reading for Intel)
    let cpu_temp = data
        .sensors
        .iter()
        .find(|g| g.name == "k10temp" || g.name == "coretemp")
        .and_then(|g| g.readings.iter().find(|r| r.label == "Tctl").or(g.readings.first()))
        .map(|r| r.temp);

    let mut result = HashMap::new();

    // Bar text: thermometer emoji + CPU temp on a single line
    let text = match cpu_temp {
        Some(t) => {
            let color = temp_color(t, SensorKind::CpuGpu);
            format!("<span size=\"x-small\">🌡️ <span foreground=\"{color}\">{t:.0}°</span></span>")
        }
        None => "<span size=\"x-small\">🌡️ <span foreground=\"#949cbb\">--°</span></span>".to_string(),
    };
    result.insert("text", text);

    // Tooltip: rich sensor dashboard
    let mut tooltip = "<span size=\"xx-large\">Hardware Sensors</span>\n".to_string();

    // Compute dynamic padding: find the longest label across all sensors
    let mut max_label_len = 3_usize; // minimum "GPU"
    for group in &data.sensors {
        for r in &group.readings {
            max_label_len = max_label_len.max(r.label.len());
        }
    }
    // Account for DIMM numbering ("DIMM XX") and GPU numbering ("GPU X")
    max_label_len = max_label_len.max(7);
    let pad = max_label_len + 2; // add some breathing room

    // Render known sensor categories in defined order
    for &(_, display_title, pattern, kind, prefix) in KNOWN_SENSORS {
        let groups: Vec<_> = data
            .sensors
            .iter()
            .filter(|g| sensor_matches(&g.name, pattern, prefix))
            .collect();
        if groups.is_empty() {
            continue;
        }

        if groups.len() == 1 {
            let labels: Vec<_> = groups[0].readings.iter().map(|r| (r.label.as_str(), r.temp)).collect();
            render_section(&mut tooltip, display_title, &labels, kind, pad);
        } else if pattern == "spd5118" {
            // RAM DIMMs: single header, one line per DIMM
            let labels: Vec<_> = groups
                .iter()
                .enumerate()
                .filter_map(|(i, g)| g.readings.first().map(|r| (i, r.temp)))
                .collect();
            tooltip.push_str(&format!("\n<b>{display_title}</b>\n"));
            for (i, temp) in &labels {
                let dimm_label = format!("DIMM {}", i + 1);
                tooltip.push_str(&format!(
                    "  {: <pad$} {}\n",
                    dimm_label,
                    format_temp(*temp, kind),
                    pad = pad,
                ));
            }
        } else {
            // Multiple devices with same name: numbered headers
            for (i, group) in groups.iter().enumerate() {
                let header = format!("{display_title} {}", i + 1);
                let labels: Vec<_> = group.readings.iter().map(|r| (r.label.as_str(), r.temp)).collect();
                render_section(&mut tooltip, &header, &labels, kind, pad);
            }
        }
    }

    // NVIDIA GPU section
    if !data.nvidia.is_empty() {
        let kind = SensorKind::CpuGpu;
        if data.nvidia.len() == 1 {
            let labels = vec![("GPU", data.nvidia[0])];
            render_section(&mut tooltip, "\u{F08AE} GPU NVIDIA", &labels, kind, pad);
        } else {
            tooltip.push_str("\n<b>\u{F08AE} GPU NVIDIA</b>\n");
            for (i, &temp) in data.nvidia.iter().enumerate() {
                let label = format!("GPU {i}");
                tooltip.push_str(&format!("  {: <pad$} {}\n", label, format_temp(temp, kind), pad = pad,));
            }
        }
    }

    // Any unknown/other sensors
    for group in &data.sensors {
        if is_known_sensor(&group.name) {
            continue;
        }
        let labels: Vec<_> = group.readings.iter().map(|r| (r.label.as_str(), r.temp)).collect();
        render_section(&mut tooltip, &group.name, &labels, SensorKind::Motherboard, pad);
    }

    result.insert("tooltip", format!("<tt>{tooltip}</tt>"));
    Ok(serde_json::to_string(&result)?)
}
