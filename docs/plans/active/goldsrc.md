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
| First specs | in progress | §6: `specs/goldsrc/` formats written (2026-11-24: bsp, wad3, mdl, spr, mount; entities 2026-11-24); `specs/cs16/` movement, weapons, damage, view_and_hud drafted 2026-11-24 (ReGameDLL as reference, cvars and model data checked); install to come |
| M0 loader, mount, maps, bodies | in progress | §5. First half done (2026-11-24): `goldsrc/formats` (BSP 30 with hull traces and solid regions, WAD3, MDL v10, SPR v2, `liblist.gam`, `materials.txt`), the collision spike (§3: the shapes are close enough), `games/cs16`'s install facts and folder chain. Second half done (2026-11-24): `goldsrc/engine` (maps to `MapData` with the §3 collision, studio models as bodies), the eight player bodies, `cs16:<map>` in the sandbox, the pool's fitness sweep. `chimera_pack cs16` published (2026-11-24: `players`, the 25 maps' sets, the guest dressing; 142 MB stored). Texture names (2026-11-24): animated sets and their alternate, random tiling, the liquid warp, chrome. Map logic (2026-11-24): `goldsrc_engine::logic` from entities.md (firing, movers, triggers, breakables, the `+a` toggle, light styles, `+use`). Logic on network clients (2026-11-24): Chimera's shared `net::movers` (lifted from Source's logic), `logic::net`. Polish (2026-11-25): the maps' sounds (doors, buttons, ambients, breakables: `content::sounds::add_map`), `env_sprite`/`env_glow` drawn and switched (`content::sprites`), light styles in the lightmap atlas (switchable 32+ and the animated presets, `content::lightmap`; converter version 3). Drawn light (2026-11-26, converter version 5): the lightmaps' curve on the styles' sum (`map::LightCurve`, `content::lightmap`), texgamma on opaque textures, studio models lit from the floor or sky (`content::model_light`). Left: debris; sentences |
| M1 weapons | done (2026-11-25) | §5: the 24 guns, the knife and the three grenades as data (`games/cs16/data/weapons`) with `cs16:gun` (accuracy, spread by state, kick, modes, bursts, penetration by GoldSrc's power rule: Chimera's `PowerPenetration`) and `cs16:grenade`; `v_`/`p_` models and sounds through `goldsrc_engine::content::weapon_models` and `sounds`; `weapon/<id>` sets published; `scaffold: false` (Log). Polish (2026-11-25): the bodies' weapon sequences (aim on the pitch blends, shots, reloads; `[hooks."cs16:body_anim"]`), the HE's and smoke's looks from the install's sprites (`weapons::looks`), dropped and thrown weapons as their `w_` models (`map::loose::loose_key`). Shells done (2026-11-26, Log). Left: measurements (weapons.md open questions) |
| M2 movement | done (2026-11-25) | §5. `cs16:movement` is the shared movement ([shared-movement.md](shared-movement.md), S0-S9) with CS 1.6's config from specs/cs16/movement.md, hull traced on GoldSrc maps (`goldsrc_engine::hulls`, the clip trees carried in the converted map), Chimera's shapes elsewhere (Log). Water volumes (2026-11-25, Log): the world's liquid leaves and the liquid brush entities (`goldsrc_engine::content::water`; converter version 4). Footsteps, the swim's details (up key, lift by liquid, currents), slime, lava and drowning, the liquid fog (2026-11-26, Log). Left: the ladder volumes' own rule (movement.md ladder gaps), measurement in the game |
| M3 party rungs | in progress | §5. The guns are rungs since M1. HUD (2026-11-25): `cs16_ui::hud` from specs/cs16/hud.md (health with the cross and bar, armour with the suit lit by armour, ammunition with its icon, fades, the damage arcs; the install's `640hud` sprites added at their pixel size), the `d_` kill icons and headshot mark in every app's kill feed, the HUD's sprites in `players` (Log). Second part (2026-11-25, Log): CS 1.6's dynamic crosshair, the weapon menu (bucket list), the round timer, the 320 layout. The HUD follows hud.md's second part (2026-11-25, Log: the spec's numbers for the crosshair, timer, menu and 320 layout). Left: hud.md questions 2-3 (money; the pistols' speed threshold and fastswitch cycling), the 320/320hud files in the served `players` set (republish) |

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
| **WAD3** | texture archives named by a map's `worldspawn` `wad` key (old absolute Windows paths: only the file name counts), searched along the mount; mip textures (`0x43`) with a 256-colour palette each, `qpic` (`0x42`) and fonts (`0x46`) in `gfx.wad`/`fonts.wad`; `decals.wad` | none (it reads only textures embedded in the BSP) | the whole reader: directory, mip textures to RGBA (`{` names: index 255 transparent, its colour filled from the opaque texels around it, as for masked studio textures and alpha-test sprites; `!` water, `+0`..`+9`/`+a`..`+j` animated and toggled frames, `-` random tiling, `sky`, `clip`, `aaatrigger` and other tool textures), decals (palette ramp, last colour is the tint) | M0 |
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
   Done 2026-11-25 (Log), flashbang and smoke included; the muzzle
   flash sprites and shells left.
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
| [`specs/cs16/movement.md`](../../../specs/cs16/movement.md) (draft) | §4's list, with numbers and test cases: cvar defaults, speed by weapon, jump and the per-command jump penalty, duck, steps, edge friction, ladders, water, fall damage, hit slowdown, msec stepping | ReGameDLL (spec-only reference), the server's cvars; in-game measurement still open |
| [`specs/cs16/weapons.md`](../../../specs/cs16/weapons.md) (draft) | every gun's numbers and behaviour: accuracy formulas and spread by state, kick tables, fire modes, scopes, silencers, bursts, reloads (checked against the view models' sequences), knife, grenades | ReGameDLL (spec-only reference), the view models |
| [`specs/cs16/damage.md`](../../../specs/cs16/damage.md) (draft) | hitgroups and multipliers, armour and helmet, range falloff, wall penetration, fall and blast damage | ReGameDLL (spec-only reference), the player models' hit boxes |
| [`specs/cs16/view_and_hud.md`](../../../specs/cs16/view_and_hud.md) (draft) | view heights, zoom, punch and decay, flash fade, the HUD's sprites; layout and the dynamic crosshair still to capture | ReGameDLL (spec-only reference), `hud.txt`; captures to come |

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
- movement.md (from `cs16::game::movement`, M2): the test cases for the
  jump disagree with the ground rule. "68 commands in the air; lands at
  vz -275.67" and the first ground command's timer of 635.79 ms hold
  when only the move itself meets the floor; by "Ground detection" (the
  2-unit probe after every move) the 67th command ends 0.22 units up,
  inside the probe, so it lands there: 66 commands end in the air, the
  fall speed counted is 259.67 and the first ground command's timer is
  645.79 (scale 0.8773). Ours follows the rule; which is the game's?
  "About 31 % after 10 commands" is the penalty's share alone (friction
  takes it to about 19 %): say so.
- (answered 2026-11-25 in movement.md "Units and conventions": the
  client clips to the maximum speed first, then × 0.52: 130 with a 250
  gun) movement.md: the walk key. By the text it scales the keys' amounts by
  0.52 before the clamp to the max speed, which walks at 208 units/s
  with a 250 gun (ours); is CS 1.6's walk not 0.52 of the max speed
  (130), as CS:S's is?
- (answered 2026-11-25 in movement.md "Ladders", "Water" and
  "Currents": the face trace is a point trace against the ladder brush,
  the origin inside the brush itself gives no ladder velocity; the up
  key is `+moveup`, `cl_upspeed` 320, not jump; no stock map has a
  current) movement.md: ladders. "The player is on it when the origin, tested
  against the ladder model's hull for the box, is not empty" and the
  trace toward the model's centre: what the trace returns when the origin
  is already inside the grown hull (ours keeps the shared rule: the
  volume is a solid ladder brush found by a 2-unit probe along the keys).
  Water: the up key's amount in Chimera's terms (jump?), and whether
  `func_water` volumes carry currents in the stock maps.
- spr.md / hud.md (from `cs16::game::hud`, M3): the install's
  `sprites/hud.txt` (client build 12934623 and HLDS 15961496 alike)
  says 215 entries but holds 193; hud.md's "215 entries" and spr.md's
  count rule should say a list may end early (our reader keeps the
  entries there). hud.md: how many rows the suit's lit part shows for
  armour that isn't a multiple of 100 / 24 (ours truncates 24 × armour /
  100); whether a fall or other undirected damage lights the arcs (ours:
  none); the kill feed's icon blending (ours blends the grey icons by
  their brightness in 255 80 0, the app's feed has no additive draw).
