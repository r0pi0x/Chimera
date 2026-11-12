# Weapon data

Weapons are data loaded at run time (docs/plans/active/engine-split.md,
"Data-driven first"): a TOML document per weapon, parsed and built by
`chimera_sim`'s `weapon::data` into the weapon parts (README, "Weapons and
items"). Behaviour data can't express is Rust, registered by a game under a
name the data asks for (a hook). A game's weapons, another game's, or new
ones become mixable: an app loads any documents on top of the games it runs,
with no Rust (`App::add_weapon_data`).

HL2:DM's are `games/hl2dm/data/weapons/*.toml`, the spec's numbers
(specs/hl2dm/weapons.md); its install's weapon scripts are plain text
beside the encrypted ones, and `games/hl2dm/tests/it/heavy/scripts.rs`
checks the data against them and the draw and reload times against the
view models (`hl2dm::game::scripts`, `source_engine::weapon_script`).

CS:S's weapons are `games/cs_source/data/weapons/*.toml`: our own numbers
(the spec's script tables and the measured values, specs/cs_source/weapons.md),
not files from the game. The game's weapon scripts are encrypted in the
install and the key isn't ours to ship, so nothing is read from them; the
data is checked against the spec's tables instead
(`games/cs_source/tests/it/cs_guns.rs`) and its model paths and sound entries
against the install (`heavy::map_de_dust2`). The files are built into the
game (`weapons::DATA`; a unit test fails for a file missing there) and parsed
when the game starts.

## Documents

A document holds a weapon (a top-level `id`), ammo types (`[ammo."<id>"]`
tables), or both. IDs are namespaced (`game:weapon_name`); `buy <name>` and
`give` also find a weapon by the part after the namespace, with or without
`weapon_`.

```toml
id = "cs_source:weapon_ak47"
base = "cs_source:gun"          # merged under this entry's tables
slot = 0                        # selection slot, 0 = first
draw = 1.0                      # s until the first attack
max_speed = 221.0               # length units/s while held
weight = 25                     # auto-switch priority (SwitchWeight)
class = "rifle"                 # an automatic gun game's class (GunClass:
#   pistol, smg, shotgun, carbine, rifle, heavy (a machine gun), marksman,
#   sniper, heavy_pistol, launcher, flamethrower; grenade: a thrown one that
#   kills): its game lends it to a ladder (minigame::auto), its band on the
#   power curve (chimera/sim/data/power_curve.toml); none: not in one;
#   not inherited from `base`

[buy]                           # economy::Prices
price = 2500
team = 1                        # only this team buys it
bot_weight = 8.0                # bots' liking when buying
# starting_reserve = 120        # reserve when given at spawn, ammo sold

[trigger]
automatic = true
cycle = 0.1                     # s between shots
# timing = "carry_over"         # or "set", "accumulate" (from the base here)
# per_press = 3                 # most shots per pull (0: no limit)
# latency = 3                   # updates from entering the firing state to
#   the first round (0: that update; CA's view-model chain: 3, primary
#   shotguns 2)
# postfire = 0.15               # s of a closing part after a pull's last
#   cycle (with `latency`): reloads and an automatic's next round wait
#   for it, a semi-automatic's press cuts it short (CA's `postfire`)

[magazine]
clip = 30
ammo = "cs_source:762mm"        # an ammo type: reserve max, box, penetration
reload = 2.4324                 # s (a shell's, with `shells`)
# shells = { start = 0.5, insert = 0.45 }   # one round at a time
#   (+ finish = 0.9: a closing part, back_to_back = true: no tick
#   between rounds; Combat Arms' shotguns; insert_first = true: each
#   round in as its insert starts, attacks waiting for each part, a held
#   attack ending the reload: HL2:DM's shotgun)
# reload_while_held = false
# auto_reload = false           # an empty clip reloads, held or not (CA)

[hitscan]
range = 8192.0
# pellets = 1, punch_scale = 2.0 (from the base)
# spread = { disc = { inaccuracy = 0.0, spread = 0.0 } } or { template = 0.05 }
#   or { cone = 0.04362 } (Source's generic HL2 cone: HL2:DM's guns)
# tracer_every = 2              # a tracer every Nth bullet by one count over
#   every bullet a network server fires: it flags each round it sends
#   other clients, who draw a tracer on every bullet of a flagged one
#   (HL2:DM; 0: none, each game counts its own)
# redrawn_through_characters = true   # other clients trace its rounds
#   again through every player (HL2:DM)
# odd_box = 3.0                # a call of several bullets traces every
#   odd-numbered one as a box of this half size (HL2:DM)
# without [damage] the frame traces nothing: a game's hook delivers each
# round's `Fired` event (Combat Arms)

[damage]
amount = 36.0
falloff = 0.98                  # x falloff^(distance / falloff_step)
armor_ratio = 1.55
# falloff_step, hitgroups { head, chest, stomach, arm, leg }, impulse,
# quantum (from the base)

[penetration]
objects = 2                     # power and reach: the ammo's

[alt]                           # attack2: one of
kind = "scope"                  # fov = [..], max_speed, unzoom_after_shot,
                                # overlay, step, sound
# kind = "silencer", time = 2.0
# kind = "burst", interval, refire, count, step, texts = [per mode],
#   once_per_press, step_blocks_primary (CA: one burst a pull, the
#   toggle's lockout)
# kind = "modes", key (a named action that steps them, once a press;
#   none: attack2), step, step_blocks_primary, burst = { count, interval
#   (0: the trigger's cycle), refire, from_last, once_per_press },
#   automatic (a continuous-fire mode, `AutoMode`): modes in that order
#   after the trigger's own (Combat Arms' fire-mode key)
# kind = "shot", pellets, rounds, cycle: attack2 fires a bigger shot
#   from the clip (`AltShot`: HL2:DM's shotgun's double shot, 12 pellets
#   for 2 shells; fewer rounds: a normal shot)

[sounds]                        # entries of the game's sound scripts
fire = "Weapon_AK47.Single"
empty = "Default.ClipEmpty_Rifle"
reload = [[0.35, "Weapon_AK47.Clipout"], [1.54, "Weapon_AK47.Clipin"]]
draw = [[0.3667, "Weapon_AK47.BoltPull"]]
# fire_alt, deploy, shot, finish ([s, entry]); modes ([mode, s, entry])

[models]                        # the game's model paths
world = "models/weapons/w_rif_ak47.mdl"
view = "models/weapons/v_rif_ak47.mdl"
# world_alt (silenced), view_right_handed, view_skins (textures by the
# model's texture index, for models that don't name theirs: LithTech's),
# view_offset ([x, y, z]: the view model's origin in the eye's space, in
# the game's own units and axes)

[hud]
name = "Cstrike_WPNHUD_AK47"    # string token
icon = "d_ak47"                 # kill icon

[anim]                          # view-model idling
idle_after = 1.9                # s after an attack
idle_interval = 20.0            # s between idles
# idle_after_draw = true        # the draw counts as an attack (the knife)
# [anim.sequences]             # the view model's sequences by action
# draw = ["select"]             # (weapon::view_anim::ACTIONS: draw, idle,
# fire = ["fire"]               # fire, fire_last, reload, reload_start,
#                               # reload_loop, reload_end, zoom_in,
#                               # zoom_out); without it the game's
#                               # view_anim::ViewAnimRules choose

[hooks."cs_source:recoil"]      # a game's behaviour, with its parameters
standing = { up = 1.0, up_step = 0.175, side = 0.375, side_step = 0.0375, up_cap = 5.75, side_cap = 1.75 }
# ...
```

Melee weapons give `[melee.primary]` and `[melee.secondary]` swings (range,
hull, hull_reach, facing_cos, refine (a box hit re-traced to twice its
distance and its corners: HL2:DM's), damage, follow_up = [window, damage], backstab,
hitgroups, hit/miss refires, force per damage point, armor_ratio, quantum,
sounds); `undroppable = true` and `pickup_team = 1` are flags; `death_drop
= 3` marks a weapon the dead drop beside their best gun (`DeathDrop`: of
those carried, the highest goes; CS:S's grenades); `kept_on_drop = true`
one the drop key leaves in hand (`KeptOnDrop`; CS:S's grenades);
`[lag_compensation]` (`fired_within`, `reach`, `view_cos`) narrows whom its
shots rewind (`lagcomp::RewindChoice`, HL2:DM's; without it every other
player). A table left
out means the part isn't there (no `[sounds]`: no `WeaponSounds`).

Ammo types:

```toml
units = { length = 0.0254 }     # for the ammo in this document

[ammo."cs_source:762mm"]
power = 39.0                    # penetration budget, length units
max_distance = inf              # not passed from this far
max = 90                        # most rounds carried
box_price = 0
box_rounds = 30                 # none: no ammo for sale
hud_icon = "ammo_762"
```

## Rules

- **Units.** `units = { length, damage }`: metres per length unit (Source's
  inches: 0.0254) and health per damage point (hit points of 100: 0.01);
  both default to 1. Speeds are length/s, impulses mass x length/s, a swing's
  force per damage point.
- **Bases.** `base` names an entry (this set's or one loaded before);
  tables merge key by key, everything else replaces. `abstract = true`
  entries are templates (not inherited, never registered). CS:S's:
  `cs_source:base` (units) and `cs_source:gun` (the shared trigger timing,
  hitgroups, falloff step, impulse, damage quantum, leg slowdown).
- **Hooks.** `[hooks."<name>"]` asks for behaviour a game registered with
  `register_weapon_hook::<Params>(name, fn)`; the table is read as `Params`.
  The hook runs after the data's parts are on the weapon. CS:S's:
  `cs_source:accuracy` (the accuracy penalty: `Spread`, `Inaccuracy*`,
  `RecoveryTime*`, `alt` keys), `cs_source:recoil` (kick sets),
  `cs_source:hit_slowdown` (`body`, `legs`), `cs_source:alternate_hands`
  (the Elites), `cs_source:grenade` (`effect` he/flash/smoke, `max`,
  `bounce_sound`), `cs_source:c4`. Combat Arms' (games/combat_arms/src/game/weapons.rs):
  `combat_arms:damage` (damage per ray, roll, range bands, server
  spacing), `combat_arms:spread` (cones and the meters' values),
  `combat_arms:recoil` (the view kick and its return), `combat_arms:handling`
  (`move_penalty`, the carrier's `HeldMovePenalty`), `combat_arms:grenade`
  (games/combat_arms/src/game/grenades.rs: counts, pre-fire, release and
  post-fire times, perturb, fuse, speed, hit points, the area damage and
  its type, the over-time sphere; data/weapons/m67.toml), `combat_arms:zoom`
  (`zoom_time`, `sway`, `reload_after_fire`, `zoomed_move_penalty`:
  game/weapons/zoom.rs; the scope is the fitted sight's),
  `combat_arms:projectile` (an impact projectile a weapon fires: speed, gravity,
  lifetime, box, blast, the lasting sphere), `combat_arms:flame` (the
  flamethrower's fire keys), `combat_arms:device` (a thrown or placed mine,
  claymore, turret, kit, strike: `kind` and its tables), `combat_arms:bolt`
  (a crossbow's), `combat_arms:controller` (a detonator), `combat_arms:knife`
  (melee: reach, the two buttons' swings and hit keys; games/combat_arms/src/
  game/special/ and inventory.rs; data/weapons/law.toml, m32.toml,
  flamethrower.toml, m16a1_mine.toml and the others),
  `combat_arms:spin` (a minigun's `prefire`, `postfire`, `loop_len`,
  `reversible`), `combat_arms:alt_fire` (attack2's volley: `rounds`,
  `interval`, `cycle`; both game/weapons/classes.rs) and
  `combat_arms:attachments` (game/attachments.rs: `accepts`, the parts it
  takes as names or `{ name, variant }`; `defaults = { muzzle, sight,
  magazine, bullet, mech_sight }`; `default_sight_visible`,
  `fire_sound_radius`, `silenced_radius = [per silencer level]`). The
  parts themselves are records in games/combat_arms/data/attachments/
  (`[[mod]]` tables, `{class}` in a name expanded per `classes`), not
  weapon documents. HL2:DM's (games/hl2dm/src/game/weapons/):
  `hl2dm:inventory` (the script's `position`, `default_clip`,
  `autoswitchto`, `autoswitchfrom`; the holstered auto-reload),
  `hl2dm:pistol` (the accuracy ramp's `penalty_per_shot`, `penalty_max`,
  `spread = [rest, most]`, the click refire's `click_unlock`,
  `dry_unlock`, `dry`), `hl2dm:punch` (`pitch`, `yaw` ranges in Source
  degrees, `reset`, `alt_pitch`, `alt_yaw`), `hl2dm:machine_gun` (`max`,
  `slide`), `hl2dm:shotgun` (`pump`, `pump_sound`, `dry`) and
  `hl2dm:dry_under_water` (`wait`: the guns that won't fire with the eyes
  under water).
- **Errors.** Unknown keys, an unknown base, ammo type or hook, a loop of
  bases, a duplicate ID and hook parameters the hook can't read are load
  errors naming the document and key. A game's own data failing to load
  panics at start (its tests catch it); `add_weapon_data` returns the error.
- **Replacing.** Loading an ID again replaces the weapon in place (the
  registry index, which the network names weapons by, stays). Server and
  clients must load the same data (as they must run the same build).
- **Order.** Weapons register in document order; a game keeps its order
  stable (CS:S: knife, guns, grenades, C4).

## Loading more

```rust
let docs = WeaponSet::read_dir(Path::new("my_weapons"))?;
let docs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
app.add_weapon_data(&docs)?;   // after the games whose hooks and bases it uses
```

`games/cs_source/tests/it/weapon_data.rs` loads a gun that exists only as
data (games/cs_source/tests/data/weapon_rail.toml), buys it, fires it and
plays it over the network.

## Penetration by material

How bullets pass walls and characters (`PassMaterials`) is a game's
document too, read by `PassMaterials::parse` (`weapon::pass_data`); each
ammo type's power and reach are its `[ammo]` entry's. CS:S's is
`games/cs_source/data/penetration.toml` (measured M13):

```toml
units = { length = 0.0254 }
[[material]]                    # a surface material class (one letter,
class = "W"                     # the install's `gamematerial`)
scale = 2.0                     # material passed per unit of power
damage = 0.6                    # damage carried on, as a factor
[default]                       # classes not listed (default 1.0, 0.5)
scale = 1.0
damage = 0.5
[character]                     # (default: characters stop bullets)
cost = 21.5                     # power one character uses, length units
damage = 0.5
range_scale = 0.5               # range left beyond its exit
[entity]                        # (default: never)
twice_from = 24.0               # an entity (prop, not world) passed this
                                # far or farther scales the damage twice
```

Unknown keys, a class that isn't one letter or is given twice are load
errors.

The shop (items that aren't weapons, the buy menu, the spawn kit) is
data too: docs/economy-data.md. What isn't data yet: the accuracy and
recoil models themselves (hooks), and the map loader loads held and view
models for CS:S's own set only (docs/tech-debt.md).
