# waybap

Custom data provider for Waybar/Hyprland. A small Rust daemon that fetches weather (wttr.in) and crypto (CoinGecko) data on a schedule, caches it to `~/.cache/waybap/`, and serves parsed Pango-markup JSON over HTTP for Waybar custom modules.

## Project structure

```
src/
  main.rs              CLI entry point (serve, test subcommands)
  server.rs            tiny_http server, routes: /api/weather, /api/crypto
  scheduler.rs         Job scheduler: periodic fetch with retries, atomic cache writes
  crypto/
    mod.rs             Re-exports query + parse_data
    query.rs           CoinGecko API client (top 10 by market cap)
    parsing.rs         Deserialize coins, format Pango markup for Waybar
  weather/
    mod.rs             Re-exports query + parse_data
    query.rs           wttr.in API client
    parsing.rs         Deserialize weather, format Pango markup for Waybar
    constants.rs       Weather code -> emoji mapping (binary search)
    utils.rs           Helpers: format_day_time, format_chances
```

## Build & test

- Nix flake project. Use `nix develop` for dev shell, `nix build` for release.
- `cargo build`, `cargo clippy`, `cargo fmt` for standard Rust workflow.
- `cargo run -- serve [address]` starts the daemon (default: 127.0.0.1:6969).
- `cargo run -- test <weather|crypto> [--cache]` runs a full query+parse cycle for testing. Use `--cache` to test parsing against cached data without network.
- `rustfmt.toml`: max_width = 121.

## Conventions

- No external arg parser — CLI is hand-rolled in main.rs.
- Output is Pango markup JSON (`{"text": "...", "tooltip": "..."}`) consumed by Waybar.
- Error handling: parsing functions return `Result<String, Box<dyn std::error::Error>>`, query functions return `Option<String>`. Prefer `?` and `.ok_or()` over `unwrap()`.
- Colors: `#e78284` (red/negative), `#a6d189` (green/positive), `#949cbb` (muted/N/A), `#F7931A` (bitcoin orange). These are Catppuccin Frappe palette colors.
- Cache files live at `~/.cache/waybap/{name}.json`, written atomically (tmp + rename).
- Scheduler threads are per-job, no shared state needed.
