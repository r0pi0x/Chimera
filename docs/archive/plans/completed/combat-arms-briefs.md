# Combat Arms: finished spec briefs (Waves 1-4)

Archived from [../active/combat-arms.md](../active/combat-arms.md) (the board,
decisions and Wave 5 stay there), unchanged. The briefs and follow-up
briefs of the finished waves: each item's spec has landed in
`specs/combat_arms/` and its implementation in `games/combat_arms/`, so these
are history and the source of the specs' questions, not work to do.

## How to run a spec wave (for the user)

1. On the Windows PC: `git pull` in the checkout (it must be clean).
2. Open one Claude Code session per board item in that checkout (several at
   once is fine: each writes only its own file).
3. Paste the item's brief below into each session.
4. When it asks, tell it in chat where the reference material is. Never
   write the path into a file or a commit.
5. Review each spec as it lands (main); then tell the coordinator, who sets
   the board item to "spec landed" and assigns an implementer.

## Spec briefs (Wave 1)

Each brief is complete on its own; paste it as is.

### 1. Player movement

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

Write specs/combat_arms/movement.md: the local player's movement. Cover walk,
run, sprint and stamina (drain, regen, thresholds, what blocks sprinting),
jump (impulse, gravity, air control, repeat delay), crouch (heights, speeds,
transition time, crouch-jump), falling and fall damage (thresholds, formula),
ground acceleration and friction, stopping, slopes and steps, speed limits by
stance and by weapon held, and the tick/frame rate dependence.

Describe behaviour in your own words: exact constants with units (say the
distance unit and up axis), formulas as math, per-tick order, clamps, edge
cases, quirks players rely on. Never paste code or translate it line by line;
no fenced code blocks except ```math. Name files you consulted only by
their in-game role (e.g. "the client's player movement"), never by path.
End with a numbered list of concrete test cases (setup, input, ticks ->
expected numbers) that an implementer without the game can check, and an
"Open questions" list.

Write only specs/combat_arms/movement.md; change nothing else, so parallel
sessions don't conflict. Commit with the repo's identity (CLAUDE.md "Commit
identity"; check `git config user.name` is r0pi0x, else stop and ask me) in
UTC: in PowerShell run `$env:TZ="UTC"` before `git commit`. End the message
with "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>". Never skip
hooks. Then `git pull --rebase` and `git push` to main.
```

### 2. Weapon firing

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

Write specs/combat_arms/weapon_firing.md: how a gun fires. Cover the trigger
(fire modes: auto, semi, burst; cycle time; what happens when held), hitscan
(ray origin, range, penetration if any), damage by range (formula and curve),
hit zones (which body parts, multipliers, armour), spread (base, by stance
and movement, growth per shot, recovery), recoil (view kick pattern, return),
reload (timings, partial reload, cancel), ammo (clip, reserve, empty
behaviour), and the order of all of this within a tick. Use one common rifle
as the worked example with its numbers; say which values come from the
weapon's attribute data and which are fixed in code.

Describe behaviour in your own words: exact constants with units, formulas as
math, per-tick order, clamps, edge cases, quirks players rely on. Never paste
code or translate it line by line; no fenced code blocks except ```math. Name
files only by their in-game role, never by path. End with a numbered
list of concrete test cases (setup, input -> expected numbers, e.g. damage at
given distances and zones, shots until spread caps) that an implementer
without the game can check, and an "Open questions" list.

Write only specs/combat_arms/weapon_firing.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 3. Weapon and movement data format

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

Write specs/combat_arms/attribute_data.md: the format of the game's attribute
data that holds weapon, ammo, movement and player values, so we can load the
real values at run time (we ship our own defaults; the install's data
overrides them). Cover where the data sits in the archives (archive and
folder by role), the file format and encoding, the record structure
(categories, records, fields, types, defaults, inheritance or links between
records), how the game looks values up, and a field table for weapons, ammo
and movement: field name as stored in the data (data names are fine), type,
unit, meaning, and which behaviour reads it. Do not include encryption keys.

Describe it in your own words: exact sizes and encodings, edge cases
(missing fields, overrides). Never paste code or translate it line by line;
no fenced code blocks except ```math. Name files only by their in-game
role, never by path. End with a numbered list of concrete test cases (a small
document or byte layout described in a table -> the values a reader must
produce) that an implementer can turn into generated fixture files, and an
"Open questions" list.

Write only specs/combat_arms/attribute_data.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 4. Inventory and switching

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

Write specs/combat_arms/inventory.md: what a player carries and how they
switch. Cover the slots (primary, secondary, melee, grenade, special; what
each holds and limits), default key bindings and the last-weapon and scroll
behaviour, draw and holster times and what may interrupt them (firing,
reloading, sprinting), the knife's primary and secondary attacks (damage,
range, timing, backstab if any), quick-knife or quick-throw if present,
pickup and drop (which items, how, what happens to ammo, what is dropped on
death), and the order of these within a tick.

Describe behaviour in your own words: exact constants with units, formulas as
math, edge cases (switching mid-reload, empty slots, dropping the last gun),
quirks players rely on. Never paste code or translate it line by line; no
fenced code blocks except ```math. Name files only by their in-game
role, never by path. End with a numbered list of concrete test cases (setup,
input, time -> expected weapon in hand, ammo, damage) that an implementer
without the game can check, and an "Open questions" list.

Write only specs/combat_arms/inventory.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 5. Crosshair and basic HUD

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

Write specs/combat_arms/hud_crosshair.md: the crosshair and the basic in-game
HUD. Crosshair: shape, size and gap in screen units at a reference
resolution, how it widens and recovers with spread, movement and firing, how
it changes when scoped or when aiming at an enemy or teammate, hit
indicators. HUD: health, armour if any, ammo (clip and reserve), weapon
name/icon, stamina, damage direction indicator, kill feed, timer and score;
for each: position, anchoring and scaling with resolution, what it shows,
when it updates, colours and thresholds (e.g. low health, low ammo).

Describe behaviour in your own words: exact numbers with units, formulas as
math, edge cases, quirks players rely on. Never paste code or translate it
line by line; no fenced code blocks except ```math. Name files only by
their in-game role, never by path. Do not include art; name a
widget's texture only by its role. End with a numbered
list of concrete test cases (state -> expected size, position, text or
colour) that an implementer without the game can check, and an "Open
questions" list.

Write only specs/combat_arms/hud_crosshair.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

## How to run Wave 2

Same as Wave 1 ("How to run a spec wave"): `git pull` in a clean checkout
on the Windows PC, then paste each brief below into its own session, or
into one session that hands each brief to a subagent while the main
session alone does the git work (commit each file, `git pull --rebase`,
`git push`). Each brief writes only its own file. Items 9 and 10 are for
the LithTech loader layer (`lithtech/formats`, "LithTech loader layer"
above): their byte layouts must be exact enough to write a reader from,
and their test cases are sizes, counts and offsets in the user's install,
which this box checks in `heavy::` tests against its own copy of the same
Steam build. The follow-up brief at the end ("Follow-up brief (Wave 1 open
questions)") can run beside them: it edits only the Wave 1 specs.

## Spec briefs (Wave 2)

Each brief is complete on its own; paste it as is.

### 6. Grenades

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/inventory.md and weapon_firing.md first: this spec
continues them (the grenade slot, quick-throw, the cooked grenade released
on death) and must not contradict them; where it does, say so in "Open
questions".

