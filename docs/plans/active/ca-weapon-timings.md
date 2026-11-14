# Plan: Combat Arms weapons' own fire, reload and draw timings

Status: 2026-10-29, parts 1 to 3 implemented (below); backlog
item "Combat Arms guns' own sounds, scopes and models: audit", fix 3.

Today every install copy fires, reloads and draws at its template's
numbers (every rifle copy at the M4A1's 105 ms, every sniper copy at the
L96A1's 1.4 s bolt) while its view model plays its own animations.

## What the specs say decides the times

- **Fire cycle** = the view model's `fire_0` length / $r_{fire}$,
  $r_{fire} = 1 + $ the record's `FireAniRate` $+$ the parts' fire bonuses
  (weapon_firing.md "Trigger and fire cycle"; view_models.md 7,
  Corrections 6). A long `fire_0` *is* the pump or bolt
  (weapon_classes.md "Fire cycle"). Any rate above 2.0 plays at 1.0.
- **Reload** = `reload` / $r_{reload}$, $r_{reload} = 1 +$ parts $+$ the
  vest's `ReloadAniRate` $+$ set bonus (no weapon-record field). A shell
  reloader (`ReloadPerBullet` 1) plays `PreReload`, `reload` per shell,
  `PostReload` (weapon_classes.md 5.5).
- **Draw** = `select` / $r_{draw}$, $r_{draw} = 1 +$ parts $+$ the vest's
  `SelectAniRate` $+$ set bonus (inventory.md 6.4).
- `FireDelay` is only the server's spacing check (already mapped, own on
  every copy); the client never reads it (view_models.md Q9).
- Each animation is found by name, first match, case ignored, in the
  model's combined list: its own animations, then its children's (the
  `ANI_*` child files; `model::load`, animation.md).

So the lengths live in the **model** (the `PVModelNormal` file and its
children); the rates in the **record** (`FireAniRate`), the parts and the
vest. Our documents' numbers (`trigger.cycle`, `magazine.reload`,
`magazine.shells`, `draw`) are those lengths at rate 1 for our weapons.

## How, without reading ~2,000 models at merge time

- The merge reads no model. It writes an empty `combat_arms:timing`
  table on each **copy** that is a gun (a trigger and a view model; not a
  knife or a grenade, whose hooks time them). Our documents get none:
  their numbers are the spec's and stay.
- The `combat_arms:timing` hook (`weapons::timing`) runs when a weapon
  entity is built, on whichever side builds it: the server's `give`, the
  client's copy of its inventory (`sync`), a remote player's held gun. It
  reads the weapon's `[models] view` and its children through the
  install mount (`world::mount`, headless: no render needed, the server
  already mounts the install for its maps), takes `fire_0`, `reload`,
  `PreReload`, `PostReload`, `select`, and caches them per model path in
  the world (`ViewTimes`, a resource: the server's and each client's;
  failures too). A missing or 0 ms animation, or a model
  that doesn't read, keeps the template's number.
- Rates on top: the cycle is divided by `fire_animation_rate`
  (`CaItem::fire_rate` = `FireAniRate`); with an attachments table the
  lengths become the `Stock` the parts' rates apply to (`apply_fitted`,
  `apply_draw`). The hook runs after the attachments, damage and
  inventory hooks (hooks build in name order), so it also replaces the
  server's reload window (`CaDamage::reload_third_person`) when that was
  the template's reload.
- `FireAniRate` everywhere: the merge makes `combat_arms:inventory` where
  the template has none (83 copies, and our documents without one: the
  MP5A4, M60, M92FS, R870, L96A1, Dragunov and the launchers), so the
  record's other item fields land with it.

## Client and server agreement

Shots are server-authoritative and the client predicts its own weapon
with its own `Trigger`, `Magazine` and `ShellReload` (not sent: only clip
and reserve are). Both sides build the weapon from the same document and
read the same install file, so they get the same numbers before the
first shot; the scenario test checks prediction stays error-free.
If the client can't read the model (no install, or no V2 key for the
rifle, SMG and sniper archives), or reads another version, the server's
numbers win: the hook leaves what it settled on the weapon
(`CaTimingValues`), the server sends it once per weapon id per client
(`weapon::params`, `net::weapon_params`) and the client's weapons of
that id take it over their own (open question 1, answered).

## Cost

One model read (about 2 ms, at most ~18 ms) when a weapon is first
built, once per model per world: in the gun game, the next weapon on a
kill. See "Measurements".

## Measurements

