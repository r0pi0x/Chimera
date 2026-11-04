# Half-Life 2: Deathmatch: projectiles, explosives and blast damage (H4)

Source basis: the public Source SDK 2013 multiplayer game code — the
HL2:DM SMG1, AR2, RPG, crossbow, frag and S.L.A.M. weapons, the HL2
contact grenade, frag grenade, energy ball, satchel charge and trip mine
entities, the shared grenade base, the explosion entity, the generic
radius-damage rule and damage-force helpers, the shared, combat-character,
HL2 and HL2:DM player damage paths, the physics impact-damage tables, the
fly/toss movement and think scheduling, the view-model event dispatch, the
HL2:DM collision rules and the client's sticky-bolt effect — read in place
in a separate spec session. From the game's files on this box (game build
24600434, dedicated server build 24600426, `steam.inf` PatchVersion
10889068 in both, checked): the plain weapon scripts in
`hl2mp/hl2mp_pak_dir.vpk`, `hl2mp/cfg/skill.cfg`, Half-Life 2's loose
`scripts/surfaceproperties.txt`, the sequence lengths and event cycles of
`v_grenade.mdl`, `v_irifle.mdl`, `v_rpg.mdl`, `v_crossbow.mdl` (Half-Life
2's misc VPK) and `v_slam.mdl` (HL2:DM's VPK), and the `.phy` masses of
`w_grenade.mdl` and `w_slam.mdl`, read for numbers only; the dedicated
server's game library was searched for cvar names only. Nothing was
launched.
Status: draft

## Summary

HL2:DM has six things that leave the gun: the SMG1's contact grenade (a
small box under reduced gravity that explodes on the first solid thing it
touches), the AR2's energy ball (a frictionless 10-unit physics sphere that
keeps a constant 1000 units/s, bounces off everything for 4 s, homes a
little after wall bounces and kills any player it touches but its owner),
the RPG's missile (a laser-guided point that rides the line from its
owner's body to the laser dot at 1500 units/s, one at a time, and cannot be
shot down), the crossbow bolt (3500 units/s with a twentieth of gravity,
100 damage times the hitgroup, sticks in walls, glances off at shallow
angles, pins bodies on the clients), the frag (a real 1 kg physics object
with a 2.5 s fuse from the moment it leaves the hand, which rolls rather
than bounces and passes through players), and the S.L.A.M. (thrown
satchels set off by a detonator, or a wall-mounted laser trip mine that
arms after 2 s). Every explosion uses one radius-damage rule: linear
falloff measured to the point where the line from the blast to the
victim's eye enters the victim's box, walls and doors block completely,
props and players in between absorb part, nothing penetrates water. All
damage that names an inflictor also shoves a walking player directly: 5
units/s per point of damage (10 when ducked), at most 1000, at most 250
upwards — this is what grenade and rocket jumps run on, and it also means
bullets and melee push players.

This spec continues [source_deltas.md](source_deltas.md) (H0: 1.4, the
weapon-script table, the ammo table and the cvar table),
[weapons.md](weapons.md) (H3: the weapon frame, timers, pickup, drop, the
bullet and melee rules), [suit.md](suit.md) (H2: the armour rule, blast at
two armour per point), [rules.md](rules.md) (H6: scoring, friendly fire,
the spawn blast grace, death-notice names) and
[movement.md](movement.md) (H1). Source engine behaviour that HL2:DM shares
is in [specs/cs_source/grenades.md](../cs_source/grenades.md) (explosion
temp entity, scorch, hearing effects),
[specs/cs_source/physics_props.md](../cs_source/physics_props.md) (VPhysics
objects, damage impulses) and [specs/source/prop_damage.md](../source/prop_damage.md)
(what blasts do to props); this spec states only what differs or is new.
The gravity gun's handling of these projectiles is H5's.

## Units and conventions

- Source units (inches), Z up, X forward, Y left; pitch positive looks
  **down**; degrees; seconds; server tick 0.015 s (66.67 Hz).
- "Forward", "right", "up" of a player are the basis of its view angles
  (eye angles) unless said otherwise; "right" is −Y when looking along +X.
