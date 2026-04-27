# SESSION-CONTEXT.md

## Status

Phase 1 implementation **shipped to main** as of 2026-04-27. Three-crate Rust workspace builds clean, 22 tests pass, clippy is silent. All seven CLI subcommands work end-to-end against real data:

- `lithium init` / `lithium config` / `lithium adapters` / `lithium doctor` work against empty config
- `lithium poll` against Claude Code local state successfully ingested 123 rows from `~/.claude/stats-cache.json`
- `lithium today` and `lithium month` render correctly (with `$0.00` totals because Claude Code Max is flat-rate; will populate variable spend once admin API key is configured)
- `lithium poll` against Anthropic Admin API tested with `wiremock` (3 tests covering happy path, 401 unauthorized, pagination); awaiting live verification with a real admin key

## In-Flight

**Awaiting your live verification** per `docs/PHASE-1-HANDOFF.md`. Specifically: generate an Anthropic admin key at console.anthropic.com, drop it into `~/.config/lithium/config.toml`, run `lithium poll` and `lithium today`, confirm the numbers match the console.

## Key Details

- Repo: https://github.com/shawnpetros/lithium
- Two commits this session:
  - `<phase 1>`: workspace + core + adapter + CLI (~2K LOC of Rust)
  - `<this commit>`: handoff doc + SESSION-CONTEXT sync
- DB lives at `~/Library/Application Support/lithium/usage.db` on macOS (per `directories` crate XDG semantics)
- Config at `~/.config/lithium/config.toml`. Template auto-written by `lithium config` on first run.
- Anthropic Cost Report endpoint locked: `GET /v1/organizations/cost_report` with `X-Api-Key + anthropic-version: 2023-06-01`. Amounts are cents-as-decimal-string (divide by 100). Group by `description` for per-model breakdown.
- Claude Code state format: `~/.claude/stats-cache.json` v3 schema. dailyModelTokens for per-day per-model totals, modelUsage for aggregates. NoobyGains-style local read. No API key needed.

## Next Steps

1. **You:** generate admin key, paste into config, run `lithium poll` + `lithium today`, confirm numbers match. (3 minutes; full instructions in `docs/PHASE-1-HANDOFF.md`)
2. **Together (next session):** record demo gif with `vhs`, cut v0.1.0 tag + GitHub release, post about it.
3. **Phase 2 (next weekend):** OpenAI adapter + OpenRouter adapter. Each is a sibling crate.
4. **Phase 3 (after that):** daemon split, launchd plist, cship segment, SwiftBar plugin.
5. **Phase 4 (after that):** OpenClaw MCP hooks for cost gates.