Write specs/combat_arms/grenades.md: every thrown weapon. Cover each kind
(fragmentation, flash, smoke, and any other: sticky, impact, gas...): pin
pull and cooking (fuse start, fuse length, what happens when held past it),
the throw (origin, launch speed and angle for the primary and secondary
throw, how the thrower's velocity adds, release delay after the button),
the flight (gravity, drag, bounce and friction on surfaces, rolling,
coming to rest, water), detonation (when, which traces decide who is
affected), damage (radius, falloff formula, walls and cover, self and team
damage, armour), flash (who is blinded by angle and distance, duration and
fade, the screen effect's strength over time, deafening), smoke (when it
starts, growth, lifetime, size, what it blocks: sight, bots, name tags),
carrying limits and how many of each a player starts with, and the order
of all of this within a tick. Give the numbers of one grenade of each kind
from the attribute data and say which values come from data and which are
fixed in code.

Describe behaviour in your own words: exact constants with units (say the
distance unit and up axis), formulas as math, per-tick order, clamps, edge
cases (throwing into a wall at point blank, cooking to zero, dying while
cooking, switching away mid-throw), quirks players rely on. Never paste
code or translate it line by line; no fenced code blocks except ```math.
Name files only by their in-game role, never by path. End with a numbered
list of concrete test cases (setup, input, time -> expected position,
damage, flash amount) that an implementer without the game can check, and
an "Open questions" list.

Write only specs/combat_arms/grenades.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 7. Weapon classes

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/weapon_firing.md, inventory.md and
attribute_data.md first: weapon_firing.md describes one rifle; this spec
says only what differs per class, and how the attribute data marks a
weapon's class.

Write specs/combat_arms/weapon_classes.md: the differences between assault
rifles, SMGs, sniper rifles (and their scope), shotguns, machine guns (LMG)
and pistols, plus any other class the game has (launchers, for example).
For each class: which firing rules from weapon_firing.md change (spread,
recoil, movement penalty, fire modes, damage falloff, penetration), with
the numbers of one typical weapon of that class. Scope: zoom levels and
field of view per level, how zoom is entered and left (key, toggle or
hold, after a shot, on reload or switch), unscoped and scoped accuracy,
the time to scope in, scoped movement speed, sway or breathing if any, the
scope overlay's layout by role (no art), the mouse sensitivity scale while
zoomed. Shotguns: pellet count, pattern (random or fixed), per-pellet
damage and falloff, shell-by-shell reload and its interruption. Machine
guns: deploying or a bipod if any, spin-up, overheating. Pistols: the
semi-auto rate limit. Bolt-action or pump cycling between shots and what
interrupts it.

Describe behaviour in your own words: exact constants with units, formulas
as math, per-tick order, clamps, edge cases (reloading while scoped,
switching while zoomed, firing during a bolt cycle), quirks players rely on
(quick-scoping, no-scope accuracy). Never paste code or translate it line
by line; no fenced code blocks except ```math. Name files only by their
in-game role, never by path. End with a numbered list of concrete test
cases (setup, input, time -> expected field of view, spread, damage, pellet
hits) that an implementer without the game can check, and an "Open
questions" list.

Write only specs/combat_arms/weapon_classes.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 8. Attachments

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/weapon_firing.md, attribute_data.md and
hud_crosshair.md first: attachments change their numbers and the HUD.

Write specs/combat_arms/attachments.md: weapon attachments and
modifications. Cover the kinds (sights: red dot, holographic, scopes;
muzzle: silencer, compensator, flash hider; grips, stocks, extended
magazines, and any other), which weapons accept which (by the data's
slots or lists), how a weapon with attachments is described in the data
(which records link where), and for each kind exactly what it changes: the
values of weapon_firing.md and weapon_classes.md it scales or replaces
(damage, range, spread, recoil, movement penalty, clip size, reload and
draw times, zoom), with the formula (added, multiplied or replaced, and in
which order when several apply), plus what it changes in sound (the
suppressed shot, hearing range), in the crosshair or a sight overlay, in
the kill feed, and on the view model. Say when the changes apply (at
spawn, on equip, live) and whether the server or the client computes them.

Describe behaviour in your own words: exact constants with units, formulas
as math, edge cases (two attachments in the same slot, an attachment on a
weapon that doesn't list it, a picked-up weapon's attachments), quirks
players rely on. Never paste code or translate it line by line; no fenced
code blocks except ```math. Name files only by their in-game role, never by
path. End with a numbered list of concrete test cases (weapon plus
attachments -> expected numbers: damage at a distance, spread, clip, zoom)
that an implementer without the game can check, and an "Open questions"
list.

Write only specs/combat_arms/attachments.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 9. LithTech world format (loader layer)

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
This spec is for the LithTech loader layer (`lithtech/formats`, see
"LithTech loader layer" in docs/plans/active/combat-arms.md). Our reader of
the game's `.rez` archives already exists (specs/combat_arms/rez_archive.md),
so this spec starts at a world file's bytes as they come out of an archive.

Write specs/combat_arms/world_dat.md: the binary layout of the compiled map
file (the world `.dat`), exact enough to write a reader from it alone.
Cover, byte by byte: the header (magic or version, every offset and count,
endianness, alignment and padding), the world info string and extents, the
geometry (world models and their nodes, planes, polygons, vertices, surfaces
and their flags, texture names and texture coordinates), the render data
(render blocks or sections, vertex formats, index lists, which polygons are
drawn and which are collision only), the lightmaps (where they sit, their
size, pixel format and compression, how a polygon's texture coordinates map
into them, light groups if any), the collision and visibility data (the
physics tree versus the render tree, portals or sectors, how a point is
located), the objects (the object list: class name, property list, each
property type's encoding: string, vector, colour, real, boolean, long,
rotation and any other; how spawn points, lights, sounds, doors, pickups
and the sky are expressed as objects and properties), the sky portal and
sky objects, and anything else in the file (blind object data, terrain,
particle blockers). For each block say how a reader finds it and how big
it is. State which file version or versions Combat Arms' install uses,
whether every map in the install has the same one, and how a reader tells
other LithTech versions apart only as far as needed to reject a file this
spec doesn't cover. Coordinates: say the unit, the axes and handedness,
and how to convert to a right-handed Y-up frame.

Describe it in your own words: tables of fields with offset, size, type and
meaning; formulas as math; no fenced code blocks except ```math. Never
paste code or translate it line by line, and never paste bytes from game
files beyond short magic or version values. Name files only by their
in-game role; a map may be named by its in-game name and its file name
inside the game's archives, never by a machine path. End with a numbered
list of concrete test cases that the coordinator's Linux box can check
against its own copy of the install (say the Steam build id you measured):
for two or three maps, the file's size, its version, each block's offset
and count, the numbers of world models, polygons, vertices, lightmaps and
objects, the count of objects of a few classes (spawn points per team), and
one object's decoded properties. Add small synthetic layouts in tables (a
minimal valid file) that a reader's unit tests can generate as fixtures.
End with an "Open questions" list.

Write only specs/combat_arms/world_dat.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 10. Models, textures and animation (loader layer)

Three briefs, one file each (10a, 10b, 10c), so three sessions can run at
once; 10c refers to 10a's file but can run beside it.

#### 10a. Models

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
This spec is for the LithTech loader layer (`lithtech/formats`, see
"LithTech loader layer" in docs/plans/active/combat-arms.md). Our reader of
the game's `.rez` archives already exists (specs/combat_arms/rez_archive.md),
so this spec starts at a model file's bytes as they come out of an archive.
Animation data inside the model file is specs/combat_arms/animation.md's
(another session): here, say only where it sits and how big it is.

Write specs/combat_arms/models_ltb.md: the binary layout of the game's
model files (`.ltb`, and any other model format the install uses), exact
enough to write a reader from it alone. Cover, byte by byte: the header
(magic or version, file type, every offset and count, endianness), any
compression of the whole file, the pieces and their levels of detail (how
a level is chosen by distance), the mesh data of each piece type (rigid,
skinned, vertex-animated): vertex formats with each attribute's encoding
(position, normal, texture coordinates, colour, bone weights and indices),
index lists, triangle strips or lists; the skeleton (node names, parent
links, bind transforms and their matrix layout); sockets (names, parent
node, offset); how a piece names its textures and render styles; hit or
collision boxes; the model's bounds and dimensions (the stand and crouch
boxes the movement spec needs); and child models. State which file version
or versions Combat Arms' install uses and whether all models share it.
Coordinates: say the unit, axes and handedness, and how to convert to a
right-handed Y-up frame.

Describe it in your own words: tables of fields with offset, size, type and
meaning; formulas as math; no fenced code blocks except ```math. Never
paste code or translate it line by line, and never paste bytes from game
files beyond short magic or version values. Name files only by their
in-game role (e.g. "the default male character", "the M4A1's first-person
model") or by their file name inside the game's archives, never by a
machine path. End with a numbered list of concrete test cases that the
coordinator's Linux box can check against its own copy of the install (say
the Steam build id you measured): for a character, a first-person weapon
and a world prop, the file's size, version, piece and level counts, vertex
and triangle counts per piece, node count and three node names with their
parents, socket names, and the bounds. Add a small synthetic layout in a
table (a minimal valid model) that a reader's unit tests can generate as a
fixture. End with an "Open questions" list.

Write only specs/combat_arms/models_ltb.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

#### 10b. Textures

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
This spec is for the LithTech loader layer (`lithtech/formats`, see
"LithTech loader layer" in docs/plans/active/combat-arms.md). Our reader of
the game's `.rez` archives already exists (specs/combat_arms/rez_archive.md),
so this spec starts at a texture file's bytes as they come out of an
archive.

Write specs/combat_arms/textures_dtx.md: the binary layout of the game's
texture files (`.dtx`), exact enough to write a reader from it alone.
Cover, byte by byte: the header (magic or resource type, version, width,
height, mipmap count, every flag and what it means, the format code, the
texture group, any text fields such as a command string and what the game
does with them), every pixel format the install uses (palettised, 32-bit,
the compressed block formats, and which code maps to which), how mipmap
levels follow each other (size of each level, alignment), cube maps or
other multi-image layouts if used, any section after the pixels, and how
the game turns a texture's flags into render behaviour (alpha test or
blend, full-bright, detail textures, wrapping or clamping). Also say how
other files refer to a texture (by archive file name, with or without
extension) and what the game does when one is missing. State which file
version or versions Combat Arms' install uses.

Describe it in your own words: tables of fields with offset, size, type and
meaning; formulas as math; no fenced code blocks except ```math. Never
paste code or translate it line by line, and never paste bytes from game
files beyond short magic or version values. Name files only by their
in-game role or their file name inside the game's archives, never by a
machine path. End with a numbered list of concrete test cases that the
coordinator's Linux box can check against its own copy of the install (say
the Steam build id you measured): for four or five textures of different
formats (a world surface, a model skin, a HUD element, one with alpha), the
file's size, version, width, height, format, mipmap count and flags, and
the offset where each mip level starts. Add small synthetic layouts in
tables (a 4x4 texture of each format with its decoded pixels) that a
reader's unit tests can generate as fixtures. End with an "Open questions"
list.

Write only specs/combat_arms/textures_dtx.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

#### 10c. Animation

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
This spec is for the LithTech loader layer (`lithtech/formats`, see
"LithTech loader layer" in docs/plans/active/combat-arms.md). The model
file's layout is specs/combat_arms/models_ltb.md (another session, maybe
running at the same time): if it has landed, `git pull` and refer to it;
otherwise describe the animation data's place in the model file only as
far as you need, and leave the rest to that spec.

