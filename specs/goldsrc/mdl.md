# GoldSrc: studio models, MDL version 10

Source basis: public format documentation and community write-ups of
the Half-Life model format; Valve's Half-Life SDK (spec-session
reference only) for the meaning of fields and the animation sampling;
every `.mdl` in the HLDS install's `cstrike/models/` and
`valve/models/` (502 files: 467 models, 35 sequence files), read with
throwaway scripts outside the repository. No engine source, no
decompiled or leaked code.
Status: draft

## Summary

A model is one file (`IDST`, version 10) holding a header and arrays
reached by (count, offset) pairs: bones, bone controllers, hitboxes,
sequences, sequence groups, textures, skins, body parts (each with
alternative submodels, each with meshes), attachments. Vertices are
rigid: each belongs to exactly one bone. Animation is per bone and per
channel (3 positions, 3 Euler angles), stored as 16-bit deltas with a
small run-length scheme. Two companions may exist: `<name>T.mdl` holds
the textures when the main file has none, and `<name>01.mdl`,
`<name>02.mdl`... (`IDSQ`) hold the animation of sequences in groups 1 and
up.

## Units and conventions

As in [bsp.md](bsp.md). Positions are in units, angles in radians,
frames at the sequence's own frames per second. Every offset is from
the start of the file that holds the structure (the sequence file's own
start for animation in a sequence file). Field names are ours.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| model_magic | `IDST` | 4 bytes | bytes 0-3 of a model or texture file |
| sequence_magic | `IDSQ` | 4 bytes | bytes 0-3 of a sequence group file |
| version | 10 | i32 | bytes 4-7 (one file in `valve/` is version 9: `doctor.mdl`; out of scope) |
| header_size | 244 | bytes | the model header |
| sequence_header_size | 76 | bytes | the sequence file header |
| bone_size | 112 | bytes | |
| controller_size | 24 | bytes | |
| hitbox_size | 32 | bytes | |
| sequence_size | 176 | bytes | |
| group_size | 104 | bytes | |
| event_size | 76 | bytes | |
| attachment_size | 88 | bytes | |
| bodypart_size | 76 | bytes | |
| submodel_size | 112 | bytes | |
| mesh_size | 20 | bytes | |
| texture_size | 80 | bytes | |
| anim_record_size | 12 | bytes | per bone per blend: six u16 channel offsets |

## Header (244 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 4 bytes | `IDST` |
| 4 | i32 | version (10) |
| 8 | 64 bytes | name as compiled (e.g. `gign\gign.mdl`, `v_ak47.mdl`; informative only) |
| 72 | i32 | file length (equals the file size in every installed file) |
| 76 | 3 × f32 | eye position |
| 88 | 3 × f32 | movement hull minimum |
| 100 | 3 × f32 | movement hull maximum |
| 112 | 3 × f32 | clipping box minimum |
| 124 | 3 × f32 | clipping box maximum |
| 136 | i32 | flags (0 in 462 models; a few Half-Life monsters use 256, 512, 768, 1792: effect bits the party does not need) |
| 140 | i32, i32 | bone count, offset |
| 148 | i32, i32 | bone controller count, offset |
| 156 | i32, i32 | hitbox count, offset |
| 164 | i32, i32 | sequence count, offset |
| 172 | i32, i32 | sequence group count, offset |
| 180 | i32, i32, i32 | texture count, texture records offset, texture pixel data offset |
| 192 | i32, i32, i32 | skin reference count (textures per skin), skin family count, skin table offset |
| 204 | i32, i32 | body part count, offset |
| 212 | i32, i32 | attachment count, offset (corrected 2026-11-24: v_ak47 has 2 attachments, count at 212, offset 4 948 at 216) |
| 220 | 4 × i32 | sound tables (unused: all 0) |
| 236 | i32, i32 | transition count, offset (0 in every file) |

The CS player and view models have all five boxes zero.

### Worked example: `cstrike/models/player/gign/gign.mdl`

2 329 328 bytes. 53 bones at 244, 1 controller at 6 180, 21 hitboxes at
6 380, 111 sequences at 2 016 024, 1 group at 2 038 144, 2 textures at 2
061 388 with pixels from 2 061 552, 2 skin references × 1 family at 2
061 548, 2 body parts at 2 038 248, 2 attachments at 6 204.

### Worked example: `cstrike/models/v_ak47.mdl`

