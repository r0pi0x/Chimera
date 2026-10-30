# AGENTS.md

The contributor contract, for any coding agent (Claude Code, Codex, Cursor,
Aider, ...) and the engineer directing it. Read this first; follow its links
for detail. [CLAUDE.md](CLAUDE.md) is the short map of the repository and
holds the full command list.

## What this is

Chimera is a Rust game engine (Bevy 0.20) that rebuilds first-person games
from the player's own installs and mixes their parts: a Combat Arms soldier
on a Counter-Strike: Source map. **Lucker Party**, a party game of
minigames built from those parts, is where it is heading. Layers, lowest
first: the engine (`chimera/`), loaders (`source/`, `lithtech/`), games as
libraries (`games/`), apps (`apps/`). Each layer uses only the ones below.
[README.md](README.md) says what works; [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
maps the code; [docs/DESIGN.md](docs/DESIGN.md) gives the reasons.

## Hard rules (checked; a PR that breaks one is refused)

1. **No other games' files.** Never commit game assets, extracted files,
   encryption keys or anything read from `chimera.local.toml`. Assets load
   at run time from the user's install. Don't read or print
   `chimera.local.toml` (or the old `mashup.local.toml`) at all: it holds
   keys. (`checks/tests/it/architecture.rs`: `no_game_assets_tracked`,
   `no_keys_tracked`; `.gitignore`.)
2. **Specs only.** Third-party code never enters the repo and is never
   translated. Original-game behaviour reaches the code only as specs in
   [specs/](specs/README.md), written in our own words in a separate
   session. Implementation work reads `specs/` and our code, nothing else.
   Specs hold no code blocks (`specs_contain_no_code_blocks`). If a spec
   is missing or unclear, stop and say so in the PR; don't guess from
   memory of the original.
3. **Linux and Windows.** Everything must build and run on both. Text is
   LF everywhere ([.gitattributes](.gitattributes)).
4. **Commit identity.** Commits are by
   `r0pi0x <339403489+r0pi0x@users.noreply.github.com>` with UTC dates
   (`TZ=UTC git commit ...`), set in the clone's local config by
   [tools/setup](tools/setup) ([scripts/setup.ps1](scripts/setup.ps1)).
   No real names, personal emails or machine paths (`/home/<you>/`) in
   commits or files. `.githooks/pre-push` refuses anything else.
5. **Never skip hooks** (`--no-verify`, `-n`). Fix what they report.
6. **Layering, the asset rule, the spec rule and the docs' links** are
   tests: `cargo test -p repo_checks`. Their failure messages say how to
   fix things.
7. **Bevy and avian3d APIs** change often: check the pinned crate source
   (`~/.cargo/registry/src/*/bevy_*-0.20.0/`, avian3d 0.8) or its examples,
   not memory.

[.claude/settings.json](.claude/settings.json) makes Claude Code refuse
the riskiest of these (reading the local config or keys, `--no-verify`,
changing the git identity, force-pushing main). Other agent tools: apply
the same rules in your tool's own permission settings.

## Setup

```
tools/setup            # Linux: repo-local identity, hooks (core.hooksPath .githooks)
tools/doctor           # checks toolchain, identity, hooks, installs, ports, CRLF;
                       # prints the exact fix for each failure
```

Windows (PowerShell): `.\scripts\setup.ps1`, then `.\scripts\doctor.ps1`.
Most tests need no game install; those that do skip without one. Running the
apps needs Counter-Strike: Source (Steam app 240), the party apps also
Combat Arms (README, "Lucker Party").

## Build, run, test

```
cargo test --features dev -- --skip heavy::      # the whole fast tier
cargo test -p <crate> --features dev             # one crate's tests
cargo test -p repo_checks                        # the repository's rules (seconds)
cargo run -q -p repo_checks --bin affected -- --base main   # which crates a branch affects
cargo run -q -p repo_checks --bin logsum -- <log>   # a saved log: counts, first failure, tail
.\scripts\affected-tests.ps1 -Base origin/main   # Windows: those crates' fast tier
cargo run --features dev -- --help               # screenshot, spawn, look options
```

- **Where tests go:** each crate has one integration test binary,
  `<crate>/tests/it/` (slow ones in `tests/it/heavy/`), in the lowest crate
  whose API the test needs. Scenario tests run headless on `harness::Sim`
  (`Sim::new(SimBasePlugins, GreyboxMapPlugin)` in Chimera's crates,
  `CsSim` in `cs_source`). [docs/OBSERVABILITY.md](docs/OBSERVABILITY.md)
  section 1.
- **Screenshots:** `cargo run --features dev -- --screenshot <scratch>/a.png
  --frames 90 --spawn 0,1,12 --look 30,-10`; write them outside the repo.
  Section 2.
- **Live state:** with `--features dev` the game serves Bevy's remote
  protocol (JSON-RPC; tool runs log the port they took). Sections 3 and 3b
  (the console from outside).
- **Long output:** don't print a build or test log, and don't read one whole.
  Save it and sum it up: `cargo run -q -p repo_checks --bin logsum -- <log>`
  prints the counts, the first failure, the tail and the repeated line shapes.
  `cargo nextest run` prints failing tests only (`.config/nextest.toml`).
  For a doc, read [docs/agent-index.md](docs/agent-index.md) for the line each
  section starts at and open that section, not the file.
- **After pushing**, `tools/post-push-tests.sh` runs everything the push can
  affect, heavy tests too. Available; not required of contributors.

## Definition of done

**Test your change before it reaches main.** There is no CI or PR test
runner (a test machine may come later), so this is on you: run the
affected tests for what you changed (Linux: `cargo run -q -p repo_checks
--bin affected -- --base main` lists them and the pre-commit hook runs
their fast tier; Windows: `.\scripts\affected-tests.ps1 -Base origin/main`),
and add a scenario test for new behaviour. A guideline, not enforced
beyond the hooks: say in the PR what you ran.

- New behaviour has a scenario test (or a unit test for pure logic); the
  affected crates' fast tier and `cargo test -p repo_checks` pass.
- Visible changes: a screenshot (before and after) in the PR, from a tool
  run, not the repo.
- Docs stay true in the same commit. A file's detail lives in its `//!`
  doc ("Known shortcuts" for shortcuts); after editing one run
  `cargo run -q -p repo_checks --bin docs_index` (a test fails when the
  generated `docs/modules.md`, `docs/tech-debt.md` or `docs/agent-index.md`
  is stale).
- After changing a dependency: `cargo hakari generate`.

## Game files from the asset store (no install)

Every load goes through Chimera's vfs, and a store of packed game files
(`chimera_assets`, docs/plans/active/asset-service.md) is one more mount
layer: under the install by default, or instead of it. On a machine with
the installs (and, for Combat Arms, the keys), record what a run reads and
pack it; the store is per user (`~/.cache/chimera/assets`,
`%LOCALAPPDATA%\chimera\assets`) and shared by every checkout and slot.

```
CHIMERA_VFS_LOG=/tmp/dust2.log cargo test -p cs_source --features dev --test it heavy::map_de_dust2
cargo run --features dev,tools --bin chimera_pack -- pack cs_source --set de_dust2 --from /tmp/dust2.log
cargo run -p chimera_assets --bin chimera_assets -- status    # also: verify, gc [--dry-run]
CHIMERA_ASSET_USE_INSTALL=0 cargo test -p cs_source --features dev --test it heavy::map_de_dust2
```

The last line runs with the install hidden, from the store alone (no
keys needed: Combat Arms' records are stored decrypted). Settings:
`[assets]` in `chimera.local.toml` (`store`, `use_install`, `builds`;
`chimera.local.example.toml`), `CHIMERA_ASSET_STORE` and
`CHIMERA_ASSET_USE_INSTALL` over it. Never copy a store, a manifest or a
vfs log into the repository. Fetching from the dev server comes with M2.

## Design principles

- **Parity first, options later.** By default a game plays exactly as the
  original (measured, specced). Our improvements are opt-in options (a
  cvar, a menu toggle), off by default.
- **Data-driven first.** Prefer run-time data (TOML: weapons, rules, HUD,
  minigames) plus hooks a game registers by name over hard-coded Rust.
  [docs/weapon-data.md](docs/weapon-data.md), `minigames/`.
- **ECS style** ([docs/DESIGN.md](docs/DESIGN.md), "ECS style"): components
  for facts, observers for reactions, messages for per-tick flows, systems
  with declared queries gated by run conditions. Whole-world access
  (`&mut World`, exclusive systems, callback registries) only where the job
  is the world (map loading, spawning, network apply, console). A game
  registers its hooks under its own id, never a single-slot resource
  another game would replace. `checks/tests/it/ecs.rs` and
  `determinism.rs` ratchet this.
- **Generalize for the second game.** When a second game needs a piece of
  the first, lift it into Chimera; games never depend on each other.

## Pitfalls (from experience)

- `pgrep -f "<pattern>"` in a wait loop matches the waiting shell itself
  (its command line holds the pattern): wait on a pid (`wait $pid`,
  `kill -0 $pid`) or on a file the process writes. `pkill -f` likewise
  kills your own shell: kill by pid.
- A run that ends with `+quit` is a normal session and writes the user's
  `config.cfg`. Use tool runs (`--screenshot`, `--views`, `--frames`,
  `--bench`), which never write it, or point `CHIMERA_CFG_DIR` at a
  scratch folder.
- Ports collide with other processes (a real CS:S on UDP 27015, another
  agent's remote protocol). Tool runs pick free ports; otherwise ask for
  port 0 or a random free one, and confirm the process you talk to by its
  pid (`ss -ltnp` / `ss -lunp`).
- Two `git commit`s at once on one index corrupt each other's staging:
  one commit at a time per checkout.
- Parallel agents on one machine (optional conveniences, Linux):
  `tools/agent-slot.sh acquire <branch>` gives a warm worktree whose
  `target/` survives between agents (`release` when done);
  `source tools/agent-env.sh && agent_run cargo ...` shares one jobserver
  and sccache so builds don't oversubscribe the box. OBSERVABILITY.md 3e.
- Windows clones made before `.gitattributes` may hold CRLF files, which
  breaks tests that compare text: `git rm -r --cached . && git reset --hard`.
- The first build takes minutes and tens of GB in `target/`; build one
  crate's tests (`-p`) instead of the workspace when you can.

## Tasks and workflows

- Ready-to-run task briefs: [docs/tasks/](docs/tasks/) (the shape:
  [.github/ISSUE_TEMPLATE/agent-task.md](.github/ISSUE_TEMPLATE/agent-task.md)).
  The full to-do list is [docs/backlog.md](docs/backlog.md); known
  shortcuts are [docs/tech-debt.md](docs/tech-debt.md).
- Workflows as skills in `.claude/skills/` (playtest, affected-tests,
  new-scenario-test, screenshot-and-compare, profile, review-pr,
  add-weapon-from-data). They are plain Markdown with exact commands, usable
  by any agent; improving them in a PR is welcome.
