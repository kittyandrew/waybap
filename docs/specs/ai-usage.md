# AI Usage Tracking Module

Track Claude and Codex rate-limit windows, display them as color-coded Pango markup in Waybar. Based
heavily on insights from [CodexBar](https://github.com/steipete/CodexBar) — a macOS menu bar app
that tracks 22+ AI provider quotas. This spec distills what CodexBar learned about data extraction
into a Linux/Waybar-native implementation scoped to Claude + Codex only.

## Reference: what CodexBar taught us

CodexBar supports three fetch strategies per provider, tried in priority order:

| Strategy | How it works | Accuracy | Speed | Linux viable? |
|----------|-------------|----------|-------|---------------|
| **OAuth API** | Hit provider's usage endpoint with OAuth bearer token | Numeric % from API | 2-3s | Yes |
| **CLI PTY** | Spawn `claude`/`codex` in a PTY, send `/usage`/`/status`, parse TUI output | Text parsing | 5-10s | Possible but fragile |
| **Web scrape** | Use browser session cookies to hit `claude.ai`/`chatgpt.com` web APIs | Numeric % | 2-3s | Cookie extraction is painful on Linux |

**Key CodexBar findings we're adopting:**

1. **OAuth is the right primary strategy.** It returns numeric percentages directly — no text
   parsing, no PTY complexity, no browser cookies. Both Claude Code and Codex CLI already store
   OAuth credentials locally.
2. **Rate-limit windows are fixed durations.** Claude: 5-hour session + 7-day weekly. Codex:
   similar primary + secondary windows. The window *durations* are static; only the reset times
   and utilization percentages are dynamic.
3. **Credentials already exist on disk.** Claude Code stores OAuth tokens at
   `~/.claude/.credentials.json`. Codex stores them at `~/.codex/auth.json`. No login flow needed
   — if the user has the CLI installed and logged in, we just read the file.
4. **Local JSONL logs give token costs for free.** Both CLIs write session logs with per-message
   token counts. Scanning these gives cost tracking without any API call.
5. **Status page polling is cheap and useful.** A single GET to `status.anthropic.com` or
   `status.openai.com` tells us if the provider is having issues — good context for the tooltip.

**What we're NOT adopting from CodexBar:**

- **Token refresh by waybap.** CodexBar refreshes tokens itself, but this risks invalidating the
  CLI's in-memory refresh token via OAuth2 rotation (see decision log D1). We read credentials
  as-is and let the CLI own its token lifecycle.
- CLI PTY probing (complex, slow, fragile — not worth it when OAuth works)
- Browser cookie extraction (platform-specific, privacy-invasive, fragile across browser updates)
- Web dashboard scraping (macOS-only WebKit dependency in CodexBar)
- Historical pace prediction (cool but out of scope — CodexBar needs 3-5 weeks of samples)
- WidgetKit / macOS menu bar rendering (obviously)
- All 20 other providers

**API stability warning:** All endpoint URLs, response field names, and header requirements in
this spec are reverse-engineered from CodexBar's Swift source, not from official API documentation.
These are internal/beta APIs that could change without notice. The `anthropic-beta: oauth-2025-04-20`
header explicitly indicates a pre-release API. Defensive parsing (handle missing/renamed fields
gracefully) is essential. When implementing, verify the wire format against actual API responses
before trusting the spec's models.

## Data sources

### Claude — OAuth usage API

**Endpoint:** `GET https://api.anthropic.com/api/oauth/usage`

**Headers:**
```
Authorization: Bearer {access_token}
Accept: application/json
User-Agent: waybap/0.1.0
anthropic-beta: oauth-2025-04-20
```

**Response model** (verified against live API, Mar 8, 2026):
```json
{
  "five_hour": { "utilization": 45.0, "resets_at": "2026-03-08T15:00:00.123456+00:00" },
  "seven_day": { "utilization": 62.0, "resets_at": "2026-03-10T00:00:00+00:00" },
  "seven_day_opus": null,
  "seven_day_sonnet": { "utilization": 2.0, "resets_at": "2026-03-10T00:00:00+00:00" },
  "seven_day_oauth_apps": null,
  "seven_day_cowork": null,
  "iguana_necktie": null,
  "extra_usage": {
    "is_enabled": true,
    "monthly_limit": 4250,
    "used_credits": 0.0,
    "utilization": null
  }
}
```

@NOTE: All field names are **snake_case**, not camelCase. CodexBar's Swift source mapped
these to camelCase (Swift convention) — the wire format was never camelCase. Verified against
live API on Mar 8, 2026. See D19.

- `utilization` is percentage used (0-100). Remaining = 100 - utilization.
- `resets_at` is ISO 8601 UTC timestamp (with timezone offset, e.g. `+00:00`).
- `extra_usage` is Claude Extra (pay-per-use overflow). `monthly_limit` and `used_credits` are in
  **cents** (divide by 100 for USD at parse time). Only present if enabled on the account.
  Don't use `extra_usage.utilization` in the `Credits` struct or rendering — derive the percentage
  from `used_credits / monthly_limit * 100` when needed. The raw field is preserved in the cache
  since responses are stored as-is.
- `seven_day_opus` / `seven_day_sonnet` are model-specific weekly windows. Which fields are present
  depends on the account's plan tier. Additional category windows (`seven_day_oauth_apps`,
  `seven_day_cowork`) and unknown fields (`iguana_necktie`) also appear — all null in testing.
  To identify model-specific fields programmatically: iterate over all keys matching `seven_day_*`;
  extract the model name by stripping the `seven_day_` prefix, splitting on `_`, capitalizing each
  word, and joining with spaces (e.g., `seven_day_sonnet` → "Sonnet", `seven_day_oauth_apps` →
  "Oauth Apps"). The result is passed through `pango::escape()` for markup safety. Non-null entries
  render in the tooltip; null entries are ignored.

**Credential file:** `~/.claude/.credentials.json`
```json
{
  "claudeAiOauth": {
    "accessToken": "eyJ...",
    "refreshToken": "ort_...",
    "expiresAt": 1741500000000,
    "scopes": ["user:inference", "user:profile", "user:sessions:claude_code"],
    "rateLimitTier": "default_claude_max_20x",
    "subscriptionType": "max"
  }
}
```

