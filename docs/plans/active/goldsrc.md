# Plan: the GoldSrc family on Chimera (Counter-Strike 1.6 first)

Started 2026-11-24 (user's direction: after DoD:S, the GoldSrc family,
CS 1.6 first; ranked second in [next-games.md](next-games.md)). This is
the plan CLAUDE.md asks for before a new loader: no loader code lands
with it. The scaffold (`tools/new-game cs16`) landed with it, so the
game already registers and plays a stand-in in the sandbox;
[cs16.md](cs16.md) keeps CS 1.6's own log and points here.

Goal: one new loader family, `goldsrc/` (crate `goldsrc_formats`), reads
Valve's first engine's files; Counter-Strike 1.6 is its first game
(`games/cs16`, crate and id `cs16`) and lends its guns, movement and
bodies to the party apps (the Ultimate Gun Game first), as CS:S, Combat
Arms, HL2:DM, TF2, Halo CE and DoD:S do ([new-game.md](../../new-game.md)).
Parity first: a CS 1.6 rung plays as the real game, from specs,
measurements and the game's own files read at run time. Then each
further GoldSrc game (Half-Life Deathmatch, DoD 1.3, TFC, Opposing
Force) is a small port on the same loader.

## Status

| Milestone | State | Summary |
|---|---|---|
| Plan, install, scaffold | done (2026-11-24) | this plan; HLDS with CS 1.6 on this box (§2); `games/cs16` and `games/cs16/ui` from `tools/new-game` |
| First specs | in progress | §6: `specs/goldsrc/` formats written (2026-11-24: bsp, wad3, mdl, spr, mount; entities 2026-11-24); `specs/cs16/` install, movement, weapons to come |
| M0 loader, mount, maps, bodies | in progress | §5. First half done (2026-11-24): `goldsrc/formats` (BSP 30 with hull traces and solid regions, WAD3, MDL v10, SPR v2, `liblist.gam`, `materials.txt`), the collision spike (§3: the shapes are close enough), `games/cs16`'s install facts and folder chain. Second half done (2026-11-24): `goldsrc/engine` (maps to `MapData` with the §3 collision, studio models as bodies), the eight player bodies, `cs16:<map>` in the sandbox, the pool's fitness sweep. `chimera_pack cs16` published (2026-11-24: `players`, the 25 maps' sets, the guest dressing; 142 MB stored). Texture names (2026-11-24): animated sets and their alternate, random tiling, the liquid warp, chrome. Map logic (2026-11-24): `goldsrc_engine::logic` from entities.md (firing, movers, triggers, breakables, the `+a` toggle, light styles, `+use`). Left: the lightmap brightening against captures; the logic's network part, its sounds and debris |
| M1 weapons | not started | §5 |
| M2 movement | not started | §5; the lift of Source's movement into Chimera planned in [shared-movement.md](shared-movement.md) (2026-11-24) |
| M3 party rungs | not started | §5 |

## 1. Rules for this port

- **Clean room.** CS 1.6's game code is not public. Valve's Half-Life
  SDK (github.com/ValveSoftware/halflife) holds Half-Life's game code and
  the shared player movement, under Valve's own non-permissive licence:
  it is reference for **spec sessions only**, as the Source SDK 2013 is
  for CS:S, and never enters the repository or an implementation
  session. Reverse-engineered or decompiled game libraries (ReGameDLL_CS,
  ReHLDS) and leaked code are **not used** by this plan (open question
  1). Xash3D FWGS (GPL) is an engine reimplementation: spec-only
  reference for formats, never code.
- Behaviour reaches us from: the game's own data files (read in place on
  the install), measurements on the dedicated server with puppet bots
  (§6, `tools/cs16_probe`), the server's cvar list, public format
  documentation (the Valve Developer Community's GoldSrc pages, the
  community's format write-ups) and, for spec sessions only, the HL SDK.
  Spec sessions write only `specs/goldsrc/` and `specs/cs16/`.
- **Code** enters only from THIRD_PARTY.md: `qbsp` (MIT OR Apache-2.0,
  approved 2026-11-23) for BSP 30; everything else is ours.
- **No game files in the repository**: maps, WADs, models, sprites,
  sounds and the palette are read from the install (or the asset
  service) at run time. No test fixture cut from them.
- **Parity first, data-driven first**; **put guns into existing modes**:
  CS 1.6's guns become rungs of the Ultimate Gun Game; its bomb and
  hostage rules and buy menu are not on the party's path.

## 2. How players get the files

| Layout | Steam app | What it has | Use |
|---|---|---|---|
| Counter-Strike (the client) | **10** (paid; Linux build). Installs into `steamapps/common/Half-Life/` with `valve/` (Half-Life's base content, depot shared with app 70) and `cstrike/` | everything, plus the client's language folders | players; refcmp captures; the packing source for `chimera_pack cs16` |
| Half-Life Dedicated Server | **90** (SteamCMD, `+login anonymous`, `+app_set_config 90 mod cstrike`) | **all the content the party needs**: checked on this box, 2026-11-24 (below) | development, tests and measurements on this box |

The other GoldSrc apps install into the same `Half-Life/` folder, each
its own content folder: Half-Life 70 (`valve`), Team Fortress Classic 20
(`tfc`), Day of Defeat 30 (`dod`), Opposing Force 50 (`gearbox`),
Deathmatch Classic 40 (`dmc`), Ricochet 60 (`ricochet`), Blue Shift 130
(`bshift`). HLDS serves the multiplayer ones by `app_set_config 90 mod
<folder>` (one folder per install run).

**Installed on this box** (2026-11-24): `~/games/cs16_ds`, HLDS app 90
with `mod cstrike`, anonymous, first try (no "Missing configuration"
retry needed), **930 MB** (`cstrike/` 282 MB, `valve/` 515 MB, the rest
engine binaries). It holds:

- `cstrike/`: 25 stock maps (BSP **30**: `as_oilrig`, `cs_*` ×10,
  `de_*` ×14 incl. `de_dust2`, `de_inferno`, `de_nuke`, `de_train`), 23
  WAD3 files (`cstrike.wad`, `cs_dust.wad`, `decals.wad`...), 179 MDL
  (all **v10**: 31 `v_`, 34 `w_`, the `p_` models, the eight player
  classes `models/player/<class>/<class>.mdl`, hostages, map props), 71
  SPR (v2: `640hud*.spr`, crosshairs, muzzle flashes, smoke),
  `sprites/hud.txt` and `sprites/weapon_*.txt`, 382 WAV (165 under
  `sound/weapons/`; 8-bit PCM mono 22 050 Hz), `sound/materials.txt`,
  `sound/sentences.txt`, `events/*.sc`, `gfx/env/*.tga` skyboxes,
  `gfx/palette.lmp`, `overviews/`, `resource/*.res` and
  `resource/cstrike_english.txt`, `titles.txt`, `liblist.gam`
  (`steam.inf` PatchVersion 1.1.2.7), and the game libraries
  (`dlls/cs.so`, `cs_amd64.so`, `cl_dlls/client.so`: never read by us).
- `valve/`: Half-Life's base content (the fallback folder): 132 maps
  (the campaign and HL:DM's), `halflife.wad`, `liquids.wad`, `decals.wad`,
  `gfx.wad`, `fonts.wad`, 323 MDL, 384 SPR, 2 542 WAV, 27 MP3.
- No archive files: no PAK, no GCF (§3).

Whether the server layout lacks anything the client has (language
folders aside) is M0's first check against a client install (open
question 2); for DoD:S and TF2 the server lacked shared texture
archives, here the textures are WADs in `cstrike/` and `valve/` and are
present.

**Found** as the other games are (`chimera_core::mount::install`):
`$CHIMERA_CS16_ROOT`, then `[games.cs16] path` in `chimera.local.toml`,
then the first-run dialog, then Steam's libraries (app 10, its manifest's
`installdir` `Half-Life`). The install root is the folder holding
`cstrike/` and `valve/` (both layouts). **Required files**:
`cstrike/liblist.gam`, `cstrike/maps/de_dust2.bsp`, `valve/halflife.wad`.

**Asset service**: app 10 is paid, so the Steam ownership check works as
for CS:S. HLDS (app 90) is free and anonymous; ownership of the client
is what the service checks, never the server's.

## 3. The formats

GoldSrc keeps its files loose on disk; its "archives" are the WAD3
texture files and, historically, Quake's PAK.

| Format | Holds | `qbsp` covers | We write | Milestone |
|---|---|---|---|---|
| **BSP 30** | the map: entities (text), planes, embedded mip textures (with their own palettes) or names to find in WADs, faces, texinfo, lightmaps (RGB, 8 bits a channel, up to 4 styles, a luxel per 16 units), the render BSP (nodes, leaves, contents, PVS), **clip node hulls** 1-3, brush models | parsing of every lump, GoldSrc's per-texture palettes, lightmap atlas, meshing, point raycast and leaf lookup on the render tree, PVS | the conversion to `MapData` in metres (faces, materials, lightmaps, brush entities, spawns), **hull traces** on the clip node trees (qbsp's raycast walks hull 0 only), the collision geometry for Chimera (below), entity key/value parsing to our map words | M0 |
| **WAD3** | texture archives named by a map's `worldspawn` `wad` key (old absolute Windows paths: only the file name counts), searched along the mount; mip textures (`0x43`) with a 256-colour palette each, `qpic` (`0x42`) and fonts (`0x46`) in `gfx.wad`/`fonts.wad`; `decals.wad` | none (it reads only textures embedded in the BSP) | the whole reader: directory, mip textures to RGBA (`{` names: index 255 transparent; `!` water, `+0`..`+9`/`+a`..`+j` animated and toggled frames, `-` random tiling, `sky`, `clip`, `aaatrigger` and other tool textures), decals (palette ramp, last colour is the tint) | M0 |
| **MDL v10** (`IDST`, version 10) | models: bones, bone controllers, hitboxes **with their hitgroups**, sequences (RLE-compressed animation per bone and channel, 2-way blends for aiming), events in sequences (sounds, muzzle flash, shells), sequence groups in `<name>01.mdl`..., textures in `<name>T.mdl` when external, body parts and submodels, skins, attachments; 8-bit paletted textures with flags (chrome, flat-shaded, fullbright, masked, additive); triangle strips and fans per mesh | none | the whole reader and its conversion to Chimera's model kit (as `source_formats::mdl` for Source): skeleton, skinned meshes, sequences to clips (with blends), events, hitboxes to `Hitgroup`; chrome's view-dependent UVs and lighting from the map's light under the model are presentation work | M0 (bodies, hitboxes), M1 (view models, events) |
| **SPR** (`IDSP` v2) | sprites: orientation type (facing, upright, oriented...), texture format (normal, additive, index-alpha, alpha-test), frames and frame groups, one palette | none | the reader; sprites as billboards in the map kit; the HUD's atlases (`640hud*.spr` with `sprites/hud.txt` and `weapon_*.txt` rectangles) | M1 (muzzle flashes, crosshair), M3 (HUD) |
| **WAV** | sounds: PCM 8-bit unsigned (most) and 16-bit, mono, 11/22/44 kHz; `cue` chunks mark loops | none | nothing new: `chimera_formats::wav` reads 8-bit PCM; loops from `cue` if it doesn't yet | M1 |
| **MP3** | `valve/media` music only | none | not needed | none |
| Text data | `liblist.gam` (game name, `fallback_dir`, `gamedll`), `sound/materials.txt` (texture name to surface letter), `sound/sentences.txt` (radio), `titles.txt`, `events/*.sc` (event names only), `sprites/hud.txt`, `weapon_*.txt`, `maps/<map>.txt`, `mapcycle.txt`, `resource/*.res` (KeyValues), `resource/cstrike_english.txt` (UTF-16 localisation), `gfx/palette.lmp` | none | small readers; KeyValues and the localisation file reuse Chimera's/Source's readers | M0 (liblist, materials), M1, M3 |
| TGA, BMP | skyboxes (`gfx/env/<sky>{up,dn,lf,rt,ft,bk}.tga`), overviews, menu art | none | the image crate reads them | M0 (sky) |
| PAK (Quake `PACK`) | old and third-party installs only; none in the Steam layouts | none | a `FileSource` for it only if a supported layout ever needs one | not planned |

