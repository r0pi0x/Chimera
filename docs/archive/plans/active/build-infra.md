# Build infrastructure: a faster local loop

Goal: a one-line change anywhere in the workspace goes from edit to a
green fast tier in well under a minute on the Windows PC, and a new
session's checkout is productive in minutes instead of a half hour. The
Linux dev box keeps its current behaviour. Done is a measured before/
after table for the scenarios below (cold slot, no-op, leaf edit, bottom
edit, playtest rebuild), the scripts and settings that produce it, and
the Windows numbers added to `docs/performance.md` "Test cycle".

This plan is about *building* (cargo, rustc, link, test selection), not
about the game's frame time (that is `docs/performance.md`).

The measured reports are [build-infra-before.md](build-infra-before.md)
and [build-infra-after.md](build-infra-after.md); both are produced with
`scripts\build-bench.ps1` so they can be repeated in any slot.

## What is slow now (2026-10-29, this machine)

Ryzen 7 7800X3D (8 cores / 16 threads), 32 GB, NVMe, Windows 11.
`rustc 1.99.1`, `cargo 1.99.1`, linker already `rust-lld`
(`.cargo/config.toml`). Real-time Defender protection is on; its
exclusions need an administrator to read and are presumably empty.

From the fast-tier logs in the parent checkout and the target directories:

- A from-scratch `cargo test --features dev -- --skip heavy::` in
  `Chimera\` compiled **581 units** (248 registry, 333 local build/test
  targets), linked **44 test binaries** and ran **4,095 tests**. The log
  spans 11 min 0 s (23:16:56-23:27:53), then failed one test.
- The immediate rerun compiled **0 units** and took 2 min 9 s (44
  binaries, same tests), failing three targets. So warm *running* is not
  the main cost on this box: cold compile and linking are.
- The build happened in `Chimera\` (the source paths in the log), the
  clone `SESSIONS.md` says never to work in. It is the only warm target:
  `Chimera\target` is 23.7 GB, `slot2\target` 1.8 GB (partial), and
  `slot1`/`slot3` had **no `target\` at all**. Every slot session
  therefore starts cold or falls back to the main clone.
- The local `slots.ps1` exists but only scans `slot1..slotN` for `N` = 3
  (`-Count` default). A fourth slot (`slot4`) already existed and a fifth
  had to be created by passing `-Count 5`; `status` with the default
  cannot see or release them.
- No shared compile cache on Windows (`sccache` is not installed), no
  `cargo-nextest`, no `CARGO_BUILD_JOBS`, and the fast-tier command
  builds every workspace member even when one leaf crate changed. The
  Linux equivalents (`tools/agent-env.sh`, `tools/agent-slot.sh`,
  `tools/post-push-tests.sh`) are bash-only.
- `[profile.dev]` keeps full debug info while `[profile.test]` uses
  `line-tables-only`, so `cargo run --features dev` and `cargo test
  --features dev` build two separate copies of the dependency graph.
  `[profile.playtest]` is a third.

Targets (to be confirmed by the baseline step):

| scenario | now | target |
|---|---|---|
| no-op `cargo test --features dev --no-run` in a warm slot | not measured | < 5 s |
| one-line edit in a leaf crate, affected fast tier | whole workspace build | < 30 s |
| one-line edit in `chimera_sim`/`chimera_core` | whole workspace build + 44 links | < 3 min |
| first build in a new slot | 30-60 min (cold) | < 8 min (seeded/cached) |
| `cargo run --features dev` after a leaf edit | relinks the app | < 30 s |
| playtest profile, incremental after a leaf edit | not measured | < 60 s |
| full fast tier (4,095 tests) | 2 min 9 s warm | < 2 min |

## Expected impact (predicted, then measured 2026-10-30)

Baseline used for the percentages: the 2026-10-29 log, roughly 8 min
50 s of compile/link plus 2 min 9 s to run 4,095 tests, 581 units and 44
test binaries. Ranges are deliberately wide; the confidence column says
how much of the number is measured elsewhere versus inferred for
Windows.

| lever | what it removes | predicted saving | confidence |
|---|---|---|---|
| Affected selection (Phase 3) | ~40 of 44 links and most local units on a leaf or test edit | 70-90% off a leaf/test edit; 50-60% averaged over all commits (bottom edits get none) | high |
| Warm slot + seeding (Phase 1) | the registry dependency rebuild in a new slot | 50-65% off a new slot's first build (11 min -> 4-5 min) | high |
| sccache (Phase 2) | the same registry codegen, without copying a target | 45-60% off a new slot's first build; little on an already-warm slot | high |
| `debug = "line-tables-only"` for dev (Phase 4) | half the debug info in every local unit and link, and the second dependency graph | 30-45% off a bottom-edit rebuild; 40-50% off link time; ~50% off target size | medium-high (44-50% measured on Linux) |
| Defender exclusions (Phase 5) | scanner work on `target\` and the registry | 10-20% on compile and link steps | medium |
| nextest (Phase 3) | sequential test-binary runs | fast tier run 1.5-2.5x faster (2 min 9 s -> 50-80 s); compile unchanged | high |
| Build jobs 12 vs 16 (Phase 2) | scheduler/memory thrash on 8 cores | 0-15% | low |
| Structural T6 (Phase 7) | how *often* a bottom edit happens (38% of commits) | 20-30% off the long-term average; no per-build change | medium |

Combined, once a slot is warm (predicted before measuring):

| scenario | now | predicted | speedup |
|---|---|---|---|
| New session, first build | 11 min measured here (30-60 min per `SESSIONS.md` on a cold box) | 4-6 min | ~2-3x measured, 5-10x worst case |
| Leaf or test edit, affected fast tier | 2-5 min for the full workspace (inferred) | 15-40 s | 5-10x |
| Bottom edit (`chimera_core`/`world`/`sim`) | 5-8 min (inferred) | 1.5-3 min | 2-3x |
| Full fast tier run | 2 min 9 s | 50-80 s | ~2x |
| Playtest profile, incremental | not measured | 20-40% faster (links + Defender; already `lto = false`) | - |

### Measured (2026-10-30)

The first [before](build-infra-before.md) / [after](build-infra-after.md)
run revised two of these estimates:

| scenario | before | after | measured change |
|---|---|---|---|
| New slot, first test build | 1,225.4 s, 584 units | 559.9 s + 45.1 s seed, 25 units | **-50.6% wall, -95.7% units** |
| Warm `source_formats` edit | 41.6 s, 20 units | 50.0-58.7 s affected, 21 units | affected ~13%, not 50-60% |
| Test-file edit, fast tier | 120-168.9 s, all 4,095 tests | 13.5-15.4 s warm affected | **8-11x** |
| Bottom `chimera_core` edit | 43.4 s, 21 units | 60.8 s affected, 19 units | no gain |
| No-op | 0.8 s | 5.9 s under load, 0 units | no relink regression |

What this changes:

- **Seeding and staying in one warm slot are the first-build win**
  (584 -> 25 units; about 2x wall on the shared box). The 20-minute
  empty-slot build is the actual pain, not the steady-state edit.
- **`affected` is a test-run win, not a build-time win.** It is 11x on
  a test-file edit because it runs one crate's tests instead of 4,095;
  it saves only ~13% on a foundational crate's build and nothing on a
  bottom-crate edit, where the dependents genuinely relink.
- **Warm source/bottom edits are already 40-60 s.** The predicted
  5-10x for leaf builds was too optimistic: cargo already rebuilds only
  the changed units, and `source_formats` has many dependents.
- **Unmeasured this round**: sccache (not installed), Defender
  exclusions (no administrator), nextest (not installed), and a
  line-tables dev profile (the test profile already uses line tables).
  They stay candidates, not claimed gains.

Recommended order of work, revised by the measurement:
Phase 0 (measure) -> Phase 1 (slot fix + seeding + always work in a
warm slot) -> Phase 3 (affected for test runs; nextest for the full
suite) -> Phase 4 (line tables, if bottom-edit build time still
matters) -> Phase 5 (Defender, needs admin) -> Phase 2 (sccache, for
targets that are trimmed or lost) -> Phase 6/7. Phase 1 and 3 delivered
the measured wins; do not spend a day on sccache before the warm-slot
habit is in place.

## Steps

### Phase 0 - Measure, so every later step has a before and after

- [x] `scripts/build-bench.ps1`: times one scenario, records the
  compiled-unit count, jobs and exit code, and appends a markdown row.
  Scenarios: `cold`, `seeded`, `noop`, `leaf-full`, `leaf-direct`,
  `leaf-affected`, `bottom-full`, `bottom-affected`,
  `test-file-affected`, `fast-tier`, `fast-tier-nextest`, `run-leaf`.
- [x] Baseline captured in [build-infra-before.md](build-infra-before.md)
  (six rows); the after run is in
  [build-infra-after.md](build-infra-after.md). The box was shared, so
  the compiled-unit counts and the same-slot pairs are the reliable
  numbers.
- [ ] Add the resulting Windows table to `docs/performance.md` "Test
  cycle" (the architecture review's T6 explicitly notes no Windows
  leaf-edit number exists).
- [x] Do **not** use `Chimera\` for measurement builds: the before run
  used slot 5 with an empty target, the seeded run slot 6.

### Phase 1 - Make warm slots the default, and make them actually warm

- [x] Fix `slots.ps1`: discover `slot*` directories instead of trusting
  `-Count`, show all of them in `status`, and let `release slotN` address
  any existing slot. Keep the hold-file race handling. (Done in
  the local `slots.ps1`, outside the repo; the benchmark found slots 4 and 5
  invisible to the old default.)
- [x] Add `seed <slot>` to `slots.ps1`: `robocopy` a reference `target\`
  (`Chimera\target` by default) into a fresh slot before its first build.
  Copy, do **not** hardlink: lld truncates stable output paths, which
  would corrupt the reference copy through a hardlink.
- [x] Verify what a seed actually reuses: the seeded slot compiled 25
  units against the cold slot's 584, so the registry crates were reused
  and only the path-dependent workspace crates rebuilt.
- [ ] Make `acquire` prefer the slot that last held the branch (as
  `tools/agent-slot.sh` does) and record `target\` size in `status`.
  Trim past a configurable limit (60 GB, `cargo sweep` if present).
- [ ] Document the rule in `SESSIONS.md`: builds run in the slot, never
  in `Chimera\`; a slot is held for the whole session and released at the
  end. Add the seeding command next to `acquire`.
- [ ] Verify: two consecutive sessions in the same slot; the second one's
  leaf edit does not recompile dependencies.

### Phase 2 - One shared compile cache on Windows

- [ ] Install a prebuilt `sccache` (or `cargo install sccache --locked`)
  and give it a directory on the NVMe with a 100 GiB limit, its own port,
  and no inherited `MAKEFLAGS` equivalent.
- [ ] Add `scripts/agent-env.ps1` (PowerShell parity with
  `tools/agent-env.sh`): `RUSTC_WRAPPER`, `SCCACHE_SERVER_PORT`,
  `CARGO_BUILD_JOBS` (start at 12 of 16; measure 8/12/16), unset
  `CARGO_TARGET_DIR`, and in a slot leave `CARGO_INCREMENTAL` at cargo's
  default so workspace crates stay incremental. Outside a slot set
  `CARGO_INCREMENTAL=0` so the registry crates cache. `AGENT_NO_SCCACHE=1`
  and `AGENT_NO_SLOT=1` overrides mirror the bash script.
- [ ] Do not share `CARGO_TARGET_DIR` between slots: cargo fingerprints
  workspace crates by absolute path, so it would rebuild them anyway and
  sccache would hash the variable and miss every entry.
- [ ] Verify with `sccache --show-stats`: a second fresh slot gets hits
  for Bevy/wgpu and friends; the miss list is only workspace crates and
  links. Record the new-slot time.
- [ ] Add a heal check (the Linux lesson: a cancelled build leaks
  sccache's jobserver tokens and later builds wait at 0% CPU forever).
  On Windows, restart the server if `--show-stats` is unreachable before
  a build; document the one-line `sccache --stop-server`.

### Phase 3 - Build and run only what the change can affect

- [ ] Add `scripts/affected-tests.ps1`:
  `cargo run -q -p repo_checks --bin affected -- --staged --format cargo
  --skip-heavy` (or `--base upstream/main`), split its one-per-line
  output, then `cargo test <args> --features dev -- --skip heavy::`.
  This is the Windows equivalent of the pre-commit hook's selection; it
  is the difference between one changed crate's tests and all 44
  binaries.
- [ ] Add `scripts/affected-build.ps1` for the pre-commit split: build
  the affected packages' tests (`--no-run`) but run only the `--direct`
  ones plus `repo_checks`, exactly as `.githooks/pre-commit` does.
- [ ] Install `cargo-nextest` and use it for the full suite and for the
  affected set. `nextest` runs test binaries in parallel (cargo test runs
  them one after another); keep the existing `heavy` memory slots in
  mind and validate the Windows overrides on 32 GB. The fast tier can be
  `cargo nextest run --features dev -E 'not test(/^heavy::/)'`.
- [ ] Make the Windows pre-commit path optional but available: the repo's
  hook is bash and `SESSIONS.md` says not to enable it in this clone; the
  PowerShell scripts give the same selection without the identity hook.
- [ ] Verify: one-line edit in `source_formats` runs one crate's fast
  tier, not the workspace; one-line edit in `chimera_core` still selects
  its dependents (expected, that is the bottom-edit cost).

### Phase 4 - Profiles: stop building the dependency graph twice

- [ ] Set the Windows default build to line tables for our code and
  dependencies as well: either
  `[profile.dev] debug = "line-tables-only"` (then `cargo run` and
  `cargo test` can share the dependency build) or a named
  `devfast` profile if keeping full debug for `cargo run` is worth a
  second graph. The docs already accepted line tables for tests
  (`performance.md` "Test cycle": 197-237 s vs 383-427 s rebuild,
  artifacts a third the size); backtraces keep files and lines.
- [ ] Keep a `devfull`/`debug` profile with `debug = 2` for the rare
  session that needs locals, and document it. The one-time full rebuild
  is paid when a slot is re-seeded, not mid-iteration.
- [ ] Measure the dev/test artifact sharing: after a one-line edit,
  `cargo test --no-run` and `cargo run` should not rebuild Bevy twice.
  If cargo keys the profile name and they still split, keep the cheaper
  profile for tests and use `cargo check` for the run-side loop.
- [ ] Auditing the link: 44 test binaries × 478 MB (line tables) is the
  main bottom-edit cost. `rust-lld` is already the linker; measure a
  replay of one link command with `rust-lld` vs `link.exe` once, and
  keep `-Clink-arg` options only if the measurement supports them.
- [ ] Playtest: keep `[profile.playtest]` (`lto = false`,
  `codegen-units = 16`, `incremental = true`, no debug info) and warm it
  in the slot used for playtesting. A/B `opt-level = 2` for
  dependencies only if cold playtest builds dominate and frame time
  allows it; the party is CPU-bound, so this is a runtime trade and not
  a default.
- [ ] Re-check no-op relinks: `build.rs` already watches the worktree's
  real HEAD/branch ref (not `.git/HEAD`), `test = false` covers
  test-less binaries, and the architecture test enforces it. Add a
  no-op `cargo test --no-run` assertion (< 5 s) to
  `scripts/build-times.ps1` so a future `rerun-if-changed` regression is
  caught immediately.

### Phase 5 - The machine (not the repo)

- [ ] Add Defender exclusions for the checkout root, `%USERPROFILE%\.cargo`
  and the sccache cache (administrator, scoped to those directories;
  document the security trade in the plan, do not put machine paths in
  committed files).
- [ ] Exclude the checkout and target directories from Windows Search
  indexing; confirm the power plan is not throttling during builds.
- [ ] Measure the effect in isolation if possible (one leaf edit with and
  without); if it is in the noise, keep the exclusions anyway as they
  remove a variable when other agents build.

### Phase 6 - Guardrails

- [ ] Add a Windows CI job (GitHub Actions `windows-latest`) that runs
  `cargo check --workspace --features dev`, `cargo test -p repo_checks`,
  and the affected fast tier of the change, with `Swatinem/rust-cache`
  or sccache. This is not local speed; it stops Windows-only breakage
  (the CRLF docs check, the `GetThreadTimes` metrics tests) from being
  discovered by a playtester and fixed in a slow local loop.
- [ ] Keep the Linux scripts and docs as the reference; every Windows
  script's doc comment says which bash script it mirrors and what it
  deliberately leaves out (cgroups/jobserver slices, mold).

### Phase 7 - Structural (tracked, not blocking)

- [ ] Link, do not duplicate, `architecture-review.md` T6: bottom-crate
  churn (`chimera_core`/`world`/`sim`, 38% of Rust commits) is the
  long-term cost. Moving non-vocabulary code out of those crates cuts
  how often 44 binaries relink; measure it with the same scenarios.
- [ ] Revisit the 44 test binaries once Phase 1-4 numbers are in: are
  the tiny unit-test binaries worth separate links? Do not split test
  binaries again (that was the 61-link problem); consider folding only
  where the architecture test allows it.
- [ ] Keep `workspace-hack`/hakari current after dependency changes
  (`cargo hakari generate`), because a stale hack adds duplicate feature
  build graphs.

## Progress

2026-10-29: plan written. Baseline gathered from the two fast-tier logs
in the parent checkout (581 units cold, 44 test binaries, 4,095 tests, 11 min cold /
2 min 9 s warm) and from the target directories (23.7 GB in `Chimera\`,
none in slot1/slot3). The before/after benchmark harness is
`scripts\build-bench.ps1`; the before run is in
`build-infra-before.md`.

2026-10-30: first before/after run, slot 5 (empty target) and slot 6
(seeded from slot 5). Cold 1,225.4 s / 584 units; seeded 559.9 s + 45.1 s
copy / **25 units**. Warm `source_formats` edit 41.6 s full vs 50-58.7 s
affected; `chimera_core` edit 43.4 s full vs 60.8 s affected. Test-file
edit: full fast tier 120-168.9 s vs 13.5-15.4 s affected once the test
binary is built (153.1 s the first time). No-op 0.8 s, 0 units.
`slots.ps1` now discovers all `slotN` and has `seed`; the harness and
`scripts/affected-tests.ps1` are committed. The measurement revised the
priority: the empty-slot build is the pain, and `affected` is a
test-run win rather than a build-time win.


## Decision log

2026-10-29: warm slots are the primary mechanism, not a shared
`CARGO_TARGET_DIR`. Cargo fingerprints workspace crates by absolute
path, so a shared target across different worktree paths rebuilds them
and makes sccache miss on the `CARGO_TARGET_DIR` part of its key.

2026-10-29: seed a new slot by copying (robocopy) a reference target,
not by hardlinking. lld truncates the output file at a stable path on
relink, which through a hardlink would truncate the reference copy too.

2026-10-29: fix `slots.ps1` before adding more slots. It currently
scans only `1..-Count` (default 3) while `slot4` already existed, so
`acquire` can report "every slot is held" with a free slot on disk.

2026-10-29: keep incremental compilation on inside slots and let sccache
serve the registry crates. The Linux measurements showed incremental
inside a fixed-path slot beats `CARGO_INCREMENTAL=0` for workspace
crates (4.3-5.5 s vs 25-47 s after a one-line edit); registry crates are
never incremental and cache either way.

2026-10-29: line tables for the dev profile are the default candidate,
with a full-debug profile kept for the sessions that need locals. The
same trade was already accepted for tests; the Windows cost of full
debug info is paid twice today because dev and test build separate
dependency graphs.

2026-10-29: no Windows-specific build flags beyond `rust-lld` until
measured. The linker is already the fast one; most of the remaining
time is dependency codegen, linking 44 binaries, and doing work the
change did not require.

2026-10-30: measured; seeding is the first-build lever (584 -> 25
units; 1,225 -> 605 s including the copy). It is a copy, not hardlinks,
for the lld-truncation reason above. A seeded slot is the right first
move for a session that cannot reuse an existing warm slot.

2026-10-30: `affected` is for test runs, not build time. A test-file
edit drops from 120-168.9 s to 13.5-15.4 s once the test binary is
built; a `source_formats` build drops only ~13% and a `chimera_core`
build not at all, because the dependents genuinely relink. Do not
promise it as a build-time win.

2026-10-30: do not chase sccache or line tables before the warm-slot
habit. The seeded build already reuses the registry crates; its 560 s is
workspace codegen and 44 links on a loaded box, and sccache cannot
cross the absolute path for those workspace crates. Revisit line tables
only if bottom-edit build time still hurts after the warm-slot habit.

2026-10-30: never benchmark by building in `Chimera\`. It was the only
warm target, which is why the slots stayed cold; slot 5 (empty) and
slot 6 (seeded) are the measurement paths.

## Not doing (for now)

- Nightly-only build flags (`-Zshare-generics`, `-Zthreads`) as a
  default: the toolchain is pinned, and the win is unmeasured here.
- A RAM disk or `target` on a compressed volume: the target is 23-40 GB
  per slot and the NVMe is already fast; the copy/seed path is enough.
- PGO/BOLT or a release-profile rework: playtest already has its own
  profile, and the runtime is CPU-bound in ways `docs/performance.md`
  covers.
- Splitting test binaries again: per-crate `tests/it/` was chosen to cut
  61 links to one per crate; the problem is selecting fewer of them, not
  making more.
