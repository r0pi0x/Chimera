# Team Fortress 2: voices — pain, death and kill lines

Source basis: the SDK's TF2 game code (Valve's Source SDK 2013 with
TF2's client and server code, read in place on the Linux box, nothing
copied): how the server reacts to damage and death with sounds, how a
player asks the response-rule system for a line (the criteria it adds,
the gates on speaking, the per-class rule pools), the shared
response-rule system (criterion matching, rule scoring, response groups,
contexts), the instanced scene playback and its speak events, the
recipient filters and the sound-script emission path, the burning and
stun code paths, and the kill bookkeeping behind "recent kills" and
dominations. The retail client install (app 440, PatchVersion 11191711),
read in place with a throwaway reader (nothing extracted kept): the
response rules (`scripts/talker/response_rules.txt` and the per-class
files it includes), the sound scripts `scripts/game_sounds_player.txt`,
`scripts/game_sounds_vo.txt` and `scripts/game_sounds.txt`, and the
compiled scenes in `scenes/scenes.image` (durations, speak events). The
per-class script files (`scripts/playerclasses/*.ctx`) are encrypted and
were not read. The sound archives are absent on this box, so no wave
length was measured. No game was launched.
Status: draft (checked against the SDK's game code and the game's data,
2026-11-18)

## Summary

Every TF2 voice reaction is server-driven. A **hurt** player (health
actually changed, alive, not disguised) asks the response rules for a
pain line: everyone near him except the attacker hears a short
`<Class>.PainSharpNN`, the attacker alone hears a louder-sounding
`<Class>.PainSevereNN`, and a crit replaces the victim's own copy with
the crit "ding" `TFPlayer.CritPain`. The next pain line waits until the
chosen pain **scene** has run out (0.32 s for a Scout's shortest, a flat
4.0 s for every Pyro scene, about 5 s for every Spy scene). Fall damage
plays the class's death cry instead; burning plays no pain but a
separate "I'm on fire" line every 2.5 s. A **death** plays one of four
class death entries (explosion, crit, melee, generic) at 95 dB on the
voice channel, which cuts whatever the dead player was saying; a fatal
fall plays a flesh splat instead and a "silent killer" weapon plays
nothing. A **kill** makes the killer ask for `TLK_KILLED_PLAYER`; most
kills say nothing: lines come from multi-kills within 30 s (30 % or
50 %), melee kills (30 %), Sniper headshots, sentry kills, and always on
dominations and revenges (2.5 s later). Each line sets a per-player
context that silences that family for 5–30 s, and a player who is still
"speaking" (inside his last scene's length) cannot start a kill line at
all.

## Units and conventions

- Times in seconds, server time. TF2's server runs at 66⅔ ticks per
  second (tick 0.015 s); delays count down in server frames.
- Distances in Hammer units (u).
- "Sound level" is the Source dB-like number of a sound entry (75 =
  `SNDLVL_NORM`, 95 = `SNDLVL_95dB`); emission, channels, the
  attenuation filter and replacement on (entity, channel) are as in
  [../cs_source/sounds.md](../cs_source/sounds.md) (same engine).
- Names in `code` are game data: sound entries, concepts, criterion,
  rule, response and context names from the rule files, file paths,
  console variables. Matching of all of them is case-insensitive.
- "Class" means the speaking player's real class (Scout, Soldier, Pyro,
  Demoman, Heavy, Engineer, Medic, Sniper, Spy, spelled like that in the
  criteria).
- r is the request's random number, a uniform integer in [0, 100]
  (101 values), drawn once per speech request.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| pain concept | `TLK_PLAYER_PAIN` | – | line the victim plays to everyone but the attacker |
| attacker-pain concept | `TLK_PLAYER_ATTACKER_PAIN` | – | line the victim plays to the attacker only |
| on-fire concept | `TLK_ONFIRE` | – | burning line |
| kill concept | `TLK_KILLED_PLAYER` | – | killer's line |
| death concept | `TLK_DIED` | – | asked on death; no rule exists, so silent |
| on-fire interval | 2.5 | s | gap between on-fire requests while burning |
| stun pain block | 2.0 | s | no pain line for this long after a stun sound |
| recent-kill window | 30 (strict <) | s | `recentkills` counts kills newer than this |
| concept/modifier weight | 5 | – | weight of the concept and of caller-given criteria |
| other criteria weight | 1 | – | weight of every criterion the player adds itself |
| minimum rule score | 0.001 | – | a rule must score at least this to be chosen |
| scene speech radius | 2500 | u | speak events reach only players whose ear is within this of the speaker's centre |
| 95 dB radius | 4500 | u | attenuation-filter cut-off for 95 dB entries (death cries, emitted directly) |
| 75 dB radius | 2500 | u | cut-off for 75 dB entries (`Player.FallGib`, `TFPlayer.Drown`) |
| empty-duration speaking time | 3 | s | speaking window when a speech scene reports length 0 (does not occur with the shipped data) |

Chance criteria (from `response_rules.txt`) against r ∈ [0, 100]:

| Criterion | Pattern | Passes for r in | Probability |
|---|---|---|---|
| `2PercentChance` | `=>98` | 98–100 | 3/101 = 2.97 % |
| `5PercentChance` | `=>95` | 95–100 | 6/101 = 5.94 % |
| `10PercentChance` | `>90` | 91–100 | 10/101 = 9.90 % |
| `20PercentChance` | `>80` | 81–100 | 19.80 % |
| `30PercentChance` | `>70` | 71–100 | 29.70 % |
| `40PercentChance` | `>60` | 61–100 | 39.60 % |
| `50PercentChance` | `>50` | 51–100 | 49.50 % |
| `75PercentChance` | `>25` | 26–100 | 74.26 % |
| `100PercentChance` | `>0` (weight 100) | 1–100 | 99.01 % |

## Behavior

### 1. How a player gets a line (the response rules)

Pain, attacker pain, on-fire and kill lines all go through one
mechanism. A request names a concept and may carry extra "modifier"
criteria and a recipient list.

#### 1.1 Rule pools

At map load the server loads `scripts/talker/response_rules.txt` with its
includes (in order: `talker/tf.txt`, the nine class files, `talker/events.txt`,
then the nine `<class>_auto.txt` files; the include names use mixed case,
the archive stores lower case). It then builds one **pool per (class,
concept)**: a rule goes into the pool of class C and concept K when its
criteria contain one criterion whose key is `playerclass` and whose
pattern text is exactly C (e.g. `IsScout`: `playerclass` `Scout`) and one
whose key is `Concept` and pattern text exactly K. Negated or other
forms (`!=Scout`) do not count. Each pool has its **own copy** of the
response groups its rules use, so the "no repeat" state in §1.4 is per
class and concept and shared by every player of that class on the
server.

A request is answered from the pool of the speaker's class (a disguised
Spy speaking to enemies uses his disguise class's pool, §2.6).

File grammar, as far as the shipped files use it: `//` comments;
`#include "file"`; `enumeration name { … }`; `criterion "Name" "key"
"pattern" ["required"] [weight N]` (default weight 1, not required; a
second definition of a name is ignored); `response Name { [permitrepeats]
[sequential] [norepeat] then lines "type" "value" [weight W]
[predelay "a[, b]"] … }` with types `scene`, `speak`, `sentence`,
`response`, `print`; `rule Name { criteria A B C …; ApplyContext
"name:value:seconds" (may repeat); applycontexttoworld; Response G
(may repeat or list several); matchonce }`. A rule that names an
undefined criterion or response is dropped. The only response
modifiers the shipped files use are `predelay` (2065 times) and
`applycontexttoworld` (26 rules); every other modifier listed above is
unused and need not be supported.

#### 1.2 The criteria set of a request

Keys are case-insensitive; a later value for the same key replaces the
earlier one. In order:

1. `concept` = the concept name, weight 5.
2. The caller's modifiers, each weight 5 (pain: `damagecritical` = `1`,
   always; kill: §4.1).
