# Brief: a spec session for a game

Paste into a **fresh** AI session started in a checkout of this
repository, on the machine with the game installed. Fill in the
`<...>`. The rules: [CONTRIBUTING-GAMES.md](../CONTRIBUTING-GAMES.md),
[specs/README.md](../../specs/README.md).

```
You are a spec session for <Game> (id <id>) in the Chimera repository.
Read specs/README.md, docs/CONTRIBUTING-GAMES.md ("two kinds of
sessions") and specs/<id>/README.md first, and follow them exactly.

Task: write specs/<id>/<topic>.md (<what it covers: e.g. the player's
movement; the pistols' and rifles' numbers; the map format>), from the
template in specs/<id>/<topic>.md if it exists, else specs/README.md's.

You may study the original game: play and measure it (a local server,
console commands, recordings), read the data and script files of my
own install at <path> (describe what they hold; quote a value only to
state it), and read public pages (wikis, format write-ups) at <links>.
You may not read or use leaked, decompiled or SDK code, and you never
write implementation code.

Write behaviour in our own words: constants with units, formulas as
math, per-tick order, edge cases, quirks, and test cases with concrete
expected numbers. No code or pseudocode, no identifiers, comments or
strings copied from the game's code, no keys, no game files. Mark what
you measured and how; put anything uncertain under "Open questions".

Commit only files under specs/<id>/ (one commit: "<Game> spec:
<topic>"); update the table in specs/<id>/README.md. Do not touch any
other file.
```
