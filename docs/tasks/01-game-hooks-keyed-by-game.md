# Key the single-slot net and core hooks by game

**Done 2026-10-30** (docs/plans/active/ecs-cleanup.md, step 2: what
landed and the behaviour differences).

## Context

`JoinRules`, `Holds`, `BotHooks` (chimera_net), `NewGame` and
`LogSubjects` (chimera_core) are single resources: the last game plugin to
insert one wins. An app with two games' parts (Lucker Party, the mashup,
Ultimate Gun Game) gets whichever plugin ran last, and a third game
(HL2DM, TF2) can't add its own without replacing CS:S's. Step 2 of
[docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md); the
rule is DESIGN.md "ECS style" ("never a single-slot hook resource").

## Files

- `chimera/net/src/net/parts.rs` (`JoinRules`, `BotHooks`, `Holds`),
  `chimera/net/src/net/server.rs`, `chimera/net/src/net/predict.rs`
- `chimera/core/src/core.rs` (`NewGame`), `chimera/core/src/game_log.rs`
  (`LogSubjects`), `chimera_core::gameplay::Gameplay` (`rules`, `bots`)
- each game's `net.rs` and `rules.rs` (`games/cs_source`, `games/combat_arms`)
- Guards: `apps/party_roster/tests/party/listen_server.rs`, `net_party.rs`, both
  games' `net_*` tests, `apps/mashup_demo/tests/it/two_games.rs`

## Definition of done

- [x] A `GameHooks<T>` resource (game id to `T`) and
      `app.game_hook::<T>(game, hooks)`; readers take the entry of
      `Gameplay::rules` (`BotHooks`: `Gameplay::bots`), the default when none.
- [x] The five hooks use it; no plugin inserts them directly.
- [x] A scenario test in `chimera/net/tests/it/`: two games register
      different `JoinRules`; switching the gameplay switches which runs.
- [x] The guards above and `cargo test -p repo_checks` pass; the plan's
      step 2 is marked done; the single-slot hooks leave
      the "Known shortcuts" of `chimera_core::gameplay`'s `//!` doc (then `docs_index`).

## Constraints

- **Behaviour flag:** in each two-game app, check the hooks chosen by its
  gameplay are those the last plugin gave before (the plan says most
  likely; confirm with the app's tests).
- Hooks stay plain `fn` data keyed by id (data-driven, DESIGN.md).

## Out of bounds

The weapon seam (`ViewAnimRules`, `LaunchHook`: step 3) and mode hooks.

## Size and labels

- Size: M
- Area: area:net, area:engine
- Needs install: no (tests needing CS:S files skip)
