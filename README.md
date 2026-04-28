<p align="center">
  <img src="assets/hero.png" alt="lithium - cross-provider LLM-spend aggregator" width="100%">
</p>

<h1 align="center">lithium</h1>

<p align="center">
  <em>Mood stabilizer for your AI bill.</em><br>
  One number, every provider, no spreadsheet.
</p>

<p align="center">
  <strong>Status:</strong> v0.2.0. Anthropic + OpenAI + OpenRouter adapters all wired. Daemon split + <code>cship</code> status-line + SwiftBar menubar are Phase 3.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/SQLite-003B57?style=flat&logo=sqlite&logoColor=white" alt="SQLite">
  <img src="https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white" alt="macOS">
  <img src="https://img.shields.io/badge/Linux-FCC624?style=flat&logo=linux&logoColor=black" alt="Linux">
  <img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="MIT">
</p>

---

<p align="center">
  <img src="assets/demo.gif" alt="lithium demo" width="800">
</p>

## What

You use Anthropic. You also use OpenAI. And OpenRouter. Maybe a local model. At the end of the month you have no idea what you spent. Each provider has its own dashboard, none of them talk to each other, and you've been doing the math in a spreadsheet, badly.

`lithium` is a tiny local daemon that polls every provider you use, normalizes the numbers into one SQLite database, and answers exactly one question:

> **How much am I actually spending on LLMs this month, across everything, fixed and variable?**

That's the whole product. No web dashboard, no SaaS, no telemetry, no analytics. The data lives on your machine. The CLI prints the answer.

## Why

Three things go wrong when you run agents across multiple providers:

1. **You don't notice runaway cost until the bill arrives.** A misconfigured Whetstone wave or a forgotten cron can burn $200 in a day before you check.
2. **Fixed costs (Max plans, monthly subscriptions) and variable costs (per-token API) live in different mental buckets.** Most operators only track one. Both add up.
3. **Cross-provider visibility is nobody's job.** Anthropic shows you Anthropic. OpenAI shows you OpenAI. The aggregate is your problem.

`lithium` makes it the daemon's problem.

## Features

| Feature | Description |
|---|---|
| **`lithium today`** | Today's spend, by source, with totals |
| **`lithium month`** | Month-to-date + projected end-of-month |
| **`lithium adapters`** | List configured providers + last-poll status |
| **`lithium config`** | Edit `~/.config/lithium/config.toml` in `$EDITOR` |
| **`lithium doctor`** | Verify config + connectivity + DB health |
| **Anthropic** | Cost Report admin API + Claude Code local-state reader |
| **OpenAI** | `/v1/organization/costs` admin API per-day USD by line item |
| **OpenRouter** | `/api/v1/key` (regular API key works) for daily/weekly/monthly |
| **Fixed costs** | Declare flat-rate subscriptions (Max, ChatGPT Pro) for true total |
| **SQLite storage** | All data local at `~/.local/share/lithium/usage.db` |
| **No telemetry** | Nothing leaves your machine. Period. |

## Quick Start

```bash
# 1. Install
cargo install --git https://github.com/shawnpetros/lithium

# 2. Initialize config + storage
lithium config       # opens ~/.config/lithium/config.toml in $EDITOR
lithium init         # creates the SQLite database

# 3. Add at least one provider's key to the config (uncomment + paste):
#    [providers.anthropic]   admin_api_key = "sk-ant-admin01-..."
#    [providers.openai]      admin_api_key = "sk-admin-..."
#    [providers.openrouter]  api_key       = "sk-or-..."

# 4. Pull data
lithium poll

# 5. Look at it
lithium today
lithium month
```

Each provider is independent — wire only the ones you use. See the per-provider notes below for where to generate each key.

### Getting keys

#### Anthropic (admin key required)

The Cost Report API requires an **admin key** (`sk-ant-admin01-...`), distinct from a regular API key (`sk-ant-api03-...`). On personal accounts, admin keys are gated behind organization existence:

