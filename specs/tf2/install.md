# Team Fortress 2: install, maps, materials, models and item data (M0)

Source basis: the dedicated server's content install (Steam app 232250),
read in place: its `steam.inf` and app manifest, `tf/gameinfo.txt`, the
VPK directory files and the archives that are present, the BSP headers,
entity lumps, game lumps and pak lumps of all 238 shipped maps, the
material (`.vmt`) text of the misc VPK, model headers read with our own
model dumper and a header reader written for this session, the plain-text
scripts (`items_game.txt`, sound scripts). Public format documentation
(Valve Developer Community pages on BSP, VPK, MDL, VMT and TF2 entities;
the community TF2 wiki) for what the data means. No game or server was
started, nothing was decrypted, no code of the game or engine was read.
Status: draft

## Summary

TF2 is a Source engine game of the same family as CS:S and HL2:DM: the
same VPK, VBSP 20 and MDL 48 formats, the same shader library plus a
few TF2-only shader features, KeyValues text everywhere. What a port
must handle that HL2:DM did not: (1) the dedicated server install has
**no textures and no sounds** (only the directory files of those VPKs),
so a renderable, audible mount needs the retail client (app 440);
(2) 183 of the 238 maps have **LZMA-compressed lumps**, a compressed
static-prop game lump (version 10, 72-byte entries) and a pak lump
whose files are **LZMA-compressed zip entries**; (3) first-person
weapons are **two models**: per-class arms (`c_<class>_arms.mdl`,
animations in an include model) with the weapon model (`c_<weapon>.mdl`)
bone-merged onto them, the same weapon model the third-person player
holds; (4) weapon identity comes from `scripts/items/items_game.txt`
(plain text, about 11 600 items with prefab inheritance), while the
per-weapon and per-class numbers are in **encrypted** `.ctx` scripts
that we never read; (5) spawns are `info_player_teamspawn` with
`TeamNum` 2 (RED) / 3 (BLU), switched by control-point ownership or
by map logic.

## Units and conventions

- Source units (1 unit = 1 inch, public "Dimensions" documentation),
  Z up, X forward, Y left; degrees; seconds.
- Tick: TF2 servers run at the Source multiplayer default of **0.015 s
  (66.67 Hz)** per public documentation and community knowledge; this
  spec did not run anything. Confirm at M0 by reading a running
  server's `status`/`net_graph`, or the tick interval stored in any
  recorded `.dem` header (Open questions 1).
- Team numbers (map data): 0 unassigned/neutral, 1 spectator, **2 RED,
  3 BLU**. Confirmed in the maps: `filter_activator_tfteam` named
  `filter_red_team` has `TeamNum` 2 and `filter_blue_team` 3 on
  ctf_2fort; `item_teamflag` `flag_red` 2, `flag_blue` 3.
- "install" below means a value read from the dedicated server's files;
  "public" means from public documentation; "inferred" is a reading of
  the data that a test or the live game must confirm.

## Part 1: install layouts

### 1.1 The dedicated server install (app 232250), as found