@NOTE: Credentials are nested under `claudeAiOauth`, not at the top level. CodexBar's source
assumed a flat structure — the actual Claude Code credential file wraps everything under this
key. Verified on this machine (Mar 8, 2026). See D19.

- `expiresAt` is milliseconds since epoch.
- If `expiresAt` has passed, the access token is expired. waybap does NOT refresh it — see
  decision log D1. Show "token expired" state and serve stale cached data until the CLI refreshes.
- `subscriptionType` (e.g., "max") is available in the credential file but NOT in the usage API
  response. Currently unused — could be used for plan display if desired (see D19 notes).

**Token refresh reference** (for context — waybap does NOT perform this):

`POST https://platform.claude.com/v1/oauth/token` with
`client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e` (public PKCE client, same as Claude Code).
Documented here so future sessions understand the mechanism if the decision is revisited.

### Codex — OAuth usage API

**Endpoint:** `GET https://chatgpt.com/backend-api/wham/usage`

**Headers:**
```
Authorization: Bearer {access_token}
ChatGPT-Account-Id: {account_id}
User-Agent: waybap/0.1.0
Accept: application/json
```

**Response model** (verified against live API, Mar 8, 2026):
```json
{
  "plan_type": "pro",
  "rate_limit": {
    "allowed": true,
    "limit_reached": false,
    "primary_window": {
      "used_percent": 25,
      "reset_at": 1741500000,
      "reset_after_seconds": 18000,
      "limit_window_seconds": 18000
    },
    "secondary_window": {
      "used_percent": 40,
      "reset_at": 1741900000,
      "reset_after_seconds": 554841,
      "limit_window_seconds": 604800
    }
  },
  "credits": {
    "has_credits": false,
    "unlimited": false,
    "balance": "0",
    "approx_local_messages": [0, 0],
    "approx_cloud_messages": [0, 0]
  },
  "code_review_rate_limit": { "...": "separate rate limit for code reviews" },
  "additional_rate_limits": [
    { "limit_name": "GPT-5.3-Codex-Spark", "metered_feature": "codex_bengalfox", "rate_limit": {"..."} }
  ]
}
```

@NOTE: All field names are **snake_case** (see D19). Additional fields beyond CodexBar's model:
`code_review_rate_limit`, `additional_rate_limits`, `reset_after_seconds`, `allowed`,
`limit_reached`, `approx_local_messages`, `approx_cloud_messages`. Currently ignored.

- `reset_at` is unix timestamp (seconds).
- `limit_window_seconds`: 18000 = 5 hours (primary), 604800 = 7 days (secondary).
- `plan_type` is one of: free, go, plus, pro, team, enterprise, etc.
- `balance` is a **string** (not a number) — parse with `str::parse::<f64>()`, fall back to
  `as_f64()` for defensive handling.

**Credential file:** `~/.codex/auth.json`
```json
{
  "tokens": {
    "access_token": "eyJ...",
    "refresh_token": "...",
    "id_token": "eyJ... (JWT with email/plan)",
    "account_id": "uuid"
  },
  "last_refresh": "2026-03-08T10:00:00Z"
}
```

- waybap does NOT refresh Codex tokens either (same reasoning as Claude — see D1).
- If `account_id` is missing, omit the `ChatGPT-Account-Id` header and try the request anyway.
- Codex has no explicit `expires_at` field. Use the access token as-is; if the API returns 401,
  treat it as expired and show the stale state.
- @NOTE: The credential file nests fields under `tokens`. Verified on Linux (Mar 8, 2026) — format
  matches CodexBar's macOS source exactly. Also contains `auth_mode` and `OPENAI_API_KEY` at the
  top level (both ignored by waybap).

**Token refresh reference** (for context — waybap does NOT perform this):

`POST https://auth.openai.com/oauth/token` with
`client_id=app_EMoamEEZ73f0CkXaXp7hrann`. CodexBar refreshes when token is >8 days old.

### Status pages

Lightweight GET, no auth needed. Fetched every query cycle (120s) — no dedicated caching needed
at this interval (see D11). Status data is included in the raw cache alongside usage responses.

- **Anthropic:** `GET https://status.anthropic.com/api/v2/status.json`
- **OpenAI:** `GET https://status.openai.com/api/v2/status.json`

Response (statuspage.io standard format):
```json
{
  "status": {
    "indicator": "none",
    "description": "All Systems Operational"
  },
  "page": {
    "updated_at": "2026-03-08T10:00:00Z"
  }
}
```

Indicator values: `none` (operational), `minor`, `major`, `critical`, `maintenance`.

### Local token cost tracking (phase 2)

Phase 2 — needs its own detailed spec before implementation. Summary of approach:

Parse JSONL session logs for per-message token counts. No API call needed.

**Claude logs:** `~/.claude/projects/*/` — JSONL files, filter for `"type": "assistant"` entries
with `usage` blocks. Deduplicate by `message.id + requestId` (streaming chunks repeat usage).

**Codex logs:** `~/.codex/sessions/{YYYY}/{MM}/{DD}/*.jsonl` — filter for `"type": "event_msg"`
with `total_token_usage`. Track file mtime + byte offset for incremental scanning.

## Architecture

### Module structure

```
src/usage/
  mod.rs          Re-exports query + parse_data
  query.rs        Credential loading + API fetch + status pages, assemble raw cache JSON
  parsing.rs      Format usage data as Pango markup for Waybar
```

Follows the same pattern as weather/crypto/sensors: `query() -> Option<String>` returns raw JSON,
`parse_data(Value) -> Result<String>` formats it. Credential loading is inline in `query.rs` as
private helper functions — matches the existing pattern where each module's query.rs is
self-contained (see D8).

### Data flow

```
query.rs                                         parsing.rs
┌────────────────────────────────────────┐    ┌──────────────────────┐
│ Read existing cache (for merge)        │    │ Deserialize raw      │
│                                        │    │ provider responses   │
│ For each provider:                     │    │                      │
│   1. Read credential file (read-only)  │    │ Format bar text:     │
│   2. Check PATH for CLI binary         │    │   per-provider       │
│   3. Check access token expiry         │    │   stacked lines      │
│   4. If valid: GET /usage              │    │                      │
│   5. If failed: carry forward from     │    │ Format tooltip:      │
│      previous cache                    │    │   Per-provider       │
│                                        │    │   session meter      │
│ Fetch status pages (every cycle)       │    │   weekly meter       │
│                                        │    │   model-specific     │
│ Assemble raw cache:                    │    │   reset countdown    │
│ {                                      │    │   credits (opt)      │
│   timestamp,                           │    │   status line        │
│   claude: { data, data_timestamp,      │    │   freshness footer   │
│             token_expired,             │    │                      │
│             has_credentials, status,   │    │                      │
│             cli_installed },           │    │                      │
│   codex:  { ... same shape }           │    │                      │
│ }                                      │    │                      │
└────────────────────────────────────────┘    └──────────────────────┘
              │
              ▼
    ~/.cache/waybap/usage.json
```