3. Weight 1 each: `randomnum` = r; `map`; `classname`; `name`;
   `health`; `healthfrac` (three decimals); global states; the
   speaker's **contexts** (name = value, expired ones removed first); the
   world's contexts with the prefix `world` (e.g. `worldDontKilledPlayer`);
   `playerhealth`; `playerhealthfrac` (health / max health, three
   decimals, after the damage that caused the request); `playerweapon`
   (active weapon's class name, e.g. `tf_weapon_scattergun`, or `none`);
   `playeractivity`; `playerspeed`; the contexts again with prefix
   `player`.
4. Weight 1, TF additions: `playerclass` (real class; the disguise class
   when a `disguiseclass` modifier is present); `OnRedTeam`;
   `recentkills` (kills in the last 30 s, §4.1); `killsthislife` (kills
   + assists + buildings destroyed this life); `disguised`, `cloaked`,
   `invulnerable`, `beinghealed` (receiving healing), `stunned`,
   `snared`, `dodging`, `doublejumping`, `waitingforplayers` (`1`/`0`);
   `teamrole` (`offense`/`defense`, when the team has a role);
   `weaponmode` of the active weapon (`primary`, `secondary`, `melee`,
   `building`, `pda`, `item1`, `item2`); `sniperzoomed` = `1` only while a
   sniper rifle is zoomed; `minigunfiretime` (one decimal) with a
   minigun out; `item_name` and `item_type_name` of the active weapon
   **only when it is not of stock ("normal") quality** (e.g. `The Boston
   basher`, `#TF_Weapon_CompoundBow`); `loadout_slot_<slot>` = the item
   definition name for every non-stock equipped item; `crosshair_on` (class
   of the player under the crosshair, with a cloaked enemy hidden unless
   seen in the last 3 s and a disguised enemy shown as his disguise) and
   `crosshair_enemy` = `yes`; round-state keys (`LostRound`,
   `PrevRoundWasTie`, `OnFriendlyControlPoint`, `OnCappableControlPoint`,
   `GameRound`, `OnWinningTeam`, `RoundsPlayed`, `IsComp6v6`,
   `IsCompWinner`) and holiday/cosmetic flags (`IsRobotCostume`,
   `IsUnicornHead`, … each `1` when the cosmetic is worn).

#### 1.3 Matching and scoring

A criterion's pattern is read character by character: `>` makes it a
lower bound, `<` an upper bound, `=` together with either makes that
bound inclusive, `!` makes it "not equal", and a comma separates two
parts (`>4.0,<=8.0`). Then, with v = the set's value as a number
(non-numeric text counts as 0; a missing key is the empty text ""):

- bounds: pass when v satisfies every bound given;
- "not equal": if the pattern text is a number, pass when v ≠ it, else
  pass when the texts differ (case-insensitive). A missing key passes
  `!=1` (v = 0);
- plain: if the pattern text is a number, pass when the value is
  non-empty and v equals it; else case-insensitive text equality. A
  missing key fails `1`.

Rule score = sum over its criteria of (weight of that key in the set,
or 1 if the key is missing) × (the criterion's weight) for every
criterion that passes. If a required criterion fails the rule is out
(every criterion in the TF files is required). The highest-scoring rule
with score ≥ 0.001 wins; ties are broken uniformly at random. No rule:
no line.

Useful consequences with the shipped weights: the concept is worth 5,
`playerclass` 1, a caller modifier (`domination`, `customdeath`,
`damagecritical`) 5, a plain player criterion 1, a chance 1, contexts
and `victimclass` 0 (they are declared weight 0), `item_name` /
`item_type_name` / `loadout_slot_*` criteria 10 (also the negated ones:
a missing `item_name` passes `!=The Candy Cane` and adds 10), cosmetic
flags 100.

#### 1.4 Picking the line

The winning rule picks one of its response groups uniformly at random.
Within the group, lines whose scene is missing from `scenes.image` are
never picked. The others are picked uniformly at random (all shipped
weights are 1) **without repeating** until every line of the group has
been used once in the current cycle; then a new cycle starts (so a line
can repeat across a cycle boundary). The cycle state belongs to the
group copy in that class-and-concept pool (§1.1).

The group's `predelay` is a delay before the scene starts: one number,
or "a, b" for a uniform draw in [a, b]. It belongs to the group, not the
line (the last `predelay` written in the group wins; in the shipped
data every line of a group carries the same value).

#### 1.5 Contexts (the cooldowns)

When the line actually starts (the scene was created; §1.6), the rule's
`ApplyContext` strings are applied: each `name:value:seconds` sets the
speaker's context `name` to `value` until now + seconds (overwriting an
existing one with the same name, expiry and all). With
`applycontexttoworld` they go to the world instead, where players read
them as `world<name>`. Contexts persist through death, respawn and
class change; they only expire. Kill rules gate on their own contexts
(`ScoutNotKillSpeech` = `ScoutKillSpeech` `!=1`, …), which is the only
"respeak delay" TF uses: the engine's own respeak, speak-once and odds
features are not used by players.

#### 1.6 Playing a line

- A scene is created for the speaker. After its predelay has counted
  down (server frames), it plays; each speak event emits its sound
  entry at the event's start time (0 s into the scene for every pain and
  kill scene checked).
- Recipients of a speak event: players in the potentially-audible set of
  the speaker's centre whose ear (eye position) is within 2500 u of it,
  further limited to the caller's recipient list if the request had one.
  Each recipient is sent the sound separately, on the speaker entity.
- Sound parameters are the sound entry's own (channel, volume, sound
  level, pitch), not the scene's. Every class voice entry used here is
  `CHAN_VOICE`, `SNDLVL_95dB`, `PITCH_NORM`; volume 1.0 for pain, death,
  on-fire and `Domination<Victim>` entries, 0.82 or 0.62 for most laugh,
  taunt, cheer and `SpecialCompleted` entries (per entry, read it from
  the script). Clients then adjust voice-channel pitch: ×1.3 for other
  players' voices when the local player sees through Pyrovision, and the
  `voice_pitch_scale` item attribute.
- **Speaking window**: if the scene has at least one speak event, the
  speaker counts as speaking from the request until request time + the
  scene's length (the `scenes.image` duration; the predelay is **not**
  added). Scenes without speech (the Engineer's and Spy's silent "attack"
  face, §4.4) do not open a window.
- A scene keeps running if its speaker dies; it is removed only if the
  speaker's entity goes away (disconnect). Nothing stops a scene on
  death, respawn or class change.

#### 1.7 Gates on a request

