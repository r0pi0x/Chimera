# Parity gaps with CS:S (what players see and hear)

The three games ranked together for a gun-game player:
[parity-gaps-ranked.md](parity-gaps-ranked.md).

Started 2026-10-13. An inventory of known and suspected differences
between Lucker Party and Counter-Strike: Source, by area, from (a) refcmp
on the maps with reference captures, (b) every tech-debt row and spec
open question that marks behaviour as unmeasured or ours, (c) a check of
what CS:S players commonly use (the install's `cfg/config_default.cfg`
binds, its sound scripts, its `resource/ui/*.res` panels, common console
commands). Fixes that need no new capture are done from the top; the
rest wait on the captures listed at the end.

Not covered here (other work in flight, 2026-10-13): mg_jacks (water
through geometry, skybox inside players, head-height camera, its say
lines, sounds and strips, the AK's sound fade) and bots (parked). Player
shadows vs decals, the third-person AK magazine and buy zones round
spawns landed on main during this pass.

Status: **have** (matches, measured or from data), **differs** (in, but
ours or unmeasured), **missing**. Impact: how often a player notices
(H every round or fight, M every session, L rarely). Cost: S (hours), M
(a day or two), L (more, or needs a spec session).

## Summary

| Area | have | differs | missing | fixed now |
|---|---|---|---|---|
| Movement | 9 | 4 | 0 | 4 |
| Weapons, recoil, spread | 11 | 7 | 1 | 9 |
| Sounds | 11 | 7 | 1 | 7 |
| HUD | 17 | 3 | 0 | 8 |
| Menus and commands | 17 | 4 | 5 | 10 |
| Rendering, lighting, materials | 18 | 16 | 3 | 14 |
| Maps and entities | 17 | 8 | 2 | 10 |
| Rules and economy | 11 | 1 | 0 | 5 |
| Networking feel | 7 | 3 | 2 | 1 |
| **Total** | **118** | **53** | **14** | **68** |

Counted from the tables below (recounted 2026-10-18; the fourth and fifth probe passes changed no row's status): "have" is every row
whose status starts with have (have, have (fixed), have (new), have
(differs)); "fixed now" the rows marked have (fixed), which are counted
under have too. Rows marked n/a or with no status are left out. What
would settle each differs or missing row, and every spec's open question,
is sorted in [open-questions.md](open-questions.md).

## refcmp: the maps with reference captures

Reference captures exist for de_dust2 (32 views), de_nuke (27),
cs_office (23) and de_aztec (13) (`~/.local/share/mashup/dump/refcmp/`,
taken 2026-10-05/06); none for cs_italy, de_dust, de_inferno, de_train,
cs_assault, cs_compound, de_port or the mg_ maps. Ours were captured
again from this branch (`refcmp capture-ours --out <scratch>`, then
`report`). No HUD references are in the dump folder
(`refcmp/hud/ref/` is empty), so `hudcmp` couldn't run; the HUD's
measured rectangles live in `games/cs_source/tests/ui/heavy/hud_layout.rs` and pass.

| Map | Views | Mean abs diff (this branch) | Before (2026-10-07 captures) | Mean luma error | Worst three views |
|---|---|---|---|---|---|
| de_dust2 | 32 | 0.0303 | 0.0308 | 0.0029 | `sky_n` 0.060 (luma 0.753/0.743), `a_sign_e` 0.052, `a_sign_w` 0.047 |
| de_nuke | 27 | 0.0239 | 0.0250 | 0.0057 | `cam6` 0.042, `cam16` 0.041 (sharpness 13.0/10.8), `cam4` 0.034 |
| cs_office | 23 | 0.0181 | 0.0204 | 0.0019 | `cam0` 0.029, `zone1_180` 0.028, `zone1_90` 0.027 |
| de_aztec | 13 | 0.0354 | 0.0409 | 0.0015 | `zone1_90` 0.051, `zone0_270` 0.050, `zone1_0` 0.046 |

Reading: abs diff is the mean absolute difference per pixel (0-1);
luma is each image's mean. Brightness matches everywhere (mean luma
error under 0.006). What the worst views show (side-by-sides in the
report folders):

- de_nuke `cam16`, `cam6` (and every view with the 3D skybox in it): the
  far mountains of nuke's 3D skybox are fogged out to flat grey in ours
  and the cooling towers turn blue-grey, where CS:S shows the mountains
  plainly with a light haze. Ours divides the sky_camera's fog distances
  by its scale (nuke: 512-11000 over 16); dust2's scale is 4, so there it
  only thickens the haze a little. Tried here without the division
  (both `map::material_fog` and the sky camera's `DistanceFog`): nuke's
  mountains then match CS:S and nuke's mean falls 0.0241 -> 0.0238
  (`cam1` 0.027 -> 0.020, `cam4` 0.034 -> 0.027), but cs_office's skyline
  (its 3D skybox: scale 16, fog 0-15000) loses the haze CS:S has (`cam0`
  0.036 -> 0.041) and dust2 doesn't change. One rule doesn't fit both,
  so the division stays until capture C10 says what nuke's mountains are
  (3D geometry or the 2D sky `de_cobble_hdr` behind fully fogged
  geometry). Nuke's cooling towers (`coolingtower.mdl`, static props in
  the 3D skybox, no baked `.vhv`) were dark teal where CS:S's are light
  grey: their sun ray ran on past the skybox's own sky into the map's
  underside, so they had only the sky's blue ambient. Fixed 2026-10-18:
  a sun ray ends at the first sky brush (`ambient::Occluders::with_sky`);
  `cam16` 0.041 -> 0.032, `cam6` 0.042 -> 0.036, `cam2` 0.025 -> 0.017.
- de_aztec `zone0_*`: the difference was mostly the displacements'
  texture mapping (projected through the sculpted surface; now from the
  undisplaced grid, 2026-10-17: `zone0_180` 0.059 -> 0.045, its detail
  correlation 0.15 -> 0.62); what is left is texture-level noise over
  the bump-mapped walls and the grass blend (luma equal to 0.001), plus
  the sky at the top: filtering, not light.
- de_dust2 `sky_*`, `a_sign_*`: the sky's rope and far hills (known:
  rope sag, haze) and the bump-mapped walls' detail.
- cs_office `cam0`: the intermission camera's snowy yard and fog.

## Movement

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Ground, air, duck, jump, bhop cap, stamina, ladders, water | have | | | movecmp to 0.002 units (OBSERVABILITY, movecmp) |
| Fall damage and landing roll | have | | | specs/cs_source/fall_damage.md |
| Surf ramps | differs | L | | tech-debt "cs_source movement": the rampbug patched out (our choice) |
| Slowdown when shot ("tagging") | have | | | Fixed 2026-10-14: weapons.md M18 (0.5/0.65 by gun, legs 0.5, +1/3 a second); the speed trace to 0.05 (`games/cs_source/hit_reaction.rs`) |
| View punch when hit (flinch) | have (fixed) | H | S | Fixed 2026-10-14 to M18 (bare head −12/±9 roll, unarmoured body −dmg/10 up to 4, helmeted head none: measured, matches) |
| Scoped speeds (AWP, Scout, autos zoomed) | have (fixed) | M | S | measured (movement.md Q1): the G3SG1 keeps 210 zoomed (was 150), the SG 552 slows to 200 (was 235); the rest matched |
| Duck spam limit, duck transition times | have | M | S | measured (movement.md Q6): 0.4 s / 0.2 s, no spam limit, as ours |
| Backpedal speed (`sv_backspeed`) | have | L | S | measured (movement.md Q10): not applied in CS:S (full speed backwards), as ours |
| Noclip across spawns | have (fixed) | M | S | measured 2026-10-17 (movement.md "Noclip and spawning"): every spawn walks (`mp_restartgame`, the next round, a respawn after death); ours kept the noclip, now `put_at_spawn` ends it and a server never spawns a client noclipping |
| Noclip takes no damage | have (fixed) | L | S | measured 2026-10-17 (movement.md "Noclip and spawning"): CS:S's noclipping player takes nothing from bullets, blasts, trigger_hurt or the world, nor loses armour, but is still hit (bullets meet its body and go on through) and falls hurt once it walks; ours took it all, now `core::Noclip` is refused by `DamageRules` and spared armour (a suicide still kills), and flies below the map unharmed |
| Base velocity (conveyors, water currents) | differs | L | M | tech-debt "Source movement"; spec Q15; trigger_push and `AddOutput basevelocity` measured and matching tick for tick 2026-10-17 (triggers.md Q7, Q9); a func_conveyor's measured 2026-10-18, as ours (game_entities.md 8); currents open |
| Camera under a low ceiling | differs | M | | other work in flight (mg_jacks) |
| Tick rate 66.67 vs community 100 | differs | L | | spec Q17 |

