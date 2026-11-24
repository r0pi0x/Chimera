# Day of Defeat: Source: players and physics props (deltas from CS:S)

Source basis: measurements on the dedicated server (2026-11-24, build
11003710) with `tools/dods_probe` (`measure.py mpprops propmass
propsurvey`): a puppet bot run at 220 u/s along open flat ground of
dod_avalanche into a stock oil drum (`fueldrum.mdl`, upright, a
multiplayer prop on that map) spawned 120 units ahead as
`prop_physics_multiplayer` or `prop_physics` with chosen masses, modes
and spawn flags, the bot's and the drum's positions logged every tick
and the drum's collision group read before and after; the collision
group, physics mode and spawn flags of every multiplayer prop of
dod_avalanche read on the running map; the server's cvar list and its
shipped config files. The CS:S spec (specs/cs_source/physics_props.md,
section 4) is the base; this spec says how DoD:S differs.
Status: draft (measured 2026-11-24)

## Summary

DoD:S ships its server config with **turbo physics on**
(`sv_turbophysics 1` in `cfg/game.cfg`). Players then have no physics
shadow, so nothing is shoved by walking into it: a `prop_physics` of any
mass is a wall. Multiplayer props use CS:S's push-away rules **only while
they are in the push-away group**; with turbo physics a multiplayer prop
that has moved and settled drops into the normal group and becomes a
wall too. On a freshly loaded map that splits the props by their spawn
flags: those that start asleep stay soft (walked into, kicked aside, the
player pushed back) until something wakes them; the rest settle at load
and are walls. Neither CS:S's soft props everywhere nor HL2:DM's solid,
shadow-pushed ones.

## Units and conventions

Source units, z up, degrees. Tick 0.015 s.

## Constants

| Name | Value | Measured? | Meaning |
|---|---|---|---|
| `sv_turbophysics` | 1 (engine default 0) | yes (cvar; set by the shipped `cfg/game.cfg`) | CS:S spec 4.2.4 applies |
| `sv_friction` / `sv_stopspeed` | 8 / 50 (defaults 4 / 100) | yes (cvar; the same `game.cfg`) | the movement spec's friction law (movement.md) |
| push-away cvars (`sv_pushaway_*`) | not settable | yes ("unknown command" over RCON) | not exposed on DoD:S's server; the push-away still acts (below), with constants not readable |
| push limit of a player | none | yes | a walking player shoves no `prop_physics` at all (1 to 400 kg tried) |

## Behavior

### 1. Normal-group props: walls

With turbo physics the player has no shadow (CS:S 4.2.4), so CS:S 4.1's
shoving never happens.

- A `prop_physics` drum of 1, 5, 10, 30, 50, 100, 150, 300, 340, 360 or
  400 kg stopped a bot running at 220 dead on contact (speed 220 to 0 in
  one tick, the bot's centre 35 units from the drum's axis) and the drum
  never moved, over 150 ticks of running into it. There is **no mass
  limit** below which a player pushes a prop by walking: every prop in
  the normal group is a wall, like the world.
- A `prop_physics_multiplayer` that has settled (below) behaves the
  same, at 10, 50, 150 and 1000 kg.

### 2. Which group a multiplayer prop is in

