# Party balance: the Halo character in Ultimate Gun Game

A balance check made on 2026-11-15 by reading code and data (nothing was
run), kept here as the reference for the cross-game balance rule and for
the sandbox's readouts. Sections 1 to 7 are the state BEFORE the rule;
the appendix gives the numbers after it. Status: the rule is on `main`,
its numbers now in one table with a host switch
(`apps/party_kit/data/party_balance.toml`, `mp_balance`: "The table and
the switch", at the end); whether its defaults are right is for
playtests.

Analysis only: nothing in the repository was changed, and no build or test was run. Four other builds were running on this machine, and every number below could be worked out by reading the code. All file references are relative to the repository root.

How each number is marked:

- **[D]** read directly from code, data or a spec in the repository.
- **[C]** computed by me from [D] numbers. The method is shown in the table or the notes.
- **[E]** an estimate: it depends on aim, spread or some behaviour I could not pin down exactly.

Unit conversions:

- Halo: 1 WU = 3.048 m, 1 tick = 1/30 s.
- Source (CS:S, HL2:DM, TF2): 1 u = 0.0254 m (`source/engine/src/source_movement/mod.rs:140`).
- Combat Arms: 1 u = 0.0185 m (`games/combat_arms/data/movement.toml:17`).

Victim names used throughout:

- **"Bare 100"** is a CS:S, HL2:DM or TF2 rung holder as UGG spawns them: 100 HP and no armour. CA is the same plus its 100 AP of gear, which only covers the body.
- **"Halo"** is a Halo-rung holder.

---

## 0. Summary

1. **A Halo holder has twice the effective health of everyone else against other games' guns, and half of it comes back on its own.**
   - The shield counts as a full 100-point armour. It absorbs 100% of every hit on every body part until it is empty. Only then does damage reach a 100-HP body.
   - Effective health is therefore 200 HP against every non-Halo gun, versus 100 HP for a CS:S, HL2 or TF2 holder [C].
   - The shield refills fully 10 s after the last hit: a 6 s delay, then 4 s to recharge. No other character heals.
   - Examples: a CS:S AK-47 needs 6 body hits on a Halo holder versus 3 on anyone else. An AWP or L96 needs 2 body hits instead of 1. The AK-47 needs 2 headshots instead of 1.
2. **Halo's guns hit non-Halo characters about twice as hard as they hit Spartans.**
   - On another game's body, one Halo damage point becomes 1/75 of a 100-HP health bar, so 1.33 HP.
   - The Spartan's flesh multiplier for that gun is also applied to the target.
   - The Halo pistol does 50 HP per body hit (2 hits kill) and kills with any headshot. Against a Spartan it needs 5 body hits or 3 head hits.
   - The assault rifle needs 8 bullets against others and 16 against a Spartan. The sniper rifle needs 1 body hit against others and 2 against a Spartan.
3. **Explosives and melee are one-hit kills on non-Halo characters over large areas.**
   - Every Halo explosion does at least its minimum, 80 points (= 107 HP), anywhere inside its outer radius. So any frag grenade within 7.6 m, and any rocket or plasma grenade within 6.1 m, kills a Bare-100 or CA holder outright [C].
   - A Halo holder gets 4 frag grenades with each fresh pick-up of a Halo gun in a life. The count drops to 2 at 5 or more players and to 1 at 9 or more.
   - The Halo melee is on its own button, reaches 2.44 m, and lands 0.1 s after the press. It does 75 HP standing still, 93 to 112 HP running and 103 to 131 HP lunging, and any hit from behind kills.
4. **The movement is very different but not clearly stronger.**
   - Gravity is half the field's: 9.78 m/s^2 against 15.2 to 20.3.
   - The jump peaks at 1.99 m, or 2.60 m with a crouch, against 0.54 to 1.45 m for the others.
   - Run speed is the field's highest base run at 6.86 m/s, and full speed is reached in 0.27 s.
   - The safe fall is 9.4 m, against 5.3 to 5.9 m for the others.
   - A jump is a slow 1.3 s arc, but the guns lose no accuracy in the air or on the move, unlike CS:S and Combat Arms.
5. **Halo's guns have autoaim and no other game's do.**
   - The shot is bent toward the target's centre when the crosshair is within 3 to 6 degrees of it, at up to 46 to 91 m depending on the gun.
6. **Weaker spots.**
   - Slow projectiles: the needler's needles fly at 12.2 m/s, the plasma pistol's bolts at about 76 m/s, rockets at 36.6 m/s.
   - The plasma rifle does only 8.5 HP a bolt to non-Halo characters, so 12 bolts and about 1.8 s to kill.
   - The Spartan is the tallest body: 2.13 m standing, eye at 1.89 m.
   - Its field of view is the narrowest: 70 degrees.
   - Its crouched pill (1.52 m) does not fit through Source crouch-only gaps (1.14 m).
   - It has no step-up: it rides over edges up to 0.375 m only.
7. **The ladder's power curve cannot see point 1.**
   - The score only measures how fast a gun kills a 100-HP baseline (`chimera/sim/src/minigame/power.rs:22-46`). The defensive bonus of being Halo plays no part in the order.
   - The Halo power reader also rates the plasma pistol as firing every tick, and ignores the needler's flight time and fuse (section 3.4).

**Verdict:** the Halo holder is clearly out of line on survivability (items 1 and 3 above). Its guns are fairly even with the field when they hit non-Halo bodies. They are weaker only where projectiles are slow. Section 6 has the ranking and the levers.

---

## 1. Movement and body, per character as UGG plays it

How UGG chooses the movement:

- The held gun's game decides the character's movement and body (`chimera/sim/src/identity.rs:1-30`; minigames `identity = "weapon"`).
- TF2 is a stand-in: Soldier numbers on the placeholder mover for every TF2 gun (`games/tf2/src/game/movement.rs:1-33`, `chimera/sim/src/movement/placeholder.rs`).
- CS:S and HL2:DM run the real Source movement; Combat Arms runs its own.
- The figures for the other games come from a delegated read of their code, with the file references given.

| | **Halo** | CS:S | HL2:DM | TF2 (stand-in) | Combat Arms |
|---|---|---|---|---|---|
| Gravity, m/s^2 | **9.78** [C] (0.0035652 WU/t^2, `games/halo_ce/data/movement.toml:9`) | 20.32 (`source_movement/mod.rs:404`) | 15.24 (`games/hl2dm/src/game/movement.rs:80`) | 20.32 (`games/tf2/src/game/movement.rs:30`) | 19.8 (`games/combat_arms/data/movement.toml:45`) |
| Jump launch speed, m/s | **6.40** [C] (0.07 WU/t, `movement.toml:40`) | 7.67 (`mod.rs:522`) | 4.06 (`movement.rs:82`) | 7.34 (`movement.rs:31`) | 5.94 (`movement.toml:42`) |
| Standing-jump apex (feet) | **1.99 m** [D] (0.6526 WU, `specs/halo_ce/movement.md` section 3) | 1.45 m [C] (measured 1.39) | 0.54 m [C] | 1.33 m [C] | 0.89 m [C] |
| Crouch-jump apex | **2.60 m** [D] (movement.md section 3: 0.8526 WU) | 1.67 m [C] (+0.216) | 1.45 m [C] (+0.914) | none | not found |
| Time in the air, standing jump | **1.30 s** [D] (39 ticks) | 0.755 s [C] | 0.533 s [C] | 0.723 s [C] | 0.60 s [C] |
| Run forward | **6.86 m/s** [C] (2.25 WU/s, `movement.toml:12`) | 3.8 to 6.6 m/s by weapon (knife 6.35, AK 5.61, AWP 5.33) | 4.83 m/s | 6.10 m/s | 5.13 m/s base, about 4.10 with vest and gun penalty |
| Back / side | 6.10 / 6.10 m/s [C] (`movement.toml:13-14`) | same as forward | same | same | same |
| Walk | none (no walk key) | 0.52 x run (knife 3.30) | 3.81 m/s | = run | 1.26 m/s |
| Crouch | 2.74 m/s forward [C] (0.9 WU/s, `movement.toml:19`) | 0.34 x run (knife 2.16) | 1.61 m/s | 2.03 m/s | 0.90 m/s |
| Sprint | none | none | 8.13 m/s on suit power | none | about 5.75 m/s with the reference loadout |
| Rest to full speed | **0.27 s** [C] (8 ticks: 0.32 WU/s per tick, `movement.toml:17`) | about 0.42 s | 0.10 to 0.15 s | 0.25 s to 95% | about 0.25 s |
| Full speed to stop | **0.27 s** [C] (same ramp, no friction) | about 0.54 s | about 0.41 s | 0.25 s to 5% | 0.1 s brake on key release |
| Air control | 3.2 m/s^2 toward the wish, 1/9 of ground; letting go brakes [D] (`movement.toml:24`, movement.md 3) | Source air-strafe (airaccelerate 10, 30 u/s wish cap); speed can be gained | same, and bunny-hop uncapped | eased at 1.5/s | 0.1 x ground |
| Step height | no step-up: rides edges up to 0.375 m at full run, 0.23 m crouched [D] (movement.md 5) | 0.457 m | 0.457 m | none | 0.55 m |
| Slope limit | 45 degrees [D] (`movement.toml:61`); 0.65x speed uphill, 1.25x downhill | about 45.6 degrees | about 45.6 degrees | about 45.6 degrees | 45 degrees |
| Fall damage | none below a 9.4 m drop [C]. Hard landing (up to 1 s locked) from 11.4 m. Damage 10 to 155 points over drops of 9.4 to 18.6 m, taken on the shield first. Death from about 17.9 m with full vitality [C] (`movement.toml:85-89`, movement.md 6) | from a 5.3 m drop; death about 15.8 m [C] | flat 10 HP above a 5.9 m drop (default) | none | from 5.4 m; 100 HP at about 9 m; 200 at 12.6 m |
| Ladder | 4.46 m/s up (look up), 8.57 m/s down [C] (0.04875 WU/t; 1.25 x run, movement.md 8) | 5.08 m/s | 5.08 m/s | none | 3.6 m/s |
| Standing height | **2.13 m** [C] (0.7 WU pill, `movement.toml:48`) | hull 1.575 m (per the read; `mod.rs:530`) | 1.83 m | capsule 1.8 m | 1.8 m |
| Crouched height | 1.52 m [C] (0.5 WU) | 1.14 m | 0.91 m | eye only, capsule unchanged | 1.44 m |
| Width | **1.22 m** pill diameter [C] (radius 0.2 WU) | 0.81 m | 0.81 m | 0.8 m | 0.54 m |
| Eye height stand / crouch | **1.89 / 1.07 m** [C] (`movement.toml:52-53`) | 1.63 / 1.19 | 1.63 / 0.71 | 1.62 / 1.0 | 1.49 / 1.08 |
| Field of view | **70 degrees** [D] (`games/halo_ce/data/weapons/_base.toml:22`) | 90 | 75 | 75 | 91.3 |

### What this means in play

**What a Halo holder can reach and the others cannot:**

- Ledges and crates up to about 2.0 m with a plain jump, and up to about 2.6 m with a crouch-jump. CS:S's best is about 1.67 m, HL2's 1.45 m, TF2's 1.33 m and Combat Arms' 0.89 m.
- Drops up to 9.4 m with no damage. Drops up to about 12.9 m cost only shield (75 points), which comes back [C].
- The highest base running speed. It also changes direction faster than CS:S: 0.27 s to full speed and 0.27 s to a stop, without friction.

**What the others can do that a Halo holder cannot:**

- Climb a 0.4 to 0.55 m step without jumping (Source and Combat Arms have step-up; Halo rides edges only up to 0.375 m).
- Fit through 1.14 to 1.44 m crouch gaps on Source and Combat Arms maps.
- Air-strafe and bunny-hop for extra speed (Source).
- Sprint (HL2 up to 8.13 m/s).
- Dodge sideways in the air. A Halo jump is a slow, predictable 1.3 s arc with little air control. A CS:S or HL2 player's arc lasts 0.53 to 0.76 s and can be bent.

**Weapon accuracy on the move:**

- Halo's weapon error grows only with how long the trigger is held. It has no term for moving, jumping or crouching (`games/halo_ce/data/weapons/*.toml` `trigger.error_*`; the weapon tick reads only the trigger).
- CS:S's AK-47 spread goes from 0.009 standing to 0.43 jumping (`games/cs_source/data/weapons/ak47.toml:52-60`).
- Combat Arms' M4A1 spread goes from 0.04 walking to 0.25 running to 0.70 jumping (`games/combat_arms/data/weapons/m4a1.toml:77-81`).
- HL2 and TF2 cones are fixed.
- So a Halo holder shoots as well in mid-air as standing, while a CS:S or Combat Arms holder who jumps cannot hit at range.

---

## 2. Survivability

### 2.1 What each character has at spawn in UGG

| Character | Health | Armour at spawn | Head / limb multipliers (of the attacking gun's game) |
|---|---|---|---|
| CS:S | 100 (`chimera/core/src/character.rs:29`) | none: UGG never gives kevlar or a helmet (`chimera/sim/src/weapon/mod.rs:1382-1417`; `docs/DESIGN.md` "Armour when the identity switches") | CS:S guns: head x4, stomach x1.25, legs x0.75 (`games/cs_source/data/weapons/_gun.toml:49`) |
| HL2:DM | 100 | suit 0, reset to 0 on every respawn (`games/hl2dm/src/game/suit.rs:386, 401-419`) | HL2 guns: head x3 (`games/hl2dm/data/weapons/_gun.toml:44`) |
| TF2 | 100 (no classes yet) | none | TF2 guns: all x1, no random crits (`games/tf2/data/weapons/_gun.toml:27`; `games/tf2/src/game/weapons/mod.rs:26-27`) |
| Combat Arms | 100 | 100 AP of default gear, given the first time a player is CA (`games/combat_arms/src/game/weapons.rs:1937-1948, 1580`) | CA guns: head x6, arm 0.9, thigh 0.9, shin 0.75, foot 0.5, applied after armour (`games/combat_arms/data/characters.toml:379-385`) |
| **Halo** | body 75 points = **100 HP** (`Health` 1.0) | shield 75 points = **100 armour** (`Armor` 1.0), full at each Halo life (`games/halo_ce/src/game/vitality/mod.rs:337, 481`) | against Halo guns: head x1 except the headshot flags; legs x0.8 on the body (`games/halo_ce/data/vitality.toml:44-75`) |

### 2.2 How armour reduces damage

There is one shared step, `hits::absorb` (`chimera/sim/src/weapon/hits.rs:197-259`):

- P is the coverage of the hit part. A fraction `(1 - 2P) + P x ratio` of the raw damage reaches health, where `ratio` is the gun's armour ratio.
- The armour loses the rest times its wear.
- How it runs out is the armour's own rule: `Cost` for CS:S, `Blocked` for Combat Arms and Halo.

Each game's armour:

- **CS:S kevlar:** P 0.5 on torso, arms and groin; wear 0.5 (`weapon/mod.rs:1390-1410`). Health takes `damage x ratio x 0.5`. The head is covered only with a helmet, which UGG never gives.
- **HL2 suit:** P 0.8 everywhere; wear 1, or 2 against blasts.
- **Combat Arms gear:** P 0.15 on torso, arms and legs. It covers bullets and burn only, not explosions or melee. A ratio-1 bullet keeps 85% of its damage.
- **Halo shield** (`games/halo_ce/src/game/vitality/mod.rs:257-277`): `shield_armour()` covers every part, including the head, and every damage class with P = 1 and wear 1, `Depletion::Blocked`. It also carries `weapon::ArmorRatio(1.0)` (`vitality/mod.rs:330`; `weapon/mod.rs:1456-1464`), which replaces the gun's own ratio.
  - With P = 1 and ratio 1, `(1 - 2) + 1 = 0`: **the shield takes 100% of every hit** until what it has left is no more than the hit.
  - The overflow (raw minus the shield left) then reaches health (`hits.rs:233-243`).
- **`weapon::PiecesOf("halo_ce")`** (`weapon/mod.rs:1466-1472`) marks those pieces as Halo's, so Combat Arms' armour step does not strip them off a Halo holder each tick (`apps/party_roster/tests/halo_ce/halo_ladder.rs` module doc).

### 2.3 Another game's bullet on a Halo holder

The rule is `vitality::take_foreign` (`vitality/mod.rs:405-427`) together with `hits::absorb`:

- Damage is **not rescaled**: the attacking game's damage goes in as it is.
- The attacking game's **head multiplier applies to the whole hit, shield included**: a CS:S headshot is x4 against the shield too. Halo's own rule, where headshots matter only once the shield is gone, does not apply to foreign hits.
- Halo's material modifiers do **not** apply. The module's own "Known shortcuts" says so (`vitality/mod.rs:66-70`).
- Any drop in shield or health restarts the 6 s shield delay, however small. Foreign hits neither stun nor appear in the Halo attacker list.
- Result: **200 effective HP**. Everything up to 100 goes to the shield, then 100 to the body. The engine kills at 0 [C].

### 2.4 A Halo gun on another game's character

The foreign branch of `games/halo_ce/src/game/damage.rs` (lines 410-433):

- The target is treated as an **unshielded Spartan's body**. Damage = `material body multiplier x amount / 75 x modifier(type 21, cyborg)`.
- So **one Halo point = 1/75 of a 100-HP health bar = 1.333 HP**. The gun's flesh modifier for type 21 is applied: pistol x1.5 (`damage.toml:55`), plasma rifle x0.5 (`:119`), plasma pistol x0.6 (`:138`), charged bolt x0.6 (`:160`), melee x1.4 (`:459`), others x1.0.
- Head hits:
  - "Can cause headshots" (pistol, sniper rifle; flag 0x2) → **instant kill** whatever the damage (`damage.rs:428-432`).
  - "Multiplayer headshots" (plasma rifle; 0x800) → x2.
  - Everything else x1 on the head.
- The result then goes through the victim's own armour at ratio 1: kevlar halves it, Combat Arms gear lets 85% through.
- A "lethal to the unsuspecting" melee (every Halo melee) from behind kills outright.
- The constant is `CROSS_GAME_MATERIAL = 21` (`damage.rs:459`).

### 2.5 Shield timing (`games/halo_ce/data/vitality.toml:36-41`; spec `health_shields.md`)

- Delay: 6.0 s after the last hit [D].
- Recharge: 0.008333 per tick, so 4.0 s from empty [D]. Total 10 s from broken to full.
- Under the party's 66.7 Hz tick, Halo still counts 30 Hz ticks (`ticks::halo_ticks`), so the delay stays 6 s.
- What resets the delay: any Halo hit whose shield damage reaches the minimum stun damage (0), or any hit while the shield is 0; any foreign drop in shield or health; a fall.
- The body never regenerates. Health packs exist only on Halo's own maps, and a party's Halo map places no pickups (`games/halo_ce/src/game/weapons/pickups.rs` module doc). So in UGG there is **no overshield, active camouflage or health pack for anyone**.

### 2.6 Effective hit points [C]

| Victim | Against CS:S guns | Against HL2 / TF2 / CA guns | Against Halo guns |
|---|---|---|---|
| CS:S, HL2, TF2 (bare) | 100 | 100 | 100 HP = 75 Halo points × modifier: 50 pistol points, 150 plasma-rifle points, about 125 plasma-pistol points |
| Combat Arms (100 AP gear) | body 100 / (0.7 + 0.15 x ratio): 107 for an AK-47, 101 for an AWP; head 100 | body 118, head 100 | body 118 / mod21; head 100 |
| **Halo** | **200** (head included) | **200** | 150 points (75 shield x shield modifier, 75 body x flesh modifier); the spec's shots-to-kill table |
| Halo with a regenerated shield | back to 200 within 10 s | same | same |

---

## 3. Time to kill

Assumptions for every cell:

- Perfect accuracy, every shot landing on the chest (body) or the head.
- Time is from the first shot (0 for a one-shot kill), with reloads counted when the magazine runs out.
- "n / t" means shots to kill / seconds.
- For shotguns, "all hit" means every pellet lands. That is only realistic up close: section 3.3 has the spread.

Sources:

- CS:S, HL2, TF2 and CA gun numbers come from each game's `data/weapons/<gun>.toml` (see 3.5).
- Falloff: CS:S `damage x range_modifier^(units/500)`; TF2 smoothstep distance modifier (`games/tf2/src/game/weapons/damage.rs:5-47`); Combat Arms range bands.
- Halo: data/weapons and damage.toml, with the direct-hit scale by impact speed (`weapons/params.rs:469-488`).
- Halo against Halo: the spec's shots-to-kill table (`specs/halo_ce/health_shields.md:1086-1110`, point blank).

All cells are [C] unless marked.

### 3.1 Other games' guns against each victim

Columns: Bare 100 (CS:S, HL2, TF2), CA with gear, Halo.

| Gun | dist | dmg body/head | Bare body | Bare head | CA body | CA head | **Halo body** | **Halo head** |
|---|---|---|---|---|---|---|---|---|
| CS Glock | 10 m | 19 / 76 | 6 / 0.75 | 2 / 0.15 | 7 / 0.90 | 2 / 0.15 | **11 / 1.50** | **3 / 0.30** |
| | 20 m | 15 / 60 | 7 / 0.90 | 2 / 0.15 | 8 / 1.05 | 2 / 0.15 | **14 / 1.95** | **4 / 0.45** |
| CS USP | 10 m | 28 / 112 | 4 / 0.45 | 1 / 0 | 5 / 0.60 | 1 / 0 | **8 / 1.05** | **2 / 0.15** |
| CS Deagle | 10 m | 45 / 180 | 3 / 0.45 | 1 / 0 | 3 / 0.45 | 1 / 0 | **5 / 0.90** | **2 / 0.23** |
| | 20 m | 38 / 152 | 3 / 0.45 | 1 / 0 | 3 / 0.45 | 1 / 0 | **6 / 1.12** | **2 / 0.23** |
| CS MP5 | 10 m | 22 / 88 | 5 / 0.32 | 2 / 0.08 | 6 / 0.40 | 2 / 0.08 | **10 / 0.72** | **3 / 0.16** |
| CS P90 | 10 m | 22 / 88 | 5 / 0.28 | 2 / 0.07 | 5 / 0.28 | 2 / 0.07 | **10 / 0.63** | **3 / 0.14** |
| CS AK-47 | 10 m | 35 / 140 | 3 / 0.20 | 1 / 0 | 4 / 0.30 | 1 / 0 | **6 / 0.50** | **2 / 0.10** |
| | 20 m | 34 / 136 | 3 / 0.20 | 1 / 0 | 4 / 0.30 | 1 / 0 | **6 / 0.50** | **2 / 0.10** |
| CS M4A1 | 10 m | 32 / 128 | 4 / 0.27 | 1 / 0 | 4 / 0.27 | 1 / 0 | **7 / 0.54** | **2 / 0.09** |
| CS M249 | 10 m | 34 / 136 | 3 / 0.16 | 1 / 0 | 4 / 0.24 | 1 / 0 | **6 / 0.40** | **2 / 0.08** |
| CS Scout | 10 m | 73 / 292 | 2 / 1.25 | 1 / 0 | 2 / 1.25 | 1 / 0 | **3 / 2.50** | 1 / 0 |
| CS AWP | 10 m | 114 / 456 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | **2 / 1.50** | 1 / 0 |
| CS M3 (9 pellets, all hit) | 10 m | 177 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | **2 / 0.88** | 1 / 0 |
| HL2 Pistol (0.5 s data cycle) | any | 8 / 24 | 13 / 6.0 | 5 / 2.0 | 15 / 7.0 | 5 / 2.0 | **25 / 13.4** | **9 / 4.0** |
| HL2 .357 | any | 75 / 225 | 2 / 0.75 | 1 / 0 | 2 / 0.75 | 1 / 0 | **3 / 1.50** | 1 / 0 |
| HL2 SMG1 | any | 5 / 15 | 20 / 1.43 | 7 / 0.45 | 24 / 1.72 | 7 / 0.45 | **40 / 2.92** | **14 / 0.97** |
| HL2 AR2 | any | 11 / 33 | 10 / 0.90 | 4 / 0.30 | 11 / 1.00 | 4 / 0.30 | **19 / 1.80** | **7 / 0.60** |
| HL2 Shotgun (7, all hit) | any | 63 | 2 / 0.33 | 1 / 0 | 2 / 0.33 | 1 / 0 | **4 / 1.00** | **2 / 0.33** |
| TF2 Scattergun (all hit) | 10 m | 75 | 2 / 0.62 | same | 2 / 0.62 | same | **3 / 1.25** | same |
| | 20 m | 38 | 3 / 1.25 | same | 4 / 1.88 | same | **6 / 3.12** | same |
| TF2 Minigun (spun up, fully ramped; +0.75 s spin-up) | 10 m | 42 per cycle | 3 / 0.21 | same | 3 / 0.21 | same | **5 / 0.42** | same |
| TF2 Rocket (direct) | 10 m | 98 | 2 / 0.80 | same | 2 / 0.80 | same | **3 / 1.60** | same |
| CA Beretta | 10 m | 26 / 156 | 4 / 0.54 | 1 / 0 | 5 / 0.72 | 1 / 0 | **8 / 1.26** | **2 / 0.18** |
| CA MP5A4 | 10 m | 20.3 / 122 | 5 / 0.32 | 1 / 0 | 6 / 0.40 | 1 / 0 | **10 / 0.72** | **2 / 0.08** |
| CA M4A1 | 10-20 m | 35 / 210 | 3 / 0.21 | 1 / 0 | 4 / 0.32 | 1 / 0 | **6 / 0.53** | 1 / 0 |
| CA M60 | 10-20 m | 47 / 282 | 3 / 0.23 | 1 / 0 | 3 / 0.23 | 1 / 0 | **5 / 0.46** | 1 / 0 |
| CA Remington 870 (8, all hit) | 10 m | 72 / 432 | 2 / 0.60 | 1 / 0 | 2 / 0.60 | 1 / 0 | **3 / 1.20** | 1 / 0 |
| CA Dragunov | any | 100 / 600 | 1 / 0 | 1 / 0 | 2 / 0.40 | 1 / 0 | **2 / 0.40** | 1 / 0 |
| CA L96A1 | any | 160 / 960 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | **2 / 1.40** | 1 / 0 |

Notes:

- Combat Arms' Remington has a maximum range of 1000 u (18 m), so it does no damage at 20 m.
- Combat Arms hits also carry a ±5% random roll, which is ignored here.
- **Pattern:** against a Halo holder, every non-Halo gun takes about twice the hits and 2 to 2.5 times the time. One-shot snipers (AWP, L96, Dragunov) become two-shot body kills that still kill with one headshot. Big headshot multipliers (Combat Arms x6, CS:S x4) narrow the gap.

### 3.2 Halo's guns

Two notes for this table:

- For victims other than Halo, damage is per hit as delivered (section 2.4).
- The Halo-victim columns are the spec's point-blank counts, with time from the fire rate.

| Halo gun | dist | per hit, Halo points / HP on others | Bare body | Bare head | CA body | CA head | Halo victim body | Halo victim head | Projectile time to target |
|---|---|---|---|---|---|---|---|---|---|
| Assault Rifle (15/s, 60 rounds) | 10 m | 10 / 13.3 | 8 / 0.47 | 8 / 0.47 | 9 / 0.53 | 8 / 0.47 | 16 / 1.00 | 16 / 1.00 | 0.010 s (988 m/s) |
| | 20 m | 10 / 13.3 | 8 / 0.47 | 8 / 0.47 | 9 / 0.53 | 8 / 0.47 | 16 / 1.00 | 16 / 1.00 | 0.020 s |
| Pistol (9 ticks, 12 rounds) | 10/20 m | 25 / **50** | **2 / 0.30** | **1 / 0 (any headshot kills)** | 3 / 0.60 | 1 / 0 | 5 / 1.20 | 3 / 0.60 | 0.011 / 0.022 s |
| Plasma Rifle (7 rising to 10/s) | 10 m | 12.75 / 8.5 | 12 / about 1.8 [E] | 6 / about 0.83 [E] | 14 / about 2.2 [E] | 6 / 0.83 | 14-15 / about 1.9 [E] | 8-9 / about 1.1 [E] | 0.07 s [E] (152 m/s, slowing) |
| | 20 m | 12.5 / 8.3 | 13 / about 2.0 | 7 / about 1.0 | 15 / about 2.3 | 7 / 1.0 | as above | | 0.14 s [E] |
| Plasma Pistol, tapped (no rate limit; heat 0.16 per bolt) | 10 m | 17.3 / 13.8 | 8 / 0.47 at the 2-tick maximum, about 1.0 at a human 7 taps/s [E]; the 8th bolt about overheats (cools 1.15 s) | same (no headshot) | 9 | 8 | 13-16 / overheats once: about 2.5-3 s [E] | same | 0.13 s (76 m/s) |
| | 20 m | 16.5 / 13.2 | 8 | 8 | 9 | 8 | as above | | 0.27 s |
| Charged bolt (0.6 s charge, homing 0.5 rad/s, emp) | 10 m | 70 / 56 | 2 (with a tap) / about 0.8 [E] | | | | strips the whole shield; then 1 pistol headshot = kill (spec) | | 0.22 s (46 m/s) |
| Shotgun (15 pellets, 10 degree cone, 1/s) | ≤ 5 m | 15 x 21.5 / 28.7 per pellet | 1 / 0 if 4+ pellets land | | | | 1 shell at point blank (spec) | | 0.02-0.05 s |
| | 10 m | | about 1.4 pellets land [E]: 3 shells / 2 s [E] | | | | 2-3 shells [E] | | |
| Sniper Rifle (0.5 s, 4 rounds) | any | 101 / 135 | **1 / 0** | 1 / 0 | 1 / 0 | 1 / 0 | 2 / 0.50 | 1 / 0 | 0.003-0.007 s |
| Needler (3 rising to 10/s, homing) | 10 m | 10 per detonating needle / 13.3 | 8 needles (7 with a supercombine, if it works on other bodies: unverified): about 3.0 s [E] = about 1.4 s firing + 0.82 s flight + 0.75 s fuse | same | 9 | 8 | 16 needles: about 3.5 s [E] | same | 0.82 s (12.2 m/s) |
| | 20 m | | about 3.8 s [E] | | | | about 4.3 s [E] | | 1.64 s |
| Rocket Launcher (2 s, 2 rounds) | any | 300-330 at the centre / 400-440 | 1 / 0, and **any blast centre within 6.1 m kills** | | same | | direct 1 (spec); splash kills within 4.7 m of the centre [C] | | 0.27 / 0.55 s (36.6 m/s) |
| Frag grenade (4 per life, see 4.3) | | 120 at ≤ 4.6 m to 80 at 7.6 m / 160 to 107 HP | **kills within 7.6 m** | | **kills** (CA gear does not cover blasts) | | survives at body 0.4 (spec) | | throw 30.5 m/s; armed 1.5 s + 0.5 s at rest |
| Melee (any Halo gun) | 2.44 m reach | standing 40 / 75; running 50-60 / 93-112; lunge 55-70 / 103-131 | standing 2 hits; running 1 hit (64% of draws) or 2; lunge 1 hit; from behind 1 hit | | same: CA gear does not cover melee | | standing 3, running 2, lunge 2, from behind 1 (spec) | | hits 0.1 s after the press; repeats every 0.9-1.3 s |

Cross-character comparison of the field's representative guns at 10 m, body shots, Bare-100 victim [C]:

| Class | CS:S | HL2 | TF2 | CA | Halo |
|---|---|---|---|---|---|
| Pistol | Glock 0.75 s, USP 0.45 | Pistol 6.0 | none | Beretta 0.54 | Plasma pistol 0.47 at 15 taps/s, about 1.0 at 7 taps/s [E] |
| Heavy pistol | Deagle 0.45 | .357 0.75 | none | none | **Pistol 0.30 s, any headshot an instant kill** |
| SMG | MP5 0.32, P90 0.28 | SMG1 1.43 | none | MP5A4 0.32 | Plasma rifle about 1.8 [E], needler about 3.0 [E] |
| Rifle | AK 0.20, M4 0.27 | AR2 0.90 | Minigun 0.21 + spin-up | M4A1 0.21 | AR 0.47 |
| Shotgun (all hit) | M3 one shot | 0.33 | 0.62 | 0.60 | one shell only ≤ about 5 m [E] |
| Sniper | AWP one shot (1.5 s cycle) | none drawn | none | L96 one shot (1.4 s cycle) | **Sniper one shot, 0.5 s cycle** |
| Launcher | none drawn | none drawn | Rocket 2 hits, 0.8 s, splash 3.7 m to half damage | install-dependent | **Rocket: one shot, lethal splash radius 6.1 m** |

### 3.3 Spread and the shotguns [E]

Assumptions: the power curve's model, a pellet hitting with chance min(1, (0.3 m / (tan(cone) x d))^2), and an aim area of 0.3 m radius.

- Halo's shotgun has a 10 degree cone (`games/halo_ce/data/weapons/shotgun.toml:77`): 1.76 m wide at 10 m, so about 3% of the pellets land.
- At 3 to 5 m most pellets land and one shell kills anything.
- Its pellets keep full damage to 14 WU (42 m) and stop at 40 WU (122 m). The deceleration timer only starts after the third update (`params.rs:480-482`), so it is a strictly close-range gun.

### 3.4 The ladder's power curve and what it thinks of Halo's guns

The Halo power reader is `games/halo_ce/src/game/weapons/mod.rs:240-275`. It feeds `chimera/sim/src/minigame/power.rs` `time_to_kill` and `score`. Its scoring parameters (`chimera/sim/data/power_curve.toml`):

- a baseline of 100 HP with no armour;
- distances of 10, 15 and 20 m;
- a 0.3 m aim radius;
- a 5% headshot share, with a default head multiplier of 2;
- 0.25 s of reaction time.

Its damage is `amount(speed scale, mean draw) x modifier(21) / 75`, the same mapping the cross-game damage path uses. I evaluated the formula by hand rather than running the `ladder_power` console command, so these are [C]:

| Halo gun | Band | Score | Model TTK at 10 / 15 / 20 m | My perfect-aim TTK (Bare 100) | Agreement |
|---|---|---|---|---|---|
| Sniper | heavy | 4.05 | 0 / 0 / 0 | 0 | agrees |
| Rocket | specials | 4.03 | 0 / 0 / 0 | 0 | agrees, but splash is not modelled |
| Plasma pistol | warm-up | **2.33** | 0.20 / 0.23 / 0.23 | 0.47 at the fastest possible tapping, about 1 s by hand, plus heat | **model over-rates it**. Its `rounds_per_second` is [0, 0], so `required_ticks` is 0 and the reader's `max(1)` gives one tick (1/30 s) per bolt; heat is ignored. That puts it at the top of the warm-up band. |
| Pistol | heavy | 1.89 | 0.30 / 0.30 / 0.30 | 0.30 body, headshot instant | agrees on bodies. The instant headshot is not modelled (default head x2 is used). |
| Plasma rifle | rising | 0.40 | 1.47 / 2.95 / 5.25 | 1.8 to 2.0 | close. The reader's interval is 4 ticks (3 x (1 + 0.005 x 0.2) rounded up), not the real 3-tick top rate. |
| Assault rifle | peak | 0.33 | 1.60 / 3.81 / 9.80 | 0.47 | model under-rates it: it charges 2 to 6.5 degrees of error but does not know about the autoaim, which centres the cone on the target |
| Needler | rising | 0.067 | 8.9 / 19 / 36 | about 3 to 4 s | model is wrong both ways: 4 degrees of error with no homing, and no 12 m/s flight or 0.75 s fuse |
| Shotgun | rising | 0.067 | 8.2 / 20 / 37 | about 2 s at 10 m, one shot at ≤ 5 m [E] | plausible for 10-20 m |

Three structural points:

- The power score only orders guns inside a band (`power.rs` doc).
- It measures attack against a 100-HP target. It knows nothing about the holder's own 200 HP or regenerating shield, grenades, melee or autoaim.
- Combat Arms' install weapons and the other games' readers were not evaluated, so I cannot place Halo's scores among theirs numerically.

### 3.5 Where the other games' gun numbers come from

- CS:S: `games/cs_source/data/weapons/<gun>.toml`, e.g. `ak47.toml:16` (cycle) and `:27` (damage), with `armor_ratio` and falloff in the `[damage]` table.
- HL2: `games/hl2dm/data/weapons/{pistol,357,smg1,ar2,shotgun}.toml`.
- TF2: `games/tf2/data/weapons/{scattergun,shotgun_primary,minigun,rocketlauncher}.toml`.
- CA: `games/combat_arms/data/weapons/{beretta_m92fs,mp5a4,m4a1,m60,remington_870,dragunov_svd,l96a1}.toml`. The ladder claims these in `games/combat_arms/src/game/parts.rs:28-36`.
- The HL2 RPG, crossbow, frag and SLAM are not in the draw (`games/hl2dm/src/game/parts.rs:23-29`).

---

## 4. Differences that are not a number in the table

### 4.1 The recharging shield

- 10 s out of combat restores 100 of a Halo holder's 200 effective HP. A partial shield takes 6 s plus the missing fraction x 4 s.
- No other character recovers anything in UGG: no regeneration, no health kits on drawn maps (HL2 chargers and batteries and the CA first-aid device, where a map places them, are untested here).
- In a gun game, where one player takes on many fights in a row, this compounds the 200-HP advantage.

### 4.2 Autoaim and bending shots

The code is `games/halo_ce/src/game/weapons/aim.rs` and `systems.rs:395-422, 935-953`.

- It is on for every Halo gun except the flamethrower, which is not in the ladder, and the rocket launcher, whose autoaim angle is 0.
- The target can be any living enemy of any game whose centre is within the angle and range. The shot is bent toward that centre (the character's Transform: `chimera/core/src/character.rs:18` puts it at the capsule centre, so the chest or waist, never the head).
- Strength is full within half the angle and half the range, then falls to 0 at the full angle and range. The bend is capped at the autoaim angle. The weapon's error cone is applied after the bend.

| Gun | Autoaim angle (full / zero) | Range (full / zero) | Data |
|---|---|---|---|
| Assault rifle | 3 / 6 degrees | 38 / 76 m | `assault_rifle.toml:51-52` |
| Pistol | 1.5 / 3 degrees | 46 / 91 m | `pistol.toml:48-49` |
| Plasma rifle | 2.5 / 5 degrees | 38 / 76 m | `plasma_rifle.toml:51-52` |
| Plasma pistol | 2.5 / 5 degrees | 30 / 61 m | `plasma_pistol.toml:52-53` |
| Needler | 3 / 6 degrees | 38 / 76 m | `needler.toml:49-50` |
| Shotgun | 3 / 6 degrees | 23 / 46 m | `shotgun.toml:48-49` |
| Sniper | 0.5 / 1 degree, zoomed only; ÷2 at 2x and ÷8 at 8x | 53 / 107 m, ×2 and ×8 zoomed | `sniper_rifle.toml:28, 50-51` |
| Rocket | none (angle 0) | | `rocket_launcher.toml:49` |

- At 20 m, a 3 degree half-angle is about 1.05 m: a crosshair a metre off a target at 20 m still sends the assault rifle's cone to the target's centre [C].
- Guided projectiles also home: needles at 1.57 rad/s, half of that against players per the data comment (`needler.toml:105`); the charged bolt at 0.51 rad/s (`plasma_pistol.toml:153`).
- **No other game's gun has any aim assist.** A search of all games' data and code found it only in Halo.

### 4.3 Grenades

- Every Halo holder throws grenades with the grenade button, next to any Halo gun (`games/halo_ce/src/game/parts.rs` doc).
- **Count:** 4 frags and no plasma grenades at "a new life" (`games/halo_ce/data/grenades.toml:26-27`; `weapons/systems.rs:803-810`). The count is capped by the most players ever seen in the match: 2 at 5 to 8 players, 1 at 9 or more.
- **When the count refills** [D, reading of `systems.rs:798-810`]:
  - The new-life flag `hands.armed` is reset only on a tick when the player holds a Halo gun and is dead.
  - So a player first gets frags on reaching a Halo rung, and keeps the count while climbing through Halo rungs in the same life.
  - A player who dies holding another game's gun may not get a refill on the next Halo rung. I did not verify whether UGG strips the inventory on death, which decides this.
- **Damage against other characters** (area hits, no material, modifier 21 = 1.0; `damage.toml:236-256, 325-343`; falloff `weapons/explode.rs:97-125`):

| Explosion | Damage to Bare 100 / CA | Damage to a full Spartan |
|---|---|---|
| Frag | 160 HP within 1.5 WU (4.6 m), falling linearly to 107 HP at 2.5 WU (7.6 m). The amount at s → 0 is the minimum 80 points. CA gear does not cover blasts. Kevlar, if carried in (4.7), halves it. | strips the shield and leaves the body at 0.4 |
| Plasma grenade | 160 HP at 0.5 WU to 107 at 2 WU (6.1 m); stuck: 160 + 160 | stuck: kills; unstuck: survives |
| Rocket splash | 400 to 440 HP within 1.5 m, 107 at 6.1 m | kills only within about 4.7 m [C] |

- A frag needs line of sight from the epicentre (four core probes).
- Arming takes 1.5 s from creation, then a 0.5 s fuse once it rests. It is thrown at 30.5 m/s.
- **Against characters with no grenades:**
  - CS:S, TF2 and CA holders carry none beside their gun. A grenade is only ever a rung of its own (CS:S HE, CA M67; `chimera/sim/src/minigame/ladder.rs:117, 1237`).
  - HL2 SMG1 and AR2 rungs carry 2 alt-fire rounds (`ladder.rs:~104-110`).
  - Those grenade rungs, against a Halo holder: CS:S HE at most 100 → all shield. CA M67 at most 120 → 100 shield + 20 body. Neither kills a full Spartan.

### 4.4 Melee

Halo (data in `damage.toml:444-461`, `vitality.toml` `melee_lunge_scale`; reach and timing in `weapons/systems.rs:1144-1214`, `params.rs:530-534`):

- **Always available:** its own button, no weapon switch.
- **Reach:** 25 probes out to 0.8 WU (2.44 m) from the eye, spread ±0.2 WU (±0.61 m) at the far end.
- **Timing:** the hit lands 3 to 8 ticks (0.1 to 0.27 s) after the press, depending on the gun. A full melee takes 0.9 to 1.3 s.
- **Damage:**
  - Speed scale s = forward speed / run speed, clamped to 0..1, or 1.5 after more than 15 ticks in the air (a lunge).
  - Amount = (1 - s) x 40 + s x U(50..60), times the flesh modifier 1.4.
  - Against Bare 100: standing 75 HP, running 93 to 112, lunging 103 to 131.
  - From behind ("lethal to the unsuspecting"): instant kill on any character.

The other games' melee:

| Melee | Damage | Reach | Rate |
|---|---|---|---|
| CS:S knife | slash 20 (15 when repeated), stab 65 (195 from behind) | 0.81-1.22 m | 0.4-1.1 s |
| HL2 crowbar | 25 | 1.9 m | 0.4 s |
| HL2 stunstick | 40 | 1.9 m | 0.8 s |
| CA M9 | 45 / 75 | 1.9 m | 0.65 / 1.1 s |
| TF2 | none | | |

- All of these need a switch from the gun. UGG's kit gives each game's knife, and Halo none (`games/halo_ce/src/game/parts.rs:56`).
- **Result:** a Halo holder kills any Bare-100 player by running into melee range and pressing one button. A CS:S knife stab needs 2 hits on a Bare 100 and does 65 of a Halo holder's 200.

### 4.5 The motion tracker

- Halo's HUD has a motion tracker (`games/halo_ce/src/ui/hud/mod.rs:9`, `draw.rs:415`): blips of moving enemies near the player. No other HUD has anything like it.
- Per `docs/plans/active/halo-ce.md:571-574`, the user asked for the party HUD to drop it, and that work is in progress.
- Until it lands, a Halo holder sees nearby moving enemies through walls. I did not establish whether the version currently built still draws it in the party apps.

### 4.6 Hit volume, body and camera

- **Spartan hit boxes:** bounding boxes of the collision model's nodes, read from the disc (`games/halo_ce/src/game/body/hits.rs:1-23`). **I could not compute the frontal area**: the boxes come from the disc image at run time and I did not build or run anything.
- Proxies: the Spartan is 2.13 m tall and its movement pill 1.22 m wide, against 1.58 to 1.83 m tall and 0.54 to 0.81 m wide hulls for the others. Its head sits about 0.25 to 0.4 m higher than other heads [E]. It is the largest target in the field, which is a disadvantage.
- Halo's head multiplier from another game's gun is that game's (x3 to x6), applied to the full 200.
- **Eye height** 1.89 m [C]: it sees over cover others cannot, but its head also shows above cover sooner.
- **Field of view:** 70 degrees (`_base.toml:22`) against 75 to 91. Less peripheral vision, but targets appear larger on screen [E].

### 4.7 Armour that enters and leaves with the Halo identity

This follows from `vitality/mod.rs:324-338` ("one it had stays") and the module doc ("one leaving takes them off, keeping `Armor`"):

- **Entering.** A player arriving on a Halo rung with an `Armor` component keeps its amount as the shield, then recharges.
  - After an HL2:DM rung, armour is the suit's 0, inserted by `games/hl2dm/src/game/suit.rs:386`. The shield then starts empty and takes 4 s to fill.
  - After a Combat Arms rung, the shield starts at whatever Combat Arms armour is left.
  - With no armour at all, the shield starts full.
- **Leaving.** The shield's value **stays as the player's armour** on the next rung (`docs/DESIGN.md` "Armour when the identity switches"; `apps/party_roster/tests/halo_ce/halo_ladder.rs` checks the pieces are removed, not the amount):
  - On a CS:S rung it becomes up to 100 kevlar, with no helmet. It halves ratio-1 damage to the torso and arms.
  - On an HL2 rung it becomes up to 100 suit armour (`insert_if_new` keeps it, `suit.rs:378-386`): 80% of each hit absorbed, until the next death.
  - On a Combat Arms rung it becomes the gear's AP.
- So a Halo rung also makes its holder tougher on the following rungs [D reading; not run]. I did not verify whether the CS:S or TF2 identity resets armour on respawn in UGG.

### 4.8 Other details

- **Stun.** Plasma hits slow a Spartan's movement (`vitality::Stun`). Foreign hits never stun a Spartan, and Halo plasma does not stun other characters (they have no `Stun`).
- **Spawn protection** is the same 2 s for everyone.
- **Kill credit.** Foreign hits on a Halo holder are not in Halo's attacker list (`vitality/mod.rs:66-70`). For example, a Halo holder dying from a fall shortly after being shot by a CS:S player credits nobody, or only a Halo attacker.
- **Fall damage** hits the shield first, so drops up to about 12.9 m cost only regenerating shield [C].

---

## 5. Where the Halo holder is at a disadvantage

1. **Slow projectiles against hitscan.** Every other game's representative gun is hitscan except the TF2 and CA rockets. A moving target dodges most of these:
   - needles at 12.2 m/s: 0.8 to 1.6 s to reach 10 to 20 m, plus a 0.75 s fuse;
   - plasma pistol bolts at about 76 m/s;
   - charged bolts at 46 m/s;
   - plasma rifle bolts at about 150 m/s;
   - rockets at 36.6 m/s.

   The AR, pistol, sniper and shotgun are effectively instant (400 to 3000 m/s).
2. **The plasma rifle and plasma pistol against non-Halo bodies.** Their flesh modifiers are 0.5 and 0.6, tuned to be weak against flesh and strong against shields. Against Bare 100 the plasma rifle needs 12 to 13 bolts (about 1.8 to 2.0 s) against an SMG field of 0.28 to 0.32 s. The plasma pistol needs 8 bolts and overheats near the 8th.
3. **The needler** takes about 3 to 4 s to kill at 10 to 20 m against the field's 0.2 to 0.9 s.
4. **The biggest body:** tallest, widest pill, highest head.
5. **Narrowest field of view** (70 degrees), with no sprint, walk key, step-up or Source air-strafing. It cannot use crouch-only gaps under 1.52 m.
6. **A long, slow jump arc** (1.3 s, half gravity): easy to predict mid-air. On the other hand, there is no accuracy loss while shooting.
7. **The assault rifle at range.** Its error grows to 6.5 degrees over 0.6 s of holding. Even with autoaim it lands only a share of its bullets beyond about 15 m [E]. Perfect-aim TTK is 0.47 s against the field's 0.20 to 0.27 s rifles.
8. **The rocket launcher's** 2 s cycle and 4.2 s reload, with 2 rounds loaded.
9. **Halo against Halo** fights are long by design: 16 AR bullets, 5 pistol body shots. When two Halo rungs meet, both are slower to kill than any other pair.

---

## 6. Verdict

### Clearly out of line (ranked)

**1. Survivability against other games' guns: 200 effective HP, refilled to that within 10 s.**

- Every non-Halo gun needs about twice the hits; snipers need two body shots. The headshot only multiplies both halves.
- Measured from code [D] and computed [C]: section 2.3 and table 3.1.

Levers that exist today:

- `weapon::ArmorRatio(1.0)` on the shield (`games/halo_ce/src/game/vitality/mod.rs:330, 380`). With P = 1 the fraction that reaches health is `ratio - 1`, so a ratio above 1 bleeds part of each hit through.
- The shield's coverage per part and wear in `shield_armour()` (`vitality/mod.rs:260-277`). Head coverage, or wear above 1 (a shield point worth fewer than one HP), are both data-like constants in code.
- The normalization "shield 1.0 = `Armor` 1.0 = 100 points" (`vitality/mod.rs:337, 481`; `Armor` "1.0 = 100", `weapon/mod.rs:1373`).
- The shield delay and recharge in `games/halo_ce/data/vitality.toml:36, 41`. These also change Halo against Halo.
- Halo's unread game-type "health option", clamped 0.25 to 4 (`vitality.toml` `health_option_min/max`; listed as a shortcut in `vitality/mod.rs:62-63`).
- The `game_multiplier` field of `vitality::Event` (`damage.rs:350`, always 1.0 now).

**2. Halo's explosives and melee against non-Halo characters.**

- Frag lethal radius is 7.6 m, rocket and plasma grenade 6.1 m: every in-radius hit does at least 80 points (107 HP) to any 100-HP body, and Combat Arms gear does not cover blasts.
- Frags come 4 per Halo life (2 at 5+ players, 1 at 9+).
- Melee is always available, reaches 2.44 m, and is a one-hit kill when running or lunging.

Levers:

- `CROSS_GAME_MATERIAL` (`games/halo_ce/src/game/damage.rs:459`) and the `1/maximum_body` scale in the foreign branch (`damage.rs:418-422`). Together they make one Halo point equal 1.33 HP.
- Each effect's type-21 modifier in `games/halo_ce/data/damage.toml` (melee `:459`; frag, rocket and plasma grenade 1.0). These also change Halo against Halo body damage.
- The explosion radii and lower bounds in `damage.toml:239-243, 325-332`.
- Grenade counts and caps in `games/halo_ce/data/grenades.toml:26-27`.
- The melee speed scale and lunge in `vitality.toml` `melee_lunge_scale`.

**3. The Halo pistol and sniper against non-Halo characters.**

- The pistol does 50 HP per body hit, kills in 0.3 s, and kills instantly with any headshot. Autoaim covers 3 degrees out to 91 m.
- The sniper kills with one body shot on a 0.5 s cycle and 4-round magazine, against the AWP's 1.5 s and the L96's 1.4 s.
- Both carry the "can cause headshots" flag, which the foreign branch turns into an instant kill (`damage.rs:428-432`).

Levers:

- The pistol's type-21 modifier of 1.5 (`damage.toml:55`).
- The headshot flags in `damage.toml` (`flags = 0x2`) or their treatment in the foreign branch.
- The power curve's placement: the pistol is already in the heavy band.

**4. Autoaim only for Halo's guns.**

- The shot bends 3 to 6 degrees toward a target's centre at 46 to 91 m.

Levers:

- Per-gun `[hooks."halo_ce:weapon".aim_assist]` with `enabled = false` (weapon data comment, e.g. `assault_rifle.toml:48-49`), or the angles and ranges there.

**5. Leaving a Halo rung with its shield as armour** (section 4.7).

- Up to 100 kevlar on CS:S, 100 suit armour on HL2 or AP on Combat Arms.

Levers:

- `leave_halo`'s rule that keeps `Armor` (`vitality/mod.rs:342-394`).
- The design rule in `docs/DESIGN.md` "Armour when the identity switches".

### About even

- Halo's AR, perfect aim: 0.47 s to kill a Bare 100. The field ranges from 0.20 s (CS:S rifles) to 0.90 s (HL2 AR2).
- Run speed: 6.86 m/s against 6.1 to 6.6 m/s for the fast others. HL2 sprints faster.
- Jump height and fall tolerance are different but both cut two ways: high, slow arcs, and no step-up or narrow gaps.
- The shotgun at very close range: a one-shot kill, as are the CS:S M3 and the HL2 shotgun.
- The rocket launcher's direct hit: one shot, but slow and with 2 rounds.

### Weaker

- Plasma rifle, plasma pistol and needler against non-Halo bodies (section 5).
- A bigger target, the narrowest field of view, a predictable jump arc.
- Halo against Halo duels, which are slow for both sides.

### The power curve's view

- The ladder orders Halo's guns by attack only. The holder's defence, grenades, melee and autoaim are invisible to it.
- The reader over-rates the plasma pistol (scored as firing every tick) and mis-rates the needler (no flight time or fuse) and the AR (no autoaim).

Levers:

- `games/halo_ce/src/game/weapons/mod.rs:240-275`, for the interval of rate-0 triggers and for projectile flight.
- `chimera/sim/data/power_curve.toml`, the band shares.

---

## 7. What I could not determine

- **The Spartan's hit-box frontal area.** The boxes come from the disc's collision model at run time.
- **The Halo power scores in context.** I did not run `ladder_power`, so they are not ranked numerically against the CS:S, HL2, TF2 or CA scores, whose readers I did not evaluate.
- **The needler supercombine on non-Halo bodies.** Whether ">6 needles in one biped" counts other games' characters.
- **When grenade counts refill**, as described in 4.3: it depends on whether UGG clears the inventory on death.
- **Whether armour carried into CS:S or TF2** is reset on respawn there. HL2 resets its suit to 0 on respawn.
- **Whether the motion tracker** still draws in the current party build.
- **Combat Arms' install-only weapons** (launchers, extra knives) and their power. They are not in the repository.
- **Which UGG maps place health or armour pickups** for the other games.
- **Exact plasma-rifle timing.** Its rate ramp's tick pattern is approximated: "5, 5, then 4 ticks" per the data comment.
- **Combat Arms latency and the ±5% roll** are left out of its times to kill.
- **The other games' movement numbers** come from a delegated read of their code, with the file references given. I spot-checked the Halo and armour paths myself, not every number for the other games.

## Appendix: after the balance rule (2026-11-15)

The rule (the owner's decision): a Halo player a little tankier than
others, its guns not also overpowered (Halo against Halo unchanged then; since 2026-11-17 it counts the table: 1.125).

- **Incoming** (another game's damage on a Halo player): counted 1.5
  times (`incoming_scale`), so a full Halo player has about 133
  effective hit points against other games' guns (was 200); the shield
  is taken first and still recharges.
- **Outgoing** (a Halo weapon on another game's character): one Halo
  point is one hit point (`halo_ce = 0.75` in the party's per-game
  damage table; it was 1.333).
- **HL2:DM** deals 1.5 times in the party apps (`hl2dm = 1.5`).
- The shield is no longer left as armour on the next rung; the ladder's
  power readers see the table.

Body hits to kill at 10 m (head where noted), from the tests that pin
them:

| Gun | Damage a hit | A 100-point body | Halo player before | Halo player now |
|---|---|---|---|---|
| CS:S AK-47 | 35.4 | 3 | 6 | 4 |
| CS:S AK-47, head | 141.7 | 1 | 2 | 1 |
| CS:S AWP | 114 | 1 | 2 | 2 (one shot needs an incoming scale of 1.754) |
| CS:S Desert Eagle | 45.7 | 3 | 5 | 3 |
| CS:S MP5 | 22.7 | 5 | 9 | 6 |
| HL2:DM .357 (with 1.5) | 112.5 | 1 | 2 | 2 |
| HL2:DM SMG1 (with 1.5) | 7.5 | 14 | 27 | 18 |
| TF2 Scattergun | 75.3 | 2 | 3 | 2 |
| Combat Arms M4A1 | 35 | 3 | 6 | 4 |

Halo's weapons on other games' characters, hits to kill (a 100-point
body / a Combat Arms body with gear):

| Weapon | Before | After |
|---|---|---|
| Assault rifle | 8 / 9 | 10 / 12 |
| Pistol, body | 2 / 3 | 3 / 4 |
| Pistol, head | 1 / 1 | 1 / 1 |
| Sniper rifle, body | 1 / 1 | 1 / 2 |
| Plasma rifle | 12 / 14 | 16 / 19 |
| Plasma pistol | 7 / 9 | 10 / 11 |
| Shotgun, point blank | 1 / 1 | 1 / 1 |
| Rocket, direct and at 2 and 4 m | 1 | 1 |
| Rocket's blast at 6 m | 1 / 1 | 2 / 2 |
| Frag at 2, 4 and 6 m | 1 | 1 |
| Frag at 7.5 m | 1 / 1 | 2 / 2 |
| Melee, standing | 2 / 2 | 2 / 2 |
| Melee, running or lunging | 1 / 1 | 2 / 2 |
| Melee, from behind | 1 | 1 |

HL2:DM's guns by the table's value (hits, seconds; a 100-point body /
Combat Arms with gear / a Halo player):

| Gun | 1.0 | 1.5 (now) | 2.0 |
|---|---|---|---|
| Pistol | 13 (6.0 s held) / 15 / 17 | 9 (4.0 s) / 10 / 12 | 7 (3.0 s) / 8 / 9 |
| .357 | 2 (0.75 s) / 2 / 2 | 1 / 2 / 2 | 1 / 1 / 1 |
| SMG1 | 20 (1.43 s) / 24 / 27 | 14 (0.98 s) / 16 / 18 | 10 (0.68 s) / 12 / 14 |
| AR2 | 10 (0.90 s) / 11 / 13 | 7 (0.60 s) / 8 / 9 | 5 (0.40 s) / 6 / 7 |
| Shotgun | 2 / 2 / 3 | 2 / 2 / 2 | 1 / 1 / 2 |

The ladder's power scores of Halo's guns before and after the readers
were corrected: the plasma pistol 2.33 (top of its band) to 0.63; the
needler 0.069 to 0.355; the assault rifle 0.325 to 0.162 (last of the
peak band, by its burst model: `burst_seconds`); the pistol 2.00 to
0.97; the rocket launcher 4.40 to 1.71; the sniper rifle 4.40 to 4.32;
HL2:DM's AR2 0.69 to 0.99 and its .357 1.02 to 4.08.

Measured from bots since (one simulated minute on the greybox, kills a
minute): the pistol 14, the rocket launcher 13, the shotgun 12, the
needler 9, the assault rifle 5, the plasma rifle 3, the sniper rifle 15
and the plasma pistol 4 once the bots could pace and tap them; CS:S's
AK-47 13.

Gravity and jumps were left alone by the owner's decision (section 1's
table stands): a map-owned gravity would have a CS:S character jump
about 3.0 m on a Halo map.

## The table and the switch (2026-11-16)

**One table.** Every number that applies only where games meet is in
`apps/party_kit/data/party_balance.toml` (read by `party_kit::balance`
into the engine's `chimera_sim::weapon::balance::Balance`):

- `[games.<game>]` `outgoing`: what the game's weapons deal to another
  game's body, times (`halo_ce` 0.75, `hl2dm` 1.5); `incoming`: what the
  game's bodies take from another game's weapon, times (`halo_ce` 1.5).
  A game or field not listed is 1.
- `[bodies]` `"<game>:<body>" = { incoming = k }`: a body's own incoming
  over its game's, by the id the sandbox uses (`halo_ce:elite`,
  `halo_ce:hunter`, `tf2:heavy`). None set yet: the campaign characters'
  health and shields are as the game states them, to be tuned here.

A hit by game A's weapon on game B's body counts: the switch off, 1; else
A's outgoing times B's incoming (or the body's own), A and B the same
game included (2026-11-17: one value per game against every game, no pair
exceptions; HL2:DM on HL2:DM 1.5, Halo on Halo 0.75 x 1.5 = 1.125, CS:S on
CS:S 1). It is
applied in one place, the engine's armour step (`weapon::HitArmor::
game_scale`), before armour: the pair's scale over the incoming the
victim's body holds. A Halo body holds its incoming as its mechanism (the
shield's armour unit and `core::DamageScale` on its health), so a hit
that takes armour in a game's own step (Combat Arms') counts it too; the
two together are the pair's scale, once. Halo's own hits on a Halo body do not take the engine's
armour step: Halo's seam asks the same `Balance::pair` itself
(`cross_game::own_scale`) and counts it in its own rule. `apps/party_roster/tests/halo_ce/balance_table.rs` checks
every pair with one gun of each game against the appendix's numbers.

**The switch** `mp_balance` (a host setting, "Cross-game balance"; 1 the
default, 0 a raw match): on the server it is put in force when a match
starts, so a change during a match applies from the next match; `status`
shows the value in force and a pending one. It is a replicated cvar, so a
client has the server's value with no message of its own (no network
version change); clients apply no damage. The ladder's power curve counts
the table's outgoing column whatever the switch (a ladder's order stays
the same in a raw match).

**What "raw" means** where no game has an answer: a gun deals its own
game's damage and a body takes it by its own game's rule; another game's
hit on a Halo shield is that game's damage as it deals it, taken by the
shield as Halo's armour (every part, the whole hit until the shield is
empty, the overflow to the body; a full shield is 100 of its points, the
Spartan's 75 shield points), with none of Halo's material multipliers; a
Halo gun on another game's body counts it as an unshielded Spartan's
(material type 21, a Halo point 1/75 of its health, 1.333 hit points).
The shield-leak fix (a character leaving Halo gets back the armour it
brought, never the shield's value) is behaviour, the same in both modes.

**How to tune.** Change a number in `party_balance.toml` (a rebuild:
the file is built in), then check it in the sandbox: `target add <body>`
(`halo_ce:spartan`, `halo_ce:elite`, `cs_source:default`, `tf2:heavy`),
shoot it with the gun in question, and read the kill card: its damage
per hit (`damage`, after every scale; `unscaled`, before the body's
incoming) and the hits to kill. Compare with `mp_balance 0` (from the
next match: `changelevel`) for each game's own numbers. Then run
`cargo test -p party_kit --features dev --test it balance_table` and fix
the expected numbers there (`today`) and in the appendix.

Known shortcuts: Combat Arms' guns take armour in their own step and read
no table (their outgoing and a non-Halo body's own
incoming against them count 1); a player's TF2 class is not looked up as
a body (its game's entry counts; the sandbox's targets are).

## The owner's decisions (2026-11-17)

- **HL2:DM against HL2:DM** keeps its 1.5 (now its outgoing value on every body, its own included).
- **DECIDED (2026-11-17): one value per game against every game, its own included,** with no permutation table and no pair exceptions. A hit's scale is the attacker's game's outgoing value times the victim's game's incoming value, the same game included, and Halo's own hits go through the table too. Done: `[[pair]]` removed, Halo's own hits go through the table. The owner's worry: two Halo players in a long duel get finished off by a third party. By the table, a Halo gun on a Halo body would be 0.75 × 1.5 = 1.125, a little quicker. 
