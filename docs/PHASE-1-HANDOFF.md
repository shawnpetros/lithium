# Phase 1 Handoff (2026-04-27)

Phase 1 build session ended with the entire Anthropic-only CLI MVP working end-to-end against real local data. Below is the exact path to verify it against your live Anthropic Admin API and confirm v0.1.0 readiness.

## What's working right now (no input needed from you)

- `lithium --help` and `lithium --version` (cargo built clean)
- `lithium init` creates the SQLite db at `~/Library/Application Support/lithium/usage.db` (macOS) or `~/.local/share/lithium/usage.db` (Linux)
- `lithium config` opens the TOML config in `$EDITOR`, creates the file from template if missing
- `lithium poll` runs both Anthropic sources:
  - admin_api: skipped if no admin key configured (clear hint)
  - claude_code_local: **already inserted 123 real rows** from your `~/.claude/stats-cache.json`
- `lithium today` renders today's spend (currently `$0.00` because Claude Code Max is flat-rate so no $ attribution; will populate once admin API key is in)
- `lithium month` renders month-to-date + projection block + fixed-cost section
- `lithium adapters` shows status with last-poll info per adapter
- `lithium doctor` runs the full health check (config, DB, admin key, Claude Code state, connectivity)
- 22 tests passing across `lithium-core` (16) and `lithium-anthropic` (6)
- `cargo clippy --all-targets --workspace` is clean (zero warnings)

## What you need to do (~3 minutes)

### Step 1: Generate an Anthropic admin API key

1. Go to https://console.anthropic.com
2. Settings -> Admin Keys -> Create Admin Key
3. Name it `lithium-local`
4. Copy the key (starts with `sk-ant-admin01-...`). **Don't paste it to Claude.** It goes in your local config file.

### Step 2: Drop the key into config

Run this from anywhere:

```bash
lithium config
```

This opens `~/.config/lithium/config.toml` in `$EDITOR`. The template is already there. Find this section:

```toml
[providers.anthropic]
# admin_api_key = "sk-ant-admin01-PASTE-HERE"
```

Uncomment the line and paste your key. Save and exit. Optionally also add fixed costs:

```toml
[fixed_costs]
anthropic_max = 200.0
```

### Step 3: Poll real data

```bash
lithium poll
```

You should see something like:

```
✓ anthropic / admin_api          N rows inserted (M unique models)
✓ anthropic / claude_code_local  123 rows inserted (per-day per-model token volume)
```

If you see `✗ anthropic / admin_api    error: 401 Unauthorized`, double-check the key is the **admin** key, not a regular API key. Admin keys start with `sk-ant-admin01-`.

### Step 4: Verify the numbers

```bash
lithium today
```

Should now show real $ spend by model for today. Cross-check against console.anthropic.com -> Usage -> Today. They should match (within the time it took the cost report to refresh).

```bash
lithium month
```

Should show month-to-date $ spend, daily average, projected end-of-month total. Cross-check against console.anthropic.com -> Billing.

If numbers match -> Phase 1 is verified.

## What's NOT in Phase 1 (intentional)

- **No `month` projection accuracy guarantee yet.** The projection just uses the running daily average; if your usage is bursty, the projection will be wrong. Phase 1 doesn't try to be smart about this.
- **No real-time / daemon mode.** You run `lithium poll` manually. Phase 3 splits out a `lithiumd` daemon with launchd.
- **No demo gif yet.** Will record with `vhs` once you've confirmed live data flows. The `vhs` install command: `brew install vhs`.
- **No v0.1.0 release tag yet.** That happens after you confirm live verification.
- **Claude Code session/weekly limit pcts.** `stats-cache.json` doesn't expose those. The Phase 1 spec mentioned them as a goal; turns out they live elsewhere (probably need to read live `~/.claude/sessions/` files or query the API). Punted to Phase 1.5 or absorbed into Phase 2.
- **No OpenAI / OpenRouter adapters.** Phase 2.

## If something breaks

- **`lithium poll` hangs**: probably a network timeout. The reqwest client has a 30s timeout. If the Anthropic API is slow, expect up to 30s per page.
- **401 Unauthorized**: regenerate the admin key (not the regular API key) at console.anthropic.com -> Settings -> Admin Keys.
- **Numbers don't match console**: the Cost Report has some lag (not real-time). Try again in ~15 minutes.
- **`lithium doctor` fails on connectivity**: check internet, check the key is in the right TOML section.
- **DB errors**: `rm ~/Library/Application\ Support/lithium/usage.db` and re-run `lithium init`. You'll lose the 123 Claude Code rows but they'll come back on the next `lithium poll`.

## Cut v0.1.0 when ready

Once you're satisfied:

```bash
cd ~/projects/lithium
git tag v0.1.0 -m "v0.1.0: Anthropic-only CLI MVP"
git push origin v0.1.0
gh release create v0.1.0 --title "v0.1.0 - Anthropic-only CLI MVP" --notes-from-tag
```

Then the `cargo install --git https://github.com/shawnpetros/lithium` line in the README actually works for anyone visiting the repo.

## Phase 2 next session

OpenAI admin key (similar shape: `https://api.openai.com/v1/organizations/usage`) + OpenRouter (`/api/v1/key` endpoint). Each is a new sibling crate (`crates/lithium-openai`, `crates/lithium-openrouter`). The adapter contract lives in `lithium-core` (just push a `Vec<UsageRow>` upstream). Should be a weekend each per the spec.
