# Combat Arms: game modes, part 2

Source basis: the dedicated server's rule sets for Hired Guns, VIP Escort (public and clan), the four clan modes, Snowball Fight and the co-operative modes; the shared player bullion pocket, the squad (mercenary) manager, the bullion item, bank trigger and escape zone objects, the character damage and death path (freezing, AI damage scaling, kill credit for AI), the player's spawn and VIP teleport, the VIP data reader; the client's Hired Guns HUD, hire and order menus, Snowball respawn request, frozen-state controls and messages; the room data, hire-menu data, weapons and projectile data, character data and VIP data of a US-region data set; the plaintext per-map mission files of an older regional client; object lists read from plaintext maps of that client (Blood Money, Oil Rig, City of Wings); the regional build configurations that decide which variants are compiled.
Status: draft

Sections: Summary; Units and conventions; Constants (shared); M11 Hired Guns; M13 VIP Escort; M18 Clan variants; M16 Snowball Fight; M19 Co-operative modes; Per-tick order; Edge cases; Quirks; Test cases; Follow-up (2026-10-31b) (exit timer); Open questions.

This spec completes `game_modes.md` (called GM below) and uses its shared rules S1 to S14 and its phase names (setup, live, round over, round end, intermission). It also uses `missions.md` for the mission file fields and `movement.md` for the move penalty P. It does not repeat them.

## Summary

Hired Guns is a five-minute round of gold collection: 80 bars lie on the map, players carry them in a pocket that slows them down, and drop them as a bag when they die; a bar counts for the team only once its carrier walks into the team's own bank. Money earned by banking and killing can be spent on up to two AI mercenaries per player, which follow, hold or fight on their hirer's side. VIP Escort is a no-respawn round in which one escort player becomes a slow VIP with a pistol, starts locked in a cell, and must reach a helicopter zone that opens one minute into the round; the roles of the two teams swap every G/2 rounds, each swap followed by a fresh warm-up. The clan modes are the public rules with one half-time swap of team ids that carries each clan's score across. Snowball Fight freezes players instead of killing them; a frozen player can be shattered by an enemy shovel or freed by a teammate's. The co-operative modes give every player one life per mission.

## Units and conventions

- As GM: LithTech units (u), Y up, server frame 20 Hz, timers on server game time, tested once per frame. Team 1 = Alpha = `JSF`, team 2 = Bravo = `Creed`.
- G is the room goal (GM S2). R in VIP Escort is G (round wins needed).
- Money in Hired Guns is a plain integer ("bank money"); it has nothing to do with the account's GP.
- Box sizes ("dims") are half-extents, as everywhere in LithTech maps.
- "Build" names the regional executable. The US build and the BR, EU, KR and RU builds differ in Snowball Fight (M16) and in the VIP choice (M13); where this spec says "US build" it means the rules compiled for the US region.

## Constants (shared)

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| ai_damage_cap | 50 | HP | Most health one non-melee hit from any AI can take from a player |
| ai_damage_cap_big | 990 | HP | An AI hit of 1000 or more is set to this instead |
| ai_damage_factor_default | 0.8 | × | Console variable `AIDamageFactor` before a mode sets it |
| end_hold_vip | 3 | s | VIP Escort: message hold after a deciding event (VIP data `VIP_MISSION_END_TIME`) |

---

## M11. Hired Guns (`SQM`)

### M11.1 Overview

- Room: `SQM`, game type 19, G = rounds played (3..9 step 2, default 7), 6..16 players step 2, friendly fire offered, join in progress offered. The only US map with a Hired Guns mission file whose mode key is `SQM` is **Blood Money**. The other map the room data lists under Hired Guns (Hired Guns 2, shown as Death Row) loads a file whose key is `SQM2`, so it runs Squad Match 2 (missions.md Quirk 1); Squad Match 2 is not described here (Open questions).
- Skeleton: Elimination Pro's round skeleton (GM M2: warm-up, then rounds, setup frozen, round over, round end, intermission) with **respawn allowed** (GM S5) and **joiners spawned at once** (never held). Round length `TimeLimit` 300 s, warm-up `TrainingTimeLimit` 10 s (missions.md F4.5). The match lasts exactly G real rounds.
- Spawns: `GameStartPoint` with `GameType` `SquadMatch`, `Teams` `JSF` / `Creed` (Blood Money: 8 per team, all `Spawn` 1), chosen as GM S4.
- Each player has two numbers: a **pocket** (bars carried and their money value) and a **bank** (money). Teams have, per round, a **banked bar count**.

### M11.2 Map objects (Blood Money)

| Object (class, `Name`) | Properties that matter | Role |
|---|---|---|
| `ObjectSpawnPoint` × 84 | `GameType` `SquadMatch` | bar drop points (GM S12) |
| `BullionSaveTrigger` `SaveZone_A` | centre (1920, 96, −384), dims (128, 160, 128), `Team` `JSF`, `TriggerTouch` true, `CommandTouch` "msg activeplayer SAVEBULLION;", `NumberOfActivations` 0 (unlimited), `TriggerDelay` 0, `PlayerTriggerable` true | JSF bank, inside the JSF spawn area |
| `BullionSaveTrigger` `SaveZone_B` | centre (−1920, 96, −384), same dims, `Team` `Creed` | Creed bank |
| `BaseClass` `UNITSPAWN_ALPHA` | position (1951, 98, −772) | where Alpha mercenaries appear |
| `BaseClass` `UNITSPAWN_BRAVO` | position (−1942, 98, −772) | where Bravo mercenaries appear |
| `CAIHuman` `Squad_0` … `Squad_9` | `Template` true, placed off the playable area | mercenary templates (M11.6) |
| `ActivateTrigger`, `Trigger`, `TargetTrigger` (vault door, lock doors, way-points `WP_SaveZone_A`/`_B`) | map scripting | doors that open by use (2 s), HUD way-point markers over the banks |

A `BullionSaveTrigger` is a trigger box with an extra `Team` property: a touch from a player of another team (when `Team` is set), or from a player whose pocket holds no money, is ignored; any other touch runs the trigger normally, whose touch command sends the "save bullion" message to that player (M11.5). `Team` empty means every team may use it.

### M11.3 Bullion bars

- **When.** At the start of every real round (the mission-start step, before setup; never in the warm-up) the `ObjectSpawnPoint` list of the mode is shuffled and one bar is placed on each of the first N points, N = min(`DropBullionNum`, number of points) = min(80, 84) = **80**. All bars and bags left from the previous round were removed at its round over.
- **The bar.** A `BullionItem` with `Amount` = `BullionAmount` (100), a stack count of 1, the `Bullion` prop model, not rotating, `GameType` `SquadMatch` (a `BullionItem` whose `GameType` is not the running mode deletes itself), dropped to the floor. It is **picked up with the use key**, not by touch: the player aims at it (it is ray-hittable and "can activate") and presses use.
- **The bag.** What a death or a leave drops: a `BullionItem` holding the carrier's whole pocket (money and bar count), the `BullionBag` prop model, rotating, **picked up by touch** (not by use), solid. The client shows its bar count ("× n") when the local player aims at it, and on the large map.
- **Pick-up rule (both kinds).** Server side, on touch or use: the toucher must be a player, not leaving the game, the item not already taken, and the distance between the player's position and the item's position at most **200 u**. Then the whole content goes into the player's pocket at once (amount added to the pocket money, stack count added to the pocket bar count), the item is removed, and the item list shrinks by one. Anyone may take any item: an enemy's dropped bag, a teammate's, a level bar. Only players: AI never pick up.

### M11.4 The pocket

- **Carry limit: none.** A pocket holds any number of bars (at most 80 exist).
- **Weight.** After every pick-up the player's custom move penalty (the P_custom term of `movement.md`'s P) is set from the pocket's bar count n by the section table of missions.md F4.5. With the stock table:

```math
P_{bars}(n) = \begin{cases} 0 & n \le 10 \\ 0.03\,(n - 10) & 11 \le n \le 25 \\ 0.45 & 26 \le n \le 80 \\ 0 & n > 80 \end{cases}
```

  It is reset to 0 when the pocket is banked, dropped, or cleared at a round start. The server sends the value to the client, which uses it in the speed formula.