## Weapons, recoil, spread

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Damage, falloff, hitgroups, armour | have | | | measured M5-M7; 2026-10-17: the armour left is truncated, so a fraction of a point costs a whole one (a knife slash on 100 leaves 98, was 99; `weapon::armor_split`) |
| Refused hits (teammates with friendly fire off, noclip, god) | have (fixed) | M | S | measured 2026-10-17 (weapons.md "Refused hits"): a teammate's bullet, knife or HE with `mp_friendlyfire 0` does nothing (no armour, sound, blood, slowdown or flinch); a noclipping player's bullet hit bleeds and flinches, nothing else. Ours took armour from every refused hit but noclip's and played the flesh impact; now armour only where a hit lands (`weapon::HitArmor`), refused hits are silent, noclip flinches. God mode unmeasured (C30); an HE blast now sounds kevlar or the flesh impact as measured (was silent) |
| HE self damage and ear-ringing | have (fixed) | L | S | measured 2026-10-17 (grenades.md "Self damage"): the thrower takes his own blast as anyone at his distance, `mp_friendlyfire` 0 or 1 alike (as ours). Who hears the ring is unmeasurable on the server (Q11, C31); ours rang every player in reach, now only those the blast hurts (not a teammate with friendly fire off, noclip, god) |
| AK-47/M4A1/pistols/AWP recoil, inaccuracy | have | | | measured M1-M4 |
| `invnext` / `invprev` (the mouse wheel's weapon commands) | have (fixed) | H | S | were missing; on the wheel by default as CS:S (MWHEELUP invprev, MWHEELDOWN invnext; wheel jumping: `bind mwheeldown +jump`) |
| Picking the second grenade from the HUD selection | have (fixed) | M | S | the pick drew the first weapon of the slot; now `Intent::pick` names it |
| `autobuy` / `rebuy` (F1 / F2) | have (fixed) | H | S | were missing (the buy menu's buttons greyed); details unmeasured (tech-debt), capture C4 |
| Spread generator | differs | L | M | tech-debt "CS:S's spread and recoil": seeded as CS:S, our generator (spec Q1) |
| Other automatics' recoil, scoped/silenced/burst | differs | M | S | measured and in the data 2026-10-15 for M4A1, FAMAS, Galil, SG 552, AUG, MP5, P90, UMP, MAC-10, TMP, M249 (standing, crouched, moving, airborne; five airborne yaw caps not reached); scoped AUG and SG 552 as unscoped; 2026-10-18: scoped AWP and scout kick 2.0 up, scoped SG550/G3SG1 their random kicks (as ours), a FAMAS burst kicks on its first round only (fixed: ours kicked all three); silenced modes left (the bot's AI takes the silencer off) (Q3) |
| Penetration materials other than wood/metal/sand/concrete | differs | M | S | tech-debt "Penetration"; spec Q7; sand's and metal's damage factors measured 2026-10-18 (0.3 each, were our 0.5), tile 0.3, computer 0.45, grate 0.99, plastic and glass 0.5 (were 0.5 for all); a prop entity crossed for 24 units or more counts twice in CS:S (the edge measured at 24.0; ours too since 2026-10-18); tile's thickness scale 0.65 (was 1.0) and glass's 1.0 measured 2026-10-18 (sixth pass); grate's and plastic's scales left |
| Unmeasured gun values (penetration...) | differs | L | S | tech-debt "The CS:S guns"; 2026-10-15 measured the powers of 5.56, 9 mm (as the data), .357 (24, was 15) and 5.7 (30, was 15), and the reach of 9 mm and .357 (800; .357 was 500); buckshot measured 2026-10-18: it passes nothing (no wood, not a player; ours passed 30 units of wood and a player, now power 0); long reaches left |
| Grenade throw timings, redraw, pin rules | differs | M | S | release (6 ticks after the throw), the throw pitch bend (80/90 looking up: fixed), fuses, HE vs armour (×0.7375) measured and matching; HE falloff fixed (a bell σ 350/3, linear with sv_legacy_grenade_damage 1); flash fixed (7.14 × (1 − d/1500) × (0.7 + 0.3 cos), alpha by facing); a switch with the pin out keeps the grenade (2026-10-17, grenades.md Q3: as ours); cover from the HE fixed 2026-10-18 (Q10: static props stop it as walls, ours let it through; prop entities and players between shield a quarter, ours nothing; `BlastShield`); a flash in smoke blinds 0.56 as long (fixed 2026-10-18, Q12; ours ignored smoke); a player's box or a prop between stops a flash whole and glass or a grate doesn't (fixed 2026-10-18, Q12: ours let players and props through and stopped at glass and grates); `drop` leaves a grenade in hand (fixed 2026-10-18, Q17: ours dropped it); redraw open (Q2: the probe's bot can't tell, its AI picks the knife), and the thrower's own blindness (Q12) |
| What the dead drop | have (fixed) | M | S | measured 2026-10-17 (grenades.md Q17): the best gun whatever was held, and one grenade, an HE over a flashbang over a smoke; ours dropped only the weapon in hand (a grenade held: no gun) (`weapon::drop::DeathDrop`) |
| Shotgun reload | have (fixed) | M | S | measured 2026-10-17 (weapons.md M9): 0.5 s start and 0.5 s a shell for the M3 and XM1014 (68 ticks to the first, 35 each after), a shot stops it; ours had the template's 0.45 s a shell |
| Same-type pickup takes nothing | have (fixed) | L | S | measured (weapons.md M20): CS:S takes no ammo from a gun of a type carried (ours took clip and reserve); a dropped gun carries its reserve, as ours; the re-pick delay (0.75 s, was 1 s) and reach measured too |
| `cl_autowepswitch` while firing | differs | L | S | tech-debt row |
| Knife idle timing | differs | L | | tech-debt "View-model idle" |
| Flashlight (`impulse 100`, F) | missing | L | M | `mp_flashlight` decides on a server; capture C9 for the default |
| Night vision (`nightvision`, N; buyable) | have (fixed) | L | M | `buy nvgs` $1250 (greyed once owned), kept through rounds, lost at death, rebuy; green view and sounds to the owner (tech-debt "Night vision"); capture C24 |
| Shield | n/a | | | CS:S has none |

## Sounds

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Weapon fire, reload parts, empty, deploy, zoom | have | | | spec sounds.md; reload parts at our times (Q9) |
| Footsteps, ladders, water, landing | have | | | spec; silence rules unmeasured (below) |
| Round announcer, radio, chat blip, bomb | have | | | |
| Hurt and death sounds | have (fixed) | H | S | measured per hit (sounds.md): a killing headshot plays DamageHeadShot and DeathHeadShot, armour-covered hits only DamageKevlar (no flesh impact), heads no flesh impact; helmeted heads unmeasured (tech-debt "Hurt and death sounds"); the knife (2026-10-17): kevlar on armour, the flesh impact on a bare player (was silent); an HE blast the same (2026-10-17, was silent); a refused hit (a teammate with friendly fire off, noclip) none at all (2026-10-17, was the flesh impact) |
| Weapon pickup and buy sounds | have (fixed) | H | S | `Player.PickupWeapon`, `BaseCombatCharacter.ItemPickup2` were never played; which plays when is inferred (tech-debt), capture C2 |
| HUD weapon selection sounds | have (fixed) | H | S | `Player.WeaponSelectionMoveSlot`/`WeaponSelected`/`DenyWeaponSelection` were never played; when each plays is inferred, capture C3 |
| Under water muffle | have (fixed) | M | S | nothing changed with the head under water; now a steady low-pass (Source's `dsp_water` is engine-side: values ours, capture C5) |
| Distance falloff curves | differs | H | S | spec sounds.md Q1 (H1/H2 hypotheses) |
| Footsteps and impacts on blended ground (`$surfaceprop2`) | have (fixed) | M | S | 2026-10-17: blended displacements always played their first surface (cs_militia's grass as dirt; cs_compound, de_aztec's mud, de_port, 6 community maps); now a triangle whose corners' mean vertex alpha is over one half takes the second (`sound::second_surface`); the threshold is ours (no spec), check by ear on militia's yard |
| Footstep silence rules, jump sound | have (fixed) | H | S | measured (sounds.md "Footsteps"): silent below 150 (was: by the walk/duck keys), 0.25 / 0.4 s under 220, 0.55 / 0.3 s above, jump 1.0; duck + jump from standing silent in CS:S, ours plays the jump step |
| Room DSP presets | differs | M | M | tech-debt "Room DSP": every value ours |
| `dsp_player`, `soundmixer` | missing | M | M | read, not played (spec Q14) |
| Wave choice (repeat avoidance) | differs | L | S | spec Q2 |
| ambient_generic levels | differs | L | S | spec Q11 |
| Scrapes | differs | L | S | spec Q8 |
| Ricochets (`Bounce.Shrapnel`) | have | M | S | 30 % of bullet impacts (impact_effects.md section 2, the template's rule; `game::impact_effects`); the rate unmeasured (impact_effects.md Q2) |
| Drowning (`Player.DrownStart`/`DrownContinue`, damage) | differs | L | S | damage and restore as measured (C6, `games/cs_source/drown.rs`); the client's own drown sounds (if CS:S plays any) not played; reset on surfacing unmeasured |
| Hint sound (`Hud.Hint`) with `cl_autohelp` hints | have (fixed) | M | M | the hints' data names it (`games/cs_source/data/hints.toml`); played with each automatic hint (`ui::hints`) |
| Spray sound, night vision and flashlight sounds | differs | L | | `SprayCan.Paint` (all hear it) and `Player.NightVisionOn`/`Off` (the owner only) play; the flashlight is missing |
| AK full-auto choppy | | | | other work in flight |

## HUD

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Health, armour, ammo, money, round timer | have | | | hudcmp (rectangles in `heavy::hud_layout`) |
| Weapon selection, pickup history | have | | | hudcmp |
| Death notices, scoreboard, chat, radio menus | have | | | |
| Bomb/kit icons, progress bar, radar dots | have | | | |
| Target ID | have | | | range a guess (tech-debt) |
| Flash white-out, smoke grey | have | | | |
| Crosshair and its options | have | | | ranges unmeasured (ui-parity.md) |
| Spectator bars, freeze cam | have | | | |
| Team menu, buy menu (VGUI) | have | | | |
| Round result | have (fixed) | H | M | the win panel from the install (`resource/ui/win_round.res` placed by `WinPanel_Round`, its `winpanel_*`/`funfact_*` strings): winner, reason, round time, MVP, a fun fact, over the network too (`client::win_panel`); the centre print only without the layout. MVP by CS:S's published award reasons; which fun fact, the title's colour, how long it shows and whether a centre print shows with it are ours (C7, C12) |
| Damage direction indicator | have (fixed) | H | M | was the CS 1.6 `pain_*` arcs around the crosshair and only where the damage was dealt (never on a network client); now `HudDamageIndicator`'s red side bars (hudlayout.res: `dmg_xpos` 30, `dmg_ypos` 100, `dmg_wide` 36, `dmg_tall1` 240, `dmg_tall2` 200) from damage that reached the health, sent to the victim's client (`net::Hurt`). Which bar for hits from in front/behind (both), the fade (1 s) and alpha are ours: C8 |
| Radar look (zoom, translucency, range) | differs | M | S | backlog 2b: ours; `cl_radaralpha` (CS:S's 200) now sets the map's opacity |
| Scoreboard row spacing, dead rows | differs | M | S | tech-debt "Game-look HUD parts" |
| Spectator menu look | have (fixed) | L | S | `bottomspectator.res`'s bar of drop-downs and `<` `>`, values aligned as the file says; the lists' look is GameUI's widgets (CS:S: the client scheme's) |
| Freeze cam panel (`freezepanel_basic.res`) | have (fixed) | M | S | name, "KILLED YOU", "[F5] Save this moment", the killer's health gauge (colours ours); no avatar or nemesis lines. Timings CS:S's server cvars (`spec_freeze_time` 5, `spec_freeze_traveltime` 0.7, replicated); its 2 s death cam before it unmeasured (specs/cs_source/spectating.md open questions) |
| Death cam and spectating timings | have (fixed) | M | S | measured on bots in a SourceTV demo (specs/cs_source/spectating.md): 6.5 s death cam without a freeze cam, 3 s on a dead target, first target in order (not the nearest), no enemies under `mp_forcecamera 1`; with nobody left a fixed view (measured 2026-10-18 on the probe, round end or not; ours flew free, now held: `SpecMode::Fixed`); the first target is the next entity index after the watcher's own (ours by name) |
| Map time left on the scoreboard | have (fixed) | L | S | beside the clock icon (`WinConditionLabel`) with `mp_timelimit`; a client has the map's start in `NetRound` |
| Class menu (`classmenu_*.res`) | have (fixed) | M | M | after picking a team; the eight classes' models (`models/player/*.mdl`) on bodies, auto-select a random one; the menu's 3D model preview (`CSClassImagePanel`) not drawn |
| MOTD on joining (`motd.res`, `cfg/motd_default.txt`) | have (fixed) | L | S | then the team menu, then the class menu; HTML shown as its text (we don't render HTML) |
| Hint panel messages (`cl_autohelp`) | differs | M | M | the game's `Cstrike_TitlesTXT_Hint_*` texts from the install in `HudHintDisplay`, once per player per map, `cl_autohelp` (userinfo) turns them off, a 5 s gap, humans only (`rules::hints`, data `hints.toml`, over the network as `HintNews`); every trigger and rule is ours, guessed from the wording: capture C26 |

## Menus and commands

The install's `cfg/config_default.cfg` binds (a fresh CS:S config) and
common console commands, against mashup's.

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Main menu, Options (six tabs), Create Server, Find Servers | have | | | docs/plans/active/ui-parity.md |
| Movement, attack, slots 1-5, `lastinv`, `drop`, `buymenu`, `chooseteam`, radio, chat, `buyammo1/2`, `+showscores` | have | | | same keys as CS:S |
| `autobuy` (F1), `rebuy` (F2) | have (fixed) | H | S | commands added, on F1 and F2 by default (the debug UI and inspector moved to F8 and F11) |
| `invnext`, `invprev` | have (fixed) | H | S | see Weapons |
| `jpeg` (F5) | have (fixed) | L | S | a PNG in the screenshots folder named `<map>NNNN` |
| `kill`, `explode`, `retry`, `connect`, `status`, `exec`, `alias`, `bind`, `toggle`, `incrementvar`, `cl_showfps`, `cl_showpos` | have | | | |
| `net_graph` 1-3, `net_graphpos`, `net_graphheight`, `net_graphproportionalfont` | have (differs) | L | S | the transport's numbers (ping, in/out, loss, choke, sv, up/cmd, lerp) and a latency and bytes graph; layout and colours from public descriptions, unmeasured; no `net_graph 4`, `net_scale` (backlog section 5) |
| Default binds | have (fixed) | M | S | every `config_default.cfg` bind (read from the install at start, the same table built in otherwise; `client::binds`); mashup's own only on keys it leaves free: Alt free look, V noclip, F9 bug report, F3/F8/F11 debug keys. Saved configs keep their binds (defaults added since go on their free keys once). F, H, I, J, F6, F7, PAUSE run commands mashup lacks (`impulse 100`, `commandmenu`, `showbriefing`, `cheer`, `save`/`load`, `pause`: an error line in the console); F10's `quit prompt` opens the game menu (no quit dialog) |
| Console | differs | L | M | a drop-down, not CS:S's VGUI frame (ui-parity.md) |
| `con_enable` and `volume` defaults | differs | L | | ui-parity.md: undecided |
| Options apply at once | differs | L | | ui-parity.md: deliberate |
| `impulse 100` (flashlight) | missing | L | M | `nightvision` is in (Weapons) |
| `impulse 201` (spray), Import Spray | differs | M | M | sprays are in (see Sprays); the logo is `cl_logofile` (console), no Import Spray or picker in the options |
| `+voicerecord` (voice chat), Voice tab | have (new) | M | L | K held talks (docs/plans/active/voice-chat.md); the Voice tab's enable box, receive volume, microphone boost, Test Microphone and meter work; its transmit volume isn't drawn |
| `playdemo` | have (differs) | L | L | CS:S demos as puppets (`game::demo`, `ui::demo`), `demo_timescale`, `demo_pause`, `demo_gototick`, `stopdemo`; animation from velocity, no zoom (tech-debt) |
| `record`, `stop`, `demoui` | missing | L | L | no recording; `demoui` panel missing (console commands instead) |
| `fps_max` | have (fixed) | L | S | 300 as CS:S (`client::perf::FrameLimit`); off under `--bench` |
| `timeleft` | have (fixed) | L | S | with `mp_timelimit` |
| `users`, `ping` | have (fixed) | L | S | Source's layouts (`net::users_text`, `ping_text`) |
| `motd` | have (fixed) | L | S | opens the MOTD panel |
| `buyequip`, `cancelselect` | have (fixed) | L | S | the buy menu's equipment page; close the weapon selection |
| `commandmenu`, `showbriefing`, `cheer` | missing | L | S | what each shows in CS:S is unclear from the data (no capture planned) |
| `cl_radaralpha`, `hud_saytext_time` | have (fixed) | L | S | 200 and 12 s, as CS:S |
| `cl_minmodels` | missing | L | S | needs player models per class (class menu) |
| Player list (mute) | have (new) | L | M | the game menu's Player List (`PlayerListDialog.res`): select a player, Mute in-game voice; client-side, for the session (voice-chat.md) |
| Stats, achievements dialogs | missing | L | M | |

## Rendering, lighting, materials

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Lightmaps, bump, envmaps, two-texture blend, sky, fog | have | | | refcmp above |
| Displacement texture mapping | have (fixed) | M | S | displacements projected their texture through the sculpted surface, smearing it in streaks wherever vertices move sideways (reported on cs_militia's rock walls at its front yard); now mapped from the undisplaced grid as their lightmaps are (shaders.md open question 20: measured, refcmp detail correlation up on every map, de_aztec 0.54 -> 0.72) |
| Bump maps in DirectX 9 fallback blocks | have (fixed) | L | S | 181 stock materials have a `LightmappedGeneric_DX9` (or `>=DX90`) block, which the VMT parser skipped; about 110 name their `$bumpmap` only there (HL2's `_c17`/`_cheap` walls and floors: cs_militia's sewer-concrete wall by the reported spot, cs_assault, cs_compound, de_port, de_inferno), drawn flat; now read with the block's keys (`lenient_vmt`) |
| Decals, overlays, impact effects, tracers | have | | | specs; the "placeholder" tech-debt row was stale and is gone; glass shards tumble and settle, slime splashes from the `.pcf` (2026-10-15) |
| Impact effects of others' shots beyond the shooter's PAS | have (fixed) | L | M | every client made them (2026-10-18): a shot (its impacts, tracer and sound) now goes only to clients whose eye is in the potentially audible set of the shooter's eye (impact_effects.md 1; the BSP's PAS, `MapVisibility::hears`); the dead and spectators get every shot (the server doesn't know where they watch from) |
| Blood bursts up close (sprite-card size limits, goop spin and flip, mist tint) | have (fixed) | H | S | were left out (2026-10-17): every sprite-card material's `$maxsize` and `$startfadesize`/`$endfadesize` from the install (the blood mist capped at a quarter of the screen, the blood smoke gone once a tenth of it, fire smoke and flames too), the goop's spin stopping at 1 s and half of it mirrored, the mist and goop a tenth toward the light (impact_effects.md 10, 14); the screen-size measure and `tint_perc` are our readings (Q7) |
| `.pcf` colour toward the light (`tint_perc`) | have (fixed) | L | S | the `.pcf` systems ignored it (2026-10-17): colour random's `tint_perc` now blends each new particle that far toward the light at its control point (the map's light field there, gamma space, as the blood's mist and goop), for placed info_particle_systems and started ones (slime splashes, 0.45); a sweep of the install's 14 `.pcf` files found it only in colour random (always control point 0, clamps 0-255), and none of the cached maps' placed systems use it; our reading (impact_effects.md Q7) |
| Knife slash mark | differs | L | S | `ManhackCut` is a guess; no spec names it (backlog 3 "Decals, remaining") |
| Ropes, sprites, dust motes, beams | have | | | specs |
| View models | have | | | view-models.md |
| HDR (`mat_hdr_level 2`, CS:S's default) | differs | M | M | tech-debt "Lightmaps": stand-in exposure and bloom; references are LDR, capture C17 |
| Player shadows | differs | M | | blob only (tech-debt "Dynamic shadows"); decals under shadows and steadier shadows merged from main 2026-10-13 |
| World reflections | differs | L | S | the cubemaps now carry their mips (round 2: cs_office mean 0.0189 -> 0.0185, `cam0` 0.041 -> 0.038; de_aztec 0.0408 -> 0.0405; dust2, nuke unchanged); the highlight stays narrower on dust2's bumped tunnel floor (`tunnel_floor_e` 0.016 before and after): per-vertex tangents next |
| Water refraction | differs | L | M | tech-debt "Water" |
| Under-water screen overlay | have (fixed) | L | S | de_port's `water_warp01` was drawn but invisible (a warp of about a pixel, `$bluramount` ignored); now blurred, 5 x 5 taps a pixel apart (no Refract spec: a guess; tech-debt "Water"). de_nuke's pool has none in either game |
| Under-water fog in non-Water-shader water (de_nuke's pool) | have (fixed) | M | S | no fog at all with the eye in nuke's pool (an additive LightmappedGeneric, which the water loader skipped); now its material's fog ({150 200 200}, 64-512) over the whole view; how the engine reads fog from other shaders is unspecified (tech-debt "Water") |
| Sky under water | have (fixed) | M | S | under refracting water (de_port) the surface from below showed flat fog colour, no sky: the sky camera was off under all water and the 3D skybox sea's under side drew black. Now the sky draws behind the view (the original's refraction view in the back buffer, spec water.md 9) and skybox water is hidden under water; fog-only water (de_nuke) and refraction off still clear to the fog colour with no sky (tests/it/underwater_sky.rs) |
| Particles blend in linear space | differs | L | M | tech-debt |
| Glow occlusion by rays | differs | L | M | tech-debt |
| LOD switch metric, no hysteresis | differs | L | S | tech-debt "Levels of detail"; capture C21 |
| Rope wind sway | have (fixed) | L | S | round 2: the spec's gusts near the camera (`render::rope_sway`, `rope_wind_dist`); maps with an env_wind hang still (not simulated); refcmp captures with `rope_wind_dist 0`; amplitude unmeasured, capture C18 |
| Detail props layout | differs | L | S | tech-debt "Detail props"; capture C22 |
| Player animation (feet slide, no death layer) | differs | M | M | tech-debt "Player animation": SDK template speeds; animation.md's open question needs a demo, capture C14 |
| Ragdoll push per weapon | differs | M | S | tech-debt "Ragdolls": the ammo impulses are measured on props since 2026-10-17 (weapons.md Q8: 2000 9 mm to 2800 .338, buckshot 600 a pellet, the knife 300 flat; `[damage] impulse` per gun); whether a player's death push uses the same force is capture C13 |
| Muzzle flash look, view model lighting | differs | M | S | tech-debt "View models"; view_models.md Q5, Q7; capture C15 |
| Prop light probes | differs | M | M | tech-debt: the engine's weighting isn't public (needs a spec session); props without `.vhv` (all of cs_militia's) take the sun from one point, whole or not at all: militia's crates by the shed (x 1344-1353) much darker than the next ones out, as the reading predicts (2026-10-17); whether CS:S does that or lights them per vertex at load is shaders.md Q11, capture C28 |
| Model reflections masked by the normal map's alpha (`$normalmapalphaenvmapmask`) | have (fixed) | M | S | 2026-10-17: the mask was dropped on props, so those materials were mirrors: militia's ammo can, cable spool, pallet, car battery, APC; the same props on nuke, inferno, train, assault, port, prodigy, compound, havana, piranesi, aztec (vents, pallets, spools, `metalwall001a`, `pi_apc`, `pi_boat`); the hands now reflect through it too. refcmp: nuke 0.0242 -> 0.0241, aztec, dust2 unchanged, office within capture noise (`cam0`'s sky) |
| Animated textures (AnimatedTexture proxy), `$frame` | have (fixed) | L | S | 2026-10-17: drawn on their first frame; now the frames in turn (`render::texture_anim`): de_nuke's pool's bump map (29 frames at 21 fps; its base has one frame), cs_office's projector slide (7 slides, one every 10 s), 7 community maps (DirectX 7 water stand-ins, mg_escape_prison_beta's slime bump); a static `$frame` (no stock map sets one) picks its frame. From the public wiki, no spec (tech-debt) |
| LightmappedGeneric `$color` | have (fixed) | L | S | 2026-10-17: ignored; now the lighting times `$color`, raw (shaders.md 2). No stock world material sets it; 6 community maps' DirectX 7 water stand-ins do (bhop_backport_css, gg_pokemonz, surf_stickybutt_alpha) |
| Props whose material names its cubemap | have (fixed) | L | S | 2026-10-17: drew no reflection; now that cubemap (no stock map's props do this) |
| 3D skybox haze, sky down face | differs | L | S | refcmp `sky_*` worst views; haze waits on C10, the down face on C20 |
| 3D skybox fog distances (nuke's far mountains fogged out) | differs | M | S | refcmp above: nuke prefers the fog undivided by the sky_camera's scale, office divided; capture C10 |
| Fog colour by view direction (`env_fog_controller` `fogblend`) | missing | L | S | cs_compound and de_port blend a second fog colour (`fogcolor2`) by how far the view looks along a direction; ours fogs in `fogcolor` alone. No spec gives the blend's curve, colour space or direction keys (shaders.md Q21, open-questions.md); windows-captures sitting 4 item 3 shows compound's two directions. Fog starts at 3000 and 2048 units there, so only the far skyline would change |
| Static props lit in the 3D skybox (nuke's cooling towers teal, CS:S light grey) | have (fixed) | M | M | the sun ray ended at the map's underside far above the skybox, not at the skybox's sky: now it ends at the first sky brush (other-maps.md 16) |
| Blood decals on characters | missing | M | M | backlog 3 "Decals, remaining"; no spec covers model decals (engine-side): a spec session, then a capture of a shot bot's body |
| Colour correction option | have (fixed) | L | S | round 2: Video > Advanced's ColorCorrection box sets `mat_colorcorrection` (the post-process was in, the control greyed) |
| Motion blur option | missing | L | M | video-settings.md (greyed): Source's camera blur needs a spec |

## Maps and entities

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Entity I/O, triggers, movers, breakables, doors, fire | have | | | specs/source |
| Breakable glass decals, brush propdata templates, bullet trace effects | have (new) | L | S | func_breakable glass takes "GlassBreak", bulletproof glass "BulletProof" and never breaks, a func_breakable_surf window no bullet decal (its panes are drawn); a brush's `propdata` template sets its health, multipliers and gibs (our reading of "fills in", tech-debt "Breakables"); half the bullets on a computer brush spark (electric sparks, "Breakable.Computer"), half on bulletproof glass ricochet (`Bounce.Shrapnel`; no spec gives the ricochet a look) |
| Breakables' damage types, broken-window drawing | differs | L | M | damage types measured 2026-10-17 (breakables.md Q1-Q5: guns and knife deal bullet damage, x0.5; scaled damage truncated then taken off; a broken func_breakable gone at once, was 7 ticks; shots keep knocking panes out of a broken window, as ours); open question 8 needs captures; the jagged edge pieces are the edge textures read as masks over the crack material, a whole pane deep (were drawn plain white; which edge goes where and how deep is ours); the crack material (ShatteredGlass) lit `$unlitfactor` toward full brightness, unset 0.3 measured from a cs_militia screenshot (was lightmap only: too dark in shade) |
| Soundscapes | have | | | |
| Stock objectives (bomb, hostages) | have | | | hypotheses: tech-debt "The bomb and hostages" |
| Props collide by their `.phy` | have | M | M | round 2: the row was stale: props with a `.phy` collide and are swept by its convex pieces (static props without one block nothing, physics props without one don't collide); only prop entities that aren't physics props and have no `.phy` use their visible mesh (tech-debt) |
| Pushers (no bevels, no players or props pushed) | differs | L | M | tech-debt "Pushers"; a rider is carried on the lift's own tick, as ours (measured 2026-10-18, doors_buttons.md Q4) |
| Round restart keep-list | have (fixed) | M | S | measured on all stock maps (entity_io.md Q1): 22 map classes kept (was the public multiplayer list's 10: env_fog_controller, env_sun, func_illusionary, point_viewcontrol, the spawn points... added); logic_auto fires OnNewGame, OnMapSpawn and OnMultiNewMap again each round, never OnMultiNewRound (was ours: OnMultiNewRound) |
| Brushes parented to physics props | have (fixed) | L | S | 2026-10-18: cs_havana's monitor screen (a func_breakable parented to a prop_physics_multiplayer) trailed the prop by a tick (followed before the physics step: 4 units off a tumbling monitor), its collider was solid to the prop carrying it, and a func_wall or func_illusionary on a physics prop got no node to move; now every brush parented to a physics prop or brush follows its body again after the physics step, place and turn, touches no physics body and is still hit by shots (on a func_physbox it was unshootable); tests `logic_physics`, `heavy::map_breakables::havana_monitor_screen_rides_the_monitor` |
| Physics shadow controller | differs | L | M | tech-debt; the 350 kg push limit measured hard (340 kg pushed, 360 not: physics_props.md Q7), as ours |
| Players shoving multiplayer props | have (fixed) | M | S | measured 2026-10-18 (physics_props.md Q12): CS:S shoves them every 7 ticks (1000 kg·units/s at most), ours shoved every tick, 7 times as hard: barrels and crates flew off players walking through them |
| Physics props' motion | have (fixed) | M | S | measured 2026-10-18 (physics_props.md section 7): friction multiplies the two surfaces' (ours averaged with the world's 0.5: a crate slid 0.65 g where CS:S's 0.61, ice 0.3 g where 0.09); air drag ½ρAv²/m (ours had none: a crate thrown at 1000 units/s by a blast kept it, CS:S's loses about 950 units/s²); rebound 0.14 of the landing speed for ordinary surfaces (ours 0.05); the `.phy`'s stored inertia (70-80% of the solid's: props spin faster); sleep 0.5 s after stopping (ours 1 s); a prop_dynamic with no `.phy` stops nothing, with solid 2 its turned box; a static prop's bbox solid is the world box around its turned bounds (was turned with it). Left: contacts rest 0.4 units apart in CS:S (ours touch: a collision margin of 0.42 units was tried 2026-10-18 and made de_dust2's and de_port's placed props shift and fall in the heavy tests), the rebound is a fit, no angular drag |
| Constraint motors and locked ragdoll constraints | have (fixed) | L | S | measured 2026-10-18 (physics_constraints.md Q2, Q7): phys_hinge SetAngularVelocity (deg/s, clockwise about its axis) and phys_slideconstraint SetVelocity (5 × the value in units/s) were ignored, now drive the body; a phys_ragdollconstraint with every limit equal is welded (was a free ball joint); hinge friction measured as having no effect; a torque limit breaks no weld at rest (ours broke them by a reading of the units) |
| Offset thrusters' turn | have (fixed) | L | S | measured 2026-10-18 (physics_brushes.md Q3): a phys_thruster's arm is from the body's centre of mass (a torque-only thruster level with an oil drum's centre spins it in place, one at its base tips it); ours took it from the body's origin; the turn rate itself unchecked (the probe reads no angles); test `map_game_entities::an_offset_thruster_turns_about_the_centre_of_mass` |
| Solid map prop_ragdolls shoved by players | differs | L | M | measured 2026-10-18 (physics_brushes.md Q6): CS:S's players shove a solid one; ours stop at it; letting the shadow touch the parts launched the player over a lying corpse (tried, reverted); no stock or community map has a solid one |
| phys_spring, phys_pulleyconstraint | missing | L | M | measured 2026-10-18 (physics_constraints.md Q4, Q8): springs are `constant` × stretch (kg·units/s² a unit), pulleys move the second body 1/gearratio as far; no stock map uses either (entsweep) |
| Prop damage left-outs | differs | L | S | tech-debt; measured 2026-10-17 (prop_damage.md Q1-Q14): knife hits are bullets (were clubs), scaled damage truncated then taken off (was the result truncated: one more point left), exploding props and env_explosions fall off as a bell σ = R/3 past R (were linear), a player shoving a sleeping prop wakes it (OnAwakened); a prop_physics meeting a player is two contacts hurting both (now ours too: a 30 kg crate landing at 544/827/1219 in/s does 5+5, 5+10, 10+20 and loses 5 at 1219; the two contacts' speed shares are fitted), multiplayer props never hit players; a 500 kg cupboard 5+50 up to 500+500 (2026-10-17, second round: ours matches every first landing now, the shares by the prop's mass; fixed 2026-10-17, third round: a landing hurts no more after its two contacts, as CS:S's (ours slid off the head and was counted as a second landing, 5+50 more at 300); the 200 stress crush now ours: a 4000 kg prop resting on a player crushes it every tick, 3000 kg doesn't, as measured; CS:S's crush the tick after a 1450–1500 in/s landing (not 1400, 1520–2000) isn't); running into props hurts nobody, as ours; client halves are capture C29 |
| Tool-textured volumes drawn | have (fixed) | M | S | cs_militia's two `func_footstep_control` volumes (one by the Deck ladder) and its skybox `env_embers` drew as yellow `tools/toolstrigger` walls: stock maps compile trigger brushes without the trigger surface flag, so only brush entity classes CS:S draws (`bsp::draws_brushes`) draw now; also env_embers on 2 community maps, env_bubbles on 1 |
| Brushes under displacements solid | have (fixed) | H | S | a playtest walked through cs_militia's concrete wall by the back porch: the world brush a displacement is built from was dropped from collision (matched by plane and base corners), but CS:S keeps it solid (probe traces start solid inside all 58 such solid brushes on cs_militia, de_aztec and de_dust2; the bullet mask can't test player clips); now every player-solid brush collides by its contents (`bsp::brush_kept`). Back: 13 on cs_militia (7 of them player clips), 43 dust2, 11 de_aztec and de_prodigy, 8 de_piranesi, 7 cs_compound, 6 de_cbble, 5 cs_office, 3 de_inferno, 2 cs_havana, 1 de_nuke. movecmp `militia_porch_wall` matches CS:S exactly |
| func_footstep_control's footstep sounds | have (fixed) | L | S | feet inside play the `destination` surface's steps for the `source` surface's (public FGD; `sound::step_remaps`): cs_militia's two turn ladder steps into concrete and wood_plank; feet-point test, kept step volume and timing unmeasured (tech-debt) |
| func_dustcloud | have (fixed) | L | S | round 2: drawn as dust with world-sized motes (sprites_dust.md 7), inside their brushes; de_train's two after the bomb; frozen dust volumes too |
| Steam heat-wave jets (`Type` 1) | missing | L | S | tech-debt; no stock map has one (every stock env_steam is Type 0) |
| Beams' noise and scroll | have (fixed) | L | S | round 2: fractal noise each frame, segments, texture repeat, TextureScroll (the spec's s² quirk) and sprite frames on lasers and env_beams (de_cbble's and de_inferno's chimney smoke scroll); the scroll speed is capture C19 |
| Community-map entity readings | differs | M | | tech-debt rows "Community-map", "Minigame flows"; measured 2026-10-18: overlapping triggers (the lowest-index teleport wins; fixed, ours took the highest), a trigger_hurt's first pass 4 ticks after the touch (fixed, was 1), env_laser damage through armour (fixed), func_tracktrain ignoring I/O Use (fixed, ours toggled it), no class absent from the stock maps kept at a restart (as ours) |
| Water and physics (buoyancy) | differs | L | M | tech-debt |
| LOD and fading of props (`fademindist`) | differs | L | S | performance backlog |
| Model doors beyond the spec | differs | L | S | tech-debt; sounds measured 2026-10-17 (doors_buttons.md Q8): the model's `door_options` skin and hardware blocks as ours; fixed: an unlocked +use moves the door 0.2 s after the handle sound (was at once), an Open input plays the handle too |
| Buy zones round spawns on maps without | have | | | merged from main 2026-10-13 |
| `info_map_parameters` buying | have | | | |

## Rules and economy

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Rounds, freeze, buy time, money table | have | | | rounds.md; defaults unmeasured (rounds.md Next 1) |
| Round-end and objective money | have (fixed) | H | S | measured 2026-10-17 (objectives.md Q5): the round's bonuses come as the next round starts (ours paid at the end), a defuse pays 3250 (was 3500), no planter reward (was 300), living losers of a time-out get nothing, hostage maps pay 3000 / 2500 / 3250 plus 150 a living, 100 a led and 850 a rescued hostage (was 3250 / 3500 / 3250 flat), a hostage hit costs 20 a point and the killing hit twice that and 500 (was a flat 1500 a kill) |
| Kill rewards, team-kill penalty | have | | | |
| Survivors keep weapons | have | | | |
| Spectators | have | | | |
| `mp_timelimit`, `mp_maxrounds`, `mp_winlimit`, `mp_chattime`, map cycle | have (fixed) | M | M | were missing (a session never left its map; CS:S's Create Server sets a 20-minute limit, `cfg/settings_default.scr`); now the intermission and the next map of the install's `cfg/mapcycle_default.txt`; details unmeasured (tech-debt), capture C9 |
| Team balance (`mp_autoteambalance`, `mp_limitteams`) | have (fixed) | M | S | `mp_limitteams` was in; `mp_autoteambalance` (1) moves players as the next round starts (`rules::balance_teams`); who moves and the warning unmeasured: C11 |
| `mp_tkpunish`, `mp_fadetoblack` | have (fixed) | L | S | both 0 as CS:S; a team killer dies as the next round starts; the dead fade to black over 1 s (ours) |
| Ammo prices and sizes | have (fixed) | M | S | measured (weapons.md M19): ammo free, bought guns and spawn pistols full, the .45 box 25; we charged per box and sold guns with the clip only |
| Bomb and hostage details | differs | M | | tech-debt; objectives.md open questions; the bomb blast measured and fixed 2026-10-15 (a bell, σ = reach / 3, was linear; armour ratio 1, armour running out spills onto health); hostage damage measured and fixed 2026-10-16 (objectives.md Q8: the hostage's own hitgroup multipliers, a second hit through its body at half damage, health a float; `weapon::BulletTarget`); hostage models and rescue rules measured and fixed 2026-10-17 (objectives.md Q9: the four models in turn over rounds, was by index; Q11: only a following, walking hostage whose 26-wide hull touches a zone is rescued, no CT-spawn fallback, was any hostage, player-wide hull, 256 units round CT spawns); hostage spawn heights measured and fixed 2026-10-17 (objectives.md 9 "Spawn": they fall from the map's origin and rest 1/32 above the floor, within 0.1 unit of CS:S on six cs_ maps; they took the local player's movement, so noclip left them hanging at the origin, cs_office's first two 36 units up, and a short drop left them up to 2 units above it); bots added in noclip flew too (fixed 2026-10-17: bots and hostages take the match's movement, `movement::match_movement`); hostages re-created at every round start confirmed on the probe 2026-10-17 (new references after a round end and `mp_restartgame`, models moving on); round money's last cases measured and fixed 2026-10-17 (objectives.md Q5: a draw pays nobody, was the loss bonus to both; an elimination off a bomb map pays 3000, was 3250 on maps without objectives; the clock ends nothing on a map without objectives, was a defenders' win, kept as `chimera_untimed_win 1`); the bomb's blast goes through walls and props, measured and fixed 2026-10-18 (objectives.md Q3: the bell's whole value behind de_dust2's B stone wall; walls stopped ours); the bomb's think measured and fixed 2026-10-18 (Q14: the explosion and a finished defuse wait for the bomb's think every 8 ticks from 7 after the plant, so 45.105 s and 10.0-10.105 s; due on one think the bomb blows; ours was on the tick due with the defuse first); the hostage's give-up distance measured and fixed 2026-10-18 (Q10: 2000 units, was 1500) |
| Draw rules on one-sided maps | have | L | | measured (objectives.md Q15): all of the one side dead is a draw at once, as ours |
| Freeze-time reload | have | L | | measured 2026-10-15 on the probe server: CS:S reloads in the freeze time at the usual pace, no firing or moving; as ours |

## Networking feel

| Item | Status | Impact | Cost | Notes |
|---|---|---|---|---|
| Prediction, interpolation, lag compensation | have | | | multiplayer.md |
| Server browser, LAN, passwords, downloads | have | | | |
| Chat and radio over the network | have | | | |
| `cl_interp`, `cl_updaterate`, `rate`, `cl_lagcompensation` | have | | | |
| Movers mispredicted for a round trip | differs | L | M | tech-debt "The network so far" |
| `cl_cmdrate` | differs | L | S | missing as a cvar (one command per tick) |
| Voice chat | have (new) | M | L | Opus over the game's connection, `sv_alltalk` (teams, the dead to the dead), `sv_voiceenable`, `voice_*`; the HUD's talking icon and list; per-player mute (the player list), open microphone (`voice_vox`); no icons over heads (voice-chat.md) |
| Sprays | have (fixed) | M | M | `impulse 201`: one per player, `decalfrequency`, kept through rounds, gone with the map; replicated by name, an imported logo as its picture (64 x 64 at most); capture C23 |
| Demos | missing | L | L | |
| Votes (kick, map) | differs | L | M | `callvote kick <userid>`, `changelevel <map>`, `restartgame`; F1/F2 vote; setup dialog (`callvote` alone; no Esc entry, as CS:S's `GameMenu.res`); joiners vote; bots vote (ours); cvars measured, rules and panel ours (tech-debt "Votes"); capture C25 |
| Game events and server log | have (new) | L | M | 2026-10-18: the specs' events (bomb, hostages, rounds, `weapon_fire`, `bullet_impact`, `player_hurt`, `player_death`, `item_pickup`, grenades) for plugins (`game_log::GameEvent`) and their `triggered` lines in a half-life-standard log (`log on`, `sv_logecho`, `sv_logfile`: `logs/`); UTC stamps, `STEAM_ID_LAN` (tech-debt); 2026-10-18 the log's lines and the events' fields measured (objectives.md Q17): kill, suicide and defuse-begin lines and `bomb_planted`'s posx/posy added; attack, chat, connect and team lines and some fields left |
| Kill, attack, chat, connect, team and purchase log lines | missing | L | S | no spec defines them (objectives.md question 17) |

## Fixed in this pass

- `invnext` / `invprev`: step through every weapon held, slot by slot
  and wrapping, through the HUD selection with `hud_fastswitch 0` (a
  move sound) or straight away with `1` (`client::weapon_select`);
  bindable in Options' keyboard tab (no longer greyed).
  Tests: `weapon_select::invnext_and_invprev_step_through_every_weapon_by_slot`,
  `client::weapon_select` unit tests.
- A pick from the HUD selection names its weapon's place in the slot
  (`core::Intent::pick`), so the second grenade can be drawn (it drew
  the first). Test: `weapon_select::a_pick_draws_the_second_weapon_in_a_shared_slot`.
- The selection's sounds: moving (and opening), picking, an empty slot.
- Buying a weapon or picking one up plays `Player.PickupWeapon`;
  buying armour or a kit `BaseCombatCharacter.ItemPickup2` (ammo kept
  `BaseCombatCharacter.AmmoPickup`). Test:
  `weapon_select::buying_and_picking_up_play_the_pickup_sounds`.
- `autobuy` and `rebuy` (and the buy menu's buttons for them):
  `cl_autobuy` / `cl_rebuy` userinfo lists, CS:S's default files'
  entries by default. Test:
  `rounds::autobuy_takes_the_list_and_rebuy_restores_the_last_buy`.
- Under water everything is muffled (`audio::hearing`). Test:
  `audio::hearing` unit test.
- `jpeg`: a screenshot named after the map. Test: `client::capture`
  unit test.
- The end of a map's game: `mp_timelimit`, `mp_maxrounds`,
  `mp_winlimit`, the intermission (`mp_chattime`, everyone held, the
  scoreboard up, over the network too) and the next map of the
  install's map cycle; `timeleft` (`rules::game_end`). Tests:
  `rounds::the_game_ends_after_mp_maxrounds_with_an_intermission`,
  `rounds::the_time_limit_ends_a_deathmatch_game`, `rules::game_end`
  and `games::cs_source` unit tests.
- tech-debt: the stale "Shot effects are placeholders" row removed.

## Fixed in round 2 (weapons, sounds, HUD, menus, rules, movement, networking)

- The round's end shows the game's win panel (`client::win_panel`,
  `resource/ui/win_round.res` at the HUD layout's `WinPanel_Round`):
  "Terrorists Win", the reason (`winpanel_end_*`), the round's live time,
  "MVP: X for most kills" (or planting, defusing, the last rescue:
  `rules::rounds::Mvp`) and a fun fact (`funfact_*`, `RoundSummary`,
  counted by `rules::rounds::RoundStats`); a client gets it in
  `net::RoundOver` (now entity-mapped). The centre print stays only
  without the layout. Tests: `objectives::*` (planter, defuser, no MVP),
  `net_rules::a_full_round_with_two_clients`, `client::win_panel` unit
  test; screenshot checked on de_dust2.
- The damage indicator is the HUD layout's `HudDamageIndicator`: red
  bars at the screen's sides (`client::hud_sprites::DamageBars`), lit by
  `core::DamageTaken` (damage that reached the health), which a network
  client now gets for its own player (`net::Hurt`; before, a client never
  saw the indicator). Tests:
  `net_weapons::the_player_hit_is_told_which_way_the_damage_came`,
  `client::hud_sprites` unit tests; screenshot checked.
- The scoreboard shows the map's time left with `mp_timelimit`
  (`NetRound::map_started` for clients). Test: `games::cs_source::ui::scoreboard` unit
  test; screenshot checked.
- `mp_autoteambalance` (`rules::balance_teams`), `mp_tkpunish`,
  `mp_fadetoblack` (`client::senses`). Tests:
  `rounds::uneven_teams_are_balanced_as_the_next_round_starts`,
  `rounds::tkpunish_kills_team_killers_as_the_next_round_starts`,
  `client::senses` unit test.
- Commands and cvars: `fps_max` (`client::perf::FrameLimit`), `ping`,
  `users`, `buyequip`, `cancelselect`, `cl_radaralpha`,
  `hud_saytext_time`. Tests: unit tests in `client::perf`, `net`,
  `client::weapon_select`, `client::chat`.
- `NET_VERSION` net16 (`Hurt`, `RoundOver`'s summary, `NetRound`'s map
  start).

### Round 2: rendering, maps and entities

- Beams (`render::beams`, beam.wesl): lasers and persistent env_beams draw
  through the spec's point count with fractal noise new each frame,
  their texture repeating every 100 units and scrolling by
  TextureScroll, and their sprite's frames at its framerate
  (visual_entities.md 1). Tests: `render::beams` unit tests (de_cbble's
  smoke beam's numbers).
- func_dustcloud (`render::dust`): dust with world-sized motes; motes start
  inside the volume's brushes; frozen volumes spawn once and hold
  (sprites_dust.md 7-8). Tests: `render::dust` unit tests,
  `heavy::map_visuals::de_train_dust_clouds_load_and_turn_on`.
- Rope wind (`render::rope_sway`): the spec's gusts sway ropes within
  `rope_wind_dist` of the camera. Tests: `map::rope_sway`, `render::rope_sway` unit tests,
  `heavy::map_de_dust2::ropes_can_sway`. refcmp captures with
  `rope_wind_dist 0` (de_dust2 mean 0.0309 either way; a swaying cable
  moved `sky_up` by up to 0.009 between runs).
- Cubemap mips: baked cubemaps load every mip level (reflections pick
  them as the game's hardware does). refcmp (`CHIMERA_CUBEMAP_TOP_MIP=1`
  for before): cs_office mean 0.0189 -> 0.0185 (`cam0` 0.041 -> 0.038,
  `zone0_*` 0.001 lower each), de_aztec 0.0408 -> 0.0405 (`cam0`,
  `cam1`, `t_spawn`, `zone0_90` 0.001 lower), de_dust2 0.0309 and de_nuke
  0.0242 unchanged. Test:
  `heavy::map_de_dust2::cubemaps_keep_their_mips`.
- Video > Advanced's colour correction box (`mat_colorcorrection`).
  Test: `games::cs_source::ui::options` unit test.
- The "props collide by their visible mesh" row was stale (the `.phy`
  pieces have been used since physics props landed); tech-debt reworded.

## Fixed: the join flow

- Joining a server, or starting a game with `map` (the menus' Create
  Server, `--map`), shows CS:S's join flow (`games::cs_source::ui::join_menus`): the
  message of the day in `resource/ui/motd.res` (the server's name, its
  `motd.txt`: the host's config folder, else the install's `motd.txt`,
  `cfg/motd.txt`, `cfg/motd_default.txt`; sent to clients as they are
  let in, `net::ServerMotd`), OK opens the team menu, a team opens the
  class menu (`classmenu_ter.res` / `classmenu_ct.res`, the hovered class's
  name and description beside it), and the player spawns as the round rules allow
  (in the freeze time, or with nobody of its team alive, this round;
  else the next). Until it picks a team it is on none
  (`core::Unassigned`: no body, watching as a spectator); on a team
  without a class (`core::ChoosingClass`) it is neither spawned nor
  counted for the round's end. The chat says "Player X has joined the
  game" as it connects and "X is joining the Terrorist force" when it
  picks. `motd` shows the message again.
- Differences: we don't render HTML: an HTML message shows its text
  (tags out, line breaks kept); `cl_disablehtmlmotd 1` (now in
  Multiplayer > Advanced) shows the server's plain-text one
  (`motd_text.txt`, else `cfg/motd_text_default.txt`), as CS:S does. A
  message that is a web address shows the address. The class menu's
  picture is CS:S's live view of the class's model (`CSClassImagePanel`
  in `classes/<class>.res`): not drawn, the box stays empty (only
  auto-select's flat picture shows); to do with a render-to-texture of
  the body. Bots keep their
  team's first model (bots are parked). Players keep their team over a
  `changelevel` (CS:S's behaviour there is unmeasured).
- `joinclass <0-8>` (`core::PlayerClass`, replicated; 0 auto-selects one
  of the team's at random); `jointeam 0` auto-assigns. The class's model
  (Phoenix Connexion, Elite Crew, Arctic Avengers, Guerilla Warfare; SEAL
  Team 6, GSG-9, SAS, GIGN) is the body others see and its hitboxes.
- Scripted runs skip it (`rules::JoinChoice`, `--join menus|auto|1|2|3`):
  `--screenshot`, `--views`, `--spawn`, `--look`, `+jointeam <n>` and
  `+spectate` join at once; tests' `Sim`/`NetSim` too (a network client
  says which in its userinfo, `_jointeam`). `bot_join_after_player`
  waits for a player on a team.
- `NET_VERSION` net17 (`ClassRequest`, `ServerMotd`, replicated
  `Unassigned`, `ChoosingClass`, `PlayerClass`). Tests: `join_flow::*`,
  `games::cs_source::ui::join_menus`, `net::motd` and `client` unit tests.

## Next fixes that need no capture

In order of impact over cost:

1. `cl_cmdrate` (CS:S's default 30 sends commands in fewer packets than
   ours, one a frame): a choice for the network feel, not a gap players
   see; left (S).
2. Ricochet sounds once spec Q7 says when they play (spec session).

1. The scoreboard's map time left (`game_end::time_left`) (S).
2. Team balance: `mp_limitteams` on joining, `mp_autoteambalance` at a
   round's end (M; rules as publicly documented, details unmeasured).
3. `fps_max` (S).
4. The MOTD on joining (`motd.res`, `cfg/motd.txt` else
   `motd_default.txt`) (S).
5. Ricochet sounds once spec Q7 says when they play (spec session).
6. Rendering and maps (round 2's leftovers, all L): env_steam's
   JetLength/Speed/SpreadSpeed inputs (visual_entities.md 7); heat-wave
   steam (Type 1: a refracting quad; no stock map has one); per-vertex
   tangents for world reflections (tech-debt row); env_wind (detail
   sway is in; ropes and dust would follow it).

## Captures wanted

The ones needing the CS:S client are grouped into sittings in
[windows-captures.md](windows-captures.md).

For the coordinator, on the reference client (listen server, de_dust2
unless named; `sv_cheats 1`; bots with `bot_stop 1` as targets). Each
says what decides the open point.

- **C1 Hit slowdown and flinch.** Probe server (movecmp harness): a bot
  running forward at full speed (knife) is shot once in the chest and
  once in the leg by a second bot with a USP and an AK-47; log the
  victim's horizontal speed and view angles every tick for 1 s after
  each hit. Decides: the speed factor and its recovery, and the punch
  (pitch/yaw, size, decay) by hitgroup and weapon.
  Done 2026-10-14: specs/cs_source/weapons.md "M18 Being hit".
  Still open: a helmeted head hit's punch (we give none), whether the
  flinch replaces or adds to recoil punch already on the view, and a
  shotgun's flinch (one pellet's damage or all of them).
- **C2 Pickup and buy sounds.** `sv_soundemitter_trace 1` on the
  listen server: buy an AK-47, kevlar, kevlar+helmet, a defuse kit, an
  HE grenade and ammo; drop the AK and walk over it; touch a dropped AK
  while carrying one (ammo only). Decides which entry each emits
  (`Player.PickupWeapon`, `BaseCombatCharacter.ItemPickup2`,
  `BaseCombatCharacter.AmmoPickup`, none), and whether the round's
  starting pistol plays one at spawn.
  Server side done (2026-10-14, dedicated server, bots driven by the
  probe plugin's `mashup_botcmd`, `sv_soundemitter_trace 1`): buying an
  AK-47, a Deagle or a smoke grenade emits `BaseCombatCharacter.AmmoPickup`
  from the buyer; kevlar, kevlar+helmet and a defuse kit emit nothing;
  night vision `BaseCombatCharacter.ItemPickup2` from another entity;
  every spawn emits `AmmoPickup` from the player (the starting pistol);
  `Player.PickupWeapon` (itempickup.wav, the same wave) showed once, from
  a bot that walked over dropped guns, with `ItemPickup2` from those
  guns. Not changed yet: what the buyer's client plays itself (a
  listen-server capture with a human, which C4 and C11 need too).
- **C3 Weapon selection sounds.** With `hud_fastswitch 0`: press 2,
  then 2 again, then 4 with no grenade, then the wheel twice, then
  attack; again with `hud_fastswitch 1`. Record the client's audio (or
  `snd_show 1`): which of MoveSlot / Selected / Open / Close / Deny
  plays at each step.
- **C4 Autobuy and rebuy.** As a CT with $16000 and an AWP in hand,
  run `autobuy` (does it drop the AWP for the M4A1?); then with $16000,
  `buy ak47` as a T, `buy vesthelm`, `buy flashbang` twice, die, set
  money to $2000 and `rebuy` (what is bought, in what order), then
  `rebuy` again next round with $16000 (does the second rebuy remember
  the first snapshot or what the short round bought?). Also the
  console text each prints.
- **C5 Under water.** Record the client's audio standing in de_aztec's
  water with the head under while a bot fires an AK-47 at 500 units,
  and the same with the head above. Decides the muffle (cutoff, level,
  any reverb) to replace `audio::hearing::UNDERWATER`.
- **C6 Drowning.** Stay under water in de_aztec's river (`noclip` off,
  head under) for 30 s with `sv_soundemitter_trace 1` and `developer
  1`: when damage starts, how much per hit and how often, the sounds,
  and whether health comes back after surfacing.
  Done 2026-10-14 on de_nuke (aztec's river is too shallow to go under):
  specs/cs_source/movement.md "Drowning".
- **C7 Round win panel.** Screenshots at 1280x720 of a round's end by
  elimination, by bomb, by defuse and by time: the panel's place and
  text, how long it stays, whether a centre print shows too.
- **C8 Damage indicator.** Screenshots 0.1, 0.5 and 1 s after being
  shot from the left, right, front and behind (a bot with `bot_stop 1`
  turned toward you by `setang`, `ent_fire !picker` with a USP, or a
  second client): the arcs' place, colour and fade.
- **C10 3D skybox fog and lighting (de_nuke, cs_office).** On the
  refcmp views `cam16` and `cam2` (de_nuke) and `cam0` (cs_office):
  captures with `r_3dsky 0`, with `fog_override 1; fog_enableskybox 0`,
  and as they are. Decides whether nuke's far mountains are 3D skybox
  geometry or the 2D sky, so whether the sky_camera's fog distances are
  in skybox or world units (`map::material_fog`); the cooling towers'
  colour without fog says what their light probe should give.
- **C9 Defaults.** `mp_flashlight`, `mp_timelimit`, `mp_maxrounds`,
  `mp_winlimit`, `mp_chattime`, `mp_autoteambalance`, `mp_limitteams`,
  `fps_max` values on a fresh listen server started from Create Server
  (RCON `cvarlist mp_`); and the map change at the time limit: does
  the round finish first, how long is the intermission, what shows.
  Partly done (2026-10-14, fresh dedicated server, RCON): mp_roundtime
  2.5 (ours was 5: fixed), mp_buytime 1.5, mp_freezetime 6,
  mp_startmoney 800, mp_c4timer 45, mp_chattime 10, mp_limitteams 2,
  mp_autoteambalance 1, mp_teams_unbalance_limit 1, mp_flashlight 0,
  mp_timelimit/maxrounds/winlimit/fraglimit 0, mp_hostagepenalty 13,
  mp_forcecamera 1, mp_playerid 0, mp_fadetoblack 0, sv_alltalk 0: the
  rest match ours. Still open: the listen server's values after Create
  Server (settings.scr). Time-limit map change (2026-10-14, bots,
  mp_timelimit 1): the round past the limit plays out;
  "Intermission_Time_Limit" comes in the same second as its "Round_End"
  (ours waited the 5 s end pause: fixed), the next map loads mp_chattime
  (10 s) later. What shows during it: unmeasured.
- **C11 Team balance.** Listen server, `mp_autoteambalance 1`,
  `mp_limitteams 0`, `bot_join_team T; bot_add` four times and one CT
  bot; play a round to its end. Screenshots and `developer 1` console
  text at the round's end and the next start: does "*** Auto-Team
  Balance next round ***" show (where), which bots move (note each
  one's score and join order first: the top scorer, the lowest, the
  newest?), at the end or the next round's start, and is a moved player
  respawned at once or kept dead.
  Partly done (2026-10-14, dedicated server, bots only): four T bots
  against one CT bot stayed so for five rounds: bots are never moved
  (ours now matches). Still open, needs a human client: which human
  moves, the warning, respawn.
- **C12 Win panel contents.** Ten rounds on de_dust2 with bots
  (`bot_quota 9`): screenshot each round's win panel at 1 s and 4 s
  after the end, with the round's kills from the console (`developer
  1`) and the scoreboard: which fun fact shows given the round's events,
  who is MVP on a time-out win and with tied kills, the title's colour
  for CTs and for a draw, and whether the panel goes when the next
  round starts or before.

- **C13 Ragdoll throw per weapon.** A bot standing still (`bot_stop 1`)
  on de_dust2's long A, killed from 5 m by one chest shot each with a
  Desert Eagle, a USP, an AK-47, an AWP, an M3 and a knife stab, and by
  an HE at 2 m; demo recorded (`record`), then `phys_pushscale 3` and the
  Deagle again. Decides the ammo impulses (ragdolls.md Q2, weapons.md
  Q8) and whether the knife uses the melee force.
- **C14 Player legs.** A demo of a bot walking (`bot_walk 1`), running,
  crouch-walking and strafing on flat ground (de_dust2's CT spawn),
  stepped frame by frame with `demoui`: the legs' cycles per second at
  each speed. Decides the ground speeds and playback rate (animation.md
  open questions; tech-debt "Player animation").
- **C15 Muzzle flash and view-model light.** In de_dust2's lower tunnel
  (refcmp view `tunnel_floor_e`'s eye), `host_timescale 0.1`: screenshots of the
  first two frames of an AK-47, USP, M3 and AWP shot, with
  `muzzleflash_light 1` and `0`; then the eye in shadow under a lit edge
  (B doors' arch) with `r_DrawModelLightOrigin 1`. Decides view_models.md
  Q5 (material, size, duration, wall light) and Q7 (lighting point).
- **C16 Round restart keep-list.** Done 2026-10-15 on the probe server
  without a test map (tools/css_probe/roundkeep.py and the mashup_ents
  plugin's logic_auto hook): `world::ROUND_KEEP` and logic_auto's firing
  are measured (entity_io.md Q1).
- **C17 HDR references.** `refcmp capture-ref` on de_dust2 and de_nuke
  with `mat_hdr_level 2` (the game's default; restart the map after
  setting it), 5 s after each view is placed (auto-exposure settles), into
  a separate folder. Decides `client::hdr`'s exposure and bloom.
- **C18 Ropes in the wind.** refcmp view `sky_n` (de_dust2) and a
  de_cbble view of a cable (it has an env_wind): 20 `jpeg`s 0.25 s
  apart. Decides the gusts' amplitude (the spec's 50 units/s², from the
  SDK) and what env_wind does to ropes.
- **C19 Beam scroll.** de_cbble's chimney smoke (env_beam `smoker1`,
  `setpos -2900 -224 700; setang 0 180 0`), `host_timescale 0.1`, a
  `jpeg` every 0.05 s for 1 s of game time, and the same with
  `ent_fire smoker1 ScrollSpeed 1`. Decides visual_entities.md Q10 (s²
  or s lengths a second).
- **C20 Sky's down face.** de_dust2 with `noclip`, from 200 units above
  each of the map's four outer walls looking 60° down past them, and
  de_aztec likewise, `r_3dsky 0`: the 2D sky below the horizon. Decides
  the down face's orientation (tech-debt "Sky").
- **C21 Prop LOD switches.** As tech-debt "Levels of detail" says
  (de_dust2's pots, backing away with `r_lod -1`, a `jpeg` every 50
  units); also moving back toward them (hysteresis).
- **C22 Detail props.** refcmp-style views over cs_militia's and
  de_port's grass (`cl_detaildist 1200`), close (200 units) and far
  (1000): density, size, sway.
- **C23 Sprays.** A listen server on de_dust2, `sv_cheats 1`: spray
  (T) at a wall from 50, 100, 120, 130 and 150 units (`getpos`, a
  `jpeg` each); one on the floor and one on a ceiling looking north,
  then east; `decalfrequency 2` and two sprays 1 s apart. Decides the
  reach (ours 128 units), the size on the wall (ours 64 units), a
  floor spray's turn, and whether a refused spray says anything.
- **C24 Night vision.** de_dust2's lower tunnel and CT spawn in daylight:
  a `jpeg` with night vision off and on, same view (`setpos`,
  `setang`); a second player standing by while the first toggles it,
  with `snd_show 1` on the second. Decides the overlay's colour and
  brightening (ours: the frame plus the frame times green), any light
  around the player, and who hears the toggle (ours: the owner).
- **C25 Votes.** A listen server on de_dust2 with a second human client
  (a second PC, or a friend): `callvote` alone (the setup dialog or the
  console's issue list: which issues CS:S offers), then `callvote kick
  <userid>` (`users` lists them) and `callvote changelevel de_dust2`; a
  `jpeg` of the running vote's panel, of the passed and failed panels and
  of a refused call (call twice in a row); press F1 during a vote (does
  it vote, or autobuy as bound?), and a vote where only the caller votes
  (does it end at once or after 15 s?). Decides the issue list, the
  panel's look and place (CS:S's HUD layout turns `CHudVote` off, the
  only panel file is hl2's, "likely out-of-date"), the issues' words and
  the early finish (ours: once everyone voted).
- **C26 Automatic hints (`cl_autohelp`).** A fresh profile (or
  `cl_autohelp 1` after deleting `cfg/config.cfg`), a listen server with
  bots on de_dust2, then cs_office: note (screenshot or a short video)
  every hint in the hint panel and when it came: spawning in the buy
  zone, the first enemy and teammate seen, a kill, hurting and killing a
  teammate (`mp_friendlyfire 1`), getting the bomb, entering a bomb
  site with it, emptying a gun with no spare ammo, buying night vision,
  near a hostage as T and CT, leading one, the rescue zone, killing one.
  Then: does a hint come again later on the same map, after
  `mp_restartgame 1`, after `changelevel`; how long one stays; how far
  apart two come; whether `cl_autohelp 0` stops them and whether ones
  missed then come once it's back on; whether bots' events show any.
- **C27 Props seen past sky brushes (cs_assault).** `refcmp capture-ref
  --views tools/refcmp/cs_assault.toml --only pvs_`: from the roof
  (`pvs_roof_90`, `pvs_roof_180`) and the T side's street (`pvs_street_180`,
  `pvs_yard_180`). Our PVS culling hides the water tower, rooftop props
  and buildings there, seen over the walls through sky brushes, and the
  map's data agrees (docs/performance.md, "Checks"); the captures say
  whether CS:S hides them too (sky there) or draws them.
  Decides every trigger in `games/cs_source/data/hints.toml` (all ours).
- **C28 Static props without baked light (cs_militia).** In CS:S at
  `mat_hdr_level 0`: `setpos 1536.72 928.09 -65.93; setang 9.81 227.75 0`
  (the crates on and beside the truck by the shed), once as is and once
  with `r_drawmodellightorigin 1`. Whether the crates by the shed are as
  dark as ours (lit from one point in the shed's shadow) or shaded per
  vertex (their tops in the sun) decides shaders.md Q11 for maps without
  `.vhv`; the truck (its lighting point inside its cargo's nodraw
  collision block, dark here) is in the same view.
- **C29 Client props and break sounds (cs_italy, de_inferno).** On a
  listen server: knife and shoot a client-side cardboard box
  (Cardboard.breakclient 10) and a mug or bottle (Pottery 20) on cs_italy:
  how many hits each takes (does the knife count as the "other" 50 or as a
  bullet's 30?), and record the audio: does a bottle breaking make a sound
  beyond the bullet impact? Then shoot one of de_inferno's `Glass `
  windows (its surface name ends in a space): `Glass.Break` or no break
  sound? Break a client bottle and `mp_restartgame 1`: is it back?
  Decides prop_damage.md open questions 2 (client half), 4, 7, 8 and 9
  (client half).
- **C30 Refused hits, client side and god mode.** A listen server on
  de_dust2, `sv_cheats 1`, `bot_stop 1`, `mp_friendlyfire 0`, a bot on
  your team: shoot it in the chest (AK-47) and knife it, with the game's
  audio recorded and screenshots 0.1 s after: does your own client draw
  blood or play a flesh impact for the refused hit (the server sends
  neither)? Then `god`, buy kevlar, `mp_friendlyfire 1` and let an enemy
  bot shoot you: armour left on the HUD, any hurt or kevlar sound, the
  flinch and slowdown, blood. Decides weapons.md "Refused hits" (god mode
  is ours: like a refused hit, no armour, sound, slowdown or flinch).
- **C31 HE ear-ringing for players it can't hurt.** A listen server on
  de_dust2, `sv_cheats 1`, `bot_stop 1`, `mp_friendlyfire 0`, a bot on
  your team: let it throw an HE at your feet (`bot_add_ct`, or throw it
  yourself off a wall so it lands at a teammate) with the game's audio
  recorded, then `dsp_player` in the console right after; the same in
  `noclip`, and once with `mp_friendlyfire 1` as the control (hurt: DSP
  32-37). Does a refused blast ring your ears? The server's DSP change
  goes to the hurt player's client only (a SourceTV demo shows nothing).
  Decides grenades.md Q11 (ours: only players the blast hurts).

- **C32 Planting the bomb.** A listen server on de_dust2, `sv_cheats 1`,
  `mp_freezetime 0`, on T with the bomb at A: a screen recording (or
  screenshots every 0.25 s) from pressing attack until the bomb is down,
  first person, with `cl_c4progressbar 1` and then 0: when each digit
  shows on the C4's screen against the fingers' key presses, the masked
  `*******`, its font and colour, and whether any progress bar shows
  (objectives.md Q16). Then a defuse with each value (the bar). Then
  `bot_add_t` and watch a bot plant from beside it (`spectate`, third
  person): its body's planting animation and the bomb in its hand.
  Ours: no bar while arming with either value; the screen's text and
  times read from v_c4's press-button sequence (model event 7001).
