# waybap

A small Rust daemon that provides weather and crypto data for [Waybar](https://github.com/Alexays/Waybar) custom modules. It fetches data from [wttr.in](https://wttr.in) and [CoinGecko](https://www.coingecko.com/) on a schedule, caches it locally, and serves parsed [Pango markup](https://docs.gtk.org/Pango/pango_markup.html) JSON over HTTP.

## Usage

```sh
# Start the daemon (default: 127.0.0.1:6969)
waybap serve

# Start on a custom address
waybap serve 127.0.0.1:6963

# Test a module without starting the daemon
waybap test weather
waybap test crypto

# Test parsing against cached data (no network)
waybap test weather --cache
```

## NixOS / Home Manager

Add waybap as a flake input:

```nix
# flake.nix
inputs.waybap.url = "github:kittyandrew/waybap";
```

### Option 1: Home Manager module

Import the module and enable the service. This installs the binary and creates a systemd user service that starts with your Hyprland session.

```nix
imports = [ inputs.waybap.homeManagerModule ];

services.waybap.enable = true;
```

### Option 2: Manual setup

Install the package and start it yourself (e.g. via Hyprland's `exec-once`):

```nix
environment.systemPackages = [
  inputs.waybap.packages.${pkgs.stdenv.hostPlatform.system}.default
];
```

```nix
wayland.windowManager.hyprland.settings.exec-once = [
  "waybap serve 127.0.0.1:6963"
];
```

### Waybar config

Point your Waybar custom modules at the API:

```nix
# waybar config
"custom/weather" = {
  format = "{}";
  tooltip = true;
  interval = 10;
  exec = "curl -s http://127.0.0.1:6963/api/weather";
  return-type = "json";
};
"custom/crypto" = {
  format = "{}";
  tooltip = true;
  interval = 10;
  exec = "curl -s http://127.0.0.1:6963/api/crypto";
  return-type = "json";
};
```

## API

| Endpoint | Description |
|---|---|
| `GET /api/weather` | Weather data (fetched every 10 min) |
| `GET /api/crypto` | Top 10 crypto by market cap (fetched every 15 min) |

Both return `{"text": "...", "tooltip": "..."}` with Pango markup, compatible with Waybar's `return-type = "json"`.

## Building from source

```sh
# With Nix
nix build

# With Cargo
cargo build --release
```
