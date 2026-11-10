---
name: playtest
description: Run one of the game apps (Lucker Party, Ultimate Gun Game, CS:S, the mashup) and take screenshots of it, to see a change working in the real app. Needs the games installed.
---

# Playtest an app and screenshot it

Needs Counter-Strike: Source installed (the party apps and the mashup also
Combat Arms with its keys in `chimera.local.toml`; never read that file).
Without installs, use a scenario test instead (new-scenario-test skill).

1. Pick a scratch folder outside the repo for images: `OUT=$(mktemp -d)`.
2. A tool run (never writes the user's config, takes free ports):
   ```
   cargo run --features dev --bin lucker_party -- +minigame originals:gun_game_dust2 \
       --screenshot $OUT/party.png --frames 300
   cargo run --features dev --bin ultimate_gun_game -- --screenshot $OUT/ugg.png --frames 300
   cargo run --features dev -- --map cs_source:de_dust2 --spawn 0,75,0 --look 0,-89 \
       --movement chimera:noclip --screenshot $OUT/dust2.png
   ```
   `cargo run --features dev -- --help` lists `--window WxH`, `--views`,
   `--join`, `--scale`. On Linux the run needs a display
   (`WAYLAND_DISPLAY`/`DISPLAY` set), unless it runs offscreen: add
   `--offscreen 1920x1080` (no window system at all; `--dump <dir>
   --dump-every <n>` saves frames, `--dump-every 1` with ffmpeg a clip,
   and the run prints its frame times; docs/OBSERVABILITY.md section 2,
   "Offscreen").
3. Read the PNG and check what changed. Put before/after images in the PR,
   never in the repo.
4. Step by step (a live run): start without `--screenshot`, find the
   `remote protocol on 127.0.0.1:<port>` line in its log, then send
   console commands (`setpos`, `give`, `screenshot <file>`) as
   docs/OBSERVABILITY.md section 3b shows. Stop the game by its pid,
   never `pkill -f`; don't end with `+quit` (it writes the user's config)
   unless `CHIMERA_CFG_DIR` points at a scratch folder.

Optimized build for feel: `cargo run --profile playtest --bin <app>`.
Windows: `.\scripts\playtest.ps1 lucker_party` (pulls, then runs).
