# Halo: Combat Evolved: multiplayer rules

Source basis: the Xbox disc's own files (the game's executable, whose
code builds the 26 stock game types; the interface map `ui.map`, which
names them and lists every option the setup menus offer; the 13
multiplayer maps' scenarios and the globals tag they all carry), read
with throwaway scripts; and a decompilation of an Xbox build of the
game's engine (a different build from the disc's, NTSC against PAL),
used for behaviour. The disc's numbers win wherever both speak.
Status: draft

Sections: Summary; Units and conventions; Constants; 1 The game type
definition; 2 The stock game types; 3 Teams; 4 Spawning; 5 Pickups; 6
Scoring and the end of a game; 7 The radar; 8 Vehicles; 9 The
announcer; 10 Per-tick order; 11 Measured facts; 12 Checks run; Edge
cases; Quirks; Test cases; Confidence per section; Open questions.

---

## Summary

- A multiplayer match is one scenario plus one **game variant**: a
  0x68-byte record naming a base game type (capture the flag, slayer,
  oddball, king of the hill, race) and carrying about 25 settings. The
  executable builds 26 stock variants in code; a saved variant is the
  same record in a 512-byte file.
- Spawning is data plus a small rating formula. Every player start
  names up to four game types; a candidate is rated by its distance to
  the other living players (nothing nearer than 2 WU to an enemy, and
  an enemy 2 to 5 WU away scales the rating down linearly), multiplied
  in team games by a bonus for living teammates 1 to 6 WU away, and
  multiplied by the square root of a fresh random number; the best
  rating wins. There is no line of sight test, no "recent death" test
  and no spawn protection. A dead player waits at least 3 s (90 ticks),
  and after the first 3 ticks of a match a spawn can only happen on the
  tick whose number is congruent to the player's slot modulo 32.
- What a player holds on spawning comes from the unit tag (assault
  rifle then pistol, 4 fragmentation grenades) unless the variant says
  "custom" starting equipment, in which case the scenario's starting
  equipment entry for the game type replaces the weapons.
- Items lie where the scenario's netgame equipment puts them. An entry
  creates a new item whenever the game clock is an exact multiple of
  its period, **whether or not the last one was taken**; an item nobody
  holds is removed 30 s (900 ticks) after it was last held, which for
  a map item is arranged to be the tick after its replacement appears.
- Slayer scores +1 per kill and -1 per suicide or betrayal; the other
  game types score through the flag, ball, hill or race checkpoints.
  Announcer lines (double kill, killing spree, running riot...) come
  from a 4 s multi-kill window and a counter of kills since the last
  death.

## Units and conventions

All of `cache_file.md` applies: 30 ticks per second (a tick is 1/30 s),
world unit WU = 3.048 m, +Z up, angles in radians. Specific to this
spec:

- Every time stored in a game variant is in **ticks**, not seconds
  (respawn time 300 is 10 s). Times in the scenario's netgame
  equipment and item collections are whole seconds. Scores, lives and
  limits are plain counts, except where a game type says "minutes".
- "Slot" means a player's index in the game's player table (0 to 15 on
  the Xbox), which is also what the free-for-all team number is.
- "Disc" in a statement means it was read from the disc's files or
  decoded from the executable; "reference" means read from the
  decompilation only; "inferred" means neither shows it directly.
- Tag paths are shown as the disc stores them, backslashes included
  for tags and written as slashes in tables of paths.
- Rates: speeds are WU per tick unless a "/s" says otherwise.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| TICKS_PER_SECOND | 30 | ticks/s | `cache_file.md` |
| VARIANT_SIZE | 0x68 | bytes | a game variant record (disc) |
| VARIANT_FILE_SIZE | 512 | bytes | a saved variant's file: the record, then a 20-byte platform signature at 0x68 over the first 104 bytes (reference) |
| VARIANT_NAME_CHARS | 11 | UTF-16 units | the name's capacity; the 12th unit is a zero |
| STOCK_VARIANTS | 26 | count | built by the executable, in the order of section 2 (disc) |
| MIN_RESPAWN_TICKS | 90 | ticks | no death waits less than this (reference) |
| SPAWN_STAGGER | 32 | ticks | a spawn happens only when the tick number mod 32 equals the slot mod 32 (reference) |
| SPAWN_STAGGER_FREE_UNTIL | 3 | ticks | up to this game time the stagger does not apply (reference) |
| SPAWN_RANDOM_EXPONENT | 0.5 | | candidate rating is multiplied by u^0.5, u uniform in [0, 1) (reference) |
| SPAWN_ENEMY_BLOCK | 2.0 | WU | an enemy this near makes a start unusable (reference) |
| SPAWN_ENEMY_FALLOFF_END | 5.0 | WU | beyond this an enemy no longer matters (reference) |
| SPAWN_TOUCH | 0.25 | WU | a player this near makes a start unusable (reference) |
| SPAWN_CROWD | 1.0 | WU | a player nearer than this (and beyond SPAWN_TOUCH) scales the rating by 0.1 (reference) |
| SPAWN_VEHICLE_RADIUS | 0.1 | WU | a vehicle within this of a start blocks it (reference) |
| FRIEND_BONUS_NEAR, FAR | 1.0, 6.0 | WU | teammates in this range raise a start's rating (reference) |
| FRIEND_BONUS_SLOPE | 0.2 | per WU | falloff inside that range |
| FRIEND_BONUS_EXPONENT | 0.6 | | |
| FRIEND_BONUS_CAP | 3.0 | | cap on the sum before scaling: bonus = 3 x sum + 1, 1 to 10 |
| ITEM_DEFAULT_PERIOD | 30 | s | item period when neither entry nor collection names one (reference; `scenario.md`) |
| ITEM_UNOWNED_LIFE | 900 | ticks | an unheld item older than this is deleted (reference) |
| CORPSE_LIFE | 900 | ticks | a dead biped lying idle this long is deleted (reference) |
| GRENADE_MAX | 4 frag, 4 plasma | count | globals, `maximum count` (disc) |
| GRENADE_MP_DEFAULT | 0, 0 | count | globals, `mp spawn default` of each (disc); not read by the reference |
| STARTING_GRENADES_5PLUS | 2 | count | cap on starting grenades of each kind once the game began with 5 or more players (reference) |
| STARTING_GRENADES_9PLUS | 1 | count | the same with 9 or more (reference) |
| GRENADE_ITEM_KEEP_5PLUS | 0.55 | probability | chance a spawned grenade item is kept in such a game (reference) |
| GRENADE_ITEM_KEEP_9PLUS | 0.30 | probability | the same with 9 or more players (reference) |
| POWERUP_CAMO_TIME | 45 | s | active camouflage, tag `powerups\active camouflage` (disc, `objects.md`) |
| POWERUP_OVERSHIELD_TIME | 60 | s | tag `powerups\over shield` (disc) |
| MULTIKILL_WINDOW | 120 | ticks | 4 s (reference) |
| SPREE_FIRST | 5 | kills | killing spree at exactly this many kills in a row; running riot at every other multiple of 5 (reference) |
| POSTGAME_DELAY | 7 | s | from the end of a game until everyone is killed and the results show (reference) |
| RADAR_RANGE | 20 | WU | motion sensor range, `hudg` tag `ui\hud\default` (disc) |
| RADAR_VELOCITY_SQ | 0.0002 | WU^2/tick^2 | squared speed a moving unit must reach (disc) |
| RADAR_MAX_BLIPS | 16 | count | per local player (reference) |
| ANNOUNCER_START_DELAY | 60 | ticks | first announcer line waits this long after the game starts (reference) |
| ANNOUNCER_QUEUE_MAX | 5 | count | queued lines; further ones are dropped (reference) |
| ANNOUNCER_GAP | 5 | ticks | added to a line's length when it is queued (reference) |
| CTF_FLAG_RETURN | 17 | s | an unheld flag away from home returns after more than this (reference) |
| CTF_CAPTURE_RADIUS | 1.0 | WU | carrier within this (3D, strict) of its own flag spot scores (reference) |
| CTF_WARNING_PERIOD | 20 | s | announcer repeats "has the flag" while a flag is taken (reference) |
| CTF_FAILURE_PERIOD | 4 | s | at most one "flag failure" sound per this (reference) |
| ODDBALL_START_BALL | 15 | s | ball n appears 15 s x (n + 1) after the start (reference) |
| ODDBALL_RESET | 40 | s | an unheld ball away from its spot resets after more than this (reference) |
| ODDBALL_MAX_BALLS | 16 | count | (reference; `scenario.md`) |
| KING_MOVE_PERIOD | 60 | s | a moving hill moves this often (reference) |
| KING_CONTROL_TIME | 10 | s | a hill held this long uncontested makes "contested" audible (reference) |
| HILL_BOTTOM, HILL_TOP | -0.1, +0.8 | WU | hill volume against the lowest and highest flag (reference; `scenario.md`) |
| RACE_MAX_FLAGS | 32 | count | checkpoints (reference) |
| RACE_MAX_VEHICLES | 8 | count | vehicles a race creates (reference) |
| TELEFRAG_TICKS | 90 | ticks | a player standing in a teleporter's exit this long is killed (reference) |

## 1. The game type definition

### 1.1 Saved layout (disc)

A game variant is 0x68 bytes, little-endian. The layout was checked by
decoding the 26 stock builders in the executable (every field they
write lands at the offset below) and against the setup menus' handlers,
which write the same offsets. Booleans are one byte.

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0x00 | 24 | name | 12 UTF-16 units, the last always zero |
| 0x18 | 4 | base game type | 1 capture the flag, 2 slayer, 3 oddball, 4 king of the hill, 5 race (0, 6 and 7 never occur in a game) |
| 0x1C | 1 | team play | 1 teams, 0 free for all (capture the flag forces 1) |
| 0x1D | 3 | pad | |
| 0x20 | 4 | flags | section 1.3 |
| 0x24 | 4 | objective indicator | 0 motion tracker, 1 nav points, 2 none |
| 0x28 | 1 | odd man out | |
| 0x29 | 3 | pad | |
| 0x2C | 4 | respawn time growth | ticks added per death, section 4.2 |
| 0x30 | 4 | respawn time | ticks |
| 0x34 | 4 | suicide penalty | ticks |
| 0x38 | 4 | number of lives | 0 is infinite |
| 0x3C | 4 | health | f32 multiplier, 1.0 is 100 percent |
| 0x40 | 4 | score to win | meaning per game type (section 6) |
| 0x44 | 4 | weapon set | 1.2 |
| 0x48 | 4 | vehicle set | 1.2 |
| 0x4C | 0x18 | game type options | union, below |
| 0x64 | 2 | variant flags | bit 0: system default (set on every stock variant) |
| 0x66 | 2 | pad | |

Game type options (offsets from the file's start; the union is 0x18
bytes, only the used part is listed):

| Base type | Offset | Size | Field |
|---|---|---|---|
| capture the flag | 0x4C | 1 | assault |
| | 0x4D | 1 | reset on capture (never read) |
| | 0x4E | 1 | flag must reset |
| | 0x4F | 1 | flag at home to score |
| | 0x50 | 4 | single flag time, ticks (0 is off) |
| slayer | 0x4C | 1 | no death bonus (set means the bonus is off) |
| | 0x4D | 1 | no kill penalty (set means the penalty is off) |
| | 0x4E | 1 | kill in order |
| oddball | 0x4C | 1 | random start |
| | 0x4D | 1 | ball spawn delay (never read) |
| | 0x50 | 4 | speed with ball: 0 slow, 1 normal, 2 fast |
| | 0x54 | 4 | trait with ball |
| | 0x58 | 4 | trait without ball |
| | 0x5C | 4 | ball type: 0 normal, 1 reverse tag, 2 juggernaut |
| | 0x60 | 4 | ball spawn count, 1 to 16 |
| king of the hill | 0x4C | 1 | moving hill |
| race | 0x4C | 4 | race type: 0 normal, 1 any order, 2 rally |
| | 0x50 | 4 | team scoring: 0 minimum, 1 maximum, 2 sum |

Traits (oddball): 0 none, 1 invisible, 2 extra damage, 3 damage
resistant.

### 1.2 Enumerations

| Field | Values |
|---|---|
| objective indicator | 0 motion tracker, 1 nav points, 2 none |
| weapon set | 0 normal, 1 pistols, 2 rifles (the menu says "rifles"; the assault rifle set), 3 plasma weapons, 4 sniper, 5 no sniping, 6 rocket launchers, 7 shotguns, 8 short range, 9 human, 10 no grenades; 11 covenant, 12 classic, 13 heavy are the PC game's (reference) |
| vehicle set | 0 default (all three), 1 none, 2 warthog, 3 ghost, 4 scorpion ("tank") |

