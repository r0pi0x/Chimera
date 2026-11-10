---
name: profile
description: Find what takes a frame's or tick's time and measure a fix (Linux perf via tools/perf-profile.sh and profsum; span traces on Windows).
---

# Profile

Guide: docs/performance.md, "Measuring". Linux needs `perf`.

1. Build with frame pointers, and the summarizer:
   ```
   CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-Cforce-frame-pointers=yes" \
     cargo build --profile profiling -p lucker_party --bin lucker_party
   cargo build --profile playtest --features tools -p chimera_tools --bin profsum
   ```
2. Sample (starts the game, waits `--after`, samples `--seconds`):
   ```
   tools/perf-profile.sh --out target/pp/before -- +minigame originals:gun_game_dust2 +MaxPlayer 4
   ```
   Output: `top` table, `flame.svg`, bars, `top.json`, `game.log`.
3. After the fix, same scene, with before/after/delta per function:
   ```
   tools/perf-profile.sh --out target/pp/after --baseline target/pp/before/top.json -- <same args>
   ```
4. Put the before/after numbers in the PR; for a lasting gain add a line
   to docs/performance.md.

- No display (the dev box): `perf record -e instructions:u -c 500000
  --call-graph fp -- target/profiling/perfgate --scenario de_dust2 --ticks
  8000` for a tick (build `--features tools --bin perfgate`), and the
  `frame_bench` example of ultimate_gun_game for a frame with the renderer
  (docs/OBSERVABILITY.md 3c); `perf script --no-inline` into profsum.
- Headless tick cost without a window: the instruction counters in tests
  (`chimera_core::metrics::ThreadCounters`; `apps/party_kit/tests/it/archetypes.rs`).
- Frame time across many views: `refcmp bench --views tools/refcmp/<map>.toml`.
- Windows: `scripts\profile.ps1` (span trace), then
  `tracesum <trace> --folded f.txt` and `profsum f.txt --unit us --frames N --out <dir>`.
- Measure on a quiet machine; note the load (`uptime`) with the numbers.