- **Think timing.** An entity that asks to think d seconds from now thinks
  on tick now + round(d / 0.015) (nearest tick): 0.1 s → 7 ticks, 0.05 s
  → 3, 0.2 s → 13, 0.25 s → 17, 0.3 s → 20, 0.5 s → 33, 1.0 s → 67, 4.0 s
  → 267. Inside a think the time is that tick's time. An entity created
  during a tick (by a player's command or by another entity) does not
  move or think until the next tick, even when it asked to think "now".
- **Weapon timers** are those of weapons.md (absolute times, first tick at
  or after; ⌈d / 0.015⌉ ticks). "Strictly after" comparisons are said
  where they apply.
- **View-model events.** A sequence started on tick s is at cycle
  (n − s) · 0.015 / L on tick n (L = its length, (frames − 1) / fps); an
  event at cycle c fires on the first tick whose cycle is **greater**
  than c, after that tick's weapon frame. Sequence lengths and event
  cycles below come from the view-model files (Chimera reads them at run
  time; the numbers are for tests).
- **Toss integration** (contact grenade, crossbow bolt, RPG missile before
  ignition): each tick, think first, then v_z' = v_z − g·dt, and the move
  is ((v_z + v_z')/2)·dt vertically and v·dt horizontally (exact for
  constant gravity), dt = 0.015, g = `sv_gravity` (600) × the entity's
  gravity factor. Then touches.
- Damage kinds as suit.md: crush 1, bullet 2, blast 64, never-gib 4096,
  dissolve 67108864, blast-surface 134217728.
- Impulse in kg·in/s; masses in kg.
- Steam builds read: game 24600434, dedicated server 24600426.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| knock_per_damage | 5 | (in/s) per hp | player shove per point of damage (box 32×32×72) |
| knock_cap | 1000 | in/s | shove magnitude cap |
| knock_up_cap | 250 | in/s | shove's upward component cap |
| knock_aim_drop | 10 | in | the inflictor's centre is lowered this much before aiming the shove |
| blast_src_lift | 1 | in | radius-damage source raised from the blast point |
| blast_water_depth | 24 | in | a blast counts as under water only if this far above it is water too |
| blast_shield_mass | 350 | kg | a physics object in the line absorbs mass/350 of the damage (all at ≥ 350) |
| blast_shield_other | 0.25 | – | share absorbed by a non-physics, non-world, non-door blocker |
| blast_force_per_hp | 300 | kg·in/s per hp | 75 kg × 4 in/s |
| blast_force_cap | 30000 | kg·in/s | 75 kg × 400 in/s |
| blast_force_jitter | 0.85–1.15 | – | uniform per victim |
| dsp_shock_damage | 30 | hp | blast damage taken (after armour) from which the "shock" DSP plays |
| dsp_ring_distance | 240 | in | inflictor nearer than this: "ear ringing" DSP |
| smg_grenade_damage / radius | 100 / 250 | hp / in | falloff 0.4 hp/in |
| smg_grenade_speed | 1000 | in/s | along view + punch, no thrower velocity |
| smg_grenade_gravity | 400 | in/s² | (factor 2/3 of 600) |
| smg_grenade_box | ±3 | in | |
| smg_grenade_spin | U(−400, 400) | deg/s per axis | cosmetic |
| smg_grenade_check | 3 | ticks | stopped/on-ground check interval (0.05 s) |
| ball_radius | 10 | in | `sk_weapon_ar2_alt_fire_radius` (clamped 1–12) |
| ball_life | 4 | s | `sk_weapon_ar2_alt_fire_duration` (HL2:DM's default; HL2's is 2) |
| ball_mass | 150 | kg | `sk_weapon_ar2_alt_fire_mass` |
| ball_speed | 1000 | in/s | launch and kept after every collision |
| ball_charge | 0.5 | s | from attack2 to launch (strictly after: 34 ticks) |
| ball_player_damage | 1000 | hp | crush + dissolve, on any touch |
| ball_bounce_gap | 0.25 | s | minimum time between counted bounces |
| ball_seek_cos | 0.966 | – | about 15°: seek cone after a bounce |
| ball_seek_ray | 2048, ±256 | in | swept box searched for seek targets |
| ball_seek_kill_radius | 512 | in | `sk_combine_ball_search_radius`, after the ball has hit a player |
| ball_shake | 20, 150, 1, 1250 | amplitude, Hz, s, in | on its explosion |
| rpg_damage / radius | 150 / 300 | hp / in | script damage, radius 2 × damage; falloff 0.5 hp/in |
| rpg_launch | 300 forward + 128 up | in/s | first 0.3 s, full gravity |
| rpg_ignite | 0.3 | s | 20 ticks |
| rpg_speed | 1500 | in/s | after ignition, constant |
| rpg_homing | 0.125 | – | share of the target direction blended in per tick |
| rpg_ride_lead | 256 | in | lead along the laser line when riding it |
| rpg_ride_near | 512 | in | within this of the dot the missile still rides the line |
| rpg_dot_sink | 10 | in | chase point is the dot pushed this far into the surface |
| rpg_grace_check | 128 | in | clear view ahead this far → missile non-solid for 0.3 s |
| rpg_muzzle | 12 fwd, 6 right, −3 up | in | from the eye |
| rpg_health | 100 | hp | never reduced in HL2:DM (see 6.6) |
| bolt_speed_air / water | 3500 / 1500 | in/s | by the shooter's eye being under water |
| bolt_gravity | 30 | in/s² | factor 0.05 |
| bolt_damage | 100 | hp | script; × hitgroup |
| bolt_force | 21000 | kg·in/s | 100 × 300 × 0.7 |
| bolt_glance_cos | 0.5 | – | n·(−v̂) below this and speed > 100: glance off |
| bolt_glance_keep | 0.75 | – | speed kept on a glance; gravity becomes full (600) |
| bolt_stuck_time | 2 | s | a stuck bolt is removed this long after (its client copy stays 30 s) |
| bolt_pin_reach | 128 | in | how far back along the bolt the clients look for a body to pin |
| xbow_refire | 0.75 | s | |
| xbow_zoom | FOV 20 in 0.1 s; back in 0.2 s | | one level |
| xbow_punch | −2 | deg pitch | |
| frag_fuse | 2.5 | s | from the grenade's creation |
| frag_warn | 1.0 | s | after creation: blips speed up |
| frag_blip_slow / fast | 1.0 / 0.3 | s | |
| frag_think | 7 | ticks | 0.1 s |
| frag_damage / radius | 150 / 250 | hp / in | script damage; falloff 0.6 hp/in |
| frag_throw | 1200 × (fwd + 0.1 up) | in/s | + thrower velocity |
| frag_lob | 350 fwd + 50 up | in/s | + thrower velocity |
| frag_roll | 700 along the ground | in/s | + thrower velocity |
| frag_mass | 1 | kg | `w_grenade.phy`, surface `grenade` (friction 0.9, elasticity 0.01) |
| frag_box | ±4 | in | |
| frag_player_bounce | 0.2 | – | speed kept when bouncing off a player |
| frag_player_tap | 0.1 | hp | crush damage of that bounce |
| frag_pickup_fuse | 1.5 | s | fuse restarted by a gravity-gun pickup (H5) |
| frag_rethrow | 0.5 | s | weapon timers after a release event |
| satchel_damage / radius | 150 / 200 | hp / in | at satchel + 16 Z; falloff 0.75 hp/in |
| satchel_throw | 500 | in/s | along the 3D view + thrower velocity |
| satchel_box | ±6, ±6, ±2 | in | |
| satchel_fuse | 0.2 | s | detonator press (or owner death) to explosion: 14 ticks |
| mine_damage / radius | 200 / 200 | hp / in | at mine + 8 along its normal; falloff 1 hp/in |
| mine_attach_reach | 128 | in | |
| mine_mode_reach | 42 | in | surface this close ahead → trip-mine mode |
| mine_offset | 3 | in | off the surface |
| mine_arm | 2.0 | s | strictly after; 139 ticks with the think grid |
| mine_beam | 2048 | in | along the surface normal |
| mine_beam_tolerance | 0.001 × 2048 = 2.048 | in | beam length change that sets it off |
| mine_check | 3 | ticks | beam check interval (0.05 s) |
| mine_delay | 17 | ticks | trip to explosion (0.25 s) |
| slam_per_pickup | 3 | charges | `default_clip2` |
| spawn_blast_grace | 0.5 | s | rules.md; pure blast refused |

Cvars (registered defaults; no shipped cfg sets any of them):
`sk_weapon_ar2_alt_fire_radius` 10, `sk_weapon_ar2_alt_fire_duration` 4,
`sk_weapon_ar2_alt_fire_mass` 150, `sk_combine_ball_search_radius` 512,
`sk_combineball_seek_kill` 0 (no effect on player balls, which always
seek after a kill), `sk_combineball_seek_angle` 15 and
`sk_combineball_guidefactor` 0.5 (only for balls not fired by a weapon),
`sk_npc_dmg_combineball` 15 (NPC balls only), `phys_pushscale` 1,
`old_radiusdamage` 0, `sk_satchel_radius` 0 and `sk_tripmine_radius` 0
(see 8.6 for why the first matters), `sk_plr_dmg_fraggrenade`,
`sk_fraggrenade_radius`, `sk_plr_dmg_smg1_grenade`,
`sk_smg1_grenade_radius`, `sk_plr_dmg_satchel`, `sk_plr_dmg_tripmine`,
`sk_plr_dmg_rpg_round`, `sk_plr_dmg_crossbow` (all 0 and all overwritten
by the weapon at launch: changing them does nothing), `smoke_trail` 1
(contact grenade smoke, cosmetic). All names were found in the shipped
server library.

## Corrections to H0

1. **1.4, primary throw direction.** The frag is not thrown "along the
   view": a tenth is added to the view's forward vector's Z before
   scaling, so the velocity is 1200 · (f_x, f_y, f_z + 0.1) plus the
   thrower's velocity — 120 units/s of extra lift looking level (9.1).
2. **1.4, "the next pull-back 0.5 s later".** The 0.5 s timers are set at
   the release event, but a new pull-back is refused until the throw
   animation has ended and the 1.2667 s draw that follows it has played:
   a tapped primary throw can be repeated after about 2.07 s (138 ticks),
   a lob or roll after about 2.2 s (9.3).
3. **1.4, "a primed grenade is dropped live when its holder dies".** The
   rule is wider and narrower: a live grenade (full 2.5 s fuse, the
   holder's velocity, no throw speed) is dropped whenever the holder dies
   with the frag in hand and attack or attack2 held — pulled back or not,
   during the redraw after a throw, even with no grenades left. A grenade
   whose throw animation has started (button released) but not reached
   its release event when the holder dies is lost (9.5).
4. **1.3, "Fire rates … RPG 1".** The RPG has no fixed refire: it fires
   one missile at a time, cannot fire while its missile flies, and after
   the missile dies plays a 1.6667 s reload animation that must end (with
   no fire button held) before the next shot; its 0.5 s timer never
   decides anything (6.7).
5. **1.1, combine ball as ground** (H1 already narrowed it): HL2:DM's
   collision rules do not know Half-Life 2's ball groups, so a
   player-fired ball collides with every player except its owner and is
   solid to player movement; the hostile-NPC ball group never occurs.
6. **Ammo table, implicit**: the AR2AltFire, RPG_Round, SMG1_Grenade,
   Grenade and slam rows' damage kinds ("dissolve", "burn") are never
   used: every projectile sets its own damage and kind (blast, or crush +
   dissolve for the ball, bullet + never-gib for the bolt).

### Corrections to wave 1

1. **weapons.md §6 "Bullets do not push players", its Open question 6
   and probe P16; §11 "knock-back … NPC wielders only".** Every hit on a
   living, walking player that names an inflictor shoves the player away
   from the inflictor's centre by 5 units/s per point of the damage
   reaching the suit (box 32×32×72; 10 when ducked), capped at 1000 and
   at 250 upwards, provided the hit costs at least one whole health point
   (3). For bullets and melee the inflictor is the shooter: a pistol body
   hit shoves 40 units/s, a .357 body hit 375, a .357 head shot 1000 (cap),
   a crowbar hit 125, a stunstick hit 200.
2. **weapons.md §6 force table, row XBowBolt (H4)**: the bolt does not use
   the ammo type's impulse (17412.50); its force is 0.7 × 300 × 100 =
   21000 kg·in/s along its flight (30 kg prop: Δv 700 in/s) (7.4).
3. **weapons.md §8 pickup table, S.L.A.M.**: new or as an owner, a
   `weapon_slam` gives 3 slam (its `default_clip2`), limit 5.
4. **rules.md and suit.md, spawn grace**: every HL2:DM explosive deals
   exactly "blast" (64) and so is refused for 0.5 s after the victim's
   spawn: contact grenade, frag, RPG, satchel, trip mine, and the
   harmless pop of a shot satchel. The energy ball (crush + dissolve) and
   the bolt (bullet + never-gib) are not.
5. **suit.md, the surface-blast refusal (H4)**: no HL2:DM projectile deals
   blast-surface; only map `env_explosion`s with their surface-only flag
   can.
6. **physics_props.md §5.3 (CS:S)**: "the base damage is the undecayed
   damage". In HL2:DM's radius damage no separate base damage is recorded,
   so a victim's blast force is computed from **its own decayed damage**
   (2.3). Kept as a note for HL2:DM; CS:S is not re-checked here.

## Behavior

### 1. What the projectiles have in common

**Entity classes** (data; the death-notice names are rules.md's):

| Weapon | Projectile class | Inflictor name in `player_death` |
|---|---|---|
| SMG1 attack2 | `grenade_ar2` | `smg1_grenade` |
| AR2 attack2 | `prop_combine_ball` | `combine_ball` |
| RPG | `rpg_missile` (guided by an `env_laserdot`; explodes through an `env_explosion`) | `rpg_missile` |
| crossbow | `crossbow_bolt` | `crossbow_bolt` |
| frag | `npc_grenade_frag` | `grenade_frag` |
| S.L.A.M. | `npc_satchel`, `npc_tripmine` | `slam` |

**Owner.** Each projectile records its player as owner. Movement traces
and physics never collide an entity with its owner, so **no projectile
can touch the player who launched it** (the satchel lets go of its owner
once 5 units clear, which changes nothing since players never collide
with it anyway). Self-damage comes only from blasts.

**Who collides with what** (HL2:DM's rules: projectiles hit everything but
debris, weapons and other projectiles; players and weapons never collide;
everything else collides):

| Projectile | Collision class | Players | Dropped weapons, frags, satchels, trip mines | Other |
|---|---|---|---|---|
| contact grenade | projectile | yes (detonates) | **no** (passes through) | passes through other projectiles |
| energy ball | its own class (unknown to the rules) | yes, except the owner | yes | yes |
| missile | ordinary | yes, except the owner | yes: it explodes on them | yes |
| bolt | ordinary | yes, except the owner | yes (damages them) | yes |
| frag | weapon | **no** (only the bounce of 9.4) | yes | yes |
| satchel, trip mine | weapon | no | yes | yes; bullets and blasts hit them |

**Server only.** All projectiles, their damage and explosions exist only
on the server and reach clients as interpolated entities and temp
entities; none is predicted (12).

### 2. Blast damage (every explosion)

Inputs: blast point B, damage D, radius R, inflictor (the projectile or
mine), attacker (its player, or none), kind exactly blast (64).

1. Falloff k = D / R per unit (R = 0 gives k = 1). Source S = B + (0, 0, 1).
2. "Under water" = B is in water **and** B + (0, 0, 24) is in water (a
   blast in shallow water counts as dry).
3. For every entity whose bounds touch the sphere (S, R) and that takes
   damage (players, props, breakables, grenades, mines…):
   1. under-water blast and the entity's water level 0 → skip; dry blast
      and the entity fully under water (level 3) → skip;
   2. target point T: a player's **eye**; anything else its bounds' centre;
   3. trace S → T with solid, moveable, window, monster, debris and grate
      contents (no hitboxes), ignoring the inflictor;
   4. the trace hit the world (static props included) or a door
      (`prop_door_rotating`, `func_door`, `func_door_rotating`) → **no
      damage**;
   5. it hit another entity (not the target, not something the target
      owns) → re-trace S → T against solid brushes only; blocked → no
      damage; clear → the blocker absorbs a share b: mass / 350 for a
      physics object (≥ 350 kg: no damage), 0.25 for anything else; the
      distance end point becomes T;
   6. else the end point is where the trace stopped: **for a player, the
      point where the line enters its collision box**; a trace starting
      inside the target's box uses S itself (distance 0);
   7. d = |end − S|; damage = (D − k·d)·(1 − b); nothing if D − k·d ≤ 0;
   8. force (2.6); delivery: if the trace stopped on the target it is a
      trace attack with hitgroup generic (×1; blast never meets the head
      multiplier), else plain damage; then damage triggers along the line.

```math
D_{\text{victim}} = \bigl(D - \tfrac{D}{R}\,d\bigr)\,(1-b),\qquad d = |E - S|
```

- No damage reduction by facing; no penetration of walls; teammates and
  the attacker take it like anyone (friendly fire per rules.md: self
  damage is never refused).
- `old_radiusdamage` 1 would switch to "any blocker blocks"; default 0.

#### 2.1 Blast parameters

| Source | D | R | k (hp/in) | B |
|---|---|---|---|---|
| contact grenade | 100 | 250 | 0.4 | the grenade's origin where it touched |
| frag | 150 | 250 | 0.6 | ground probe point + 0.6 × normal (9.6) |
| RPG missile | 150 | 300 | 0.5 | the missile's origin where it touched |
| satchel | 150 | 200 | 0.75 | satchel origin + (0, 0, 16) |
| trip mine | 200 | 200 | 1.0 | mine origin + 8 × its normal |
| shot satchel | 150 | 0 | 1.0 | 8.6: harmless |
| energy ball | – | – | – | its explosion does **no damage** (5.6) |

#### 2.2 Players: armour, shove and hearing

- The victim's suit takes the hit per suit.md with the blast cost (two
  armour per absorbed point): health part h = 0.2·D_victim, armour cost
  1.6·D_victim; if that exceeds the armour A, h = D_victim − A / 2.
- Spawn grace: refused entirely within 0.5 s of the victim's spawn (pure
  blast), rules.md.
- The shove (section 3) uses D_victim (before armour).
- Hearing: after armour, if the health part ≥ 30 the "shock" DSP preset
  (a random one of 35–37) is set for the victim; else if the inflictor's
  origin is within 240 units of the victim's origin, the "ringing" preset
  (32–34). As specs/cs_source/grenades.md 5.5.

#### 2.3 Blast force on physics objects and bodies

A victim's force: |J| = min(300 · D_victim, 30000) · U(0.85, 1.15) ·
`phys_pushscale`, along normalize(T − S), applied at S. Note D_victim is
the victim's own decayed (and shielded) damage. Props then follow
prop_damage.md and physics_props.md. For players the force only feeds the
ragdoll (H7) — the shove of section 3 is what moves living players.

#### 2.4 Chain reactions

| Hit by a blast | Result |
|---|---|
| contact grenade in flight | explodes at once (health 1), attacker its own thrower |
| frag | pushed only (takes no health damage) |
| missile | nothing (6.6) |
| satchel | destroyed with the harmless pop of 8.6 |
| trip mine | sets off: full explosion 17 ticks later, credited to its **placer** |
| energy ball | pushed only |

### 3. Damage shove of players (all damage, not only blasts)

When a living player takes damage that names an inflictor, the attacker
is not a trigger, the player is walking (not on a ladder, not noclip),
and the hit removes at least one whole health point (after armour and the
fractional remainder of suit.md; a hit that only adds to the remainder
shoves nothing):

```math
\begin{aligned}
F &= \min\!\Bigl(D_{in}\cdot 5\cdot\frac{32\cdot32\cdot72}{s_x s_y s_z},\ 1000\Bigr)\\
\hat a &= \operatorname{normalize}\bigl((C_{inflictor} - (0,0,10)) - C_{victim}\bigr)\\
\Delta v &= -F\,\hat a,\qquad \Delta v_z \leftarrow \min(\Delta v_z,\ 250)
\end{aligned}
```

- D_in: the damage arriving at the suit (after falloff, shielding and
  hitgroup, **before** armour). s: the victim's current collision box (32,
  32, 72 standing; 32, 32, 36 ducked → factor 2). C: bounds centres
  (standing player: origin + 36; ducked: + 18; a missile, grenade or
  mine: its origin).
- Δv is added to the velocity at once (server); only the upward part is
  capped (downward and sideways keep up to 1000).
- Inflictors: the projectile for explosions, bolts and balls; **the
  shooter** for bullets and melee (weapons.md).
- A shove with Δv_z > 140 lifts the player off the ground (movement's
  non-jump threshold), which is what grenade and rocket jumps are.

### 4. SMG1 contact grenade (`weapon_smg1` attack2, `grenade_ar2`)

#### 4.1 Firing

- Attack2 with next secondary passed. Refused with no SMG1_Grenade ammo or
  the shooter's eyes under water: dry-fire animation, `Weapon_SMG1.Empty`,
  next secondary and next empty sound = now + 0.5. (With no ammo the
  shared frame's empty rule may answer first: empty sound, next secondary
  + 0.5; same timing.)
- Firing cancels a running SMG1 reload (no ammo moved, weapons.md 7.3).
- Sound `Weapon_SMG1.Double`; `ACT_VM_SECONDARYATTACK`; 1 SMG1_Grenade
  used; next primary = now + 0.5 (34 ticks), next secondary = now + 1.0
  (67 ticks), next empty sound = now + 1.0. The player is not made busy.
- The grenade appears at the eye with velocity 1000 · forward(view angles
  + punch) — **the shooter's velocity is not added** — and random spin
  U(−400, 400) °/s on each axis; damage 100, radius 250; model
  `models/Weapons/ar2_grenade.mdl`; smoke trail.

#### 4.2 Flight

- Toss integration with gravity 400 (2/3 of 600), box ±3, collision class
  projectile. No drag; water does not slow it.
- **Live from the start**: there is no arming distance or delay. It
  explodes:
  - on touching anything solid (world, players other than its owner,
    props, breakables, doors); triggers and water do not count;
  - every 3 ticks (0.05 s) a check: if its speed is exactly 0 or it has
    a ground entity, it explodes;
  - when it takes any damage (health 1): bullets, blasts, the energy
    ball; the explosion is still its thrower's;
  - on a `+use`-style use (not reachable in DM).
- Never against its owner and never against dropped weapons, frags,
  satchels or trip mines (2nd table of 1).

#### 4.3 Explosion

At its current origin: explosion temp entity (fireball scale 2.0, 15
fps, radius and magnitude as data), scorch decal from a 60-unit line along
its flight direction ("SmallScorch" on non-world entities), screen shake
(amplitude 25, 150 Hz, 1 s, radius 750), then blast damage D 100, R 250
(section 2), then removal. Ammo: carry 3, none at spawn (H3).

### 5. AR2 energy ball (`weapon_ar2` attack2, `prop_combine_ball`)

#### 5.1 Charge and launch

- Attack2 with next secondary passed and no charge running. Refused with
  no AR2AltFire ammo or the shooter's eyes under water: dry fire,
  `Weapon_IRifle.Empty`, next secondary and empty sound + 0.5.
- **Charge** (tick 0): `ACT_VM_FIDGET` (1.0 s), `Weapon_CombineGuard.Special1`;
  next primary, next secondary and the launch time = now + 0.5. While
  charging the AR2 can't be put away (switching is refused) or reloaded.
  The water check is not repeated at launch.
- **Launch** on the first tick strictly after the launch time (tick 34),
  in the weapon frame: `ACT_VM_SECONDARYATTACK` (0.7 s), muzzle flash,
  `Weapon_IRifle.Single`; the player is busy until now + 0.7 (tick 81);
  ball at the eye with velocity 1000 · forward(view angles + punch) (no
  shooter velocity); a white screen fade (255, 255, 255, alpha 64, 0.1 s)
  for the shooter; view punch (pitch whole U{−12…−8}, yaw whole U{1, 2},
  0) — up and to the left; 1 AR2AltFire used; next primary = now + 0.5,
  next secondary = next empty sound = now + 1.0 (tick 101).
- If the shooter dies or the AR2 leaves the hand before the launch,
  nothing is fired and no ammo is used.

#### 5.2 The ball as a physics object

- A VPhysics sphere of radius 10 (collision box ±10), surface
  `metal_bouncy` (elasticity 1000, friction 0), mass 150, no gravity, no
  drag, linear damping 0, effectively no rotation. Model
  `models/effects/combineball.mdl`, black sprite trail.
- **Constant speed**: after every collision its velocity becomes the
  post-collision direction times its stored speed (1000; raised if it
  ever arrives faster than stored, e.g. pushed by something). It never
  slows, never stops and never falls.
- It explodes 4 s after launch (tick 267, `sk_weapon_ar2_alt_fire_duration`),
  or at once when a collision's surface is sky.
- It has no bounce limit (a player's ball never "runs out of bounces").

#### 5.3 What it does on contact

- **A player** (any but its owner): the physics impact rule gives a hit
  by this ball a flat **1000 damage, crush + dissolve**, whatever the
  speed or mass — through any armour (with 200 armour health still takes
  800). Inflictor the ball, attacker its owner. Applied after the physics
  step of that tick. Not applied when the ball is the victim's ground
  entity. The victim's ragdoll and the weapon it drops **dissolve**
  (nobody can take that weapon); the death sound is silenced (rules.md).
  The ball itself flies on, deflected off the player.
- **A prop or breakable**: the same rule gives 1000 crush + dissolve
  through the prop's own impact damage (breakable props break, explosive
  barrels go off); unbreakable ones are knocked (150 kg at 1000 in/s).
  Props are not dissolved.
- **Weapons, grenades, mines, other balls**: knocked; no damage.
- **The world** (and static props): a bounce, below.
- No touch counts as a bounce except world surfaces; touching an entity
  never triggers seeking.

#### 5.4 Bounces and seeking

On a collision with the world: an impact effect (`cball_bounce`, radius
16) and `NPC_CombineBall.Impact` every time; then, only if at least 0.25 s
has passed since the last counted bounce:

1. the bounce counts;
2. **seek**: d = post-bounce direction, Q = contact point.
   - Not yet struck a player: candidates are players whose bounds meet a
     box swept from Q to Q + 2048·d with half-extent 256; each must be
     "attractive" (below) and satisfy normalize(C − Q)·d > 0.966 (about
     15°, C its bounds centre); the **nearest** wins.
   - After it has struck a player: candidates are attractive players
     within 512 of the ball (`sk_combine_ball_search_radius`) with
     normalize(C − Q)·d > 0 (in front); the nearest wins.
   - The winner's direction becomes the ball's: velocity =
     normalize(C − Q) × speed.
   - Attractive: a player, alive, not the owner, not a teammate in TDM,
     not frozen (intermission), and a solid-only line from the ball's
     centre to the player's eye is clear or ends on that player.

```math
\vec v' = 1000\cdot\operatorname{normalize}(C - Q)\quad\text{if}\quad \operatorname{normalize}(C-Q)\cdot\hat d > 0.966
```

#### 5.5 Sounds

`NPC_CombineBall.Launch` at launch; `NPC_CombineBall.Impact` per bounce;
`NPC_CombineBall.KillImpact` only for NPCs and server ragdolls (never in
DM); no whizz-by in multiplayer (that code runs only with one player
slot).

#### 5.6 Explosion

At 4 s (or sky): `NPC_CombineBall.Explosion`, screen shake (20, 150 Hz,
1 s, radius 1250), `cball_explode` effect and two expanding rings (radius
10 → 1024 over 0.2 s and 0.5 s); then it stops, becomes non-solid and is
removed 0.5 s later (33 ticks). **The explosion does no damage and pushes
nothing.**

#### 5.7 Gravity gun (H5 owns it; the ball's side)

Caught by a gravity gun: its 4 s life stops, it becomes debris (no
collisions with players), mass 20, owner = the holder (kills now credit
the holder), a holding sound ramps up after 7 s and it explodes (harmless)
8 s after the catch. Released or launched: owner = that player, speed
1500, a fresh 4 s life; a plain drop sends it in a random direction
within ±55° pitch and yaw of the holder's facing at 1500. Punted without
being held: owner = the punter, speed = its new speed, fresh 4 s life.

### 6. RPG (`weapon_rpg`, `rpg_missile`, `env_laserdot`)

#### 6.1 The laser dot

- While the RPG is in hand, out of its draw animation, and the owner has
  rockets or a missile in flight, the server keeps one `env_laserdot`
  owned by the player. It is created (`Weapon_RPG.LaserOn`) the first
  time the weapon frame runs with an activity other than the draw (so the
  dot appears as the 1.1 s draw ends) and when it is missing and rockets
  are carried.
- Each weapon frame of its owner it is placed where a line from the eye
  along the view (no punch, 56755.84 units) stops, masked like a bullet
  but **through windows** (window contents excluded), ignoring the owner;
  it records the surface normal n and, when the line stopped on a
  damageable non-world entity (a player, a prop), that entity as target.
- **There is no way to turn it off in HL2:DM**: attack2 does nothing on
  the RPG (the single-player toggle is not wired). It is removed
  (`Weapon_RPG.LaserOff`) when the RPG is put away, dropped (death), or
  when the owner has no rockets and no missile in flight. It is
  suppressed (kept but off) only while the weapon shows its lowered idle,
  which happens when it has no ammo and no missile.

#### 6.2 Firing

- Attack with next primary passed, refused if a missile of this RPG is
  still alive or the weapon's activity is the reload. No ammo: the shared
  empty rule (`Weapon_SMG1.Empty`).
- Next primary = now + 0.5; 1 RPG_Round used; `ACT_VM_PRIMARYATTACK`
  (1.1333 s); `Weapon_RPG.Single`. No view punch.
- Missile at eye + 12·forward + 6·right − 3·up, angles = the view's,
  velocity 300·forward + (0, 0, 128), box ±4, full gravity, model
  `models/weapons/w_missile_launch.mdl`, damage 150.
- **Grace**: a bullet-masked line from the eye 128 units along the view;
  if it hits nothing the missile is non-solid for its first 0.3 s (it
  passes through anything, players included); if it hits, the missile is
  solid at once.
- While its missile lives the RPG **can't be put away** (all switches,
  `lastinv` and `phys_swap` refused) and won't lower.