**Cache format:** Stores raw API responses wrapped with metadata. This preserves the original
wire format for debugging, inspired by the weather module's wrapping approach (see D10).

```json
{
  "timestamp": "2026-03-08T12:00:00+00:00",
  "claude": {
    "data": { "five_hour": {"utilization": 45.0, "resets_at": "..."}, "seven_day": {"..."} },
    "data_timestamp": "2026-03-08T12:00:00+00:00",
    "token_expired": false,
    "has_credentials": true,
    "status": { "indicator": "none", "description": "All Systems Operational" },
    "cli_installed": true
  },
  "codex": {
    "data": { "plan_type": "pro", "rate_limit": {"..."} },
    "data_timestamp": "2026-03-08T12:00:00+00:00",
    "token_expired": false,
    "has_credentials": true,
    "status": { "indicator": "minor", "description": "Partial outage" },
    "cli_installed": true
  }
}
```

Provider entries always exist (even without credentials) — they carry `cli_installed` and `status`
for the "not configured" smart message (D13). `has_credentials` distinguishes "not logged in" from
"transient fetch failure". The `data` field is `null` when credentials are missing or fetch failed.

**Partial failure handling:** Each provider is fetched independently. On partial failure (one
provider succeeds, the other fails), `query()` reads the existing cache via
`scheduler::get_cache_fp("usage")` and carries forward the previous entry for the failed
provider — preserving its stale `data` while updating `timestamp`. This prevents a transient
timeout on one provider from wiping the other's last-known-good data (see D14).

@NOTE: This is a novel pattern — no other module's `query()` reads its own cache. It's
thread-safe because each scheduler job runs on a single thread (`query()` → write is sequential
within `tick()`). On first run (no existing cache), partial failure simply produces `null` for
the failed provider — the carry-forward logic only applies when a previous cache exists.

`query()` returns `Some(serialized_json)` when at least one provider has data (fresh or carried
forward) OR when no credentials exist for either provider (to render setup instructions). `None`
is reserved for total failure when credentials exist but all fetches failed, triggering the
scheduler's retry logic.

**Cache file permissions:** Follow user's umask (typically 644). The cache exposes plan type and
credit balances to other local users. Acceptable for single-user workstations; consider explicit
`0600` permissions via `std::fs::set_permissions` if multi-user systems are a concern.

### Shared types

Internal types in `parsing.rs`, used by `parse_data()` — not part of the cache format (cache stores raw responses).

```rust
struct ProviderUsage {
    session: Option<RateWindow>,       // 5-hour window
    weekly: Option<RateWindow>,        // 7-day window
    model_weekly: Vec<(String, RateWindow)>,  // Model-specific weekly, e.g. [("Sonnet", ...)]
    credits: Option<Credits>,
    status: Option<ProviderStatus>,
    plan: Option<String>,              // Codex only ("Pro", "Plus", etc.); Claude API has no plan field
    data_timestamp: Option<String>,    // ISO 8601 UTC — when this provider's data was last fetched
    token_expired: bool,               // true if access token was expired (local or 401/403)
    has_credentials: bool,             // true if credential file existed and parsed
    cli_installed: bool,               // true if CLI binary found on PATH
}

struct RateWindow {
    used_percent: f64,          // 0-100
    resets_at: Option<String>,  // ISO 8601 UTC
}

// @NOTE: All monetary values normalized to USD at parse time.
//   Claude Extra: API returns cents, divide by 100 during deserialization.
//   Codex credits: API returns balance as a string, parsed to f64.
enum Credits {
    ClaudeExtra { used_usd: f64, limit_usd: f64 },
    CodexBalance { balance_usd: f64 },
}

struct ProviderStatus {
    indicator: String,  // "none", "minor", "major", "critical", "maintenance"
    description: String,
}
```

**`RateWindow` notes:** Window labels ("Rate (5h)", "Weekly") are derived from which field
the `RateWindow` occupies in `ProviderUsage`, not from a stored duration value. The Claude API
doesn't return window durations — hardcoding them would silently break if Anthropic changes
windows (see D12).

### Credential handling

Inline in `query.rs` as private functions (see D8). Shared struct for both providers:

```rust
struct OAuthCredentials {
    access_token: String,
    expires_at: Option<u64>,     // ms since epoch (Claude only; Codex has no expiry field)
    account_id: Option<String>,  // Codex only, for ChatGPT-Account-Id header
}

// Read-only. Never writes to credential files.
fn load_claude_credentials() -> Option<OAuthCredentials>;
fn load_codex_credentials() -> Option<OAuthCredentials>;
```

**Safety rules:**

