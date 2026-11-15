# Halo: Combat Evolved: sounds

Source basis: the game's own map files from the original Xbox disc (all
24 `.map` files: sound tags, looping sound tags, sound environments and
sample data, read with throwaway scripts; the bulk measurements are on
`bloodgulch.map`, with `ui.map`, `a10.map` and `a30.map` for music,
dialogue and long sounds) and the game's executable on the same disc (the
sound class table); and a decompilation of an Xbox build of the game's
engine (the sound manager, the sound class and sound definition code, the
game-side sound code, and the console audio layer), used for meaning
only. The decompilation describes a different Xbox build from the disc's
(see cache_file.md); where it differs from the disc, the disc wins and
the difference is noted. Nothing was listened to: the decoder was
checked numerically (see "What was checked").
Status: draft

Sections: Summary; Units and conventions; Constants; The sound tag
(header, enumerations, class table, pitch ranges, permutations, long
sounds, sample data); Xbox ADPCM; Playback rules (a play request,
admission, pitch and range, permutation choice, gain, distance, cone and
Doppler and delay, obstruction and reverb, channels and limits, ducking,
per-frame order); Looping sounds; Sound environment, background sound,
sound scenery; Measured facts; What was checked; Edge cases; Quirks; Test
cases; Confidence per section; Open questions.

---

## Summary

- A sound is a `snd!` tag: a header of playback settings, then one or
  more *pitch ranges*, each holding *permutations*. A permutation is one
  recording. Its bytes are not in the tag data: they sit in the map's
  sound region, at an absolute file offset, as **Xbox ADPCM**: 4-bit
  codes in 36-byte blocks of 64 samples (mono) or 72-byte blocks of 64
  sample frames (stereo). Every sound on every disc map is Xbox ADPCM,
  mono at 22 050 Hz, or stereo at 22 050 or 44 100 Hz. Nothing is
  uncompressed, nothing is Ogg.
- Playing a sound picks a pitch, the pitch range that covers it, and one
  permutation in that range at random (a shuffled deck that never repeats
  the same one twice in a row). The sound's loudness is the product of
  the permutation's gain, the tag's gain modifier, the class gain and
  fades. Mono sounds are positional: they fall off as `min / distance`
  (6 dB per doubling) past their minimum distance and are cut off at the
  maximum distance. Stereo sounds are never panned; they fall off
  linearly to nothing at the maximum distance.
- Sounds with no distances of their own (about four in five) take them
  from their *sound class*: weapon fire is full volume out to 4 world
  units and inaudible past 70, footsteps 0.9 and 10, dialogue 3 and 20.
  The class table is in the game's executable and is given in full
  below.
- Long sounds (music, ambience) are cut into 65 520-byte chunks chained
  by "next permutation" links, and are played by *looping sounds*
  (`lsnd`): a start sound, a loop that re-picks a chain each time round,
  an end sound, cross-fades, and random "detail" one-shots.

## Units and conventions

Everything in cache_file.md applies: little-endian, tag blocks, tag
references, data references (12, 16 and 20 bytes), class codes, the
world unit (WU) of 3.048 m, radians, the 30 Hz tick.

- **Distance**: world units. A mono sound's distance is from the
  listener's camera position to the sound's position.
- **Gain**: a linear amplitude factor, 0 to 1 (values above 1 in a tag
  are clamped to 1 where the final gain is applied). The console's
  volume control takes hundredths of a decibel: `v = trunc(2000 *
  log10(g))`, with `g = 0` meaning the floor (-10 000, silence). So a
  gain of 0.5 is -602 and 0.1 is -2000. (The 3D attenuation below is
  applied by the hardware in the same domain.)
- **Pitch**: a frequency ratio. 1.0 plays at the sound's sample rate; 2.0
  an octave up. The hardware's playback frequency is `rate * pitch`,
  limited to the hardware's range (100 to 200 000 Hz; inferred).
- **Time**: the sound manager runs once per rendered frame on real
  milliseconds, not on ticks. Fade times, the dialogue duck and
  preemption times are real time. Only the per-class gain fades
  (`set class gain` from scripts) count whole ticks (30 per second), and
  mouth data is indexed in ticks (30 bytes per second of speech).
- **Output**: a decoded sound is a clip of 16-bit signed samples with a
  sample rate (22 050 or 44 100) and 1 or 2 channels. Stereo is stored as
  two separate channels per block and interleaved L, R on output.
- Angles are radians; all cone angles on the disc are 2 pi.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| SOUND_TAG_SIZE | 0xA4 (164) | bytes | the `snd!` main struct |
| PITCH_RANGE_SIZE | 0x48 (72) | bytes | one pitch range element |
| PERMUTATION_SIZE | 0x7C (124) | bytes | one permutation element |
| ADPCM_MONO_BLOCK | 36 | bytes | header 4 + 32 data bytes |
| ADPCM_STEREO_BLOCK | 72 | bytes | two headers 8 + 64 data bytes |
| ADPCM_BLOCK_SAMPLES | 64 | samples per channel | 1 header sample + 63 coded |
| STEP_TABLE_LEN | 89 | entries | indices 0..88 |
| RATE_0, RATE_1 | 22 050, 44 100 | Hz | the two sample rates |
| MAX_PITCH_RANGES | 8 | per sound | tool limit; disc max 4 |
| MAX_PERMUTATIONS_PER_RANGE | 256 | | tool limit; disc max 179 (a30) |
| MAX_PERMUTATION_BYTES | 0x400000 | bytes | tool limit; disc max 269 676 |
| CHUNK_BYTES | 65 520 (0xFFF0) | bytes | the size a long sound is cut into; = 1820 mono or 910 stereo blocks |
| MOUTH_RATE | 30 | bytes per second | mouth data resolution (one per tick) |
| MAX_MOUTH_BYTES | 8192 | bytes | tool limit |
| MAX_SUBTITLE_BYTES | 512 | bytes | tool limit; none on the disc |
| SOUND_CLASS_COUNT | 51 | classes | indices 0..50 |
| MONO_3D_CHANNELS | 51 (46 hardware streams) | voices | positional mono voices |
| MONO_2D_CHANNELS | 10 (9) | voices | unspatialized mono |
| STEREO_22K_CHANNELS | 10 (9) | voices | stereo at 22 050 Hz |
| STEREO_44K_CHANNELS | 10 (9) | voices | stereo at 44 100 Hz |
| DISTANCE_FACTOR | 3.048 | m per WU | the 3D audio world scale |
| ROLLOFF_FACTOR | 1.0 | | inverse-distance rolloff (6 dB per doubling) |
| MS_PER_WU | 8.9647064 | ms | `3.048 m / 340 m/s`: sound travel time per WU |
| DELAY_THRESHOLD | 250 | ms | travel time above which a sound is delayed (about 27.9 WU) |
| INAUDIBLE_FADE_OUT | 2.0 | s | linear fade when a sound goes beyond its maximum distance |
| INAUDIBLE_FADE_IN | 0.5 | s | linear fade when it comes back |
| PLAYER_DEAD_FADE | 0.3 | s | fade of scripted player dialogue when all players are dead |
| PITCH_RANGE_CROSSFADE | 0.5 | s | cross-fade between pitch ranges of a loop |
| CROSSFADE_EXPONENT | 2.5 | | shapes that cross-fade |
| DUCK_GAIN | 0.7 | | non-dialogue gain while scripted dialogue plays |
| DUCK_ATTACK | 0.03 | per tick | rate the duck gain falls |
| DUCK_RELEASE | 0.007 | per tick | rate it recovers |
| CLUSTER_SOUND_DISTANCE_MAX | 256 | WU | scale of the cluster sound distance table |
| UNDERWATER_DIRECT_GAIN | 0.25 | | direct-path gain of every sound while the listener is underwater |
| OBSTRUCTION_DEFAULT | 0.6 | | obstruction when the clusters are not connected |
| OBSTRUCTION_PVS | 0.45 | | obstruction when they are connected but a ray hits something |
| OCCLUSION_BOOST | 1.4 | | multiplier in the occlusion formula |

The channel counts are the engine's defaults for the console's audio
hardware; a software mixer has no such limit but should keep the
per-class limits below, which are what shape the sound.

## The sound tag (`snd!`)

### Header (0xA4 bytes)