2026-10-29, this box, debug build, both keys (`cargo test -p
combat_arms --features dev --test it heavy::timings -- --nocapture`):

- The gun game's pool: 1,989 weapons with a `class`; 1,974 copies get
  the timing table (the rest are ours).
- 1,971 now fire, reload and draw on their own model's lengths; before,
  591 had the right fire cycle by chance (their template's equals
  theirs). 3 keep the template's: 2 whose model doesn't read
  (`abnormal_fortress_fire_target` and `_area`, the minigun launcher
  model) and 1 whose model has no `fire_0` (`groza_tropical`).
- 476 distinct view models (variants share them): 0.91 s to read all,
  1.9 ms a model on average, the slowest 17.7 ms. Done at merge time
  that would add about a second to every start; done when a weapon is
  first built it is a few milliseconds once per model.

## Parts

1. (Done 2026-10-29.) The hook, the merge's table, `FireAniRate`
   everywhere, tests: `tests/it/weapon_timings.rs` (copies with their
   view models' lengths written in the test fire, reload and draw on
   them, on the server and predicted; ours unchanged), the heavy
   `heavy::timings` over the install's pool.
2. (Done 2026-10-29.) The vest's `ReloadAniRate` / `SelectAniRate` and
   the sets' rates (effect kinds 10, 11, 12, player-wide and per weapon)
   in the weapon's own cycle, reload (a shell reload's three parts too)
   and draw: one sum with the parts' (attachments.md "Animation rates":
   $r_{reload} = 1 + \sum_{socket}$ `Reloadanirate` $+ r_{model} + r_{set}$,
   capped as the rest). `gear::CaGearRates` on every weapon with an
   attachments table, derived each predicted tick from its owner's look
   (`Selection::model`: the vest) and set tiers (`CaGearEffects`), on the
   server and the client alike from the replicated look and loadout;
   `attachments::apply_fitted` / `apply_draw` add it, the view model
   plays at the same rates. The gear records now go to a client whose
   own differ (`data::served::GEAR`, `GearData::to_doc`, net34), so both
   derive the same rates and movement penalties. Not pushed per weapon:
   derived from replicated data. Test:
   `weapon_timings::a_vests_rates_shorten_the_reload_and_draw_on_both_sides`
   (the M4A1's 2.3 s reload in 1.84 s and 1.2 s draw in 0.8 s on both
   sides, a client on our gear records takes the server's, 0 prediction
   errors over the reload).
