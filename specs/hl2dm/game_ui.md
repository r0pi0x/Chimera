# Half-Life 2: Deathmatch: GameUI (main menu, Multiplayer options, Create Server)

Source basis: the game's own data files on this box (game build
24600434): `hl2mp/hl2mp_pak_dir.vpk`'s `resource/gamemenu.res`,
`resource/optionssubmultiplayer.res`, `resource/multiplayeradvanceddialog.res`,
`resource/multiplayercustomizedialog.res`, `resource/optionssubadvanced.res`,
`cfg/settings.scr`, `materials/vgui/playermodels/` and
`materials/vgui/logos/`; Half-Life 2's loose
`resource/createmultiplayergameserverpage.res`,
`resource/createmultiplayergamegameplaypage.res`,
`resource/valve_english.txt` and `resource/gameui_english.txt`; the same
files of the CS:S install for the comparison; the shared GameUI library
searched for file paths, cvar names, console command strings and
localisation keys only (never disassembled); the public SDK's shared
script-option reader (the code that turns `settings.scr` into option rows
and back into console commands), its VGUI panel settings reader, and the
HL2:DM client's `cl_playermodel` definition. GameUI's own page and dialog
code is not in the public SDK, so behaviour that only its code decides is
marked "inferred" (with the evidence) or listed under Open questions
instead of guessed.
Status: draft

## Summary

HL2:DM uses the same GameUI library as CS:S; what differs is data. Its
main menu lacks CS:S's Achievements, Report Bug and Benchmark entries; its
Multiplayer options page shows a spray picker, a player-model picker and
the download filter, and hides every crosshair control and the Advanced
button; Create Server has no bot controls; the Game tab is built from
HL2:DM's `cfg/settings.scr`, which lists `sv_password` twice (shown
once). Controls are named by their layout `fieldName`s (tables in
sections 2 and 4), the names to bind settings by.

## Units and conventions

Positions and sizes are the `.res` files' units (the dialogs' own
coordinate space, origin top left of the page). Labels are the English
texts of the named localisation keys.

## Behavior

### 1. Main menu (`resource/gamemenu.res`)

In order, with "in game" meaning shown only while connected:

| Key | Label | Command | Shown |
|---|---|---|---|
| 1 | RESUME GAME | `ResumeGame` | in game |
| 2 | DISCONNECT | `Disconnect` | in game |
| 3 | MUTE PLAYERS | `OpenPlayerListDialog` | in game |
| 4 | (empty: a gap) | – | in game |
| 5 | FIND SERVERS | `OpenServerBrowser` | always |
| 6 | CREATE SERVER | `OpenCreateMultiplayerGameDialog` | always |
| 7 | ACTIVATE VIRTUAL REALITY | `engine vr_activate` | only with VR enabled and inactive (in-game order 40) |
| 8 | DEACTIVATE VIRTUAL REALITY | `engine vr_deactivate` | only with VR enabled and active |
| 10 | OPTIONS | `OpenOptionsDialog` | always |
| 11 | QUIT | `Quit` | always |

CS:S's file has the same 1–6, 10 and 11, and in place of 7–8:
Achievements (`OpenCSAchievementsDialog`), Report Bug (`engine bug`),
Benchmark (`OpenBenchmarkDialog`). HL2:DM has none of those three.

### 2. Options > Multiplayer (`resource/optionssubmultiplayer.res`, HL2:DM's own)

Page 496 × 312 at (0, 28) (the page itself: field `OptionsSubMultiplayer`,
class `COptionsSubMultiplayer`). Visible controls, with the `fieldName`
each has in the file (the name to bind by; note the space in
`Player model`) and its `ControlName` class:

| Field name | Class | Position, size | What |
|---|---|---|---|
| `Label2` | Label | (28, 34) 150 × 24 | "Spraypaint image" (`#GameUI_SpraypaintImage`) |
| `LogoImage` | ImagePanel | (28, 61) 64 × 64 | the selected spray's picture (2.2) |
| `SpraypaintList` | CLabeledCommandComboBox, not editable | (100, 61) 124 × 24 | the sprays (2.2) |
| `ImportSprayImage` | Button | (100, 93) 124 × 24 | "Import Spray..." (`#GameUI_ImportSprayEllipsis`), command `ImportSprayImage` (2.3) |
| `Label3` | Label | (26, 129) 196 × 64 | "Changes to your spraypaint image take effect when you join a server." |
| `DownloadFilterLabel` | Label | (4, 190) 275 × 36 | "When a game server tries to download\ncustom content to your computer" |
| `DownloadFilterCheck` | ComboBox, not editable | (10, 235) 250 × 24 | "Allow all custom files from server", "Do not download custom sounds", "Only allow map files", "Do not download any custom files" (`cl_downloadfilter` all / nosounds / mapsonly / none, same as CS:S) |
| `TextEntry1` | TextEntry, not editable, `zpos` −1 | (277, 33) 190 × 203 | an empty backdrop box behind the model picture (2.4) |
| `ModelImage` | Panel | (277, 34) 240 × 256 | the selected model's picture (2.1) |
| `Label1` | Label | (277, 243) 216 × 24 | "Player model" (`#GameUI_PlayerModel`) |
| `Player model` | CLabeledCommandComboBox, not editable | (277, 266) 190 × 24 | the player models (2.1) |
| `OK` / `Cancel` / `Apply` | Button | (308, 320) / (378, 320) / (448, 320), 64 × 24 | commands `Ok`, `Close`, `Apply` |

A CLabeledCommandComboBox is GameUI's combo whose every entry is a display
text paired with a whole console command line; when the page's changes
are applied, the command of the entry selected then is run (whether an
unchanged selection is run again is not observable: it sets the same
value; Chimera may skip it). Like every
Options page, applying happens on **OK** or **Apply**, never on selection
change; Cancel drops the change. Choosing an entry only updates the
preview picture at once and enables Apply.

Present in the file but hidden (`visible` 0): the "Advanced..." button,
the high-quality-models check box, the primary/secondary colour sliders,
every crosshair control (colour, size, translucency, image, red/green/
blue/scale sliders, the advanced crosshair list and its image, the
crosshair description). So HL2:DM's page has **no crosshair settings and
no Advanced button**.

#### 2.1 Player model (`Player model` combo, `ModelImage` preview)

- **Lists**: one entry per model picture found by walking
  `materials/vgui/playermodels/` and its subfolders through the game's
  search paths (in this install only the HL2:DM VPK has any). Each
  picture is a `.vmt` + `.vtf` pair; count each stem once. HL2:DM's 19:
  `combine_soldier`, `combine_soldier_prisonguard`,
  `combine_super_soldier`, `humans/group03/female_01`, `_02`, `_03`,
  `_04`, `_06`, `_07`, `humans/group03/male_01` … `male_09`, `police`
  (paths below that folder, without extension).
- **Order**: the file system's find order, which the library does not
  sort as far as can be seen. Chimera: sort the paths case-insensitively
  (the order above).
- **Entry text**: not visible without the page (Open questions 5).
  Chimera: the file stem (`male_05`, `police`); the stems are unique in
  the stock set.