**`qbsp`: adapt, don't depend** (recommendation; open question 3).
Taking it from crates.io would compile Quake's palette into our
binaries (`lib.rs` embeds `palette.lmp` with `include_bytes!`, a
768-byte id Software file), and would bring Quake 1/2, BSPX and meshing
code the family never uses. THIRD_PARTY.md already allows "adapted
files with the header": take its BSP 30 reader and data structures
(and its leaf/PVS queries) into `goldsrc/formats/src/bsp/`, each file
with the credit header, the palette and the Quake 1/2-only paths
dropped, and record the commit taken in THIRD_PARTY.md. glam: qbsp
0.17 is on glam 0.33, which the workspace already has, so its types
carry over if we keep them.

**Collision** is the format's real difference from Source: a BSP 30
file has no brushes. The map's solids exist only as (a) the render tree
(hull 0: visible geometry and its solid leaves) and (b) three clip node
trees with the world already grown by a player's box (hull 1 standing,
32×32×72; hull 2 large, 64×64×64; hull 3 ducking, 32×32×36), traced as
points. Player clip brushes are only in hulls 1-3. Chimera's physics
wants shapes, and other games' characters of other sizes walk these
maps, so M0 builds both:

- **Shapes for Chimera**: hull 0's solid leaves as convex pieces (exact
  for bullets and every game's characters), plus the clip-only volumes
  from hull 1 (solid in hull 1, not in hull 0 grown by the standing box)
  shrunk back by the standing box: exact for axis-aligned faces,
  approximate at bevelled edges. A spike in M0 decides whether this is
  close enough; if not, the clip volumes come from hull 3 as well.