- **Warning.** When the local pocket count changes to a value of 11 or more, the client prints a red chat line saying the speed penalty is ((min(n, 25) − 10) × 3) % with n bars.
- **What a death drops.** In the live phase of a real round, when a player with pocket money > 0 dies (any cause), a bag with the whole pocket is created at the body's position raised by the body's half-height, then dropped to the floor; the pocket is emptied and the team's "in pockets" count falls by the bag's bars. A death outside a real round's live phase drops nothing (but the pocket is still cleared at the next round start). Respawning does not touch the pocket (it is already empty).
- **Leaving** with pocket money > 0 drops the same bag first (in any phase), then the player's mercenaries are removed.

### M11.5 Banks and banking

- **How.** Every server frame in which a player's box overlaps their own team's `BullionSaveTrigger` box while their pocket money is > 0, the bank fires: the whole pocket moves to the bank at once. **Time to bank: 0 s** (the first frame of overlap; no use key, no gauge). There is no limit on how often.
- **What it changes:** player bank money += pocket money; pocket emptied; move penalty 0; the team's banked bar count for the round += pocket bars; player mission points += pocket money; player statistics "bullion banked" (money and bars) += the same; every client is told (a "saved" event with that player's id: the HUD plays the ally or enemy banking sound and a short "transport" effect).
- **Early round end.** If after a banking the two teams' banked counts add up to the number of bars placed this round and no bar or bag object is left in the world, the round asks to end (GM S3 step 5 at the next mission update).
- A carrier cannot bank in the enemy's bank, and a carrier of the right team with an empty pocket does nothing.

### M11.6 Bank money, bonuses and hiring

The bank is each player's money for hiring mercenaries. It is **not reset between rounds**: payments add to what is left.