- **Command and format**: an entry's command is `cl_playermodel
  models/<path>.mdl`, with `<path>` the picture's path below
  `materials/vgui/playermodels/` (forward slashes, no quotes), e.g.
  `cl_playermodel models/humans/group03/male_05.mdl`,
  `cl_playermodel models/police.mdl`. Run on OK / Apply.
  `cl_playermodel` is the client's archived user-info cvar (saved in the
  config, sent to the server); its default is the word `none`. What the
  server does with it (accepted models, team overrides, when it takes
  effect) is rules.md's.
- **Preview**: the material `vgui/playermodels/<path>` (256 × 256,
  translucent), drawn in the `ModelImage` area at once when the selection
  changes.
- **Nothing chosen**: on opening, the entry whose model equals the
  current `cl_playermodel` is selected (inferred: the library reads that
  cvar by name). With the default `none` (or
  any model not in the list) nothing matches; what the combo then shows
  is the library's (Open questions 6). Chimera: select the first entry
  (`combine_soldier`) as displayed only, and do not run its command unless
  the player changes the selection, so `cl_playermodel` stays `none`
  until a real choice.

#### 2.2 Spray (`SpraypaintList` combo, `LogoImage` preview)

- **Lists**: one entry per `.vtf` file directly in `materials/vgui/logos/`
  (pattern `*.vtf`, no subfolders; the `UI` subfolder holds only `.vmt`
  files the library writes, below). Stock HL2:DM (VPK): `spray_canned`,
  `spray_combine`, `spray_cop`, `spray_dog`, `spray_freeman`,
  `spray_head`, `spray_lambda`, `spray_plumbed`, `spray_soldier`; imported
  sprays (2.3) are added to the same folder in the game's writable
  directory and appear too.
- **Order and text**: file system find order; Chimera sorts
  case-insensitively by stem and shows the stem (Open questions 5).
- **Command and format**: an entry's command is `cl_logofile
  materials/vgui/logos/<stem>.vtf` (no quotes), e.g. `cl_logofile
  materials/vgui/logos/spray_lambda.vtf`. Run on OK / Apply. The note
  `Label3` holds: the new spray is sent when the player next joins a
  server, not to the current one.
- **Preview**: for the selected stem the library makes sure a UI material
  `materials/VGUI/logos/UI/<stem>.vmt` exists in the writable directory
  (it writes one if missing: unlit, translucent, vertex colour and alpha,
  depth test off, base texture `VGUI/logos/<stem>`) and shows image
  `logos/UI/<stem>` in `LogoImage`, at once on selection change. Chimera
  need not write a file: draw `materials/vgui/logos/<stem>.vtf` with alpha
  in the 64 × 64 box. The layout's own default image,
  `logos/UI/Spray_canned`, is what shows before any selection.
- **Nothing chosen**: the engine's default `cl_logofile` is
  `materials/vgui/logos/spray.vtf`, which HL2:DM does not ship, so on a
  fresh config nothing matches. The layout's default picture is
  `Spray_canned`, which points at `spray_canned` (the first entry by
  name) as what the page shows then. The library also knows the command
  `cl_logofile ""` (an empty spray); when it is used (e.g. an empty
  folder) is not visible (Open questions 6). Chimera: show `spray_canned`
  selected without running its command until the player changes it.
  Likewise on opening, the entry matching the current `cl_logofile` is
  selected (inferred, as for the model).

#### 2.3 Import Spray (`ImportSprayImage` button)

- Opens a file-open dialog titled "Import Spray Image"
  (`#GameUI_ImportSprayImage`) with filters, in this order: "All Images
  (*.jpg,*.tga,*.vtf)" (`*.tga,*.jpg,*.vtf`), "Targa Images (*.tga)",
  "JPEG Images (*.jpg)", "VTF Images (*.vtf)". No BMP. Its start folder
  is the game's `materials/vgui/logos` (inferred from the folder string).
- A `.vtf` is taken as is; a `.tga` (24-bit RGB or 32-bit RGBA) or `.jpg`
  is converted into a VTF. Width and height must each be one of 256, 128,
  64, 32, 16, 8, 4, 2, 1, else the error "Source image is not the correct
  size. …" Errors show in a message box titled "Spray Import Error" with
  the `#GameUI_Spray_Import_Error_*` texts (cannot read, corrupt, wrong
  TGA format, out of memory, cannot write temp or output file, converter
  missing).
- The result is written as `materials/vgui/logos/<source file stem>.vtf`
  in the game's writable directory, the list is rebuilt, and the new
  spray is selected (inferred; Open questions 7). As with any selection
  it takes effect (`cl_logofile`) on OK / Apply.
- Chimera: the same filters and size rule; write the converted VTF into
  Chimera's own user data (never into the game install) and list that
  folder's `.vtf` files together with the install's.

#### 2.4 The empty box `TextEntry1`