- The speaker must be alive (a dying player's requests fail).
- **Normal requests** (kill, on-fire): refused, not queued, while the
  speaker is in his speaking window or has already had a request
  accepted in this same server frame.
- **Pain requests** (pain and attacker pain, and the stun sound) are
  made in a "multiple scenes allowed" mode: the speaking check is
  skipped, a playing scene is not interrupted, and the pain scene plays
  alongside it. They still set the speaking window (each overwrites
  it: after a pain step the window is the length of the **last** line
  started, normally the attacker-pain one).
- The `TLK_DIED` request at death skips the speaking check but finds no
  rule.

### 2. Pain

#### 2.1 When the pain step runs

Once per damage event, on the server, after the damage has been applied,
if the victim's health differs from his health before the event and the
victim is still alive. A killing hit has already made the victim dying,
so **the killing hit plays no pain line** (the death sound covers it).
Damage that changes nothing (ubercharge, Bonk, absorbed, same team with
friendly fire off, zero damage) plays nothing.

#### 2.2 Order of checks

1. Victim dead or dying: nothing.
2. Victim disguised (Spy in disguise, either team's): nothing.
3. Now < the victim's next-pain time: nothing.
4. Fall damage: unless the victim landed on a player while able to
   fall-stomp (Mantreads-type boots), play the class's generic death
   entry `<Class>.Death` (§3.2) directly (not a scene; to everyone in
   range, 95 dB, 4500 u, the victim and any attacker included). Stop;
   the next-pain time is **not** changed. (The engine's own landing sound
   `Player.FallDamage`, `CHAN_STATIC`, also plays for any fall damage.)
5. Damage with no type at all, or only the "no physics push" type:
   nothing.
6. Drowning damage: `TFPlayer.Drown` directly (`CHAN_VOICE`, 75 dB,
   volume 1, pitch 95–105, one of `player/drown1.wav`–`drown3.wav`;
   2500 u). Stop; next-pain time unchanged.
7. Burn damage (afterburn ticks and flame damage carry it): nothing here
   (§2.7).
8. Otherwise the normal pain step (§2.3).

#### 2.3 Normal pain step

Let A be the attacker if it is a player.

1. Recipient list P = everyone in the potentially-audible set of the
   victim's origin, minus A.
2. If the damage carries the crit flag (full crits; mini-crits are
   assumed not to, see Open questions): play `TFPlayer.CritPain` to the
   victim only (`CHAN_STATIC`, 75 dB, volume 1, pitch 95–105, one of
   `player/crit_received1.wav`–`3.wav`) and remove the victim from P.
   gap = that wave's length.
3. Request `TLK_PLAYER_PAIN` (modifier `damagecritical` = `1`, always,
   crit or not) with recipients P. If a line started, gap = max(gap,
   that **scene's length**).
4. If A exists, request `TLK_PLAYER_ATTACKER_PAIN` (same modifier) with
   recipients {A}. Its length does not count toward the gap.
5. Next-pain time = now + gap (gap 0 when nothing played: the next
   damage may try again at once).

The speak events further restrict recipients to the 2500 u radius
(§1.6): an attacker farther than 2500 u from the victim hears no pain at
all. Self-damage (rocket or sticky jump, own Caber): A is the victim, so
the victim does not hear his PainSharp line but hears the attacker-pain
line meant for A; everyone else hears the PainSharp line. Damage from
the world or a non-player (trigger, map hazard): no attacker line;
everyone in range, the victim included, hears the pain line.

#### 2.4 The pain rules (all classes)

Every class has `PlayerPain<Class>` (concept `TLK_PLAYER_PAIN`) and
`PlayerAttackerPain<Class>` (`TLK_PLAYER_ATTACKER_PAIN`), each needing
only the class and `IsNotDominating` (context `IsDominating` not 1;
weight 0), so both score 6. One scene per line, one sound entry per
scene, no predelay, no context.

| Class | Pain (to all but attacker): entries, scene lengths (s) | Attacker pain: entries, scene lengths (s) |
|---|---|---|
| Scout | `Scout.PainSharp01`–`08`: 0.321, 0.593, 0.396, 0.370, 0.639, 0.382, 0.354, 0.360 | `Scout.PainSevere01`–`06`: 1.093, 1.297, 0.476, 0.723, 0.623, 0.834 |
| Soldier | `Soldier.PainSharp01`–`08`: 0.508, 0.393, 0.462, 0.583, 0.434, 0.321, 0.406, 0.891 | `Soldier.PainSevere01`–`06`: 2.925, 2.744, 1.781, 2.098, 1.300, 1.463 |
| Pyro | `Pyro.PainSharp01`–`07`: all 4.000 | `Pyro.PainSevere01`–`06`: all 4.000 |
| Demoman | `Demoman.PainSharp01`–`07`: 1.000, 1.000, 1.771, 1.000, 1.000, 1.000, 2.344 | `Demoman.PainSevere01`–`04`: 2.115, 2.208, 2.615, 2.375 |
| Heavy | `Heavy.PainSharp01`–`05`: 1.814, 1.440, 1.380, 1.440, 1.440 | `Heavy.PainSevere01`–`03`: 2.997, 1.486, 1.671 |
| Engineer | `Engineer.PainSharp01`–`08`: 1.000, 1.000, 1.000, 1.003, 1.007, 1.010, 2.019, 1.921 | `Engineer.PainSevere01`–`07`: 2.484, 2.099, 1.380, 2.071, 1.583, 2.146, 2.690 |
| Medic | `Medic.PainSharp01`–`08`: 2.354, 2.906, 2.021, 2.292, 1.625, 1.958, 2.583, 1.010 | `Medic.PainSevere01`–`04`: 3.135, 2.792, 2.604, 2.729 |
| Sniper | `Sniper.PainSharp01`–`04`: 1.188, 1.073, 1.240, 1.281 | `Sniper.PainSevere01`–`04`: 1.458, 1.396, 1.229, 2.719 |
| Spy | `Spy.PainSharp01`–`04`: 5.188, 5.021, 4.979, 5.073 | `Spy.PainSevere01`–`05`: 5.063, 5.042, 5.156, 5.135, 5.104 |

Scene files (all under `scenes/Player/<Class>/low/`): Scout pain 464,
465, 466, 1364–1368, attacker 461–463, 1361–1363; Soldier 1168–1170,
1374–1378 / 1165–1167, 1371–1373; Pyro 1584–1590 / 1581–1583,
1591–1593; Demoman 986–988, 1392–1395 / 983–985, 1396; Heavy 300–302,
1390, 1391 / 297–299; Engineer 136–138, 1249–1253 / 133–135,
1254–1257; Medic 636–638, 1244–1248 / 633–635, 1243; Sniper 1703–1705,
1837 / 1700–1702, 1838; Spy 806–808, 1381 / 803–805, 1387, 1388 (each
`<n>.vcd`, listed in the order of the entries above; the Sniper's pain
order is 1705, 1704, 1703, 1837). The scene lengths are longer than the
sounds (a Spy pain sound is about 0.5 s inside a 5 s scene): the gap
and the speaking window use the scene length.

Two more rules answer pain concepts and outrank the generic ones:

- `DrinkNotReady` (Scout, `TLK_PLAYER_PAIN`): active weapon is a drink
  (`tf_weapon_lunchbox_drink`), the equipped secondary is not
  `Crit-a-Cola` (weight 10), health fraction < 0.641, context
  `ScoutDrinkReadySpeech` unset. Plays one of `Scout.InvincibleNotReady01`,
  `02`, `03`, `05`, `06`, `07` (scenes 2732–2734, 2774–2776) instead of a
  pain cry; sets `ScoutDrinkReadySpeech` for 5 s.
- `KilledPlayerCaberDemoman` (Demoman, `TLK_PLAYER_ATTACKER_PAIN`):
  active weapon `tf_weapon_stickbomb` (weight 10), `IsCritical`
  (always true here), health fraction > 0.429, context
  `DemomanKillSpeechMelee` unset. The attacker hears one of
  `Demoman.SpecialCompleted11`, `12`, `01` (scenes 1007, 1008, 997;
  3.769, 5.095, 5.906 s) instead of a PainSevere; sets
  `DemomanKillSpeechMelee` for 10 s (which also blocks the Demoman's
  melee kill lines).

#### 2.5 Minimum gap, summarized

No pain line can start while now < next-pain time. next-pain time =
time of the last normal pain step + max(chosen pain scene length, crit
sound length if crit). Fall cries, drowning sounds and burning lines do
not move it (but are blocked by it), a stun sets it to now + 2.0 s
(§2.8). It starts at 0 and is never reset (not on respawn either).

#### 2.6 Who never makes pain sounds

- Disguised Spies (any disguise). A cloaked, undisguised Spy does make
  them (cloak is not checked).
- Dead or dying players; the killing hit.
- Players whose health did not change.
- A player with the `IsDominating` context set (10 s after one of his
  domination or revenge lines, 5 s after the Heavy's `KilledMedicHeavy`,
  10 s after an achievement line): no pain rule matches, so nothing
  plays and no gap is set; the crit ding still plays to him.

#### 2.7 Burning (`TLK_ONFIRE`)

While a player has the burning condition, the server asks
`TLK_ONFIRE` (normal request, no modifiers, no recipient list) whenever
now > next-burn-line time, then sets next-burn-line time = now + 2.5 s
whether a line played or not (the timer starts at 0, so the first ask is
on the first burning think). Rules per class: `PlayerOnFire<Class>`
(context `<Class>OnFire` not set, `IsNotDominating`; score 6) and, for
Scout, Soldier, Demoman, Heavy, Medic and Spy, `PlayerOnFireRare<Class>`
(also `10PercentChance`; score 7, so it wins when r > 90) play an
`<Class>.AutoOnFireNN` line; `PlayerStillOnFire<Class>` (context
`<Class>OnFire` = 1; score 6) plays one scene of three quick
`<Class>.PainSharpNN` cries (e.g. Scout scene 1932, 1.149 s:
`Scout.PainSharp01`, `04`, `03`). All three set `<Class>OnFire` for 7 s,
so a long burn alternates one "on fire" line, then still-on-fire cries
at most every 2.5 s, as far as the speaking window allows. The
Engineer, Sniper and Pyro have no rare rule. Burn ticks themselves make
no pain sound.

#### 2.8 Stun

A stun plays its own stun sound (not a voice line) and sets the
next-pain time to now + 2.0 s.

### 3. Death sounds

#### 3.1 Which entry

At death (after the kill line request of the killer, §5), unless one of
the "no sound" cases of §3.4 applies, the server emits **one** entry from
the dead player, chosen by the type flags of the **killing** damage, first
match wins:

1. fall damage → `Player.FallGib`;
2. blast damage → the class's explosion entry;
3. crit flag → the class's crit entry, and `TFPlayer.CritPain` to the
   victim only;
4. club (melee) damage → the class's melee entry;
5. anything else (bullets, fire, afterburn, bleed, telefrag, world
   damage, …) → the class's generic entry.

So a crit rocket or crit grenade kill plays the explosion entry (no crit
ding), a crit melee or a backstab plays the crit entry, burning to death
plays the generic entry, and a headshot (a crit) plays the crit entry.

#### 3.2 The class entries

The class scripts name four entries; the shipped data has exactly these
four per class (the encrypted class scripts could not be read, see Open
questions), all `CHAN_VOICE`, volume 1 (`VOL_NORM`), pitch 100,
`SNDLVL_95dB`:

| Kind | Entry | Waves (per class) |
|---|---|---|
| generic | `<Class>.Death` | the class's `vo/<class>_PainSevereNN.mp3` set |
| crit | `<Class>.CritDeath` | `vo/<class>_PainCrticialDeathNN.mp3` set (the misspelling is in the data) |
| melee | `<Class>.MeleeDeath` | the same set as crit |
| explosion | `<Class>.ExplosionDeath` | `vo/<class>_PainSharpNN.mp3` set |

Wave counts (generic / crit = melee / explosion): Scout 6/3/8, Soldier
6/4/8, Pyro 6/3/7, Demoman 4/5/7, Heavy 3/3/5, Engineer 7/6/8, Medic
4/4/8, Sniper 4/4/4, Spy 5/3/4. One wave is drawn at random per play
(the engine's random-wave rule). If a class script lacked a key, the
fallbacks would be `Player.Death`, `TFPlayer.CritDeath`,
`Player.MeleeDeath`, `Player.ExplosionDeath` (in
`game_sounds_player.txt`).

`Player.FallGib`: `CHAN_STATIC`, volume 1, pitch 92–96, 75 dB,
`player/pl_fleshbreak.wav`.

#### 3.3 Where and who

- Emitted on the dead player's entity, directly (not through a scene),
  to everyone in the potentially-audible set of his centre whose ear is
  within the entry's attenuation radius: 4500 u for the 95 dB class
  entries, 2500 u for `Player.FallGib`. The victim himself and the
  killer are included.
- Being on `CHAN_VOICE` of the same entity, a class death entry
  **replaces** whatever voice sound that player had playing on each
  receiving client (a pain line, a kill line, a domination taunt).
  `Player.FallGib` is `CHAN_STATIC` and replaces nothing.
- Before the death sound, the server sends the dead player's
  `BaseCombatCharacter.StopWeaponSounds` (silence on `CHAN_WEAPON`), which
  does not touch the voice channel.

#### 3.4 Deaths with no death sound

- The killer's **active** weapon has the "silent killer" attribute (Your
  Eternal Reward family): no death entry at all, whatever killed the
  victim, as long as that weapon is out.
- The player had no class yet.
- Mann vs. Machine defenders play `MVM.PlayerDied` instead (out of
  scope).

#### 3.5 Gibs, Dead Ringer

- A gibbing death additionally plays `BaseCombatCharacter.CorpseGib`
  (`CHAN_WEAPON`, volume 1, 75 dB,
  `physics/flesh/flesh_squishy_impact_hard2.wav`) just before the death
  entry (see Open questions).
- A Dead Ringer fake death plays the same death entry as a real death
  (after any pain line of the triggering hit); a Spy disguised as his own
  team uses the disguise class's entries. The fake death also counts as a
  kill for the attacker (§4).

### 4. Kill lines (`TLK_KILLED_PLAYER`)

#### 4.1 The request

When a player's death is credited to a killer player (the killer, not
the assister), in the same server frame, after the victim's domination
bookkeeping and before the victim's death sound, the killer makes a
normal request with these modifiers (weight 5 each):

- `customdeath`: `sentrygun` if the attacker or the inflictor is a
  building (`minisentrygun` for a mini-sentry inflictor), else
  `headshot` for a headshot kill, `backstab`, `burning` (afterburn),
  `taunt` (a taunt kill), `flareburn`, else `none`;
- `domination`: `revenge` if this kill was a revenge for the killer
  **or for an assister**, else `dominated` if this kill **started** the
  killer's domination of the victim (the 4th unanswered kill; later kills
  of a dominated player are `none`), else `none`;
- `victimstunned` (`1`/`0`), `victimdoublejumping` (`1`/`0`),
  `victimclass` (the victim's real class).

No recipient list: the line reaches everyone within 2500 u of the
killer (§1.6), the killer and the victim included, and a disguised
Spy's kill line is spoken in his own voice to everyone. A Spy who kills
by backstab with a knife that disguises him on kill makes no request.

`recentkills`: every damage event a player deals to another player is
recorded with a kill flag (blast events within 0.1 s of a previous blast
event are merged into it, their kills added); `recentkills` = the kills
of the events newer than 30 s, **including the current kill**. So
`IsManyRecentKills` (> 1) means this is at least the second kill in
30 s and `IsVeryManyRecentKills` (> 3) at least the fourth.
`weaponmode`, `playerweapon` and `item_name` describe the killer's active
weapon at the moment of the kill (not the weapon that fired).

#### 4.2 Gates

The killer must be alive (dying in the same frame or earlier: no line),
must not be in his speaking window (a pain line he took less than its
scene length ago blocks it: up to about 5 s for a Spy, 4 s for a Pyro),
and must not have had a request accepted earlier in the same frame (only
the first of several simultaneous kills can speak). A refused request is
dropped.

#### 4.3 Rule families common to classes

(`KilledPlayerDelay` = world context `DontKilledPlayer` not 1 appears in
most rules; nothing sets that context, so it always passes and adds 1.)

| Family (rule name pattern) | Needs | Chance | Context set | Predelay |
|---|---|---|---|---|
| `KilledPlayerAssistAuto<Class>` | being healed; ≥ 2 recent kills (Scout, Soldier, Pyro, Demoman, Heavy) or ≥ 1 (Engineer, Medic, Sniper, Spy); `<Class>AssistSpeech` unset | 20 % | `<Class>AssistSpeech` 20 s | 2.5 s |
| `KilledPlayerMany<Class>` | ≥ 2 recent kills; primary weapon out (Demoman and Spy: secondary; Engineer and Medic: any); `<Class>KillSpeech` unset | 30 % | `<Class>KillSpeech` 10 s (Scout 5 s) | – |
| `KilledPlayerVeryMany<Class>` (Soldier, Demoman, Heavy, Engineer, Medic, Sniper) | ≥ 4 recent kills; same weapon slot as Many | 50 % | `<Class>KillSpeech` 10 s | – |
| `KilledPlayerMelee<Class>` (and weapon variants) | melee out (or a named melee); `<Class>KillSpeechMelee` unset | 30 % (Demoman named weapons 10–30 %) | `<Class>KillSpeechMelee` 10 s (Spy 5 s) | – |
| `PlayerKilledDominating…` | `domination` = `dominated` | see §4.4 | `<Class>KillSpeech` 10 s **and** `IsDominating` 10 s | 2.5 s |
| `PlayerKilledForRevenge<Class>` | `domination` = `revenge` | none (Sniper 50 %) | `<Class>KillSpeech` 10 s and `IsDominating` 10 s | 2.5 s |

Domination and revenge rules do not check the `KillSpeech` contexts:
they speak whenever the gates of §4.2 allow. Many/VeryMany scores: 10
for Many (5 + 1 + 1 + 1 + 1 + 1), 10 for VeryMany; when both match (≥ 4
kills, r > 70) they tie and one is picked at random.

#### 4.4 Per class

Lines are listed as sound entries; "N lines" pools are read from the
data. Scores in brackets where they decide between rules.

**Scout** (`ScoutKillSpeech` 5 s for Many; melee 10 s)
- Many: 10 lines (`Scout.GoodJob02`, `LaughEvil02`, `LaughHappy01`–`04`,
  `LaughLong01`–`02`, `Taunts04`, `Taunts16`).
- Melee with the stock bat or any melee not named below
  (`KilledPlayerMeleeBatScout`, 30 %): a random one of two groups:
  `Scout.SpecialCompleted04`, `06`, `07`, `09`, `Taunts11` or the generic
  group `Scout.SpecialCompleted02`, `03`, `11`, `Taunts08`. Its seven
  "not this item" criteria add 70, so it scores 79.
- Named melee rules (30 %, each also has the generic group): Boston
  Basher (`Scout.Revenge01`, `Misc04`), Candy Cane (`Cheers04`,
  `Taunts13`, `Taunts15`), Sun-on-a-Stick and Three-Rune Blade
  (`DominationPyr01`, `Generic01`, `Misc09`), Holy Mackerel, Festive Holy
  Mackerel, Fan O'War and Saxxy (`LaughHappy02`, `LaughLong02`,
  `Taunts18`, `Domination19`).
- Melee kill of a Heavy (`KilledPlayerMeleeScoutFatScout`, 75 %, score
  9; context `ScoutKillSpeechMelee` must be unset but it sets
  `ScoutKillSpeechMeleeFat` 10 s): `Scout.SpecialCompleted01`. It only
  wins when the bat rule fails (r ≤ 70).
- Domination: generic `PlayerKilledDominatingScout` (30 %, score 12;
  `Scout.Domination02`, `Misc07`, `Domination15`, `16`); with melee out
  `PlayerKilledDominatingBatScout` (score 12; `Scout.Misc05`, `Misc04`);
  else per victim class (score 11; 6–14 lines each, e.g.
  `Scout.DominationHvy01`–`10`). Revenge: 15 lines.

**Soldier** (`SoldierKillSpeech` 10 s)
- Many (4 lines), VeryMany (`Soldier.Taunts17`, `Taunts02`), assist
  (`Soldier.SpecialCompleted-AssistedKill01`).
- `SpreeMedicSoldier`: being healed, ≥ 4 recent kills, primary out, no
  chance; `Soldier.KaBoomAlts01`–`03`; sets `SoldierKillSpeech` 20 s.
- Melee 30 %: `Soldier.SpecialCompleted05`, `DirectHitTaunt01`, `03`, `04`.
- Domination: generic (laughs and taunts, 10 lines) and per victim class
  both score 11: a 50/50 pick. Revenge: 3 lines.
- Robot costume variants (weight 100) replace these when worn.

**Pyro** (`PyroKillSpeech` 10 s)
- Many: `Pyro.Taunts04`. Melee 30 %: `Pyro.Taunts01`–`03`. Assist: 1 line.
- Domination: generic only (`Pyro.LaughEvil01`, `02`, `LaughHappy01`,
  `LaughLong01`). Revenge: `Pyro.ActivateCharge01`, `BattleCry01`.

**Demoman** (`DemomanKillSpeech` 10 s; melee contexts vary)
- Many and VeryMany need the **secondary** (stickybomb launcher) out.
- Bottle (`tf_weapon_bottle`, not Pain Train or Saxxy), 30 %: a random
  group of the eight `Demoman.gibberishNN` lines or
  `Demoman.SpecialCompleted02`; 10 s.
- Frying Pan or Saxxy, 20 %: gibberish; 10 s. Pain Train, 10 %:
  `Demoman.gibberish13` or the gibberish group; 15 s.
- Eyelander-type swords (`tf_weapon_sword`) and katana, 20 %:
  `Demoman.eyelandertaunt01` against a Demoman, else `eyelandertaunt02`,
  `LaughEvil01`–`03`; sets `DemomanKillSpeechMelee` **30 s**.
- Domination: generic (11 laughs) vs per victim (score tie, 50/50).
  Revenge: 4 lines.

**Heavy** (`HeavyKillSpeech` 10 s)
- Many and VeryMany (primary): 6 lines each. Melee 30 %:
  `Heavy.LaughShort01`, `02`; Fists of Steel adds `Heavy.SpecialWeapon01`.
- `KilledPlayerTauntHeavy`: Engineer victim, another enemy under the
  crosshair, 30 %: `Heavy.Taunts17`, `Taunts13`.
- `KilledMedicHeavy`: Medic victim, Heavy being healed, 50 %, context
  `HeavyMedicSpeech` unset (this criterion has weight 1): `Heavy.Specials01`–`04`;
  sets `HeavyMedicSpeech` 20 s and `IsDominating` 5 s.
- Domination: generic only, 32 lines. Revenge: 18 lines.

**Engineer** (`EngineerKillSpeech` 10 s)
- Sentry kills: `KillTauntsEngineerSpecial` (≥ 2 recent kills, 30 %,
  score 14): `Engineer.SpecialCompleted02`, `06`, `08`, `09`; mini-sentry
  `KillTauntsEngineerMiniSentry` (20 %, predelay 0.25 s):
  `engineer_littlesentry01`–`03`; wrangler out (`EngineerLaserPointer`,
  30 %, 0.25 s): `engineer_wranglekills01`–`04`. When none of these
  passes, `SentryKill` / `MiniSentryKill` (score 11) play the silent face
  scene `scenes/player/engineer/low/attack01.vcd` (no sound) and set
  `MiniSentryKill` 15 s.
- Golden Wrench, Saxxy, Gold Frying Pan (no chance, 0.25 s):
  `engineer_goldenwrenchkill01`–`04`; `EngineerKillSpeechMelee` 10 s.
- Melee 30 % (not a sentry kill): `Engineer.SpecialCompleted07`.
- Gunslinger after its punch combo (context `EngyFistSwung` set):
  `engineer_gunslingertriplepunchfinal01`–`03`; melee context 20 s.
- Many and VeryMany: not with the Gunslinger equipped.
- Domination: per victim class only. Revenge: 18 lines.

**Medic** (`MedicKillSpeech` 10 s)
- `KilledPlayerMedic`: **any** kill, 30 %, both `MedicKillSpeech` and
  `MedicKillSpeechMelee` unset (score 8): one of five groups at random
  (`Medic.SpecialCompleted02`, `03`, `07` / `SpecialCompleted01` / the
  Many pool / the VeryMany pool / the domination laughs); sets
  `MedicKillSpeechMelee` 10 s.
- Many (score 9), VeryMany, assist.
- Domination: generic only (10 laughs). Revenge: 4 lines.

**Sniper** (`SniperKillSpeech` 10 s)
- Rifle (`tf_weapon_sniperrifle`) headshot, no chance
  (`SniperHeadShotKill`, score 12, predelay 1.75 s or 1 s by group):
  a random one of three groups (13 headshot lines, the 10 "zoomed"
  lines, the 11 generic lines); sets `SniperKillSpeech` **20 s**.
- Headshot of a Soldier, Heavy or Spy with the rifle or the Classic,
  50 % (score 13, 1.75 s): victim-specific lines. Without 50 %: the
  generic headshot rule.
- Any rifle kill, 20 % (`SniperKill`, 1 s): 11 lines; while zoomed
  (`SniperKillSoft`, score 9): 10 lines. Bow kills use the same pools.
- Any kill of a Spy, 50 % (`SniperKillSpy`, 1 s):
  `Sniper.SpecialCompleted01`, `12`, `18`, `21`.
- Many, VeryMany (primary or bow), melee 30 % (Shiv and Saxxy
  variants), assist.
- Domination: per victim class only. Revenge: 50 % (zoomed: a second
  group).

**Spy** (`SpyKillSpeech` 10 s)
- Melee (knife) kill, 30 %, `SpyKillSpeechMelee` 5 s:
  `Spy.SpecialCompleted02`, `03`, `11`; while disguised
  (`KilledPlayerMeleeDisguisedSpy`, predelay 0.75 s) a random group of 7
  disguised lines or those 3.
- Many: **secondary** (revolver) out, 30 %.
- Engineer victim: after the Spy's sapper was destroyed
  (`PlayerSapperKillSpy`, 50 %): `Spy.Taunts07`, `08`; after his
  buildings were destroyed (`EngineerKilledAfterSap`):
  `Spy.SpecialCompleted05`; otherwise (`EngineerKilled`, score 6) the
  silent face scene, setting `EngyKilled` 10 s.
- Domination: per victim class only. Revenge: 9 lines.

Cosmetic rules (`item_…`, weight 100) win whenever their cosmetic is worn
and their chance passes.

#### 4.5 Timing and interruption

- Lines without predelay start on the scene's first think (the same or
  the next server frame); domination and revenge lines 2.5 s after the
  kill; Sniper 1 or 1.75 s; Spy disguised 0.75 s; Engineer special
  weapons 0.25 s; assist lines 2.5 s.
