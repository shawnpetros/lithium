#!/usr/bin/env bash
# Seed a fixture lithium DB at a sandboxed XDG location for the demo recording.
# This avoids leaking the real account's data and keeps the demo deterministic.
#
# Numbers are crafted to tell a clean story:
# - month-to-date variable: plausible operator burn ($58.43)
# - per-model split shows Sonnet workhorse, Opus heavy, Haiku cheap, web_search nontrivial
# - day-by-day spread across the last 12 days so projection has real signal
# - fixed Max ($200) declared in config so `lithium month` shows total picture

set -euo pipefail

DEMO_ROOT="/tmp/lithium-demo"
export XDG_CONFIG_HOME="$DEMO_ROOT/config"
export XDG_DATA_HOME="$DEMO_ROOT/data"

rm -rf "$DEMO_ROOT"
mkdir -p "$XDG_CONFIG_HOME/lithium" "$XDG_DATA_HOME/lithium"

# Fixture config — fake keys for all three providers + fixed costs.
cat > "$XDG_CONFIG_HOME/lithium/config.toml" <<'EOF'
[storage]

[poll]
default_cadence_minutes = 15

[providers.anthropic]
admin_api_key = "sk-ant-admin01-DEMO-FIXTURE-KEY-DO-NOT-USE"

[providers.openai]
admin_api_key = "sk-admin-DEMO-FIXTURE-KEY-DO-NOT-USE"

[providers.openrouter]
api_key = "sk-or-DEMO-FIXTURE-KEY-DO-NOT-USE"

[fixed_costs]
anthropic_max = 200.0
chatgpt_pro = 200.0
EOF

# Resolve lithium binary path. Prefer cargo install location since the demo runs
# in non-interactive shells where PATH may not include ~/.cargo/bin.
LITHIUM_BIN="${LITHIUM_BIN:-$HOME/.cargo/bin/lithium}"
if [[ ! -x "$LITHIUM_BIN" ]]; then
  if command -v lithium >/dev/null 2>&1; then
    LITHIUM_BIN=$(command -v lithium)
  else
    echo "lithium binary not found. Install with: cargo install --path crates/lithium-cli" >&2
    exit 1
  fi
fi

# Initialize schema via lithium itself.
"$LITHIUM_BIN" init >/dev/null

DB="$XDG_DATA_HOME/lithium/usage.db"

# Use today's date for today rows; spread last 12 days for month projection.
TODAY=$(date -u +%Y-%m-%d)
NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# Helper to insert one usage row. Args:
#   $1 = days_ago (0 = today)
#   $2 = provider (anthropic, openai, openrouter)
#   $3 = model
#   $4 = source (admin_api or claude_code_local)
#   $5 = cost_usd (use 0 if NULL)
#   $6 = input_tokens
#   $7 = output_tokens
insert_row() {
  local days_ago=$1
  local provider=$2
  local model=$3
  local source=$4
  local cost=$5
  local input_tok=$6
  local output_tok=$7

  local day=$(date -u -v-"${days_ago}"d +%Y-%m-%d)
  local start="${day}T00:00:00+00:00"
  local end_day=$(date -u -v-$((days_ago - 1))d +%Y-%m-%d 2>/dev/null || date -u +%Y-%m-%d)
  local end="${end_day}T00:00:00+00:00"

  sqlite3 "$DB" <<SQL
INSERT INTO usage (
  polled_at, period_start, period_end, provider, source, model,
  input_tokens, output_tokens, cache_read_tokens, cache_create_tokens,
  cost_usd, raw_payload
) VALUES (
  '${NOW}', '${start}', '${end}', '${provider}', '${source}', '${model}',
  ${input_tok}, ${output_tok}, 0, 0,
  ${cost}, '{}'
);
SQL
}

# --- Anthropic (admin_api): today + past 11 days ---
insert_row 0 anthropic "claude-sonnet-4-6"    admin_api 3.18 142000 38000
insert_row 0 anthropic "claude-opus-4-7"      admin_api 1.84  18000  4200
insert_row 0 anthropic "claude-haiku-4-5"     admin_api 0.12  62000  8400
insert_row 0 anthropic "anthropic.web_search" admin_api 0.28      0     0

