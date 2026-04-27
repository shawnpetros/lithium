# lithium - Phase 1 Spec

**Status:** Spec-complete, awaiting implementation
**Scope:** Anthropic-only CLI MVP
**Target:** v0.1.0
**Owner:** [[Shawn]]
**Date:** 2026-04-27

## Goals

1. **Answer the daily question.** `lithium today` shows what's been spent on Anthropic today across the API directly + Claude Code session/weekly usage, with totals.
2. **Answer the monthly question.** `lithium month` shows month-to-date totals + projected end-of-month based on running average.
3. **Be installable in under 5 minutes** from a fresh Mac with `cargo install`.
4. **Be public from day 1.** First commit goes to a public GitHub repo with a finished README, hero image, and at least one working subcommand.
5. **Ship at santifer-discipline quality.** Hero banner, demo gif, single-line positioning, MIT license, clean docs. No half-finished commands in `main`.

## Non-Goals (Phase 1)

- Multi-provider support (OpenAI, OpenRouter, Gemini). Phase 2.
- Daemon split (always-on background process). Phase 3. Phase 1 polls on demand only.
- Status-line / menubar / web surfaces. Phase 3+.
- OpenClaw MCP hooks. Phase 4.
- Real-time updates. Polled on `lithium poll` invocation.
- Push-based usage tracking (proxy interception). Out of scope entirely.

## Success Criteria

- A new Mac user can `cargo install --git https://github.com/shawnpetros/lithium`, run `lithium config`, paste an Anthropic admin key, run `lithium poll`, and see correct numbers from `lithium today` in under 5 minutes total.
- All 11 P1 features in `features.json` reach `passes: true` on their scenarios.
- `clippy --all-targets` is clean.
- README quick-start works verbatim.

## Workspace Layout

```
lithium/
├── Cargo.toml                          # workspace root
├── README.md
├── LICENSE                             # MIT
├── CLAUDE.md
├── SESSION-CONTEXT.md
├── features.json
├── assets/
│   ├── hero.png
│   └── demo.gif
├── docs/
│   ├── SPEC-PHASE-1.md                 # this file
│   └── HERO-IMAGE-PROMPT.md
├── crates/
│   ├── lithium-core/                   # types, storage, projection logic
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types.rs                # UsageRow, Source enum, etc.
│   │   │   ├── storage.rs              # SQLite wrapper
│   │   │   ├── projection.rs           # month projection logic
│   │   │   └── config.rs               # config.toml loader
│   │   └── migrations/
│   │       └── 0001_initial.sql
│   ├── lithium-anthropic/              # Anthropic adapter
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── admin_api.rs            # /v1/organizations/usage_report/...
│   │       └── claude_code.rs          # ~/.claude/ state file reader
│   └── lithium-cli/                    # binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           └── cmd/
│               ├── today.rs
│               ├── month.rs
│               ├── poll.rs
│               ├── init.rs
│               ├── config.rs
│               ├── adapters.rs
│               └── doctor.rs
└── tests/
    └── integration/
        ├── today.rs
        ├── month.rs
        └── poll.rs
```

Three crates in a workspace. `lithium-core` owns types + storage + projection logic. `lithium-anthropic` owns the adapter. `lithium-cli` owns the binary. Future adapters become new sibling crates without touching core.

## Data Model

### `usage` table

```sql
CREATE TABLE usage (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    polled_at       TEXT NOT NULL,           -- ISO 8601 UTC, when this row was recorded
    period_start    TEXT NOT NULL,           -- ISO 8601 UTC, start of the usage window this row covers
    period_end      TEXT NOT NULL,           -- ISO 8601 UTC, end of the usage window
    provider        TEXT NOT NULL,           -- 'anthropic'
    source          TEXT NOT NULL,           -- 'admin_api' | 'claude_code_local'
    model           TEXT,                    -- e.g. 'claude-sonnet-4-6', NULL for non-API rows
    input_tokens    INTEGER,                 -- NULL for non-API rows
    output_tokens   INTEGER,                 -- NULL for non-API rows
    cache_read_tokens   INTEGER,
    cache_create_tokens INTEGER,
    cost_usd        REAL,                    -- NULL when source has no $ cost (Claude Code session pct)
    session_pct     REAL,                    -- 0.0-1.0, NULL for API rows
    weekly_pct      REAL,                    -- 0.0-1.0, NULL for API rows
    session_resets_at TEXT,                  -- ISO 8601 UTC
    weekly_resets_at  TEXT,                  -- ISO 8601 UTC
    raw_payload     TEXT NOT NULL            -- JSON dump of the source response, for debugging
);

CREATE INDEX idx_usage_period_start ON usage(period_start);
CREATE INDEX idx_usage_provider_source ON usage(provider, source);
```