The disc's setup menu offers weapon sets 0 to 9 and vehicle sets 0 to
4; set 10 (no grenades) is reachable only from a saved file or the PC
menus. Every stock variant uses weapon set 0, 4 or 6 and vehicle set 1,
2 or 4 (disc).

### 1.3 Flag bits (0x20)

| Bit | Value | Meaning (menu label) |
|---|---|---|
| 0 | 1 | other players show on the radar (OTHER PLAYERS ON RADAR) |
| 1 | 2 | friend indicators on screen (FRIEND INDICATORS ON SCREEN) |
| 2 | 4 | infinite grenades |
| 3 | 8 | no shields (the menu's SHIELDS NO sets it) |
| 4 | 0x10 | invisible players (every player is active-camouflaged) |
| 5 | 0x20 | generic starting equipment (STARTING EQUIPMENT GENERIC; clear is CUSTOM) |

### 1.4 What the game does with a variant it is given

The reference cleans every variant before use: name terminated;
base type pinned to 1..5; every boolean made 0 or 1; growth, respawn
time, suicide penalty and lives raised to at least 0; health pinned to
0.25..4.0; weapon set pinned to 0..13; vehicle set to 0..4; for
capture the flag teams forced on; oddball ball count pinned to 0..16.
Disc values for all 26 stock variants are inside these ranges (checked).

### 1.5 What the setup menus offer (disc, `ui.map`)

| Setting | Choices (stored value) |
|---|---|
| lives | infinite (0), 1, 3, 5 |
| maximum health | 50, 100, 150, 200, 300, 400 percent (0.5, 1.0, 1.5, 2.0, 3.0, 4.0) |
| respawn time | instant (0), 5 s (150), 10 s (300), 15 s (450) |
| respawn time growth | none (0), 5 s, 10 s, 15 s (same ticks) |
| suicide penalty | none (0), 5 s, 10 s, 15 s |
| odd man out, invisible players, shields, infinite grenades, team play | yes / no |
| weapon set | normal, pistols, rifles, plasma weapons, sniper, no sniping, rocket launchers, shotguns, short range, human |
| vehicle set | all, none, warthog, ghost, scorpion |
| starting equipment | custom, generic |
| objectives indicator | motion tracker, nav points, none |
| slayer kills to win | 5, 10, 15, 25, 50 |
| capture the flag captures to win | 1, 3, 5, 10, 15; single flag: off, 1, 2, 3, 5, 10 minutes (1800, 3600, 5400, 9000, 18000 ticks) |
| king of the hill score to win | 1, 2, 5, 10, 15 minutes |
| oddball minutes/points to win | 1, 2, 5, 10, 15; ball spawn count 1 to 16 |
| race laps to win | 1, 3, 5, 10, 15, 25 |
| oddball speed with ball | normal (1), slow (0), fast (2) (menu order) |

The menus never write other values; a saved file may carry any value
that section 1.4 accepts.

### 1.6 Options the PC game adds (reference only, not on the disc)

The port stores these after the variant in the same file (offset 0x100
in the 512-byte file; the disc's own files have zeros there) and sends
them with a game. They do not exist in the Xbox game, where friendly
fire is always on and a game ends only by score, lives, or players
leaving. Defaults are what a variant without them gets.

| Option | Values | Default | Effect |
|---|---|---|---|
| time limit | minutes, 0 none | none | the game ends when the game clock reaches it |
| friendly fire | on, off, shields only, explosives only | on | applies in team games only, to a teammate's hit (never to oneself): off removes the damage, shields only lets it hit shields, explosives only lets explosions through |
| friendly fire penalty | seconds | 0 | added to a team killer's next respawn |
| vehicle respawn time | seconds, 0 never | never | an empty vehicle more than 2 WU from its home, or a wreck, is replaced after this |
| auto team balance | on / off | off | at a death, a player on a team 2 or more larger moves to the other team |
| radar players | all, friends, none | all (none if flag bit 0 clear) | which units show on the motion tracker |
| vehicle set and counts per team | | the variant's | which vehicles each team's nearer placements keep |
| loadout | category (weapon set), custom primary/secondary | category | custom replaces every weapon by the two chosen |
| no map weapons | on / off | off | netgame equipment entries whose item is a weapon create nothing |
| force team spawns | on / off | off | a team game other than CTF spawns as CTF does |

Weapon sets 11 to 13 are listed in section 5.3.

## 2. The stock game types

Decoded from the executable's table of 26 builder functions (disc).
Names are the strings of `ui\default_multiplayer_game_setting_names`.
Times are given in seconds (ticks / 30). "Flags" is the value of 0x20.
Indicator: M motion tracker, N nav points, - none. "Lives" - means
infinite. The reference's builders give identical values in every
column except the one noted below.

| # | Name | Base | Teams | Flags | Respawn | Growth | Suicide | Lives | Health | Score to win | Ind. | Weapon set | Vehicle set | Odd man | Options |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | Slayer | slayer | no | 3 | 0 | 0 | 10 | - | 100% | 15 | M | 0 | 2 | no | death bonus on, kill penalty on, in order off |
| 1 | Slayer Pro | slayer | no | 35 | 0 | 0 | 15 | - | 100% | 25 | M | 0 | 2 | no | death bonus off, kill penalty off |
| 2 | Elimination | slayer | no | 3 | 0 | 0 | 10 | 1 | 100% | 25 | M | 0 | 2 | no | bonus on, penalty on |
| 3 | Phantoms | slayer | no | 18 | 5 | 0 | 5 | - | 100% | 10 | N | 0 | 2 | no | bonus off, penalty off, in order on |
| 4 | Endurance | slayer | no | 3 | 0 | 10 | 10 | 5 | 100% | 10 | M | 0 | 2 | yes | bonus on, penalty on |
| 5 | Rockets | slayer | no | 34 | 0 | 0 | 10 | - | 100% | 25 | N | 6 | 2 | no | bonus on, penalty on |
| 6 | Snipers | slayer | no | 34 | 0 | 5 | 10 | - | 100% | 15 | N | 4 | 2 | no | bonus on, penalty on |
| 7 | Oddball | oddball | no | 3 | 5 | 0 | 5 | - | 100% | 2 | N | 0 | 1 | no | type 0, 1 ball, speed slow, traits none, random start off |
| 8 | Reverse Tag | oddball | no | 2 | 5 | 0 | 5 | - | 100% | 2 | N | 0 | 1 | no | type 1, 1 ball, speed slow, traits none, random start off |
| 9 | Accumulate | oddball | no | 2 | 5 | 0 | 5 | - | 100% | 5 | - | 0 | 1 | no | type 1, 16 balls, speed slow, traits none, random start off |
| 10 | Juggernaut | oddball | no | 3 | 5 | 0 | 5 | - | 100% | 10 | N | 0 | 2 | no | type 2, 1 ball, trait with ball extra damage, trait without none, speed slow (stored 0: the builder sets no speed), random start off |
| 11 | Stalker | oddball | no | 2 | 5 | 0 | 5 | - | 100% | 10 | M | 0 | 2 | no | type 2, 1 ball, speed fast, with ball invisible, without ball damage resistant, random start off |
| 12 | King | king | no | 3 | 5 | 0 | 5 | - | 100% | 2 | N | 0 | 2 | no | moving hill off |
| 13 | King Pro | king | no | 35 | 10 | 0 | 15 | - | 100% | 2 | N | 0 | 2 | no | moving hill off |
| 14 | Crazy King | king | no | 3 | 0 | 0 | 5 | - | 100% | 2 | N | 0 | 2 | no | moving hill on |
| 15 | Race | race | no | 3 | 0 | 0 | 10 | - | 100% | 3 | N | 0 | 2 | no | type normal, team scoring minimum |
| 16 | Rally | race | no | 3 | 0 | 0 | 10 | - | 100% | 15 | N | 0 | 2 | no | type rally, team scoring minimum |
| 17 | CTF | ctf | yes | 3 | 10 | 0 | 5 | - | 100% | 3 | N | 0 | 2 | no | assault off, must reset off, at home off, single flag off |
| 18 | Invasion | ctf | yes | 3 | 0 | 0 | 5 | 5 | 100% | 3 | N | 0 | 2 | no | assault on |
| 19 | Iron CTF | ctf | yes | 3 | 15 | 0 | 5 | - | 200% | 3 | N | 0 | 4 | no | flag must reset on |
| 20 | CTF Pro | ctf | yes | 35 | 10 | 0 | 15 | - | 100% | 3 | N | 0 | 2 | no | flag at home to score on |
| 21 | Team Race | race | yes | 3 | 0 | 0 | 10 | - | 100% | 3 | N | 0 | 2 | no | type normal, minimum |
| 22 | Team Rally | race | yes | 3 | 0 | 0 | 10 | - | 100% | 5 | N | 0 | 2 | no | type rally, minimum |
| 23 | Team Ball | oddball | yes | 35 (inferred, see notes) | 10 | 0 | 5 | - | 100% | 2 | N | 0 | 1 | no | type 0, 1 ball, speed slow (stored 0), traits none, random start off |
| 24 | Team King | king | yes | 3 | 10 | 0 | 5 | - | 100% | 2 | N | 0 | 2 | no | moving hill on |
| 25 | Team Slayer | slayer | yes | 3 | 10 | 0 | 10 | - | 100% | 50 | M | 0 | 2 | no | bonus on, penalty on |

Notes:

- An oddball option a row does not name is stored as 0 by that builder:
  speed with ball 0 is **slow** (not normal), traits 0 are none, random
  start 0 is off, ball type 0 is normal. The "ball spawn delay" byte
  (never read) is 1 on Oddball and Reverse Tag and 0 elsewhere. Checked
  on the executable's builders (all 6 oddball builders).
- "Score to win" is kills for slayer, captures for CTF, laps for race
  (flag points for rally), and **minutes** for oddball types 0 and 1
  and for king of the hill; for oddball type 2 (juggernaut, stalker)
  it is kills.
- Flags 3 is "radar and friend indicators"; 35 adds generic starting
  equipment (the menu text of Slayer Pro, CTF Pro and Team Ball says
  "Start with Pistol, AR and Grenades"); 34 and 18 have no radar
  objects (Rockets and Snipers, Phantoms); 18 is friend indicators and
  invisible players; 2 is friend indicators only. Rockets and Snipers
  also have generic equipment (34 = 2 + 32).
- The Team Ball builder never initialises the flags word: the code that
  should set bits 0, 1 and 5 reads and writes the wrong slot of the
  record, so the saved flags are stack garbage. The value 35 above is
  the intent (its menu text says it starts with grenades, pistol and
  assault rifle). A rebuild should use 35. Every other builder was
  checked field by field against the reference and agrees.
- The menu descriptions on the disc: Slayer "classic deathmatch, 15
  kills"; Slayer Pro "25 kills, start with pistol, AR and grenades";
  Elimination "one life, last cyborg standing"; Phantoms "invisible,
  follow the nav points and kill your targets in order"; Endurance "5
  lives, respawn time growth and odd man out"; Rockets "everybody has
  rocket launchers, nobody has a motion tracker, 25 kills"; Snipers
  "sniper weapons, respawn time growth on, 15 kills"; Oddball "hold the
  skull for 2 minutes"; Reverse Tag "when you are it you run slowly but
  score time, accumulate 2 minutes"; Accumulate "score 5 minutes; kills
  speed up your timer, but you lose your time bonus if you die";
  Juggernaut "extra damage; kill him to steal his powers; 10 kills";
  Stalker "one sneaky player is invisible; kill him to gain his powers;
  10 kills"; King, King Pro and Crazy King "2 minutes" (the last moves);
  Race "3 laps"; Rally "15 flag points"; CTF "3 captures"; Invasion "3
  assaults, 5 lives"; Iron CTF "health 200 percent, both teams have
  Scorpions"; CTF Pro "flag must be at home to score"; Team Race "all
  players touch the flags in order, 3 laps"; Team Rally "5 flag
  points"; Team Ball, Team King "10-second respawn"; Team Slayer "50
  kills; kills make you slower, dying makes you faster".
- Accumulate has the indicator "none" and 16 balls; Reverse Tag and
  Accumulate are ball type 1 (section 6.5).

## 3. Teams

- Two teams: 0 red, 1 blue. Team colours for the player's armour in a
  team game are pure red (1, 0, 0) and pure blue (0, 0, 1); in free
  for all each player's profile colour (the 18-entry table in
  `objects.md`) is used. Disc and reference agree.
- **Assignment.** In a team game the host gives players teams in the
  order they join, alternating 0, 1, 0, 1 (the lobby can balance later
  joiners onto the smaller team: the new player joins the team with
  fewer players, red on a tie). In free for all each player's team is
  their own slot number, so every other player is an enemy and a
  player is not their own enemy.
- **Betrayal and friendly fire** (Xbox rules). Damage between
  teammates always applies. A kill of a teammate is a betrayal: the
  killer scores -1 in slayer; the victim's respawn gets the suicide
  penalty added (it is not "a kill by an enemy"). Nothing else is
  charged. The PC options of section 1.6 add a friendly fire mode, a
  respawn penalty for the team killer and auto balance.
- **Balance.** The Xbox game has none after the game starts except the
  lobby's rule above. The game ends at once if every player who is
  still alive or has lives left is on one team (and there was more than
  one player to begin with); a solo player never triggers that.
- Capture the flag, oddball with teams, king of the hill with teams
  and race with teams score per team; slayer with teams scores per
  team and per player (section 6.2).
- The radar and friend indicators are section 7.

## 4. Spawning

### 4.1 When a player spawns

Each tick, in slot order, the game looks at every player with no unit
(the players step of section 10) and asks whether to spawn
them. Reference:

1. A player who has quit never spawns.
2. A player with **no deaths yet** (`deaths` = 0) spawns now. This is
   the first spawn at the start of a match and any later join.
3. A player out of lives (`lives` > 0 and deaths >= lives) never
   spawns again. With odd man out on, the dead player who died most
   recently (ties: the highest slot) does not spawn while they hold
   that place: they respawn only when a later death takes the place.
4. While the game is over (the postgame states) nobody spawns.
5. Otherwise the player's **respawn timer** (ticks) is tested: if it is
   above 0 it is decremented now, and the player may spawn on the tick
   it reaches 0 (the first decrement is on the first player update
   after the unit is gone, open question 10); if it is already 0 the
   player may spawn now.
6. **Stagger.** A player who may spawn after game time 3 actually
   spawns only if the game time in ticks mod 32 equals the player's slot
   mod 32; otherwise nothing happens and the test repeats next tick (the
   timer stays 0). The wait is 0 to 31 ticks.

For a local human the HUD plays the "countdown for respawn" sound when
the timer is 90, 60 and 30 at the test and the "player respawn" sound
when it is 1 (the sounds of section 9; they are not queued).

When the game finds the player a place, it makes the unit
(4.3 to 4.7) and the postspawn step gives it equipment (4.6). If it
finds **no usable start** no unit is made and the timer is set to 1 so
the test repeats, which with the stagger means one try every 32 ticks
(1.07 s) once game time passes 3.

### 4.2 The respawn timer

Computed when a player dies, inside the game type's kill handler
(section 6.1). Let R = respawn time, G = respawn time growth, S =
suicide penalty (ticks), P = the dead player's respawn penalty
(starts at 0 with the game and is kept), and "kill" mean the death was
credited to an enemy player (6.1 step 2: the credited player is not the
dead player and not a teammate; a fall or suicide soon after an
enemy's damage is credited to that enemy and so is a kill).

1. timer = P + R.
2. If G > 0: P becomes min(P + G, 5 G); and if it was a kill, the
   killer's own P becomes max(P_killer - G, 0).
3. If not a kill: timer = timer + S. (Suicide, team kill, and a death
   with no player killer all pay S.)
4. timer = max(timer, 90).

The decrement is one per tick from the first update with no unit.
Examples are in the test cases.

### 4.3 Which starts are candidates

Only the scenario's player starting locations (`scenario.md`) are
used, all of them in block order. A start is **eligible** when any of
its four game type slots matches the game in play: equal to the base
game type (1 to 5), or 12 (all), or 13 (all but capture the flag), or
14 (all but capture the flag and race). A free slot (0) matches
nothing during a game. Team Slayer and Slayer are both base type 2.

The start's team index is used **only in capture the flag**: a start
counts for a player there only if its team index equals the player's
team (0 or 1); starts with team index 2 or 3 are never usable in CTF.
In every other game type, teams or not, the team index is ignored: a
red player may spawn at a "blue" start of a team slayer map.

When a capture the flag game begins the game also checks each CTF
eligible start with team 0 or 1: a start that is nearer (squared 3D
distance) to the **enemy** team's flag spot than to its own team's is
given team index 3 and is unusable for the whole game. (In assault the
code's flag slots are swapped and its comparison is reversed, which
comes to the same test on the real flag spots.) Disc: 3 starts are
removed this way across the 13 maps (Beaver Creek's blue starts 16
and 63, Rat Race's blue start 44).

### 4.4 Rating a candidate

For a player p choosing a start s (every eligible start, every
attempt, a fresh random draw each):

1. rating = 1. In capture the flag, if p's team differs from s's
   team index: rating = 0.
2. **Distance to other players.** For every other player q that
   currently has a unit, let d be the 3D distance from the unit's
   origin to s. In a team game a teammate with d > 0.25 is skipped
   entirely. Otherwise:
   - d < 0.25: rating = 0; else if d < 1: rating = rating x 0.1;
   - if q is an enemy (a different team): d < 2: rating = 0; else if
     d <= 5: rating = rating x (d - 2) / 3. (A free for all
     opponent is an enemy.)
3. In a team game, rating > 0: **friends bonus.** Over living
   teammates with 1 <= d <= 6, sum (1 - 0.2 (d - 1))^0.6, cap the sum
   at 3, and multiply the rating by 3 x sum + 1.
4. **Vehicles.** If any vehicle lies within 0.1 WU of s, rating = 0.
   (Only a vehicle blocks; a standing biped is handled by step 2.)
5. **Assault capture the flag only.** Let D2 be the squared distance
   from s to the player's **own** team's flag spot, pinned to 0.5..10,
   and r = 1 / D2; after game time 30 ticks, if D2 > 1 r = r^0.33,
   then r pinned to 0.5..2.0; rating = rating x r. (A start near its
   own base is preferred; the effect is gentle once the game runs.)
6. A start eligible by game type but not by 4.3's team rule has
   rating 0.

The game then multiplies each rating by u^0.5, u a fresh uniform
random number in [0, 1), and takes the start with the **largest
product**, ties to the earlier start; the product must be strictly
greater than 0. With no such start nothing spawns (4.1). If no game
is running (campaign) the start is chosen at random: not a multiplayer
case.

So the best start is not always chosen: a start with rating 1 beats a
start with rating 0.5 with probability P(u1 > 0.25 u2) = 1 - 0.25 / 2 =
87.5 percent. Test cases fix the draws.

### 4.5 Position, facing and protection

The unit is created at the start's position (the feet origin, no
offset) with the start's heading as its facing, up = +Z, and the
player's change colour (section 3). The unit is the globals'
multiplayer unit `characters\cyborg_mp\cyborg_mp` (disc), not the
player information unit `characters\cyborg\cyborg`. There is no
invulnerability, no spawn protection, no temporary camouflage and no
delay before the unit can act: none exists in the reference, and
nothing on the disc points to one. The unit is made with full health
and shields of its tag (`health_shields.md`). A start the unit does not
fit in is not detected here: the unit simply collides with whatever is
there.

### 4.6 What the player holds

Order, as the reference does it after the unit exists:

1. **The unit's own weapons.** The multiplayer biped's tag lists two
   starting weapons: `weapons\assault rifle\assault rifle` then
   `weapons\pistol\pistol` (disc; the campaign biped lists none). Each
   is created through the item substitution of section 5.3 (the weapon
   set), and a second one of the same tag is discarded. The assault
   rifle is the first weapon (the one in hand, inferred); the pistol is
   holstered.
2. **Custom starting equipment.** If the variant's flag bit 5 (generic)
   is **clear**, the scenario's starting equipment entries (`scenario.md`)
   are walked in block order and the first whose game types are eligible
   (same match rule as 4.3, with the base game type) is used: for each
   of its first five non-null item collections, one item is drawn
   (5.2) and created through the weapon set substitution; the **first**
   drawn weapon replaces every weapon the unit has (the assault rifle
   and pistol are deleted), later ones are added to the unit and kept
   holstered, and a drawn weapon of a tag the unit already holds is
   discarded. If no entry matches, the unit keeps the assault rifle and
   pistol. If the flag bit is **set**, none of this happens.
3. **Grenades.** Start counts: frag = 4, plasma = 0 (the globals'
   maximum of the frag grenade). If the game began with 9 or more
   players the cap for both kinds is 1, with 5 to 8 players the cap is
   2, else 4 (the player count is taken when the game starts and never
   goes down). Then the matched starting equipment entry's flags:
   bit 0 "no grenades" sets both counts to 0; bit 1 "plasma grenades"
   adds the frag count to the plasma count and sets frag to 0. Then,
   if infinite grenades is on, both counts become the cap. Then the
   weapon set: plasma weapons (3): plasma += frag, frag = 0; human
   (9): frag += plasma, plasma = 0; no grenades (10): both 0 unless
   infinite grenades is on. Finally each count is limited to the cap
   (4, 2 or 1) and stored. With infinite grenades on, throwing does
   not decrease the counts (`weapons.md`).
4. **Ammunition.** A new weapon has its loaded rounds = min(initial
   total, loaded maximum) and spare rounds = initial total - loaded
   (`weapons.md` magazines): assault rifle 60 loaded + 180 spare,
   pistol 12 + 48, shotgun 12 + 12, sniper rifle 4 + 8, needler 20 + 60,
   rocket launcher 2 + 2, flamethrower 100 + 500. Plasma weapons have
   a battery (full, age 0), not rounds.

Disc starting equipment per map (entries in block order; "gt" the
entry's first game type slot, flags the grenade flags):

| Map | Entries |
|---|---|
| beavercreek | gt 1: pistol; gt 13: plasma pistol |
| bloodgulch | gt 1: assault rifle, pistol; gt 13 (flags 1): plasma pistol |
| boardingaction | gt 1: assault rifle; gt 13: pistol |
| carousel | gt 13 (1): plasma pistol; gt 1 (1): plasma pistol, plasma rifle |
| chillout | gt 14 (1): assault rifle; gt 1 (2): plasma pistol; gt 5 (1): needler |
| damnation | gt 13 (1): plasma pistol; gt 1: assault rifle, pistol |
| hangemhigh | gt 14 (1): pistol; gt 1 (2): plasma rifle; gt 5 (1): plasma pistol |
| longest | gt 13 (1): plasma pistol; gt 1 (1): plasma rifle |
| prisoner | gt 13 (1): plasma pistol; gt 1 (1): pistol, assault rifle |
| putput | gt 12: pistol |
| ratrace | none |
| sidewinder | gt 1: assault rifle, pistol; gt 14: pistol; gt 5 (1): plasma pistol |
| wizard | gt 2, 3 and 5 (1): plasma pistol; gt 1 (1): shotgun; gt 4 (1): needler |

Example: Slayer (generic bit clear) on Blood Gulch: the second entry
(13 matches 2) applies, so the unit starts with **one plasma pistol
and no grenades**. Slayer Pro on the same map (generic bit set): assault
rifle, pistol, 4 frag grenades (fewer in a big game).

### 4.7 Health, shields and the "health" setting

Full health and shields at spawn. The variant's **health** value does
not change a player's maximum: every damage dealt to a player is
divided by min(max(health, 0.25), 4.0), a player in a game with 200
percent health takes half damage (`health_shields.md` applies the
number; this spec only states where it comes from). The oddball
traits add: a carrier with "extra damage" deals 1.5 x damage; a player
with "damage resistant" takes 0.5 x. With "no shields" on, every tick
the unit's shield and maximum shield are set to 0. With "invisible
players" on, every tick the unit is given the active camouflage
powerup effect with a 15 tick duration (so it never lapses).

## 5. Pickups

### 5.1 Netgame equipment: when an item is created

Entry layout and counts: `scenario.md`. Behaviour (reference), each
tick, for every entry whose game types match the game in play
(4.3's rule with the base game type; a free slot never matches):

1. period (ticks) = 30 x the entry's `spawn time` if it is not 0;
   otherwise 30 x the collection's `spawn time` if that is not 0;
   otherwise 900.
2. If the game time (ticks since the match began) is an exact multiple
   of the period, one item is drawn from the collection (5.2), passed
   through the substitution of section 5.3 as it is created at the
   entry's position, with the default orientation (the entry's facing
   is ignored on the Xbox), and the item is marked "at rest" if the
   entry's levitate flag is set (otherwise it falls).
3. A created item has its "last owned" time advanced by period - 900
   ticks.

Consequences:

- **The respawn clock is global.** An item whose entry has period T
  appears at game times 0, T, 2T... regardless of when the previous
  one was taken. After a pickup at time t the next item appears at the
  next multiple of T after t (up to a full period later, 1 tick
  later at best). A fresh match creates every entry's first item at
  tick 0 (inferred: the clock's first tick is 0).
- **An item nobody picks up is replaced.** Section 5.4's rule deletes
  an unheld item when `now - 900 > last owned`; with the advance of
  step 3 that is the tick after the next one appears (the old item and
  the new one overlap for exactly one tick, at the multiple).
- A random draw that yields nothing, or the "no map weapons" option
  with a weapon, creates nothing that period.
- Grenade entries (frag and plasma, in groups of four at the same
  spot) are items too: each created grenade item is one pickup of one
  grenade. Section 5.3 removes some of them.
- Entries are visited in block order, so draws consume the random
  number stream in that order.

Periods in effect on the disc (13 maps, 535 entries): entries with
spawn time 0 take the collection's: 60 s for `powerups/powerup
invisibility` and `powerups/powerup super shield`, 120 s for
`single weapons/rocket launcher`, 180 s for `single weapons/plasma
grenades` and `powerups/shield-invisibility`, 30 s for every other
collection (`item collections` has no spawn time on them). Entries with
their own spawn time: 30 (17 entries), 45 (40, all of Chiron TL-34's),
90 (2), 120 (7), 180 (11). Per-map tables are in section 11.

### 5.2 Item collections

A collection (`itmc`) is a list of permutations, each a weight and an
item (a weapon or equipment tag). A draw: total = the sum of the
weights truncated to a whole number; r uniform in [0, total) (an integer
from the game's seeded random stream); walk the permutations in order,
subtracting each weight; the first that makes r negative is the item. A
collection with no permutation, or total 0, yields nothing. On the
disc (16 collections, all resolve; checked): 14 have one permutation of
weight 100; `powerups/shield-invisibility` is active camouflage 50 and
over shield 50 (period 180 s); `powerups/group all powerups` (Chiron
TL-34's one entry) is camouflage 70, over shield 90, health pack 15.
The probabilities of the last: camouflage 40 percent, over shield 51.4,
health pack 8.6 (r in 0..174: 0..69, 70..159, 160..174).

### 5.3 The weapon set: item substitution

Every object created while a game is running (netgame equipment, the
unit's own weapons, custom starting equipment, a vehicle) is passed
through the game's substitution first. The weapon table below is the
reference's; the disc fixes only the list of the 14 substitutable tags,
the globals' multiplayer weapon list (`weapon list`, identical on the 13
maps):

assault rifle, flamethrower, gravity rifle, needler, pistol, plasma
pistol, plasma rifle, rocket launcher, shotgun, sniper rifle, ball,
flag, frag grenade, plasma grenade (tags `weapons\<name>\<name>`).

The ball and flag are never changed. A weapon not in the list is
unchanged. Before the table is used, **the flamethrower and the
gravity rifle become the rocket launcher** (Derelict's one flamethrower
entry creates a rocket launcher in every game). Then:

| Weapon set | AR | flame | needler | pistol | plasma pistol | plasma rifle | rocket | shotgun | sniper |
|---|---|---|---|---|---|---|---|---|---|
| 0 normal | AR | RL | needler | pistol | PP | PR | RL | SG | sniper |
| 1 pistols | pistol | PP | PP | PP | pistol | PP | PP | pistol | pistol |
| 2 rifles | AR | PR | PR | PR | AR | PR | PR | AR | AR |
| 3 plasma | PR | PR | PP | PP | PP | PR | PR | PR | PR |
| 4 sniper | sniper | sniper | sniper | pistol | sniper | sniper | sniper | sniper | sniper |
| 5 no sniping | AR | RL | needler | AR | PP | PR | RL | SG | SG |
| 6 rocket launchers | RL | RL | RL | RL | RL | RL | RL | RL | RL |
| 7 shotguns | SG | SG | SG | SG | SG | SG | SG | SG | SG |
| 8 short range | SG | RL | SG | SG | PP | PR | RL | SG | SG |
| 9 human | AR | RL | AR | pistol | pistol | AR | RL | SG | sniper |
| 10 no grenades | as 0 | | | | | | | | |
| 11 covenant (PC) | PR | needler | needler | PP | PP | PR | needler | needler | needler |
| 12 classic (PC) | as 0 | | | | | | | | |
| 13 heavy (PC) | RL | RL | RL | RL | RL | RL | RL | RL | RL |

(AR assault rifle, PP plasma pistol, PR plasma rifle, RL rocket
launcher, SG shotgun.) With a PC custom loadout there is no weapon
set: only the flamethrower and gravity rifle rule applies.

**Grenade items and starting grenades.** The frag and plasma grenade
tags are substituted as items: plasma weapons (3): frag grenade item
becomes plasma grenade item (both kinds become plasma); human (9):
plasma becomes frag; no grenades (10): both become nothing. Then, with
infinite grenades on, a grenade item becomes nothing. Then if the game
began with 9 or more players, a grenade item is kept only if a random
draw is at most 0.3, with 5 to 8 players at most 0.55. The starting
grenade counts follow 4.6.

**Powerups.** An over shield item becomes nothing when "no shields" is
on; an active camouflage item becomes nothing when "invisible players"
is on. Health packs and other equipment are never changed.

**Vehicles.** The vehicle set (0 all, 1 none, 2 warthog, 3 ghost, 4
scorpion) keeps only that kind of the three the globals name (section
11); set 1 keeps none. Vehicles the globals do not name are deleted.

### 5.4 Items on the ground

- Every item has a "last owned" time: its creation time, then every
  tick the tick it is attached to a unit. Each tick the game deletes
  every item that is not attached whose last owned time is less than
  now - 900 (so it lives 901 ticks after it was last held), except
  **flag and ball weapons**, which are never purged (their games reset
  them: 6.4, 6.5). A map item's last owned time is advanced by its
  period less 900 ticks when it is created (5.1).
- A **dropped weapon** (swapped away, or dropped by a dead player) is
  unattached from that moment, so it is deleted 901 ticks (30.03 s)
  later unless picked up. A weapon dropped with no ammunition
  (`weapons.md`) is deleted at once.
- A dead biped lying with no activity for more than 900 ticks is
  deleted.
- An item thrown or left in the air falls with the normal physics;
  the "at rest" levitate flag is only for map items (disc: 5 of 535
  entries, all powerups).

### 5.5 Picking up

Weapons and equipment are picked up by the unit that overlaps them: the
check runs each tick for every item whose position is within the
unit's bounding sphere. Only the host decides pickups.

- **Powerups** (equipment with a powerup type) are used at once by a
  unit that has no stored equipment (multiplayer never stores one).
  The effect's own conditions refuse some pickups: an overshield is
  refused when `health_shields.md`'s double-charge refuses; a health
  pack is refused when the body is full; active camouflage is refused
  while the unit is already camouflaged. A refused item stays. Taken:
  duration = powerup time x 30 ticks (camouflage 45 s = 1350 ticks,
  counted down once per tick in the after-game step); in multiplayer
  a duration of the same kind is **added** to any remaining one (the
  campaign does not add; for camouflage this never happens because
  of the refusal). The overshield's time is 60 s but its effect is
  `health_shields.md`'s; the health pack's time of 1 only means
  "valid". The pickup sound plays for a local player.
- **Grenades** (equipment of type grenade): +1 of that kind per item
  while the count is below 4 (the globals' maximum); otherwise the item
  stays. The count is not limited to the starting cap of 4.6.
- **Ammunition from a weapon on the ground** (a weapon of a tag the unit
  already holds): done by the same check, no button: for each magazine
  of the held weapon whose spare rounds are below the tag's maximum,
  take min(the ground weapon's spare rounds, maximum - held spare);
  the ground weapon's spare rounds fall by that; the ground weapon is
  deleted if its spare rounds reach 0 (its loaded rounds are lost).
  The held weapon's pickup sound plays for a local player. If the held
  weapon's spare rounds are already at the maximum nothing is taken
  and the ground weapon is treated as a candidate for swapping. Spare
  maxima: assault rifle 600, pistol 120, shotgun 60, sniper rifle 24,
  needler 80, rocket launcher 8, flamethrower 1200.
- **Ammunition crates** (equipment named in a magazine's ammunition
  objects) give min(rounds listed, rounds needed) and are deleted.
  No multiplayer map places any.
- **Weapons**: auto pickup, swap and the 7-frame action hold are
  `weapons.md`'s; here: the unit holds at most two weapons that count
  (flag and ball do not count); a third is a swap. A unit with one weapon
  picks a different second weapon up on the action hold; walking over it
  does not (the reference's port adds the walk-over pickup; the Xbox
  rule is the hold). **Capture the flag's flag** is picked up by
  walking (forced), by an enemy; the owning team touching it away from
  home returns it (6.4). The ball is picked up by walking unless the
  unit holds another weapon that must be readied.

## 6. Scoring and the end of a game

### 6.1 Kill bookkeeping (all game types)

At a death the game credits a killer and records statistics, then calls
the game type's handler, in this order (reference):

1. The dead player's deaths +1; their kills in a row = 0; their last
   kill time = none; their multi-kill count = 0. If the killer is the
   dead player, suicides +1.
2. **Who is credited.** The unit keeps a list of attackers with the
   damage each inflicted and the tick of their last hit. Call the death
   "unattributed" when there was no player killer, or the killer is on
   the dead player's team (a suicide is both). Candidates are the
   killer's own entry and, when unattributed, every attacker who is an
   enemy of the dead player; only entries hit within the last 180
   ticks (6 s) count, and the candidate with the most damage is
   credited (if none qualifies, the killer, if any, is credited; if
   there is no killer either, nobody). So a fall, a grenade of one's
   own or a teammate's shot within 6 s of an enemy's damage is
   credited to that enemy as a normal kill. (Reference quirk: the
   enemy test compares the attacker's player slot as if it were a team
   number: right in free for all, where team = slot, wrong in team
   games, where it can credit the wrong side or nobody.) A credited
   player on the dead player's team (a suicide counts as such) is a
   **friendly fire** credit: their friendly-fire-kill count +1 and no
   kill; otherwise their kills +1, kills in a row +1, and the
   **multi-kill rule**: if their last kill time is not before (now -
   120) their multi-kill count +1, else it is set to 1; their last
   kill time = now.
3. The game type's kill handler runs with (credited player, dead
   player, friendly fire).
4. The respawn timer is set (4.2).
5. A message is shown and the announcer sound chosen (section 9).

The score changes are the handler's. A **suicide** (the killer is the
victim) is a friendly fire credit: -1 in slayer.

Announcer choice for a kill of an enemy by player K, first match
wins: multi-kill count >= 4 "killtacular"; = 3 "triple kill"; = 2
"double kill"; kills in a row = 5 "killing spree"; kills in a row >= 1
and a multiple of 5 (10, 15, 20...) "running riot"; else the plain
kill message. (A single kill never makes a multi-kill sound; the count
1 case falls through.)

### 6.2 Slayer (and Team Slayer), in full

State: a team score and a player score for each team and each slot
(free for all: team = slot, so the two are equal).

On a kill with the killer not none, the dead player not quit:

- **Enemy kill (not friendly fire).** Apply the speed rule below to
  both. If kill in order is on: the kill scores only if the dead player
  is the killer's current target, and then a new target is chosen; any
  other kill scores nothing (the speed rule still applied). Otherwise
  the killer's team score and player score each +1.
- **Friendly fire credit** (a teammate, or oneself): the killer's team
  score and player score each -1. No other change.
- A death with no credited player changes no score (but the suicide
  penalty applies to the timer, 4.2).

Scores may go negative. Scores keep changing after the game has
ended (7 s of postgame) because the handler does not check; the
reference's other game types do check.

**Speed rule** (kill penalty and death bonus). Each player has a speed
multiplier, starts at 1.0, which multiplies their movement speed
(`movement.md` applies it); an enemy kill changes it only when the
option is on (the variant's "no ..." byte clear):
- Kill penalty on: the killer's multiplier -= 0.02; if now >= 1.0,
  -= 0.15 and raised to at least 1.0; then raised to at least 0.9.
  (A killer at 1.0 goes to 0.98; at 1.1 goes to 1.0; each further kill
  costs 0.02 down to 0.9.)
- Death bonus on: the dead player's multiplier += 0.1; if it is now
  <= 1.0, += 0.1 again and limited to at most 1.0; then limited to at
  most 1.5. (A dead player at 1.0 goes to 1.1, at 0.98 to 1.08, at 0.9
  to 1.0; at most 1.5.)
- Each tick, if kill penalty is **off** and the multiplier is above 1.0
  it falls by 0.000111 (back to 1.0 in 30 s from 1.1); if death
  bonus is **off** and it is below 1.0 it rises by 0.0000111 (back to
  1.0 in 5 min from 0.9). With both on, nothing returns it to 1.0
  except the events above.

**Kill in order.** A player's target is a random living enemy (not
themselves, not the current target, not on their team, not out of
lives or odd man out), chosen when they have a unit and no target, or
when their target is "out" (quit, out of lives, odd man out); on a
scoring kill. The target is shown with a nav point ("target blue") at
the target's bounding sphere centre. The radar indicator is
unaffected.

**End of the game.** Each tick, for each player, if the player's team
score (free for all: their own score) >= score to win the game ends
(6.3). The game also ends when only one team has players with units or
lives (fewer than two teams remain and more than one player is in
the game). **Winner**: with teams, the team with the higher team score
(equal scores: a tie, nobody wins); free for all, the player ranked
first on the score; tied for first is a tie. The player's ranking uses
the score, ties broken by the other statistics (not specified here).
There is no time limit (section 1.6).

### 6.3 Ending a game

Ending a game (reference): if the game is active, set the postgame delay
state, the timer 7.0 s, play the "game over" announcer line and close
the menus. During the delay play continues (scores still change in
slayer). When the timer reaches 0: every player's unit is killed
(deaths and scores are not changed for it: it is a "no killer" death),
every vehicle is deleted, and the network game moves to the results
screen. Nobody spawns during this time (4.1 step 4). In the last 2 s
of the delay the sound mix fades (every class to 0, the three ambient
classes to 0.2, over 30 ticks).

### 6.4 Capture the flag (brief, complete)

Objects: two flag objects, tag `weapons\flag\flag`, created at game
start, each with the owner team's number, at the capture-the-flag
netgame flags of type 0 (`scenario.md`): flag of team t at the spot of
team t's flag. With single flag on one flag only, chosen at random, is
created. Flag weapons are objective weapons: never purged, "must be
readied" and not counted toward the two-weapon limit (`weapons.md`),
and a carrier loses active camouflage every tick.

- **Grab.** An enemy touching a flag (walking over it) picks it up
  (flag taken: announcer "red/blue team has the flag" for the
  **carrier's** team; the same line repeats every 20 s while taken).
- **Return by touch.** A player of the **owning** team touching their
  own flag when it is away from home (the "handled" bit set: it was
  picked up or dropped) returns it to its spot instantly (a return is
  announced and counted), unless the variant has "flag must reset", in
  which case touching does nothing (a "flag failure" sound at most
  every 4 s) and only the timer returns it. Touching their own flag at
  home does nothing.
- **Drop and timeout.** A carrier who dies drops the flag where they
  died. A flag that is not carried, away from its spot, and has not
  been held for more than 510 ticks (17 s: `now - last held > 510`)
  returns to its spot by itself (announced as returned).
- **Capture.** Each tick, a carrier of the enemy flag whose unit
  origin is less than 1.0 WU (3D, strict) from the carrier's own team's
  flag spot scores: the team's score +1 and the player's flag score +1;
  the flag is dropped and returned. With "flag at home to score" the
  capture counts only if the carrier's own flag is at spot (not
  "handled"); otherwise it is the failure sound. No scoring after the
  game ended.
- **Assault** (invasion): the flag spots swap roles: team t's flag
  object belongs to t and starts at the spot of the **other** team, and
  a team scores by taking the enemy-owned object from its own base and
  planting it at the enemy base (the spot where the carrier's team's
  own object sat). The announcer for grabs is not played. (A start
  nearer its own flag is dropped in section 4.3.)
- **Single flag.** With a nonzero time, one flag is made for a random
  team at the start; every time the time elapses with the flag not
  carried, the flag's owner team swaps (the other team now defends)
  and the flag returns to the new owner's spot.
- **Win.** The first team whose score reaches the score to win (in
  captures) wins; a tie score at the limit cannot occur.
- **Scoring is per team**; a player's own score is their number of
  flag scores.

Kill handling is not part of capture the flag on the disc (kills do not
score).

### 6.5 Oddball (brief, complete)

Objects: the ball, tag `weapons\ball\ball`, a weapon created at the
oddball netgame flags (type 2) of the matching ball number (team index
= ball number); with random start, at a random ball flag. Ball n
appears 450 x (n + 1) ticks (15 s each) after the start (type 0 only).
Ball type 0 (normal): the ball is a weapon a player picks up by
walking (not if they hold a weapon that must be readied); a carrier
scores **1 per tick per ball held** while the game can score; the
team score is the sum; the game ends when a team's (free for all: a
player's) score reaches score to win x 1800 (minutes to ticks). The
ball resets to its spot if it is not held and not in a unit and more
than 1200 ticks (40 s) passed since it was last held (announcing
"ball reset" if it was touched; a ball-reset sound if there are 2 or
fewer balls). A carrier's speed is multiplied by 0.75 slow, 1.0
normal, 1.25 fast; a carrier loses active camouflage unless the trait
with ball is invisible. Traits: "extra damage" +50 percent damage
dealt, "damage resistant" -50 percent damage taken, "invisible" =
camouflage; applied to the carrier (trait with ball) or to every
player without a ball (trait without ball).

Ball types 1 (reverse tag) and 2 (juggernaut) have **no ball object**:
the "ball" is a status ("it"), all balls exist from the start. A kill
of an enemy while a ball is free makes the killer "it" (and scores
1 point, that is 1 tick); killing the player who is "it" passes the ball
to the killer; "it" is dropped at the holder's death. Type 1 scores by
time like type 0 (1 per tick per ball held). Type 2 gives no time
score: a kill of the holder, or a kill by the holder, gives the killer
+1 (the score to win is then in points, not minutes). A holder's
speed and traits apply as for type 0. Score announcements: 60 s and
30 s warnings ("one minute to win", "30 seconds to win", team
variants) when the score is exactly that far from the limit; a
chime every 5 s of holding.

### 6.6 King of the hill (brief, complete)

A hill is the set of type 8 netgame flags sharing a team index (hill
number); the first 12 flags in block order are used; their convex hull in
the XY plane is the hill; one flag alone is a 2 WU square; vertical
extent from the lowest flag - 0.1 WU to the highest flag + 0.8 WU
(`scenario.md`). A player is **on the hill** when the unit's bounding
sphere centre is inside that volume. Each tick, for each player on the
hill, the player's time on hill +1 and their team's score +1, but a team
gets at most one point per tick however many of its players are on
the hill (free for all: the player's own). The game ends when a team's
score reaches score to win x 1800 (minutes). The winner is the higher
score. Announcer: "60 s" and "30 s" warnings at exactly that distance
from the limit, a chime every 150 ticks, "hill contested" when a hill
that had been held more than 10 s by one side gets players of both
sides. **Contested hills still score for every team on them** (the
state only drives the sound and the hill's drawing; reference).
**Moving hill:** a timer of 1800 ticks (60 s) runs while the game can
score; at zero the hill becomes a randomly chosen other hill number
(not the current one) and the "hill move" sound plays; the timer
restarts. A map with one hill number keeps it. The game starts on
hill number 0.

### 6.7 Race (brief, complete)

Checkpoints are the race track netgame flags (type 3), their team
index is the checkpoint's sequence number (0 to 31); a player touches
one when within 2.5 WU in a vehicle, or 1.5 WU horizontally and 0.6 WU
vertically on foot (`scenario.md`). On start the game deletes every
placed vehicle and creates up to 8 race vehicles at the race vehicle
netgame flags (type 4), one per player in order of closeness to a free
flag; the n-th (from 0) is: vehicle set default: warthog, scorpion,
then ghosts for n = 2..5, none after; warthog: n < 4; ghost: n < 8;
scorpion: n < 4; none: none. Race types: normal (checkpoints in
sequence), any order, rally (touch the one lit flag; a random other
flag then lights). A lap is all checkpoints touched and the first
re-touched; rally scores a point per flag. Score to win is laps (rally:
flags). The first to reach it wins; teams combine members' laps by
minimum, maximum or sum (a team of minimum scoring cannot win once a
member is out of lives or quit). **Catch-up:** a player 1 lap (rally:
3 flags) behind the leader moves 1.1 x speed, 2 or more laps (rally: 6
or more) behind 1.2 x.

### 6.8 Teleporters

`scenario.md` owns the flags (type 6 sources, type 7 targets, pairs by
team index). This is the whole rule, run once per tick for each player
that has a unit, in step 6 of section 10 (after the units' movement of
that tick, before the objects' update and the telefrag counters).

| Item | Value | Unit | Meaning (reference) |
|---|---|---|---|
| Trigger centre and point | source flag position; the player's origin (between the feet) | | no use of the camera or the pill |
| Trigger radius | 0.5, inclusive (squared distance ≤ 0.25) | WU | a 3D sphere; no height limit |
| Source chosen | first source flag in block order within the radius | | skipped if it is the player's "arrived at" flag |
| Target chosen | first target flag in block order with the source's team index | | none: nothing happens (retried every tick) |
| Exit test volume | a pill of radius 0.2 and segment length 0.3 − 0.2 c (c = the arriving player's crouch fraction: 0.3 standing, 0.1 crouched), its bottom-sphere centre exactly at the target flag's position | WU | the flag is used as that centre, not as the feet: the volume reaches 0.2 below the flag and 0.5 above it standing |
| Things that block | living bipeds and vehicles only | | not scenery, machines, equipment, weapons or the level; a dead biped, a biped inside a vehicle's seat, and an object flagged invisible or without collisions never block |
| A biped blocks when | the flag lies within 0.4 (0.2 + 0.2) of the segment from (its origin z + 0.2 − the arriving segment length) up to (its origin z + 0.2 + its own segment length), measured as a capsule | WU | its own segment length is 0.3 − 0.2 c' with its crouch c'; the segment has rounded ends |
| A vehicle blocks when | the same pill overlaps its collision model's surfaces | | no simple shape |
| Blocked: effects | the arriving player is not moved; the blocking player (if the blocker is a player's unit) gets +1 on its telefrag counter and is marked "blocking" this tick; nothing else happens to a non-player blocker; with several overlapping objects only the deepest one is credited | | |
| Blocked message | shown to the arriving player on the first blocked attempt and then on every 121st (one counter shared by all players, decremented only on blocked attempts) | attempts | not "every 120 ticks" |
| Telefrag counter | +1 per blocked attempt (one per attempting player per tick); −1 per tick on which the player was not blocking (never below 0) | | the check runs at the end of the tick, after the attempts |
| Telefrag | the blocker's unit is killed when it is marked "blocking" this tick and its counter is at least 90 | ticks | a death with no killer; the killing tick is the 90th consecutive blocked attempt |
| Arrival position | exactly the target flag's position, as the origin | | whatever the floor there |
| Heading | body heading + target facing − source facing, in radians, not wrapped | rad | body heading = the angle of the body's forward vector in the ground plane (the body, not the aim, which can differ by up to 60 degrees at rest); the new forward vector is (cos, sin, 0) and the up vector +Z |
| View | for a local player: the view's yaw becomes the new heading and its pitch 0 | rad | a looking-up view is levelled |
| Velocity | unchanged, in world axes | | not zeroed, not rotated with the heading |
| Ground state | unchanged | | the ground plane, support surface and airborne flag are those of the source until the next movement tick finds the new floor |
| Re-arm | the player is "armed off" for the first source flag within 1 WU (≤ 1, 3D) of the new position, if any; it is cleared on a later tick when the player's origin is more than 1 WU (squared distance > 1) from that flag | WU | every other source works at once; no timer |
| Local player effects | the sound "teleporter activate", and a screen flash of colour (0.35, 1.0, 0.35), alpha 0.5, 1 s long | | not for other players |

Because the counter is on the blocker, a blocker who steps out of the
exit loses 1 per tick; two players waiting at sources of the same pair
add 2 per tick. A flag carrier keeps the flag. The arrival check
`scenario.md` describes ("treated as having arrived at the first
source within 1 WU") is the re-arm row.

## 7. The radar

(Display is `hud.md`'s; this is the rule of who is shown.) The motion
sensor widget is shown when flag bit 0 is set, or (for every game type
except slayer without kill in order) when the objective indicator is
"motion tracker". Units are listed once every 15 ticks (the list is
copied on the other ticks) for each local player:

- the unit is alive;
- the variant allows it: flag bit 0 set (other players on radar); with
  bit 0 clear no unit is listed (the PC "friends" option lists only the
  viewer's team);
- the unit is **firing** (primary trigger held) or **throwing a
  grenade** (any state but idle and ending), or **moving**: the
  squared speed of the unit >= 0.0002 WU^2/tick^2 (speed >= 0.01414
  WU/tick = 0.424 WU/s); in a game, camouflage does not hide a unit;
- its horizontal distance (height ignored in multiplayer) from the
  viewer's camera is at most 20 WU (61 m); at most 16 blips are kept
  per viewer, in object order.

The blip's colour and type (self, friend, enemy, vehicle) and the
sweep are `hud.md`'s. Game goals (flags, balls, hills, a slayer target)
appear on the radar as extra blips when the objective indicator is
"motion tracker", as nav points on screen when it is "nav points",
and not at all for "none". Who sees a goal: a flag's marker is shown to
the team that does not own the flag, except its carrier, and the
carrier alone sees the destination marker; kill in order shows the
target only to its hunter; the hill is hidden from players standing
on it.

## 8. Vehicles in multiplayer

- The scenario's vehicle placements (`scenario.md`: Blood Gulch and
  Sidewinder place 2 warthogs, 4 ghosts, 2 scorpions each; no other
  map has any) carry no game type or team data on the disc. The game
  creates all of them at load through the substitution of 5.3: vehicle
  set all keeps every one; none keeps none; warthog keeps the 2
  warthogs; ghost the 4 ghosts; scorpion the 2 scorpions. The stock
  variants use warthog (set 2) for slayer, king, race and capture the
  flag (so the maps show only the warthogs), none (1) for oddball
  types, and scorpion (4) for Iron CTF. In a team game a set applies to
  both teams equally on the disc.
- **There is no respawn on the Xbox.** A vehicle that is destroyed or
  abandoned stays as it is; nothing recreates it. (The PC option of
  1.6 recreates an empty vehicle more than 2 WU from its home after
  the time; a wreck is deleted and recreated.) Vehicles are all deleted
  when a game ends.
- Race creates its own vehicles (6.7).
- A vehicle on a start blocks it (4.4 step 4).

## 9. The announcer

The globals' multiplayer information block holds 43 sound references
(identical on all 13 maps; section 11). The game plays them through a
queue. **Queueable** lines wait for the line before them; **immediate**
ones play at once (index 26 to 31 and 42: countdown timer, teleporter
activate, flag failure, countdown for respawn, hill move, player
respawn, countdown timer end).

Queue rules (reference): at the start of a game the queue holds one
silent entry that lasts 60 ticks. A queueable line is appended with a
delay of `length x 30 / 1000 + 5` ticks (integer division, the sound
tag's stored longest permutation length in ms; it is 1.125 x the real
length, `sounds.md`), if fewer than 5 are queued, else dropped. When
the queue holds one entry (the new one), it plays at once. Each tick
the first entry's delay is decremented; when it reaches 0 the entry is
removed and the next plays.

| # | Event (tag `sound\...`) | Trigger | Stored ms | Queue ticks |
|---|---|---|---|---|
| 0 | `dialog\multiplayer1\play_ball` | oddball: a ball spawns | 1260 | 42 |
| 1 | `dialog\multiplayer1\game_over` | a game ends | 1802 | 59 |
| 2 | `dialog\multiplayer1\one_minute_to_win` | score is 60 s from the limit (no teams; oddball, king) | 1338 | 45 |
| 3 | `dialog\multiplayer1\30_seconds_to_win` | 30 s from the limit | 1557 | 51 |
| 4 | `dialog\multiplayer1\red_team_minute_to_win` | as 2, a red team | 2448 | 78 |
| 5 | `dialog\multiplayer1\red_team_30_to_win` | | 2657 | 84 |
| 6 | `dialog\multiplayer1\blue_team_minute_to_win` | | 2413 | 77 |
| 7 | `dialog\multiplayer1\blue_team_30_to_win` | | 2605 | 83 |
| 8 | `dialog\multiplayer1\blue_team_has_the_flag` | a blue player takes the enemy flag; repeats every 20 s | 2553 | 81 |
| 9 | `dialog\multiplayer1\blue_team_flag_returned` | the blue flag is returned (by touch or timeout) | 2546 | 81 |
| 10 | `dialog\multiplayer1\blue_team_score` | blue captures | 2115 | 68 |
| 11 | `dialog\multiplayer1\red_team_has_the_flag` | a red player takes the enemy flag | 2099 | 67 |
| 12 | `dialog\multiplayer1\red_team_flag_returned` | | 2240 | 72 |
| 13 | `dialog\multiplayer1\red_team_score` | red captures | 2217 | 71 |
| 14 | `dialog\multiplayer1\double_kill` | multi-kill count 2 | 1498 | 49 |
| 15 | `dialog\multiplayer1\triple_kill` | multi-kill count 3 | 1374 | 46 |
| 16 | `dialog\multiplayer1\killtacular` | multi-kill count 4 or more | 1433 | 47 |
| 17 | `dialog\multiplayer1\running_riot` | 10, 15, 20... kills in a row | 1472 | 49 |
| 18 | `dialog\multiplayer1\killing_spree` | 5 kills in a row | 1312 | 44 |
| 19 | `dialog\multiplayer1\oddball` | oddball (no teams) starts, game time 60 | 1008 | 35 |
| 20 | `dialog\multiplayer1\race` | race starts | 813 | 29 |
| 21 | `dialog\multiplayer1\slayer` | slayer (no teams) starts | 1120 | 38 |
| 22 | `dialog\multiplayer1\capture_the_flag` | CTF starts | 1733 | 56 |
| 23 | `dialog\multiplayer1\warthog` | race vehicle announcements | 1061 | 36 |
| 24 | `dialog\multiplayer1\ghost` | | 1048 | 36 |
| 25 | `dialog\multiplayer1\scorpion` | | 1087 | 37 |
| 26 | `sfx\ui\countdown_timer` | race flag touched (immediate) | 702 | 26 |
| 27 | `sfx\ui\teleporter_activate` | a local player teleports (immediate) | 1537 | 51 |
| 28 | `sfx\ui\flag_failure` | CTF scoring refused (immediate) | 561 | 21 |
| 29 | `sfx\ui\countdown_for_respawn` | respawn timer 90, 60, 30 (immediate) | 97 | 7 |
| 30 | `sfx\ui\hill_move` | the hill moves; ball reset (immediate) | 3379 | 106 |
| 31 | `sfx\ui\player_respawn` | respawn timer 1 (immediate) | 261 | 12 |
| 32 | `dialog\multiplayer1\team_king_of_the_hill` | team king starts | 1648 | 54 |
| 33 | `dialog\multiplayer1\team_oddball` | team oddball starts | 1593 | 52 |
| 34 | `dialog\multiplayer1\team_race` | team race starts | 1531 | 50 |
| 35 | `dialog\multiplayer1\team_slayer` | team slayer starts | 1440 | 48 |
| 36 | `dialog\multiplayer1\king_of_the_hill` | king starts | 1492 | 49 |
| 37 | `dialog\multiplayer1\blue_team_ctf` | single flag: blue defends/attacks | 2491 | 79 |
| 38 | `dialog\multiplayer1\red_team_ctf` | | 2586 | 82 |
| 39 | `dialog\multiplayer1\hill_contested` | hill contested | 1325 | 44 |
| 40 | `dialog\multiplayer1\hill_controlled` | hill controlled | 1609 | 53 |
| 41 | `dialog\multiplayer1\hill_occupied` | (no trigger found) | 1691 | 55 |
| 42 | `sfx\ui\countdown_timer_end` | race lap, a score chime every 5 s (immediate) | 143 | 9 |

Multi-kill and spree rules are in 6.1. Per-killer counters: kills in a
row (zeroed by the player's own death), multi-kill count, last kill
time. The window test is `last kill time >= now - 120`; the last kill
time is stored as a 16-bit value (reference), so on a game that runs
past tick 32767 (18.2 minutes) it no longer compares correctly and
multi-kills stop being counted (a quirk to decide on, section Quirks).
Messages with the same events are drawn by `hud.md`; they are
selected by the same rule, and the "with score" variants are used when
the game type shows scores (slayer, oddball type 2).

## 10. Per-tick order

One game tick (reference):

1. Remove players who quit; update allegiances.
2. Units update (movement, damage, deaths; a death sets the player's
   unit to none and calls 6.1 and 4.2).
3. AI update (none in multiplayer).
4. **Players, before the game**: for every player in slot order: a
   player with no unit goes through 4.1 and may spawn (4.3 to 4.6);
   then the player's input is applied; nearby items, equipment and
   vehicles are examined for pickup (5.5).
5. Effects, first-person weapons.
6. **Game engine update**: announcer queue tick; purge old items and
   corpses (5.4); weapon scan (objective weapon hooks: flag return,
   ball reset); netgame equipment spawns (5.1); (PC vehicle respawn);
   for each player: no shield, invisible, teleporter, and the game type's
   per-player update (flag capture, ball time, hill time, race
   checkpoints, slayer speed and score check); the game type's own
   update; the end of game test; the postgame countdown.
7. Scripts, recorded animations; objects update (physics and item
   timers); players after the game (powerup timers, telefrag
   counters, the player-in-vehicle bookkeeping).
8. Game time +1.

## 11. Measured facts

### 11.1 The globals' multiplayer information (disc, all 13 maps identical)

Tag `globals\globals` (`matg`; `globals.md` owns the layout of the whole
tag), the block at 0x164, one element of 0xA0 bytes:

| Offset | Size | Field | Value |
|---|---|---|---|
| 0x00 | 16 | flag | `weapons\flag\flag` |
| 0x10 | 16 | unit | `characters\cyborg_mp\cyborg_mp` |
| 0x20 | 12 | vehicles | 3 elements of 16: `vehicles\warthog\warthog`, `vehicles\ghost\ghost_mp`, `vehicles\scorpion\scorpion_mp` |
| 0x2C | 16 | hill shader | `scenery\hilltop\shaders\hilltop` |
| 0x3C | 16 | flag shader | `weapons\flag\shaders\flag_blue` |
| 0x4C | 16 | ball | `weapons\ball\ball` |
| 0x5C | 12 | sounds | 43 elements of 16 (section 9) |
| 0x68 | 56 | | zero |

Other globals fields this spec uses: the multiplayer weapon list at
0x14C (14 entries, in the order of 5.3); grenades at 0x128, two 0x44-byte
elements (frag, plasma): `maximum count` 4, `mp spawn default` 0 for
both (the reference never reads the second), throwing effects
`weapons\frag grenade\effects\throw` and
`weapons\plasma grenade\effects\throw`, hud interfaces
`ui\hud\default` and `ui\hud\plasma_flare`, item and projectile tags
`weapons\frag grenade\frag grenade` and `weapons\plasma
grenade\plasma grenade`. Player information (0x170, one element): player
unit `characters\cyborg\cyborg`, `multiplayer only speed multiplier` 0
(`movement.md`). The multiplayer biped lists weapons `weapons\assault
rifle\assault rifle` and `weapons\pistol\pistol`; the campaign biped lists
none. HUD globals `ui\hud\default`: motion sensor range 20, velocity
sensitivity 0.0002, scale 32 (field offsets 0x2D0, 0x2D4, 0x2D8).

### 11.2 Player starts and flags, per map (disc)

"Slayer" is the count of starts eligible for base types 2, 3 and 4 (the
same set on every map); race is base type 5; CTF red and blue are
the starts with that team index that are eligible for base type 1 and
survive the check of 4.3. Counts of starts by team index inside the
slayer set (informational: the team is ignored there) are in brackets.

| Map | Starts | Slayer, oddball, king | Race | CTF red | CTF blue | CTF removed |
|---|---|---|---|---|---|---|
| beavercreek | 70 | 22 (10 / 12) | 22 | 30 | 28 | 2 team 1 (16, 63) |
| bloodgulch | 71 | 16 (8 / 8) | 17 | 17 | 21 | 0 |
| boardingaction | 73 | 16 | 16 | 27 | 25 | 5 with team 2 or 3 |
| carousel | 63 | 27 | 27 | 18 | 18 | 0 |
| chillout | 58 | 25 | 25 | 16 | 17 | 0 |
| damnation | 67 | 30 (all team 1) | 30 | 19 | 18 | 0 |
| hangemhigh | 67 | 28 (10 / 18) | 28 | 19 | 20 | 0 |
| longest | 71 | 28 | 28 | 20 | 23 | 0 |
| prisoner | 58 | 24 (21 / 1 / 1 / 1) | 24 | 16 | 18 | 0 |
| putput | 59 | 27 (15 / 12) | 27 | 16 | 16 | 0 |
| ratrace | 48 | 48 (19 / 25 / 0 / 4 in team 3) | 48 | 14 | 16 | 1 team 1 (44) |
| sidewinder | 76 | 16 | 16 | 21 | 23 | 0 |
| wizard | 48 | 16 (1 / 1 / 1 / 13) | 16 | 16 | 16 | 0 |

(Bloodgulch's race set is the 17 starts of type 5 only: its slayer
starts are type 14, which excludes race.) CTF flag spots per map are in
`scenario.md`'s tables; teleporter, ball, hill and checkpoint counts:

| Map | Teleporter pairs | Ball spawns | Hills | Race checkpoints |
|---|---|---|---|---|
| beavercreek | 2 | 8 | 10 | 11 |
| bloodgulch | 2 | 9 | 10 | 5 |
| boardingaction | 8 | 10 | 9 | 13 |
| carousel | 4 | 5 | 10 | 10 |
| chillout | 3 | 8 | 4 | 12 |
| damnation | 2 | 8 | 8 | 4 |
| hangemhigh | 0 | 5 | 14 | 11 |
| longest | 0 | 8 | 9 | 5 |
| prisoner | 0 | 2 | 6 | 9 |
| putput | 30 | 5 | 9 | 9 |
| ratrace | 4 | 7 | 4 | 4 |
| sidewinder | 4 | 8 | 10 | 5 |
| wizard | 4 | 5 | 9 | 4 |

(Every teleporter source has exactly one target with the same team
index on every map; Wizard's ball spawn numbers are 0, 1, 2, 3, 9, and
Longest's checkpoints start at 1.) Vehicles: Blood Gulch and
Sidewinder only, 2 warthogs, 4 ghosts, 2 scorpions each (positions in
`scenario.md`); race vehicle spots: 16 on each of those two.

### 11.3 Items per map (disc)

Entries by item with the period in effect when it is not 30 s ("@n").
AR assault rifle, PR plasma rifle, PP plasma pistol, RL rocket launcher,
SG shotgun, SR sniper rifle. "camo/over" is
`powerups/shield-invisibility` (50:50 camouflage or over shield).

| Map | Entries | Weapons | Grenades | Powerups |
|---|---|---|---|---|
| beavercreek | 35 | AR 2, PR 2, RL 1 @120, SR 2, needler 3 | frag 8, plasma 8 @180 | camo 3 @60, health 4, overshield 2 @60 |
| bloodgulch | 35 | AR 2, PR 2, RL 1 @90, SG 4, SR 2 @120, pistol 4 | frag 8, plasma 8 @180 | camo/over 1 @180, health 2, overshield 1 @60 |
| boardingaction | 52 | AR 2, PR 3, RL 2 @120 + 2 @180, SG 2, SR 2, needler 4 | frag 8 @180, plasma 16 @180 | camo 2 @60, health 6 + 1 @180, overshield 2 @60 |
| carousel | 37 | AR 1, RL 1 @120, SG 2, SR 3, flamethrower 1, needler 2, pistol 4 | frag 8, plasma 8 @180 | camo 3 @60, health 1, overshield 3 @60 |
| chillout | 33 | AR 1, PR 2, RL 1 @120, SG 1, SR 1, needler 2, pistol 2 | frag 8, plasma 8 @180 | camo 2 @120, health 4, overshield 1 @60 |
| damnation | 46 | AR 4, PP 1, PR 3, RL 1 @120, SG 4, SR 2, pistol 1 | frag 16, plasma 8 @180 | camo 1 @60, health 4, overshield 1 @60 |
| hangemhigh | 45 | AR 2, PR 2, RL 1 @120, SG 4, SR 3, needler 3, pistol 2 | frag 16, plasma 8 | camo 1 @60 (levitate), camo/over 1 @180, health 2 |
| longest | 40 | AR 5, PR 1, SG 2, pistol 2 | frag 16, plasma 8 @180 | camo 1 @60, health 4, overshield 1 @60 |
| prisoner | 51 | AR 2, PR 3, RL 1 @120, SG 4, SR 4 | frag 12, plasma 16 @180 | camo 1 @60, health 5, overshield 3 @60 |
| putput | 40 | AR 1, PP 4, PR 1, RL 1, SG 1, pistol 2 (all @45) | frag 8, plasma 8 (@45) | all-powerups 1 @45, health 13 @45 |
| ratrace | 30 | AR 2, PR 1, SG 3, pistol 4 | frag 8, plasma 8 @180 | camo 1 @90, health 2, overshield 1 @60 |
| sidewinder | 47 | AR 2, PR 2, RL 2 @120, SG 4, SR 2, needler 4, pistol 2 | frag 16, plasma 8 @180 | camo 1 @60, health 2, overshield 2 @60 |
| wizard | 44 | AR 3, PR 3, RL 2 @120, SG 2, needler 2, pistol 6 | frag 8, plasma 8 @180 | camo 2 @60, camo/over 2 @180, health 4, overshield 2 @60 |

The entries are filtered by their own game type slots (`scenario.md`);
the number of entries eligible per base type (CTF, slayer, oddball,
king, race), from the disc: beavercreek 32, 30, 30, 28, 28;
bloodgulch 35 each; boardingaction 52, 50, 50, 50, 50; carousel 13,
30, 30, 29, 29; chillout 32 each; damnation 45, 40, 40, 40, 40;
hangemhigh 43 each; longest 28, 26, 26, 26, 26; prisoner 35, 29, 29,
29, 29; putput 40 each; ratrace 29, 26, 26, 26, 26; sidewinder 47
each; wizard 32, 30, 30, 30, 30. (Carousel has few capture the flag
items because most of its entries name types 13 or 2, 3, 4, 5.) Blood
Gulch's every entry is in `scenario.md`.

### 11.4 Powerup effects used here (disc, `objects.md`)

| Tag | Powerup | Time |
|---|---|---|
| `powerups\active camouflage` | 3 | 45 s |
| `powerups\over shield` | 2 | 60 s |
| `powerups\health pack` | 5 | 1 |
| `powerups\double speed` | 1 | 60 s (no map places it) |
| `powerups\full-spectrum vision` | 4 | 60 s (none placed) |

## 12. Checks run

- **Executable.** The 26 stock builders were found through the table
  of their addresses in the executable's data, decoded with a small
  interpreter of the instructions they use, and all fields compared with
  the reference's builders: every field equal, with the Team Ball
  exception (section 2). All values lie in the cleaned ranges (base
  1..5, indicator 0..2, weapon set 0, 4 or 6, vehicle set 1, 2 or 4,
  ball type 0..2, traits 0..3, speed 0..2, race type 0..2, health 1.0
  or 2.0, ball count 1 or 16).
- **Menus.** The 26 names, 36 descriptions and every option list were
  read from `ui.map`; the setup handlers' writes match the layout of 1.1
  offset for offset.
- **Scenario data, all 13 maps.** Every one of the 535 netgame
  equipment entries names an item collection that exists, has class
  `itmc`, a positive total weight and permutations that resolve to
  `weap` or `eqip` tags (16 collections); every starting equipment
  reference exists; teleporter pairs are one-to-one; eligible starts were
  counted with the matching rule of 4.3.
- **Globals.** The multiplayer information block, grenades, weapon list
  and sound list were read from every map and are byte-for-byte equal in
  content; the 43 announcer tags exist; their stored lengths are equal
  on three maps compared.
- **Tags present.** The 14 weapon list tags and the three powerup
  tags exist on all 13 maps.
- **Not checked on the disc:** the spawn rating, respawn rules, item
  clock, substitution tables, ammunition and scoring rules (read from
  the reference; the executable was not disassembled for them).

## Edge cases

- A start eligible by type but with team index 2 or 3 is never used by
  capture the flag, always used (team ignored) elsewhere.
- Two players choosing starts on the same tick: the second sees the
  first's new unit, so they do not stack (distance rule), unless the map
  has one usable start.
- A dead player in a game with `lives` whose last life is used never
  respawns; the game ends when only one team remains with units or
  lives.
- A respawn with no usable start retries only on staggered ticks.
- A pickup of a grenade at 4 is refused; of a powerup the effect
  refuses, refused: the item stays and the entry's next period still
  creates a new one (two items stand there for one tick; the first is
  deleted the tick after the second appears).
- A weapon set can turn an entry's item into any weapon of the
  globals' list: all 14 tags of that list, and the three powerups, are
  present on every one of the 13 maps (checked), so a substitution
  never needs a tag the map lacks.
- Free for all with 2 players: the team score of each is their own.
- A flag carrier who is teleported keeps the flag.

## Quirks

- **The respawn clock is global**, not a delay after pickup (keep).
- **Spawn stagger**: a respawn is up to 31 ticks late; keep.
- **Team Ball's flags are undefined** on the disc (use 35).
- **A suicide scores -1** and **being betrayed adds the suicide
  penalty** to the victim's wait.
- **Slayer scoring does not stop at the end** of the game; other types
  stop.
- **Contested hills score** for both teams.
- **Multi-kill time wraps** at game tick 32768 if the 16-bit store is
  real (open question; a rebuild should use a full-width time).
- **The weapon set changes the unit's own pistol and rifle**, so a
  "rockets" game spawns with one rocket launcher (the second is
  discarded as a duplicate), and a custom equipment entry's first
  weapon replaces them.
- **Credit with no killer** uses the player slot as a team number
  (wrong in team games).
- **The 5-player and 9-player grenade rules** thin grenade pickups and
  cap starting grenades with the number of players at the game start,
  and the Xbox's infinite grenades did not apply in such games (the
  reference's port comment on it; open question 4).

## Test cases

### Spawn eligibility, Blood Gulch (starts numbered as in `scenario.md`)

| Start | Position (x, y, z) | Team | Game type slot 1 | Slayer | Team Slayer | CTF red | CTF blue | Race |
|---|---|---|---|---|---|---|---|---|
| 0 | (98.49, -157.64, 1.70) | 0 | 1 | no | no | yes | no | no |
| 6 | (43.91, -80.68, 1.70) | 1 | 1 | no | no | no | yes | no |
| 36 | (42.91, -67.76, 0.52) | 0 | 14 | yes | yes (either team) | no | no | no |
| 44 | (98.66, -149.65, 0.08) | 1 | 14 | yes | yes (either team) | no | no | no |
| 32 | (26.04, -65.91, 1.16) | 0 | 5 | no | no | no | no | yes |

Counts: Slayer 16, Team Slayer 16, oddball 16, king 16, race 17, CTF
red 17 and CTF blue 21 (all 38 pass the flag-distance check).

### Spawn choice

Free for all, player at slot 0 (enemies slots 1 and 2): enemy B at
(40.0, -70.0, 0.3), enemy C at (30.0, -80.0, 0.2). Candidate starts
36, 37, 38 of Blood Gulch.

| Start | d to B | d to C | Distance rating |
|---|---|---|---|
| 36 (42.91, -67.76, 0.52) | 3.6789 | 17.7929 | (3.6789 - 2) / 3 = 0.55962 |
| 37 (29.27, -81.39, 0.17) | 15.6487 | 1.5703 | 0 (an enemy within 2 WU) |
| 38 (41.10, -90.28, 0.18) | 20.3102 | 15.1291 | 1.0 |

Random draws (u36, u37, u38) = (0.81, 0.90, 0.16): scores 0.55962 x
0.9 = 0.50366; 0; 1.0 x 0.4 = 0.4: **start 36**. Draws (0.16, 0.90,
0.81): 0.22385; 0; 0.9: **start 38**.

Team Slayer, player on team 0, the same enemies on team 1, and a
living teammate F at (42.0, -68.5, 0.3): the distance from F to start 36
is 1.1934, so its bonus = 3 x (1 - 0.2 x 0.1934)^0.6 + 1 = 3 x 0.97661
+ 1 = 3.92984, and the rating of 36 is 0.55962 x 3.92984 = 2.19923;
start 37 is still 0; start 38 is 1.0 (F is 18 WU away, bonus 1).
Draws (0.64, 0.25, 0.04): 2.19923 x 0.8 = 1.75938; 0; 0.2: **start 36**.
Draws (0.04, 0.25, 0.81): 0.43985; 0; 0.9: **start 38**.

More: an enemy exactly 2.0 WU from a start: rating 0; at 5.0: 1.0;
at 0.5: 0 (0.1 then < 2). A teammate 0.2 WU away in a team game:
rating 0; 0.5 WU away: no effect. A warthog 0.05 WU from the start:
rating 0, 0.15 WU away: no effect. No eligible start with rating > 0
(for example one start and an enemy standing on it): no spawn, timer 1,
try again at the next tick with game time = slot (mod 32).

### Respawn timelines (ticks of waiting after the unit is gone)

| Variant, death | Computation | Wait |
|---|---|---|
| Slayer, killed by an enemy, first death | 0 + 0; min 90 | 90 (3.0 s) |
| Team Slayer, killed by an enemy | 300 | 300 (10 s) |
| Slayer, suicide | 0 + 0 + 300 | 300 |
| Team Slayer, killed by a teammate, no enemy damage in the last 6 s | 300 + 300 | 600 (victim waits; killer score -1) |
| Slayer, fell, no enemy damage in the last 6 s | 0 + 300 | 300 |
| Slayer, fell 4 s after an enemy shot the player | credited to the enemy: a kill | 90; enemy +1 |
| Snipers (respawn 0, growth 150, suicide 300), enemy kills, no kills of its own: deaths 1 to 8 | P = 0, 150, 300, 450, 600, 750, 750, 750 then min 90 | 90, 150, 300, 450, 600, 750, 750, 750 |
| the same, but after death 4 the player kills an enemy | P_4 would be 600; kill: P = 450 | death 5: 450 (not 600) |
| Snipers, suicide at death 3 (P = 300) | 300 + 300 | 600 |
| Endurance (lives 5), the 5th death | | never respawns; the game ends when one team has no one left |

Stagger example: slot 5, timer reaches 0 on tick 1000 (1000 mod 32 =
8): the next tick with number mod 32 = 5 is 1029, so the player spawns
there (29 ticks later). At game time 3 or less the player spawns at
once regardless.

### Item respawn on Hang 'Em High (entry numbers as in `scenario.md`)

| Case | Result |
|---|---|
| entry 0 (assault rifle, period 30 s = 900 ticks), taken at tick 100 | next item at tick 900 (26.7 s later) |
| entry 0, taken at tick 901 | next item at tick 1800 |
| entry 0, never taken | the item made at 0 is deleted at tick 901 (so two items stand at tick 900); the one made at 900 is deleted at tick 1801 |
| entry 8 (`powerup invisibility`, 60 s = 1800 ticks, levitate), taken at tick 1000 | next at tick 1800 |
| entry 8, taken at tick 1801 | next at tick 3600 (59.97 s) |
| entry 9 (`shield-invisibility`, 180 s = 5400 ticks): draws | camouflage with probability 0.5, over shield 0.5, at tick 0, 5400, 10800 |
| Blood Gulch entry 21 (rocket launcher, own spawn time 90, collection 120) | period 90 s = 2700 ticks (the entry's own field wins) |
| Blood Gulch entry 24 (`powerup super shield`, own 0, collection 60) | period 60 s = 1800 ticks |
| picking up camouflage (45 s = 1350 ticks) at tick 2000 while holding none | camouflage lasts until about tick 3350 (timers count in the after-game step) |
| picking up another camouflage 600 ticks later | refused (still camouflaged); the item stays |

### Ammunition from a duplicate weapon

| Held (spare) | Ground weapon (spare) | After |
|---|---|---|
| assault rifle, spare 100 | assault rifle, spare 180 (a fresh map one) | held spare 280, ground weapon deleted |
| assault rifle, spare 500 | assault rifle, spare 180 | held spare 600, ground spare 80, ground weapon stays |
| assault rifle, spare 600 | assault rifle, spare 180 | nothing taken |
| pistol, spare 100 | pistol, spare 48 (a fresh map one) | needs 20, takes 20: held 120, ground 28 |
| shotgun, spare 50 | shotgun, spare 12 (new) | take 10: held 60, ground 2 |

A ground assault rifle that holds loaded rounds loses them when its
spare rounds go to 0 (the weapon is deleted).

### Starting equipment

| Setup | Expected |
|---|---|
| Slayer (stock, generic bit clear) on Blood Gulch, 4 players | one plasma pistol, 0 frag, 0 plasma grenades |
| Slayer Pro (generic) on Blood Gulch, 4 players | assault rifle (60 + 180), pistol (12 + 48), 4 frag, 0 plasma |
| Slayer Pro, 6 players | the same, 2 frag |
| Slayer Pro, 12 players | the same, 1 frag |
| CTF (stock) on Blood Gulch | assault rifle then pistol, 4 frag |
| CTF on Hang 'Em High | plasma rifle, 0 frag, 4 plasma (flag 2) |
| Rockets (weapon set 6, generic) | one rocket launcher (2 + 2), 4 frag |
| Snipers (weapon set 4, generic) | the rifle becomes a sniper rifle (4 + 8) and the pistol stays: sniper rifle and pistol |
| any game on Rat Race, custom | assault rifle and pistol, 4 frag |

### Scoring and multi-kills

| Case | Result |
|---|---|
| Slayer, player A kills enemy B | A +1 (team and player), B +0, speeds: A 0.98, B 1.1 |
| A at speed 1.1 kills again | 1.1 - 0.02 = 1.08, >= 1.0 so - 0.15 = 0.93, raised to 1.0 |
| A teammate kills teammate in Team Slayer | killer team and player -1 |
| A player suicides | -1 |
| Kill in order: A's target is B; A kills C | no score change |
| A kills B (target) | +1, a new target is drawn among living enemies other than B |
| kills at ticks 1000, 1100, 1250, 1300 by one player | counts 1, 2 (double kill), 1, 2 (double kill) |
| kills at 1100 and 1220 | 1220 - 1100 = 120: counts 1, 2 (inside) |
| kills at 1100 and 1221 | counts 1, 1 (outside) |
| a fifth kill without dying | "killing spree"; tenth: "running riot"; fifteenth: "running riot" |
| a fifth kill that is also a double kill | "double kill" (the multi-kill is chosen first) |
| the killer dies, then kills | kills in a row restart at 1 |
| Slayer: team 0 has 49, a kill makes 50 (score to win 50) | game over at the next player update; 7 s later everyone is killed |

### Capture the flag (Blood Gulch, red flag spot at (95.69, -159.45, -0.10))

| Case | Result |
|---|---|
| a red player carries the blue flag to (96.29, -159.45, -0.10) | distance 0.6 < 1.0: red +1, flag returns to the blue spot |
| the same at (96.69, -159.45, -0.10) | distance 1.0, not less: no capture |
| red player carries the blue flag with the red flag taken (flag at home to score on) | no capture; flag failure sound |
| a dropped blue flag at tick L (last held tick) | returns at the first update with now - L > 510 (tick L + 511) |
| flag must reset on, red touches its dropped flag | nothing happens; failure sound; returns at L + 511 |
| capture with score to win 3 | the third capture ends the game |

### Radar

| Case | Shown |
|---|---|
| enemy 15 WU away, standing still | no |
| enemy 15 WU away, walking (0.5 WU/s) | yes (0.5 WU/s = 0.0167 WU/tick, squared 0.00028 >= 0.0002) |
| enemy 15 WU away, crouch-walking at 0.3 WU/s | no (0.01 WU/tick, squared 0.0001) |
| enemy 15 WU away, standing and firing | yes |
| enemy 25 WU away, firing | no |
| enemy 10 WU away horizontally and 30 WU above | yes (height ignored) |
| flag bit 0 clear | no unit except the viewer's |

## Confidence per section

| Section | Level |
|---|---|
| 1.1 layout, offsets, enumerations, flag bits | checked against the disc (executable builders and menu handlers) |
| 1.4 cleaning, 1.6 PC options | reference only |
| 1.5 menu choices | checked against the disc |
| 2 stock game types | checked against the disc (executable); Team Ball flags inferred |
| 3 teams | reference; colours checked (`objects.md`) |
| 4.1, 4.2 respawn timer and stagger | reference only |
| 4.3 eligibility | disc data, rule from the reference (`scenario.md`) |
| 4.4 spawn rating | reference only |
| 4.5 position, facing, no protection | reference; absence of protection inferred |
| 4.6 starting equipment | disc data (multiplayer unit's weapons, scenario entries, globals' grenades), rules from the reference |
| 5.1 item clock | reference only (the disc fixes the data) |
| 5.2 collections | checked against the disc (weights), draw from the reference |
| 5.3 weapon sets | reference only; the weapon list checked against the disc |
| 5.4, 5.5 pickups | reference only; ammunition maxima checked against the disc (`weapons.md`) |
| 6.1 to 6.3 slayer and kill bookkeeping | reference only |
| 6.4 to 6.7 other game types | reference only; counts and flags checked against the disc |
| 6.8 teleporters | reference only (the flags and pairs checked against the disc, `scenario.md`); the exit volume's numbers follow from the biped's pill (`movement.md`) |
| 7 radar | the three numbers checked against the disc; rule from the reference |
| 8 vehicles | placements checked against the disc; no respawn from the reference |
| 9 announcer | list and lengths checked against the disc; queue from the reference |
| 11, 12 | checked against the disc |

## Open questions

1. **Spawn rating and the clock.** All of 4.1 to 4.4 come from a
   different build. Measure on a live game: the spawn stagger (record
   spawn ticks of two players with different slots; expected tick mod 32
   = slot mod 32), the minimum 3 s wait with instant respawn, and a
   start with an enemy at 1.5 and 2.5 WU. Also confirm that nothing
   tests line of sight.
2. **The first tick of the match.** Whether items first appear at game
   time 0 and what the spawn clock is for a late map load. Time the
   first pickup on a map with a known spawn time.
3. **The 5-player and 9-player grenade thresholds** and their
   probabilities (0.55, 0.3): count grenades at spawn with 1, 5 and 9
   players, and count grenade items over a minute on Blood Gulch.
4. **Infinite grenades with 5 or more players** (the reference port's
   comment says the Xbox ignored the setting): test with 6 players.
5. **Weapon set table and the flamethrower rule.** Only sets 0, 4 and 6
   appear in the stock types; check the table with sets 1, 2, 3, 5, 7, 8
   and 9 and see whether Carousel's flamethrower is a rocket launcher.
6. **Contested hills scoring** (both teams score) and the 16-bit
   multi-kill time: play past 18 minutes and check whether a double
   kill is still announced.
7. **Vehicle respawn.** None found; check whether a destroyed Blood
   Gulch warthog comes back.
8. **Kill credit with no killer** (the slot-as-team comparison):
   knock an enemy off a cliff in a team game and in free for all.
9. **Team Ball's flags**: read the saved variant after first launch on
   a console or emulator: motion-sensor on, friend indicators, generic
   equipment?
10. **Respawn countdown start.** Whether the first decrement is on the
    tick of the death or the next one: record a death and the respawn
    frame at 60 fps (expected 90 ticks + 0..31 stagger + 1).
11. **Powerup stacking.** Whether a second overshield or camouflage
    adds duration (multiplayer adds in the reference) and whether it is
    refused while the effect is active; and when the over shield's
    double charge refuses (`health_shields.md`).
12. **Walk-over pickup of a second weapon** (reference port adds it;
    the Xbox rule is the action hold): spawn with one weapon, walk over
    another.
13. **Friendly fire on the Xbox disc** has no option; confirm there is
    no damage reduction between teammates in team slayer.
14. **Teleporter exit volume.** The exit test puts the arriving pill's
    bottom-sphere centre on the flag itself, 0.2 WU lower than the
    feet will be. Check: stand a second player 0.3 WU and then 0.5 WU
    from the target flag of a pair and walk into the source: expected
    blocked at 0.3, not at 0.5; and that a corpse at the exit does not
    block.
