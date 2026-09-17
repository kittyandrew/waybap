# waybap

Custom data provider for Waybar/Hyprland. A small Rust daemon that polls a few sources on their own
schedules and serves the results over HTTP, so Waybar modules read a local endpoint instead of each
spawning a script.

Providers: weather, crypto, sensors and system usage. Output is Pango-formatted and themed with
Catppuccin.

## Usage

```bash
waybap serve [address]                 # daemon, default 127.0.0.1:6969
waybap test <weather|crypto|sensors|usage> [--cache]
```

`waybap test` fetches and parses live data once, or replays the cache with `--cache`.

## Build

```bash
nix build .#waybap
nix run .#waybap -- serve
```

## Home Manager

The flake exports a Home Manager module as `homeManagerModules.waybap` (aliased as `homeManagerModule`):

```nix
imports = [inputs.waybap.homeManagerModules.waybap];
```

## Binary cache

Builds are published to `cache.kittyandrew.dev`, so Nix can download these outputs instead of rebuilding them.

On NixOS:

```nix
nix.settings = {
  extra-substituters = ["https://cache.kittyandrew.dev/nix-cache"];
  extra-trusted-public-keys = ["cache.kittyandrew.dev-1:yy5fdErj1riKOjND10kzD5mp0L8/C8RFG3VkMizhGg4="];
};
```

Elsewhere, in `~/.config/nix/nix.conf` (or `/etc/nix/nix.conf` for all users):

```
extra-substituters = https://cache.kittyandrew.dev/nix-cache
extra-trusted-public-keys = cache.kittyandrew.dev-1:yy5fdErj1riKOjND10kzD5mp0L8/C8RFG3VkMizhGg4=
```

The `extra-` prefixes append rather than replace, so `cache.nixos.org` keeps working. The cache is read-only
and needs no credentials; it serves only what this repository's flake builds.