- The context is set when the request succeeds, i.e. at the kill, not
  when the sound starts.
- The killer's speaking window runs from the kill for the scene length
  only, so with a 2.5 s predelay the window can end before the line has
  finished (another normal request may start then and its sound would
  replace the first on the voice channel).
- A newer sound on the speaker's `CHAN_VOICE` replaces the playing one on
  each client: his own pain lines (allowed during a kill line), his death
  entry, his next line. Nothing else stops a line.
- Speaker dies mid-line: the death entry cuts it (except a fatal fall,
  whose splat is on `CHAN_STATIC`, and a silent-killer death, which play
  no voice sound: the line plays to its end). Speaker dies during a
  predelay: the scene still plays and the line is emitted from his
  (dead) player entity when the predelay ends.
- Class change (respawn in the spawn room) does not stop a playing line
  or a pending scene.

## Per-tick order

Damage that does not kill (one damage event):

1. Damage applied; killing-hit bookkeeping (attacker's recent-kill
   record) done.
2. Pain step (§2.2–§2.3): crit ding to the victim, `TLK_PLAYER_PAIN` to
   the others, `TLK_PLAYER_ATTACKER_PAIN` to the attacker, next-pain
   time set.
3. Later in the frame: scenes think; speak events with start time 0 emit
   their sounds.