#### 6.3 Ignition and flight

- Ticks 1–19: toss integration (gravity 600). Tick 20 (0.3 s): ignition
  — model `models/weapons/w_missile.mdl`, collision box a **point**,
  solid, `Missile.Ignite` (looping until removal), velocity =
  1500·forward(launch angles), smoke trail, a screen fade (255, 225, 205,
  alpha 64, 0.1 s) for the owner; it moves 22.5 along the new velocity
  that same tick. From then on: straight flight without gravity, one
  guidance step every tick (think first, then the move).
- Level launch from a still player: at ignition the missile is 97.5
  forward, 6 right and 9.11 above the eye (12 + 300·0.285 forward; −3 +
  128·0.285 − 300·0.285² up).

#### 6.4 Guidance (every tick after ignition)

1. Pick the nearest dot that is on and owned by the missile's owner (in
   practice the owner's only dot). None → no change this tick (the
   missile keeps its direction: **losing the dot means flying straight**).
2. Aim point A:
   - the dot has a target entity → A = dot − 10·n;
   - else with C = the owner's bounds centre (origin + 36 standing, **not
     the eye**), L = dot − 10·n, ℓ = |L − C|, ê = (L − C)/ℓ: if the
     missile is nearer C than ℓ, or within 512 of L: A = (the point of
     line C→L nearest the missile) + 256·ê — it **rides the beam**; else
     A = L.
