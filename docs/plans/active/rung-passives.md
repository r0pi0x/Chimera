# Rung passives: worn and carried items on a gun game rung

Status: P0, P1, P2, P3 and P5 built (2026-11-24). The user's decisions
of 2026-11-24 (section 9) are in: HUD option A, on by default at 20 %,
up to two (the second at a quarter of the odds), only the character's own
game's items, utility items in. Parent plans:
[ultimate-gun-game.md](ultimate-gun-game.md) (the gun game),
[tf2-weapons.md](tf2-weapons.md) (the items), [ugg-ui.md](ugg-ui.md)
(the HUD). The idea is the user's (2026-11-24): "passive items on some of
the rungs, optionally ... like if you're a pyro, you might have this as a
passive secondary, or the same thing for the boots, like the rocket
jumping boots for soldiers ... and a way to show that you have that on
your character". UI mockup (private artifact, for the user's review):
"Rung Passives HUD", https://claude.ai/artifact/VS8inwM9Q9dfeFM6Fbikhz.

## Status

| Milestone | State | Summary |
|---|---|---|
| P0 the extra in Chimera | done | `RungExtras::passives`, `minigame::passives` (`PassiveTable`, `draw`, `wear`, `RungPassive`), `Ladder::is_passive` (the kill rule), the option (`passives = true` or a table; `on = false`), two host settings (`rules.rung_extras.passives.on`, `.odds`), carried in `NetMinigame::rung_extras` (`net73`); tests: chimera_sim `rung_passives` |
| P1 TF2's worn passives | done | data/rung_passives.toml (Mantreads, Ali Baba's Wee Booties, Bootlegger, Darwin's Danger Shield, Cozy Camper, Razorback; the gun slots per class and the guns' keys), `game::passives` (draw with the rung's class, `apply` through `tf2:worn`); Darwin's and the Cozy Camper made worn items (no longer `wearable`); Gunboats left out (their worn own-blast reduction isn't built); tests: tf2 `rung_passives` |
| P2 the HUD | done | option A, the chip tail (`chimera_client::rung_gear`: slanted tiles under the race chip, icon, name, the live key and verb, folding to icons after 5 s, hidden while the rank-up toast or the board is up); the Tab board's Gear column (`race_chip`, only on a ladder with gear; with the cursor free each icon is a kit item carrying the kit's item card, `chimera_ui::shell::Tip::item`); the kill feed's item icon in the accent text tone; the spectator plate's gear; icons: the game's own (TF2's kill icons from the item pack), else the kit's new glyphs (boots, shield, jetpack, chute, pack, jar, can, food, banner; watches the clock); `ui_scene passives [folded]`, `board passives [pick]`; tests: chimera_client `rung_gear`. The toast's "+ item" line not built |
| P3 keyed passives | done | data/rung_passives.toml: the shields and watches (`keys = ["attack2"]`), the B.A.S.E. Jumper (`["jump"]`, Soldier and Demoman), the carried Thermal Thruster (its slot its key); `[gun_keys]` adds the Dragon's Fury, Cow Mangler and Huntsman bases and an unlock reads its base's row (`items::base_in`); the items already worked carried (their parts read the holder's keys whatever is out); bots: Chimera's `core::BotFocus` (the brain's target, on every `Bot`) and `core::BotItems` (a set after the brains, before the rules and the switch), Combat Arms' brain writes the focus and leaves `RungPassive` and `Unselectable` weapons out of its guns, TF2's `items::bots` charges at a target within 600 units ahead, cloaks hurt and backs away (decloaks after 3 s with nobody in sight; the Dead Ringer arms on sight), opens the chute past a 550 units/s fall, and draws the thruster at a target past 900 units, launches and draws its gun back; tests: tf2 `rung_passives` (draw per rung, each item with the gun out, bots with a stand-in brain, leaving mid-charge and mid-cloak on `NetSim`), combat_arms `bots::the_brain_shares_its_target_and_leaves_passives_alone` |
| P4 worn on the body | not started | the items drawn on the third-person body (TF2's notes: "drawn on the legs" ...) |
| P5 active utility | done | `kind = "active"` rows under their own option (`utility`, on, the table's `[utility]` odds 0.2, one a rung): Jarate, Mad Milk, the Sandvich, Dalokohs Bar, Second Banana, Bonk, Crit-a-Cola, the three banners (not the Buffalo Steak Sandvich, melee-only for 19 s, nor the Gas Passer); never in a slot or on a key the gun or a drawn passive takes; their kills: the gun's, as any kill; bots: `items::bots` (`use_once`: a jar at a target in sight within 800 units, food hurt with nobody in sight, Bonk hurt and Crit-a-Cola on sight, a banner at full rage on sight); tests: chimera_sim and tf2 `rung_passives` (drawn per class and slot, used by a player and a bot, the client's eat predicted with no correction on `NetSim`) |
| P6 other games | not started | Combat Arms' gear perks first; others as they have something |

## 1. What it is

A **rung passive** is an item that is not the rung's weapon but rides
with it: worn (Mantreads, a shield, Razorback) or carried and switched
to (Thermal Thruster). It is a new kind of **rung extra**, beside the
drawn character, the fitted parts and the melee (docs/DESIGN.md, "Rung
extras, the same for everyone on a rung"), and follows their rules:

- **Drawn per rung, the same for everyone on it**, when the ladder is
  made, from the match's seeded generator (`ladder::RungExtrasHook::draw`).
  Not per player: the climb stays fair, a late joiner, a respawn and a
  network client get the same, and the board can show it from the rung
  alone.
- **Chimera names no game.** Chimera owns the field, the draw helper, the
  odds and options, the HUD and the board. Each game owns its table
  (which items, for which class, at what weight) and its `apply` (giving
  the item).
- **On by default** (the user's call, 2026-11-24, over the plan's first
  "off": parity first, but this is the gun game's own flair). The host
  turns them off; a rung gets one at the data's odds (TF2's 0.2), a
  second at a quarter of that (`next`). The setting carries the
  "Chimera" origin mark (ui-kit preferences).
- **Only the character's own game** (the user, 2026-11-24): a passive is
  drawn only from the game whose character the rung's players are (the
  gun's game: characters follow the gun; `RungGun::game`, and the ladder
  drops any other game's id). No Mantreads on a Combat Arms soldier.
  Cross-game passives (the user floated a "goomba stomp" for every game's
  characters) are a later idea, not built.
- **No new mode** (memory): this only adds to the existing gun game's
  rungs.

## 2. Data and odds

Chimera: `ladder::RungExtras` gains `passives: Vec<String>` (item weapon
ids, at most `max` of them, default 1). A game's `draw` fills it with
Chimera's helper `passives::draw(table, class, gun, odds, rng)`, so every
game draws the same way:

1. Roll the rung's odds (`odds`, default from the game's data). A miss:
   no passive.
2. Take the candidates for the rung's **drawn class** (TF2's
   `RungClass`; a game without classes uses `"*"`), minus those that
   conflict with the rung's gun (below).
3. Pick by weight; with `max` > 1, roll again for a second from a
   different `group` (one pair of boots, never two).

Per game data, e.g. `games/tf2/data/rung_passives.toml`:

```toml
# The gun game's rung passives (docs/plans/active/rung-passives.md).
odds = 0.25        # a rung's chance of one passive when the host has them on
max = 1

[[passive]]
item = "tf2:mantreads"     # a weapon id (data/items.toml)
classes = ["soldier"]
slot = "secondary"         # the TF2 loadout slot it takes
group = "feet"             # one per group on a rung
weight = 2
kind = "worn"              # worn | carried

[[passive]]
item = "tf2:thermal_thruster"
classes = ["pyro"]
slot = "secondary"
kind = "carried"           # switched to, like a weapon (its key: its slot)
weight = 1

[[passive]]
item = "tf2:chargin_targe"
classes = ["demoman"]
slot = "secondary"
group = "arm"
keys = ["attack2"]         # the keys it takes from any weapon
```

**Conflict rule** (keeps a legal loadout, parity first):

- The item's `slot` must differ from the rung gun's slot for the drawn
  class. A Soldier on the Shotgun (secondary) gets no boots; a Soldier
  on the Rocket Launcher may. A Demoman on the Stickybomb Launcher gets
  no shield.
- The item's `keys` must be free on the rung's gun: a shield's or a
  watch's attack2 never goes on a rung whose gun uses attack2 (the
  Flame Thrower's airblast, the Minigun's spin, the Sniper Rifle's zoom,
  the Stickybomb Launcher's detonation).
- Not on the melee band (the knife finale) in the first pass; a shield
  with the finale's knife is a later question for the user.

**Options**, as the other extras (`rung_extras = { passives = false }`,
`rung_extras = { passives = { odds = 0.5, max = 2 } }` in a definition's
`[rules]`). Two rows in `apps/party_kit/data/host_settings.toml`:

| Label | cvar | key | choices |
|---|---|---|---|
| Rung extras: passives | `mp_rung_extras_passives` | `rules.rung_extras.passives.on` | Default (on), On, Off |
| Rung extras: passive odds | `mp_rung_extras_passives_odds` | `rules.rung_extras.passives.odds` | Default, 10 %, 20 %, 50 %, Always |
| Rung extras: passives a rung | `mp_rung_extras_passives_max` | `rules.rung_extras.passives.max` | Default, 1, 2 |
| Rung extras: utility items | `mp_rung_extras_utility` | `rules.rung_extras.utility.on` | Default (on), On, Off |

Both `lobby = true`, with the Chimera origin mark and a help line saying
it is ours, not the game's. In the Options screen the odds row shows
only while passives are on (progressive disclosure).

**Kills with a passive** (a Mantreads or Thruster stomp, a shield bash)
climb as a kill with the kit's melee does today: they count, and they
never steal a rung (`demote_on = "melee_death"` is the melee weapons'
alone). One rule in Chimera; no per-item data.

## 3. Which TF2 items fit

From data/items.toml (`gun_game = false` or `wearable = true` today).

| Item | Class | Kind | Fits | Notes |
|---|---|---|---|---|
| Mantreads | Soldier | worn | P1 | stomp, push resistance; air control part unmeasured |
| Gunboats | Soldier | worn | P1 | not in data yet: one attribute, batch-1 cheap |
| B.A.S.E. Jumper | Soldier, Demoman | worn, keyed (jump in the air) | P3 | jump is never a gun's key, so no conflict |
| Ali Baba's Wee Booties, Bootlegger | Demoman | worn | P1 | health and speed alone; turn control only with a shield (both drawn: `max = 2`) |
| Chargin' Targe, Splendid Screen, Tide Turner | Demoman | worn, keyed (attack2) | P3 | only with an attack2-free gun (Grenade Launcher, Loose Cannon ...) |
| Thermal Thruster | Pyro | carried | P3 | the user's "passive secondary"; switched to and fired |
| Razorback | Sniper | worn | P1 | blocks one backstab; useful only when a Spy rung exists |
| Darwin's Danger Shield, Cozy Camper | Sniper | worn | P1 | numbers only; regen and flinch rules still open |
| Invis Watch, Cloak and Dagger, Dead Ringer | Spy | worn, keyed (attack2) | P3 | the Revolver and knife leave attack2 free |
| Buff Banner, Battalion's Backup, Concheror | Soldier | carried, active | P5 | rage buffs the wearer too, so solo works; still a use-it item |
| Gas Passer | Pyro | carried, active | P5 | a throwable |
| Jarate, Mad Milk | Sniper, Scout | carried, active | P5 | throwables with a recharge |
| Sandvich family, Bonk, Crit-a-Cola | Heavy, Scout | carried, active | P5 | consumables |
| Medigun family, PDAs, Sapper, Disguise Kit, Wrangler | | | never | team or building items (tf2-weapons.md 5) |

**Jars, food, drinks and banners are active**: a use with a cooldown,
more like a second weapon than something you wear. They are their own
extra, `rung_extras = { utility = ... }`, with its own toggle and odds
(P5, built: "special flair additions to a TF2 kit", the user). A kill
they set up is the rung's gun's and counts as any kill; the jar's or
banner's effect just helps. The Gas Passer (it deals damage of its own)
and the Buffalo Steak Sandvich (melee only for 19 s) stay out.

## 4. Other games

The hook shape is game-neutral; each game adds a table when it has
something worth wearing.

- **Combat Arms**: the gear perks (specs/combat_arms/gear_perks.md 13-14:
  reload, sprint, stamina perks) and goggles and masks (flash
  protection). Not vests: a rung's body keeps the baseline vest (the
  "changes looks, not the odds" rule), so armour stays off the table.
  Backpacks only if a rung has grenades.
- **CS:S**: nothing that changes play in a free-for-all gun game (the
  defuse kit, night vision); none.
- **HL2:DM, DoD:S**: nothing worn; none.
- **Halo: CE**: the overshield and active camouflage are timed power-ups:
  the top-left effects chip, not passives.
- Chimera's own originals may add passives of their own later.

## 5. Networking and prediction

- **The draw** is the host's, with the ladder; `RungExtras::passives`
  rides in `NetMinigame::rung_extras` (a protocol bump, as every
  `NetMinigame` change). Late joiners and clients have it from the match
  state; nothing per player crosses the wire for the HUD or the board.
- **The item** is given by the game's `apply` on the host (as Combat
  Arms' parts): a worn item becomes an `Unselectable` weapon in the
  inventory (`tf2:worn`, as the shields today), a carried one a normal
  one in its slot. Inventory already replicates; the items' parts
  already predict where they move the player (the shield's charge, the
  thruster, the parachute, the Mantreads' air control).
- **Arriving and leaving**: `OnRung` fires on clients too
  (`ladder::follow_rungs`), so the HUD and any client-side look follow
  without new messages. Leaving a rung mid-charge or mid-glide ends the
  item's state the same tick on both sides (the item's part is removed
  with the item; its `Discard` observer ends the charge); a test covers
  it.
- **Bots** (P3, built): a keyed item's bot hook (the game's, in
  Chimera's `core::BotItems`, the server's) presses its key after the
  brain wrote its own, from the brain's `core::BotFocus` (its target and
  whether it sees it): TF2's `items::bots` (charge at a target close
  ahead, cloak when hurt and back away, chute on a long fall, thruster to
  close distance). Worn items need nothing; a brain never draws a
  passive. Leaving a rung mid-charge or mid-cloak: the item is taken
  with the rung, so the server's state ends that tick; the client's
  prediction has it until the change arrives, then its history from
  that tick has none (one correction at most, then the same speeds).

