# Adding a game to Chimera

For friends porting a game with their own AI coding assistant. The short
version: **one command makes a game that already runs in the sandbox;
then two kinds of sessions replace its stand-ins one part at a time.**
The checklist behind it is [new-game.md](new-game.md); what the engine
already does for you is [engine-capabilities.md](engine-capabilities.md).

## Zero to a game in the sandbox

```
git clone https://github.com/r0pi0x/Chimera && cd Chimera
git config core.hooksPath .githooks          # the checks run on commit
tools/new-game blood_gulch "Blood Gulch Wars" --short BGW
                                             # Windows: tools\new-game.ps1 (same arguments)
cargo test -p blood_gulch --features dev     # the skeleton's tests pass
cargo test -p repo_checks                    # the shape and the hard rules
export CHIMERA_BLOOD_GULCH_ROOT=~/games/bgw  # your install (any folder at first);
                                             # PowerShell: $env:CHIMERA_BLOOD_GULCH_ROOT="..."
cargo run --features dev --bin ultimate_gun_game -- +sandbox +rung blood_gulch:weapon_standin
```

The last line opens the Ultimate Gun Game's sandbox on the greybox with
your game's stand-in pistol in hand: you are a `blood_gulch` player on
`blood_gulch:movement`. A game's guns are offered only when its install
is found (the sandbox's "mount doctor" line says so), hence the
variable. `B` changes gun, `T` the bench (targets),
the console (`~`) takes `rung <gun id>`.

`tools/new-game` made:

| Where | What |
|---|---|
| `games/<id>/` | the game crate: `INFO` (what the shell knows of it), `BUNDLE` (the game for a party app), `MAP_LOADER` (no maps yet), the mount, a stand-in movement (`data/movement.toml`) and gun (`data/weapons/`), `harness::<Game>Sim`, a fast test (registers, a body spawns and walks) and a heavy one (your install mounts) |
| `games/<id>/ui/` | its front end crate (`party::CLIENT`, `with_hud`): no HUD yet |
| `specs/<id>/` | a README and spec templates: install, movement, weapons |
| `docs/plans/active/<id>.md` | the plan: milestones, status, log |
| `Cargo.toml`, `apps/party_roster` | the crates in the workspace; the game in the party's `GUESTS` and `CLIENTS` |
| `chimera.local.example.toml` | where your install is found: `[games.<id>] path` (copy it into your own `chimera.local.toml`, never committed) or `CHIMERA_<ID>_ROOT` |

## The rule that shapes everything: two kinds of sessions

Chimera reimplements games; it never copies them. So the work is split
between two kinds of AI sessions that never mix (CLAUDE.md, "Hard
rules"; [specs/README.md](../specs/README.md)):

| | Spec session | Implementation session |
|---|---|---|
| May read | the original game: play it, measure it, read your own install's files (data, scripts, formats), public wikis and format docs | `specs/`, this repository's code, THIRD_PARTY.md's sources |
| Never reads | (no limit on what it studies, but nothing leaked) | the original's code in any form, decompiled output, the SDK, the reference material the spec session used |
| Writes | **only** `specs/<id>/*.md`: behaviour in our own words (constants, math, per-tick order, test cases) | code, data, tests, docs; never `specs/` |
| Brief | [tasks/game-spec-session.md](tasks/game-spec-session.md) | [tasks/game-implementation-session.md](tasks/game-implementation-session.md) |

Start a fresh session for each kind (a new chat, not the same one), and
paste the brief. If an implementation session finds a spec unclear, it
writes the question into the plan's log and stops; a spec session
answers it.

Never, in either: leaked or decompiled code, code from an SDK (the
Source SDK included), code from a source not in
[THIRD_PARTY.md](../THIRD_PARTY.md), encryption keys, or any file from
the game (models, textures, sounds, maps, even "just a small one" as a
test fixture). Tests read your install and pass with a note without it.

## A milestone, as an example

Movement, for the game above:

1. **Spec session**: "write specs/blood_gulch/movement.md". It measures
   the running game (run speed in units/s over 2 s, jump height, gravity
   from a fall), fills in the template's tables and test cases, commits
   only that file.
2. **Implementation session**: "implement specs/blood_gulch/movement.md".
   It turns the spec's test cases into scenario tests in
   `games/blood_gulch/tests/it/` first, then either puts the numbers in
   `data/movement.toml` (enough while the placeholder's feel is close) or
   writes the game's own movement (a marker component,
   `register_movement`, its numbers as data; new-game.md step 4).
3. Both update `docs/plans/active/blood_gulch.md` (status table, log).

Guns are the same: a spec of each gun's numbers, then one TOML document
per gun in `data/weapons/` (the `add-weapon-from-data` skill,
[weapon-data.md](weapon-data.md)). Most guns need no Rust.

## Testing

| What | Command |
|---|---|
| your game's fast tests | `cargo test -p <id> --features dev -- --skip heavy::` |
| against your install (`CHIMERA_<ID>_ROOT` set) | `cargo test -p <id> --features dev --test it heavy::` |
| the hard rules and the crate's shape | `cargo test -p repo_checks` |
| everything your change can affect | `.githooks/pre-commit --worktree` (the `affected-tests` skill) |
| play it | `cargo run --features dev --bin ultimate_gun_game -- +sandbox +rung <id>:<gun>` |
| see it without a display (screenshots) | add `--offscreen 1600x900 --frames 300 --dump <dir>` ([OBSERVABILITY.md](OBSERVABILITY.md), section 2) |
| a new behaviour | a scenario test on `harness::<Game>Sim` (the `new-scenario-test` skill) |

Every new behaviour gets a scenario test; every file format the game
reads gets a heavy test on the real install.

## What the checks enforce

`cargo test -p repo_checks` (and the commit hook) refuses a change that
breaks a hard rule, and says how to fix it:

- **Layering** (`layers_follow_paths`, `games_share_one_shape`): a game
  depends on Chimera and its loader family only, never on another game
  or an app; every game has the same seams (what `tools/new-game` made).
- **Assets** (`no_game_assets_tracked`, `no_keys_tracked`): no game
  files, no key-like hex strings.
- **Specs** (`specs_contain_no_code_blocks`): specs are prose, tables
  and ```` ```math ````, never code.
- **Third party** (`third_party_headers_name_listed_sources`): a file
  that says it was adapted from somewhere names a THIRD_PARTY.md source.
- Windows-safe paths, docs links, generated docs fresh
  (`cargo run -q -p repo_checks --bin docs_index` regenerates them).

## How players get the game's files

Never through the repository. A player's game reads its files at run
time from:

1. **their own install**: found by `CHIMERA_<ID>_ROOT`, `[games.<id>]
   path` in their `chimera.local.toml`, or Steam's libraries when
   `INFO.steam` names the app; or
2. **the asset service** a host runs from their own install
   (`chimera_pack` packs and converts; [assets-for-developers.md](assets-for-developers.md)).
   Access is gated by the hub's owner (an invite); don't publish a
   service or a pack yourself. Adding your game to `chimera_pack` is a
   later milestone the owner reviews.

## Needing something from the engine

Look in [engine-capabilities.md](engine-capabilities.md) first: most
things a shooter needs exist. If it's missing:

- **Build it in your game first** (`games/<id>/src/game/`). That is
  allowed and expected.
- When a **second game** needs the same thing, it moves into Chimera
  (or the loader family's crate), and both games use it: never one game
  depending on another (DESIGN.md, "Rule of three").
- A change to Chimera that other games feel (a new slot, a new field
  on `GameInfo` or `GameBundle`, a shared default) gets an issue or a
  short plan in `docs/plans/active/` first, saying which games need it.

## Pull request checklist

- [ ] `.githooks/pre-commit --worktree` passes (affected tests,
      repo_checks).
- [ ] Spec and implementation in separate commits from separate
      sessions; no file outside `specs/` in a spec commit.
- [ ] No game files, extracted data, keys, machine paths or personal
      names anywhere (commits included).
- [ ] New behaviour has a scenario test; new formats a heavy test.
- [ ] The plan's status and log are updated; docs changed with the code
      they describe.
- [ ] Reviewed with the `review-pr` skill (`.claude/skills/review-pr`):
      ask your assistant to "review this branch with the review-pr skill".