### `poll_log` table

```sql
CREATE TABLE poll_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    provider        TEXT NOT NULL,
    source          TEXT NOT NULL,
    status          TEXT NOT NULL,           -- 'ok' | 'error'
    error_message   TEXT,
    rows_inserted   INTEGER NOT NULL DEFAULT 0
);
```

`poll_log` is what `lithium adapters` reads to show last-poll status.

### Idempotency

The poller MUST NOT double-count. Strategy: dedup by `(provider, source, period_start, period_end, model)` tuple. On re-poll, UPSERT semantics: if the same tuple exists, replace the row.

## Configuration

Path: `~/.config/lithium/config.toml`

```toml
[storage]
db_path = "~/.local/share/lithium/usage.db"   # default; override if needed

[poll]
default_cadence_minutes = 15                  # not enforced in P1 (no daemon)

[providers.anthropic]
admin_api_key = "sk-ant-admin01-..."          # required for API direct usage
claude_code_state_dir = "~/.claude"           # default; override if Claude Code lives elsewhere

# Future:
# [providers.openai]
# admin_api_key = "..."
# [providers.openrouter]
# api_key = "..."
```

`lithium config` opens the file in `$EDITOR`. If the file does not exist, it writes a template with placeholder values and a comment block explaining each field, then opens.

The admin key MUST never be logged. Tracing spans involving the key MUST redact via `***REDACTED***`.

## Anthropic Adapter

Two sources, both behind the same provider:

### Source 1: Admin API direct

- Endpoint: Anthropic Usage & Cost Admin API. Verify exact endpoint shape at build time. As of late 2025 / early 2026 the endpoint is in the `https://api.anthropic.com/v1/organizations/usage_report/...` family. Implementer should fetch current docs at https://docs.anthropic.com first.
- Auth: `x-api-key: <admin_api_key>`. Admin keys are distinct from regular API keys.
- Request: scope to today (or month for `lithium month`), all models, all keys.
- Parse response into `usage` rows with `source = 'admin_api'`, `model`, token counts, `cost_usd`.
- One poll = one HTTP call typically. Pagination handling required if > N rows; default page size to whatever Anthropic returns.
- On non-200 response: log status + body, write `error` row to `poll_log`, return error to CLI.
- On 401: surface "admin key invalid or expired" with a hint to regenerate at console.anthropic.com.
- On rate limit (429): respect `retry-after` header.

### Source 2: Claude Code local state

- Inspiration: NoobyGains/claude-pulse parses Claude Code's local state files. Reverse-engineer that approach. Their Python source is the reference; do NOT vendor it, just understand the file format.
- Files of interest under `~/.claude/`: subscription tier, current session state (token usage, last reset), weekly state. Exact filenames and JSON schema must be confirmed at implementation time by reading what exists there.
- Output: rows with `source = 'claude_code_local'`, `session_pct`, `weekly_pct`, `session_resets_at`, `weekly_resets_at`, `cost_usd = NULL` (Max plans are flat-rate, not per-call billed).
- If Claude Code is not installed (no `~/.claude/` directory), this source returns no rows and logs an info-level message. NOT an error.

## CLI Surfaces

### `lithium init`

Initializes the SQLite database at the configured path. Idempotent (safe to run twice). Runs migrations.

```
lithium init
✓ Database initialized at /Users/shawnpetros/.local/share/lithium/usage.db
✓ Schema version: 1
```

### `lithium config`

Opens `~/.config/lithium/config.toml` in `$EDITOR`. If missing, creates from template first.

### `lithium poll [--provider anthropic]`

Runs all configured adapters (Phase 1 = Anthropic only). Optional `--provider` flag to scope. Prints one line per adapter:

```
lithium poll
✓ anthropic / admin_api          12 rows inserted (3 models)
✓ anthropic / claude_code_local   2 rows inserted
```

On error:

```
✗ anthropic / admin_api    error: 401 Unauthorized
  Hint: regenerate admin key at console.anthropic.com -> Settings -> Admin Keys
```

### `lithium today`

Prints today's spend by provider/source.

```
lithium - 2026-04-27

Anthropic
  API direct           $4.21    (claude-sonnet-4-6: $3.80, claude-haiku-4-5: $0.41)
  Claude Code session  47% used  (resets in 1h 12m)
  Claude Code weekly   23% used  (resets in 4d 2h)

Total today: $4.21

Last polled: 14m ago
```

If never polled: prints empty state with hint to run `lithium poll`.

### `lithium month`

Prints month-to-date + projection.

```
lithium - April 2026 (day 27 of 30)

Anthropic
  API direct          $128.40
  Claude Code Max     $200.00 (fixed monthly)

Total: $328.40
Daily avg (variable): $4.76
Projected end of month: $342.68
```