3. t̂ = normalize(A − missile), v̂ = current direction; new direction =
   normalize(0.125·t̂ + 0.875·v̂) (if that is near zero length: t̂);
   speed unchanged; the missile's angles follow its direction.

```math
\hat v_{n+1} = \operatorname{normalize}\bigl(0.125\,\hat t + 0.875\,\hat v_n\bigr),\qquad |\vec v| = 1500
```

- Turn rate: at 90° error 8.13° per tick (542 °/s); a turning circle of
  about 158 units at full deflection.
- The dot moves with its owner's view every command; between commands it
  stays.

#### 6.5 Explosion

On touching anything solid (triggers ignored except weapon-class ones:
dropped weapons, frags and mines set it off; never its owner): a 16-unit
line ahead; unless that line hit sky, an `env_explosion` at the missile's
origin with magnitude 150 and radius 300, owner = the missile's player,
inflictor = the missile (no sparks, no dynamic light, no smoke; its
fireball and sound are the explosion entity's, H9). Blast damage with B =
the missile's origin (no ground probe for damage; the explosion's visual
probes 40 down and is drawn 24 off the surface). A missile hitting sky is
removed silently. The RPG is told its missile died (6.7).

#### 6.6 Shooting it down

The missile has 100 health but refuses every damage kind HL2:DM produces
(only an airboat gun or anti-missile defences, absent in DM, can hurt
it): **bullets, bolts, blasts and balls do nothing to it**. Its "shot
down" spiral (1.5 s of random wobble at 1000, then explosion) never
happens in DM.

#### 6.7 Reload rules

- When the missile explodes or is removed, the RPG (if still owned and
  not already showing the reload) plays `ACT_VM_RELOAD` (1.6667 s) when
  the owner has rockets left; no ammo moves (there is no magazine).
- A new missile can be fired only when the activity is no longer the
  reload. The activity returns to idle only in a weapon frame with
  **no fire button held** on a tick strictly after the reload animation's
  end: holding attack through the reload keeps the RPG from firing until
  the button is released for at least one tick.
- Fastest refire after an explosion on tick X: idle on tick X + 112, a
  fresh press fires on tick X + 113 (1.695 s).
- With no rockets left: no reload animation; the dot goes; the weapon
  lowers.

#### 6.8 Death of the owner

The RPG is dropped (H3), its dot is removed: the missile **flies straight
on** and still credits its player.

### 7. Crossbow (`weapon_crossbow`, `crossbow_bolt`)

#### 7.1 Firing and reload

- Fires under water. Clip 1, XBowBolt carry 10; taken new: clip 1 + 4
  reserve (H3).
- Attack with next primary passed and clip 1: a bolt from the eye along
  forward(view + punch) at **3500**, or **1500 if the shooter's eyes are
  under water** (the bolt keeps that speed whatever it flies through);
  clip 0; view punch (−2, 0, 0) (up); `Weapon_Crossbow.Single` and
  `Weapon_Crossbow.BoltFly`; `ACT_VM_PRIMARYATTACK` (0.6667 s); next
  primary and next secondary = now + 0.75.
- **Automatic reload**: on the first weapon frame strictly after the fire
  animation's end (tick 45) the reload starts (`ACT_VM_RELOAD`, 1.8333 s,
  player busy), if reserve > 0; the bolt enters at its end (tick 168).
  Attack held throughout fires again on tick 168: **168 ticks (2.52 s)
  per shot**. Its animation events (cycles 0.509, 0.636, 1.0) only drive
  the glow effects and `Weapon_Crossbow.BoltElectrify`.
- Clip 0 when attack is pressed (no auto reload pending): the reload
  starts instead.
- Draw: 1.0 s (H3); the view-model skin shows the glowing bolt when
  loaded.

#### 7.2 Zoom

- **Attack2 pressed** (the press, not the hold) toggles the zoom, also
  while busy (during the reload): in = FOV 20 over 0.1 s; out = the
  player's default FOV over 0.2 s. One level only.
- Firing and reloading keep the zoom. Putting the crossbow away unzooms.
  The zoom owner rule against the suit zoom is suit.md's.

#### 7.3 The bolt in flight

- Box ±1 (traces with the box), toss integration with gravity 30 (factor
  0.05), model `models/crossbow_bolt.mdl`, a glow sprite. Its moves hit
  **hitboxes** (precise per-bone hits) and **pass through grates**.
- Its angles follow its velocity every 7 ticks.
- It flies until it touches something; no lifetime.

#### 7.4 What a bolt does on touch

- **Something that takes damage** (player, prop, breakable, grenade,
  mine): damage 100, bullet + never-gib, as a trace attack with the
  touch's hitgroup: a player takes 100 × the hitgroup cvar (head **300**,
  everything else 100). Force 21000 along the flight at the touch point
  (props: Δv = 21000 / mass; players: ragdoll force only; the shove of
  section 3 uses 100 or 300). Then:
  - breakable glass: the bolt **keeps flying**;
  - otherwise it stops, `Weapon_Crossbow.BoltHitBody`, and is removed; if
    a line from it 128 units further along its facing hits the world or
    a non-moving entity, a "BoltImpact" effect is sent at that point
    (below).