A TextEntry with no text, `editable` 0, `visible` 1, `tabPosition` 0 (not
reached by Tab), `zpos` −1, at (277, 33) 190 × 203. Nothing in the
library refers to it by name, so nothing hides or fills it: **it is shown
in the real game**. Its `zpos` −1 (the panel settings reader honours
`zpos`) puts it below its siblings, so it is drawn first, behind
`ModelImage` (277, 34) 240 × 256, which covers it all but its top row.
The model pictures are translucent, so the box shows through their
transparent parts: in effect a sunken text-field background (the
scheme's TextEntry background and border) acting as a backdrop for the
model picture. It is a leftover of the layout tool, not a feature; it
takes no input that matters (not editable). Chimera: draw it as an
empty, non-focusable text-field frame behind the model preview, or skip
it if the scheme's text-field look is not available; a screenshot would
settle its exact look (Open questions 3).

#### 2.5 CS:S, for comparison

- Its page shows the CS crosshair editor
  (preview, colour combo, size, thickness, alpha, red/green/blue sliders,
  translucency, dynamic and dot check boxes), "lock radar rotation", a
  "Reset Stats" button, the spray picker, the download filter and the
  visible "Advanced..." button; it has **no** player model picker.

### 3. The Advanced dialog

`resource/multiplayeradvanceddialog.res` (a 472 × 376 frame with an
option list (16, 56) 442 × 274, OK and Cancel) is shipped, but HL2:DM's
Multiplayer page hides the button that opens it. Its list is built from
`cfg/user.scr`, else `cfg/user_default.scr`; **HL2:DM ships neither**, so
the dialog would be empty even if opened. CS:S ships `cfg/user_default.scr`
(`mp_decals`, `cl_righthand`, `cl_clanid`, `cl_autowepswitch`,
`hud_centerid`, `cl_autohelp`, `hud_takesshots`, `cl_disablefreezecam`,
`cl_disablehtmlmotd`, `cl_cloud_settings`). For Chimera: no Advanced
button on HL2:DM's page.

`resource/multiplayercustomizedialog.res` (player model, colour sliders,
high-quality models, spray list and spray colour, its own Advanced
button) is shipped too; nothing in HL2:DM's menus opens it (Open
questions 4).

### 4. Create Server

Two tabs, "Server" and "Game". CS:S's Server tab adds bot controls;
HL2:DM's has none (and no bot tab).

**Server tab** (Half-Life 2's `createmultiplayergameserverpage.res`, page
332 × 364 at (0, 28); HL2:DM has no own copy):

| Field name | Class | Position, size | What |
|---|---|---|---|
| `MapLabel` | Label | (16, 20) 40 × 24 | "Map" (`#GameUI_Map`) |
| `MapList` | ComboBox, not editable | (56, 20) 200 × 24 | the installed maps (`maps/*.bsp`) plus a random-map entry (`#GameUI_RandomMap`) |
| `VisibilityLabel` | Label | (16, 50) 200 × 24 | "Server Type" (`#Server_Visibility`) |
| `VisibilityType` | ComboBox, not editable | (16, 74) 200 × 24 | Server Type, below |

**Server Type (`VisibilityType`)**: a plain ComboBox (not a command
combo): the dialog itself turns the choice into console commands when
**Start** is pressed (nothing happens on change). Values (localisation key,
English text) in the order the library holds them, which is the order to
show (inferred from the library's strings: the first three are together,
Friends Only was added apart from them; Open questions 1):

| # | Key | Text | Commands on Start (inferred) |
|---|---|---|---|
| 0 | `#Server_Visibility_Local` | Local Network (LAN) | `sv_lan 1`, `sv_allow_server_adverisement_to_master_server 0`, `sv_friends_only 0` |
| 1 | `#Server_Visibility_Unlisted` | Public (Unlisted) | `sv_lan 0`, `sv_allow_server_adverisement_to_master_server 0`, `sv_friends_only 0` |
| 2 | `#Server_Visibility_Global` | Public (Listed) | `sv_lan 0`, `setmaster enable`, `sv_allow_server_adverisement_to_master_server 1`, `sv_friends_only 0` |
| 3 | `#Server_Visibility_FriendsOnly` | Friends Only | `sv_lan 0`, `sv_allow_server_adverisement_to_master_server 0`, `sv_friends_only 1` |

