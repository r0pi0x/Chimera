# Ultimate Gun Game: playtest guide for testers

Thanks for helping. This is an early build made by AI coding agents; it will
have bugs, and finding them is the point. This page is everything you need.

Anything marked `TODO(asset flow)` is a step that is still being built and
will be filled in before the playtest. The host will tell you the exact
words in the meantime.

## What it is

Ultimate Gun Game is a free-for-all shooter. Everyone climbs the same ladder
of 13 guns, one rung per kill. The ladder mixes guns from Counter-Strike:
Source, Half-Life 2: Deathmatch and Combat Arms, and each gun makes you its
game: your movement, body and HUD change with the gun in your hand. The
last rung is melee. The first kill on the last rung wins the match, then the
players vote on the next map.

You do not need any of those games installed. The game fetches the files it
needs from our server.

## What you need

- Windows 10 or 11, 64-bit.
- A GPU with DirectX 12 or Vulkan (anything from the last 8 years or so).
- 16 GB of RAM recommended (8 GB is tight: memory grows over a long session,
  and quitting and rejoining after an hour or so resets it).
- Free disk space for the downloaded game files: TODO(asset flow) (the
  maps and guns the matches use; expect several GB over the day).
- A wired or good Wi-Fi connection; the game needs roughly 0.15 Mbit/s per
  player once it is running (the first download is the big one).
- From the host: the **release link**, your **token** and the **server
  address**. Do not share them.

## Download and start

1. Download the Windows zip from the release link the host sent you
   (`LuckerParty-<version>-windows.zip` or `UltimateGunGame-<version>-windows.zip`;
   the host will say which).
2. Unzip it to a normal folder, e.g. `C:\UltimateGunGame`. Do not run it from
   inside the zip, and avoid `Program Files` (the game writes next to itself).
3. Start `Play-UGG.bat` (it runs `ultimate_gun_game.exe` and saves a log, see
   "Reporting a bug"). Running `ultimate_gun_game.exe` directly also works.
4. Windows may show "Windows protected your PC" (SmartScreen), because the
   program is not code-signed: click **More info**, then **Run anyway**.
   Windows Defender Firewall may also ask whether the game may use the
   network: allow it (private networks is enough for joining).
5. A console window opens beside the game window. Leave it open: it shows
   what the game is doing and is what to copy when something goes wrong.

## First run: your token

TODO(asset flow): how the game asks for the token and server's asset address
(a prompt in the game, or a settings file next to the exe), and what
confirms it worked. Until then the host will tell you. The token looks like
`chimera1_...`; paste it exactly, and keep it private: it is yours alone and
the host can revoke it.

## Joining the server

1. At the main menu choose **Join**.
2. Type the address the host gave you: `<host address>` (add `:<port>` if
   the host named a port other than 27015) and press Connect.
3. If the host set a password, type it too.

Or from the console (`~`): `connect <host address>:<port>`.

The first time you join, the game downloads the files the match needs.
TODO(asset flow): the real screen, its text and its progress display. Expect
a loading screen with a progress bar and the speed of the download; the
first join takes the longest (minutes on a slow line), and the next ones
read what is already saved. If it stalls, wait a minute, then see
"Reporting a bug".

If a join is refused, the message on the menu says why. "Different build"
means the host and you are on different versions: download the newest release.

## Controls

The defaults (Esc > Options > Binds, or `F6`, changes any of them):

| Action | Key |
|---|---|
| Move | `W` `A` `S` `D` |
| Jump | `Space` |
| Crouch | `Ctrl` (Combat Arms and CS:S guns alike) |
| Sprint (Combat Arms guns) | `Shift` |
| Walk (Combat Arms guns: `C`; CS:S guns: `Shift`) | |
| Fire / secondary fire (scope, knife alt) | left / right mouse button |
| Reload | `R` |
| Use | `E` |
| Previous weapon | `Q` |
| Weapon slots | `1` to `6` (the ladder gives you one gun at a time, so mostly the knife swap) |
| Scoreboard | hold `Tab` |
| Text chat | `Enter` opens it; `F2` all, `F3` team (no teams in this mode) |
| Voice chat | hold `T` (push to talk); `F8` toggles hearing voice |
| Radio menus | `B` report, `V` answer, `N` request |
| Radar | `-` and `=` zoom; hold `M` for the full map |
| Menu | `Esc` (resume, options, vote to skip the map, disconnect, quit) |
| Console | `~` |
| Bug report | `F9` or `F4` |

