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

## Next, by estimated saving

| Idea | Estimate | Notes |
|---|---|---|
| Agents run `.githooks/pre-commit --worktree` as their final check instead of hand-picked broad runs (`cargo test -p X` of whole crates, the whole fast tier) | 10-20 h/week of the 60 h of tests | Guidance done (AGENTS.md); measure the `tests` bucket next week |
| The hook's fast tier on nextest where installed, plus `cargo test --doc` for the doctests nextest skips | 30-50% of the hook's test time on multi-binary crates | Changes the pass key (format) and needs the doctest step; post-push already runs the suite under nextest, so tests are nextest-clean |
| Game runs: build the binary once (`cargo build --bin X`) and run `target/debug/X` for each capture instead of `cargo run` (each run re-checks fingerprints, and a feature difference rebuilds) | part of the 22 h of capture waiting | Needs a measurement of how much of a capture run is cargo |
| A warm "capture slot" whose `target/` keeps the app binaries built with `--features dev` | cold app builds (minutes) per capture task | Like the post-push slot |
| Pre-push reuses a stamp for `repo_checks` (it runs that crate's tests on every push) | 30-60 s per push | Same key as the hook's stamp, on HEAD's tree |
| Cheaper models for mechanical briefs, narrower briefs, `docs/agent-index.md` before reading whole docs | part of the 145 h of model time | Orchestrator's choice; measure per agent type |
| Post-push affected runs coalesced across pushes (already) and split by crate onto idle slots | FAIL latency, not agent time | Only matters once nobody waits |

## Progress

2026-11-21: plan written, baseline taken, items 1-4 done.
