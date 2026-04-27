# SESSION-CONTEXT.md

## Status

Repo live at https://github.com/shawnpetros/lithium (public, MIT). Initial scaffold pushed 2026-04-27: README + hero image + Phase 1 spec + project frame. No Rust code yet. Phase 1 build is the next session.

## In-Flight

Nothing currently active. Awaiting decision: dispatch P1 to [[Whetstone]] for an autonomous build OR run the build interactively.

## Key Details

- Repo: https://github.com/shawnpetros/lithium
- Topics set: llmops, cost-tracking, claude, anthropic, openai, openrouter, rust, cli, developer-tools, ai-agents, observability
- Anthropic Admin API key required for P1 build (separate from regular API key, generated at console.anthropic.com -> Settings -> Admin Keys). Goes in `~/.config/lithium/config.toml` per the spec.
- Claude Code session reader uses NoobyGains/claude-pulse pattern. Their Python source is the reference; do NOT vendor it, just understand the file format.
- Hero alts at `assets/hero-alts/` are gitignored (local-only reference).

## Next Steps

1. Decide Whetstone-dispatch vs interactive for the P1 build session.
2. Phase 1 build: `cargo new --bin lithium-cli`, then implement the spec at `docs/SPEC-PHASE-1.md`. All 11 P1 features in `features.json` with their scenarios.
3. Verify exact Anthropic Admin API endpoint shape from docs.anthropic.com at build time (one of the four open questions in the spec).
4. Reverse-engineer Claude Code state file format by reading `~/.claude/` plus NoobyGains/claude-pulse Python source.
5. Record demo gif with `vhs` once `lithium today` and `lithium month` are working.
6. Cut v0.1.0 with GitHub release notes once Phase 1 acceptance criteria pass.
