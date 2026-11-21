# Ultimate Gun Game: playtest guide for testers

Thanks for helping. This is an early build made by AI coding agents; it will
have bugs, and finding them is the point. This page is everything you need.

## What it is

Ultimate Gun Game is a free-for-all shooter. Everyone climbs the same ladder
of 13 guns, one rung per kill. The ladder mixes guns from Counter-Strike:
Source, Half-Life 2: Deathmatch and Combat Arms, and each gun makes you its
game: your movement, body and HUD change with the gun in your hand. The
last rung is melee. The first kill on the last rung wins the match, then the
players vote on the next map.

You do not need any of those games installed. The game fetches the files it
needs from our server, and it does so even if you have them installed, so
everyone plays exactly the same files. To use your own installs instead,
turn on **Settings ▸ Network ▸ Use my installed games** (it applies at the
next start); the server's files are the ones the host tests with, so leave
it off unless the host asks.

## What you need

- Windows 10 or 11, 64-bit.
- A GPU with DirectX 12 or Vulkan (anything from the last 8 years or so).
- 16 GB of RAM recommended (8 GB is tight: memory grows over a long session,
  and quitting and rejoining after an hour or so resets it).
- Free disk space for the downloaded game files: about 1 GB for the first
  match, a few GB over the day (the maps and guns the matches use; kept in
  `%LOCALAPPDATA%\chimera\assets`, delete it to start over).
- A wired or good Wi-Fi connection; the game needs roughly 0.15 Mbit/s per
  player once it is running (the first download is the big one).
- From the host: the **release link**, your **invite link** and the
  **server address**. Do not share the invite: it is yours alone.

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

## First run: your invite

The first time the game starts it asks for your **invite**: paste the
link the host sent you (it starts with `chimera-hub://`; the bare code
works too once the game knows the hub) and press Enter. Or **click your invite**: once the game has started once on this PC, a
`chimera-hub://` link opens it (or the game already open) with the
invite filled in and checked; the browser asks first whether to open the
game, say yes. Chat apps may not make such a link clickable: paste it
into the browser's address bar instead. The game shows
the hub's name; check it is the host's, then:

1. Choose a **username** (3 to 20 letters, digits or `_`, starting with a
   letter; the game says at once if it is taken and offers free ones), a
   display name if you want another, and an avatar.
2. Save your **recovery key**: Copy or Save as file, then tick "I saved
   it". It gets your account back if you lose this PC; keep it private.
3. You land on the main menu, signed in (your name in the corner).

From then on the game signs you in by itself; there is nothing to type
and nothing else to keep. The invite is single use: if it says "used" or
"withdrawn", ask the host for a new one. To start from a command prompt
instead, `Play-UGG.bat +chimera:hub <your invite link>` does the same.

A second PC: on the signed-in one, Settings > Account > **Link a device**
shows a short code; on the new one choose "I already have an account" and
type it. A lost PC: "I already have an account" > Recover with your
recovery key, or ask the host for a new invite into your account.

"Play without an account" plays on the LAN and by address with the games
installed on your PC only: no downloads, no server list.

## Joining the server

The main menu starts over its own animated backdrop. Once you have joined
a match, later starts show one of the maps you already have behind the
menu (never downloading anything for it).

1. At the main menu choose **Join**.
2. Type the address the host gave you: `<host address>` (add `:<port>` if
   the host named a port other than 27015) and press Connect.
3. If the host set a password, type it too.

Or from the console (`~`): `connect <host address>:<port>`.

