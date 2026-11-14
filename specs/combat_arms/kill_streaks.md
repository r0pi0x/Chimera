# Combat Arms: kill streaks and special-kill callouts

Source basis: reference material the user pointed the spec session to: the client's handling of the kill message (who killed whom, the multi-kill and first-kill fields, hit location, revenge and lucky-shot flags), the client's centre-screen effect banner (its animation, replacement rule and sound helper), the client UI image's scaling, the client's interface-sound path and the server's per-player match statistics. The first version of this spec did not find the client panel that turns a kill into a particular banner, side-log line and voice; it inferred those rules. A second pass (2026-10-28) found that panel in the reference and settled them: A2, A3, A6, B1, B2 and B4 now follow it, and "Corrections (2026-10-28)" lists what changed. Statements still marked "(inferred)" were not covered by that panel. A third pass (2026-10-28) specified the plain-kill banner in full (B2a) and the side log's behaviour (A7), from the same panel, the banner player's digit handling and the install's archive directories (names and sizes only). Data was read from the Steam install on the Linux dev box through the repo's own archive reader: the HUD layout attributes (`attributes/layout.txt`, `[HUDLayout0]`, `[RevengeSystem]` and the weapon-effect sections), the client attributes (`attributes/clientbutes.txt`, `[Interface]` and the weapon-effect sections), the weapon list (`attributes/weapons.txt`, `HUDEffectOverride`), the main HUD layout (`xml/hud_main.xml`, control `Effect`), the English string table, the archives' file lists and the sound files' headers (lengths only).
Status: draft

Sections: Summary; Units and conventions; Part A, the rules (A1 what the server counts and sends, A2 which event a kill raises, A3 who sees and hears what, A4 the banner player, A5 the banner animation, A6 the voice player, A7 the side log, A8 weapon skins, A9 options); Part B, the Combat Arms pack (B1 steps, B2 special kills, B3 banner layout, B4 timings, B5 voice files, B6 side log, B7 weapon skins, B8 unused art); Per-frame order; Edge cases; Quirks; Corrections (2026-10-28); Corrections (2026-10-28, plain kill); Test cases; Open questions.

---

## Summary

- The server counts each player's **multi-kill**: enemy kills each less than 5 s after the previous one. Every kill message carries that count, a first-kill flag, the hit location (head, groin, ...), and revenge and lucky-shot flags. Every client receives every kill message.
- The **killer's own client** shows a big animated banner at the top centre of the screen (DOUBLE KILL, MULTI KILL, ULTRA KILL, FANTASTIC, UNBELIEVABLE, UNBELIEVABLE+; also FIRST KILL, HEADSHOT, NUTSHOT, REVENGE, LUCKY SHOT, and "KILL"/"N KILLS" for a plain kill) and plays an announcer voice. The banner slams in from 5x size, throws off a growing "echo" copy while it shakes, holds, then blows up and fades. The whole thing lasts 1.4 s.
- **One kill, one announcement.** A kill picks exactly one event by a fixed priority (first kill, revenge, streak step, headshot or groin shot, plain kill). That one event gives the banner and **at most one voice**. A headshot double kill shows DOUBLE KILL with the skull drawn over it and says only "double kill". Voices of different kills are never stopped, queued or skipped: kills in quick succession overlap.
- Only one banner shows at a time. Each banner kind has a **priority level**: a new banner replaces the current one unless the current one has a higher level, in which case the new one is dropped.
- A weapon can carry a **skin** (a named section of data) that swaps the banner art and some voice files for kills made with it.

The split asked for by the engine plan: **Part A** is rules any game's announcer would share (counting, signalling, banner replacement, animation shape, sound playback). **Part B** is Combat Arms' data: step names and thresholds, art, sounds, layout, timings, levels and skins. An implementation can turn all of Part B into an "announcer pack" file; nothing in Part A names a Combat Arms file or number except as a pack value.

## Units and conventions

- Time in seconds (s) or milliseconds (ms) as marked. Server times are server clock; banner times are the client's frame time.
- Screen positions are in the 1024 x 768 reference frame of `ui_system.md`: x and widths are multiplied by W/1024, y and heights by H/768 for a W x H screen. Nothing is anchored to an edge.
- Alpha 0..255. "Scale" is a size multiplier applied about the centre of the image's rectangle.
- Pack keys in `code` are the install's own data keys (players' installs carry them), quoted so an implementation can read the stock data directly.

## Part A: the rules

### A1. What the server counts and sends

**Multi-kill count** (per player, per match). Already fixed in `gear_perks.md` (Constants, Corrections 10); repeated here because the announcer reads it:

- An enemy kill made less than `window` after the same player's previous enemy kill raises the count by 1. Otherwise the count restarts at 1.
- Exactly `window` after the previous kill restarts it (strictly less than).
- Kills of teammates and of oneself neither raise nor reset it.
- `window` is a pack value (Combat Arms: 5000 ms, B1).
- **Dying does not reset it** (answered 2026-11-14 from the server's death and respawn handling; high). Neither the killer's death nor its respawn touches the count or the time of its last enemy kill (the reference even carries a disabled reset of that time in the respawn path). Only these reset them: the player joining (both start at 0, so a first kill always starts at 1) and the mode's end-of-mission handling, which re-initialises every player's statistics (for the elimination modes at the match's end; some round-based PvE modes per round; inferred per mode, medium). So a kill, a death and, after respawning, another enemy kill less than 5 s after the first is a double kill.
- The time compared is the server's millisecond clock at each enemy kill. A kill made by an AI squad member counts for its leader player. In PvE modes a player's kills of AI share the same "last kill time" (it is also stamped by AI kills for the mode's combo counter), they do not raise the multi-kill count themselves, but an enemy player kill less than 5 s after one continues the count instead of restarting it.

**Other per-kill facts** the server works out:

- **First kill**: a flag (one byte, nonzero = set) on the kill message when this kill is the first one the server has marked as "first kill" (Open question 3 for its scope, round or match).
- **Hit location**: of the killing hit. Head for a headshot, groin ("nut") for a groin shot. The server downgrades head to torso for damage types that cannot headshot (area, over-time and thrown-weapon damage; `death_killcam.md` 11).
- **Revenge**: set when the victim is the killer's revenge target (the player who had killed them repeatedly; `death_killcam.md` 11, revenge fields).
- **Lucky shot**: a flag; the server rolls it with a rate it receives from the lobby (Open question 6).

**The kill message.** One message per death, sent reliably to **every** client: victim, killer (255 = none), multi-kill count (one byte), weapon id, hit location, first-kill flag, revenge flags, lucky-shot flag and the other fields in `death_killcam.md` 11. There is no separate "streak" message: every announcement is worked out on the clients from this one message.

**Match statistics** (server, per player, sent to the account at the match's end; not shown live): number of first kills, headshot kills, groin-shot kills, groin-shot deaths, and one counter per streak step (double, multi, ultra, fantastic, unbelievable, unbelievable+). Separately, a **kill streak** (enemy kills without dying, no time limit) and the longest one in the match are kept. That is a different number from the multi-kill count. It is logged at the match's end and announced nowhere. The client also keeps, per player, one counter per multi-kill count value it has seen in kill messages (for the scoreboard; Open question 8).

### A2. Which event a kill raises

On every kill message that names a killer, each client works out **exactly one event** for that kill. The inputs: whether the local player is the killer, the multi-kill count n, the first-kill flag, the revenge flag, the ice-crush flag (Snowball Fight), the victim's VIP rank (VIP modes), and the local **head and groin flags** (A3: refreshed from the hit location only on the local player's own kills). The first rule that matches picks the event:

1. The victim has a VIP rank above 0 and the kill is a step kill or a first kill (n > 0 or first-kill flag) → **VIP kill**, level "event kill".
2. First-kill flag → **first kill**, level "event kill". The count is not looked at.
3. Ice-crush flag → **ice crush**, level "event kill".
4. Revenge flag → **revenge**, level "event kill". A revenge kill that is also a double kill or a headshot shows REVENGE, not the step or HEADSHOT.
5. n = 1 (no streak):
   1. head flag set → **headshot**, level "special shot". The head flag is cleared here.
   2. otherwise groin flag set → **groin shot** ("nutshot"), level "special shot". The groin flag is cleared here.
   3. otherwise, if the local player is the killer (and the client has the local player's scoreboard entry) → **plain kill**, level "kills", with the number N = the local player's scoreboard kill total read just after this kill message added to it (enemy kills only; nothing is added in a training round or in Bounty Hunter's second stage). N ≤ 1 uses the single "KILL" image, N > 1 the "KILLS" image. The number N is drawn in both, and N = 0 draws no digits. B2a has the full rules.
   4. otherwise nothing.
6. n from 2 to 6 → that **step** (B1); n ≥ 7 → Unbelievable+. Level "special shot" (the same level as a headshot). n = 0 (or anything else) → nothing.

So a headshot or groin shot is announced as such **only on a kill that is not a streak step**, not a first kill and not a revenge. On those other kills the head and groin flags still matter, but only for the top image and as a voice fall-back (A4, A6).

What the chosen event does:

- **Side log** (every client, A7): a line for a VIP kill, first kill, ice crush or step. Revenge, headshot, groin shot and the plain kill get none: the plain kill picks a line kind that has no bar image, and the side log skips such kinds (Corrections (2026-10-28, plain kill) 2).
- **Banner and voice**, only when the local player is the killer:
  - Banner request (A4) with the event's image and level; the number N for a plain kill (none otherwise); and a **top image**: the skull if the head flag is still set at this point, else the nut if the groin flag is still set, else none. The VIP kill never gets a top image. Because rule 5 clears the flag it used, a single-kill HEADSHOT or NUTSHOT banner has **no** top image, while a headshot DOUBLE KILL, FIRST KILL or REVENGE has the skull over it (Quirk 7).
  - **One voice** at most (A6): the event's own voice if the pack (with the skin, A8) has one; otherwise the headshot voice if the head flag is still set and the pack has one; otherwise the groin-shot voice if the groin flag is still set and the pack has one for the local player's gender. Otherwise silence.

The **lucky shot** is separate: after the kill's event, if the lucky-shot flag is set and the local player is the killer, a lucky-shot banner is requested at level "lucky shot" (no voice). Its level is above every kill event, so it replaces the kill's banner on that same kill. Everyone also gets the extra kill-feed line (A3). In Snowball Fight the killer's client also requests the frozen-kill banner after the kill's event (level "snowball kill"), which likewise replaces it.

In co-operative mission modes (kills of AI enemies), the same banner player shows "N KILLS" with N in digits, or "COMBO" with a number when the server's co-op combo count is above 1, instead of the steps. Those numbers come from the co-op kill message, not from the multi-kill count. They are pack data (B2) and outside this spec's main scope.

### A3. Who sees and hears what

| What | Who | Notes |
|---|---|---|
| Kill-feed line with the headshot / groin-shot / revenge badges | everyone | `hud_crosshair.md` D5. The headshot badge whenever the location is head, killer or not |
| Lucky-shot extra kill-feed line | everyone | a second line for the same kill, showing only the lucky-shot icon (no other badges) |
| Centre banner (A4, A5) | the **killer's own client** only | other players' kills never show a banner |
| Announcer voice (A6) | the killer only, local, no position | |
| Side-log line (A7) | everyone, for the events A2 lists | small bar on the left edge, "name STEP" |
| Killer's own "enemy kill" grunt | the killer only | the character's voice, not the announcer: `effects_sounds.md` Corrections 8. Not on a groin-shot kill |
| Victim's hurt sounds (head: `HEADSHOT.WAV` grunt, groin: `NutPain.wav`) | everyone near, 3D | `effects_sounds.md` 11. They are not announcer lines |

The headshot and groin-shot flags the banner panel uses are only updated when the local player is the killer (the client compares the killer's name with its own). For anyone else's kill they keep their last value (Quirk 3). A flag stays set after a kill that did not use it (a headshot step kill) until the local player's next kill rewrites it, or until another player's single kill passes rule 5 of A2 on this client and clears it there.

### A4. The banner player

There is one banner area. It holds a small pool of banner slots, but at most one is visible.

A **request** carries: the banner image, the banner's **level** (an integer; higher wins), an optional number (drawn in digits), an optional top image (drawn over the banner, e.g. the skull over HEADSHOT) and optional custom timings (A5).

On a request with level ℓ:

1. Let c be the level of the banner now visible (0 if none).
2. If c > ℓ: **drop the request**. Nothing is shown, nothing is queued, and the visible banner keeps playing. Equal levels replace.
3. Otherwise hide every banner at once (no fade-out) and start the new one at t = 0. If the same image was shown before, its slot is reused and restarted. Otherwise any hidden slot is reused, else a new slot is made. The started slot is moved to the top of the draw order.
4. Set the banner image. Hide all digits and the top image, then:
   - number N > 0: the ones digit always; the tens digit when tens or hundreds is nonzero; the hundreds digit when hundreds is nonzero. N ≥ 1000 shows only its last three digits.
   - top image given: show it.

There is **no queue**. A request that loses to a higher level is gone, even if that banner ends 0.1 s later. A step that arrives while a lower or equal banner plays cuts it off at once.

**Sound is separate.** The voice of a kill (A6) is played right after its banner request, whether or not that request was dropped. A double kill whose banner loses to a showing FIRST KILL still says "double kill".

On entering a world (map load, reconnect) every banner is hidden and its timer stopped.

A second, independent **text line** shares the area: a centred text (used by VIP Elimination for "+N Points"). It has its own single slot and the same timing as a default banner. Banner levels do not apply to it.

### A5. The banner animation

Each banner is drawn twice: the **main** copy and an **echo** copy of the same image (and the same digits and top image), the echo on top. Times are from the request (t = 0). Every time below is a pack value (B4):

| Symbol | Meaning |
|---|---|
| T | total time. The banner is hidden on the first frame with t ≥ T |
| t₀ | start of the slam-in |
| c | slam-in length |
| b | echo ("blow") length; starts when the slam-in ends |
| tₑ | start of the blow-out |
| d | blow-out length |
| S₀ | main copy's scale at the start of the slam-in |
| B | echo's final scale |
| L | main copy's final scale at the end of the blow-out |

**Main copy:**

- **Slam-in**, for τ = t − t₀ in [0, c): scale shrinks with an ease-in quartic,
  ```math
  s = S_0 - (S_0 - 1)\left(\frac{\tau}{c}\right)^4
  ```
  and alpha follows the scale, transparent at S₀ and opaque at 1:
  ```math
  a = \operatorname{clamp}\left(255 - \frac{(s-1)\,255}{S_0-1},\ 0,\ 255\right)
  ```
  (truncated to an integer).
- **Hold**: scale 1, alpha 255.
- **Blow-out**, for τ = t − tₑ in [0, d): scale grows with an ease-out quartic,
  ```math
  s = L - (L - 1)\left(\frac{d-\tau}{d}\right)^4
  ```
  with alpha 255 at scale 1 falling linearly to 0 at scale L (same formula as above with L in place of S₀).
- If the slam-in and the blow-out overlap (never with the stock numbers), the blow-out wins.

**Echo copy**: alpha 0 (invisible) except during the echo phase, τ = time since the slam-in ended, in [0, b): scale grows ease-out from 1 to B,
```math
s = B - (B-1)\left(\frac{b-\tau}{b}\right)^4
```
with alpha 255 at scale 1 falling linearly to 0 at scale B.

**Shake**: during the echo phase, each frame the **main** copy (with its digits and top image) is offset by a random whole number of pixels in each axis, uniformly −5..+5 (11 values). These are real screen pixels, not reference units. The echo copy does not shake. Outside the echo phase the offset is 0.

**Scaling**: every image is scaled about the centre of its own layout rectangle (after the screen stretch), so the banner grows and shrinks in place. Digits and the top image use the same scale and alpha as the copy they belong to.

**Phase starts are frame-driven**: a phase begins on the first frame at or after its start time, and the echo begins on the first frame after the slam-in timer is seen expired, so each start can be up to one frame late (Quirk 1).

**Custom timings**: a request may carry its own (T, tₑ, d). The other values stay the pack's. VIP Escort and some co-op banners use T = 3 s with d = 3 − (t₀ + c + b) (Quirk 4).

### A6. The voice player

- **One voice per kill, at most.** The kill's single event (A2) picks it: the event's own line; if the event has none in the pack, the headshot line when the head flag is still set; else the groin-shot line when the groin flag is still set. A kill never plays two announcer lines. Examples (stock pack): headshot double kill → "double kill" only; headshot first kill → "first kill" only; headshot revenge → "headshot" (revenge has no line, so the fall-back speaks); plain revenge → nothing.
- The file is looked up at the moment of the event, in the killing weapon's skin first (A8).
- Played as an **interface sound**: local (no position, no distance falloff), never passed through the map's sound filter (no reverb), medium priority, at the effects volume (`music_ambience.md` 8: effects master s × 1). Nothing plays when sound is disabled (`soundenable` 0).
- **Kills in quick succession**: each kill message plays its own line at once. A new line does **not** stop, wait for or replace the previous one, and nothing is queued or skipped: two kills 0.3 s apart play both lines, overlapping, and two kills in the same frame start both lines in that frame. There is no cooldown. (The banner area's own sound helper has a "skip if the previous sound is still playing" mode, used by some mode messages; the announcer never goes through it.)
- Whether its banner request won (A4) does not matter: the line plays either way.
- Events without a line in the stock pack: revenge, plain kill ("KILLS" has no key; "KILL" reads `OneKillSound`, but the install ships no file for it, Open question 11), lucky shot.

### A7. The side log

A short line per event on the left side ("NAME  DOUBLE KILL" on a coloured bar), stacked, each fading out after a few seconds. Which events get a line, and that every client adds it, is settled in A2: VIP kill, first kill, ice crush and step; not revenge, headshot, groin shot or the plain kill.

- **Skipped kinds:** a line kind whose pack bar image is empty is never added. The plain kill's two kinds ("KILL", "KILLS") have no bar image and no label string, so they add nothing.
- **A new line:** it takes the first hidden line slot. If none is hidden and 5 slots exist, the oldest line is dropped and its slot reused. Otherwise a new slot is made, so at most 5 lines show. The line shows the bar image, the player's name, and the label 5 ref px to the right of the name. The name is in `MKColor` when the player is on the local player's team, else in `MKEnemyColor`. The label has its own colour key (the label key + `Color`) when that is not black, else white. Lines are stacked downwards from the template's position, each as tall as its control.
- **Animation** (the bar and the label only; the name is neither scaled nor faded): let t be the time since the line was added, in ms.
  - t < `MKShowFirstSection`: scale 1 + (t / first)² · (`MKLargeScaleValue` − 1), and alpha min(t / first · 255, 255).
  - From first to `MKShowSecondSection`: scale falls back from `MKLargeScaleValue` to 1 by the same squared ease.
  - From `MKShowThirdSection` to `MKShowMessageTime`: alpha falls linearly from 255 to 0.
  - Once t > `MKShowMessageTime`: the **oldest** line is removed (whatever line timed out) and the stack is re-laid.
- **Open:** the line template's own position and size come from the keyed HUD layout (Open question 4).

### A8. Weapon skins

- A weapon may name a **skin** (`HUDEffectOverride` in the weapon list: the name of a data section).
- When a kill message is handled, the client first selects the killing weapon's skin (the kill message's weapon id), handles the kill, then clears it.
- While a skin is selected, every banner image and voice file is looked up first in the skin's section (the layout file's section for images, the client attribute file's section for sounds). A key the section lacks falls back to the default pack's value: the skin's table starts as a copy of the default one and only the keys present in the section replace entries. The skull and nut top images follow the same rule.
- So the skin follows the **killer's weapon**, on the killer's client.

### A9. Options and modes

- There is **no option** to turn the announcer, the banners or the voices off. The only controls are the general effects sound switch and volume (`options.md`), which mute every interface sound.
- The `MessageDur` HUD message multiplier (`options.md`) does not touch these timings.
- The counting rule is the same in every mode that has enemy kills. Kills during warm-up add nothing to the score (`game_modes.md` S7), but the kill message still carries a warm-up flag. Whether warm-up kills are announced was not found (Open question 9).

## Part B: the Combat Arms pack

### B1. Streak steps

| Count n | Step | Banner (`[HUDLayout0]` key → file) | Voice (`[Interface]` key → file) | Side-log label (string key) | Side-log bar |
|---|---|---|---|---|---|
| 1 | none: the kill is a headshot, groin shot or plain kill instead (A2 rule 5) | – | – | – | – |
| 2 | Double Kill | `MKComboDoubleKillImg` → `UI_HUD_Message\Effect\DoubleKill.dtx` | `DoubleKillSound` → `UI_SND\doublekill.wav` (1.41 s) | `MKDoubleKillImg` "DOUBLE KILL" | `bg_kill03.dtx` |
| 3 | Multi Kill | `MKComboMultiKillImg` → `...\MultiKill.dtx` | `MultiKillSound` → `UI_SND\multikill.wav` (1.47 s) | "MULTI KILL" | `bg_kill02.dtx` |
| 4 | Ultra Kill | `MKComboUltraKillImg` → `...\UltraKill.dtx` | `UltraKillSound` → `UI_SND\ultrakill.wav` (1.40 s) | "ULTRA KILL" | `bg_kill02.dtx` |
| 5 | Fantastic | `MKComboFantasticKillImg` → `...\fantastic.dtx` | `FantasticKillSound` → `UI_SND\fantastic.wav` (1.67 s) | "FANTASTIC" | `bg_kill02.dtx` |
| 6 | Unbelievable | `MKComboUnbelImg` → `...\Unbelievable.dtx` | `UnbelievableSound` → `UI_SND\Unbelievable.wav` (1.81 s) | "UNBELIEVABLE" | `bg_kill01.dtx` |
| ≥ 7 | Unbelievable+ | `MKComboUnbelPlusImg` → `...\Unbelievable_plus.dtx` | `UnbelievablePlusSound` → `UI_SND\Unbelievable.wav` (the same file as Unbelievable) | "UNBELIEVABLE+" | `bg_kill01.dtx` |

- Names and order are certain: they are the order of the server's six step counters, the layout keys, the string table and the art. The **thresholds** are the reference's: n = 2 to 6 map to Double to Unbelievable, any n ≥ 7 to Unbelievable+. Every step requests at the "special shot" level (B4). The string table's old "Triple kill" entry is not used by any of this.
- Window: 5000 ms, strictly less (A1).
- The step art is drawn without a number.

### B2. Special kills

| Event | Banner image | Top image | Voice | Level (B4) |
|---|---|---|---|---|
| First kill | `MKComboFirstKillImg` → `Effect\FirstKill.dtx` | – | `FirstKillSound` → `UI_SND\FirstKill.wav` (1.30 s) | event kill |
| Headshot | `MKComboHeadShotImg` → `Effect\HeadShot.dtx` | `MKComboSkullImg` → `Effect\Head.dtx` (skull) | `HeatShotKillSound` (sic) → `UI_SND\headshot.wav` (0.93 s) | special shot |
| Groin shot ("nutshot") | `MKComboNutShotImg` → `Effect\NutShot.dtx` | `MKComboNutsImg` → `Effect\Nut.dtx` | `NutShotMaleKillSound` → `UI_SND\NutShot_Male.wav` (1.87 s) or `NutShotFeMaleKillSound` → `UI_SND\NutShot_Female.wav` (1.57 s), by the gender of the **local player's own** body (the killer's: a worn cosmetic body's figure type if any, else the character's) | special shot |
| Revenge | `MKRevengeImg` → `Effect\Revenge.dtx`; the revenge panel's own copy `RevengeSuccessMsgImg` (same file, 250 x 101 at (512, 213), same timings as B4) | – | none in the default pack (`RevengeSound` exists only in the PSY skin), so the headshot or groin-shot line speaks instead when that flag is set (A6) | event kill |
| Lucky shot | `LuckyShotEffectEXImg` → `effect\LuckyShot_EXPass.dtx` (the plain `LuckyShotEffectImg` is never used) | – | none | lucky shot |
| VIP kill (VIP modes) | `MKVIPKillEffectImg` → `Effect\VIPKill.dtx` | – | `VipKillSount` (sic) → `UI_SND\VIP_kill.wav` | event kill |
| Ice crush (Snowball Fight, killer only) | the event's own image is `MKComboIceCrushImg` (its file is missing, B8), at once replaced by a second request `SnowballFrozenKill` → `effect\IceCrush.dtx` | – | `IceCrushSound` → `UI_SND\SnowBallFight\IceCrush.wav` | event kill, then snowball kill |
| Plain kill (A2 rule 5.3, B2a) and co-op single kills | `MKComboOneKillImg` → `Effect\Kill.dtx` "KILL" (N ≤ 1) / `MKComboOneKillsImg` → `Effect\Kills.dtx` "KILLS" (N > 1), with digits `MKComboNumberImg` + `0..9` + `.dtx` (`Effect\Number_0.dtx` to `Number_9.dtx`) | – | `OneKillSound` for "KILL" only, if the pack sets it. The stock install has no file for it (Open question 11, narrowed). "KILLS" has no voice key | kills |
| Co-op combo | `MKComboComboImg` → `Effect\Combo.DTX` with digits | – | – | combo |

### B2a. The plain-kill banner in full

- **When:** on the killer's own client, for every enemy kill by the local player that passes A2 rules 1 to 5.2 without a match. That is: the multi-kill count is 1 (no enemy kill by this player in the previous 5 s), the kill message has no first-kill flag, no ice crush and no revenge, the victim has no VIP rank, and the killing hit was neither head nor groin. Other players' plain kills show nothing anywhere: no banner, no voice and no side-log line.
- **The number N:** the local player's scoreboard kill count, read after this kill message added to it. The client adds 1 before choosing the event, for an enemy kill outside a training round and outside Bounty Hunter's second stage. So the first counted kill of a match shows "KILL" and 1, the second (if it is a plain kill) "KILLS" and 2, and so on. The count is the match total: it is not reset per round. A server update of the scoreboard entry replaces it.
- **Image choice:** N ≤ 1 → `MKComboOneKillImg` ("KILL"); N > 1 → `MKComboOneKillsImg` ("KILLS"). N = 0 can occur when the client did not add the kill (a training round before any counted kill, Bounty Hunter's second stage): the "KILL" image is shown with **no digits**.
- **Digits:** one image per digit, not a strip or a font. The file is the `MKComboNumberImg` value with the digit and `.dtx` appended: stock `UI_HUD_Message\Effect\Number_0.dtx` to `Number_9.dtx`, each 64 x 128, 32-bit (32,932 bytes in the archive). They are drawn in three fixed slots of the banner layout (B3), whose positions do not depend on how many digits show:
  - ones slot `Num_One` at (425, 175), always shown when N > 0;
  - tens slot `Num_Ten` at (385, 175), shown when the tens or the hundreds digit is not 0;
  - hundreds slot `Num_Hun` at (345, 175), shown when the hundreds digit is not 0.
  - Each slot is 64 x 128 and shows its whole digit image. The slots are 40 ref px apart, so neighbouring digits overlap by 24 px of (transparent) texture. A one-digit N stays in the ones slot: it is not centred.
  - Thousands are dropped: N = 1234 shows 2 3 4, and N = 1005 shows only 5 (hundreds and tens are both 0).
- **Animation:** the banner's (A5, B4). The digits belong to both copies. On each copy every digit takes that copy's scale and alpha, scaled about the centre of **its own** slot. At scale 4.75 the ones digit is a 304 x 608 rect centred on (457, 239), not a part of the banner image scaled about (512, 204). The main copy's digits take the same shake offset as its image.
- **Level and replacement:** level 1. It is dropped while any banner of level 2 or more shows. It replaces a showing HiSec pickup (0) or kill count (1).
- **Top image:** none. The plain kill is reached only when both the head and groin flags are clear.
- **Voice:** `OneKillSound` (`[Interface]`, or the weapon skin's section, A8) for the "KILL" image only. "KILLS" has no voice key, and since both flags are clear, no fall-back speaks. Either way no voice in the stock install (Open question 11).
- **Skins:** PSY, Graffiti and Turkey replace "KILL" and "KILLS" (B7). The digits are never skinned. Graffiti's "KILL" names a missing file (Quirk 5), so a plain kill with N ≤ 1 made with that weapon shows the digits over an empty banner image.
- **Lucky shot:** a lucky-shot plain kill shows the lucky-shot banner instead (A2: requested after the kill's event, at level 11).

No knife, grenade or other weapon-class callout exists. The only other kill-type marks are kill-feed icons (`hud_crosshair.md` D5).

### B3. Banner layout (`xml/hud_main.xml`, window `Effect`, 1024 x 768 reference, full screen)

| Control | Rect (x, y, w, h) | Texture's used area | Role |
|---|---|---|---|
| `effect_Img` | 256, 76, 512, 256 | 423 x 214 | the banner (centre (512, 204): top centre of the screen) |
| `effect_head` | 256, 93, 512, 256 | 323 x 122 | top image (skull, nut) |
| `Num_Hun` | 345, 175, 64, 128 | 54 x 75 | hundreds digit (the digit images are 64 x 128; the used area is where the glyph's pixels are) |
| `Num_Ten` | 385, 175, 64, 128 | 61 x 84 | tens digit |
| `Num_One` | 425, 175, 64, 128 | 54 x 76 | ones digit |
| `SubText` | 0, 257, wraps at 1024, centred | – | the "+N Points" line: bold 40, white, black 2x shadow and stroke |

All images are drawn at full colour (no tint). The banner art files are 512 x 256 images in the base set. The skin sets mix sizes; each is drawn into the same rectangle.

### B4. Timings and levels

Default banner timings (`[HUDLayout0]`):

| Key | Value | Symbol (A5) |
|---|---|---|
| `MKComboTotalTime` | 1.4 s | T |
| `MKComboMessageStartTime` | 0.0 s | t₀ |
| `MKComboMessageCreatingTime` | 0.1 s | c |
| `MKComboMessageBlowingTime` | 0.15 s | b |
| `MKComboMessageEndTime` | 1.2 s | tₑ |
| `MKComboMessageDestroyingTime` | 0.2 s | d |
| `MKComboMessageStartSize` | 5.0 | S₀ |
| `MKComboMessageBlowSize` | 3.0 | B |
| `MKComboMessageLastSize` | 5.0 | L |

The `MKComboSkull*` values (0.0, 0.1, 0.15, 1.2, 0.05; sizes 5.0, 2.0, 2.0) are read but drive nothing: the top image follows the banner's own timing (Quirk 2).

Timeline with these values: slam-in 0 to 0.1 s (5x to 1x, fading in), echo and shake 0.1 to 0.25 s (echo 1x to 3x, fading out), hold to 1.2 s, blow-out 1.2 to 1.4 s (1x to 5x, fading out), hidden at 1.4 s.

Levels, low to high (a request is dropped while a higher one shows):

| Level | Kind |
|---|---|
| 0 | HiSec case pickup (and "nothing showing") |
| 1 | kills: plain kill ("KILL" / "N KILLS"), co-op kill count |
| 2 | infection |
| 3 | revenge (no kill event uses it) |
| 4 | co-op combo |
| 5 | special shot: streak steps, headshot, groin shot |
| 6 | event kill: first kill, revenge, VIP kill, ice crush |
| 7 | mutation |
| 8 | game message (bounty, mode messages) |
| 9 | extra life |
| 10 | VIP Escort messages |
| 11 | lucky shot |
| 12–14 | co-op mission complete, coin, boss down |
| 15–16 | Snowball Fight frozen kill, safe |

The kinds come from the level table; which level each kill event requests is the reference's (A2). Level 3 ("revenge") is not used by the revenge kill, which requests level 6. So: a step replaces a showing HEADSHOT and the other way round (equal levels); nothing at level 5 replaces a showing FIRST KILL or REVENGE; a plain kill (level 1) replaces nothing but a co-op count or a HiSec pickup.

### B5. Voices

As in B1 and B2. All base-pack lines are mono 16-bit, 22050 Hz for the step and headshot lines and 44100 Hz for the groin-shot lines. The lengths are given in the tables. There is no per-gender or per-character variant of the step lines. The only gendered pair is the groin shot.

### B6. Side log (`[HUDLayout0]`; behaviour in A7)

Only `MKShowMessageTime`, the three section times, `MKLargeScaleValue`, `MKColor`, `MKEnemyColor` and the label colours (`<label key>Color`, e.g. `MKDoubleKillImgColor`) drive anything (Corrections (2026-10-28, plain kill) 3). `MKPos`, `MKSize`, the two blanks, `MKOnlineSize`, `MKFont` and the two alpha values are read by the layout loader but used by nothing. The line's geometry comes from the HUD layout's line template.

| Key | Value |
|---|---|
| `MKPos` | (10, 180): first line's position |
| `MKSize` | (256, 32): one line |
| `MKTextureWidthBlank`, `MKTextureHeightBlank` | 10, −6 |
| `MKOnlineSize` | 32 |
| `MKShowMessageTime` | 3750 ms |
| `MKShowFirstSection`, `MKShowSecondSection`, `MKShowThirdSection` | 1000, 1375, 2375 ms |
| `MKPlusAlphaValue`, `MKMinusAlphaValue` | 140, 70 |
| `MKLargeScaleValue` | 1.1 |
| `MKFont` | 21 |
| `MKColor`, `MKEnemyColor` | (255, 255, 255), (242, 101, 33) |
| `MKDoubleKillImgColor`, `MKUnbelImgColor` | (255, 233, 172), (255, 174, 0) |
| bars | first kill and double: `Superlatives\bg_kill03.dtx`; multi, ultra, fantastic: `bg_kill02.dtx`; unbelievable, unbelievable+: `bg_kill01.dtx` |

### B7. Weapon skins

| Section | Weapons using it | Images replaced (`attributes/layout.txt`) | Voices replaced (`attributes/clientbutes.txt`) |
|---|---|---|---|
| `PSYWeaponEffect` | 9 | first kill, kill, kills, double, multi, ultra, fantastic, unbelievable, unbelievable+, headshot, combo, groin shot, skull, nut, revenge (`*_PSY.dtx`) | first kill, headshot, both groin shots (one file), revenge (`*_PSY.wav`) |
| `PhantomWeaponEffect` | 2 | first kill, double, multi, ultra, fantastic, unbelievable, headshot, combo (`*_phantom.dtx`) | none |
| `GraffitiWeaponEffect` | 1 | first kill, kill, kills, double, multi, ultra, fantastic, unbelievable, headshot, combo, groin shot, revenge (`*_Graffiti.dtx`) | none |
| `TurkeyWeaponEffect` | 3 | as Graffiti (`*_turkish.dtx`) | none |
| `TMWeaponEffect` | 3 | headshot, skull (`*_RoboSkull.dtx`) | headshot (`UI_SND\TM_Headshot.wav`) |

Two of the skin keys name files that do not exist in the install (`Kil_Graffiti.dtx`; the archive has `kill_graffiti.dtx` and `kil_turkish.dtx`). See Quirk 5.

### B8. Art in the archive that no data names

`unbelievableplus_graffiti`, `headshot_halloween`, `head_halloween`, `nutshot_christmas`, `nut_christmas`, `graffitiheadshot`, and the language variants (`*_ltur`, `*_ldeu`, `*_lita`, `*_lpol`). The game applies no per-language texture swap (`ui_system.md` R1), so these are never drawn. `MKComboIceCrushImg` names `Ice_Crush_big.dtx`, which is not in the install (Snowball uses `SnowballFrozenKill` instead).

## Per-frame order

Client, one frame:

1. Network messages, in arrival order. For each kill message: update the kill feed (everyone); select the killer's weapon skin; if the local player is the killer, update the headshot and groin-shot flags; pick the kill's one event (A2); add its side-log line; if the local player is the killer, make its banner request (A4) and then play its one voice (A6); clear the skin; then the lucky-shot request (and Snowball's frozen-kill request), if any.
2. HUD update: for each visible banner, apply A5 (scale, alpha, shake) for the current t; hide those with t ≥ T.
3. Draw.

Several kill messages in one frame are handled one after another, so the last request that is not dropped is the one seen. Each of those kills plays its own one voice, all starting in that frame.

## Edge cases

- **Kill during a higher banner**: a double kill (level 5) 0.5 s after a FIRST KILL banner (level 6) shows no banner, but "double kill" plays.
- **Step during HEADSHOT**: a double kill 0.5 s after a single-kill HEADSHOT cuts it off and starts DOUBLE KILL (equal levels); both lines are heard, overlapping ("headshot" is 0.93 s long).
- **Kill during an equal or lower banner**: a triple-count kill 0.6 s after DOUBLE KILL cuts it off and starts MULTI KILL at t = 0 (equal level).
- **Counts above the table**: any n ≥ 7 is Unbelievable+, again on each further kill, with the same voice as Unbelievable.
- **No killer, suicide or team kill**: no step, first-kill or special banner for the killer (the count only counts enemy kills). The kill feed still shows a head badge.
- **Headshot on a streak, first kill or revenge**: no HEADSHOT banner and no "headshot" line (except a revenge, whose stock pack has no line, A6); the skull is drawn over the step, FIRST KILL or REVENGE banner.
- **Training round**: kills still raise events and banners, but the plain-kill number does not grow (the kill total is not added to).
- **Damage types that cannot headshot**: the location is torso, so no headshot event.
- **Spectating or dead**: the killer's own client gets the banner even while dead (e.g. a grenade kill after death). The banner area is drawn at every HUD level (inferred; Open question 9).
- **Map change**: every banner is hidden (A4).
- **Banner with digits**: the hundreds digit shows only for N ≥ 100, tens for N ≥ 10.

## Quirks

1. **Phase starts are a frame late.** The slam-in, echo and blow-out each start on the first frame at or after their time, and the echo only on the frame after the slam-in is seen to end. At 30 fps the echo can begin up to about 33 ms late, and the main copy sits at scale 1 for that frame. Keep (invisible at normal frame rates).
2. **Skull timings unused.** The top image has its own timing values in the data, but it follows the banner's. Keep: use the banner's.
3. **Stale special-shot flags.** The headshot and groin-shot flags the banner panel reads are only refreshed on the local player's own kills, and only rule 5 of A2 clears them. Another player's single kill can therefore take the headshot branch on this client with the local player's stale flag, which shows and plays nothing (not the killer) and only clears the flag. Nothing visible depends on it. Fix in ours: take them from each kill message.
4. **Custom-timing banners compute their blow-out from the defaults**: d = 3 − (0 + 0.1 + 0.15) = 2.75 s with tₑ unchanged at 1.2 s (when not given). Keep the formula for those modes.
5. **Missing skin art.** The Graffiti skin's single "KILL" image names a file that does not exist, so that banner draws blank (or the engine's missing-texture look) for a co-op single kill, or a plain kill with N ≤ 1 (B2a), made with that weapon. The digits still show. Keep the lookup; an empty image is fine.
6. **Unbelievable+ has no voice of its own**: it replays the Unbelievable line. Keep (data).
7. **No skull on a single-kill HEADSHOT.** The top image is chosen after the headshot rule has cleared the head flag, so the HEADSHOT banner of a single kill is drawn without the skull (and NUTSHOT without the nut), while a headshot step, first kill or revenge carries the skull. Keep (unless the banner art itself already includes it, which this pass did not look at).

## Corrections (2026-10-28)

A second pass found the client panel that turns a kill message into the banner, side-log line and voice. Changes from the first version:

1. **One event per kill** (A2), by a fixed priority: VIP kill, first kill, ice crush, revenge, then the count. Headshot and groin shot are events only on a single (non-streak) kill. Before: a step and a special shot could both be raised by one kill.
2. **One voice per kill** (A6). Before: "every voice plays" (inferred). A headshot double kill says only "double kill"; the playtest of 2026-10-28 heard both lines, which is wrong.
3. **Levels** (B2, B4): steps request level 5 (the same as headshot and groin shot), not 4; first kill, revenge, VIP kill and ice crush request 6. A step now replaces a showing HEADSHOT (equal levels) and loses to a showing FIRST KILL or REVENGE.
4. **Plain kills** by the local player show "KILL" / "N KILLS" with the match kill total (A2 rule 5.3). The side-log line said here is withdrawn: see Corrections (2026-10-28, plain kill) 2.
5. **Top image**: the skull or nut goes over a step, first-kill or revenge banner when the flag is set, and not over a single-kill HEADSHOT or NUTSHOT (Quirk 7). Before: the skull always went with HEADSHOT.
6. **Groin-shot voice gender** is the killer's (local player's) body, not the victim's.
7. **Skin fall-back** to the default pack is confirmed.

## Corrections (2026-10-28, plain kill)

A third pass over the reference's kill panel, banner player and side log,
and over the install's archive directories (file names and sizes only; the
HUD art and attribute archives are keyed and were not opened):

1. **The plain-kill banner is specified in full** (new B2a). It covers when
   it shows, N, the image choice, N = 0 drawing no digits, the three fixed
   digit slots 40 px apart, each digit scaled about its own slot, level 1,
   no top image and the voice rule. The digit art is ten separate
   64 x 128 images, `Number_0.dtx` to `Number_9.dtx`.
2. **No side-log line for a plain kill** (A2, A7; withdraws that part of
   Corrections (2026-10-28) 4). The panel does pick the "KILL" / "KILLS"
   line kinds for the local player's plain kill. But the side log adds a
   line only for kinds that have a bar image, and those two have neither
   a bar image nor a label string. So nothing is added.
3. **Side-log behaviour** (A7, B6): at most 5 lines, slot reuse, name and
   label colours, label placement and the three-phase animation are now
   given. Several B6 values (`MKPos`, `MKSize`, the alpha values, the
   font) turn out to drive nothing. This narrows Open question 4.
4. **`OneKillSound` (Open question 11) narrowed:** no announcer file for a
   single kill exists in any archive of the Steam install. The directories
   of all archives, keyed ones included, list every `.wav` name. The
   announcer folder `UI_SND` holds only the step, first-kill, headshot,
   groin-shot, VIP-kill and skin lines. So the stock plain kill is silent
   whatever the key says. Only whether the key is set at all stays open
   (the attribute file is keyed).

## Test cases

Times in seconds unless marked. "Banner" means the local killer's centre banner.

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | Player P, no previous kill, no first-kill flag | P kills an enemy at 0 (body hit) | count 1; banner "KILL" with the number 1 at level 1; no step voice (a voice only if the pack sets `OneKillSound`) |
| 2 | As 1 | further enemy kills at 1.0, 2.0, 3.0, 4.0, 5.0, 6.0 | counts 2..7 → DOUBLE KILL, MULTI KILL, ULTRA KILL, FANTASTIC, UNBELIEVABLE, UNBELIEVABLE+; voices doublekill, multikill, ultrakill, fantastic, Unbelievable, Unbelievable |
| 3 | P killed an enemy at 0 | next enemy kill at exactly 5.000 (body hit) | count 1; no step: banner "KILLS" with the number 2, no voice |
| 4 | P killed an enemy at 0 | next enemy kill at 4.999 | count 2; DOUBLE KILL |
| 5 | P killed an enemy at 0 | team kill at 1, enemy kill at 2 | count 2 at 2 (the team kill neither counts nor resets); DOUBLE KILL at 2 |
| 6 | Count 7 at 6.0 | enemy kill at 7.0 | count 8; UNBELIEVABLE+ again |
| 7 | Any | kill message with first-kill flag, count 1 | FIRST KILL banner, `FirstKill.wav` |
| 8 | Any | head kill, count 1, no first-kill or revenge flag | HEADSHOT banner (level 5) **without** a top image (Quirk 7); `headshot.wav` only |
| 9 | Local player's body female | groin kill (of anyone), count 1 | NUTSHOT banner without a top image; `NutShot_Female.wav`; no "enemy kill" grunt for the killer. Same kill by a male-bodied local player: `NutShot_Male.wav` |
| 10 | Banner player idle | request level 4 at t = 0 | visible 0 ≤ t < 1.4; hidden on the first frame with t ≥ 1.4 |
| 11 | Default timings | main copy at t = 0.05 | scale 5 − 4·(0.5)⁴ = 4.75; alpha ⌊255 − 3.75·255/4⌋ = 15 |
| 12 | Default timings | main copy at t = 0.5 | scale 1, alpha 255, offset (0, 0) |
| 13 | Default timings | main copy at t = 1.3 (blow-out τ = 0.1) | scale 5 − 4·(0.5)⁴ = 4.75; alpha ⌊255 − 3.75·255/4⌋ = 15 |
| 14 | Default timings | echo at 0.075 s into the echo phase | scale 3 − 2·(0.5)⁴ = 2.875; alpha ⌊255 − 1.875·255/2⌋ = 15 |
| 15 | Default timings | echo outside its phase (t = 0.05 or t = 0.5) | alpha 0 |
| 16 | Default timings, echo phase | main copy offset | each axis an integer in −5..5, new every frame; echo offset 0 |
| 17 | FIRST KILL (level 6) shown at 0 | DOUBLE KILL request (level 5) at 0.5 | request dropped; FIRST KILL keeps playing to 1.4; `doublekill.wav` still plays at 0.5, overlapping `FirstKill.wav` (1.30 s) |
| 18 | DOUBLE KILL (5) shown at 0 | MULTI KILL request (5) at 0.6 | DOUBLE KILL hidden at once; MULTI KILL at its t = 0 |
| 19 | REVENGE (6) shown at 0 | DOUBLE KILL request (5) at 0.2 | request dropped; REVENGE keeps playing; `doublekill.wav` plays |
| 20 | DOUBLE KILL shown | LUCKY SHOT (11) at 0.3 | replaced; no lucky-shot voice |
| 21 | Two kills in one frame raising DOUBLE then MULTI | – | MULTI KILL shown; `doublekill.wav` and `multikill.wav` both start that frame, overlapping |
| 22 | Voice playing (`fantastic.wav`, 1.67 s) | next step at +0.5 | `Unbelievable.wav` starts at +0.5 while `fantastic.wav` keeps playing |
| 23 | `soundenable` 0 | any announced kill | banner as usual, no voice |
| 24 | Screen 1920 x 1080 | banner at t = 0.5 | `effect_Img` rect x 480, y 106.875, w 960, h 360 (512·1.875, 256·1.40625); centre (960, 286.875) |
| 25 | Screen 1024 x 768, t = 0.05 | `effect_Img` drawn rect | 2432 x 1216 (512·4.75 by 256·4.75), centred on (512, 204) |
| 26 | Request with number 5 | – | only `Num_One` shown, `Number_5.dtx` |
| 27 | Request with number 120 | – | hundreds 1, tens 2, ones 0 all shown |
| 28 | Request with number 1234 | – | shows 2, 3, 4 |
| 29 | Kill with a weapon whose skin is `PSYWeaponEffect`, count 2 | – | `DoubleKill_PSY.dtx`; voice `doublekill.wav` (the PSY skin has no double-kill voice) |
| 30 | Head kill with a `TMWeaponEffect` weapon | count 1 | `HeadShot_RoboSkull.dtx`, no top image, `TM_Headshot.wav`. Same weapon, head kill at count 2: default `DoubleKill.dtx` with top `Head_RoboSkull.dtx`, `doublekill.wav` only |
| 31 | Head kill with a `PhantomWeaponEffect` weapon | count 2 | `DoubleKill_phantom.dtx`, top image the default skull `Head.dtx` (no skull key in the section), `doublekill.wav` (default, the section has no voices) |
| 32 | Another player Q gets a double kill | on P's client | no banner, no voice; kill-feed line and a side-log line "Q DOUBLE KILL" |
| 33 | Map change while a banner shows | – | banner hidden, timer stopped |
| 34 | Lucky-shot kill by P of R | on every client | two kill-feed lines; on P's client the `LuckyShot_EXPass.dtx` banner (it replaces the kill's own banner, requested just before it) |
| 35 | P's previous enemy kill 2 s ago | head kill, count 2 (the playtest case) | DOUBLE KILL banner (level 5) with the skull over it; **only** `doublekill.wav`; no HEADSHOT banner, no `headshot.wav`; side-log line "P DOUBLE KILL" |
| 36 | Any | head kill with the first-kill flag | FIRST KILL banner with the skull; only `FirstKill.wav` |
| 37 | Any | head kill with the revenge flag, count 2 | REVENGE banner (level 6) with the skull; `headshot.wav` (the stock revenge has no line, so the head fall-back speaks); no side-log line |
| 38 | Any | revenge kill, body hit | REVENGE banner, no voice |
| 39 | Any | groin kill at count 3, local body male | MULTI KILL with the nut top image; only `multikill.wav` |
| 40 | Kills by P at 0.0 (head, count 1) and 0.4 (body, count 2) | – | HEADSHOT at 0.0, replaced at 0.4 by DOUBLE KILL (equal level); `headshot.wav` from 0.0 to 0.93 and `doublekill.wav` from 0.4, overlapping; neither is stopped, delayed or skipped |
| 41 | P's single kill with a PSY-skin weapon, revenge flag, body hit | – | the PSY skin's revenge image and its revenge line (the only skin with one) |
| 42 | P's scoreboard kills 0, not a training round | P's body-shot enemy kill, count 1, no flags | N = 1: `Kill.dtx` banner, level 1, ones slot `Number_1.dtx` at (425, 175) 64 x 128, tens and hundreds hidden, no top image, no voice in the stock pack; **no** side-log line on any client |
| 43 | P's scoreboard kills 6 | as 42, 10 s after P's previous kill | N = 7: `Kills.dtx`, ones `Number_7.dtx`; tens hidden |
| 44 | P's scoreboard kills 11 | as 42 | N = 12: `Kills.dtx`, tens slot (385, 175) `Number_1.dtx`, ones (425, 175) `Number_2.dtx` |
| 45 | P's scoreboard kills 104 | as 42 | N = 105: hundreds `Number_1.dtx` (345, 175), tens `Number_0.dtx`, ones `Number_5.dtx` |
| 46 | P's scoreboard kills 1004 | as 42 | N = 1005: only ones `Number_5.dtx` (hundreds and tens are 0) |
| 47 | training round, P's scoreboard kills 0 | as 42 | N = 0 (not added): `Kill.dtx` with no digits, level 1 |
| 48 | Q (another player) makes a plain kill | on P's client | no banner, no voice, no side-log line; kill feed only |
| 49 | P's plain kill, banner at t = 0.05 | main copy | banner image 2432 x 1216 centred on (512, 204); ones digit 304 x 608 centred on (457, 239); both alpha 15 |
| 50 | P's plain kill, echo phase | main copy shake (dx, dy) | banner image and every shown digit of the main copy offset by the same (dx, dy); echo copy digits not offset |
| 51 | FIRST KILL (6) showing | P's plain kill (1) 0.5 s later | dropped; FIRST KILL keeps playing |
| 52 | P's plain kill showing | lucky-shot flag on the same kill | lucky-shot banner (11) replaces it at once |
| 53 | P's head kill, count 1 | – | HEADSHOT (not a plain kill): no digits; P's kill count still rises |
| 54 | P's plain kill with a Graffiti-skin weapon, N = 1 | – | the skin's missing "KILL" image (blank) with `Number_1.dtx` in the ones slot |
| 55 | side log, five lines showing | a sixth event | the oldest line's slot is reused; still five lines |
| 56 | side log line added at t = 0 (stock values) | t = 500 ms; t = 1200 ms; t = 3000 ms | bar and label: scale 1 + 0.25 · 0.1 = 1.025 and alpha ≈ 127; scale 1.1 − (200/375)² · 0.1 ≈ 1.0716; alpha 255 − (625/1375) · 255 ≈ 139.1. The name: scale 1, alpha 255 throughout |
| 57 | P kills an enemy at 0, dies at 1.0 (killed by anyone), respawns at 3.0 | P kills an enemy at 4.5 | count 2: DOUBLE KILL (death and respawn reset nothing). Same with the second kill at 5.0 → count 1 |

## Open questions

1. **Thresholds and levels per event.** **Answered (2026-10-28)** from the reference's kill panel: A2, B1, B2, B4. A headshot second kill shows DOUBLE KILL with the skull and plays only "double kill"; other players' streaks show no banner. Still worth a live look: whether the stock "KILL" / "N KILLS" banner really shows on every plain kill in the Steam build (A2 rule 5.3), since nobody has described it from play.
2. **Death and the count.** Does the killer's own death reset the multi-kill count? Check live: kill, die within 2 s, kill again from respawn within 5 s of the first kill. **Answered (2026-11-14):** no; only joining and the mode's end-of-mission reset it (A1). Case 57.
3. **First kill scope.** First kill of the round or of the match, per team or overall? Check in a round mode.
4. **Side log.** Which events get a line, who sees it, the 5-line limit, the colours and the animation are answered (A2, A7; Corrections (2026-10-28, plain kill) 2 and 3). A plain kill adds no line. Still open: the line template's own position and size and its children's layout. They come from the keyed HUD layout (`xml/hud_main.xml`, window `Superlatives`, template `EffectLogLine` with children `Name`, `Title` and `Background`), not from B6. Check live with screenshots.
5. **Voice when the banner is dropped, and overlap.** **Answered (2026-10-28):** one kill plays at most one line (its single event's, with the head and groin fall-backs of A6); the line plays whether or not its banner request was dropped; lines of different kills overlap, never stopped, queued or skipped (the skip-if-playing sound helper is not used by the announcer). Narrowed to a live check only: that the engine's interface-sound voice limit does not cut a line when many sounds play at once.
6. **Lucky-shot roll.** The rate comes from the lobby; its value and what decides a lucky shot are not in the reference.
7. **Skin fall-back.** **Answered (2026-10-28):** a skin's table starts as a copy of the default one; missing keys keep the default (A8).
8. **Scoreboard step counters.** The client counts kills per multi-kill count value per player; where (if anywhere) the scoreboard shows them.
9. **Warm-up and HUD levels.** Narrowed (2026-10-28): the client announces kills whatever the round; in a training round it only skips adding to the kill total (so the plain-kill number stalls). Whether the server sets the count and flags during warm-up, and whether the banner is drawn while the local player is dead (death screen up) or spectating, is still open.
10. **Groin-shot voice gender.** **Answered (2026-10-28):** by the local player's (the killer's) own body: a worn cosmetic body's figure type if any, else the character's model (B2).
11. **Single-kill voice key.** Narrowed (2026-10-28): the plain-kill "KILL" event (N ≤ 1) looks up `OneKillSound` in `[Interface]` (and in a skin's section). "KILLS" has no key. No archive of the Steam install contains a single-kill announcer file (Corrections (2026-10-28, plain kill) 4), so the stock game is silent there either way. Still open: whether the keyed attribute file sets the key at all. It matters only if a pack ships such a file. An implementation reads the key and plays nothing when it is missing or empty.
