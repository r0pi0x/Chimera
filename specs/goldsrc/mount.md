# GoldSrc: the mount (content folders, loose files, `liblist.gam`, `materials.txt`)

Source basis: the HLDS install (app 90, `mod cstrike`, 2026-11-24): its
folder layout, `cstrike/liblist.gam`, `valve/liblist.gam`,
`cstrike/steam.inf`, `cstrike/sound/materials.txt`,
`valve/sound/materials.txt`, the maps' references to WADs, skies and
models; public documentation of the GoldSrc mod folder layout and the
`liblist.gam` keys; Valve's Half-Life SDK (spec-session reference only)
for how texture names are matched against `materials.txt`.
Status: draft

## Summary

A GoldSrc game is a content folder of loose files (`cstrike/`) on top of
Half-Life's (`valve/`), both under one install root. A file is looked up
by its path relative to a content folder, in the game's folder first and
then in the fallback folder, ignoring case. There are no archives in the
Steam layouts. `liblist.gam` describes the game; `sound/materials.txt`
maps texture names to footstep and impact surface types.

## Units and conventions

Paths are relative to a content folder, `/`-separated; data written on
Windows uses `\`, which is a separator too. Comparison of path and file
names ignores ASCII case.

## Constants

| Name (ours) | Value | Meaning |
|---|---|---|
| game_folder | `cstrike` | CS 1.6's content folder |
| default_fallback | `valve` | the fallback when `liblist.gam` names none |
| material_name_chars | 12 | characters of a texture name that `materials.txt` matching uses |
| default_material | `C` | concrete, for textures not listed |

## The install root and the folder chain

The install root is the folder that holds `cstrike/` and `valve/` (the
client's `steamapps/common/Half-Life/`, or the dedicated server's
folder). Required to call it a CS 1.6 install: `cstrike/liblist.gam`,
`cstrike/maps/de_dust2.bsp`, `valve/halflife.wad`.

The original engine searches, highest priority first: the game folder's
`_addon` folder (user content, when allowed), its language folder
(`cstrike_<language>`, on clients set to a language other than
English), its `_hd` folder (high-definition models, when enabled and not
refused by `nohimodel`), the game folder itself, its `_downloads` folder
(files fetched from servers); then the same chain for the fallback
folder. **Our chain**:

1. `cstrike_<language>` when present (client installs only; none on the
   server),
2. `cstrike`,
3. the fallback folder: `liblist.gam`'s `fallback_dir` value, else
   `valve`.

Left out on purpose: `_addon` and `_downloads` (other people's content),
`_hd` (CS 1.6's `liblist.gam` says `nohimodel "1"`), `_lv` (the
low-violence variant).

Shadowing: the first folder that has a path wins. Example: both
`cstrike/decals.wad` (225 entries) and `valve/decals.wad` (222) exist;
the chain reads `cstrike`'s. `cstrike/gfx/palette.lmp` and
`valve/gfx/palette.lmp` are both 768 bytes.

## `liblist.gam`

Text. One key per line: a bare key, whitespace (spaces or a tab), a
quoted value; `//` starts a comment; values may have trailing spaces
after the closing quote (`cldll "1" `). Keys compare case-insensitively.

`cstrike/liblist.gam` holds: `game` "Counter-Strike", `url_info`,
`url_dl` "", `version` "1.6", `size`, `svonly` "0", `secure` "1", `type`
"multiplayer_only", `cldll` "1", `hlversion` "1111", `nomodels` "1",
`nohimodel` "1", `mpentity` "info_player_start", `gamedll`,
`gamedll_linux` "dlls/cs.so", `gamedll_osx`, `trainmap` "tr_1", and
`edicts` "1800" (tab-separated). It has **no `fallback_dir`**: the
fallback is `valve`. `valve/liblist.gam` adds `startmap`,
`animated_title`, `hd_background`.

What we read: `game` (display name), `fallback_dir` (the chain),
`mpentity` (the default multiplayer spawn class), `nohimodel`. The
library keys name game code we never load.

`cstrike/steam.inf` (`PatchVersion=1.1.2.7`, `ProductName=cstrike`)
identifies the build.

## Loose files and case

Every file is a plain file under a content folder; no PAK or GCF exists
in either layout. Looking a path up:

1. Turn `\` into `/` and drop a leading `/` (the stock maps reference
   `/models/hostage.mdl` 5 times).
2. Find it in each chain folder in order, **ignoring case**.

Case matters on Linux and the data disagree with the file names:
de_dust2's `skyname` is `des` and the files are `gfx/env/Desup.tga`...;
de_train's sky files are `TrainYardUp.tga`...; the WAD names in maps are
any case; `arcticorange.mdl`'s textures are in `arcticorangeT.mdl` while
Half-Life's are `barneyt.mdl`. No two files in `cstrike/` or `valve/`
differ only by case, so a case-folded index per folder is unambiguous.

## Where things live

| Kind | Path | Notes |
|---|---|---|
| maps | `maps/<map>.bsp` | 25 in `cstrike`; also `maps/<map>.txt` (a description), and `maps/<map>.res` for two maps (`de_storm.res`: the extra files a client needs, one path per line, `//` comments) |
| WADs | `<name>.wad` at the folder root | found by file name only (below) |
| sky | `gfx/env/<skyname><face>.tga`, faces `up`, `dn`, `lf`, `rt`, `ft`, `bk` | `valve` also has `.bmp` copies of some; the TGA is the one to read |
| models | `models/...mdl` | entity keys name them with the `models/` prefix |
| sprites | `sprites/...spr`, `sprites/hud.txt`, `sprites/weapon_*.txt` | |
| sounds | `sound/...wav` | entity keys and model events name them **without** `sound/` (`weapons/ak47_clipout.wav`) |
| surface types | `sound/materials.txt` | below |
| palette | `gfx/palette.lmp` | 768 bytes; textures carry their own palettes, so not needed for them |
| map rotation | `mapcycle.txt` | one map name per line |
| radar overviews | `overviews/` | |
| UI text | `resource/*.res` (KeyValues), `resource/cstrike_english.txt` (UTF-16) | M3 |

