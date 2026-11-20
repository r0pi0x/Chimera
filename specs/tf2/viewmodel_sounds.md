# Team Fortress 2: weapon sounds played by the view model's animations (reloads first)

Source basis: Valve's Source SDK 2013 (2025 TF2 release) read in place in
a spec session: the client's animation-event handling (which events a
model's sequences carry, when the client fires them, who hears a sound
event), the shared weapon base's sound roles and recipient rules, and
the TF weapon base's reload paths (whole clip and one round at a time,
the reload's playback rate). Data: the nine class arms' animation models
(`models/weapons/c_models/c_<class>_animations.mdl` in
`tf2_misc_dir.vpk`), read in place with a dumper of ours (each
sequence's frames, fps, fade times and its event list: cycle, event
number, options), and `scripts/game_sounds_weapons.txt`. Measured on
2026-11-20 against real TF2 (client build 11191711, listen server,
`host_framerate 0.015`, `sv_soundemitter_trace 1` printing every sound
the server (sv) and the client (cl) start, the local player's stock
Pistols, the view model's sequence and cycle read every tick). Nothing
copied; restated in our words. Continues [weapons.md](weapons.md),
[weapons_more.md](weapons_more.md) §9.4 and [viewmodel.md](viewmodel.md).
Status: draft (checked against the SDK and measured)

## Summary

A TF2 gun's reload is heard two ways. The shooter hears the **first-
person view model's animation**: its reload sequences carry sound
events at fixed points of the animation (the clip out, the clip in, the
drum, the shell, the pump), and the shooter's own client plays each
when the view model's animation passes it, for the shooter alone. Every
other player hears the **weapon script's world reload sound**, played
by the server once when a reload starts (and once per round for guns
that load one round at a time), from the shooter, to everyone but the
shooter. So the Pistol's reload is a different set of sounds per class
only because the Scout's and the Engineer's arms animate it differently.

## Units and conventions

Seconds; server tick 0.015 s. A sequence's length is (frames − 1) / fps.
An animation event sits at a **cycle** (0 to 1 of the sequence); its
time into the sequence is cycle × length / playback rate. Every stock
reload plays at playback rate 1 (below), so the times in §4 are
cycle × length.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| sound_event | 5004 | – | the client-side animation event "play this sound entry" (its option text is the entry's name in `game_sounds_*.txt`) |
| clip_reload_early | 0.2 | s | a whole-clip reload counts as done this long before its reload sequence ends (weapons_more.md §9.2) |
| pistol_reload_scout | 1.2 | s | `p_reload`, 37 frames at 30 fps; the reload takes 1.0 s (measured 67 ticks, 1.005 s) |
| pistol_reload_engineer | 1.2333 | s | `pstl_reload`, 38 frames at 30 fps; the reload takes 1.033 s (measured 69 ticks, 1.035 s) |

## Behavior

### 1. Two sources of a reload's sounds

1. **The view model's sound events (the shooter only).** Each sequence
   of the class arms' animation model may list events; event 5004 with
   an entry name plays that sound. The shooter's client fires the
   events of its own view model as the view model's animation passes
   them (each frame it fires every event whose cycle lies between last
   frame's cycle and this frame's; on a looping sequence also across the
   wrap). The sound plays **to the local player only**, from the weapon
   (at the event's attachment if it names one; none here do), so
   nobody else hears these. Only the owner draws a view model, so other
   players never run these events at all.
2. **The script's `reload` sound (everyone else).** When a reload
   starts, the server plays the weapon script's `reload` role (a
   `…WorldReload` entry, weapon_scripts.md) from the shooter with the
   predicted-weapon recipient rule: every player in hearing range
   **except the shooter**. A one-round-at-a-time gun plays it again at
   the start of each round's insert. The shooter's own client does not
   play it (measured for the Pistol: four reloads, two per class, the
   server's `Weapon_Pistol.WorldReload` at the start and no client copy;
   for the Scattergun and Shotguns the same: the server's
   `…WorldReload`, the client's view-model entries).

The shooter's reload is therefore exactly the list of §4 for his class
and gun, timed by the view model; others hear one world reload sound per
reload (per round for shell guns), started on the server's reload tick.

### 2. Timing

- A reload sequence starts on the tick the reload starts (whole clip:
  one sequence; one at a time: the start sequence, one loop sequence per
  round, the end sequence; weapons.md §4). Each start restarts the
  sequence at cycle 0, so a loop sequence's events play once per round.
- An event's time into its sequence is cycle × length at playback rate
  1. A reload's playback rate is (the reload's base time / its actual
  time), 1 for every stock weapon with no reload-speed attribute; it is
  set when the reload starts (a new sequence resets the rate to 1, the
  reload then sets it).
- An event is skipped if its sequence is replaced before reaching it
  (a reload interrupted by firing or switching plays only the events
  already passed).
- The client fires the events in its own frame loop, from the predicted
  view model: at 66.7 frames a second or more the sound starts within a
  frame of its time. Measured (Pistol, cycle of the server's view model
  on the tick the client started the sound): Engineer `ClipIn` at 0.365
  and 0.377 (event at 0.378); Scout `ReloadScout` at 0.0125 to 0.025
  (event 0.028) and `ClipIn` at 0.4875 and 0.500 (event 0.500). One
  earlier Engineer run started `ClipIn` at cycle 0.523 (0.18 s late);
  taken as a client hitch, not a rule.

### 3. The Pistol, per class

| Class | Sequence (length) | Shooter hears | Others hear |
|---|---|---|---|
| Scout | `p_reload` (1.2 s) | `Weapon_Pistol.ReloadScout` at 0.033 s (cycle 0.028); `Weapon_Pistol.ClipIn` at 0.600 s (cycle 0.5) | `Weapon_Pistol.WorldReload` at 0 |
| Engineer | `pstl_reload` (1.2333 s) | `Weapon_Pistol.ClipIn` at 0.467 s (cycle 0.378) only | `Weapon_Pistol.WorldReload` at 0 |

The entries (`game_sounds_weapons.txt`): `ReloadScout` →
`weapons/pistol_reload_scout.wav`, static channel, 74 dB, volume 0.5;
`ClipIn` → `weapons/pistol_clipin.wav`, static channel, normal level
(75 dB), volume 0.35; `WorldReload` → `weapons/pistol_worldreload.wav`,
static channel, 74 dB, volume 0.5. The Engineer's stock Pistol (item
22) played `Weapon_Pistol.WorldReload` to others, not
`…WorldReloadEngineer` (measured twice).

### 4. Every stock reload's sound events

Times at playback rate 1, seconds from the sequence's start. Sequences
not listed (and the start/end sequences marked "none") carry no sound
event. Non-sound events in the same sequences are noted in brackets.

| Class, gun | Sequence (length) | Sound events |
|---|---|---|
| Scout, Scattergun | `sg_reload_start` (0.2) | none |
| | `sg_reload_loop` (0.5, per shell) | `Weapon_Scatter_Gun.Reload` 0.033 [brass eject at 0.167] |
| | `sg_reload_end` (16 frames at 26 fps, 0.577) | none |
| Scout, Pistol | `p_reload` (1.2) | `Weapon_Pistol.ReloadScout` 0.033; `Weapon_Pistol.ClipIn` 0.600 |
| Soldier, Rocket Launcher | `dh_reload_start` (0.5) | none |
| | `dh_reload_loop` (0.8, per rocket) | `Weapon_RPG.Reload` 0.067 [an "add ammo" event at 0.333] |
| | `dh_reload_finish` (0.767) | none |
| Soldier, Shotgun | `reload_start` (0.467) | none |
| | `reload_loop` (0.5, looping, per shell) | `Weapon_Shotgun.Reload` 0.133 |
| | `reload_end` (0.7) | `Weapon_ShotgunEngineer.Cock_Back` 0.233; `…Cock_Forward` 0.367 |
| Pyro, Shotgun | `reload_start` (0.5) | none |
| | `reload_loop` (0.5, looping) | `Weapon_Shotgun.Reload` 0.033 |
| | `reload_end` (0.967) | `…Cock_Back` 0.400; `…Cock_Forward` 0.533 |
| Demoman, Grenade Launcher | `g_reload_start` (0.6) | `Weapon_GrenadeLauncher.DrumOpen` 0.167 [the gun's `reload` body group to 1 at 0.033] |
| | `g_reload_loop` (0.667, per grenade) | `…DrumLoad` 0.200 [body group 1 at 0.033, 0 at the end] |
| | `g_reload_end` (0.633) | `…DrumClose` 0.167 [body group 0 at 0.033] |
| Demoman, Stickybomb Launcher | `sb_reload_start` (0.333) | none |
| | `sb_reload_loop` (0.667, per bomb) | `Weapon_StickyBombLauncher.BoltBack` 0.067; `…BoltForward` 0.400 |
| | `sb_reload_end` (0.533) | none |
| Heavy, Shotgun | `reload_start` (0.333) | none |
| | `reload_loop` (0.5, looping) | `Weapon_Shotgun.Reload` 0.067 |
| | `reload_end` (27 frames at 35 fps, 0.743) | `…Cock_Back` 0.286; `…Cock_Forward` 0.429 |
| Engineer, Shotgun | `fj_reload_start` (0.333) | none |
| | `fj_reload_loop` (0.5, looping) | `Weapon_Shotgun.Reload` 0.333 |
| | `fj_reload_end` (36 frames at 35 fps, 1.0) | `…Cock_Back` 0.343; `…Cock_Forward` 0.486 |
| Engineer, Pistol | `pstl_reload` (1.2333) | `Weapon_Pistol.ClipIn` 0.467 |
| Medic, Syringe Gun | `sg_reload` (1.5) | `Weapon_SyringeGun.Draw` 0.033; `…Reload1` 0.300; `…Reload2` 0.600; `…Reload3` 0.700 |
| Sniper, SMG | `smg_reload` (1.2333) | `Weapon_smg.ClipOut` 0.267; `Weapon_smg.ClipIn` 0.767 |
| Spy, Revolver | `reload` (1.333) | `Weapon_Revolver.Reload1` 0.033; `…Reload2` 0.400; `…Reload3` 0.933 |

(Sound entry names are matched without regard to case: `Weapon_smg.*`
are the `Weapon_SMG.*` entries.) Guns with no reload: the Minigun, the
Flame Thrower, the Sniper Rifle (its bolt sounds are events of its fire
sequence `fire`, 1.667 s: `Weapon_SniperRifle.BoltBack` 0.767,
`…BoltForward` 0.967, brass at 0.933) and the melee weapons.

**Answer to "are reload sounds generally animation-driven":** yes. For
every stock gun with a reload, every reload sound the shooter hears is a
view-model sound event; the code plays only the world reload sound, and
not for the shooter. Other first-person sounds come the same way: the
draw sounds (`…Draw` events near the start of each `…draw` sequence,
weapons_stock.md §8, viewmodel.md), the Minigun's `Weapon_Minigun.Draw`,
the Sniper Rifle's bolt. Shots, empty clicks and the Minigun's
loops are played by the code (sound roles), not by events.

## Per-tick order

1. Server tick: the reload starts (the reload sequence is sent, its rate
   set); the server plays the world reload sound to everyone but the
   shooter.
2. The shooter's client, predicting the same tick, starts the same
   sequence; from then, each rendered frame advances the view model and
   fires the events passed (§2).

## Edge cases

- A reload cancelled before an event's time skips that event (and the
  events after it); the world reload sound has already played.
- A one-round-at-a-time reload stopped by firing after the loop's sound
  event but before the round is in: the shooter heard the insert sound,
  no round was added (weapons.md §4).
- `r_drawviewmodel 0` still advances the view model (*inferred* from
  the client: the events belong to the view model's animation, which
  keeps running; not measured).

## Quirks

- The two Pistol reloads sound different to their owners (the Scout's
  opening slide and clip, the Engineer's single clip-in) but identical
  to everyone else.
- The Engineer's clip-in comes before the Scout's (0.467 vs 0.600 s)
  though his reload is longer.

## Test cases

| # | Setup | Action | Expected |
|---|---|---|---|
| 1 | Scout, stock Pistol, 3 rounds in the clip | reload at tick 0 | owner hears `ReloadScout` at 0.033 s (tick 2–3), `ClipIn` at 0.600 s (tick 40); others hear `WorldReload` at tick 0; full at tick 67 |
| 2 | Engineer, stock Pistol | reload at tick 0 | owner hears `ClipIn` at 0.467 s (tick 31–32) only; others `WorldReload` at tick 0; full at tick 69 |
| 3 | Scout, Pistol | reload, fire at tick 20 (a round left) | owner heard `ReloadScout` only; no `ClipIn` |
| 4 | Soldier, Shotgun, 4 of 6 | reload | owner: start 0.467 s silent, then per shell `Weapon_Shotgun.Reload` 0.133 s into each 0.5 s loop (2 shells), then the pump at 0.233 and 0.367 s into `reload_end`; others: `Weapon_Shotgun.WorldReload` at each shell's start |
| 5 | Demoman, Grenade Launcher, 1 of 4 | reload | owner: `DrumOpen` 0.167 s, one `DrumLoad` per grenade (3), `DrumClose` 0.167 s into the end |

## Open questions

1. The code allows the Pistol's owner's client to play the script's
   reload sound itself at the reload's start (a per-gun switch that the
   Pistol, the Flare Gun and the energy guns set), yet in four measured
   reloads it played none. Whether the client's reload start is not
   predicted under `cl_autoreload` (all four were automatic reloads), or
   the sound is suppressed another way, is unknown. Follow the
   measurement: the owner hears only the view-model events.
2. The one late Engineer `ClipIn` (0.18 s) was not reproduced; if it
   recurs, check the client's view-model interpolation under load.