295 128 bytes. 42 bones, no controllers, 4 hitboxes, 6 sequences at 25
104, 1 group, 11 textures at 56 400 with pixels from 57 304, 11 skin
references × 1 family, 3 body parts (`rhand`, `lhand`, `weapon`), 2
attachments.

## Bones (112 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 32 bytes | name |
| 32 | i32 | parent bone, −1 for a root |
| 36 | i32 | flags (0; garbage in a handful of old files: ignore) |
| 40 | 6 × i32 | controller driving each channel, −1 = none |
| 64 | 6 × f32 | default value per channel: position x, y, z, rotation x, y, z |
| 88 | 6 × f32 | scale per channel: what one step of stored animation is worth |

A parent always comes before its children. The bind pose is the
default values. Example, gign bone 0 `Bip01`: root, defaults (0.2338,
−2.2517, 38.1922, 0, 0, −1.5708), position scales 0.0039064 (1/256), rotation
scales about 9.6e−5, 5.0e−5, 9.5e−5.

**Rotation channels** are Euler angles in radians about X, Y and Z; the
bone's rotation is the rotation about Z by the third, applied after the
rotation about Y by the second, applied after the rotation about X by
the first (q = q_z · q_y · q_x). A bone's local transform is that rotation
and the position; its world transform is its parent's world transform
times the local one.

## Bone controllers (24 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | bone |
| 4 | i32 | channel flags: 1, 2, 4 = position x, y, z; 8, 16, 32 = rotation x, y, z; 0x8000 = wraps around (rotation loops) |
| 8 | f32 | start (degrees for rotations, units for positions) |
| 12 | f32 | end |
| 16 | i32 | rest value |
| 20 | i32 | index: 0-3 are general controllers set by the game, 4 is the mouth |

A controller's input (0-255) maps linearly to start..end and is added to
the channel's value (converted to radians for rotation channels). Over
the install: 74 rotate about X, 21 about Z, 11 about Y, 4 wrap.
Example: gign's only controller is on bone 8 about Z (type 32), 0 to 30,
index 4 (the mouth).

## Hitboxes (32 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | bone |
| 4 | i32 | hit group |
| 8 | 3 × f32 | box minimum, in the bone's frame |
| 20 | 3 × f32 | box maximum |

Hit groups as CS 1.6's player models use them: 0 generic, 1 head, 2
chest, 3 stomach, 4 left arm, 5 right arm, 6 left leg, 7 right leg, 8
the shield (a large box on the left hand; inferred to count only while
a shield is carried, to be measured). Values 10 and 11 occur in some Half-Life monsters. The damage
each group takes is `specs/cs16/damage.md`'s business.

gign.mdl's 21 hitboxes:

| # | Bone | Group | Minimum | Maximum |
|---|---|---|---|---|
| 0 | Bip01 Pelvis | 3 | (−4.69, −4.44, −6.75) | (4.0, 5.56, 6.75) |
| 1-3 | L Thigh, L Calf, L Foot | 6 | | |
| 4-6 | R Thigh, R Calf, R Foot | 7 | | |
| 7 | Bip01 Spine1 | 3 | (−3.25, −5.5, −5.5) | (4.83, 5.5, 5.5) |
| 8, 9, 10 | Spine2, Spine3, Neck | 2 | | |
| 11 | Bip01 Head | 1 | (−0.33, −4.5, −2.5) | (8.0, 4.5, 2.5) |
| 12-15 | L Clavicle, UpperArm, Forearm, Hand | 4 | | |
| 16-19 | R Clavicle, UpperArm, Forearm, Hand | 5 | | |
| 20 | Bip01 L Hand | 8 | (−16.1, −6.5, −19.0) | (10.6, 1.5, 33.0) |