| What | Value |
|---|---|
| `tf/steam.inf` | PatchVersion, ClientVersion, ServerVersion **11191711**; ProductName tf; appID 440; ServerAppID 232250 |
| App manifest (`steamapps/appmanifest_232250.acf`) | name "Team Fortress 2 Dedicated Server"; **TargetBuildID 26334371**; `buildid` reads 0 and `UpdateResult` 42 with a partly staged update (the download was not finalised in Steam's eyes; the files themselves are in place). Depots 232250 (about 15.1 GB) and 232256 (about 156 MB) |
| Size on disk | about 15 GB |
| Top level | `srcds_run`, `srcds_run_64`, `srcds_linux` (32-bit), `srcds_linux64` (64-bit), `bin/` (24 engine libraries, plus `bin/linux64/`), `hl2/`, `platform/`, `tf/`, `steamapps/` |
| Game libraries | `tf/bin/server_srv.so` (32-bit) and `tf/bin/linux64/server_srv.so` (64-bit); no client library |
| `tf/` loose | `gameinfo.txt`, `steam.inf`, `cfg/` (configs, 30+ map cycles), `maps/` (238 `.bsp`, plus an empty `workshop/` folder), `resource/` (152 files: fonts `tf.ttf`, `tf2.ttf`, `tf2build.ttf`, `tf2professor.ttf`, `tf2secondary.ttf`, `ocra.ttf`; `tf_<language>.txt` localisation; close captions; game icons), `scripts/items/items_game.txt` (+ `.sig`) and `scripts/protodefs/`, `materials/models/player/items/templates/standard.vmt` (one file), `custom/` and `custom/workshop/` (readmes only), `download/` (empty), `media/` (empty), `workshop/` |
| `hl2/` loose | `media/`, `resource/` only (no `scripts/`, unlike HL2:DM's install) |

VPKs (all directory format version 2; "absent" means the directory file
lists data archives that are not on disk):

| VPK | Files | Content | Archives |
|---|---|---|---|
| `tf/tf2_misc_dir.vpk` | 104 343 | vtx 43 221, vmt 26 086, mdl 14 452, vvd 14 408, phy 4 757, res 778, txt 201, pcf 135, **ctx 88**, vfe 64, nav 46, pop 34; **no vtf** | 28 (`_000`–`_027`), 3 287 373 647 bytes, all present |
| `tf/tf2_textures_dir.vpk` | 29 669 | vtf only (about 12.8 GB) | 108, **absent** |
| `tf/tf2_sound_misc_dir.vpk` | 3 230 | wav 2 757, mp3 472, 1 midi | 12, **absent** (a `.vpk.sound.cache` is present) |
| `tf/tf2_sound_vo_english_dir.vpk` | 12 728 | mp3 12 668, wav 60 | 3, **absent** (`.sound.cache` present) |
| `hl2/hl2_misc_dir.vpk` | 18 946 | Half-Life 2's models, materials (text), shaders, scripts | 4, present |
| `hl2/hl2_textures_dir.vpk` | 5 032 | vtf | 11, absent |
| `hl2/hl2_sound_misc_dir.vpk` | 2 970 | wav, mp3 | 3, absent |
| `hl2/hl2_sound_vo_english_dir.vpk` | 2 553 | wav | 5, absent |
| `platform/platform_misc_dir.vpk` | 512 | VGUI resources, one TrueType collection | 1, present |

Consequences: the server install has every model, material text, map,
script, HUD layout and font, but **no texture and no sound data**. A
loader can parse and place everything (and run collision and game
logic) from it; drawing textured surfaces or playing sounds needs the
client's archives. Each map's own pak carries that map's custom
textures (ctf_2fort: 174 vtf), so map-specific surfaces do render from
the server install alone; stock texture paths (`materials/concrete/…`,
the player and weapon textures) do not.

### 1.2 The retail client (app 440, folder `Team Fortress 2`), from public descriptions

Same tree, plus (public, to confirm on the Windows PC; Open questions 2):

- the data archives of `tf2_textures`, `tf2_sound_misc`,
  `tf2_sound_vo_english` and of Half-Life 2's textures and sounds;
- the client game library in `tf/bin/` (Windows 64-bit in
  `tf/bin/x64/`, Linux 64-bit in `tf/bin/linux64/`) and the
  launchers at the root (`tf_win64.exe` / `tf.exe` on Windows,
  `tf_linux64` / `hl2.sh` on Linux; older builds `hl2.exe`);
- loose `tf/media/` (startup video), `tf/materials/` additions, and
  per-user folders (`tf/cfg/config.cfg`, `tf/custom/`, `tf/download/`).

Fonts, HUD layouts (`resource/ui/*.res`, 710 files in the misc VPK;
`scripts/hudlayout.res`, `scripts/hudanimations_tf.txt` and its
manifest), VGUI materials (2 558 `materials/vgui/` and 609
`materials/hud/` entries in the misc VPK, their textures in the
textures VPK: 3 055) are in **both** layouts; only their textures need
the client.

### 1.3 Install check

A folder is a TF2 install when all of these exist (present in both
layouts):

1. `tf/gameinfo.txt` whose `game` is "Team Fortress 2" and SteamAppId 440;
2. `tf/tf2_misc_dir.vpk` and `tf/tf2_misc_000.vpk`;
3. `tf/maps/ctf_2fort.bsp`;
4. `hl2/hl2_misc_dir.vpk`.

Then classify: **full content** when `tf/tf2_textures_000.vpk` and
`tf/tf2_sound_misc_000.vpk` exist (client), **server content** when
they don't (server: render untextured or fall back, play no sound).
The check must not require `tf/bin/client*` (absent from the server)
nor `srcds_*` (absent from the client).

### 1.4 `tf/gameinfo.txt`

Keys: game "Team Fortress 2", type multiplayer_only, `nomodels` 1,
`nohimodel` 1, `nocrosshair` 0, `advcrosshair` 1, `supportsvr` 1,
`nodegraph` 0, `GameData` "tf.fgd", `InstancePath` "maps/instances/",
hidden maps `test_speakers`, `test_hardware`, SteamAppId 440, and a
`ToolsEnvironment` block (tools only).

Search paths, in order (first wins). `|all_source_engine_paths|` is the
install root; `|gameinfo_path|` is `tf/`; a missing file is skipped.

| # | Path ids | Path | On the server install |
|---|---|---|---|
| 1 | game, mod, custom_mod | `tf/custom/*`: every VPK and subfolder, alphabetical, scanned at start | only `custom/workshop/` (a readme) |
| 2 | game_lv | `tf/tf2_lv.vpk` (low-violence content, only mounted for low-violence players) | absent |
| 3 | game, mod | `tf/tf2_textures.vpk` | directory only |
| 4 | game, mod | `tf/tf2_sound_vo_english.vpk` | directory only |
| 5 | game, mod | `tf/tf2_sound_misc.vpk` | directory only |
| 6 | game, mod, vgui | `tf/tf2_misc.vpk` | yes |
| 7 | game | `hl2/hl2_textures.vpk` | directory only |
| 8 | game | `hl2/hl2_sound_vo_english.vpk` | directory only |
| 9 | game | `hl2/hl2_sound_misc.vpk` | directory only |
| 10 | game, vgui | `hl2/hl2_misc.vpk` | yes |
| 11 | platform, vgui | `platform/platform_misc.vpk` | yes |
| 12 | mod, mod_write, default_write_path | `tf/` (the gameinfo folder) | yes |
| 13 | game, game_write | `tf/` | yes |
| 14 | gamebin | `tf/bin` | binaries |
| 15 | game | `hl2/` (loose) | `media/`, `resource/` |
| 16 | platform | `platform/` (loose) | yes |
| 17 | game, download | `tf/download` | empty; searched last |

A VPK path is written without `_dir`: the file on disk is
`<name>_dir.vpk` plus `<name>_NNN.vpk` archives. An entry containing
`_english` gets a language-specific entry inserted in front of it for
other languages (public; the gameinfo's own comment says so too).
While a map is loaded its pak lump is searched before everything.

Differences from HL2:DM's gameinfo (specs/hl2dm/source_deltas.md, Part 2):
no `hl2_complete/` layer; a `custom_mod` path id and a `game_lv`
low-violence VPK; `vgui` ids on the misc VPKs; `nomodels` 1 (no
player-model picker) and `nocrosshair` 0 with `advcrosshair` 1; all
TF2 VPKs come **before** any loose folder, `tf/` loose included, as in
HL2:DM.

### 1.5 Consequences for the mount

- Order as the table; a missing archive must not fail the mount: the
  file is "listed but unreadable", and lookups for it should fall
  through to the next search path (Open questions 3), then to a
  placeholder texture or silence.
- HL2's VPKs provide shared content (HL2 props, `materials/tools/`,
  decals, some world textures: ctf_2fort uses 7 materials from
  `hl2_misc`, cp_badlands 1).
- `tf/materials/` and `tf/scripts/` loose folders are mostly empty: the
  materials and scripts are in `tf2_misc`; `items_game.txt` is the
  exception (loose only, not in a VPK).

## Part 2: maps

### 2.1 All maps

238 `.bsp` files in `tf/maps/` (loose), from 0.9 MB (cp_cloak) to 75 MB
(2koth_abbey). By prefix:

| Prefix | Count | Prefix | Count |
|---|---|---|---|
| `cp_` | 59 | `zi_` | 6 |
| `koth_` | 45 | `vsh_` | 6 |
| `pl_` | 41 | `pass_` | 3 |
| `ctf_` | 25 | `tr_` | 2 |
| `arena_` | 13 | `sd_` | 2 |
| `pd_` | 12 | `tc_`, `tow_`, `rd_` | 1 each |
| `plr_` | 9 | others: `2koth_abbey`, `background01`, `cppl_gavle`, `htf_marshlands`, `itemtest` | 5 |
| `mvm_` | 7 | **total** | **238** |

- **BSP version**: all 238 are VBSP **20**. Map revisions from 1 to 25 479.
- **LZMA lumps**: **183 maps** store most lumps LZMA-compressed (47–51
  lumps each, including entities, planes, faces, lighting, pak excluded);
  the other 55 are uncompressed. Our loader's existing LZMA lump support
  (community-map path) is needed for stock content here; the compressed
  lump header is the public "LZMA" one (magic, uncompressed size,
  compressed size, 5 property bytes).
- **Lighting**: every map has LDR lighting (lump 8). 237 also carry HDR
  lighting (lump 53); **cp_cloak has no HDR lighting**.
- **Static props** (`sprp` game lump) versions: **10** in 199 maps (183
  of them with the game lump flagged compressed, 16 uncompressed), **6**
  in 36, **5** in 2 (background01, cp_cloak), **7** in 1
  (pl_cactuscanyon). Version 10 entries are **72 bytes** each in
  ctf_2fort and cp_badlands. Detail props (`dprp`) are version 4
  everywhere; 130 maps also have `dplt`/`dplh` (detail prop lighting,
  LDR/HDR), version 0. Some maps list empty game lump entries (id of
  four zero bytes, length 0): skip them.
- **Pak lump**: present in every map; its zip entries use **compression
  method 14 (LZMA)**, not stored, in both maps examined (ctf_2fort 4 861
  entries: 189 vmt, 174 vtf, 4 498 vhv; cp_badlands 2 886: 70 vmt, 444
  vtf, 2 372 vhv). `.vhv` files are per-vertex static-prop lighting
  (`sp_<n>.vhv`, `sp_hdr_<n>.vhv`); cubemap textures and patched
  materials make up the rest.
- **Displacements**: in all but 3 maps.
- **Navigation meshes**: no `.nav` next to the maps; 46 `.nav` files in
  the misc VPK (bots; not needed to walk a map).
- Every map has at least one `info_player_teamspawn` (12 594 in all).

### 2.2 ctf_2fort and cp_badlands

| | ctf_2fort | cp_badlands |
|---|---|---|
| Map revision | 4067 | 1061 (`mapversion`; header revision 173) |
| File bytes | 22 899 183 | 25 981 141 |
| LZMA-compressed lumps | 50 | 48 |
| `skyname` | sky_tf2_04 | sky_badlands_01 |
| `detailmaterial` / `detailvbsp` | detail/detailsprites_2fort / detail_2fort.vbsp | detail/detailsprites / detail.vbsp |
| Other worldspawn keys | `world_mins` −3536 −4720 −560, `world_maxs` 2624 4720 1728, `maxpropscreenwidth` −1, `minoccluderarea_x360` 110 | `world_mins` −14892 −4864 −14672, `world_maxs` 4096 15420 1008, `maxpropscreenwidth` −1 |
| 3D skybox | yes: one `sky_camera`, scale 16, fog on (start −6000, end 24000, colour 94 104 140) | yes: scale 16, fog on (start 500, end 12000, colour 113 115 142) |
| Displacements | 232 | 1 191 |
| Faces / HDR faces | 15 768 / 15 768 | 13 845 / 13 845 |
| Lighting bytes LDR / HDR (decompressed) | 9 604 712 / 9 604 712 | 10 351 660 / 10 351 660 |
| Cubemaps | 87 | 225 |
| Overlays | 237 | 124 |
| World lights LDR / HDR | 1 740 / 1 740 | 237 / 237 |
| Static props (v10) | 2 265 props, 318 model names | 1 232 props, 145 model names |
| Water | yes: 6 Water materials (the moat, under the bridge, one in the 3D sky), leaf water data, `water_lod_control` (cheap water from 500 to 800 units) | no water brushes (leaf water data empty), but a `water_lod_control` |
| Entities | 1 097 | 674 |
| `light_environment` | `_light` 216 207 194 700, `_ambient` 200 202 230 60, pitch −45, HDR values −1 (use LDR) | present (1) |
| Fog controllers | 6 (`fog_controller` and per-area interiors and sewer) | 1 |

Entity classes (count per map; "–" = none):

| Class | 2fort | badlands | Class | 2fort | badlands |
|---|---|---|---|---|---|
| ambient_generic | 4 | – | info_target | 15 | 4 |
| color_correction | 1 | 1 | infodecal | 19 | – |
| env_fog_controller | 6 | 1 | item_ammopack_medium | 4 | 4 |
| env_lightglow | 54 | 27 | item_ammopack_small | 2 | 18 |
| env_soundscape | 6 | 2 | item_healthkit_medium | 4 | 4 |
| env_soundscape_proxy | 44 | 28 | item_healthkit_small | 2 | 14 |
| env_sprite | 63 | 6 | item_teamflag | 2 | – |
| env_tonemap_controller | 1 | 1 | keyframe_rope | 88 | 48 |
| filter_activator_tfteam | 2 | 2 | light | 2 | 3 |
| func_areaportal | 29 | 58 | light_environment | 1 | 1 |
| func_brush | 50 | 2 | light_spot | 286 | 140 |
| func_capturezone | 2 | – | logic_auto | 2 | 2 |
| func_door | 14 | 12 | logic_case | – | 1 |
| func_dustmotes | 1 | 1 | logic_relay | – | 5 |
| func_illusionary | 13 | 88 | math_counter | – | 2 |
| func_nobuild | 2 | – | move_rope | 27 | 16 |
| func_occluder | 13 | – | point_devshot_camera | 36 | – |
| func_regenerate | 8 | 4 | point_spotlight | 138 | 2 |
| func_respawnroom | 6 | 6 | prop_dynamic | 27 | 31 |
| func_respawnroomvisualizer | 8 | 8 | prop_physics_multiplayer | 14 | – |
| func_rotating | 3 | – | shadow_control | 1 | 1 |
| func_smokevolume | 4 | – | sky_camera | 1 | 1 |
| game_end | – | 1 | team_control_point | – | 5 |
| game_intro_viewpoint | 9 | – | team_control_point_master | – | 1 |
| game_round_win | – | 1 | team_round_timer | – | 1 |
| game_text | – | 2 | tf_gamerules | – | 1 |
| info_null | 1 | – | tf_logic_competitive | – | 1 |
| info_observer_point | 7 | 6 | trigger_capture_area | – | 5 |
| info_particle_system | 5 | – | trigger_multiple | 36 | 12 |
| info_player_start | 2 | – | water_lod_control | 1 | 1 |
| info_player_teamspawn | 30 | 92 | worldspawn | 1 | 1 |

### 2.3 Spawns: `info_player_teamspawn`

Keys seen and their meaning (public entity documentation for the
meaning; values from the two maps):

| Key | Meaning | ctf_2fort | cp_badlands |
|---|---|---|---|
| `TeamNum` | team allowed to spawn here: 2 RED, 3 BLU, 0 any (here only the match-summary points) | 15 × 2, 15 × 3 | 40 × 2, 40 × 3, 12 × 0 |
| `StartDisabled` | 1 = starts disabled (inputs Enable/Disable toggle it) | all 0 | all 0 |
| `targetname` | for logic | `spawn_red_1` 8, `spawn_red_2` 7, `spawn_blue_2` 7, 8 BLU unnamed | `spawn_red_1` 16, `spawn_red_2` 12, `spawn_red_3` 12, `spawn_blue_1` 16, `spawn_blue_2` 12, `spawn_blue_3` 12 |
| `controlpoint` | name of a `team_control_point`: the spawn is used only while its team owns that point (public; inferred to be how badlands moves spawns forward, since no logic targets the spawns) | – | `cap_red_1` 16, `cap_blue_1` 16, `cap_center` 24, `cap_red_2` 12, `cap_blue_2` 12 |
| `spawnflags` | which classes may spawn here, bits 1 Scout, 2 Sniper, 4 Soldier, 8 Demoman, 16 Medic, 32 Heavy, 64 Pyro, 128 Spy, 256 Engineer (public) | key absent | 511 (all nine) on all 92 |
| `SpawnMode` | 0 normal, 1 triggered: only used by players sent through a `trigger_player_respawn_override` naming it (public) | – | 0 on the 12 team-0 points |
| `MatchSummary` | position for the end-of-match summary scene (1 or 2; inferred: losing/winning team; Open questions 5) | – | 6 × 1, 6 × 2, all `TeamNum` 0 |
| `round_redspawn`, `round_bluespawn` | `team_control_point_round` this spawn belongs to, per team (public) | – | – |

Badlands' pairs: RED `spawn_red_1` on `cap_red_1` (home), `spawn_red_2`
on `cap_center`, `spawn_red_3` on `cap_blue_2`; BLU mirrored. So with
the start ownership (`cap_red_1`, `cap_red_2` owned by RED,
`cap_blue_*` by BLU, `cap_center` neutral) the active spawns at round
start are `spawn_red_1` (16) and `spawn_blue_1` (16).

On ctf_2fort all 30 are enabled from the start and nothing targets
them: a player spawns at any spawn of its team. 2fort's teamspawns have
no `spawnflags`; since every class spawns there, an absent key must
mean "all classes" (inferred; Open questions 4).

`info_player_start` (2 on ctf_2fort, 120 in 47 maps) is a fallback;
TF2 does not spawn players there when team spawns exist (public).

### 2.4 Other TF2 classes a map loader meets (public entity docs; one line each)

| Class | In all maps (count / maps) | What it is |
|---|---|---|
| `func_respawnroom` | 1 055 / 232 | brush volume of a team's spawn room (`TeamNum`); players inside can change class freely; ends uber/cloak etc. for enemies |
| `func_respawnroomvisualizer` | 1 766 / 215 | the visible (and, with `solid_to_enemies` 1, enemy-solid) door brush of a respawn room named by `respawnroomname` |
| `func_regenerate` | 1 015 / 204 | resupply volume: refills health and ammo of team `TeamNum`; `associatedmodel` names the cabinet `prop_dynamic` to animate |
| `item_healthkit_small` / `_medium` / `_full` | 1 546 / 1 826 / 213 | health pickups (20.5 %, 50 %, 100 % of max health, public); respawn 10 s; `AutoMaterialize`, `TeamNum` (0 = both) |
| `item_ammopack_small` / `_medium` / `_full` | 933 / 2 000 / 669 | ammo pickups (20.5 %, 50 %, 100 %, public), same keys |
| `item_teamflag` | 80 / 40 | the intelligence: `TeamNum`, `GameType` (0 CTF), `ReturnTime` 60 on 2fort, `flag_model` `models/flag/briefcase.mdl`, `trail_effect`, `NeutralType` |
| `func_capturezone` | 87 / 48 | where a carried flag is captured; `TeamNum`, `capturepoint` |
| `team_control_point` | 534 / 181 | a capture point: `point_index`, `point_default_owner`, `point_printname`, models and icons per team (`team_model_N`, `team_icon_N`, `team_bodygroup_N`), `point_warn_sound` |
| `team_control_point_master` | 173 / 173 | ties points into a round and decides the winner |
| `team_control_point_round` | 91 / 50 | a multi-round (stage) grouping of points |
| `trigger_capture_area` | 390 / 180 | brush where players capture `area_cap_point`; `area_time_to_cap`, `team_cancap_N`, `team_numcap_N`, `team_spawn_N` (respawn time change) |
| `team_round_timer` | 155 / 144 | round clock: `timer_length`, `setup_length`, `max_length`, `show_in_hud` |
| `team_train_watcher` | 85 / 56 | payload cart progress (with `func_tracktrain`/`path_track`) |
| `mapobj_cart_dispenser` | 84 / 56 | the payload cart's healing dispenser |
| `tf_gamerules` | 201 / 201 | map-set rules (`hud_type`, `ctf_overtime`), takes inputs |
| `tf_logic_koth`, `_arena`, `_competitive`, `_holiday`, `_mann_vs_machine`, `_player_destruction`, `_robot_destruction`, `_multiple_escort`, `_medieval`, `_training_mode`, `_hybrid_ctf_cp`, … | 49, 10, 20, 67, … | game-mode switches; their presence selects the mode |
| `filter_activator_tfteam` | 505 / 226 | filter by team (`TeamNum`) |
| `func_nobuild` | 2 061 / 197 | Engineer may not build inside |
| `info_observer_point` | 1 295 / 232 | spectator camera positions |
| `game_intro_viewpoint` | ctf_2fort 9 | camera points of the map's intro presentation |
| `trigger_add_tf_player_condition`, `trigger_stun`, `trigger_ignite`, `trigger_catapult`, `trigger_player_respawn_override` | 263, 42, 72, 72, 22 | TF2-specific triggers |
| `func_respawnflag`, `func_flagdetectionzone`, `func_upgradestation`, `func_croc`, `func_powerupvolume` | 52, 19, 17, 16, 8 | special volumes (flag reset, flag detection, MvM upgrades, crocodiles, Mannpower) |
| `tf_halloween_gift_spawn_location`, `tf_spell_pickup`, `tf_zombie_spawner`, `tf_generic_bomb`, `halloween_*` | 635, 437, 148, 139 | holiday content (Halloween maps) |
| `bot_hint_*`, `bot_generator`, `bot_roster`, `tf_point_nav_interface` | – | bot and MvM support |

### 2.5 What a "walk the map" loader needs

Needed: the world (LZMA-aware lumps, displacements with
WorldVertexTransition), static props (sprp versions 5, 6, 7 and 10),
the pak (LZMA zip entries) mounted first, the 3D skybox (`sky_camera`)
and 2D sky, `info_player_teamspawn` filtered by `TeamNum` and
`StartDisabled` (and, for cp maps, `controlpoint` ownership at round
start), solid brush entities that block (`func_door` closed,
`func_brush`, `func_respawnroomvisualizer` as enemy-solid, else
passable), water (2fort), `light_environment` and fog controllers for
look. Can be ignored at first: all `item_*` pickups (draw their models
only), objectives (flags, points, capture areas, timers, gamerules,
`tf_logic_*`), respawn rooms and resupply, `func_nobuild`, soundscapes,
observer points, intro viewpoints, ropes, sprites and glows, particles,
bot hints, holiday entities.

## Part 3: materials and shaders

### 3.1 World materials of the two maps

Resolved through the map's pak, then `tf2_misc`, then `hl2_misc`, with
`patch` materials followed to their include (shader names compared
case-insensitively; materials list `LightMappedGeneric`, `UnLitGeneric`
spellings too). None missing.

| Shader | ctf_2fort | cp_badlands |
|---|---|---|
| LightmappedGeneric | 272 | 146 |
| UnlitGeneric | 8 | 4 |
| Water | 6 | – |
| WorldVertexTransition | 4 | 1 |
| VertexLitGeneric | 1 (a wood decal) | – |
| **total** (texdata names) | 291 | 151 |
| found in pak / tf2_misc / hl2_misc | 123 / 161 / 7 | 41 / 109 / 1 |

Every one of these is covered by specs/cs_source/shaders.md and
water.md. Skies are `Sky`-shader materials (192 in the misc VPK, e.g.
`skybox/sky_well_01up`).

### 3.2 Shaders in the whole misc VPK

| Shader | Materials | Covered by our specs? |
|---|---|---|
| UnlitGeneric | 12 674 | yes (mostly backpack/UI icons) |
| VertexLitGeneric | 10 571 | yes (shaders.md, shaders_phong.md) |
| LightmappedGeneric | 1 432 | yes |
| SpriteCard | 710 | **no** (particle shader) |
| Sky | 192 | yes |
| WorldVertexTransition | 138 | yes |
| Refract | 86 | partly (water.md, visual_entities.md) |
| Sprite | 81 | yes (sprites_dust.md) |
| UnlitTwoTexture | 61 | named in hl2dm |
| Water | 27 | yes |
| replacements | 22 | not a shader: material replacement tables |
| Subrect | 19 | not a shader: a sub-rectangle of another material (decals) |
| Modulate | 16 | yes |
| EyeRefract | 15 | **no** (bosses: Merasmus, etc.; class eyes are VertexLitGeneric) |
| DecalModulate | 12 | yes |
| Cable | 8 | yes |
| VertexLitGeneric_DX6 | 4 | fallback name |
| LightmappedReflective | 2 | yes (opaque approximation) |
| Cloak | 2 | **no** (sniper lens cloak) |
| pyro_vision, BlurFilterX/Y, color_projection, screenspace_general, downsample_nohdr, SkyFog, wireframe, debugluxels, translucentlightmap | 1–2 each | post-processing / editor; ignore |

### 3.3 Character and weapon material parameters

All player, arms and weapon materials are VertexLitGeneric on the Phong
path of specs/cs_source/shaders_phong.md (phong with a lightwarp,
normal map or base-alpha mask). Values seen:

| Parameter | scout_red (body) | scout_head_red | scout_hands (arms) | c_scattergun | c_rocketlauncher | c_shotgun | c_minigun |
|---|---|---|---|---|---|---|---|
| `$bumpmap` | scout_normal | shared 16 × 16 flat normal | scout_hands_normal | – | – | – | – |
| `$phong` | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
| `$phongexponent` | 20 | 20 | 10 | (texture) | (texture) | (texture) | (texture) |
| `$phongexponenttexture` | – | – | – | `…_exp` | `…_exp` | `…_exp` | `…_exp` |
| `$phongexponentfactor` | – | – | – | 195 | 100 | 150 | 80 |
| `$phongboost` | 0.3 | 0.3 | 0.05 | 6 | 2.75 | 2.5 | 4 |
| `$phongalbedotint` | – | – | – | 1 | 1 | 1 | 1 |
| `$phongfresnelranges` | [.3 1 8] | [.3 1 8] | [1 5 60] | [.3 .5 3] | [.3 .5 3] | [.3 .5 3] | [.3 .5 3] |
| `$lightwarptexture` | models/player/pyro/pyro_lightwarp | same | models/player/engineer/engineer_hands_lightwarp | models/lightwarps/softened_weapon_lightwarp | same | same | same |
| `$halflambert` | 0 | 0 | 1 | 1 | 1 | 1 | 1 |
| `$basemapalphaphongmask` | – | – | – | 1 | 1 | 1 | 1 |
| `$rimlight` / exponent / boost | 1 / 4 / 2 | 1 / 4 / 2 | 1 / 4 / 0.1 | 1 / 1 / 1.5 | 1 / 2 / 2.5 | 1 / 1 / 2.25 | 1 / 2 / 2 |
| `$rimmask` | – | – | – | 1 | 1 | 1 | 1 |
| `$envmap` / `$envmaptint` | – | – | – | env_cubemap / .085 | env_cubemap / .022 | env_cubemap / .05 | env_cubemap / .025 |
| `$detail` (burning) | effects/tiledfire/fireLayeredSlowTiled512, scale 5, blend factor .01, blend mode 6 | same, scale 1 | – | – | – | – | – |
| `$cloakPassEnabled` | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
| `$sheenPassEnabled` | – | – | – | 1 (`$sheenmap` cubemaps/cubemap_sheen001, mask Effects/AnimatedSheen/animatedsheen0) | 1 | 1 | 1 |
| `$yellow` | 0 | 0 | – | 0 | 0 | 0 | 0 |

Meaning (public material docs): `$lightwarptexture` replaces the
half-Lambert curve by a ramp (TF2's painted look); `$rimlight` adds the
rim term; `$basemapalphaphongmask` takes the specular mask from the
base alpha; `$phongexponenttexture` with `$phongexponentfactor` gives a
per-texel exponent and (green) albedo tint, (alpha) rim mask with
`$rimmask`; all of it is in shaders_phong.md. **Not in our specs**:
the cloak pass (`$cloakPassEnabled`, a refractive second pass driven by
the Spy's cloak), the sheen pass (`$sheenPassEnabled` and `$sheenmap*`:
an animated cube-map sheen for killstreak weapons), `$detailblendmode`
6 used as a burning overlay whose factor a proxy drives, `$yellow`
(Jarate tint), and the paint parameters of cosmetics
(`$blendtintbybasealpha`, `$blendtintcoloroverbase`, `$colortint_base`,
`$colortint_tmp`, `$color2`; template
`materials/models/player/items/templates/standard.vmt`). With all
proxies at rest (not burning, not cloaked, no Jarate, no paint, no
sheen) every one of them is a no-op, so a first renderer can skip them.

Team colour is **not** a material parameter on players: it is the model's
skin family (section 4.4). Cosmetic and some weapon paint uses
`$color2` via the ItemTintColor proxy.

Material proxies seen (names only, with counts of materials using
them): YellowLevel 7 731 (Jarate tint into `$yellow`), invis 7 548 and
spy_invis 97, weapon_invis 59, building_invis 19, vm_invis 1 (cloak
amount), AnimatedTexture 7 037 (the fire detail frames, 30 fps),
BurnLevel 6 876 (burning amount into `$detailblendfactor`), Equals /
Multiply / Add / Subtract / Divide / Clamp / LessOrEqual / Sine /
LinearRamp / UniformNoise / SelectFirstIfNonZero (generic arithmetic),
ItemTintColor 3 939 (paint colour), ModelGlowColor 875 (team or effect
glow into `$glowcolor`), AnimatedWeaponSheen 795 (killstreak sheen),
WeaponSkin 174 (war-paint skins), InvulnLevel 59 (Über effect),
CommunityWeapon 42, StatTrakDigit / StatTrakIllum / StatTrakIcon
(kill counters), TextureScroll 148, TextureTransform 56, WaterLOD 26,
ShieldFalloff 6, StickybombGlowColor 6, PlayerTeamMatch 5,
PlayerProximity 5, SniperRifleCharge 3, BuildingRescueLevel 2,
TeamTexture 1, BenefactorLevel 1, HeartbeatScale 1,
CustomSteamImageOnModel 18, MaterialModify 2.

## Part 4: models

### 4.1 MDL versions

| Group | Versions (files) |
|---|---|
| `models/player/*.mdl` (classes and their animation models) | 48 (27) |
| `models/weapons/c_models/c_*_arms.mdl` | 48 (9) |
| `models/weapons/c_models/` (other) | 48 (317) |
| `models/weapons/v_models/` (legacy) | 48 (37) |
| `models/weapons/w_models/` | 48 (60), 45 (19), 46 (1) |
| cosmetics (`models/player/items/`, `models/workshop/`) | 48 (10 483) |
| `models/props*` in tf2_misc | 48 (1 801), 44 (526), 45 (243), 46 (27), 47 (1) |
| other in tf2_misc | 48 (736), 44 (143), 45 (12), 46 (5), 47 (5) |
| hl2_misc (shared) | 44 (2 116), 48 (68), 45 (6), 46 (9), 47 (2) |

So mdl_v48.md is the main path and its §9 (45–47) is needed for props.

### 4.2 The `c_` rig versus `v_`

CS:S and HL2:DM use one view model per weapon (`v_*.mdl`) holding arms
and gun and its own sequences (specs/cs_source/view_models.md). TF2's
first person is **two models**:

1. the class's arms, `models/weapons/c_models/c_<class>_arms.mdl`
   (scout, soldier, pyro, demo, heavy, engineer, medic, sniper, spy),
   which owns the animation; and
2. the weapon, the item's `model_player` (`c_scattergun.mdl` …), drawn
   **bone-merged** onto the arms: each weapon bone takes the world
   transform of the arms bone with the same name; the weapon's own
   sequences are not played for this (except where the weapon animates
   its own extra bones; 4.3).

The **same weapon model** is what other players see in third person:
it is bone-merged onto the player model, whose skeleton has the same
`weapon_bone…` names (4.4). The legacy one-piece `v_` models still ship
(37, e.g. `v_scattergun_scout.mdl`: 45 bones, 7 sequences draw, idle,
fire, reload_start/loop/end with the same frame counts as the arms'
`sg_*`), but no stock item points to them except the Spy's invisibility
watch (`v_watch_spy.mdl`, defindex 30). The `w_` models (80, e.g.
`w_scattergun.mdl`, `w_rocketlauncher.mdl` 1 bone, `w_shotgun.mdl`
2 bones, `w_minigun.mdl`) are not referenced as `model_player` by the
stock weapons; they serve as dropped-weapon or legacy world models
(Open questions 7).

### 4.3 Arms models and their animation

Each arms model has a small local skeleton, one bodypart, 10 skin
families, one default hitbox set, **one local animation and a set of
1-frame stub sequences without activities**, and **one include model**
`models/weapons/c_models/c_<class>_animations.mdl` holding the real
sequences (with activities). The stubs are **forward declarations**:
sequence flag 0x800, no animation at all (a 0 × 0 blend grid), no
activity. A same-named sequence from an include **replaces** a
forward-declared one in its slot, so both by-name and by-activity
lookups land on the include's real sequence (rule and order:
player_animation.md §1a; Open questions 6 settled). A same-named
sequence without that flag never replaces an earlier one: the first
wins. Our dumper's merged listing of an
arms model shows the stubs first: dump the `_animations` model to see
the real data. All arms models have a pose parameter `r_hand_grip`
(0..12).

| Arms | Bones | Local stub seqs | Include seqs | Attachments | Textures (skin 0) |
|---|---|---|---|---|---|
| c_scout_arms | 58 | 46 | 91 | 11 | scout_hands |
| c_soldier_arms | 67 | 54 | 97 | 15 | soldier_hands, soldier_sleeves_red (blue in skin 1), w_rocket01 |
| c_heavy_arms | 60 | 42 | 80 | 21 | hvyweapon_hands, hvyweapon_hands_sheen |
| c_engineer_arms | 68 | 54 | 98 | 10 | engineer_red, engineer_handL, engineer_handR_red, engineer_mech_hand (blue variants in skin 1) |

Weapon-carrying bones in the arms (children of the hands):
`weapon_bone`, `weapon_bone_1`…`_4` (scout, heavy, engineer up to `_3`
on soldier), `weapon_bone_L`, `vm_weapon_bone`, `vm_weapon_bone_1`…`_7`,
`vm_weapon_bone_L`, `vm_weapon_bone_L_1`, `_L_2` (heavy:
`vm_weapon_bone_1_L`…`_4_L`), `effect_hand_L`, `effect_hand_R`; the
soldier adds `handle_bone` and `rocket`. Finger chains are
`bip_thumb/index/middle/ring/pinky_0..2_L/R`; arm chains
`bip_collar`, `bip_upperArm`, `bip_lowerArm` (`bip_wrist` on soldier
and engineer), `bip_hand`, all under `root`.

Sequences for the four weapons (all at 30 fps unless shown; "loop" =
the looping flag; duration = (frames − 1) / fps):

Scout (`c_scout_animations.mdl`), scattergun = `sg_*`:

| Sequence | Activity | Frames | fps | Duration s | Loop |
|---|---|---|---|---|---|
| sg_draw | ACT_PRIMARY_VM_DRAW | 31 | 30 | 1.0000 | |
| sg_idle | ACT_PRIMARY_VM_IDLE | 57 | 30 | 1.8667 | loop |
| sg_fire | ACT_PRIMARY_VM_PRIMARYATTACK | 19 | 30 | 0.6000 | |
| sg_reload_start | ACT_PRIMARY_RELOAD_START | 7 | 30 | 0.2000 | |
| sg_reload_loop | ACT_PRIMARY_VM_RELOAD | 16 | 30 | 0.5000 | **not** looping |
| sg_reload_end | ACT_PRIMARY_RELOAD_FINISH | 16 | 26 | 0.5769 | |

(Pistol: p_draw 21 / 0.6667, p_idle 61 loop, p_fire 9 / 0.2667,
p_reload 37 / 1.2. Bat: b_draw 41 / 1.3333, b_idle 31, b_swing_a/b
ACT_MELEE_VM_HITCENTER 24 / 0.7667 (weights 2 and 1), b_swing_c
ACT_MELEE_VM_SWINGHARD 24.)

Soldier (`c_soldier_animations.mdl`), rocket launcher = `dh_*`, shotgun
= unprefixed:

| Sequence | Activity | Frames | fps | Duration s | Loop |
|---|---|---|---|---|---|
| dh_draw | ACT_PRIMARY_VM_DRAW | 25 | 30 | 0.8000 | |
| dh_idle | ACT_PRIMARY_VM_IDLE | 41 | 30 | 1.3333 | loop |
| dh_fire | ACT_PRIMARY_VM_PRIMARYATTACK | 31 | 30 | 1.0000 | |
| dh_reload_start | ACT_PRIMARY_RELOAD_START | 16 | 30 | 0.5000 | |
| dh_reload_loop | ACT_PRIMARY_VM_RELOAD | 25 | 30 | 0.8000 | not looping |
| dh_reload_finish | ACT_PRIMARY_RELOAD_FINISH | 24 | 30 | 0.7667 | |
| draw | ACT_SECONDARY_VM_DRAW | 26 | 30 | 0.8333 | |
| idle | ACT_SECONDARY_VM_IDLE | 26 | 20 | 1.2500 | loop |
| fire | ACT_SECONDARY_VM_PRIMARYATTACK | 26 | 30 | 0.8333 | |
| reload_start | ACT_SECONDARY_RELOAD_START | 15 | 30 | 0.4667 | |
| reload_loop | ACT_SECONDARY_VM_RELOAD | 16 | 30 | 0.5000 | loop |
| reload_end | ACT_SECONDARY_RELOAD_FINISH | 22 | 30 | 0.7000 | |

(Also `dh_*_alt` with `_2` activities, `mangler_*` `_3`, `bet_*` `_QRL`
for other launchers; shovel `s_*` ACT_MELEE_VM_*.)

Heavy (`c_heavy_animations.mdl`), minigun = `m_*`, shotgun = unprefixed:

| Sequence | Activity | Frames | fps | Duration s | Loop |
|---|---|---|---|---|---|
| m_draw | ACT_PRIMARY_VM_DRAW | 31 | 30 | 1.0000 | |
| m_idle | ACT_PRIMARY_VM_IDLE | 51 | 30 | 1.6667 | loop |
| m_spool_up | ACT_PRIMARY_ATTACK_STAND_PREFIRE | 31 | 30 | 1.0000 | |
| m_spool_idle | ACT_PRIMARY_VM_SECONDARYATTACK | 10 | 30 | 0.3000 | loop |
| m_fire | ACT_PRIMARY_VM_PRIMARYATTACK | 18 | 30 | 0.5667 | loop |
| m_spool_down | ACT_PRIMARY_ATTACK_STAND_POSTFIRE | 41 | 30 | 1.3333 | |
| draw | ACT_SECONDARY_VM_DRAW | 31 | 30 | 1.0000 | |
| idle | ACT_SECONDARY_VM_IDLE | 61 | 30 | 2.0000 | loop |
| fire | ACT_SECONDARY_VM_PRIMARYATTACK | 26 | 30 | 0.8333 | |
| reload_start | ACT_SECONDARY_RELOAD_START | 11 | 30 | 0.3333 | |
| reload_loop | ACT_SECONDARY_VM_RELOAD | 16 | 30 | 0.5000 | loop |
| reload_end | ACT_SECONDARY_RELOAD_FINISH | 27 | 35 | 0.7429 | |

(Fists `f_*`: f_swing_left ACT_FISTS_VM_HITLEFT and f_swing_right
21 frames at 25 fps = 0.8 s, f_swing_crit 30 at 25 = 1.16 s.
`r_handposes` in the heavy file is 139 frames, looping, weight 0: a
pose library for the grip parameter, not to be played.)

Engineer (`c_engineer_animations.mdl`), shotgun = `fj_*`:

| Sequence | Activity | Frames | fps | Duration s | Loop |
|---|---|---|---|---|---|
| fj_draw | ACT_PRIMARY_VM_DRAW | 42 | 30 | 1.3667 | |
| fj_idle | ACT_PRIMARY_VM_IDLE | 66 | 30 | 2.1667 | loop |
| fj_fire | ACT_PRIMARY_VM_PRIMARYATTACK | 26 | 30 | 0.8333 | |
| fj_reload_start | ACT_PRIMARY_RELOAD_START | 11 | 30 | 0.3333 | |
| fj_reload_loop | ACT_PRIMARY_VM_RELOAD | 16 | 30 | 0.5000 | loop |
| fj_reload_end | ACT_PRIMARY_RELOAD_FINISH | 36 | 35 | 1.0000 | |

(Pistol `pstl_*`: draw 24, idle 54 loop, fire 7 / 0.2, reload 38 /
1.2333. Wrench `pdq_*`. PDAs `pda_*`, `bld_*`, toolbox `box_*`.)

The activity names are per **slot**, not per weapon:
`ACT_PRIMARY_VM_*`, `ACT_SECONDARY_VM_*`, `ACT_MELEE_VM_*`,
`ACT_ITEM1_VM_*` … The game picks the slot from the item (`item_slot`,
overridden by `anim_slot`) and an item can remap activities with
`visuals` → `animation_replacement` (e.g. the Force-a-Nature maps
ACT_VM_* to ACT_ITEM2_VM_*, which are the scout's `db_*` sequences).
The shotgun sequence sets differ per class, so the same `c_shotgun`
is animated by four different arm sets.

Every arms file also has inspect sequences (`*_inspect_start/idle/end`)
and mode-specific ones (grapple, passtime ball, spellbook, bread
monster): ignorable for a first port.

### 4.4 Weapon models

| Item | `model_player` | Bones | Attachments | Own sequences | Skins |
|---|---|---|---|---|---|
| Scattergun (13) | `models/weapons/c_models/c_scattergun.mdl` | `weapon_bone`, `weapon_bone_1`, `c_weapon_stattrack` | `muzzle` (on weapon_bone at 0, 3.97, 23.38), `eject_brass`, `pedestal_0`, `unusual_0`…`5`, `icon_camera` | `idle` (1 frame) | 10 families: 0–7 normal, 8–9 gold |
| Rocket launcher (18) | `models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl` | `weapon_bone`, `vm_weapon_bone_1`, `vm_weapon_bone_2`, `c_weapon_stattrack` | `muzzle` (0, 6.69, 9.91), `backblast` (0, 6.69, −35.43), `pedestal_0`, `unusual_0`…`5`, `icon_camera` | `idle` (1) | 10 (gold as above) |
| Shotgun (9–12) | `models/weapons/c_models/c_shotgun/c_shotgun.mdl` | `weapon_bone`, `weapon_bone_1`, `vm_weapon_bone`, `vm_weapon_bone_1`, `c_weapon_stattrack` | `muzzle` (0, 3.73, 34.92), `eject_brass`, `pedestal_0`, `unusual_0`…`5`, `icon_camera` | `idle` (1) | 1 |
| Minigun (15) | `models/weapons/c_models/c_minigun/c_minigun.mdl` | `weapon_bone`, `barrel`, `c_weapon_stattrack` | `muzzle` (0, −7.76, 57.76), `eject_brass`, `pedestal_0`, `unusual_0`…`5`, `icon_camera` | `idle` ACT_IDLE, `spool_up` ACT_MP_ATTACK_STAND_PREFIRE, `fire_loop` ACT_MP_ATTACK_STAND_PRIMARY (loop), `spool_down` ACT_MP_ATTACK_STAND_POSTFIRE: each 30 frames at 30 fps (0.9667 s) | 10 |

Bone match with the arms (bone-merge by name): `weapon_bone` and
`weapon_bone_1` exist in every arms model; `vm_weapon_bone`,
`vm_weapon_bone_1`, `vm_weapon_bone_2` exist in soldier, heavy and
engineer arms (and scout). The weapon's `c_weapon_stattrack` (kill
counter mount) and the minigun's `barrel` have no arms counterpart:
they keep their own pose relative to `weapon_bone`. The minigun's
barrel spin therefore comes from the weapon's own sequences (or game
code), not from the arms (inferred; Open questions 8). Attachments
are on the weapon's bones, so the muzzle follows the merged
`weapon_bone`: muzzle flashes and tracers start at the weapon model's
`muzzle`, not at an arms attachment.

### 4.5 Player models

`models/player/<class>.mdl` (scout, soldier, pyro, demo, heavy,
engineer, medic, sniper, spy; all version 48). Each has a local stub
sequence set (2 local animations; every stub but `ref` and `ragdoll`
is a 0x800 forward declaration, 4.3) and **three include models**:
`models/player/<class>_user_animations.mdl`,
`models/player/<class>_animations.mdl` (the real animation: scout 377
sequences, 1 012 animations), and
`models/workshop/player/animations/<class>_workshop_animations.mdl`.

| Class | Bones | Local seqs | Merged seqs (our dumper) | Hitboxes (set name) | Attachments | Skin families | Flex controllers |
|---|---|---|---|---|---|---|---|
| scout | 78 | 306 | 471 | 19 (scout) | 29 | 8 | 42 |
| soldier | 86 | 361 | 497 | 18 (soldier) | 24 | 8 | 42 |
| pyro | 76 | 270 | – | 19 (pyro) | 24 | 8 | 1 |
| demo | 84 | 276 | – | 18 (demoman) | 25 | 8 | 41 |
| heavy | 79 | 259 | 395 | 18 (heavy) | 20 | 8 | 46 |
| engineer | 73 | 351 | 469 | 18 (engineer) | 13 | 8 | 41 |
| medic | 92 | 213 | – | 18 (medic) | 20 | 8 | 49 |
| sniper | 74 | 240 | – | 18 (sniper) | 20 | 8 | 53 |
| spy | 84 | 274 | – | 18 (spy) | 27 | 26 | 44 |

- Pose parameters (merged): `body_pitch` −45..90, `body_yaw` −45..45
  (both wrap 360), `move_x` −1..1, `move_y` −1..1, `r_hand_grip`
  0..16, `r_arm` 0..3.
- Root bone `bip_pelvis`; head `bip_head`; weapon bones `weapon_bone`,
  `weapon_bone_1`…, `weapon_bone_L` (same names as the arms, so the
  same `c_` weapon bone-merges onto the player); `prop_bone`,
  `prop_bone_1`… for taunt props.
- Hitgroups (scout; heavy identical without group 10): 0 generic
  `bip_pelvis`, 1 head `bip_head`, 2 chest `bip_spine_2`, `bip_spine_3`,
  3 stomach `bip_spine_0`, `bip_spine_1`, 4 left arm (upper arm, lower
  arm, hand), 5 right arm, 6 left leg (hip, knee, foot), 7 right leg,
  10 gear `bip_packmiddle` (scout's bag).
- **Team colour by skin family**: 0 RED, 1 BLU, 2 RED Über
  (invulnerable), 3 BLU Über, 4 RED zombie, 5 BLU zombie, 6 RED Über
  zombie, 7 BLU Über zombie (scout's skin table; the others follow the
  same texture naming). Arms models switch team the same way (soldier's
  sleeves, engineer's glove): skin 0 RED, 1 BLU. Stock weapons are the
  same in families 0–7 (team-neutral); 8–9 are the gold (Australium)
  look. Some items select team skins through `visuals` → `styles`
  (`skin_red`, `skin_blu`).
- Sequence names are per weapon kind, e.g. `stand_PRIMARY`,
  `crouch_PRIMARY`, `run_PRIMARY`, `airwalk_PRIMARY`, `swim_PRIMARY`
  (the player animation state is a later spec).

## Part 5: `items_game.txt` as a data layout

`tf/scripts/items/items_game.txt` (loose, 8 251 383 bytes, KeyValues
text with `//` comments and occasional `[$PLATFORM]` conditionals, a
detached signature next to it). One root block `items_game` with these
sections in order: `game_info` (first/last valid class 1–9, item slots
0–18, 4 loadout presets), `qualities` (16: name → `value`, e.g. normal
0, rarity1 1, vintage 3, unique 6), `colors`, `rarities`,
`equip_regions_list`, `equip_conflicts`, `quest_objective_conditions`,
`item_series_types`, `item_collections`, `operations`, **`prefabs`**
(226), **`items`** (11 616), **`attributes`** (843),
`item_criteria_templates`, `random_attribute_templates`,
`lootlist_job_template_definitions`, `item_sets`, `client_loot_lists`,
`revolving_loot_lists`, `recipes`, `achievement_rewards`,
`attribute_controlled_attached_particles`, `armory_data`,
`item_levels`, `kill_eater_score_types`, `mvm_maps`, `mvm_tours`,
`matchmaking_categories`, `maps`, `master_maps_list`,
`steam_packages`, `string_lookups`, `community_market_item_remaps`,
`war_definitions`.

### 5.1 An item

`items` maps a **defindex** (decimal string) to a block. Resolving an
item: take its `prefab` value, a space-separated list of prefab names;
resolve each prefab the same way (prefabs may name prefabs); merge them
**left to right**, then the item's own keys on top; a later value
replaces an earlier scalar with the same key (compared case-
insensitively), and sub-blocks with the same key merge key by key,
recursively. Example: item 13 is `name`, `first_sale_date`, `prefab`
`weapon_scattergun`, `baseitem` 1; everything else (class, slot, model)
comes from the prefab, whose own `baseitem` 0 the item overrides.

Item keys (count of items that set them directly):

| Key | Items | Meaning |
|---|---|---|
| `name` | 11 616 | internal name (`TF_WEAPON_SCATTERGUN`) |
| `prefab` | 11 393 | prefabs to inherit |
| `item_name`, `item_type_name`, `item_description` | 11 085, 2 113, 10 729 | localisation tokens (`#TF_Weapon_Scattergun`) |
| `item_class` | 732 | the weapon or wearable kind the game creates (`tf_weapon_scattergun`, `tf_wearable`) |
| `item_slot` | 505 | primary, secondary, melee, pda, pda2, building, action, utility, head, misc, taunt … |
| `anim_slot` | 47 | animation slot override (primary, secondary, pda, MELEE_ALLCLASS, FORCE_NOT_USED …) |
| `used_by_classes` | 2 366 | block of class names (scout, sniper, soldier, demoman, medic, heavy, pyro, spy, engineer) → 1 |
| `model_player` | 4 478 | the `c_` model (first and third person) |
| `model_player_per_class` | 5 335 | block class → model, for multi-class items |
| `model_world` | (prefabs) | dropped/world model where set |
| `attach_to_hands` | 120 | 1: bone-merge onto the arms in first person |
| `item_quality` | 844 | a `qualities` name |
| `baseitem` | 127 | 1: a stock item every player owns |
| `attributes` | 906 | block: attribute **name** → { `attribute_class`, `value` } |
| `static_attrs` | 742 | block: attribute name → value (e.g. `min_viewmodel_offset` "10 0 -11") |
| `visuals`, `visuals_red`, `visuals_blu` | 2 796, 142, 142 | presentation: `sound_single_shot`, `sound_burst` (crit), `sound_double_shot`, `sound_empty`, `sound_reload`, `sound_deploy`, `sound_special1..3`, `sound_melee_hit`, `sound_melee_hit_world`, `sound_melee_miss`, `custom_sound0..2`, `muzzle_flash`, `tracer_effect`, `animation_replacement` (activity → activity), `styles` (skin_red, skin_blu, bodygroups), `skin`, `player_bodygroups`, `attached_models`, `attached_models_festive`, `material_override`, `particle_effect`, `player_poseparam`, `item_poseparam`, vm/wm bodygroup overrides |
| `equip_region`, `equip_regions` | 1 606, 95 | cosmetic conflicts |
| `capabilities`, `tags`, `tool`, `bundle`, `craft_class`, `craft_material_type`, `image_inventory*`, `min_ilevel`, `max_ilevel`, `drop_type`, `mouse_pressed_sound`, `drop_sound`, `first_sale_date`, `show_in_armory`, `hidden`, `xifier_class_remap`, `extra_wearable`, `extra_wearable_vm`, `taunt`, `default_skin`, `particle_suffix`, … | | economy, backpack and UI data |

The stock weapons carry **no fire sounds** in `visuals`: their sounds
(and damage, fire rate, clip size, spread, projectile) come from the
encrypted weapon scripts (Part 6). Non-stock items override sounds
here (Force-a-Nature: `Weapon_Scatter_Gun_Double.Single`, crit
`…SingleCrit`, muzzle flash `muzzle_bignasty`) and change numbers
through attributes (Force-a-Nature: `mult_postfiredelay` 0.5,
`mult_bullets_per_shot` 1.2, `mult_dmg` 0.9, `mult_clipsize` 0.34,
`set_scattergun_has_knockback` 1, `set_scattergun_no_reload_single` 1).

### 5.2 Attributes

`attributes` maps an attribute defindex to: `name` (the key items use),
`attribute_class` (what the game multiplies or sets, e.g. `mult_dmg`;
several names can share a class: "damage bonus" (2) and "damage
penalty" (1) are both `mult_dmg`), `description_string`,
`description_format` (value_is_additive 489, value_is_percentage 202,
value_is_inverted_percentage 44, value_is_additive_percentage 11,
value_is_or 10, …), `effect_type` (positive, negative, neutral),
`hidden`, `stored_as_integer`, sometimes `armory_desc`,
`attribute_type`. Attributes by class: a `mult_*` is a multiplier
(1 = no change), a `set_*`/`add_*` sets or adds; the actual effect of
each class is game behaviour for later specs.

### 5.3 Stock weapons of the nine classes (resolved)

| defindex | `name` | `item_class` | Slot | Classes | `model_player` |
|---|---|---|---|---|---|
| 0 | TF_WEAPON_BAT | tf_weapon_bat | melee | scout | `models/weapons/c_models/c_bat.mdl` |
| 1 | TF_WEAPON_BOTTLE | tf_weapon_bottle | melee | demoman | `…/c_bottle/c_bottle.mdl` |
| 2 | TF_WEAPON_FIREAXE | tf_weapon_fireaxe | melee | pyro | `…/c_fireaxe_pyro/c_fireaxe_pyro.mdl` |
| 3 | TF_WEAPON_CLUB | tf_weapon_club | melee | sniper | `…/c_machete/c_machete.mdl` |
| 4 | TF_WEAPON_KNIFE | tf_weapon_knife | melee | spy | `…/c_knife/c_knife.mdl` |
| 5 | TF_WEAPON_FISTS | tf_weapon_fists | melee | heavy | none (the arms are the weapon) |
| 6 | TF_WEAPON_SHOVEL | tf_weapon_shovel | melee | soldier | `…/c_shovel/c_shovel.mdl` |
| 7 | TF_WEAPON_WRENCH | tf_weapon_wrench | melee | engineer | `…/c_wrench/c_wrench.mdl` |
| 8 | TF_WEAPON_BONESAW | tf_weapon_bonesaw | melee | medic | `…/c_bonesaw/c_bonesaw.mdl` |
| 9 | TF_WEAPON_SHOTGUN_PRIMARY | tf_weapon_shotgun | primary | engineer | `…/c_shotgun/c_shotgun.mdl` |
| 10 | TF_WEAPON_SHOTGUN_SOLDIER | tf_weapon_shotgun | secondary | soldier | same (hidden 1) |
| 11 | TF_WEAPON_SHOTGUN_HWG | tf_weapon_shotgun | secondary | heavy | same (hidden 1) |
| 12 | TF_WEAPON_SHOTGUN_PYRO | tf_weapon_shotgun | secondary | pyro | same (hidden 1) |
| 13 | TF_WEAPON_SCATTERGUN | tf_weapon_scattergun | primary | scout | `models/weapons/c_models/c_scattergun.mdl` |
| 14 | TF_WEAPON_SNIPERRIFLE | tf_weapon_sniperrifle | primary | sniper | `…/c_sniperrifle/c_sniperrifle.mdl` |
| 15 | TF_WEAPON_MINIGUN | tf_weapon_minigun | primary | heavy | `…/c_minigun/c_minigun.mdl` |
| 16 | TF_WEAPON_SMG | tf_weapon_smg | secondary | sniper | `…/c_smg/c_smg.mdl` |
| 17 | TF_WEAPON_SYRINGEGUN_MEDIC | tf_weapon_syringegun_medic | primary | medic | `…/c_syringegun/c_syringegun.mdl` |
| 18 | TF_WEAPON_ROCKETLAUNCHER | tf_weapon_rocketlauncher | primary | soldier | `…/c_rocketlauncher/c_rocketlauncher.mdl` |
| 19 | TF_WEAPON_GRENADELAUNCHER | tf_weapon_grenadelauncher | primary (anim secondary) | demoman | `…/c_grenadelauncher/c_grenadelauncher.mdl` |
| 20 | TF_WEAPON_PIPEBOMBLAUNCHER | tf_weapon_pipebomblauncher | secondary (anim primary) | demoman | `…/c_stickybomb_launcher/c_stickybomb_launcher.mdl` |
| 21 | TF_WEAPON_FLAMETHROWER | tf_weapon_flamethrower | primary | pyro | `…/c_flamethrower/c_flamethrower.mdl` |
| 22 | TF_WEAPON_PISTOL | tf_weapon_pistol | secondary | engineer, scout | `…/c_pistol/c_pistol.mdl` |
| 23 | TF_WEAPON_PISTOL_SCOUT | tf_weapon_pistol | secondary | engineer, scout | same (hidden 1) |
| 24 | TF_WEAPON_REVOLVER | tf_weapon_revolver | secondary | spy | `…/c_revolver/c_revolver.mdl` |
| 25 | TF_WEAPON_PDA_ENGINEER_BUILD | tf_weapon_pda_engineer_build | pda | engineer | `…/c_builder/c_builder.mdl` |
| 26 | TF_WEAPON_PDA_ENGINEER_DESTROY | tf_weapon_pda_engineer_destroy | pda2 (anim pda) | engineer | `…/c_pda_engineer/c_pda_engineer.mdl` |
| 27 | TF_WEAPON_PDA_SPY | tf_weapon_pda_spy | pda | spy | `models/weapons/w_models/w_cigarette_case.mdl` |
| 28 | TF_WEAPON_BUILDER | tf_weapon_builder | building | engineer | `…/c_toolbox/c_toolbox.mdl` (hidden 1) |
| 29 | TF_WEAPON_MEDIGUN | tf_weapon_medigun | secondary | medic | `…/c_medigun/c_medigun.mdl` |
| 30 | TF_WEAPON_INVIS | tf_weapon_invis | pda2 (anim FORCE_NOT_USED) | spy | `models/weapons/v_models/v_watch_spy.mdl` |
| 735 | TF_WEAPON_BUILDER_SPY | tf_weapon_builder | building | spy | `…/c_sapper/c_sapper.mdl` |

(`…` = `models/weapons/c_models`. All `baseitem` 1, quality normal.
Other `baseitem` 1 items: 1132 spellbook, 1152 grappling hook, 1155
PASS Time gun (hidden), 1163 MvM canteen.) No stock weapon sets
`model_world`. Each prefab carries `static_attrs`
`min_viewmodel_offset` (scattergun "10 0 -11", minigun "10 0 -9") used
by the minimal-viewmodel option.

## Part 6: other scripts

In `tf2_misc` (262 files under `scripts/`): 118 `.txt`, 9 `.res`, 34
`.pop` (MvM populations), 6 `.raw`, 2 `.lst`, 1 `.vdf`, 1 `.xsc`, 1
`.py`, 2 `.png`, and **88 `.ctx`**: `scripts/tf_weapon_*.ctx` (78) and
`scripts/playerclasses/<class>.ctx` (10: the nine classes and
"civilian"). The `.ctx` files are **encrypted** and stay unread: TF2's
per-weapon numbers (damage, fire rate, clip, spread, bullets per shot,
projectile) **and per-class numbers** (health, speed, ammo maxima) are
not available as plain data in either install. Unlike HL2:DM there is
no plain `.txt` twin. Later specs must get them from public
documentation (the TF2 wiki's weapon and class pages) or live
measurement.

Plain text and useful: `scripts/game_sounds_manifest.txt` (below),
`game_sounds*.txt`, `soundscapes_manifest.txt` and 40+ soundscape
files (`soundscapes_2fort.txt`, `soundscapes_badlands.txt`, …),
`surfaceproperties_manifest.txt` + `surfaceproperties_tf.txt`,
`decals_subrect.txt`, `objects.txt` (buildings), `hudlayout.res`,
`hudanimations_manifest.txt`, `hudanimations_tf.txt`, `screens/*.res`,
`talker/*` (voice response rules), `population/*.pop`,
`items/` (custom texture blend layers), `weapon_manifest.txt`. The
shared `propdata.txt` is not in TF2's VPK; it comes from `hl2_misc`
(with HL2's surface properties and HL2's plain weapon scripts, which
TF2 does not use).

Sound script manifest (`scripts/game_sounds_manifest.txt`, in order):
`game_sounds.txt`, `game_sounds_physics.txt`,
`game_sounds_weapons.txt`, `game_sounds_vo.txt`,
`game_sounds_vo_handmade.txt`, `game_sounds_music.txt`,
`game_sounds_player.txt` (listed as `preload_file`, the rest
`precache_file`), `game_sounds_mvm.txt`,
`game_sounds_vo_mvm_handmade.txt`, `game_sounds_vo_rd_robots.txt`,
`game_sounds_vo_taunts.txt`, `game_sounds_taunt_workshop.txt`,
`game_sounds_vo_pauling.txt`, `game_sounds_vo_merasmus.txt`,
`game_sounds_vo_tough_break.txt`, `game_sounds_passtime.txt`.
`game_sounds_footsteps.txt` exists but is not in the manifest.

Weapon entries in `game_sounds_weapons.txt` (813 entries; no `pitch`
key, so pitch 100; waves under `sound/`; a leading `)` marks a
spatialised stereo wave per the public sound-character docs):

| Entry | Channel | Level | Volume | Wave |
|---|---|---|---|---|
| Weapon_Scatter_Gun.Single | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/scatter_gun_shoot.wav` |
| Weapon_Scatter_Gun.SingleCrit | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/scatter_gun_shoot_crit.wav` |
| Weapon_Scatter_Gun.Empty | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/shotgun_empty.wav` |
| Weapon_Scatter_Gun.Reload | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/scatter_gun_reload.wav` |
| Weapon_Scatter_Gun.WorldReload, .Pump | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/scatter_gun_worldreload.wav` |
| Weapon_Scatter_Gun.Draw | CHAN_AUTO | SNDLVL_74dB | 0.5 | `weapons/draw_secondary.wav` |
| Weapon_RPG.Single | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/rocket_shoot.wav` |
| Weapon_RPG.SingleCrit | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/rocket_shoot_crit.wav` |
| Weapon_RPG.Reload, .WorldReload | CHAN_STATIC | SNDLVL_74dB | 1 | `weapons/rocket_reload.wav` |
| Weapon_RPG.Draw | CHAN_WEAPON | SNDLVL_74dB | 0.5 | `weapons/draw_primary.wav` |
| Weapon_Shotgun.Single | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/shotgun_shoot.wav` |
| Weapon_Shotgun.SingleCrit | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/shotgun_shoot_crit.wav` |
| Weapon_Shotgun.Empty | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/shotgun_empty.wav` |
| Weapon_Shotgun.Reload | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/shotgun_reload.wav` |
| Weapon_Shotgun.WorldReload | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/shotgun_worldreload.wav` |
| Weapon_Shotgun.Pump | CHAN_STATIC | SNDLVL_NORM | 1.0 | `weapons/shotgun_cock.wav` |
| Weapon_Shotgun.Draw | CHAN_AUTO | SNDLVL_74dB | 0.5 | `weapons/draw_secondary.wav` |
| Weapon_Minigun.WindUp | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/minigun_wind_up.wav` |
| Weapon_Minigun.Spin | CHAN_STATIC | SNDLVL_NORM | 1.0 | `)weapons/minigun_spin.wav` |
| Weapon_Minigun.Fire | CHAN_STATIC | SNDLVL_94dB | 1.0 | `)weapons/minigun_shoot.wav` |
| Weapon_Minigun.FireCrit | CHAN_STATIC | SNDLVL_94dB | 1.0 | `)weapons/minigun_shoot_crit.wav` |
| Weapon_Minigun.WindDown | CHAN_WEAPON | SNDLVL_94dB | 1.0 | `)weapons/minigun_wind_down.wav` |
| Weapon_Minigun.ClipEmpty | CHAN_STATIC | SNDLVL_NORM | 1.0 | `)weapons/minigun_empty.wav` |
| Weapon_Minigun.Reload | CHAN_WEAPON | SNDLVL_NORM | 1.0 | `weapons/default_reload.wav` |
| Weapon_Minigun.Draw | CHAN_AUTO | SNDLVL_74dB | 0.5 | `weapons/draw_minigun_heavy.wav` |

Which entry a stock weapon plays for which event is named in its
encrypted script; the pairing above (scattergun → `Weapon_Scatter_Gun`,
rocket launcher → `Weapon_RPG`, …) is by name (inferred; Open
questions 9). The waves themselves are in the sound VPK archives, so
the server install cannot play them.

## Part 7: what differs from HL2:DM

References are to specs/hl2dm/source_deltas.md (Part 2) and the
specs/cs_source/ files it lists.

| Area | HL2:DM | TF2 | What our port must do differently |
|---|---|---|---|
| Install contents | server lacks textures and sounds; the game has all | same split: server has no textures/sounds | install check accepts both; full rendering needs app 440's folder (1.3) |
| Search path | `hl2mp/` VPKs, then an `hl2_complete/` layer, then HL2's VPKs | `custom/*`, low-violence VPK, `tf/` VPKs, HL2's VPKs, platform; no `hl2_complete` | new path ids (`custom_mod`, `game_lv`, `vgui`); skip missing archives per file |
| Scripts in loose `hl2/` | sound manifest, propdata, surface properties loose | `hl2/` has only `media/` and `resource/`; TF2's manifest is in its VPK | don't expect loose HL2 scripts; `propdata.txt` from `hl2_misc` |
| BSP | v20, no LZMA, pak stored | v20, **183/238 LZMA**, **pak entries LZMA**, sprp v5/6/7/**10 (compressed)** | LZMA lumps, LZMA game lump entries, LZMA zip method 14, sprp v10 (72 bytes) |
| HDR | all maps LDR + HDR | all but cp_cloak | already LDR-first |
| Spawns | `info_player_deathmatch`, team spawns combine/rebel | `info_player_teamspawn` (`TeamNum` 2/3, `controlpoint`, `StartDisabled`, class `spawnflags`) | new spawn selection |
| Weapons, first person | one `v_` model per weapon, sequences in it | arms model (include holds sequences) + bone-merged `c_` weapon; activities per slot; item can remap activities | two-model view rig, bone merge by name, activity lookup into includes |
| Weapons, third person | `w_` model on the player | the same `c_` model bone-merged onto the player | reuse the first-person weapon model |
| Weapon data | plain `weapon_*.txt` next to `.ctx` | `.ctx` only (encrypted); identity, models and some sounds in `items_game.txt` | parse `items_game.txt` with prefab merge; numbers from public docs or measurement |
| Class data | – (one player kind) | nine classes; class numbers in encrypted `playerclasses/*.ctx` | per-class data from public docs |
| Player models | HL2 citizens/Combine, animation via search-path override | per-class model with 3 include models, 8 skin families (team, Über, zombie) | team colour = skin 0/1 |
| Materials | VertexLitGeneric, a few Phong view models | almost every character and weapon material is Phong + lightwarp + rim; cloak, sheen, burn, Jarate, paint via proxies | shaders_phong.md is the main path; proxies at rest |
| Sounds | `hl2/` loose manifest + `hl2mp` overrides | manifest of 16 files in `tf2_misc`; fire sounds per weapon in `game_sounds_weapons.txt` | read TF2's manifest only |
| HUD | GorDIN/HalfLife2 fonts, scheme in `hl2mp` VPK | fonts loose in `tf/resource/` (tf, tf2, tf2build, tf2secondary, tf2professor, ocra), 710 `resource/ui/*.res` | a later HUD spec |

## Test cases

| # | Setup | Check | Expected |
|---|---|---|---|
| 1 | dedicated server install | `tf/steam.inf` PatchVersion | 11191711 (this build) |
| 2 | install check on the server install | classification | TF2, server content (no `tf2_textures_000.vpk`) |
| 3 | mount from `tf/gameinfo.txt` | ordered VPK entries found | tf2_textures, tf2_sound_vo_english, tf2_sound_misc, tf2_misc, hl2_textures, hl2_sound_vo_english, hl2_sound_misc, hl2_misc, platform_misc (tf2_lv skipped) |
| 4 | `tf2_misc_dir.vpk` | file count; `.vtf` count | 104 343; 0 |
| 5 | `tf2_textures_dir.vpk` | files; archives present | 29 669 (all `.vtf`); 0 of 108 |
| 6 | `tf/maps/*.bsp` | count; versions | 238; all 20 |
| 7 | all maps | maps with any LZMA lump | 183 |
| 8 | all maps | sprp versions | 10 × 199, 6 × 36, 5 × 2, 7 × 1 |
| 9 | cp_cloak | HDR lighting lump size | 0 |
| 10 | ctf_2fort | entities; `info_player_teamspawn` | 1 097; 30, 15 with `TeamNum` 2 and 15 with 3, all `StartDisabled` 0 |
| 11 | ctf_2fort | worldspawn `skyname`; `sky_camera` scale | sky_tf2_04; 16 |
| 12 | ctf_2fort | displacements; static props | 232; 2 265 |
| 13 | ctf_2fort | `item_teamflag` | 2: `flag_red` `TeamNum` 2, `flag_blue` 3, `ReturnTime` 60, model `models/flag/briefcase.mdl` |
| 14 | ctf_2fort | world material shaders | 272 LightmappedGeneric, 8 UnlitGeneric, 6 Water, 4 WorldVertexTransition, 1 VertexLitGeneric; none missing |
| 15 | ctf_2fort | pak lump | 4 861 zip entries, all method 14 |
| 16 | cp_badlands | entities; teamspawns | 674; 92: 40 RED, 40 BLU, 12 team 0 with `MatchSummary` |
| 17 | cp_badlands | spawns usable at round start (point ownership defaults) | 16 RED (`spawn_red_1`, `cap_red_1`), 16 BLU (`spawn_blue_1`, `cap_blue_1`) |
| 18 | cp_badlands | `team_control_point` | 5; `point_default_owner` 2 × RED, 2 × BLU, 1 × 0 (`cap_center`) |
| 19 | cp_badlands | displacements; static props; skyname | 1 191; 1 232; sky_badlands_01 |
| 20 | cp_badlands | world material shaders | 146 LightmappedGeneric, 4 UnlitGeneric, 1 WorldVertexTransition |
| 21 | `c_scout_arms.mdl` | bones; include | 58; `models/weapons/c_models/c_scout_animations.mdl` |
| 22 | scout arms, activity ACT_PRIMARY_VM_PRIMARYATTACK | sequence; duration | `sg_fire`, 19 frames, 0.6 s |
| 23 | scout arms, ACT_PRIMARY_RELOAD_FINISH | duration | 16 frames at 26 fps = 0.5769 s |
| 24 | heavy arms, ACT_PRIMARY_VM_SECONDARYATTACK | sequence | `m_spool_idle`, 10 frames, looping |
| 25 | soldier arms, ACT_PRIMARY_VM_RELOAD | sequence; looping | `dh_reload_loop`, 25 frames, not looping |
| 26 | `c_scattergun.mdl` bone-merged onto scout arms | bones matched | `weapon_bone`, `weapon_bone_1` (`c_weapon_stattrack` unmatched) |
| 27 | `c_scattergun.mdl` | `muzzle` attachment | on `weapon_bone` at (0.0016, 3.969, 23.384) |
| 28 | `models/player/scout.mdl` | bones; hitboxes; includes | 78; 19 in set "scout"; 3 includes |
| 29 | scout player skin table | family 1 first texture | `scout_blue` |
| 30 | `items_game.txt` resolved item 13 | item_class; slot; model | tf_weapon_scattergun; primary; `models/weapons/c_models/c_scattergun.mdl` |
| 31 | `items_game.txt` | counts of items, prefabs, attributes | 11 616; 226; 843 |
| 32 | item 45 resolved | `visuals` `sound_single_shot`; attribute "fire rate bonus" value | `Weapon_Scatter_Gun_Double.Single`; 0.5 |
| 33 | sound script `Weapon_Minigun.WindUp` | channel, level, wave | CHAN_WEAPON, SNDLVL_94dB, `)weapons/minigun_wind_up.wav` |
| 34 | `scripts/` in tf2_misc | `.ctx` count | 88 (78 weapons, 10 classes) |

## Open questions

1. **Tick rate**: 66.67 Hz is the public default; confirm with a
   running server (`status`, or the server's reported tick interval) or
   a demo header. TF2's server may also accept `-tickrate`.
2. **Client layout**: the client-only files (launcher names, client
   library paths, `tf/media`, archive counts of the textures and sound
   VPKs) are from public descriptions; list them from the Windows PC's
   app 440 folder.
3. **Missing archives**: when a file is listed in a VPK whose archive is
   missing, does the real engine fail the open or fall through to later
   paths? With the server install it decides whether stock textures
   resolve to nothing or to an older copy elsewhere (no other copy
   exists here, so a placeholder is needed either way).
4. **Absent `spawnflags` on teamspawns** (ctf_2fort): inferred to mean
   every class; confirm on a listen server with a non-scout class.
5. **`MatchSummary` values 1 and 2**: which team or result each means.
6. ~~**Local stub vs include sequence with the same name**~~ Settled
   (2026-11-19, from the model data and the engine's include merge
   read in the SDK): every stub in the arms and class models carries
   the forward-declaration flag 0x800 and no animation, and an include's
   same-named sequence takes the stub's slot; without the flag the first
   one wins. The include's sequence is what plays (player_animation.md
   §1a, with the include order and the duplicate rules).
7. **`w_` models**: whether dropped weapons use `model_world`, the
   `c_` model, or the `w_` model; TF2's dropped weapons are recent
   (2014+) and mostly use the `c_` model (public).
8. **Minigun barrel**: whether the barrel spin is driven by the
   weapon's own `spool_up`/`fire_loop` sequences while bone-merged, or
   by game code rotating the `barrel` bone.
9. **Which sound entry each stock weapon plays**: the mapping is in
   the encrypted script; confirm by listening (or the TF2 wiki's weapon
   pages).
10. **Build drift**: `items_game.txt` changes with game updates (it is
    also distributed by the item servers); tests that pin counts should
    read them from the install, not hard-code them, beyond this build.
