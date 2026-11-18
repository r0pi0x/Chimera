# The first mashup app

`apps/mashup_demo` (crate and binary `mashup_demo`): Counter-Strike:
Source's maps played as Combat Arms, the engine's premise (docs/DESIGN.md,
"Goal") shown end to end. The app composes `cs_source` (its map loader
only) and `combat_arms` (its whole simulation and front end) as libraries;
neither game knows the other (checks/tests/it/architecture.rs).

## Done (2026-10-19)

- Chimera's `map::sources` (chimera/world/src/map/sources.rs): an app's
  maps as `MapSources`, each game's `MapLoader` (`game:name` to
  `MapData`), the gameplay's `MapDressing` (its parts on any map) and
  `SpawnTeams` (another game's spawn teams as the gameplay's).
  `MapData::kit` (a `GameKit`) holds what a map's game brings to every
  map (bodies, held and view models, HUD, radio, bot names, navigation,
  round sounds, muzzle flash, shells, decals); a mashup replaces it
  (`GameKit::stay`; was `take_game_parts`, X7), keeping the kept guns
  and the impact decals (the map's game's impact looks stay with it).
- The games' sides: `cs_source::game::MAP_LOADER` (the CS:S app's
  `games::load_map_level` uses it too), `combat_arms::game::world::
  MAP_LOADER` and `DRESSING` (`world::dress`: effect looks, view models,
  character bodies; `effects::library::add_looks` split from
  `add_to_map`).
- Scale: nothing to rescale. `MapData` is metres for every game (each
  loader applies its own unit: Source's 0.0254 m inch, Combat Arms'
  `meters_per_unit`), entity space keeps its own (`MapData::
  entity_scale`), and a gameplay converts its own numbers with its own
  unit. The heavy test checks the soldier's eye height on dust2.
- The shell's map change runs at the gameplay's tick (the default game's),
  not the map's game's (chimera/client/src/client/console.rs).
- The app: de_dust2 by default, three bots (`MaxPlayer`), Team Death
  Match, T spawns Alpha's and CT spawns Bravo's, Combat Arms' HUD; CS:S
  registered as a second game for its install. Bots navigate areas
  generated from dust2's collision (dust2's own `.nav` is CS:S's part and
  is taken off).
- Tests: unit tests in `map::sources` and the app; `heavy::dust2` (both
  installs): the map dressed, a soldier on the floor at Combat Arms'
  size, walking, firing the M4A1, bots fighting and respawning.

## Done (2026-10-20)

- Source's map logic in the mashup: `mashup_demo::MashupPartsPlugin`
  adds `source_logic`'s `LogicPlugin` (doors, buttons, breakables,
  triggers, movers) to Combat Arms' simulation. The logic already spoke
  Chimera's vocabulary (characters by `Intent`/`MovementState`, `+use`
  as `Intent::use_key`, `core::Damage` on `Damageable` nodes); the gap
  was Combat Arms' rounds, which now deal their damage to anything
  `Damageable` that isn't a character (`combat_arms::game::weapons`).
- Surfaces: Chimera's `map::sources::SurfaceTable` (the app's data
  file per game pair, apps/mashup_demo/data/surfaces_cs_source_combat_arms.toml:
  by name, by Source game material letter, a default) gives the map's
  surface names as the gameplay's (`MapData::surface_names`, spawned
  as `map::SurfaceNames`); the map keeps its own names, so its game's
  parts (CS:S's guns' penetration) still read them. Combat Arms reads
  the translation where it reads surfaces (`effects::Ground`,
  `surface_type`: impacts, footsteps, shells, grenades; penetration,
  which now also reads the world's surface type where a wall has no
  `PropSurface`, and a brush entity's `map::BrushSurface`, so a Combat
  Arms round passes CS:S's glass windows and breakables).
- CS:S's guns per slot: `cs_source::game::weapons::CsGunsPlugin` (the
  data, its hooks, accuracy and recoil, penetration materials) split
  from `CsWeaponsPlugin` (which adds the shop, rules, effects and the
  rest on it); the app adds it and `loadout_slot <slot> <weapon>`
  (`+loadout_slot primary cs_source:weapon_ak47`) puts either game's
  weapon in a loadout slot; `MapSources::kept` keeps CS:S's weapons'
  held and view models on its maps.
- Units: `GameInfo::meters_per_unit` (the game's world unit) makes the
  third-person camera's `cam_idealdist` the default game's units;
  sound distances are metres (`map::sound::distance_gain`/
  `distance_cutoff` take metres; Source's levels convert to inches
  inside, LithTech's radii are metres; the room send and near pan are
  metric); shadows' sizes are Source's renderer's, named as such
  (`SOURCE_UNIT`), metres for any game's characters.
- Tests: `heavy::cs_maps` (both installs): a Combat Arms soldier opens
  de_nuke's doors with +use and breaks a metal vent with the M4A1;
  de_dust2's sand, wood and concrete answer Combat Arms' types, and a
  round into the sand plays Combat Arms' sand impact; a soldier with
  `loadout_slot primary cs_source:weapon_ak47` fires the AK-47 into
  another on dust2. Unit tests: `map::sources` (tables, kept models),
  the app (the table's names are Combat Arms types, `set_slot`).

## Left

- Map logic: the logic maps teams by Source's numbering (gameplay team
  + 1); a gameplay whose team ids differ from the spawn mapping's needs
  `SpawnTeams` read back. Combat Arms' knife and grenades' own damage to
  breakables isn't checked on CS:S maps.
- CS:S's guns in the mashup: no impact effects (Combat Arms' effects
  read only its weapons' `CaWeaponFx`; CS:S's impact plugins would also
  answer Combat Arms' shots); its view model is kept on the map but
  doesn't show (Combat Arms' view-model driver, `game::view_model`,
  plays Combat Arms' sequences; CS:S's `ViewAnimPlugin` isn't added);
  Combat Arms' HUD shows its name as CS:S's localization token
  (`Cstrike_WPNHUD_AK47`); held models on CS:S's skeleton (drawn
  on Combat Arms' bodies only where bones match), no view model on a
  Combat Arms map (only CS:S maps carry its models); no menu for the
  slots (the console command).
- Surfaces: the table is ours by eye; CS:S's names not listed fall to
  the game material letter.
- Network: no map offer of CS:S maps to clients (`map_file_bytes`), no
  dedicated mashup server.
- Other Source-inch constants in presentation (`render::water`,
  `dust`, `detail`, `emitters`, `beams`, the spectator's distances)
  are Source map features' or CS:S's spectator spec's own: left in
  inches.
- Menus: the app starts straight in game (Combat Arms has no front-end
  menus yet).
