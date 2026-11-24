# Team Fortress 2: batch-3 mechanics, part 1 (shield charge, banners, cloak, Spy knives, disguise)

Source basis: the retail client's item schema (`scripts/items/items_game.txt`,
read in place: each item's class, prefab chain and attributes, and each
attribute's class and description format), the English resource file's item
and attribute texts, the console variables the game lists, and the running
game, measured the same way as [batch2_mechanics.md](batch2_mechanics.md)
and [batch2b_mechanics.md](batch2b_mechanics.md): the Linux client as a
listen server on `itemtest`, muted, driven over RCON through VScript, each
unlock created as a live entity and equipped on the local player only (the
account's loadout untouched); puppet bots as targets, as a teammate and,
with `bot_forceattack`, as an attacker; `player_hurt`, `player_healed`,
`player_healonhit` and `player_death` read back with their game time;
position, velocity, conditions, max speed, health and the item meters
(charge, rage, cloak, hype, disguise state) sampled every server tick.
Random crits and damage spread were on unless a section says otherwise.
No game code was read. **(measured)** marks numbers read from the running
game on 2026-11-23; "fitted" marks a law chosen to reproduce the measured
values, not read anywhere; "(data)" marks values read from the item schema
or the resource file and not measured.
Status: draft

**Method limit (shields).** A shield is a wearable, and VScript has no way
to equip a wearable on a player: the shield entity could be created but
not attached to the player's wearables, so attack2 never started a charge.
The charge was therefore started by giving the player the charge condition
(`SHIELD_CHARGE`) directly, with the game's "shield equipped" flag set and
the shield's attributes put on the player as custom attributes (the same
attribute classes the shield would provide). Everything the condition does
by itself (speed, meter drain, turn limit, the crit-boost condition, the
end, the recharge, the attributes) was measured; what the shield entity
does (starting on attack2, the impact damage, the crit on the melee hit
after an impact, removing debuffs) did not happen in that set-up and is
under Open questions, with the item texts' description where one exists.

## Summary

Four team-flavoured kits. The Demoman's shields give a 1.5 s charge at
750 HU/s with a capped turn rate, a 12 s recharge and fire/blast
resistance; the variants change the turn cap, the recharge, the charge
length and what happens on damage and kills. The Soldier's banners fill a
rage meter from damage dealt (600 HP, the Concheror 480) and, blown, give
every teammate within 450 HU a 10 s buff: mini-crits, a 35 % damage cut
with crit immunity, or speed with heal on hit. The Spy's watches cloak in
1 s and decloak in 2 s, drain and refill a meter at per-watch rates, and
cut damage taken by 20 % while cloaked; the Dead Ringer instead arms a
fake death that cuts the triggering hit by 75 % and the next 3 s by a
falling 65–47 %. Disguises take 2 s (0.5 s when already disguised), copy
a real enemy's name and health, slow the Spy to the disguise class's
speed, and end when he fires.

## Units and conventions