Damage that kills:

1. Damage applied, health ≤ 0; the attacker's damage record gets the
   kill.
2. Victim's death handling: `TLK_DIED` (silent); domination/revenge
   flags computed; victim becomes dying; `BaseCombatCharacter.StopWeaponSounds`.
3. Killer's `TLK_KILLED_PLAYER` request (§4).
4. Gib check: `BaseCombatCharacter.CorpseGib` if gibbing.
5. Victim's death entry (§3).
6. The pain step is skipped (victim no longer alive).
7. Scene thinks: the killer's line starts (no predelay) or counts down.

## Edge cases

- Rapid hits: a minigun stream on a Scout produces a pain line about every
  0.32–0.64 s (each new line chosen without repeats from his eight);
  on a Pyro, one every 4.0 s; on a Spy, one about every 5 s.
- Shotgun pellets of one shot arrive as one damage event (one pain step).
- Two attackers: each damage event's attacker gets the attacker-pain
  line only if that event passed the gap; the other attacker hears the
  normal pain line, if any.
- Attacker beyond 2500 u (Sniper across the map): hears no pain line.
- Healing does not create sounds; overheal reduced by damage counts as a
  health change.
- A request whose chosen scene has no speech (Engineer/Spy face) still
  sets the rule's context and counts as the accepted request of the frame.
- `recentkills` is 0 during training mode.
- A team-mate hurt with friendly fire on behaves like an enemy hurt.
- A Spy disguised as an enemy making normal requests (on-fire, voice
  commands) plays his disguise class's line to the enemy team and
  himself and his own to his team; pain does not apply (disguised: no
  pain), kill lines are not split.
- The fall cry and drowning sound are not limited by the 2500 u scene
  radius but by their own entry radius (4500 u, 2500 u).

## Quirks

- `damagecritical` = 1 is sent with every pain request, crit or not, so
  `IsCritical` always passes there. Effect worth keeping: an enemy who
  hurts a Demoman holding the Ullapool Caber above 42.9 % health hears a
  Caber boast (`KilledPlayerCaberDemoman`) instead of a pain cry, at most
  once per 10 s. Keep (parity).
- Chance criteria are slightly off: "30 %" is 30/101, "2 %" is 3/101,
  "100 %" fails at r = 0. Keep.
- Revenge lines fire when only an assister got revenge. Keep.
- Domination lines play only on the kill that starts the domination.
- The `IsDominating` context (10 s after a domination/revenge line)
  silences pain and on-fire lines. Keep.
- The pain gap uses the scene length, which for the Spy (≈5 s) and Pyro
  (4 s) is far longer than the sound. Keep.
- A pain line taken shortly before a kill blocks the kill line (speaking
  window), notably for Spy and Pyro. Keep.
