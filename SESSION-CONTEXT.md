# SESSION-CONTEXT.md

## Status

**v0.2.0 shipped 2026-04-27.** OpenAI + OpenRouter adapters wired alongside Anthropic. Cross-provider visibility is live in `lithium today` / `lithium month` / `lithium adapters` / `lithium doctor`. 28 tests passing across 5 crates, clippy clean.

Tag + GitHub release: https://github.com/shawnpetros/lithium/releases/tag/v0.2.0

## In-Flight

Nothing. Phase 2 is the natural pause. Next session is Phase 3 (daemon split + cship segment + SwiftBar menubar).

## Key Details

- Repo: https://github.com/shawnpetros/lithium
- Latest release: v0.2.0
- Workspace crates: lithium-core, lithium-anthropic, lithium-openai, lithium-openrouter, lithium-cli (5 total)
- Live verification status:
  - Anthropic admin API: ✓ verified (Shawn's real key, $10.83 month-to-date)
  - Anthropic Claude Code local: ✓ verified (123 rows from ~/.claude/stats-cache.json)
  - OpenAI: ⚠ wiremock'd only — Shawn would need to drop a sk-admin-... key in config to live-verify
  - OpenRouter: ⚠ wiremock'd only — Shawn has an OpenRouter account, regular API key in `~/projects/<repo>/.env.local` could be transferred to lithium config to verify
- Adapter contract documented at `docs/ADAPTER-CONTRACT.md` for future contributions

## Next Steps

1. **Optional: live-verify OpenAI / OpenRouter** — Shawn drops keys in `~/.config/lithium/config.toml`, runs `lithium doctor` + `lithium poll` + `lithium month`. Same flow as Anthropic Phase 1 verify.
2. **Phase 3:** daemon split (`lithiumd` Rust binary + launchd plist), `cship` status-line segment crate (`lithium-cship`), SwiftBar menubar plugin in `plugins/swiftbar/`. Each ships at finished quality before next surface starts.
3. **Phase 4:** OpenClaw MCP hooks for cost gates + optional web dashboard at `web/`.
4. **Optional Phase 2.5 polish:** per-model OpenAI breakdown via `/v1/organization/usage/completions`, OpenRouter generation activity endpoint if useful, OpenClaw early-warning integration.
