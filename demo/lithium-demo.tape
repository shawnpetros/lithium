# lithium demo tape — recorded with vhs (https://github.com/charmbracelet/vhs)
#
# Walks through the three read commands a Phase 1 user actually runs daily:
#   lithium adapters   what's wired up + last poll status
#   lithium today      today's spend by source/model with totals
#   lithium month      month-to-date + EOM projection (the money shot)
#
# Numbers are fixture data seeded by demo/seed-fixture.sh into a sandboxed
# XDG location, so the recording is reproducible and doesn't leak real
# account spend. Re-record with: bash demo/seed-fixture.sh && vhs demo/lithium-demo.tape

Output assets/demo.gif
Output assets/demo.mp4

Set FontFamily "Fira Code"
Set FontSize 14
Set Width 920
Set Height 620
Set Padding 18
Set Theme "Catppuccin Mocha"
Set TypingSpeed 45ms
Set CursorBlink false
Set LineHeight 1.2
Set Shell bash

# Setup (hidden): point at fixture XDG dirs + put lithium on PATH.
Hide
Type "export PATH=$HOME/.cargo/bin:$PATH"
Enter
Type "export XDG_CONFIG_HOME=/tmp/lithium-demo/config"
Enter
Type "export XDG_DATA_HOME=/tmp/lithium-demo/data"
Enter
Type "clear"
Enter
Show

Sleep 600ms

# 1. Adapters: shows what's configured + last-poll status.
Type "lithium adapters"
Sleep 700ms
Enter
Sleep 3s

Type "clear"
Enter
Sleep 400ms

# 2. Today: per-source per-model spend with running total.
Type "lithium today"
Sleep 700ms
Enter
Sleep 4s

Type "clear"
Enter
Sleep 400ms

# 3. Month: the money shot. Variable + fixed + projection.
Type "lithium month"
Sleep 700ms
Enter
Sleep 6s