1. **Read-only.** Never create, modify, or write to credential files. If the file doesn't exist,
   the provider is disabled (user hasn't logged in to the CLI).
2. If the file's structure doesn't match the expected format, skip the provider and log a warning
   to stderr. Don't try to "fix" or migrate the file.
3. Tokens and refresh tokens must never appear in cached output (`~/.cache/waybap/usage.json`).
   The cache stores parsed usage data and status, never raw credentials.

### Scheduler integration

New job in `main.rs` alongside weather/crypto/sensors:

```rust
scheduler::Job::new("usage", 120, usage::query).run();
```

**Refresh interval: 120 seconds (2 minutes).** Rationale:
- Weather is 10m, crypto is 15m — usage is more time-sensitive (you want to know when you're
  about to hit a rate limit).
- CodexBar's default refresh is 30s-5m. 2 minutes is a reasonable middle ground that won't
  hammer the APIs.

**Timeout budget:** Build one `reqwest::blocking::Client` with `.timeout(Duration::from_secs(5))`
at the top of `query()` and reuse it for all 4 requests (2 usage APIs + 2 status pages).
`reqwest`'s timeout is per-request, so worst case (all 4 timing out sequentially) = 20 seconds,
well within the 120-second interval. Existing modules were refactored to match this shared-client
pattern (see D16).

**Reset countdowns stay fresh:** `parse_data()` runs on every Waybar HTTP request and computes
countdowns from `resets_at` minus `Utc::now()`. So countdowns update continuously regardless of
the 120-second query interval.

### Server route

`GET /api/usage` — same pattern as `/api/weather`, `/api/crypto`, `/api/sensors`.

Extract the repeated read-cache/parse/serve logic into a shared helper (see D18):
```rust
fn serve_cached_api<F>(request: Request, name: &str, parse: F) -> io::Result<()>
where F: FnOnce(serde_json::Value) -> Result<String, Box<dyn std::error::Error>>
```
Each existing handler (`serve_api_weather`, `serve_api_crypto`, `serve_api_sensors`) becomes a
one-liner delegating to this helper. The usage route uses it directly. Before adding the 4th
copy of identical logic, consolidate the existing three.

### Test subcommand

`cargo run -- test usage [--cache]` — same pattern as other modules. Useful for verifying
credential loading and API responses without starting the server.

### Wiring checklist (completed)

All touchpoints in existing files that were modified when adding the usage module:

- `src/main.rs`: `mod usage;` declaration alongside other modules
- `src/main.rs`: `scheduler::Job::new("usage", 120, usage::query).run();` in serve subcommand
- `src/main.rs`: `"usage"` match arm in test subcommand
- `src/main.rs`: Updated help/usage string to include `usage` in target list
- `src/server.rs`: `(Method::Get, "/api/usage")` route via `serve_cached_api`
- `src/server.rs`: `use crate::usage;` import
- `src/server.rs`: Refactored existing handlers to use `serve_cached_api` (see D18)

## Display format

### Bar text

Vertical bar layout — one line per provider, stacked. Claude on top, Codex below.
Each line: provider initial + color-coded weekly percentage.

```
C 78
X 25
```

- `C` / `X` prefixes identify the provider at a glance.
- Number is weekly `used_percent`, clamped to 0-100 (no `%` suffix — obvious from context,
  saves width). See D9 for rationale on used% vs remaining%, D15 for why weekly over session.
- Color by remaining percentage (100 - used), applied independently per provider:
  - `>50%` remaining: `#a6d189` (green) — plenty of headroom
  - `25-50%` remaining: `#e5c890` (yellow) — getting there
  - `10-25%` remaining: `#ef9f76` (peach) — slow down
  - `<10%` remaining: `#e78284` (red) — about to hit the wall
- If a provider's token is expired: show the number from stale cache in `#949cbb` (muted) with
  a `?` suffix (e.g., `C 42?`) — signals data staleness at a glance (see D17).
- If a provider is not configured: omit its line entirely (don't show "N/A" per provider).
- If both providers are disabled: show a single muted icon.
- Works in both vertical and horizontal Waybar orientations (Waybar renders multi-line custom
  module text in both).

### Tooltip

Rich Pango markup wrapped in `<tt>...</tt>` for monospace rendering (required for meter bar
alignment, matching the pattern in all existing modules).

All API-sourced strings (`plan_type`, status `description`, model names) must be passed through
`pango::escape()` before embedding in tooltip markup — these fields could contain `&` or `<`
characters, especially status descriptions during incidents.

```
<span size="xx-large">AI Usage</span>

━━━ Claude ━━━
Rate (5h):  ████░░░░░░  42%  resets in 2h 10m
Weekly:        ██████░░░░  58%  resets in 3d 12h
Opus weekly:   ████░░░░░░  38%  resets in 3d 12h
Extra: $12.34 / $50.00
✓ Operational

━━━ Codex (Pro) ━━━
Rate (5h):  ███░░░░░░░  25%  resets in 3h 41m
Weekly:        ████░░░░░░  40%  resets in 5d 2h
Credits: $42.50
⚠ Partial outage

Updated 45s ago
```

**Tooltip header:** `<span size="xx-large">AI Usage</span>` matching the `xx-large` pattern used
by sensors and crypto modules (weather is the exception — it uses `large`).

**Section separators:** `━━━ Provider (Plan) ━━━` when plan is known, `━━━ Provider ━━━` when
not — deliberate deviation from existing modules (which use `<b>header</b>`). Only Codex returns
`plan_type`; Claude's API has no plan field, so its separator omits the parenthetical. The usage
tooltip has two distinct provider sections that benefit from stronger visual separation. Test
with the actual Waybar tooltip renderer — `━` characters can have surprising width in some
monospace fonts.

**Meter bar:** 10-character block using `█` (filled = used) and `░` (empty = remaining), colored
by the same thresholds as bar text. Filled blocks represent usage, empty blocks represent remaining
capacity. Color the filled portion by threshold; empty portion in `#949cbb` (muted).

Implement as a shared utility in `pango.rs` (see D13):
```rust
pub fn meter_bar(used_percent: f64, width: usize, filled_color: &str, empty_color: &str) -> String
```

**Reset countdown:** Computed at parse time from `resets_at` timestamp minus `Utc::now()`. Format:
- Negative (past reset): `resetting...` in `#949cbb` (muted) — covers the window between a
  reset occurring and the next query cycle fetching fresh data
- `< 1 minute`: `Xs` (e.g., `42s`)
- `< 1 hour`: `Xm` (e.g., `42m`)
- `1-24 hours`: `Xh Ym` (e.g., `3h 41m`)
- `> 24 hours`: `Xd Yh` (e.g., `3d 12h`)

**Model-specific weekly windows:** Render each non-null model-specific window as an additional
meter line. Label dynamically from the field name: `seven_day_opus` → "Opus weekly:",
`seven_day_sonnet` → "Sonnet weekly:", `seven_day_oauth_apps` → "Oauth Apps weekly:". Strip the
`seven_day_` prefix, split on `_`, capitalize each word, join with spaces. Escaped via
`pango::escape()` before embedding in markup.

**Status indicator:** Use the icon and color from the indicator mapping, but display the API's
`description` field (passed through `pango::escape()`) for incident-specific detail:
- `none` → `<span foreground="#a6d189">✓ {description}</span>`
- `minor` → `<span foreground="#e5c890">⚠ {description}</span>`
- `major` / `critical` → `<span foreground="#e78284">✗ {description}</span>`
- `maintenance` → `<span foreground="#949cbb">⚙ {description}</span>`

**Data freshness footer:** Line at the bottom showing cache age (see D13). Computed from
`Utc::now()` minus the cache's `timestamp` field (written as `Utc::now().to_rfc3339()`, parsed
back with `DateTime::parse_from_rfc3339()`). Uses `Utc` (not `Local`) because API timestamps
are UTC — don't cargo-cult weather's `Local::now()`. Color by age:
- Clock skew / just fetched: `#949cbb` (muted) — `Updated just now`
- Normal (< 4 minutes): `#949cbb` (muted) — `Updated 45s ago`
- Stale (4-10 minutes): `#e5c890` (yellow) — `Updated 6m ago`
- Very stale (> 10 minutes): `#ef9f76` (peach) — `Updated 14m ago`

Durations are clamped to zero to handle clock skew (NTP corrections, VM drift). Negative
durations display as "just now" rather than "-Xs ago".

Useful because `parse_data()` runs on every HTTP request but underlying data only refreshes
every 120 seconds. The color shift from muted to warm provides a passive health indicator —
prolonged staleness signals network issues or expired tokens without requiring the user to read
per-provider status lines.

**Token expired state:** When the access token is expired and data is stale:
```
━━━ Claude ━━━
Token expired — run: claude login
Last data (12m ago):      ← age from per-provider data_timestamp
Rate (5h):  ████░░░░░░  42%  resetting...
Weekly:        ██████░░░░  58%  resets in 3d 12h
```

Note: stale `resets_at` timestamps may have elapsed, producing "resetting..." countdowns. This
is expected — the data reflects the last successful fetch, not current state.

**Provider not configured:** Check whether the CLI binary exists on PATH via manual PATH lookup
(`std::env::var("PATH")` + `split_paths` + `Path::is_file`) — no subprocess spawn, checks that
the match is a regular file (not a directory). This check
runs in `query()` (every 120s) and its result is stored in the cache as `cli_installed: bool`,
so `parse_data()` can render the appropriate message without spawning processes (see D13):

```
━━━ Claude ━━━
Not installed — see claude.ai/cli
```
vs.
```
━━━ Claude ━━━
Not logged in — run: claude login
```

## Error handling

| Scenario | Behavior |
|----------|----------|
| Credential file missing | Provider disabled, show "not configured" in tooltip (smart message) |
| Credential file malformed | Skip provider, log warning to stderr |
| Access token expired | Show "token expired" in tooltip, serve stale cached data |
| API returns 401/403 | Treat as token expired — show stale state (see D21) |
| API returns 429 | Retry on next cycle, no special backoff (see D7). Carry forward stale data on partial failure (see D14) |
| API returns 5xx | Retry on next cycle (normal scheduler retry logic) |
| Network timeout (5s) | Retry on next cycle, show stale data from cache |
| One provider fails, other succeeds | Carry forward stale data for failed provider from previous cache (see D14) |
| Both providers disabled | Bar shows single muted icon, tooltip shows setup instructions |
| Unknown response fields | Ignore unknown fields (serde `deny_unknown_fields` OFF) |
| Missing expected fields | Use `Option` everywhere, degrade gracefully |

## Dependencies

- **HTTP client:** `reqwest::blocking::Client` — already used by weather and crypto modules.
  No new dependency needed.
- **JSON:** `serde_json` + `serde` — already dependencies.
- **Timestamps:** `chrono` — already a dependency (used by scheduler, weather parsing).

## Phases

### Phase 1: OAuth usage windows (MVP)

- `query.rs`: Credential loading + fetch usage from both OAuth endpoints + status pages
- `parsing.rs`: Format bar text + tooltip with meters, countdowns, status, freshness footer
- `pango.rs`: Add shared `meter_bar()` utility
- Server route, scheduler job, test subcommand, wiring in main.rs + server.rs
- This gives: session %, weekly %, reset countdowns, plan type, credits, provider status

### Phase 2: Local token cost tracking

Needs its own detailed spec before implementation. High-level approach:
- Scan `~/.claude/projects/` and `~/.codex/sessions/` JSONL logs
- Incremental scanning (track file mtime + byte offset)
- Calculate per-model token costs using pricing tables
- Add "Today: $X.XX (Yk tokens)" line to tooltip per provider
- Optional "30-day: $X.XX" rollup

### Phase 3 (stretch): CLI fallback

- If OAuth credentials are missing but the CLI binary exists on PATH, try `claude /usage` or
  `codex /status` as a fallback. Parse text output.
- Only worth doing if OAuth proves unreliable or if users don't want to use the Claude Code
  login flow. CodexBar's experience suggests OAuth is strictly better when available.

## Out of scope

- **All providers except Claude and Codex.** No Gemini, Cursor, Copilot, etc.
- **Token refresh by waybap.** See decision log D1.
- **Browser cookie extraction.** Platform-specific, fragile, privacy-invasive.
- **OAuth login flow.** We read existing credentials from CLIs the user already has. If they
  haven't logged in, we tell them to run `claude login` / `codex login`.
- **Historical pace prediction.** CodexBar's recency-weighted median prediction needs 3-5 weeks
  of historical samples and a persistent store. Cool feature, but premature for an MVP.
- **WebView/dashboard scraping.** macOS-specific in CodexBar, not applicable.
- **Notifications.** Waybar itself doesn't support push notifications. If you're at 95% usage,
  the red bar text is the notification.
- **Multi-account support.** CodexBar supports multiple Codex accounts. We support one per
  provider.
- **Codex `config.toml` base URL override.** CodexBar reads `~/.codex/config.toml` for custom
  API base URL. Deferred — not worth adding a TOML parser for one edge case. Revisit if users
  report issues.

## Open questions

1. **Icon choice.** The bar text uses `C` / `X` as provider initials. Should there also be a
   leading Nerd Font icon on each line? If so, which glyph? Needs to be visually distinct from
   the thermometer (sensors), 󰠓 (crypto), and weather emoji. Also: the "both providers disabled"
   state currently shows 󰻀 (`nf-md-head_cog`) as a placeholder — needs testing in the actual bar.
   Best decided by visual testing rather than spec discussion.

**Resolved:**

2. ~~**Codex credential format on Linux.**~~ Verified Mar 8, 2026: `~/.codex/auth.json` on Linux
   matches CodexBar's macOS source exactly. The `tokens` nesting, field names, and structure are
   identical. No adjustment needed.

## Decision log

### D1: No token refresh by waybap

**Decision:** waybap never refreshes OAuth tokens. It reads credentials as-is and treats expired
tokens as a degraded state (serve stale cached data + show "token expired" in tooltip).

**Context:** CodexBar refreshes tokens by calling the provider's OAuth refresh endpoint and
writing updated credentials back to the CLI's credential file. This works for a standalone app
but creates a dangerous race condition when sharing credential files with the CLI:

1. OAuth2 refresh token rotation (standard practice) invalidates the old refresh token when a
   new one is issued.
2. If waybap refreshes the token while Claude Code is running, Claude Code's in-memory refresh
   token becomes invalid.
3. Claude Code's next token refresh fails with `invalid_grant`, forcing a manual `claude login`.

**Tradeoff:** waybap's usage data goes stale when the CLI's access token expires (typically 1+
hours). But since waybap is only useful when you're actively using Claude Code / Codex CLI (which
keep their own tokens fresh), the access token is almost always valid during active use. When you
stop using the CLI, stale usage data is fine — there's nothing new to track.

