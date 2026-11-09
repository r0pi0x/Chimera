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
searched for file paths, cvar names and localisation keys only. GameUI's
code is not in the public SDK, so behaviour that only its code decides is
listed under Open questions instead of guessed.
Status: draft

## Summary

HL2:DM uses the same GameUI library as CS:S; what differs is data. Its
main menu lacks CS:S's Achievements, Report Bug and Benchmark entries; its
Multiplayer options page shows a spray picker, a player-model picker and
the download filter, and hides every crosshair control and the Advanced
button; Create Server has no bot controls; the Game tab is built from
HL2:DM's `cfg/settings.scr`, which lists `sv_password` twice.

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

Page 496 × 312 at (0, 28). Visible controls:

| Control | Position, size | What |
|---|---|---|
| Label "Spraypaint image" | (28, 34) 150 × 24 | |
| Spray preview (image) | (28, 61) 64 × 64 | the selected spray's picture |
| Spray list (combo) | (100, 61) 124 × 24 | one entry per spray texture |
| "Import Spray..." button | (100, 93) 124 × 24 | command `ImportSprayImage` (file picker that converts an image into a new spray) |
| Note "Changes to your spraypaint image take effect when you join a server." | (26, 129) 196 × 64 | |
| Download filter label "When a game server tries to download\ncustom content to your computer" | (4, 190) 275 × 36 | |
| Download filter (combo) | (10, 235) 250 × 24 | "Allow all custom files from server", "Do not download custom sounds", "Only allow map files", "Do not download any custom files" (`cl_downloadfilter` all / nosounds / mapsonly / none, same as CS:S) |
| Model preview panel | (277, 34) 240 × 256 | the selected model's picture |
| An empty text box | (277, 33) 190 × 203 | visible in the file, no text; see Open questions 3 |
| Label "Player model" | (277, 243) 216 × 24 | |
| Player model (combo) | (277, 266) 190 × 24 | one entry per model picture |
| OK / Cancel / Apply | (308, 320) / (378, 320) / (448, 320), 64 × 24 | |

Present in the file but hidden (`visible` 0): the "Advanced..." button,
the high-quality-models check box, the primary/secondary colour sliders,
every crosshair control (colour, size, translucency, image, red/green/
blue/scale sliders, the advanced crosshair list and its image, the
crosshair description). So HL2:DM's page has **no crosshair settings and
no Advanced button**.

- **Player models**: one entry per picture under
  `materials/vgui/playermodels/` (in the HL2:DM VPK: `combine_soldier`,
  `combine_soldier_prisonguard`, `combine_super_soldier`, `police`,
  `humans/group03/female_01`, `_02`, `_03`, `_04`, `_06`, `_07`,
  `humans/group03/male_01` … `male_09`: 19). The picture's path below
  that folder is the model's path below `models/`: choosing an entry sets
  `cl_playermodel` to `models/<path>.mdl` (e.g. `models/police.mdl`,
  `models/humans/group03/male_05.mdl`), and the preview shows
  `vgui/playermodels/<path>`. The player-model rules (which models the
  server accepts, team overrides) are rules.md's.
- **Sprays**: one entry per `.vtf` in `materials/vgui/logos/` (stock:
  `spray_canned`, `spray_combine`, `spray_cop`, `spray_dog`,
  `spray_freeman`, `spray_head`, `spray_lambda`, `spray_plumbed`,
  `spray_soldier`; imported ones land in the same folder). Choosing one
  sets `cl_logofile` to `materials/vgui/logos/<name>.vtf`. The preview is
  that texture (the file's default image `logos/UI/Spray_canned` is not in
  the install; the library makes a UI material for the chosen logo
  itself, under `materials/VGUI/logos/UI/`).
- **CS:S, for comparison**: its page shows the CS crosshair editor
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

- "Map" label (16, 20) and map combo (56, 20) 200 × 24: the installed maps.
- "Server Type" label (16, 50) and combo (16, 74) 200 × 24. Its values
  (localisation keys and English texts): `#Server_Visibility_Local` "Local
  Network (LAN)", `#Server_Visibility_FriendsOnly` "Friends Only",
  `#Server_Visibility_Unlisted` "Public (Unlisted)",
  `#Server_Visibility_Global` "Public (Listed)". The library also knows
  the commands `sv_lan 1` and `sv_lan 0`. The list order, the default
  and which cvars each value sets are the library's (Open questions 1).
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

- **"Server password" twice**: the data lists it twice (rows 2 and 4,
  same cvar, same prompt). Whether the library drops the second copy or
  shows two rows is its code's (Open questions 2). Either way only one
  value can reach the server, since both rows write `sv_password`; which
  row wins when two are shown with different texts is also open.
  CS:S's `settings_default.scr` has it once (`hostname`, `maxplayers`,
  `sv_password`, then its round and money settings).
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
| 3 | Options > Multiplayer | – | spray picker, Import Spray..., download filter, player model picker with 19 entries; no crosshair controls, no Advanced button |
| 4 | choose `male_05` in the model picker, OK | – | `cl_playermodel` = `models/humans/group03/male_05.mdl` |
| 5 | choose `spray_lambda`, OK | – | `cl_logofile` = `materials/vgui/logos/spray_lambda.vtf` |
| 6 | Create Server > Server | – | Map combo and Server Type combo with the four values; no bot controls, no name or password field |
| 7 | Create Server > Game, fresh config | – | the rows of section 4 (row 4 subject to Open questions 2) with their defaults |

## Open questions

1. **Server Type**: the order of the four values in the combo, which is
   preselected, and what each sets (`sv_lan`; Steam visibility for
   Friends Only and Unlisted) are in the library's code. Check in the
   game: open Create Server, read the combo, start a server with each and
   read `sv_lan` (and whether it is listed in the server browser).
2. **Duplicate `sv_password` row**: open Create Server > Game on a fresh
   install (delete the user's `cfg/settings.scr` first so the shipped one
   is used) and count the "Server password" rows. Until checked, Chimera
   shows one row (first occurrence wins), which gives the same result on
   the server.
3. **The empty text box** (`TextEntry1`, (277, 33) 190 × 203, visible)
   overlapping the model preview: whether it is drawn (a bordered empty
   box behind or over the model picture) needs a screenshot of the page.
4. **Customize dialog**: whether anything opens
   `multiplayercustomizedialog.res` in HL2:DM (expected: nothing).
5. **Combo texts** of the model and spray pickers (the file stem, e.g.
   `male_05`, or a path) need a look at the page.