Projection logic: take total variable spend (i.e., excluding fixed/flat-rate sources), divide by days elapsed, multiply by total days in month, add fixed costs back. Implementation in `lithium-core/src/projection.rs`.

### `lithium adapters`

Lists configured providers and last-poll status.

```
Adapters

✓ anthropic / admin_api           configured, last poll 14m ago, 12 rows
✓ anthropic / claude_code_local   configured, last poll 14m ago, 2 rows
- openai                          not configured (phase 2)
- openrouter                      not configured (phase 2)
```

### `lithium doctor`

Verifies setup. Equivalent to running each check and printing pass/fail.

```
lithium doctor

✓ Config file exists       /Users/shawnpetros/.config/lithium/config.toml
✓ Database accessible      /Users/shawnpetros/.local/share/lithium/usage.db
✓ Schema version current   1
✓ Anthropic admin key set  sk-ant-admin01-***REDACTED***
✓ Claude Code state found  /Users/shawnpetros/.claude
✓ Connectivity to api.anthropic.com  200 OK

All checks passed.
```

## Logging

All structured logging via the `tracing` crate per the global CLAUDE.md debug-logging rule:

- INFO on subcommand entry with key params (redacted)
- DEBUG on adapter HTTP calls with method, URL, status code (no body)
- ERROR on any failure with full context

`println!` is reserved for user-facing CLI output only. Anything diagnostic goes through `tracing`.

`RUST_LOG=lithium=debug lithium poll` enables debug output for troubleshooting.

## Error Handling

- Use `anyhow::Result` at the CLI boundary. Use `thiserror`-derived enums inside crates.
- Surface user-actionable hints alongside errors (e.g., the 401 → regenerate-key hint).
- Never panic in normal operation. Every `unwrap` must have a comment explaining why it's safe.
- All external calls (HTTP, file I/O, SQLite) are fallible and handled.

## Tests

- Unit tests in each crate alongside source.
- Integration tests at `tests/integration/` exercise the full CLI via `assert_cmd`.
- Anthropic adapter tests use `wiremock` for the HTTP layer.
- Claude Code reader tests use a fixture directory under `tests/fixtures/claude/`.

Goal: 80%+ line coverage on `lithium-core/src/projection.rs` (the most complex pure logic). Adapter coverage gated by mocked HTTP.

## Demo Gif

Recorded with [`vhs`](https://github.com/charmbracelet/vhs) for reproducibility. Tape file at `assets/demo.tape`. Generated `assets/demo.gif` committed to repo. Shows: `lithium config` → paste key → `lithium poll` → `lithium today` → `lithium month`. ~30 seconds.

## Build & Publish

- `cargo install --git https://github.com/shawnpetros/lithium` is the install path. Phase 1 does NOT publish to crates.io.
- Phase 2 may add a Homebrew tap and crates.io publish. Phase 1 keeps it simple.
- GitHub release for v0.1.0: cut tag, write release notes, attach pre-built binaries for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu` via `cargo dist` (or manual cross-compile).

## Pre-Push Checklist

Before the first push to `github.com/shawnpetros/lithium`:

- [ ] Hero image generated and at `assets/hero.png`
- [ ] README renders cleanly on GitHub at default width
- [ ] No secrets in any committed file (`git secrets --scan`)
- [ ] `.gitignore` excludes `~/.config/lithium/`, `Cargo.lock` is committed (binary), `target/` is excluded
- [ ] LICENSE present (MIT)
- [ ] CLAUDE.md, SESSION-CONTEXT.md, features.json present
- [ ] At least one subcommand actually works end-to-end (typically `lithium init` + `lithium today` with empty data)

## Open Questions (resolve at implementation time)

1. Exact Anthropic Admin API usage endpoint shape and pagination contract. Fetch from docs.anthropic.com at build time, do not assume from this spec.
2. Claude Code state file format. Reverse-engineer from a real `~/.claude/` directory plus NoobyGains/claude-pulse Python source as reference.
3. Whether to ship pre-built binaries via `cargo dist` for v0.1.0, or require `cargo install` only. Lean toward `cargo install` only for Phase 1, add binaries in Phase 2.
4. Currency. Phase 1 is USD only. If Anthropic Admin API returns multiple currencies for some accounts, normalize to USD via static rate (don't pull live FX in P1).

## Definition of Done (Phase 1)

`lithium today` and `lithium month` print correct, current numbers from a fresh install on a Mac with both an Anthropic admin key configured AND Claude Code installed. The README renders, the hero image displays, the demo gif plays, the install command works verbatim, and the repo is public on GitHub.

That's the bar. Anything beyond it is Phase 2.
