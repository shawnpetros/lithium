# SESSION-CONTEXT.md

## Status

**v0.1.0 shipped 2026-04-27.** Anthropic-only CLI MVP. Live-verified against Shawn's real admin API key and against fixture data for the demo. Tag and GitHub release at https://github.com/shawnpetros/lithium/releases/tag/v0.1.0.

22 tests passing, `cargo clippy --all-targets --workspace` clean, demo gif rendering at the top of the README.

## In-Flight

Nothing. v0.1.0 is the natural pause. Phase 2 work (OpenAI + OpenRouter adapters) is next session.

## Key Details

- Repo: https://github.com/shawnpetros/lithium
- Latest release: v0.1.0 (https://github.com/shawnpetros/lithium/releases/tag/v0.1.0)
- Install: `cargo install --git https://github.com/shawnpetros/lithium`
- Local DB: `~/.local/share/lithium/usage.db`
- Config: `~/.config/lithium/config.toml`
- Anthropic admin key required for Cost Report polling. Personal accounts must create an org first before the Admin Keys section appears in the console.
- Demo fixture at `demo/seed-fixture.sh`; tape at `demo/lithium-demo.tape`. Re-record with `bash demo/seed-fixture.sh && vhs demo/lithium-demo.tape`.

## Known limitations carried forward

- Anthropic Cost Report has up to 24h lag; `lithium today` explains this when today's bucket is empty.
- Claude Code rows have no USD attribution (Max is flat-rate); declare in `[fixed_costs]` for total visibility.
- Session/weekly limit percentages not exposed by `stats-cache.json`; punted to Phase 2 or later.

## Next Steps

1. **Phase 2 next session:** OpenAI admin API adapter (similar shape to Anthropic). Then OpenRouter `/api/v1/key` adapter (works with regular keys). Each as a sibling crate.
2. **Phase 3 after that:** daemon split, launchd plist, `cship` status-line segment, SwiftBar menubar plugin.
3. **Phase 4:** OpenClaw MCP hooks for cost gates.
4. **Optional polish before Phase 2:** record an asciinema fallback for accessibility, add `cargo dist` for pre-built binaries, write an `ADAPTER-CONTRACT.md` doc for community contributions.