for d in 1 2 3 4 5 6 7 8 9 10 11; do
  case $d in
    1) sonnet=4.21; opus=2.07; haiku=0.18; search=0.41 ;;
    2) sonnet=2.95; opus=1.58; haiku=0.09; search=0.23 ;;
    3) sonnet=5.83; opus=3.42; haiku=0.21; search=0.55 ;;
    4) sonnet=1.04; opus=0.31; haiku=0.04; search=0.08 ;;
    5) sonnet=3.66; opus=1.92; haiku=0.15; search=0.31 ;;
    6) sonnet=4.48; opus=2.64; haiku=0.19; search=0.44 ;;
    7) sonnet=2.13; opus=0.83; haiku=0.07; search=0.15 ;;
    8) sonnet=3.82; opus=2.18; haiku=0.16; search=0.38 ;;
    9) sonnet=5.21; opus=2.94; haiku=0.20; search=0.49 ;;
    10) sonnet=2.78; opus=1.34; haiku=0.10; search=0.21 ;;
    11) sonnet=4.06; opus=2.43; haiku=0.17; search=0.36 ;;
  esac
  insert_row $d anthropic "claude-sonnet-4-6"    admin_api $sonnet 100000 25000
  insert_row $d anthropic "claude-opus-4-7"      admin_api $opus    18000  4500
  insert_row $d anthropic "claude-haiku-4-5"     admin_api $haiku   55000  7000
  insert_row $d anthropic "anthropic.web_search" admin_api $search      0     0
done

# Claude Code local (token volume only, no cost — Max plan flat-rate)
for d in 0 1 2 3 4 5 6 7 8 9 10 11; do
  insert_row $d anthropic "claude-opus-4-7"   claude_code_local 0 0 1850000
  insert_row $d anthropic "claude-sonnet-4-6" claude_code_local 0 0  920000
done

# --- OpenAI (admin_api, /v1/organization/costs grouped by line_item) ---
insert_row 0 openai "openai.completions" admin_api 2.41 0 0
insert_row 0 openai "openai.embeddings"  admin_api 0.18 0 0

for d in 1 2 3 4 5 6 7 8 9 10 11; do
  case $d in
    1) comp=3.04; emb=0.22 ;;
    2) comp=1.85; emb=0.14 ;;
    3) comp=4.21; emb=0.31 ;;
    4) comp=0.92; emb=0.08 ;;
    5) comp=2.66; emb=0.19 ;;
    6) comp=3.18; emb=0.24 ;;
    7) comp=1.43; emb=0.11 ;;
    8) comp=2.78; emb=0.20 ;;
    9) comp=3.79; emb=0.27 ;;
    10) comp=1.97; emb=0.15 ;;
    11) comp=2.92; emb=0.21 ;;
  esac
  insert_row $d openai "openai.completions" admin_api $comp 0 0
  insert_row $d openai "openai.embeddings"  admin_api $emb  0 0
done

# --- OpenRouter (admin_api, /api/v1/key usage_daily snapshot per day) ---
# OpenRouter only gives "today" so each row represents that day's snapshot.
insert_row 0 openrouter "openrouter (daily)" admin_api 1.18 0 0
for d in 1 2 3 4 5 6 7 8 9 10 11; do
  case $d in
    1) v=2.04 ;;
    2) v=0.92 ;;
    3) v=2.85 ;;
    4) v=0.31 ;;
    5) v=1.66 ;;
    6) v=2.18 ;;
    7) v=0.78 ;;
    8) v=1.94 ;;
    9) v=2.51 ;;
    10) v=1.09 ;;
    11) v=1.83 ;;
  esac
  insert_row $d openrouter "openrouter (daily)" admin_api $v 0 0
done

# Successful poll log entries so `lithium adapters` reports green status everywhere
sqlite3 "$DB" <<SQL
INSERT INTO poll_log (started_at, finished_at, provider, source, status, rows_inserted)
VALUES ('${NOW}', '${NOW}', 'anthropic', 'admin_api', 'ok', 48);
INSERT INTO poll_log (started_at, finished_at, provider, source, status, rows_inserted)
VALUES ('${NOW}', '${NOW}', 'anthropic', 'claude_code_local', 'ok', 24);
INSERT INTO poll_log (started_at, finished_at, provider, source, status, rows_inserted)
VALUES ('${NOW}', '${NOW}', 'openai', 'admin_api', 'ok', 24);
INSERT INTO poll_log (started_at, finished_at, provider, source, status, rows_inserted)
VALUES ('${NOW}', '${NOW}', 'openrouter', 'admin_api', 'ok', 12);
SQL

echo "Fixture seeded at $DB"
echo "XDG_CONFIG_HOME=$XDG_CONFIG_HOME"
echo "XDG_DATA_HOME=$XDG_DATA_HOME"