Hammer units (HU), z up, seconds, HP. Server tick 66.67 Hz (0.015 s).
Meters (charge, rage, cloak, hype) run 0–100. Distances between players
are origin to origin on flat ground. Attribute values combine as
[item_attributes.md](item_attributes.md) §1. The speed-boost condition is
[batch2_mechanics.md](batch2_mechanics.md) §5.1 (× 1.4, at most +105).
Class speeds, health and the overheal decay are [classes.md](classes.md).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| charge_speed | 750 | HU/s | max speed while charging (× the melee weapon's move-speed attributes) |
| charge_time | 1.5 | s | full meter to empty (100 per 1.5 s, one point per tick) |
| charge_recharge | 12 | s | empty to full (8.33 per s) |
| charge_crit_below | 75 | meter | the crit-boost condition is on while the meter is under this |
| charge_crit_linger | 0.3 | s | the crit-boost condition after the charge ends |
| charge_turn_cap | ≈ 48 | degrees/s | turn limit while charging, base (at this client's frame rate) |
| rage_per_damage | 1/6 | meter per HP | Buff Banner, Battalion's Backup (600 HP fills) |
| rage_per_damage_conch | 1/4.8 | meter per HP | Concheror (480 HP fills) |
| banner_delay | ≈ 2.7 | s | press to buff (the horn) |
| rage_drain | 10 | per s | rage while the buff runs (10 s) |
| banner_radius | 450 | HU | teammates within it are buffed (449 in, 455 out) |
| banner_pulse | ≈ 0.5 / 1.2 | s | the buff is re-given about every 0.5 s and lasts about 1.2 s |
| backup_cut | 0.65 | × | damage taken under the Battalion's Backup; crits become normal |
| conch_speed | 1.4 | × | speed under the Concheror (240 → 336) |
| conch_heal | 0.35 | × damage | heal on hit under the Concheror |
| cloak_in / cloak_out | 1.0 / 2.0 | s | transition to invisible / back to visible |
| cloak_lock | 0.5 | s | after cloaking, before decloaking is allowed |
| cloak_drain | 10 | per s | Invis Watch while cloaked |
| cloak_regen | 3.33 | per s | Invis Watch while visible (30 s) |
| cloak_cut | 0.8 | × | damage taken while cloaked (any watch) |
| cad_drain_full | 15 | per s | Cloak and Dagger at full speed |
| cad_regen | 6.6 | per s | Cloak and Dagger visible, or cloaked and standing still |
| dr_regen | 5.0 | per s | Dead Ringer while visible |
| dr_drain | ≈ 7.1 | per s | Dead Ringer while cloaked after a feign (measured) |
| dr_feign_cost | 50 | meter | taken when the feign triggers |
| dr_feign_time | 3.0 | s | the feign's conditions (speed boost, afterburn immunity) |
| dr_first_cut | 0.75 | × reduction | the triggering hit |
| dr_feign_cut | 0.65 − t/16 | × reduction | hits t s after the trigger, while the feign lasts (fitted) |
| disguise_time | 2.0 | s | from undisguised |
| redisguise_time | 0.5 | s | while already disguised |
| disguise_wearoff | 0.5 | s | after firing, the disguise fades out |

## Behavior

### 1. Demoman shield charge (Chargin' Targe 131, Splendid Screen 406, Tide Turner 1099)

#### 1.1 Movement and meter **(measured, the charge condition)**

- While the charge condition is on, the Demoman's max speed is 750 and he
  accelerates to it with the normal ground acceleration (forward held: 280
  → 375.7 → 465.7 → 550.2 → 629.7 → 750 over five ticks). With no input he
  does not move, and the charge ended after 12 ticks; whether the game
  forces forward motion on a real charge was not measured.
- The melee weapon's move-speed attributes scale the charge speed: the
  Scotsman's Skullcutter (× 0.85) charged at 637.5. Eyelander heads do
  not raise it (750 with 4 heads, whose walking max was 369.6).
- The meter drains one point per tick: 100 to 0 in 1.5 s (1104 HU covered
  from a standstill). `charge time increased` adds to the 1.5 s: + 0.5 →
  2.0 s (50 per s), + 1.0 → 2.5 s (40 per s). The Claidheamh Mòr has + 0.5
  (data).
- **End.** The charge ends when the meter is empty, or when the Demoman
  is stopped (bumping a player or the world). On the end the meter is 0,
  and his speed drops at once to his walking max (the next tick's
  velocity is clamped to 280).
- **Recharge.** From the end the meter refills linearly: 8.33 per s (12 s).
  `charge recharge rate increased` multiplies it: the Splendid Screen's
  1.5 → 12.5 per s (8 s).
- **Crit-boost condition.** While charging with the meter under 75 (the
  first tick at 74, 25 ticks in) the Demoman carries the crit-boost
  condition `CRITBOOSTED_DEMO_CHARGE`; it stays 0.3 s after the charge
  ends. In the condition-started charge, melee hits during the charge did
  not crit and did not end the charge (Open questions).

#### 1.2 Turning **(measured)**

Turning while charging is limited. Keyboard turning at 200, 1000 and 3000
degrees/s gave the same cap, so it is a cap, not a scale:

| Turn control (attribute) | Cap (degrees/s) |
|---|---|
| none (Chargin' Targe, Splendid Screen) | 46.6–51.4 (≈ 48) |
| `mult charge turn control` 2 | 96.5 |
| `mult charge turn control` 3 (Ali Baba's Wee Booties, Bootlegger) | 136–151 |
| `full charge turn control` 1 | 151–161 |
| `full charge turn control` 2 | 267–269 |
| `full charge turn control` 5 | 565–585 |
| `full charge turn control` 50 (Tide Turner) | no cap seen (3000 passed) |

The "mult" attribute multiplies the base cap. The "full" attribute grows
faster than linearly in its value (fitted: none); the Tide Turner's 50 is
in effect free turning. The game lists `tf_demoman_charge_frametime_scaling`
(1, "scale yaw limiting based on client performance (frametime)"), so the
cap depends on the client's frame time; ours is the rate at this client's
frame rate.

#### 1.3 The three shields and the related items

| Item | Fire taken | Blast taken | Other (data unless measured) |
|---|---|---|---|
| Chargin' Targe (131) | × 0.5 | × 0.7 | — |
| Splendid Screen (406) | × 0.8 | × 0.8 | recharge × 1.5 (8 s, measured); impact damage × 1.7 |
| Tide Turner (1099) | × 0.85 | × 0.85 | full turn control (measured); damage taken while charging removes that much meter (30 → −30, 55 → −55, measured); melee kill refills 75 (measured: +75) |

- The resistances act on enemy damage only: with the Targe's attributes, a
  scripted enemy blast of 100 did 70, but the Demoman's own blast was not
  reduced by them **(measured, on a Soldier with the attributes)**.
- **Ali Baba's Wee Booties (405) / Bootlegger (608):** turn control × 3
  (measured above), max health + 25, walking speed × 1.10 with a shield
  (280 → 308, measured; it applied whether or not the shield flag was set
  in our runs), melee kills refill 25.
- **Kill refill** (`kill refills meter`): on a melee kill the meter gains
  value × 100 at once (0.75: 14.9 → 90.1, measured).
- **Eyelander and its re-skins:** max health − 25; heads do not change
  the charge speed (measured). Heads' effect on impact damage: Open.
- **Persian Persuader:** hits give 20 % meter, ammo gives meter (data).

### 2. Soldier banners (Buff Banner 129, Battalion's Backup 226, Concheror 354)

#### 2.1 Rage **(measured)**

- Damage the Soldier deals to enemies fills rage: 1/6 point per HP for the
  Buff Banner and the Battalion's Backup (a 103-damage rocket: + 17.196,
  i.e. the unrounded 103.18 / 6; 600 HP fills it), 1/4.8 per HP for the
  Concheror (+ 21.495; 480 HP).
- Damage taken gives none, the Battalion's Backup included (bot rockets of
  72 and 55, scripted hits of 30 and 50: rage stayed 0). The game's text
  for every banner says "Rage increases through damage done".
- At 100, attack with the banner out blows the horn; the buff starts about
  2.7 s after the press (2.715 from the first sample after it).
- The rage then drains 10 per s; the buff runs while it drains (10 s).

#### 2.2 The buff's reach **(measured)**

- The Soldier and every teammate whose origin is within 450 HU of his get
  the banner's condition (440, 445, 449 in; 455, 460 out). Line of sight was
  clear in every run.
- The condition is handed out in pulses: a teammate stepping in got it
  0.51 s later; one stepping out kept it 0.77–1.2 s. On the Soldier it
  lasts 11.2 s from the start (10 s of drain + about 1.2 s). Fitted: a
  pulse every 0.5 s giving 1.2 s.

#### 2.3 What each buff does

| Banner | Condition | Effect **(measured)** | Data |
|---|---|---|---|
| Buff Banner | `OFFENSEBUFF` | the buffed player's damage mini-crits (rocket 103 → 139, flagged mini-crit) | — |
| Battalion's Backup | `DEFENSEBUFF` | damage taken × 0.65 (scripted 100 bullet → 65, 100 blast → 46 with a 0.7 blast resist also on, self blast 100 → 65); a crit taken is a normal hit (crit 100 → 65; a bot's crit rocket of 243 → 50, its normal one of 72 → 50) | sentry damage × 0.5 (text); max health + 20 (220 measured) |
| Concheror | `REGENONDAMAGEBUFF` | max speed × 1.4 (Soldier 240 → 336, on teammates too); heal on hit 35 % of the damage dealt (103 → 36) | `health regen` 4: the holder regenerates up to 4 HP each second, ramping 1, 1, 1, 1, 2, 3, 3, 3 … per second after being hurt (measured) |

The bot rocket's 72 → 50 (0.69) under the Battalion's Backup is not 0.65:
the cut is applied at a step of the blast's damage that the rocket's
distance rules then change (Open questions).

### 3. Spy cloak (Invis Watch 30 and re-skins, Cloak and Dagger 60, Dead Ringer 59)

#### 3.1 Cloaking **(measured)**

- Attack2 with the watch: the cloak condition (`STEALTHED`) comes on that
  tick and the Spy is fully invisible 1.0 s later; he cannot decloak for
  0.5 s. Attack2 again: the condition goes that tick and he is fully
  visible 2.0 s later; he cannot attack until then.
- Speed is unchanged while cloaked (320), and stays the Spy's own while
  cloaked in a disguise (320 under a Pyro disguise whose walking speed was
  300).
- Damage taken while cloaked × 0.8 (100 bullet and 100 blast → 80, both
  the Invis Watch and the Cloak and Dagger).
- When the meter reaches 0 he decloaks (the 2.0 s fade).

#### 3.2 Meters **(measured)**

| Watch | Cloaked | Visible | Data |
|---|---|---|---|
| Invis Watch | − 10 per s, moving or not (10 s) | + 3.33 per s (30 s) | — |
| Cloak and Dagger | standing still: + 6.6 per s; moving: − 15 × (v / v_max)² per s (320 → 15, 106.7 → 1.67; fitted on intermediate speeds, one tick behind the speed) | + 6.6 per s | no cloak from ammo while cloaked; ammo gives × 0.65 |
| Dead Ringer | ≈ − 7.1 per s after a feign (48 → 0.9 in 6.6 s) | + 5.0 per s | consume × 0.6 and regen × 1.5 (data); no cloak from health or ammo |
| Your Eternal Reward (knife, with any watch) | Invis Watch drain × 1.33 (13.3 per s) | — | — |

#### 3.3 Dead Ringer feign **(measured)**

- Attack2 arms it, only with a full meter (60 and 40 refused).
- The next damage taken triggers it: a death event goes out (credited to
  the attacker), the Spy is invisible at once (no fade-in), the meter
  drops by 50, and for 3.0 s he carries the feign, speed-boost (425) and
  afterburn-immunity conditions; the cloak lasts until he decloaks or the
  meter runs out.
- Damage cut: the triggering hit × 0.25 (100 → 25; 50 → 13). Hits while
  the feign lasts are cut by 0.65 − t/16, t the time since the trigger
  (fitted; 100-damage hits gave 37, 38, 40, 41, 43, 44, 46, 47, 49, 50, 52,
  53 at t = 0.24 … 2.85 s). After the 3 s, while still cloaked, the
  ordinary 20 % (80).

### 4. Spy knives

- **Your Eternal Reward (225), Wanga Prick (574):** a backstab kill
  disguises the Spy as his victim about 0.2 s later (no 2 s wait): the
  victim's class, team and name, the victim's health (200 of 200) and
  weapon, and the victim's class speed (240). A normal disguise is refused
  unless the cloak meter is full and empties it (60: refused; 100 → 0).
  Cloak drain × 1.33. Backstabs are silent (data). **(measured)**
- **Big Earner (461):** a kill gives + 30 cloak at once and the
  speed-boost condition for 3.0 s (425); max health 100. **(measured)**
- **Spy-cicle (649):** the first fire hit is taken in full; then the knife
  melts (the Spy is switched to his next weapon), he is fire-immune for
  1.0 s (later fire hits do nothing) and afterburn-immune for 10 s. The
  knife can be drawn again 15 s after melting (on switching to it). A
  backstab freezes the victim's body into a statue (data, not measured).
  **(measured)**
- **Conniver's Kunai, Diamondback:** [batch2_mechanics.md](batch2_mechanics.md)
  §1.4, [batch2b_mechanics.md](batch2b_mechanics.md) §16.

### 5. Disguise basics (Disguise Kit, `disguise <class> <team>`)

**(measured)**

- From undisguised: the disguising condition for 2.0 s, then disguised.
  Changing disguise while disguised (either team): 0.5 s.
- What others see: the disguise class's body in the chosen team's colours,
  a real player of that team as the disguise target (his name), health
  copied from that player (150 when the target had 150; 300 for a Heavy
  disguise when the target's health was above it), and the class's stock
  primary (minigun, scattergun, flame thrower, rocket launcher).
- Speed: the slower of the Spy's and the disguise class's (Heavy 230,
  Soldier 240, Pyro 300; Scout stays 320).
- **What breaks it:** firing a weapon (the disguise is gone that tick, and
  a "wearing off" condition lasts 0.5 s). Taking damage does not, and the
  shown health did not change. Cloaking does not.

### 6. Batch 2's open questions answered

- **Dalokohs Bar:** the + 50 max health starts with the first heal step
  (1.05 s after eating) and ends 34.1 s after the eat (about 30 s after the
  4.25 s taunt). **(measured)**
- **Bonk! Atomic Punch:** during the phase condition the Scout takes no
  damage at all: enemy bullets, enemy blast and his own blast did nothing
  (no hurt event). Max speed stays 400. **(measured)**
- **Soda Popper hype:** 5 extra air jumps during hype (each sets the
  vertical speed to 262.3, as the normal air jump), a sixth press does
  nothing; landing resets the count. The jumps do not drain hype beyond its
  9.4 % per s. **(measured)**
- **Diamondback:** stored crits are capped at 35 (60 set by script became
  35 after the next backstab kill). **(measured)**
- **Bazaar Bargain:** a headshot kill adds a head (1 → 2; 6 → 7, so heads
  count past the rate's cap of 6); a body-shot kill and a non-lethal body
  hit change nothing. **(measured)**

## Per-tick order

1. Input: the charge's turn cap clamps the view's yaw change (client).
2. Meters: charge drain (one point per tick) or refill; cloak drain or
   refill (the Cloak and Dagger from the last tick's speed); rage drain;
   hype drain; Dalokohs and feign timers.
3. Movement with the charge's 750 max speed; a stop ends the charge
   (meter to 0, speed to walking max); a 0 meter ends it.
4. Damage events: cloak cut (× 0.8) or the feign cut; the Battalion's cut
   and crit removal; the Tide Turner's meter loss; a Dead Ringer trigger
   (death event, meter − 50, conditions).
5. After damage dealt: rage, Concheror heal, kill refills (charge meter,
   Big Earner cloak and speed boost, YER disguise, Diamondback, heads).
6. Banner pulse every 0.5 s to teammates within 450 HU.

## Edge cases

- A charge that is stopped early still empties the meter.
- The crit-boost condition only appears after a quarter of the charge.
- Disguise speed never exceeds the Spy's 320.
- The Dead Ringer needs a full meter to arm; feigning costs half of it.
- The Cloak and Dagger refills while cloaked if the Spy stands still.
- The shields' resistances do not cut the Demoman's own blasts.

## Quirks

- A cloaked Spy moves at his own speed even when disguised as a slower
  class.
- Banner buffs linger about a second after their rage is gone or after a
  teammate leaves the radius.
- Bazaar heads count past 6 although the charge rate stops growing there.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| Demoman, full meter | charge, forward | 1.5 s | ended, 1104 HU from a standstill, meter 0 |
| charging | — | 0.375 s | crit-boost condition on (meter < 75) |
| charge ended | — | 0.3 s | crit-boost condition off |
| charge ended | — | 12 s | meter full |
| Splendid Screen | charge ends | 8 s | meter full |
| Claidheamh Mòr | charge | — | 2.0 s |
| Skullcutter | charge | — | 637.5 max |
| Eyelander 4 heads | charge | — | 750 max |
| charging, Targe | turn right fast | 1 s | ≈ 48° |
| charging, Booties | turn right fast | 1 s | ≈ 3 × that |
| charging, Tide Turner | turn | — | uncapped |
| Tide Turner, charging, meter 79 | take 55 | — | meter 23 |
| Tide Turner, meter 15 | melee kill | — | meter 90 |
| Buff Banner | deal 103 | — | rage + 17.2 |
| Concheror | deal 103 | — | rage + 21.5 |
| Battalion's Backup | take 72 | — | rage + 0 |
| rage 100 | blow | 2.7 s | buff on Soldier and teammates ≤ 450 HU |
| buff on | — | 10 s | rage 0; buff gone ≈ 1.2 s later |
| teammate at 455 | — | — | not buffed |
| Buff Banner buff | rocket 103 | — | 139 mini-crit |
| Battalion's buff | take 100 / crit 100 | — | 65 / 65 |
| Concheror buff | deal 103 | — | heal 36; speed 336 |
| Invis Watch | cloak | 1.0 s | fully invisible |
| cloaked | decloak | 2.0 s | fully visible, can attack |
| Invis Watch cloaked | 10 s | — | meter 0, forced decloak |
| Cloak and Dagger cloaked, still | — | 1 s | meter + 6.6 |
| Cloak and Dagger cloaked, 320 HU/s | — | 1 s | meter − 15 |
| cloaked | take 100 | — | 80 |
| Dead Ringer armed, 125 HP | take 100 | — | 25 taken, death event, meter 50, speed 425 for 3 s |
| feign 1.2 s old | take 100 | — | 43 |
| feign over, cloaked | take 100 | — | 80 |
| Dead Ringer, meter 60 | attack2 | — | not armed |
| YER | backstab kill on a Soldier | 0.2 s | disguised as him, speed 240 |
| YER, meter 60 | disguise | — | refused |
| Big Earner, meter 20 | kill | — | meter 50, speed 425 for 3 s |
| Spy-cicle | fire hit 10 | — | 10 taken; knife melted 15 s; fire-immune 1 s; afterburn-immune 10 s |
| undisguised | disguise as Heavy | 2.0 s | disguised; speed 230 |
| disguised | disguise again | 0.5 s | new disguise |
| disguised | fire | — | disguise gone, wearing-off 0.5 s |
| disguised | take damage / cloak | — | still disguised |
| Dalokohs | eat | 1.05 s / 34.1 s | max 350 / back to 300 |
| Bonk phase | any damage | — | none |
| Soda Popper hype | jump 7 times in the air | — | 5 air jumps |
| Diamondback, 35 stored | backstab kill | — | 35 |
| Bazaar, 1 head | headshot kill | — | 2 heads |

## Open questions

- **The shield entity's own behaviour** (answered by measurement on a bot
  given a real shield in [batch3b_mechanics.md](batch3b_mechanics.md) §1): attack2
  starting the charge (a full meter needed?), the impact on bumping an
  enemy (damage, and its scaling with distance charged, heads and the
  Splendid Screen's × 1.7), the melee crit or mini-crit after the impact
  (the item texts: the Splendid Screen "Gain a critical melee strike after
  impacting an enemy at distance", the Tide Turner "Gain a mini-crit melee
  strike after impacting an enemy at distance"; the Targe's crit-boost
  condition below 75 % suggests a full crit there), whether a melee swing
  ends the charge, and the debuff removal on starting ("remove debuffs",
  both texts). Measuring these needs a Demoman whose loadout holds a
  shield (a second account or a bot given one by a map script).
- The turn cap's law in frame time and the "full" attribute's curve.
- The Battalion's cut on blast damage (72 → 50 is 0.69, not 0.65); its
  sentry cut (text only).
- The Dead Ringer's drain (7.1 per s measured against 6 from the data's
  × 0.6) and whether it differs before the feign ends; cloak from ammo.
- Disguise health for a class whose target is not chosen (no player of
  that class on the team), and what a Spy disguised as his own team shows.
- Sappers (Sapper, Red-Tape Recorder, Ap-Sap, Snack Attack): need an
  Engineer's building; not run.
- Bonk: any slow after the phase; Soda Popper: the sixth jump with a
  low ceiling (the fifth jump's run met the map's ceiling).