| Event | Bank change | Mission points change | Constant (Blood Money's mission file, `[BullionManage]`) |
|---|---|---|---|
| Start of each real round | + `BaseBankAmount`, plus `RoundWinBonusAmount` if the player's team won the previous round | – | 1000; 500 |
| Joining a live real round | + `BaseBankAmount` (at the joiner's first info update) | – | 1000 |
| Banking a pocket | + pocket money | + pocket money | `BullionAmount` 100 per placed bar |
| Kill of an enemy player (also by your mercenary) | + `KillBonusAmount` to the killer | + `KillBonusAmount` | 100 |
| Kill of an AI (any team the damage filter let you hurt) | + `KillBonusAmount` to the killer | + `KillBonusAmount` | 100 |
| Own death (any cause) | + `DeathBonusAmount` | + `DeathBonusAmount` | 0 |
| Team wins a round | – (paid at the next round start) | + `RoundWinBonusAmount`, at round over | 500 |
| Hiring a mercenary | − its cost | – | `SpawnCostAI0..9`: 200, 200, 200, 200, 500, 500, 500, 500, 500, 500 |

The numbers come from the map's mission file (missions.md F4.5); every missing field is 0. Kill and death bonuses and the round counters below apply only in the live phase of a real round. The bank never goes below 0: a hire that costs more than the bank is refused.

### M11.7 Mercenaries (the "hired guns")

- **What they are.** Not `bots.md`'s bots: they are the engine's AI humans spawned from the map's `CAIHuman` template objects, with the squad goal set. They use the same AI engine as `bots.md` (perception, attack, navigation on the map's `AIVolume`s) but different data: the `Squad` goal set of the multiplayer AI goal file (goals `AttackRangedDynamic`, `Follow`, `AttackTower`) and the brains `Squad_AR`, `Squad_SMG`, `Squad_MG` of the multiplayer AI attribute file (all three: ranged attack range 0..2000 u, grenade range 250..1000 u, grenade chance 0.25 every 5 s, no firing while moving).
- **Who.** Unit id i (0..9) uses the template named `AI<i>` in the mission file's `[AIRelations]`; the hire menu (hire-menu data, game type 19) lists them with keys 1..9, 0:

| Id | Template | Brain | Weapon (right hand), grenade | Cost | Menu name (US data), key |
|---|---|---|---|---|---|
| 0 | `Squad_1` | `Squad_AR` | `M4A1_NPC`, `M67_npc` | 200 | Heavy – Male, 1 |
| 1 | `Squad_2` | `Squad_AR` | `M4A1_NPC`, `M67_npc` | 200 | Heavy – Female, 2 |
| 2 | `Squad_3` | `Squad_SMG` | `MP5A4_NPC`, `M67_npc` | 200 | Light – Male, 3 |
| 3 | `Squad_4` | `Squad_SMG` | `MP5A4_NPC`, `M67_npc` | 200 | Light – Female, 4 |
| 4 | `Squad_5` | `Squad_SMG` | `P90_NPC`, `M67_npc` | 500 | Light – Mei, 5 |
| 5 | `Squad_6` | `Squad_SMG` | `P90_NPC`, `M67_npc` | 500 | Light – Deckland, 6 |
| 6 | `Squad_7` | `Squad_AR` | `M416_NPC`, `M67_npc` | 500 | Medium – Hana, 7 |
| 7 | `Squad_8` | `Squad_AR` | `M416_NPC`, `M67_npc` | 500 | Medium – Zadan, 8 |
| 8 | `Squad_9` | `Squad_MG` | `MG21E_NPC`, `M67_npc` | 500 | Heavy – Kamara, 9 |
| 9 | `Squad_0` | `Squad_MG` | `MG21E_NPC`, `M67_npc` | 500 | Heavy – Baron, 0 |

  Every template: `MaxHitPoints` 100, `MaxArmor` 100, box dims (24, 53, 24), `MoveToFloor` true, `BodyLifetime` 2 s (the body vanishes 2 s after death), `NPCType` `NPCDefault`. The menu's model and gear descriptions are cosmetic.
- **How many.** At most **2 living mercenaries per player** at a time. There is no team or match limit.
- **Hiring (server, on the client's hire request, any phase).** Refused, in this order, if: the player already has 2 living units; the unit id has no cost entry; the bank is smaller than the cost. Otherwise the cost is taken from the bank, then a unit is created from the template at the position and rotation of the object named `UNITSPAWN_ALPHA` (Alpha hirer) or `UNITSPAWN_BRAVO` (Bravo hirer); the hire is refused only now if there is not exactly one object of that name (the money is already spent, Quirks). The unit gets the hirer as its leader, the hirer's team and the unit id, and starts with the hirer's current order.
- **Team.** The hirer's team, for the damage filter (same-team damage between an AI and anyone of its team is always 0, whatever the friendly-fire setting), for its target choice, and for kill credit.
- **Orders** (one current order per hirer, applied to all their units; the client's order wheel: hold the order key, move the mouse left / up / right or press 1 / 2 / 3, release to send; 4 closes):
  - **Hold position (1):** each unit's follow point is set to where it stands now; the follow goal keeps its normal priority.
  - **Default (2):** no follow point; follow goal at normal priority (importance 3, below attacking).
  - **Follow me (3):** no follow point; the follow goal is raised to the highest priority, so units follow the hirer even when enemies are around.
  - The server also accepts an "attack-move to a point" order; the US client never sends it.
  - A unit that follows (no follow point) keeps within 200 u of its leader in the horizontal plane: it starts running toward the leader once the squared horizontal distance exceeds 200² + 1000 (about 202.5 u) for 0.25 s, and holds (aiming, walking pace) once inside 200 u.
- **Damage.** Mercenary damage to anyone is multiplied by `AIDamageFactor`, which a Hired Guns match sets from the objective's `AIDamageFactor` (1.0 on Blood Money). The per-pair AI-versus-AI table of `[AIRelations]` (all 1.0) would scale damage only to an AI that cannot be hurt by AI; the squad templates can, so it never applies. The health damage of one non-melee AI hit on a player is capped at 50 (990 if the hit was 1000 or more; ai_damage_cap). Players and mercenaries of the other team hurt mercenaries normally (armour first, as for players).
- **Death and respawn.** A mercenary never respawns. When it dies it leaves its hirer's unit list (so the hirer may hire again), and its body disappears after 2 s. **All of a player's mercenaries are removed** when the player leaves; **every AI in the world is removed** at each round over. A player's own death does not remove their mercenaries; they keep their last order and, with no leader alive, have nobody to follow.
- **Kill credit.** A player killed by a mercenary counts as killed by that mercenary's hirer (the kill feed marks it as a squad kill): the hirer gets the enemy kill, the kill bonus and a round kill for their team; a mercenary killing a player of its own team cannot happen (damage 0). A mercenary killed by a player gives that player an enemy kill, the kill bonus, a headshot statistic when hit in the head, and one round kill for their team; a mercenary killed by another AI or by the world gives nothing. A mercenary's death adds no death to anyone and no round death.
- **All mercenaries dead:** nothing happens; the round goes on.

### M11.8 Kills, round winner and match

Per real round, the rule set keeps, per team: banked bars, bars in pockets, round kills, round deaths; and per round: bars placed, level bars still on the ground, bars in bags on the ground. In the live phase of a real round:

| Death | Counted |
|---|---|
| Every player death | victim's team round deaths +1; victim deaths +1; death bonus; pocket dropped (M11.4) |
| Enemy kill (killer a player, or the hirer of the killing mercenary) | killer kills +1, kill bonus, killer's team round kills +1 |
| Suicide (own damage) | victim suicides +1 |
| Team kill | victim's death +1 is taken back; killer friendly kills +1 |
| No killer (fall) | only the first row |
| Enemy mercenary killed by a player | as "enemy kill" (no death for anyone) |

- **Round win:** nothing ends a round early except the early-end rule (M11.5) and leaving players (M11.9). At round over, if no winner was set: the team with more **banked bars** this round wins; else more round kills; else fewer round deaths; else no winner (nobody's round wins move, and nobody gets the round-win bonus).
- **At round over** (in this order): the time-out rule above; the winner's round wins +1 (not in the warm-up, whose winner is cleared); every player of the winning team gets mission points + `RoundWinBonusAmount`; every bar and bag is removed; every AI is removed. Then GM S3 round end, intermission, and the next round's bars and pay.
- **Match end:** after the intermission that follows real round G, or earlier when a team has no players. Winner: if a team is empty, the other team; else more round wins; else **Bravo** (GM Quirk 4).

### M11.9 Leaving players

Before the leaver is removed (GM S10): the leaver's mercenaries are removed and their pocket dropped as a bag (M11.4). Then: if the leaver was the last player of their team, the round asks to end (winner by the M11.8 rule) and, at the following match-end test, the other team wins the match. Else, if the leaver was alive and no other player of their team is alive at that moment (teammates dead and waiting to respawn), the other team wins the round at once (Quirks).

### M11.10 HUD and scoreboard

- **Score panel** (all from the server's mode message, sent at every round start, pick-up, drop and banking): Alpha and Bravo round wins (top and bottom rows); Alpha and Bravo **bars banked this round**; **bars in bags** on the ground; **level bars still on the ground** (this counter's HUD element is labelled "bank" in the layout, but it counts unclaimed placed bars).
- **Own status:** pocket bar count, two digits ("07"); bank money in the hire menu and the shop, with a short "+n" / "−n" animation when it changes.
- **Other players:** each player's pocket bar count is shown in their name marker and on the large map; bags are shown on the large map with their count.
- **Mercenary panel:** the local player's living units with health bars, and the current order icon; an order change plays a male or female voice line by the local character's sex.
- **Scoreboard:** per player kills, deaths, assists, ping and the mode column **mission points** (banked money + kill bonuses + death bonuses + round-win bonuses), sorted by mission points, then kills, then fewer deaths (GM S13). Team rows show round wins.

---

## M13. VIP Escort (`ESC`, clan `CESC`)

### M13.1 Overview

- Room `ESC`, game type 23, G = R = round wins needed (5..11 step 2, default 7), 4..16 players step 2, **no friendly-fire option** (the room data does not offer it, so friendly fire is always off), no enemy view camera, join in progress offered. US maps: Oil Rig, City of Wings, and a City of Wings variant with a different VIP (M13.4).
- Rounds: warm-up (`TrainingTimeLimit` 10 s, no VIP), then real rounds of `TimeLimit` 180 s, setup = the server file's `ObjectStart` (players frozen in real rounds only), no respawn, joiners held until the next round when 3 or more players are in the game and a round is live (GM S1).
- Roles: the **escort team** (starts as Alpha) and the **elimination team** (starts as Bravo). One escort player is the VIP.
- Spawns: `GameStartPoint` with `GameType` `VIPEscort` (Oil Rig: 11 JSF, 12 Creed), GM S4. Before the first swap Alpha uses the JSF points and Bravo the Creed points; while swapped, **Alpha uses the Creed points and Bravo the JSF points** (the map's JSF side is always the escort side). Spawn cursors reset to 0 at each setup after a swap.

### M13.2 Choosing the VIP

At the mission start of every real round (never in a warm-up, which has no VIP and clears the previous one), US build:

1. A list holds every player in the match (players are appended when they enter, removed when they leave). For each entry the server computes: on the escort team (yes/no); was the VIP of the previous round (yes/no); times VIP so far in this match (n); a score = VIP escapes + VIP kills + enemy kills + assists + sub-missions, all counted over the match; and a fresh random rank (a random permutation of 0..size−1, seeded from the clock).
2. The list is sorted by: escort team first; then not-previous-VIP first; then fewer times VIP; then higher score; then higher random rank.
3. The first entry becomes the VIP (times VIP +1). If it is not the previous VIP, the previous VIP loses VIP state (model and loadout restored).

So the VIP rotates through the escort team: nobody is VIP twice in a row unless they are the only escort player, and everyone gets a turn before anyone gets a second one; ties go to the better player, then to chance.

(Other builds: the list is the escort team sorted by experience, then by kill/death ratio, rebuilt at each side swap; the same VIP is kept while the escort team keeps winning; after a lost round the first entry is dropped and the next one becomes VIP.)

### M13.3 The VIP's spawn point

- The mission file's `[Mission0]` `VIPSpawn` names an object; stock: `VipTeleportPoint`, of class **`TeleportPoint`** with `MoveToFloor` true and `ChangeRot` true (Oil Rig at (−320, 120, −1408), City of Wings at (1256, 52, −2178)).
- Every time the VIP is spawned (the round setup's respawn of everyone), they are first placed at an escort spawn point as usual (S4, with its telefrag rule), then teleported to the first object of that name, taking its rotation, then dropped to the floor. If the name is empty or no object has it, the VIP stays at the normal spawn point.
- The stock point is inside a locked cell (`RotatingDoor` `VIP_Door`, 90° in 1 s). Two `ActivateTrigger`s open it, each locking both when used: `AT_VIP` next to the VIP, use time 13 s (set by the mission file's world-object override), and `AT_VIP_Door` on the outside, use time 1 s. Neither checks the team, so any player can open the cell. The round start trigger re-closes the door and unlocks both.

### M13.4 The VIP's loadout, health and limits

- **Model and loadout** from the mission file's `[VIPChangeCharInfo]` (missions.md F4.6). Stock: character model 87 (`VIP_Melissa`), weapon list primary **91** (G18 pistol), secondary 0, nothing else. The City of Wings variant file (US room data map record 66) uses model 289 (Hauser) with primary 209 (minigun) and throwing weapon 1255 (smoke grenade). The whole weapon list is replaced; the VIP cannot pick up weapons from the ground.
- **Health:** the model's values: model 87 has no hit-point entry, so the figure type's 100 HP; armour = the model's `MaxArmor` 100. The VIP data file's `VIP_HP` 300, `VIP_AP` 100, `VIP_STAMINA` 100 and `VIP_SPEED` 10 are read but used by no rule.
- **Movement:** the move penalty is the model's `MovePenalty` alone (model 87: 0.44), ignoring the weapon, gear and the custom term (movement.md calls this case "a VIP account"); sprint stamina is the figure's `Maxstamina` (100) with no gear bonuses. With movement.md's base run speed (285 u/s before the penalty for the reference figure), the VIP runs at 285 × (1 − 0.44) = 159.6 u/s.
- **Other limits:** a VIP cannot be vote-kicked; a VIP's death gives no assists (GM S7); the VIP's perk use is switched to the VIP set (perks outside this spec).
- **Losing VIP state:** at the next VIP choice (if someone else is chosen), at a warm-up, or when leaving. The model and weapons go back to the player's own.

### M13.5 The exit

- **Object:** class **`EscapeZoneTrigger`**, named by the objective's `EscapeZoneTrigger` field (stock `EscapeZoneTrigger`), `GameType` `VIPEscort`, box dims (128, 80, 128) (Oil Rig centre (−534, 606, 2242), City of Wings (320, 432, 704)), `Locked` true at load, `NumberOfActivations` 1.
- **Opening:** the zone is off until it receives an unlock message. The stock map's `TimerTrigger` `VipExitTimer` (named by the objective's `TimerTrigger`) is started by the round's start trigger and by the rule set at each real round's setup; its time is set by the mission file to `TimeAmount` **60 s**, with an "in time" command at 54 s (the helicopter starts to arrive) and, at 60 s, commands that show the helicopter, swap the zone markers and **unlock the zone**. The round's end trigger locks it again. Unlocking tells every player the exit is open (VIP, escort and elimination messages).
- **Escape detection** (server, every frame the zone is on and a touch is reported): the toucher must be a player, alive, the current VIP, on the escort team; the round must not be decided yet; and the round timer must be running and not yet expired. Then: escort team wins, VIP escapes +1, every living player is frozen and becomes immune to damage, the success message is shown for 3 s, and the map's zone commands run (helicopter leaves, camera).
- So with setup 0 s the exit opens at 120 s left on the 180 s round clock.

### M13.6 Round results

| Event (live phase, real round, no result yet) | Winner | Players frozen and immune | Hold before round over |
|---|---|---|---|
| VIP escapes | escort | yes | 3 s |
| VIP dies (any cause: enemy, own grenade, fall, world) | elimination | yes | 3 s |
| Last living elimination player dies | escort | yes | 3 s |
| Last living escort player dies (the VIP already dead, so the row above applies first) | elimination | yes | none |
| Death leaving both teams empty | elimination | yes | none |
| VIP leaves the game (alive) | elimination | yes | 3 s |
| Last living player of a team leaves (not the VIP) | the other team | no | 3 s |
| Round clock reaches 0 with no result | elimination | no | 3 s |

- The "hold" is a message timer (`end_hold_vip`); the round stays live during it, so kills during it still count as kills, but the winner cannot change.
- Kills: enemy kill → killer kills +1, victim deaths +1, killer's team round kills +1, plus VIP kill statistics when the victim is the VIP; suicide → victim deaths +1 and suicides +1; team kill → killer friendly kills +1, victim deaths unchanged; no killer → victim deaths +1. Assists as GM S7.
- Round wins: the winner +1 at round over (never in a warm-up).
- **Match end** (tested after each round's intermission, before any swap): a team has R round wins, or a team has no players. Winner: more round wins, else **Bravo**.

### M13.7 Side swap

- **When:** after real round k, if k is a multiple of T = ⌊R / 2⌋ and the match did not just end.

```math
T = \left\lfloor \frac{R}{2} \right\rfloor,\qquad \text{swap after rounds } T, 2T, 3T, \dots,\qquad \text{rounds played} \le 2R - 1
```

  R = 5: T = 2; R = 7: T = 3; R = 9: T = 4; R = 11: T = 5.
- **Timing** after the deciding event of round k: 3 s hold (if any), round over `ObjectEnd`, intermission `MissionEnd`, then **3 s** wait, then the swap itself with a **3 s notice** (every client gets the swap flag, both spawn areas' positions and the 3 s countdown), then a **new warm-up round** (10 s, no VIP, everyone revived every 5 s), then real round k + 1.
- **What swaps:** only the roles. The escort team becomes the elimination team and back; the spawn sides follow the roles (M13.1). Team ids do **not** change, so each team keeps its round wins, kills, deaths and scoreboard. Each swap also switches off any running satellite scan.
- **What resets:** the VIP (cleared by the warm-up, a new one chosen at round k + 1); spawn cursors; everyone is respawned with a fresh loadout at each setup anyway. The VIP rotation counters (times VIP, score) are kept across swaps (US build).
- **HUD:** at each round over the clients get "rounds until the swap": with m = k mod T (taken as T when 0), the value is T + 1 − m; after the last swap the game shows none (the value 255), the last swap being number ⌊(2R − 1) / T⌋. For R = 7 after rounds 1, 2, 3, 4: 3, 2, 1, 3.

### M13.8 Clan VIP Escort (`CESC`)

- Room data: clan mode `CESC`, game type 24, G 5..11 step 2, default 7, **not enabled** (`EnablePlay` false) in the US and older regional room data. Mission files exist for Oil Rig and City of Wings (same values as public).
- Rules as public VIP Escort except: the match ends when **the two teams' round wins add up to R** (a fixed R rounds, every round has a winner) or a team is empty; winner more wins, else Bravo; a clan that empties its team is recorded as having left. The swap period is the same T = ⌊R / 2⌋ (not R / 4).
- The reference does not support it fully: its spawn family is `ClanVIPEscort`, which no stock map has, so players fall back to the map's first `GameStartPoint` (GM S4), and the swapped spawn sides are not applied; the escape zone still reports escapes through its touch notification. Treat `CESC` as unshipped (Open questions).

---

## M18. Clan variants (`CDM`, `CMM`, `CCTF`, `CTSV`, and `CESC` in M13.8)

### M18.1 What every clan mode shares

- Clan rooms (`ClanMode` records of the room data): no friendly-fire option (always off), no join in progress, 8..16 players step 2, fixed goals: `CDM` 100 kills, `CMM` 6 rounds, `CCTF` 6 captures, `CTSV` 6 rounds. Enabled in the US data: `CDM`, `CMM`, `CCTF`, `CTSV` (also clan VIP Elimination `CVTDM`, not covered). Each team is one clan (the host clan starts as Alpha, the guest clan as Bravo).
- **Warm-up first:** every clan mode starts with a warm-up round (`TrainingTimeLimit` 10 s): nothing counts, no bombs, every dead player revived at once and then every 5 s.
- **One half-time swap of team ids.** At half time every player on team 1 is moved to team 2 and back; the clan identities swap with them, and the team counters that decide the match are swapped too, so **each clan keeps its own score**. The players are respawned and a **new warm-up round** (10 s) is played before the second half. The client gets a team-swap message with the new team counters. Since team 1 is always `JSF` (red, attacking in S&D, JSF spawn points and flag), the clans physically change ends and S&D attack/defence roles.
- **Leaving:** the last player of a team leaving ends the match at once for the other team (the leaving clan is recorded as having left).
- Statistics per player (kills, deaths, scoreboard) are match totals and are kept across the swap; per-round statistics are cleared.

### M18.2 Clan Elimination (`CDM`)

- TDM scoring (GM M1: every death adds 1 to the victim's enemy team, falls and suicides included), no time limit (the clan files have no `TimeLimit`), round setup 0 s, joiners spawn at once.
- **Half time:** the first death that brings either team's counter to ⌊G / 2⌋ = **50** or more ends the round (winner not set). After round over and intermission the swap happens (counters swapped with the clans), everyone is respawned, the warm-up (10 s) runs, then the second half starts with the carried scores. The half-time test is made once only.
- **Win:** the first death that brings a counter to G = 100 or more; the higher counter wins (a counter can reach 100 only by one death, so no tie). Match over after that round.

### M18.3 Clan Capture the Flag (`CCTF`)

- CTF rules (GM M4) with two halves of a flat `TimeLimit` **600 s** each (not 180 × G), round setup 0 s, warm-up 10 s before each half.
- **Half time:** the first capture that brings either team to ⌊G / 2⌋ = **3** captures, or the end of the first half's 600 s, ends the first half. Then: swap of team ids, and of the team counters for captures, returns, kills and deaths; every player is respawned (a carried flag is let go, GM S5); warm-up; second half of 600 s.
- **Win:** the first capture that brings a team to G = 6 ends the match; at the second half's time-out, more captures, else more returns, else more enemy kills, else fewer deaths, else draw.

### M18.4 Clan Search & Destroy (`CMM`) and clan Elimination Pro (`CTSV`)

- Public rules (GM M3, M2) with **exactly R = G = 6 real rounds**, no early match end on round wins.
- **Half time:** when the round counter reaches ⌊R / 2⌋ + 1 = **4** (that is, after real round 3) the swap happens before round 4, with a warm-up first. Round wins swap with the clans; in S&D the clan now on team 1 attacks (receives the bombs) from round 4 on.
- **Match result** after round 6, by clan: more round wins; else (S&D only) more "mission completions"; else more enemy kills (clan total over the match); else fewer deaths (clan total); else **draw** (no winner). A mission completion is credited, at a round over that was not a detonation or a defuse, to the clan currently on team 1, when a planter is on record (Open questions). The rule set has a "deuce" state for extra rounds, but the match end test runs first, so a tie after round 6 is always a draw.

### M18.5 Swap timing summary

| Mode | Trigger | Ends the round? | Notice / pause | Kept by clan | Reset |
|---|---|---|---|---|---|
| `CDM` | a counter ≥ 50 | yes (round over → intermission) | none beyond round over + intermission + a 10 s warm-up | kill counter, round wins | per-round stats; positions (respawn) |
| `CCTF` | captures ≥ 3 or 600 s | yes | same | captures, returns, kills, deaths | positions, carried flags, per-round stats |
| `CMM`, `CTSV` | after round 3 of 6 | (between rounds) | same | round wins, clan kill/death totals | as every round; bombs handed to the new team 1 |
| `ESC` (public) | after every T = ⌊R/2⌋ rounds | (between rounds) | 3 s wait + 3 s notice + warm-up | everything (team ids unchanged) | VIP, roles, spawn sides |

---

## M16. Snowball Fight (`SFM`)

### M16.1 Availability

- Room: `SFM`, game type 10, G = team tickets 30..90 step 20 (default 50), 2..16 players step 2, friendly fire offered, join in progress offered, perks off. **`EnablePlay` is false** in the US room data and in the older regional room data, so the lobby never opens it (a clan `CSFM` entry exists with no game type and is disabled too). Mission files: Snow Valley, Cold Seed, Freezer, Road Kill, Vertigo and the Snowball Fight map; the US room data names Snow Valley, Cold Seed and Freezer.
- **Two rule variants exist, chosen at build time:** the **ticket variant** in the US build, and the **recovery variant** in the BR, EU, KR and RU builds. No available data set makes either playable; a seasonal room-data change would be needed (Open questions).
- Common to both: one round, no warm-up, setup 0 s, `Team Death Match` spawn points, respawn allowed (server wait 1.0 s, GM S5), **no death drops** (inventory.md), every player's loadout replaced by the mission file's forced weapon list (missions.md F4.9).

### M16.2 Snowball weapons (US weapons data)

| Id | Weapon | Class | Clip / carried | Ammo damage | Projectile |
|---|---|---|---|---|---|
| 100 | `Snow_Ball` | snowball gun (14), right mouse = alt-fire | clip 20, selection 30, max 999 | instant 10 of type 47 (snowball) plus area 30 of type 47 in radius 100 u; range factors 1.0 / 0.8 / 0.5 at 1000 / 2000 / 2500 u | `Snow_Ball`: 1750 u/s (alt 2000), gravity −1000 u/s², lifetime 10 s; alt-fire projectile `Snow_Ball_Alt`: 1550 u/s, no gravity |
| 101 | `Ice_Ball` | snowball gun (14) | clip 1, selection 3, max 3 | no instant damage; on impact a lingering cloud of type 48 (ice-ball): 6 per tick for 2.5 s in radius 275 u, cloud lifetime 9 s | `Ice_Ball`: 1750 u/s, gravity −1000, lifetime 10 s |
| 102 | `Snow_Shovel` | melee (0), range 135 u | infinite | 1 of type 35 (crush-ice); knock-back value 2.0 | – |
| 103 | `Snow_Grenade` | throwing (7) | 1 | area 70 of type 8 (freeze) in radius 450 u | grenade, 1550 u/s (alt 3000, drop 500), fuse 2.5 s, gravity −1000 |

Move penalties: snowball guns 0.2, shovel and grenade 0.03 (movement.md P). Projectile and grenade physics as `weapon_firing.md` and `grenades.md`.

### M16.3 Freezing (server, in the damage step)

A player hit by damage from **another player** whose result would leave fewer than 1 HP is frozen instead of killed when the damage type freezes:

| Variant | Freezing damage types | Other types (bullets never exist here; falls, own grenades) |
|---|---|---|
| ticket (US) | freeze (8) only, i.e. the snow grenade; or any damage while already frozen | normal death (so a snowball or ice-ball "kill" is a real death) |
| recovery | freeze (8), snowball (47), ice-ball (48); or any damage while already frozen | normal death |

- **Becoming frozen:** HP set to 1.1; the player is frozen in place (movement locked), their animation stops, an ice render style replaces the body's, a "help me" voice and (ticket variant) a help effect above them; everyone is told. Damage that would bring a frozen player below 1 HP again sets HP back to 1.1. A frozen player counts as **alive** for every rule.
- **Freeze time:** ticket variant 10 s, recovery variant 15 s (shown on a client gauge).
- **Thawing by time:** recovery variant: after 15 s the player thaws with full maximum health restored and HP set to **25**. Ticket variant: the intended 10 s timer is tested in a way that is true on the very frame it starts, so the player thaws on the same frame they freeze, with maximum health **and** health set to 25 % of normal (25 of 100) until their next respawn (Quirks; Open questions).
- **Freed by a teammate:** crush-ice damage (the shovel) on a frozen player from a teammate thaws them: recovery: health 25 (maximum unchanged); ticket: maximum health and health 50 % of normal (50). The helper is healed 30 HP and gets one "unfreeze" statistic (stored in the bomb-detonation counter).
- **Shattered by an enemy:** crush-ice damage on a frozen player from an enemy kills them at once: an ice-shatter effect plays, no body is left. This is the snowball "kill".
- **Frozen player's state:** cannot move; ticket variant: the camera switches to third person; recovery variant: first person, mouse turn at 0.2 × the normal rate, pitch locked, cannot fire or switch weapons, a freeze gauge shows the time left. The client shows "you can respawn" to a frozen player, "unfreeze" when aiming at a frozen teammate within 100 u, "crush" when aiming at a frozen enemy within 100 u. The server accepts a respawn request from a frozen (not dead) player like one from a dead player; how the client sends it from the frozen state is not clear (Open questions).
- Damage to AI: freezing types between AI of the same team do nothing (no AI in stock maps).

### M16.4 Ticket variant (US build)

- **Tickets:** each team starts with G tickets (the HUD's team score). Accepting a respawn request (from a dead or frozen player) costs one ticket of the player's team; with 0 tickets the request is refused and the player stays as they are. The client does not even ask while its team's ticket count is 0. A frozen player leaving the game also costs their team a ticket.
- **Round time:**

```math
T = 60 \cdot \left\lfloor \frac{G}{3} + 0.999 \right\rfloor \ \text{s}\qquad (G = 30, 50, 70, 90 \Rightarrow 600, 1020, 1440, 1800\ \text{s})
```

- **Round end by a death:** after any death in the live phase, if a team has ≤ 0 tickets and nobody alive (frozen players are alive), the round ends: more tickets wins; else more living players; else no winner. A respawn does not run this test.
- **Time-out:** more tickets; else more living players; else draw. **Leaving:** the last player of a team leaving gives the other team the win. The match is this one round.
- Kills and deaths count as statistics only (enemy kill → kills +1; any death with a killer → deaths +1).

### M16.5 Recovery variant (BR, EU, KR, RU builds)

- **TDM-style score instead of tickets:** each team's counter starts at 0; every death in the live phase whose damager is a player (an enemy shatter, a teammate, the victim themselves) adds 1 to the victim's enemy team; a death with no player damager adds nothing. The first counter to reach G wins; there is **no round timer**. Respawn is free; the client asks after **1.5 s** (the server's 1.0 s limit, GM S5).
- **Assists:** freezing damage dealt to a player who is already frozen earns a "freeze" assist entry; when that player is shattered, the player who froze them first gets one assist.
- Leaving: as the ticket variant (last player of a team → the other team wins). Death counting as GM M1 rows "enemy kill", "team kill", "suicide".

---

## M19. Co-operative modes (`COOP`, `IFS`, `COOP_CF`)

What the reference settles for GM Open question 9 (mission scripting stays out of scope):

- **Difficulty → objective:** clamp(G, 0, 2) selects `[Object0]` (normal), `[Object1]` (hard) or `[Object2]` (extreme) of the mission file; its `TimeLimit` is the mission's time, its `AIDamageFactor` multiplies every AI hit, its `LifeCount` and `MaxLifeCount` are the lives. Stock: every objective `LifeCount` 1, `MaxLifeCount` 1; times and factors as missions.md "Stock values" (Desert Fox 1200 / 1080 / 900 s, factors 0.15 / 0.4 / 0.8).
- **Lives:** a player enters with `LifeCount` lives, the current life included. Each death spends one. A dead player respawns on request (after the GM S5 wait) only while they have a life left; with stock data (1 life) **a death is final** unless a respawn token item (a shop item, offered to the dead player) or a mission script adds a life. A script's "add n lives" adds them only if the result stays ≤ `MaxLifeCount`; otherwise that player's health and ammo are refilled instead.
- **Failure:** the mission fails when (a) the mission time runs out; (b) the sum of all players' remaining lives is 0: every client is offered the respawn token, and if nobody has used one **12 s** later the mission fails (0 s in the KR build); (c) a mission script reports failure. Success is a script event.
- **Joining** is never held; the room closes to joiners at the objective's `LevelCloseTime` (180 or 300 s).
- **AI damage:** every AI hit is multiplied by the objective's `AIDamageFactor` and then capped (ai_damage_cap).
- Scores, stages, bosses, spawners and the mission scripts are left to a co-op spec.

---

## Per-tick order

Server, one frame (refines GM "Per-tick order" for these modes):

1. **Client messages** as they arrive: fire (instant hits apply damage at once; a death runs the death path below), use (pick up a level bar; activate triggers such as the VIP cell), hire and order requests (Hired Guns), respawn requests (Snowball tickets are spent here).
2. **Objects update**: projectiles and grenades move and explode (more damage and deaths); touch notifications run: bag pick-ups, bank triggers, the escape zone; `TimerTrigger`s fire (the exit unlock); AI think and shoot.
   - Death path, in order: pocket dropped as a bag (Hired Guns); lives spent (co-op); the mode's counting (M11.8, M13.6, M16); assists; mercenary leaves its hirer's list (AI deaths); kill feed.
   - Damage path, in order: the damage filters (team, friendly fire, protection), the AI damage factor for AI damagers, armour, the AI cap, freezing (Snowball), then the death test.
3. **Mission update**: phase changes; the round timer test; the VIP Escort result hold; the co-op lives test.

Consequences:

- **A banking and a death on the same frame:** a death from an instant hit (step 1) comes first, so the bag drops and the bank sees an empty pocket. A death from a grenade explosion in step 2 may come before or after the bank touch depending on object order (Open questions).
- **The VIP escaping as the clock ends:** the escape test reads the round timer directly. If server time has reached the round's end, the escape is refused even though the mission update has not yet run, and the time-out gives the round to the elimination team. An escape on the frame before counts.
- **The VIP dying and escaping on the same frame:** a death by an instant hit (step 1) decides the round first; the escape in step 2 is refused (the round is decided).
- **The last bar banked and a kill on the same frame:** both count; the round ends at the mission update; the winner is still chosen by banked bars.

## Edge cases

- **Carrier dies inside their bank:** the bag drops inside the bank box (on the floor). The dead player cannot bank (empty pocket). Anyone may pick the bag up by touch; a teammate standing in the bank picks it up and banks it on the same or the next frame.
- **Carrier dies inside the enemy bank:** same; an enemy picks it up and banks it at once.
- **Bag on an unreachable spot** (fell out of the world): the bag is dropped to the floor below its creation point; if there is none it stays where created. The early round end then cannot happen (an item is left); the round runs to time.
- **All mercenaries dead:** no effect on the round.
- **Hirer dies:** their mercenaries stay, keep fighting, and stop following.
- **Hire with no unit spawn object:** money spent, no unit (Quirks).
- **Bank money between rounds:** kept; a player who hires nothing all match has 1000 × rounds + 500 × rounds won (+ bonuses).
- **Hiring during round over or intermission:** allowed (no phase check); the unit is not removed by the next round's start, only by that round's round over.
- **VIP leaves while dead, or after the round was decided:** nothing changes the result; the VIP list drops them.
- **VIP is the only escort player:** chosen every round.
- **Escort team empty at a round start:** the US sort then picks an elimination player as VIP (escort-first is only a sort key); the match ends at the next match-end test anyway.
- **Elimination player opens the VIP's cell:** allowed (the cell triggers check no team).
- **VIP Escort time-out hold:** during the 3 s, a VIP death restarts the 3 s message but cannot change the winner.
- **Clan half time reached by two deaths in one frame** (`CDM` 49 + 2): both count; the swap carries 51.
- **Clan leaving in the warm-up after the swap:** the other clan wins the match (leaving rule).
- **Snowball, frozen player leaves** (ticket variant): their team loses a ticket.
- **Snowball, team at 0 tickets with frozen players only:** frozen players are alive, so the round does not end until they are shattered (or thaw and die).
- **Co-op, last player alive on their last life:** the lives sum is 1, so the failure countdown starts only when they die.

## Quirks

1. **Hired Guns banks pay in, never reset.** The 1000 per round adds to the remaining bank, so money accumulates over the match. Keep.
2. **The round winner counts bars, not money.** A team that banks fewer, more valuable pockets cannot exist (all bars are worth 100), but kill bonuses do not help a team win a round except as the second tie-break. Keep.
3. **The Hired Guns "bank" HUD counter shows unclaimed placed bars.** Keep the numbers, label it as you like.
4. **A hire is paid before the spawn point is checked**, so on a map without the unit spawn object the money is lost. Keep (stock maps have the objects).
5. **Leaving can hand over a Hired Guns round** while the leaver's teammates are briefly dead waiting to respawn (Elimination Pro's leaving rule in a respawn mode). Keep.
6. **Mercenaries can be hired between rounds** and then survive the next round's start. Keep.
7. **The VIP escape test uses the raw clock**, so an escape on the last frame loses. Keep.
8. **After each VIP Escort swap a full warm-up is replayed.** Keep.
9. **Clan S&D and clan Elimination Pro cannot go to extra rounds;** a full tie after round 6 is a draw. Keep.
10. **Snowball ticket variant thaws instantly** (as coded) and leaves the player at 25 maximum HP until respawn; the recovery variant fixes both. Implement the recovery behaviour for freezing if the ticket variant is wanted for its tickets, or keep the as-coded behaviour deliberately; decide in review (Open questions).
11. **Snowballs kill instead of freezing in the ticket variant** (only the snow grenade freezes there). Keep together with Quirk 10's decision.
12. **Friendly fire and AI:** mercenaries ignore the friendly-fire setting: same-team AI damage is always 0, both ways. Keep.

## Corrections (2026-10-22): match flow

Checked against the reference's Hired Guns and VIP Escort rule sets and our runtime (`chimera/sim/src/mode/flow.rs`, `games/combat_arms/src/game/rules/`). The shared findings are GM's Corrections (2026-10-22); these are the parts that touch this spec.

1. **An empty team ends the match after a warm-up too** (GM Corrections 1).
   - Reference: VIP Escort and Hired Guns run their match-end test at the end of every intermission, the warm-up's included: a team with no players ends the match (VIP Escort: more round wins, else Bravo; Hired Guns: the non-empty team). For VIP Escort this includes the warm-up replayed after each side swap (M13.7).
   - Ours: `chimera/sim/src/mode/flow.rs:302` skips the match end after any warm-up, so a real round (VIP Escort 180 s, Hired Guns 300 s) is played with one team empty.
   - Expect: VIP Escort, the whole Bravo team leaves during the post-swap warm-up after round 3 (round wins 2–1) → the warm-up ends, then match over at the end of the following intermission, winner Alpha (2 > 1); round 4 never starts.

2. **VIP Escort's "3 or more players" counts the joiner** (GM Corrections 3).
   - Reference: the joiner is counted (with observers and game masters), the round must have been live for at least one whole second, never in a warm-up.
   - Ours: `games/combat_arms/src/game/rules/flow.rs:327-331` counts only the players already in, from the first live instant.
   - Expect: a real round live 20 s with 2 players in, a third joins → held until the next round's setup (M13.1). Joining 0.5 s after the round went live → spawns at once.

3. **Assist sums don't cross rounds** (GM Corrections 5). VIP Escort and the clan modes drop every damage sum after each real round, so damage dealt in round k never earns an assist in round k + 1. Ours keeps them (`games/combat_arms/src/game/rules/scoring.rs:177`, cleared only at a match reset). Expect: escort player A deals 60 to B in round 1, both survive; in round 2 C kills B → no assist for A.

Not determined: whether the clan modes (M18) also end the match on an empty team after their warm-ups; their own leaving rule (the match ends at once for the other clan) makes this unreachable in practice.

## Test cases

Common setup unless stated: server file client copy (`ObjectStart` 0, `ObjectEnd` 3 s, `MissionEnd` 3 s), 20 Hz, US room data, friendly fire off, spawn protection and respawn waits as GM.

**Hired Guns (Blood Money, G 3, 8 v 8)**

1. Round start. Warm-up ends. → 80 bars on 80 distinct `SquadMatch` drop points (of 84); HUD: banked 0 – 0, bags 0, unclaimed 80; every bank 1000; nobody holds bars. In the warm-up before it: no bars, banks 0, a hire request is refused (bank 0 < 200).
2. Level bar pick-up by use. Player 199 u from a bar presses use while aiming at it → pocket 1 bar, 100; unclaimed 79. At 201 u → nothing.
3. Bag pick-up by touch. A bag of 12 bars lies at (100, floor, 200); an enemy walks over it → their pocket +12 bars, +1200; HUD bags 12 → 0.
4. Weight. Pocket 10, 11, 20, 25, 30 bars → P_custom 0, 0.03, 0.30, 0.45, 0.45. Player with an M4A1 and the default vest (movement.md P = 0.2) carrying 25 bars → P = 0.65 → run speed 285 × 0.35 = 99.75 u/s. Chat warning at 11 bars: 3 %; at 30 bars: 45 %.
5. Banking. Alpha carrier with 7 bars (player box half-extents 15, 50, 15) walks toward `SaveZone_A` (centre (1920, 96, −384), dims (128, 160, 128)) along z = −384 at floor height → at centre x = 1780 the boxes overlap (|dx| = 140 < 143): banked on that frame: bank +700, mission points +700, Alpha banked 7, pocket 0, P_custom 0. At x = 1770 (|dx| = 150) → nothing yet. A Bravo carrier inside `SaveZone_A` → nothing.
6. Death drop. Alpha carrier with 12 bars killed by a Bravo rifle at (100, 0, 200), half-height 50 → bag created 50 u above the body position and dropped to the floor; HUD bags 12; Alpha "in pockets" −12; the Bravo killer's bank +100 and mission points +100; Bravo round kills +1; Alpha round deaths +1.
7. Death during round over. Same death 1 s after the round was decided → no bag, no counts.
8. Kill credit by mercenary. Alpha player A's `Squad_7` kills Bravo player B → A kills +1, A bank +100, A mission points +100, Alpha round kills +1; B deaths +1, Bravo round deaths +1.
9. Mercenary killed. Bravo player kills A's mercenary → Bravo player kills +1, bank +100, Bravo round kills +1; nobody's deaths change; A may hire again.
10. Hiring. A has 1000: hires unit 4 → bank 500, unit at `UNITSPAWN_ALPHA` (1951, 98, −772) on Alpha; hires unit 0 → 300; a third hire → refused (2 alive), bank stays 300. Bank 150, hire unit 0 → refused.
11. Mercenary damage. A mercenary rifle hit of 80 on a player (factor 1.0) → 50 health lost at most (after armour). A teammate's mercenary hitting a player → 0.
12. Follow. Order "follow me", hirer stands still 250 u away (horizontal) → the unit starts moving 0.25 s later; stops when within 200 u.
13. Bank money over rounds. Round 1: A spends 200, gets 3 kills (+300), banks 5 bars (+500): bank 1600 at round over; Alpha wins round 1. Round 2 start → A's bank 1600 + 1000 + 500 = 3100. Bravo player who did nothing → 1000 + 1000 = 2000. A's mission points after round 1: 300 + 500 + 500 (round win) = 1300.
14. Time-out winner. 300 s pass: banked 30 – 30, round kills 12 – 10 → Alpha. Banked 30 – 30, kills 10 – 10, round deaths 9 – 11 → Alpha. All equal → no winner, round wins unchanged, no round-win bonus.
15. Early end. Alpha banks the last carried bars so that banked = 50 + 30 = 80 and no item is left → round over at the next frame; winner Alpha (50 > 30).
16. Match end. G 3; round wins after 3 rounds 1 – 1 (one draw) → winner Bravo. A team empties after round 1 → match ends after that round's intermission; the other team wins.
17. Leaving carrier. A player with 4 bars and 2 mercenaries leaves → a bag of 4 at their position, both mercenaries removed. Last Alpha player leaves → round asks to end; match won by Bravo.
18. Same-frame bank and death. A carrier with 5 bars steps into their bank on the frame an enemy rifle kills them (fire message handled first) → bag of 5 dropped in the bank, nothing banked.
19. Scoreboard sort. Mission points 1300 / 1300 / 900 with kills 3 / 5 / 9 → order: the 5-kill player, the 3-kill player, the 900-point player.

**VIP Escort (Oil Rig, R 7, 5 v 5)**

20. VIP choice, US. Escort (Alpha) players P1 score 5, P2 score 9, P3 score 9 (P3 ranks lower by random), none VIP yet → round 1 VIP P2 (fewest times VIP, higher score than P1, P2 wins the random tie). Round 2 → P3 (P2 is the previous VIP; P1 and P3 have 0 turns; P3 has the higher score). Round 3 → P1. Round 4, after the swap, Bravo escorts: the Bravo player with the most score among those never VIP.
21. VIP spawn. Round setup → the VIP is at `VipTeleportPoint` (−320, 120, −1408) dropped to the floor, facing the point's yaw; the other escort players at `VIPEscort` JSF points; the elimination team at `VIPEscort` Creed points.
22. VIP loadout. → weapon list: weapon 91 only; 100 HP, 100 armour; P = 0.44 whatever they wear; run 159.6 u/s; picking up a ground weapon does nothing.
23. Cell. The VIP holds use on `AT_VIP` for 13 s → door opens, both triggers lock. An escort player outside uses `AT_VIP_Door` 1 s → same. Round restart → door closed, triggers unlocked.
24. Exit opens. Setup 0 s, round live at t = 0 → zone unlocked at t = 60 s (120 s on the clock); open messages sent. VIP enters the zone at t = 59.95 s → nothing. At t = 75 s → escort wins; everyone alive frozen and immune; round over at t = 78 s; round wins Alpha +1.
25. VIP killed. Elimination player kills the VIP at t = 100 s → elimination wins; freeze; round over at t = 103 s. Same by a fall → same winner.
26. Elimination wiped. Last elimination player dies at t = 90 s (VIP alive in the cell) → escort wins; round over at t = 93 s.
27. Time-out. Clock reaches 0 at t = 180 s, VIP alive → elimination wins, not frozen, round over at t = 183 s. A kill at t = 181 s counts as a kill; the winner stays.
28. Last frame. The VIP touches the open zone on the frame where server time first reaches the round end → refused; elimination wins by time-out. One frame earlier (50 ms) → escort wins.
29. VIP leaves alive at t = 30 s → elimination wins; round over at t = 33 s.
30. Swap timing, R 7 (T 3). Round 3 decided at t = 0 with an escape → hold to 3 s; round over 3 – 6 s; round end at 6 s; intermission 6 – 9 s; wait 9 – 12 s; swap message with a 3 s countdown at 12 s; warm-up live from 15 s (setup 0) to 25 s; its round over 25 – 28 s and intermission 28 – 31 s; round 4 live at about 31 s (plus a few frames). In round 4 Bravo escorts and spawns at the JSF points; Alpha's round wins are still Alpha's.
31. Swap period. R 5 → swaps after rounds 2, 4, 6, 8; R 9 → 4, 8, 12, 16; R 11 → 5, 10, 15, 20. A match with R 7 ends at most after round 13 (7 – 6).
32. Swap HUD. R 7, values sent at round over of rounds 1, 2, 3, 4 → 3, 2, 1, 3; after the 4th swap → 255.
33. Match end. R 7, Alpha reaches 7 round wins at round 11 → match over, Alpha wins, no swap after that round even if 11 were a multiple of T.
34. Join. A 9th player joins 20 s into a live real round → held until the next round's setup. Joins during the warm-up → spawns at once.
35. Clan VIP Escort (if enabled), R 7 → exactly 7 rounds; wins 4 – 3 → the team with 4; swaps after rounds 3 and 6.

**Clan modes (8 v 8)**

36. `CDM`, G 100. Clan X (Alpha) reaches 50 with Bravo (clan Y) at 41 → round over next frame (no winner), intermission; then clan X players are team 2 with 50, clan Y team 1 with 41; everyone respawned; 10 s warm-up (deaths count nothing); second half from 50 – 41. Clan Y (team 1) reaches 100 first → team 1 wins.
37. `CDM` half by a suicide: team 1 at 49, a team 2 player kills themselves → team 1 50 → half time.
38. `CCTF`, G 6. First half: team 1 reaches 3 captures at t = 400 s → half time; captures 3 – 1, returns 2 – 0 swap with the clans; second half 600 s. Second half time-out with captures 5 – 5 by clan, returns 4 – 4, kills 30 – 28 → the 30-kill clan wins.
39. `CCTF` first half time-out at 600 s with 2 – 1 → half time anyway; second half starts from 2 – 1 (by clan).
40. `CMM`, G 6. After round 3 (clan X attacking, wins 2 – 1 for X) → warm-up, clan X now team 2 (defending), still 2 wins; round 4 bombs go to clan Y on team 1. After round 6 wins 3 – 3, mission completions 2 – 1 → clan with 2 wins the match. All equal → draw.
41. `CTSV`, G 6. Wins 3 – 3 after round 6, clan kills 25 – 25, clan deaths 24 – 26 → the clan with 24 deaths wins.

**Snowball Fight (Snow Valley, 6 v 6)**

42. Loadout. Spawn → weapons 100, 101, 102, 103 only, with 30 snowballs (clip 20), 3 ice balls, the shovel, 1 snow grenade; no death drops.
43. Ticket timer. G 30 → 600 s; G 50 → 1020 s; G 70 → 1440 s; G 90 → 1800 s. Recovery variant → no timer.
44. Tickets. G 50, Alpha 1 ticket, two Alpha players dead request respawn → first accepted (Alpha 0), second refused; the client of a dead Alpha player does not ask while the HUD shows 0.
45. Ticket round end. Bravo 0 tickets, one Bravo player alive (not frozen) is killed → round over: Alpha wins (tickets 7 > 0). If that last Bravo player is frozen instead → round goes on.
46. Freeze, recovery. Player at 8 HP hit by a snowball (10) from an enemy → frozen at 1.1 HP; at t + 15 s thaws at 25 HP (max 100). A teammate's shovel at t + 5 s → thaws at 25 HP, the teammate heals 30 HP and gets one unfreeze statistic.
47. Shatter, recovery. Enemy shovel hits a frozen player → dies, no body; shoveler kills +1; victim's enemy team +1; the player who froze them gets one assist. First team to G = 50 → wins at once.
48. Freeze, ticket variant (as coded). Snow grenade blast of 70 on a player at 40 HP from an enemy → frozen and thawed on the same frame: health 25, maximum health 25. A snowball doing the last 10 HP → real death.
49. Snowball damage. Direct hit 10 + splash 30 within 100 u on the same target (at most 40 at the rate-1.0 range); ice ball cloud: 6 per tick for 2.5 s in 275 u.

**Co-operative (Desert Fox, 4 players)**

50. Difficulty. G 0, 1, 2, 5 → mission time 1200, 1080, 900, 900 s; AI damage × 0.15, 0.4, 0.8, 0.8; 1 life each.
51. Lives. A player dies (1 → 0 lives) → no respawn; the token offer is shown. The other three die → lives sum 0 → token offer to all → 12 s later mission failed (no one used a token).
52. Life bonus at maximum. A script adds 1 life to players with 1 life (max 1) → no life added; their health and ammo refilled.
53. AI cap. An AI hit computed at 120 health damage on a player → 50. At 1500 → 990. A melee AI hit of 120 → 120.

## Follow-up (2026-10-31b): the exit timer's 61 s

`world_objects.md` Follow-up (2026-10-31b) 1.1 traces the `TimerTrigger`: `START` sets its run-out to `TimeAmount` + 1 s, and `AutoStop` stops it, running its over-time commands, at the first update at or after that. So the stock `VipExitTimer` (60 s, early command at 54 s) unlocks the escape zone **61 s** after it was started, not 60 s; the helicopter's arrival commands still run at 54 s. Case 24 becomes: zone unlocked at t = 61 s (119 s on the clock); the VIP entering at t = 60.95 s → nothing.

54. **Exit at 61 s.** Round live at t = 0 with the exit timer started then (20 Hz): helicopter-coming effects at t = 54.0; the zone still locked at t = 60.95; unlocked and open messages sent at t = 61.0.

## Open questions

1. **Region of the mission values.** Hired Guns, VIP Escort, clan and Snowball mission values were read from the older regional client's plaintext mission files (the US files are encrypted, missions.md Open question 2); room data, weapons, VIP and hire-menu data are the US set. Check the Blood Money, Oil Rig and Snow Valley values in decrypted US or Steam data.
2. **Squad Match 2** (`SQM2`, Death Row, reached from the Hired Guns room on the Hired Guns 2 map): flag points, buy zones, turrets, a 1800 s round and a 15000..30000 point goal. Not described; it needs its own section.
3. **Snowball availability and variant.** No data set enables `SFM`. Which variant the live US service ran (if any) is unknown; the US build compiles the ticket variant, whose freeze timer thaws on the same frame (Quirk 10). Check an old event build, or decide in review to use the recovery variant's freezing.
4. **How a frozen player respawns.** The server accepts a respawn request from a frozen player and has an unused "give up" flag that kills them; the client's request path only runs for a dead player. Watch a frozen player in a recovery build: can they press a key to respawn, and does the ticket variant charge a ticket for it?
5. **Grenade death vs bank touch on one frame** (Per-tick order): depends on object update order; observe or choose "death first".
6. **AI cap order.** Whether the 50-HP cap applies to the health part only (after armour) or to the whole hit was read as "after armour"; check with a mercenary hit on an armoured player.
7. **Clan S&D mission completions:** the planter on record may carry over from an earlier round, and detonations themselves are not counted; observe a 3 – 3 clan S&D match.
8. **Carried and dropped flags at clan CTF half time:** respawn releases a carried flag; whether a dropped flag is sent home at half time was not traced.
9. **VIP perks:** the VIP's perk state switches to a VIP set; its effects belong to the perk spec.
10. **Hired Guns join pay:** a player who joins during a live real round gets 1000 at their first info update; a player who joins between rounds gets only the next round's pay. Confirm in play.
11. **Contradictions with `game_modes.md`:**
    - M11 says Hired Guns uses "Elimination Pro timing": rounds are 300 s (missions.md already notes this); also M11's "a death drops the pocket; kills and deaths pay bank bonuses" is right, but the bonuses are bank money for hiring mercenaries, which M11 does not mention.
    - M11 lists Squad Match 2 as having no room entry; the US room data reaches it (missions.md Quirk 1).
    - M13 says clan rooms swap "every R / 4" rounds; the clan rule set swaps every ⌊R / 2⌋ rounds like the public one, and ends after R total rounds.
    - M13 does not say that each swap is followed by a new warm-up, nor that team ids stay and only roles and spawn sides change.
    - M16 says the round ends "when a team has no tickets and nobody alive" (right for the ticket variant) and that the recovery variant "is not built": it is built into the BR, EU, KR and RU executables, only not the US one. M16 gives no time limit; the ticket variant has 60 × ⌊G/3 + 0.999⌋ s.
    - M18 says TDM and CTF clan halves end "when either team reaches half the goal": CTF's first half also ends at its own 600 s time-out; S&D and Elimination Pro swap at round R/2 + 1 as M18 says, and play exactly R rounds.
    - S1 lists VIP Escort among modes that "swap sides"; it swaps roles, not team ids (the clan modes swap team ids).
12. **Contradictions with `missions.md`:**
    - F4.5 and test 21 say `BaseBankAmount` is "each player's bank at every real round start"; it is **added** to the remaining bank (Quirk 1). Test 21's numbers hold for round 1 only.
    - "Timers per mode" gives Snowball Fight no timer; true for the recovery variant only (M16.4).
    - F4.6 says the VIP is moved to the teleport point "who spawns"; correct, and it happens after the normal spawn-point choice, so the telefrag of GM S4 can still happen at the escort point.
13. **Contradictions with `movement.md`:** its "VIP account" (penalty from the vest alone, stamina from the figure alone) is the VIP Escort VIP, and the "vest" value used is the VIP model's own `MovePenalty` (0.44 for model 87). Rename there after review.
14. **Contradictions with `bots.md`:** none in rules; its "Does the game have bots" table omits Hired Guns' mercenaries, which are AI humans spawned by players (M11.7), outside both bots.md's bots and its PvE section.
