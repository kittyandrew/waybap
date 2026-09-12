use serde_json::{Value, json, value::from_value};

use super::{SensorData, SensorReading};
use crate::catppuccin;

#[derive(Clone, Copy)]
enum SensorKind {
    CpuGpu,
    Nvme,
    Ram,
    Motherboard,
}

fn temp_color(temp: f64, kind: SensorKind) -> &'static str {
    let (warm, hot, critical) = match kind {
        SensorKind::CpuGpu => (50.0, 70.0, 85.0),
        SensorKind::Nvme => (40.0, 55.0, 70.0),        // NVMe drives throttle at ~70°C.
        SensorKind::Ram => (40.0, 50.0, 60.0),         // DDR5 normally runs at 30-50°C, with concern above 50°C.
        SensorKind::Motherboard => (50.0, 70.0, 85.0), // Mixed sensors need more generous thresholds.
    };
    match temp {
        t if t >= critical => catppuccin::RED,
        t if t >= hot => catppuccin::PEACH,
        t if t >= warm => catppuccin::YELLOW,
        _ => catppuccin::GREEN,
    }
}

fn format_temp(temp: f64, kind: SensorKind) -> String {
    format!("<span foreground=\"{}\">{temp:>5.1}°C</span>", temp_color(temp, kind))
}

// Show sections in this order with a Nerd Font icon in each title.
// Icons: 󰻠 cpu(F0EE0), 󰢮 expansion_card(F08AE), 󰋊 harddisk(F02CA), 󰘚 chip(F061A), 󰍛 memory(F035B)
const KNOWN_SENSORS: &[(&str, &str, SensorKind, bool)] = &[
    ("k10temp", "\u{F0EE0} CPU", SensorKind::CpuGpu, false),
    ("coretemp", "\u{F0EE0} CPU", SensorKind::CpuGpu, false),
    ("amdgpu", "\u{F08AE} GPU AMD", SensorKind::CpuGpu, false),
    ("nvme", "\u{F02CA} NVMe", SensorKind::Nvme, true), // prefix match
    ("nct6799", "\u{F061A} Motherboard", SensorKind::Motherboard, false),
    ("spd5118", "\u{F035B} RAM", SensorKind::Ram, false),
];

fn sensor_matches(hwmon_name: &str, pattern: &str, prefix: bool) -> bool {
    if prefix { hwmon_name.starts_with(pattern) } else { hwmon_name == pattern }
}

fn is_known_sensor(hwmon_name: &str) -> bool {
    KNOWN_SENSORS.iter().any(|(pat, _, _, pfx)| sensor_matches(hwmon_name, pat, *pfx))
}

fn render_section(tooltip: &mut String, header: &str, readings: &[SensorReading], kind: SensorKind, pad_width: usize) {
    tooltip.push_str(&format!("\n<b>{}</b>\n", crate::pango::escape(header)));
    for SensorReading { label, temp } in readings {
        let label = crate::pango::escape(&format!("{label: <pad_width$}")); // Pad before escaping so &amp; counts as one char.
        tooltip.push_str(&format!("  {label} {}\n", format_temp(*temp, kind)));
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

    // Use a Nerd Font thermometer to avoid the extra spacing around emoji in the bar.
    let text = match cpu_temp {
        Some(t) => {
            let color = temp_color(t, SensorKind::CpuGpu);
            format!("<span size=\"x-small\">\u{F050F} <span foreground=\"{color}\">{t:.0}°</span></span>")
        }
        None => format!("<span size=\"x-small\">\u{F050F} <span foreground=\"{}\">--°</span></span>", catppuccin::MUTED),
    };

    let mut tooltip = "<span size=\"xx-large\">Hardware Sensors</span>\n".to_string();

    let mut max_label_len = 7; // Leave room for numbered labels like "DIMM XX" and "GPU X".
    for group in &data.sensors {
        for r in &group.readings {
            max_label_len = max_label_len.max(r.label.chars().count());
        }
    }
    let pad = max_label_len + 2; // add some breathing room

    for &(pattern, display_title, kind, prefix) in KNOWN_SENSORS {
        let groups: Vec<_> = data.sensors.iter().filter(|g| sensor_matches(&g.name, pattern, prefix)).collect();
        if groups.len() > 1 && pattern == "spd5118" {
            // Group RAM DIMMs under one header, with one temperature per DIMM.
            tooltip.push_str(&format!("\n<b>{display_title}</b>\n"));
            for (i, temp) in groups.iter().enumerate().filter_map(|(i, g)| g.readings.first().map(|r| (i, r.temp))) {
                let dimm_label = format!("DIMM {}", i + 1);
                tooltip.push_str(&format!("  {: <pad$} {}\n", dimm_label, format_temp(temp, kind), pad = pad,));
            }
        } else {
            for (i, group) in groups.iter().enumerate() {
                let header = if groups.len() == 1 { display_title.to_string() } else { format!("{display_title} {}", i + 1) };
                render_section(&mut tooltip, &header, &group.readings, kind, pad);
            }
        }
    }

    if !data.nvidia.is_empty() {
        tooltip.push_str("\n<b>\u{F08AE} GPU NVIDIA</b>\n");
        for (i, &temp) in data.nvidia.iter().enumerate() {
            let label = if data.nvidia.len() == 1 { "GPU".to_string() } else { format!("GPU {i}") };
            tooltip.push_str(&format!("  {: <pad$} {}\n", label, format_temp(temp, SensorKind::CpuGpu), pad = pad,));
        }
    }

    for group in &data.sensors {
        if is_known_sensor(&group.name) {
            continue; // Already shown in a named section, so don't list it twice.
        }
        render_section(&mut tooltip, &group.name, &group.readings, SensorKind::Motherboard, pad);
    }

    Ok(serde_json::to_string(&json!({"text": text, "tooltip": format!("<tt>{tooltip}</tt>")}))?)
}