## 6. The HUD (mockup)

The mockup gives three options on the Signal HUD, all on kit pieces (the
plate tone, the 12° slant, the key cap, the badge):

- **A, chip tail** (recommended): small slanted tiles under the race
  chip, one per passive; on arriving, icon, name and the live key with
  its verb ("2 LAUNCH"), folding to icons only after 5 s. Top centre is
  the Signal look's own area, so it never covers a game's HUD.
- **B, gear rail**: a row on the rung hint's line, bottom centre, with
  a recharge bar per keyed item; dims after the arrival. Repeats meters
  TF2 already draws.
- **C, badge and toast**: only a badge on the chip in play (first icon,
  "+N"); the rank-up toast's line 2 gains "+ Mantreads"; the rest on Tab.

For all three: the Tab board gets a **Gear** column after the gun (the
item icons, "—" for none; the same for everyone on a rung), hovering or
focusing an icon opens a **hover card** (name, game, class, worn or
carried, one line of what it does, the key, "Rung extra" and the Chimera
mark); a kill by a passive puts the item's icon in the kill feed in the
gun's place; the spectator plate lists the watched player's gear.
Icons: the source game's own item icons when the install has them
(`WeaponIcons`, as gun icons), else a fallback glyph by kind (boots,
shield, jetpack, chute, pack) in the kit's icon set. The one-line effect
is data in the game's table (`summary = "..."`), never generated.

