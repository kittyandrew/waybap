# Open-Meteo Weather API Integration

Replaces the previous wttr.in integration, which became unreliable in early 2026 (frequent timeouts, aggressive rate-limiting). See [GitHub issue #1175](https://github.com/chubin/wttr.in/issues/1175).

Open-Meteo is free, keyless, open-source, and uses ECMWF models (best accuracy for European locations).

## API endpoint

Single request for all weather data:

```
GET https://api.open-meteo.com/v1/forecast
```

### Query parameters

| Parameter | Value |
|---|---|
| `latitude` | Decimal degrees (e.g. `50.45`) |
| `longitude` | Decimal degrees (e.g. `30.52`) |
| `current` | `temperature_2m,apparent_temperature,weather_code,wind_speed_10m,wind_direction_10m,relative_humidity_2m,is_day` |
| `hourly` | `apparent_temperature,weather_code,precipitation_probability,cloud_cover,snowfall,visibility,is_day` |
| `daily` | `weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset` |
| `timezone` | `auto` (returns all times in the location's local timezone -- no chrono timezone conversion needed) |
| `forecast_days` | `3` (matches old wttr.in default; keeps tooltip manageable) |

Wind speed defaults to km/h, which matches our display format. No explicit `wind_speed_unit` needed.

### Example request

```
https://api.open-meteo.com/v1/forecast?latitude=50.45&longitude=30.52&current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,wind_direction_10m,relative_humidity_2m,is_day&hourly=apparent_temperature,weather_code,precipitation_probability,cloud_cover,snowfall,visibility,is_day&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset&timezone=auto&forecast_days=3
```

### Error responses

On invalid parameters, Open-Meteo returns HTTP 400 with:

```json
{
  "error": true,
  "reason": "Latitude must be in range of -90 to 90. Given: 999.0."
}
```

Always read the response body first (even on non-success status), then check `response.status().is_success()`. On non-success, try to parse the body for `"reason"` and log it. On success, check for `"error": true` in the parsed JSON body and log `"reason"` if present. Return `None` from `query()` in either case.

## Response format

The response uses a **columnar layout** -- each field is a parallel array rather than an array of objects. This is fundamentally different from wttr.in's nested-object style.

Below is a real response for Kyiv (2026-03-03), truncated for readability. All hourly arrays contain exactly `forecast_days * 24` elements (72 for 3 days). All daily arrays contain exactly `forecast_days` elements (3).

```json
{
  "latitude": 50.4375,
  "longitude": 30.5,
  "elevation": 169.0,
  "timezone": "Europe/Kiev",
  "timezone_abbreviation": "GMT+2",
  "utc_offset_seconds": 7200,

  "current": {
    "time": "2026-03-03T17:45",
    "interval": 900,
    "temperature_2m": 3.1,
    "apparent_temperature": -0.6,
    "weather_code": 3,
    "wind_speed_10m": 8.0,
    "wind_direction_10m": 225,
    "relative_humidity_2m": 67,
    "is_day": 0
  },

  "hourly": {
    "time": ["2026-03-03T00:00", "2026-03-03T01:00", "2026-03-03T02:00", "...72 total"],
    "apparent_temperature": [-1.0, -1.3, -1.1, "..."],
    "weather_code": [3, 3, 3, "..."],
    "precipitation_probability": [18, 25, 20, "..."],
    "cloud_cover": [82, 84, 77, "..."],
    "snowfall": [0.00, 0.00, 0.00, "..."],
    "visibility": [33500.0, 31300.0, 29640.0, "..."],
    "is_day": [0, 0, 0, "..."]
  },

  "daily": {
    "time": ["2026-03-03", "2026-03-04", "2026-03-05"],
    "weather_code": [3, 3, 3],
    "temperature_2m_max": [4.6, 7.8, 5.4],
    "temperature_2m_min": [1.0, -0.1, -0.4],
    "precipitation_probability_max": [25, 45, 10],
    "sunrise": ["2026-03-03T06:37", "2026-03-04T06:35", "2026-03-05T06:33"],
    "sunset": ["2026-03-03T17:42", "2026-03-04T17:43", "2026-03-05T17:45"]
  }
}
```

### Type reference

| Field | Type | Notes |
|---|---|---|
| `temperature_2m`, `apparent_temperature`, `snowfall`, `visibility`, `wind_speed_10m` | `f64` | |
| `weather_code`, `relative_humidity_2m`, `precipitation_probability`, `cloud_cover`, `is_day`, `wind_direction_10m` | `i32` | `is_day`: 0=night, 1=day |
| `time` (current/hourly) | `String` | `"YYYY-MM-DDTHH:MM"` local time, no TZ suffix |
| `time` (daily) | `String` | `"YYYY-MM-DD"` |
| `sunrise`, `sunset` | `String` | `"YYYY-MM-DDTHH:MM"` local time |

### Key differences from wttr.in

- **Columnar arrays** instead of array-of-objects for hourly/daily data.
- **Numeric values** (`f64`/`i32`) for all data, not strings. The `serde_aux` crate (`deserialize_number_from_string`) is no longer needed.
- **ISO 8601 times** (`"2026-03-03T14:00"`) instead of wttr.in's integer-as-string (`"1400"`). The `format_day_time` helper in `utils.rs` needs a full rewrite (no more `%I:%M %p` parsing).
- **No weather description text** -- must map WMO codes to strings ourselves (see constants section).
- **No location name** in the response -- resolved separately (see Location section).
- **24 hourly entries per day** instead of wttr.in's 8 (3-hour intervals). We filter to 3-hour intervals at display time.
- **No `chanceof*` fields** -- `format_chances` in `utils.rs` is replaced by the new conditions line logic.

## Deserialization structs (Rust)

Sketch of the serde structs for the columnar format:

```rust
#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: CurrentWeather,
    hourly: HourlyWeather,
    daily: DailyWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    time: String,
    temperature_2m: f64,
    apparent_temperature: f64,
    weather_code: i32,
    wind_speed_10m: f64,
    wind_direction_10m: i32,
    relative_humidity_2m: i32,
    is_day: i32,
}

#[derive(Deserialize)]
struct HourlyWeather {
    time: Vec<String>,
    apparent_temperature: Vec<f64>,
    weather_code: Vec<i32>,
    precipitation_probability: Vec<i32>,
    cloud_cover: Vec<i32>,
    snowfall: Vec<f64>,
    visibility: Vec<f64>,
    is_day: Vec<i32>,
}

#[derive(Deserialize)]
struct DailyWeather {
    time: Vec<String>,
    weather_code: Vec<i32>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    precipitation_probability_max: Vec<i32>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
}
```

## Query wrapper format

The `query()` function makes up to two HTTP requests (geolocation + Open-Meteo) and wraps the result so `parse_data()` has access to the location name. The cached JSON has this shape:

```json
{
  "location_name": "Kyiv, Ukraine",
  "data": { /* raw Open-Meteo response */ }
}
```

- `location_name` is a string or `null`. Comes from `WAYBAP_LOCATION_NAME` env var, or auto-detected from geolocation, or `null` if unavailable.
- `data` is the raw Open-Meteo response object, deserialized into the structs above.
- This wrapper is what gets written to `~/.cache/waybap/weather.json` and what `parse_data()` receives.
- The `--cache` test path works correctly since the wrapper is persisted.

## WMO weather codes

Open-Meteo uses WMO Weather Interpretation Codes (28 codes). These replace the old wttr.in (WWO) code table in `constants.rs` entirely.

Codes are not contiguous (0,1,2,3,45,48,51,...). Binary search on a sorted array works well.

Thunderstorm codes (95, 96, 99) are only reliably reported in Central Europe. Outside that region, the API simply won't return them -- no special handling needed, just have the mappings ready.

**Unknown codes**: If a code is not in the table, use a fallback: icon `"?"`, description `"Unknown"`. This guards against future API changes.

### Icon + description table

Day/night variants apply only to codes 0 and 1 (clear/mainly-clear). All other codes use the same icon regardless. Implementation: special-case codes 0/1 in the lookup function rather than doubling the table.

```rust
// Suggested implementation:
fn get_icon(code: i32, is_day: bool) -> &'static str {
    if !is_day && (code == 0 || code == 1) {
        return "🌙";
    }
    // ... normal table lookup with fallback
}
```

| Code | Description | Day | Night |
|------|---|---|---|
| 0 | Clear sky | ☀️ | 🌙 |
| 1 | Mainly clear | 🌤️ | 🌙 |
| 2 | Partly cloudy | ⛅ | ⛅ |
| 3 | Overcast | ☁️ | ☁️ |
| 45 | Fog | 🌫️ | 🌫️ |
| 48 | Depositing rime fog | 🌫️ | 🌫️ |
| 51 | Light drizzle | 🌧️ | 🌧️ |
| 53 | Moderate drizzle | 🌧️ | 🌧️ |
| 55 | Dense drizzle | 🌧️ | 🌧️ |
| 56 | Light freezing drizzle | 🌧️ | 🌧️ |
| 57 | Dense freezing drizzle | 🌧️ | 🌧️ |
| 61 | Slight rain | 🌦️ | 🌦️ |
| 63 | Moderate rain | 🌧️ | 🌧️ |
| 65 | Heavy rain | 🌧️ | 🌧️ |
| 66 | Light freezing rain | 🌨️ | 🌨️ |
| 67 | Heavy freezing rain | 🌨️ | 🌨️ |
| 71 | Slight snowfall | 🌨️ | 🌨️ |
| 73 | Moderate snowfall | 🌨️ | 🌨️ |
| 75 | Heavy snowfall | 🌨️ | 🌨️ |
| 77 | Snow grains | 🌨️ | 🌨️ |
| 80 | Slight rain showers | 🌦️ | 🌦️ |
| 81 | Moderate rain showers | 🌧️ | 🌧️ |
| 82 | Violent rain showers | 🌧️ | 🌧️ |
| 85 | Slight snow showers | 🌨️ | 🌨️ |
| 86 | Heavy snow showers | 🌨️ | 🌨️ |
| 95 | Thunderstorm | 🌩️ | 🌩️ |
| 96 | Thunderstorm with slight hail | 🌩️ | 🌩️ |
| 99 | Thunderstorm with heavy hail | 🌩️ | 🌩️ |

## Rate limits

Free tier (no key, no signup):

| Window | Limit |
|---|---|
| Per minute | 600 |
| Per hour | 5,000 |
| Per day | 10,000 |

Requests with >10 weather variables or >2 weeks of data count as >1 API call. Our request has 18 variables across current/hourly/daily but only 3 forecast days, which may count as ~1.5-1.8 calls. At one query every 10 minutes (~144 requests/day), even at 1.8x weight that's ~260 effective calls/day -- well within limits.

## Location resolution

### Coordinate resolution

Priority chain:

1. **Env vars `WAYBAP_LAT` + `WAYBAP_LON`** -- both must be set. If only one is set, log a warning ("WAYBAP_LAT and WAYBAP_LON must both be set, ignoring partial config") and return `None` (fail fast -- the user intended manual config, so silently falling back to IP geolocation for a different city would be worse). Values are validated against API bounds (lat: -90..90, lon: -180..180) before caching; out-of-range values are rejected with an error log.
2. **IP geolocation fallback** via `https://ipwho.is/` (HTTPS, no key, 60 req/min).

If both fail (env vars incomplete + geolocation service down), `query()` returns `None`. The scheduler retries 3 times with backoff, then waits for the next 10-minute cycle. Stale cache (if any) continues to be served.

### Geolocation caching

The IP geolocation result (coordinates + city/country) is cached **in-memory for the lifetime of the daemon process** using `OnceLock`. Rationale: IP address rarely changes during a desktop session, and re-fetching every 10 minutes is wasteful.

This means geolocation is resolved **once at first query**, not at daemon startup (lazy init).

### Tooltip header display name

- **`WAYBAP_LOCATION_NAME` env var** -- if set, used verbatim as the header text, wrapped in `<span size="large">...</span>`.
- **Auto-detected** -- if not set and geolocation succeeded, format as `"{city}, {country}"` (e.g. `"Kyiv, Ukraine"`).
- **Omitted** -- if no name is available (explicit coords without display name, or geolocation returned no city), skip the header entirely.

### IP geolocation API (ipwho.is)

```
GET https://ipwho.is/
```

HTTPS supported on free tier (unlike ip-api.com which is HTTP-only). No API key required.

**Known limitation**: VPN/proxy users will get the exit node's location, not their actual location. Users behind a VPN should set `WAYBAP_LAT`/`WAYBAP_LON` explicitly.

**Success response:**

```json
{
  "success": true,
  "ip": "1.2.3.4",
  "city": "Kyiv",
  "region": "Kyiv City",
  "country": "Ukraine",
  "country_code": "UA",
  "latitude": 50.4501,
  "longitude": 30.5234,
  "timezone": { "id": "Europe/Kiev" }
}
```

**Failure response:**

```json
{
  "success": false,
  "message": "..."
}
```

Check the `"success"` field. On failure, log the message and return `None` from `query()`.

### Request timeouts

| Request | Timeout |
|---|---|
| ipwho.is (geolocation) | 3 seconds |
| Open-Meteo (weather) | 10 seconds |

The geolocation timeout is shorter because it's a simpler lookup. The Open-Meteo timeout is longer than the old wttr.in 5s to accommodate occasional slowness.

Both HTTP clients set `User-Agent: waybap/0.1.0` for API hygiene.

## Hourly display

Open-Meteo returns 24 hourly entries per day. We filter to **3-hour intervals** (hours 0, 3, 6, 9, 12, 15, 18, 21) to keep the tooltip compact -- 8 entries per day, matching the density of the old wttr.in format.

### Past-hour filtering

For today's entries, skip hours that are more than 2 hours before the current time. This preserves the existing behavior from the wttr.in implementation.

Example: if it's 14:00, show hours 12, 15, 18, 21 (skip 0, 3, 6, 9 since they're >2 hours ago).

### Stale cache handling

With columnar data, "days" are array indices rather than separate objects. To filter:

1. Parse `daily.time[]` to find the index of today's date (from `current.time` in the cached response).
2. If found, slice all arrays from that index onward.
3. If not found, start from index 0 (show all data as-is; stale forecast is better than nothing).

Hourly arrays correspond to daily arrays: day `i` covers hourly indices `i*24` through `(i+1)*24 - 1`.

**Today/Tomorrow labels** use `chrono::Local::now().date_naive()` (the system clock) rather than the cached `current.time` date. This ensures stale cache data (e.g., network down for days) doesn't misleadingly label old dates as "Today". The system clock is only used for labels -- hour filtering still uses the API response time for timezone correctness.

## Temperature display

All temperatures are rounded to the nearest integer for display: `temp.round() as i32`. This keeps the tooltip clean and aligned, matching the existing style.

## Wind direction

The `wind_direction_10m` field (degrees, 0-360) is converted to a compass direction for the current conditions line:

```
Wind: 12 km/h NW
```

Conversion: normalize with `rem_euclid(360)` (handles negative degrees), then divide into 8 sectors of 45 degrees each. `["N","NE","E","SE","S","SW","W","NW"][(direction.rem_euclid(360) as f64 / 45.0).round() as usize % 8]`.

## Daily header

Each forecast day shows a header line with max/min temps, precipitation probability, sunrise, and sunset:

```
Today, 03.03 2026
  4° /  1°  Precip 25%  06:37 - 17:42
```

- `precipitation_probability_max` from the daily data, shown when > 0.
- Sunrise/sunset extracted from the ISO 8601 strings (just take the `THH:MM` part).
- "Today"/"Tomorrow" labels use actual date comparison, not array index.

## Conditions line algorithm (per hourly entry)

Each line format: `HH ICON TEMP° DESC[, extras]`

The base line always includes hour, icon, temperature (rounded), and WMO description. Additional fields are appended conditionally:

| Field | Show when | Format |
|---|---|---|
| `precipitation_probability` | > 0 | `Precip X%` |
| `snowfall` | > 0.0 | `Snow X.Xcm` |
| `visibility` | < 1000m | `Vis Xm` (rounded to integer) |
| `cloud_cover` | weather_code is 0, 1, or 2 (clear/mainly-clear/partly-cloudy) | `Clouds X%` |

Cloud cover is only shown for clear-ish weather (codes 0-2) because for overcast/rain/snow/fog the description already implies cloudiness.

Example output:
```
12 ☀️   3° Partly cloudy, Precip 15%, Clouds 40%
15 🌧️   1° Moderate rain, Precip 85%
18 🌨️  -2° Moderate snowfall, Precip 90%, Snow 1.2cm, Vis 500m
21 🌙  -4° Clear sky
```

## Color-coded temperatures

Use the existing Catppuccin Frappe palette to color temperature values in Pango markup:

| Range | Color | Name |
|---|---|---|
| <= -10 | `#949cbb` | muted (extreme cold) |
| -9 to 0 | `#8caaee` | blue (cold) |
| 1 to 15 | (default/white) | neutral |
| 16 to 30 | `#ef9f76` | peach (warm) |
| >= 31 | `#e78284` | red (hot) |

Applied to temperature values in both the hourly entries and the daily max/min header.

## Tooltip rendering

The tooltip is wrapped in `<tt>...</tt>` for monospace rendering, matching the crypto and sensors modules. This ensures `{temp: >3}` alignment padding works correctly in Waybar's Pango renderer.

## Files affected by migration

| File | Change |
|---|---|
| `weather/query.rs` | Full rewrite: geolocation + Open-Meteo requests, JSON wrapping, new timeouts |
| `weather/parsing.rs` | Full rewrite: new deserialization structs (columnar), new tooltip formatting, temperature rounding, color coding |
| `weather/constants.rs` | Replace entire weather code table (62 WWO entries -> 28 WMO entries + descriptions). New `get_icon(code, is_day)` and `get_description(code)` functions. Add unknown-code fallback. |
| `weather/utils.rs` | Full rewrite: remove `format_day_time` (no more `%I:%M %p` parsing), remove `format_chances` and `CHANCES` array, add wind direction helper, add conditions line builder |
| `weather/mod.rs` | Update doc comment (remove wttr.in reference) |
| `Cargo.toml` | Remove `serde_aux` if no longer used elsewhere |

## Design decisions log

| Question | Decision | Alternatives considered |
|---|---|---|
| Weather API provider | Open-Meteo (free, keyless, ECMWF models) | WeatherAPI.com (easiest migration but needs API key), Pirate Weather (US-focused), Tomorrow.io (tight limits) |
| Location input | Env vars for lat/lon, IP geolocation fallback (ipwho.is) | Hardcoded coords, geocoding from city name, config file, ip-api.com (HTTP-only) |
| IP geolocation API | ipwho.is (HTTPS, no key, 60 req/min) | ip-api.com (HTTP-only, privacy risk), ipapi.co (1000/day limit, "dev only") |
| Geolocation caching | In-memory OnceLock, resolved once per daemon lifetime | Re-fetch every cycle, cache to disk with TTL |
| Tooltip header | Optional: env var > auto-detected "City, Country" > omitted | "City, Region, Country", coordinates only, always show |
| Hourly granularity | Every 3 hours (8 entries/day) | Every hour (24/day), every 2 hours (12/day) |
| Conditions line | Precip% + snow + visibility + clouds (when clear-ish) | Single precip% only, always show clouds, drop conditions entirely |
| Weather icons | Day/night variants for codes 0-1 via `is_day` field | Single icon per code regardless of time |
| Temperature display | Rounded to integer | One decimal place |
| Query wrapper | Wrap Open-Meteo response with `{"location_name": ..., "data": ...}` | parse_data reads env vars directly, change function signatures |
| Wind display | Show compass direction from `wind_direction_10m` | Speed only, no direction |
| Daily precip | Show `precipitation_probability_max` in day header | Omit, show per-hour only |
| Temp colors | Catppuccin Frappe palette by range | No coloring, single color |

## Review findings & decisions log

Tracks issues found during adversarial code review rounds, decisions made, and whether they were fixed or dismissed. This prevents re-discussing the same items across sessions.

### Round 1-2 (initial implementation review)

| Finding | Decision | Status |
|---|---|---|
| OnceLock caches failure permanently | Changed to only cache on success; failures retry next cycle | Fixed |
| `Local::now()` uses system TZ, but API returns location-local times | Parse `current.time` from API response instead of using `Local::now()` (Approach A). Approach B (utc_offset_seconds) rejected: degrades poorly on stale cache | Fixed |
| `.unwrap()` on `Client::builder().build()` | Changed to `match` with error logging | Fixed |
| Pango escaping missing on `location_name` | Added `crate::pango::escape()` call | Fixed |
| `color_temp` alignment broken for multi-digit temps | Use `{temp: >3}` format specifier for right-aligned 3-char width | Fixed |
| `HashMap` produces non-deterministic JSON key ordering | Replaced with `serde_json::json!()` macro in all 3 modules (weather, crypto, sensors) | Fixed |

### Round 3 (null values & timezone)

| Finding | Decision | Status |
|---|---|---|
| Open-Meteo may return null values in hourly/daily arrays | Researched: zero nulls observed at `forecast_days=3` across 15+ global locations. Nulls only appear at 14-16 day forecast tails. Skip adding `Option<T>` handling | Dismissed (not applicable at 3 days) |

### Round 4

| Finding | Severity | Decision | Status |
|---|---|---|---|
| Weather tooltip missing `<tt>` monospace wrapper (crypto/sensors have it) | MEDIUM | Add `<tt>` wrapper for consistency and alignment | Fixed |
| Open-Meteo 400 error body never read (reason not logged) | MEDIUM | Restructure: read body on non-success status, parse and log `"reason"` field | Fixed |
| `u32::is_multiple_of()` is nightly-only | MEDIUM | **Investigated: STABLE since Rust 1.87.0 (May 2025).** Critic was wrong. No change needed | Dismissed (stable API) |
| Invalid lat/lon from env vars cached in OnceLock forever | MEDIUM | Validate ranges (-90..90 lat, -180..180 lon) before caching | Fixed |
| `#[allow(dead_code)]` on entire `DailyWeather` struct | LOW | Move to just the `weather_code` field | Fixed |
| `wind_direction()` silently returns "N" for negative degrees | LOW | Add `.rem_euclid(360)` before division for correctness | Fixed |
| No User-Agent header on HTTP requests | LOW | Add `waybap/0.1.0` User-Agent to both clients | Fixed |
| Daily `weather_code` deserialized but unused | LOW | Intentional: field required for deserialization, not displayed in header. Spec example doesn't include daily icon | Dismissed (by design) |
| No parallel array length validation for hourly data | MEDIUM | Open-Meteo API is well-behaved; arrays are always parallel at 3 days. Defensive checks would add complexity for a scenario that never occurs | Dismissed (API contract) |
| `now_hour` staleness (15-min API granularity + 10-min cache) | MEDIUM | By design: inherent lag of at most ~25 minutes. Acceptable for a desktop weather widget | Dismissed (by design) |
| Weather descriptions not Pango-escaped | LOW | All 28 descriptions are safe static ASCII. Developer-controlled constants | Dismissed (safe) |
| OnceLock race condition (double geolocation resolve) | LOW | Single-threaded scheduler; benign even if multi-threaded (OnceLock::set is safe) | Dismissed (benign) |
| `location_name` re-read every 10min vs lat/lon cached forever | LOW | Correct per spec: coordinates are stable per session, display name could change | Dismissed (by design) |

### Round 5

| Finding | Severity | Decision | Status |
|---|---|---|---|
| Partial `WAYBAP_LAT`/`WAYBAP_LON` silently falls through to IP geolocation | MEDIUM | Add `return None` after warning -- fail fast since user intended manual config | Fixed |
| `crypto/query.rs` `.expect()` on `Client::builder().build()` panics + aborts daemon | MEDIUM | Replace with `match` + error log + `return None`, consistent with weather module | Fixed |
| Stale cache labels: `has_today=false` dead code, old dates labeled "Today" | LOW | Use `Local::now().date_naive()` for labels (not hour filtering). Removed dead `has_today` code path | Fixed |
| Bar `text` field contains `\n` newline (two-line stacked display) | MEDIUM | Intentional stacked layout (icon above, temp below). No change | Dismissed (intentional) |
| `{temp: >3}` alignment breaks for temps below -99 | LOW | Earth temps never reach -100C even with wind chill. Theoretical edge case | Dismissed (unrealistic) |
| Every HTTP request re-parses + re-renders weather data | LOW | Architecture change, not a bug. 10-min data re-rendered per request is negligible | Dismissed (acceptable) |
| `nvidia-smi` thread + process leak on GPU hang | LOW | Sensors module, out of scope for weather migration. Separate concern | Dismissed (out of scope) |
| `succ_opt().unwrap_or(today)` dead code on NaiveDate::MAX | LOW | Cannot trigger (year 262143). Harmless, not worth changing | Dismissed (unreachable) |
