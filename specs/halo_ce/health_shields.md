# Halo: Combat Evolved: health, shields and damage

Source basis: the game's own map files from the original Xbox disc
(the multiplayer Spartan's collision model, the damage effects of every
multiplayer weapon, grenade and melee, the globals, the equipment; read
with throwaway scripts on Blood Gulch and checked identical on Hang 'Em
High and Damnation); and a decompilation of an Xbox build of the
game's engine (the damage, shield, unit damage reaction, stun,
powerup, camouflage, melee and kill credit code), used for meaning
only. The decompilation describes a different Xbox build (PAL
01.01.14.2342) from the disc's (NTSC 01.10.12.2276); where they differ
the disc wins and the difference is noted. Conventions (tag index,
blocks, tag references, units, axes, the tick) are in
[cache_file.md](cache_file.md); the collision model's layout is in
[collision.md](collision.md); the unit, biped and equipment tags in
[objects.md](objects.md). None of that is repeated here.
Status: draft

Sections: Summary; Units and conventions; Constants; The damage event
(the seam with projectiles.md); Where the numbers live; How vitality is
represented; Applying a damage event; Shield recharge; Health; The
overshield; Active camouflage; Stun and pings; Death; Melee; Falling;
What Chimera's hit model needs from this game; Per-tick order; Edge
cases; Quirks; Shots to kill; Test cases; Confidence per section; Open
questions.

---

## Summary

- A player has two vitalities, both stored as **fractions**: body
  (0 to 1, of 75 points) and shield (0 to 1 normally, up to 3 with an
  overshield, of 75 points per 1.0). Every hit goes to the shield
  first; damage the shield cannot hold carries over to the body. The
  body dies when it goes **below** zero (exactly zero survives).
- What a hit does depends on three numbers multiplied together: the
  amount (from the damage effect and the distance or speed scale), the
  collision model's multiplier for the material that was hit (shield
  or body), and the damage effect's own modifier for the material
  type (the Spartan's shield is type 22 "cyborg energy shield", its
  body type 21 "cyborg"). That is how the pistol does 25 points to
  shields but 37.5 to flesh, and how plasma does double to shields
  and half to flesh.
- Headshots matter only once the shield is gone: a damage effect
  flagged "can cause headshots" (pistol, sniper rifle) kills outright
  when any of its damage reaches the head's body; one flagged "can
  cause multiplayer headshots" (plasma rifle) does double body damage
  to the head. A melee from behind kills outright, shields or not.
- The shield waits 180 ticks (6.0 s) after the last hit, then
  recharges linearly over 120 ticks (4.0 s) from empty. The body never
  regenerates; only a health pack restores it, to full at once. An
  overshield charges the shield to 3.0 over 2 s (invulnerable while
  it charges) and the extra drains over 90 s.
- All arithmetic is single-precision and its order matters: the
  pistol's famous three-shot kill exists only because two shots leave
  the shield a hair below one third.

## Units and conventions

- Vitalities are fractions of the collision model's maximum (points =
  fraction × maximum). Damage amounts are in points.
- Time: 30 ticks per second. Tag durations are in seconds and are
  turned into ticks by multiplying by 30 in single precision and
  truncating toward zero (0.15 s → 4.4999 → 4 ticks; 6.0 s → 180).
- Arithmetic: every quantity is an IEEE single (f32) and every
  operation rounds to f32 in the order this spec writes it. Several
  results below depend on it (Quirks).
- "Tick 0" in the timelines is the tick on which a hit lands, with the
  hit applied before the victim's own update of that tick (see
  Per-tick order for the other case).
- Angles in radians.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| SPARTAN_BODY | 75.0 | points | maximum body vitality (collision model) |
| SPARTAN_SHIELD | 75.0 | points | maximum shield vitality (collision model) |
| SHIELD_DELAY | 180 | ticks | trunc(stun time 6.0 s × 30): wait after a hit before recharging |
| RECHARGE_STEP | 0.008333334 (0x3C088889) | fraction/tick | runtime shield recharge velocity, = 1/(4.0 s × 30) |
| RECHARGE_TICKS_FROM_EMPTY | 121 | ticks | increments from 0 to full (120 leave 0.9999994; the 121st clamps) |
| OVERCHARGE_STEP | 0.033333335 | fraction/tick | overshield charge-up rate (1/30) |
| OVERSHIELD_MAX | 3.0 | fraction | shield at the end of the charge-up |
| OVERSHIELD_DRAIN | 0.00074074074 | fraction/tick | drain of the part above 1.0 (1/1350: 2.0 in 2700 ticks = 90 s) |
| OVERSHIELD_FLOOR | 0.01 | fraction | the shield an overshield starts from if it was empty |
| NEGLIGIBLE | 0.0001 | points | shield damage below this does not count as "recently damaged" |
| DAMAGE_FADE | 0.016666668 | fraction/tick | fade of the current and recent damage values (1/60) |
| RECENT_HOLD | 60 | ticks | the recent damage values start fading this long after a hit |
| CAMO_STEP | 0.008333334 (f32 of 1/120) | per tick | camouflage fade in and out; 120 steps leave 0.9999994, so a full fade takes 121 updates (Active camouflage) |
| CAMO_FIRE_DEFAULT | 0.1 | | camouflage lost per shot when the weapon has no "active camo ding" |
| CAMO_FIRE_FLOOR | 0.05 | | firing never takes camouflage below this |
| MELEE_LUNGE_SCALE | 1.5 | | melee scale after more than 15 airborne ticks |
| MELEE_AIRBORNE_TICKS | 15 | ticks | airborne ticks beyond which a melee counts as a lunge |
| IMPULSE_LIFT | 0.45 | | added to a hit's direction's z before it becomes a push |
| PING_REAR_CONE | 0.78539819 | rad | |angle| below this: hit from behind |
| PING_FRONT_CONE | 2.159845 | rad | |angle| above this (123.75°): hit from the front |
| ATTACKERS | 4 | | attackers a unit remembers for kill credit |
| CREDIT_WINDOW | 180 | ticks | an attacker this recent can take the credit for a fall or suicide |
| ASSIST_WINDOW | 120 | ticks | |
| HEALTH_OPTION | 1.0 (clamp 0.25 to 4.0) | | game type "health": every hit is divided by it |
| MP_DIFFICULTY | normal (index 1) | | multiplayer reads every difficulty table at normal: all 1.0 |

## The damage event (the seam with projectiles.md)

`projectiles.md` owns the damage effect tag (`jpt!`: its layout,
category, flags, side effect, damage bounds, the per-material modifier
table, stun and impulse fields) and everything that decides who is hit
and with what scale (direct hit, area of effect, speed and distance
falloff). This spec starts when a damage event reaches one target
object. It assumes the event carries:

| Item | Meaning here |
|---|---|
| damage effect | the `jpt!` tag; this spec reads its: damage minimum, damage lower and upper bound, category, flags, side effect, "active camouflage damage", stun, maximum stun, stun time, instantaneous acceleration, vehicle passthrough penalty, and the material modifier for a given material type |
| scale s | 0 to 1 (1.5 for a lunging melee): speed scale of a direct hit, distance scale of an area hit, the melee's speed scale |
| multiplier m | 1 except for riders of a hit vehicle and game-type traits (below) |
| amount A | A = ((1 − s)·minimum + U·s)·m, U uniform in [lower bound, upper bound], drawn once per event from the game's random source; this formula is `projectiles.md`'s and is restated only so the examples can be checked |
| hit node, region, material | collision model indices of the surface hit (direct hits only); "none" for area damage, melee, falling and riders |
| area flag | set for area-of-effect damage and for the player's melee |
| epicenter | where the damage comes from (impact point, explosion centre, the attacker's centre for melee) |
| direction | unit vector of travel (from epicenter to the target's centre for area damage) |
| instigator | owner player, owner object and owner team (the projectile's or effect's owner); none for falling |
| silent, no statistics, kill instantly | flags from scripts and acts of god (not used by weapons) |

Back to the projectile: after the hit, this spec reports which material
type the impact effect should use and its scale: when the shield took
more of the hit (in raw points, below) than the body, the shield's
material type (22) and the shield fraction after the hit; otherwise
the hit material's type and the body fraction clamped to 0 to 1.

## Where the numbers live

### The collision model's vitality fields

The Spartan (`characters\cyborg_mp\cyborg_mp`) uses the collision model
`characters\cyborg\cyborg` (the same tag as the campaign Spartan).
Offsets are from the collision model tag's start, as listed in
`collision.md`; the units and the Spartan's values are this spec's.
Checked on Blood Gulch, identical on Hang 'Em High and Damnation.

| Offset | Type | Field (tag name) | Unit | Spartan | Meaning |
|---|---|---|---|---|---|
| 0x000 | u32 | flags | | 0x7 | see Collision model flags |
| 0x004 | i16 | indirect damage material | index | 1 ("body") | material used when the hit has no material (area, melee, falls, riders) |
| 0x008 | f32 | maximum body vitality | points | 75.0 | |
| 0x00C | f32 | body system shock | | 0 | not read by the damage code (no effect) |
| 0x044 | f32 | friendly damage resistance | fraction | 0 | friendly body damage is scaled by (1 − this) |
| 0x070 | ref `effe` | localized damage effect | | `characters\cyborg\localized damage` | played at the hit point when the event asks for it; no multiplayer damage source asks (never plays) |
| 0x080 | f32 | area damage effect threshold | points | 0 | |
| 0x084 | ref `effe` | area damage effect | | `effects\blood aoe human` | played on the object when an area hit's body amount exceeds the threshold (not for the flame category) |
| 0x094 | f32 | body damaged threshold | points | 0 | effect below plays once when body points drop below it (0: never) |
| 0x098 | ref `effe` | body damaged effect | | none | |
| 0x0A8 | ref `effe` | body depleted effect | | none | played on death |
| 0x0B8 | f32 | body destroyed threshold | points | 0 | if negative and body points fall below it, the object is destroyed (deleted); 0: never |
| 0x0BC | ref `effe` | body destroyed effect | | none | |
| 0x0CC | f32 | maximum shield vitality | points | 75.0 | 0 means no shield |
| 0x0D2 | i16 | shield material type | material type | 22 (cyborg energy shield) | selects the damage effect's modifier for shield damage |
| 0x0EC | i16 | shield failure function | transition function | 0 | see failing shields |
| 0x0F0 | f32 | shield failure threshold | fraction | 0 | 0: shields never fail |
| 0x0F4 | f32 | failing shield leak fraction | fraction | 0 | |
| 0x108 | f32 | minimum stun damage | points | 0 | shield damage at or above this restarts the shield delay |
| 0x10C | f32 | stun time | s | 6.0 | the shield delay |
| 0x110 | f32 | recharge time | s | 4.0 | empty to full |
| 0x184 | f32 | shield damaged threshold | fraction | 0 | effect below plays once when the shield drops below it |
| 0x188 | ref `effe` | shield damaged effect | | none | |
| 0x198 | ref `effe` | shield depleted effect | | `characters\cyborg\cyborg shield depletion` | played when the shield breaks |
| 0x1A8 | ref `effe` | shield recharging effect | | none | played when recharge starts after a break |
| 0x1C0 | f32 | runtime shield recharge velocity | fraction/tick | 0.008333334 (0x3C088889) | precomputed 1/(recharge time × 30); the engine uses this, not the time |

All other bytes from 0x010 to 0x233 are zero on the Spartan's model.

**Failing shields** (no multiplayer object uses them): when the shield
fraction is at or below a positive failure threshold t, shield damage
is multiplied by (1 − L)·g(σ/t) + L, where σ is the shield fraction, L
the failing shield leak fraction and g the transition function
(`objects.md`, transition functions).

### Collision model flags (offset 0x000)

| Bit | Tag name | Effect |
|---|---|---|
| 0 | takes shield damage for children | hits on attached objects also go through this object's shield |
| 1 | takes body damage for children | hits on attached objects also go to this object's body (and stop there) |
| 2 | always shields friendly damage | friendly hits ignore the leak and failing rules (the shield takes the whole hit) |
| 3 | passes area damage to children | an area hit on this object is repeated on each attached object |
| 4 | parent never takes body damage for us | when this object is hit, its parents never take the body damage |
| 5 | only damaged by explosives | body damage is zero unless the damage effect "detonates explosives" |
| 6 | only damaged while occupied | a vehicle without a driver takes no body damage |

The Spartan has 0x7 (bits 0, 1, 2). Bits 0 and 1 matter only for
objects attached to a Spartan (nothing in a multiplayer match); bit 2
makes friendly fire bypass the leak, which is zero anyway.

**Vehicle collision models.** The three multiplayer vehicles'
collision models (`vehicles\warthog\warthog`,
`vehicles\scorpion\scorpion_mp\scorpion_mp`, `vehicles\ghost\ghost_mp`;
the same values on every multiplayer map checked: Blood Gulch, Hang
'Em High, Sidewinder, Damnation, Boarding Action) have flags 0x28
(bit 3 passes area damage to children, bit 5 only damaged by
explosives), **maximum body vitality 0 and maximum shield vitality 0**,
indirect damage material -1, shield material type 7 (thick metal) on
the warthog and scorpion and 0 on the ghost, and stun damage and stun
time 0. A vehicle therefore starts with body vitality 0 and no shield.
Reading the reference's damage rule (inverse of the maximum taken as 0
when the maximum is 0), every hit scales to 0 body points, the vitality
never goes below 0 (death needs a value below 0) and the vehicle cannot
be destroyed by damage; only the riders' share of a hit
(above) matters. This follows from the numbers on the disc plus the
reference; no vehicle was destroyed in a running game to confirm it
(open questions). `vehicles.md` owns the rest.

### Materials (block at 0x234, 0x48 bytes each; `collision.md`)

A material: name, flags (bit 0 "head"), material type, shield leak
percentage (a fraction, 0 to 1, despite the name), shield damage
multiplier, body damage multiplier. The Spartan's:

| Index | Name | Head | Type | Leak | Shield multiplier | Body multiplier | On nodes |
|---|---|---|---|---|---|---|---|
| 0 | head | yes | 21 cyborg | 0 | 1.0 | 1.0 | bip01 head (18 surfaces) |
| 1 | body | no | 21 cyborg | 0 | 1.0 | 1.0 | pelvis, spine, spine1, both thighs, upper arms, forearms, hands |
| 2 | legs | no | 21 cyborg | 0 | 1.0 | 0.8 | both calves and feet |
| 3 | shield generator | no | 21 cyborg | 0 | 1.0 | 1.0 | one surface on bip01 spine |

So in multiplayer only two things depend on where a Spartan is hit:
the head flag, and the 0.8 body multiplier of the lower legs.

### Regions (block at 0x240, 0x54 bytes each)

A region: name, flags, damage threshold (fraction of body), destroyed
effect, permutations. Region flags:

| Bit | Tag name | Effect when the region is destroyed (or as stated) |
|---|---|---|
| 0 | lives until object dies | (not read by the damage code) |
| 1 | forces object to die | the object dies |
| 2 | dies when object dies | destroyed when the object dies |
| 3 | dies when object is damaged | (not read by the damage code) |
| 4 | disappears when shield is off | while the shield is broken the region shows permutation 1, else 0 (needs at least 2 permutations) |
| 5 | inhibits melee attack | |
| 6 | inhibits weapon attack | |
| 7 | inhibits walking | |
| 8 | forces drop weapon | the unit drops its weapon on its next update |
| 9 | causes head-maimed scream | (read by the unit's dialogue, out of scope) |

Each body hit with a region adds trunc(normalised body damage × 255)
to that region's byte-sized damage counter (it wraps above 255); with a
positive threshold, a counter above threshold × 255 destroys the region
(destroyed effect, permutation `~damaged`, the flags above). Regions
0 to 7 export their counter to object functions (`objects.md`).

The Spartan has one region, `cyborg`: flags 0, threshold 0 (never
destroyed), no destroyed effect, one permutation `__base`. Every node
with geometry belongs to it.

### Nodes and damage parts

Each collision node has a precomputed damage part (offset 0x32) that
picks the flinch and death animation set. On the Spartan: pelvis and
spine 0 (gut), spine1 1 (chest), neck and head 2 (head), left clavicle
and upper arm 3 (left arm), left forearm and hand 4 (left hand), left
thigh, calf and foot 6 (left foot), right clavicle and upper arm 7,
right forearm and hand 8, right thigh, calf and foot 10 (right foot).
Parts 5 and 9 (legs) are unused. A hit without a node uses part 0.

### The globals (`matg`)

| Where | Field | Value | Use |
|---|---|---|---|
| difficulty block (0x11C), element 0 (`globals.md` owns the layout) | 4 values per row (easy, normal, heroic, legendary) | normal = 1.0 in rows 0 to 7 (the damage, vitality, shield and recharge rows) | multiplayer uses normal for: body maximum ("enemy vitality"), shield maximum ("enemy shield"), recharge rate ("enemy recharge"); so they are unscaled |
| player information (0x170; `globals.md` owns the layout) | stun movement penalty 0x80, stun turning penalty 0x84, stun jumping penalty 0x88 | 0.8, 0.8, 1.0 | `movement.md` says which one scales what (the movement penalty the speed, the turning penalty the jump; the jumping penalty is never read) |
| player information | minimum stun time 0x8C, maximum stun time 0x90 | 0 s, 1.25 s (37 ticks) | body stun duration limits |
| player information | run forward speed 0x34 | 2.25 WU/s | the melee's speed scale |
| multiplayer information (0x164) | unit | `characters\cyborg_mp\cyborg_mp` | the player's biped; nothing else in the block concerns vitality |
| falling damage (0x188) | `globals\falling`, `globals\distance` | | `movement.md`; the layout is `globals.md`'s |

Other difficulty rows read on normal: friend rows 1.0; enemy damage
1.0 (not used in multiplayer).

### The unit and equipment tags

From `objects.md` (unit layer of the Spartan): soft ping threshold 0,
soft ping interrupt 0.3 s (9 ticks), hard ping threshold 1.0, hard
death threshold 0, feign death off, unit flags 0x8008 (no "resists
pings", no "never unsuspecting"), melee damage
`characters\cyborg\melee` (used only when the held weapon has none).
Biped flags 0x202: not immune to falling damage. Equipment: overshield
(powerup type 2, time 60 s), active camouflage (3, 45 s), health pack
(5, 1 s).

## How vitality is represented

Per object with a collision model:

| State | Range | Initial (spawn) |
|---|---|---|
| maximum body B_max | points | 75 (from the collision model; the game type's "no shields" option does not change it) |
| maximum shield S_max | points | 75 (0 when the game type has "no shields") |
| body β | fraction; below 0 when dead | 1.0 if B_max > 0, else 0 |
| shield σ | fraction, 0 to 3 | 1.0 if S_max > 0, else 0 |
| shield delay counter d | ticks | 0 |
| current shield damage c_s, recent shield damage r_s | 0 to 1 | 0 |
| current body damage c_b, recent body damage r_b | 0 to 1 | 0 |
| flags | dead, shield depleted, shield charging, overcharging, cannot take damage, shield-damaged effect played, body-damaged effect played | clear |

Points are σ·S_max and β·B_max (multiplied by the difficulty value,
1.0 in multiplayer). The shield goes above 1 only through an
overshield (3.0 at most). The game type's "no shields" option sets
both σ and S_max to 0 on every update of a player's unit.

## Applying a damage event

Damage is applied at once, during the update of whatever caused it
(the projectile, the melee, the explosion's effect, the falling
biped). The steps, for one event on one target:

**1. The amount.** T = A × game multiplier, where the game multiplier
is 1 / clamp(health option, 0.25, 4) (1.0 by default), × 1.5 if the
attacker has the game type's "damage" trait and × 0.5 if the victim
has the "damage resistant" trait (Oddball's ball carrier options;
`multiplayer_rules.md`). If T ≤ 0 nothing happens at all (no delay
reset, no flinch). There is no rounding and no minimum beyond T > 0.

**2. Who takes it.** For a direct hit (no area flag): the hit object,
then its parent, its parent's parent and so on, processed from the
outermost parent inward; each parent takes shield damage only if its
collision model "takes shield damage for children" and body damage
only if it "takes body damage for children" (and the hit object's model
does not say "parent never takes body damage for us"). The first
object that takes body damage ends the chain with whatever is left.
An area hit or the player's melee goes to the target alone. A player
standing on foot is his own outermost object: the chain is just him.
The steps below are for one object.

**3. Friendly.** The hit is *friendly* when the instigator has a team
and it is not an enemy of the target's team. In free-for-all every
player has a team of his own, so only self-damage is friendly; in team
games, teammates' hits and self-damage are. Friendly hits ignore the
leak (bit 2 of the Spartan's flags) and lose the friendly damage
resistance (0) from their body damage: for the Spartan, friendly and
self damage are full damage. (The game type's friendly fire option:
Open questions.)

**4. Instant kills.** If the damage effect's side effect is "lethal to
the unsuspecting", the target is a biped whose unit tag lacks "never
unsuspecting", the target can take damage, and the target is
unsuspecting (Melee), the target dies now: β = 0, death (section
Death), whatever the shield or overshield. The rest of the steps
still run on the dead target (it takes the shield damage; body damage
to a dead object changes nothing that matters).

**5. Material.** The material M is the hit material if the event has
one and this object is the one hit; otherwise the model's indirect
damage material (the Spartan's "body"). From M: head flag h, leak λ,
shield multiplier k_s, body multiplier k_b, type τ. Let μ_s be the
damage effect's modifier for the shield material type (22) and μ_b its
modifier for τ (21 on the Spartan).

**6. Shield** (skipped when the damage effect "skips shields", when the
event bypasses shields (AI riders, below) or when S_max = 0). Let T_in
= T.

- If σ = 0: the shield takes nothing (D_s = 0) and T is unchanged.
- If σ > 0:
  1. D_s = T if the hit is friendly (Spartan flag bit 2), else
     D_s = (1 − λ)·T, times the failing factor if failing applies.
  2. If the overshield is charging: D_s = T, T = 0, σ unchanged. The
     whole hit is absorbed: the player is invulnerable (except to
     instant kills and to effects that skip shields).
  3. Otherwise: D_s = max(D_s, 0); T = T − D_s; then the shield's
     damage in points P = (k_s·D_s)·μ_s and its fraction
     δ = P·(1/S_max) (1/S_max is the f32 0.013333334).
     - If δ > σ, or the side effect is "emp": the excess
       E = P − S_max·σ is added to T if positive; σ = 0; if the shield
       was not already marked depleted: mark it, play the shield
       depleted effect, set c_s = 0, show the shield-off region
       permutations, and remember "this hit depleted the shield".
     - Otherwise: σ = σ − δ (unless the object cannot take damage);
       the first time σ falls below the shield damaged threshold, play
       the shield damaged effect.
  4. If the overshield is charging, or P ≥ NEGLIGIBLE: the shield
     damage fade timer restarts; c_s = 1 unless the shield is now
     depleted; r_s = min(1, r_s + (T_in − T)/S_max).

Note that E is in the shield's points, already multiplied by k_s and
μ_s; it then becomes body damage and is multiplied again by k_b and
μ_b (Quirks).

**7. Shield delay.** This is the last part of step 6 and runs only
when step 6 ran (the effect does not skip shields, the event does not
bypass them, S_max > 0, and for the object in the chain of step 2 that
takes shield damage). Then, if D_s ≥ minimum stun damage (0 on the
Spartan: always; D_s is 0 when σ was 0) or σ = 0 after step 6, the
shield delay counter d = trunc(stun time × 30) = 180. So every hit
that goes through step 6 restarts the 6-second wait, including hits
the shield fully absorbs and hits on an empty shield. A hit whose
effect skips shields (or an event that bypasses them) never reaches
step 6 and does **not** reset d, whether σ is above 0 or 0.

**8. Body** (skipped when the damage effect "only hurts shields"; for
a parent object as in step 2). The body's amount a = k_b·T (zero if
the model is "only damaged by explosives" and the effect does not
"detonate explosives", or "only damaged while occupied" and nobody
drives). Then, in this order:

- x = a; if friendly, x = (1 − friendly damage resistance)·a.
- x = x·(1/B_max) (the f32 0.013333334); x = x·μ_b.
- If the object can take damage:
  - **Headshots.** If a > 0 and M has the head flag:
    - if the damage effect "can cause headshots": β = 0 (then the
      subtraction below makes it negative: death), and the hit forces
      a hard flinch. In the campaign this applies only to non-player
      targets; in a multiplayer game to everyone.
    - else if the damage effect "can cause multiplayer headshots" and
      a multiplayer game is running: x = 2x; if x > β the hit forces a
      hard flinch.
  - β = β − x.
- Region damage (above), when the hit has a region.
- The body damage fade timer restarts; c_b = min(1, c_b + x);
  r_b = min(1, r_b + x).
- Thresholds on body points b = β·B_max:
  - if the body destroyed threshold is negative and b is below it: the
    object is destroyed (dies, body destroyed effect, deleted after
    the event);
  - else if b < 0 and the object is not already dead: every region
    flagged "dies when object dies" is destroyed, then the object
    dies (section Death);
  - else if b < body damaged threshold, the first time: play the body
    damaged effect.
- If the event asks for a localized effect, play the localized damage
  effect at the hit; if the event is an area hit, a > area damage
  effect threshold, and the category is not flame: play the area
  damage effect on the object.

Because only a > 0 triggers a headshot, a head hit that the shield
absorbs entirely does nothing special, and one whose leftover is a
millionth of a point kills (Quirks).

**9. Aftermath**, in this order, for each object that took part:

- Push: if the object's acceleration scale (`objects.md`) is above
  0.0001: direction v = normalise(direction + (0, 0, 0.45)), push
  a = v × instantaneous acceleration × acceleration scale × 0.033333335
  (WU/tick); a biped takes it as `movement.md` describes, a vehicle
  doubled if the effect "detonates explosives".
- Statistics: damage dealt (D_s + a) and, if the object died, the kill
  (section Death).
- For a unit (Stun and pings): the delayed damage record for AI,
  camouflage loss, flinch or death animation, the attacker list,
  damage sounds, **unzoom** (any D_s > 0 or a > 0 drops a scoped
  player out of zoom), AI reactions, body stun, and the death
  handling.

**Riders of vehicles (in brief, `vehicles.md`).** A direct hit on a
vehicle is also passed to each seated player and to the driver: the
same event with multiplier (1 − the effect's vehicle passthrough
penalty) × the vehicle's rider damage fraction (0.18 on the disc's
vehicles), no material, no area flag; AI riders take it bypassing
their shields, players through them. The vehicle then takes the hit
itself. The penalty is a field of every damage effect: on the disc it
is **0 for every damage effect except the sniper bullet's 0.5**
(every damage effect tag on three maps; `projectiles.md`, "Vehicle
passthrough"). So riders take 0.18 of any direct hit on the vehicle
(bullets, bolts, needles) and 0.09 of a sniper bullet. An area hit reaches riders only through "passes area damage to
children" or by touching them. A hit on a rider in an open seat goes
through the chain of step 2 (the vehicle's own flags decide whether it
soaks the hit).

## Shield recharge

Each tick, in the object's damage update (after its type update), if
the object is not dead and S_max > 0:

1. If the overshield is charging: σ = σ + 0.033333335; if σ ≥ 3: σ = 3
   and charging ends.
2. Else, if σ > 1 and a multiplayer game is running: σ drains toward
   1 by OVERSHIELD_DRAIN (to exactly 1 when less than a step is left).
3. Else, if σ < 1:
   - if d = 0: if the shield is marked depleted, clear the mark, play
     the shield recharging effect (none on the Spartan) and restore
     the shield regions; then σ = σ + RECHARGE_STEP, and if the result
     is above 1, σ = 1;
   - else d = d − 1.

The "shield charging" state (for the HUD and sounds) is true on ticks
where step 1 or 3 added to σ and σ did not end above 1.

Consequences, with a hit on tick 0 before the victim's update:

- d is 180 after the hit and is decremented on ticks 0 to 179; the
  first increment is on **tick 180** (exactly 6.0 s), every tick
  after until full. The recharge is linear.
- From 0, full takes 121 increments: ticks 180 to 300 (σ = 0.9999994
  after the 120th, the 121st clamps to 1.0).
- Any hit that goes through the shield step during the wait or the
  recharge sets d back to 180 (step 7 of Applying); the recharge simply
  stops and the shield keeps what it had.
- d does not count down while σ ≥ 1 (it is simply left as it is) and
  is irrelevant there.
- The engine never reads the recharge time; it reads the precomputed
  runtime velocity. Multiplayer's difficulty multiplier is 1.0.

## Health

The body **never regenerates**: nothing in the update raises β. Two
things set it:

- **Health pack** (powerup type 5): touching it, if the player is
  alive and β < 1, sets β = 1 at once (shield unchanged), plays the
  pickup's screen flash (white, 2 s, half intensity) and consumes the
  pack. With β = 1 the pack is not picked up and stays. Its powerup
  time (1 s) only has to be positive.
- Respawn: a new unit with β = 1, σ = 1.

## The overshield

Picking up an overshield (powerup type 2), only if σ ≤ 1 (otherwise
it is not picked up): the overcharging state is set, σ = 0.01 if it
was 0, and the shield delay d = 0. Then, from the next damage update
(pickups happen after the objects' update in the tick, Per-tick order):

| Phase | Duration | Shield | Damage |
|---|---|---|---|
| charge-up | from σ = 1: 61 updates (2.03 s); from empty (0.01): 90; from 0.5: 76 | +0.033333335 per tick, clamped to 3.0 on the update that reaches it | absorbed entirely (invulnerable), except instant kills and effects that skip shields |
| full and draining | 2700 ticks (90 s) from 3.0 to 1.0 | −1/1350 per tick | the shield takes damage normally from up to 3.0 (225 points); no recharge while σ > 1 |
| normal | | ordinary recharge to 1.0 only | |

The drain does not wait for a delay: it runs every tick while σ > 1,
hit or not. The equipment's powerup time (60 s) is not used, beyond
having to be positive. An "emp" hit (charged plasma pistol, plasma
grenade) on an overshielded player removes the whole overshield (its
excess is P − S_max·σ, negative). On death the shield no longer
updates; the new unit spawns with σ = 1. A game type with "no shields"
removes overshields from the map.

## Active camouflage

The unit has a camouflage value κ (0 = visible, 1 = fully
camouflaged) and a camouflaged state.

- **Pickup** (powerup type 3): refused if the unit is already
  camouflaged. Otherwise the camouflaged state is set, the player's
  camouflage timer becomes trunc(45 × 30) = 1350 ticks, and the
  pickup's screen flash plays. The timer counts down once per tick
  after the objects' update; at 0 the camouflaged state ends.
- **Each unit update**: while camouflaged, κ = min(1, κ + rate), where
  the rate is the held weapon's "active camo regrowth rate" (per tick,
  `weapons.md`) while the regrowth after a shot is in progress and
  that rate is not zero, else CAMO_STEP (the f32 1/120); when κ reaches 1
  the shot regrowth ends. While not camouflaged, κ = max(0, κ −
  CAMO_STEP). In f32 120 steps of CAMO_STEP from 0 give 0.9999994, not
  1.0, so the fade in takes **121 updates** (about 4 s): κ is
  0.9999994 after the 120th update and exactly 1.0 after the 121st
  (the clamp). The fade out from 1.0 likewise takes 121 updates (the
  121st clamps to 0). The first update of a pickup is on the tick after
  it (pickups happen after the objects' update, Per-tick order).
- **Firing** (multiplayer): if κ ≥ 0.05, κ = max(0.05, κ − ding), ding
  being the weapon's "active camo ding" (0.1 if the weapon has 0, none
  if the weapon "does not depower active camo"), and the shot regrowth
  starts.
- **Taking damage**: while camouflaged, κ = max(0, κ − the damage
  effect's "active camouflage damage") on every hit (pistol bullet
  0.45, assault rifle bullet 0.25, sniper 1.0; `projectiles.md`).
- **Death** clears the camouflaged state (κ then fades out on the
  body); the new unit's powerup timers start at 0.
- A game type's "always invisible" option gives every living player a
  15-tick camouflage that is renewed each tick.

How κ is drawn belongs to `shaders.md` and the renderer.

## Stun and pings

**Body stun** (slows the player; `movement.md` uses it). A unit has
stun s_b (0 to 1) and a stun counter t_b. After a hit on a player's
unit in a multiplayer game, if the damage effect's stun is above 0:

- i = max(0, stun·s), cap = clamp(maximum stun·s, 0, 1);
- if s_b < cap: s_b = min(s_b + i, cap);
- t_b = max(t_b, trunc(minimum stun time × 30) = 0) +
  trunc(stun time × 30); then t_b = min(t_b, trunc(maximum stun time
  × 30) = 37).

Each unit update: if t_b > 0, t_b = t_b − 1, and when it reaches 0,
s_b = 0. There is no gradual decay: the stun holds its value and
drops to 0 at once. What s_b does to the player's speed and jump
(which penalty scales which) is stated once, in `movement.md` ("Damage
stun"); this spec only produces s_b. Of the
multiplayer weapons only the plasma rifle and plasma pistol bolts stun
(stun 1.0, maximum stun 1.0, stun time 0.15 s = 4 ticks): a bolt at
full speed (s = 1) sets s_b = 1 (wanted speed × 0.2, jump × 0.2, as
`movement.md` says) for 4 ticks, and
rapid fire stacks the ticks up to 37. Bolts slowed by distance
(`projectiles.md`) stun less.

**Shield delay** is also called stun in the tag (minimum stun damage,
stun time); it is the counter d above and does not slow anyone.

**Pings (flinches).** After a hit that is not silent, on a living unit
(or the killing hit), with no "does not ping units" flag on the damage
effect:

- soft ping if c_b or c_s (after the hit) exceeds the unit's soft ping
  threshold (0 on the Spartan: every hit); not restarted while a soft
  ping animation is in its first 9 frames (soft ping interrupt ticks);
- hard ping if c_b exceeds the hard ping threshold (1.0: never on the
  Spartan, since c_b ≤ 1) unless the unit "resists pings" and the
  effect does not "ping resistant units"; **forced** (soft and hard)
  when this hit depleted the shield, destroyed a region, or was a
  headshot as in step 8. A hard ping does not play in a seat.
- direction: the signed angle between the hit's direction and the
  unit's facing, both flattened to the ground plane: |angle| < 0.7854
  back, > 2.1598 front, else left (positive) or right. In multiplayer,
  a hard-ping kill on the head part always uses the left set.
- the animation is picked by (soft ping, hard ping, soft kill, hard
  kill) × direction × the node's damage part from the animation graph
  (`animations.md`).

Camera shake and screen flashes of a hit belong to `effects.md`.

## Death

On the tick the body goes below zero (or an instant kill):

1. The dead state is set; the body depleted effect plays; the shield
   is depleted (its depleted effect plays if it was up); regions that
   die with the object are destroyed.
2. Kill statistics: the kill is recorded against the instigator's
   player (below).
3. The unit's reaction: the death animation by direction and part
   (Pings), or the airborne death set if airborne. The unit stops
   being controlled, loses its camouflage state, leaves its seat, and
   drops its other weapons, its equipment and **all its grenades** (one
   grenade object per grenade) at once; the held weapon is dropped
   after a random delay between a quarter and three quarters of the
   death animation's frames (at least 1 tick), or at once if the
   killing effect's instantaneous acceleration is 2 or more. The body
   is the biped itself (the Spartan uses limp body physics once the
   animation ends, `animations.md`). The player is detached from the
   unit; respawning is `multiplayer_rules.md`'s.

**Credit.** Every hit with an instigator is recorded in the victim's
list of 4 attackers (player, object, damage dealt, time; the oldest
except the biggest is replaced). At death:

- killer = the instigator's player of the killing hit; if it is the
  victim himself, a suicide is counted; a death is always counted.
- If the killing hit was friendly or had no player (a teammate, a
  suicide, a fall), the kill goes to the attacker in the list with the
  most damage among those hit within the last 180 ticks (6 s) who are
  enemies, if any; otherwise to the killer. A credited enemy gets the
  kill; a credited teammate a friendly kill (betrayal). Enemies of the
  victim in the list who did more than 40% of the credited attacker's
  damage, and the killer if he was not credited, get assists.
- The game type is then told who killed whom (scores, messages,
  penalties: `multiplayer_rules.md`).

A fall has no instigator: a recent enemy attacker (by the rule above)
takes the kill; otherwise nobody does, no suicide is counted, and the
game type decides the penalty.

## Melee

The hit test (rays from the head along the aim, which object is
struck) is `weapons.md`'s. When the player's melee strikes a **biped**
(other objects are only pushed or opened):

- damage effect: the held weapon's melee damage (every multiplayer
  gun's `weapons\<name>\melee`), else the unit's
  (`characters\cyborg\melee`, which a player never uses in practice);
- the event: area flag set (the target alone), no node, region or
  material (so the indirect material "body": **a melee never
  headshots**), epicenter the attacker's centre, direction the
  attacker's aim, instigator the attacker;
- scale s = clamp(v_f·30 / 2.25, 0, 1), v_f being the attacker's
  velocity along his facing (WU/tick) and 2.25 the globals' run
  forward speed; s = 1.5 if the attacker has been airborne more than
  15 ticks (a lunge).

The guns' melee effects (all identical): minimum 40, bounds 50 to 60,
category melee, side effect "lethal to the unsuspecting", flags "does
not hurt owner" and "pings resistant units", modifier 1.4 for both
cyborg (21) and cyborg energy shield (22). So a melee does, before
the Spartan's multipliers: 40 standing (×1.4 = 56 shield points),
50 to 60 running (70 to 84), 55 to 70 lunging (77 to 98). The ball's
and flag's: 80 flat, modifiers 1.0.

**From behind.** The target is *unsuspecting* when (c_t − c_a)·l > 0,
where c_t is the target's centre, c_a the attacker's centre and l the
target's 3D looking direction: the attacker is anywhere in the
half-space behind the plane through the target's centre perpendicular
to where the target looks (a 180° rear arc, pitch included). Such a
melee kills outright, through any shield or overshield (step 4). The
Spartan has no "never unsuspecting" flag; a unit sitting in an
"asleep" seat is also unsuspecting (not in multiplayer).

## Falling

`movement.md` decides when and with what scale: the effect
`globals\falling` (minimum 10, bounds 155, category falling, modifiers
1.0 for types 21 and 22) at scale q_f, giving T = 10 + 145·q_f, and
`globals\distance` (105) every tick of a too-fast fall. The event has
no instigator, no material (indirect: body), direction none. It goes
to the shield first like any hit: 155 points kill a full Spartan (75
shield points, then the other 80 hit a body of 75: β = −0.0667), 105
does not (body 0.6), a second 105 does.
In a multiplayer game no difficulty applies to falls.

## What Chimera's hit model needs from this game

For `docs/DESIGN.md`, "Hits across games":

- **Regions.** Halo's collision model has one damage region; hit
  zones come from materials per node. Map the Spartan's nodes to
  `BodyRegion`: `bip01 head` → head (material "head", the only head
  flag); pelvis → groin, spine → stomach, spine1 → chest, thighs →
  thigh, calves → shin, feet → foot, upper arms → upper arm, forearms
  → forearm, hands → hand. The neck and clavicles have no geometry.
- **Damage per region from the gun's game.** A Halo gun has no
  per-region table: its damage effect has a modifier per *material
  type*, and the target's collision material adds a shield and a body
  multiplier (Spartan: legs ×0.8 body, all else ×1) and the head
  flag. A Halo gun on a body of another game needs a Halo material
  type for that body (for the modifiers) and a rule for which regions
  count as "head" and "legs".
- **Armour.** Halo's shield is an armour layer over every part and
  every damage class except effects that skip shields: it absorbs the
  whole hit (the Spartan's leak is 0) until it breaks, the overflow
  carries to the body, and it recharges (6 s delay, 4 s from empty).
  It does not fit a fixed pass-through fraction: the step is "shield
  first, overflow to body" with per-material multipliers on each
  side. A Halo gun on a non-Halo body with ordinary armour needs that
  game's absorb rule; a non-Halo gun on a Spartan needs this one.
- **Headshot.** Shield first, always: Halo's head rule applies only to
  the damage that reaches the body.

## Per-tick order

Within one game tick (`movement.md` and `objects.md` give the rest):

1. Effects update: explosions' damage parts deal area damage now
   (`projectiles.md` says when an explosion's effect is created).
2. The game type's update ("no shields", "always invisible").
3. The objects' update, each object in turn. For each object:
   1. its type update; for a unit, first the unit part: camouflage
      step, **body stun counter step**, delayed weapon drop; then the
      biped part (`movement.md`: movement, landing, **falling damage**;
      the player's **melee** strikes here);
   2. for objects with a collision model, the **damage update**: acts
      of god (scripted kills), then the shield step (Shield recharge),
      then the current/recent damage fades (below);
   3. functions, node matrices and the rest.
   Projectiles are objects too: a projectile's hit applies its damage
   **during the projectile's update**, at once. If the projectile is
   updated before its victim, the victim's damage update of the same
   tick already counts down the new delay (the timelines here assume
   that); if after, the victim's countdown starts next tick and every
   later tick in the timeline shifts by one.
4. Players' update: pickups (health pack, overshield, camouflage),
   powerup timers count down (camouflage ends at 0).
5. HUD and player screen effects.

**Damage fades** (in the damage update): each of the body and shield
pairs has a timer, restarted to 0 by a hit (step 6.4 and step 8),
idle otherwise. Each tick while active: timer + 1; current damage −
1/60 (from the first tick); recent damage − 1/60 once the timer is at
60 or more; both clamped at 0; when both are 0 the timer goes idle.
These feed the pings, the shield glow (`objects.md`, recent shield
damage) and the AI.

No death check exists in the update: death happens inside the hit.

## Edge cases

- Body exactly 0 is alive. Two pistol body shots on a Spartan whose
  shield is already gone leave β = 0.0 exactly; the third kills. A
  rocket's 300-point minimum on a full overshield (225 shield points)
  leaves β = 0.0 and the player alive.
- A hit with T = 0 (an effect with zero damage, e.g. the needle's
  impact or the explosions' shock waves) does nothing at all: no delay
  reset, no flinch, no unzoom.
- The shield delay is reset by every hit that goes through the shield
  step (step 6), even one the shield fully absorbs or one that lands on
  an empty shield; not by a hit that skips shields, with the shield up
  or empty (that hit never enters the shield step).
- A shield partially recharged (σ = 0.1) takes the next hit normally:
  the delay restarts and recharge resumes 180 ticks later from what is
  left.
- Overshield pickup when σ > 1 (still draining from a previous one):
  refused, the pickup stays.
- Health pack at full health: refused, stays.
- A player in his overshield charge-up dies only to instant kills.
- An "emp" hit with the shield already 0 is an ordinary body hit.
- Damage to a dead unit: the shield and body still change, no second
  death, no flinch unless lethal.
- The leftover of a shield-breaking hit is computed from the shield
  *before* the hit in shield points, then multiplied by the body's
  multipliers (Quirks).
- Ice (31) and hunter shield (32) material modifiers read 0 on every
  disc damage effect; no multiplayer body uses them.

## Quirks

- **f32 order decides the pistol.** The shield fraction after two
  pistol shots is 0.33333328 (0x3EAAAAA9), below the third shot's
  0.33333334 (25 × 0.013333334). The third shot therefore breaks the
  shield with an excess of 25 − 75 × 0.33333328 = 0.0000038 points,
  which reaches the body; on the head that is a headshot and a kill.
  This is the three-shot kill players know. In exact arithmetic (or
  with intermediates kept in higher precision) the third shot leaves
  the shield at a tiny positive value, nothing reaches the head and
  four shots are needed. Keep f32, every operation rounded, in this
  spec's order.
- **Carry-over is multiplied twice.** The excess of a shield-breaking
  hit is already in shield points (× k_s × μ_s); it is then multiplied
  by k_b × μ_b as body damage. With the melee's 1.4 modifiers, the
  overflow of a running melee hurts the body 1.4 times more than its
  raw amount. Keep.
- **Strictly below zero to die.** Keep (see Edge cases).
- **Kill credit compares a player index with a team.** When looking
  for an enemy attacker to credit with a friendly or ownerless death,
  the game tests the attacker's *player index* as if it were a team
  index (from the reference). Keep the stated behaviour only if
  `multiplayer_rules.md` confirms it; see Open questions.
- **Overshield charge-up invulnerability** comes from the damage code
  absorbing every hit while charging, not from a separate flag. Keep.
- The shield delay does not count while σ ≥ 1; harmless.

## Shots to kill

Against a full-shield, full-health multiplayer Spartan at point blank
(scale 1, the hit applied as a direct hit on the named material;
explosions centred on him, scale 1), multiplayer rules (no health
option, no traits), computed by a scratch simulation of the rules
above in f32, with the disc's numbers. "Break" is the hit that brings
σ to 0; "kill" the hit that brings β below 0. Random bounds: the
range over all draws, and the share of 20 000 random sequences.

| Weapon (damage effect: amount; μ_s, μ_b; flags) | Body: break / kill | Head: break / kill |
|---|---|---|
| pistol (25; 1.0, 1.5; can cause headshots) | 3 / 5 (legs: 3 / 6) | 3 / **3** |
| assault rifle (10; 1.0, 1.0) | 8 / 16 (legs: 17) | 8 / 16 |
| plasma rifle (12 to 14; 2.0, 0.5; multiplayer headshots) | 3 to 4 / 14 to 15 (15: 85%) | 3 to 4 / 8 to 9 (9: 93%) |
| plasma pistol, uncharged (16 to 20; 0.6, 0.6) | 7 to 8 / 13 to 16 (14 or 15: >99.9%) | same |
| plasma pistol charged (70, emp; 1.0, 0.6) then pistol | charged shot: σ = 0, β = 1; then pistol head: kill (1 + 1); pistol body: 3 more (β 0.5, 0.0, −0.5) | |
| shotgun pellets (18 to 25; 0.5, 1.0; 15 per shot) | 6 to 9 / 9 to 13 pellets (11: 68%, 12: 31%): one shell kills | same |
| sniper rifle (101; 1.0, 1.0; can cause headshots) | 1 / 2 | 1 / **1** |
| needler needles (detonation 10 each; impact 0) | 8 / 16 | same (attached damage has no material) |
| needler supercombine (60, emp, area) | strips the shield, no body damage; with the 6 other needles after it: β = 0.2; with all 7: 0.067: **does not kill alone** | |
| rocket, direct (300 to 330 area at the centre) | 1 / 1 (β −2.0 to −2.4) | |
| frag grenade at the centre (120 area) | 1 / survives at β = 0.4 | |
| plasma grenade, unstuck at the centre (120 area, emp) | 1 / survives at β = 0.4 | |
| plasma grenade stuck (120 attached, emp, then 120 area) | 1 / 1 (β 0.4 then −1.2) | |
| melee from the front, standing (40 × 1.4) | 2 / 3 | |
| melee from the front, running (50 to 60 × 1.4) | 1 to 2 / 2 | |
| melee from the front, lunging (55 to 70 × 1.4) | 1 / 2 | |
| melee from behind | 1 / **1** | |
| ball or flag melee (80) | 1 / 2 | |

Against common knowledge: pistol three headshots ✓; a charged plasma
pistol shot removes a full shield ✓ (and does no body damage); sniper
one headshot or two body shots ✓; melee from behind ✓; health does not
regenerate ✓. Two differ: the shield waits **6.0 s**, not "about
five", and the needler's supercombine does not kill by itself (Open
questions).

## Test cases

All on a fresh multiplayer Spartan (β = 1, σ = 1, d = 0) unless said;
"rnd" fixes the uniform draw. Direct hits at scale 1. Two instants are
named wherever a tick is involved: **at the hit** (right after the
damage event, before the victim's own update of that tick; d = 180
there) and **after the update of tick t** (after the victim's damage
update of tick t, which is where d has counted down once and σ has
stepped once). The timelines assume the hit lands before the victim's
update in the same tick (Per-tick order).

| Setup | Input | After | Expected |
|---|---|---|---|
| spawn | none | creation | β = 1.0, σ = 1.0, B_max = S_max = 75 |
| full | pistol bullet on "head" | hit 1 | σ = 0.66666663 (0x3F2AAAAA), β = 1 |
| full | pistol on "head" | hit 2 | σ = 0.33333328 (0x3EAAAAA9) |
| full | pistol on "head" | hit 3 | σ = 0, excess 0.0000038 points, β = −7.63e−8: dead, hard flinch forced |
| full | pistol on "body" | hits 3, 4, 5 | β = 0.99999994, 0.49999994, −0.00000006 (dead on 5) |
| full | pistol on "legs" | hits 4, 5, 6 | β = 0.5999999, 0.1999999, dead on 6 |
| full | assault rifle on "body" | hit 1, at the hit | σ = 0.8666667 (0x3F5DDDDE), d = 180 |
| full | assault rifle on "body" | hits 7, 8 | σ = 0.06666669, then σ = 0 with excess 4.9999981 points: β = 0.93333334 |
| full | assault rifle on "body" ×16 | hit 16 | dead (hit 15 leaves β > 0) |
| full | sniper on "body" | hit 1 | σ = 0, β = 0.6533333 (26 points carried) |
| full | sniper on "legs" | hit 1 | β = 0.7226666 (26 × 0.8) |
| full | sniper on "body" | hit 2 | β = −0.6933334, dead |
| full | sniper on "head" | hit 1 | dead |
| full | plasma rifle rnd 12 on "body" | hits 1 to 4 | σ = 0.68, 0.36, 0.04, then 0 and β = 0.86 |
| full | plasma rifle rnd 12 on "head" | hits 4, 9 | β = 0.72 after 4 (double), dead on 9 |
| full | plasma rifle rnd 14 on "body" / "head" | | dead on 14 / 8 |
| full | plasma pistol rnd 16 / rnd 20 on "body" | | break on 8 / 7, dead on 16 / 13 |
| full | charged plasma pistol bolt (70, emp) | hit | σ = 0, β = 1.0, d = 180 |
| after the charged bolt | pistol on "head" | hit | dead (β = −0.5) |
| after the charged bolt | pistol on "body" ×2 | | β = 0.0 exactly, alive; the third kills |
| full | shotgun pellet rnd 18 / 21.5 / 25 | | dead on pellet 13 / 11 / 9 |
| full | frag explosion, scale 1, rnd 120 | hit | σ = 0, β = 0.39999998, alive |
| full | stuck plasma grenade: attached (120, emp), then explosion (120) | two hits | β = 0.4 then −1.2: dead |
| full | supercombine (60, emp) then 7 needles (10 each) | | σ = 0, β 1.0 then 0.8666667 … 0.0666667: alive |
| σ = 3.0 | rocket explosion rnd 300 | hit | σ = 0, β = 0.0, alive |
| σ = 3.0 | rocket explosion rnd 301 | hit | β = −0.0133333, dead |
| σ = 3.0 | sniper on "head" | at the hit; after the update of that tick | σ = 1.6533333, β = 1; then σ = 1.6525925 (one drain step of 1/1350 has run) |
| σ = 3.0 | assault rifle on "body" | | dead on hit 30 |
| full | melee, standing (s = 0) | hits 1, 2, 3 | σ = 0.25333333; σ = 0, β = 0.30933332 (37 excess × 1.4); dead |
| full | melee, running (s = 1), rnd 60 | hit 1 | σ = 0, β = 0.832 (9 excess × 1.4 = 12.6) |
| full | melee, lunge (s = 1.5), rnd 50 | hit 1 | T = 55, σ = 0, β = 0.96266669 |
| full, overshield charging | any melee from behind | hit | dead |
| attacker behind | target looks along +X at (0, 0, 0), attacker centre (−0.5, 0, 0) | unsuspecting test | (0.5, 0, 0)·(1, 0, 0) = 0.5 > 0: unsuspecting |
| attacker beside | attacker centre (0, 0.5, 0) | | dot = 0: not unsuspecting |
| full | falling, q_f = 0.0574539 | hit | T = 18.33, σ = 0.7555891 |
| full | falling, q_f = 1 | hit | dead (β = −0.0666667) |
| full | `globals\distance` | hits 1, 2 | β = 0.6, then dead |
| full | one assault rifle hit on tick 0 | after the update of ticks 0, 1, 178, 179 | σ = 0.8666667 throughout; d = 179, 178, 1, 0 |
| same | | after the update of ticks 180, 181, 195, 196 | σ = 0.875, 0.8833333, 0.9999999, 1.0 (17 increments) |
| sniper body hit on tick 0 (σ = 0) | none | after the update of tick 179 / 180 | σ = 0, d = 0 / σ = 0.008333334, depleted mark cleared, recharging effect (none) |
| same | none | after the update of ticks 299, 300 | σ = 0.9999994, then 1.0 |
| same, second hit on tick 250 (at the hit: d = 180) | any | | σ keeps its value; d = 0 after the update of tick 429; next increment in the update of tick 430 |
| overshield picked up at σ = 1 at the end of tick N | none | after the update of ticks N+1 … N+61 | σ = 1.0333333, 1.0666666, …, 2.9999981 on N+60, 3.0 on N+61 (61 updates; the first drain step is the update of N+62: 2.9992592) |
| σ = 3.0 (from the end of the charge-up) | none | after the 1st and the 2700th update | σ = 2.9992592, then 1.0 |
| overshield picked up at σ = 0 | | | σ = 0.01 at the pickup; 3.0 after the 90th update |
| β = 0.4 | health pack | pickup | β = 1.0, σ unchanged |
| β = 1 | health pack | touch | not picked up |
| σ = 1.5 | overshield | touch | not picked up |
| plasma rifle bolt, s = 1, on a still player | | at the hit | s_b = 1.0, t_b = 4; t_b = 3 after the first unit update, s_b = 0 after the 4th |
| eight bolts one tick apart | | at each hit | t_b = 4, 7, 10, … capped at 37 (each tick's update takes 1 off between hits) |
| camouflage picked up at the end of tick N, κ = 0 | none | after the update of tick N + 120 / N + 121 | κ = 0.9999994 / 1.0 (121 updates); timer 1350 at the pickup |
| κ = 1, pistol in hand | fire | | κ = 0.35; regrows 0.01 per tick |
| κ = 1 | hit by an assault rifle bullet | | κ = 0.75 |
| hit by AR, AR, AR on ticks 0, 1, 2 | | at the third hit; after the update of tick 2 | d = 180 (restarted), r_s = 0.4 (recent damage does not fade before 60 ticks); then d = 179 |

## Confidence per section

| Section | Confidence |
|---|---|
| Collision model fields, flags, materials, regions, nodes, Spartan values | checked against the disc (bloodgulch; identical on hangemhigh and damnation); field meanings from the reference |
| Vehicle collision models (vitality 0, flags 0x28) | checked against the disc (five multiplayer maps); the consequence is the reference's rule applied to the disc's numbers |
| Damage effect values used in examples | read from the disc; their layout is `projectiles.md`'s |
| Globals (difficulty, stun, run speed, falling) | checked against the disc; that multiplayer reads "normal" is reference only |
| Applying a damage event (order, maths, headshots, emp, overcharge) | reference only; consistent with the disc's numbers and with the well-known pistol, sniper and charged plasma results |
| f32 behaviour | inferred (the pistol's three-shot kill needs it) |
| Shield recharge (delay, rate, linearity, timing) | rate and delay from the disc; the update logic reference only |
| Health packs, overshield, camouflage | durations and types from the disc; logic reference only |
| Stun and pings | thresholds and stun values from the disc; logic reference only |
| Death and credit | reference only |
| Melee (scale, unsuspecting rule) | effect values from the disc; logic reference only |
| Friendly fire option | not found in the reference's damage code: inferred (Open questions) |
| Shots-to-kill table | computed from the rules with the disc's numbers |

## Open questions

1. **Shield delay 6.0 s vs "about five seconds".** The disc's stun
   time is 6.0 s (180 ticks) and the recharge 4.0 s. Measure: in a
   2-player game, shoot a partner once with the assault rifle and
   count frames on a capture until the shield bar starts to rise;
   expected 180 ticks (6.0 s) plus at most one tick.
2. **The game type's friendly fire option** (on, off, shields only,
   explosives only) exists in the game variant, but the reference's
   damage code never reads it in this build. The natural rule: off →
   teammates' hits do nothing; shields only → only the shield step
   runs; explosives only → only area, grenade and high-explosive hits
   count; self-damage always full. Measure on the Xbox: a team game
   with friendly fire off, shoot a teammate (shield bar), throw a
   grenade at him, melee him from behind.
3. **Needler supercombine.** The super detonation's damage effect is a
   60-point emp area hit, which only strips a full shield; the needles
   then add 10 each, leaving 15 or 5 points. Common accounts treat the
   supercombine as a kill. Check `projectiles.md`'s needle sequence
   (which needles still deal their attached damage, and whether the
   explosion's area hit also reaches nearby needles) and measure:
   fill a full-health partner with exactly 7 needles and watch the
   health bar.
4. **Precision.** The three-shot pistol kill requires f32 rounding at
   every step. Measure: three pistol headshots on a full partner kill
   on the Xbox (known); also two pistol body shots after a charged
   plasma shot should leave him alive on an empty bar.
5. **Same-tick order of a hit and the victim's update** depends on the
   order of objects in the update (projectile before or after the
   victim). Chimera may fix one order; the difference is one tick on
   every delay. Measure: not observable without frame capture at 30
   Hz; accept either.
6. **Kill credit's team test** (Quirks): with the reference's rule, a
   fall right after an enemy's hit gives that enemy the kill only by
   accident of index values. Measure: in Slayer, shoot a partner once
   and let him jump off a cliff within 6 s; see whose kill it is.
7. **Frag grenade strength.** A centred frag does 120 points and
   leaves a full Spartan alive at 30 health points. Measure with a
   partner standing on a grenade.
8. **Hard pings on shield break.** The rule forces a hard flinch when
   the shield breaks; whether the multiplayer Spartan's animation
   graph has hard ping animations (so that it shows) is
   `animations.md`'s.
9. **Melee scale while strafing or backing**: the scale uses only the
   velocity along the facing (negative gives 0, the minimum damage).
   Measure: a backpedalling melee should do the standing 56 shield
   points.
10. **Indestructible vehicles?** The vehicle collision models carry 0
    body and 0 shield vitality, so by the reference's rules damage
    cannot destroy a vehicle. Measure: empty a rocket launcher into a
    parked warthog and a scorpion and check that they never blow up.