- movement.md: on a duck let go before it completes, the text puts the
  standing box 18 units up on the ground (the duck-tap hop: ours does);
  confirm it is the game's and not only the reference's.

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
   movement specs far quicker and more exact. **Answered 2026-11-24:
   yes** ("do whatever is easiest for you to recreate the game
   faithfully"): spec sessions may read ReGameDLL_CS (cloned outside the
   repo, deleted after) and check behaviour against the game; specs stay
   in our own words, nothing else from it enters the repository, and each
   spec's header names the sources.
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
  exactly); both for the next spec session (fixed in the specs
  2026-11-24, below).
- 2026-11-24: M0, second half. `goldsrc/engine` (`goldsrc_engine`, a
  new crate of the `goldsrc` layer) converts a map: faces meshed per
  texture (embedded or from the `wad` key's WADs, found ignoring case;
  all 40 of de_dust2's found), lightmaps (style 0 in one atlas, bytes as
  linear light: the brightening, bsp.md's "Drawn light", is implemented), the
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
- 2026-11-24: GoldSrc logic on network clients (`goldsrc_engine::logic::net`).
  Source's mover replication moved into Chimera as `chimera_net::net::movers`
  (the network part `map_movers`: `NetMover`, `Stepper`, the client's
  stepping from snapshots, `BrushTint` onto the client's nodes, and a new
  `NetEntityPart` for switched parts), used by Source's logic and
  GoldSrc's alike. The server writes every GoldSrc node's state (origin,
  angles, velocities, the mover's clock, its move's end and goal; shown,
  solid, shootable, gone); `BrushTint` (render changes, the `+a` frame)
  replicates beside it; `env_sprite` on/off goes as `NetEntityPart`. A
  client places the nodes before movement where the server's movement met
  them and, after movement, steps them and carries or shoves its own
  player with the logic's own `movers::push_player` (GoldSrc moves
  players first, movers after: `core::Predict::CarryRiders`), so walls
  toggle, breakables broken on the server are hidden and passable and
  passable doors and named `func_water` are not solid on clients. Heavy
  `NetSim` tests (`games/cs16/tests/it/heavy/net_logic.rs`): cs_office's
  `door1` matches the server tick for tick after the start's round trip,
  riding as_oilrig's elevator has no prediction errors after it,
  de_dust2's broken crates are passable on the client. Not done: light
  styles (the converted lightmaps carry style 0 only), debris (needs
  GoldSrc gib models as gib sets), sprite drawing.