(`adverisement` is the cvar's real spelling.) The library also issues
`sv_use_steam_networking 1` or `0`; that belongs to the "Use Steam
Networking" check box (`EnableSteamNetworkingCheck`), which this layout
lacks, so its value without the box is the library's (Open questions 1;
Chimera has no Steam networking: ignore it). The **default** is not in
any data file; the dialog stores its last choices in `ServerConfig.vdf`
in the user's config and restores them. Chimera: default to **Local
Network (LAN)** on a fresh config (the only value that needs no master
server, and the safe choice), remember the last choice after that.

Order of the start sequence (from the library's command strings, in that
order): the Server Type commands; `disconnect`, two `wait`s;
`maxplayers <n>`, `sv_password "<text>"`, `hostname "<text>"` from the
Game tab's values; `progress_enable`; `map <name>`. The Game tab's other
rows are applied as commands too (below), before `map` (where exactly
in the sequence is not visible; any point before `map` gives the same
result). With an empty hostname the server is
named "<player name>'s Server" (inferred from the strings; "Someone"
when the name is unknown).

For Chimera: Local = LAN only, not shown in any internet list; Unlisted
= reachable by address, not advertised; Listed = advertised to the
server list; Friends Only = only the host's friends may join (the
`sv_friends_only` meaning; no Steam in Chimera, so it behaves like
Unlisted unless Chimera grows a friends list).
- No server name or password field on this tab: the library looks for
  controls named `ServerNameEdit`, `PasswordEdit` and
  `EnableSteamNetworkingCheck` ("Use Steam Networking"), and neither
  Half-Life 2's nor HL2:DM's file has them, so they do not appear. Name,
  password and player count are on the Game tab.

**Game tab** (`createmultiplayergamegameplaypage.res`: an option list at
(10, 12) 312 × 340): one row per entry of the game's `cfg/settings.scr`
(else `cfg/settings_default.scr`), in file order, each row a prompt and a
control by type (STRING a text box, NUMBER a numeric box with its range,
BOOL a check box, LIST a combo). HL2:DM's file:

| # | Cvar | Prompt | Type | Default |
|---|---|---|---|---|
| 1 | `hostname` | "Hostname" | text | Half-Life 2 Deathmatch |
| 2 | `sv_password` | "Server password" | text | empty |
| 3 | `maxplayers` | "Max. players" | number 1–32 | 32 |
| 4 | `sv_password` | "Server password" | text | empty |
| 5 | `mp_timelimit` | "Time Limit (Min.)" | number, min 0, no max | 0 |
| 6 | `mp_fraglimit` | "Frag Limit" | number, min 0, no max | 0 |
| 7 | `mp_teamplay` | "Teamplay" | check | off |
| 8 | `mp_weaponstay` | "Weapons Stay" | check | off |
| 9 | `mp_footsteps` | "Footsteps" | check | on |
| 10 | `mp_flashlight` | "Flashlight" | check | off |
| 11 | `mp_forcerespawn` | "Force Respawn" | check | off |

- **"Server password" shows once**, as the second row. The shared
  script-option reader (public SDK) looks every entry up by cvar name
  (case-insensitive) before adding it: a later entry with a name already
  read adds no new row; it only replaces the existing row's default (and
  current value, and its tooltip if it has one), keeping the first
  entry's prompt, type, range and list items. So the Game tab shows 10
  rows: `hostname`, `sv_password`, `maxplayers`, `mp_timelimit`, …
  `mp_forcerespawn`, with the password at position 2 (both entries are
  identical here). Chimera: same rule for any `.scr`: one row per cvar at
  its first position, the last entry's default wins. When the
  file is written back (below) the duplicate is gone.
  CS:S's `settings_default.scr` has it once (`hostname`, `maxplayers`,
  `sv_password`, then its round and money settings).
- **How a row becomes a command** (shared reader, public SDK): each row
  is run as `<cvar> "<value>"`: a check box as `"1"` / `"0"`; a number as
  a decimal with six places (`"20.000000"`), clamped to the row's min
  and max where those are not −1; a text with the text inside the quotes
  (characters invalid in a command removed); a LIST row as the selected
  item's value (`"0"` when none matches). A row marked `SetInfo` in the
  script is sent as `setinfo <cvar> "<value>"`; HL2:DM's has none.