## Sequences (176 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 32 bytes | name |
| 32 | f32 | frames per second |
| 36 | i32 | flags: 1 = loops |
| 40 | i32 | activity (a game-defined number; 0 = none) |
| 44 | i32 | activity weight (for picking among sequences of one activity) |
| 48 | i32, i32 | event count, offset |
| 56 | i32 | frame count |
| 60 | i32, i32 | pivot count, offset (unused by us) |
| 68 | i32 | motion type: which root channels were extracted (same bits as controllers; 0x40 = linear X) |
| 72 | i32 | motion bone |
| 76 | 3 × f32 | linear movement over one cycle |
| 88 | i32, i32 | unused automove offsets |
| 96 | 3 × f32 | box minimum |
| 108 | 3 × f32 | box maximum |
| 120 | i32 | blend count |
| 124 | i32 | animation offset (into the main file for group 0, into the group's file otherwise) |
| 128 | 2 × i32 | blend channel types (same bits; 8 = rotation about X: pitch) |
| 136 | 2 × f32 | blend start (degrees) |
| 144 | 2 × f32 | blend end (degrees) |
| 152 | i32 | blend parent (unused) |
| 156 | i32 | sequence group |
| 160 | 4 × i32 | transition graph: entry node, exit node, node flags, next sequence |

One cycle runs from frame 0 to frame count − 1, so it lasts (frame
count − 1) / frames per second; a looping sequence then wraps to frame 0
(its last frame repeats its first: in gign's `run` every channel's last
stored value equals its first), a non-looping one holds its last frame.
A one-frame sequence is a still pose.

Linear movement: when the motion type has linear bits, the compiler
removed the root's travel from the animation and stored it here; the
game moves the entity, and the animation's speed is linear movement ÷
duration. gign `walk` (30 fps, 33 frames, 64 units along X): 64 / (32 /
30) = 60 units per second; `run` (60 fps, 37 frames, 150 units): 150 / (36
/ 60) = 250; `crouchrun` (30 fps, 31 frames, 75 units): 75.

**Blends.** A sequence with n blends has n sets of animation records
back to back: blend b's records start at animation offset + b × bone
count × 12 (the streams they point at follow all the records). Blend values are spread evenly from blend
start to blend end. Counts in the install: 2 750 sequences with 1
blend, 855 with 2, **660 with 9**: CS 1.6's player models aim by pitch
with 9 blends from −90° to +90° (one every 22.5°), all with blend type 8.
gign `ref_aim_ak47`: 2 frames at 30 fps, looping, 9 blends, pitch −90
to 90. The pose for a pitch p is the blend of the two neighbouring
blends by where p falls between them (open question 2).

Example, v_ak47's sequences:

| # | Name | fps | Frames | Loops | Events |
|---|---|---|---|---|---|
| 0 | idle1 | 30 | 17 | no | |
| 1 | reload | 37 | 91 | no | frame 13 sound `weapons/ak47_clipout.wav`; frame 57 `weapons/ak47_clipin.wav` |
| 2 | draw | 30 | 31 | no | frame 11 `weapons/ak47_boltpull.wav` |
| 3-5 | shoot1-3 | 20 | 17 | no | frame 0 muzzle flash, option `22` |

So the reload animation lasts 90 / 37 = 2.43 s and the draw 30 / 30 =
1.0 s (whether the game's timings match is `specs/cs16/weapons.md`'s
business).

## Events (76 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | frame |
| 4 | i32 | event number |
| 8 | i32 | type (0) |
| 12 | 64 bytes | options text |

Event numbers seen and what they mean in CS 1.6 (client events are
5000 and up):

| Number | Count | Meaning (ours) |
|---|---|---|
| 5001 | 376 | muzzle flash at attachment 0; options: a two-digit code, tens = scale, units = which flash sprite (`muzzleflash1-4`) (open question 3) |
| 5011 | 106 | muzzle flash at attachment 1 (the second gun of the dual pistols) |
| 5021 | 45 | muzzle flash at attachment 2 |
| 5004 | 158 | play the sound named in options, at the model |
| 5002 | 22 | spark effect |
| 1000-1008, 2001, 2002, 1-50 | | Half-Life monsters' server events (footsteps, attacks): not needed |

## Sequence groups (104 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 32 bytes | label (`default` for group 0) |
| 32 | 64 bytes | file name for groups 1 and up, with backslashes (`models\barney01.mdl`) |
| 96 | 2 × i32 | runtime fields (0) |

Group 0's animation is in the model file itself. For group g ≥ 1 the
file is the named one (mount-relative, backslashes to slashes, looked up
case-insensitively); its header is `IDSQ`, version 10, a 64-byte name
and a length (76 bytes), and sequence animation offsets are relative to
its start. Example: `valve/models/barney.mdl` has 4 groups, files
`models\barney01.mdl`..`03.mdl`; sequence 18 `cower_stand` (51 frames)
is in group 1 with animation offset 76, right after `barney01.mdl`'s
header (that file: `IDSQ`, 10, 64 668 bytes). No CS 1.6 model uses
groups beyond 0 except the hostages (`hostage01.mdl`..`08.mdl` are
sequence files: `hostage.mdl` has 9 groups).

## Animation streams

For blend b of a sequence and bone i, the animation record is at
animation offset + (b × bone count + i) × 12: six u16 offsets, one per
channel, each **relative to the record's own start**. An offset of 0
means the channel is constant: its value is the bone's default. Otherwise
the channel's stream is a run of blocks:

- a block begins with two bytes, *valid* then *total* (frames covered);
- *valid* i16 values follow;
- frames 0 to valid − 1 of the block take those values in order; frames
  valid to total − 1 repeat the last stored value.

Blocks follow one another until the sequence's frames are covered. A
channel's value at frame f is default + stored × scale (from the bone),
plus the controller's contribution when one drives it.

Worked examples (`v_ak47.mdl`):

- `shoot1` (17 frames) has its blend 0 records at 21 276. Bone 0's
  record: offsets (504, 540, 576, 612, 648, 684). Channel 0's stream at
  21 276 + 504 is one block (valid 17, total 17) starting 3 634, 3 662,
  3 664...: position x at frame 0 = −3.8209 + 3 634 × 0.0039064 =
  10.3748. Rotation z at frame 0 = 0.6488 + 17 657 × 8.9e−5 ≈ 2.2195.
  Bone 36 (`Bone_AK47`)'s record at 21 708 is all zeros: the gun bone
  holds its defaults throughout.
- `reload` (91 frames), bone 5, rotation x (offset 3 204): blocks (valid
  1, total 27) holding 0, then (64, 64) holding 2, 11, 24, ..., 0. So
  frames 0-26 read 0 and frames 27-90 read the 64 values. Rotation y
  (offset 3 338): (1, 28) holding 0; (30, 32) holding 2, 6, 10, ..., 167,
  so frames 28-57 read those and frames 58-59 repeat 167; then (31, 31)
  for frames 60-90.

**Sampling between frames**: for a time t, frame position u = t × fps
over 0 to count − 1 (wrapped for looping sequences, clamped
otherwise); f = floor(u), s = u − f. Positions blend linearly between
frames f and f + 1; rotations are converted to quaternions at both
frames and blended by spherical interpolation. At f = count − 1 there is
no next frame: use frame f alone.

## Body parts, submodels, meshes

### Body part (76 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 64 bytes | name |
| 64 | i32 | submodel count |
| 68 | i32 | base |
| 72 | i32 | submodel offset |

An entity's `body` number picks one submodel per part: submodel index =
(body ÷ base) mod count (integer division). base is the product of the
earlier parts' counts. gign: part 0 `studio` (1 submodel), part 1
`backpack` (2: `blank` with no meshes, and the bomb backpack).

### Submodel (112 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 64 bytes | name |
| 64 | i32 | type (0) |
| 68 | f32 | bounding radius (0 here) |
| 72 | i32, i32 | mesh count, offset |
| 80 | i32, i32, i32 | vertex count, vertex bone table offset (one u8 per vertex), vertex offset (3 × f32 each) |
| 92 | i32, i32, i32 | normal count, normal bone table offset (u8 each), normal offset (3 × f32 each) |
| 104 | i32, i32 | group count, offset (unused: 0) |

Each vertex and normal is in its bone's frame: its model-space position
is the bone's world transform applied to it. v_ak47's `weapon` part,
submodel `f_ak47_template`: 258 vertices, 209 normals; vertex 0 is on
bone 36 at (0.41, 1.97, 3.48).

### Mesh (20 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | triangle count |
| 4 | i32 | triangle command offset |
| 8 | i32 | skin reference (index into the current skin family's row) |
| 12 | i32 | normal count |
| 16 | i32 | normal offset (unused) |

### Triangle commands

A list of runs, each starting with an i16 n: n > 0 is a triangle strip
of n vertices, n < 0 a triangle fan of −n vertices, n = 0 ends the
mesh's list. Each vertex is four i16: vertex index, normal index, s, t.
s and t are **texels** (0 to width/height), so the texture coordinate
is (s / width, t / height), t growing downward. A strip of n vertices
makes n − 2 triangles, alternating winding; a fan makes n − 2 triangles
around its first vertex.

Example: v_ak47 `weapon` mesh 0: 20 triangles, skin reference 3 →
texture `wood.bmp` 128 × 64; its first run is a strip of 8 starting at
vertex 0, normal 4, texel (126, 1). Counted over the submodel: 148
strips, 12 fans, 454 triangles. gign's body: one mesh of 740 triangles
in 178 strips and 31 fans.

## Textures and skins

### Texture (80 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 64 bytes | name (a source bitmap name, e.g. `view_glove.bmp`) |
| 64 | i32 | flags |
| 68 | i32 | width |
| 72 | i32 | height |
| 76 | i32 | pixel offset |

At the pixel offset: width × height palette indices, then **768 bytes of
palette directly** (no colour count, unlike WAD3). One mip level only.
v_ak47 texture 0 `view_glove.bmp` 256 × 128 at 57 304; the next texture
starts 32 768 + 768 = 33 536 bytes later, at 90 840.

| Flag | Meaning |
|---|---|
| 1 | flat-shaded (no smooth lighting) |
| 2 | chrome: texture coordinates come from the view direction and normal, not s/t |
| 4 | fullbright |
| 32 | additive |
| 64 | masked: index 255 transparent (alpha-tested) |

Seen: 0 (1 830), 3 = flat + chrome (90: the `Chrome*.bmp` textures of
gsg9, leet, sas and Half-Life models), 64 (27: foliage such as
`bush.mdl`, `palmtree.mdl`).

### Skins

The skin table is family count × reference count i16 texture indices: a
mesh's skin reference r in family k uses texture table[k][r]. gign: 1
family (0, 1); v_ak47: 1 family (0..10).

### External textures (`<name>T.mdl`)

When a model has no textures (72 in the install: Half-Life's monsters
and, in `cstrike/`, `arcticorange.mdl` and `orange.mdl`), they are in `<name>T.mdl` beside it (the T in any
case: `barneyt.mdl`, `arcticorangeT.mdl`): an `IDST` version 10 file
with no bones and the texture, pixel and skin fields filled in (its
offsets are relative to its own start). Example `valve/models/barneyt.mdl`:
26 textures at 244, pixels from 2 376, 26 references × 1 family.

## Attachments (88 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 32 bytes | name (empty in CS models) |
| 32 | i32 | type (0) |
| 36 | i32 | bone |
| 40 | 3 × f32 | position in the bone's frame |
| 52 | 9 × f32 | unused vectors |

v_ak47: attachment 0 on bone 20 (`Bone 04`) at (2.75, −22.5, 2.9), the
muzzle; attachment 1 on bone 41 at (0, −3, 0), the shell ejection.

## Edge cases

- Names (sequence, bone) may repeat (gign's `crouch_aim_dualpistols`
  twice); refer to sequences by index, keep the first match by name.
- Bone names may contain spaces and dashes (`-- L knuckle`).
- `blank` submodels have no meshes.
- A version 9 file exists (`valve/models/doctor.mdl`); reject anything
  but 10.
- Some old files have garbage in unused fields (bone flags, a
  sequence's blend fields in a version 9 file); never trust unused
  fields.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| gign.mdl | header | `IDST`, 10, length 2 329 328, 53 bones, 21 hitboxes, 111 sequences, 2 textures, 2 body parts |
| gign.mdl | hitbox 11 | bone 7 `Bip01 Head`, group 1, box (−0.33, −4.5, −2.5) to (8, 4.5, 2.5) |
| gign.mdl | sequence `run` | 60 fps, 37 frames, loops, motion 0x40, linear movement (150, 0, 0) |
| gign.mdl | sequence `ref_aim_ak47` | 9 blends, type 8, −90 to 90 |
| v_ak47.mdl | sequence 1 events | frames 13 and 57, event 5004, `weapons/ak47_clipout.wav`, `weapons/ak47_clipin.wav` |
| v_ak47.mdl | `shoot1` bone 0 frame 0 position x | 10.3748 |
| v_ak47.mdl | `reload` bone 5 rotation y stored values at frames 27, 28, 57, 58, 59 | 0, 2, 167, 167, 167 |
| v_ak47.mdl | weapon submodel triangle commands | 454 triangles |
| barney.mdl | sequence 18 | group 1, its animation read from `barney01.mdl` at 76 |
| barney.mdl | textures | none; `barneyt.mdl` has 26 |
| every v10 file in the install | parse all arrays | every offset within the file |

## Open questions

1. Normal smoothing and the original's model lighting (ambient and
   shade from the floor's lightmap, chrome) are presentation work, to be
   matched by captures.
2. CS 1.6's client picks among 9 aim blends by the view pitch; whether it
   blends the two neighbours or snaps is to be confirmed by captures.
3. The muzzle flash option code (scale and sprite digits) is read from
   public descriptions; confirm against a capture.