- 2026-11-24: CS 1.6 game specs (`specs/cs16/`): movement.md, weapons.md,
  damage.md, view_and_hud.md, from ReGameDLL_CS (spec session only, its
  original-game behaviour; §8 question 1) and checked against the
  install: HLDS build 10211's cvar defaults (accelerate 5, airaccelerate
  10, friction 4, stopspeed 75, edgefriction 2, stepsize 18), the
  shipped client config (400 move speeds, fps_max 100), every view
  model's reload/draw lengths, the nine player models' hit-box groups.
  Not yet measured in the running game: firing, recoil, penetration,
  movement trajectories (the client needs a running Steam client and
  32-bit libraries this box lacks; no bot nav or profile in the server
  install). mdl.md: attachments at 212/216, sound tables at 220. bsp.md:
  the lightmap blocks pack with no gaps; `disposal`, `pool_party`,
  `rocket_frenzy` carry 837, 855, 822 trailing bytes.
- 2026-11-25: M1, CS 1.6's weapons (specs/cs16/weapons.md, damage.md,
  view_and_hud.md's flash fade). `games/cs16/data/weapons`: the 24 guns
  in the spec's numbers (damage, range modifier, count and bullet type,
  armour ratio, cycle, clip, reserve, reload, draw, price, side, speed as
  `max_speed` with the scopes' zoomed speeds for the movement), the knife
  and the HE, flash and smoke grenades. `cs16:gun` (games/cs16/src/game/
  weapons/gun.rs): spread by state from the accuracy (the automatic
  guns' shots count with the original's whole-number division and the
  spray's stale accuracy, the recovery 0.4 s after the release then one
  every 0.0225 s; the pistols' by the time since the last shot; the
  G3SG1's and SG550's by theirs), the kick tables with the side flag,
  the pistols' and bolt snipers' 2° up, the autosnipers' random kick and
  the shotguns' whole degrees, the punch's decay (Chimera's
  `weapon::decay_punch`, lifted from CS:S), the silencers' damage, the
  Glock's follow-ups on the next two commands at a fixed 0.05, the
  FAMAS's 34 first round and follow-ups at its spread, the timers set per
  command so cycles round up (`timing = "set"`). Penetration: Chimera's
  new `PowerPenetration` and `PowerRules` (GoldSrc's power, count,
  per-material power scale and factor, 42/0.75/0.75 after a player or
  the last hit, the next segment `power` beyond the hit, a start inside
  a solid passing out freely), `games/cs16/data/penetration.toml`. The
  knife on Chimera's melee (slash 15 always, stab 65, the backstab by the
  levelled aim, `Backstab::Aim::level`). Grenades on Chimera's shared
  grenade (`cs16:grenade`: the throw's bend and speed, 1.5 s fuse checked
  every 0.1 s, HE 100 linear to 350 through walls, the flash's
  4 − d × 4/1500 strength with its facing and behind holds and fades,
  the smoke). Models: `goldsrc_engine::content::weapon_models` (view
  models unraised in the eye's space with their sequences' events and
  attachments; held `p_` models on the player skeleton's hand by bone
  name, the Elites' left gun merged on its own hand), `studio` lifted
  into a mesh builder (`studio::meshes`) and events on every sequence;
  `sounds`: GoldSrc's waves as entries named by path (several by `;`).
  The view rules (`weapons/view.rs`): the `_unsil` sequences with the
  silencer off, the Glock's burst sequence, the Elites by hand, the
  shotguns' shell sequences, the knife's and grenades'; 5001-5031 muzzle
  flashes, 5004 sounds for the holder. `weapon/<id>` asset sets (`v_`,
  `p_`, `w_`, every wave). `PARTS.scaffold` false: the guns are dealt
  into the gun game's ladders. Tests (`games/cs16/tests/it/weapons.rs`):
  the AK-47's first shots (0.0055, then 0.009625 to the sixth, 0.034375
  after), 11 shots in 1 s, its first and second kicks, a reload, the
  Deagle by time, the Glock's burst on consecutive commands, the AWP's
  0.081 and zoomed-ducked 0 with its unzoom and return, the M4A1's
  silencer, the knife's slash 15 and backstab 195, the M3's nine pellets
  in one event, HE by distance, through the north wall and on armour;
  heavy `guns`: every weapon's models and sounds load from the install.
  Shortcuts: each module's doc (the muzzle flash sprites, shells, the
  HE's and smoke's looks, the bodies' weapon sequences, the shotguns'
  pellets summed before the distance rule, the blast's armour depletion,
  the Elites' 5-unit offsets, friendly fire's × 0.35).
