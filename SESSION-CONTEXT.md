# SESSION-CONTEXT.md

## Status

Project just scaffolded 2026-04-27. No code written yet. Spec for Phase 1 is complete at `docs/SPEC-PHASE-1.md`. README candidate is at `README.md`. Hero image prompt is at `docs/HERO-IMAGE-PROMPT.md` ready to paste into ChatGPT image gen.

## In-Flight

None. Awaiting the user to:
1. Generate the hero image via ChatGPT image gen using the prompt
2. Place the result at `assets/hero.png`
3. Decide whether to dispatch P1 build to [[Whetstone]] or do it interactively

## Key Details

- Repo not yet on GitHub. Private local only.
- No `Cargo.toml` yet. Phase 1 build session will run `cargo new` to scaffold the workspace.
- Anthropic Admin API key required for Phase 1 (separate from regular API key, generated at console.anthropic.com under Settings -> Admin Keys). Goes in `~/.config/lithium/config.toml`.
- Claude Code session-limit reading uses NoobyGains/claude-pulse pattern: parse `~/.claude/` state files. Reverse-engineer via their Python source if unclear.

## Next Steps

1. Generate hero image (user action, ChatGPT)
2. Initial commit + push to `github.com/shawnpetros/lithium` (public)
3. Phase 1 build session: `cargo new --bin lithium`, implement the spec at `docs/SPEC-PHASE-1.md`. Either Whetstone-dispatch or interactive.
4. Cut v0.1.0 once `lithium today` and `lithium month` are working with real Anthropic data.
5. Loop in `cship` segment + SwiftBar plugin for Phase 3 (after OpenAI + OpenRouter adapters land in Phase 2).