3. (Done 2026-10-29.) The rest of the fire sequence from the model
   (`weapons::timing`, `AnimTimes` now also reads `prefire`'s presence
   and length, `postfire`, `alt_fire_0` and the `FIRE_KEY` /
   `ALT_FIRE_KEY` times):
   - **Post-fire**: Chimera's `Trigger::postfire` (data
     `trigger.postfire`, `WeaponState::postfire_end`): after a pull's last
     cycle with the trigger let go (or the clip empty) the closing part
     plays in the firing state; a semi-automatic's press cuts it short and
     fires on that update, an automatic waits for its end and fires on the
     update that sees it over (view_models.md 7 and Corrections 1); the
     key's and the automatic reloads wait for it ("Reload after firing").
     A copy's is its model's `postfire` / the fire rate (none without
     one; `Stock::postfire` under the parts' rates).
   - **Pre-fire's presence**: a copy of a class that plays it (the
     template's latency 3) whose model has no `prefire` gets latency 2.
   - **Spinning guns**: the merge writes `combat_arms:spin` on copies with
     `AccumPreFire` (their template, the M60, has none); the hook fills
     its spin-up (`prefire`), spin-down (`postfire`) and loop (`fire_0`),
     the cycle the loop over its keys before the end (0.17 / 5 = 34 ms),
     latency 1, no trigger post-fire.
   - **Flame guns**: `CaFlame`'s pre-fire, loop, key count and mean key
     spacing (the trigger's cycle), post-fire.
   - **Knives**: the merge now writes the timing table on knife copies;
     slash `fire_0` with its `FIRE_KEY`s, stab `alt_fire_0` with its
     `ALT_FIRE_KEY`s, post-fire, played at 1 + `FireAniRate`.
   All of it is in the pushed `CaTimingValues` (net35). Tests
   (`tests/it/weapon_timings.rs`): `a_semi_automatic_copy_takes_its_postfire`
   (a press 250 ms after a shot fires on that update, a reload waits the
   300 ms post-fire, latency 2 without a prefire),
   `a_minigun_copy_spins_on_its_own_model` (first round at the 1 s
   spin-up, 40 ms a round), `a_knife_copys_swings_come_from_its_model`,
   `a_client_without_the_models_takes_the_servers_sequence`; the heavy
   `heavy::timings` over every timed copy.

Measured 2026-10-29 (`heavy::timings`, both keys): 2,160 copies with a
timing table (1,989 in the pool), 2,152 on their own model's numbers
(2 models unread, 6 without `fire_0`: the emote "weapons" and the
Groza); of 1,962 guns 96 have a post-fire (92 semi-automatic), 3 lose
the pre-fire's update, 32 have a pre-fire of non-zero length (question
5); 10 spinning, 6 flame, 174 knives. The gear records as a document:
513 KB (ours alone 22 KB).

## Open questions

1. Answered 2026-10-29 (user: "help the clients as much as possible
   by pushing from the server where there are desyncs"): the server
   sends each weapon's settled timing once per weapon id per client
   (`weapon::params` codec `combat_arms:timing`, `WeaponParams`, net32);
   a client's own read never overrides it. Tests:
   `weapon_timings::a_client_without_the_models_predicts_on_the_servers_timing`,
   `a_client_with_its_own_models_takes_the_servers_timing`.
2. The third-person reload window (weapon_classes.md, server reload
   bookkeeping) is the third-person reload animation's length per stance;
   we use the first-person `reload` for it, as before.
3. Whether to read ahead (a background read of the next rung's model in
   the gun game) if a hitch shows on slow disks.
4. The gear's rates reach only weapons with an attachments table (every
   gun): a knife's, grenade's or special weapon's draw keeps its own
   length, though inventory.md 6.4 adds the model's select bonus to
   every draw. The view model plays those at the vest's rates as before.
   **Answered (2026-10-30):** the model's and sets' select bonus applies
   to every draw, knives, grenades and specials included (inventory.md
   Corrections (2026-10-30) 2). **Done (2026-10-30):** every weapon's
   draw takes the item's and gear's select terms (`inventory::DrawStock`,
   `attachments::apply_draw`; test inventory t51).
5. A pre-fire of non-zero length on a gun that doesn't spin (32 copies):
   weapon_firing.md says the first round waits for the pre-fire to end;
   we count it as one update (latency 3), as for a 0 ms one. Taking its
   length needs a timed pre-fire in the trigger (the spin's mechanism
   without the reversal): left until a spec case or a playtest shows it.
   **Answered (2026-10-30):** the first round waits for the whole
   pre-fire (weapon_classes.md Corrections (2026-10-30) 1, case 44).
   **Done (2026-10-30):** the latency counts it at the simulation's tick
   (`timing::prefire_latency`; test weapon_timings t43_t44).
6. Pistols' pre-fire: view_models.md 7 and C2 say pistols and knives
   never play `prefire`; weapon_firing.md and weapon_classes.md case 40
   count it for pistols (M92FS: round on update 3). The data keeps the
   M92FS's latency 3 (a pistol copy whose model has none: 2). A knife
   copy keeps its template's 1 ms pre-fire (inventory.md 8.2 against
   view_models.md C1). Needs a spec decision.
   **Answered (2026-10-30):** view_models.md is right: secondary and
   melee classes never play `prefire` (M92FS round on update 2), knives
   swing with no pre-fire (weapon_classes.md, weapon_firing.md and
   inventory.md Corrections (2026-10-30)). **Done (2026-10-30):** the
   M92FS's latency 2, the knife's swing on the update after the press is
   armed (tests inventory t37_t50, weapon_corrections t64, weapon_classes
   t27).
7. Our documents keep their spec numbers and no post-fire: the M92FS's
   150 ms (weapon_classes.md 4) isn't in beretta_m92fs.toml yet, so ours
   doesn't hold a reload after a shot. Adding `trigger.postfire = 0.15`
   is a one-line change once its tests are checked.
   **Done (2026-11-13):** beretta_m92fs.toml's `trigger.postfire = 0.15`
   (test weapon_timings `our_m92fs_holds_a_reload_for_its_postfire`;
   weapon_classes t27's press at 250 ms now cuts the post-fire short and
   fires on its update, as case 27 says).
8. The spin's and the flame's lengths play at rate 1 (no `FireAniRate`
   or parts on them; every stock value found is 0); the semi-automatic
   cancel of a primary with a `prefire` (view_models.md Corrections 2:
   through `prefire`, one update more) fires at once as a pistol's.
