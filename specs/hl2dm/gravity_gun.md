# Half-Life 2: Deathmatch: gravity gun, `+use` carry and physics damage (H5)

Source basis: the public Source SDK 2013 multiplayer game code — HL2:DM's
own gravity gun (punt, pull, pick-up, the hold controller, launch, drop,
claws and effect states, networking and client prediction of the held
object), the player pick-up controller used by `+use`, the HL2 and
HL2:DM players (use, pick-up permission, death, physics damage scale),
the base player's pick-up limits, the shared physics impact-damage
tables and the combat character's shadow-collision damage, the physics
props (prop data interactions, physics attacker and influence time,
punts and launches, `prop_physics_respawnable`), items and the frag
grenade's reaction to the gun, the combine ball's reaction to the gun,
the game rules' collision pairs and the physics collision filter — read
in place in a separate spec session. From the game's files on this box
(game build 24600434, dedicated server build 24600426, `steam.inf`
PatchVersion 10889068, both checked): `hl2mp`'s and Half-Life 2's sound
scripts, the stock maps' entity lumps (counts and keys only), the
`physgun_interactions` blocks of the stock maps' prop models, and the
shipped server library's cvar names. Nothing was launched.
Status: draft

## Summary

The gravity gun (`weapon_physcannon`) has two buttons. **Primary punts**:
a short (250-unit) shove that gives any free physics object a fixed
impulse of 15 000 plus 600 × its mass (mass capped at 250 kg), so light
things fly at the 2000-unit/s physics cap and heavy things barely move;
it never hurts or pushes players. **Secondary grabs**: anything movable up
to 250 kg whose line or cone lies within 250 units is lifted at once and
held about 47 units plus its own depth in front of the eye; farther than
that (up to 1000) the object is only tugged toward the player every 0.1
s. While holding, primary launches the object at 1500 units/s (less above
100 kg) and secondary drops it. Held objects weigh 1 kg, never collide
with or are shot by their holder, can't be stood on by the holder and do
no damage; thrown ones hit players through the physics impact table with
HL2:DM's energy scale of **4**, credited to the last player who touched
them with the gun for **1 s**. `+use` carrying is compiled in but
**switched off** (`hl2mp_allow_pickup` 0): `+use` on a small prop only
plays a click. The gun is usable the instant it is drawn.

## Units and conventions

- Source units (inches), Z up, X forward, Y left; degrees; pitch positive
  looks **down**. Seconds; one server tick = 0.015 s.
- Timers are absolute times; a delay of d lands on the first tick whose
  time is ≥ now + d: ⌈d / 0.015⌉ ticks (0.5 s → 34, 0.1 s → 7, 0.4 s →
  27), as weapons.md's "Units and conventions".
- Mass in kg; impulses in kg·in/s (Δv = J / m in in/s), as
  specs/cs_source/physics_props.md. "Impulse at the centre" changes only
  linear velocity; "impulse at a point" also adds the torque
  (p − c) × J.
- "Eye" = the player's eye position (origin + 64, ducked + 28); "view
  forward" f = the unit vector of the view angles. The player's
  horizontal radius r_p = √(16² + 16²) = 22.627 (standing and ducked).
- Sound entry names, cvars, entity classes, keyvalues and model data keys
  are the game's data names. Steam builds read: game 24600434, dedicated
  server 24600426.
- "Influence" of a prop by a player = the pair (player, time) the prop
  stores when a gun or `+use` touches it (section 9).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| `physcannon_tracelength` | 250 | in | punt range, claw check range, grab range, held-object launch range |
| grab_search_length | 1000 | in | 4 × tracelength: pick-up/pull trace length |
| `physcannon_maxmass` | 250 | kg | heaviest object (all its bodies summed) the gun can hold or pull |
| `physcannon_maxforce` | 1500 | in/s | launch speed of held objects up to 100 kg |
| `physcannon_minforce` | 700 | in/s | launch speed reached at 600 kg (spline) |
| `physcannon_pullforce` | 4000 | kg·in/s | pull impulse (scaled down below 50 kg) |
| `physcannon_cone` | 0.97 | cos | cone for grabbing without a direct hit (14.07° half angle) |
| `physcannon_ball_cone`, `physcannon_chargetime` | 0.997, 2 | –, s | registered, **unused** by HL2:DM's gun |
| `player_throwforce` | 1000 | kg·in/s | `+use` throw impulse base (only with `hl2mp_allow_pickup` 1) |
| `hl2mp_allow_pickup` | 0 | bool | server cvar (not cheat, not replicated): `+use` carry allowed |
| punt_base_impulse | 15 000 | kg·in/s | punt, at the mass centre |
| punt_mass_impulse | 600 × min(M, 250) | kg·in/s | punt, at the hit point |
| punt_down_reflect | −0.65 | × | a downward punt's z component is multiplied by this |
| punt_hull | ±8 | in | punt trace box |
| grab_hull, claw_hull | ±4 | in | pick-up fallback box, claw check box |
| refire_punt | 0.5 | s | after any primary press when not holding (34 ticks) |
| repunt_guard | 0.5 | s | the same object can't be punted/launched again within this |
| refire_after_grab | 0.5 | s | next secondary after a successful grab |
| refire_after_drop_launch | 0.5 | s | next primary and secondary after a drop or launch |
| refire_search | 0.1 | s | next secondary after a failed grab or a pull (7 ticks) |
| hold_base_distance | 24 | in | added to the object's radius for the hold point |
| hold_pitch_clamp | ±75 | deg | view pitch used for the hold point |
| hold_max_speed | 1000 | in/s | controller's linear speed limit (damping limit 2000) |
| hold_max_angular | 3600 | deg/s | controller's angular limit before the contact factor |
| held_mass | 1 | kg | mass of the grabbed body while held (others 1 / max(n/7.5, 1)) |
| held_rot_damping | 10 | – | angular damping while held |
| hold_error_limit | 12 | in | smoothed position error above which the object is dropped |
| hold_error_grace | 1.0 | s | error not computed for this long after a grab |
| drop_speed_cap | 285 | in/s | 1.5 × `hl2_normspeed` 190: speed cap on a plain drop |
| drop_spin_cap | 720 | deg/s | spin cap on a plain drop |
| launch_spin | U(−600, 600) per axis | deg/s | random spin added on launch (and pull-free punts of launch-force objects) |
| fire_view_punch | (−6, U{−2..2}, 0) | deg | every punt/launch (integer yaw, shared seed) |
| punt_recoil | (U(1, 2), U(−1, 1), 0) | deg | extra punch on a physics punt, applied first |
| fire_fade | (245, 245, 255, 32), 0.1 s | rgba, s | white screen fade-in on the punter |
| claws_open_time / close_time | 0.2 / 0.5 | s | view-model "active" pose blend, spline |
| claws_close_delay | 0.5 | s | claws close this long after the target is lost |
| target_suppress | 0.25 | s | claw check suspended after a punt or launch |
| player_impact_energy_scale | **4** | – | HL2:DM player's physics impact scale (CS:S 1) |
| impact_credit_window | 1.0 | s | thrower credited for impacts on players within this of the influence |
| explosion_credit_window | 2.0 | s | a breaking prop's blast credited to its influencer within this |
| first_impact_scale | 10 | × | a prop's own impact damage on its first collision after a launch from hold |
| use_carry_mass / size | 35 / 128 | kg / in | `+use` carry limits (summed mass; each box dimension) |
| use_carry_align | cos 30° = 0.866025 | – | `+use` carried object snaps axes within 30° |
| respawn_default | 60 | s | `prop_physics_respawnable` `RespawnTime` when 0 or absent |
| respawn_retry | 1 | s | retry while the spawn spot is blocked |
| item_trigger_bloat_held | 48 | in | item touch box grown by this while held (24 normally) |
| frag_fuse_after_gun | 1.5 | s | a frag's fuse is reset to this when the gun grabs or punts it |
| ball_hold_life | 8 | s | a held combine ball explodes this long after the grab |
| ball_relaunch_speed | 1500 | in/s | combine ball target speed after a drop or launch |

All `physcannon_*` cvars and `player_throwforce` are **cheat,
replicated** cvars; the shipped server library registers exactly these
names (plus `physcannon_mega_enabled`, from shared HL2 rules, which
HL2:DM's gun ignores). No shipped `.cfg` sets any of them.

## Behavior

### 1. The weapon itself

- Script facts (H0 "Weapon scripts", H3): bucket 0 position 2, no ammo,
  weight 0, `autoswitchto` 0, `autoswitchfrom` 0, item flags 2. It always
  counts as having ammo. It is given at every spawn and is the default
  weapon in hand (`cl_defaultweapon`).
- **Deploy**: claws close (if open), effect state "ready", then the next
  primary, next secondary and the player's next attack are all set to
  **now** — the draw animation plays but doesn't delay use (Corrections,
  item 4).