The first time, the game downloads the files it needs, in its own window:
the loading panel says "Downloading game files: X of Y MB" with the speed
and time left (what the games read at startup, then the match's files),
then the map loads and you spawn. There is no second window
and the game doesn't start again. The first
start takes the longest (minutes on a slow line; the host may ask you to do
it the day before), and the next ones read what is already saved. The
next map downloads once the vote has picked it, during the match-end
screen and the lobby; the music's next track downloads in the background
while one plays. If the download fails, the loading panel says
why; join again once the host has fixed it. If it stalls, wait a minute, then see
"Reporting a bug". Messages you may see instead:

- "the asset service refused you": this device or your account was
  revoked; ask the host for a new invite.
- "can't reach the asset service at ...": the host's asset service is down
  or its port isn't open; tell the host.
- "This match needs ... sign in with your invite link": you joined without
  an account (you chose "Play without an account"); start the game with
  `Play-UGG.bat +chimera:hub <your invite link>`.

**The lobby before each match.** Each match (the first, and each new map)
waits until everyone has the map. Over the map (or over your loading panel
while you still download) a panel shows the mode, the map and every player
with their status: Connecting, Downloading (a bar with MB done of the total
and the percentage), Loading map, Ready; bots are ready at once, and your
own row is on a gold band. A line says how many players it waits for and
when it starts anyway (the host's longest wait, 90 s by default, counted
from when the first player arrived). Once everyone is ready a big countdown
(5 s) runs and everyone spawns together. If you are ready first you stand
frozen at your spawn until then. If you are still downloading when it
starts, you join the running match when you are ready, as a late joiner.

If a join is refused, the message on the menu says why. "Different build"
means the host and you are on different versions: download the newest release.

**If the server goes away** (the host restarts it, or it crashes), your
game goes back to the menu and shows "Connection lost - reconnecting...
(attempt N)" with the reason. It asks the server every 3 seconds and joins
it again on its own once it is back (the lobby and any downloads as on
the first join), for up to 2 minutes; then it says it gave up and you
rejoin from Join. Cancel or Escape stops it. A server that crashed is
noticed after 30 seconds without a word from it. `cl_autoreconnect 0` in
the console turns this off.

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

The easiest way, in a build that has it: press **F10** (or Esc > **Report a
bug**) when it happens. The game takes a screenshot of that moment, then
opens a small report window: pick **Bug** or **Feedback** (Tab), type what
happened and what you expected, keep or untick **Include screenshot**, and
press **Send** (Ctrl+Enter). It goes to the host's hub, signed in as you;
"What's sent" lists what goes with it, and no passwords, keys or game
files are in it. Without an account, or while the host's service doesn't
answer, the report waits on your PC under
`%LOCALAPPDATA%\chimera\feedback\outbox\` and goes later by itself (the
console's `feedback_status` says why it waits); if it never goes, zip that
folder and send it. F10 can be moved in Options > Controls ("Report a
bug").

Without it, or for a quick save: press **F9** (or **F4**) when it happens. It saves a
screenshot and a `report.txt` (build, map, weapon, health, position, the last
console lines) into a new folder under

`%LOCALAPPDATA%\chimera\bugreports\`

(paste that into the Explorer address bar) and the console says the folder's
name. Zip that folder and send it. You can add a note: `bugreport the gun
floated` in the console.

For crashes and freezes, in a build that has it: the game keeps a log of
its last sessions (`%LOCALAPPDATA%\chimera\logs\session-<time>.log`, the
last five, with a line of memory and frame times every 10 seconds). If it
crashes, freezes and is killed, or the PC goes down, the next start says
**"The last session ended unexpectedly. Send a report?"** on the main menu:
**Send** sends that session's log tail (and, after a crash, what crashed)
to the host as a report; **Always send** does that from now on without
asking; **Not now** keeps nothing. No passwords, tokens or addresses are in
the log. The console's `cl_crash_reports ask|always|never` is the same
choice.

In an older build: `Play-UGG.bat` saves everything the console window
shows into `ugg-last.log` in the game's folder (overwritten on each start,
so copy it before you start again). If you started the game another way,
copy the console window's text, or take a photo of it before closing it.
Send `ugg-last.log` plus what you were doing. If the game closes by
itself, send the log first, then the details.

## Known issues

Things you may hit, so you do not need to report them (unless they look
worse than described):

- **Hitches when a new map loads.** The map swap and the first seconds on a
  map can stutter. A load takes 3 to 8 seconds once the files are saved.
  The first time you play a map its files download; for a map the host
  didn't prepare that can take minutes on a slow line, and the match
  starts without you 20 seconds after the vote (you join it when your
  load ends).
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
- **Server list**: Join lists the servers on your network, your
  favourites and the ones you joined before. Its Internet tab lists the
  host's hub's servers once you are signed in to that hub (the invite
  link you were sent); until then join by address.
- **Menu widgets are plain**: option choices use `-`/`+` buttons instead of
  drop-downs.
- **Many fixes are new and untested by anyone but the author**, so tell us if
  these look wrong: ambient sounds carrying over to the next map, the death
  camera and "Respawning in N" countdown, projectiles (grenade launcher,
  rockets, bolts) looking smooth on your screen, kill-feed gun icons,
  attachments showing on guns, bullet holes on walls, the rung-up and win
  sounds, the end-of-match vote.

Have fun, and thank you.
