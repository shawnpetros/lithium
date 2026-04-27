# lithium

## Purpose

Cross-provider LLM-spend aggregator. A local daemon that polls each provider's usage API, normalizes into SQLite, and exposes the result through multiple UI surfaces (CLI, status line, menubar, agent harness hooks). The single question it answers: **"How much am I actually spending on LLMs this month, across all providers, fixed and variable?"**

Phase 1 scope: Anthropic only, CLI surface only. Public from day 1, santifer-grade README polish.

## Vault Sync

Search terms for vault on session start:
- `lithium`
- `LLM spend` / `LLM cost` / `LLM aggregator`
- `claude-pulse`
- `cship`

Relevant vault pages to pull:
- `50-References/santifer-portfolio.md` (origin synthesis, contains the actionable that spawned this project)
- `50-References/claude-code-terminal-tools-2026-04-25.md` (cship install context)

## Feature Tracker

`features.json` at repo root. Currently in P0 (Direction & Foundation) plus P1 (Phase 1: Anthropic CLI MVP) defined in detail. P2-P4 are sketch only.

## Project-Specific Rules

- **No secrets in the repo, ever.** Admin keys, API keys, OAuth tokens live in `~/.config/lithium/config.toml` (gitignored). The repo references the path, never the value.
- **Public from day 1.** This is a santifer-discipline project. The first commit goes to a public GitHub repo with a finished README, hero image, and at least one working command. Do not build for weeks then publish.
- **Each phase ships at finished quality.** No half-built surface in main. If a feature is in `main`, it works, has at least one example, and is documented in the README.
- **Rust is the implementation language** to match [[Whetstone]] and [[Content Pipeline V3]]. Match Whetstone's crate organization patterns.
- **Polled, not push-based.** The daemon polls provider APIs on a schedule. Push-based usage tracking (interceptor-style) is out of scope; it would require modifying every caller, which defeats the cross-provider point.
- **Conservative poll cadence.** Default 15 minutes. Provider-specific rate limits respected. No "real-time" claim until phase 3 streaming work is shipped.
- **Hero image lives in `assets/hero.png`** referenced from README. Generated via the prompt at `docs/HERO-IMAGE-PROMPT.md`.

## Conventions

- Crate-per-adapter layout (`crates/lithium-anthropic`, `crates/lithium-openai` future, etc.) so each adapter is independently testable.
- SQLite at `~/.local/share/lithium/usage.db` by default. Configurable via `~/.config/lithium/config.toml`.
- Config TOML at `~/.config/lithium/config.toml`. Spec includes example.
- CLI entry: `lithium <subcommand>`. Subcommands documented in `docs/SPEC-PHASE-1.md`.
- All structured logging goes through `tracing` crate, never `println!` for non-output.
- All commands MUST log function entry, error branches, and external call results per global `~/.claude/CLAUDE.md` debug-logging rule.

## Reading

Before starting any implementation work, read in order:
1. `docs/SPEC-PHASE-1.md` (the canonical spec)
2. `README.md` (the public face the work is in service of)
3. `features.json` (current phase + active feature)
