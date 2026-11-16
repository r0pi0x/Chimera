# Team Fortress 2: weapons, part 3 — the nine stock melee weapons, and what is left of the stock loadout (T-W3)

Source basis: public community documentation (the community TF2 wiki's
pages on each stock melee weapon and its general pages "Melee",
"Backstab" and "Critical hits", with their patch histories); the retail
client install read in place: the item schema `scripts/items/items_game.txt`
(items, prefabs), the plain sound scripts and the kill-icon table in
`tf2_misc_dir.vpk` (`scripts/game_sounds_weapons.txt`,
`scripts/game_sounds_player.txt`, `scripts/mod_textures.txt`), and the
sequences, events, bones and attachments of the class animation models
and the melee `c_` models through our dumper; and the nine stock melee
weapon scripts (`scripts/tf_weapon_<name>.ctx`), decrypted in memory by
our keyed helper and read from its output only. From the scripts this
spec records only numbers and field names (data the game exposes); field
names follow specs/tf2/weapon_scripts.md. No game or SDK code was read.
Continues specs/tf2/weapons.md (T-W1) and weapons_more.md (T-W2) and uses
their conventions, constants and crit rules.
Status: draft

## Summary

Every TF2 class carries one stock melee weapon. They are simple: one
swing on attack (the Heavy's Fists also on attack2), a fixed damage with
**no distance modifier and no headshot**, a swing interval, and a short
delay between the start of the swing and the moment the hit is traced
and dealt. Eight of the nine deal 65 a hit every 0.8 s; the Scout's Bat
is faster and weaker (35 every 0.5 s); the Spy's Knife deals 40 but
**backstabs**: a hit from behind kills outright (6 × the victim's current
health, as a crit). Melee weapons roll random crits much more often than
guns (15 % base, up to 60 %), and a crit is ×3. All nine are worth having
in a gun game: as the class's melee for the party's melee finale (the
Knife being the obvious last rung) or as a ladder rung of their own. The
rest of the stock loadout's guns are already specified in weapons.md and
weapons_more.md; the items left (Medi Gun, PDAs, toolbox, disguise kit,
watch, sapper) are not weapons a gun game can use (§7).

## Units and conventions

As weapons.md: Hammer units (inches), Z up, X forward, Y left; degrees;
seconds; server tick 0.015 s; damage in hit points; "wiki" is the
community TF2 wiki as of its 2024–2026 edits. Sequence durations are
(frames − 1) / fps. Animation event times are **fractions of the
sequence** (0 to 1), as our dumper prints them. Every class uses the
same 48 × 48 × 82 collision box (48 × 48 × 62 ducked; classes.md), which
is what a melee swing hits (melee does not use the hitboxes).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| melee_range | 48 | units | swing reach from the eye along the view (wiki "Melee"; Open questions 1) |
| melee_hull | 18 | units | half-size of the box swept along the reach (a 36-unit cube; Open questions 1) |
| smack_delay_default | 0.2 | s | swing start to hit, every stock melee without its own field (wiki "Melee") |
| smack_delay_knife | 0.1 | s | the Knife script's `SmackDelay` (Open questions 2) |
| swing_interval | 0.8 | s | `TimeFireDelay`: eight of the nine |
| swing_interval_bat | 0.5 | s | `TimeFireDelay`: Bat |
| damage | 65 | hp | `Damage`: Shovel, Fire Axe, Bottle, Fists, Wrench, Bonesaw, Kukri |
| damage_bat | 35 | hp | `Damage`: Bat |
| damage_knife | 40 | hp | `Damage`: Knife (non-backstab) |
| crit_mult | 3.0 | – | weapons.md |
| melee_crit_base_chance | 0.15 | – | per swing (wiki "Critical hits") |
| melee_crit_max_chance | 0.60 | – | at ≥ 800 damage in the last 20 s (the same × 4 rule as guns) |
| backstab_mult | 6 | – | backstab damage = 6 × the victim's current health |
| backstab_behind_dot | 0 | – | condition 1 of §4.2 (strictly greater) |
| backstab_facing_dot | 0.5 | – | condition 2 of §4.2 (60° half-angle) |
| backstab_same_way_dot | −0.3 | – | condition 3 of §4.2 (≈ 107.5°) |
| deploy | 0.5 | s | weapons.md §3: every stock weapon |

Script fields as found (each stock melee script, decrypted; the full
mapping is §6):

| Script | `Damage` | `TimeFireDelay` | `SmackDelay` | other numeric |
|---|---|---|---|---|
| `tf_weapon_bat` | 35 | 0.5 | – | `TimeIdle` 5.0 |
| `tf_weapon_shovel` | 65 | 0.8 | – | `TimeIdle` 5.0 |
| `tf_weapon_fireaxe` | 65 | 0.8 | – | `TimeIdle` 5.0 |
| `tf_weapon_bottle` | 65 | 0.8 | – | `TimeIdle` 5.0 |
| `tf_weapon_fists` | 65 | 0.8 | – | `TimeIdle` 5.0; `DontDrop` 1 |
| `tf_weapon_wrench` | 65 | 0.8 | – | `TimeIdle` 5.0; `clip_size` −1 |
| `tf_weapon_bonesaw` | 65 | 0.8 | – | `TimeIdle` 5.0 |
| `tf_weapon_club` | 65 | 0.8 | – | `TimeIdle` 5.0 |
| `tf_weapon_knife` | 40 | 0.8 | 0.1 | `Secondary_SmackDelay` 0.3; `TimeIdle` 5.0 |

None of the scripts has a range, hull or crit field. All have
`MeleeWeapon` 1, `WeaponType` melee, `primary_ammo` and
`secondary_ammo` None, `bucket` 2, `bucket_position` 0, `weight` 1 (the
Knife 2), `ITEM_FLAG_NOITEMPICKUP` 1.

Console variables (defaults from weapons.md):

| Cvar | Default | Effect here |
|---|---|---|
| `tf_weapon_criticals` | 1 | random crits on |
| `tf_weapon_criticals_melee` | 1 | melee random crits follow `tf_weapon_criticals` (Open questions 6 for its other values) |
| `tf_damage_disablespread` | 1 | 1: melee damage is exact; 0: ±jitter as weapons.md §2.3 |

## Behavior

### 1. The nine weapons at a glance

| Class | Item `name`, defindex | `item_class` | Script | Damage | Interval | Smack delay | Crit | `model_player` |
|---|---|---|---|---|---|---|---|---|
| Scout | Bat, `TF_WEAPON_BAT` 0 | `tf_weapon_bat` | `tf_weapon_bat` | 35 | 0.5 | 0.2 | 105 | `models/weapons/c_models/c_bat.mdl` |
| Soldier | Shovel, `TF_WEAPON_SHOVEL` 6 | `tf_weapon_shovel` | `tf_weapon_shovel` | 65 | 0.8 | 0.2 | 195 | `…/c_shovel/c_shovel.mdl` |
| Pyro | Fire Axe, `TF_WEAPON_FIREAXE` 2 | `tf_weapon_fireaxe` | `tf_weapon_fireaxe` | 65 | 0.8 | 0.2 | 195 | `…/c_fireaxe_pyro/c_fireaxe_pyro.mdl` |
| Demoman | Bottle, `TF_WEAPON_BOTTLE` 1 | `tf_weapon_bottle` | `tf_weapon_bottle` | 65 | 0.8 | 0.2 | 195 | `…/c_bottle/c_bottle.mdl` (broken: `c_bottle_broken.mdl`) |
| Heavy | Fists, `TF_WEAPON_FISTS` 5 | `tf_weapon_fists` | `tf_weapon_fists` | 65 | 0.8 | 0.2 | 195 | none (empty string: the arms are the weapon) |
| Engineer | Wrench, `TF_WEAPON_WRENCH` 7 | `tf_weapon_wrench` | `tf_weapon_wrench` | 65 | 0.8 | 0.2 | 195 | `…/c_wrench/c_wrench.mdl` |
| Medic | Bonesaw, `TF_WEAPON_BONESAW` 8 | `tf_weapon_bonesaw` | `tf_weapon_bonesaw` | 65 | 0.8 | 0.2 | 195 | `…/c_bonesaw/c_bonesaw.mdl` |
| Sniper | Kukri, `TF_WEAPON_CLUB` 3 | `tf_weapon_club` | `tf_weapon_club` | 65 | 0.8 | 0.2 | 195 | `…/c_machete/c_machete.mdl` |
| Spy | Knife, `TF_WEAPON_KNIFE` 4 | `tf_weapon_knife` | `tf_weapon_knife` | 40 | 0.8 | 0.1 (script) | backstab only (§4) | `…/c_knife/c_knife.mdl` |

(`…` = `models/weapons/c_models`.) All are `baseitem` 1, slot `melee`,
`attach_to_hands` 1. The "Upgradeable" copies 190 (Bat), 191 (Bottle),
192 (Fire Axe), 193 (Kukri), 194 (Knife), 195 (Fists), 196 (Shovel), 197
(Wrench), 198 (Bonesaw) use the same prefabs and behave the same.

Hits to kill an unhurt class, no crits (for a gun game's balance-free
reference only): 65-damage weapons 2 hits on 125 hp (Scout, Engineer,
Sniper, Spy), 3 on 150–175, 4 on the Soldier's 200, 5 on the Heavy's 300; the Bat 4 on 125, 9 on
300; the Knife 4 on 125 (or one backstab).

### 2. The swing (shared)

Inputs: attack pressed (the Fists also attack2, §5.3), the weapon's next
attack time, its smack delay, damage, range and hull.

1. **Start**: when attack is held and now ≥ next attack time: roll for
   a crit (§3), set next attack time = now + interval (0.5 or 0.8 s;
   the same for a hit or a miss), play the swing animation (§8.1: a
   random `HITCENTER` sequence by weight, or the `SWINGHARD` sequence on
   a crit) and the swing sound (`melee_miss`, or `burst` on a crit).
   Holding attack swings again every interval (automatic).
2. **Smack**: at start + smack delay (0.2 s; the Knife 0.1 s), with
   lag compensation for the attacker: trace from the eye along the
   **current** view (where the player looks at the smack, not at the
   start) to `melee_range`, first a line, then if it hits nothing a box
   of ±`melee_hull` swept the same way (Open questions 1). The first
   enemy player (collision box) or solid it meets is the target.
3. **Hit a player**: damage = the weapon's damage (§1), ×3 if the swing
   was a crit, no distance modifier, no hitgroup multiplier. Play the
   `melee_hit` (flesh) sound on the victim; a crit also plays
   `TFPlayer.CritHit` (§8.3). The damage counts toward the attacker's
   20 s crit history (weapons.md §2.6).
4. **Hit the world**: play `melee_hit_world` at the impact point (and an
   impact decal). No damage.
5. **Miss**: nothing more (the swing sound already played).
6. Switching weapons between start and smack cancels the smack (Open
   questions 3).

A swing that starts during the 0.5 s deploy waits until deploy ends
(deploy works as for guns, weapons.md §3). Melee has no ammo, no clip
and no reload.

With `tf_damage_disablespread` 0 the melee damage gets weapons.md §2.3's
random jitter around a modifier of 1 (melee ignores distance, so t is
0.5 + j): 65 ranges about 55–75. With the default (1) it is exact.

### 3. Crits (shared)

- **Random crits**: one roll per swing, at the swing's start (so the
  crit animation and crit swing sound show before anything is hit; a
  missed crit swing is a wasted crit):