- 2026-11-25: M2, movement. The shared movement's GoldSrc seams landed
  (shared-movement.md S7-S9: clip hulls, command milliseconds, edge
  friction) and `cs16:movement` runs on it (`cs16::game::movement`, the
  numbers in `games/cs16/data/movement.toml` under the spec's names):
  the cvar defaults as `cs16:` console variables, the jump set to
  268.33 with the per-command jump penalty and the 1.2 x cap cut to
  0.96 x, the 36-unit duck (0.333 keys from the press, the box 40
  commands later, at once in the air with the feet 18 up, up at once
  when let go, the duck-tap hop), the ground tested and snapped every
  command, steps of 18, ladders at 200, the water jump at 225, landing
  effects from 250 and damage above 500 of (speed - 500) / 4.8, the hit
  slowdown (0.5; 0.65 with the 170 push; +0.01 a ground command, scaling
  the velocity), the punch roll's linear decay, the speed table by gun
  (data; the weapon layer hands the held gun's `max_speed`, which the
  tests hold to the table). Each GoldSrc rule beyond the
  three seams is a named switch of `QuakeConfig`, off for Source's games
  (shared-movement.md, Log). **Hull traces**: the conversion carries the
  planes, the clip nodes and the placed models' hull roots (the world and
  the static brush entities) as `map::clip_trees::MapClipTrees` in the
  map's extras (converter version 2; the served `map/<name>` entries
  must be packed again); `goldsrc_engine::hulls` rebuilds a `Bsp` of
  just those and traces it with the formats' own `Bsp::trace`
  (`GoldSrcHulls`, a `chimera_movement::ClipHulls`), offered as
  `MapClipHulls` when a map with trees is loaded (`GoldSrcHullsPlugin`,
  server and client alike). On every other game's map there are no
  trees and the movement sweeps Chimera's shapes; other games'
  characters on GoldSrc maps sweep the reconstructed solids as before
  (§3). Movers, ladder volumes, characters and props are box sweeps for
  everyone. The large flinch is by the hitting gun's id (the data's
  twelve). `GameParts::scaffold` is false since M1 landed the same day.
  Tests: the spec's test cases
  on the greybox at 10 ms (`games/cs16/tests/it/movement.rs`: 12.5, 22,
  97.31, 250 at the 41st command; 240, 166.21, 73.46 and 0 at the 55th;
  the apex 44.99; the duck's 40 commands, the duck jump and the hop; no
  standing up under 60 units; 20, 125 and 0 points at 600, 1 100 and
  500; the cut to 240; 200 and 282.8 up a ladder, 270 off it; the
  modifier 0.66 on the next command; two disagree with the spec's table
  and follow its rules: the questions above), a golden run
  (`0x455eb29c0bc087e4`, 1 620 ticks, the step 71 460 instructions a
  tick on the greybox's shapes), `net_prediction` (no prediction error
  through runs, jumps, ducks, the hop and the walk key at 80 ms; every
  predicted tick the server's bits; the same place after loss), and on
  the install `heavy::movement`: 112 six-second walks from de_dust2's
  spawns, ducked for a stretch, never with the box where its hull is
  solid (hull 1 standing, hull 3 ducked: exactly, no allowance), 13
  stair steps climbed, a resting player within 1/20 unit of the floor;
  26 of the 28 ladder volumes of de_nuke, cs_assault and de_train
  climbed to their tops from their feet (de_dust2 has no ladder; one
  each on de_nuke and cs_assault not reached from the four sides tried).

### 2026-11-25: polish (bodies, looks, sounds, sprites, light styles)

- **Bodies** (`games/cs16/src/game/bodies.rs`): over the gait, the held
  weapon's set of the player model's own sequences: `ref_aim_<set>` /
  `crouch_aim_<set>` on the 9 pitch blends (`body_pitch` = the view's
  pitch down, GoldSrc's sign: the model's forearms point up at −90,
  `heavy::bodies`),
  a shot, swing or throw `ref_shoot_<set>` (the Elites `shoot`/`shoot2`
  by the clip's parity), a reload `ref_reload_<set>`, each played once
  over the aim; the body holds its active weapon's `p_` model
  (`hold_weapons`, `map::Held`: no CS 1.6 body held one before). The
  set is weapon data (`[hooks."cs16:body_anim"] set`),
  ours by the sequences' names: no spec says (Known shortcut).
- **Grenade looks** (`weapons::looks`): the HE plays
  `sprites/zerogxplode.spr` once, the smoke puffs `sprites/gas_puff_01.spr`
  every second 21 times; GoldSrc sprites as particle sheets
  (`goldsrc_engine::content::sprites::particle_material`). Sizes, rates
  and lives are ours (weapons.md open question 5).
- **Dropped weapons**: a held model may carry a loose form
  (`map::loose::loose_key`, Chimera), which loose and shown items take;
  CS 1.6's is its `w_` model (`weapon_models::dropped`).
- **Map sounds**: the conversion adds every wave the logic can play
  (`World::sound_paths`: door, button, train, fan and break tables,
  `ambient_generic`, `+use`) as entries by path; sentences (`!NAME`) are
  not read (no spec; none in the stock maps).
- **Sprites**: `env_sprite` and `env_glow` as `MapSprite`s (first frame,
  `scale`, render keys), switched by the logic through `EntityPart`.
- **Light styles**: bsp.md's layout (a block per style, back to back);
  six stock maps carry styles beside 0 (cs_747 4/10/32, cs_estate,
  cs_havana and cs_militia 32/33, de_survivor 32, de_torn 1), contrary to
  bsp.md's "only style 0" (to correct in the spec). The atlas is the
  start's sum and each style a `MapLightStyle` share; the logic's
  switches reach `map::LightStyles`. Network clients (2026-11-25): the logic's changed styles replicate as `net::movers::NetLightStyles` (pattern letters; clients play them by their own clock under the writer `net`), a light's own `pattern` key plays at run time through `LightStyles::patterns` (Chimera's `render::light_styles` steps any style's blocks, not only the atlas's animated ones), and the letter scale is Chimera's `map::style_levels` (Source's pattern conversion uses it too). Tests: `cs16` `light_styles` (scenario and `NetSim`), heavy `cs_militia_switched_lights_reach_the_client` and `flickering_styles_blend_by_their_letters`. No converter change.
- Converter version 3; the cs16 sets republished under builds 12934623
  and 15961496 (`sets cs16 --pack --sign`, `convert cs16 --sign`; the
  manifests snapshotted first): the maps' sets gained their sounds and
  sprites (1-14 files each), the HE's and smoke's their look sprites.

### 2026-11-25: M3, the HUD

- **Pictures** (`games/cs16/src/game/hud.rs`): `sprites/hud.txt`'s and
  each weapon's `sprites/weapon_<id>.txt`'s 640 entries cut out of frame
  0 of their atlases (`goldsrc_formats::spr`) as grey RGBA: the digits,
  cross, suits, kill icons by weapon id (`d_<id>`, the HE's `d_grenade`;
  the smoke grenade has none), `d_headshot`, `d_skull`, every weapon's
  `weapon`, `weapon_s` and `ammo`, and `sprites/640_pain.spr`'s four
  arcs; read only once their files are here (the asset store), never
  fetched. `parse_sprite_list` keeps the entries of a list that ends
  before its count (the install's `hud.txt`: 193 of 215).
- **HUD** (`games/cs16/ui/src/hud.rs`): hud.md's layout as pure draws
  (`draws`, unit tested on the spec's test cases), a pool of
  `HudGlyphMaterial` nodes added onto the frame in the HUD colour, at
  the window's physical pixels (no scaling); the fades (`Fades`: health
  and armour dim 100 and 228 on a change over 5 s, health 255 at 15 or
  less, the ammunition 200 on every change); the pain arcs from
  `DamageTaken`. `GameParts::hud` is `cs16` and the `GameClient` adds the
  HUD after the shell's (as DoD:S's); the kill feed is the app's with
  CS 1.6's icons (tinted 255 80 0) and its headshot mark registered. The
  crosshair stays the shell's. Capture:
  `apps/ultimate_gun_game/scenes/cs16_hud.scene`.
- **HUD, second part** (same files): the dynamic crosshair (four 1 pixel
  lines added onto the frame in 50 250 50, `GameDraws::crosshair` on so
  the shell's is off; the gap the held gun's `Hitscan` spread projected to
  the screen plus a base gap, opening at once, closing with a 0.1 s time
  constant; hidden dead and under a scope); the weapon selection (hud.md's
  bucket list from the shell's `WeaponMenu`: slot numbers, boxes under
  closed slots, the open slot's `weapon` pictures with the chosen one's
  `weapon_s` and `selection`; `cs16:hud_fastswitch`, 1 by default, 0
  makes the slot keys open it); the round timer (stopwatch, minutes, a
  two square colon, seconds; the match's `round_ends`, warning colour
  under 20 s, hidden with none or over 99:59); the 320 layout
  (`pictures(mount, 320)`: that layout's own atlases and `320_pain`, chosen
  by the window's width, digit and icon cells from the sprites' sizes).
  Capture: `apps/ultimate_gun_game/scenes/cs16_hud2.scene` at 1920x1080,
  640x480 and 320x240 (`+mp_timelimit 35` for the timer). The 320 files
  are not in the published `players` set yet: until it is republished a
  set-fed client has the 640 layout only.
- **HUD, spec's second part** (hud.md as of 2026-11-25, same files): the
  guesses of the first pass replaced by the spec's numbers. Crosshair:
  each weapon's base gap and step (`rule_of`) with the air, speed, duck
  and x1.4 rules (`base_gap`, integer products), the gap opening a step
  per shot up to 15 and closing 1.3 % + 0.1 pixel per drawn frame
  (`Fades::step`: per frame, not per second; a shot is the held clip
  falling), length (g - d) / 2 + 5, `cl_crosshair_size` scale bases,
  `cs16:cl_crosshair_color`, `_size` and `_translucent` (a bad value
  prints its usage), hidden for the four snipers and at fov 40 or less.
  Timer: stopwatch's left edge at W / 2, whole seconds truncated, two
  places of minutes, the colon's two 2 x 2 squares at alpha 100, always
  dim, flashing reddish at 20 s or less (phase s / 40 + 0.1 s kept between
  rounds), hidden with no limit (over a day) or the player dead. Menu: no
  timeout (`keep_open` holds the shell's clock; death closes it), columns
  from the radar's width + 10, rows from y 30, the highlighted weapon at
  192 like the rest, 20 x 4 ammunition bars (10 x 2 at 320), closed-slot
  boxes added; `cs16:hud_fastswitch` 0 by default. 320: the spec's worked
  positions are unit and node tests. Still ours: hud.md questions 2-3.
- **Served**: `players` (both builds) now carries the lists, the atlases
  their 640 entries name and the pain sprite (`sets::load_players` reads
  them: 53 files, 0.3 MB stored more); manifests snapshotted first.

### 2026-11-25: M2, water volumes

- **Water** (`goldsrc/engine/src/content/water.rs`, bsp.md "Contents",
  entities.md "func_water"): the world's hull 0 paths to a water, slime
  or lava leaf as convex regions (`solids::convex_regions_where` with
  `hull::liquid`), and each `func_water` or `func_illusionary` whose
  `skin` is a liquid's contents as its model's filled hull 0 regions at
  its origin (`hull::filled`); a named `func_water` (a mover) is a
  `MapData::water_movers` entry, so its volumes follow its node. Every
  game's movement swims in them (`MapWater`). Slime and lava are both
  slime volumes. The stock maps: 51 liquid entities (the 50 `func_water`
  and `cs_italy`'s `*13`) and 6 world liquid regions, on 10 maps; most is
  shallow (de_aztec's river 16 units: a wade), as_oilrig's sea 192 and
  de_survivor's 160 deep. Converter version 4: the served `map/<name>`
  entries need packing again.
- Tests: `goldsrc_engine` unit tests (a built tree's water leaf, a
  `func_water` at its origin and a named one moving), `cs16` `water`
  (a converted water leaf as the greybox tank: level 3, the 48 units/s
  sink, 100 up with jump, dry outside), heavy `water` (every stock
  liquid entity's centre in a volume; a player swims in as_oilrig's sea).
- Not drawn: the view under water (no spec says what GoldSrc shows);
  the spec's swim details beyond the shared swim (movement.md's up key,
  slime and lava speeds, currents) stay `cs16::game::movement`'s
  shortcuts.

### 2026-11-26: footsteps (specs/cs16/footsteps.md)

- **Lookup** (`goldsrc_formats::materials`): `Rule::Cs16` (up to 1024
  entries, 16 characters, a line counting by the spec's reading, sorted by
  name and searched by halving: `generic015v` is metal) beside
  Half-Life's (`Rule::HalfLife`, 12 characters, first match);
  `GoldSrcGame::materials` picks it. Heavy test on the install.
- **Sets** (`goldsrc_engine::content::sounds::add_steps`): every surface
  kind's step surface (wood, computer, glass, grass as concrete) and
  `ladder`, `wade` in the map's sounds, the feet as entries listing their
  files; the water's splash, bubble and gasp entries. Converter version
  bumped by its sources: **a cs16 republish is needed** (the served map
  entries gain surfaces and sounds).
- **Movement** (`chimera_movement::QuakeConfig::goldsrc_steps`, steps.rs):
  the 150 units/s gate with the 400 ms idle timer, 300 ms steps, +100 ducked
  or on a ladder, 600 ms wading with every fourth skipped on the shared
  counter, ladder 0.35, wade 0.65, slosh 0.5, concrete 0.5, dirt 0.55, vent
  0.7, ducked x 0.35, the foot flipped even when silent, standing up takes
  100 ms off, a jump's step at 1.0 from 150, a landing's timer 300,
  `mp_footsteps`. The walk key now multiplies after the clip (130); the
  golden run's hash was re-taken for it.
- Tests: cs16 `footsteps` (the spec's cases), heavy `footsteps`.
- **Surface probe** (2026-11-26, `Mover::record_surface`): the face a line
  from the box's middle 64 units straight down hits (28 under the feet
  standing, 46 ducked), as the spec has it; nothing hit: concrete (it was
  the ground trace's face, else the world face 6 units under the feet).

### 2026-11-26: shells (specs/cs16/shells.md)

- **Cases** (`games/cs16/src/game/weapons/shells.rs`, data/shells.toml): each
  shown shot's `Fired` event throws one case per bullet event, on every
  machine that shows it, at the gun's first-person offset from the eye
  (the local player's, `ViewModelSettings::right_hand` mirrors it; the
  Elites by shot) or the third-person one (20, -12, 4), with the spec's
  random speeds, half the shooter's velocity and the 0.1 s lead
  (`event_throw`); the M3, Scout and AWP at 0.45, 0.56 and 0.55 s, from the
  eye 16 ahead and 9 down at the view of that moment (`late_throw`, then
  9 units and 120 units/s to the left as `late_client`). Chimera got the
  generic `map::shells::ThrowShell` and `render::shells::throw` (a world
  shell, drawn and bounced as the view-model ports' are), and
  `weapon_models::shell` reads the four models; the five types
  (`pshell`, `rshell`, `rshell_big`, `shotgunshell`, and its shotgun-clink
  twin) join the kit with their bounce entries.
- Known shortcuts (module doc): the late cases are made by every machine
  from the shot's event, not by a server's shell message; the sideways
  directions (open questions 1 and 2) and the cases' physics (3: gravity,
  damping 0.45, spin, bounce sounds, a 0.2 s fade inside the 2.5 s) are
  ours.
- Tests: cs16 `shells` (the spec's test cases, the shot's messages),
  heavy `footsteps` (the models and sounds on the install). Capture:
  `apps/ultimate_gun_game/scenes/cs16_shells.scene`.

### 2026-11-26: water and the swim (specs/cs16/water.md, movement.md "Water")

- **Liquids** (`chimera_movement::liquids`, `QuakeConfig::liquid_hazards`):
  slime 4 and lava 10 times the water level on every command (`MapWaterVolume::
  lava`, set by `water::add_world`/`add_entity`), the 12 s of air, drowning
  3/4/5 a second (never absorbed), the gasps on surfacing (`pl_wade1` when
  the air ran out, `pl_wade2` after more than 3 s under, on the voice
  channel), the bubbles (1 in 32 a command then a roll against the air
  left, `pl_swim1`-`4` at 0.8), the recovery of 10 every 2 s; the
  splash and swim stroke are `pl_wade1`-`4`.
- **The swim** (movement.md "Water"): the wish from the keys plus the up
  key (`+moveup`/`+movedown`, the named actions, 320 clipped with the other
  keys to the max speed, then x 0.52 walking, x 0.333 ducked), sinking at 48
  with no input, the jump's lift 100/80/50, 96 rising, 174.4 horizontal
  with forward and jump; currents (`MapWaterVolume::current`: 50 x the
  water level, counted as the spec's code has it; filled by `water::current_of` from a leaf's or a `func_water`'s current contents -9 to -14, which count as water; no stock map has one, a unit test builds them).
- **The view**: the drawn eye kept 4 units from the surface
  (`Mover::view_eye`, `MovementState::view_shift`: the camera only, shots and traces start at `eye_offset`, the real eye; the golden run re-taken, its hash held the shifted eye); the fog under a liquid
  is the liquid texture's palette (entry 3 its colour, the red of entry 4
  its thickness: `water::fog_material`, fog-only water materials on the
  liquid faces); the curve is ours (open question 1), and as_oilrig's
  `!water2b` has thickness 0, so its fog is faint. No colour tint or
  room effect (open questions 2 and 3). Capture:
  `apps/ultimate_gun_game/scenes/cs16_water_fog.scene`.
- Tests: cs16 `water` (steady speeds, forward and jump, slime and lava, the
  drowning timeline with the recovery and gasps, a current, the drawn
  eye), goldsrc_engine `water` (the fog), heavy `water` (as_oilrig's fog).
- Converter version changes again: **a cs16 republish is needed** (the
  liquid fog materials and the step sets in the served maps).
