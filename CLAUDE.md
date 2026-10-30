# CLAUDE.md

Lucker Party (public monorepo; its engine is **Chimera**, `chimera/`;
"mashup" is the old name of the whole and of the dev sandbox): minigames
built from other games' assets and behavior, with parts mixed across games
(e.g. a Combat Arms character on a Counter-Strike: Source map), and later
original minigames. Layers: Chimera, loaders (`source/`), games as
libraries (`games/`), apps (`apps/`). Planned layout:
[docs/plans/active/engine-split.md](docs/plans/active/engine-split.md).

This file is a map. Follow the links for detail; keep it short.

## Where things are

| Need | Read |
|---|---|
| What the project is, installing and playing | [README.md](README.md) |
| Contributor contract for any agent (rules, setup, done, pitfalls) | [AGENTS.md](AGENTS.md) |
| Ready-to-run task briefs | [docs/tasks/](docs/tasks/README.md) |
| Design decisions and why (slots, mounts, weapon model, legal rules) | [docs/DESIGN.md](docs/DESIGN.md) |
| Code layout, layers, data flow | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Every source file, one line (generated) | [docs/modules.md](docs/modules.md) |
| Where each doc's section starts (generated) | [docs/agent-index.md](docs/agent-index.md) |
| How to check your work (tests, screenshots, live ECS queries) | [docs/OBSERVABILITY.md](docs/OBSERVABILITY.md) |
| Current plan and progress | [docs/plans/active/](docs/plans/active/engine-split.md) |
| What to build next (to-do list) | [docs/backlog.md](docs/backlog.md) |
| Known shortcuts in existing code | [docs/tech-debt.md](docs/tech-debt.md) |
| Writing behavior specs | [specs/README.md](specs/README.md) |

## Hard rules

- Never commit other games' assets, extracted files or encryption keys. Load
  assets at runtime from the user's own install.
- Third-party code never enters this repo and is never translated line by
  line. Behavior reaches us only as specs in our own words, written in a
  separate session. Implementation sessions read specs only.
- Bevy is pinned in `Cargo.toml` (0.20.0) and its API changes often: check the
  pinned crate source in `~/.cargo/registry/src/*/bevy_*-0.20.0/` or its
  examples instead of memory. Same for avian3d (0.8).
- Must build and run on Linux and Windows.
- Scope: first-person games only; CS:S and Combat Arms first.

`cargo test` enforces layering, the asset rule, the spec rule and this file's
links (`checks/tests/it/architecture.rs`). Its failure messages say how to fix things.

## Commands

```
cargo run --features dev       # the game, `cs_source` (apps/cs_source); day-to-day
                                   # (dynamic linking + remote protocol)
cargo run --profile devfull --features dev   # full debug info for inspecting locals
cargo test --features dev -- --skip heavy::   # whole fast tier
cargo nextest run --features dev   # full suite (or `cargo test --features dev`,
                                   # 2x slower)
cargo run -q -p repo_checks --bin affected -- --base main   # which crates' tests
                                   # a branch can affect, and why (pre-commit
                                   # builds those and runs the changed crates'
                                   # fast tier; post-push-tests.sh runs all)
cargo test -p cs_source --features dev --test it weapons::   # one test
                                   # file (games/cs_source/tests/it/weapons.rs)
cargo test -p chimera_world --features dev   # one crate's own tests (also
                                   # chimera_core, chimera_present, chimera_sim, chimera_net,
                                   # chimera_client, source_logic, cs_source, cs_source_app,
                                   # ultimate_gun_game)
cargo test -p cs_source --features dev,ui   # CS:S with its front end (menus, HUD)
cargo test -p source_formats   # one workspace crate's tests (seconds)
cargo test -p repo_checks      # architecture and docs checks (seconds)
                             # Windows: scripts/affected-tests.ps1 -Staged
                             # or -Files <repo-relative paths> for local edits;
                             # -Base upstream/main considers committed changes only
cargo run --features dev -- --help    # screenshot, spawn, look, movement options
cargo run --features dev,tools --bin mapsweep   # debugging tools need `tools`
cargo run --features dev --bin cs_source_server -- +map de_dust2   # dedicated server
cargo build --release -p cs_source_server   # the server as released: no render/audio
cargo run --features dev --bin combat_arms_server -- +map combat_arms:warehouse +MaxPlayer 4   # CA's
cargo run --profile playtest   # optimized, quick to rebuild; for playtesting
cargo run --features dev --bin combat_arms   # Combat Arms' skeleton (greybox)
cargo run --features dev --bin mashup_demo   # mashup: de_dust2 played as Combat Arms
cargo run --features dev --bin lucker_party -- +minigame originals:gun_game_dust2   # Lucker Party
cargo run --features dev --bin lucker_party_server -- +minigame originals:gun_game_dust2 +MaxPlayer 3   # its server
cargo run --features dev --bin ultimate_gun_game   # Ultimate Gun Game (every game's guns)
cargo run --features dev --bin ultimate_gun_game_server -- +map greybox   # its server
tools/agent-slot.sh acquire <branch>   # an agent's warm checkout: cd to the
                                   # printed path, work and commit there; then
                                   # `tools/agent-slot.sh release` (OBSERVABILITY 3e)
source tools/agent-env.sh && agent_run cargo ...   # several agents on one box:
                                   # shared jobserver + agents.slice (OBSERVABILITY 3e)
tools/post-push-tests.sh   # right after each push: the affected tests (heavy too)
                                   # and build checks of what was pushed
cargo hakari generate   # after changing a dependency (chimera/workspace-hack)
cargo run -q -p repo_checks --bin docs_index   # regenerate docs/modules.md, the
                                   # debt list and docs/agent-index.md
```

