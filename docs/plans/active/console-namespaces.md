# Console namespaces

Approved by the user on 2026-10-31 (docs/backlog.md, "Console namespaces").
With several games in one app, flat console names collide (`bot_add`,
`options`, `cl_righthand`). Every cvar and command gets a namespace, like
our ids.

## Design as built

- **Full names.** `<namespace>:<name>`: `cs_source:sv_cheats`,
  `combat_arms:MaxPlayer`, `chimera:noblock`. The full name always works.
  Names are case-insensitive, as before.
- **Which namespace.** A name registered with a namespace is in it. A bare
  name is one of Source's, the names Chimera's engine implements (its
  movement, network, video and audio settings), which are CS:S's: it goes
  to `cs_source` (`console::SOURCE_NAMESPACE`). Combat Arms registers its
  names as `combat_arms:` (its `ca_*` names keep their prefix: they are
  its own established names). Chimera's own names are `chimera:`: the
  `chimera_*` ones lose the prefix (`chimera_noblock` is
  `chimera:noblock`), our general settings lose their `sv_`/`cl_`/`net_`
  prefix (`cl_hitmarker` is `chimera:hitmarker`, `OLD_NAMES`), and the
  console's own language (`bind`, `alias`, `exec`, `echo`, `help`, `find`,
  `scope`, ...) is `chimera:`, as every game's console uses it. The
  input actions (`+jump`) are Source's names, so `cs_source:`; binds keep
  them as typed.
- **Namespace aliases.** Each game's `GameInfo::console_namespaces`: the
  first is the canonical one (saved in configs, sent on the wire), the
  rest are accepted everywhere a namespace is (`css:` for CS:S, `ca:` for
  Combat Arms). Making `css` canonical is swapping that list's order.
- **Short names.** A short name resolves in the scope's game first, then
  `chimera`, then wherever it is the only one. Several left: an error
  with a hint listing the full names and `scope`.
- **Scope.** `scope <game>` (an alias works) pins it; `scope auto`
  follows the default again: the local player's `GameIdentity` (the
  party apps: the game of the gun in hand), else the app's default game.
  `scope` alone says what it is.
- **Old names.** `OLD_NAMES` (renamed settings) and the old prefixes
  (`chimera_*`, `mashup_*`) still resolve, hidden from completion, so
  scripts, tests, `playtest.ps1` and saved configs keep working.
- **Saving.** config.cfg writes cvars by full name. `bind`, `bindgame` and
  `alias` store each command's name in full (`Console::qualify_line`),
  except Chimera's own names that no game shares, `+`/`-` actions and
  the binds the client polls (`slot1`, `buymenu`, ...: `Console::keep_short`).
- **Network.** Replicated cvars travel by full name; a client's commands
  for its server (cheats, `kill`: `ClientCommand`, our RCON) are sent
  with the full name; the server checks `CHEATS` and friends by full name.
  Userinfo keys stay the short names (Source's userinfo keys, which the
  simulation reads: `cl_autowepswitch`).

## Steps

1. Core (`chimera_core::console`): full-name registry, resolution, scope,
   aliases, old names, qualification, tables by full name. Tests.
2. Registrations: `chimera:` and `combat_arms:` names at the call sites;
   `GameInfo::console_namespaces`; the scope following the identity.
3. Client console: completion and help by display name; binds.
4. Network: full names on the wire; NetSim tests.
5. Docs (DESIGN, OBSERVABILITY, README/AGENTS names), docs_index.

## Later

- A name Chimera implements for several Source games (HL2:DM's
  `sv_gravity`) is CS:S's today; when HL2:DM arrives, decide whether it
  gets an alias in `hl2dm:` too.