```math
p_{\text{melee}} = 0.15 \times \left(1 + 3\,\min\!\left(\frac{D}{800}, 1\right)\right)
```

  with D the damage the player dealt in the last 20 s: 15 % at 0, 26.25 %
  at 200, 37.5 % at 400, 60 % at ≥ 800 (wiki "Critical hits"; the same
  ×4 growth as guns, from a base of 15 % instead of 2 %).
- Crit damage: 3 × base (Bat 105, Knife 120, the others 195); melee has
  no distance modifier either way. With `tf_weapon_criticals` 0 (or
  `tf_weapon_criticals_melee` 0) there are no random melee crits.
- **The Knife never rolls random crits** (wiki "Knife"): its only crits
  are backstabs (§4). (Open questions 6.)
- The crit "bucket" of weapons.md Open questions 6 applies to melee as
  well and is equally unknown; tests force crits on or off.

### 4. The Spy's Knife: backstab

#### 4.1 Ordinary stab

A Knife hit that is not a backstab is an ordinary melee hit: 40 damage,
0.8 s interval, never a random crit.

#### 4.2 The backstab rule (wiki "Backstab")

All vectors are flattened to the horizontal plane (Z set to 0) and
normalised. Let

- **v** = the victim's view direction (yaw only),
- **a** = the Spy's view direction (yaw only),
- **d** = the direction from the Spy's centre to the victim's centre.