- **Hull traces for CS 1.6's own movement** (M2): its player traces
  hull 1/3 as the original does, for parity on GoldSrc maps; on other
  games' maps it uses Chimera's shapes like any character.

**The M0 spike's result (2026-11-24): close enough; build it.** Measured
by `goldsrc_formats`' `heavy::collision_spike` (its doc says how to run
it) on de_dust2, cs_office, de_aztec, de_nuke and de_inferno, 20 000
points each where hull 0 is open, and a third as many traces of 32-256
units from free points, mostly horizontal. Ground truth is hull 1: a
standing box at p is blocked exactly when hull 1 calls p solid, and its
first contact is hull 1's trace. The shapes are `bsp::solids`: hull 0's
solid regions plus hull 1's solid regions shrunk back by the standing
box (`Shrink::Measured`, the spec's measured growth; moving every face
by the box's full support, `Shrink::Support`, measures the same within
0.1 %).

| Map | Point mismatch (> 1 u, > 4 u) | Worst point | Traces off > 1 u (> 4 u) | Trace p99 | Shapes (hull 0 + shrunk hull 1) |
|---|---|---|---|---|---|
| de_dust2 | 0.85 % (0.68 %, 0.28 %) | 31.5 u | 6.8 % (6.1 %) | 21.7 u | 1 566 + 829 |
| cs_office | 1.11 % (0.94 %, 0.61 %) | 30.6 u | 4.0 % (3.9 %) | 19.4 u | 2 047 + 1 448 |
| de_aztec | 0.27 % (0.20 %, 0.13 %) | 32.5 u | 4.7 % (4.2 %) | 18.4 u | 1 275 + 527 |
| de_nuke | 1.26 % (1.07 %, 0.56 %) | 27.8 u | 8.2 % (7.5 %) | 23.8 u | 1 482 + 1 100 |
| de_inferno | 0.38 % (0.25 %, 0.12 %) | 26.1 u | 3.6 % (3.3 %) | 15.2 u | 1 688 + 1 045 |

What the numbers say:

- **Nearly every mismatch is on the safe side**: the shapes block a box
  that hull 1 lets through (152 of de_dust2's 171 point mismatches,
  237 of de_nuke's 253). That is the compiler's growth being smaller
  than the box at slanted faces (bsp.md: diagonal walls grown by 16, not
  22.6; ramps by about 36 × normal z): hull 1 lets a box's corners into
  a 45° wall by up to 6.6 units, which true shapes cannot reproduce.
  Along a grazing move that small offset becomes a long distance, which
  is where the traces' tail (p99 15-24 u, worst ~200 u) comes from: in
  the sampled cases the shapes stop the box 6-11 u earlier against a
  diagonal wall or ride a ramp's face a few units higher.
- **Clip brushes are recovered**: hull 0 alone misses 4-69 % of the
  points (de_aztec and de_inferno are mostly player clip above the
  roofs); with the shrunk hull 1 regions, points that hull 1 calls
  solid but the shapes leave free are 0.03-0.12 % (7-23 of 20 000).
- **One fix was needed and is in**: hull 1's regions are pieces of one
  solid cut by the tree's planes; shrinking every face of every piece
  lost a box's width at each cut (the first run left 62 % of hull 1's
  regions empty and let boxes into clip). Now a face is moved back only
  where it borders hull 1's open space (`solids::exposed_in`, probed
  every 8 units); faces between two solid pieces stay.
- Hull 3 is not needed for the clip volumes; hull 1's shrunk pieces
  alone are worse (0.6-2.2 % mismatches: their shrinking is
  approximate), so both sets are kept.

Recommendation: build Chimera's collision for GoldSrc maps as hull 0's
solid regions (bullets and every character) plus hull 1's shrunk regions
as player clip (characters only), as convex shapes; CS 1.6's own player
keeps tracing hulls 1 and 3 (M2) for parity. Characters of other games
stop a few units short of slanted walls and ramps compared with CS 1.6's
players; nothing lets them through a wall or into player clip. Left for
the map loader: dropping hull 1 pieces that only repeat hull 0 (the
shapes count is 2 400-3 500 a map) and the brush entities' own hulls.

## 4. What carries over from Source, and what is new

GoldSrc is Source's ancestor: same unit (the inch, 0.0254 m), same axes
(Z up), the same player box when standing (32×32×72), the Quake-lineage
movement, the same style of entity key/values (`classname`, `origin`,
`targetname`), the same hitgroups idea. Carried over:

- **Chimera as is**: the mount (`chimera_vfs::LooseDir` per content
  folder), map data, bodies, view models, the weapon data model and its
  hooks (hitscan, seeds, magazines, kick), grenades on CS:S's thrown
  grenade, decals, particles, sound, the party bundle.
- **CS:S as a model, not as code**: CS 1.6 is CS:S's direct ancestor
  (same guns, buy menu, armour, round rules), so CS:S's specs are the
  shape for CS 1.6's, but every number is measured again: the games
  differ in spread, recoil, penetration, speeds and timings. A game never
  depends on another (`cs16` never on `cs_source`); what both need moves
  into Chimera.
- **Source movement as the base**: `source_engine::source_movement` is
  already parameterised (DoD:S runs it with its own config), but it is
  in the `source` loader layer, which a GoldSrc game may not use. The
  movement both families share is lifted into Chimera
  (a new crate, `chimera_movement`: a "Quake-lineage ground and air movement")
  when cs16 needs it (M2); until then cs16 runs the placeholder. The
  lift is the main shared-code work of M2.

**Movement: what differs and needs a spec** (specs/cs16/movement.md;
each item measured on the server, with the HL SDK's shared movement as
a spec session's reference for Half-Life's base behaviour):

1. **Hull traces** against clip nodes instead of box sweeps against
   brushes (§3); stepping, ground checks and slides on those hulls.
2. **The origin is the box's centre**, not its feet (the standing box
   spans ±36 in Z around it); spawns, eye height and our conversion to
   Chimera's feet-origin bodies must agree.
