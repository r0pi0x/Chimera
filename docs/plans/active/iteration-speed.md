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

Seen while measuring: `cs_source`'s
`view_models::everyone_hears_the_view_models_reload_events_the_others_a_little_late`
failed in all four runs on 55277a2d1, with both runners.

## Next, by estimated saving

| Idea | Estimate | Notes |
|---|---|---|
| Measure the second pass: `tools/agent-timing --since 2026-11-22` against the baseline (tests, commit hook, captures buckets; `stamp` rows in commit-hook.log) | - | A week of data |
| nextest only for big selections (cs_source and the apps above it), `cargo test` for the rest | part of cs_source's fast tier (-7% to -40% above) | Measure on a quiet box first; the split needs two runs and a merged log |
| Cheaper models for mechanical briefs, narrower briefs, `docs/agent-index.md` before reading whole docs | part of the 145 h of model time | Orchestrator's choice; measure per agent type |
| Post-push affected runs coalesced across pushes (already) and split by crate onto idle slots | FAIL latency, not agent time | Only matters once nobody waits |

The architecture's side of the same question (how many crates a change
rebuilds, type-checks and selects; plugins per game) is
[architecture-for-iteration.md](architecture-for-iteration.md).

## Progress

2026-11-21: plan written, baseline taken, items 1-4 done.
2026-11-21: second pass: the hook as the agents' test run, pre-push reuses stamps, the racy key fixed, nextest measured (opt-in), tools/run-built; a shared capture target ruled out.