### WADs named by a map

A map's `worldspawn` `wad` key lists paths from the mapper's machine.
Take each entry's last path component (after the last `\` or `/`) and
find that file name at the root of each chain folder; skip names not
found (they are normal: a map that embedded its textures still lists
the WADs it was made with, e.g. de_inferno lists 22 and 10 are not
installed). For de_dust2 the three entries resolve to
`valve/halflife.wad`, `cstrike/decals.wad` and `cstrike/cs_dust.wad`
(the key says `valve\` for all three). Over the 25 stock maps, every
non-embedded texture is found in a listed, installed WAD; of the
resolved references, 428 are in `valve` and 351 in `cstrike`.

A texture is searched in the listed WADs in the key's order; the first
WAD holding the name wins (open question 1).

## `materials.txt`

Text. A line is a type letter, whitespace and a texture name; `//`
starts a comment; blank lines are ignored. Types:

| Letter | Surface |
|---|---|
| `C` | concrete (the default for unlisted textures; never written) |
| `M` | metal |
| `D` | dirt |
| `V` | ventilation duct |
| `G` | grate |
| `T` | tile |
| `S` | slosh (liquid) |
| `W` | wood |
| `P` | computer |
| `Y` | glass |
| `N` | snow (CS addition) |
| `X` | grass (CS addition) |

`cstrike/sound/materials.txt`: 787 lines, 701 entries (D 95, G 56, M
131, N 6, P 144, S 14, T 77, V 13, W 155, X 2, Y 8). It replaces
`valve/sound/materials.txt` (398 lines) by the chain.

Matching a face's texture name:

1. Drop a leading `-` or `+` together with the character after it (the
   tiling or animation frame: `-0SandRock2` → `SandRock2`).
2. Then drop one leading `{`, `!`, `~` or space.
3. Keep at most the first 12 characters.
4. Compare case-insensitively with each entry's name, itself cut to 12
   characters; the first match gives the type; none gives `C`.

That is Half-Life's rule. Counter-Strike 1.6's game and client
libraries match differently (more entries, longer names, a sorted
search); CS 1.6's rule is in its own spec,
[../cs16/footsteps.md](../cs16/footsteps.md) "The surface type".

Quirks to keep:

- Entries written with a prefix never match, since names are stripped
  before comparing: the file has entries like `-0cstrike_me` and
  `-1cstrike_we`. 30 cut names appear more than once (`pl_out_shell`...);
  the first wins.
- 94 entries are longer than 12 characters and match on their first 12
  (`pl_out_shell4a` and `pl_out_shell3` both become `pl_out_shell`).
- The file's own comment says only 512 entries are read; it has 701.
  Whether CS 1.6 reads past 512 is open question 2; until measured,
  read them all.

## Edge cases

- A missing fallback folder: the chain is just the game folder.
- `maps/<map>.res` lists files with `\` or `/`; only needed to know what
  a server would send.
- Sky name absent from `worldspawn`: `desert` (present in `valve`).
- A sky face missing: draw that face black (none missing in the install
  once case is ignored).

## Test cases

| Setup | Input | Expected |
|---|---|---|
| the HLDS install | parse `cstrike/liblist.gam` | `game` = Counter-Strike, no `fallback_dir`, chain = `cstrike`, `valve` |
| same | look up `gfx/env/desup.tga` | `cstrike/gfx/env/Desup.tga` |
| same | look up `decals.wad` | `cstrike/decals.wad` (not `valve/`) |
| same | look up `halflife.wad` | `valve/halflife.wad` |
| same | look up `\models\barney01.mdl` | `valve/models/barney01.mdl` |
| de_dust2 `wad` key | resolve | `valve/halflife.wad`, `cstrike/decals.wad`, `cstrike/cs_dust.wad` |
| de_inferno `wad` key | resolve | 12 found, 10 skipped, no error |
| materials | texture `-0SandRock2` | name compared: `sandrock2` |
| materials | texture `{pi_treeb` | name compared: `pi_treeb` |
| materials | a texture `pl_out_shell4a` | first entry whose 12-character cut is `pl_out_shell` |
| materials | an unlisted texture | `C` |

## Open questions

1. WAD precedence when two listed WADs hold the same texture name: first
   listed is assumed; check on a map where it matters (none found yet).
2. The 512-entry limit on `materials.txt` (Half-Life's rule): entry 512
   is `OUT_SLAT01` (W) and 513 `OUT_WD` (W). For CS 1.6 see
   [../cs16/footsteps.md](../cs16/footsteps.md), which also says how to
   check it in the game.
3. The client's language folders and anything else the client has that
   the server lacks: checked when a client install is available
   (`specs/cs16/install.md`).