- `item_unicorn_backstabSpy` applies its context to the world, so the
  Spy's own melee cooldown is not set by it. Keep.
- `KilledPlayerDelay` never blocks (nothing sets its context).
- Several Scout domination scenes named in the rules (2552, 2553) are
  missing from `scenes.image` and are never picked.
- The "not this item" criteria weigh 10 each, so the stock-bat rule (79)
  always beats the Heavy-victim rule (9) when both match.

## Test cases

1. Response matcher: pattern `>70` passes r = 71 and fails r = 70;
   `=>98` passes 98; `>0` fails 0; `>4.0,<=8.0` passes 8.0 and fails
   4.0; `!=1` passes a missing key; `1` fails a missing key; `!=Scout`
   fails `scout`.
2. Scoring: Scout, concept `TLK_KILLED_PLAYER`, `domination` =
   `dominated`, victim Spy, r = 75, no melee: `PlayerKilledDominatingScout`
   scores 12, `PlayerKilledDominatingSpyScout` 11 → the generic rule. With
   r = 50 → the Spy rule.
3. Soldier dominates a Medic (any r): generic and Medic rules both score
   11; over many trials each is picked about half the time.
4. Scout kills with the scattergun, first kill in 30 s, r = 99: no rule
   matches, no sound, no context.
5. Same, second kill 10 s later, r = 85, `ScoutKillSpeech` unset:
   `KilledPlayerManyScout` (score 10); one of its 10 lines starts at once
   on the Scout's `CHAN_VOICE` for players within 2500 u; `ScoutKillSpeech`
   set until +5 s; a third kill 3 s later with r = 85 says nothing.
6. Scout kills a Heavy with the stock bat: r = 80 → Bat rule (79), a line
   from one of its two groups, `ScoutKillSpeechMelee` 10 s; r = 50 →
   `Scout.SpecialCompleted01`, `ScoutKillSpeechMeleeFat` 10 s; r = 20 →
   nothing.
7. Sniper rifle headshot of a Soldier: r = 60 → one of
   `Sniper.SpecialCompleted14`, `42`, `38`, `Taunts42` 1.75 s after the
   kill; r = 40 → the generic headshot rule, one of its three groups
   (predelay 1.75 s for the headshot group, 1 s for the others);
   `SniperKillSpeech` 10 s or 20 s respectively.
8. Depletion: a group of 3 lines asked 3 times (by any players of that
   class) yields 3 different lines; the 4th may equal any.
9. Pain, Scout hit for 30 by an enemy Soldier 300 u away, not crit, no
   gap: Soldier receives one `Scout.PainSevereNN`, every other player
   within 2500 u of the Scout (the Scout included) one
   `Scout.PainSharpNN`; if the PainSharp scene was 464.vcd the next pain
   is allowed 0.321 s later; the Scout's speaking window is the PainSevere
   scene's length.
10. Same hit 0.2 s after a pain whose scene was 465.vcd (0.593 s): no
    pain sound for anyone.
11. Crit hit on a Scout: the Scout hears `TFPlayer.CritPain` only; others
    hear PainSharp; the attacker PainSevere; gap = max(scene, crit wave).
12. Pyro hit at t = 0, 1, 2, 3, 4.1 s: pain lines at 0 and 4.1 only.
13. Spy hit at t = 0 (pain scene 808.vcd, 4.979 s), kills someone at
    t = 2: his kill line is refused (speaking window of the last pain
    line); at t = 6 it is allowed.
14. Disguised Spy hit for 50, crit or not: no pain line and no crit ding
    (the disguise check comes before the crit step).
15. Ubercharged Medic hit: health unchanged → nothing.
16. Soldier rocket-jumps (self damage 40): the Soldier hears a
    `Soldier.PainSevereNN`; other players a `Soldier.PainSharpNN`.
17. Engineer falls for 20 damage and survives: `Engineer.Death` (one of
    `vo/engineer_PainSevere01`–`07.mp3`) to everyone within 4500 u, plus
    `Player.FallDamage`; the next-pain time is unchanged.
18. Afterburn on a Heavy (no other damage): no pain lines; `TLK_ONFIRE`
    at the first burning think (r > 90 → `PlayerOnFireRareHeavy`, else
    `PlayerOnFireHeavy`), then at +2.5 s `PlayerStillOnFireHeavy`
    (scene 1925.vcd) if his speaking window has ended.
19. Heavy killed by a crit rocket: `Heavy.ExplosionDeath`, no crit ding.
20. Sniper killed by a crit headshot: `Sniper.CritDeath` to all within
    4500 u; `TFPlayer.CritPain` to the Sniper only.
21. Soldier killed by a non-crit shovel: `Soldier.MeleeDeath`. Killed by
    afterburn: `Soldier.Death`.
22. Fatal fall while a kill line plays: `Player.FallGib` plays and the
    kill line keeps playing to its end.
23. Scout killed by a bullet 0.3 s into his kill line: on every client
    that receives `Scout.Death`, the kill line stops.
24. Killer holds Your Eternal Reward and kills by any means: no death
    entry.
25. Two enemies killed by one rocket in one frame: only the first kill
    request can produce a line.
26. Killer dies 1 s after a domination kill: his domination line still
    starts 2.5 s after the kill, from his dead entity, and replaces his
    death entry if that is still playing.
27. Player dominating (context `IsDominating` set 4 s ago) is hurt: no
    pain line, next-pain time unchanged.
28. Enemy hurts a Demoman holding the Caber at 120/175 HP (0.686): the
    attacker hears `Demoman.SpecialCompleted11`, `12` or `01`; a second
    hit 5 s later gives the attacker a PainSevere.

## Open questions

1. The class death entries are taken from the four entries per class in
   `game_sounds_vo.txt`; the encrypted `scripts/playerclasses/<class>.ctx`
   keys `sound_death`, `sound_crit_death`, `sound_melee_death`,
   `sound_explosion_death` were not read. Check by decrypting a class
   script locally or by listening to a death of each kind.
2. Lengths of `player/crit_received1`–`3.wav` (crit pain gap) were not
   measured: the sound archives are not on this box.
3. Replacement on (entity, `CHAN_VOICE`) is engine-side and taken from
   [../cs_source/sounds.md](../cs_source/sounds.md); confirm that a death
   cry cuts a playing kill line in TF2 (record one).
4. That a pending scene still speaks after its speaker died (predelay
   running) follows from the scene code (the dead player entity remains
   its actor); confirm in a game with a domination followed by a quick
   death.
5. Whether the per-class rule pools' no-repeat state is reset at map
   change was not followed; assumed reset at map load.
6. Whether mini-crits set the crit flag for the death/pain choice: assumed
   not (only full crits); see [weapons.md](weapons.md) for crit damage.
7. Gibbing: the base gib path plays `BaseCombatCharacter.CorpseGib` and
   still lets the death entry play; whether players actually hear the
   squish in TF2 should be checked in a game.
8. The potentially-audible set: implement as "every player" unless the
   map's visibility data is available; the radii above are exact.
9. Event start times inside multi-sound scenes (the still-on-fire
   scenes) were not decoded; read them from the scene data at run time
   ([faces.md](faces.md) §12 gives the layout).
10. How the client spatializes the local player's own voice sounds
    (positioned or "in the head") is engine-side and not settled here.

## Appendix A: kill-line pools

Every scene below has exactly one speak event, so each line is one sound
entry. Notation: `Entry` n/len = scene `scenes/Player/<Class>/low/<n>.vcd`
and its length in seconds (the speaking window, §1.6). "Context" is the
`ApplyContext` the rule sets (name, seconds; value 1). Every rule also
needs the class, the concept and its own context unset (the one it sets,
except where noted); chances are against r (§Constants). A rule with
several groups picks one group uniformly, then a line (§1.4).

### A.1 Weapon needed by Many and VeryMany

The criterion is `weaponmode` of the killer's active weapon: `primary` is
the weapon's primary role (slot 0), `secondary` slot 1.

| Class | Many | VeryMany |
|---|---|---|
| Scout | primary | no rule |
| Soldier | primary | primary |
| Pyro | primary | no rule |
| Demoman | secondary | secondary |
| Heavy | primary | primary |
| Engineer | any (but not with `The Gunslinger` as `item_name`) | any (same) |
| Medic | any | any |
| Sniper | primary (plus `…Bow` rules: Huntsman type, same pools) | primary (plus bow rule) |
| Spy | secondary | no rule |

### A.2 Many (`KilledPlayerMany<Class>`): ≥ 2 recent kills, `30PercentChance`, no predelay

