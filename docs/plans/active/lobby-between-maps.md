# Plan: the lobby between maps, with everyone's load progress

Status: started 2026-11-17. Backlog: "Lobby between maps, with everyone's
load progress". Builds on the pre-match lobby (`chimera_net::prematch`)
and the host's lobby first (docs/performance.md, "A match's start and a
host's way to the lobby (2026-11-17)").

The user's decision: keep returning to the lobby after every match, so
the host can change settings and players can leave.

## What exists

- The pre-match lobby begins when a new match starts (`prematch::begin`
  on `Minigames::starts`). For a match on another map that is after the
  server's own map is in (`minigame_net::follow_map_change`), so between
  the match-end screen and that moment the clients see only their
  loading screens.
- Each client reports its join's status (`LobbyReport`: connecting,
  downloading with bytes, loading, ready); a row's "loading" has no
  share.
- The next map's files are already fetched ahead during the match-end
  screen (`net::assets::map_ahead`, `MapAhead`), but no lobby row shows
  it.
- The loading screen's bar: the loader's own reports while the map
  loads on its task (`map::loading::LoadReporter`), then nothing for the
  spawn and the settling. The Signal loading screen's "Loading" stage is
  striped (no share) all the way through.

## Steps

1. **Real loading stages.** A load is four stages, in order
   (`chimera_world::map::loading::LoadStep`): **Download** (bytes: the
   join's or level change's download, or the next map fetched ahead),
   **Map** (the loader's task: `LoadReporter`'s fraction), **Models**
   (the stepped spawn: its start, each step of the drawer's parts, then
   its props, `PROPS_PER_STEP` at a time, out of `2 + props`), **Physics**
   (the held-back colliders inserted, out of all of them, then the
   settle hooks done (navigation mesh, spawn spots, shadows, the HUD's
   warm-up), out of all of them). The spawn and the settling write their
   counts into `LoadWork` (a resource); `chimera_net::net::load_stages`
   puts the four together with the download (`stages(world)`: each
   stage pending, running with its share, or done; only the stages this
   load has). The loading screens' bars read it; with one stage only the
   main bar shows.
2. **Every player's progress in the lobby.** `LobbyReport` and
   `LobbyRow` carry the stage (`step`) and its share in thousandths
   (`permille`). The rows show "Downloading 42%", "Loading map 30%",
   "Building models 70%", "Physics 90%", "Ready"; a listen host's row is
   its own load's. The local player's row is drawn from this machine's
   own stages each frame (live while the host's simulation waits on its
   spawn). **Network version:** `net66` to `net67`, in
   `chimera_net::net::NET_VERSION` only (the one place; the docs say
   which number and why).
3. **The lobby between maps.** With the lobby on, a match's change to
   another map (a rotation after the match-end screen, a vote's map, a
   host's `changelevel`) begins the lobby at once
   (`prematch::begin_between`): `NetLobby` names the coming map and mode
   and everyone's row loading it, while the map is fetched and loaded in
   the background on every machine (the server's `changelevel` the same
   tick; a client's fetch ahead already running from the match-end
   screen shows as its download). When the match starts there the same
   lobby goes on (no new one, no second log line) and counts its wait
   from then. A change that is called off ends it.
4. **Start waits for everyone or the host's timeout.** After a match the
   lobby counts down once every player is ready, or once the host's
   longest wait (`mp_lobby_max_wait`, default 90 s, now on the Host
   page as "Lobby waits at most") has passed since the match was ready
   on the server; the host can still start it early. The first lobby
   keeps waiting for the host's Start.

5. **Joining: the lobby first, the loading screen when the game is on
   (user, 2026-11-17).** The load's state (`load_stages(world)`, the
   download, `LoadWork`) is the one source. Every screen that shows
   loading reads it and owns none of it: the lobby's rows, the loading
   screen, a small bar.
   - A player joining a server in its lobby lands in the lobby, loading
     behind it, with its own row's progress. The lobby shows the map,
     the mode and the match's settings, so the player can leave before
     being put into a match they don't want.
   - When the host starts while the player is still loading, or the
     player joins a match already being played, they see the loading
     screen at once (same state, no restart of the load) and spawn when
     it's done.
   - Test: the one load drives both screens, and switching screens
     doesn't restart or duplicate it.
   - **Built (2026-11-17):** the lobby tells the match's settings
     (`NetLobby::settings`, **net68**: the mode's length, then each
     `prematch::LobbyFact` an app registers, the party's from its host
     settings' `lobby = true`: bots, balance, footsteps, rung extras,
     demote, knife finale, rotation, spawn protection; a value of "" says
     nothing and is left out); shown in the themed lobby's mode column
     and as a line in the plain one; a joiner gets the kit's Leave
     button on the action bar. One source: `load_stages::LoadSource` (a
     system parameter over `Inputs::from_parts`) is what the lobby's own
     row and the plain loading panel read; they no longer pick the
     load's resources apart themselves.

Each step has a scenario test: step 1 on `harness::Sim` with the
client's real stepped load (`chimera_client` `map_load_steps`), steps 2
to 4 on `NetSim` (`chimera_net` `prematch_lobby`).

## Progress

- 2026-11-17: plan.
- 2026-11-17: steps 1-4 built (`load_stages`, `LoadWork`, net67,
  `prematch::begin_between`, the Host page's "Lobby waits at most").
  Tests: `chimera_client` `map_load_steps::a_loads_stages_follow_its_work`;
  `chimera_net` `prematch_lobby::{each_players_load_stage_reaches_the_other_clients,
  after_a_match_the_lobby_shows_at_once_while_the_next_map_comes_in,
  the_lobby_after_a_match_waits_for_everyone_or_the_hosts_timeout}`,
  `load_stages` and `prematch` unit tests.
- 2026-11-17: step 5 built. Tests: `party_kit` `join_flow::{a_joiner_in_the_lobby_sees_it_loading_with_its_row_and_the_settings,
  the_host_starting_while_a_joiner_loads_shows_the_loading_screen_and_the_same_load_goes_on,
  a_late_joiner_into_a_running_match_gets_the_loading_screen_directly}`,
  `chimera_net` `prematch_lobby::the_lobby_tells_a_joiner_the_matchs_settings`,
  `chimera_client` `lobby_panel::a_joiners_lobby_tells_the_settings_and_has_leave`.
  Finding: the screens already switched correctly (the lobby over the
  loading panel while it is up, the loading screen when it goes); the
  tests pin it. Not done: the plain (unthemed) lobby has no Leave button
  (its Escape menu leaves).
- Not yet: loading the next map's data ahead during the match-end
  screen (the change starts its load at once, after it); a listen host's
  row seen by others holds still while its simulation waits on its
  spawn (a few seconds; its own screen reads it live).