Each crate's integration tests are one binary, `<crate>/tests/it/` (slow
ones in `tests/it/heavy/`), in the lowest crate whose API they need;
scenario tests run on Chimera's `harness::Sim` with their layer's base
(`SimBasePlugins` in Chimera, `CsSim` in cs_source); docs/OBSERVABILITY.md
section 1 says where new ones go. `tools/post-push-tests.sh` runs the
affected crates' tests (`CHIMERA_PUSH_TESTS=all`: everything, `=0`: none).
Environment variables are `CHIMERA_*` and the local config
`chimera.local.toml`; the old names (`MASHUP_*`, `mashup.local.toml`)
still work.

## Working conventions

- Verify your own work with tests, screenshots or remote queries
  (docs/OBSERVABILITY.md) before calling it done. New behavior gets a
  scenario test.
- Commit at working milestones: `.githooks/pre-commit` builds the
  affected crates' tests and runs the changed crates' fast tier;
  `pre-push` checks identity and `repo_checks` only (enable per clone:
  `git config core.hooksPath .githooks`). Push first, test after: run
  `tools/post-push-tests.sh` right after each push (affected tests, heavy
  too; one-line result in `.claude/post-push/last.txt`) and fix a FAIL
  at once.
- Propose a plan before writing a new mount adapter or loader. Multi-session
  work gets a plan in `docs/plans/active/`, kept updated.
- When something was hard because a tool, doc or check was missing, add it.
- Keep docs true: when code changes a fact in a doc, change the doc in the
  same commit. A file's detail lives in its `//!` doc (shortcuts under
  "Known shortcuts"); docs/modules.md, the code part of tech-debt.md and
  docs/agent-index.md are generated from those docs and the doc headings:
  after changing one, run
  `cargo run -q -p repo_checks --bin docs_index` (a test fails when stale).

## Commit identity

Commits are by `r0pi0x <339403489+r0pi0x@users.noreply.github.com>` with
UTC dates: set that identity in the clone's local git config and commit
with `TZ=UTC`. `.githooks/pre-push` refuses anything else. No real names,
personal emails or machine paths in commits or files.

## Testing on Windows

Remote: https://github.com/r0pi0x/Chimera (public; renamed from
LuckerParty on 2026-10-16, the old URL redirects). The games and the
spec reference material live on the Windows PC, which has its own checkout
(`git clone https://github.com/r0pi0x/Chimera`); `scripts\playtest.ps1`
pulls and runs an optimized build. Spec sessions there write into `specs/`
in that checkout and push; implementation sessions pull them.
