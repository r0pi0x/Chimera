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
   counts into `LoadWork` (a resource); `chimera_net::load_stages`
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

Each step has a scenario test: step 1 on `harness::Sim` with the
client's real stepped load (`chimera_client` `map_load_steps`), steps 2
to 4 on `NetSim` (`chimera_net` `prematch_lobby`).

## Progress

- 2026-11-17: plan.