## 7. Tests

- chimera_sim `rung_extras`: passives drawn from the seed are the same
  on every run; off by default; odds 0 never, 1 always; `max` and
  `group` respected; the conflict rule (slot, keys) with a test table;
  none on the melee band; the definition's and the host's overrides.
- tf2 `tests/it/rung_passives.rs`: a Soldier Rocket Launcher rung may
  draw Mantreads, a Soldier Shotgun rung never; a Pyro Flame Thrower
  rung never gets a shield; `apply` gives a worn item `Unselectable` and
  removes it on leaving; a stomp kill climbs and does not demote; a
  charge ends when the rung changes mid-charge.
- chimera_net (`NetSim`): a client and a late joiner see the same
  passives; the client predicts the charge or the thruster with no
  correction past tolerance.
- chimera_client: the chip tail (or the chosen option) shows and folds
  under the Signal theme, nothing without passives; the board's Gear
  column and the hover card; `ui_scene passives` stages them;
  `kit_conformance` passes; the HUD keeps clear of game HUDs.
- party_kit: the host settings' rows apply as overrides from the next
  match.

## 8. Milestones

1. **P0, Chimera**: the field, the helper, the options and host
   settings, the net field and bump; tests with a test game's table.
2. **P1, TF2 worn**: data/rung_passives.toml with the worn items that
   need no key (boots, Darwin's, Cozy Camper, Razorback, Gunboats as
   data), `apply` through `tf2:worn`.