3. **Frame-rate-dependent movement**: GoldSrc runs one movement step
   per client command, with the command's duration in whole
   milliseconds, not on a fixed tick. Chimera's fixed tick is chosen to
   match the common case (100 commands a second, 10 ms each; the
   server's `sys_ticrate` default to read); integer-millisecond rounding
   and what changes at other rates go in the spec.
4. **Duck**: a 36-unit duck box (CS:S's is taller), the duck and unduck
   times, the eye's travel, ducking in the air (the box shrinks upward:
   the "duck jump"), and what blocks unducking.
5. **Edge friction**: GoldSrc multiplies friction near a drop-off
   (`edgefriction` cvar); Source removed it.
6. **The cvars' defaults**: `sv_accelerate`, `sv_airaccelerate`,
   `sv_friction`, `sv_stopspeed`, `sv_maxspeed`, `sv_stepsize`,
   `sv_gravity`, `sv_bounce`, `sv_maxvelocity` as CS 1.6's server lists
   them (they differ from CS:S's in places).
7. **Jumping and landing**: jump speed, CS 1.6's slow-down after a
   jump/landing (its form, duration and whether repeated jumps stack),
   bunny-hopping as the game allows it (no cap is assumed either way).
8. **Speed by weapon**: each gun's maximum speed, the shield, zoomed
   speeds.
9. **Ladders** (`func_ladder` volumes), **water** (leaf contents; wading,
   swimming, the water jump), **slopes** (the walkable limit),
   **fall damage** (its speed thresholds and curve), **being slowed when
   hit**.
10. **View**: punch angle (view kick) and its decay, the eye heights
    standing and ducked, the view's bob and roll if any.

**Weapons: CS 1.6 keeps its numbers in code** (unlike CS:S's and
DoD:S's scripts; its `sprites/weapon_*.txt` hold HUD rectangles only).
Every gun's damage, range modifier, penetration, fire rate, magazine,
reload and draw times, spread and accuracy model, recoil, speed and
price comes from measurement on the server (puppet bots firing at
measured distances, reading hits and damage) and the view models'
sequences (durations from frames and frame rate are data in the MDL).
The specs flag anything unmeasured. This is the largest spec effort.

## 5. The crates and milestones

The shape is new-game.md's, plus a family crate:

| Crate | Folder | Holds |
|---|---|---|
| `goldsrc_formats` | `goldsrc/formats` | BSP 30 (adapted from `qbsp`), WAD3, MDL v10, SPR, `liblist.gam`, `materials.txt`, the hull trace: plain data, no Bevy; may use `chimera_vfs` and `chimera_formats` only; fixtures written by its tests, `heavy::` tests on the install |
| `goldsrc_engine` | `goldsrc/engine` | GoldSrc's content in Chimera's types (from M0): maps (`content::maps`, textures, lightmap, brushes, collision, entities, sky) and studio models as bodies (`content::studio`); its sources are hashed by each game's converter version; the maps' entity logic (`logic`: `GoldSrcLogicPlugin`, which `cs16` adds) |
| `cs16` | `games/cs16` | the game (scaffolded 2026-11-24): `INFO`, mount, maps, bodies, movement, guns, `BUNDLE`; `loaders = ["goldsrc"]` from M0 |
| `cs16_ui` | `games/cs16/ui` | its HUD (sprites from `640hud*.spr`), kill icons; `party::CLIENT` |
| `cs16_app`, `cs16_server` | `apps/cs16`, `apps/cs16_server` | optional, as for TF2 and DoD:S (a guest first); for testing its own rules later |

What every GoldSrc game shares lives in `goldsrc/engine`
(`goldsrc_engine`, on Chimera and `goldsrc_formats`): its `content`
converts a BSP 30 map into `MapData` and a studio model into a
character body for any GoldSrc game (`content::maps::GoldSrcGame`: the
game's id, converter version, spawns, gravity and kit), as
`source_engine::content` does for Source. It was started in M0 rather
than lifted later (the orchestrator's call, 2026-11-24); the mount's
folder chain stays in `cs16` until a second game needs it. The architecture
check learns the `goldsrc` loader family in M0
(`layers_follow_paths`, `[package.metadata.chimera] loaders`).

1. **M0, loader, mount, maps, bodies in the sandbox.**
   - `goldsrc/formats` with the BSP 30 reader (from qbsp, credited),
     WAD3, MDL v10 (bodies and hitboxes first), `liblist.gam`;
     `heavy::` tests: every stock map parses, every WAD and MDL in
     `cstrike/` and `valve/` reads.
   - `INFO`: content folder `cstrike`, Steam app 10, tick 0.01 s
     (to confirm, §4.3), 0.0254 m a unit, console namespaces `cs16`
     and `cstrike`. The mount: `cstrike_<language>` (client), `cstrike`,
     then `liblist.gam`'s `fallback_dir` (default `valve`); the
     `_downloads`, `_addon`, `_hd` and `_lv` folders are left out
     (`nohimodel 1` in CS 1.6's liblist). A heavy test: the server layout
     mounts and the 25 maps list.
   - Maps: faces, embedded and WAD textures, lightmaps (with GoldSrc's
     brightening to match the game in refcmp), the sky, brush entities
     by render mode, water, the collision shapes of §3; spawns from
     `info_player_start` (CT) and `info_player_deathmatch` (T);
     `cs16:<map>` in the party apps; a map pool data file after a
     fitness sweep.
   - The eight player bodies (`models/player/<class>/<class>.mdl`) worn
     by name on placeholder movement; hitboxes from their hitgroups.
   - The roster: already scaffolded (`GUESTS`, `CLIENTS`).
     `chimera_pack cs16` (`players`, `map/<name>`, `weapon/<id>` once
     its guns have files): published 2026-11-24 (Log).
2. **M1, weapons**: the rifles, SMGs, pistols, shotguns, snipers and
   the M249 as data (`data/weapons/`), numbers from the spec;
   CS 1.6's spread, accuracy and recoil as hooks; scopes and the
   burst/silencer modes (FAMAS, Glock, M4A1, USP); the knife
   (`GameParts::knife`); HE grenades (flashbang and smoke later); view
   models (`v_*.mdl`, arms included) with their sequences' events
   (sounds, shells, muzzle flash sprites), `p_`/`w_` models, sounds.
3. **M2, movement**: the shared Quake-lineage movement lifted into
   Chimera (§4); `cs16:movement` on it with CS 1.6's config and the
   differences of §4 (hull traces, duck, edge friction, landing
   slow-down, speeds by weapon, ladders, water, fall damage); its
   envelope for other games' characters on its maps.
4. **M3, party rungs**: each CS 1.6 gun a rung of the Ultimate Gun Game,
   the holder a CS 1.6 class body (team by the side, as CS:S's), its HUD
   while its gun is in hand (`640hud` sprites, the dynamic crosshair,
   health/armour/ammo), its kill icons. Cross-game balance stays the
   host's opt-in.

After CS 1.6: **Half-Life Deathmatch** (HLDS `mod valve`, its guns are
in Half-Life's content already installed here: crowbar, 9mm, .357, MP5
with grenades, shotgun, crossbow, RPG, Gauss, Egon, hornet gun,
grenades, satchels, tripmines, snarks) is the natural second game and
the moment `goldsrc/engine` is lifted; then DoD 1.3, TFC, Opposing
Force. Each needs its own plan section and specs.

Not planned: CS 1.6's bomb/hostage/VIP rules, buy menu and money in the
party; bots; HLTV; the single-player campaign.

## 6. Specs to write

Spec sessions follow [tasks/game-spec-session.md](../../tasks/game-spec-session.md):
they read the install's files in place, measure the dedicated server,
use public format documentation and (spec sessions only) the HL SDK;
never leaked or decompiled code; write only `specs/goldsrc/` and
`specs/cs16/`, in our own words, no code blocks.

| Spec | Covers | From |
|---|---|---|
| [`specs/goldsrc/bsp.md`](../../../specs/goldsrc/bsp.md) (draft) | every lump and record with de_dust2's numbers, lightmap sizing, and what `qbsp` does not: hull/clip node traces (what a hull trace returns, start-solid, all-solid, fractions), contents values, the render modes of brush entities, texture name prefixes, lightmap brightening, the sky, `worldspawn` keys | public docs, the files, HL SDK (spec-only), measurement for the brightening |
| [`specs/goldsrc/wad3.md`](../../../specs/goldsrc/wad3.md) (draft) | the WAD3 directory and lump types, mip textures, `qpic`, fonts, decals' palette ramp, transparency | public docs, the files |
| [`specs/goldsrc/mdl.md`](../../../specs/goldsrc/mdl.md) (draft) | MDL v10: header, bones, controllers, hitboxes, sequences and groups, the animation compression, blends, events, textures and flags, meshes, external texture and sequence files | public docs, the files, HL SDK (spec-only) |
| [`specs/goldsrc/spr.md`](../../../specs/goldsrc/spr.md) (draft) | SPR v2: types, texture formats, frames and groups, palette | public docs, the files |
| [`specs/goldsrc/mount.md`](../../../specs/goldsrc/mount.md) (draft) | the content folder chain (`liblist.gam`, fallback, language and the folders we skip), file name case, `materials.txt` | the install, public docs |
| [`specs/goldsrc/entities.md`](../../../specs/goldsrc/entities.md) (draft) | the entities the 25 stock CS 1.6 maps use, with counts: firing (target, delay, killtarget, master, `multi_manager`, `multisource`), movers (doors, buttons, trains, rotators: movedir, lip, wait, return, blocking, sounds by index), breakables, triggers, the `+a` texture toggle (frame 1 on buttons and used `func_wall`s), light styles, sprites, sounds, the CS objective entities; round restarts and CS's `+use` left to measurement | the entity lumps, the shipped FGD, public level-design docs, HL SDK (spec-only) |
| `specs/cs16/install.md` | the two layouts, required files, what the server layout lacks against the client's | the installs |
| `specs/cs16/movement.md` | §4's list, with numbers and test cases | measurement (probe), the cvar list, HL SDK for the base |
| `specs/cs16/weapons.md` | every gun's numbers and behaviour (fire modes, scopes, silencers, burst, shield later) | measurement, the view models' sequences, public pages |
| `specs/cs16/damage.md` | hitgroups and multipliers, armour and helmet, range falloff, wall penetration, fall and blast damage | measurement |
| `specs/cs16/view_and_hud.md` | view kick and decay, the dynamic crosshair, the HUD's sprites and layout | measurement, `hud.txt`, captures |

**Questions for the next spec session** (found by implementation
sessions; to add to the specs' open questions there, not edited in by
an implementation session):

- bsp.md: the `!` liquid warp's form, size, wavelength and speed ("a
  sine warp, turbulent" only; `goldsrc_engine::content::textures` uses
  our own 8 texels, 64 texels, 4 s); whether liquids are lit (already
  open question 4) and whether a `!` face makes its leaf or brush entity
  water for drawing (fog, the view under water).
- bsp.md: random tiling (`-0`..`-9`): per face or per texture tile of a
  face, and how the variant is picked (ours: per face, a fixed hash of
  the face's index); an animation set with a gap in its frame
  characters (ours: the present frames in order).
- (answered in entities.md, "Alternate textures") bsp.md: which entities toggle the `+a`..`+j` set and when (buttons
  while pressed? doors? any brush entity's `frame`), and whether the
  alternate set animates at the same rate.
- entities.md (from `goldsrc_engine::logic`): the brush size the
  movers' travel uses is the models lump's bounds; is it the same as the
  hull 0 regions' bounds on every stock mover (ours: the regions')?
  Whether a `multi_manager`'s target may be named like a common key
  (`target`, `speed`: ours skips the common keys); a `func_wall_toggle`
  fired with *set*; what plays a `func_rotating` without `sounds`.
- mdl.md: chrome's coordinate mapping (which vectors, the scale, whether
  it turns with the view or the light, the flat-shade flag beside it);
  ours is Chimera's camera-space sphere map.

A **measurement tool** comes first: `tools/cs16_probe`, as
`tools/hl2dm_probe` and DoD:S's probe. HLDS is a 32-bit Linux binary
(32-bit libraries are on this box); a server-side plugin (Metamod and
AMX Mod X installed into `~/games/cs16_ds/cstrike/addons` only, never
the repository) drives a puppet bot's commands per frame and logs
positions, speeds and damage. It never binds UDP 27015 or 27030: it
runs on its own port on `127.0.0.1`, killed by exact pid.

## 7. Risks

- **No public CS 1.6 game code**: every gun and the CS-specific
  movement come from measurement, the largest spec cost of the port.
  Some quirks (recoil patterns, the accuracy curve, penetration) need
  many samples; unmeasured numbers are flagged.
- **Collision without brushes** (§3): the hull-0-plus-clip-volume
  reconstruction is new work; bevelled clip edges may be off by a few
  units for other games' characters. CS 1.6's own player traces hulls
  directly, so its parity does not depend on it.
- **Frame-rate-dependent physics**: matching the 100 Hz case is the
  target; other rates' quirks (e.g. jump height at odd rates) are out
  of scope unless asked.
- **Lifting Source's movement into Chimera** touches CS:S, HL2:DM, TF2
  and DoD:S; its own short plan is [shared-movement.md](shared-movement.md), with their movement tests green
  throughout (M2).
- **Palette textures and lightmaps** in our renderer: converted to RGBA
  at load, so no renderer change, but the game's lightmap brightening
  and its chrome and additive model textures need care to look right
  in refcmp; the sky is a six-image box, not Source's.
- **The `qbsp` licence route**: adapting files needs credit headers and
  THIRD_PARTY.md's commit line; the Quake palette must not come along.
- **Measurement tooling**: AMX Mod X and Metamod are GPL server plugins,
  used only as instruments on this box's server; a puppet bot may be
  refused actions as DoD:S's was (its probe could not jump or sprint);
  found out in the probe's first session.
- **Disk**: the dedicated server is 930 MB here; each further GoldSrc
  game adds its folder (HL:DM needs nothing more: `valve/` is in).

## 8. Open questions for the user

1. May spec sessions read the reverse-engineered CS 1.6 game library
   (ReGameDLL_CS, derived from decompilation) as reference, as spec
   agents read the Combat Arms source? It would make the weapon and
   movement specs far quicker and more exact. Until answered: no, and
   specs come from measurement and the HL SDK.
2. Do you own Counter-Strike (app 10)? Not blocking (the server holds
   the content); the client is wanted for refcmp captures and to check
   the server layout lacks nothing.
3. `qbsp`: adapt its BSP 30 files with credit (recommended, keeps
   Quake's palette out of our binaries) or depend on the crate?
   Adapted (2026-11-24, M0): its record shapes, leaf lookup and
   visibility decoding, credited in each file and THIRD_PARTY.md.

## Log

- 2026-11-24: plan written. HLDS (app 90, `mod cstrike`) installed
  anonymously at `~/games/cs16_ds` (930 MB) and its content inventoried
  (§2): BSP 30, WAD3, MDL v10 and SPR v2 confirmed by their headers.
  `tools/new-game cs16 "Counter-Strike 1.6" --short "CS 1.6"` scaffolded
  `games/cs16`, `games/cs16/ui`, `specs/cs16/` (templates) and
  [cs16.md](cs16.md); the scaffold fits (GoldSrc's install is loose
  files, which the template's mount already reads); its content folder,
  Steam app, tick and unit are M0's.
- 2026-11-24: the M0 format specs written in `specs/goldsrc/` (index
  [README.md](../../../specs/goldsrc/README.md)): BSP 30, WAD3, MDL v10,
  SPR v2 and the mount, each checked against every matching file of the
  HLDS install. Findings for M0: the clip hull sizes are confirmed from
  the files (hull 1 ±16/±16/±36, hull 2 ±32, hull 3 ±16/±16/±18;
  spawns stand 36 + 4 units above hull 0's floor); non-axial faces are
  grown by less than the box's support distance (vertical ones by the
  half-width), so shrinking hull 1 back needs the hull 0 planes, not a
  formula; lightmap sizes need the texel extents computed in double and
  rounded to single precision (then all 150 maps tile exactly); CS 1.6
  player models aim with 9 pitch blends; file lookups must ignore case
  (de_dust2's sky files are `Des*.tga`).
- 2026-11-24: M0, first half. `goldsrc/formats` (`goldsrc_formats`, the
  `goldsrc` loader layer in the architecture checks): BSP 30 (every lump,
  lightmap sizing, visibility, entities, point contents and hull traces
  on hulls 0-3, solid regions as convex polytopes; record shapes, leaf
  lookup and visibility decoding adapted from `qbsp` 0.17.0, its palette
  left out), WAD3 (mip textures, pictures, fonts, decals), MDL v10
  (every array, animation streams, triangle commands, external textures
  and sequence groups), SPR v2 and the HUD sprite lists, `liblist.gam`
  and `materials.txt`; fixture tests from the specs and `heavy::` tests on
  the install (all 150 maps, every WAD, every version 10 model and
  sequence file, every sprite). The collision spike (§3): close enough. `games/cs16`:
  `cstrike` with `liblist.gam`'s fallback (`valve`), Steam app 10, tick
  0.01 s, 0.0254 m a unit, `loaders = ["goldsrc"]`. Found against the
  specs: MDL's attachment count and offset are at header bytes 212 and
  216 (mdl.md's table says 216 for the count); three of Half-Life's
  deathmatch maps (`disposal`, `pool_party`, `rocket_frenzy`) carry
  800-odd bytes after the last lightmap (bsp.md says every map tiles
  exactly); both for the next spec session.
- 2026-11-24: M0, second half. `goldsrc/engine` (`goldsrc_engine`, a
  new crate of the `goldsrc` layer) converts a map: faces meshed per
  texture (embedded or from the `wad` key's WADs, found ignoring case;
  all 40 of de_dust2's found), lightmaps (style 0 in one atlas, bytes as
  linear light: the brightening is still bsp.md's open question 1), the
  `gfx/env` sky, the entity list with spawns put on hull 1's floor,
  brush entities by class (movers on their own node, `angles` handed
  over as `movedir`; static solids; drawn-only; ladders; hidden
  triggers) and render mode, surfaces from `materials.txt`, and the §3
  collision: hull 0's regions as solids (1 566 on de_dust2) and hull 1's
  shrunk regions as player clip, those whose probe points all lie in
  hull 0's solid dropped (829 shrunk pieces, 176 kept). A studio model
  becomes a body: bind pose from the bone defaults (the CS models'
  already stand on the origin), every sequence of group 0 as animations
  with root bones raised 36 units (their frames are about the box's
  centre), 9-blend aims on a `body_pitch` grid, aim/shoot/reload
  sequences weighted on the upper body (from `Bip01 Spine`). `cs16`:
  `MAP_LOADER` (`cs16:<map>`, the converted-map cache, build.rs
  converter version), the eight classes' bodies numbered as CS:S's
  (`game::bodies`) with a stand-in driver (gait by speed and stance, the
  one-handed aim layered), the guest dressing (its bodies on other
  games' maps), its map pool rules and fitness measurements. Heavy tests:
  de_dust2 converts with no missing texture and the same each time, its
  spawns stand on hull 1's floor and hold a player, 80 walks from its
  spawns never end inside hull 1's solid, shots pass player clip that
  stops characters; the eight bodies read with their sequences and stand
  on their feet facing -Z. The map fitness sweep (`map_fitness --game
  cs16`) measured all 25 stock maps fit (no spawn spot off a map or
  stuck, the spots covering about the whole walkable area): all enter
  the pool (`games/cs16/data/map_pool.toml`). `+sandbox cs16:de_dust2`
  plays in the Ultimate Gun Game with the stand-in pistol among the rungs.
- 2026-11-24: `chimera_pack cs16` (`cs16::game::sets`): `players` (the
  eight class bodies), `map/<name>` for the 25 stock maps (each converted
  with the players' kit, never from the cache: its BSP, the WADs its
  `wad` key names, its sky, `sound/materials.txt`) and `weapon/<id>` per
  gun once a gun names files. `sets --pack` may now start a manifest, so
  the loose install is packed from its pieces' reads rather than whole
  (`valve/` is mostly Half-Life's campaign). Published to the host's
  store as HLDS build 15961496 (`sets cs16 --pack --build`, `convert
  cs16`): 175 files, 324 MB plain, 142 MB stored, the shared WADs one
  record each (two skies share one face). Test: cs16 `heavy::served`
  (de_dust2 and the bodies from the asset service alone, the same as
  from the install).
- 2026-11-24: texture names and chrome (`goldsrc_engine::content::
  textures`, `brushes`, `studio`). `+0`..`+9` frames play as Chimera's
  animated textures (`MapTextureAnim`, ten a second, bsp.md), the `+a`..
  `+j` set as a second animation; a mover's faces with both carry their
  first frames as `MapMesh::frames` (the toggle, which no GoldSrc logic
  drives yet). A family's frames and variants come from the map's own
  texture lump (the compiler stores them all; checked on the stock maps).
  `-0`..`-9` variants: one per face by a fixed hash of its index, so the
  conversion stays deterministic. `!` liquids warp: a new Chimera
  `MapUvWarp` on world meshes (world.wesl), with our own size and speed.
  Chrome (MDL flag 2) draws its texture as a `MapLayer` on sphere-map
  coordinates. Heavy tests: cs_office's `+0~lab_crt2` four frames in
  order and its buttons' alternate sets, de_aztec's `!watergreen` warped,
  de_dust2's variants all shown and the same each time, chrome on the
  bodies. What the specs leave open: §6, "Questions for the next spec
  session".
- 2026-11-24: `specs/goldsrc/entities.md` written from the 25 stock maps'
  entity lumps, the shipped FGD and (spec session only) the HL SDK:
  every class with counts and worked examples (cs_office's door button,
  de_nuke's glass doors, as_oilrig's elevator, de_survivor's `CLIC`
  blinker that drives a `+a` wall). `de_dust2` itself has no doors or
  triggers, only crates its bomb targets break. Left to measure: what a
  CS round restart resets, CS's `+use` on doors, the objective volumes'
  sizes, hostage and armoury placement details.
- 2026-11-24: GoldSrc map logic (`goldsrc_engine::logic`, from
  [entities.md](../../../specs/goldsrc/entities.md)): the firing model
  (target, delay, killtarget, use types, master), `multi_manager`,
  `multisource`, `trigger_relay`, `trigger_auto`; the movers' shared rules
  (movedir, lip, the mover's clock, pushing and carrying players, blocking)
  and `func_door`, `func_door_rotating`, `func_water`, `func_button`,
  `func_rot_button`, `func_train`/`path_corner`, `func_rotating`,
  `func_pendulum`, `func_conveyor`; `func_wall` (the `+a` set through its
  frame: `map::BrushTint` on its node, whose meshes carry
  `MapMesh::frames`), `func_wall_toggle`, `func_illusionary`,
  `func_breakable` (`func_pushable` as one); `trigger_multiple`, `once`,
  `hurt`, `push`, `teleport`; switchable light styles, `env_sprite`,
  `env_render`, `ambient_generic`; `+use` by Half-Life's search; the CS
  objective entities kept as data (a bomb target fires its `target` on
  `FireEntityOutput`). The conversion gives a named `func_wall`/
  `func_water` and every `func_wall_toggle` a node, and marks the map's
  logic family (`map::entities::LOGIC_FAMILY_KEY`), so Source's logic
  leaves GoldSrc maps alone in a party app. Unit tests of the worked
  examples; heavy tests on the install: cs_office's button opens `door1`
  (108 units after 1 s + 2.7 s), de_nuke's four `GlassDoor2` doors slide
  62 units together, cs_militia's rotating door swings away from the
  player walking into it, de_survivor's `CLIC` wall flips every 0.5 s,
  de_dust2's `tgt_a` crates break when `*42` fires them and not when
  shot. The spec's open questions are taken at its stated assumptions,
  each a Known shortcut in the module docs (round restarts reset
  everything, Half-Life's `+use`, players only, box overlap for
  triggers). Not yet: network replication, sounds (no GoldSrc sound
  bank), debris, sprite drawing, light style rendering.