1. Go to https://platform.claude.com/settings
2. If you don't have an org: walk through "Create an organization" first. Personal accounts become 1-person orgs (you're the owner).
3. https://platform.claude.com/settings/admin-keys → Create Admin Key, name it `lithium-local`
4. Paste under `[providers.anthropic] admin_api_key`

`lithium doctor` prints the key prefix so you can verify the type at a glance.

#### OpenAI (admin key required)

`/v1/organization/costs` also requires an admin key (`sk-admin-...`):

1. https://platform.openai.com/settings/organization/admin-keys
2. Create admin key → paste under `[providers.openai] admin_api_key`

#### OpenRouter (regular key works)

`/api/v1/key` accepts any OpenRouter API key — no admin / management key dance:

1. https://openrouter.ai/keys → create or copy an existing key
2. Paste under `[providers.openrouter] api_key`

Bonus: OpenRouter pre-aggregates `usage_daily` / `usage_weekly` / `usage_monthly`, so polling once gives you all three at once.

Output looks like:

```
lithium - 2026-04-27

Anthropic
  API direct           $4.21    (claude-sonnet-4-6: $3.80, claude-haiku-4-5: $0.41)
  Claude Code session  47% used  (resets in 1h 12m)
  Claude Code weekly   23% used  (resets in 4d 2h)

Total today: $4.21
```

## How It Works

```
┌─ Provider adapters (Rust) ─────────────────────────┐
│  anthropic.rs   - Admin API + Claude Code session  │
│  openai.rs      - Admin API           [phase 2]    │
│  openrouter.rs  - /api/v1/key         [phase 2]    │
└────────────────┬───────────────────────────────────┘
                 │
                 ▼
        SQLite at ~/.local/share/lithium/usage.db
                 │
                 ▼
   ┌─────────────┼─────────────┬──────────┬─────────┐
   ▼             ▼             ▼          ▼         ▼
  CLI         cship         SwiftBar   OpenClaw    Web
 today/      status         menubar    MCP tool   dashboard
 month       line                      + hooks    [phase 4]
            [phase 3]      [phase 3]   [phase 4]
```

Phase 1 ships only the CLI. Each subsequent phase adds one surface, polished to the same standard before the next one starts.

## Roadmap

| Phase | Scope | Status |
|---|---|---|
| **P1** | Anthropic adapter + CLI surface | ✅ v0.1.0 |
| **P2** | OpenAI + OpenRouter adapters | ✅ v0.2.0 |
| **P3** | Daemon split + `cship` segment + SwiftBar menubar | Not started |
| **P4** | OpenClaw MCP hooks (cost gates) + optional web dashboard | Not started |

The discipline: each phase ships at finished quality before the next one starts. No half-built surface in `main`. See [docs/ADAPTER-CONTRACT.md](docs/ADAPTER-CONTRACT.md) if you want to contribute another provider.

## Privacy

`lithium` runs entirely on your machine. No analytics, no telemetry, no phoning home. The only network calls go directly to provider APIs (Anthropic, OpenAI, OpenRouter) using the admin keys you provide. Source is auditable; if you find a single egress that isn't to a provider you configured, open an issue and call it out.

## Tech Stack

- **Rust** for the daemon and CLI
- **SQLite** for storage (via `rusqlite`)
- **Reqwest** for provider API calls
- **Tracing** for structured logs
- **Clap** for the CLI
- **Tokio** runtime

## Contributing

`lithium` is built in the open as a santifer-discipline project: each phase ships at finished public-portfolio quality before the next one is started. Issues, PRs, and adapter contributions for additional providers welcome. Adapter contract is documented in `docs/ADAPTER-CONTRACT.md` (added in Phase 2).

## License

MIT. See `LICENSE`.

## Author

Built by [Shawn Petros](https://github.com/shawnpetros) ([petrosindustries.com](https://petrosindustries.com)).

---

<p align="center"><sub>Named after the periodic-table element and the mood stabilizer. Both stop runaway.</sub></p>
