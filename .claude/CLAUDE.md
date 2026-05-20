# waybap

Custom data provider for Waybar/Hyprland. A small Rust daemon that fetches weather (Open-Meteo), crypto (CoinGecko), hardware sensor data, and AI usage tracking (Claude + Codex) on a schedule, caches results to `~/.cache/waybap/`, and serves parsed Pango-markup JSON over HTTP for Waybar custom modules.

## Project structure

```
src/
  main.rs              CLI entry point (serve, test subcommands; hand-rolled arg parsing)
  catppuccin.rs        Catppuccin Frappe color palette constants (single source of truth)
  pango.rs             Shared Pango/XML escape, capitalize, and meter bar utilities
  server.rs            tiny_http server, routes: /api/weather, /api/crypto, /api/sensors, /api/usage
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
  usage/
    mod.rs             Re-exports query + parse_data
    query.rs           Claude + Codex OAuth credential loading, usage API fetch, status pages
    parsing.rs         Format usage data as color-coded Pango markup with meter bars
```

## Specs

**Important**: Always keep this index up to date when creating, renaming, or deleting files in `docs/`.

Single Allium specification covers the full system. Read `docs/specs/waybap.allium` before making changes to scheduling, formatting, color coding, or API behavior — it captures the design decisions and constraints. Run `allium check docs/specs/waybap.allium` to validate.

```
docs/specs/
  waybap.allium   Full system spec: scheduler, server, all 4 data modules, display formatting
```

## Build & test

- Nix flake project. Use `nix develop` for dev shell, `nix build` for release.
- CI-equivalent local checks:
  - `nix develop -c actionlint`
  - `nix develop -c zizmor .github/workflows`
  - `nix develop -c alejandra -c .`
  - `nix develop -c deadnix flake.nix hmModule.nix`
  - `nix flake check --print-build-logs`
  - `nix build .#waybap --print-build-logs`
  - `nix develop -c cargo fmt --check`
  - `nix develop -c cargo clippy --all-targets --all-features -- -D warnings`
  - `nix develop -c cargo test --all-targets --all-features --locked`
  - `nix develop -c cargo build --all-targets --all-features --locked`
- Project-specific spec check: `allium check docs/specs/waybap.allium`.
- `cargo run -- serve [address]` starts the daemon (default: 127.0.0.1:6969).
- `cargo run -- test <weather|crypto|sensors|usage> [--cache]` runs a full query+parse cycle for testing. Use `--cache` to test parsing against cached data without network.
- `rustfmt.toml`: max_width = 121.

## Conventions

- No external arg parser -- CLI is hand-rolled in main.rs.
- Output is Pango markup JSON (`{"text": "...", "tooltip": "..."}`) consumed by Waybar.
- Error handling: parsing functions return `Result<String, Box<dyn std::error::Error>>`, query functions return `Option<String>`. Prefer `?` and `.ok_or()` over `unwrap()`.
- Colors: Catppuccin Frappe palette, centralized in `src/catppuccin.rs`. All modules use `crate::catppuccin::*` constants — never inline hex strings.
- Cache files live at `~/.cache/waybap/{name}.json`, written atomically (tmp + rename).
- Scheduler threads are per-job, no shared state needed.
- Each data module exposes two public functions: `query() -> Option<String>` and `parse_data(Value) -> Result<String, ...>`. See `mod.rs` in any module for the pattern.