- The script's header says the file is regenerated: starting a server
  writes the chosen values back into the user's `cfg/settings.scr`, so
  the dialog remembers the last choices.

### 5. Other GameUI differences from CS:S

- `resource/optionssubadvanced.res` (HL2:DM's own) holds only a "Content
  Lock" button (40, 42) 100 × 24 (`ContentControl`) and its label (162,
  34) 300 × 40: the parental content lock page.
- No CS:S-only dialogs (achievements, stats, buy presets, bots).
- Everything else (video, audio, keyboard, mouse, voice, server browser)
  is the shared library with Half-Life 2's files and is the same as in
  CS:S apart from the key list (`scripts/kb_act.lst`, game data).

## Test cases

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | not connected | open the main menu | FIND SERVERS, CREATE SERVER, OPTIONS, QUIT (no Achievements, Report Bug or Benchmark) |
| 2 | connected | Esc | RESUME GAME, DISCONNECT, MUTE PLAYERS, a gap, then the four above |
| 3 | Options > Multiplayer | – | `SpraypaintList` (9 stock entries), `ImportSprayImage`, `DownloadFilterCheck`, `Player model` with 19 entries, `TextEntry1` empty behind `ModelImage`; no crosshair controls, no Advanced button |
| 4 | choose `male_05` in `Player model`, OK | – | runs `cl_playermodel models/humans/group03/male_05.mdl` |
| 5 | choose `spray_lambda` in `SpraypaintList`, OK | – | runs `cl_logofile materials/vgui/logos/spray_lambda.vtf` |
| 6 | choose `police`, then Cancel | – | `cl_playermodel` unchanged; preview had shown `vgui/playermodels/police` |
| 7 | choose `police`, Apply | – | `cl_playermodel` = `models/police.mdl` at once, page stays open |
| 8 | fresh config (`cl_playermodel none`), open the page, OK without touching | – | `cl_playermodel` stays `none` |
| 9 | Import Spray, pick a 100 × 100 TGA | – | "Spray Import Error" box, wrong-size text; list unchanged |
| 10 | Import Spray, pick a 128 × 128 32-bit TGA `myspray.tga` | – | `myspray` in the list and selected; OK runs `cl_logofile materials/vgui/logos/myspray.vtf` |
| 11 | Create Server > Server | – | `MapList` and `VisibilityType` with Local Network (LAN), Public (Unlisted), Public (Listed), Friends Only; no bot controls, no name or password field |
| 12 | Create Server, fresh config, Server Type left alone, Start | – | `sv_lan 1` (Chimera's chosen default) |
| 13 | Server Type Public (Listed), Start | – | `sv_lan 0`, server advertised |
| 14 | Create Server > Game, fresh config | – | 10 rows: the table of section 4 without row 4 |
| 15 | Game tab: Max. players 20, Teamplay on, Start | – | `maxplayers 20`, `mp_teamplay "1"`, then `map <chosen>` |

## Open questions

1. **Server Type**: the order (inferred Local, Unlisted, Listed, Friends
   Only), the preselected value on a fresh config, the exact commands per
   value (inferred table in section 4) and `sv_use_steam_networking`
   without its check box are in GameUI's code, which is not public. Check
   in the game: open Create Server on a fresh config, read the combo;
   start with each value and read `sv_lan`, `sv_friends_only`,
   `sv_allow_server_adverisement_to_master_server`.
2. ~~Duplicate `sv_password` row~~: answered from the public shared
   reader: one row (section 4).
3. **`TextEntry1`'s look**: shown (section 2.4); a screenshot would
   confirm how much of its frame shows around and through the model
   picture.
4. **Customize dialog**: whether anything opens
   `multiplayercustomizedialog.res` in HL2:DM (expected: nothing).
5. **Combo texts** of the model and spray pickers (the stem, e.g.
   `male_05`, or the path `humans/group03/male_05`) and their order need
   a look at the page.
6. **Nothing chosen**: what the model and spray combos show when the
   cvar matches no entry (blank, or the first entry), and when the
   library runs `cl_logofile ""`.
7. **After Import Spray**: whether the new spray is selected
   automatically, the dialog's start folder, and what happens when a
   spray of that name already exists (overwrite or refuse).
