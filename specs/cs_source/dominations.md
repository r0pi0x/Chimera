# Counter-Strike: Source: dominations and revenge

Source basis: Valve's public Source SDK 2013, whose only game with
dominations is Team Fortress 2: I read its rule for when a kill starts a
domination or is a revenge, the per-pair kill counters behind it and when
they are cleared, the per-player relationship flags and how they are
networked, the kill feed's extra lines and sounds, and the freeze panel's
nemesis line. CS:S's own game DLL is **not** in the SDK. From the CS:S
install I read data only: the game-event definitions (`modevents.res`), the
freeze panel layout (`resource/ui/freezepanel_basic.res`), the HUD layout,
the English localization (`cstrike_english.txt`), the HUD materials and
their texture sizes, the achievement names, and every sound script. Every
CS:S rule below is either data from those files or marked *inferred* (TF2's
rule, assumed carried over because CS:S uses the same event fields, panel,
strings and icons) with a way to check it.
Measured 2026-11-14 on a CS:S dedicated server (the probe server, with a
small SourceMod plugin of ours that credits kills between bots through the
game's own damage path, logs every `player_death` and reads each player's
networked relationship flags): the threshold, the event values, team
kills, suicides, and clearing on round end, `mp_restartgame`, team change
and disconnect. Those rules are no longer inferred. The client side (freeze
panel, scoreboard, kill feed) was not captured: no display was available.
Status: draft

## Summary

A player who kills the same opponent four times without being killed by
that opponent in between starts **dominating** them; the victim now has a
**nemesis**. The dominator keeps that status until the dominated player
kills them back, which is a **revenge** and ends the domination. The kill
that starts a domination and the kill that takes revenge are flagged in the
`player_death` event (`dominated`, `revenge`), and the clients show it on
the victim's freeze panel ("IS YOUR NEW NEMESIS", "IS STILL YOUR NEMESIS",
"HAS GOTTEN REVENGE") and as icons on the scoreboard. CS:S ships no sound
for any of it.

## Units and conventions

- Players are identified on the server by entity index (1..maxplayers) and
  in events by user ID (`userid`, `attacker`).
- "K kills V" means V died and K is credited with the kill (the event's
  `attacker`). K ≠ V.
- No time base: everything happens on a death.

## Constants

| Name (ours) | Value | Unit | Meaning | Source |
|---|---|---|---|---|
| domination_kills | 4 | kills | unanswered kills of one player by another that start a domination | measured (CS:S server, 2026-11-14) |
| freeze icon (nemesis) | `hud/freeze_nemesis` | material, 64×64 texture | freeze panel icon for a new or continuing nemesis | CS:S data |
| freeze icon (revenge) | `hud/freeze_revenge` | material, 64×64 | freeze panel icon for a revenge | CS:S data |
| freeze icon (dominated) | `hud/freeze_dominated` | material, 32×32 | small icon; use not known (Q4) | CS:S data |
| scoreboard icons | `hud/scoreboard_dominated`, `hud/scoreboard_nemesis`, and `-dead` variants `hud/scoreboard_domination-dead`, `hud/scoreboard_nemesis-dead` | materials, 64×64 | scoreboard markers, alive and dead | CS:S data |

All the icon materials are unlit, translucent; the four non-`-dead` ones
also take vertex colour and alpha (they can be tinted and faded); the two
`-dead` ones do not.

## Behavior

### 1. Kill counters (server)

The server keeps, for every ordered pair of players (A, B), a count
`U(A←B)`: how many times B has killed A since A last killed B ("unanswered
kills"). All counts start at 0.

When K kills V (measured in CS:S, 2026-11-14; same order as TF2):

1. Read `n = U(V←K) + 1` (this kill included).
2. Decide the flags, first match wins:
   - **Domination:** if `n` equals exactly 4 and K and V are on different
     teams, K starts dominating V. Set
     the event's `dominated` flag. Record "K dominates V" (and its mirror,
     "V's nemesis is K").
   - **Revenge:** otherwise, if V is currently dominating K and they are on
     different teams, the kill is a revenge. Set the event's `revenge` flag
     and clear "V dominates K".
   - **Already dominating:** otherwise, if K already dominates V, nothing is
     flagged (TF2 counts it for stats and achievements only; CS:S's
     "Overkill" achievements count the same thing).