Write specs/combat_arms/animation.md: the game's skeletal animation, as
data and as playback. Data, byte by byte: where animations are stored (in
the model file, in separate files or child models), the animation list
(names, lengths, keyframe times or rates, dimensions, the interpolation
time into each animation), the keyframe encoding for each node (position
and rotation formats, compression and quantisation, how a missing key is
filled), the keys' string events (sounds, footsteps, fire points) and how
the game reads them, and vertex animation if any. Playback: time base and
rate, looping, blending between animations and the blend time, layered
playback (several animations on parts of the skeleton at once: legs,
torso, weapon), the order nodes are evaluated in, how a skinned vertex is
computed from weights and transforms (as math), and how the first-person
weapon's animations are chosen (draw, idle, fire, reload, holster: which
names, and which spec values such as draw times come from animation
lengths). State which file version or versions Combat Arms' install uses.

Describe it in your own words: tables of fields with offset, size, type and
meaning; formulas as math; no fenced code blocks except ```math. Never
paste code or translate it line by line, and never paste bytes from game
files beyond short magic or version values. Name files only by their
in-game role or their file name inside the game's archives, never by a
machine path. End with a numbered list of concrete test cases that the
coordinator's Linux box can check against its own copy of the install (say
the Steam build id you measured): for the default character and one
first-person weapon, the number of animations, a few animations' names,
lengths and key counts, one node's decoded rotation and position at a given
time, and the string events of one animation with their times. Add a small
synthetic layout in a table (one node, two keys) with the interpolated
values a reader must produce. End with an "Open questions" list.

Write only specs/combat_arms/animation.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 11. UI system

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/hud_crosshair.md first: its open questions on font
scaling and text alignment (2 and 3) are this spec's to settle.

Write specs/combat_arms/ui_system.md: how the game builds and lays out its
HUD and menus. Cover the layout data (which files describe screens, panels
and widgets: their format and encoding, element types, every attribute and
its meaning: position, size, anchor, alignment, texture by role, colour,
font, text or string-table id, visibility rules, child elements), the
coordinate system and reference resolution, how positions, sizes and fonts
scale and anchor at other resolutions and aspect ratios (as math), fonts
(how a font is chosen and sized, how text width is measured, wrapping and
clipping), the string tables (format, how an id resolves, language
selection), how a screen is opened, closed and stacked, input focus and
mouse and keyboard navigation, the sounds widgets play, and which screens
exist in a match (HUD, scoreboard, team and loadout or buy screens, pause
or options, chat, the end-of-match screen) with the layout data that
drives each. Say which parts are data and which are fixed in code, so we
can decide whether to read the game's layouts or rebuild them as our data.

Describe it in your own words: exact numbers with units, formulas as math,
edge cases (a missing string id, a widget off screen at a narrow aspect),
quirks players rely on. Never paste code or translate it line by line; no
fenced code blocks except ```math. Name files only by their in-game role,
never by path; no art (name a texture only by its role). End with a
numbered list of concrete test cases (a small layout described in a table,
plus a resolution -> each element's pixel rectangle and text) that an
implementer without the game can check, and an "Open questions" list.

Write only specs/combat_arms/ui_system.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

## Follow-up brief (Wave 1 open questions)

Answered 2026-10-18 (the specs' "Answered:" entries, cases movement
37-55, weapon_firing 38-49, inventory 37-44, hud_crosshair 71-77,
attribute_data 30-32) and implemented on `worktree-agent-a87fdd83de43f5b4b`:
each module's Known shortcuts now lists only what is still ours. Answers
that need a live measurement are sitting 5 of
[windows-captures.md](../active/windows-captures.md).

Questions the Wave 1 implementers answered for themselves (each module's
`//!` "Known shortcuts": `games/combat_arms/src/game/movement.rs` and
`inventory.rs` on main, `weapons.rs` on `ca/weapon-firing`, `ui/hud.rs`
on the HUD agent's branch; `data.rs` has none yet) plus the coordinator's
list. Paste the brief below into one session; the implementers then
change their code to the answers.

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

This is a follow-up session. The Wave 1 specs in specs/combat_arms/
(movement.md, weapon_firing.md, attribute_data.md, inventory.md,
hud_crosshair.md) have been implemented, and the implementers had to pick
a reading where the spec was silent or ambiguous. Answer the numbered
questions below from the reference material. For each one, in the spec
named after it:

- add the question to that spec's "Open questions" section if it isn't
  there (or find the existing one), and mark it "Answered:" with the answer
  in your own words, the constants and formulas it needs, and where in the
  spec's body it applies;
- where the answer changes or completes the behaviour, fix the body text
  too (sections, constants table, per-tick order), not only the question;
- add numbered test cases after the existing ones (never renumber or
  remove existing cases; if one is wrong, say so in it and add the
  corrected case), with concrete numbers an implementer without the game
  can check;
- if the reference can't settle it, say what you checked and give a short
  live-game measurement that would (setup, action, what to read off),
  and leave the question open.

The questions (Q = an existing "Open questions" number in that spec, case =
a test case number):

movement.md
1. Jump factor and crouched air acceleration: the Acceleration section
   scales the acceleration by the jump factor j when j < 1, but test case
   20's "+2.25 u/s per tick" for crouching in the air after a jump fits
   j = 1. Does j scale the crouched in-jump air acceleration (150 u/s²)?
   Which j does case 20 assume, at which tick of the jump?
2. The moving-start rescale to 500 u/s²: does it apply when crouch is held
   (base acceleration halved to 1500) and when the start ramp is bypassed?
   Give the first three ticks' speeds from rest, crouched, holding forward.
3. Crouch direction reversal: when crouch is released before the crouch
   transition finishes (or pressed again while standing up), does the
   other direction's elapsed time restart at zero, continue, or start from
   the mirrored remainder? Give heights per tick for a release halfway.
4. Standing up on the ground: does the hull keep its feet (grow upward),
   as the spec says for shrinking? And Q5 (standing up in the air): grow
   around the centre and then push up, or fail and force?
5. Steps and slopes: exactly what counts as a step (when is a step-up
   tried: which hit, which height, which normal threshold) and how is the
   downhill projection or ground snap done (how far down, refused when?
   e.g. a drop that meets only an edge beside the hull)?
6. Falling after an air spawn: for a player spawned or teleported in the
   air, where is the fall height for fall damage measured from (the
   spawn point, the highest point, the first airborne tick)?
7. Walk key versus +speed: is walking a separate key from sprint? What
   happens with both held, and with opposing keys held (the spec's
   zero-acceleration "moving" state)?
8. The frame accumulator (at most 6 ticks per frame, the rest dropped):
   do stamina and the crouch release delay run per tick or per rendered
   frame? Is the hull resized once or twice per tick (the second resize)?
   Does sprint take effect at once or only after the server's reply?
9. Eye height (Q2): the eye's height above the feet standing and
   crouched, and how it moves during the crouch transition. Inventory's
   pickup ray and weapon_firing's shot origin start there (we assume 0.9
   of the hull's height). If it belongs to a camera spec, give the
   numbers here anyway.
10. Unit scale (Q3): anything in the reference tying world units to real
    size (1 u = 1 cm?), e.g. a sound or physics constant in metres.

inventory.md
11. One clock or two: inventory.md runs the weapon logic once per rendered
    frame, movement.md runs on a 15 ms tick. Does the weapon logic (draw,
    holster, knife swing, pickup) run per frame or per movement tick, and
    at what rate does the server run it? What does "the next update" mean
    for the knife's first hit (155 ms "plus 2 or 3 frames")?
12. The pickup fallback (camera-to-centre distance − 35 ≤ 100): does it
    apply only to an item the view ray hits beyond 100 units, or to any
    item within reach (about 135 units from the camera) even if the ray
    misses it? What box does an item without a model use for the ray?
13. Spent grenades: when the last grenade is thrown, does the grenade stay
    in the inventory at total 0 (unselectable), and what do Q, the grenade
    key, the HUD and a respawn do with it?
14. The "better weapon" rule (Q6): after a pickup, when exactly does the
    server order a switch (command-id comparison, "did not already have
    it", held weapon or empty hands)? A test case each for the switch and
    the no-switch outcome.
15. The knife's hit: does the knife ray use the M9's perturb (spread) and
    weapon_firing.md's hit-zone multipliers, or neither?
16. Where dropped items appear: the drop origin (a foot, the hand, the
    eye) and its offset from the hull without a skeleton, for a death drop
    and a deliberate drop.

weapon_firing.md
17. Spread recovery: does recovery wait while the time since the last shot
    is at most DecDelay or strictly less (case 16's 95 ms)?
18. "Airborne" for spread: from leaving the ground after a jump until
    landing only, or any time off the ground (falls too)? How does the
    airborne spread rise grow per step?
19. The run flag and the speed in the movement term: is "running" simply
    "not walking" (true at rest)? Does the stance's maximum speed in the
    movement term include the weapon's movement penalty?
20. The turn term: does the view change it measures include the recoil
    kick, and how is one frame's turn split across spread sub-steps?
21. A second victim (Q5): when one bullet passes through a character, is
    the second victim's damage banded by distance again from the already
    banded value, or from the base damage?
22. The fire delay: does it count from the previous shot fired, or from
    the previous round received even if dropped or refused?
23. Armour: does armour protect against bullets only, and is the groin
    counted as torso for armour?

hud_crosshair.md
24. The HUD clock: do the HUD's timers (hit indicator, kill feed life,
    crosshair kick) run on the rendered-frame clock in milliseconds?
25. The crosshair gap: drawn from the shot cone after its whole-unit
    rounding, or from the unrounded spread value?
26. "Enemy" for the hit indicator and name tags in a mode without teams:
    everyone else?
27. Name tags: their height above the character (which socket or offset)
    and the distance beyond which they hide (Q10).
28. The weapon panel's class and fire-mode icon: which attribute data
    field gives a weapon's class (assault rifle, pistol...) and whether it
    has selectable fire modes? Put the field in attribute_data.md too.

attribute_data.md
29. DamageMovePenalty (Q3): a percentage of speed or an absolute speed
    loss, which behaviour applies it (movement.md or weapon_firing.md),
    and for how long; give a test case with numbers.

Write only these files: specs/combat_arms/{movement,weapon_firing,
attribute_data,inventory,hud_crosshair}.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

## How to run Wave 3

Same as Waves 1 and 2: `git pull` in a clean checkout on the Windows PC,
then paste each brief below into its own session, or into one session that
hands each brief to a subagent while the main session alone does the git
work. Each brief writes only its own file. Board numbers are kept: 12, 13
and 14 were already on the board (15 is the network part, which has no
spec), so view models are 16 and effects and sounds 17. Items 12 and 16
build on the loader specs of item 10 (`models_ltb.md`, `animation.md`):
pull first so they can refer to them. The follow-up brief at the end
("Follow-up brief (Wave 2 open questions)") can run beside them: it edits
only Wave 2 specs.

## Spec briefs (Wave 3)

Each brief is complete on its own; paste it as is.

### 12. Characters

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/models_ltb.md, animation.md, weapon_firing.md
(its hit spheres and "Open questions" 9), inventory.md (drops) and
hud_crosshair.md (name tags) first, if they have landed (`git pull`): this
spec builds on the model and animation layouts and must not contradict
the gameplay specs; where it does, say so in "Open questions".

Write specs/combat_arms/characters.md: the player characters as the game
builds, dresses and animates them. Cover: which character models exist and
how a player's model is chosen (team, gender or body type, the figure type
the hit-sphere tables key on, the default when the choice is missing);
gear and skins (how headgear, vests, backpacks and other gear swap pieces
or textures, how a skin is applied, what is data and where the data lives
by role); the rigging (the skeleton's nodes by role, and every socket the
game uses: the hand that holds the weapon, back and holster sockets for
carried weapons, the name tag's anchor, where dropped items and thrown
grenades leave from, the eye or camera node), with each socket's parent
node and offset; third-person animation selection: for each state (stance:
stand, crouch; movement: idle, walk, run, sprint, strafe, backwards, jump,
fall, land; weapon class held; firing, reloading, drawing, switching,
throwing a grenade, knifing; hit reactions; dying) which animation plays,
how the name is built (prefixes, suffixes per weapon class), how upper
and lower body are layered, aim pitch and yaw (blend or bone twist, the
limits), how ground speed drives the playback rate, and blend times
between them; death (a death animation chosen by what, a ragdoll or not,
how long the body stays, what happens to the weapon); and hit-sphere
placement: for each sphere of weapon_firing.md's tables, which skeleton
node it sits on and its offset from it, so the spheres follow the animated
bones, for the male and the female skeleton.

Describe behaviour in your own words: exact constants with units (say the
distance unit and up axis), formulas as math, per-tick order (when the
animation state is decided relative to movement and firing; server or
client), edge cases (switching weapons mid-reload, dying while crouched or
in the air, a model missing a socket), quirks players rely on (a hit
sphere that sticks out of the body, a crouch animation that lowers the
head more than the hull). Animation and node names as stored in the model
files are data and may be named; never paste code or translate it line by
line; no fenced code blocks except ```math. Name files only by their
in-game role or their file name inside the game's archives, never by a
machine path. End with a numbered list of concrete test cases that an
implementer can check (state -> expected animation name and rate; node ->
sphere centre in model space in the bind pose and in one animation at a
given time; socket -> parent and offset), the ones that need the install
marked so this box can check them against its own copy (say the Steam
build id you measured), and an "Open questions" list.

Write only specs/combat_arms/characters.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 13. Game modes

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/inventory.md (death drops), grenades.md (its
"Open questions" 14, the infection mode's knock-back), hud_crosshair.md
(timer and score) and world_dat.md ("Open questions" 3, the spawn point
objects) first: this spec continues them and must not contradict them;
where it does, say so in "Open questions".

Write specs/combat_arms/game_modes.md: the match rules of every game mode,
each mode in its own top-level section so they can be implemented one by
one, Team Death Match first, then Elimination, Search & Destroy, Capture
the Flag, and every other mode the reference has (say which ones you found
and give each its own section, even a short one). Start with what all
modes share: teams (how many, names, how a player joins or is assigned,
balancing, switching team mid-match), the spawn (which spawn objects of a
map a mode uses and by which properties, how one is chosen: random,
farthest from enemies, blocked when occupied; spawn protection: length,
what ends it early, what it blocks, how it shows), respawn (delay, by key
or automatic, what the player respawns with), the match clock and
rounds, score and kill counting (kills, deaths, assists, suicides, team
kills, the points each gives), friendly fire, the end of a round and of
the match (limits, ties, overtime or sudden death), what happens to
dropped weapons and live grenades at a round's end, the data the
scoreboard shows per player and per team, and the server's mode settings
(each setting by its name as the game exposes it, type, range, default,
and which rule reads it). Then, per mode, only what differs: its win
conditions, round structure, objectives (bomb sites, flags, their
objects in the map, carry and drop rules, plant and defuse times, capture
and return rules), respawn or not, and its scoring.

Describe behaviour in your own words: exact constants with units, formulas
as math, per-tick order (when deaths are counted against the round's end,
the order of a round's end, score and respawn), server or client, edge
cases (the last player of a team leaving, two objectives finishing on the
same tick, killing yourself with a grenade after the round ends), quirks
players rely on. Setting names, mode names and object class names the
game exposes are data and may be named; never paste code or translate it
line by line; no fenced code blocks except ```math. Name files only by
their in-game role, never by path. End with a numbered list of concrete
test cases, grouped by mode (setup: settings, players, time; events ->
expected score, time, winner, spawn chosen, respawn time), that an
implementer without the game can check, and an "Open questions" list.

Write only specs/combat_arms/game_modes.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 14. Bots

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/movement.md, weapon_firing.md, inventory.md and
grenades.md first (grenades.md already points to bots.md: a resting
grenade announced as danger, bots not blinded, smoke not blocking bot
sight): bots move, aim and fire through those rules and this spec must
not contradict them; where it does, say so in "Open questions".

Write specs/combat_arms/bots.md: computer-controlled players. First say
whether the game has bots or AI opponents at all, in which modes (for
example a training or AI mode, or enemies in a co-op mode) and how they
are added (server setting, menu, count, names). If it has none, say so,
list what the reference does have that comes closest, and stop there with
"Open questions". Otherwise cover: navigation (what the bots move on: a
navigation mesh or waypoints, how it is built or stored in the map's
files, how a path is found and its costs, how they handle doors, ladders,
jumps, drops and getting stuck), perception (view cone and distance, how
sight is tested, hearing of shots, steps and grenades, memory of a target
after losing sight), target selection (scores by distance, threat, angle,
health), aim and reaction (reaction time, aim speed and error as
formulas, where they aim on the body, how the error shrinks while
tracking), weapon use (which weapon at which range, firing in bursts or
taps, reloading, switching, knifing, throwing grenades: when and how they
aim the throw), movement in a fight (strafing, crouching, taking cover,
retreating at low health), team behaviour and objectives per game mode,
and difficulty settings (each level's values for the numbers above, and
how the level is chosen).

Describe behaviour in your own words: exact constants with units, formulas
as math, the order of a bot's think within a tick and how often it
thinks, randomness (distributions and ranges), edge cases (no path, a
target behind smoke, a teammate in the line of fire), quirks players rely
on. Never paste code or translate it line by line; no fenced code blocks
except ```math. Name files only by their in-game role, never by path.
End with a numbered list of concrete test cases (setup: bot difficulty,
target position and motion; time -> expected reaction delay, aim error
range, weapon chosen, path taken) that an implementer without the game
can check, and an "Open questions" list.

Write only specs/combat_arms/bots.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 16. View models

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/animation.md, models_ltb.md, inventory.md,
weapon_firing.md, weapon_classes.md, grenades.md and attachments.md first,
if they have landed (`git pull`): their draw, holster, reload, fire and
throw timings must line up with the view model's animations; where they
don't, say so in "Open questions" (grenades.md's "Open questions" 3 and
5, the throw animation's release time and the re-draw, are this spec's
to settle if the reference allows).

Write specs/combat_arms/view_models.md: the first-person weapon as the
player sees it. Cover: which model is drawn (the weapon's first-person
model, the arms or hands model, how they combine: one model, a child
model, or arms attached to the weapon's nodes; how sleeves or gloves
follow the player's character or team; how attachments show); animation
selection: for each action (draw, holster, idle and its variations, fire
per fire mode and the last shot, dry fire, reload full and empty and
shell by shell, sprint start, loop and end, inspect if any, knife swings,
grenade pin pull and throw, scope in and out) which animation plays,
how the name is found, its length and which gameplay timing it sets or
must match (inventory.md's draw and holster, weapon_firing.md's fire
delay and reload, grenades.md's throw), animation key events (sounds,
shell ejection, muzzle flash) and their times; bob and sway (per stance
and movement speed, as formulas with constants, how turning and landing
move the model, what zooming does to it); the field of view of the view
model and of the world, and any per-weapon offset; the sockets or nodes
for the muzzle flash and the shell ejection and how the effect is placed
there; and the per-frame order (when the view model's pose is decided
relative to the camera, firing and recoil).

Describe behaviour in your own words: exact constants with units (say the
distance unit and axes), formulas as math, frame or tick dependence, edge
cases (firing during the draw, sprinting mid-reload, switching while
zoomed, the last round), quirks players rely on (an animation cancel that
shortens a reload, a bob that hides the crosshair). Animation, node and
socket names as stored in the model files are data and may be named;
never paste code or translate it line by line; no fenced code blocks
except ```math. Name files only by their in-game role or their file name
inside the game's archives, never by a machine path. End with a numbered
list of concrete test cases (weapon, action, time -> expected animation
and frame time, bob offset, field of view, muzzle socket position), the
ones that need the install marked so this box can check them against its
own copy (say the Steam build id you measured), and an "Open questions"
list.

Write only specs/combat_arms/view_models.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main.
```

### 17. Effects and sounds

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/weapon_firing.md (surfaces), grenades.md (its
"Open questions" 10, the smoke's emission), movement.md (footsteps),
attachments.md (the suppressed shot) and world_dat.md (surface flags)
first: this spec gives their events a look and a sound and must not
contradict them; where it does, say so in "Open questions".

Write specs/combat_arms/effects_sounds.md: what the player sees and hears
when things happen. Effects: the muzzle flash (per weapon or class,
first and third person, size, lifetime, light), tracers (which shots get
one: every n-th, per weapon; speed, length, look by role), impacts per
surface type (how a hit surface's type is found from the world and from
models, and for each type the particles, decal, its size and lifetime,
how many decals stay), blood (on hit, by zone, on walls behind, at
death), shell casings (ejected from where, velocity, bounce, lifetime,
sound), grenade explosions, flash and smoke looks (grenades.md's
behaviour, the look here), and how the effect files are structured if the
game describes effects as data (format, fields, by role). Sounds: how a
sound is chosen for each event (weapon fire first and third person, near
and far, suppressed; reload and draw key sounds; footsteps by surface,
stance and speed, and when a step is silent; impacts by surface; bullet
flybys; hurt, death and hit-confirm sounds; grenade bounces and
explosions; ambient and map sounds), random variations, volume and pitch,
attenuation with distance (formula, inner and outer radius per class of
sound), what hears what (teammates, occlusion through walls), how many
sounds play at once and which are dropped, and the order of these within
a frame.

Describe behaviour in your own words: exact constants with units, formulas
as math, frame or tick dependence, server or client, edge cases (shooting
a surface with no type, many impacts at once, a sound beyond range), quirks
players rely on (hearing footsteps through walls, a flash that gives away
a position). Effect and sound names as data and surface type names the
game exposes may be named; never paste code or translate it line by line;
no fenced code blocks except ```math; no art and no audio (name an asset
only by its role or its file name inside the game's archives, never by a
machine path). End with a numbered list of concrete test cases (event,
surface, distance -> expected effect, decal, sound name and volume) that
an implementer without the game can check, and an "Open questions" list.

Write only specs/combat_arms/effects_sounds.md; change nothing else.
Commit with the repo's identity (CLAUDE.md "Commit identity"; check `git
config user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell
run `$env:TZ="UTC"` before `git commit`. End the message with
"Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>". Never skip
hooks. Then `git pull --rebase` and `git push` to main.
```

## Follow-up brief (Wave 2 open questions)

Answered 2026-10-18 (the specs' "Answered:" entries, cases grenades 36-58,
world_dat 8-22, textures_dtx 15-22) and implemented on
`worktree-agent-a89a90c8151a78770`: each module's Known shortcuts now
lists only what is still ours (and the new `game/volumes.rs`'s). Answers
that need a live measurement are sitting 5 of
[windows-captures.md](../active/windows-captures.md) (items 11-16).

Optional. Questions the Wave 2 implementers answered for themselves (each
module's `//!` "Known shortcuts" on main: `games/combat_arms/src/game/`
`grenades.rs` and `world.rs`, `lithtech/formats/src/` `world.rs` and
`dtx.rs`; the model, prop and animation readers have no implementation
yet, so 10a and 10c have no questions here). Paste the brief below into
one session; the implementers then change their code to the answers.

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

This is a follow-up session. The Wave 2 specs grenades.md, world_dat.md and
textures_dtx.md in specs/combat_arms/ have been implemented, and the
implementers had to pick a reading where the spec was silent or ambiguous.
Answer the numbered questions below from the reference material. For each
one, in the spec named after it:

- add the question to that spec's "Open questions" section if it isn't
  there (or find the existing one), and mark it "Answered:" with the answer
  in your own words, the constants and formulas it needs, and where in the
  spec's body it applies;
- where the answer changes or completes the behaviour, fix the body text
  too (sections, constants table, per-tick order), not only the question;
- add numbered test cases after the existing ones (never renumber or
  remove existing cases; if one is wrong, say so in it and add the
  corrected case), with concrete numbers an implementer without the game
  can check;
- if the reference can't settle it, say what you checked and give a short
  live-game measurement that would (setup, action, what to read off),
  and leave the question open.

The questions (Q = an existing "Open questions" number in that spec, case =
a test case number):

grenades.md
1. The throw's timing (Q3, Q5): we release the grenade at the end of the
   67 ms throw animation (767 ms after the press), start the throw on the
   press's tick, and use the M67's animations for the XM84 and M18. Is
   the release at the animation's end or at a key inside it, do the
   flash's and smoke's animations differ, and is the re-draw after a
   throw with grenades left the ordinary draw (its length)?
2. The first physics step (Q6): does a new grenade take its first step in
   the server frame it is created in or the next (cases 1-5 assume the
   next)?
3. The throw spread: we use n = n_min + floor((n_max - n_min) p) g, with
   p the thrower's spread fraction and the limits not multiplied by g, so
   section 4's 0.06 degrees for the flash survive g = 0 (weapon_firing.md's
   rule would make it 0). Which is right, and does any stock grenade have
   p > 0?