- **Holster** is refused while holding an object (so `slotN`, `lastinv`,
  `invnext` and pick-up auto-switches all fail while holding). Otherwise
  holstering drops nothing (nothing is held), clears effects and proceeds.
- `phys_swap` (bound to G): with the gun in hand, closes the client's
  weapon menu, **force-drops** the held object (as a plain drop, section
  6, with the `Weapon_PhysCannon.Drop` sound) and switches to the last
  weapon; otherwise selects the gun. Nothing with no weapon in hand.
- On the holder's death the gun force-drops its object, then deletes
  itself (never dropped as a weapon, weapons.md §9).
- The gun's per-command frame (after movement, weapons.md order) replaces
  the generic one: no reload, no empty handling. Its steps are in
  "Per-tick order".

### 2. Primary when not holding: punt

Runs whenever primary is held and next primary ≤ now (so a held button
punts every 0.5 s: ticks 0, 34, 68, …).

1. next primary := now + 0.5 (before anything else; a dry fire also
   waits 0.5 s).
2. **Target**: from the eye along f, a box trace (±8) of length 250
   against shot-solid geometry plus grates, ignoring the player itself
   but **not** what the player owns (own grenades can be punted). The hit
   is invalid if nothing was hit, or the hit entity is marked "no gravity
   gun interaction", or it is neither a physics-moved object nor
   damageable (the world, static brushes). If invalid, the same trace as
   a line; if that finds nothing valid either → **dry fire**.
3. **Non-physics target** (moved by something other than physics):
   - not damageable → dry fire;
   - a **player → dry fire** (players are never punted, pushed or hurt
     by the gun; a player standing in front of a prop shields it, since
     the box trace stops on the player);
   - otherwise (a `func_breakable`, a tripmine, …): unless it is the
     object punted within the last 0.5 s, it takes **1** damage of type
     crush + physgun from the punter (inflictor: the gun), force = f (a
     unit vector, i.e. no push), at the trace end; launch effect at that
     point, fire effect, `ACT_VM_SECONDARYATTACK`; claws to close 0.5 s
     later; claw check suspended 0.25 s. Next secondary unchanged.
4. **Physics target**: if its physics is flesh (a fleshy ragdoll) → dry
   fire. Otherwise the physics punt (below).
5. **Dry fire**: `ACT_VM_PRIMARYATTACK` and `Weapon_PhysCannon.DryFire`;
   no punch, no effect, no other timer.

**Physics punt.** If the target was punted or launched by this gun
within the last 0.5 s, nothing at all happens (silently; step 1 already
cost 0.5 s). Otherwise, remember it as last punted (now + 0.5), then:

1. The target receives a 0-damage hit of type physgun from the punter
   (inflictor the gun): this fires its damage outputs and makes the
   punter its **last attacker** (prop_damage.md §4 step 5; it carries no
   force).
2. The target may refuse (a prop that must first be pulled loose; none
   in the stock maps). An entity with no physics bodies gets the
   "punted" notification and a dry fire.
3. d := f; if d_z < 0 then d_z := −0.65 × d_z (a downward punt goes
   **up**, flattened). d is not renormalised.
4. The target is told it was **punted** by the player (section 9:
   influence set to (punter, now); a motion-disabled prop with spawnflag
   64 "enable motion on gravity gun" is enabled first; one without it
   stays pinned and the impulses below do nothing; a frag's thrower and
   fuse change, section 11; a combine ball, section 12; the prop's
   `OnPhysGunPunt` output and punt sound).
5. Unless the target uses "launch forces" (only the combine ball does),
   with M = sum of the target's body masses (their **current** masses: a
   body held by someone else weighs 1 kg) and m_eff = min(M, 250):
   for each body i of mass m_i, r_i = m_i / M, plus 0.5 (capped at 1)
   for the main body, or × 0.5 for the others; then

```math
J_{centre,i} = 15000\, r_i\, \mathbf d \qquad J_{point,i} = 600\, m_{eff}\, r_i\, \mathbf d \ \text{applied at the trace end point}
```

   For a single-body object (r = 1):

```math
\Delta v = \frac{15000 + 600\,\min(m,250)}{m}\ \text{along}\ \mathbf d,\ \text{then the 2000 in/s cap}
```

   plus the spin from J_point's lever arm about the mass centre.
6. A launch-force target instead gets the launch velocity of section 7
   **added** along the unflattened f, plus its launch spin.
7. View punch (punt_recoil, then fire_view_punch), launch effect at the
   trace end, white fade, `Weapon_PhysCannon.Launch`,
   `ACT_VM_SECONDARYATTACK`; claws to close in 0.5 s; claw check
   suspended 0.25 s; **next primary and next secondary := now + 0.5**.

Punt speeds (single body, hit through the mass centre, level aim):

| Mass (kg) | 1 | 5 | 10 | 15 | 20 | 30 | 50 | 100 | 250 | 500 | 1000 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Δv (in/s) | 2000 (15 600) | 2000 (3600) | 2000 (2100) | 1600 | 1350 | 1100 | 900 | 750 | 660 | 330 | 165 |

(values in brackets before the physics speed cap of physics_props.md
§3.6). There is no mass limit on punting.

### 3. Secondary when not holding: grab or pull

The gun's frame calls the secondary routine every command while
secondary is held, except that after a successful grab the button is
**masked until released** (holding it doesn't drop or re-grab). The
routine returns at once if next secondary > now. It is server-only (the
client predicts nothing of it).

**Search.** From the eye along f, length 1000:

1. A line trace (shot-solid plus grates, ignoring only the player). If it
   missed, or hit the world, a ±4 box trace of the same length instead.
2. If it hit a non-world entity: the candidate is that entity's root
   move-parent; **grab** if the hit lies within 250 (fraction ≤ 0.25),
   else **pull**.
3. Otherwise a **cone search**: every entity with a physics body whose
   world-space centre is less than 251 from the eye and whose direction
   from the eye has cosine > 0.97 with f, and to whose centre a line from
   the eye (same mask) hits that very entity; the nearest wins and is
   always grabbed (its centre is within 1000). Player shadows and brush
   entities count here and then fail the check below.
4. **Can it be held?** (a candidate that is missing, the world or fails
   any test plays `Weapon_PhysCannon.TooHeavy` **once per button press**
   and the search fails):
   - not dissolving, not marked "no gravity gun interaction";
   - not the holder's ground entity (what the player stands on);
   - not flesh;
   - its main body is not currently held by anyone (gun or `+use`);
   - moved by physics, with at least one body;
   - no body flagged "no player pick-up" (`prop_physics` spawnflag 512)
     and no body hinged;
   - summed mass ≤ 250 (no mass limit for a combine ball);
   - if any body is motion-disabled: only a `prop_physics*` with
     spawnflag 64 or a `func_physbox` with its "enable on gravity gun"
     flag passes.
   So with nothing in reach (sky, a wall, a player, a 300 kg crate) every
   fresh press of secondary plays the TooHeavy sound once.
5. The target may refuse to be pulled loose (none in the stock maps);
   then next secondary := now + 0.01 (retry next tick).
6. **Grab** (section 4) or **pull**.

**Pull.** The candidate's main body gets, at its centre, the impulse

```math
\mathbf J = 4000\,\hat{\mathbf u}\cdot\begin{cases}(M+0.5)/50 & M<50\\ 1 & M\ge 50\end{cases}
\qquad \hat{\mathbf u} = \text{unit}(\text{eye} - \text{object centre})
```

with M the summed mass; nothing else (no gravity compensation). The
routine then counts as "not found": claws close, next secondary := now +
0.1. Holding secondary therefore tugs every **7 ticks**: Δv = 84 in/s for
10 kg, 81.3 for 30 kg, 80 for 50 kg, 16 for 250 kg.

**After the routine** (every call, found or not): effect state
"holding" (beams; section 10). On a grab: `Weapon_PhysCannon.Pickup`,
`ACT_VM_PRIMARYATTACK`, next secondary := now + 0.5, button masked. On
"not found": next secondary := now + 0.1, claws close. When secondary is
not held: the TooHeavy once-per-press latch resets and (not holding)
effect "ready".

### 4. Grabbing

1. Held := true. The object is told it was **picked up** by the player
   (influence set; a debris prop becomes interactive debris — which in
   HL2:DM collides with players; a pinned prop with flag 64 is enabled;
   its `OnPhysGunPickup`/`OnPhysGunOnlyPickup`/`OnPlayerPickup` outputs
   fire; the prop is forced always visible).
2. The object's surface impact sound plays as if it hit the player at
   64 in/s (feedback).
3. Every body is woken, flagged "player held", its mass saved and set to
   1 / max(n / 7.5, 1) kg (n bodies; 1 kg for ordinary props) and its
   angular damping saved and set to 10; the grabbed body is set to **1
   kg** and drag is switched off. The load weight = the saved masses'
   sum.
4. **Orientation memory**: the object's current angles are stored
   relative to the player's current full view frame (pitch and yaw). If
   the prop's model data has `preferred_carryangles` (stock: the propane
   tank, "−90 0 0"), those angles in the player's **yaw-only** frame are
   used instead every tick; a "spin about z on launch" prop (the sawblade)
   is held at (−5, 0, 0) in the view frame.
