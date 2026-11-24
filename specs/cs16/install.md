# Counter-Strike 1.6: the install

Source basis: <what this was worked out from, by role: "the retail install's
folder layout", "the community wiki's file format pages"; no paths>
Status: draft

## Summary
Where a player's copy of Counter-Strike 1.6 comes from, what is in it, and which
files Chimera reads.

## How a player gets it
Store, app id if any, a free server package if any, the version this spec
describes.

## Layout
The folders and files that matter, by role (maps, models, textures,
sounds, scripts), and which are required to call a folder an install.

## How files are found
The game's search order (archives, loose folders, mods), and what wins
when two hold the same name.

## Formats
One line per format the game uses: what it holds, whether it is
encrypted (never write a key here: the user supplies keys locally), and
whether a parser exists under a licence THIRD_PARTY.md allows.

## Units and conventions
The world unit in metres, the up axis, the tick rate.

## Open questions