As CS:S 4.2.4 with turbo physics on (measured on spawned drums and on
the map's own props):

- Spawned, it is in the push-away group.
- While its body is awake it stays there; when it falls asleep it moves
  to the **normal group** (physics mode 1, "solid") or the **debris
  group** (mode 2, "non-solid"). Spawned awake and left to settle, a
  mode-1 drum read push-away, then normal once still; a mode-2 drum read
  debris after being walked through.
- A prop that **starts asleep** (spawn flag 1) is never re-sorted until
  something wakes it: it stays in the push-away group indefinitely.
  Woken by the player's push and settled again, it reads normal (a wall)
  from then on.
- dod_avalanche's 138 multiplayer props, read on the running map with
  nothing disturbed: 83 in the normal group (spawn flags 256, without
  "start asleep": they settled at load), 41 in the push-away group
  (spawn flags with bit 1: 257, 265, 9, 1537, 1545), 14 in the debris
  group (the debris flag 4: 260, 261; or mode 2: spawn flags 256 and
  8448). All read physics mode 1 except 10 mode 2.

### 3. Push-away-group props (mode 1): CS:S's soft props

Walking into a mode-1 multiplayer prop still in the push-away group
(spawn flag 1, resting upright, 120 units ahead):

- The player is **not stopped by its shape**: the first contact (the
  bot's centre about 36 units from the drum's axis) does not zero the
  speed. Instead the movement turns into full backing-off: each tick
  v ← 0.88 v − 33 (the ground friction with a 220 wish speed pointing
  away: 220 → 160.6 → 108.3 → 62.3 → 21.8, then moving back), as CS:S
  4.2.3's push-back with its strong input. The bot then jostles at the
  edge (speed 0 to 150, a few units back and forth) while the prop is
  shoved, and a light prop is kicked ahead of it.
- The prop is shoved away from the player (CS:S 4.2.2), less the heavier
  it is: over the 120 ticks after contact the drum moved about 320 units
  at 10 kg (kicked ahead, bouncing, the bot following at up to 220),
  35 units at 50 kg, 1.7 at 150 kg, 0.6 at 400 kg, 0.3 at 1000 kg.
- The jostling ends when the prop goes to sleep: it then drops to the
  normal group and is a wall (a 1000 kg drum: the bot stopped dead 87
  ticks after contact, 33.5 units from its axis). If the player is
  already inside it then, the player continues through (seen once: the
  bot passed a 1000 kg drum).
- Walking at 60 u/s (below CS:S's 75 u/s threshold) into a 10 kg drum:
  the prop barely moved (7 units over 100 ticks, during the moments the
  push-back threw the bot above 75), consistent with CS:S's "solid props
  are not pushed by slow players" rule.

### 4. Push-away-group props (mode 2) and debris

- A mode-2 multiplayer prop in the push-away group is walked straight
  through: no slowing at all (220 throughout), the prop not moved (a
  1000 kg drum).
- The debris group (CS:S): not collided with by players (taken from
  CS:S; the walk through a debris-group prop was not run separately).

### 5. Consequences for the party's maps

- On a DoD:S map at load, a player meets most crates, sandbags and
  furniture as walls (83 of 138 on dod_avalanche), and the start-asleep
  ones as CS:S's soft props. A walker that admits a spot past a prop
  should count only the push-away and debris ones as passable (spawn
  flag 1, the debris flag, or mode 2), the rest as solid.
- A settled prop given the `Wake` input stayed in the normal group (2
  and 120 ticks later): waking alone doesn't make it soft. Whether a
  bullet or blast that moves it does (CS:S 4.2.4 says a moving prop is
  push-away) was not measured.

## Per-tick order

As CS:S's spec. The group is re-picked after each physics step from the
body's sleep state.

## Edge cases

- A prop spawned overlapping the ground (an upright drum placed with its
  origin on the floor; its origin is 17.9 units above the floor when it
  rests) is pushed out and wakes; it then settles into the normal group.
- The props' mass and mode come from the model's `prop_data` (CS:S
  4.2); a map's or spawner's `physicsmode` loses to it.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| `prop_physics` drum, any mass 1 to 400 kg, 120 ahead | run forward at 220 | contact | speed 0, drum unmoved, bot stays 35 from its axis |
| multiplayer drum, mode 1, settled (normal group) | run forward | contact | as above |
| multiplayer drum, mode 1, spawned asleep, 1000 kg | run forward | first 4 ticks of contact | 160.6, 108.3, 62.3, 21.8 u/s |
| multiplayer drum, mode 1, spawned asleep, 10 kg | run forward | 120 ticks | drum shoved about 320 units ahead |
| multiplayer drum, mode 2, spawned asleep | run forward | through it | 220 u/s throughout |
| multiplayer drum, mode 1, spawned awake | none | settled | collision group normal |

## Open questions

1. The push-away's constants (force, maximum, the speed threshold) are
   not readable on DoD:S's server; the measured shoves fit CS:S's
   behaviour but were not fitted number by number.
2. Whether a retail client predicts the push-back (in CS:S it is the
   client's input; here it acted on a bot with no client, so the server
   applies it at least for bots).
3. Whether a bullet or blast that moves a settled prop puts it back in
   the push-away group while it moves (the `Wake` input alone did not).
