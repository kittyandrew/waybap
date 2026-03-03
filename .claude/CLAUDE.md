# waybap

Custom data provider for Waybar/Hyprland. A small Rust daemon that fetches weather (Open-Meteo), crypto (CoinGecko), and hardware sensor data on a schedule, caches results to `~/.cache/waybap/`, and serves parsed Pango-markup JSON over HTTP for Waybar custom modules.

## Project structure

```
src/
  main.rs              CLI entry point (serve, test subcommands; hand-rolled arg parsing)
  pango.rs             Shared Pango/XML escape utility
  server.rs            tiny_http server, routes: /api/weather, /api/crypto, /api/sensors
  scheduler.rs         Job scheduler: periodic fetch with retries, atomic cache writes (tmp + rename)
  crypto/
    mod.rs             Re-exports query + parse_data
    query.rs           CoinGecko API client (top 10 coins by market cap)
    parsing.rs         Deserialize coins, format Pango markup for Waybar
  weather/
    mod.rs             Re-exports query + parse_data
    query.rs           Open-Meteo API client (current + hourly + daily forecast)
    parsing.rs         Deserialize weather JSON, format Pango markup tooltip with forecast
    constants.rs       WMO weather code -> emoji/description mapping (binary search)
    utils.rs           Helpers: wind direction, conditions line builder
  sensors/
    mod.rs             Re-exports query + parse_data; defines SensorReading/SensorGroup/SensorData structs
    query.rs           Read /sys/class/hwmon + nvidia-smi, return JSON
    parsing.rs         Format sensor temps as color-coded Pango markup
```

## Docs

**Important**: Always keep this index up to date when creating, renaming, or deleting files in `docs/`. Every doc file must be listed here.

```
docs/
  open-meteo-spec.md   Open-Meteo API integration spec: endpoints, response format, WMO codes, design decisions
```

## Build & test

- Nix flake project. Use `nix develop` for dev shell, `nix build` for release.
- `cargo build`, `cargo clippy`, `cargo fmt` for standard Rust workflow.
- `cargo run -- serve [address]` starts the daemon (default: 127.0.0.1:6969).
- `cargo run -- test <weather|crypto|sensors> [--cache]` runs a full query+parse cycle for testing. Use `--cache` to test parsing against cached data without network.
- `rustfmt.toml`: max_width = 121.

## Conventions

- No external arg parser -- CLI is hand-rolled in main.rs.
- Output is Pango markup JSON (`{"text": "...", "tooltip": "..."}`) consumed by Waybar.
- Error handling: parsing functions return `Result<String, Box<dyn std::error::Error>>`, query functions return `Option<String>`. Prefer `?` and `.ok_or()` over `unwrap()`.
- Colors: `#e78284` (red/negative/hot), `#a6d189` (green/positive), `#949cbb` (muted/N/A/extreme-cold), `#8caaee` (blue/cold), `#F7931A` (bitcoin orange), `#e5c890` (yellow/warm), `#ef9f76` (peach/hot/warm). These are Catppuccin Frappe palette colors.
- Cache files live at `~/.cache/waybap/{name}.json`, written atomically (tmp + rename).
- Scheduler threads are per-job, no shared state needed.
- Each module (weather, crypto, sensors) exposes two public functions: `query() -> Option<String>` and `parse_data(Value) -> Result<String, ...>`.