| Class | Context | Lines |
|---|---|---|
| Scout | `ScoutKillSpeech` 5 | `Scout.GoodJob02` 396/1.38, `Scout.LaughEvil02` 1297/2.38, `Scout.LaughHappy01` 432/0.97, `Scout.LaughHappy02` 433/5.39, `Scout.LaughHappy03` 1295/1.43, `Scout.LaughHappy04` 1296/1.45, `Scout.LaughLong01` 434/5.75, `Scout.LaughLong02` 435/4.77, `Scout.Taunts04` 493/1.92, `Scout.Taunts16` 506/1.89 |
| Soldier | `SoldierKillSpeech` 10 | `Soldier.SpecialCompleted03` 1181/1.03, `Soldier.Taunts01` 1356/1.14, `Soldier.Taunts09` 1197/3.63, `Soldier.Taunts10` 1199/3.26 |
| Pyro | `PyroKillSpeech` 10 | `Pyro.Taunts04` 1534/4.00 |
| Demoman | `DemomanKillSpeech` 10 | `Demoman.SpecialCompleted04` 1000/4.89, `Demoman.Taunts02` 1014/3.79, `Demoman.Taunts04` 1016/4.21, `Demoman.Taunts08` 1020/5.02 |
| Heavy | `HeavyKillSpeech` 10 | `Heavy.SpecialCompleted09` 310/4.00, `Heavy.SpecialCompleted10` 311/5.61, `Heavy.SpecialCompleted11` 1279/5.08, `Heavy.Taunts04` 326/4.18, `Heavy.Taunts05` 327/4.13, `Heavy.Taunts11` 335/4.08 |
| Engineer | `EngineerKillSpeech` 10 | `Engineer.SpecialCompleted05` 151/3.61, `Engineer.SpecialCompleted10` 158/2.98, `Engineer.SpecialCompleted11` 154/1.92 |
| Medic | `MedicKillSpeech` 10 | `Medic.SpecialCompleted12` 651/4.74, `Medic.Taunts02` 663/5.27 |
| Sniper | `SniperKillSpeech` 10 | `Sniper.LaughEvil01` 1670/10.54, `Sniper.LaughEvil02` 1671/6.02, `Sniper.LaughHappy01` 1672/5.62, `Sniper.LaughHappy02` 1673/5.03, `Sniper.LaughLong01` 1674/5.26, `Sniper.LaughLong02` 1675/5.12, `Sniper.SpecialCompleted04` 1715/1.86, `Sniper.SpecialCompleted15` 1726/2.42, `Sniper.SpecialCompleted45` 1810/3.51 |
| Spy | `SpyKillSpeech` 10 | `Spy.LaughEvil01` 772/4.91, `Spy.LaughEvil02` 1312/5.18, `Spy.LaughHappy01` 773/5.17, `Spy.LaughHappy02` 774/5.10, `Spy.LaughHappy03` 1313/1.67, `Spy.LaughLong01` 775/10.15, `Spy.LaughShort06` 1322/4.00, `Spy.SpecialCompleted09` 824/4.99, `Spy.SpecialCompleted10` 825/5.08 |

### A.3 VeryMany (`KilledPlayerVeryMany<Class>`): ≥ 4 recent kills, `50PercentChance`, no predelay

| Class | Context | Lines |
|---|---|---|
| Soldier | `SoldierKillSpeech` 10 | `Soldier.Taunts17` 1206/4.43, `Soldier.Taunts02` 1188/2.31 |
| Demoman | `DemomanKillSpeech` 10 | `Demoman.SpecialCompleted01` 997/5.91, `Demoman.SpecialCompleted03` 999/5.13, `Demoman.SpecialCompleted07` 1003/5.86, `Demoman.SpecialCompleted08` 1004/7.58, `Demoman.SpecialCompleted10` 1006/6.54, `Demoman.Taunts12` 1024/2.41 |
| Heavy | `HeavyKillSpeech` 10 | `Heavy.SpecialCompleted07` 318/8.38, `Heavy.SpecialCompleted08` 319/7.53, `Heavy.Taunts01` 323/4.11, `Heavy.Taunts14` 338/3.99, `Heavy.Taunts19` 340/6.95, `Heavy.Taunts10` 333/4.43 |
| Engineer | `EngineerKillSpeech` 10 | `Engineer.SpecialCompleted01` 147/3.21 |
| Medic | `MedicKillSpeech` 10 | `Medic.SpecialCompleted02` 646/4.72, `Medic.SpecialCompleted09` 655/6.49, `Medic.SpecialCompleted10` 656/5.42, `Medic.SpecialCompleted11` 652/4.92 |
| Sniper | `SniperKillSpeech` 10 | `Sniper.SpecialCompleted03` 1714/2.77, `Sniper.SpecialCompleted07` 1718/3.16, `Sniper.SpecialCompleted10` 1721/2.43, `Sniper.Taunts05` 1742/2.42, `Sniper.Taunts06` 1743/2.70, `Sniper.Taunts07` 1744/3.05, `Sniper.Taunts11` 1748/3.73, `Sniper.Taunts18` 1755/4.24 |

### A.4 Stock melee (generic rule only), `30PercentChance`, no predelay

| Class | Rule and condition | Context | Groups (one picked) |
|---|---|---|---|
| Scout | `KilledPlayerMeleeBatScout`: melee out, `item_name` none of the seven named items | `ScoutKillSpeechMelee` 10 | (1) `Scout.SpecialCompleted04` 479/0.91, `Scout.SpecialCompleted06` 482/2.22, `Scout.SpecialCompleted07` 481/1.29, `Scout.SpecialCompleted09` 483/2.25, `Scout.Taunts11` 501/1.57; (2) `Scout.SpecialCompleted02` 476/0.48, `Scout.SpecialCompleted03` 477/0.45, `Scout.SpecialCompleted11` 484/1.15, `Scout.Taunts08` 498/1.79 |
| Soldier | `KilledPlayerMeleeSoldier`: melee out | `SoldierKillSpeechMelee` 10 | `Soldier.SpecialCompleted05` 1185/2.38, `Soldier.DirectHitTaunt01` 3403/1.86, `Soldier.DirectHitTaunt03` 3405/1.34, `Soldier.DirectHitTaunt04` 3406/1.59 |
| Pyro | `KilledPlayerMeleePyro`: melee out | `PyroKillSpeechMelee` 10 | `Pyro.Taunts03` 1594/4.00, `Pyro.Taunts01` 1532/4.00, `Pyro.Taunts02` 1533/4.00 |
| Demoman | `KilledPlayerMeleeDemoman`: `playerweapon` `tf_weapon_bottle`, not Pain Train, not Saxxy | `DemomanKillSpeechMelee` 10 | (1) `Demoman.gibberish01` 3566/7.79, `02` 3567/3.77, `04` 3569/5.54, `05` 3570/5.62, `06` 3571/7.43, `07` 3572/4.50, `08` 3573/4.10, `10` 3575/5.92; (2) `Demoman.SpecialCompleted02` 998/4.85 |
| Heavy | `KilledPlayerMeleeHeavy`: melee out | `HeavyKillSpeechMelee` 10 | `Heavy.LaughShort01` 272/2.66, `Heavy.LaughShort02` 273/2.72 |
| Engineer | `KilledPlayerMeleeEngineerEngineer`: melee out, `customdeath` `none` | `EngineerKillSpeechMelee` 10 | `Engineer.SpecialCompleted07` 155/2.14 |
| Medic | no melee rule (see A.5) | – | – |
| Sniper | `KilledPlayerMeleeSniper`: melee out, not Saxxy | `SniperKillSpeechMelee` 10 | `Sniper.SpecialCompleted06` 1717/3.30, `Sniper.SpecialCompleted09` 1720/3.03 |
| Spy | `KilledPlayerMeleeSpy`: melee out | `SpyKillSpeechMelee` **5** | `Spy.SpecialCompleted02` 817/5.03, `Spy.SpecialCompleted03` 818/5.08, `Spy.SpecialCompleted11` 826/5.18 |

### A.5 Medic any kill (`KilledPlayerMedic`)

Any kill, `30PercentChance`, needs both `MedicKillSpeech` and
`MedicKillSpeechMelee` unset; sets `MedicKillSpeechMelee` 10. Five groups,
one picked uniformly; each group keeps its own predelay:

| Group | Predelay | Lines |
|---|---|---|
| 1 | – | `Medic.SpecialCompleted02` 646/4.72, `Medic.SpecialCompleted03` 647/3.88, `Medic.SpecialCompleted07` 653/1.66 |
| 2 | – | `Medic.SpecialCompleted01` 645/4.74 |
| 3 (the Many pool) | – | `Medic.SpecialCompleted12` 651/4.74, `Medic.Taunts02` 663/5.27 |
| 4 (the VeryMany pool) | – | `Medic.SpecialCompleted02` 646/4.72, `Medic.SpecialCompleted09` 655/6.49, `Medic.SpecialCompleted10` 656/5.42, `Medic.SpecialCompleted11` 652/4.92 |
| 5 (the domination pool) | 2.5 | as `PlayerKilledDominatingMedic` in A.7 |

Groups 3–5 are the groups of the named rules, referenced by name.
Whether the pool holds one copy (no-repeat cycle shared with those rules)
or a copy per referencing rule was not followed; assume one copy per
group name.

### A.6 Sniper rifle rules

