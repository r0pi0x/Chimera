# Half-Life 2: Deathmatch: the suit (health, armour, aux power, breath, flashlight, zoom, pickups, chargers) (H2)

Source basis: the public Source SDK 2013 multiplayer game code (the shared
player and its damage intake, the HL2 player and its suit-power devices,
the HL2:DM player, the multiplayer rules, the HL2 items and wall chargers,
the shared +use search, the client's HL2:DM player and suit HUD), read in
place in a separate spec session; the game's own files (game build
24600434, dedicated server build 24600426): `cfg/skill.cfg`,
`cfg/skill_manifest.cfg`, `cfg/valve.rc` (in `hl2mp_pak.vpk`), the sound
scripts, and the entity lumps of the 8 stock maps, read for names,
numbers and counts only. No game was launched and no server was started.
Status: draft

## Summary

Every HL2:DM player wears the HEV suit from spawn: 100 health, 0 armour
and 100 aux power. Armour takes 80 % of most damage at one armour point per
point absorbed (two against blasts) and whole armour points are lost, while
health keeps the fractions of damage in a running remainder, so armoured
players lose health in uneven steps (1, 2, 1, 2 for pistol hits). Aux power
is a 0–100 pool that sprinting (25/s) and breathing under water (6.7/s)
draw on; it recharges at 12.5/s half a second after the last device stops,
and switching a device off costs a tenth of its rate at once. While any
aux power is left the player breathes under water; after that the air lasts
7 s and drowning takes 10 a second, given back 10 every 2 s after
surfacing. The flashlight costs nothing in HL2:DM and is off unless
`mp_flashlight` is set. Health kits (25), vials (10) and batteries (15) are
walked over; wall chargers are held with `+use` and give one point every 7
ticks.

This spec continues [source_deltas.md](source_deltas.md) (H0: its
"Behavior" section, 1.1, 1.2 and the cvar table) and
[specs/cs_source/movement.md](../cs_source/movement.md) (drowning) and
[specs/cs_source/fall_damage.md](../cs_source/fall_damage.md). Where H0 is
wrong this spec says so ("Corrections to H0").

## Units and conventions

- Source units, Z up; degrees; seconds; server tick 0.015 s (66.67 Hz).
- "Tick n" counts player commands at one per tick; a duration d in seconds
  becomes "the first tick at or after d" unless the rule is a strict
  comparison, which is stated (several timers here wait for the first tick
  strictly **after** d, which is one tick later when d is a whole number of
  ticks and otherwise the same).
- Health and armour are whole numbers; aux power is a float (single
  precision in the original: test numbers below are computed in single
  precision where it matters).