5. The anchor point = the object's bounding-box centre, in object space.
6. If the object has no owner, its **owner becomes the holder** (cleared
   again when it is let go). Owned objects keep their owner.
7. Error state: smoothed error 0, error clock −1 s (section 5).
8. Next secondary := now + 0.4 (then overwritten by the 0.5 of section
   3). Next primary is **not** touched: a launch is possible on the very
   next command.
9. Effect "holding", claws open.

The holder can still sprint (the sprint lock is HL2 single player's; in
HL2:DM's gun it is disabled) and walks at normal speed.

### 5. Holding: the target and the controller

**Target, once per server frame**, at the start of each of the holder's
commands (before movement). In order, the hold ends (as a plain drop,
section 6, with the Drop sound) if the smoothed error > 12, the holder's
ground entity is the object, it has lost its physics body, or its main
body has become motion-disabled. Otherwise:

1. p = view pitch normalised to [−180, 180] and clamped to [−75, 75];
   f′ = forward of (p, yaw).
2. e = the object's hull extent from its origin in the direction −f′ at
   its current angles (the distance from the origin to the hull's
   farthest point toward the player, measured along f′).
3. R = r_p + e = 22.627 + e; D = 24 + 2R.
4. A line from the eye along f′ of length D against **world brushes
   only** (other entities ignored). If it is blocked at less than half
   (fraction < 0.5): point P = eye + f′ · R/2; else P = eye + f′ · (D −
   R) = eye + f′ · (24 + R).
5. Horizontal clamp: if P's horizontal distance from the player's
   vertical axis (through its box centre) is less than R, P is pushed out
   horizontally to exactly R.
6. Target angles = the stored relative angles applied to the player's
   current view frame (or the preferred carry angles, step 4 of
   section 4).
7. Target origin = P − (anchor rotated by the target angles): the box
   centre is placed at P. Time to arrive := one tick. The body is woken.

**Controller, each physics step**, on the held body:

- Contact factor c (starts at 0, i.e. ramps up after a grab): if the
  body touches anything immovable or heavier than the load weight, c
  moves toward 0.1, else toward 1, at 2 per second. Angular limit =
  3600 × c³ deg/s.
- The body is driven toward the target position and angles to arrive
  in the time to arrive (one tick), with linear speed limited to 1000
  in/s (damping limit 2000) and the angular limit above; the exact
  shadow-control law is inside the physics engine (Open questions 1). A
  model that reproduces it closely: v = (target − x) / 0.015, scaled
  down to 1000 if faster; ω likewise toward the target orientation.
- Then, for every contact with something immovable or heavier than the
  load weight, the linear velocity's component **into** that contact's
  normal is removed (slide along walls instead of bouncing).
- The error clock advances by the step time.

**Smoothed error**, computed when the target is updated: while the
clock ≤ 0 (the first 1 s after the grab) it is 0. Otherwise with
t = min(clock, 1) and e_now = |last target − current position|; if
e_now / t > 1000 then e_now := e_now / 2; error := (1 − t) · error +
t · e_now; clock := 0. At one update per tick (t = 0.015):

```math
E_{n+1} = 0.985\,E_n + 0.015\,e_n',\qquad e_n' = \begin{cases}e_n & e_n\le 15\\ e_n/2 & e_n>15\end{cases}
```

and the object drops once E > 12. Held against a wall with a constant
gap of 48 the drop comes 46 ticks after the grace; a gap of 14 drops
after 129 ticks; gaps between 15 and 24 **never** drop (Quirks).

**Holder and object never collide**: physics never collides an entity
with its owner (step 6 of section 4), and traces ignore what the
tracing entity owns. So the holder walks through his held object, his
bullets pass through it, and other players' bullets and bodies hit it (a
1 kg body: bullets knock it about; it is a shield that wobbles). Other
players can stand on someone's held object; only the holder standing on
it (becoming its ground entity, checked whenever the ground entity
changes as well as each tick) forces a drop. Objects that already had an
owner (an enemy's grenade, a satchel) do collide with the holder.

**What a held object can and can't do**: it deals no impact damage to
anyone (held objects are skipped as the "other" body); players' bodies
deal it none either; against other props it counts as 1 kg. It can be
punted by another player (the punt sees its 1 kg mass: 15 600 kg·in/s,
the speed cap) without being dropped; the punter becomes its influencer
(Quirks). Nobody else can grab it.

If the holder's primary is held and the object's centre is more than 250
from the holder's box centre, each command plays a dry fire (no timer)
instead of launching.

### 6. Dropping (secondary press while holding)

Only a **fresh press** of secondary (pressed this command) with next
secondary ≤ now drops (the earliest is 0.5 s after the grab). Then:

1. next primary := next secondary := now + 0.5;
2. the hold ends: every body gets back its saved mass and angular
   damping, drag on, woken, "player held" cleared; its velocity is
   **clamped** to 285 in/s and its spin to 720 deg/s (not zeroed);
   collisions with the holder resume (owner cleared if the gun set it);
3. the object is told it was **dropped** by the player (influence set to
   (player, now));
4. `Weapon_PhysCannon.Drop`; the hold loop sound fades out; effect
   "ready"; `ACT_VM_PRIMARYATTACK`.

A press during the first 0.5 s after the grab is swallowed (the press
flag lasts one command); if secondary is then **held** past the 0.5 s,
the grab search runs on the held object, fails (it is "player held"),
plays TooHeavy once, closes the claws and repeats every 0.1 s — the
object stays held (Quirks).

Forced drops (phys_swap, death, error, ground, lost body, motion
disabled, the gun removed) are the same without step 1 and without
`ACT_VM_PRIMARYATTACK`; the claws close and effects stop.

### 7. Launching (primary while holding)

If next primary ≤ now and the object is within 250 (section 5) and was
not punted or launched by this gun in the last 0.5 s:

1. The hold ends as in section 6 step 2 but with velocity and spin
   **zeroed** instead of clamped; the object is told it was **launched**
   (influence (player, now); a prop gets its "first collision after
   launch" mark and the "was thrown" body flag; a sawblade-type prop
   spins 5000 deg/s about its z axis); remembered as last punted (now +
   0.5).
2. Velocity added along the view forward f (no flattening, the holder's
   own velocity not included), with m the object's mass **after** the
   release (its real mass, except a combine ball: 750):

```math
v = \begin{cases} 1500 & m \le 100\\ 1500 - 800\,S\!\left(\dfrac{\min(m,1000)-100}{500}\right) & m>100\end{cases}\qquad S(x)=3x^2-2x^3\ \text{(not clamped)}
```

   giving 1477.6 at 150 kg, 1416.8 at 200, 1327.2 at 250, 700 at 600,
   959.2 at 750, 1500 at 850, 3055 at 1000 kg (the unclamped spline
   rises again past 600; only the ball reaches it). Spin: U(−600, 600)
   deg/s on each axis, unless the prop's interactions say "spin_none" or
   "spin_zaxis".
3. next primary := next secondary := now + 0.5; launch effect at the
   object's centre; claws to close 0.1 s later; claw check suspended
   0.25 s; the hold loop sound fades.
4. Fire effect (fire_view_punch, white fade, `Weapon_PhysCannon.Launch`)
   and `ACT_VM_SECONDARYATTACK`.

### 8. `+use` and props (HL2:DM default: no carry)

The `+use` search is specs/source/doors_buttons.md's. A physics prop is
**usable** when it has spawnflag 256 ("enable pick-up output") or would
pass the carry test: summed mass ≤ 35 kg, each box dimension ≤ 128,
movable (or spawnflag 64), no "no pick-up" flag, not hinged.

**With `hl2mp_allow_pickup` 0 (default)**, pressing `+use` at a usable
prop: `HL2Player.Use` plays (`common/wpn_select.wav`), its `OnPlayerUse`
output fires if it has spawnflag 256, and **nothing else happens** — no
pick-up, no push, no deny sound. At an unusable prop (heavier than 35 kg,
too big) with nothing else usable found: `HL2Player.UseDeny`
(`common/wpn_denyselect.wav`). Items (`item_*`) are always usable and
behave the same (click, nothing; they are taken only by touch, H2/H10).
CS:S's turbo-physics `+use` push doesn't exist in HL2:DM.

**With `hl2mp_allow_pickup` 1** (`+use` on a usable prop that also
passes the carry test, is not the player's ground entity):

1. The weapon in hand is holstered (if it refuses — a gun holding
   something — nothing happens); sprint is disabled (and stopped); the
   weapon-selection HUD is hidden.
2. A debris prop becomes interactive debris. The prop is told it was
   picked up (influence set). The same hold as section 4–5, except: the
   stored angles are relative to the player's **yaw-only** frame (the
   object keeps its attitude when looking up or down), and each of the
   object's axes that lies within 30° of a frame axis is snapped onto it
   at the grab.
3. Each tick after the weapon frame while the weapon is hidden: if
   secondary is held, or the error > 12, or the object is gone or
   motion-disabled → drop; if primary is held → **throw**; else update
   the target. Pressing `+use` again drops it. (A failed target update —
   e.g. standing on it — doesn't drop here, but standing on it drops it
   through the ground-entity rule.)
4. **Drop**: if the object touches anything at that moment its velocity
   is zeroed, otherwise clamped (285, 720); told "dropped".
5. **Throw**: released with the clamp, told "thrown by player", then an
   impulse at the centre along the view forward and a random torque:

```math
k = 0.5 + \frac{(\mathrm{clamp}(m,0.5,15)-0.5)\cdot 3.5}{14.5},\qquad \mathbf J = 1000\,k\,\mathbf f,\qquad \boldsymbol\tau = k\cdot U(-10,10)^3
```

   so Δv = 620.7 in/s (1 kg), 317.2 (5), 279.3 (10), 266.7 (15), 200
   (20), 114.3 (35), on top of the clamped velocity.
6. After any release: sprint re-enabled, HUD shown, the weapon deployed
   again (or the next best weapon if it can't be).

### 9. Physics damage to players and credit

Impacts on a living player follow specs/source/prop_damage.md §6 (the
`player` table, the min-mass, small-mass and min-speed filters, the
falling factor, crush damage, force = the player's post velocity × 85 ×
`phys_impactforcescale` 1, queued and applied after the physics step),
with these HL2:DM facts:

- **Energy scale 4** (prop_damage.md's player value 1 is CS:S's). With
  Δ_p the player shadow's speed loss (0 if 0 < Δ_p < 320), Δ_o the
  object's, m_o its mass and L the falling factor (1 + |Δz_o/Δ_o| when it
  was falling and was stopped, else 1):

```math
E' = \frac{\Delta_p^2\cdot 85 + \Delta_o^2\, m_o\, L}{85}\cdot 4
```

  read in the `player` table (150² → 5, 250² → 10, 450² → 20, 550² → 50,
  700² → 100, 1000² → 500), with the < 2 kg → 0, < 5 kg capped at 5 and
  slow-object filters. A 30 kg crate stopped dead from 1000 in/s does
  500; from 600 in/s falling onto a head (L = 2) also 500; stopped from
  300 level, 10.
- Stress crushing (prop_damage.md §6 "Stress") uses the same scale: the
  received stress is × 4 × 0.125 / 85, so the 200-damage crush needs a
  quarter of CS:S's contact force.
- Excluded: held objects (no damage at all), the player's own ground
  entity, ragdolls marked dying, motion-disabled bodies; a combine ball
  flagged to dissolve does 1000 dissolve instead (H4).
- **Who is credited.** Every gravity-gun or `+use` touch — grab, drop,
  launch, punt, throw, and the drop at the holder's death — sets the
  prop's influence to (that player, now). An impact on a player within
  **1.0 s** of the influence names that player the attacker (inflictor
  stays the prop, so the death notice's weapon is `physics`, rules.md);
  later impacts have the prop itself as attacker: a world kill (victim
  −1). Holding doesn't refresh the influence (it was set at the grab) but
  held objects deal no damage, so the window effectively starts at the
  release.
- **Self damage**: the holder is spared only if the influence time equals
  **now** (the same tick). A prop punted or launched into a nearby wall
  that rebounds into its punter a tick or more later hurts him, credited
  to himself (a suicide if it kills: frag −1).
- **Team damage**: in TDM with `mp_friendlyfire` 0 an impact credited to
  a teammate is refused entirely (rules.md "Friendly fire"); an
  uncredited prop always hurts.
- Classes with an influence: `prop_physics` and its variants
  (`_override`, `_multiplayer`, `_respawnable`), `func_physbox`,
  `func_breakable` pushables, gibs; anything else (weapons, items,
  grenades) never credits a player through impacts (a frag credits its
  thrower through its blast instead, section 11).
- **Explosive props**: a prop that breaks and explodes names as blast
  attacker its **last attacker** (any player who damaged it — a punt
  counts, section 2 step 1 — kept until replaced), else its influencer if
  the influence is at most **2.0 s** old, else the damage's attacker.
- Armour absorbs crush and dissolve as usual (suit.md).

**Prop damage from the gun**: a punt deals no damage to physics props
(0 physgun); a non-physics target takes 1 crush. A prop **launched from
the hold** takes **10 ×** its own computed impact damage on its first
collision after the launch (not after a punt or a `+use` throw); props
with the "onfirstimpact break" interaction (the explosive oil drum, 68
placed; the propane tank; the gas can) break on that first collision
whatever its speed, and "paintsplat" paint cans decal what they hit.

### 10. Claws, effect states and sounds (behaviour only; visuals H9)

- **Claws** (an open/closed flag, networked and predicted): every frame while not
  holding and not suppressed, a ±4 box trace of length 250 from the eye
  (ignoring the player and what it owns) looks for something that passes
  the hold test of section 3 step 4. Found → claws open at once
  (`Weapon_PhysCannon.OpenClaws`, idle animation, view-model pose
  "active" → 1 over 0.2 s). Not found → after the claws have been
  without a target for 0.5 s they close (`Weapon_PhysCannon.CloseClaws`,
  pose → 0 over 0.5 s). A punt schedules closing 0.5 s later, a launch
  0.1 s later, both suspend the check 0.25 s; a grab opens them; a failed
  grab or a pull closes them; deploy closes them.
- **Effect states** (networked): none (holstered), closed, ready (idle
  glow), holding (beams from the claws, brighter core, the
  `Weapon_PhysCannon.HoldSound` loop — client-local, only while the
  claws are open — rising to pitch 100, volume 0.8 over 0.5 s), launch
  (a `PhyscannonImpact` effect: a beam from the muzzle to the point and
  sparks; the punter's own copy is predicted).
- **Sounds** by entry (`hl2mp`'s `scripts/game_sounds_weapons.txt`):
  `Weapon_PhysCannon.Launch` (punt/launch), `.DryFire`, `.Pickup`,
  `.Drop`, `.OpenClaws`, `.CloseClaws`, `.TooHeavy`, `.HoldSound`;
  `.Charge` is defined but unused. `+use`: `HL2Player.Use`,
  `HL2Player.UseDeny` (HL2's `game_sounds_player.txt`). Held combine
  ball: `NPC_CombineBall.HoldingInPhysCannon`, `NPC_CombineBall.Launch`.
- Idle animation: `ACT_VM_RELOAD` (the "shake") while holding,
  `ACT_VM_IDLE` otherwise.

### 11. Special objects

- **Frag grenades** (`npc_grenade_frag`): when the gun grabs or punts one
  its **thrower becomes the gun's user** (blast credit moves) and its
  fuse is **reset to 1.5 s** from that moment (shorter or longer than
  what was left), with a blip at once and fast blips (0.3 s) after. A
  primed grenade can thus be caught and returned, owned by the catcher.
- **Satchels and S.L.A.M.**: satchels are physics objects (grab, punt;
  the owner and detonator don't change); tripmines are pinned, not
  grabbable; a punt deals them 1 damage (H4 says what that does).
- **Items and weapons** are physics objects ≤ 250 kg: they can be pulled
  and held; a held item's touch box grows from 24 to 48 units so pulling
  it into oneself takes it (H2's rules; Open questions 6). Weapons
  collide with no player, so a held weapon passes through everyone.
- **Ragdolls**: player corpses (`hl2mp_ragdoll`) exist only on clients
  and can't be targeted; no stock map places `prop_ragdoll`.
- **Debris** (`prop_physics` spawnflag 4, 219 placed) can be grabbed;
  it becomes interactive debris, which in HL2:DM collides with players.

### 12. The combine ball in the gun

(The ball itself is H4's: radius 10, 4 s life, bounces.)

- **Catch** (secondary): any combine ball passes the hold test with no
  mass limit. On the grab: its owner becomes the catcher (kills credited
  to him); its trail goes dark; it switches to the debris group (touches
  only the world while held), mass 20, its life timer stops; the hold
  loop `NPC_CombineBall.HoldingInPhysCannon` starts; **8 s after the
  catch it explodes in the gun** (the loop's pitch ramps to 150 over the
  last 1 s). A ball with "unlimited bounces" gets 0 bounces left.
- **Launch** (primary): released (velocity zeroed), its target speed set
  to 1500, mass 750, life restarted (4 s), `NPC_CombineBall.Launch`; then
  section 7's velocity for 750 kg is added: it leaves at **959.2 in/s**
  straight along the view, and its first bounce resets its speed to its
  target, 1500 (Quirks; Open questions 3).
- **Drop** (secondary): the same, but the velocity is set to 1500 along
  the view direction turned by whole-degree offsets U{−55..55} in pitch
  and in yaw.
- **Punt** (primary at a free ball): its target speed := its current
  speed, owner := the punter, mass 750, life restarted; then 959.2 in/s
  along the unflattened view is **added** to its current velocity; a
  faster arrival raises the stored speed, so it keeps that speed through
  its bounces (measured, Open questions 14).

### 13. `prop_physics_respawnable` (329 placed)

A `prop_physics` that comes back after it breaks.

- Keys: `RespawnTime` (s; 0 or absent → **60**). Stock values: 60 × 229,
  6000 × 68 (effectively never), 15 × 12, 90 × 11, 120 × 5, 5 × 3,
  25 × 1. `physdamagescale` 0.1 × 232, 1 × 97. Spawnflags as
  `prop_physics` (256 on 105, 64 and 8 on 33, 1 on 44, 2 on 33, 4 on 14).
- At spawn it stores its origin, angles and box, and clears its owner.
- **Only breaking** (lethal damage) respawns it: it breaks as any prop
  (gibs, sound, explosion), loses its physics body, becomes invisible,
  is moved back to the stored origin and angles, and is **not removed**.
- After `RespawnTime`: if its stored box at the stored origin overlaps
  anything solid (players included, itself excluded) it retries every
  1 s; else it becomes visible and spawns afresh: health, physics, motion
  flags and its stored spawn state as at map start.
- Moving it without breaking it never sends it back; it stays wherever
  it ends.

### 14. What the server simulates and what clients see

- All physics (punt impulses, pulls, the hold, launches, impacts, damage,
  credit) is server-side. The gun is a predicted weapon: the holder's
  client runs the same frame for timers, animations, sounds, the claws
  and the launch effect; grabbing, pulling and punt physics are server
  only.
- Networked from the gun: held (bool), the held entity, the anchor point
  (coordinates), the stored relative angles (3 × 11-bit angles), the
  effect state and the claws (the last two predicted).
- **The holder's client** builds its own local physics body for the held
  entity when it learns of it, attaches it to a copy of the hold
  controller (same target rule, with a fixed 2-tick time to arrive), and
  updates it every frame from the predicted view, so the object follows
  the view without waiting for the server; when the hold ends (or the
  gun loses its owner) the local body is destroyed and the object
  returns to plain interpolation. Other clients see only the networked
  origin and angles of the object (interpolated as any server prop,
  physics_props.md §6). Open questions 4.

## Per-tick order

For each player command (weapons.md's order), then the physics frame:

1. Pre-think: if the gun holds something, update the hold target
   (section 5, once per server frame), possibly dropping it.
2. `+use` search (doors_buttons.md) and movement (H1). A change of the
   ground entity onto a held object forces that player's drop at once.
3. Post-think: a `+use` carry (allowed only by the cvar) is serviced
   (drop / throw / target update).
4. The gun's frame: (a) not holding: claw check and pending claw change;
   (b) secondary held and not masked → secondary routine (drop if
   holding and freshly pressed; else search: grab / pull / fail); else
   reset the deny latch and, not holding, effect "ready"; (c) secondary
   up → unmask; (d) primary held → primary routine (launch, or punt /
   dry fire); else idle animation.
5. Physics frame (after all commands of the tick): hold controllers and
   contact slide, integration, collisions; impact damage (props and
   players) is queued and then applied in collision order, with credit
   decided at application from the influences current then.
6. Thinks: respawnable props, ball timers, grenade fuses.

## Edge cases

- Secondary at a prop beyond 1000, or with the line blocked by the world
  and nothing in the 251 cone: TooHeavy once per press, claws close.
- The pick-up trace hitting a brush entity (a door) or a player: candidate
  fails the test → TooHeavy (no pull of a player).
- A prop exactly at 250 (fraction 0.25): grabbed. Just beyond: pulled.
- An object grabbed through the cone may be off-centre by up to 14.07°.
- Constraint-attached props (`phys_ballsocket`, 16 in the stock maps;
  `phys_hinge` 1; `phys_lengthconstraint` 1) can be grabbed unless hinged
  (the constraint holds them); punts push them against the constraint.
- `func_clip_vphysics` (32 placed) blocks held and thrown objects but not
  players.
- A held object broken by gunfire (its physics body gone): the hold ends
  (Drop sound) on the next target update.
- A prop held by one player and punted by another: stays held; error may
  grow and drop it; influence → the punter.
- Holding while the holder is teleported: the target jumps with him; the
  object chases at ≤ 1000 in/s and is dropped by the error rule if far.
- `prop_physics_multiplayer` (114 placed, 92 on dm_underpass) is solid to
  players in HL2:DM (Corrections 1): it can be stood on, pushed by the
  player shadow, and it hits players.
- Grabbing then launching on the next tick: allowed (no primary delay).
- Two players grabbing the same free prop in one tick: the first command
  processed wins; the second sees it "player held" → TooHeavy.

## Quirks (keep all)

- **Prop flying is impossible for the holder**: he can't stand on his own
  held object (instant drop) and passes through it; but **another
  player can ride** someone's held object (lifts, "prop elevators").
- **Held objects as shields**: they stop enemy bullets and bodies, not
  the holder's own shots.
- **Punting a downward-aimed prop sends it up** (z × −0.65).
- **The light-object punt cap**: everything under 10.7 kg leaves at the
  2000 in/s physics cap.
- **The error dead zone**: a held object stuck 15–24 units from its
  target never drops; 12–15 and > 24 drop (the halving of large errors).
- **Early second click**: a secondary press within 0.5 s of a grab is
  lost; holding the button on then plays TooHeavy and closes the claws
  while the object stays held.
- **TooHeavy on empty space**: every fresh secondary press that finds
  nothing grabbable (a wall, the sky, a player) plays the TooHeavy sound.
- **Instant draw**: the gun works the tick it is selected.
- **Caught balls are slow until they bounce** (959 then 1500).
- **A punted ball stays fast**: its speed after a punt (1959) is its speed
  through every bounce (Open questions 14).
- **Punting a grenade shortens its fuse to 1.5 s** even if it had more.
- **Self prop-kills count as suicides** after the first tick.
- **A punt makes you the prop's last attacker** for good: a barrel you
  once punted and that later explodes from a fall credits you.
- **Launch speed rises again past 600 kg** (unclamped spline); only
  reachable by the combine ball (750).

## Corrections to H0

1. **H0 1.5 ("push-away and the player shadow: same")** and
   physics_props.md §4.2: in HL2:DM the push-away collision group
   (`prop_physics_multiplayer`, 114 placed) **collides with players**
   (HL2:DM's rules drop the CS:S exception), and HL2:DM's player calls
   neither the server push-away nor the client push-back. Multiplayer
   props in server modes 1 and 2 therefore behave as solid `prop_physics`
   to players (blocked movement, shadow pushing, impact damage);
   client-side ones (mode 3) exist on clients only. Interactive debris
   also collides with players in HL2:DM.
2. **H0 Part 1, prop_damage.md "same"**: in HL2:DM the break routine
   itself doesn't remove the prop; only the lethal-damage path does (and
   `prop_physics_respawnable` overrides that to hide and respawn). A
   prop broken by the `Break` input (or any non-damage break) stays in
   the world, non-solid and undamageable (P15).
3. **H0 1.5 / prop_damage.md §6 energy scale**: confirmed 4 for HL2:DM's
   players (CS:S 1); prop_damage.md's `player_impact_scale` 1 and its
   worked numbers are CS:S's. The stress rule uses it too (section 9).
4. **Wave 1, weapons.md §2/§3 and test 1** ("the physcannon ends in hand
   (draw 22 ticks)"): the gun's deploy resets the next primary,
   secondary and attack times to now; it can be used on the deploy tick.
   Its draw length matters only for the animation.
5. The plan's brief line "drop on damage": there is no such rule in
   HL2:DM (the force-drop on a stunstick hit is the NPC stunstick's
   path). Damage never makes a holder drop.
6. H0's cvar table row (`physcannon_*` 250, 250, 700, 1500, 4000, 0.97,
   2, `player_throwforce` 1000) and `hl2mp_allow_pickup` 0 are
   confirmed; add that `physcannon_chargetime` and `physcannon_ball_cone`
   are unused, and `hl2mp_allow_pickup` is a server-only, non-cheat cvar
   while the others are cheat-protected and replicated.
7. rules.md Open questions 5 is answered by section 9 (1 s influence,
   2 s for a breaking prop's blast, last attacker otherwise).

## Test cases

Implementer checks, no game needed. Tick 0.015, defaults, flat ground,
player origin (0, 0, 0) unless said, eye (0, 0, 64), view +X level.
"Crate" = a single-body box prop, ±10 units, origin at its centre, 30 kg
unless said.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | crate at rest, centre (100, 0, 64) | primary | punt tick | Δv = 1100 in/s along +X (15 000 + 18 000 = 33 000 / 30), no spin (hit through the centre line); next primary and secondary +34 ticks |
| 2 | as 1, crate 10 kg / 250 kg / 500 kg / 1000 kg | primary | – | 2000 (capped from 2100) / 660 / 330 / 165 in/s |
| 3 | crate centre (100, 0, 0), view pitch +45 (down) | primary | – | d = (0.7071, 0, +0.4596); Δv = 1100 · d = (777.8, 0, 505.6), speed 927.7 |
| 4 | crate at 260 units | primary | – | box and line traces miss within 250 → dry fire (`ACT_VM_PRIMARYATTACK`, `.DryFire`); crate unchanged; next primary +34, secondary unchanged |
| 5 | another player at 100 | primary | – | dry fire; the player's velocity and health unchanged |
| 6 | player between the punter and a crate | primary | – | dry fire (the box trace stops on the player) |
| 7 | `func_breakable` health 1 at 100 | primary | – | 1 crush damage from the punter → breaks; no next-secondary change |
| 8 | crate, primary held | 3 s | – | punts on ticks 0, 34, 68, … (whether or not they hit) |
| 9 | crate punted at tick 0 by A | A's gun timers forced ready at tick 10, primary | – | nothing (repunt guard until tick 34) |
| 10 | crate centre at 200 | secondary | 1 tick | grabbed: held, `.Pickup`, claws open; body mass 1 kg, rot damping 10, drag off; next secondary +34; owner = holder |
| 11 | crate at 400 | hold secondary | 21 ticks | pulls on ticks 0, 7, 14, 21: each Δv = 4000 · 30.5 / 50 / 30 = 81.33 in/s toward the eye; TooHeavy never plays |
| 12 | crate 10 kg / 50 / 250 at 400 | one pull | – | Δv 84 / 80 / 16 in/s |
| 13 | crate 300 kg at 100 | press secondary twice (release between) | – | TooHeavy on each press, once; no pull, no grab |
| 14 | facing a wall at 50, nothing else | press secondary | – | TooHeavy once; next secondary +7 ticks; claws closed |
| 15 | standing on a crate, looking down at it | secondary | – | TooHeavy (ground entity) |
| 16 | crate with spawnflag 8 / 8 + 64 | secondary | – | refused (TooHeavy) / grabbed and its motion enabled |
| 17 | crate with spawnflag 512 | secondary / primary | – | refused / punted normally |
| 18 | crate held, e = 10, no wall | – | target | R = 32.627, D = 89.25; box centre target (56.627, 0, 64) |
| 19 | as 18, wall plane at x = 30 | – | target | blocked at 30 < 44.63 → P = eye + 16.31 f, then pushed to horizontal 32.627: (32.627, 0, 64) |
| 20 | as 18, view pitch 89 (down) | – | target | pitch used 75; P = eye + 56.627 (0.2588, 0, −0.9659) = (14.66, 0, 9.30) → pushed out to (32.627, 0, 9.30) |
| 21 | as 18, view yaw turned 90° left in one tick | – | 1 tick | target (0, 56.627, 64); object angles turned by 90° about Z with the view; linear correction speed limited to 1000 in/s |
| 22 | crate held | primary on the tick after the grab | – | launched: velocity 1500 along f, spin each axis in [−600, 600] deg/s; masses restored; next primary/secondary +34; `.Launch`; view punch pitch −6 |
| 23 | held crate of 150 / 200 / 250 kg | primary | – | 1477.6 / 1416.8 / 1327.2 in/s |
| 24 | holder running 190 in/s along +Y, crate held | primary | – | crate velocity exactly 1500 f (the holder's motion not added) |
| 25 | crate held 1 s, moving at 400 in/s with the view | fresh secondary press | – | dropped: speed clamped to 285, spin ≤ 720; `.Drop`; next primary/secondary +34; owner cleared; influence (holder, now) |
| 26 | grab at tick 0 | press secondary at tick 20, keep it held | – | no drop at 20; at tick 34 TooHeavy once, claws close; still held; again every 7 ticks (no sound) |
| 27 | crate held, kept 48 from its target by a wall from tick 100 on (grab at 0) | – | – | error clock positive from tick 67; error reaches 12 at tick 100 + 46 → dropped with `.Drop` |
| 28 | as 27 with a constant gap of 20 | – | 10 s | never dropped (error → 10) |
| 29 | crate held | holder jumps onto it | ground change | dropped at once |
| 30 | crate held by A | B stands on it | – | not dropped; B rides it |
| 31 | crate held by A | A walks into it; A shoots through it; B shoots it | – | A passes through; A's bullets hit what is behind; B's bullets stop on it and push the 1 kg body |
| 32 | crate held by A | B punts it | – | still held by A; impulse on a 1 kg body (capped speed); crate's influence (B, now) |
| 33 | crate held | `slot2` / `lastinv` | – | refused (gun can't be put away) |
| 34 | crate held, last weapon SMG1 | `phys_swap` | – | dropped (`.Drop`), SMG1 deployed |
| 35 | gun selected from the pistol | primary on the deploy tick at a crate | – | punt happens that tick |
| 36 | holder killed holding a crate moving 400 in/s | – | – | crate released with speed 285; influence (dead holder, death time); gun deleted |
| 37 | standing player (shadow unmoved), 30 kg crate arrives at 1000 in/s and stops dead | impact | – | E′ = 1000² · 30 / 85 · 4 = 1 411 765 → 500 crush |
| 38 | as 37, crate keeps 400 of 1000 / stops from 300 | – | – | E′ 508 235 → 100 / 127 059 → 10 |
| 39 | 30 kg crate falling at 600 in/s stops on a head | – | – | L = 2: E′ = 1 016 471 → 500 |
| 40 | 4 kg can at 1000 stops / 1.5 kg at 1000 / 30 kg at 20 in/s | – | – | 5 (small cap; table gives 10) / 0 (< 2 kg) / 0 (slower than 24) |
| 41 | crate punted by A at t, kills B at t + 0.9 s / t + 1.1 s | – | – | attacker A, weapon `physics`, A +1 / no player attacker: world kill, B −1 |
| 42 | A launches a crate into a wall 40 units away; it rebounds into A 5 ticks later with 600 in/s | – | – | A takes damage (≥ 100 at a dead stop), attacker A; if lethal: suicide, A −1 |
| 43 | TDM, `mp_friendlyfire 0`, teammate's crate credited 0.5 s ago | impact | – | no damage; at 1.5 s: damage, world-attacked |
| 44 | crate held by A pushed into B at 1000 in/s | – | – | 0 damage (held objects never damage) |
| 45 | explosive drum punted by A (no other damage), falls 30 s later and explodes | – | – | blast attacker A (last attacker) |
| 46 | explosive drum launched from hold by A | first collision | – | breaks at once (first-impact break), explodes; blast attacker A |
| 47 | `hl2mp_allow_pickup 0`, 20 kg crate within use reach | `+use` | – | `HL2Player.Use`; crate not moved, weapon unchanged |
| 48 | same, 100 kg crate, nothing else usable | `+use` | – | `HL2Player.UseDeny` |
| 49 | `hl2mp_allow_pickup 1`, 20 kg crate | `+use`, then primary | – | weapon holstered, sprint off; throw Δv = 200 in/s along f (k = 4) plus the clamped carry velocity; weapon redeployed |
| 50 | as 49 with 1 kg / 10 kg / 40 kg / a 10 kg plank 130 long | `+use` | – | thrown at 620.7 / 279.3 / refused (deny) / refused |
| 51 | `prop_physics_respawnable` without `RespawnTime` | break it at t | – | invisible at once at its spawn spot; visible and solid at t + 60 s |
| 52 | as 51, a player standing on the spot at t + 60 | – | – | retries at t + 61, t + 62, … until clear |
| 53 | respawnable pushed 300 units away, never broken | 10 min | – | stays where it is |
| 54 | live frag with 2.3 s left | grab it with the gun | – | thrower = grabber; explodes 1.5 s after the grab |
| 55 | combine ball (owner B) within 200 | A secondary | – | held; owner A; mass 20; explodes 8 s after the grab if kept |
| 56 | A holds the ball | primary | – | velocity 959.2 along f; after its first bounce 1500; life 4 s from the launch |
| 57 | A holds the ball | secondary press | – | ball leaves at 1500 within ±55° (whole degrees) of the view in pitch and yaw |
| 58 | pointing at a 30 kg crate at 200 | – | 1 tick | claws open (`.OpenClaws`); look away at tick 10 → closed on tick 44 (`.CloseClaws`) |
| 59 | `item_battery` held by the gun | – | – | its touch box ±48 beyond its bounds (24 when released) |
| 60 | `prop_physics_multiplayer` crate (mode 1) | walk into it | – | blocked like a `prop_physics` (Corrections 1) |

Checks on the install or the probe server (game build 24600434,
dedicated server build 24600426), marked **(probe)**:

| # | How | Expected |
|---|---|---|
| P1 (probe) | RCON: the `physcannon_*` cvars, `player_throwforce`, `hl2mp_allow_pickup`, `phys_impactforcescale`, `sv_turbophysics`, `breakable_multiplayer` | 250, 250, 700, 1500, 4000, 0.97, 0.997, 2; 1000; 0; 1; 0; 1 |
| P2 (probe) | spawn a crate with `overridescript mass,30` and others at 10, 100, 250; punt each level through its centre; read its velocity next tick | 1100, 2000, 750, 660 |
| P3 (probe) | punt a crate looking 45° down | vertical speed > 0 (≈ 506 for 30 kg) |
| P4 (probe) | fake client grabs a 30 kg crate, looks level; log the crate's centre relative to the eye over 1 s | settles ≈ 24 + 22.6 + its depth ahead at eye height |
| P5 (probe) | launch held crates of 30 and 250 kg | 1500 and ≈ 1327 in/s; the holder's speed not added |
| P6 (probe) | drop a held crate moving fast (swing the view) | speed ≤ 285 right after |
| P7 (probe) | hold a crate against a wall, measure the gap and time to drop | drops for gaps > 24 and 12–15, not 15–24 (Open questions 2) |
| P8 (probe) | pull a 30 kg crate from 600: log its speed every tick | jumps of ≈ 81 in/s every 7 ticks |
| P9 (probe) | `+use` on a small crate, default server | click sound only; then `hl2mp_allow_pickup 1`: carried |
| P10 (probe) | drop the 30 kg crate onto a 5000-health bot from 300 units | ≈ 500 crush per contact (two contacts possible as CS:S's spec saw) |
| P11 (probe) | punt a crate into a bot at 0.5 s and at 1.5 s travel time (SourceMod damage hook) | attacker the punter / the prop |
| P12 (probe) | catch an AR2 ball, launch it, log its speed | ≈ 959 until the first bounce, then 1500 (Open questions 3) |
| P13 (probe) | break a respawnable prop with no `RespawnTime`, stand on its spot | reappears 60 s later, or 1-s retries while blocked |
| P14 (probe) | stand still and let a `prop_physics_multiplayer` crate be pushed into the bot | it collides and can damage (Corrections 1) |
| P15 (probe) | `ent_fire <prop> break` on a `prop_physics` | the prop stays (non-solid) or vanishes: Corrections 2 |
| P16 (probe) | press secondary at a wall | TooHeavy sound once per press |
| P17 (probe) | switch to the gun and punt in the same tick (bot) | punt happens (Corrections 4) |

## Open questions

1. **The hold controller's law.** The shadow-control computation (how
   velocity is chosen from the target, the time to arrive, the speed and
   damping limits) is inside the physics engine, not in the public code.
   The model in section 5 (velocity to reach the target in one tick,
   capped at 1000 in/s) should be checked by P4 with fast view swings
   (does the object lag by ≈ 15 units per tick at 1000 in/s?).
2. **Error smoothing at real tick timing**: the error update assumes one
   physics step per target update; with several commands per tick (lag,
   packet bursts) the clock can be 0 and the update skipped. P7 settles
   the dead zone.
3. **Ball speed after a launch from the gun**: the code adds the
   750 kg launch speed (959.2) and restores 1500 only on the first
   bounce; HL2:DM players remember caught balls as fast. P12 decides;
   H4's AR2 ball mass (150 by cvar) may also change the mass seen here if
   the game sets it after the gun's 750.
4. **Client view of the held object**: whether the holder's screen draws
   the object at its local physics copy or at the interpolated server
   position (and how the two are blended) is client engine behaviour;
   compare a held object's on-screen lag in a capture at 100 ms ping.
5. **The first second after a grab**: the contact factor starts at 0, so
   angular control starts at 0 and ramps up (c³); how visible this is
   depends on question 1.
6. **Held items and weapons touching the holder**: whether a held item
   whose touch box (48) reaches the holder is taken while held (the item
   rules need the item at rest after a drop in some paths). Probe: pull
   a battery into yourself.
7. **Punted tripmines**: the 1-damage punt on a non-physics tripmine
   should kill it (health 1); whether the tripmine's death path explodes
   it, and whose kill that is, is H4's (the punter is the attacker).
8. **`func_physbox` gun flags** ("never pick up", "never punt",
   "always pick up"): HL2:DM's gun checks none of them except through the
   generic "no player pick-up" body flag; the 11 stock physboxes' flags
   were not checked against this. H10 should list them.
9. **`Break` input on a `prop_physics`** in HL2:DM (Corrections 2): the
   prop should remain, non-solid; confirm (P15), since map logic may
   rely on it.
10. **Integer yaw of the fire punch** uses the shared random stream with
    a label; tests should check the range {−2, …, 2}, not values.
11. **Probe results (2026-11-03, dedicated server build 24600426,
    `tools/hl2dm_probe/cases.py gravity`).** Answered:
    - P3: a crate hanging motion-disabled with flag 64 (nothing under it)
      punted looking 45° down leaves at (772, 488) a tick after, the
      spec's 1100 × (0.707, 0.65 × 0.707) less a tick of drag and gravity;
      21.8° down at its face (1011, 252) for (1021, 266). The **point
      impulse acts at the trace's end** — the box's centre where the ±8
      box stopped, not the contact point on the crate's face: the crate
      pitches at 2964°/s (the trace end's lever predicts 2960, the
      contact point's 2150). Level punts 5, 10 and 15 above the centre
      pitch it at 176°/s per unit: its rotational inertia is 0.714 of a
      solid box's (as CS:S's measured "70-80 %"). The round-1 case, a
      crate on the floor, leaves at (827, 577): its leading bottom edge,
      spun into the floor, is pushed up and forward (Chimera's solver
      gives (771, 813) for that case: Open, a physics-engine contact
      difference).
    - P6: a held crate swung at 816 and 1000 in/s and dropped leaves at
      283 and 280 (the 285 cap).
    - P10, P11: a crate punted into a bot at about 1050 does 50 and 10
      crush in two contacts on one tick, credited to the punter; a 30 kg
      crate dropped 300 onto a bot's head does 10 then 20 (from 150: 5
      then 10), not test 39's 500 (Chimera: 60 for the punt, as the
      server; 20 for the drop).
    - P16: the TooHeavy sound once per press (held 60 ticks: once).
    - P17: selecting the gun and pressing primary on one command punts
      that tick (Corrections 4).
    Not measured: P7, P8, P12-P15.
12. **Probe results, round 3 (2026-11-04, build 24600426,
    `cases.py crate_hits floor_punts use_carry`).**
    - P10 (test 39 is wrong: a crate stopped by a head is not stopped
      dead): crates let fall onto a standing bot's head (5000 health) hurt
      it as two crush contacts on one tick, credited as usual. By mass and
      the crate's speed on its last free tick: 30 kg at 284, 389, 524,
      663 in/s: 5 + 10, 5 + 10, 10 + 20, 10 + 50; 10 kg at 420: one 10;
      100 kg at 572: 20 + 100 (and a 5 twenty ticks later), at 780:
      100 + 500; 250 kg at 582: 100 + 500. Read through the energy formula
      (scale 4, falling factor 2) each contact takes a share of the speed
      into the player: the first 0.29-0.37, the second 0.51-0.61 at 30 kg,
      over 0.6 at 10 kg, 0.41-0.56 at 100 kg (CS:S's 30 kg crate's second
      share 0.43-0.52 overlaps at about 0.51). A heavy crate keeps on the
      head after its landing (no second landing within 1.5 s).
    - Crates hanging at eye height punted level into a bot 150 ahead
      (two or three contacts on the hit's tick, all credited to the
      punter): 10 kg at 1500: 20 + 10; 30 kg at 1000: 20 + 10 + 5; 100 kg
      at 730: 50 + 10 + 10 (10 more twenty ticks later); 250 kg at 650:
      100 + 5 + 10, then 50, 10, 5 over the next 20 ticks. (Round 2's
      punt at 1050 into a bot from closer: 50 + 10.)
    - P3 on the floor: the 30 kg crate resting on the floor (centre 20.4
      up) 40 ahead, punted looking 45 degrees down: its first tick's step
      (773, 898) as its leading edge digs into the floor, then (695, 20,
      871) (1114 in/s, 51.4 degrees up), pitching forward at about
      1490 degrees/s; 30 down at 60 ahead: (944, 803), then (781, 777),
      2130 degrees/s.
    - P9 and section 8: with `hl2mp_allow_pickup` 0 a 20 kg crate clicks
      (`common/wpn_select.wav`) and stays; a 40 kg one denies
      (`common/wpn_denyselect.wav`) whatever the cvar. With 1 the 20 kg
      crate is carried (the click again, no other sound): it reaches the
      hold point within 4 ticks at under 1000 in/s, 66.5 ahead at eye
      height looking level, (63.9, 37.0) looking 30 up and (63.8, −36.8)
      30 down, keeping its own attitude (level); it turns with the yaw.
      Primary throws it: 199 in/s (20 kg) and 276 (10 kg) along the view a
      tick after, 469 for 1 kg (the 620.7 less its heavy drag); the
      weapon in hand (here the SMG1) isn't switched away while carrying,
      doesn't fire, and is deployed again at the throw (its next attack
      0.8333 s on: the draw); with primary held at the `+use` press the
      crate is thrown at once. `+use` again or secondary drops it from
      rest. Sprint held while carrying runs at 190.
13. **Probe results, round 4 (2026-11-04, build 24600426, `cases.py
    crate_hits gravity_more`).**
    - Punts into a player (P10, P11's horizontal hits; the bot's damage
      lines and its position tracked): a crate 50 ahead of a bot 200
      away, punted level: 10 kg at 1500: 20 + 10; 30 kg at 1000:
      20 + 10 + 5; 100 kg at 730: 50 + 10 + 10; 250 kg at 650:
      100 + 5 + 10; the 30 kg crate with the bot 100 away (at 1100):
      50 + 10 + 5. The crate keeps on forward after the hit (10 kg about
      0, 30 kg 165, 100 kg 337, 250 kg 445 in/s; near a plain inelastic
      collision with an 85 kg body) and the bot is pushed back: 650 in/s
      on the hit's tick for 30 kg, 1110 for 100 kg, slowing on the floor.
    - P14: a `prop_physics_multiplayer` crate punted into a bot hurts it
      as a `prop_physics` one: 30 kg 20 + 10 + 5, 100 kg 50 + 10 + 10
      (Corrections 1).
    - P8: secondary held at a 30 kg crate resting 600 ahead (looking at
      it): its floor speed 75.7 on the first tug's tick, then 72.0,
      66.2, 62.4, 57.3, 51.9, 46.6, 121.8, ... (each tug 81.3 less a
      tick of floor friction, about 5.5 in/s, every 7 ticks); 10 kg:
      78.2, ... 125.9 (84 less friction). As section 3.
    - P12 (Open questions 3): an AR2 ball caught by the gravity gun as it
      left the catcher's own gun and launched flew at 959 until its first
      bounce, then 1500.
    - P15 (Open questions 9): the `Break` input on a 30-health wood crate
      leaves it where it was, drawn (effects 0), solid flags 4 (not
      solid), take-damage 0, health 30; nothing created or removed on
      the server.
    - P13: a `prop_physics_respawnable` crate with no `RespawnTime` shot
      with the .357 is hidden at once (effects 32, put at its spawn
      origin) and drawn again 4005 ticks (60.08 s) later; with the bot
      standing on its spot it came back 402 ticks (6 tries 1 s apart)
      after the 60 s, 68 ticks after the bot left.
    - P7: a 20 kg crate held level against a frozen 200 kg crate whose
      edge stopped it about 41.6 from the hold point dropped 57 ticks
      after the 1 s grace (the formula's 57 for e = 41.6, halved). Held
      14.0-14.4 from it (by our estimate of the hold point, 66.8 ahead)
      it never dropped in 333 ticks after the grace (the formula: 114 to
      129); held 11.6 from it it dropped after 266. Our estimate of the
      gap may be off by about a unit and the hold point by more (the
      contact keeps the crate a little off the frozen one), so the small
      gaps don't settle the 15-24 band; the measured 41.6 case fits.
    - P3 on the floor (round 3's 51.4 degrees and 1490 degrees/s;
      Chimera 46 degrees, 1670): not closed. In Chimera the floor
      contact's restitution decides it (0: 43 degrees, 2220 degrees/s;
      the crate's 0.14: 46, 1670; 0.3: 49, 1190; 0.5: 52, 590), the
      friction not at all, and no value gives both the climb and the
      spin; the first tick's step is (773, 728) where the real crate's
      is (773, 898): vphysics' edge contact acts within the punt's tick.
14. **Probe results, round 5 (2026-11-08, build 24600426, `cases.py
    phys_rules phys_rules2`).**
    - Punts into a player, the bot's motion by tick (a crate 50 ahead of a
      bot 200 away; hits on tick t): its step t to t + 1, then t + 1 to
      t + 2, in/s forward: 10 kg 340, 263; 30 kg 199, 454; 100 kg 320, 775;
      250 kg 253, 922, then 1094; the 30 kg crate with the bot 100 away 47,
      568. Its velocity on t + 1 reads 454 (30 kg) and 795 (100 kg), then
      falls 6 % a tick (friction). Round 4's 650 and 1110 are the first two
      steps added (199 + 454, 320 + 775), not a speed. Less the damage's
      shove (5 per point: 150, 175, 350, 575) the bot gains what a hit on
      85 kg with an elasticity of about 0.07 gives: 158-190, 279, 445, 519.
      The crate after, in/s: 8, 165, 337, 445; as a hit on 85 kg with an
      elasticity of about 0.15.
    - Open questions 6: a battery on the floor grabbed from 100 and 130
      ahead is taken 4 and 6 ticks after the grab (armour + 15, the battery
      sound and the gun's drop sound on one tick): on its way in at 1000
      in/s, as a touch box grown 48 reaches it (about 69 from the bot's axis)
      one tick earlier and one grown 24 (45) never would (it stops 51 away).
    - Section 12, a free ball punted: an AR2 ball (1000 in/s) a few ticks
      after it left the bot's own gun, punted along its flight: 1959 at
      once (959.2 added), and **1959 through every later bounce**: the
      stored speed is raised by the faster arrival (projectiles.md 5.2),
      not "restored".
    - Section 4 step 4 and 7: `models/props_junk/sawblade001a.mdl` held by a
      bot looking level at yaw 180 reads (−5, −180, 0); launched, its yaw
      turns 48.7 degrees a tick (3250 deg/s, not 5000: the 3600 cap, less
      drag), pitch and roll wobbling ±13. The propane tank
      (`preferred_carryangles` "-90 0 0") held likewise reads (−90, 0, 180),
      the same rotation as (−90, 180, 0), and its launch adds the random
      spin. The model data's keys: `physgun_interactions` `onlaunch`
      `spin_zaxis` or `spin_none`, `preferred_carryangles`.
    - Section 9: launched at a wall a 30-health wood crate and the
      explosive drum break on it; punted at it at about 1000 and 1100 in/s
      they don't (0 lost, the punt's 0-damage hit logged); a gas can punted
      took 10 from a later bounce.
