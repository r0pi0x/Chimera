# Agents' iteration speed

Goal: cut the wall time agents spend per task outside thinking (waiting on
tests, hooks, post-push runs, builds and game runs) and keep it measured,
so each change to the workflow shows up in the numbers. The test cycle's
own speed (test binaries, heavy tier, the commit gate's build) is
[test-speed.md](test-speed.md); this plan is the agents' side of it.

Measure with `tools/agent-timing` (docs/OBSERVABILITY.md 3e, "Where agents'
time goes"): it reads Claude Code's transcripts of the subagents and prints
hours per bucket, how much of each was waiting, tools/wait-for's named
waits and the commit hook's log (stamp reuses vs runs).

## Baseline (2026-11-21)

`tools/agent-timing --since 2026-11-14` on the dev box, 339 agents of
7 days (1,136 h of summed agent wall time, 374 h accounted for by tool
calls and model turns; the rest is time between tasks):

| Bucket | Hours | Of it waiting in loops |
|---|---:|---:|
| Model thinking and writing | 145.1 | - |
| Tests (agents' own runs) | 59.6 | 28.5 |
| Game runs and captures | 40.0 | 21.9 |
| Post-push tests | 33.2 | 30.6 |
| Commit hook | 32.6 | 23.1 |
| Waiting, unidentified | 23.8 | 23.8 |
| Builds | 19.6 | 7.3 |
| Scripts, edits, reads | 15.9 | - |
| Git sync and slots | 3.6 | - |

An earlier pass over the same week with a broader attribution (waits on a
background command's notification counted to that command) put post-push
at 61 h (59 h waiting, 18 min a wait on average), the commit hook at 50 h
(42 h waiting) and unidentified waits at 62 h. Either way: the two
biggest avoidable items were waiting for post-push runs the rule already
said not to wait for, and the hook re-running the fast tier the agent had
just run. Also seen: `until ...; sleep` loops left running for days after
their agents finished (18 killed on 2026-11-21), rebases made without
TZ=UTC refused at push, and agents waiting for post-push runs that a newer
push had already superseded.

## Done (2026-11-21)

1. **Nobody waits for post-push.** `tools/post-push-tests.sh --bg --note
   "<task>"` detaches the run (setsid; it outlives the agent) and returns.
   A FAIL writes a record (`.claude/post-push/fails/<time>-<commit>.md`:
   failing steps and tests, the commits since the last pass, the pushes'
   notes from `pushes.txt`, the log, a fix brief) and one line to
   `.claude/post-push/inbox`; the orchestrator watches that with one
   `tail -n0 -F .claude/post-push/inbox` and dispatches a fix agent from
   the record. A pass after a fail adds `FIXED`. Logs are capped (newest 40).
   Expected saving: the post-push waiting, 30-59 h a week.
2. **The commit hook reuses a fresh pass.** `.githooks/pre-commit
   --worktree` runs what the hook will run, on the working tree (every
   change against HEAD), and stamps the pass; the hook stamps its own too.
   A commit whose working tree (tracked files' tree, untracked files'
   names, sizes and times), toolchain, mode and CHIMERA_* settings match a
   stamp whose packages cover the commit's selection doesn't run them
   again. Nothing is weaker: the hook always tested the working tree, and
   the stamp is of exactly that tree. Stamps: `<git common
   dir>/test-stamps`, newest 100. Expected saving: most of the hook's
   23-42 h of waiting (the agent's own final run and the hook's become
   one), measured from `commit-hook.log` (`stamp` rows) from now on.
3. **Measured continuously.** `tools/agent-timing` (the transcripts'
   breakdown); `tools/wait-for <label> ...` instead of hand-written wait
   loops (named, timed out, ends with the shell that started it, logged
   to `<git common dir>/agent-timing/waits.log`); the hook logs each run
   to `commit-hook.log`.
4. **UTC in agents' shells.** `tools/agent-env.sh` exports `TZ=UTC`, so
   commits and rebases in a shell that sourced it are dated as the
   pre-push hook wants; the hook's refusal names the rebase that fixes it.

## Done (2026-11-21, second pass)

Measured on the dev box with other agents building (load 38-65 on 12
threads), so wall times are rough; each comparison alternated its runs.

1. **Agents' test run is the hook's.** AGENTS.md, the task template
   (`.github/ISSUE_TEMPLATE/agent-task.md`) and OBSERVABILITY 3e say:
   `.githooks/pre-commit --worktree` is the test run before committing,
   not whole-crate `cargo test -p` runs or the whole fast tier; while
   iterating, one test module. The hook now prints the exact commands it
   runs and, with `--worktree`, what passed (packages type-checked and
   tested, seconds). Orchestrators' briefs should say the same in one
   line: "Test with `.githooks/pre-commit --worktree` (the commit reuses
   its pass); don't run whole crates' tests by hand." Saving: measure the
   `tests` bucket with `tools/agent-timing` next week.
2. **The push hook reuses the commit's pass.** The stamp code moved to
   `.githooks/stamps.sh`, sourced by both hooks; `pre-push` skips `cargo
   test -p repo_checks` when a stamp of the same working tree tested
   `repo_checks` (every commit hook run does) and stamps its own pass. A
   push right after a commit costs ~0.2 s instead of 15-77 s (that run's
   time here, warm). It now runs `repo_checks` with `--features dev --
   --skip heavy::` (the crate has no heavy tests and an empty `dev`), the
   same build as the commit hook's.
3. **A stale-key bug fixed.** The key's scratch index copy got a new
   mtime, so git's "racily clean" check missed a same-size edit made in
   the same second as the index was written, and the key could be the old
   tree's: a commit could have reused a stamp of other content. Found by
   the new stamps test (`checks/tests/it/scripts.rs`): 2 of 10 and 2 of
   20 stress runs failed; with `cp -p` (the copy keeps the index's time)
   30 of 30 pass.
4. **nextest for the hook: measured, kept opt-in.** Fast tier, warm,
   `cargo test` against `cargo nextest run` plus `cargo test --doc`:

   | Packages | `cargo test` | nextest + doctests |
   |---|---:|---:|
   | chimera_world | 5.4, 4.4 s | 23.4, 26.6 s |
   | repo_checks | 77.4, 65.3 s | 86.6, 56.5 s |
   | cs_source | 277.8, 325.8 s | 258.4, 195.5 s |

   A process per test costs small crates several times their run (each
   test binary loads the dynamic Bevy), and only the biggest crate gains.
   Default stays `cargo test`; `CHIMERA_COMMIT_RUNNER=nextest` switches
   (both run the same tests, so stamps are shared). Windows measured no
   gain earlier (docs/performance.md, "nextest on the Windows fast tier").
5. **Game runs without cargo.** `tools/run-built [--features F] <bin> --
   <args>` builds only when the working tree's key (the stamps' key plus
   bin, features and profile) differs from the binary's last build
   (`target/run-built/`), and runs it with `cargo run`'s environment
   (`CARGO_MANIFEST_DIR`, the library path for the dynamic Bevy). Start
   overhead with nothing changed: 0.35-0.45 s against `cargo run`'s
   1.3-3.9 s (cs_source, `--help`, three rounds); an offscreen run of
   cs_source through it rendered as with `cargo run`. Non-Linux: runs
   `cargo run`. In the playtest skill.
6. **A shared warm capture target: not done.** Slots are separate
   checkouts: a shared `CARGO_TARGET_DIR` makes cargo's build lock
   serialize every slot's builds, the workspace crates' fingerprints
   differ by path (each switch rebuilds them), and sccache doesn't cache
   with it set (tools/agent-env.sh unsets it). Each slot's own target
   already stays warm between tasks; a slot's whole-workspace build is
   ~30 GB, and a separate capture target would be another 20-30 GB on a
   disk at 75%.

7. **The hook's type-check beside its tests.** The architecture study
   (architecture-for-iteration.md) measured 704 s for the hook after a
   one-constant `chimera_sim` edit, 131 s of it tests and the rest
   `cargo check --all-targets` of 29 packages. Now on Linux: the fast
   tier's binaries are built (`cargo test --no-run`) and run directly
   while the check runs (cargo holds its locks for a whole `cargo test`,
   so a second cargo waited out the tests: measured, a check started 8 s
   into a 57 s `cargo test` finished at 73 s); doctests after; the check
   leaves out the packages the fast tier builds. The log gains the
   phases. Same edit (a constant in a function appended to
   `chimera_sim`'s lib.rs, a new value per run), slot1, alternating,
   `CHIMERA_COMMIT_STAMPS=0`:

   | Run | Mode | Load | Wall | Check | Fast tier |
   |---|---|---:|---:|---:|---:|
   | 1 | before | 65 | 249 s | 42 s | 204 s |
   | 2 | after | 33 | 64 s | 46 s | 57 s |
   | 3 | before | 36 | 90 s | 42 s | 46 s |
   | 4 | after | 55 | 70 s | 49 s | 61 s |

   About -25% warm (90 to 64-70 s at similar loads); the first run paid a
   cold test build. In this slot the check itself was 42-49 s, not the
   study's ~570 s: incremental check builds of the same packages and
   features were warm. A cold check (another feature set, a merge that
   touched many crates) is where the remaining cost is; see Next.

Seen while measuring: `cs_source`'s
`view_models::everyone_hears_the_view_models_reload_events_the_others_a_little_late`
failed in all four runs on 55277a2d1, with both runners.

## Checkpoint 2026-11-23

`tools/agent-timing --since 2026-11-22` (66 agents, 91.8 h wall, 73.5 h
accounted; 36 commits, a partial day) against the baseline week (the
`--since 2026-11-14` totals minus that, 346 agents, 387.8 h accounted;
about 1,150 commits, so per commit is not comparable: shares of accounted
time are). The report now files named waits (`tools/wait-for`) under their
own bucket; the old "unidentified" 24 h is 0.3 h now.

| Bucket | Baseline h (share) | Now h (share) | Waiting before / now |
|---|---:|---:|---:|
| Model thinking and writing | 148.3 (38%) | 17.5 (24%) | - |
| Tests (own runs) | 62.2 (16%) | 13.4 (18%) | 31.0 / 7.4 |
| Game runs and captures | 40.8 (11%) | 2.2 (3%) | 22.3 / 0.8 |
| Commit hook | 34.0 (9%) | 5.8 (8%) | 24.4 / 5.4 |
| Post-push tests | 35.3 (9%) | 1.2 (2%) | 32.5 / 0.6 |
| Builds | 21.2 (5%) | 7.5 (10%) | 8.7 / 5.5 |
| Waiting, named + unidentified | 25.3 (7%) | 24.2 (33%) | all |

Improved: post-push waiting 32.5 h to 0.6 h (nobody waits); captures
waiting 22 h to 0.8 h; unidentified waits 24 h to 0.3 h (named now).
Not improved: the hook's stamp reuse is 40 of 170 runs plus stamps (24%),
as in the first day of logging (6 of 25); 42 of 130 hook runs failed, and a
failed run averages 741 s against 408 s for a pass (7.9 h in three days).
The biggest bucket is now named waiting (24 h), mostly builds (cs_source,
ugg, `slot12 build`: 15 of 15 cs_source waits ended in the wait-for timeout
after 2.8 h) and `pc` (Windows PC); then the hook and builds together
(13 h). Of 1,334 logged waits 780 ended in timeout, 621 of them at 6 min
or less (a poll in slices, 30 h): counted as waits, not failures.

Top remaining wastes:

1. **Failed hook runs run to the end** (`--no-fail-fast`, 7.9 h per 3
   days). Done: `CHIMERA_HOOK_FAILFAST=1` stops at the first failure; try
   it in briefs for iteration, keep the default for the final run. Saves
   perhaps 4 h per 3 days.
2. **Cold builds waited on in the foreground** (cs_source cold 1-2 h
   timeouts, `slot12 build`, ugg: ~8 h per 3 days). Done: `agent-slot
   acquire` takes the free slot warmest for the branch (the fewest Rust
   files and manifests between its last commit and the base or
   origin/main; a lockfile or settings difference counts as nearly cold;
   then the newest build) and prints "warm", "lukewarm" or "cold";
   AGENTS.md and OBSERVABILITY 3e say to start long builds in the
   background and wait with tools/wait-for. Still open: the sccache hit
   rate in the report.
3. **Hook reuse stays at 24%**: agents change a file after the
   `--worktree` pass (docs, rebase) and the stamp no longer matches. Fix
   (plan item): key the stamp on the tested crates' inputs, not the whole
   tree, so doc-only or other-crate edits keep it. Saves up to 5 h.
   Done: passes are reused per package (`stamp_reuse`, `affected --trees`
   on what changed since a stamp's tree); `commit-hook.log` says
   `partial` for a run that reused some.

Simulated agent sequence (a leaf crate, cs_source_server, changed;
then a docs edit, a rebase over another crate's commit, the commit; warm
slot), packages whose fast tier ran per hook run:

| Step | Before | After |
|---|---|---|
| `--worktree` pass | cs_source_server, repo_checks | cs_source_server, repo_checks |
| docs edit, `--worktree` | both again | repo_checks (cs_source_server reused) |
| rebase over hl2dm_server, `--worktree` | both again | repo_checks (reused) |
| commit | stamp | stamp |
| The changed crate's pass reused | 1 of 3 later runs | 3 of 3 |

repo_checks reruns after any change by design (it holds the docs rules);
its fast tier is ~16-28 s warm, so the saving is the agent's own crate
(cs_source's fast tier is minutes) and its dependents' type-check. The
lookup costs about 2 s per stamp consulted (a `cargo run` of `affected`;
at most 3, only stamps holding a package the run needs). Wall time for
this leaf was flat (its tests take seconds); judge the change by
commit-hook.log's `partial` and `stamp` share next week.

Next: measure FAILFAST use and the `partial`/`stamp` share next week.

## Next, by estimated saving

| Idea | Estimate | Notes |
|---|---|---|
| Measure the second pass: `tools/agent-timing --since 2026-11-22` against the baseline (tests, commit hook, captures buckets; `stamp` rows in commit-hook.log) | - | A week of data |
| The hook's check when cold (the study's ~570 s): check only the dependents' libraries and binaries in the hook and leave their test targets to post-push (`CHIMERA_COMMIT_DEPENDENTS=lib`), or keep one feature set for every check so its build stays warm | most of a cold hook run | A weaker gate (a dependent's test code breaking shows after the push): the user's call; measure first how often the check is cold (`commit-hook.log`'s check column) |
| nextest only for big selections (cs_source and the apps above it), `cargo test` for the rest | part of cs_source's fast tier (-7% to -40% above) | Measure on a quiet box first; the split needs two runs and a merged log |
| Cheaper models for mechanical briefs, narrower briefs, `docs/agent-index.md` before reading whole docs | part of the 145 h of model time | Orchestrator's choice; measure per agent type |
| Post-push affected runs coalesced across pushes (already) and split by crate onto idle slots | FAIL latency, not agent time | Only matters once nobody waits |

The architecture's side of the same question (how many crates a change
rebuilds, type-checks and selects; plugins per game) is
[architecture-for-iteration.md](architecture-for-iteration.md).

## Progress

2026-11-21: plan written, baseline taken, items 1-4 done.
2026-11-21: second pass: the hook as the agents' test run, pre-push reuses stamps, the racy key fixed, nextest measured (opt-in), tools/run-built; a shared capture target ruled out. Then the hook's type-check beside its tests (-25% warm), phases logged, stamps written when nothing was only checked.
2026-11-21: sccache-heal no longer restarts the server (it killed other agents' running builds, exit 101, nothing printed): `agent-env.sh` starts one only when none listens, under a lock, and heal only reports a hung one (`tools/sccache-heal.sh --force` restarts it by hand); test in checks/tests/it/scripts.rs. One right way against full rebuilds: always source `agent-env.sh` (RUSTFLAGS/mold, incremental and target dir differ otherwise; docs/OBSERVABILITY.md, "Shared compile cache").
2026-11-22: post-push starved for six hours (no verdict over ~15 pushes): a `sleep infinity` jobserver holder that `scripts::sourcing_agent_env_leaves_a_running_sccache_server_alone` left behind inside a post-push run had inherited the lock's fd and held it after the run ended, so every later run waited and the waiters replaced one another forever. Fixed: the run closes the lock's fd for all it starts, a lock held by no live run is moved aside after two polls, the inbox gets `STALE` lines (the lock; no verdict for 2 h while pushes wait), a run from a slot's older copy re-executes the pushed copy, and the test kills its holder (about 80 had piled up); tests `scripts::post_push_*`.
2026-11-23: hook stamps reused per package (a docs edit, a docs_index regeneration or a rebase over other crates after a `--worktree` pass reruns only repo_checks; the agent's crate is reused; tests `scripts::stamps_reuse_each_package_whose_inputs_did_not_change`); `agent-slot acquire` prefers the warmest slot and says how warm (`scripts::acquire_prefers_the_warmest_slot_and_says_so`); background builds in AGENTS.md.