**Alternatives considered:**
- *Refresh to separate file* — avoids corrupting the CLI's file but still triggers refresh token
  rotation, invalidating the CLI's refresh token.
- *Refresh with compare-and-swap* — re-reads the credential file before writing back, discards
  if the refresh token changed. Reduces but doesn't eliminate the race window.
- *Never refresh* (chosen) — simplest, zero risk to CLI, and the degradation mode is acceptable.

### D2: Per-provider bar text on vertical bar

**Decision:** Show both providers stacked vertically: `C 78` on one line, `X 25` below. Each
colored independently by its own session threshold. When only one provider is active, show just
that one line. Omit providers that aren't configured.

**Context:** The Waybar bar is vertical (left side of screen), so horizontal space is limited
but vertical stacking is natural. Showing both providers gives immediate visibility into which
one is the bottleneck without hovering. Also works in horizontal Waybar orientations — Waybar
renders multi-line custom module text in both.

**Alternatives considered:**
- *Worst-of single %* — most compact but loses which provider is critical.
- *Worst-of with initial* — adds provider identity but still only shows one.
- *Worst-of marker (`▸`)* — considered in review round 2 but rejected; per-provider color coding
  already conveys urgency without extra visual noise.

### D3: Always show status page indicator

**Decision:** Always show the status line in the tooltip (even when operational).