4. The bounce's push off the surface: the formula has -0.1 n, the
   Constants table 0.05. Which value, and where in the bounce step?
5. The grenade half-height for the at-rest nudge (Q9): the number per
   grenade (we use a placeholder).
6. Grenades and characters (Q7): is the touch tested against the hit
   spheres or the character's box, does a grenade slow again while still
   inside, do grenades touch each other (the data's same-kind flag), and do
   moving brushes and props bounce it like the world?
7. Water: how a liquid volume is marked in a map (object class and
   properties, as world_dat.md names them), and its default gravity,
   viscosity and current.
8. Cover traces: exactly what blocks the blast's line of sight
   (characters, props, invisible surfaces, moving brushes), and whether
   breakables take the blast with or without a cover test.
9. Armour against blasts and damage over time: which zone's armour (we
   use the torso), and which gear gives flash protection and how much.
10. The flash's facing and strength (Q11, Q12): yaw only or with pitch;
    the W and C factors from the victim's weapon (Quirk 6) for weapons
    other than the defaults; is the white drawn over the HUD?
11. Grenades at a round's end: does a grenade in flight or cooking
    explode, vanish or carry over into the next round? (If game_modes.md
    has landed, use its terms; answer it in grenades.md only.)
12. Shooting grenades (Q8): can a bullet hit a flying or resting grenade,
    and what happens when its hit points run out?

