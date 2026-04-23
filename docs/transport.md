# Transport Policy (`plnk-core`)

`plnk-core` now has a shared transport policy model for HTTP behavior across the SDK and CLI stack.

## Current status

This document describes the transport work completed so far in **TRN-001** through **TRN-004**:

- `TransportPolicy` is the single canonical configuration object for transport tuning
- `HttpClient` carries shared transport runtime state and routes every request through one common hook
- authenticated and unauthenticated clients can both be created with an explicit policy
- shared in-process concurrency caps are enforced per client instance
- shared in-process rate limiting is enforced per client instance
- safe read requests retry automatically on transient HTTP/transport failures
- `Retry-After` is honored when present on retryable responses
- transport settings can now be supplied via CLI flags, environment variables, and config file

At this stage, the whole transport stack is active end-to-end: shared policy defaults, SDK overrides, CLI/env/config tuning, concurrency limits, rate limiting, and safe-method retries.

## Defaults

`TransportPolicy::default()` currently resolves to:

| Field | Default | Meaning |
|------|---------|---------|
| `max_in_flight` | `8` | Enforced max in-flight requests per client instance |
| `rate_limit_per_second` | `Some(10)` | Enforced sustained request rate |
| `burst_size` | `Some(10)` | Enforced short-burst allowance |
| `retry_attempts` | `2` | Enforced retry attempts after the initial request |
| `retry_base_delay_ms` | `250` | Enforced base retry delay |
| `retry_max_delay_ms` | `2000` | Enforced max retry delay |
| `retry_jitter` | `true` | Enforced retry delay jitter |
| `retry_safe_methods_only` | `true` | Enforced safe-method retry restriction |

These values are intentionally conservative for self-hosted Planka instances.

## Validation rules

Invalid policy values fail fast with `PlankaError::InvalidOptionValue`.

Current validation rules:

- `max_in_flight >= 1`
- `rate_limit_per_second`, when set, must be `>= 1`
- `burst_size`, when set, must be `>= 1`
- `retry_max_delay_ms >= retry_base_delay_ms`

## SDK usage

### Authenticated client with defaults

```rust
use plnk_core::client::HttpClient;
use url::Url;

let server = Url::parse("https://planka.example.com")?;
let http = HttpClient::new(server, "api-token")?;
```

### Authenticated client with explicit transport policy

```rust
use plnk_core::client::HttpClient;
use plnk_core::transport::TransportPolicy;
use url::Url;

let server = Url::parse("https://planka.example.com")?;
let policy = TransportPolicy {
    max_in_flight: 4,
    retry_attempts: 1,
    ..TransportPolicy::default()
};

let http = HttpClient::with_policy(server, "api-token", policy)?;
```

### Unauthenticated client for bootstrap/login-style flows

```rust
use plnk_core::client::HttpClient;
use plnk_core::transport::TransportPolicy;
use url::Url;

let server = Url::parse("https://planka.example.com")?;
let http = HttpClient::unauthenticated_with_policy(
    server,
    TransportPolicy::default(),
)?;
```

### Inspect the effective policy

```rust
let policy = http.transport_policy();
println!("max_in_flight={}", policy.max_in_flight);
```

## Active retry behavior

Current retry policy:

- automatic retries apply to safe methods only by default: `GET`, `HEAD`, `OPTIONS`
- retryable status codes: `429`, `502`, `503`, `504`
- retryable transport failures: timeout/connect errors surfaced by `reqwest`
- writes are **not** retried automatically by default: `POST`, `PATCH`, `DELETE`
- `404`, `401`, malformed requests, and other non-transient failures are not retried
- `Retry-After` is honored when parseable and clamped to a sane upper bound

## CLI, environment, and config knobs

Transport settings resolve in this order:

1. CLI flags
2. environment variables
3. config file (`~/.config/plnk/config.toml`, honoring `XDG_CONFIG_HOME`)
4. built-in defaults

### CLI flags

```bash
--http-max-in-flight <n>
--http-rate-limit <rps>
--http-burst <n>
--retry-attempts <n>
--retry-base-delay-ms <ms>
--retry-max-delay-ms <ms>
--no-retry
```

### Environment variables

```text
PLNK_HTTP_MAX_IN_FLIGHT
PLNK_HTTP_RATE_LIMIT
PLNK_HTTP_BURST
PLNK_RETRY_ATTEMPTS
PLNK_RETRY_BASE_DELAY_MS
PLNK_RETRY_MAX_DELAY_MS
```

### Config file

```toml
server = "https://planka.example.com"
token = "your-api-token"

[http]
max_in_flight = 8
rate_limit = 10
burst = 10
retry_attempts = 2
retry_base_delay_ms = 250
retry_max_delay_ms = 2000
```

### Validation ranges

Current CLI/env/config validation:

- `max_in_flight`: `1..=64`
- `rate_limit`: `1..=1000`
- `burst`: `1..=1000` and requires `rate_limit` to also be set
- `retry_attempts`: `0..=10`
- `retry_base_delay_ms`: `1..=60000` (library invariant too, not just CLI validation)
- `retry_max_delay_ms`: `1..=60000`
- `--no-retry` forces `retry_attempts = 0`

## Test coverage

Transport behavior is covered at multiple layers:

- `plnk-core` unit tests for:
  - retry classification
  - backoff bounds
  - `Retry-After` parsing/clamping
  - policy validation
- `plnk-core` integration tests for:
  - `503 -> 200` retry success
  - `429 + Retry-After` retry success
  - write non-retry behavior
  - concurrency/rate-limit timing behavior
- `plnk-cli` tests for:
  - precedence (`flags > env > config > defaults`)
  - `--no-retry` override behavior
  - invalid transport settings exiting with code `2`
  - config preservation when auth commands rewrite credentials
  - machine-readable help exposing the transport knobs

## Socializing this internally

If you need to explain the current state to contributors or agentic callers, the short version is:

1. `TransportPolicy` is now the single source of truth for HTTP tuning defaults
2. every request now flows through one shared transport hook in `HttpClient`
3. SDK consumers can already set policy explicitly
4. CLI, env, and config can now override transport settings
5. concurrency, rate limiting, and safe-method retries are active now by default

## Recommended message to the team

> We now have a common transport policy object in `plnk-core` with documented defaults and validation. All HTTP requests flow through one shared transport runtime hook. Shared concurrency limits, rate limiting, and safe-method retries are active for every SDK/CLI request path, and transport settings can be tuned via CLI flags, environment variables, config file, or direct SDK policy construction.