**Context:** Status pages are fetched anyway (every query cycle). Not displaying them saves
no HTTP requests — only tooltip space. Showing "Operational" provides positive confirmation and
makes the tooltip layout consistent. When there IS an issue, the status line is immediately
visible in its expected position rather than suddenly appearing.

### D4: Use reqwest (already a dependency)

**Decision:** Use `reqwest::blocking::Client` for all HTTP requests, same as weather and crypto.

**Context:** The original spec incorrectly stated that existing modules use curl via
`std::process::Command`. They actually use `reqwest::blocking`. No new dependency needed.

### D5: Use chrono (already a dependency)

**Decision:** Use `chrono` for ISO 8601 parsing and countdown computation.

**Context:** Already in `Cargo.toml` and used by scheduler and weather parsing. Adding manual
timestamp math when chrono is available would be gratuitous complexity.

### D6: Credits as enum, not struct

**Decision:** Model credits as `enum Credits { ClaudeExtra { used_usd, limit_usd }, CodexBalance
{ balance_usd } }` rather than a flat struct with optional fields.

**Context:** Claude Extra (used/limit meter) and Codex credits (balance) are fundamentally
different display patterns. A unified struct with all-optional fields forces runtime provider
detection in the rendering code and risks unit confusion (Claude API returns cents, Codex returns
USD). The enum makes the rendering code exhaustive-match clean and normalizes units at parse time.

### D7: No 429 rate-limit backoff

**Decision:** Drop 429-specific backoff. If the API returns 429, treat it like any other transient
failure — retry on the next regular cycle (120s).

**Context:** At 120-second intervals, 429s are unlikely. CodexBar (30s-5m refresh) doesn't
implement 429-specific backoff either — its retry logic handles strategy fallback (OAuth → CLI →
web), not rate-limit backoff. Adding per-provider static Mutex backoff state inside query.rs is
complexity for a theoretical problem. If 429s become real, the simpler fix is increasing the
refresh interval.

### D8: No separate credentials.rs

**Decision:** Inline credential loading as private functions in `query.rs`. No `credentials.rs`
file.

**Context:** No existing module splits data loading into a separate file. Each credential loader
is ~15-20 lines of read-file-parse-JSON-extract-fields. Keeping it in query.rs makes the module
self-contained, matching weather, crypto, and sensors patterns. If Phase 3 CLI fallback adds
complexity, credential handling can be extracted then.

### D9: Show used% (not remaining%) in bar text

**Decision:** Bar text shows percentage *used* (`C 78` = 78% consumed), not percentage remaining.

**Context:** Matches what the APIs return directly (both Claude and Codex report utilization as
percentage used). The color coding (green → yellow → peach → red as remaining capacity decreases)
provides the urgency signal. Showing remaining% would eliminate one mental inversion but
introduces another: the number goes down over time (counter-intuitive for a "how much have I
used" mental model).

### D10: Cache raw API responses

**Decision:** Cache raw API responses wrapped with metadata (`timestamp`, `token_expired`,
`status`), not processed structs.

**Context:** Inspired by the weather module's approach of caching raw API responses with metadata
(weather wraps with `location_name`; usage wraps with per-provider `token_expired`, `status`,
`cli_installed` — structurally different but same philosophy). Benefits: (1) raw responses in cache aid debugging when APIs change,
(2) `parse_data()` handles all interpretation, keeping the single-responsibility boundary clean,
(3) struct changes don't invalidate the cache format.

### D11: No dedicated status page caching

**Decision:** Fetch status pages on every query cycle (120s). No static Mutex with TTL.

**Context:** At 120-second intervals, 2 extra lightweight GETs every 2 minutes is negligible
overhead. Status data gets included in the raw cache alongside usage responses, so stale-on-failure
behavior comes for free from the scheduler's cache mechanism. The nvidia-smi cache pattern (which
inspired the original spec) exists because sensors runs every 1 second and nvidia-smi is expensive
— neither condition applies here.