| Offset | Size | Type | Name | Unit / meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | bit 0 "fit to ADPCM block size" (the build tool padded every permutation to whole blocks; a no-op for us); bit 1 "split long sound into permutations" (the permutations are chunks of long sounds linked by next-permutation indices; see Long sounds) |
| 0x04 | 2 | enum16 | class | sound class, 0..50 (table below) |
| 0x06 | 2 | enum16 | sample rate | 0 = 22 050 Hz, 1 = 44 100 Hz |
| 0x08 | 4 | f32 | minimum distance | WU; 0 = take the class's |
| 0x0C | 4 | f32 | maximum distance | WU; 0 = take the class's |
| 0x10 | 4 | fraction | skip fraction | chance a play request is dropped (0 on every sound on the disc); see Admission |
| 0x14 | 8 | real_bounds | random pitch bounds | lower, upper; the pitch of a play is drawn uniformly from this. (1, 1) on 310 of 315 in bloodgulch; others (0.8, 1.2), (0.4, 0.6), (0.9, 1.1) |
| 0x1C | 4 | angle | inner cone angle | radians, 2 pi on every sound on the disc |
| 0x20 | 4 | angle | outer cone angle | radians, 2 pi on every sound on the disc |
| 0x24 | 4 | f32 | outer cone gain | gain outside the outer cone; 1.0 on every sound on the disc |
| 0x28 | 4 | f32 | gain modifier | multiplies the permutation's gain; typically 0.5 (weapons, footsteps), 0.85 (dialogue), 1.0 |
| 0x2C | 4 | f32 | maximum bend per second | a ratio limiting how fast a looping sound's pitch may change; 0 = no limit (1.003 on one sound) |
| 0x30 | 12 | | unused | zero |
| 0x3C | 4 | f32 | when scale is zero: skip fraction modifier | a multiplier, 1.0 usual |
| 0x40 | 4 | f32 | when scale is zero: gain modifier | 1.0, or 0.0 on 134 of 315 (impact sounds that fade in with the impact's strength) |
| 0x44 | 4 | f32 | when scale is zero: pitch modifier | 1.0 usual (0.7 on two) |
| 0x48 | 12 | | unused | zero |
| 0x54 | 4 | f32 | when scale is one: skip fraction modifier | 1.0 |
| 0x58 | 4 | f32 | when scale is one: gain modifier | 1.0 |
| 0x5C | 4 | f32 | when scale is one: pitch modifier | 1.0 usual; 1.4, 1.6, 2.0, 10.0 on a few |
| 0x60 | 12 | | unused | zero |
| 0x6C | 2 | enum16 | encoding | 0 = mono, 1 = stereo |
| 0x6E | 2 | enum16 | compression | 0 = none (16-bit PCM; never on the disc), 1 = Xbox ADPCM (every sound on the disc), 2 = IMA ADPCM (the PC format; never), 3 = Ogg Vorbis (the PC format; never) |
| 0x70 | 16 | tag ref | promotion sound | class `snd!`, or none |
| 0x80 | 2 | i16 | promotion count | 0 = no promotion; see Admission |
| 0x82 | 2 | | unknown | 0 |
| 0x84 | 4 | i32 | longest permutation length | milliseconds, an estimate: see Quirks (it is 1.125 times the true duration of the longest permutation) |
| 0x88 | 4 | i32 | runtime | promotion counter; 0 |
| 0x8C | 4 | i32 | runtime | promotion time; 0 |
| 0x90 | 4 | u32 | runtime | scripting time; 0xFFFFFFFF |
| 0x94 | 4 | i32 | runtime | scripting sound index; -1 |
| 0x98 | 12 | block | pitch ranges | element 0x48 |

The "scale" is a 0..1 number the caller supplies with every play
request (how hard an impact was, how fast an engine turns). The two
modifier triples are the tag's response to it: for each of skip
fraction, gain and pitch, the effective multiplier is
`lerp(zero modifier, one modifier, scale)`, with
`lerp(a, b, s) = (b - a) * s + a`. Disc: 172 of 315 bloodgulch sounds
have 1.0 for all six (scale does nothing); 134 have gain-at-zero 0.0
(silent for a feeble impact, full for a hard one); a few have other
values.

Distances resolve independently: a zero minimum takes the class
minimum, a zero maximum takes the class maximum.

### Sound class (the number at 0x04)

The class names are the 33 the game defines; 51 slots exist and the
unnamed ones carry no defaults (all zero; a sound with such a class is
damage). Classes seen on the disc, over all 24 maps: 0, 1, 4 to 10, 13,
14, 15, 18, 19, 22, 23, 26, 27, 28, 30, 32, 33, 34, 39, 44, 45, 47, 50.

The table is the game's own, stored in its executable as 51 records of
0x2C bytes (checked against the disc): per-definition and per-object
instance limits (4 and 1 unless shown), preemption time (ms), speech,
priority (higher wins a channel), cache-miss mode (0 discard, 1
postpone; see Channels), wet gain (reverb attenuation, see
Obstruction), and the default minimum and maximum distances.

| # | Name | Per def. | Per obj. | Preempt ms | Speech | Priority | Cache miss | Wet gain | Min WU | Max WU |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | projectile impact | 6 | 4 | 100 | no | 4 | 0 | 0.5 | 1.4 | 8 |
| 1 | projectile detonation | 4 | 1 | 200 | no | 5 | 1 | 0.5 | 8 | 120 |
| 4 | weapon fire | 4 | 1 | 0 | no | 4 | 1 | 0.5 | 4 | 70 |
| 5 | weapon ready | 4 | 1 | 500 | no | 4 | 1 | 0.5 | 1 | 9 |
| 6 | weapon reload | 4 | 1 | 500 | no | 4 | 1 | 0.5 | 1 | 9 |
| 7 | weapon empty | 4 | 1 | 60 | no | 4 | 1 | 0.5 | 1 | 9 |
| 8 | weapon charge | 4 | 1 | 500 | no | 4 | 1 | 0.5 | 1 | 9 |
| 9 | weapon overheat | 4 | 1 | 500 | no | 4 | 1 | 0.5 | 1 | 9 |
| 10 | weapon idle | 4 | 1 | 500 | no | 4 | 1 | 0.5 | 1 | 9 |
| 13 | object impacts | 4 | 1 | 100 | no | 3 | 1 | 0.5 | 0.5 | 3 |
| 14 | particle impacts | 4 | 1 | 100 | no | 3 | 0 | 0.5 | 0.5 | 3 |
| 15 | slow particle impacts | 4 | 1 | 1000 | no | 3 | 0 | 0.5 | 0.5 | 3 |
| 18 | unit footsteps | 4 | 1 | 200 | no | 3 | 0 | 0.5 | 0.9 | 10 |
| 19 | unit dialog | 4 | 1 | 100 | yes | 3 | 1 | 0.8 | 3 | 20 |
| 22 | vehicle collision | 4 | 2 | 400 | no | 3 | 0 | 0.5 | 1.4 | 8 |
| 23 | vehicle engine | 4 | 2 | 100 | no | 3 | 1 | 0.9 | 1.4 | 8 |
| 26 | device door | 4 | 1 | 100 | no | 2 | 1 | 0.5 | 0.9 | 5 |
| 27 | device force field | 4 | 1 | 100 | no | 2 | 1 | 0.5 | 0.9 | 5 |
| 28 | device machinery | 4 | 1 | 100 | no | 1 | 1 | 0.5 | 0.9 | 5 |
| 29 | device nature | 4 | 1 | 100 | no | 1 | 1 | 0.5 | 0.9 | 5 |
| 30 | device computers | 4 | 1 | 100 | no | 1 | 1 | 0.5 | 0.5 | 3 |
| 32 | music | 4 | 4 | 100 | no | 2 | 1 | 1.0 | 0.9 | 5 |
| 33 | ambient nature | 4 | 1 | 100 | no | 1 | 1 | 1.0 | 0.9 | 5 |
| 34 | ambient machinery | 4 | 1 | 100 | no | 1 | 1 | 1.0 | 0.9 | 5 |
| 35 | ambient computers | 4 | 1 | 100 | no | 1 | 1 | 1.0 | 0.5 | 3 |
| 39 | first person damage | 4 | 1 | 100 | no | 4 | 1 | 1.0 | 0.5 | 3 |
| 44 | scripted dialog player | 4 | 4 | 100 | yes | 6 | 1 | 0.8 | 3 | 20 |
| 45 | scripted effect | 4 | 4 | 100 | no | 3 | 1 | 0.8 | 2 | 5 |
| 46 | scripted dialog other | 4 | 4 | 100 | yes | 5 | 1 | 0.8 | 3 | 20 |
| 47 | scripted dialog force unspatialized | 4 | 4 | 100 | yes | 6 | 1 | 0.8 | 3 | 20 |
| 50 | game event | 4 | 1 | 100 | no | 5 | 1 | 1.0 | 3 | 20 |

Slots 2, 3, 11, 12, 16, 17, 20, 21, 24, 25, 31, 36, 37, 38, 40 to 43,
48 and 49 are unnamed and all zero. Two more per-class values exist in
the record (a gain and a scale, 0 or 1 and always 1) and nothing reads
them. Every class starts a map with gain 1.0, enabled; scripts can set a
class's gain (0..1, interpolated over a number of ticks) and the
developer console can disable a class.

### Pitch range (0x48 bytes)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | string | name | `default` on 306 of 322 ranges; engine sounds name them (`idle`, `low`, `med`, `high`) |
| 0x20 | 4 | f32 | natural pitch | the pitch at which the recordings play unchanged (1.0 on 306; 1.1 to 2.3 on engine ranges) |
| 0x24 | 8 | real_bounds | bend bounds | the pitches this range is used for; (0.0, 1.0) on the 306 single-range sounds |
| 0x2C | 2 | i16 | actual permutation count | how many of the range's permutations a play may pick from: the number of *chain heads* (see Long sounds); equals the element count except on split sounds |
| 0x2E | 2 | | unused | 0 |
| 0x30 | 4 | f32 | playback rate | **stored on the disc as `1 / natural pitch`** (0.5714286 for natural 1.75) |
| 0x34 | 4 | u32 | runtime | played-permutation mask; 0xFFFFFFFF on disc, start it at 0 |
| 0x38 | 2 | i16 | runtime | previous permutation; -1 |
| 0x3A | 2 | i16 | runtime | forced permutation; -1 |
| 0x3C | 12 | block | permutations | element 0x7C |

Hardware pitch of a played sound = `pitch * playback rate`, so a sound
at its natural pitch plays at exactly the sample rate. Of 322 ranges in
bloodgulch, 321 have actual count = element count; the exception is
split (the ambience `countryday2`: 4 of 21).

### Permutation (0x7C bytes)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | string | name | the recording's source name |
| 0x20 | 4 | fraction | skip fraction | chance this permutation is passed over when picked (see Permutation choice); 0 on 903 of 916, 0.99 on 9, 0.6 on 3, 0.5 on 1 |
| 0x24 | 4 | f32 | gain | 1.0 on 890 of 916; 0.5, 0.6, 0.2, 0.8, 0.4 elsewhere |
| 0x28 | 2 | enum16 | compression | same enumeration as the header; equals the header's on every permutation (916 of 916) |
| 0x2A | 2 | i16 | next permutation index | -1 = none; else an index in the same range: the next chunk of a split sound |
| 0x2C | 4 | | runtime | cache block, -1 on disc |
| 0x30 | 4 | | runtime | 0 |
| 0x34 | 4 | tag id | runtime | the sound tag's own id |
| 0x38 | 4 | | runtime | 0 |
| 0x3C | 4 | tag id | runtime | the sound tag's own id |
| 0x40 | 20 | data ref | samples | size in bytes; address 0; **file offset = absolute offset of the sample bytes in the decompressed image** (see cache_file.md); 512-byte aligned on all 916 bloodgulch permutations; they follow each other in tag order, never overlapping |
| 0x54 | 20 | data ref | mouth data | in tag data (address nonzero), or size 0 |
| 0x68 | 20 | data ref | subtitle data | size 0 everywhere on the disc (a10 and bloodgulch checked) |

The runtime words are build-tool leftovers or console scratch; ignore
them. The samples data reference's size is always a whole number of
ADPCM blocks (36 for mono, 72 for stereo) on all 24 maps, including
sounds without the "fit" flag.

**Mouth data** is one byte per tick (30 per second) of mouth openness,
0..255 meaning 0..1, present on dialogue only (bloodgulch: 23 unit
dialogue and 36 scripted-effect permutations; a10 4465 unit dialogue).
Its length is the duration in seconds times 30, rounded up. Lip-sync
reads it by the elapsed time of the playing permutation (ticks, scaled
by pitch), clamped to the last byte, and a permutation of a speech class
without it gives 0.

### Long sounds: chains of permutations

A recording longer than 65 520 bytes of ADPCM (910 stereo blocks or
1820 mono blocks: 58 240 frames, 1.32 s at 44.1 kHz) is cut into chunks
of exactly 65 520 bytes plus a shorter remainder, each chunk its own
permutation, and the header's "split long sound" flag is set. Each
chunk's *next permutation index* names the following chunk; the last
chunk has -1. Verified on the disc: the permutation list of a split
sound is the **heads first** (indices 0 to H-1, where H is the range's
*actual permutation count*, one per recording), then all the later
chunks; following next-links from the H heads visits every permutation
exactly once. Example, bloodgulch `sound\sfx\ambience\multiplayer\
countryday2` (stereo 44 100 Hz): H = 4, 21 permutations, chains
0-4-5-6-7, 1-8-9-10, 2-11-12-13-14-15, 3-16-17-18-19-20; chunk sizes
65 520 except 40 176, 288, 10 512 and 2880 at chain ends. Chunks begin
with a fresh block header, so a chain is decoded chunk by chunk and the
samples concatenated; the seams are continuous (checked on the music
`title1\loops`: mean jump at a seam 639 against 618 inside chunks).

Which sounds are split: music (class 32), music-like scripted loops and
long ambience, all with flags 3 (or 2) on the disc: 1 of 315 in
bloodgulch, about 80 in a30. Only looping sounds follow the chain (see
Looping sounds). A plain play request (impulse) plays only the head it
picked.

### Where the bytes are

The sample region of the decompressed image (cache_file.md): bloodgulch
0x175AA00 to 0x233B540, 12 454 720 bytes holding 12 229 380 bytes of
samples and 225 340 bytes of alignment gaps (0 to 504 bytes between
permutations). A reader takes `size` bytes at `file offset` and checks
`offset + size` against the image size.

## Xbox ADPCM

This is IMA ADPCM in blocks of 64 samples, laid out as Microsoft's
console variant: a per-channel header and codes in the same block. Every
block is independent (the decoder state is set from the block's own
header), so a block can be decoded alone.

### Mono block (36 bytes)

| Bytes | Content |
|---|---|
| 0..1 | predictor: i16, little-endian: the block's first output sample |
| 2 | step index: u8, 0..88 (a reader clamps larger values to 88) |
| 3 | reserved: 0 (all 292 954 blocks of bloodgulch) |
| 4..35 | 32 data bytes = 64 nibbles |

The nibbles are numbered 0 to 63: nibble `2k` is the **low** nibble of
data byte `k`, nibble `2k + 1` its **high** nibble. Nibbles 0 to 62 are
codes; **nibble 63 (the high nibble of byte 35) is padding, always
0 on the disc, and is never decoded**. The block therefore holds 64
samples: sample 0 is the header's predictor, samples 1 to 63 are the
decoded codes 0 to 62.

### Stereo block (72 bytes)

| Bytes | Content |
|---|---|
| 0..3 | left header (predictor i16, step index u8, reserved 0) |
| 4..7 | right header |
| 8..71 | 8 groups of 8 bytes; group g (g = 0..7) is at bytes `8 + 8g`: 4 bytes of left data (bytes `4g` to `4g + 3` of the left channel's 32 data bytes), then 4 bytes of right data (the same for the right) |

Each group of 4 bytes is 8 nibbles, low nibble first, so group g
covers nibbles `8g` to `8g + 7` of its channel, i.e. samples `8g + 1`
to `8g + 8`. Each channel is then decoded exactly like a mono block (so
nibble 63 of each channel, the high nibble of the last byte of group
7, is padding). The two channels never mix; the output frame i is the
pair (left i, right i).

### The decoder

Per channel, state `pred` (i32, kept in -32768..32767) and `idx`
(0..88). Initialise `pred` and `idx` from the block header. Output
`pred` as sample 0. For each code `c` (0..15) of nibbles 0 to 62:

1. `step = STEP[idx]`
2. `diff = (step >> 3)`, plus `step` if bit 2 of `c` is set, plus
   `(step >> 1)` if bit 1 is set, plus `(step >> 2)` if bit 0 is set
   (integer shifts; bit 3 of `c` is the sign and does not enter here).
3. If bit 3 of `c` is set `pred = pred - diff`, else `pred = pred +
   diff`; then clamp `pred` to -32768..32767.
4. `idx = clamp(idx + ADJ[c], 0, 88)`.
5. Output `pred`.

The index adjustment table (16 entries; `c` and `c + 8` are equal):

| c | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ADJ | -1 | -1 | -1 | -1 | 2 | 4 | 6 | 8 | -1 | -1 | -1 | -1 | 2 | 4 | 6 | 8 |

The step size table. These are the published IMA ADPCM constants (a
fixed mathematical table, 89 entries, indices 0 to 88; the game uses it
unchanged, and it also appears in the game's executable):

| idx | +0 | +1 | +2 | +3 | +4 | +5 | +6 | +7 | +8 | +9 |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 16 | 17 |
| 10 | 19 | 21 | 23 | 25 | 28 | 31 | 34 | 37 | 41 | 45 |
| 20 | 50 | 55 | 60 | 66 | 73 | 80 | 88 | 97 | 107 | 118 |
| 30 | 130 | 143 | 157 | 173 | 190 | 209 | 230 | 253 | 279 | 307 |
| 40 | 337 | 371 | 408 | 449 | 494 | 544 | 598 | 658 | 724 | 796 |
| 50 | 876 | 963 | 1060 | 1166 | 1282 | 1411 | 1552 | 1707 | 1878 | 2066 |
| 60 | 2272 | 2499 | 2749 | 3024 | 3327 | 3660 | 4026 | 4428 | 4871 | 5358 |
| 70 | 5894 | 6484 | 7132 | 7845 | 8630 | 9493 | 10442 | 11487 | 12635 | 13899 |
| 80 | 15289 | 16818 | 18500 | 20350 | 22385 | 24623 | 27086 | 29794 | 32767 | |

(Row 80 has 9 entries: 80 to 88.)

### Sample counts

- Mono: `samples = floor(bytes / 36) * 64`. Stereo: `frames =
  floor(bytes / 72) * 64`. Duration = samples (frames) / sample rate.
- A trailing partial block does not occur on the disc (every size is a
  multiple of the block size, checked on all 24 maps). A reader must
  still not read past the data: it decodes whole blocks only and ignores
  a remainder.
- The tail of a recording is silence: the last samples of the assault
  rifle's fire are exactly 0. The padding to whole blocks is therefore
  part of the clip; there is no separate "valid sample count" in the
  tag, and none is needed.
- A chain (long sound) is decoded chunk by chunk and concatenated.

### Output rules

A decoded clip is 16-bit PCM: one array of samples for mono, or
interleaved L, R for stereo, plus the sample rate from the header
(22 050 or 44 100). Nothing is resampled at load: the hardware plays at
`rate * pitch * playback rate` and a software mixer resamples at play.

## Playback rules

### A play request

Gameplay asks for a sound with: the sound tag; a *source*, which is one
of: unspatialized (no position; plays as is), absolute (a world
position, a forward vector for the cone, a velocity, and the BSP
cluster the position is in, for obstruction) or relative (a position
measured from the listener); a *source gain* (1.0 for all of the game's
own requests); a *scale* 0..1 (impact strength, engine revolutions, or
1.0); and optionally an owner (the object, for the per-object limit and
for following a moving object). Effects play their sound parts as
absolute requests at the effect's location, weapons and footsteps through
effects and animations (see effects.md and animations.md). Scripts can
play unspatialized and force-unspatialized sounds. The nearest of up to
four local players' cameras is the listener.

### Admission (the order of tests)

A request becomes a sound only if all of these pass; otherwise it is
dropped silently (no error, no replacement):

1. The sound manager is on, and the sound is one the mixer can play: Xbox
   ADPCM, and mono at 22 050 Hz or stereo at either rate. (Anything else
   is refused; none exists on the disc.)
2. Its class is not disabled, and its pitch range 0 has at least one
   permutation. (Only range 0 is tested.)
3. **Scale test**: if `scale = 0` and the "gain when scale is zero"
   modifier is 0, it is dropped. So a feeble impact makes no sound.
4. **Skip test**: draw `r` uniformly in [0, 1). It is dropped unless
   `r > lerp(zero skip modifier, one skip modifier, scale) * skip
   fraction` (header values). With skip fraction 0 (every sound on the
   disc) it plays except for r exactly 0.
5. **In range**: the nearest valid listener is within the sound's
   maximum distance (squared compare), and the source's occlusion is
   not 1. Unspatialized sources are always in range.
6. **Promotion** (sounds with promotion count `N > 0` and longest
   length `Lms`): a per-sound counter `C` (starts 0) decays by the
   milliseconds since the previous request (floored at 0), then gains
   `Lms`. If `C > N * Lms`: with a promotion sound set, `C` resets to 0
   and the request plays that *promotion sound* instead (which goes
   through this whole list); with none, `C` loses `Lms` again and the
   request is dropped. Used on the disc by the particle-impact class:
   rapid rock hits (smallrock -> medrocks -> lrgrocks -> smallrock, N =
   5), metal chips, shell casings (single -> double -> multi, N = 4)
   and glass (small -> medium -> large, N = 2, 3, 1). Glass large and the
   shotgun shell sounds have N = 1 and no promotion sound: a second
   request inside the sound's own length is dropped.
7. A permutation is picked and a voice is found (Channels).

If the listener is beyond about 27.9 WU (travel time over 250 ms at
8.9647 ms per WU), the sound is created *delayed*: it starts
`distance * 8.9647` ms later (the speed of sound, 340 m/s), so distant
gunfire arrives after its flash.

### Pitch and pitch range

1. `pitch = lerp(zero pitch modifier, one pitch modifier, scale) *
   uniform(pitch lower, pitch upper)`. (For looping sounds the draw is
   made once per voice and the modifier pair is re-applied every frame
   with the current scale.)
2. The range: first a range already chosen (the voice's) is kept if
   `bend lower <= pitch <= bend upper` and it has permutations.
   Otherwise scan the ranges in file order, skipping ranges with no
   permutations: the first whose bend bounds contain the pitch wins.
   If none contains it, the winner is the range with the smallest ratio
   `pitch / bend upper` (when the pitch is above the range) or `bend
   lower / pitch` (when below); ties keep the earlier. If no range was
   chosen by the scan the first range with permutations is used.
3. The hardware frequency is `rate * pitch * playback rate`, where
   playback rate is the range's stored `1 / natural pitch`.
4. For looping voices, each frame the new pitch is limited toward the
   previous hardware pitch times natural pitch: if `maximum bend per
   second` is nonzero, the pitch may rise to at most `old * bend` and
   fall to at most `old / bend` per update (an update is a rendered
   frame, not a second: see Quirks). If the new pitch falls in another
   range a second voice in that range starts and the two cross-fade over
   0.5 s (Looping sounds).

### Permutation choice

Per pitch range, with `n` = actual permutation count, a played mask
(starts 0), and a previous index (starts -1). The disc's stored mask
0xFFFFFFFF behaves like 0 (the first call sees every bit set and
clears it).

1. Draw `sel` uniformly from 0 to n-1 (integer).
2. Repeat: if every one of the n low bits of the mask is set, clear the
   mask, and if `n > 1` and the previous index is valid, set its bit.
   If bit `sel` is not set: set it; count one attempt; if this was
   the 17th attempt accept `sel`; otherwise draw `r` uniformly in [0, 1)
   and accept `sel` if `r >= skip fraction of permutation sel`. If `sel`
   was not accepted (already played, or skipped), `sel = sel + 1`,
   wrapping from n-1 to 0, and repeat.
3. Remember `sel` as the previous index; the result is `sel`.

So the picks are a random shuffle with no repeats until all have played,
and the first pick after a reset is never the one just played. A skipped
permutation stays marked as played until the next reset: a permutation
with a high skip fraction mostly drops out of the deck. For sounds with
the split flag the same procedure runs over the heads only (`n` =
actual count); later chunks are reached by the next index. Forced
permutation (a cache-miss recovery) does not exist for us.

### Gain

For an impulse (a one-shot) the hardware gain is

`G = clamp(pg * gm * lerp(zg, og, scale) * fade * sg * cg * duck, 0, 1)`

where `pg` is the permutation's gain, `gm` the header's gain modifier,
`zg` and `og` the two "gain when scale is zero / one" values, `fade` the
fade factor (1 normally; see Distance and Channels), `sg` the source
gain (1), `cg` the class's current gain (1 unless a script changed it;
no static per-class gain table exists), and `duck` 1.0 except for the
dialogue duck below. For a looping voice the factor `track gain` of the
track (see Looping sounds) also multiplies. The hardware volume is then
`trunc(2000 * log10(G))` hundredths of a decibel.

Example: the assault rifle's fire: `pg = 1`, `gm = 0.5`, scale 1 with
both modifiers 1: `G = 0.5` within the minimum distance (4 WU), 0.25 at
8 WU, as the mono 3D attenuation below multiplies it.

**Dialogue duck.** While any voice of class 44, 46 or 47 (scripted
dialogue) exists, `duck` for every other class moves toward
`DUCK_GAIN = 0.7` by 0.03 per elapsed tick (about a third of a second),
and when none exist back toward 1.0 by 0.007 per elapsed tick (about
1.4 s). Dialogue classes are never ducked.

### Distance

Let `m` and `M` be the resolved minimum and maximum distance and `d`
the listener distance in WU (3D length, camera to source).

- **Mono sounds (positional voices)** are attenuated by the 3D audio
  hardware with an inverse-distance model: `a(d) = 1` for `d <= m`, and
  `a(d) = m / d` for `d > m` (rolloff 1: 6 dB per doubling, no
  cut-off, no floor; with the hardware's own max distance set to
  infinity). The maximum distance is **not** where the sound reaches
  zero: it is an *audibility cutoff*. A sound whose distance exceeds `M`
  (or whose occlusion is 1) is *inaudible*: its fade goes linearly to 0
  over 2.0 s (an impulse then stops and frees its voice; a looping
  voice keeps running silently); coming back in range fades it up over
  0.5 s. A request starting beyond `M` is not created at all.
- **Stereo sounds** are never positional: they are not panned, filtered
  or reverbed in the original. If their source is absolute or relative
  they take a linear fade `a(d) = clamp(1 - (d - m) / (M - m), 0, 1)`
  multiplied into `fade`; an unspatialized stereo sound has no
  attenuation. (The game's announcer and music requests are
  unspatialized or have large distances.)
- **Unspatialized mono** sounds (a 2D mono voice) have no attenuation.

Worked values for `m = 4`, `M = 70` (weapon fire's class defaults):

| d (WU) | 0 to 4 | 5 | 8 | 16 | 40 | 70 | 71 |
|---|---|---|---|---|---|---|---|
| mono gain | 1 | 0.8 | 0.5 | 0.25 | 0.1 | 0.0571429 | inaudible (fade out in 2 s) |
| stereo gain | 1 | 0.98485 | 0.93939 | 0.81818 | 0.45455 | 0 | 0 |

### Cone, Doppler, delay

- **Cone**: the sound's forward vector, inner and outer angle (the
  hardware takes them as whole degrees: `trunc(angle * 57.29578)`) and
  outer gain. A listener inside the inner cone hears full gain; beyond
  the outer cone, `outer gain`; between, the hardware interpolates
  (assumed linear in decibels; not checked). **Every sound on the disc
  has inner = outer = 2 pi and outer gain 1.0, so cones are off**; an
  implementation may skip them.
- **Doppler**: positional mono sounds are given their velocity (the
  tick velocity * 30, relative to the listener's own velocity) and the
  3D hardware applies a first-order Doppler shift (speed of sound
  343 m/s, factor 1, at 3.048 m per WU), changing the playback
  frequency; the shift is limited to half to double. Reference only for
  the formula; the engine sets only the velocities.
- **Delay**: see Admission; the sound starts after the travel time when
  over 250 ms.

### Obstruction, occlusion, reverb

For an absolute source in range, each frame the game computes two
numbers (0..1) per source: *obstruction* and *occlusion*:

1. Default obstruction 0.6, occlusion 1.0.
2. If the source and the camera are each in a cluster: take the
   cluster-to-cluster sound distance table entry (structure_bsp.md,
   "sound cluster data": low 7 bits, 0 to 127 mapping to 0 to 256 WU,
   top bit "unreachable"): `cd = value * 256 / 127`. If `cd < 256`:
   - If the source's cluster is in the camera's cluster's visibility
     set, obstruction becomes 0.45 and a ray from the camera to the
     source is traced against the structure, scenery, machines and
     other objects (front faces, media). If the ray is clear both become
     0.
   - If obstruction is not 0, `occlusion = clamp(1.4 * (1 - d / (cd +
     d)), 0, 1)`.
3. A source whose occlusion is 1 (clusters too far apart, or no
   connection) is inaudible.

The hardware turns them into high-frequency losses of the direct path:
obstruction `20 * log10(1 - obstruction)` dB on the high band and
occlusion `20 * log10(1 - occlusion)` dB (with a 0.2 ratio on the low
band) respectively. There is **no whole-spectrum gain loss from
obstruction**: a software port may approximate it with a low-pass and a
small gain reduction. While the listener is underwater the direct path
of every sound is scaled to 0.25.

Reverb (the sound environment of the listener's cluster, see below) is
fed by a per-class *wet gain* (class table): the room send is
`1 - wet gain` (and halved when the listener is not underwater). Music
and ambience (wet 1.0) therefore get no room send.

### Channels, limits, preemption

The engine has fixed voice pools, split by what is played: 10 unspatialized
mono, 51 positional mono, 10 stereo 22 050 Hz, 10 stereo 44 100 Hz. A new
sound needs a free voice of its kind (mono positional or not by its
source's spatialization; stereo by rate). Selection:

1. A *speech* class sound (classes 19, 44, 46, 47) with an owner takes
   the voice already playing another speech sound of the **same
   owner** (it cuts the earlier line off); otherwise falls through to
   step 3.
2. Otherwise count voices playing the same sound (per definition) and
   the same sound from the same owner (per object), against the class
   limits (table: usually 4 and 1; projectile impact 6 and 4; vehicle
   classes 2 per object; music and scripted classes 4 per object). If the
   per-object limit is reached the candidates are those voices; else if
   the per-definition limit is reached the candidates are the sound's
   other voices. A candidate is *taken over* if it started at least
   `preemption time` ms ago and the new sound is not more than 1 WU^2
   farther away (`d_new^2 - d_old^2 < 1`). If none qualifies the new
   sound is dropped.
3. Otherwise: a free voice of the right kind, or, if none is free, the
   voice of the lowest class priority below the new sound's (or equal
   priority and farther away than it). If none, the new sound is dropped.

Priorities (higher wins): scripted dialogue to player and force
unspatialized 6; game event, scripted dialogue to others, projectile
detonation 5; weapons, projectile impact, first person damage 4;
object, particle and footstep, vehicle, unit dialogue, scripted effect
3; music and doors 2; machinery, nature, computers, ambient 1.

*Cache miss mode* describes the console's streaming cache; with all
samples in memory it never arises, and a port ignores it.

When all players are dead, scripted dialogue to the player not yet
playing is dropped and one playing fades out over 0.3 s; scripted
dialogue to others not yet playing is dropped.

### Per-frame order

1. Elapsed real milliseconds since the last frame; whole elapsed ticks
   advance the class gain fades.
2. Refresh each local player's listener: camera position and
   orientation, velocity, underwater flag.
3. Process looping sounds: delete any not refreshed since the last
   frame; fire the detail sounds that are due.
4. For every playing sound: finish it if its permutation ended
   (impulses) or its owner went away; re-read its source position and
   velocity (it follows its object); compute obstruction and the nearest
   listener; start the 2 s fade out when it becomes inaudible, the
   0.5 s fade in when audible again; update the dialogue duck.
5. For every sound that has no voice yet and has started: find a voice
   (Channels).
6. For every voice: compute the fade, the location in listener space, the
   linear distance fade of a stereo sound, the gain (formula above) and
   the pitch; queue the next permutation of a chain.

## Looping sounds (`lsnd`)

A looping sound is what plays music, ambience, engines, projectile
whistles, a charging weapon's hum and the like. One request gives it a
*source* with a changing *scale* (0..1) and a state: start, keep going,
or stop. Game objects own them through attachments (an engine on a
vehicle, the hum on a rocket) and the level through the background
sound and sound scenery.

### Header (0x54 bytes)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | bit 0 deafening (no use found in the engine code read; never set on the disc), bit 1 "fake impulse sound" (the sounds play once through and the loop ends instead of repeating; 10 of 55 in a30), bit 2 stops music (consulted by scripts; never set on the disc) |
| 0x04 | 12 | 3 x f32 | scale lower bound | detail period multiplier at scale 0, then two reals; (1.0, 1.0, 1.0) on every sound on the disc |
| 0x10 | 12 | 3 x f32 | scale upper bound | the same at scale 1; (1.0, 1.0, 1.0) |
| 0x1C | 4 | i32 | runtime | scripted sound index; -1 |
| 0x20 | 4 | f32 | runtime: maximum distance | WU: the largest maximum distance (resolved with class defaults) of every sound it plays; 2.5, 9, 15, 5, 70 on bloodgulch |
| 0x24 | 8 | | unused | zero |
| 0x2C | 16 | tag ref | continuous damage effect | class `cdmg`; none on bloodgulch (the engine refreshes a damage effect each frame the loop is audible) |
| 0x3C | 12 | block | tracks | element 0xA0, 1 to 4 (disc: 1 or 2) |
| 0x48 | 12 | block | detail sounds | element 0x68, up to 32 (disc: 0 or 1) |

Only the first real of each triple is used (the detail period
multiplier); the two extra reals read 1.0 and nothing uses them.

### Track (0xA0 bytes)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | bit 0 fade in at start, bit 1 fade out at stop, bit 2 fade in alternate (disc: 0, 2, 3) |
| 0x04 | 4 | f32 | gain | multiplies every sound of the track (0.1 to 1.0) |
| 0x08 | 4 | f32 | fade in duration | seconds |
| 0x0C | 4 | f32 | fade out duration | seconds |
| 0x10 | 32 | | unused | |
| 0x30 | 16 | tag ref | start sound | `snd!`, or none |
| 0x40 | 16 | tag ref | loop sound | `snd!` |
| 0x50 | 16 | tag ref | end sound | `snd!` (the "stop" sound) |
| 0x60 | 32 | | unused | |
| 0x80 | 16 | tag ref | alternate loop sound | |
| 0x90 | 16 | tag ref | alternate end sound | |

### Detail sound (0x68 bytes)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref | sound | the one-shot `snd!` |
| 0x10 | 8 | real_bounds | random period | seconds between plays |
| 0x18 | 4 | f32 | gain | source gain of each play (0.1 to 1.0) |
| 0x1C | 4 | flags32 | flags | bit 0 don't play with alternate, bit 1 don't play without alternate (disc: 0) |
| 0x20 | 48 | | unused | |
| 0x50 | 8 | angle_bounds | yaw (theta) bounds | radians; (-pi, pi) on the disc |
| 0x58 | 8 | angle_bounds | pitch (phi) bounds | radians; (-pi/2, pi/2) |
| 0x60 | 8 | real_bounds | distance bounds | WU; (0, 0) on every detail on the disc |

### How a loop plays

State per loop: the *alternate* flag, the track voices, the next play
time of each detail. Each frame the owner refreshes the loop with state
*start* (first frame), *loop* (every later frame) or *stop*, the scale
and the source; a loop not refreshed for a frame is deleted.

- **Start** (per track): if the track has a start sound, a voice plays
  it as an ordinary sound of its own tag. Then, unless the track has
  "fade in at start", the loop sound is *chained* to follow it: when the
  start sound's last chunk ends the same voice switches to the loop
  sound (new pitch range, new random head) with no gap. With "fade in
  at start" instead, the loop sound starts at once and fades in
  linearly over `fade in duration` while the start sound plays.
- **Loop**: when a loop sound's chain reaches its last chunk the next
  chunk queued is a fresh random head of the same sound (the deck of
  Permutation choice), so a loop of several recordings varies from pass
  to pass; with the loop's "fake impulse" flag set it does not repeat
  and the loop ends (a one-shot sequence). Chain chunks are queued to the
  hardware before the previous one ends: gapless.
- **Alternate**: an *alternate* state picks the alternate loop sound
  (and alternate end sound) of a track if it has one. A change of the
  flag, with the track's "fade in alternate" bit, starts the new sound
  and cross-fades linearly over `fade out duration`; without the bit
  the voice just continues into the other sound at the end of its
  chain.
- **Stop**: if the track's "fade out at stop" bit is set, or it has no end
  sound (and the loop is not "fake impulse"), the loop sound fades out
  linearly over `fade out duration` (or over the fade time a script
  gave: fixed 4 s for the "fixed fadeout" kind). With an end sound: with
  "fade out at stop" the end sound starts as its own voice while the
  loop fades out; without it the end sound is chained to follow the
  current chunk of the loop (the voice's type becomes "stopping").
- **Pitch ranges** (engines): each frame the voice's pitch is
  `lerp(zero pitch, one pitch, scale) * its random draw`, bounded by
  *maximum bend*. If it leaves the voice's pitch range a new voice
  begins in the right range and the two cross-fade over 0.5 s with
  `t' = t^(1/2.5)` for the rising voice and `1 - (1 - t)^(1/2.5)` for
  the falling one (t from 0 to 1).
- **Gain per voice**: `class gain * duck * track gain *
  lerp(zg, og, scale) * gm * source gain * fade * permutation gain` and
  the same distance rules as impulses (a mono loop is positional; a
  stereo loop with no spatialization is dry).
- **Detail sounds**: when the loop is created each detail's first time
  is `now + 1000 * lerp(lower period mod, upper period mod, scale) *
  uniform(period lower, upper)` ms. Whenever the time is due (checked
  once per frame while the loop is not stopping) and the flags allow, a
  one-shot of the detail's sound plays with gain `detail gain` and the
  loop's scale: at the loop's position plus a random offset (a
  distance drawn from the distance bounds, in a direction drawn from
  the yaw and pitch bounds; zero distance gives no offset) if the loop
  is positional, or, if the loop is unspatialized, as a *relative*
  source at that offset from the listener. The next time is then `now +
  1000 * lerp(...) * uniform(period) + longest permutation length of
  the detail sound` ms.
- **Audibility**: a loop is created only when audible (within its
  runtime maximum distance); a loop with no live voices and out of
  range is deleted and is re-created when it returns.

## Sound environment, background sound, sound scenery

**Sound environment** (`snde`, 72 bytes = 0x48): reverb parameters for
the room the listener is in (the BSP cluster's palette entry, structure_bsp.md;
the scenario picks the cluster of the camera). The values go to the 3D
audio hardware's room model (I3DL2 style):

| Offset | Size | Type | Name | Unit / meaning |
|---|---|---|---|---|
| 0x00 | 4 | i32 | unknown | 0 |
| 0x04 | 2 | i16 | priority | 0 |
| 0x06 | 2 | | unused | |
| 0x08 | 4 | f32 | room intensity | linear gain (volume = 2000 log10(g) hundredths dB) |
| 0x0C | 4 | f32 | room intensity HF | linear gain |
| 0x10 | 4 | f32 | room rolloff | factor |
| 0x14 | 4 | f32 | decay time | s |
| 0x18 | 4 | f32 | decay HF ratio | |
| 0x1C | 4 | f32 | reflections intensity | linear gain, volume offset by +1000 hundredths dB |
| 0x20 | 4 | f32 | reflections delay | s |
| 0x24 | 4 | f32 | reverb intensity | linear gain, volume offset by +2000 |
| 0x28 | 4 | f32 | reverb delay | s |
| 0x2C | 4 | f32 | diffusion | 0..1 (the hardware takes percent) |
| 0x30 | 4 | f32 | density | 0..1 (percent) |
| 0x34 | 4 | f32 | HF reference | Hz |
| 0x38 | 16 | | unused | |

Disc (a30): `mountains` (room 0.316, HF 0.0562, decay 1.49 s, HF ratio
0.21, reflections 0.0128 at 0.3 s, reverb 0.009 at 0.1 s, diffusion
0.27, density 1.0, HF reference 5000 Hz), `hallway` (0.316, 0.71, 1.49,
0.59, 0.077 at 0.007, 0.166 at 0.011, 1.0, 1.0, 5000), `concert_hall`
(0.316, 0.5623, 3.92, 0.7, 0.0767 at 0.02, 0.0997 at 0.029, 1.0, 1.0,
5000). The environment of a cluster is chosen by the scenario's
palette; a port without a reverb unit may ignore the tag.

**Background sound**: each BSP cluster may name a background sound (a
looping sound tag, structure_bsp.md "background sound palette"). The
game keeps one background loop at a time: when the listener's cluster
changes it stops the old loop and starts the new one, unspatialized
(plays dry, stereo ambience at 44 100 Hz on bloodgulch), with scale 1.0.
Bloodgulch's is `sound\sfx\ambience\multiplayer\bloodgultch` (one track,
loop `countryday2`, fade in and out 3.0 s, track gain 1.0).

**Sound scenery** (`ssce`): an object (objects.md) with no fields of its
own beyond the object header. Its sound is a looping sound attached at
a marker by the object's attachments block (scale from a named
function); the placements are in scenario.md (bloodgulch has two, both
`sound\sfx\ui\teleporter_loop`). The loop plays positionally like any
vehicle's: mono 3D with the loop sound's distances (teleporter loop:
0.5 and 2.5 WU, class 50).

## Measured facts

### bloodgulch.map

- 315 `snd!` tags (the other 13 multiplayer maps have 314 to 319), 14
  `lsnd`, 1 `snde`, 1 `ssce`. 322 pitch ranges (312 sounds with one, 2
  with four, 1 with two) and 916 permutations.
- **Compression**: 315 of 315 Xbox ADPCM (and 916 of 916 permutations).
- **Encoding and rate**: 269 mono 22 050 Hz; 45 stereo 22 050 Hz; 1
  stereo 44 100 Hz (`countryday2`). No mono 44 100 Hz.
- **Flags**: 263 none, 51 fit to block, 1 fit + split.
- **Distances**: 248 of 315 have both zero (class defaults); the others
  carry explicit pairs, e.g. 1.4/8 (22), 0.9/10 (20), 2/8 (5), 3/15,
  2.5/15, 6/18 (engines), 0.5/2.5 (1).
- **Cones**: 315 of 315 are (2 pi, 2 pi, 1.0). **Skip fraction** 0 on
  315 of 315.
- **Sample bytes**: 12 229 380 total (916 permutations); 826.8 s of
  audio counting each permutation once (stereo counted once).
- **Longest permutation**: `sound\sfx\weapons\weapon_anims\sniper_
  posing`, mono, 66 240 bytes = 1840 blocks = 117 760 samples = 5.34 s.
  Longest in the 24 maps: 21.74 s (`sound\dialog\x10\sgt05e`, a10, mono).
- **Class histogram** (class number: count): 0: 33, 1: 3, 4: 13, 5: 22,
  6: 20, 7: 3, 8: 2, 9: 5, 10: 8, 13: 75, 14: 20, 15: 2, 18: 41, 19: 2,
  22: 1, 23: 10, 33: 1, 39: 5, 45: 36, 50: 13 (sum 315). As names:
  projectile impact 33, projectile detonation 3, weapon fire 13, ready
  22, reload 20, empty 3, charge 2, overheat 5, idle 8, object impacts
  75, particle impacts 20, slow particle impacts 2, footsteps 41, unit
  dialog 2, vehicle collision 1, vehicle engine 10, ambient nature 1,
  first person damage 5, scripted effect 36, game event 13.
- **All 24 maps**: compression 1 on every sound (a10: 1468 of 1468; a30
  1465; a50 1510; b30 1435; b40 1503; c10 1346; c20 371; c40 935; d20 757;
  d40 769; ui 29; the 13 multiplayer maps 314 to 319 each); mono always
  22 050 Hz; stereo at 22 050 Hz or 44 100 Hz (campaign maps 31 to 65
  stereo, 24 to 57 of them at 44 100 Hz; multiplayer maps 46 to 48
  stereo, 1 to 3 at 44 100 Hz; `ui` 7 stereo, 2 at 44 100 Hz). Sample
  data per permutation is always a whole number of blocks.

### The four first weapons (bloodgulch)

All are class defaults for the distances unless noted (marked `*`).
Cones (2 pi, 2 pi, 1.0), random pitch (1, 1), skip 0, one pitch range,
mono at 22 050 Hz throughout. Durations are decoded sample counts
(`blocks * 64 / 22 050`), the range over a sound's permutations.

| Weapon | Sound (tag path under `sound\sfx\`) | Role | Class | Perms | Duration s | Min / max WU | Gain mod | Perm gain |
|---|---|---|---|---|---|---|---|---|
| assault rifle | `weapons\assault rifle\fire` | fire | 4 weapon fire | 4 | 0.601 to 0.795 | 4 / 70 * | 0.5 | 1.0 |
| | `weapons\pistol\eject` (shared) | casing eject (part of fire) | 4 | 2 | 0.401 to 0.403 | 4 / 70 * | 0.1 | 1.0 |
| | `weapons\assault rifle\dryfire` | empty | 7 weapon empty | 1 | 0.145 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\assault rifle\weapon ready` | ready | 5 | 1 | 1.158 | 1 / 9 * | 0.5 | 0.5 |
| | `weapons\weapon_anims\ar_reload` | reload | 6 | 1 | 3.065 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\ar_melee` | melee | 13 object impacts | 2 | 1.274 to 1.756 | 0.5 / 3 * | 0.5 | 1.0 (zero-scale gain 0) |
| | `weapons\assault rifle\flashlight` | light toggle | 10 weapon idle | 1 | 0.215 | 1 / 9 * | 0.5 | 1.0 |
| pistol | `weapons\pistol\fire` | fire | 4 | 1 | 1.335 | 4 / 70 * | 0.5 | 1.0 |
| | `weapons\pistol\dryfire` | empty | 7 | 1 | 0.125 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\pistol_ready` | ready | 5 | 1 | 1.097 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\pistol_reload` | reload | 6 | 1 | 1.686 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\pistol_melee` | melee | 13 | 2 | 1.155 | 0.5 / 3 * | 0.5 | 1.0 (zero-scale gain 0) |
| plasma rifle | `weapons\plasma rifle\fire` | fire (upper and lower barrel) | 4 | 5 | 0.775 to 0.975 | 4 / 70 * | 0.5 | 1.0 |
| | `weapons\plasma rifle\overheat` | overheat (misfire) | 9 weapon overheat | 3 | 0.949 | 1 / 9 * | 1.0 | 1.0 |
| | `weapons\weapon_anims\plasrifle_overheat` | overheat animation | 9 | 1 | 2.516 | 1 / 9 * | 1.0 | 1.0 |
| | `weapons\weapon_anims\plasrifle_oh_exit` | overheat recovery | 9 | 1 | 0.668 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\plasrifle_ready` | ready | 5 | 1 | 0.981 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\plasrifle_melee` | melee | 13 | 1 | 1.350 | 0.5 / 3 * | 0.5 | 1.0 (zero-scale gain 0) |
| | `weapons\plasma rifle\startcharge`, `charge`, `chargefire` | the charge (looping `plasma rifle\charging`), charged shot | 8 weapon charge, 8, 4 | 1, 1, 1 | 1.753, 2.598, 1.059 | 1 / 9 *, 1 / 9 *, 4 / 70 * | 0.2, 0.2, 0.5 | 1.0 |
| shotgun | `weapons\shotgun\fire` | fire (2 effect parts) | 4 | 1 | 1.646 | 4 / 70 * | 0.8 | 1.0 |
| | `weapons\weapon_anims\shotgun_fire` | pump animation | 4 | 1 | 0.810 | 4 / 70 * | 0.5 | 1.0 |
| | `weapons\assault rifle\dryfire` (shared) | empty | 7 | 1 | 0.145 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\shotgun_ready` | ready | 5 | 1 | 0.714 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\shotgun_reload1_1`, `reload2_1`, `reload3`, `reload4` | reload steps | 6 | 3, 3, 1, 1 | 0.543; 0.357 to 0.464; 0.729; 1.443 | 1 / 9 * | 0.5 | 1.0 |
| | `weapons\weapon_anims\shotgun_melee` | melee | 13 | 2 | 1.059 to 1.297 | 0.5 / 3 * | 0.5 | 1.0 (zero-scale gain 0) |

How the weapon reaches its sounds: the weapon's fire and empty effects
carry the fire and dryfire sounds; the plasma rifle's overheat is in the
misfire effect; the first-person animation graph carries the ready,
reload and melee sounds (sound references on animation frames) and the
overheat animation; the looping charge is the weapon tag's charging
loop. The melee sounds are class 13 sounds with gain-at-zero 0: they fade
in with the strength of the melee impact.

## What was checked

On the disc (`bloodgulch.map` unless noted):

- The tag layouts above against every `snd!` (315), `lsnd` (14), `snde`
  and the permutation data references of every map: sizes, offsets (all
  512-aligned, in tag order, no overlaps, region 0x175AA00 to
  0x233B540), enumerations on all 24 maps.
- The class table against the game's executable (51 records, all values
  of the table above), and the 8.9647064 ms per WU constant and the 0.007
  duck release rate against the executable (each occurs there once).
  The other timing constants (fade times, duck 0.7, delay threshold)
  and the 2.5 exponent are from the decompilation only.
- **All 292 954 blocks** of bloodgulch (mono and stereo): reserved byte
  0, step index at most 88, the padding nibble (nibble 63) 0 in every
  mono block and in both channels of every stereo block.
- **Decoded clips** (checked as numbers): assault rifle fire (4
  permutations), pistol fire, plasma rifle fire, the dirt, gravel and
  concrete walking footsteps, the Chief's death lines, the announcer
  lines `play_ball` and `double_kill` (stereo), the ambience
  `countryday2` (stereo, 44 100 Hz), the stereo bullet-impact sound, and
  the music `title1\in` from `ui.map` (stereo 44 100 Hz), including a
  whole chain of the music `title1\loops`.
  - No decoded clip runs away: means are within -77 to +26 of zero
    (guns 4 to 11, voice 0 to 3, music -3 to 1.5).
  - Saturation (samples at -32768 or 32767): assault rifle fire perm 0:
    1 sample of 15 488 (0.0065 %); pistol 0.037 %; plasma rifle 0.35 %
    (60 samples; a hot low-frequency thump, contiguous); everything
    else 0 or below 0.01 %. A wrong step table or wrong nibble order
    saturates a large share of a clip or gives flat noise.
  - **Block boundaries**: the mean jump between the last decoded sample
    of a block and the first of the next, divided by the mean jump
    between adjacent samples inside blocks, is 0.91 to 1.16 on every
    clip checked (for example: assault rifle fire 1.16, pistol 0.97,
    plasma rifle 1.09, footsteps 0.91, announcer 0.98, ambience 1.02 and
    0.96, bullet impact 0.95 and 0.92, music 1.00). A wrong block
    alignment or header interpretation would give a ratio of several
    (the same measure taken one sample off gave jumps 1.5 to 2 times
    the in-block mean on the voice line). Seams between chain chunks:
    639 against 618 inside chunks (all three chains of the music
    `title1\loops`).
  - **Spectra** (share of energy by band, below 250 Hz / 250-500 /
    500-1000 / 1000-2000 / 2000-4000 / 4000-8000 / above): assault
    rifle fire 0.60 / 0.22 / 0.05 / 0.03 / 0.05 / 0.04 / 0.01; a Chief
    death line 0.01 / 0.10 / 0.82 / 0.06 / 0.01 / 0.00 / 0.00 and
    another 0.00 / 0.03 / 0.44 / 0.43 / 0.08 / 0.01 / 0.00 (a voice
    centred around 1 kHz); music 0.50 / 0.40 / 0.09 / 0.01 / 0 / 0 / 0
    (low and bass-heavy); the bullet-impact crackle 0.03 / 0.01 / 0.02 /
    0.83 / 0.08 / 0.02 / 0.01; the ambience bright 0.03 / 0.08 / 0.15 /
    0.11 / 0.11 / 0.21 / 0.32 (wind and birds).
  - **Durations**: gunshots 0.60 to 1.65 s (assault rifle 0.60 to 0.80,
    pistol 1.34, plasma 0.78 to 0.98, shotgun 1.65), footsteps 0.55 to
    0.72, unit voice lines 0.37 to 1.68 s, announcer lines 1.12 and 1.33 s (the two checked),
    music chunk 1.32 s; all plausible.
- **Not checked**: anything by ear; the hardware's exact behaviour for
  distance, cone, Doppler, obstruction, occlusion and reverb (read from
  the decompilation and the 3D audio API's published model); the fade
  and duck timings (decompilation only); that a software mixer's output
  matches the console's.

## Edge cases

- **Missing or empty data**: a sound with no pitch range, or with range 0
  empty, is not playable (dropped). A permutation with sample size 0 is
  silent; a reader must not fail on it.
- **Index checks**: a permutation's next index outside its range's
  element count ends the chain; an actual permutation count above the
  element count or below 1 is clamped into 1..count; a class index
  outside 0..50 is clamped to the nearest class.
- **Tag distances**: if the maximum is not above the minimum after
  resolving, a stereo linear fade divides by 0 or a negative: treat the
  fade as 1 inside `m` and 0 outside. (Not on the disc.)
- **Stereo sounds with a nonzero spatialization**: dry and unpanned in
  the original (see Distance). A port may pan them; that is an
  extension, not the original.
- **Mono 44 100 Hz, uncompressed, IMA and Ogg sounds** are not playable on
  the Xbox and never appear on the disc; a reader for this port refuses
  such a tag (compression other than 1, mono at rate 1).
- **Sample rate other than 0 or 1**: refuse.
- **Impulse sounds and chains**: see Quirks.
- **Moving sources**: an impulse keeps tracking its owning object while it
  plays (position, velocity); if the owner disappears the sound ends
  (non-speech sounds continue at the last position).

## Quirks

- **Impulses play only one chunk of a split sound.** A one-shot request
  for a split sound plays the chosen head only (1.32 s of a long
  sound). The long sounds on the disc are all reached through looping
  sounds, which follow the chain; keep it that way.
- **Maximum distance is a cutoff, not an endpoint**, for mono sounds: a
  gunshot is audible (very quietly, `4 / 70 = 0.057`) right up to 70 WU
  and then fades away over 2 s; stereo sounds fall linearly to 0 at
  the maximum instead. Keep both.
- **Maximum bend per second** is applied once per update (per rendered
  frame), not per second: a value of 1.003 therefore limits a frame, so
  the pitch of an engine follows its scale at a speed that depends on the
  frame rate. Only one sound on the disc sets it; the quirk is kept as
  data and noted.
- **The skip roll** marks a skipped permutation as played; and the 17th
  attempt accepts unconditionally, so a deck of very skippable
  permutations still returns something.
- **Stored "longest permutation length"** is `bytes / (rate * 0.5 *
  channels) * 1000` ms (it ignores the 4-byte block headers), i.e. 1.125
  times the true duration. It is used for promotion and for detail
  sound spacing, so keep using the stored value (not the true length) for
  those.
- **Scale zero and gain-at-zero 0** is a silent request: a melee swing
  that hits nothing (scale 0) makes no melee impact sound.
- **Weapon ready on the assault rifle** has permutation gain 0.5 on top
  of gain modifier 0.5 (a quarter of full amplitude); keep the product.
- **The class gain/ duck** applies to every class except the three
  dialogue classes; music is ducked during scripted dialogue.
- **The relative-source audibility test** compares a *squared* distance
  with the *unsquared* maximum distance (relative sources are rare;
  not checked on the disc). Keep it only if a relative source needs the
  exact original behaviour.
- **Fields nothing reads**: the class record's two spare values, the
  scripting fields in the tag, the looping sound's extra scale reals.

## Test cases

### (a) Synthetic

All blocks little-endian. Decoded values are of the decoder above.

**Mono block A** (36 bytes, hex):
`e8030a00 078f53a1` followed by seven groups of `00000000`
(header: predictor 1000, step index 10, reserved 0; data bytes 07 8F 53
A1 then 28 zero bytes). The codes are, in order: 7, 0, F, 8, 3, 5, 1, A,
then zeros. Expected 64 samples (8 per row):

| Samples | | | | | | | |
|---|---|---|---|---|---|---|---|
| 0..7 | 1000 | 1034 | 1039 | 971 | 961 | 1024 | 1114 | 1150 |
| 8..15 | 1095 | 1105 | 1114 | 1122 | 1129 | 1135 | 1141 | 1146 |
| 16..23 | 1151 | 1155 | 1159 | 1162 | 1165 | 1168 | 1170 | 1172 |
| 24..31 | 1174 | 1176 | 1178 | 1179 | 1180 | 1181 | 1182 | 1183 |
| 32..39 | 1184 | 1185 | 1185 | 1185 | 1185 | 1185 | 1185 | 1185 |
| 40..63 | 1185 (all 24) | | | | | | | |

The first steps, for reading the recurrence: step index 10 (step 19),
code 7: diff = 2 + 19 + 9 + 4 = 34, predictor 1034, index 10 + 8 = 18;
code 0 at step index 18 (step 41): diff 5, predictor 1039, index 17;
code F at step index 17 (step 37): diff 4 + 37 + 18 + 9 = 68 subtracted,
predictor 971, index 25.

**Mono block B** (clamping): header predictor 32000 (bytes 00 7D),
step index 85, reserved 0; data bytes 77 77 FF FF FF FF then 26 zero
bytes. Expected samples: 32000, 32767, 32767, 32767, 32767, -28669,
-32768 (then -32768 up to sample 12), sample 13 = -28673, 14 = -24949, 15
= -21564; sample 63 = 11920. (The first code 7 at step index 85, step
24 623, adds 3077 + 24 623 + 12 311 + 6155 = 46 166 and clamps to 32767,
index 85 + 8 = 93 clamps to 88 (step 32 767). The first code F then
subtracts 4095 + 32 767 + 16 383 + 8191 = 61 436: 32 767 - 61 436 =
-28 669, not clamped; the next code F clamps at -32768; the first zero
code after them adds 32 767 >> 3 = 4095, so sample 13 = -28 673.)

**Mono block C** (floor of the index): header predictor -32000 (bytes
00 83), step index 2, reserved 0; data bytes 88 88 00 00 and 28 zero
bytes. Samples 0..9: -32000, -32001, -32002, -32002, -32002, -32002,
-32002, -32002, -32002, -32002 (code 8: negative with diff `step >> 3`
= 1, 1, then 0 once the index reaches 0 and stays there).

**Stereo block** (72 bytes): left header `f4 01 00 00` (500, index 0),
right header `0c fe 14 00` (-500, index 20); data (64 bytes):
`21 43 65 07` (left group 0), `9f ba dc 0e` (right group 0), `11 11 11
11` (left group 1), `99 99 99 99` (right group 1), then 48 zero bytes.
Expected left samples 0..17: 500, 501, 504, 508, 515, 527, 547, 588,
594, 610, 625, 638, 650, 660, 670, 679, 686, 688; sample 63 = 701.
Expected right samples 0..17: -500, -593, -632, -692, -769, -859, -992,
-1223, -1192, -1277, -1355, -1425, -1489, -1547, -1599, -1647, -1690,
-1677; sample 63 = -1564. Left codes of group 0 are 1 2 3 4 5 6 7 0
(low nibble first), right's F 9 A B C D E 0. (A decoder that
interleaves the channels per byte, or per nibble, gives different
numbers.)

| Setup | Input | Expected |
|---|---|---|
| 36 + 36 bytes mono | decode | 128 samples; each block decodes alone |
| 72 bytes stereo | decode | 64 frames, interleaved L, R |
| 36 * 3 + 20 bytes mono | decode | 192 samples; the 20-byte remainder ignored |
| a block with header index 200 | decode | index clamped to 88 first |
| duration | 15 488 samples at 22 050 Hz | 0.7024 s |

**Distance** (`m = 4`, `M = 70`): mono gains for `d = 0, 4, 5, 8, 16,
40, 70` are 1, 1, 0.8, 0.5, 0.25, 0.1, 0.0571429; `d = 70.01` is
inaudible and the voice fades to zero over 2 s. Stereo (linear) gains:
`d = 4, 20.5, 37, 70, 100` give 1, 0.75, 0.5, 0, 0.

| Setup | Input | Expected |
|---|---|---|
| tag min 0, max 0, class 4 | resolve distances | 4 and 70 |
| tag min 1.4, max 0, class 13 | resolve | 1.4 and 3 (each separately) |
| tag min 6, max 18, class 23 | resolve | 6 and 18 |
| class 14, both zero | resolve | 0.5 and 3 |
| class 19 sound, both zero | resolve | 3 and 20 |
| class 33, both zero | resolve | 0.9 and 5 |

**Gain**: permutation gain 1.0, header gain modifier 0.5, both scale
modifiers 1.0, scale 1, class gain 1, no duck, fade 1, source gain 1,
within the minimum distance: `G = 0.5`, hardware volume -602 (-6.02 dB);
at 8 WU (min 4): `G = 0.25`. A melee sound with zero-scale gain 0 and
one-scale gain 1, gain modifier 0.5: at scale 0.5 the lerp is 0.5 and
`G = 0.25` (before distance); at scale 0 the request is dropped.

**Pitch range selection** (the bloodgulch sound `sound\sfx\vehicles\
warthog_engine`, ranges in file order: `med` natural 1.7 bend 1.25 to
1.7; `low` natural 1.45 bend 1.0 to 1.45):

| Pitch | Old range | Expected |
|---|---|---|
| 1.3 | none | `med` (first in file order that contains it) |
| 1.3 | `low` | `low` (the old range still contains it) |
| 1.1 | none | `low` |
| 1.8 | none | `med` (ratio 1.8/1.7 = 1.059 against 1.8/1.45 = 1.241) |
| 0.9 | none | `low` (ratio 1.0/0.9 = 1.111 against 1.25/0.9 = 1.389) |

Hardware pitch: pitch 1.7 in range `med` (playback rate 1/1.7) gives
exactly 1.0 times the sample rate.

**Permutation choice** (n = 3, skip fractions all 0, mask 0, previous
-1; the "draw" is the integer drawn first):

| Call | Draw | Result | Mask after |
|---|---|---|---|
| 1 | 1 | 1 | 0b010 |
| 2 | 1 | 2 (1 is set, move on) | 0b110 |
| 3 | 0 | 0 | 0b111 |
| 4 | 0 | 1 (all set: reset, mask = bit of previous 0; 0 is set so move to 1) | 0b011 |

Skip: n = 2, skip fractions [0.5, 0], mask 0, previous -1; draw 0, then
`r = 0.3` (less than 0.5: skipped, bit 0 stays set), move to 1, `r =
0.1` (1's fraction is 0: accepted): result 1, mask 0b11. With `r = 0.7`
for the first candidate the result is 0.

### (b) Disc-backed (slow tests; need the user's own disc files)

**bloodgulch.map**, `sound\sfx\weapons\assault rifle\fire` (tag 458, id
0xE33E01CA, struct address 0x8085336C):

| Check | Expected |
|---|---|
| class, rate, encoding, compression, flags | 4, 0 (22 050 Hz), 0 (mono), 1, 0 |
| minimum / maximum distance, gain modifier | 0 / 0, 0.5 |
| cone inner, outer, outer gain | 2 pi, 2 pi, 1.0 |
| pitch ranges | 1, named `default`, natural pitch 1.0, bend (0.0, 1.0), actual count 4, playback rate 1.0 |
| permutations | 4: `brown2_1`, `brown2_2`, `brown2_3`, `brown2_5`; next index -1; gain 1.0, skip 0 |
| sample sizes (bytes) and offsets | 8712 at 0x019C3800, 8712 at 0x019C5C00, 7452 at 0x019C8000, 9864 at 0x019C9E00 |
| sample counts | 15 488, 15 488, 13 248, 17 536 |
| stored longest permutation length | 894 ms (= 9864 / 11.025) |
| permutation 0, first 8 samples | -155, -144, -174, -237, -373, -666, -1297, -2654 |
| permutation 0, last 2 samples | 0, 0 |
| permutation 0: minimum and maximum | -32768, 31859; peak amplitude 32768 |
| permutation 0: RMS, mean | 5700 (5676.3), 4.3 |
| permutations 1 to 3, RMS (two digits) | 6400, 6100, 5000; first sample -957, -104, -100 |

**bloodgulch.map**, `sound\sfx\ambience\multiplayer\countryday2`
(stereo 44 100 Hz, class 33, flags 3, 1 range, 21 permutations, actual
count 4; next indices 4, 8, 11, 16, 5, 6, 7, -1, 9, 10, -1, 12, 13, 14,
15, -1, 17, 18, 19, 20, -1; distances 0 / 0 so 0.9 / 5 from the class;
gain modifier 1.0):

| Check | Expected |
|---|---|
| permutation 0: size, offset | 65 520 bytes at 0x0221E000 |
| permutation 0: frames per channel | 58 240 (1.32 s) |
| left first 8 | -37, -350, 113, 174, 230, 77, 123, 81 |
| right first 8 | -267, -730, -175, -101, -33, 398, 454, 403 |
| left min / max / RMS / mean | -1440 / 1612 / 360 (361.2) / 26 (25.5) |
| right min / max / RMS / mean | -2026 / 1788 / 440 (435.0) / 0.9 |
| last 3 frames | left -465, 82, 181; right 65, -17, -91 |
| chain from head 3 | 3, 16, 17, 18, 19, 20; last chunk 2880 bytes (= 2880 / 72 * 64 = 2560 frames) at file offset 0x0233AA00 |

**Class table** (from the disc's executable): the 51 records match the
table above (for example record 4: limits 4 and 1, preemption 0 ms,
priority 4, wet gain 0.5, distances 4 and 70).

**ui.map**, `sound\music\title1\in`: stereo 44 100 Hz, class 32, flags
3, 26 permutations, chunks of 65 520 bytes; permutation 0 begins with
silence and has a peak of about 13 000 (13 065 left, 13 120 right).
`sound\music\title1\loops` has 154 permutations in 3 chains of 24, 47
and 83 chunks (heads 0, 1, 2; actual count 3).

**bloodgulch.map**, the 4-range engine `sound\sfx\vehicles\
warthog_engine5` (class 23, 3.0 / 15.0 WU, gain modifier 1.0, flags 1): ranges
`med` (natural 1.63, bend 1.5 to 1.75, 3 permutations), `low` (1.38,
1.1 to 1.5, 3), `idle` (1.1, 1.0 to 1.1, 3), `high` (2.3, 1.75 to 2.3,
1); playback rates 1 / natural.

## Confidence per section

| Section | Confidence |
|---|---|
| Sound tag header, pitch range and permutation layouts and units | checked against the disc (all 24 maps for enumerations, bloodgulch for the rest) |
| Runtime and unknown words | checked on disc (values only); meanings from the decompilation |
| Class table | checked against the disc's executable (all 51 records); field meanings from the decompilation |
| Xbox ADPCM block layouts, decoder, padding nibble, sample counts | checked against the disc numerically (every block of bloodgulch; clip sanity as above); hardware decode detail assumed equal |
| Long sound chains | checked on the disc (bloodgulch, ui, a30) |
| Mouth data | checked against the disc (length rule); use from the decompilation |
| Admission rules, skip, promotion | decompilation only (promotion data checked on the disc) |
| Pitch range choice, permutation choice | decompilation only (data fields checked on the disc) |
| Gain composition | decompilation only (values checked on the disc) |
| Mono distance model (inverse, cutoff at max), delay | decompilation only for the sound manager; the hardware's inverse-distance model is its documented behaviour; the 8.9647 delay constant and the degrees conversion were found in the disc's executable (a 3.048 constant is there too, not tied to this use) |
| Stereo linear fade | decompilation only |
| Cones, Doppler, obstruction numbers | decompilation only; cones unused on the disc |
| Voice pools, limits, preemption | decompilation only (class limits from the disc's executable) |
| Looping sound tag layouts | checked against the disc (all of bloodgulch's 14, a30 for details) |
| Looping behaviour (start, loop, stop, alternate, details) | decompilation only |
| Sound environment layout | checked against the disc (three tags) |
| Background sound and sound scenery | layout checked against the disc; behaviour from the decompilation |
| Measured facts | checked against the disc |

## Open questions

1. **Hardware behaviour not reproduced**: Doppler, cone interpolation,
   obstruction and occlusion filters, reverb, and the head-related
   positioning on the console are the console's audio unit's job; this
   spec gives the engine side. A first port can use inverse distance,
   panning by direction, and ignore the rest. Compare against a real
   console or a trusted emulator if exactness matters.
2. **Header sample**: block samples are `header predictor` then 63
   decoded codes with the padding nibble unused. Supported by the zero
   padding nibble in all 292 954 blocks and by seam continuity (ratio
   about 1.0 against several for the alternatives), not by hardware
   documentation. A one-sample shift would not be audible.
3. **Uncompressed sounds** (compression 0) have no layout evidence: no
   disc sound uses it; the PC release's 16-bit big-endian PCM is the
   likely format but is unverified.
4. **Fade and duck constants** (2 s, 0.5 s, 0.3 s, 0.7, 0.03, 2.5, 250 ms)
   come from the decompilation of another build; only 8.9647 and 0.007
   were found in the disc's executable. They are immediates elsewhere.
   Confirm by disassembling the sound manager of the disc's executable.
5. **Per-class "unknown" values** (gain 0 or 1, scale 1) exist in the
   table and nothing reads them.
6. **Maximum bend per second**: applied per update in the decompilation
   (hence frame-rate dependent) though named per second; only one sound
   on the disc has a value (1.003). Decide whether to apply per second
   (`bend^(dt)`) for a stable port.
7. **Looping sound scale pair**: only the first real of each scale
   triple is used (detail periods); the other two are 1.0 everywhere.
8. **Voice limits**: 51 virtual and 46 hardware positional voices; how
   the game multiplexes virtual onto hardware voices when over 46 are
   active is not specified here; a software mixer can use 51.
9. **Subtitle data** is empty everywhere checked (bloodgulch, a10);
   a layout for non-empty subtitle data is unknown.
10. **Promotion counter details**: the decay uses the render clock; the
    promotion rule is from the decompilation and has not been observed.
11. **How a listener is chosen with split screen** (nearest of 4) is
    written from the decompilation; the PC release of the game is not
    this build.
12. **Stereo positional sounds**: this build does not pan stereo
    sounds. Later releases did; confirm no sound relies on it by checking
    which stereo sounds a map plays with absolute sources (the bullet
    impact stereo sounds, class 0, are the candidates).