A hit is a backstab when all three hold:

```math
\mathbf{d}\cdot\mathbf{v} > 0,\qquad \mathbf{a}\cdot\mathbf{d} > 0.5,\qquad \mathbf{a}\cdot\mathbf{v} > -0.3
```

1. the Spy is behind the victim (in the half-plane behind the victim's
   facing);
2. the Spy faces the victim (within 60° either side of the line to him);
3. the two face roughly the same way (their views differ by less than
   about 107.5°). This third condition (added 2014-06-18 per the wiki)
   stops "face-stabs" from the side while the victim turns.

Pitch never matters (looking up or down doesn't change the result).

#### 4.3 Backstab damage

- Damage = 6 × the victim's **current** health, dealt as a **crit**
  (crit hit sound and effects). This kills anything that can be killed
  by damage; a backstab on a victim at 125 hp deals 750.
- It ignores the crit chance, and `tf_weapon_criticals` 0 does not stop
  it.
- The kill is credited with the custom kill type "backstab": the kill
  feed shows the `backstab` icon (§8.4) and the victim plays a backstab
  death animation (player_animation.md: `primary_death_backstab` and
  the like).

#### 4.4 Backstab ready (the raised knife)

- While the Knife is out and not swinging, the game checks each tick (on
  the Spy's client and the server) whether the player in front of the
  Spy within reach would be backstabbed now (§4.2). When it becomes true,
  the arms play `knife_backstab_up` (`ACT_BACKSTAB_VM_UP`, 0.5333 s) and
  then loop `knife_backstab_idle`; when it becomes false,
  `knife_backstab_down` (0.5333 s) then the normal idle.
- A swing while ready plays `knife_backstab` (`ACT_MELEE_VM_SWINGHARD`)
  instead of a stab; a swing that turns out to be a backstab plays it
  too.
- Whether the backstab is decided at the swing's start (the ready state)
  or at the smack from the positions then: Open questions 4. Implement
  at the smack, with the start's ready state choosing only the
  animation.

### 5. Per-weapon notes

#### 5.1 Bat (Scout)

The only fast melee: 35 every 0.5 s (70 per second against 81.25 for the
65 / 0.8 weapons). Its draw animation (`b_draw`, 1.3333 s) is a flourish
with four sound events (§8.3); the deploy is still 0.5 s. Third-person
`anim_prefix` `bat`.

#### 5.2 Shovel (Soldier), Fire Axe (Pyro), Kukri (Sniper), Bonesaw (Medic)

Plain 65 / 0.8 melee weapons; they differ only in models, animations and
sounds. The stock Bonesaw has no special effect (the Übercharge-on-hit is
an unlock's, the Ubersaw's). The Fire Axe's third-person `anim_prefix` is
`brandingiron`; the Kukri's item class is the generic `tf_weapon_club`
and its model is the machete.

#### 5.3 Fists (Heavy): left and right

- **Attack** punches with the **left** fist, **attack2** with the
  **right** (the item's `animation_replacement` maps the generic
  `ACT_VM_HITLEFT` / `ACT_VM_HITRIGHT` to `ACT_FISTS_VM_HITLEFT` /
  `_HITRIGHT`, the arms' `f_swing_left` / `f_swing_right`). Both deal 65
  and share one interval: a punch with either button blocks both for
  0.8 s. Holding both alternates the hands (Open questions 5).
- A crit punch plays `f_swing_crit` (an uppercut, `ACT_FISTS_VM_SWINGHARD`)
  whichever button.
- No `c_` model: `model_player` is empty and the arms are the weapon
  (the bone-merge has nothing to attach). The Fists' item style sets the
  arms' skin: `v_skin_red` 2, `v_skin_blu` 3 (and `skin_red` 2,
  `skin_blu` 3 on the player), not the arms' usual 0 / 1. The legacy
  `models/weapons/v_models/v_fist_heavy.mdl` still ships but is unused.
- `DontDrop` 1 in the script (nothing to drop).

#### 5.4 Wrench (Engineer)

- Against players: a plain 65 / 0.8 melee.
- Its other job is hitting the Engineer's own buildings (repair,
  upgrade, ammo), with the `special1` / `special2` sounds
  `Weapon_Wrench.HitBuilding_Success` / `_Failure`. A gun game has no
  buildings: **out** (ignore the building hit; treat a building like the
  world).
- `clip_size` −1 in the script (no clip, nothing to map). Its `c_` model
  is the only stock melee with a `muzzle` attachment (§8.2).
- Its idle (`pdq_idle_tap`) taps the wrench into the hand with
  `Weapon_Wrench.HitHand` (volume 0.1).

#### 5.5 Bottle (Demoman): breaks on a crit

- A plain 65 / 0.8 melee. When a **crit swing hits** (a player, per the
  wiki; Open questions 7 for the world), the bottle **breaks**: the
  weapon model becomes `c_bottle_broken.mdl` (first and third person),
  `Weapon_Bottle.Break` plays, and later hits use the broken hit sounds.
- **Cosmetic only**: damage, interval and crit chance are unchanged.
- It stays broken until the player respawns or resupplies (in a gun
  game: until death or until the rung changes).
- Sound sets: the script names `Weapon_Bottle.HitFlesh` / `.HitWorld`,
  and the sound file also has `Weapon_Bottle.IntactHitFlesh` /
  `.IntactHitWorld` / `.BrokenHitFlesh` / `.BrokenHitWorld`; which the
  game plays when (intact vs broken): Open questions 7. Use the script's
  pair while intact and the `Broken…` pair once broken.

#### 5.6 Knife (Spy)

§4. Draw `knife_draw` flips the knife open (the `c_knife` model has its
own chain `vm_weapon_bone` → `_1` → `_2` → `_3` for the blade, animated
by the arms' sequences through the bone-merge) with four sound events.
`weight` 2 in the script (auto-switch priority; ignored as for guns).

### 6. Mapping the script (weapon_scripts.md, extended to melee)

weapon_scripts.md left melee out; its rules carry over:

| Script field | Our target | Guns |
|---|---|---|
| `Damage` | `[melee.primary] damage` (and `[melee.secondary]` for the Fists) | all nine |
| `TimeFireDelay` | `[melee.primary] hit_refire` and `miss_refire` (TF2 has one interval for hit and miss) | all nine |
| `SmackDelay` | the swing's hit delay (a field our melee model lacks today: Open questions 8); absent ⇒ 0.2 | Knife only has it |
| `Secondary_SmackDelay` | not mapped (the Knife has no secondary attack; Open questions 2) | Knife |
| `MeleeWeapon`, `WeaponType` melee | kind check: the TOML must be a melee weapon | all |
| `SoundData` `melee_miss`, `melee_hit`, `melee_hit_world` | check against `sound_miss`, `sound_hit`, `sound_hit_world` | all |
| `SoundData` `burst` | the crit swing sound (no target yet) | all |
| `SoundData` `special1`, `special2` | Wrench building hits: ignored | Wrench |
| `SoundData` `taunt` | ignored (taunts are out) | Bonesaw |
| `clip_size` −1, `DontDrop`, `TimeIdle`, `anim_prefix`, `bucket*`, `weight`, `printname`, `viewmodel` / `playermodel` (present on the Wrench and Kukri, equal to the schema's `model_player`), `HasTeamSkins_*`, `BuiltRightHanded`, `TextureData` | ignored, as weapon_scripts.md §3 | – |

Range, hull and facing have no script field: they stay the TOML's
(48, 18, and no facing limit beyond the box; Open questions 1).

### 7. The rest of the stock loadout

Every stock **gun** is already specified: Scattergun, Rocket Launcher,
the four Shotguns (9 Engineer primary, 10 Soldier, 11 Heavy, 12 Pyro:
weapons.md §1, §4, §8; they share damage, spread, interval and clip and
differ in arms, animations, draw sound and reload start) and Minigun in
weapons.md; Pistol (22, 23), SMG, Sniper Rifle, Revolver, Grenade and
Stickybomb Launchers, Flame Thrower and Syringe Gun in weapons_more.md.
There is no stock gun left. The remaining stock items are out:

| Item (defindex, `item_class`) | Why it is out |
|---|---|
| Medi Gun (29, `tf_weapon_medigun`) | heals teammates and builds Übercharge; deals no damage |
| Construction PDA (25, `tf_weapon_pda_engineer_build`) | a menu for placing buildings; no attack |
| Destruction PDA (26, `tf_weapon_pda_engineer_destroy`) | a menu for destroying one's buildings; no attack |
| Toolbox (28, `tf_weapon_builder`) | carries a building being placed; no attack |
| Disguise Kit (27, `tf_weapon_pda_spy`) | a disguise menu; no attack |
| Invisibility Watch (30, `tf_weapon_invis`) | cloaking; no attack |
| Sapper (735, `tf_weapon_builder`) | disables enemy buildings only; no player damage |

### 8. Assets (from the install)

#### 8.1 First-person sequences

From `models/weapons/c_models/c_<class>_animations.mdl` (included by
`c_<class>_arms.mdl`; install.md §4.3). All 30 fps unless shown; "w" is
the sequence weight used when several share an activity (a random pick
in proportion); "loop" = the looping flag.

| Class / weapon | Draw | Idle | Swing (`ACT_MELEE_VM_HITCENTER`) | Crit (`ACT_MELEE_VM_SWINGHARD`) | Other |
|---|---|---|---|---|---|
| Scout / Bat | `b_draw` 1.3333 | `b_idle` 1.0 (no loop flag) | `b_swing_a` w2, `b_swing_b` w1, 0.7667 each | `b_swing_c` 0.7667 | `b_throw` 0.4333 (`ACT_MELEE_VM_throw`, an unlock's) |
| Soldier / Shovel | `s_draw` 0.8333 | `s_idle` 1.6667 loop | `s_swing_a`, `s_swing_b` w1, 0.8333 | `s_swing_c` 0.8333 | – |
| Pyro / Fire Axe | `fa_draw` 1.6667 | `fa_idle` 1.8333 loop | `fa_swing_a` w2, `fa_swing_b` w1, 1.4 | `fa_swing_c` 1.4 | – |
| Demoman / Bottle | `b_draw` 0.9 | `b_idle` 1.6667 loop | `b_swing_a`, `b_swing_b` w1, 0.8 | `b_swing_c` 0.8 | – |
| Heavy / Fists | `f_draw` 0.9667 (`ACT_FISTS_VM_DRAW`) | `f_idle` 1.3333 (`ACT_FISTS_VM_IDLE`, no loop flag) | `f_swing_left` (`ACT_FISTS_VM_HITLEFT`), `f_swing_right` (`_HITRIGHT`): 21 frames at 25 fps, 0.8 | `f_swing_crit` (`ACT_FISTS_VM_SWINGHARD`) 30 at 25 fps, 1.16 | – |
| Engineer / Wrench | `pdq_draw` 1.0 | `pdq_idle_tap` 1.3333 loop | `pdq_swing_a`, `pdq_swing_b` w1, 1.0 | `pdq_swing_c` 1.0 | – |
| Medic / Bonesaw | `bs_draw` 1.0 | `bs_idle` 2.0 loop (31 frames at 15 fps) | `bs_swing_a` w2, `bs_swing_b` w1, 1.1333 | `bs_swing_c` 1.1333 | – |
| Sniper / Kukri | `m_draw` 1.6667 | `m_idle` 1.9667 loop | `m_swing_a` w2, `m_swing_b` w1, 1.0 | `m_swing_c` 1.0 | – |
| Spy / Knife | `knife_draw` 1.0 | `knife_idle` 1.6667 loop | `knife_stab_a` w2, `knife_stab_b`, `knife_stab_c` w1, 0.7667 | `knife_backstab` 0.8333 (backstab and crit) | `knife_backstab_up` 0.5333 (`ACT_BACKSTAB_VM_UP`), `knife_backstab_idle` 1.6667 loop (`ACT_BACKSTAB_VM_IDLE`), `knife_backstab_down` 0.5333 (`ACT_BACKSTAB_VM_DOWN`), `knife_stun` 2.0 (`ACT_MELEE_VM_STUN`, an unlock's) |

- Draw, idle and swing activities are `ACT_MELEE_VM_DRAW`, `…_IDLE`,
  `…_HITCENTER`, `…_SWINGHARD` (Fists: the `ACT_FISTS_VM_*` set, by the
  item's `animation_replacement`; the Knife's item maps the generic
  `ACT_VM_HITCENTER` / `ACT_VM_SWINGHARD` to the `ACT_MELEE_VM_` ones).
- The Heavy's file also has a `bg_*` set on the `ACT_MELEE_VM_*`
  activities (`bg_draw`, `bg_idle`, `bg_swing_crit`, and `bg_swing_left`
  / `_right` on `ACT_VM_HITLEFT` / `_HITRIGHT`), an unlock's; the stock
  Fists use `f_*` (by the replacement map; pick by name).
- Every arms file also has `melee_allclass_*` sequences (for all-class
  melee unlocks) and `melee_inspect_*`; not used by the stock items.
- The Spy's file has the same backstab family for two unlocks
  (`eternal_*` on `ACT_ITEM2_*`, `acr_*` on `ACT_ITEM1_*`); pick the
  `knife_*` set.
- The Scout's `b_idle` and the Heavy's `f_idle` have no loop flag
  (Open questions 9): play them looping anyway, as every other idle.
- As for guns, sequence lengths are animation only: deploy 0.5 s,
  interval 0.5 / 0.8 s. Several swing animations are longer than the
  interval (Fire Axe 1.4, Bonesaw 1.1333, Wrench and Kukri 1.0): the
  next swing restarts the animation.

Animation sound events (time as a fraction of the sequence):

| Sequence | Events |
|---|---|
| Scout `b_draw` | 0.025 `Weapon_Bat.Draw`; 0.2 `Weapon_Bat.DrawSwoosh2`; 0.4 `Weapon_Bat.DrawSwoosh1`; 0.5 `Weapon_Bat.DrawCatch` |
| Soldier `s_draw` | 0.04 `Weapon_Shovel.Draw` |
| Pyro `fa_draw` | 0.02 `Weapon_FireAxe.Draw` |
| Demoman `b_draw` | 0.037 `Weapon_Bottle.Draw` |
| Heavy `f_draw` | 0.0345 `Weapon_Fist.Draw`; 0.1724 `Weapon_Fist.HitHand` |
| Engineer `pdq_draw` | 0.1 `Weapon_Wrench.Draw` and `Weapon_Wrench.Draw2`; 0.6667 `Weapon_Wrench.HitHand` |
| Engineer `pdq_idle_tap` | 0.525 `Weapon_Wrench.HitHand` |
| Medic `bs_draw` | 0.0333 `Weapon_Machete.Draw` (the Kukri's draw sound; the Bonesaw has no draw entry of its own) |
| Sniper `m_draw` | 0.02 `Weapon_Machete.Draw` |
| Spy `knife_draw` | 0.0333 `Weapon_Knife.Draw`; 0.3333 `Weapon_Knife.Open1`; 0.5333 `.Open5`; 0.7333 `.Open8` |

No swing sequence carries a sound event: the swing, hit and crit sounds
come from the weapon (§2, §8.3).

#### 8.2 Models

All are bone-merged onto the class arms in first person and onto the
player in third person (install.md §4.2), except the Fists (none).
Each has a single 1-frame `idle` sequence. No stock melee sets
`model_world`. Attachments are on `weapon_bone` unless said.

| Weapon | `model_player` | Bones | Attachments | `min_viewmodel_offset` |
|---|---|---|---|---|
| Bat | `models/weapons/c_models/c_bat.mdl` | `weapon_bone` | `pedestal_0`, `icon_camera` | 10 0 −7 |
| Shovel | `…/c_shovel/c_shovel.mdl` | `weapon_bone`, `vm_weapon_bone` | `pedestal_0`, `icon_camera` | 10 −6 −12 |
| Fire Axe | `…/c_fireaxe_pyro/c_fireaxe_pyro.mdl` | `weapon_bone` | `pedestal_0`, `icon_camera` | 10 0 −9 |
| Bottle | `…/c_bottle/c_bottle.mdl`; broken `…/c_bottle/c_bottle_broken.mdl` (1 bone, no attachments) | `weapon_bone` | `pedestal_0`, `icon_camera` | 10 0 −14 |
| Fists | none | – | – | 10 0 −7 |
| Wrench | `…/c_wrench/c_wrench.mdl` | `weapon_bone`, `c_weapon_stattrack` | `muzzle` (0, 19.92, 1.32), `pedestal_0`, `unusual_0`…`5`, `icon_camera` | 10 −2 −9 |
| Bonesaw | `…/c_bonesaw/c_bonesaw.mdl` | `weapon_bone` | `pedestal_0`, `icon_camera` | 10 0 −12 |
| Kukri | `…/c_machete/c_machete.mdl` | `weapon_bone` | `pedestal_0`, `icon_camera` | 10 0 −6 |
| Knife | `…/c_knife/c_knife.mdl` | `weapon_bone`, `vm_weapon_bone`, `vm_weapon_bone_1`…`_3`, `c_weapon_stattrack` | `unusual_0`…`5` (on `vm_weapon_bone_1`), `pedestal_0`, `icon_camera` | 5 0 −4 |

Festive copies (`*_festivizer.mdl`) and holiday ones (`*_xmas.mdl`)
ship beside most of them; not stock. The legacy `w_` models
(`w_bat.mdl`, `w_bottle.mdl`, `w_fireaxe.mdl`, `w_shovel.mdl`,
`w_wrench.mdl`, `w_bonesaw.mdl`, `w_knife.mdl`; no `w_machete`) and one-
piece `v_` models (`v_bottle_demoman.mdl`, `v_knife_spy.mdl`,
`v_fist_heavy.mdl`) are not referenced by the stock items. A gun game's
dropped or world display, if any, uses the `c_` model (as for guns).

#### 8.3 Sounds (`scripts/game_sounds_weapons.txt`)

Swing sounds on the weapon channel at 64 dB, volume 0.6 (the Fists'
at normal level, volume 1); hit sounds on the static channel at normal
level, volume 1. "rnd" = a random pick of the listed waves. Names are the
scripts' `SoundData` (case-insensitive: the Bonesaw script writes
`Weapon_Bonesaw.*`, the entries are `Weapon_BoneSaw.*`).

| Weapon | Swing (`melee_miss`) | Crit swing (`burst`) | Hit flesh (`melee_hit`) | Hit world (`melee_hit_world`) | Draw (by animation event) |
|---|---|---|---|---|---|
| Bat | `Weapon_Bat.Miss` → `weapons/cbar_miss1.wav` | `Weapon_Bat.MissCrit` → `cbar_miss1_crit.wav` | `Weapon_Bat.HitFlesh` → `bat_hit.wav` | `Weapon_Bat.HitWorld` → rnd `cbar_hit1/2.wav` | `Weapon_Bat.Draw` → `bat_draw.wav` (+ swooshes, catch) |
| Shovel | `Weapon_Shovel.Miss` → `shovel_swing.wav` | `.MissCrit` → `shovel_swing_crit.wav` | `.HitFlesh` → rnd `axe_hit_flesh1/2/3.wav` | `.HitWorld` → rnd `cbar_hit1/2.wav` | `Weapon_Shovel.Draw` → `draw_shovel_soldier.wav` |
| Fire Axe | `Weapon_FireAxe.Miss` → `cbar_miss1.wav` | `.MissCrit` → `cbar_miss1_crit.wav` | `.HitFlesh` → rnd `axe_hit_flesh1/2/3.wav` | `.HitWorld` → rnd `cbar_hit1/2.wav` | `Weapon_FireAxe.Draw` → `draw_melee.wav` |
| Bottle | `Weapon_Bottle.Miss` → `cbar_miss1.wav` | `.MissCrit` → `cbar_miss1_crit.wav` | `.HitFlesh` → rnd `bottle_hit_flesh1/2/3.wav` (also `.IntactHitFlesh` → rnd `bottle_intact_hit_flesh1/2/3.wav`, `.BrokenHitFlesh` → rnd `bottle_broken_hit_flesh1/2/3.wav`) | `.HitWorld` → rnd `bottle_hit1/2/3.wav` (also `.IntactHitWorld` → rnd `bottle_intact_hit_world1/2.wav`, `.BrokenHitWorld` → rnd `bottle_broken_hit_world1/2/3.wav`) | `Weapon_Bottle.Draw` → `draw_secondary.wav`; break `Weapon_Bottle.Break` → `bottle_break.wav` |
| Fists | `Weapon_Fist.Miss` → rnd `bat_draw_swoosh1/2.wav` | `.MissCrit` → `fist_swing_crit.wav` | `.HitFlesh` → rnd `cbar_hitbod1/2/3.wav` | `.HitWorld` → rnd `fist_hit_world1/2.wav` | `Weapon_Fist.Draw` → `draw_melee.wav` (74 dB, volume 0.5); `.HitHand` → rnd `metal_hit_hand1/2/3.wav` |
| Wrench | `Weapon_Wrench.Miss` → `wrench_swing.wav` | `.MissCrit` → `wrench_swing_crit.wav` | `.HitFlesh` → rnd `cbar_hitbod1/2/3.wav` | `.HitWorld` → `wrench_hit_world.wav` (volume 0.5, pitch 95–105) | `Weapon_Wrench.Draw` → `draw_primary.wav` + `.Draw2` → `draw_wrench_engineer.wav` |
| Bonesaw | `Weapon_BoneSaw.Miss` → `cbar_miss1.wav` | `.MissCrit` → `cbar_miss1_crit.wav` | `.HitFlesh` → rnd `cbar_hitbod1/2/3.wav` | `.HitWorld` → rnd `cbar_hit1/2.wav` | `Weapon_Machete.Draw` (event) |
| Kukri | `Weapon_Machete.Miss` → `machete_swing.wav` | `.MissCrit` → `machete_swing_crit.wav` | `.HitFlesh` → rnd `cbar_hitbod1/2/3.wav` | `.HitWorld` → rnd `cbar_hit1/2.wav` | `Weapon_Machete.Draw` → `draw_machete_sniper.wav` (74 dB, volume 1) |
| Knife | `Weapon_Knife.Miss` → `knife_swing.wav` | `.MissCrit` → `knife_swing_crit.wav` | `.HitFlesh` → rnd `blade_hit1/2/3.wav` | `.HitWorld` → `blade_hitworld.wav` | `Weapon_Knife.Draw` → `draw_melee.wav`; `.Open1`…`.Open8` → `knife_open1…8.wav` |

(All waves under `weapons/`.) The swing sound plays on every swing, hit
or miss; the hit sound adds to it on a hit.

Crit and backstab, shared (`scripts/game_sounds_player.txt`):
`TFPlayer.CritHit` (static channel, 85 dB; rnd `player/crit_hit.wav`,
`crit_hit2.wav` … ) on a crit hit; `TFPlayer.CritPain` (rnd
`player/crit_received1/2/3.wav`) for the victim; `TFPlayer.CritDeath`
(`player/death.wav`, voice channel, 95 dB) for a crit kill. The stock
Knife has **no backstab sound of its own** (the `…Backstab` entry in the
file is an unlock's): a backstab sounds as a crit hit with
`Weapon_Knife.HitFlesh`.

The sound file defines `Weapon_BoneSaw.Miss`, `.MissCrit` and
`.HitWorld` twice with identical contents (harmless; the last wins).

#### 8.4 Kill icons (`scripts/mod_textures.txt`, `HUD/d_images`; cell x, y, w, h)

| Kill-icon name | Cell | Used for |
|---|---|---|
| `bat` | 0, 0, 96, 32 | Bat |
| `shovel` | 0, 192, 96, 32 | Shovel |
| `fireaxe` | 0, 480, 96, 32 | Fire Axe |
| `bottle` | 0, 320, 96, 32 | Bottle (intact or broken) |
| `fists` | 191, 446, 64, 32 | Fists |
| `wrench` | 96, 96, 96, 32 | Wrench |
| `bonesaw` | 96, 128, 96, 32 | Bonesaw |
| `club` | 0, 160, 96, 32 | Kukri |
| `knife` | 96, 0, 96, 32 | Knife, ordinary stab |
| `backstab` | 116, 384, 48, 32 | Knife, backstab kill |

Drawn as hud.md §3.6; a crit kill gets the `crit` cell (192, 409, 64, 34)
behind the icon. Whether a backstab kill also gets the crit background:
Open questions 10. The stock items have no `item_iconname`; the icon is
the weapon's (and for a backstab the custom kill's).

### 9. Use in a gun game

Every stock melee fits two places (docs/DESIGN.md, "A melee finale"):

- **The melee finale / the class's kit melee.** Each TF2 class's melee
  is the natural melee a TF2 character carries and the natural entry for
  a melee rung of the TF2 pool. Suggested: the **Knife** (with backstab)
  as TF2's `knife_finale` entry, mirroring CS:S's knife (a reward for
  getting behind, an instant kill); the other eight as melee-pool
  entries, one per class.
- **A ladder rung.** The melee band at the end of the power curve.
  Among them the Bat is the fastest and weakest (70 per second, 4 hits
  on 125), the Knife the weakest from the front (50 per second) and the
  strongest from behind; the seven 65 / 0.8 weapons are the same weapon
  in seven skins (pick one or rotate). Party modes stay unbalanced on
  purpose: no tuning.
- Wrench building hits and the Bottle's break are not needed for either
  use; the break is cheap and worth keeping as a visual.
- Random crits: follow the TF2 rules (15–60 %) by default, as parity;
  a host option to turn them off belongs to the crit hook, not here.

## Per-tick order

One melee weapon, each tick (after the player's movement and view update):

1. If a smack is pending and now ≥ its time: rewind other players (lag
   compensation), trace from the eye along the current view (line, then
   box; §2), restore. On a player: for the Knife test §4.2 (backstab ⇒
   6 × current health as a crit), else damage (×3 if the swing was a
   crit); apply as one damage event; play the hit sound (and the crit
   hit sound); add the damage to the crit history; the Bottle breaks on
   a crit hit. On the world: the world hit sound. Clear the pending smack.
2. Read buttons. If attack (Fists: attack or attack2) is held, deploy
   has ended and now ≥ next attack time: roll the crit (not for the
   Knife), next attack time = now + interval, play the swing (or crit /
   backstab) sequence and the swing (or crit swing) sound, schedule the
   smack at now + smack delay.
3. Knife only: if no swing is in progress, test §4.2 against the player
   in reach and play `knife_backstab_up` / `_down` on a change.

The smack scheduled in step 2 runs in step 1 of a later tick: at 0.2 s
that is 14 ticks after the swing (0.2 / 0.015 = 13.33, first tick with
time ≥ start + 0.2); the Knife's 0.1 s is 7 ticks.

## Edge cases

- **Turning during the delay**: the target is whoever the view points at
  when the smack happens, not at the swing's start ("swing then turn"
  hits work).
- **A target walking out of reach during the delay**: a miss, though
  the animation already played.
- **Dying during the delay**: no smack (Open questions 3).
- **Crit swing that misses**: the crit is spent; the next swing rolls
  again. A missed crit Bottle swing does not break the bottle.
- **Backstab on a victim with 1 hp**: 6 damage, a crit kill, the
  backstab icon.
- **Backstab and pitch**: a Spy looking down at a crouched victim from
  behind backstabs; only yaw matters.
- **Backstab from directly behind but turned away**: condition 2 fails
  (the Spy must face the victim), an ordinary stab or a miss.
- **Two players in the box**: the first hit by the trace takes the hit;
  melee hits one player per swing.
- **Teammates**: with friendly fire off a swing that reaches a teammate
  first stops there and does nothing (Open questions 11).
- **Holding attack**: an automatic swing every interval, hit or miss.

## Quirks

- The swing animation is often longer than the interval (Fire Axe 1.4 s
  against 0.8 s); the next swing cuts it. Keep the interval.
- The Medic's Bonesaw draws with the Kukri's sound (`Weapon_Machete.Draw`
  by its animation event). Keep (it is the game's data).
- A crit swing shows itself before the hit (crit animation and sound at
  the start), so players see a crit coming and its wasted misses. Keep.
- The backstab third condition makes some "obvious" stabs from the side
  fail. Keep.

## Test cases

Crits off unless said, `tf_damage_disablespread` 1, victim a standing
125-hp class unless said, attacker facing the victim at 40 units
(centre to centre, within reach) unless said.

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | Bat | swing, hit | 35 damage, 14 ticks after the press (smack 0.2 s) |
| 2 | Shovel / Fire Axe / Bottle / Fists / Wrench / Bonesaw / Kukri | swing, hit | 65 each |
| 3 | Knife, victim facing the Spy | swing, hit | 40; smack 7 ticks after the press (0.1 s) |
| 4 | Bat, hold attack 2 s | – | swings at 0, 0.5, 1.0, 1.5 s (ticks 0, 34, 67, 100) |
| 5 | Shovel, hold attack 2 s | – | swings at 0, 0.8, 1.6 s (ticks 0, 54, 107) |
| 6 | Bat, forced crit | hit | 105; the swing plays `b_swing_c` and `Weapon_Bat.MissCrit` |
| 7 | Any 65-damage melee, forced crit | hit | 195 |
| 8 | Bottle, the gap from the attacker's eye to the victim's box face (along the view) 30 / 47 / 70 | swing | 65 / 65 / miss (70 is beyond 48 + 18; the exact edge: Open questions 1, P1) |
| 9 | Melee, target moves from 40 to 100 units in the 0.2 s delay | swing | miss |
| 10 | Melee, attacker turns 90° onto a target in the 0.2 s delay | swing | hit |
| 11 | Melee crit chance with 0 / 200 / 400 / 800 / 2000 damage in the last 20 s | – | 15 % / 26.25 % / 37.5 % / 60 % / 60 % |
| 12 | Knife, crits on, 1000 front stabs | – | no random crit, every hit 40 |
| 13 | Knife, Spy directly behind (d·v = 1), facing the victim (a·d = 1), same way (a·v = 1) | swing, hit | backstab: 6 × 125 = 750, crit, kill, `backstab` icon |
| 14 | As 13, victim at 37 hp | – | 222 damage, kill |
| 15 | As 13, victim a 300-hp Heavy | – | 1800, kill |
| 16 | Spy behind, victim turned 100° away from the Spy's view (a·v = cos 100° ≈ −0.174) | swing | backstab (−0.174 > −0.3) |
| 17 | Spy behind, a·v = cos 110° ≈ −0.342 | swing | no backstab: 40 |
| 18 | Spy beside the victim: d·v = 0 exactly | swing | no backstab (strictly greater than 0) |
| 19 | Spy behind, but the Spy faces 61° off the line to the victim (a·d = 0.485) | swing | no backstab (and likely a miss) |
| 20 | As 13, Spy looking 60° down | – | backstab (pitch ignored) |
| 21 | As 13, `tf_weapon_criticals` 0 | – | still a backstab |
| 22 | Knife out, Spy walks behind a victim | – | `knife_backstab_up` then `knife_backstab_idle`; walking away: `knife_backstab_down` then `knife_idle` |
| 23 | Fists | attack | left punch `f_swing_left`, 65 |
| 24 | Fists | attack2 | right punch `f_swing_right`, 65 |
| 25 | Fists, attack then attack2 0.3 s later | – | the second punch waits until 0.8 s |
| 26 | Fists, forced crit | attack2 | `f_swing_crit`, 195 |
| 27 | Bottle, forced crit | hit a player | 195; the model becomes `c_bottle_broken.mdl`, `Weapon_Bottle.Break` plays; the next normal hit is 65 with the broken hit sound |
| 28 | Bottle broken | die and respawn | intact again |
| 29 | Bottle, forced crit | miss | not broken |
| 30 | Any melee | switch to it, hold attack | first swing 0.5 s after the switch (34 ticks) |
| 31 | Any melee | swing at a wall at 30 units | the `melee_hit_world` sound, no damage |
| 32 | Any melee | swing at nothing | only the `melee_miss` sound |
| 33 | Script mapping, synthetic script `Damage` 50, `TimeFireDelay` 0.6, `SmackDelay` 0.15, `MeleeWeapon` 1 | apply to a melee TOML | damage 50, hit and miss refire 0.6, smack delay 0.15 |
| 34 | Script mapping, a melee TOML and a synthetic script without `SmackDelay` | apply | smack delay 0.2 |
| 35 | Script mapping, a gun TOML and a script with `MeleeWeapon` 1 | apply | nothing applied; a kind-mismatch log |
| 36 | `tf_damage_disablespread` 0, Shovel, 1000 hits | – | damage within about 55–75, mean ≈ 65 |

Probe checks (dedicated server, bots; never port 27015/27030):

| # | How | Expected |
|---|---|---|
| P1 | A bot at measured distances in front: 40, 48, 60, 66, 70 (centre to centre and face to face) | the reach and box size (Open questions 1) |
| P2 | Log the tick of the swing and of the damage, every stock melee | 0.2 s; the Knife 0.1 s (Open questions 2) |
| P3 | Spy at set yaw offsets behind a bot | the three conditions of §4.2 and their edges |
| P4 | Heavy pressing attack2 only; both buttons held | right punches; alternation |

## Open questions

1. **Reach and box.** 48 units and a ±18 box are the wiki's numbers; the
   scripts have no such field. Not settled: whether a line trace comes
   first, whether the box result is refined (as HL2:DM's), whether reach
   is measured from the eye to the target's box surface (so a target's
   centre can be 48 + 24 away), and whether there is a facing limit. P1.
2. **Smack delay.** The wiki gives 0.2 s for melee generally. The
   Knife's script sets `SmackDelay` 0.1 (and `Secondary_SmackDelay` 0.3,
   for an attack the Knife doesn't have); the others have none, so 0.2 is
   assumed to be the game's default. Whether the Knife's stab is really
   0.1 s, and whether a backstab is instant: P2.
3. **Cancelling a pending smack** by switching weapons or dying: assumed
   to cancel. Check on the probe.
4. **When the backstab is decided**: at the swing's start (the raised
   knife) or at the smack (positions 0.1 s later). Community descriptions
   mention both "ready" stabs that fail when the victim turns and stabs
   that land; P3 with a turning bot.
5. **Fists with both buttons held**: assumed to alternate left and right
   every 0.8 s. Also whether the crit uppercut can be either hand.
6. **Knife crits and `tf_weapon_criticals_melee` values.** That the
   stock Knife never random-crits is the wiki's statement; the prefab's
   tags differ between weapons (`can_deal_critical_damage` on the Bat,
   Wrench and Bonesaw only) and so don't decide it. The cvar's values
   besides 1 (0 = none, 2 = always?) are not in our notes: read its help
   text on the Windows client.
7. **Bottle**: whether a crit hit on the world breaks it, and when the
   game plays the `Intact…` / `Broken…` sound entries versus the plain
   `HitFlesh` / `HitWorld` the script names.
8. **Our melee model** has range, hull, damage and refires but no hit
   delay (HL2:DM and CS:S melee hit at once). TF2 needs a per-swing
   delay field (and the smack done later, with the view then); a
   `tf2:` hook or a Chimera melee field. Implementation choice.
9. **Loop flags**: `b_idle` (Scout) and `f_idle` (Heavy) are not flagged
   looping, unlike every other idle. Probably the game restarts idles
   itself; assumed looping.
10. **Kill feed for a backstab**: whether the `backstab` icon is drawn
    with the crit background (the hit is a crit). Check a backstab kill
    on the client.
11. **Teammates in the swing**: whether a melee trace stops at a
    teammate (blocking the hit on an enemy behind him) or passes through
    as bullets and rockets do (weapons.md §2.7). P1 with a teammate bot.
12. **Melee knockback**: melee damage pushes like any damage
    (weapons.md §5.4, scale 6, unknown base); not measured.