| Rule | Needs | Chance | Context | Groups: predelay, lines |
|---|---|---|---|---|
| `SniperHeadShotKill` | `playerweapon` `tf_weapon_sniperrifle`, `customdeath` `headshot` | none | `SniperKillSpeech` **20** | (1) 1.75 s: `Sniper.SpecialCompleted25` 1790/3.16, `30` 1795/2.03, `36` 1801/3.93, `37` 1802/3.56, `02` 1713/1.59, `17` 1728/2.35, `24` 1789/0.28, `26` 1791/2.32, `27` 1792/1.00, `35` 1800/1.37, `46` 1811/3.32, `Sniper.Taunts01` 1738/1.47, `Sniper.Taunts23` 1812/2.23; (2) 1 s: the `SniperKillSoft` pool; (3) 1 s: the `SniperKill` pool |
| `SniperKill` | `tf_weapon_sniperrifle` | `20PercentChance` | `SniperKillSpeech` 10 | 1 s: `Sniper.SpecialCompleted13` 1724/2.04, `Sniper.Taunts45` 1834/1.44, `Taunts22` 1759/4.23, `Taunts08` 1745/2.04, `Taunts09` 1746/2.04, `Taunts10` 1747/3.38, `Taunts13` 1750/4.06, `Taunts14` 1751/2.26, `Taunts17` 1754/5.23, `Taunts19` 1756/2.99, `Taunts02` 1739/1.47 |
| `SniperKillSoft` | `tf_weapon_sniperrifle`, `sniperzoomed` 1 | `20PercentChance` | `SniperKillSpeech` 10 | 1 s: `Sniper.SpecialCompleted44` 1809/1.90, `Sniper.Taunts24` 1813/1.44, `Taunts37` 1826/1.83, `Taunts38` 1827/2.66, `Taunts39` 1828/4.16, `Taunts27` 1816/2.03, `Taunts28` 1817/1.68, `Taunts29` 1818/1.70, `Taunts30` 1819/1.35, `Taunts31` 1820/1.76 |
| `SniperKillSpy` | `victimclass` Spy, any weapon | `50PercentChance` | `SniperKillSpeech` 10 | 1 s: `Sniper.SpecialCompleted01` 1712/4.06, `Sniper.SpecialCompleted12` 1723/2.44, `Sniper.SpecialCompleted18` 1729/4.85, `Sniper.SpecialCompleted21` 1732/3.08 |

(The `…Classic` and `…Bow` variants use the same pools with the Classic
or the Huntsman instead of the rifle.)

### A.7 Generic domination and revenge (predelay 2.5 s)

Domination needs `domination` = `dominated`; revenge `domination` =
`revenge`. Both set `<Class>KillSpeech` 10 and `IsDominating` 10 and do
not check `<Class>KillSpeech`. Classes without a generic domination rule
(Engineer, Sniper, Spy) use only their per-victim rules.

| Class | Generic domination (chance) | Revenge (chance) |
|---|---|---|
| Scout | `PlayerKilledDominatingScout` (`30PercentChance`): `Scout.Domination02` 2551/0.73, `Scout.Misc07` 2687/4.26, `Scout.Domination15` 2764/3.00, `Scout.Domination16` 2765/3.00 | none: `Scout.Award12` 2681/3.14, `Scout.Cheers02` 374/0.60, `Scout.Misc02` 2641/4.00, `Scout.Misc09` 2706/4.00, `Scout.NiceShot02` 453/0.98, `Scout.Revenge01`–`09` 2586–2594 (2.00, 2.25, 2.25, 2.35, 2.18, 3.00, 2.44, 2.50, 2.25), `Scout.SpecialCompleted12` 486/1.57 |
| Soldier | `PlayerKilledDominatingSoldier` (none): `Soldier.LaughEvil01` 1132/1.96, `LaughEvil02` 1346/2.00, `LaughEvil03` 1348/0.92, `LaughHappy01` 1133/1.52, `LaughHappy02` 1134/1.92, `LaughHappy03` 1347/5.35, `LaughLong01` 1135/4.51, `LaughLong03` 1349/7.34, `Taunts07` 1195/4.36, `Taunts13` 1202/4.66 | none: `Soldier.BattleCry06` 1060/1.30, `Soldier.Cheers01` 1065/1.91, `Soldier.GoodJob02` 1096/1.23 |
| Pyro | `PlayerKilledDominatingPyro` (none): `Pyro.LaughEvil01` 1450, `LaughEvil02` 1482, `LaughHappy01` 1483, `LaughLong01` 1485 (all 4.00) | none: `Pyro.ActivateCharge01` 1403/4.00, `Pyro.BattleCry01` 1418/4.00 |
| Demoman | `PlayerKilledDominatingDemoman` (none): `Demoman.Cheers05` 886/4.31, `LaughEvil01` 910/3.94, `LaughEvil02` 950/3.44, `LaughEvil03` 1379/3.24, `LaughEvil04` 1384/2.81, `LaughEvil05` 1385/2.31, `LaughHappy01` 951/3.63, `LaughLong01` 953/4.00, `LaughLong02` 954/4.00, `LaughShort02` 956/2.58, `LaughShort05` 1383/2.06 | none: `Demoman.GoodJob01` 914/1.07, `Demoman.PositiveVocalization02` 990/4.41, `Demoman.SpecialCompleted09` 1005/5.54, `Demoman.Taunts16` 1025/7.16 |
| Heavy | `PlayerKilledDominatingHeavy` (none), 32 lines: `Heavy.Domination01`–`09` 1948/4.04, 1950/3.67, 2074/4.35, 2075/3.71, 2076/3.87, 2077/4.64, 2078/3.69, 2079/5.58, 2080/4.04; `Heavy.Domination12`–`18` 2083/3.69, 2084/5.47, 2085/3.04, 2103/4.91, 2115/5.37, 2194/4.21, 2256/4.12; `Heavy.GoodJob02` 235/1.66, `Heavy.Jeers06` 263/5.62, `Heavy.LaughEvil01` 267/4.03, `LaughHappy01` 268/3.45, `LaughHappy02` 269/4.56, `LaughHappy03` 1268/5.06, `LaughHappy04` 1269/4.33, `LaughHappy05` 1272/4.49, `LaughLong01` 270/9.64, `LaughLong02` 271/5.74, `LaughterBig02` 2067/3.98, `LaughterBig03` 2265/2.78, `LaughterBig04` 2266/5.76, `PositiveVocalization01` 303/3.46, `PositiveVocalization02` 304/3.24, `Taunts12` 336/6.77 | none, 18 lines: `Heavy.Cheers03` 213/5.76, `Cheers06` 216/4.56, `PositiveVocalization03` 305/4.18, `Heavy.Revenge01`–`15` 1951/3.67, 1952/3.40, 1953/3.69, 2100/4.78, 2101/3.75, 2102/3.33, 2104/3.87, 2105/3.73, 2108/3.78, 2109/6.02, 2110/5.38, 2111/3.27, 2112/5.59, 2113/3.47, 2114/5.99 |
| Engineer | – | none, 18 lines: `engineer_revenge01` 3579/3.12, `Engineer.BattleCry07` 1329/5.47, `Cheers01` 40/4.13, `Cheers06` 39/3.56, `Cheers07` 44/6.29, `Jeers04` 98/5.47, `LaughEvil01` 100/4.38, `LaughEvil02` 1331/5.79, `LaughEvil05` 1335/4.52, `LaughEvil06` 1336/5.57, `LaughHappy01` 101/5.88, `LaughHappy02` 1334/5.69, `LaughHappy03` 102/5.44, `LaughLong01` 103/8.41, `SpecialCompleted03` 149/1.55, `Taunts02` 164/3.59, `Taunts04` 168/3.38, `engineer_revenge02` 3696/3.10 |
| Medic | `PlayerKilledDominatingMedic` (none): `Medic.LaughEvil02` 1227/1.80, `LaughEvil05` 1232/5.62, `LaughHappy01` 605/3.42, `LaughHappy02` 606/4.26, `LaughHappy03` 1229/2.98, `LaughLong01` 607/4.56, `LaughLong02` 608/7.32, `LaughShort03` 1230/2.49, `Taunts12` 679/4.83, `Taunts14` 1240/4.32 | none: `Medic.GoodJob02` 569/4.00, `NegativeVocalization02` 620/3.75, `NegativeVocalization06` 1233/3.32, `NegativeVocalization07` 1234/3.06 |
| Sniper | – | `50PercentChance`: `Sniper.Cheers05` 1622/2.78, `GoodJob01` 1641/2.41, `PositiveVocalization03` 1708/1.75, `PositiveVocalization04` 1709/1.99, `Taunts04` 1741/3.79, `Taunts03` 1740/3.62, `Taunts15` 1752/1.78, `Taunts16` 1753/2.41 |
| Spy | – | none: `Spy.Cheers01` 710/5.18, `Cheers06` 715/5.03, `GoodJob01` 743/5.10, `PositiveVocalization04` 812/4.95, `PositiveVocalization05` 813/4.99, `Taunts16` 841/5.06, `Spy.Revenge03` 3017/4.15, `Revenge01` 3038/1.83, `Revenge02` 3039/3.33 |

Within a cell, an entry written without a class prefix keeps the prefix
of the entry before it (e.g. `LaughEvil02` after `Soldier.LaughEvil01` is
`Soldier.LaughEvil02`), and `Sniper.SpecialCompleted25`, `30`, … means
`Sniper.SpecialCompleted30`, ….
