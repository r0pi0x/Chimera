# Source: which sounds loop, and how

Source basis: the engine's sound mixer is not public, so the rule is
worked out from the data and from behaviour: the RIFF chunks of TF2's
247 WAVs whose names contain "loop" and of the six Minigun WAVs (read in
place with our dumper; chunk layout only), CS:S's looping ambient waves
(specs/cs_source/sounds.md "Looping"), the public SDK's game code (which
never re-triggers its loops and has no "loop" key anywhere in its sound
scripts or emit calls), and a measurement in real TF2 (2026-11-20,
`sv_soundemitter_trace 1`): a Minigun's 4.0 s fire WAV started once and
was still playing, with no second start, 15.8 s later; its 0.60 s empty
WAV started once and played 10 s. Applies to every Source game (CS:S,
HL2:DM, TF2). Nothing copied.
Status: draft (data and behaviour; the mixer's own code unseen)

## Summary

A Source sound loops if, and only if, its **WAV file** says so: a `cue `
chunk (cue points) or a `smpl` chunk (sampler loops) gives the loop's
start sample; the loop runs from there to the end of the sample data and
repeats until something stops the sound. Sound scripts, channels, sound
levels, the name's prefix characters and the code that emits the sound
have no say. The game never re-triggers a loop. A player that drops the
loop chunks plays every loop once and then goes silent: a 4.0 s fire loop
is silent after 4.0 s, a 0.6 s click loop clicks once.

## Behavior

### 1. Deciding that a WAV loops

Read the RIFF chunks (any order, each padded to an even size):

1. A `cue ` chunk with at least one cue point: the WAV loops; the loop
   start is the **first** cue point's sample offset (its "sample offset"
   field, in sample frames from the start of the data). Further cue
   points are ignored (TF2 has 15 loop files with two cue points: either
   both at 0, or the second at the very last sample).
2. Else a `smpl` chunk whose loop count is at least 1: the WAV loops;
   the loop start is the first loop's start sample. (TF2 has 143 loop
   files with only a `smpl` chunk, e.g. the Phlogistinator's and the
   Rainblower's loops, which loop in the game; 18 of them start the loop
   later than sample 0, the vehicle-engine loops, so the intro before
   the loop start plays once.)
3. A `cue ` chunk with zero points or a `smpl` chunk with zero loops
   counts as absent (4 TF2 files carry both with the `smpl` empty; one
   carries only an empty `smpl`).
4. Otherwise the WAV plays once.

The loop **end** is always the end of the sample data: in every TF2 file
scanned, the `smpl` loop's end is the last sample frame (end inclusive),
so honouring it or not gives the same result; use the data end.

Decoding: the loop points are in sample frames of the decoded audio, so
they hold for 8- and 16-bit PCM, mono or stereo, and for MS-ADPCM WAVs
(one TF2 loop file is ADPCM) once decoded. MP3 files never loop.

### 2. Playing a loop

- Play from sample 0 (or from a requested start offset) to the end of
  the data, then continue at the loop start, seamlessly (no gap, no
  crossfade, no restart of the voice's volume or pitch envelope), for
  ever.
- A loop start of 0 means the whole file repeats; a later loop start
  means an intro that plays once, then the tail section repeats.
- Pitch and volume changes sent to a playing loop apply to the running
  voice; they don't restart it.

### 3. What stops a loop (and a one-shot)

- A stop request for its (entity, channel, wave name)
  (cs_source/sounds.md, channels).
- A new sound on the same entity and channel, when that channel
  replaces (any channel but auto and static; cs_source/sounds.md: "a new
  sound on the same entity and channel replaces the one playing there").
  The classic way to end a loop on the weapon channel is to play a short
  silent wave there (specs/source/fire.md: `General.StopBurning`).
- The entity going away (the sound follows the entity; its removal stops
  it), a level change, `stopsound`.
- Nothing else: a loop never ends by itself.

### 4. What does not make a sound loop

- The sound script: Source 1 scripts (TF2's, CS:S's, HL2's) have no loop
  key; `channel`, `volume`, `pitch`, `soundlevel`, `rndwave` change
  nothing about looping. (A script that names several waves picks one per
  emit; a loop keeps the wave it started with.)
- The prefix characters in a wave name (`) ^ * # @ < > } ! ?`): they set
  spatialisation and mixing, not looping.
- The emitting code: ambient_generic's "not looping" flag only changes
  its own on/off bookkeeping (cs_source/sounds.md); soundscape
  "playlooping" entries, sound patches (the envelope controller the
  Minigun, flames and engines use) and plain emits all play the wave as
  it is, and a looping wave loops in each of them.
- Repeated emits: game code that wants a sound to continue simply leaves
  it playing; it does not re-emit it when the WAV ends. (The trace shows
  a second "emit" of a sound patch's wave right after its start: that is
  a volume/pitch update to the same voice, not a new start.)

## Edge cases

- **A loop on a channel that replaces**, e.g. the Minigun's wind-up on
  the weapon channel: a new sound on that channel ends the loop or
  one-shot there, whatever its length.
- **Sound started late for a joining client**: starts from sample 0 (or
  the sound's start time if the server sends one); a loop then repeats
  from its loop start as usual.
- **Very short loops** (0.6 s Minigun spin, empty): must wrap without a
  gap; a player that restarts the file on end-of-stream instead of
  wrapping inside the mixer adds a click per cycle.

## Test cases

| # | File | Expected |
|---|---|---|
| 1 | TF2 `weapons/minigun_shoot.wav` (cue at 0, 4.0009 s) played 10 s | continuous sound for 10 s, wrapping at 4.0009 and 8.0018 s to sample 0 |
| 2 | TF2 `weapons/minigun_empty.wav` (cue at 0, 0.6023 s) played 3 s | about 5 wraps, no gap |
| 3 | TF2 `weapons/minigun_wind_down.wav` (no cue, no smpl, 3.33 s) played | ends at 3.33 s, silence after |
| 4 | a TF2 `smpl`-only loop with start 0 (e.g. `phlog_loop.wav`, 160 064 frames) | loops the whole file |
| 5 | a TF2 `smpl` loop starting later (e.g. `v8_turbo_on_loop1.wav`, loop 15 361 … 337 854 of 337 855) | plays 0 → end once, then 15 361 → end repeatedly |
| 6 | a cue loop with a later start (TF2 `rocket_pack_boosters_loop.wav`, 315 112 frames, cue at 132 566 = 3.006 s) | 0 → 7.145 s once, then 3.006 → 7.145 s repeatedly |
| 7 | any of the above, then a stop for its entity/channel/wave | silent at once |
| 8 | CS:S `ambient/fire/fire_med_loop1.wav` (cue loop, 4.55 s) via ambient_generic with "not looping" set | still loops (the flag doesn't matter) |

## Open questions

1. The precedence when a file has both a non-empty `cue ` and a
   non-empty `smpl` loop with different starts: none in TF2's loop files
   (the four with both have the `smpl` empty). Use the cue first.
2. Whether a `smpl` loop end earlier than the data end would be honoured
   (none in TF2's data). Use the data end.
3. Scan the whole of TF2's, CS:S's and HL2's `sound/` for loop chunks in
   files whose names don't say "loop" (the Minigun's do not), so no
   loop goes unnoticed: the rule is by chunk, never by name.