world_dat.md
13. Which surfaces collide (Q5): we let a polygon block players and
    bullets when its surface has the solid bit, ignoring the invisible
    bit. Is that right, and do collision blockers stop bullets or only
    players?
14. Lightmap coordinates beyond 0..1 (some up to 3.02): clamped per texel
    by the sampler, wrapped, or clamped per vertex? And Q7: the extra
    lightmap scale (the world properties' factor), with its value.
15. Vertex lighting: one colour per section or per vertex, and where the
    per-vertex colours sit if they exist.
16. The sky: how the sky portals, the sky's own world model and the sky
    scene object are drawn (order, depth, camera offset or scale), and
    which world properties set fog (start, end, colour) and what else they
    hold.
17. Light groups: their state when a map loads, what switches them, and
    how a switched group changes the lightmaps.
18. Second textures in a section: how detail textures, environment maps,
    fullbright masks and dual-texture sections combine with the base
    texture (blend, scale, which UVs).
19. Moving world models: which object classes move a world model (doors,
    rotating doors, breakables), the properties that drive them, and
    whether they block bullets and players while moving; and what the
    endless-fall volume and the other volume classes do.
20. Map identity: does the game check a map file's hash or size between
    server and client, and how?
21. Malformed data: what does the engine do with a property whose stored
    length doesn't fit its type, a polygon over 40 vertices, or an object
    record that ends early (we refuse the first two)? And the string
    encoding: which code page are names and strings in?
22. The blockers' trailer and unknown section shader codes: what the
    trailer holds, and which shader codes exist beyond those the spec
    lists.

textures_dtx.md
23. Load-time mip skipping (Q4): the shipped DetailLevel and the in-game
    texture quality option's effect, as a table of texture group ->
    levels skipped per option value.
24. Detail and environment links: how a texture's command string names
    its detail texture or environment map, how the name resolves (archive
    file name, extension), and the detail scale and blend.
25. DXT colour interpolation: do the 1/3 and 2/3 colours round to nearest
    or truncate (we round), and the same for the alpha block?

Write only these files: specs/combat_arms/{grenades,world_dat,
textures_dtx}.md; change nothing else. Commit with the repo's identity
(CLAUDE.md "Commit identity"; check `git config user.name` is r0pi0x, else
stop and ask me) in UTC: in PowerShell run `$env:TZ="UTC"` before `git
commit`. End the message with "Co-Authored-By: Claude Opus 5.5
<noreply@anthropic.com>". Never skip hooks. Then `git pull --rebase` and
`git push` to main.
```

## Follow-up brief (Wave 3 open questions)

Questions the Wave 3 implementers answered for themselves (each module's
`//!` "Known shortcuts" on main: `games/combat_arms/src/game/`
`characters.rs`, `view_model.rs` and `view_model/hands.rs`, `rules.rs`
and `rules/*.rs`, `bots.rs` and `bots/map_ai.rs`, `effects/mod.rs`,
`effects/fx.rs` and `effects/sounds.rs`; `src/ui/` `hud/score.rs`,
`scoreboard.rs` and `effects.rs`). Items that are whole missing topics
(the mission files, the lobby, gear, the missing modes, bots' objectives)
are Wave 4's briefs, not questions here. Paste the brief below into one
session (or hand it to a subagent); the implementers then change their
code to the answers.

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

This is a follow-up session. The Wave 3 specs characters.md,
view_models.md, game_modes.md, bots.md and effects_sounds.md in
specs/combat_arms/ have been implemented, and the implementers had to pick
a reading where the spec was silent or ambiguous. Answer the numbered
questions below from the reference material. For each one, in the spec
named after it:

- add the question to that spec's "Open questions" section if it isn't
  there (or find the existing one), and mark it "Answered:" with the answer
  in your own words, the constants and formulas it needs, and where in the
  spec's body it applies;
- where the answer changes or completes the behaviour, fix the body text
  too (sections, constants table, per-tick order), not only the question;
- add numbered test cases after the existing ones (never renumber or
  remove existing cases; if one is wrong, say so in it and add the
  corrected case), with concrete numbers an implementer without the game
  can check; mark the ones that need the install and say the Steam build
  id you measured;
- if the reference can't settle it, say what you checked and give a short
  live-game measurement that would (setup, action, what to read off),
  and leave the question open.

The questions (Q = an existing "Open questions" number in that spec, C =
a contradiction note, M/S = a section, case = a test case number):

characters.md
1. Hit spheres and animation: does the server place the hit spheres on
   the animated skeleton every update (which trackers count: the walking
   legs, the upper body's weapon and aim pose) or on a fixed pose? We use
   the measured `LST` t = 0 pose, crouched by squashing heights towards
   the feet, turned by the aim. Give the female `Bip01 Neck` node and the
   female right-hand socket's parent and offset too (not measured).
2. Update rate: is a character's pose decided per server frame, per
   movement tick or per rendered frame on the client, and does the server
   animate characters at all?
3. The drawn body against the hull: the body's offset from the standing
   and the crouched hull (feet at the hull's bottom?), and during the
   crouch transition.
4. Defaults without lobby data: which head piece, goggle, backpack, face
   and skin a player gets when the selection carries none (offline, a bot),
   per stock body, and the skin tables of the stock bodies other than the
   default male (as data by role).
5. Upper-body rates: where `HHAniFrame` (or the field that sets the upper
   body's playback rate per weapon) lives and its default; confirm the
   digit suffixes of classes 24, 26, 27 and 29 (we read revolver 1, FAMAS
   2, double barrel 1, snowball 2).
6. "Firing" for the driver: the exact condition for the fire posture
   (fire held; a magazine weapon not reloading and not empty;
   semi-automatic between shots; the knife; a grenade being cooked).
7. Q2 (the server's reload timer and its 500 ms fallback): settle it, and
   say whether a reload's end is the weapon's or the animation's.
8. Q5 (the server's `Torso` pitch 0): intended, and which hit tests use
   the server's spheres rather than the client's.
9. Death choice: how poison and the endless fall pick death animations
   (which damage kinds name them), and Quirk 6's coin (the clock in
   seconds or a random draw); is the dead player's position moved to the
   floor, or only the body's?
10. Q9 (corpses and shots): are bodies drawn with hit spheres, and do they
    stop bullets for the client's test?
11. Third-person weapons: which attribute field names a weapon's
    third-person model, the socket it hangs on (`RightHand`?) and its
    offset and rotation there, and where carried weapons sit (back,
    holster).
12. Emotes and the hit kick on remote characters: what triggers each,
    which animations, their length and layering.

view_models.md
13. C1 and the gameplay clock: does the view model's key (or its fire
    animation) time the shot, the knife's hit and the clip's refill, or the
    weapon's own timers (inventory.md 8.2, weapon_firing.md)? For a
    full-automatic: does the fire animation restart on the frame after it
    ends while shots run on the fire delay? A reload asked for during the
    fire sequence: after `postfire`, and when is the clip filled?
14. C6 (dry fire): when the dry-fire animation plays, and its sound.
15. Holster: does `deselect` play, and with what length (we use holster
    time 0)? When does `reload_1` play instead of `reload`?
16. Pieces and attachments on the view model: hidden pieces, projectile
    pieces, `PVAttachFX` (attached effects) and player-view attachment
    models: how each is chosen, placed and shown.
17. The hand model per character: how a character, uniform and colour
    pick the hands (the hand colour variants' table by role), and the
    file-name reset when the character or uniform changes.
18. Projection: confirm the 0.1 u near and 7 u far planes of the view
    model and how the world's FOV is given (vertical 75 degrees at 4:3?
    horizontal?) and what other aspect ratios do to it.
19. Spectating: the sway of a spectated player's view model, and the
    150-unit footing ray (what it tests, what it changes).
20. The flash hider: what it changes on the first-person muzzle flash.

game_modes.md
21. M3's bomb detonation: the blast's damage (radius, falloff formula,
    maximum, through walls or with a cover test, who is hurt: everyone or
    one team, whether the planter is credited with its kills and whether
    they count as kills or suicides), its effect and sound (by role; put
    the look in effects_sounds.md if that is its place), and the planted
    bomb's own object (model, beeping, light, which client sees what).
    Settle Q4 too (standing still during the plant, a partial defuse's
    progress).
22. The sites' `PlantedCommand` and other command strings: what they do
    (we only close the other site), and which other map objects modes run
    commands on.
23. Quirk 11 and S3: does `MissionTimeLimit` end anything? Does the
    intermission (`MissionEnd`) come after every round's end or only
    before the match's result (S3 step 6)?
24. Spawn families: which `GameStartPoint` `GameType` Bombing Run (M9)
    and Seize and Secure (M10) use, and where Seize and Secure's flag pole
    stands (object class, properties).
25. Arms Race (M14): the weapon table's source and its kill steps, and
    the random roll between the table and the player's equipment (odds).
26. VIP Elimination (M12): the VIP data (each rank's points) and how a
    player gets a rank.
27. Spy Hunt (M7, Q10): `Stage2PlayTime` and the time-out's winner; the
    stage-2 refill (health only, or ammunition too).
28. Quarantine Regen (M8, Q12): when a human killed by an infected comes
    back infected (at once, after the wait, at which point), the infected's
    model and weapon (from where), whether host selection runs in the
    warm-up, the heal's unit, and the match winner.
29. Hunted (M15): the scavenging (starting loadout, supply drops: what,
    where, when), the `DyingTime` hold, and how time hunted is counted.
30. S12's room options: what mutation boxes and the revenge buff do.
31. Objects and boxes: is a box object's `Dims` a half-extent or a full
    size? A player's box for S4's occupied test and objective touches (per
    figure type, or the fallback 15 x 50 x 15)? The pick-up box of the
    bomb (M9) and intel (M7) on the ground, and how their floor is found.
32. Spawn protection (S6): do instant-hit shots pass through a protected
    player during `protect_bullets`, or stop and do nothing; does the
    client draw blood? A point's `Command` on respawn; the fast respawn
    item.
33. Scoring (S7, S8): is damage scaled before or after armour; the team
    id "0 or above 254" filter; heals (which exist outside Quarantine);
    the teammate-hit voice.
34. M3: do sides ever swap outside the clan variants; when the bomb is
    handed out relative to the setup's respawn and loadout.
35. The HUD and scoreboard per mode (S13, hud_crosshair.md D6/D7): the
    goal string per mode, the objective lines and the action gauge's
    looks, the scoreboard's columns per mode and their order, the level
    icon and clan emblem, how ties are ordered, and the round or match
    result screen.
36. Random draws (drop points, hosts): which generator and seed, or is
    any uniform draw parity?
37. Capture the Flag (M4): where a carried flag is drawn (socket,
    offset), whether taking and returning work outside the Live phase,
    and assists.
38. Elimination modes: whom the dead may spectate (team only, free, a
    fixed camera), and the late-join closing (Q11).

bots.md
39. Q6: the templates' speeds and the confinement to AI volumes: give the
    run, walk and jump speeds per template and what happens at a volume's
    edge, so a bot moving through the player's movement can match them;
    do AI shooters get hit-zone factors?
40. Q3 (fire key timing): the bot's fire rate per weapon class as numbers.
41. Navigation volumes: what each volume type does (ladder, stairs,
    ledge, jump-up, jump-over, junction, teleport: one-way links, climbs,
    jumps), doors, path weights (`PreferredPath`), a neighbour cap; the
    bot game-mode file's patrol start nodes (`PatrolNode_Alpha0` ...), node
    actions (waits, look-arounds), guard nodes, and `AIRegion`,
    `AINodeGoto`, `AINodeUseObject`.
42. Q11 (stuck bots): what a bot does when it makes no progress.
43. Senses: the torso and head points sight tests aim at (sockets or
    heights), footstep stimuli (cadence, speed threshold, per-surface
    noise), "false stimulations", and Q14 (ally gunfire).
44. Grenade danger: are bot-thrown grenades announced too, and how many
    directions the dodge tries.
45. Small timings: the "catch breath" stop, a teammate's crouch request,
    the roll, the knife approach and swing distances, and whether a
    reload's attack move starts with the reload key or its animation.
46. Bots' spawn protection and respawn wait: the bot base's 3 s or the
    mode's.
47. Profiles: the profile file's fields by role (name format, archetype,
    item sets) and the draw ρ (the clock plus a random number: parity or
    any seeded draw?).
48. Bot chat: when bots talk and what (string ids by role).

effects_sounds.md
49. Third-person points: where others' muzzle flash and casings start
    (the third-person weapon's sockets: names, offsets) and where their
    shot sounds come from; do clients know others' hit zones (6)?
50. Liquids: the ammo's under-water record (`UWBullet`) contents, the
    bubble trail's look, and whether a ray goes on through a water
    surface.
51. Others' footsteps: their animation rate and gait on a network client
    (Q8).
52. Kill call-outs (`EnemyDown`): who hears them and when; map
    `SoundFX` objects (properties, looping, radius).
53. Critical hits, ricochets, the pusher, screen tints and blast lights:
    what each is and when it shows.
54. The effect file: `UpdatePos` attach modes and `Follow`; how multi-key
    scale, colour and alpha curves interpolate; cone and cylinder
    emitters' shapes; `AdditionalAcceleration`; bounce planes (which
    particles bounce, how the plane is found); friction; `DisableZ` and
    `CastVisibleRay` sprites; `LightningFX`, `LTBModel`, `PolyTrail` and
    `PlaySound` keys.
55. Sounds: is a playing 3D sound's gain and low-pass updated as the
    listener moves; Q1 and the 2R send radius; priorities and the voice
    cap (which sound is dropped).
56. Camera and decals: several camera jitters at once (largest, sum?),
    the camera's rise in free-movement volumes, decal clipping
    (`MarksClip`) and offset from the surface, and Q4 (lights through
    walls).

Write only these files: specs/combat_arms/{characters,view_models,
game_modes,bots,effects_sounds}.md; change nothing else. Commit with the
repo's identity (CLAUDE.md "Commit identity"; check `git config user.name`
is r0pi0x, else stop and ask me) in UTC: in PowerShell run `$env:TZ="UTC"`
before `git commit`. End the message with "Co-Authored-By: Claude Opus 5.5
<noreply@anthropic.com>". Never skip hooks. Then `git pull --rebase` and
`git push` to main (or to `specs-incoming` if I say so).
```

## Follow-up brief (movement in volumes)

Questions the volume-movement implementation (2026-10-19,
`games/combat_arms/src/game/movement.rs` and `volumes.rs`, their `//!`
"Known shortcuts") answered for itself or left as the spec has it. Paste
the brief into one session (or hand it to a subagent); the implementer
then changes the code to the answers.

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.

Answer these about players in volumes in specs/combat_arms/movement.md
(Open questions, with test cases where numbers are involved; new behaviour
in its sections):

1. Ladder from the ground. Standing on walkable ground inside a `Ladder`
   volume, looking up and holding forward: the Integration section pushes
   the upward acceleration back into the ground (a·n > 0.01), so the
   player never leaves the floor and the ladder can only be climbed by
   entering it in the air. What really lifts a player at a ladder's foot
   (no standing on a ladder, a different ground rule, a lift, the jump)?
   Also: jumping off a ladder, and whether anything happens on leaving
   one (velocity kept or cleared).
2. Container membership: is a player "in" a volume when its position (the
   hull's centre) is inside, or when the hull touches it?
3. `viscosity_max`: its value for players (grenades.md uses 5000 x c).
   Does the player's viscosity also apply only above 1 u/s, and is the
   current applied when the volume's viscosity is 0 (grenades: not)?
4. "Skipped right after a jump": on which ticks exactly.
5. Swimming: speed limit, acceleration and gravity in `Water`,
   `CorrosiveFluid` and `FreezingWater` (is player gravity changed?);
   whether looking up or down or the jump or crouch keys move a swimmer
   up or down; getting out over an edge; drowning or breath. Is
   "free movement" exactly the liquid classes?
6. Ladder speed: is the 200 limit scaled by the penalty and the
   containers' speed factors like the other limits?
7. Jump pads (`JumpVolume`): `Velocity` is a number in the maps (800 to
   3000): its direction, and when it fires (entering, standing in it, or
   only a jump inside it, as the Jumping section reads).
8. Volume property defaults for `Friction` and `PlayerSpeedFactor` per
   class; the maps set `Friction` 0 on safety nets and on GRAVITYB's
   gravity volumes: is a player's ground friction really 0 there, and do
   safety nets take part in the physics at all?
9. Explosion pushers: the pusher records (`BulletExpPusher` and others:
   radius, strength, duration), the push direction, and how the 100 u/s
   upward cap works when the speed is already above it.
10. Damage volumes: the `DamageType` property's values and what the
    freezing damage does (slowdown?).

Write only specs/combat_arms/movement.md (and world_dat.md if a volume
property's meaning belongs there); change nothing else. Commit with the
repo's identity (CLAUDE.md "Commit identity"; check `git config user.name`
is r0pi0x, else stop and ask me) in UTC: in PowerShell run `$env:TZ="UTC"`
before `git commit`. End the message with "Co-Authored-By: Claude Opus 5.5
<noreply@anthropic.com>". Never skip hooks. Then `git pull --rebase` and
`git push` to main (or to `specs-incoming` if I say so).
```

## How to run Wave 4

Same as Wave 3: `git pull` in a clean checkout on the Windows PC, then
one session that hands each brief below to its own subagent while the
main session alone does the git work (pull, commit, push). Each brief
writes only its own file. Push to the `specs-incoming` branch for review,
or straight to main. Board numbers continue from 17 (18 to 22). Items 18
and 19 read missions.md (21) where it has landed; they can run beside it
and leave mission-file details to it. The follow-up brief above
("Follow-up brief (Wave 3 open questions)") can run beside them: it edits
only Wave 3 specs.

## Spec briefs (Wave 4)

Each brief is complete on its own; paste it as is.

### 18. Game modes, part 2

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/game_modes.md first (its shared rules S1-S14 and
sections M11, M13, M16 and M18, which this spec completes), and
missions.md and bots.md if they have landed (`git pull`): this spec
continues them and must not contradict them; where it does, say so in
"Open questions".

Write specs/combat_arms/game_modes_2.md: the modes game_modes.md names
but doesn't describe enough to build, each in its own top-level section,
using game_modes.md's section names and shared rules (refer to them, don't
repeat them). Hired Guns (`SQM`): the AI mercenaries (who they are, how
many, which team, how they spawn and respawn, their weapons and health,
whether they are bots.md's bots or other AI, what they do), the bullion
(bars dropped on drop points: how many, when, where; the pocket: carry
limit, pick-up and drop rules, what a death drops), the banks (objects,
properties, how banking works, time to bank), the bank bonuses for kills
and deaths and where their numbers come from, the HUD and scoreboard
lines. VIP Escort (`ESC`, `CESC`): how the VIP is chosen, the VIP's spawn
point (object class and properties), the VIP's loadout, health and
limits, the exit (object, how escape is detected), the side swap's
timing and what it resets. The clan variants (`CDM`, `CMM`, `CCTF`,
`CTSV`, `CESC`): the half-time side swap per mode (the trigger, the
notice, what is kept and reset: scores, round wins, loadouts, the bomb,
flags), the warm-up, and anything else that differs from the public
mode. Snowball Fight (`SFM`): it is in the reference but not playable in
the US data; describe the tickets, freezing (how a snowball freezes, how
long, unfreezing, the frozen player's state), the snowball weapon's data,
and the "recovery" variant, at the depth the reference supports, and say
which build has it playable if any. If the reference has the co-operative
modes' details (game_modes.md M19, its Open questions 9) settle what you
can in an extra section.

Describe behaviour in your own words: exact constants with units, formulas
as math, per-tick order (a banking and a death on the same tick, the VIP
escaping as the clock ends), server or client, edge cases (the VIP
leaving, a carrier dying inside a bank, all mercenaries dead), quirks
players rely on. Setting names, mode names and object class names the
game exposes are data and may be named; never paste code or translate it
line by line; no fenced code blocks except ```math. Name files only by
their in-game role, never by path. End with a numbered list of concrete
test cases grouped by mode (setup: settings, players, time; events ->
expected score, time, winner, spawn chosen, side after the swap) that an
implementer without the game can check, and an "Open questions" list.

Write only specs/combat_arms/game_modes_2.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main (or to `specs-incoming` if I say so).
```

### 19. Bot objectives

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/bots.md (navigation, senses, goals, the bot
game-mode file), game_modes.md (the objectives: bomb sites, flags, intel,
seize points, the shared bomb) and game_modes_2.md and missions.md if
they have landed (`git pull`) first: this spec continues bots.md and
must not contradict them; where it does, say so in "Open questions".

Write specs/combat_arms/bots_objectives.md: what bots do about each
mode's objectives. First say, per mode, whether the reference's bots play
it at all (bots.md says bots exist in Bot Elimination and the
single-player builds; check whether any other mode runs bot goals for
objectives, and what the bot game-mode file holds per mode). For each
mode where they do: Search & Destroy (attackers: who carries the bomb,
choosing a site, going there, planting, guarding the planted bomb;
defenders: guarding sites, hearing a plant, going to defuse, when to
defuse rather than fight), Capture the Flag (attack, escort, defend,
return a dropped flag, carry home), VIP Escort (the VIP's own route,
escorting, hunting the VIP), Seize and Secure and Bombing Run (taking
and holding points, carrying the shared bomb), Spy Hunt and Hired Guns
(picking up intel or bullion, banking), and any other the reference
covers. Cover: how a bot picks its role and target (scores, team
quotas, randomness), the goals' priorities against fighting, how it
paths there (map nodes or volumes named by mode), how long it waits or
guards, how it reacts to the objective's events (plant, drop, capture,
pick-up), and difficulty's effect. If a mode has no bot behaviour in
the reference, say so in its section and don't invent one.

Describe behaviour in your own words: exact constants with units, formulas
as math, the order of objective goals within a bot's think, randomness
(distributions and ranges), edge cases (the carrier dies, the objective
moves, no path to the site), quirks players rely on. Object and node
class names the game exposes are data and may be named; never paste code
or translate it line by line; no fenced code blocks except ```math. Name
files only by their in-game role, never by path. End with a numbered list
of concrete test cases (setup: mode, map objects, bots and their
positions; time -> expected goal, target, path, action) that an
implementer without the game can check, and an "Open questions" list.

Write only specs/combat_arms/bots_objectives.md; change nothing else.
Commit with the repo's identity (CLAUDE.md "Commit identity"; check `git
config user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell
run `$env:TZ="UTC"` before `git commit`. End the message with
"Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>". Never skip
hooks. Then `git pull --rebase` and `git push` to main (or to
`specs-incoming` if I say so).
```

### 20. Lobby, rooms and menus

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/ui_system.md (how menus are built: layouts,
controls, the canvas; its Open question 6), game_modes.md (S2, the
server's mode settings the room sets) and inventory.md and
attachments.md (loadouts) first: this spec uses their terms and must not
contradict them; where it does, say so in "Open questions".

Write specs/combat_arms/lobby.md: the screens and flow from starting the
game to a match and back, as far as they matter for offline and LAN play
(we have no online service: describe what the game does and say which
parts depend on its servers). Cover: the main menu and its entries; the
server and channel choice and the room list (columns, filters, sorting,
refresh, joining a room, password, full and in-progress rooms); room
creation (every option by the name the game shows: mode, map, goal,
time, player count, team balance, friendly fire, join in progress,
password, spectators, the room options of game_modes.md S12, their
ranges, steps and defaults, which combinations are refused); inside a
room (slots, teams, ready, the host's controls, kicking, chat, map and
mode change, the start conditions, the countdown); loadout selection
(the loadout slots, how many sets, what may go in each slot, defaults
for a new player); the shop and inventory screens (only what decides a
player's equipment and gear: items, rental durations as data, what an
expired item does; leave out payment); the result screen after a match
and the return to the room; and the in-game menu's "return to lobby"
and leaving mid-match. For each screen: its layout file by role (the
layout itself is data in ui_system.md's format), controls and what each
does, keys, and the messages the client and server exchange as events
(not their bytes).

Describe behaviour in your own words: exact constants, ranges and
defaults, the order of events (who decides what, client or server or the
lobby service), edge cases (the host leaving, a player joining while the
match starts, an invalid loadout), quirks players rely on. Option names,
mode names and item ids the game exposes are data and may be named; no
art (name an image only by its role or its file name inside the game's
archives); never paste code or translate it line by line; no fenced code
blocks except ```math. Name files only by their in-game role or their
file name inside the game's archives, never by a machine path. End with
a numbered list of concrete test cases (screen, action -> expected
state, message, room setting, loadout) that an implementer without the
game can check, and an "Open questions" list.

Write only specs/combat_arms/lobby.md; change nothing else. Commit with
the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main (or to `specs-incoming` if I say so).
```

### 21. Mission files

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/game_modes.md (S2 settings, S3 phases, S4 spawns,
S12 drop points, and every mode's reference to mission data),
world_dat.md (map objects) and attribute_data.md (the attribute file
format) first: this spec gives the data they read and must not contradict
them; where it does, say so in "Open questions".

Write specs/combat_arms/missions.md: the per-map mission data the modes
read. Cover: where it lives (archive and file name by role, one file per
map or one table for all, keyed or not), its format (if it is
attribute_data.md's format or the world file's objects, say so and give
only the differences; else describe it field by field), how a map and a
mode find their mission entry (by map name, mode, room key), and every
field a mode reads: timers (round and match time, warm-up, phase
lengths: `ObjectStart`, `ObjectEnd`, `MissionEnd`, `MissionTimeLimit`),
goals and their ranges, spawn families (which `GameStartPoint`
`GameType` each mode uses on each map, team assignments, offence and
defence teams), objective weapons (the bomb, the infected's weapon and
model), objective objects (bomb sites, flags, intel, seize points, banks,
the VIP's spawn and exit), loadout restrictions, bonuses (Hired Guns'
bank bonuses), Arms Race's weapon table, co-op lives (`LifeCount`), and
anything else. For each field: name as the data exposes it, type, unit,
default when missing, and which rule reads it (game_modes.md section).
Also the server's own settings file that game_modes.md S3 mentions (the
phase lengths' defaults) and how a room's settings override the mission's.
Give a table of the stock maps' values for the fields modes need most
(time limits, goals, spawn families) per map and mode.

Describe behaviour in your own words: exact constants with units, the
order of overrides (stock default, mission, server file, room), edge
cases (a map without an entry for the chosen mode, a field out of range),
quirks players rely on. Field, map and mode names the game exposes are
data and may be named; never paste code or translate it line by line;
no fenced code blocks except ```math. Name files only by their in-game
role or their file name inside the game's archives, never by a machine
path. End with a numbered list of concrete test cases (map, mode, room
settings -> expected time limit, goal, spawn family, objective data;
the ones that need the install marked, with the Steam build id you
measured) that an implementer without the game can check, and an "Open
questions" list.

Write only specs/combat_arms/missions.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main (or to `specs-incoming` if I say so).
```

### 22. Gear, body types and perks

```text
You are a spec session for Lucker Party/Chimera, re-implementing Combat Arms
(the Classic, LithTech) from specs. Read CLAUDE.md and specs/README.md first
and follow their rules exactly. Ask me in chat where the reference material
is; never write that path, or any machine path, into the repo or a commit.
Read specs/combat_arms/characters.md (the selection record, model
records with `GearID`, `MaxArmor`, `MovePenalty`, `DashSpeed`, figure
types), attribute_data.md, movement.md (sprint and stamina),
weapon_firing.md (armour) and grenades.md (flash protection, its answered
question on gear) first: this spec gives gear its gameplay and must not
contradict them; where it does, say so in "Open questions".

Write specs/combat_arms/gear_perks.md: what a player's gear and perks
change. Cover: body types and figure types (each stock one's stats:
speed, sprint, stamina, armour, mass, and how they combine with the
weapon's movement penalty); vests (armour amount, which zones and damage
kinds it covers, how it wears down, movement cost); helmets (headshot
protection: damage factor, armour, which zones); backpacks (extra
ammunition, grenades or slots, by how much); goggles and masks (flash,
smoke or gas protection, by how much); every perk or special item the
reference has (extra ammunition, faster reload, faster respawn, more
health or stamina, radar, the fast respawn item game_modes.md mentions,
and anything else: what it changes, by how much, how long, and whether
it stacks); where each item's data lives by role (attribute files,
item tables) and its fields; how gear reaches the match (the loadout,
the selection record, the server's validation) and what applies when a
player has none (offline, bots). Give each stock item's numbers as a
table, or the formula and the data field it reads.

Describe behaviour in your own words: exact constants with units,
formulas as math, the order in which gear modifiers apply (armour before
or after zone factors, movement penalties combined), server or client,
edge cases (two items changing the same number, gear that expires
mid-match), quirks players rely on. Item ids and field names the game
exposes are data and may be named; no art; never paste code or translate
it line by line; no fenced code blocks except ```math. Name files only by
their in-game role or their file name inside the game's archives, never
by a machine path. End with a numbered list of concrete test cases
(gear, event -> expected damage taken, speed, stamina, ammunition,
protection) that an implementer without the game can check, and an
"Open questions" list.

Write only specs/combat_arms/gear_perks.md; change nothing else. Commit
with the repo's identity (CLAUDE.md "Commit identity"; check `git config
user.name` is r0pi0x, else stop and ask me) in UTC: in PowerShell run
`$env:TZ="UTC"` before `git commit`. End the message with "Co-Authored-By:
Claude Opus 5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main (or to `specs-incoming` if I say so).
```