3. Update the counters: `U(V←K) += 1`, and `U(K←V) = 0` (K has answered
   every kill V had on them).

Consequences worth testing:

- The fifth, sixth, ... unanswered kills do **not** re-flag a domination
  (the test is "equals 4", not "at least 4").
- A revenge does not by itself start anything: K's counter on V is only 1
  after the revenge kill.
- Two players can only dominate each other in one direction: as soon as
  the dominated one kills back, it is a revenge and the domination ends.
- One kill flags at most one of `dominated` and `revenge` (CS:S has no
  assists, so TF2's assister flags do not apply).

- **Team kills** (measured with `mp_friendlyfire 1`) never flag, never
  start a domination and never take revenge (a dominated player who
  team-kills their dominator leaves the domination in place), but they
  **do update the counters** (step 3) like any kill. So three team kills
  followed by one kill as enemies start a domination (that kill is the
  4th), and four team kills followed by enemy kills never do (the 4th was
  spent on a team kill; the next is the 5th). Players only become
  teammates with live counters through a forced switch that keeps them
  (Behavior 2).

Not counted (measured: a suicide credited to the victim and a death to
the world): suicides and deaths with no credited attacker change no
counter and no relationship. *Inferred* the same for falling, the bomb
and triggers.

### 2. Clearing (server)

Measured in CS:S (2026-11-14):

| Event | Relationships | Counters |
|---|---|---|
| round end and the next round's start | kept | kept |
| `mp_restartgame` (scores reset to 0) | kept | kept (3 kills before it + 1 after = a domination) |
| the player joins another team (the normal team change, which kills them) | the player's, both directions, cleared | the player's row and column cleared (3 kills before + 1 after = no domination) |
| a forced switch that does not kill the player (as a server plugin's team switch does) | kept | kept |
| the player disconnects (kicked bot) | cleared both directions; the slot's next player starts clean | cleared |

A map change reconnects everyone, so it clears by the disconnect rule
(*inferred*, not run). Whether the game's own automatic team balance uses
the clearing or the keeping kind of switch was not checked (Q3).

### 3. What the server sends

- **`player_death` event** (CS:S `modevents.res`, broadcast to every
  client): `userid` (short), `attacker` (short), `weapon` (string),
  `headshot` (bool), `dominated` (short), `revenge` (short). Measured:
  `dominated` is 1 on the kill that starts a domination, else 0 (also 1,
  not 2, for a player's second simultaneous domination); `revenge` is 1
  on a revenge kill, else 0. Never both.
- **Relationship flags** (measured from the server's network tables):
  two arrays of 66 one-bit flags on each player, indexed by entity index,
  "I dominate player i" and "player i dominates me", both in the part of
  the player's data sent **only to that player's own client**. So a client
  knows its own relationships and nobody else's: a scoreboard can mark
  only the local player's nemeses and victims (Q4 answered).
- Nothing else: no chat message, no centre print (the CS:S localization has
  no "is dominating" / "got revenge" text, only the freeze panel and
  scoreboard words), and the server log's kill line has no domination or
  revenge marker (checked with `mp_logdetail 3`).

### 4. Freeze panel nemesis line (client, the victim's screen)

CS:S's freeze panel (`freezepanel_basic.res`) is a 220×56 box at
horizontal centre − 110, y 140 (640×480 layout units), black at alpha 192.
Inside: the killer's name (grey 204,204,204), two red (184,0,0) info lines
(`InfoLabel1` small font at y 19, `InfoLabel2` large font at y 26, both 100
wide at x 60), the "[F5] Save this moment" line, the killer's avatar
(32×32 at 12,11 with a 64×64 glow behind it), a killer health gauge (32×3 at
12,45) and a **domination icon** 36×36 at x 172, y 9 whose default image
is `hud/freeze_nemesis`.

When the local player dies, the two info lines and the icon come from the
`player_death` event and the relationship flags (*inferred* order, TF2's):

| Case | Line 1 | Line 2 | Icon |
|---|---|---|---|
| the event has `revenge` (the killer avenged themselves on you: you had been dominating them) | HAS GOTTEN | REVENGE | `hud/freeze_revenge` |
| the event has `dominated` and the killer still dominates you when the panel opens | IS YOUR NEW | NEMESIS | `hud/freeze_nemesis` |
| no flag, but the killer already dominates you | IS STILL YOUR | NEMESIS | `hud/freeze_nemesis` |
| none of these | KILLED YOU | (empty) | hidden (*inferred*) |

Strings (English): `FreezePanel_NewNemesis1/2` "IS YOUR NEW" / "NEMESIS",
`FreezePanel_OldNemesis1/2` "IS STILL YOUR" / "NEMESIS",
`FreezePanel_Revenge1/2` "HAS GOTTEN" / "REVENGE",
`FreezePanel_Killer1/2` "KILLED YOU" / "". Shorter forms also exist
(`CS_FreezeNemesis` "Nemesis", `CS_FreezeNewNemesis` "New Nemesis",
`CS_FreezeRevenge` "Revenge"); where they are used is Q5.

TF2 re-checks "the killer still dominates me" when the panel is shown (the
victim may have taken revenge after dying, e.g. with a grenade); keep that
check.

### 5. Scoreboard (client)

CS:S has a column headed "Dominate" and one headed "Nemesis"
(`Cstrike_ScoreBoard_Dominate`, `Cstrike_ScoreBoard_Nemesis`)
(*inferred*: columns or a legend; Q6). On the local player's scoreboard,
*inferred* from the material names:

- a player **you are dominating** shows `hud/scoreboard_dominated`
  (`hud/scoreboard_domination-dead` while that player is dead);
- a player **dominating you** (your nemesis) shows `hud/scoreboard_nemesis`
  (`hud/scoreboard_nemesis-dead` while dead).

### 6. Kill feed (client)

The CS:S death notice list (`HudDeathNotice`: at most 4 lines, 22 units
high, right-justified, team-coloured names) has no text line for
dominations or revenges (TF2 adds "X is DOMINATING Y" lines; CS:S has no
such strings). *Inferred, low confidence:* the line of a dominating or
revenge kill carries an icon (the 32×32 `hud/freeze_dominated` is the right
size for a 22-unit line). Q4 says how to check.

### 7. Sounds

None. CS:S's sound scripts (every `game_sounds*.txt`, `level_sounds*`)
have no domination, nemesis or revenge entry, and no such wave exists in
any of its or the shared HL2 packs. (TF2 plays its own entries to the
killer and victim only; CS:S does not have them.) The freeze cam's own
sound is `ui/freeze_cam.wav` (spectating.md), unrelated to dominations.

## Per-tick order

On a player death (server), after the kill is credited and before the
`player_death` event is fired:

1. Compute the flags (Behavior 1, step 2) from the counters as they stood
   before this kill.
2. Update relationships (set or clear).
3. Update the counters (step 3).
4. Fire `player_death` with `dominated` and `revenge`.
5. The relationship change reaches clients with the next snapshot; the
   event may arrive first, so a client deciding "still my nemesis" must
   tolerate either order (TF2 reads the flags when the freeze panel opens,
   seconds later).

## Edge cases

- **Kill by a dead player** (grenade after death): counts like any kill.
- **Two kills in one tick** (K and V kill each other): processed in the
  order the deaths are handled; the second one sees the first's counter
  update. If V was dominating K and both die on the same tick, whichever
  death is processed second may be a revenge.
- **Bots** take part like humans (measured: every test above was between
  bots).
- **Team kills** count toward the counters but never flag (Behavior 1);
  a forced switch that keeps counters can leave a domination between
  teammates, and a team kill does not end it.
- **Player slots reused**: counters are per slot, so a disconnect must
  clear them (Behavior 2) or the next player in the slot inherits them.

## Quirks

- The exact-equals test means a player who is dominated, takes revenge and
  is then killed four more times is dominated again (counter restarted at
  the revenge). Keep.
- Dominations persist across rounds and even `mp_restartgame` (measured),
  so in CS:S's short rounds they build up over a whole map. Keep.
- A team kill can "use up" the 4th kill (Behavior 1): the domination then
  never comes until the victim answers. Keep for parity.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| fresh pair A, B | A kills B three times | 3rd death | `dominated` 0 each time; no relationship |
| fresh pair | A kills B four times | 4th death | 4th event `dominated` 1, `revenge` 0; A dominates B |
| A dominates B | A kills B again (5th) | – | `dominated` 0, `revenge` 0; A still dominates B |
| A dominates B | B kills A | – | `revenge` 1, `dominated` 0; A no longer dominates B; U(A←B) = 1, U(B←A) = 0 |
| A killed B 3 times | B kills A once, then A kills B 3 more times | – | no domination (A's count restarted at 0 by B's kill; now 3) |
| A dominates B, revenge taken | A kills B 4 more times | 4th | `dominated` 1 again |
| A dominates B | A disconnects | – | B has no nemesis; all counters with A are 0 |
| A dominates B | B changes team (joins the other team) | – | relationship cleared; B's counters 0 |
| A killed B 3 times | `mp_restartgame`, then A kills B | 4th | `dominated` 1 |
| A dominates B | round ends, new round | – | A still dominates B |
| A, B teammates, friendly fire on | A kills B 4 times | each | `dominated` 0, no relationship |
| A, B teammates | A kills B 3 times; B is moved to the other team without dying; A kills B | that kill | `dominated` 1 |
| A, B teammates | A kills B 4 times; B is moved to the other team; A kills B | that kill | `dominated` 0 |
| A dominates B, then B is moved onto A's team without dying | B kills A | – | `revenge` 0; A still dominates B |
| A dominates B and C | – | – | each of the two 4th kills had `dominated` 1 |
| A dominates B | A suicides, or dies to the world | – | no change; B's next kill of A is a revenge |
| local player V, killer K | K's 4th unanswered kill | freeze panel | "IS YOUR NEW" / "NEMESIS", icon `freeze_nemesis` |
| K dominates V | K kills V again | freeze panel | "IS STILL YOUR" / "NEMESIS" |
| V dominates K | K kills V | V's freeze panel | "HAS GOTTEN" / "REVENGE", icon `freeze_revenge` |
| no relationship | K kills V | freeze panel | "KILLED YOU", no icon |
| A suicides | – | – | no counters change |
| A dominates B | A disconnects; a new player takes A's slot | – | B has no nemesis; the new player has no relationships |

## Open questions

1. **Answered 2026-11-14:** 0/1 (Behavior 3). Was: **`player_death` field values.** Are `dominated`/`revenge` 0/1, or a
   count (e.g. the dominator's number of dominations)? Check: on the probe
   server (SourceMod), log every `player_death` with these fields while
   bots fight with `bot_quota 2` until one dominates the other.
2. **Answered 2026-11-14:** they never flag but do update the counters
   (Behavior 1). Was: **Team kills** (`mp_friendlyfire 1`): do they count toward a domination
   or revenge? Same probe, two bots on one team with friendly fire.
3. **Mostly answered 2026-11-14** (Behavior 2): round restarts and
   `mp_restartgame` keep everything; joining a team or disconnecting
   clears. Still open: which kind of switch the automatic team balance
   (`mp_autoteambalance`) makes, and a map change (expected: cleared).
   Was: **Clearing:** does a team change, `mp_restartgame` or a round restart
   clear relationships? Probe: make a domination, then do each, and see
   whether the next kill by the dominated player is flagged `revenge`.
4. **Half answered 2026-11-14:** relationships reach only their own
   player's client (Behavior 3), so no third client can show them. The
   kill feed icon is still open. Was: **Kill feed and who sees what.** Does the kill feed show an icon on a
   dominating or revenge kill, and which (`hud/freeze_dominated`?)? Are
   relationships between two other players visible to a third client (on
   the scoreboard)? Needs a client capture: a human client watching two
   bots, screenshots of the kill feed on the 4th kill and of the
   scoreboard after.
5. **Short strings** `CS_FreezeNemesis`, `CS_FreezeNewNemesis`,
   `CS_FreezeRevenge`: used by the freeze cam callouts, the scoreboard, or
   unused? Capture.
6. **Scoreboard layout:** where the "Dominate" and "Nemesis" headings and
   the icons sit, and their size. Capture with a domination in both
   directions.
7. **Answered 2026-11-14:** 4, measured (the 4th unanswered kill flags,
   the 5th and 6th do not). Was: **Whether the 4-kill threshold is the same in CS:S.** The achievement
   texts fit 4 but do not prove it; Q1's probe settles it.