- **A non-moving solid** (world, static brush entities), not sky:
  `Weapon_Crossbow.BoltHitWorld`, sparks (unless in water). With s its
  speed and c = n·(−v̂):
  - c < 0.5 and s > 100 (incidence within 60° of the normal is
    "steep"; shallower than 30° from the surface glances): **glance** —
    reflect, speed × 0.75, gravity becomes full 600, keeps flying;
  - otherwise **stick**: stop, hidden, removed 2 s later; impact decal and
    a "BoltImpact" effect at the hit point.
- **Anything else** (a moving non-damageable entity, sky): removed (an
  impact decal unless sky).

```math
\vec v' = s\cdot 0.75\cdot\bigl(2c\,\hat n + \hat v\bigr)\quad\text{when } c = \hat n\cdot(-\hat v) < 0.5,\ s > 100
```

#### 7.5 Pinning (client side, cosmetic)

Each client, on a "BoltImpact" effect at point P with direction d: unless
a 16-unit line from P along d hits sky, it looks along P → P − 128·d for
a client ragdoll bone; the first one crossed is pinned to the world at P
by a ball-socket joint (that bone's mass doubled), with
`Weapon_Crossbow.BoltSkewer`; and it shows a bolt model at P − 8·d for 30
s. So a body killed by a bolt in front of a wall within 128 units hangs
from the wall, and a body lying in front of a wall where a later bolt
sticks is pinned too. Each client decides for its own ragdoll copy.

### 8. S.L.A.M. (`weapon_slam`, `npc_satchel`, `npc_tripmine`)

#### 8.1 Modes

The S.L.A.M. has two modes chosen by what is ahead: **trip-mine mode**
when a solid-only line from the eye 42 units along the view hits
something that is not a player (or NPC), else **throw mode**. Each
weapon frame (unless a mode-change animation is still playing) the mode
follows: changing plays a transition (throw → trip mine 1.3333 s; trip
mine → throw 1.6667 s without detonator, 1.0 s with it) during which the
mode does not flip back. A **detonator** is armed while the owner has
live satchels out. Ammo: the "slam" pool (carry 5); a pickup gives 3.
Satchels and trip mines draw on the same pool.

#### 8.2 Trip mine

- Attack in trip-mine mode with ammo, next primary passed: a solid-only
  line 128 units along the view must hit something (not a conveyor) — then
  the attach animation (0.2333 s); the mine is created when it ends (the
  first weapon frame strictly after: tick 16), at the hit point + 3·n,
  facing out along the normal; 1 slam used; then the second attach
  animation (0.6 s) and a redraw (0.8333 s): the next placement after
  about 1.7 s. The line is re-traced at creation, so the mine goes where
  the view then points (it is not attached to the surface entity: a mine
  on a door or prop stays in the air when that moves).
- `npc_tripmine`: model `models/Weapons/w_slam.mdl` (body 3), box ±4, ±4,
  ±2, frozen in place, collision class weapon (players pass through it;
  bullets and blasts hit it), health 1; `TripmineGrenade.Place`.
- **Arming**: its first check 13 ticks after creation, then every 7
  ticks; it arms on the first check strictly after 2.0 s (**tick 139**):
  `TripmineGrenade.Activate`, the beam (a laser sprite, colour
  255, 55, 52, width 0.35, brightness 64) along its normal. The beam
  length λ is where a bullet-masked line from the mine to mine + 2048·n
  stops; if a player is in it at that moment, λ is measured past that
  player.
- **Watching**: the first beam check 7 ticks after arming, then every 3
  ticks: a line from the mine to mine + 2048·n with **solid-only** contents
  (solid, moveable, window, monster, grate); if it hits **any player**
  (its placer included, teammates included) or its length differs from λ
  by more than 2.048 units (a prop moved in or out, a door moved), it
  trips.
- **Tripped or damaged** (any damage at all, armed or not — a bullet, a
  bolt, a blast, a ball): `TripmineGrenade.StopSound`, and 17 ticks later
  the explosion: screen shake (25, 150 Hz, 1 s, radius 750), an
  `env_explosion` at mine + 8·n with damage 200, radius 200, owner = the
  **placer** (whoever set it off), inflictor the mine; then removal.
- Trip mines never expire and survive their placer's death; a placer
  who disconnected leaves mines that kill with no scorer (world kills).

#### 8.3 Satchel throw

- Attack in throw mode with ammo, next primary passed: throw animation
  (0.4667 s; next primary and secondary = its end; the detonator becomes
  armed with the first throw); the satchel leaves when it ends (tick 32):
  at the owner's bounds centre + 18·dir + (0, 0, 24), with dir the 3D
  view direction, velocity = owner velocity + 500·dir, spin 400 °/s yaw;
  `Weapon_SLAM.SatchelThrow`; 1 slam used. If by then a surface is within
  42 units ahead, it is instead set down at centre + 5·dir with speed 1
  along dir. Then the second throw animation (0.8 s) and a redraw (1.0
  s): the next throw after about 2.3 s.
- `npc_satchel`: VPhysics object from `w_slam.phy` (mass 1, surface
  `weapon`), collision class weapon (passes through players), box ±6, ±6,
  ±2, health 1, red strobing glow sprite. It rolls and slides as physics;
  it does not stick to walls (wall-attached satchels never happen in
  HL2:DM: near a wall the weapon is in trip-mine mode). Once it has moved
  less than 1 unit between two of its 7-tick checks it stops thinking.

#### 8.4 Detonator

- Attack2 with next secondary passed, a detonator armed, and the weapon
  showing its throw-mode idle or the detonator-only idle (not during
  throw, transition or draw animations, and **not in trip-mine mode**):
  detonate animation (1.3333 s in throw mode, 0.6667 s detonator-only;
  timers = its end); `Weapon_SLAM.SatchelDetonate`; every live satchel
  whose thrower is this player explodes **0.2 s later** (14 ticks); the
  detonator is disarmed.
- **Satchel explosion**: `env_explosion` at satchel + (0, 0, 16) (world Z,
  whatever its orientation), damage 150, radius 200, owner = the thrower,
  inflictor the satchel; removal.
- With no slam left and no detonator, the weapon throws itself away and
  is removed from the inventory (no S.L.A.M. in the slots afterwards).
  With satchels out and no ammo, it shows the detonator only.

#### 8.5 Owner death and disconnect

- Death: all the dead player's live satchels explode 0.2 s after the
  death (with `Weapon_SLAM.SatchelDetonate` from the body), credited to
  the dead player. Trip mines stay and keep their placer.
- Disconnect: in TDM the leaver is killed first (rules.md), so the
  satchels go off; in DM nothing happens to them: they lie live and can
  only be destroyed by damage.

#### 8.6 Shot satchels

A satchel that takes damage (a bullet, a bolt, a blast, a ball touch) is
**destroyed with a harmless pop**: it runs the generic grenade explosion
(scorch, `BaseGrenade.Explode`, a temp-entity explosion of scale 0) with
radius `sk_satchel_radius` = 0, which reaches nothing in practice (falloff
1 hp/unit and a zero-size sphere). This is how players clear satchels.
When several satchels go off together, one's blast destroys the others
this way but their queued explosions still happen in the same tick (the
removal is deferred), so detonation is unaffected.

### 9. Frag (`weapon_frag`, `npc_grenade_frag`)

#### 9.1 Pull-back and release

- Attack (primary, "throw") or attack2 (secondary, "lob/roll") with
  grenades left and the weapon not redrawing: pull-back animation
  (`ACT_VM_PULLBACK_HIGH` / `_LOW`, 0.25 s); that button's timer becomes
  "never" until the release.
- The pull-back must finish (event at cycle 0.8: tick 14) before a
  release counts. On the first weapon frame after that with the button
  **up** (tick 15 for a tap):
  - primary: `ACT_VM_THROW` (0.55 s, release event at cycle 0.0909: 4
    ticks later);
  - secondary with the **duck button** held at that moment:
    `ACT_VM_SECONDARYATTACK` roll (0.7 s, release at cycle 0.1429: 7
    ticks later);
  - secondary without: `ACT_VM_HAULBACK` lob (0.7 s, release 7 ticks
    later).
- Holding the button keeps the pin pulled indefinitely; **the fuse has
  not started** (no cooking).
- Switching weapons while pulled back puts the grenade away unthrown; no
  ammo used.

#### 9.2 The release event (server)

The grenade is created (9.4), 1 grenade used, next primary and secondary
= now + 0.5, and the weapon is marked for a redraw. Sounds: throw and lob
`common/null.wav` (silent), roll `WeaponFrag.Roll`.

| Kind | Start point | Velocity (+ thrower velocity) | Spin (°/s about local x, y, z) |
|---|---|---|---|
| throw | eye + 18·fwd + 8·right | 1200·(fwd + (0, 0, 0.1)) | (600, U{−1200…1200} whole, 0) |
| lob | eye + 18·fwd + 8·right − (0, 0, 8) | 350·fwd + (0, 0, 50) | (200, U{−600…600} whole, 0) |
| roll | below | 700·g | (0, 0, 720), oriented (0, body yaw, −90) |

- Start points are checked: a box ±6 swept from the eye (roll: from the
  player's bounds centre) to the start point; if it hits, the grenade
  starts where the box stopped. No grenade is ever created inside a wall.
- Roll: P0 = the centre of the player's box bottom + (0, 0, 4); f = the
  horizontal facing; a player-solid line from P0 16 units down; if it
  hits a surface with normal m, f is projected onto that surface (f ←
  m × (f × m), normalised as a direction); start = P0 + 18·f; g = f.

#### 9.3 Redraw and refire

After the release, when the throw/roll/lob animation has finished and
both timers have passed, the draw animation plays (1.2667 s, timers =
its end); only then can a new pull-back start. Tapped throw from a press
on tick 0: throw animation tick 15, grenade tick 19, redraw tick 53, next
press accepted tick 138. Lob or roll: release event tick 22, redraw tick
63, next press tick 148. With no grenades left after the release, no
redraw: the shared empty rules switch to the next best weapon.

#### 9.4 The grenade as a physics object

- `npc_grenade_frag`, model `models/Weapons/w_grenade.mdl`, VPhysics from
  its `.phy`: **mass 1 kg, surface `grenade` (friction 0.9, elasticity
  0.01)**, damping 0; gravity 600 (physics). It barely bounces off the
  world and mostly rolls and slides; physics_props.md's speed caps and
  contact rules apply. CS:S's 45 % bounce rule does **not** apply.
- Collision class weapon: **it passes through players physically**.
  Instead, after each physics step a line from its origin along
  velocity × 0.015 (hitbox, monster and solid contents, only entities the
  weapon class doesn't collide with — players) is checked:
  - starts inside a player: velocity × −0.2 once (until it is out again);
  - hits a player: that player takes **0.1 crush damage** from the
    thrower (an armoured player loses 1 armour; others add 0.1 to the
    remainder; no shove, no hurt event), the velocity is reflected on the
    hit plane and scaled by 0.2, the spin by −0.5.
- It takes no health damage (bullets and blasts only push it). Never
  touches its own thrower.
- Blips and light: `Grenade.Blip` at creation; a red glow sprite
  (`sprites/redglow1.vmt`, scale 0.2) at its "fuse" attachment and a red
  trail (`sprites/bluelaser1.vmt`, width 8 → 1, 0.5 s) for its whole life.

```math
\vec v' = 0.2\,\bigl(\vec v - 2(\vec v\cdot\hat n)\hat n\bigr)
```

#### 9.5 Fuse and blips

The fuse is set at creation: explode at creation + 2.5 s; warn at
creation + 1.0. The grenade thinks on the tick after creation and every 7
ticks after that; at each think, in order: past the fuse (strictly) →
explode; at or past the warn time → warned; past the next blip time
(strictly) → `Grenade.Blip` and next blip = now + 0.3 if warned, else +
1.0. With creation on tick 0 (thinks on ticks 1 + 7k):

| Event | Tick | s |
|---|---|---|
| blip | 0 | 0 |
| blip (warned from here) | 71 | 1.065 |
| blips | 92, 113, 134, 155 | 1.38, 1.695, 2.01, 2.325 |
| explosion | 169 | 2.535 |

From a tapped press: explosion 188 ticks (2.82 s) after the press; from
the release of the button, 173 ticks (2.595 s).

**Dropped on death** (H0 correction 3): if the holder dies with the frag
in hand and attack or attack2 held, a grenade is created as a throw
(start point and spin as the throw) but with the dead holder's velocity
only, full fuse, 1 grenade removed (none left: none removed, still a
grenade). Instead of dropping the frag weapon.

**Gravity gun** (H5): caught or punted, the grenade's thrower becomes
that player, its fuse restarts at 1.5 s with the warning at 0.75 s, it
blips at once and every 0.3 s.

#### 9.6 Explosion

1. A line from origin + (0, 0, 8) to 32 units below that (shot-hull
   contents, ignoring itself); if it starts in solid, from the origin
   instead.
2. If it hit: the grenade moves to hit + 0.6·n.
3. Explosion temp entity (scale R × 0.03 = 7.5, 25 fps, the surface's
   material and normal; water explosion when in water), blast damage (D
   150, R 250, section 2), scorch decal on the probe's surface,
   `BaseGrenade.Explode`, screen shake (25, 150 Hz, 1 s, radius 750);
   invisible and removed next tick.

As CS:S's HE (grenades.md 5.2) apart from the numbers.

### 10. Kill credit

| Projectile | Attacker (scorer) | Changes when |
|---|---|---|
| contact grenade | its shooter | never |
| energy ball | its current owner | a gravity gun catches/punts it (H5) |
| missile | its shooter | never (also after the shooter's death) |
| bolt | its shooter | never |
| frag | its thrower | a gravity gun catches/punts it |
| satchel | its thrower | never |
| trip mine | its placer | never (whoever trips or shoots it) |

- A projectile keeps crediting its player after that player dies (the
  player entity persists); a disconnected player's projectiles credit
  nobody (world kill: −1 frag to the victim, rules.md).
- Friendly fire (rules.md) is judged at the hit with current teams.
- Self kills by own explosives are suicides (rules.md).

### 11. Damage summary against players (100 health)

| Hit | Damage | Kills, armour 0 | Kills, armour 100 |
|---|---|---|---|
| contact grenade, direct (d = 0) | 100 | yes | no: health 50 |
| frag, d = 0 | 150 | yes | yes (health part 100) |
| RPG, d = 0 | 150 | yes | yes |
| satchel, d = 0 | 150 | yes | yes |
| trip mine, d = 0 | 200 | yes | yes |
| energy ball | 1000 | yes | yes |
| bolt, body / head | 100 / 300 | yes / yes | no (health 80, armour 20) / yes |

With armour A and blast damage D: health loses D − A/2 when 1.6·D > A,
else 0.2·D (suit.md). Blast never uses the head multiplier.

### 12. Server, client and prediction

- **Predicted on the shooter's client** (weapon frame, weapons.md §12):
  timers, animations, the frag's pull-back/throw state, the AR2's charge
  start, the SMG1's timers, the crossbow's fire and zoom request, the
  RPG's fire animation, the S.L.A.M.'s modes.
- **Server only**: creating every projectile (the frag's on the server's
  own view-model event), zoom FOV (sent to the client), the laser dot used
  for guidance, explosions, all damage and shoves. The local player sees
  its own projectile one round trip late; clients interpolate projectiles
  (`cl_interp` 0.1) like other entities.
- **No lag compensation** for projectiles or explosions (only bullets and
  melee are rewound): they hit where players are on the server now.
- The RPG's beam and dot drawn for the local player come from the
  client's own view (H9); the server dot follows the owner's commands.
- Bolt pinning, the stuck-bolt models, impact and explosion effects are
  client-side, from temp entities and effects sent to clients in
  range.

## Per-tick order

One server tick (0.015 s):

1. Rules and pre-entity systems (rules.md).
2. Entities, in the list order taken at the start of the tick (projectiles
   created during this step wait for the next tick):
   - a **player**: its commands (weapons.md): movement, then the weapon
     frame (frag release check, SMG1/AR2 secondary, RPG fire and dot
     placement, crossbow auto reload and zoom, S.L.A.M. modes and
     placement), then the view model advances and its events fire
     (frag release → grenade created; drawback finished);
   - a **toss/fly projectile** (contact grenade, bolt, missile): think
     first (missile guidance or ignition; contact grenade's 3-tick check;
     bolt angles every 7 ticks), then its move and touches (explosion,
     damage, sticking);
   - a **VPhysics projectile** (frag, satchel, ball): its think only
     (frag fuse and blips, satchel rest check, ball life timer); a trip
     mine's arming and beam checks.
3. The physics step: frags, satchels, balls and props move; collisions
   call back (ball bounce, seek and speed reset; ball impact damage
   queued); after the step each frag checks for players along its
   velocity (9.4); queued impact damage is applied.
4. Queued entity inputs whose time has come (satchel explosions 0.2 s
   after a detonator or death).
5. Each explosion inside these steps applies its blast damage and shoves
   immediately, in order.

## Edge cases

- **Point-blank contact grenade**: fired into a wall at the eye, it
  explodes where it is (≈ the eye); the shooter is inside the radius:
  full self damage (distance ≈ 0 to its own box).
- **Rocket at the feet**: the blast source 1 unit above the floor is
  inside the shooter's box: distance 0, 150 damage: death even at 100
  health / 100 armour (health part 150 − 50 = 100).
- **Grace**: an enemy standing within 90 units in front but off the
  128-unit eye line is passed through by a fresh missile.
- **Missile on windows**: the dot passes through windows, the missile
  doesn't (it explodes on the glass).
- **Missile and dropped weapons**: it explodes on weapons, frags and mines
  lying in its path (weapons are solid to it, unlike the contact grenade).
- **Holding fire on the RPG** blocks the reload's end (6.7).
- **Contact grenade vs weapons**: it passes through dropped weapons,
  frags, satchels and mines (can't set off a trip mine by contact; its
  blast can).
- **Grenade inside a player**: frags pass through players; a frag that
  starts inside one is reversed at 0.2 once.
- **AR2 ball and its owner**: never; it can bounce straight back through
  its shooter harmlessly.
- **Two balls** collide and bounce; no damage.
- **Ball through a held prop / debris**: debris doesn't collide with it.
- **Ball in water**: physics fluids may act on it; its speed is reset at
  the next collision only (Open questions 4).
- **Bolt under water**: speed by the shooter only; a bolt fired from
  above into water keeps 3500.
- **Trip mine with a grate in its beam**: the arming length is measured
  through grates (bullet mask) but the watch line stops on them (solid
  mask): the lengths differ and the mine trips on its first check (Quirks).
- **Trip mine placed on a moving door/prop**: trips when the door or prop
  changes the beam length; stays in the air.
- **Trip mine placer walks through own beam**: dies (self damage).
- **Teammate in own trip mine beam (TDM)**: trips it; the teammate takes
  no damage with `mp_friendlyfire` 0, enemies near the mine do.
- **Shot satchels** are harmless (8.6); shot trip mines are not (they
  credit their placer).
- **Frag thrown, holder dies before the release event**: lost (9.5).
- **Frag with no grenades and attack held at death**: a live grenade
  anyway.
- **Spawn grace** vs explosives (rules.md): blast refused for 0.5 s; a
  trip mine on a spawn point still kills 0.5 s after the spawn.
- **Ceiling explosions** of point-size missiles: the source is lifted 1
  unit into the ceiling (Open questions 1).
- **Explosion under water**: dry victims are skipped; above water, fully
  submerged victims are skipped.

## Quirks

Keep all of these.

- **Bullets and melee shove players** (3); crouching doubles every shove
  (smaller box) except the upward cap.
- **Grenade/rocket jumping** is capped at +250 units/s upwards per hit;
  horizontal shove up to 1000; it costs the full self damage.
- **No cooking**: frags start their fuse on leaving the hand.
- **Martyrdom**: dying with fire held and the frag out drops a live
  grenade even with none left.
- **Frags pass through players** but ricochet off them at 0.2 speed with a
  0.1-point tap that costs armoured players 1 armour.
- **The contact grenade has no arming distance** and passes through
  dropped weapons and S.L.A.M.s.
- **The energy ball never slows**, kills through any armour, can't hurt
  its owner, homes up to 15° toward an enemy after wall bounces, and after
  a kill seeks the nearest visible enemy within 512 on each bounce; its
  final explosion is harmless.
- **The RPG can't be turned off or shot down**; the missile rides the line
  from the shooter's body (not eye) to the dot, so it swings in from the
  side and follows flicks; holding fire blocks its reload.
- **Crossbow bolts glance off at shallow angles** and drop after a
  glance; they pass through glass and grates. (One headshot (300) was
  predicted to kill through full armour; the server's bolt hits the
  generic hitgroup, 100 anywhere: Open questions 14.)
- **Shooting a satchel defuses it**; shooting a trip mine sets it off for
  its placer.
- **Trip mines watch with a different mask than they measure with**
  (grates, debris): such mines go off as soon as they arm.
- **Detonation is impossible in trip-mine mode** (face away from the wall
  to detonate).
- **Blast distance is to the victim's box**, so a blast at the feet does
  full damage, and blasts never head-shot.

## Test cases

Implementer checks (tick 0.015, gravity 600, `sk_player_head` 3, health
100, armour 0 and remainder 0 unless said; "box" = standing player box
(−16..16, −16..16, 0..72) around its origin, eye + 64).

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | blast D, R per 2.1; target distance d (to its box) | – | – | contact grenade 100 − 0.4d; frag 150 − 0.6d; RPG 150 − 0.5d; satchel 150 − 0.75d; mine 200 − d; e.g. d = 100: 60, 90, 100, 75, 100; d ≥ R: 0 |
| 2 | flat floor z = 0; frag blast point (−100, 0, 0.6); victim origin (0, 0, 0) | explode | – | S = (−100, 0, 1.6); line to eye (0, 0, 64) enters the box at (−16, 0, 54.016); d = 99.012; damage 90.593; health 10 (integer 90, remainder 0.593) |
| 3 | as 2, armour 100 | explode | – | cost 1.6 × 90.593 = 144.95 > 100 → health part 90.593 − 50 = 40.593: health 60, remainder 0.593, armour 0 |
| 4 | as 2 | explode | – | shove: inflictor centre (−100, 0, 0.6) − 10 Z; â = (−0.910554, 0, −0.413392); Δv = (412.45, 0, 187.25) |
| 5 | rocket blast point (−50, 0, 0), victim origin (0, 0, 0) | explode | – | d = 0.68 × 80.430 = 54.692; damage 122.654; armour 0: dead; armour 100: health 28 (remainder 0.654), armour 0; shove (451.33, 0, 415.22) → z capped: (451.33, 0, 250) |
| 6 | as 5, victim ducked (box height 36) | – | – | shove factor 10 per point: F = min(1226.5, 1000) = 1000 (direction from the ducked centre, origin + 18) |
| 7 | blast with a world wall between S and the eye | explode | – | 0 damage |
| 8 | blast; a 50 kg prop crosses the line S→eye; solid-only line clear | explode | – | damage × (1 − 50/350) = × 0.857, distance measured to the eye (not the box) |
| 9 | as 8 with a 400 kg prop | – | – | 0 |
| 10 | blast point in water 10 units deep (24 above is air) | explode | – | counts as dry: a dry victim is hit, a fully submerged one skipped |
| 11 | blast point 30 deep (24 above still water) | – | – | under water: victims with water level 0 skipped |
| 12 | victim spawned on tick 0 | frag blast on tick 33 | – | refused (pure blast); on tick 34: applied |
| 13 | pistol body hit (8) on an unarmoured standing victim | – | – | shove 40 units/s away from the shooter's centre (minus 10 Z); armour 100: health part 1.6 → whole 1 → shove 40 (uses 8); SMG1 hit at armour 100 (part 1.0): shove 25 |
| 14 | 0.1 crush tap, armour 0 | – | – | no shove, no `player_hurt`, remainder +0.1 |
| 15 | contact grenade from eye (0, 0, 64) along +X, punch 0, floor z = 0 | fire tick 0 | – | position on tick n: x = 15n, z = 64 − 0.045n²; touches the floor during tick 37 (z bottom < 0); explosion near x ≈ 552, 3 above the floor |
| 16 | SMG1 with grenades | attack2 tick 0 | – | next primary tick 34, next secondary tick 67; player not busy; a running reload cancelled with no ammo moved |
| 17 | shooter moving at 300 along +Y | fire a contact grenade along +X | – | grenade velocity (1000, 0, 0) (no inheritance) |
| 18 | contact grenade's path crosses a dropped `weapon_shotgun` and then a player | – | – | passes the weapon; explodes on the player |
| 19 | AR2, ball ammo 1 | attack2 tick 0 | – | charge sound tick 0; switching refused ticks 0–33; ball created tick 34 at the eye, speed 1000; ammo 0; first primary shot possible tick 81; next charge tick 101 |
| 20 | ball launched tick 34 | no contacts | – | harmless explosion on tick 34 + 267 = 301; removed 33 ticks later |
| 21 | ball meets a wall | bounce | – | post-collision speed reset to exactly 1000 |
| 22 | ball touches player B (armour 200) | – | – | B takes 1000 crush + dissolve, attacker the shooter: health part 800 → dead; ragdoll and dropped weapon dissolve; death notice `combine_ball` |
| 23 | ball's path crosses its own shooter | – | – | no contact, no damage |
| 24 | ball bounce at Q = origin, post-bounce d = (1, 0, 0), player centre (1000, 200, 0), not yet struck, visible | bounce | – | cos 0.98058 > 0.966 → velocity 1000·(0.98058, 0.19612, 0); centre (1000, 300, 0): cos 0.95783 → no change |
| 25 | as 24 within 0.25 s of the previous counted bounce | – | – | impact effect and sound only; no seek |
| 26 | ball that already killed; players at 400 in front (visible) and 300 behind | wall bounce | – | redirected at the one in front |
| 27 | RPG, standing still, eye (0, 0, 64), view +X, nothing within 128 | fire tick 0 | – | missile at (12, −6, 61), velocity (300, 0, 128), non-solid; on tick 20 before its move at (97.5, −6, 73.11); velocity (1500, 0, 0); solid |
| 28 | as 27 with a wall 100 units ahead | fire | – | solid at once (no grace); explodes on touching the wall |
| 29 | missile direction (1, 0, 0), aim direction (0, 1, 0) | one guidance tick | – | direction (0.98995, 0.14142, 0), speed 1500 |
| 30 | owner centre C (0, 0, 36), dot on a wall at (2000, 0, 36) normal (−1, 0, 0), no target entity; missile at (500, 100, 36) | guidance | – | L = (2010, 0, 36); missile nearer C than ℓ → A = (500, 0, 36) + 256·(1, 0, 0) = (756, 0, 36); t̂ = (0.93146, −0.36385, 0) |
| 31 | dot on a player | guidance | – | A = dot − 10·n (no beam riding) |
| 32 | owner dies with a missile in flight | – | – | dot removed; missile keeps its direction; its kill credits the dead owner |
| 33 | missile in flight | 10 pistol hits, a frag blast on it | – | missile unaffected |
| 34 | missile explodes tick X, rockets left, no buttons | – | – | reload animation from X; idle on X + 112; a press on X + 113 fires; with attack held from X: no shot until released |
| 35 | RPG with a missile in flight | `lastinv` | – | refused |
| 36 | crossbow, clip 1, reserve 4 | hold attack from tick 0 | – | shots on ticks 0 and 168 (reload starts tick 45); reserve 3 after the second reload starts |
| 37 | bolt fired level at 3500 | – | 67 ticks | 3517.5 units ahead, 15.15 lower (15·t², t = 1.005) |
| 38 | shooter's eyes under water | fire | – | bolt speed 1500 |
| 39 | bolt hits a player's head / chest, armour 0 | – | – | 300 / 100: dead both; armour 100: chest → health 80, armour 20; head → dead |
| 40 | bolt v̂ = (0.9, 0, −0.43589) at 3500 hits the floor | – | – | c = 0.43589 < 0.5 → glance: v̂' = (0.9, 0, 0.43589), speed 2625, gravity 600 from now |
| 41 | bolt hitting the floor with c = 0.6 | – | – | sticks; removed 2 s later; "BoltImpact" at the hit point |
| 42 | bolt hits a 30 kg prop | – | – | Δv = 700 in/s along the flight; 100 damage to the prop |
| 43 | crossbow | press attack2; press again | – | FOV → 20 over 0.1 s; back to default over 0.2 s |
| 44 | frag, standing still, view +X | tap primary on tick 0 | – | throw animation tick 15; grenade created tick 19 at eye + (18, −8, 0), velocity (1200, 0, 120); next pull-back accepted tick 138 |
| 45 | as 44, view pitch −30° (up) | throw | – | velocity (1039.23, 0, 720) |
| 46 | as 44, thrower moving (100, 50, 0) | throw | – | velocity (1300, 50, 120) |
| 47 | frag, secondary released without duck | – | – | grenade 7 ticks after the release at eye + 18 fwd + 8 right − 8 Z, velocity (350, 0, 50) |
| 48 | as 47 with duck held, flat floor, origin (0, 0, 0) | – | – | start (18, 0, 4), velocity (700, 0, 0), spin (0, 0, 720) |
| 49 | frag created tick 0 | – | – | blips ticks 0, 71, 92, 113, 134, 155; explosion tick 169 |
| 50 | frag resting on flat floor, origin 4 above | explode | – | probe from +8 hits the floor: blast point floor + 0.6; S floor + 1.6 |
| 51 | frag moving (500, 0, 0) meets a player's face with normal (−1, 0, 0), victim armour 50 | – | – | velocity (−100, 0, 0); victim armour 49, health 100 (remainder +0.02) |
| 52 | frag in hand, attack held, holder killed while moving (0, 200, 0) | death | – | live `npc_grenade_frag` at eye + 18 fwd + 8 right, velocity (0, 200, 0), explodes 169 ticks later; no `weapon_frag` dropped |
| 53 | holder releases attack on tick R and dies on tick R + 2 | – | – | no grenade; `weapon_frag` dropped as a weapon; ammo unchanged |
| 54 | S.L.A.M., wall 30 units ahead (trip-mine mode), slam 1 | attack tick 0 | – | mine created tick 16 at hit + 3·n; arms tick 155 (16 + 139); first beam check tick 162, then every 3 ticks |
| 55 | armed mine, beam length 300; a player steps into it on a check tick | – | – | trips; explosion 17 ticks later at mine + 8·n: 200 − d damage, credited to the placer |
| 56 | armed mine | one pistol bullet into it (before or after arming) | – | explosion 17 ticks later, credited to the placer |
| 57 | S.L.A.M. throw mode, standing still, level view | attack tick 0 | – | satchel on tick 32 at (origin + 36 Z) + 18 fwd + 24 Z, velocity 500·fwd; slam −1; detonator armed |
| 58 | two satchels out, idle in throw mode | attack2 tick 0 | – | both explode tick 14 at their origin + 16 Z, 150 / 200 |
| 59 | idle in trip-mine mode with satchels out | attack2 | – | nothing |
| 60 | satchel takes a pistol bullet | – | – | removed; no damage to a player standing beside it |
| 61 | thrower dies with satchels out | – | – | they explode 14 ticks after the death, credited to the dead thrower; trip mines unaffected |
| 62 | last slam placed as a trip mine, no satchels out | – | – | `weapon_slam` leaves the inventory |
| 63 | blast health part 35 after armour; same blast with part 20 and the inflictor 200 units away | – | – | DSP 35–37; DSP 32–34 |
| 64 | each projectile kills a player | – | – | death notices `smg1_grenade`, `combine_ball`, `rpg_missile`, `crossbow_bolt`, `grenade_frag`, `slam` |

Checks on this install or the probe server (game build 24600434,
dedicated server build 24600426), each marked **(probe)**:

| # | How | Expected |
|---|---|---|
| P1 (probe) | spawn a frag from a bot's throw; log its creation tick, blip sounds and explosion tick | 169 ticks to the explosion; blips at 0, 71, 92, 113, 134, 155 (±1 tick each if the think grid differs) |
| P2 (probe) | read the velocity of a thrown frag (`physics_debug_entity` or SourceMod) from a still thrower looking level | (1200, 0, 120); lob (350, 0, 50); mass 1 |
| P3 (probe) | contact grenade fired level from a standing bot | lands ≈ 552 units ahead, 37 ticks; speed 1000 regardless of the shooter's movement |
| P4 (probe) | RPG: missile origin each tick | toss for 19 ticks, 1500 from tick 20; per-tick turn 8.13° at 90° error; flies straight once the shooter dies |
| P5 (probe) | shoot a missile with every weapon | no effect |
| P6 (probe) | `ent_create` a frag/env_explosion at a measured distance from a bot (test 2) | health 10 (90.6 damage); with armour 100: 60/0 |
| P7 (probe) | read a bot's velocity right after test 2's blast, after a pistol hit, after a .357 hit; repeat ducked | (412, 0, 187); 40; 375; doubled when ducked, z ≤ 250 (settles weapons.md P16) |
| P8 (probe) | AR2 ball at a bot with 200 armour, at its owner, and into an empty room | dead; owner untouched; harmless explosion 4.005 s after launch |
| P9 (probe) | crossbow: tick of each shot holding attack; bolt speed in air and from under water; head and body damage | 168 ticks; 3500 / 1500; 300 / 100 |
| P10 (probe) | trip mine: placement tick, `TripmineGrenade.Activate` tick, trip-to-explosion delay | 139 ticks; 17 ticks |
| P11 (probe) | shoot a satchel next to a bot | bot unhurt |
| P12 (probe) | RPG and contact grenade into a ceiling above a bot | open question 1: full damage or none |
| P13 (probe) | RCON `sk_weapon_ar2_alt_fire_duration`, `_radius`, `_mass`, `sk_combine_ball_search_radius`, `phys_pushscale`, `old_radiusdamage`, `sk_satchel_radius` | 4, 10, 150, 512, 1, 0, 0 |
| P14 (probe) | a bot dies holding attack with the frag out and 0 grenades | a live grenade appears |
| P15 (probe) | a trip mine whose beam crosses a grate | trips right after arming |

## Open questions

1. **Ceiling hits.** A point-size missile or a contact grenade (box ±3
   leaves 2 units) touching a ceiling has its blast source raised 1 unit;
   for the missile that point lies inside the ceiling brush and every
   damage line starts in solid. Whether the engine then reports "blocked
   by world" (no damage to anyone) or a clear trace is engine trace
   behaviour not in the SDK. P12.
2. **Player shielding mass.** A player standing between a blast and a
   victim is a physics object for the blast rule; its shadow's mass (85
   kg in weapons.md's drop rule, giving 24.3 % absorbed) is assumed, not
   read. Measure with two bots.
3. **View-model event ticks.** Release ticks (frag 4 and 7 ticks after the
   animation starts, S.L.A.M. ends of animations) assume the server
   advances the view model by whole ticks from the tick the sequence
   starts; ±1 tick possible. P1/P10 timings settle it.
4. **Physics fluids.** How water acts on the frag (1 kg, volume 87.2) and
   the ball (no gravity, speed reset only at collisions) was not followed
   into the physics engine; measure.
5. **Unloaded crossbow draw.** Deploying with an empty clip asks the view
   model for an activity `v_crossbow.mdl` doesn't have
   (`ACT_CROSSBOW_DRAW_UNLOADED`); the resulting animation and busy time
   are unknown (probably the default sequence and its length). Probe.
6. **Trip mine aim.** The placement lines use the player's angles during
   the weapon frame, which are the view angles at that point of the tick;
   if the server's body yaw (feet) were used instead the mine could land
   off the crosshair. Assumed view. Probe by placing at a corner.
7. **Frag vs player shadow.** Frags pass through players by collision
   class; the player's physics shadow uses the same class pair, so no
   VPhysics contact is assumed. Probe: roll a frag at a standing bot.
8. **Standing on a ball** (H1 Open questions 5): it is solid to player
   movement but never stops; whether a player can ever ride one is
   untested and irrelevant in practice.
9. **Random numbers** (spin, shove-less force jitter, ball punch) follow
   the engine's uniform generator; tests should check ranges.
10. **Satchel double explosion.** Several satchels detonated together:
    each one's blast destroys the next with a pop before that one's own
    queued explosion fires in the same tick; the sound order and whether
    the pop's scorch appears are cosmetic and unverified.
11. **Grenade in a wall at launch.** A missile created with its muzzle
    point inside a wall (shooter hugging it at an angle) starts in solid;
    whether it explodes at once or sticks until ignition is engine
    behaviour; probe.
12. **Mine expiry on restart.** Trip mines and satchels are assumed to be
    removed by a game restart with the other non-map entities (rules.md);
    not checked here.

13. **Probe results (2026-11-03, build 24600426, `tools/hl2dm_probe/cases.py`).**
   A held crossbow fires every **130** ticks, not 168: the bolt is in 130
   ticks after the shot and `bolt_load` plays 69 ticks after it, so the
   reload starts about 7 ticks after the shot (the fire animation ends
   after about 0.1 s, not 0.6667; Chimera uses 0.1). The frag starts at
   eye + 18 forward + 8 right and flies about 2.2° right of the view (a
   lateral part 3.8 % of its speed, every throw; ≈1160 forward and 120
   up by its positions); the lob ≈ 340-380 forward, 43 up. Contact grenade
   1000 with gravity 400, missile (300, 0, 128) with gravity 600 then 1500,
   bolt 3500 with gravity 30, as listed. Fuses and ignition count from the
   thrower's command clock, 4 ticks ahead of the server's on a bot, so in
   server ticks the frag explodes 173 after creation (blips 0, 74-75, 96,
   117, 137-138, 159) and the missile ignites 23-24 ticks after.
14. **Probe results, round 2 (2026-11-03, same build).** Answered:
    - **The frag's throw** is as 9.2 says, 1200 × (forward + 0.1 up) plus
      the thrower's velocity (a falling thrower's −711 shows in the
      grenade), the lob 350 forward + 50 up. Round 1's "2.2° to the
      right" was the origin circling its mass centre as the grenade spins
      (3-5 units, at the spin's rate); fitted past that circle the side
      part is 0. **It feels air drag** (9.4 says damping 0, which holds:
      this is the physics engine's drag): fitted over 15-tick windows at
      speeds 50-1550, the velocity loses (0.07 + 0.00009 |v|) v per
      second (0.176 of it a second at 1176, 0.10 at 350): a throw 1185
      over its first 10 ticks, 1157 over the next 10.
    - **Head hits** (on a bot that doesn't animate: its head hitbox is
      low and to one side): a pistol head hit takes 24 (8 × 3), a .357's
      225; the crowbar 25 with no multiplier (hitgroup generic, as 4
      says). **A crossbow bolt never head-shots**: on the head, the chest
      and the legs alike its hit is the generic hitgroup and takes 100
      (with 100 armour: health 80, armour 20). Quirks' "one headshot
      (300) kills through full armour" and test 39's 300 don't happen on
      the server.
    - Open question 1: a rocket and a contact grenade going off against
      the ceiling above their shooter hurt by the distance rule (53 and
      24 to the shooter 190 below): the lifted point inside the brush
      blocks nothing.
    - P6, P7: a frag's blast at measured distances does 150 − 0.6 d
      within the frag's position error (125.6 for 126.4, 74.3 for 75.1),
      and shoves 5 per point away from the blast point lowered 10
      (|Δv| 371.6 for 74.3). Its 0.1-point tap shows when it rolls into
      the victim.
    - P8: the ball kills through 200 armour (1000, dissolve); fired
      straight up in a room it never touches its owner and goes off 272
      server ticks after its creation (4.005 s + the command clock's 4-5
      ticks), removed 33 ticks after that.
    - P10: the mine is created 16 ticks after the press, `TripmineGrenade.
      Activate` 143-144 server ticks after it (139 + the command clock);
      its placer standing in the beam trips it at the first check, 7
      ticks after arming, and the blast is 17 ticks after that.
    - P11: a shot satchel takes the bullet, is removed 5 ticks later and
      hurts nobody (a bot 40 away untouched).
    - P14: dying with the pin pulled on the last grenade drops it live
      (the count 1 → 0 at the death).
    Not settled: P5 (needs a second driven player), P9's water speed (the
    test spots in dm_underpass's pool put walls in the bolt's way), P12's
    exact distances, P15 (a grate in a mine's beam), the ducked shove.