### D12: Drop window_minutes from RateWindow

**Decision:** `RateWindow` stores only `used_percent` and `resets_at`. Window labels ("Rate (5h)",
"Weekly") are derived from which field the RateWindow occupies in `ProviderUsage`.

**Context:** The Claude API doesn't return window duration at all — `window_minutes` would have
to be hardcoded from field names (`five_hour` → 300). Codex returns `limit_window_seconds` but
converting to minutes just to store a value that's never used in rendering is wasted complexity.
If Anthropic changes window durations, hardcoded values would silently be wrong.

### D13: UX extensions (freshness footer, smart not-configured, shared meter bar)

**Decision:** Add three low-cost extensions:

1. **Data freshness footer.** Muted "Updated Xs ago" at bottom of tooltip. Builds trust that data
   is current, especially since parse_data runs every HTTP request but data refreshes every 120s.
2. **Smart "not configured" message.** Check if CLI binary exists on PATH to distinguish "not
   installed" from "not logged in." Small addition that meaningfully reduces user confusion.
3. **Meter bar in pango.rs.** Extract the 10-character meter bar as a shared utility
   (`pub fn meter_bar(...)`) rather than inline in usage/parsing.rs. Reusable by future modules.

**Rejected:** Worst-of marker (`▸`) on the more constrained provider — per-provider color coding
already conveys urgency without extra visual noise.

### D14: Read-merge-write for partial failure

**Decision:** On partial failure, `query()` reads the existing cache and carries forward the
previous entry for the failed provider, preserving its stale `data` while updating `timestamp`.

**Context:** The scheduler's "don't overwrite cache on failure" logic only triggers when
`query()` returns `None` (total failure). When one provider succeeds and the other fails,
`query()` returns `Some(...)` — the scheduler writes the new cache, which would set the failed
provider to `null`, losing its last-known-good data for up to 120 seconds. This is a novel
pattern vs. existing single-source modules (weather, crypto, sensors) which never face partial
failure. The read-merge-write adds ~5 lines of code and prevents silent data loss.

### D15: Bar text shows weekly window

**Decision:** Bar text number is the weekly (7-day) window `used_percent`. Session and
model-specific windows are visible only in the tooltip.

**Context:** Originally showed the session (5h) window as the "most immediately actionable"
constraint, but in practice the weekly window is more useful at a glance — it represents the
hard budget that actually matters over time. Session limits reset every 5 hours and rarely
cause sustained concern, while a nearly-full weekly window means real constraint for the rest
of the week. The weekly number gives a better ambient signal of overall capacity.

### D16: Shared reqwest Client + existing module refactor

**Decision:** Build one `reqwest::blocking::Client` per `query()` call and reuse it across all
requests within that call. Existing modules (weather, crypto) were refactored to match this
shared-client pattern.

**Context:** Weather previously built two separate clients (3s timeout for geolocation, 10s for
the weather API) — refactored to a single shared client. Crypto already used one client per call.
The usage module makes 4 requests per cycle and benefits from a shared client for code simplicity.
Weather's different timeouts were consolidated to the longer timeout (10s) for both requests —
the geolocation endpoint is fast anyway; the 3s timeout was defensive, not load-bearing.

### D17: Token expired `?` suffix in bar text

**Decision:** When a provider's token is expired, append `?` to the stale number in bar text
(e.g., `C 42?`). The `?` signals "this data is uncertain" — a qualitatively different state
from normal usage display.

**Context:** Muted color alone (`#949cbb`) is too subtle for a state that means "data might be
hours old." The user needs to hover for the full "Token expired" message in the tooltip, but a
single character in the bar text saves that hover in the common case. `?` was chosen over `!`
(which implies "action needed" rather than "uncertainty") and over strikethrough (which Pango
supports but may render inconsistently in Waybar).

### D18: Extract `serve_cached_api` helper in server.rs

**Decision:** Extract the repeated read-cache/parse/serve logic into a generic helper function
before adding the 4th copy for the usage module. All existing handlers (`serve_api_weather`,
`serve_api_crypto`, `serve_api_sensors`) become one-liners delegating to this helper.

**Context:** The three existing handlers are identical except for the module name and parse
function. This is direct deduplication of identical logic — not a premature abstraction. The
4th copy is the natural trigger point for extraction. The helper takes a cache name and a parse
function, reads the cache file, deserializes JSON, calls the parse function, and serves the
result — or returns an error JSON response on any failure step.

### D19: Wire format is snake_case + credential nesting correction

**Decision:** All API field names and credential file structures updated to match verified wire
format. CodexBar's Swift source used camelCase (Swift convention) — the actual APIs return
snake_case. Claude credentials are nested under `claudeAiOauth`, not flat.

**Changes from CodexBar's model:**

1. **Claude usage API:** `five_hour` (not `fiveHour`), `seven_day` (not `sevenDay`),
   `resets_at` (not `resetsAt`), `extra_usage` (not `extraUsage`), `is_enabled` (not `isEnabled`),
   `monthly_limit` (not `monthlyLimit`), `used_credits` (not `usedCredits`).
   Model-specific: `seven_day_sonnet` (not `sevenDaySonnet`), prefix is `seven_day_` (not
   `sevenDay`). Additional fields: `seven_day_oauth_apps`, `seven_day_cowork`, `iguana_necktie`
   (all null in testing — unknown purpose, silently ignored).
2. **Codex usage API:** `plan_type` (not `planType`), `rate_limit` (not `rateLimit`),
   `primary_window` (not `primaryWindow`), `secondary_window` (not `secondaryWindow`),
   `used_percent` (not `usedPercent`), `reset_at` (not `resetAt`), `has_credits` (not `hasCredits`),
   `limit_window_seconds` (not `limitWindowSeconds`). `balance` is a **string**, not a number.
   Additional fields: `code_review_rate_limit`, `additional_rate_limits`, `reset_after_seconds`,
   `allowed`, `limit_reached`, `approx_local_messages`, `approx_cloud_messages` (all ignored).