3. **P2, HUD**: the option the user picks, the board column, the hover
   card, the toast line, the kill feed icon, the spectator plate.
4. **P3, keyed TF2 items**: shields, watches, B.A.S.E. Jumper, Thermal
   Thruster, with the key-conflict rule and bot hooks.
5. **P4, worn on the body**: the items drawn on the third-person body
   (TF2's item models from the pack).
6. **P5, active utility** (after the user's call): jars, food, drinks,
   banners, Gas Passer as `rung_extras.utility`.
7. **P6, other games**: Combat Arms' perks first.

## Progress

- 2026-11-24, P0 and P1: as the status table says. Choices made on the
  way: the on/off key is a table's `on` (`passives = { on = true }`) so
  the two host settings each set one key (`overrides` would replace a
  `true` by the odds' table); a tuned table is on, so the odds row set
  alone turns passives on (no progressive disclosure yet). The conflict
  rule's data is the game's table: `slot_names` (a weapon's data slot by
  name), `[slots]` (the Shotgun: the Engineer's primary, the others'
  secondary) and `[gun_keys]` (the Flame Thrower, Minigun, Sniper Rifle
  and Stickybomb Launcher use attack2). A passive's kill: `Ladder::
  is_passive` exempts it from `demote_on = "melee_death"` (a stomp is
  generic damage and never demoted anyway; a shield bash will be melee).
  Drawing rolls only while on, so the ladders without passives draw as
  before; with them on, the later rungs' other draws (a Shotgun's class)
  follow a different sequence.

- 2026-11-24, P2 and P5 and the user's decisions: defaults on, 0.2,
  `max = 2`, `next = 0.25` (Chimera's and TF2's table); the draw rolls on
  a fork of the ladder's generator (salted, never advancing it), so
  turning passives on or off changes no other draw; the own-game rule in
  the helper and the ladder; the HUD (P2 row); utility items (P5 row).
  The chip tail waits for the rank-up toast (which shows under the chip
  on arrival) and its 5 s fold counts from its first showing on the rung.
  The kill feed's passive icon uses the accent text tone, not the
  mockup's amber (the kit keeps amber for network warnings only).

## 9. The user's answers (2026-11-24)

1. HUD option A, with the shared pieces (Gear column, hover card, kill
   feed icon, spectator plate), the source game's own icons where there
   are any: built.
2. On by default, 20 %: built.
3. Up to two, the second rarer (a quarter of the odds, data): built.
4. Only the character's own game's items; cross-game ("goomba stomp")
   later. P5 in: jars, food, drinks, banners; a kill they set up is the
   gun's. Built.

Open: cross-game passives; the toast's "+ item" line; P4 (worn on the
body); P6 (other games).
