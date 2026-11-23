# Brief: an implementation session for a game

Paste into a **fresh** AI session (never the one that wrote the spec).
Fill in the `<...>`. The rules: [AGENTS.md](../../AGENTS.md),
[CONTRIBUTING-GAMES.md](../CONTRIBUTING-GAMES.md); the checklist:
[new-game.md](../new-game.md).

```
You are an implementation session for <Game> (id <id>) in the Chimera
repository. Read AGENTS.md, docs/CONTRIBUTING-GAMES.md,
docs/new-game.md, docs/engine-capabilities.md and
docs/plans/active/<id>.md first.

Task: implement specs/<id>/<topic>.md in games/<id> (<milestone, e.g.
M3 movement>). Read only specs/ and this repository's code. Do not look
at the original game's code in any form, decompiled output, an SDK, or
the material the spec was written from; if the spec is unclear, write
the question into the plan's log and stop.

Work order:
1. Turn the spec's test cases into scenario tests in
   games/<id>/tests/it/ (on harness::<Game>Sim; the new-scenario-test
   skill); heavy tests that read my install go in tests/it/heavy/ and
   pass with a note without it.
2. Implement until they pass. Numbers go in games/<id>/data/ (TOML),
   not in Rust. Use what Chimera has (docs/engine-capabilities.md); if
   something is missing, build it in the game first.
3. Keep docs true: the files' //! docs (shortcuts under "Known
   shortcuts"), the plan's status and log; then
   `cargo run -q -p repo_checks --bin docs_index`.
4. `.githooks/pre-commit --worktree` must pass; commit with a message
   saying what changed and which spec it implements.

Never commit game files, extracted data or keys; never add code from a
source THIRD_PARTY.md does not list.
```
