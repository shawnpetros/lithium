# Adapter Contract

This doc describes the shape an adapter must conform to so it can be wired into lithium alongside the existing Anthropic / OpenAI / OpenRouter ones. It exists so adding a new provider doesn't require re-reading the whole codebase.

## TL;DR

An adapter is a Rust crate at `crates/lithium-<provider>/` that:

1. Reads a per-provider section of `[providers.<provider>]` in `~/.config/lithium/config.toml`
2. Exposes one or more methods that hit the upstream provider's billing/usage API
3. Emits a `Vec<UsageRow>` per call, where each row carries period start/end, model, USD cost (when available), and a JSON `raw_payload` for forensics
4. Surfaces clear, user-actionable error messages on auth / rate-limit / network failures
5. Never logs API keys, ever

## What "an adapter does" looks like in code

```rust
pub struct MyProviderClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl MyProviderClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> { /* ... */ }
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        // ^ same constructor, used by wiremock tests with a localhost base
    }

    pub async fn fetch_usage(
        &self,
        starting_at: DateTime<Utc>,
        ending_at: DateTime<Utc>,
    ) -> Result<Vec<UsageRow>> {
        // 1. Build the request with proper headers/query params
        // 2. Send it
        // 3. On non-2xx: surface a typed, user-actionable error
        // 4. On 2xx: parse, paginate if needed, return UsageRows
    }
}
```

Reference implementations:
- `crates/lithium-anthropic/src/admin_api.rs` — full pagination + admin-key auth + day-bucket query params
- `crates/lithium-openai/src/lib.rs` — Unix-second timestamps, line-item grouping, the `data[].results[]` shape
- `crates/lithium-openrouter/src/lib.rs` — pre-aggregated `usage_daily`, no pagination, accepts envelope-wrapped or flat response

## UsageRow contract

The `lithium_core::types::UsageRow` shape is non-negotiable. Adapters fill in:

- `provider` — your `Provider` enum variant (add a new one in `crates/lithium-core/src/types.rs` if needed)
- `source` — typically `Source::AdminApi` (already supported); add a new variant if your data shape genuinely differs
- `period_start`, `period_end` — UTC, snapped to bucket boundaries (typically full days)
- `model` — your provider's term for "what kind of spend this row represents." For OpenAI we use `openai.completions`, `openai.embeddings`. For Anthropic we use the model name verbatim. Choose what makes the row's meaning clear at a glance.
- `cost_usd` — USD spend, or `None` if your source doesn't expose dollar attribution (e.g., flat-rate plans)
- `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_create_tokens` — fill what the API returns, leave the rest `None`
- `raw_payload` — the original API response (or relevant slice of it) as `serde_json::Value`. This is what `lithium doctor` and future debug tools read.

## Idempotency

Adapters MUST produce rows that are idempotent on the unique key `(provider, source, period_start, period_end, model_key)`. Re-polling for the same period overwrites instead of duplicating. The storage layer enforces this via UPSERT, but your adapter's row-generation has to be deterministic for a given (provider, source, period) tuple.

## Error handling

Three error types matter for users:

1. **Auth (401)** — surface as a clear error message including a hint to regenerate the key. Example: `"401 Unauthorized from <Provider> API. Hint: regenerate the key at <console URL> and update ~/.config/lithium/config.toml"`.
2. **Rate limit (429)** — respect `Retry-After` if present; surface as a transient error.
3. **Anything else** — log status code + truncated body (first 500 chars), wrapped in `anyhow::Error`.

Use the `truncate(s, max)` pattern from existing adapters to keep error messages from filling the screen with HTML error pages.

## Secrets

API keys live in `~/.config/lithium/config.toml`. The adapter receives the key by value via its constructor and uses it. **Never log or print the key.** If your adapter needs to surface "this key is configured" status, use the `redact()` pattern in `crates/lithium-cli/src/cmd/doctor.rs` (first 16 chars + `***REDACTED***`).

## Tests

Every adapter ships with `wiremock`-backed tests covering at minimum:

- The happy path (200 OK with sample response, parses correctly into `UsageRow`)
- The 401 path (unauthorized error has a user-actionable message)
- Pagination, if the API supports it

Tests live in `#[cfg(test)] mod tests { ... }` inside the adapter crate. See `lithium-anthropic/src/admin_api.rs::tests` for the full pattern.

## Wiring into the CLI

Once your adapter compiles and passes tests, wire it into `crates/lithium-cli/src/cmd/`:

1. Add a `poll_<provider>` async function in `cmd/poll.rs` that mirrors the existing ones (load config, init client, call fetch, upsert rows, record poll log)
2. Add the new provider/source pair to the `entries` vec in `cmd/adapters.rs`
3. Add a connectivity check + key redaction display in `cmd/doctor.rs`
4. Add a config struct in `crates/lithium-core/src/config.rs` (`<Provider>Config` with `api_key` or `admin_api_key`)

The display logic in `cmd/today.rs` and `cmd/month.rs` already groups by provider, so no changes there.

## Naming

- Crate name: `lithium-<provider>` (lowercase, hyphenated)
- Provider enum variant: PascalCase matching the public brand (`Anthropic`, `OpenAI`, `OpenRouter`)
- `model` field labels: use the provider's own term where a model name exists; otherwise prefix with `<provider>.<line_item>` (e.g., `openai.completions`)

## Out-of-scope (for now)

- Push-based usage tracking (intercepting outbound requests). lithium is poll-only by design — it works regardless of which client library or harness made the calls.
- Non-USD currency. The current cost aggregation assumes all rows are USD. If your provider returns other currencies, you can store the raw amount in `raw_payload` but `cost_usd` should be `None` until we add FX conversion.
- Per-key usage breakdowns. Phase 2 just needs per-provider per-period totals. Per-key granularity is a future enhancement.

## Want to add an adapter?

Open an issue at https://github.com/shawnpetros/lithium/issues describing the provider's API. If the API exposes per-day USD spend with bearer-token auth, the implementation follows one of the three reference adapters closely. If the shape is genuinely different (push-based, no historical data, etc.), the issue is the right place to discuss.