- "Damage kind" means Source's damage-type bits as map data and entity
  keys expose them (`trigger_hurt`'s `damagetype`): generic 0, crush 1,
  bullet 2, slash 4, burn 8, fall 32, blast 64, club 128, shock 256, sonic
  512, energy beam 1024, drown 16384, nerve gas 65536, poison 131072,
  radiation 262144, acid 1048576, physgun 8388608, plasma 16777216,
  dissolve 67108864, buckshot 536870912. A hit can carry several bits.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| health_max | 100 | points | max health; spawn health |
| armour_spawn | 0 | points | armour at spawn |
| armour_max_items | 100 | points | cap for batteries and normal suit chargers |
| armour_max_citadel | `sk_suitcharger_citadel_maxarmor` = 200 | points | cap for a citadel-flagged suit charger |
| armour_health_share | 0.2 | – | share of a hit that reaches health while armour lasts |
| armour_cost | 1 | armour per absorbed point | `player_old_armor` 0 (default) |
| armour_cost_blast | 2 | armour per absorbed point | any blast bit, multiplayer |
| armour_cost_old | 0.5 (blast 1) | | `player_old_armor` 1 |
| armour_min_cost | 1 | point | per hit (new armour only); see "Armour" for why it never shows |
| aux_max | 100 | – | aux power pool, spawn value |
| aux_recharge | 12.5 | per s | 0.1875 per tick |
| aux_recharge_delay | 0.5 | s | after the last device stops (34 ticks) |
| sprint_drain | 25 | per s | 0.375 per tick |
| breather_drain | 6.7 | per s | 0.1005 per tick |
| flashlight_drain | 0 | per s | HL2:DM attaches no device (HL2 single player: 2.222) |
| device_off_cost | 0.1 × drain | aux | sprint 2.5, breather 0.67, charged once when a device stops |
| sprint_min_aux | 10 | – | aux needed to start sprinting |
| aux_empty | 0.01 | – | below this after a drain, aux is set to 0 |
| still_epsilon | 0.001 | units/s | each horizontal velocity component at most this: "not moving" |
| air_time | 7 | s | air after aux power runs out under water (467 ticks to the first hit) |
| drown_damage | 10 | per hit | first and every hit (HL2 set: start 10, max 10) |
| drown_interval | 1 | s | strict: 67 ticks between hits |
| drown_restore | 10 | per check | health given back after surfacing |
| drown_restore_interval | 2 | s | at least 2 s between checks (134 ticks) |
| suit_zoom_fov | 25 | degrees | +zoom target FOV |
| suit_zoom_in_time | 0.4 | s | transition to 25 |
| suit_zoom_out_time | 0.2 | s | transition back to the default FOV |
| fov_min_client | 5 | degrees | the client never draws below this |
| `sk_healthkit` | 25 | health | `item_healthkit` (skill.cfg; registered 0) |
| `sk_healthvial` | 10 | health | `item_healthvial` (skill.cfg; registered 0) |
| `sk_battery` | 15 | armour | `item_battery` (skill.cfg; registered 0) |
| `sk_suitcharger` | 75 | armour | `item_suitcharger` pool (skill.cfg; registered 0) |
| `sk_suitcharger_citadel` | 200 | armour | citadel-flagged pool (skill.cfg) |
| kleiner_pool | 25 | armour | Kleiner-flagged suit charger pool (fixed) |
| `sk_healthcharger` | 50 | health | `item_healthcharger` pool (skill.cfg; registered 0) |
| charge_gap | 0.1 | s | strict: one charge every 7 ticks (0.105 s), 9.52 per s |
| charger_idle_off | 0.25 | s | charger switches off this long after the last use (17 ticks) |
| charger_deny_repeat | 0.62 | s | empty-charger deny sound at most this often (42 ticks) |
| charger_loop_delay | 0.56 | s | from the start sound to the charging loop |
| suit_charger_refill | 30 | s | multiplayer rules; citadel-flagged 60 |
| health_charger_refill | 60 | s | multiplayer rules |
| `sv_hl2mp_item_respawn_time` | 30 | s | kits, vials, batteries (H10 owns the respawn) |
| pickup_bloat | 24 sideways, 12 up, 0 down | units | item touch box grown from its bounds |
| use_radius | 80 | units | +use reach (specs/source/doors_buttons.md) |

`cfg/skill.cfg` (identical in the game and the server) is executed
through `cfg/skill_manifest.cfg` at every map start, so its values
override the registered zeros on both a listen and a dedicated server.

## Behavior

### Health

- Spawn: health 100, max 100. Nothing in HL2:DM raises health above 100:
  kits, vials, chargers and drown recovery all clamp at 100 and refuse (or
  give nothing) at 100.
- **Fractional damage.** Every character keeps a remainder r (starts 0 when
  the player connects; **never reset**, not even on respawn). A hit of h
  health damage (after armour) is applied as:

```math
\begin{aligned}
f &= h - \lfloor h \rfloor,\qquad n = \lfloor h \rfloor,\qquad r \leftarrow r + f\\
\text{if } r \ge 1:&\quad n \leftarrow n + 1,\; r \leftarrow r - 1\\
\text{if } n \le 0:&\quad \text{the hit does nothing to health (no hurt event, no pain, no push)}\\
\text{else}:&\quad \text{health} \leftarrow \text{health} - n
\end{aligned}
```

  At most one carried point is added per hit. Armour already paid for the
  hit even when n is 0.
- Death when health ≤ 0 after a hit. Health may be negative; if it is
  below −99 at death it is set to 0. The hurt event reports max(0, health).
- Healing (kits, chargers, drown recovery) adds whole points and clamps at
  100; it fails (returns "not taken") when health is already ≥ 100.

### Armour

Order of one hit on a living player (bullets: after the hitgroup scale,
`sk_player_head` 3 etc., source_deltas.md 1.3; the pellets of one shotgun
shot that hit the same player arrive as **one** hit with their sum, H3):

1. HL2:DM-only refusals before anything else: blast-only damage (kind
   exactly 64) within 0.5 s of spawning (H4/H6); a surface blast (bit
   134217728) against a player whose eyes are under water from an
   inflictor out of the water (H4); a fall within an ignore window set by
   map input (none in stock maps).
2. A hit of 0 damage does nothing; a dead player takes nothing; the rules
   may refuse the hit (team play friendly fire, H6). A refused hit costs
   no armour.
3. **Armour**, when armour A > 0 and the hit carries none of the bits
   fall (32), drown (16384), poison (131072), radiation (262144). Everything
   else is absorbed: bullet, buckshot, club, slash, crush and physics, burn,
   shock, blast, dissolve, plasma, sonic, energy beam, and **generic** (0),
   which is what most `trigger_hurt`s deal. With raw damage D and cost
   factor w (1, or 2 when the hit carries the blast bit; with
   `player_old_armor` 1: 0.5, blast 1):

```math
\begin{aligned}
h &= 0.2\,D \\
c &= \max\big(1,\ (D - h)\,w\big) \quad(\text{no }\max\text{ with }\texttt{player\_old\_armor}\ 1)\\
\text{if } c > A:&\quad h = D - A/w,\quad A \leftarrow 0\\
\text{else}:&\quad A \leftarrow \operatorname{trunc}(A - c)
\end{aligned}
```

   then h goes to health as "Fractional damage" says. Armour is a whole
   number and the subtraction truncates, so the armour lost is
   ⌈0.8·D·w⌉ (when that is not already whole): 7 bullet damage costs 6
   armour, 1 costs 1. That is why the "at least 1" rule never changes a
   result: for whole A ≥ 1 and 0 < c < 1, trunc(A − c) = A − 1 anyway
   (checked for every whole A 1–200 and D 1–300, both w, in single
   precision: truncation and exact ⌈⌉ agree everywhere).
4. Health takes h; death check; the hurt event.

Where armour comes from: batteries (+15, cap 100), suit chargers (+1 per
charge, cap 100; citadel-flagged +2, cap 200; Kleiner-flagged +1, cap 100).
Armour is 0 at every spawn. No cvar sets a spawn armour.

### Whether `hits::absorb` can express it (C2)

Our `chimera_sim::weapon::hits::absorb` lets through (1 − 2P) + P·ratio
of the raw damage, makes armour lose the rest times the entries' wear,
truncates both results by one `quantum`, and on running out (`Depletion::
Cost`) gives health raw − amount/wear. HL2's suit maps onto that shape:

- P = 0.8 on every `BodyPart` (head, torso, arm, leg, groin alike), ratio 1
  (HL2 guns have no armour ratio): (1 − 1.6) + 0.8 = 0.2 to health.
- wear 1, or 2 for `DamageClass::BLAST`; `Depletion::Cost`: on running out
  health takes raw − amount/wear, exactly step 3's first branch, and the
  comparison "cost > amount" is the same.
- The armour remainder truncated to whole points (quantum 0.01 of the
  normalised armour) is HL2's truncation, and with it the minimum cost of
  1 comes for free (step 3).

So **no per-game armour rule hook is needed for the formula**. Two things
in our API do not fit as they stand, and C2 should change them instead:

1. **One quantum for both outputs.** HL2 truncates the armour but must not
   truncate the health share: health keeps the fraction in the
   per-character remainder ("Fractional damage"). `absorb` needs separate
   armour and health quanta (HL2: armour 0.01, health 0), and HL2:DM's
   health delivery keeps the remainder (a property of the body's game, as
   CS:S's own truncation is).
2. **Damage classes.** `DamageClass::of` sends drown to `GENERIC` and fall
   to `CRUSH`, both of which the suit absorbs; radiation has no class. The
   suit needs fall, drown, poison and radiation distinguishable from
   generic and crush, and a coverage list that means "every class except
   these four, BLAST at wear 2": a catch-all entry (`class: None`) plus a
   BLAST entry would sum to P = 1.6 (clamped 1), so the suit must list one
   entry per covered class, or `Coverage` needs an exclusion list. The
   classes HL2:DM adds (shock, dissolve, sonic, energy beam, plasma,
   club/slash as melee) must all be covered by default.

Cross-game, a CS:S gun (ratio 1.4–2) on the suit then lets through
(1 − 1.6) + 0.8 × ratio (0.52–1.0): a consequence of the one-interface
design (DESIGN.md "Hits across games"), not of HL2.

### Aux power

State per player: aux a ∈ [0, 100]; the set of active devices (sprint,
breather; flashlight exists in the suit but HL2:DM never attaches it); the
load L = sum of the active devices' drains; the time t_off when the set
last became empty (0 at map start).

Once per tick, inside the movement step (Per-tick order 4b), after the
sprint device was attached or detached for the tick:

- **Recharge** if no device is active, a < 100 and now ≥ t_off + 0.5:

```math
a \leftarrow \min(100,\ a + 12.5 \times 0.015)
```

- **Else drain** if any device is active:

```math
\begin{aligned}
L' &= L - 25 \quad\text{if sprint is active and } |v_x| \le 0.001 \text{ and } |v_y| \le 0.001\\
a &\leftarrow a - L' \times 0.015;\qquad \text{if } a < 0.01:\ a \leftarrow 0
\end{aligned}
```

  (v is the player's velocity at the **start** of the tick, before this
  tick's move: a sprint started from standing costs nothing on its first
  tick.) With only the sprint device active and standing still, L' is 0
  exactly.
- **Attaching** a device adds its drain to L (nothing else; fails without
  the suit or if already active). **Detaching** one first drains 0.1 × its
  drain at once (sprint 2.5, breather 0.67; clamped at 0, the 0.01 rule
  applies), then removes it; if no device is left, t_off = now. Detaching
  a device that isn't active, or without the suit, does nothing (see
  Quirks: devices that stay attached across death).
- `sv_infinite_aux_power` 1 (cheat): drains (including the detach cost)
  do nothing.
- Recharge at full and drain at 0 are both no-ops; a tick never both
  charges and drains.

### Sprint

Decided at the start of each movement step (before acceleration; H1 uses
the result as the max speed):

- **Start**: only on a tick where `+speed` was pressed (its state changed),
  when the suit is on, the player may sprint (server: not fully ducked —
  ducked and no duck transition running; eyes not under water, water
  level ≠ 3) and a ≥ 10. If a < 10 on that press: `HL2Player.SprintNoPower`
  and no sprint. A successful start plays `HL2Player.SprintStart`.
- **Stop**: when `+speed` is released, or when a < 0.01 (aux ran out),
  checked every tick.
- Nothing else stops it: ducking, going under water or pressing `+walk`
  while sprinting keep the sprint (and its drain). A press refused by duck,
  water or low aux is not retried while the key stays down: the player must
  release and press `+speed` again.
- **Walk**: `+walk` gives 150 only when not sprinting and duck is not held;
  sprinting wins over walking in either order. Without the suit the
  player always walks (never in HL2:DM while alive).
- Max speed: sprint `hl2_sprintspeed` 320, walk `hl2_walkspeed` 150,
  otherwise `hl2_normspeed` 190; `sv_maxspeed` 320 caps (H1).
- The sprint device is attached on every tick the chosen max speed equals
  the sprint speed and detached otherwise, so a stop costs 2.5 once.
- Server and client differ in one condition: the client refuses a start
  also while a duck transition runs (ducked or ducking); the server only
  when fully ducked. A press during a duck transition sprints on the
  server and is corrected on the client (prediction error). Use the
  server's rule.
- `sv_stickysprint` (0, server only): when 1 the server stops exempting the
  standing player from the sprint drain; the client always exempts. Keep 0.

### Breath under water

Each tick in the pre-think (Per-tick order 2b), for a living player not
noclipping:

- **Breathing** when the eyes are not under water (water level < 3) **or**
  a > 0 (the suit breathes for the player while any aux power is left):
  - if the air had run out (now > air deadline): play `Player.DrownStart`
    (the gasp);
  - air deadline = now + 7; drown damage per hit reset to 10;
  - if drowning damage taken exceeds what was given back, mark recovery
    active (see below).
- **Not breathing** (eyes under water and a = 0): recovery stops; when now
  > air deadline and now > pain deadline: one drown hit of 10 (kind drown:
  bypasses armour, no attacker: the world), pain deadline = now + 1, add
  10 to the drowning total. The first hit of an episode (nothing pending
  recovery) plays `Player.DrownStart`, later ones `Player.DrownContinue`.
- Then the underwater state: eyes go under → attach the breather (6.7/s);
  eyes come out → detach it (0.67 at once, recharge 0.5 s later). The
  breather drains like any device in the movement step, so under water with
  a = 100 the suit lasts 995 ticks (14.925 s), then the 7 s of air.
- **Recovery** (the shared time-based check, Per-tick order 2d): at most
  once per 2 s (the check runs when |now − last check| ≥ 2, the last check
  being per player and updated only when some time-based effect is
  pending), give back min(10, total − returned) health (healing clamps at
  100; the returned count grows even if health was full) until all
  drowning damage is returned. The first +10 comes on the surfacing tick
  itself when no time-based check ran in the last 2 s; then every 134
  ticks.
- Noclip without a parent refreshes the air every tick. Spawning forgets
  any unreturned drowning damage and refreshes the air.

CS:S (movement.md, measured) for comparison: 12 s of air, 3, 4, 5… per
hit, no server drown sounds. HL2:DM: 7 s after the aux power, flat 10,
sounds on every hit.

### Flashlight

- `impulse 100` (bound to F) toggles: if the light is on, off; else on.
- On: only when `mp_flashlight` > 0 (registered 0, and no shipped config
  sets it: **off by default**, H0 open question 1) and the player is alive.
  Sets the player's dim-light effect and plays `HL2Player.FlashlightOn`
  (the script defines the entry as `HL2Player.FlashLightOn`: lookups ignore
  case). No aux power device, no drain, no battery: on forever.
- Off: always works (also with `mp_flashlight` 0 for a light already on);
  `HL2Player.FlashlightOff` only if alive.
- The HUD's flashlight element is hidden while `mp_flashlight` is 0.
- Others see it as a beam from the model's right-hand attachment 200 units
  along the eye angles (H9 draws it).
- Death turns it off silently. A drained aux pool never turns it off
  (single player does).

### Suit zoom

- `+zoom` (bound to Z): pressing starts the suit zoom, releasing stops it
  (checked in the pre-think, needs the suit). `toggle_zoom` (console
  command, unbound) toggles it.
- `impulse 50` (bound to C by `config_default.cfg`) is **not** zoom: it is
  HL2's squad command and does nothing in HL2:DM (no squad).
- Start: request FOV 25 over 0.4 s. Stop: request the default FOV
  (`fov_desired`, 75–90, H0 1.8) over 0.2 s.
- **One zoom owner.** A request is refused while another owner holds the
  zoom (the crossbow's scope, H4: 20 over 0.1 s, out over 0.2 s). So: the
  suit zoom can't start while the crossbow is zoomed and its release does
  nothing then; the crossbow can't zoom while the suit zoom holds (its
  zoom toggle is refused and it stays unzoomed). The suit zoom stops on
  `+zoom` release, on any weapon switch, on death; not on damage, sprint
  or anything else. Sprint and zoom don't interact.
- **While `+zoom` is held, attack and attack2 are cleared** for the tick
  (no shooting through the suit zoom held on the key; `toggle_zoom` doesn't
  hold the key, so it allows firing).
- The FOV moves from the FOV at the moment of the request (the server's
  current value truncated to a whole degree) to the target with a smooth
  step over the given time T:

```math
x = \operatorname{clamp}\!\left(\frac{t - t_0}{T}, 0, 1\right),\qquad
\text{FOV}(t) = F_0 + (F_1 - F_0)\,(3x^2 - 2x^3)
```

  The client evaluates it at its render time (not tick-quantised) and
  never draws below 5°. The suit zoom's HUD overlay (H8) follows the zoom
  state.

### HUD side of the suit (behaviour only; H8 owns layout)

- The aux power panel draws while aux power changes or its fade
  animation runs; it plays `SuitAuxPowerMax` when aux reaches 100 and
  `SuitAuxPowerNotMax` when it drops below; a "low" state when the lit
  chunks ≤ a quarter of the chunk count (lit = round(chunks × a/100)).
- It names the active devices from the device bits: OXYGEN (breather),
  SPRINT (sprint device: shown also when sprinting in place, which drains
  nothing), FLASHLIGHT (never in HL2:DM), and plays the one/two/three/no
  items animation as the count changes.
- No HEV voice lines in multiplayer.

### Pickups: item_healthkit, item_healthvial, item_battery

- **Touch**: the item's touch box is its bounds grown 24 units in X and Y
  and 12 up (not down). A living player overlapping it touches it every
  tick. It can be taken only if a line from the item's centre (its physics
  position) to the player's eye hits no solid world or entity (the player
  and the item ignored); otherwise the touch is ignored.