3. **Claude credentials:** Nested under `claudeAiOauth` key. Also contains `rateLimitTier` and
   `subscriptionType` fields (not in CodexBar's model). `subscriptionType` could provide Claude
   plan info (e.g., "max") but is currently unused — the usage API doesn't return it.
4. **Cache `has_credentials` field:** Added to provider entries to distinguish "not logged in"
   (credential file missing) from "transient fetch failure" (creds exist, fetch failed). Provider
   entries are never null — they always carry `cli_installed` and `status` for the smart message.

**Context:** CodexBar was the best reference available during spec design, but its Swift source
deserializes into native structs with Swift naming conventions. The actual HTTP wire format uses
consistent snake_case across both providers. This was only discoverable by hitting the live APIs.
The spec's "verify wire format before trusting" warning (in the API stability section) was
explicitly written for this scenario.

### D20: Per-provider `data_timestamp` for staleness tracking

**Decision:** Each provider entry in the cache includes a `data_timestamp` (ISO 8601 UTC) recording
when that provider's data was last successfully fetched. On carry-forward (D14), the stale
provider's `data_timestamp` is preserved from the previous cache rather than updated. The tooltip
shows "Last data (Xm ago):" when displaying stale data under token expiry, using `data_timestamp`
for the age calculation.

**Context:** The envelope `timestamp` updates on every successful `query()` call, even when one
provider's data is carried forward from a previous cycle. Without per-provider timestamps, there's
no way to tell if Claude's data is 2 minutes old or 2 hours old after a series of Codex-only
successful fetches. The `data_timestamp` field makes staleness visible in the tooltip.

### D21: Treat HTTP 403 as token expired

**Decision:** HTTP 403 responses from usage APIs are treated identically to 401 — as token
expiry/revocation. The `FetchResult::TokenExpired` variant covers both status codes.

**Context:** Revoked accounts or disabled API access may return 403 instead of 401. Without this,
the user would see the data silently disappear with no hint to re-authenticate. The user action
is the same regardless (re-login), so conflating the two status codes is correct.

### D22: Rename "Session (5h):" to "Rate (5h):"

**Decision:** The 5-hour rate limit window label is "Rate (5h):" instead of "Session (5h):".

**Context:** "Session" implies a login session or continuous usage period, which is misleading —
this is a rolling rate limit window. "Rate" accurately describes what it is: a rate limit with a
5-hour duration. The "(5h)" suffix is retained because the window duration is useful context for
the user, even though it's hardcoded (see D12). Both providers use 5-hour primary windows:
Claude's `five_hour` field name implies it directly; Codex returns `limit_window_seconds: 18000`
(= 5 hours) in its `primary_window` response.

### D23: Extract `capitalize()` to `pango.rs`

**Decision:** The `capitalize()` utility (first letter uppercase) lives in `pango.rs` as a public
function, used by both `usage/parsing.rs` (model names, plan types) and `server.rs` (error messages).

**Context:** The function was duplicated between parsing.rs and server.rs. `pango.rs` is the
natural home — it already hosts shared formatting utilities (escape, meter_bar), and capitalize
is a display/formatting concern.

### D24: Review round 2 hardening

**Decision:** Batch of defensive fixes from creative/critic review:

1. **Model name word splitting:** `seven_day_oauth_apps` → "Oauth Apps" (split on `_`, capitalize
   each word, join with spaces) instead of just capitalizing the first letter.
2. **Pango escape model names:** Model names from API keys are passed through `pango::escape()`
   before embedding in tooltip markup. Spec requires escaping all API-sourced strings.
3. **Clock skew clamping:** `format_data_age` and `format_freshness` clamp durations to zero and
   display "just now" for zero/negative values, preventing "-Xs ago" from NTP corrections.
4. **Token expired guard:** "Last data" header now also checks `model_weekly` — prevents stale
   model-specific meters from appearing without a staleness warning.
5. **Bar text clamping:** `used_percent` clamped to 0-100 in bar text, matching `meter_bar`'s
   existing clamping behavior.
6. **`cli_on_path` uses `is_file()`:** Prevents false positives from directories named `claude`
   or `codex` in PATH.
7. **`fetch_status` logging:** Added `eprintln!` for all failure modes, matching the pattern in
   `fetch_usage_claude`/`fetch_usage_codex`.
8. **Pango attribute standardization:** Usage module uses `foreground=` attribute to match
   existing crypto/sensors convention (functionally identical to `color=`).
9. **Codex install URL:** Changed from `codex.ai` to `github.com/openai/codex` (verified).
10. **Carry-forward `token_expired` not preserved:** Documented as accepted trade-off — server-side
    revocation followed by network failure shows stale data without warning for up to 120s.

### D25: Review round 3 hardening

**Decision:** Batch of fixes from creative/critic review round 3:

1. **`serve_cached_api` error JSON:** Handler now serves error JSON on cache-read and JSON-parse
   failures instead of dropping the HTTP connection. Waybar gets a broken-chain icon with a
   diagnostic tooltip on cold start (before first cache write) instead of a hung request.
2. **Extract shared `fetch_usage` helper:** `fetch_usage_claude` and `fetch_usage_codex` now
   delegate to a shared `fetch_usage(client, req, label)` function for the send/check/read/parse
   chain. Provider-specific code (URL, headers) remains in the thin wrapper functions.
3. **Sub-minute countdown:** `format_countdown` now shows `"resets in Xs"` for 1-59 seconds
   remaining instead of `"resets in 0m"`.
4. **Tooltip meter % clamping:** `format_meter_line` now clamps `used_percent` to 0-100, matching
   `format_bar_line` and `meter_bar` clamping behavior.
5. **Bar line fallback for missing session:** `format_bar_line` now returns a muted placeholder
   (`"C —"`) when session data is missing instead of hiding the provider entirely. Keeps bar and
   tooltip consistent.
6. **`is_token_expired` safe cast:** `timestamp_millis()` clamped to `.max(0)` before `as u64`
   cast, preventing silent wrap on negative timestamps.
7. **Token expired guard includes credits:** "Last data" header now also checks `credits` —
   covers theoretical plan tiers with credits but no rate windows.
8. **`foreground=` standardization completed:** Weather `color_temp_fmt` (4 occurrences in
   `weather/utils.rs`) and crypto icon span (1 occurrence in `crypto/parsing.rs`) updated from
   `color=` to `foreground=`, completing codebase-wide standardization.