The rules of the match:

- A kill moves you up a rung to the next gun. The ladder is the same for
  everyone and is dealt fresh for each match (and for each map).
- Dying to a knife or melee weapon costs you a rung; so does a suicide or a
  fall. You cannot go below the first rung.
- One or two rungs are grenades (you get another if you throw it and miss),
  and the last rung or two are melee.
- You respawn after 2 seconds and cannot be hurt for 2 seconds more.
- Guns on the ground are not picked up; the ladder is the only way to arm.
- A player who joins late starts on the lowest rung of the others.
- A match needs at least 2 players (bots fill empty slots), lasts up to
  10 minutes and ends when someone kills with the last gun. Then everyone
  votes on the next map. `Esc` > "Vote to skip the map" calls a vote.
- The games are not balanced against each other on purpose: a lucky or
  unlucky draw of guns is part of the fun.

Your rung and the ladder show in the HUD panel; the scoreboard shows
everyone's rung.

## Settings worth knowing

Esc > Options:

- **Mouse > Sensitivity**: the first thing to set. Also zoomed sensitivity,
  mouse filter, acceleration.
- **Video**: field of view (75 to 90), Display (Windowed, Full screen,
  Borderless), Vertical sync, Frame limit, Brightness. If the game runs
  badly, set a Frame limit of 60 and Display to Borderless first.
- **Audio**: game volume, music volume, mute when unfocused.
- **Party**: hit marker, swap effect, music (party playlist, the game's own
  loop or none), radar.
- **Binds**: all keys; a gun view FOV per game is on each game's tab.

Your settings are saved in `%APPDATA%\chimera\cfg\config.cfg`. Deleting that
file puts everything back to the defaults.

## Reporting a bug

Please send the host (a message in the group chat is fine):

1. **What happened**, in a sentence or two, and what you expected.
2. **When**: roughly what time, and what you were doing (just spawned, mid
   fight, on the end screen, loading the next map).
3. **The map and the gun** you held (the HUD shows both).
4. **A screenshot**, ideally from the bug report below.
5. **The log** (see below).

The easiest way: press **F9** (or **F4**) when it happens. It saves a
screenshot and a `report.txt` (build, map, weapon, health, position, the last
console lines) into a new folder under

`%LOCALAPPDATA%\chimera\bugreports\`

(paste that into the Explorer address bar) and the console says the folder's
name. Zip that folder and send it. You can add a note: `bugreport the gun
floated` in the console.

For crashes and freezes: the game has no crash reporter. `Play-UGG.bat`
saves everything the console window shows into `ugg-last.log` in the game's
folder (overwritten on each start, so copy it before you start again). If you
started the game another way, copy the console window's text, or take a photo
of it before closing it. Send `ugg-last.log` plus what you were doing. If
the game closes by itself, send the log first, then the details.

## Known issues

Things you may hit, so you do not need to report them (unless they look
worse than described):

- **Hitches when a new map loads.** The map swap and the first seconds on a
  map can stutter. A load takes 3 to 8 seconds once the files are saved.
- **Memory grows over a long session.** If the game slows down after an hour,
  restart it.
- **Some guns look or work oddly**: the Combat Arms "Neon Outlaw" ASW338LM
  has scrambled textures and the "AUG A3 Champions Lightning" is grey (the
  game's own files); the RMS-12 flamethrower and the AT4 do no damage (as in
  the original game's data).
- **A faint vertical line** across the frame on de_dust2 near the B tunnels
  is the map's lighting, as the original draws it.
- **Explosion fire** is cut off where it meets walls.
- **Sound balance differs by game**: some guns and footsteps are louder or
  heard farther than others. That is known and not being balanced.
- **No server browser yet**: join by address only.
- **Menu widgets are plain**: option choices use `-`/`+` buttons instead of
  drop-downs.
- **Many fixes are new and untested by anyone but the author**, so tell us if
  these look wrong: ambient sounds carrying over to the next map, the death
  camera and "Respawning in N" countdown, projectiles (grenade launcher,
  rockets, bolts) looking smooth on your screen, kill-feed gun icons,
  attachments showing on guns, bullet holes on walls, the rung-up and win
  sounds, the end-of-match vote.

Have fun, and thank you.