- `item_healthkit` (`models/items/healthkit.mdl`): heal 25; refused at 100
  health. Sound `HealthKit.Touch`.
- `item_healthvial` (`models/healthvial.mdl`): heal 10; refused at 100.
  Sound `HealthVial.Touch`.
- `item_battery` (`models/items/battery.mdl`): +15 armour capped at 100;
  refused when armour ≥ 100 (also after a citadel charger raised it above
  100) or without the suit. Sound `ItemBattery.Touch`.
- Taken: the pickup-history message to the taker (H8), the item hides and
  is gone for `sv_hl2mp_item_respawn_time` 30 s, then reappears at its
  original place with `AlyxEMP.Charge` and a flash (H10 owns respawn,
  placement and the no-respawn flag). Refused: nothing happens; the item
  stays and is offered again on the next tick of touching.
- Stock-map counts (H10's): 76 kits, 44 batteries, 9 vials (dm_underpass).

### Wall chargers: item_suitcharger, item_healthcharger

Both are static physics props (`models/props_combine/suit_charger001.mdl`,
`models/props_combine/health_charger001.mdl`) used with `+use`
(specs/source/doors_buttons.md: found within 80 units by the trace and
radius search; HL2's search traces against solid, debris and player-clip
contents, not CS:S's mask). They are "continuous use": while `+use` is
held and the charger stays the found entity, it is used **every tick**.
Pressing `+use` on anything found plays `HL2Player.Use`; pressing it on
nothing plays `HL2Player.UseDeny` (after the weapon step; not while on a
ladder, H1).

One use, in order:

1. **Suit charger only**: without the suit, `SuitRecharge.Deny` (at most
   every 0.62 s) and stop.
2. **Empty** (pool ≤ 0): show the empty dial, switch off now (which
   schedules the refill, below), deny sound at most every 0.62 s
   (`SuitRecharge.Deny`, `WallHealth.Deny`), stop.
3. **Full player**: suit charger: armour ≥ cap (and for a citadel charger
   also health ≥ 100); health charger: health ≥ 100. Then the press is
   consumed and the charger becomes press-only until pressed again (so one
   deny per press, held or not), deny sound (no repeat limit), stop.
4. Schedule "switch off" 0.25 s from now (pushed back by every use).
5. **Rate gate**: if now ≤ last charge + 0.1, stop (strict: charges come
   every 7 ticks, the first at once).
6. First charge of a run: `SuitRecharge.Start` / `WallHealth.Start`, the
   map output `OnPlayerUse`; the first charge tick at least 0.56 s later
   starts the loop `SuitRecharge.ChargingLoop` /
   `WallHealth.LoopingContinueCharge` (with a run from tick 1: tick 43).
7. **Charge**: suit: if armour < cap, pool −1 and armour +1 (capped).
   Health: if healing 1 succeeds, pool −1. Then last charge = now. Outputs
   `OutRemainingCharge` / `OutRemainingHealth` = pool / pool max; the suit
   charger also fires `OnHalfEmpty` (crossing half), `OnEmpty`, `OnFull`.
8. **Switch off** (0.25 s after the last use, or at once when used empty):
   stop the loop sound, the next use starts a new run with the start
   sound. If the pool is 0 and no refill is pending, schedule the refill
   after the rules' time: suit 30 s, health 60 s. Refill: pool to max,
   `SuitRecharge.Start` / `WallHealth.Recharge`. The map key `dmdelay` is
   ignored (overwritten at spawn).

Spawn-flag variants of `item_suitcharger` (map data):

| Flag | Pool | Per charge | Armour cap | Also | Refill | Stock maps |
|---|---|---|---|---|---|---|
| none | `sk_suitcharger` 75 | +1 armour, −1 pool | 100 | – | 30 s | 9 (dm_lockdown 1, dm_powerhouse 2, dm_steamlab 2, dm_underpass 3, halls3 1) |
| 8192 (citadel) | `sk_suitcharger_citadel` 200 | +2 armour, −2 pool (HL2:DM's value; single player 10) | `sk_suitcharger_citadel_maxarmor` 200 | +5 health per charge while health < 100, even when armour is full (then no pool is used) | 60 s | dm_lockdown 1 |
| 16384 (Kleiner) | 25 | +1, −1 | 100 | – | 30 s | dm_resistance 2 |

Health chargers: 8 in stock maps (dm_lockdown 1, dm_powerhouse 4,
dm_underpass 2, halls3 1), all plain. The dial (model cycle 1 − pool/max,
eased down a seventh of a point per tick while charging) is visual (H10).

### Death and spawn

- At death: flashlight off (silent), the FOV reset to the default at once
  and the suit zoom flag cleared (unless the crossbow owns the zoom, H4),
  sprinting stops with the suit; health may be ≤ 0 (−99 floor rule
  above); armour keeps its value while dead (it is only reset at spawn);
  no item pickups (the dead player is not solid). On the first death think
  the weapons are dropped or removed **and the suit is removed**.
- At spawn: health 100 (max 100), armour 0, aux 100, suit on (with the
  loadout, H6/H3), FOV default, flashlight off, air refreshed, unreturned
  drowning damage forgotten, the remainder r **kept**.

## Per-tick order

One player command (0.015 s) on the server; the client predicts steps 4a
and 4b (sprint, walk, aux) with the same rules except the sprint-start duck
condition.

1. Buttons: pressed and released this tick.
2. Pre-think:
   a. `+use` (search and continuous use: chargers charge here);
   b. breath and drowning hit; then the underwater state change attaches or
      detaches the breather (detach cost 0.67 now);
   c. the flashlight HUD hidden or shown from `mp_flashlight`;
   d. the time-based check (drown recovery, at most once per 2 s);
   e. suit zoom on `+zoom` press / release;
   f. if dying or dead: the death think (first one strips weapons and suit),
      and nothing below in the pre-think;
   g. while `+zoom` is held: attack buttons cleared.
3. The player's own think.
4. Movement:
   a. sprint / walk / run choice (Sprint), the max speed;
   b. aux: attach or detach the sprint device (detach cost 2.5 now), then
      recharge or drain for 0.015 s using the velocity from the start of
      the tick;
   c. the move (H1); a landing deals fall damage here (armour bypassed).
5. Post-think: the weapon (H3; damage it deals to others lands now);
   impulses (100 flashlight, 50 squad command: nothing); the `+use` deny
   sound.

Entities' own timers (charger switch-off and refill, item respawn) run in
the server's entity think phase of the tick (H10). Damage from others
lands whenever their command or think runs; each hit follows "Armour"
steps 1–4 at once.

## Edge cases

- Aux exactly 10 starts a sprint; 9.99 doesn't.
- A sprint whose aux runs out on tick k: aux 0 after tick k; tick k+1 runs
  at 190 and pays the 2.5 detach cost against 0 (stays 0); recharge from
  tick k+35.
- Sprinting in place with `+speed` held keeps the sprint device and the
  SPRINT label but drains nothing, and aux does **not** recharge (a device
  is active).
- Under water while sprinting (started above water): drain 31.7/s; aux
  from 100 lasts 211 ticks.
- Diving with aux 0 already (just after a long sprint): the breather
  attaches, a = 0, no breathing help: 7 s of air from the tick the eyes
  went under (the last breathing tick sets the deadline).
- A player who surfaces and goes under again before the recharge delay
  gets no new aux power (the breather re-attaches; the 0.5 s timer starts
  again at the next detach).
- Drown recovery runs on the surface **and** under water while aux power
  lasts (both count as breathing).
- A `trigger_hurt` of the generic kind (stock: dm_runoff 500, dm_steamlab
  200, 70, 30) is absorbed by armour; dm_underpass's 75 is radiation and
  bypasses it. CS:S's trigger_hurt never touches armour (fall_damage.md):
  the bite timing is H0's "same", the armour part is not.
- Health 99 and a kit: health 100, the whole kit used. Armour 95 and a
  battery: 100.
- A charger's rate gate counts from the last charge, also across runs: let
  go and press again within 0.1 s and the first charge of the new run
  waits for the gate.
- A suit charger used by a player at armour ≥ 100 but on a citadel charger
  with health < 100: health +5 per 7 ticks, no pool used, no deny.
- Two players on one charger: each use passes the same shared gate, so
  together they get one point per 7 ticks, not two.
- The citadel pool can step from 1 to −1 (pool − 2): empty either way.
- Item bounds for the touch are the physics object's; a kit knocked by
  physics moves its touch box with it (H10/H5).
- A hit carrying blast plus another bit (blast|burn) costs armour at 2; a
  hit carrying fall plus anything bypasses armour.

## Quirks

- **Armoured health loss is uneven**: the 0.2 share is carried in the
  remainder, so 8-damage pistol hits take 1, 2, 1, 2 health (Keep).
- **The remainder survives death**: a player may die and on the next life
  lose a point to a 0.2 hit (Keep).
- **Armour loses whole points rounded up** (7 damage costs 6) (Keep).
- **Standing sprint is free but blocks recharge** (Keep).
- **Ducking doesn't stop a sprint already running**, and the speed key held
  through a refused press never sprints until pressed again (Keep).
- **Detach costs**: tapping `+speed` while moving costs 0.375 + 2.5 (Keep;
  it is HL2's guard against tap-sprinting).
- **Dying with a device attached leaves it attached.** The suit is removed
  at the first death think, after which detaching fails. A player killed
  while sprinting respawns and pays the 2.5 sprint detach on the first
  tick (aux 97.5, full again 0.5 s + 14 ticks later). A player who dies
  with the eyes under water keeps the breather attached after respawn: aux
  drains at 6.7/s from 100 to 0 and never recharges until the player next
  goes under water and comes out. Predicted from the reference; probe 9
  confirms before we copy it (Keep if confirmed: it is the game's
  behaviour).
- `+zoom` held blocks firing, `toggle_zoom` doesn't (Keep).
- `impulse 50` on C does nothing (Keep).
- The flashlight is free and off by default (`mp_flashlight` 0) (Keep).
- The citadel charger lifts armour to 200 while batteries refuse above
  100 (Keep).
- Health chargers' and suit chargers' full-player deny plays once per
  press; the empty deny repeats every 0.62 s while held (Keep).

## Corrections to H0

- 1.1 says "walking wins over sprint": the reverse. Walking applies only
  when not sprinting; holding `+walk` neither stops a sprint nor prevents
  one from starting (H0's own test 6 agrees with this spec).
- Test 3 ("standing, aux 100, hold forward + `+speed`, 1 tick → aux
  99.625"): from standing the first tick drains nothing (velocity at the
  start of the tick is 0); 99.625 is after the second tick, or after the
  first if already moving (this spec's tests 15 and 16).
- The `config_default.cfg` list calls C (`impulse 50`) "suit zoom": it is
  HL2's squad command and does nothing in HL2:DM; the suit zoom is `+zoom`
  (Z) and `toggle_zoom`.
- 1.2's `trigger_hurt` "same as CS:S": same timing, but HL2:DM's armour
  absorbs generic `trigger_hurt` damage (CS:S's doesn't).
- 1.1's "stops when aux power falls below 0.01" and "needs at least 10 to
  start" hold; the start also needs the press itself (state change) and
  not fully ducked, not under water.

## Test cases

Implementer checks (no game needed). Defaults: tick 0.015 s, health 100,
armour as stated, aux 100, suit on, `player_old_armor` 0, remainder 0.
Damage "bullet" is kind 2, "blast" 64, "generic" 0.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | armour 100 | 30 bullet | the hit | health 94, armour 76, remainder 0 |
| 2 | armour 100 | 30 blast | the hit | health 94, armour 52 |
| 3 | armour 10 | 30 bullet | the hit | cost 24 > 10: health 80, armour 0 |
| 4 | armour 10 | 30 blast | the hit | cost 48 > 10: health 30 − 10/2 → 75, armour 0 |
| 5 | armour 24 | 30 bullet | the hit | cost 24 not > 24: armour 0, health 94 |
| 6 | armour 100 | four 8-damage bullet hits (pistol, chest) | each hit | health 99, 97, 96, 94; armour 93, 86, 79, 72; remainder 0.6, 0.2, 0.8, 0.4 |
| 7 | armour 50 | five 1-damage bullet hits | each hit | armour 49, 48, 47, 46, 45; health 100, 100, 100, 100, 99 (remainder 0.2, 0.4, 0.6, 0.8, then 0); hits 1–4 fire no hurt event |
| 8 | armour 100 | pistol head shot (8 × 3 = 24) | the hit | health 96 (4.8: 4, remainder 0.8), armour 80 |
| 9 | armour 100 | one shotgun shot, 7 pellets × 9 on the chest | the shot | one hit of 63: health 87 (12.6), armour 49 (100 − 50.4 truncated) |
| 10 | armour 50 | fall 10; drown 10; radiation 10; poison 10 | each | health −10 each, armour 50 throughout |
| 11 | armour 100 | generic 10 (a `trigger_hurt` bite of a damage-20 trigger) | the bite | health 98, armour 92 |
| 12 | armour 100, `player_old_armor` 1 | 30 bullet; then (fresh) 30 blast | the hit | bullet: health 94, armour 88; blast: health 94, armour 76 |
| 13 | armour 10, `player_old_armor` 1 | 30 bullet | the hit | cost 12 > 10: health 30 − 10/0.5 = 10 → 90, armour 0 |
| 14 | remainder 0.8 left from the previous life, respawned, armour 50 (from a charger) | 1 bullet | the hit | health 99 (0.2 completes a point), remainder ≈ 0 |
| 15 | standing still, aux 100 | hold forward, press `+speed` at tick 1 | ticks 1, 2 | max speed 320 from tick 1, `HL2Player.SprintStart` once; aux 100 after tick 1, 99.625 after tick 2 |
| 16 | moving at 190, aux 100 | press `+speed` at tick 1, hold | tick 1; tick 267; tick 268 | 99.625; aux 0 after tick 267 (267 drains); tick 268: speed 190, sprint device detached (aux stays 0) |
| 17 | as 16 | keep holding `+speed` | ticks 268–835 | recharge starts tick 302 (+0.1875), aux 100 at tick 835 (534 charges); no sprint at any point while held, even past aux 10 |
| 18 | moving, aux 100 | `+speed` on tick 1 only, released tick 2 | ticks 1–51 | 99.625, then 97.125 at tick 2; unchanged ticks 3–35; 97.3125 at tick 36; 100 at tick 51 |
| 19 | aux 9.99 | press `+speed` | 1 tick | no sprint, `HL2Player.SprintNoPower`, speed 190; aux 10.0 → sprint |
| 20 | holding `+walk`, moving | press `+speed` | 1 tick | 320 (sprint); then release `+walk`/press it again: still 320 |
| 21 | holding `+walk` and duck | forward | 1 tick | not walking: run max speed 190 (before H1's duck scale) |
| 22 | fully ducked | press `+speed`, keep it, stand up | until standing | no sprint, no sound; 190 standing; release and press again: sprint |
| 23 | sprinting, moving | hold duck (keep `+speed`) | 60 ticks | still sprinting: aux −0.375 each tick |
| 24 | standing in place, sprinting (velocity 0) | hold `+speed` 2 s | 133 ticks | aux unchanged, no recharge |
| 25 | aux 100, standing on the bottom, eyes under water from tick 1, no sprint | wait | ticks 1–2065 | aux 99.8995 after tick 1; 0 after tick 995; first drown hit 10 at tick 1462 (`Player.DrownStart`), then every 67 ticks (1529, 1596, …, `Player.DrownContinue`); dead at the 10th hit, tick 2065; armour unchanged |
| 26 | as 25, surface on tick 1650 after 3 hits (health 70), no time-based check in the last 2 s | stay up | from tick 1650 | `Player.DrownStart` (gasp) at 1650; breather detached (aux 0); health 80 at 1650, 90 at 1784, 100 at 1918, then no more; aux recharge from tick 1684 |
| 27 | aux 100 | eyes under for 100 ticks, then out | the surfacing tick | aux 100 − 100 × 0.1005 = 89.95, then −0.67 → 89.28; recharge 34 ticks later |
| 28 | sprinting under water (sprint started above), aux 100, moving | hold | 211 ticks | aux 0 after tick 211 (31.7 per s) |
| 29 | `mp_flashlight` 0 | `impulse 100` | 1 tick | light stays off, no sound |
| 30 | `mp_flashlight` 1, aux 100 | `impulse 100`, wait 20 s, `impulse 100` | – | on with `HL2Player.FlashlightOn`; aux 100 throughout; off with `HL2Player.FlashlightOff` |
| 31 | flashlight on | die | – | off, no sound |
| 32 | FOV 75 | press `+zoom` at t0 | client times | 67.1875 at t0 + 0.1, 50 at +0.2, 32.8125 at +0.3, 25 from +0.4 |
| 33 | zoomed at 25 | release at t1 | | 50 at t1 + 0.1, 75 from t1 + 0.2 |
| 34 | zooming in, FOV exactly 50 at release (t0 + 0.2) | release | | from 50 to 75 over 0.2 s: 62.5 at +0.1 |
| 35 | holding `+zoom` and attack with the SMG1 | 1 s | – | no shots; with `toggle_zoom` instead: shots fire |
| 36 | `impulse 50` | – | – | nothing: FOV, weapons, aux unchanged |
| 37 | suit zoom held | switch weapon | – | zoom out to 75 over 0.2 s |
| 38 | crossbow zoomed (20) | press and release `+zoom` | – | FOV stays 20 throughout |
| 39 | suit zoom held (25) | crossbow zoom toggle | – | refused: FOV 25, crossbow unzoomed |
| 40 | health 100 / 90 / 60; vial at 95 | touch a kit / vial | – | refused, kit stays / 100 / 85; vial 100 |
| 41 | armour 95 / 100 / 150 | touch a battery | – | 100 / refused / refused |
| 42 | a kit, a wall between its centre and the eye, boxes overlapping | touch | – | refused |
| 43 | kit taken at t | – | t + 30 s | reappears at its spawn point, `AlyxEMP.Charge` |
| 44 | `item_suitcharger` (75), armour 0 | hold `+use` from tick 1 | ticks 1–2520 | armour k at tick 1 + 7(k − 1): 1 at 1, 2 at 8, …, 75 at 519; `SuitRecharge.Start` tick 1, loop from tick 43; tick 520: deny, refill scheduled; pool 75 again 30 s later (tick ≈ 2520) with `SuitRecharge.Start` |
| 45 | suit charger, armour 90 | hold `+use` from tick 1 | ticks 1–81 | armour 100 at tick 64 (10 charges), pool 65; tick 65 one `SuitRecharge.Deny`, no more while held; switch-off (loop stops) at tick 81; no refill (pool not empty) |
| 46 | `item_healthcharger` (50), health 40 | hold `+use` | ticks 1–345 | health +1 per 7 ticks, 90 at tick 344, pool 0; tick 345 `WallHealth.Deny`, refill 60 s later with `WallHealth.Recharge` |
| 47 | health charger, health 100 | press `+use` 3 times | – | 3 × `WallHealth.Deny`, pool unchanged |
| 48 | empty charger | hold `+use` 2 s | 133 ticks | deny sound 4 times (ticks 1, 43, 85, 127) |
| 49 | suit charger flag 16384, armour 0 | hold `+use` | – | armour 25 at tick 169, then empty |
| 50 | suit charger flag 8192, armour 100, health 50 | hold `+use` | ticks 1–345 | each 7 ticks +2 armour, +5 health: health 100 at tick 64; armour 200 at tick 344 (50 charges), pool 100; tick 345 deny |
| 51 | `+use` pressed looking at nothing / at a charger | – | – | `HL2Player.UseDeny` / `HL2Player.Use` (plus the charger's) |
| 52 | killed with armour 60, health −20, flashlight on, suit-zoomed | respawn | – | health 100, armour 0, aux 100, FOV default, flashlight off |

Probe checks on the real server (game build 24600434, dedicated server
build 24600426; the probe and SourceMod as H0 Part 2 sets up; damage dealt
with SourceMod's take-damage call naming the kind; aux power, armour,
health, FOV and the active-device bits read from the player's network
properties every tick):

| # | How | Expected |
|---|---|---|
| P1 | probe: armour 100, bullet 30, then blast 30; four 8-bullet hits; five 1-bullet hits at armour 50 | tests 1, 2, 6, 7 exactly (proves truncation and the remainder) |
| P2 | probe: kill a player whose remainder is 0.8 (four 0.2 hits), respawn, give armour, one 1-bullet hit | health 99 (test 14: remainder survives death) |
| P3 | probe: bot standing, `+speed` and forward pressed on one tick; log aux per tick | 100 after the first tick, 99.625 after the second (test 15; H0 test 3 corrected) |
| P4 | probe: sprint from 100 while moving until empty; tap test 18 | 267 drain ticks; recharge 34 ticks after the stop; 2.5 detach cost |
| P5 | probe: sprint, then hold duck; sprint in place 2 s | sprint continues ducked; no drain and no recharge in place |
| P6 | probe on dm_underpass or dm_runoff deep water: bot submerged with aux 100 | aux 0 at about 14.9 s; first hit 10 about 21.9 s after going under; every 1.005 s; `Player.DrownStart`/`DrownContinue` emitted; recovery +10 at once then every 2.01 s |
| P7 | probe: `mp_flashlight 1`, flashlight on 30 s | aux stays 100; device bits never show the flashlight |
| P8 | probe: `+zoom` held, read FOV and the zoom owner; `impulse 50` | FOV target 25 with rate 0.4; out 0.2; `impulse 50` changes nothing |
| P9 | probe: kill a sprinting bot; kill a bot with eyes under water; respawn both on land | sprinting: aux 97.5 after the first tick; underwater: breather bit still set, aux draining 6.7/s and not recharging (Quirks) |
| P10 | probe: suit charger on dm_steamlab, health charger on dm_powerhouse: hold `+use` | one point per 7 ticks; pools 75 and 50; refills 30 s and 60 s after the last use |
| P11 | probe: dm_resistance's two flag-16384 suit chargers; dm_lockdown's flag-8192 one | 25-point pool; 200-point pool, +2 armour and +5 health per charge, armour up to 200 |
| P12 | probe RCON at map start | `sk_healthkit` 25, `sk_healthvial` 10, `sk_battery` 15, `sk_suitcharger` 75, `sk_suitcharger_citadel` 200, `sk_suitcharger_citadel_maxarmor` 200, `sk_healthcharger` 50, `mp_flashlight` 0, `player_old_armor` 0, `sv_stickysprint` 0, `sv_hl2mp_item_respawn_time` 30 |
| P13 | probe: armour 100 bot in dm_steamlab's damage-30 `trigger_hurt` | each bite 15: health −3, armour −12 |

## Open questions

1. **H0 corrections** (section "Corrections to H0"): "walking wins over
   sprint", test 3's first-tick drain, C as suit zoom, and `trigger_hurt`
   armour. H0 should be amended when it is next touched.
2. **Shipped sprint code.** The SDK's sprint is the newer state-change
   model (start only on the key's press, no stop on duck); the binaries of
   build 24600434/24600426 are assumed to match. P3–P5 confirm.
3. **Devices stuck across death** (Quirks): predicted from the order of the
   death think, the suit's removal and spawning; P9 confirms before
   Chimera copies it.
4. **A dead player's `+use`.** The pre-think handles `+use` before the
   death think returns, so a dead player's use search may still reach a
   charger (the suit charger then denies: no suit; the health charger
   gives nothing). Check on the probe whether anything audible happens.
5. **Sound entry case.** The code asks for `HL2Player.FlashlightOn`/`Off`
   and `AlyxEmp.Charge`; the scripts define `HL2Player.FlashLightOn`/`Off`
   and `AlyxEMP.Charge`. Assumed case-insensitive (they play in the game);
   confirm with `snd_show`-style logging on the probe.
6. **Client and server sprint start during a duck transition** differ
   (Sprint). Chimera uses the server's rule; whether the real client
   shows a visible correction is not worth measuring unless movecmp flags
   it.
7. **`mp_flashlight` on real servers.** Registered 0 and no shipped config
   changes it, so a stock server has no flashlight; community servers
   often set 1. Party modes may want 1 (a minigame setting, not parity).
8. **Armour above 100** (citadel charger, dm_lockdown): how the HUD shows
   it is H8's.
9. **C2** (this spec's verdict): the formula fits `hits::absorb` with
   separate armour and health quanta, a per-character damage remainder in
   HL2:DM's health delivery, and damage classes that tell fall, drown,
   poison and radiation apart from generic and crush. If the implementer
   finds the class extension costlier than a hook, an armour rule per game
   remains the fallback; the tests above pin either.
10. **Probe results (M3, 2026-11-03, dedicated server build 24600426,
    tools/hl2dm_probe).** P3: aux 100 after the first sprint tick from
    standing, 99.625 after the second (H0 test 3 corrected, confirmed);
    P4: 268 ticks to empty (0.25 after tick 267), the tap costs 0.375 +
    2.5 and the recharge starts 34 ticks after the release; P5: a sprint
    kept ducked drains 0.375 a tick, a standing sprint drains nothing and
    doesn't recharge; `HL2Player.SprintStart` is `player/suit_sprint.wav`
    at 0.9. P12 at map start: `sk_healthkit` 25, `sk_healthvial` 10,
    `sk_battery` 15, `sk_healthcharger` 50, `sk_suitcharger_citadel` 200,
    `sk_suitcharger_citadel_maxarmor` 200, `mp_flashlight` 0,
    `player_old_armor` 0, `sv_stickysprint` 0, `sv_hl2mp_item_respawn_time`
    30 as listed, but **`sk_suitcharger` is 30** (skill 1), not 75: the
    spec's pool (and test 44) needs a follow-up spec session (Chimera keeps
    75 until then). Also read: `mp_forcerespawn` 0 on the dedicated server
    (H0 Open question 1: `valve.rc` runs there too).
    **Second probe session (2026-11-03, `tools/hl2dm_probe/cases.py`):**
    P1: tests 1, 2 and the run-outs as listed, but armour pays its cost
    **truncated**, at least 1: 8 bullet damage on 100 leaves 94 (test 6:
    94, 88, 82, 76), 24 leaves 81 (test 8), 63 leaves 50 (test 9), blast
    6 leaves 91; "Armour" step 3's ⌈0.8·D·w⌉ is trunc(0.8·D·w) on this
    build (Chimera follows the server). P2: the remainder (0.8) survives
    death; one 1-bullet hit then takes 1. P6 (dm_underpass): aux falls
    0.1005 a tick, empty at about 14.9 s, hits of 10 at 21.9 s and every
    1.005 s, `player/pl_drown*.wav`, recovery +10 at once and 2.01 s
    later, as listed. P10: suit charger 1 point per 7 ticks, pool 30 (the
    cvar), refilled 30 s after it ran empty (a use held on the empty
    charger doesn't push the refill back); health charger 1 per 7 ticks,
    pool 50. P11: dm_lockdown's flag-8192 charger: +2 armour and +5
    health a charge, pool 200, armour to 200. Not measured yet: P7-P9,
    P13.
11. **Probe results, round 2 (2026-11-03, same build).** P7: with
    `mp_flashlight 1` the flashlight on (effects bit 4) for 9 s keeps aux
    at 100. P9 (sprint half): a bot killed while sprinting and
    respawned with no key held has aux 97.5 from its first tick (the
    sprint's 2.5 detach paid at the respawn: Quirks confirmed; Chimera
    keeps the sprint across the death). Not measured: P8 (the zoom needs
    the key held after a run ends), P9's breather half (dm_underpass),
    P13 (dm_steamlab).
