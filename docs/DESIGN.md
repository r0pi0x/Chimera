# Chimera: design

The design behind Chimera and the games built on it: why the layers,
slots, mounts and weapon model look the way they do, and the legal rules
the project keeps. This was the README's content before the README became
the user-facing page; the code map is [ARCHITECTURE.md](ARCHITECTURE.md).

## Goal

Pull a "microcosm" out of an existing game (a map, its movement feel, a few
weapons, its HUD) and make it playable in Lucker Party. Then mix parts freely:

- Combat Arms rules, playing as a Modern Warfare 2 character with MW2 guns.
- A Combat Arms character in a Counter-Strike: Source defusal map.
- A Minecraft character on a CS:S minigame map.
- Smaller swaps: a Minecraft crosshair in CS:S, MapleStory damage numbers in
  Combat Arms.

Element swaps double as Roll the Dice modifiers.

## Vision

Layers, each using only those below it:

```
apps            one faithful app per game (apps/cs_source: CS:S as CS:S),
                mashup apps that mix several games, Lucker Party the rich one
games           each game a library (games/cs_source, later combat_arms, ...):
                its parts (loaders, movement, weapons, rules, HUD, menus)
loaders         a family's formats and shared behaviour (source/: Source's
                files, its map logic)
Chimera         the engine (chimera/): game-agnostic slots, simulation,
                presentation, network, client shell
Bevy, Rust
```

- **Chimera** knows no game. A game fills its slots and hooks (movement,
  weapons, rules, HUD, replicated parts) with data and registered code.
- **Games as libraries.** A game crate is the faithful game's parts; its
  own app runs them as the original (parity first), and any other app can
  take single parts: a Combat Arms character on a CS:S map is an app
  picking two games' parts, never one game reaching into another.
- **Apps on top.** The faithful apps are the parity baselines; mashup apps
  mix games; Lucker Party adds the party (lobby, rotation, minigames as
  data). Later, users mash games up themselves: minigame definitions pick
  parts by id at run time, no rebuild.
- **Performance holds as games are added**: a game costs only what an app
  uses of it (crates and features), the simulation runs without the
  presentation (the dedicated server), and tools sit at the lowest layer
  whose knowledge they need.

docs/ARCHITECTURE.md ("Crates") has the crates; the plan is
docs/plans/active/engine-split.md.

## Scope for now

- First-person games only.
- First pair: Counter-Strike: Source, then Combat Arms. They are similar enough
  to test the shared core without fighting it, and different enough to expose
  CS-shaped assumptions.
- A third game (MW2 or Minecraft) is the next test of the interfaces.

Out of scope for now: 2D games such as MapleStory. Bringing 3D content into 2D
is workable (side-locked 3D or sprites rendered from 3D models); 2D into 3D
needs information the source does not have and is a gimmick at best.

## Engine direction

Leaning Bevy (Rust) for this prototype. The idea is essentially "an ECS with a
plugin per game", which is what Bevy is. Rust also has Source format parsers
(`vbsp`, `vmdl`, `vtf`, `vpk`). Known costs: Bevy API churn, immature UI, no
editor (Blender-as-editor tooling partly covers it). The editor matters less
here because imported content comes from importers, not hand placement.

## Architecture

### Mount layer

One adapter per game. It finds the player's install, reads its archives, and
converts assets into neutral data: mesh, skeleton, animation, texture,
material, sound, map, sprite, font, UI layout. Each adapter also owns the
game's units and coordinate conventions (a Source unit is about an inch; a
Minecraft block is a meter; some games are Z-up).

The engine only ever sees neutral data.

Mounts target the official, current release of each game, never a specific old
or modified build: users can't legitimately obtain old builds, and asking them
to undermines the "load from your own install" model. To survive game updates:

- Parse at the format level, not the build level. Formats change far less
  often than content.
- Read assets straight from the game's own archives at runtime and convert
  only what is actually loaded. Results that are expensive to produce
  (decrypting and parsing a world, BSP to meshes) go into a local cache.
- The cache is content-addressed: entries are keyed by a hash of the source
  file's bytes plus our converter version, so a game update only re-converts
  files whose bytes changed, and unchanged files are shared across builds.
  Each build has a small manifest (asset ID to hash); entries no current
  manifest references are deleted, and the cache has a size cap.
- A "mount doctor" per game checks expected files and reports what is mounted,
  what changed and what failed. The multiplayer pre-match check uses it too.

Supported sources are the Steam releases. An install is identified by its
Steam build ID plus file hashes. That identifies which build is installed
(the mount doctor warns on builds we haven't tested); it never names cache
folders, so an update doesn't duplicate anything.

Where imported data lives:

- Converted files are still the publisher's assets. They follow the same rule
  as the originals: never in git, never in a distributed build.
- The cache is per user, outside the repository:
  `~/.local/share/chimera/cache/` on Linux, `%LOCALAPPDATA%\chimera\cache\` on
  Windows.
- Development also has a `dump` tool that extracts an install's raw files into
  a local folder outside the repo, for exploring formats and writing specs.
  It shares the archive readers with the runtime mount; the game itself never
  needs a full dump.
- Moving assets between our own machines means copying the install or the
  cache directly (rsync, Syncthing), never through git.
- The game finds the CS:S install itself: Steam's library folders
  (`libraryfolders.vdf`, the app's manifest, which also gives the build ID
  for the mount doctor). If it can't, a first-run dialog asks for the
  folder and remembers it in the user's settings file (outside the
  repository); `chimera_install` in the console shows or changes it. A path
  in the gitignored `chimera.local.toml` (see `chimera.local.example.toml`)
  always wins; that file also holds the Combat Arms keys.
- Tests that need a real install skip when it isn't present.
- Our own original assets (greybox textures, our sounds, UI) may be committed,
  with Git LFS if they get large.

How code refers to assets:

- Game code uses stable namespaced IDs we own (`combat_arms:weapon/m4a1`),
  never archive paths.
- The importer builds the catalog mapping each ID to where the asset lives in
  this install, following the game's own data (a weapon's data file names its
  model and sounds) rather than hard-coded paths.
- When a release moves something, a committed per-fingerprint alias file
  records it; content hashes catch pure renames. File names and hashes are
  facts about the game and may be committed; contents may not.
- Importing writes a catalog snapshot (IDs, source paths, hashes; no content)
  that we commit, so a game update shows up as a reviewable diff.

### Slots

A match is a loadout that picks one implementation per slot. Each game
contributes implementations as a plugin.

| Slot | Contents | Examples |
|---|---|---|
| Map | Geometry, collision, lighting, spawns, objectives | de_dust2, a Combat Arms map |
| Ruleset | Rounds, win conditions, economy, respawns | CS defusal, CA elimination |
| Movement | Speeds, acceleration, jump, crouch, sprint, air control | Source, MW2 |
| Character body | Third-person model, skeleton, animations | CA soldier, MW2 operator, Steve |
| Weapon | Trigger/cost/delivery/effect plus feel | CS AK-47, MW2 M4 |
| Viewmodel | First-person hands and gun, reload/draw/fire animations | MW2 hands with M4 |
| HUD pieces | Crosshair, health, ammo, killfeed, damage numbers | CS crosshair, MapleStory numbers |
| Audio | Footsteps, gunshots, announcer, hit sounds | CA announcer |

Slots can get more granular as needed (the HUD row already is).

### Intent

Nothing reads the keyboard directly. Local input, bot brains and (later) the
network all write the same `Intent` component: move direction, look, jump,
crouch, sprint, fire, reload. The Movement slot turns intent into motion and
publishes movement state; weapons read intent plus that state. A bot is just
another intent source, so it uses exactly the same movement and weapons as a
player, and this matches the multiplayer model (clients submit intent).

### Interfaces

Each slot declares what it provides and what it needs, so any implementation
that satisfies the contract can be swapped in:

- A crosshair needs the current spread and aim state. Any weapon providing
  spread works with any crosshair.
- Movement provides state (sprinting, crouching, airborne, landing). Weapons
  read it, so "can't fire while sprinting" is a weapon rule, not a hard link
  between MW2 movement and MW2 guns.
- A character body maps its animations onto a standard action set (run,
  crouch, jump, aim, reload, die). That mapping is the retargeting work and
  probably the hardest single piece.
- A ruleset declares required capabilities (defusal needs bomb sites and a
  plant/defuse interaction). Loadouts are validated before a match starts;
  invalid combinations fail cleanly or use a declared fallback.

### Bundles

Some parts belong together: MW2's M4 model, reload animation, hands and fire
sound. Pick the bundle by default; allow overriding individual pieces for
chaos combinations.

### Weapons and items

"Weapon" is not a type. Any use action is four composable parts:

| Part | Question | Examples |
|---|---|---|
| Trigger | How is it activated? | primary/secondary, hold, charge, tap, toggle |
| Cost | What does it consume? | magazine and ammo type, energy, mana, durability, cooldown, nothing |
| Delivery | How does it reach things? | hitscan, projectile, beam, melee swing, area, self |
| Effect | What happens on hit? | damage, break block, knockback, stun, heal, ignite |

Examples: CS AK-47 is primary, magazine, hitscan with spread, damage. A
Minecraft pickaxe is primary, durability, melee swing, break block plus small
damage.

Each host declares which effects it supports. Unsupported parts do nothing or
go through a bridge (a pickaxe in CS:S deals melee damage; mining has no
blocks to act on unless a bridge maps it to doors and glass).

Feel that doesn't decompose (CS recoil patterns, MW2 aim-down-sights) lives in
per-game components that travel with the weapon.

Prior art: Minecraft's item data components; Unreal's Gameplay Ability System.

### Components and categorization

- Core vocabulary: Transform, Velocity, Health, Team, Hitbox, Inventory,
  Weapon parts. Small and normalized.
- Per-game components are namespaced (`cs_source::RecoilPattern`,
  `mw2::Perk`).
- Bridges translate between per-game and core components.
- Rule of three: a concept moves into the core only after two or three games
  need it. A slot interface is not proven until two games implement it.
- Things are described with tags, not a class hierarchy (an AWP is `weapon`,
  `ranged`, `hitscan`, `scoped`).
- Every imported item keeps a namespaced ID and its source path
  (`cs_source:weapons/awp` from `scripts/weapon_awp.txt`).
- The catalog is generated by importers, never hand-edited, so changing the
  categorization means changing importer code and regenerating.
- LLM help is used offline for bulk tagging and proposing normalized values;
  the rules are saved as importer code and spot-checked by a human.

### ECS style

Prefer, in order: components for facts about an entity, observers for
reactions to them (`On<Insert<C>>`, `On<Discard<C>>`), messages for
per-tick flows (hits, deaths, weapon events) read by normal systems,
events triggered at a known moment (`world.trigger`, so the observers
run at once, where a hook would have run: a round's or match's step,
`mode::ModeStep`; a round restart, `core::RoundRestart`), and systems with declared
queries gated by run conditions (`gameplay_is`, `hud_is`, `mode_is`;
never a new set, whose flattening reorders unordered systems) for
per-tick logic. A fact other machines must see the same way is put on
locally from replicated state, as `ladder::OnRung` is (see "Arriving
on a rung is a component" under "Gun game rules"). Data hooks stay for decisions the
runtime asks (`fn(&World, ..) -> answer`) and for what is keyed by an
id (weapon builds, movements, codecs). Whole-world access (`&mut World`,
exclusive systems, callback registries) only where the job is the
world: map loading, spawning, network apply and capture, codecs, the
console, menus, tools.

How a game registers its hooks: its plugin adds its systems to the
schedule's existing sets gated by `gameplay_is(<its id>)` (its HUD's by
`hud_is`, a mode's per-tick work by `mode::mode_is(<mode id>)`, chained
where the game runs the mode runtime), its observers filtered by its
own components (a thrown weapon's projectile: an observer of
`grenade::Launch` that skips weapons without the game's part; a
mode's reaction to a death: an observer of `mode::ModeDeath` that skips
other modes' ids; a minigame rules module's join and a life's arming:
observers of `minigame::Joined` and `minigame::Arming` that skip other
modules' names), and its data hooks under its own id
(`ModeAppExt::add_modes(game, ..)`, `WeaponDataAppExt::add_weapon_data`,
`ViewAnimAppExt::view_anim_rules(game, ..)`, a movement by id,
`NetAppExt::net_part`). It never inserts a single-slot hook resource
that another game's plugin would replace: a hook of a kind the engine
calls one of (`JoinRules`, `Holds`, `BotHooks`, `NewGame`,
`LogSubjects`) is registered under the game's id (`app.game_hook(game,
hooks)`, `gameplay::GameHooks`) and the engine calls the gameplay's
game's (`gameplay::active_hook`), so a gameplay switch switches them;
view-model rules are the held weapon's game's.
`checks/tests/it/ecs.rs` keeps the callback registries on an allowed
list and ratchets each crate's exclusive systems in per-tick schedules;
`checks/tests/it/determinism.rs` checks every simulation crate, a new
one from its first commit.

### Normalization and mixing rules

- The host game's rules win; imported parts adapt to the host.
- Shared quantities are normalized: health and damage as a fraction of a
  standard player's health, speed relative to the host's walk speed, fire rate
  in seconds.
- Cross-game questions (whose sprint rules apply, what an MW2 perk does to
  Steve) are design decisions, made explicit as bridges or as ruleset options.
  Many make good dice modifiers.
- Loudness stays each game's by default: a CS:S-identity player's steps
  fall off as CS:S's (linear, silent from 25.4 m), a Combat Arms one's as
  Combat Arms' (squared, silent from about 20 m, louder close by). The
  party's `party_even_footsteps 1` (the server's, replicated; default 0)
  gives every game's others' steps one profile, CS:S's level and its
  volume for the pace (`map::sound::EvenFootsteps`), so neither identity
  is heard more: CS:S's rather than a mean, because it is a specified,
  tuned curve and leaves the CS:S identity exactly as in CS:S.
- Cross-game kits (a CS:S character's moveset in another game) are generated
  ahead of time as data, reviewed and playtested, never generated during a
  match. A runtime "chaos kit" could exist later as a server-generated mode.

### Impacts across games

Any game's gun works on any game's map; where a game lacks something,
Chimera has a generic fallback (`chimera_world::map::impact`,
`chimera_sim::weapon::impact`):

- **Impacts.** Surfaces map onto common classes (`SurfaceClass`:
  concrete, metal, wood, dirt, sand, grass, snow, glass, tile, plastic,
  cloth, flesh, water), each game registering how its names go to and
  from them. A map lists whose impact looks it carries (its loader's game;
  a guest's dressing adds its own). An impact is drawn by the gun's game
  when its looks are on the map, else the map's game, else any game whose
  looks are there, else Chimera's generic mark and puff; each game's
  impact code asks this route and draws only its own.
- **Pushes.** Every game's bullets tell a common `BulletImpulse` (N·s at
  the hit point; each game converts its own numbers: CS:S's measured
  damage force, Combat Arms' damage × 2 N·s per hit point), which Chimera
  gives to dynamic bodies times `phys_pushscale`.
- **Recoil.** The view punch is shared, the rules per game; `RecoilGame`
  names the game of the held weapon, and a switch to another game's
  weapon zeroes the punch: the old game's kick doesn't keep returning
  into the new game's view.

### Hits across games

Any game's gun hits any game's body by one game-agnostic interface
(`chimera_core::core`, `chimera_sim::weapon::hits`); nothing is written
per gun-body pair. Plain CS:S and plain Combat Arms keep their exact
numbers (pinned by `parity_pin_*` tests in each game).

- **Regions.** `BodyRegion` is the superset of what the games tell apart
  (head, neck, chest, stomach, groin, upper arm, forearm, hand, thigh,
  shin, foot by side, generic); `hitgroup()` gives Source's coarser
  hitgroup that `Damage` and everything downstream carry, `part()` the
  part armour covers (head, torso, arm, leg, groin).
- **Hitboxes, from the body's game.** Each body fills `Hitboxes` in its
  own game's way, converted into Chimera's shapes (boxes or spheres with
  region, priority and the body's own damage scale): CS:S from the
  model's hitbox set (regions by bone), Combat Arms its skeleton's hit
  spheres posed every tick. Lag compensation keeps them
  (`weapon::lagcomp::HitHistory`). A gun's rule (CS:S's nearest box
  entered, Combat Arms' client test and server walk) asks only shape
  queries (`ray_span`, `segment_passes`/`power`, `line_passes`), so it
  works on any body. A rule that tests the character's box first
  (Combat Arms') widens it to hold the body's shapes
  (`Hitboxes::bounds`): a taller model's head poking out of the hull
  still counts, as in Source. A game that keeps posing its own skeleton
  under another identity's body (Combat Arms) hits that body's shapes
  with everything (rounds, knife, grenades), never its hidden skeleton.
- **Damage per region, from the gun's game.** A gun's table scales by
  region (CS:S's five hitgroup multipliers; Combat Arms' zone factor, the
  body's own scale where its shape has one, else its male skeleton's).
- **Armour: one value, coverage by identity.** Every character has at
  most one `weapon::Armor` (normalized points, 1.0 = 100). What covers
  it is its game's: the default kit (CS:S's kevlar: torso, arms and
  groin half, every damage class; the helmet the head), or a game's own
  pieces worn with it (`weapon::ArmorPieces`: Combat Arms' gear, its
  protection fractions per damage class, used up when consumable). Every
  gun's hit splits between armour and health through one step,
  `hits::absorb`: P, the part's coverage for the damage class, lets
  `(1 - 2P) + P x ratio` through (the gun's armour ratio: CS:S's
  `WeaponArmorRatio`, Combat Arms' guns 1), armour loses the rest times
  the pieces' wear, and running out is the armour's rule (CS:S by what
  it would lose, Combat Arms by what it blocks). So a Combat Arms round
  on CS:S kevlar is halved, a CS:S AK-47 on Combat Arms' body gear loses
  0.15 x 0.45 of itself.
- **Armour when the identity switches** (the party apps, where a
  character's game follows the held gun): the armour value stays and
  what covers it follows the identity. A game gives armour only to its
  own characters (Combat Arms its default 100 points at the first life;
  CS:S by buying), puts its pieces on a character that becomes its own
  and takes them off one that becomes another game's; a character
  without pieces is covered by the default kit. So a CS:S-gun player's
  armour is kevlar by CS:S's rules and shows on CS:S's HUD, a Combat
  Arms-gun player's the same points as Combat Arms' gear on its HUD,
  and nobody carries armour their HUD doesn't show. (Combat Arms puts on
  its default gear at a switch, not a loadout's.)

- **Spawn protection: see-through, shot through** (`core::SpawnProtected`,
  a minigame's: `protection = { damage, bullets, fire_ends }`, in shape
  Combat Arms' 4 s and 3 s; the party gun games keep one 2 s for both,
  ended by the player's first attack: user, 2026-10-29): no damage lands
  in the damage window (`core::God`); in the bullets window the player
  isn't there for attacks: rounds and swings pass through to what is
  behind (the wall's decal, the next player's hit), thrown projectiles fly
  through, blasts and flashes leave it out, in every game's delivery. A
  hit that still reaches it (the damage window alone) is told with no hit
  (`weapon::impact::hide_protected`): no game draws blood, a puff, a
  decal or an impact sound, nor a hit indicator, name-tag flash or the
  victim's reaction. The windows replicate on each client's clock
  (`chimera_net::minigame_net::NetProtected`), so the server and every
  client agree, and on every machine the protected body draws half
  see-through (`chimera_present`'s `render::tint::fade_protected`),
  fading back over 0.2 s when the protection ends: unlike Combat Arms,
  whose protection has no look, the party shows that shots won't land.

### Announcers

Kill-streak and special-kill callouts (Combat Arms' DOUBLE KILL ...
UNBELIEVABLE+, FIRST KILL, HEADSHOT, NUTSHOT, REVENGE) are Chimera's, not
a game's HUD's (specs/combat_arms/kill_streaks.md: Part A the rules,
Part B Combat Arms' pack).

- **Counting is the server's, for any gun on any body**
  (`chimera_sim::announce`): from every enemy kill, the killer's
  multi-kill count (the pack's window), the round's first kill, the
  killing hit's place (head; groin, from the gun's or knife's full
  `BodyRegion`, `core::RegionHit`, since `Damage`'s hitgroup reads a groin as the
  stomach), revenge and the killer's body (`core::FemaleBodies`). These
  `KillFacts` travel with the kill (`net::Killed`); every client decides
  what to show from them: one event per kill by priority (first kill,
  revenge, then at count 1 a headshot or groin shot, else the step), at
  most one voice (the event's, else headshot, else groin by the killer's
  body), the skull or nut over a step, first kill or revenge. Which
  priority levels, art and voices is the pack's data. A kill with none of
  those is a **plain kill** when the pack has a `plain_kill` section:
  "KILL" / "KILLS" (by the killer's match kill total, `KillFacts::total`,
  counted by the server for the map) with the total in per-digit images,
  level 1, no top image; a pack without the section announces none.
- **An announcer pack is data** (`chimera_core::announcer::AnnouncerPack`,
  a TOML document): step names and thresholds, art and voice files (paths
  in the pack's game's install, loaded at run time), layout on its
  reference screen, the banner animation's timings, priority levels,
  the plain-kill banner and its digit slots, weapon skins. A game contributes its pack as one of its parts
  (`GameParts::announcer`; Combat Arms' is `data/announcer.toml`).
- **The shell draws it** (`chimera_client::client::announcer`), above
  whichever HUD is up, so in the party apps a CS:S-identity player gets
  the same banner and voice as a Combat Arms one.
- **The server picks the pack**: `sv_announcer` (replicated) is
  `default` (the first registered: Combat Arms' wherever Combat Arms is
  composed, so the party apps and plain Combat Arms; none in plain CS:S,
  which has no pack), `none`, `random` (one per map) or a pack's id. More
  packs (a Quake-style one) are more data files; a minigame could set the
  cvar.
- **A minigame's moments are callouts too**: the gun game's rung up or
  down, the last rung (yours, and another player's to everyone), the
  lead taken, lost or tied, and the win are `MinigameEvent`s the server
  sends; each client's minigame HUD data turns the local player's into a
  `Callout` (`chimera_sim::minigame::hud`; text from the rules module,
  `RulesModule::callout`), which the shell draws centre screen
  (`chimera_client::client::callouts`) with a cue's sound, unless the
  HUD that is up draws it in its own looks (`GameDraws::callout`:
  Combat Arms' does, in its notices' place): one callout whichever HUD
  the gun has swapped in. The
  sounds are a **cue pack** (`chimera_core::cues::CuePack`, TOML: cue
  key to a file in a game's install, loaded at run time), a game's part
  (`GameParts::cues`; CS:S's is stock sounds, data/cues.toml; Combat
  Arms' its Arms Race interface sounds for a rung up and the last rung);
  a definition picks one (`cues = "<id>"`, `none`; default: the pack of
  the game whose HUD is up, which follows the gun in a gun game, else the
  first registered). The match-end screen's sound is the HUD's game's
  too: its `match_won_sound` for the winner, and for the others its
  `match_lost_sound` where it has one (Combat Arms' result screen's win
  and lose), else the won sound; a draw plays none. Plain CS:S and
  Combat Arms play no minigame, so neither changes.
- **A minigame's music is one game's director's**: the match's moments
  for the local player (`chimera_sim::minigame::moments::
  MinigameMoments`: the phase, alive or not, players on the last rung,
  the respawn countdown's seconds, the result), on every machine, are
  read by the director of the game the definition's `music` names
  (default: the first game whose parts lend one, `GameParts::music`:
  Combat Arms'; CS:S has none). Combat Arms' plays Arms Race's cues for
  them (specs/combat_arms/music_ambience.md Section 10: the loading
  track, the round start's sound and start track at the start of play
  if alive, the last stage's music while anyone is on the last rung, the
  respawn countdown's ticks, silence on the match-end screen); the
  others stay silent. Sounds a callout plays (rung up, last rung) are
  the cue pack's, not the director's, so nothing plays twice. The party's
  music is not the original's loop (it gets stale): by default (cvar
  `party_music 1`, data/music.toml `[party]`) the start track plays once
  and a shuffled playlist of Combat Arms' menu tracks follows, each once
  before any repeats, cross-fading; the last stage takes over and the
  playlist resumes after it. `party_music 2` is the original's loop, 0
  none; Combat Arms' own modes always keep the original.

### Gun game rules

The party's gun game (`chimera:ladder`, `chimera_sim::minigame::ladder`)
follows the common GunGame server conventions, each a setting of the
definition's `[rules]` (no spec covers them: CS:S has no gun game, and
Combat Arms' Arms Race, which is plain Combat Arms' own rules, climbs by
kill count with no cost for a suicide, game_modes.md M14, and keeps
that):

- **A kill a rung up; a knife kill steals one** (`demote_on =
  "melee_death"`): the knifed player goes down, the knifer still climbs.
- **One's own death costs a rung** (`demote_on_suicide`, default on):
  a suicide (one's own grenade or blast, `kill`) or a death with no
  killer (a fall, the world), never below the first; so `kill` is no
  free respawn. Called "Suicide! Level down: <gun>"; it is nobody's kill,
  so the announcer's streaks ignore it.
- **A late joiner starts near the others** (`join_rung`, default
  `lowest`; `average`, rounded down; `first`), never on the last rung.
- **A melee finale** (`knife_finale`; off in the module, on in both gun
  games, Lucker Party's `gun_game_dust2` and Ultimate Gun Game's
  automatic definition; Host settings turn it off): after the guns the
  ladder ends on melee rungs where the player holds only a melee weapon.
  How many (`melee_rungs`, default `"auto"`, or a number): the power
  curve's melee band (below), 7% of the whole ladder, at least 1: 1 for
  10 or 20 guns, 2 for 30, so a longer game ends longer. Each
  melee rung draws its own weapon, from the ladder's seed, from the
  knives and melee weapons of the games in play (CS:S's knife, Combat
  Arms' knife-class weapons; never a character's hands), none twice
  while there are enough; `knife_finale = "<id>"` names the last rung's.
  The infected's hands (a game's rung character, at its odds) may take a
  melee rung's place, at most once a ladder, and the last rung is melee
  either way; the last melee rung's kill wins. The draw is the host's and
  replicated with the match.
- **A natural power curve** (user, 2026-10-29/30; `minigame::power`,
  chimera/sim/data/power_curve.toml): a drawn ladder climbs in bands,
  warm-up (pistols), rising (SMGs, shotguns), peak (rifles, carbines),
  heavy (machine guns, marksman rifles, snipers, heavy pistols: heavier,
  not harder), specials (launchers, flamethrowers), a grenade, melee
  last. Each gun class maps to a band by data (a weapon's `class`; a
  pistol hitting 50 or more a body hit is a heavy pistol). Each band has
  a share of the ladder (15/20/28/15/8/7/7%): rung i of N sits at
  (i + 0.5) / N and takes the band covering that point (a point on a
  boundary the band above), every ladder moving the boundaries up to 3%
  (seeded), the first rung always warm-up and the last melee. A band's
  guns are drawn at random from its pool, the games mixed, then sorted by
  a **power score**: how fast the gun kills a baseline player (100
  health, no armour, the same for all) at 10, 15 and 20 m, from its data
  (damage per pellet with falloff, pellets, fire interval, pre-fire
  latency, spread as a hit rate on a 0.3 m target, a small headshot share
  and magazine term; each game's readers say what its hooks' numbers
  mean). The score orders guns only inside a band; the bands are
  categories, so a strong SMG never comes after a weak rifle.
  `ladder_power` prints every gun's score and band. A band without guns
  gives its rungs to the nearest band with guns to spare. Ultimate Gun
  Game's ladder is drawn this way; a hand-written definition keeps its
  ladder unless it opts in (`ladder = "auto"`, `rungs`, `power_curve` to
  tune it), drawn on the host at each match's start and replicated.
- **Rung extras, the same for everyone on a rung** (`rung_extras`,
  default on): when a ladder is made (each map, each new ladder) a game
  may draw something for each of its rungs from the match's seeded
  generator (`ladder::RungExtrasHook`, registered by the game; Chimera's
  ladder names no game): kept in the ladder's rung data and replicated
  with the match, so a late joiner, a respawn and a network client get
  the same; Chimera itself swaps a drawn melee weapon in for the kit's
  and shows a label after the rung's gun. Combat Arms' rungs
  (`combat_arms::game::rungs`) each draw a fitted set from the gun's
  accepted parts (per slot a part or the stock one), a character for
  everyone on the rung from every player body the install has
  (specialists and mercenaries too; without the install the stock six;
  a body no stock group has loads when first worn), and a melee weapon
  of its own (knives, the shovel, the machete, ...); the melee finale
  rungs keep their knife unless a game makes it its character's (below). CS:S's rungs (`cs_source::game::rungs`) each draw one
  of its eight player models (four T, four CT; a `PlayerClass`, which a
  free-for-all player's body follows too) for everyone on the rung, named
  in the rung's HUD line. Each part turns off by key
  (`rung_extras = { attachments = false, character = false, melee =
  false, infected = false }`), all with `rung_extras = false`. The apps
  seed them anew at each launch (Ultimate Gun Game with its launch seed).
- **A rung's character changes looks, not the odds** (user,
  2026-10-29): a drawn Combat Arms body keeps only what is intrinsic to
  it, its body type (`FigureType`: sprint, stamina, the variable terms);
  armour, protection, speed penalty, sprint and stamina bonuses, hit
  points and animation rates are a Combat Arms vest's (the body record
  is the vest, gear_perks.md 3), and on a rung every body wears the
  baseline vest, model 0's Normal, what the party's players wear without
  gear (`gear::RUNG_VEST`). So Combat Arms players are no harder to kill
  than CS:S ones whatever body a rung draws. The install's two body types
  have the same numbers, so every rung body plays as the stock one; a
  body type of its own would apply.
- **A special character tied to its weapon** (user, 2026-10-29):
  a game's draw may make a rung a character's (`ladder::RungCharacter`):
  on it everyone holds the character's weapon alone (no gun, no kit),
  and the next rung, up or down, makes them themselves again. Combat
  Arms' is the infected (Quarantine's zombies, their hands: the
  install's knife-class weapons with the infected leg set), one choice
  in the melee pool: only a rung of the ladder's melee band (its last
  rungs whose weapon is a melee weapon, today the knife finale's,
  whichever game's knife) can be theirs, at Combat Arms' data odds
  (data/rung_melee.toml, 0.25; `rung_extras = { infected = { odds =
  0.5 } }`, 0 or `infected = false`: never), else it keeps its knife
  (a rung whose own weapon is a hand is theirs too); an infected body
  and face, the infection's sound and change effect. Without the knife
  finale there is no melee rung, so no infected. Not copied: the
  infected's large health and their slow-down and knock-back from
  bullets; they keep the gun game's baseline health and movement, so it
  stays fair.
- **Arriving on a rung is a component, games react with observers**
  (user, 2026-10-29): arming puts `ladder::OnRung { index, extras, from }`
  on the player, replacing it only when the rung or its extras change
  (never for a respawn on the same rung); games react to every rung with
  observers, `On<Insert<OnRung>>` (arrived) and `On<Discard<OnRung>>`
  (left, the old value readable), not a callback registry. It isn't
  replicated: a network client puts the same on from the replicated
  `Rung` and the server's rung extras (`ladder::follow_rungs`), so its
  observers fire there too and nothing fires twice. `from` (none: the
  first rung this machine knew the player on) tells an arrival from a
  sighting: Combat Arms' infection moment plays for arrivals only. What
  a rung puts on a player that replicates anyway (a look, fitted parts)
  stays the host's `RungExtrasHook::apply`.

### Automatic gun games (variety)

Ultimate Gun Game's ladder and maps are made, not written
(`chimera_sim::minigame::auto`, docs/plans/active/ultimate-gun-game.md):

- **Every gun a game has**: a game lends its claimed guns
  (`GameParts::guns`) plus every weapon of its own whose data names a
  class (`class = "rifle"` in the weapon TOML, docs/weapon-data.md), so
  an install's extra weapons (Combat Arms' copies of our documents)
  join without a hand list. A ladder draws from all of them on the power
  curve (above: bands by class, shares of the ladder, each band's guns
  by power score; each game at least once, a game's guns weighted by one
  over the square root of its count); its grenade band holds the games'
  grenades (class `grenade`: thrown and lethal, CS:S's HE, Combat Arms'
  M67), before the melee band; the ladder gives a thrown rung grenade
  back (`chimera:ladder`'s `refill_ammo`).
- **A fresh seed every launch, a new ladder every match**: the apps take
  a random seed at launch and say it; each rotation makes the next
  match's ladder from the next seed (`NextMatchHooks`); `ugg_new <seed>`
  replays one. Tests keep a fixed seed.
- **A random map, not a recent one**: a definition may rotate at random
  (`rotation = "random"`, `avoid_recent`): the host (never a client)
  keeps the maps played for its lifetime (`minigame::rotation::
  MapHistory`) and skips the last min(10, candidates - 1). The pool is
  every game's claimed maps plus the installed ones its
  `GameParts::map_patterns` match (CS:S's `de_*`, `cs_*`). Lucker Party's
  hand-made minigames keep the default, in turn.

### One HUD layer over several games' HUDs

In the party apps the HUD follows the gun in hand, so anything that
must survive a swap or look the same on every HUD is the shell's, not a
game's:

- **The kill feed is Chimera's** (`chimera_client::client::kill_feed`):
  its entries (killer, victim, weapon id, headshot, suicide, team kill,
  time) come from `core::Died` on host and clients alike (a client's
  from `net::Killed`), held by no HUD, so a kill just before a rung swap
  is still listed after it. The shell draws them in one place (top
  right), each gun with its own game's icon (`weapon_icons::
  WeaponIcons`, which games fill with their art: icons and kill marks)
  or its name. An app asks for it with `SharedKillFeed`, and the games'
  own feeds stay empty while it is in; plain CS:S and Combat Arms keep
  theirs.
- **Radars are an app setting** (`client::hud::Radars`, `hud_radar`):
  one game's radar over another game's map matches neither style and
  many maps have no overview, so the party apps start it at 0 and
  `hud_radar 1` brings both games' radars back.
- **The hit marker is an opt-in setting** (`client::hit_marker`,
  `cl_hitmarker`, default 0): CS:S has no hit feedback and Combat Arms
  has its own indicator, so parity is none for CS:S guns. On, the shell
  draws a cross on the shooter's own client for damage actually dealt
  (headshot and kill variants), on every HUD but one that draws its own
  (`GameDraws::hit_marker`), so a gun game's CS:S rungs answer like
  Combat Arms ones.
- **The swap effect is an opt-in setting** (`client::swap_effect`,
  `party_swap_effect`, default 0): neither game has a swap, so a rung's
  change of game is a hard cut by default. On, the local player's screen
  flashes and fades over 0.3 s while the new gun draws, and another
  player's body gets a brief puff, generic, no game's look.

### The party apps' Escape menu

Ultimate Gun Game and Lucker Party have a menu of their own
(`party_kit::esc_menu`), not one game's: Combat Arms' in-game menu
yields Esc to it (`chimera_client::client::front_end::AppMenu`), and it
draws above everything but the console, the match-end screen and the
Tab scoreboard included. While it is open the game gets no input (the
shell sees a menu open; keys pressed are captured until let go). Its
pages are data (apps/party_kit/data/options.toml, host_settings.toml):

- **Options** set the player's own cvars (video, audio, mouse, the
  party's opt-in settings), saved in the config; "Binds" is the
  Controls page.
- **Host settings**, for the host only (a listen server's player; RCON
  admins later): existing server cvars that apply at once (bots,
  `party_even_footsteps`, votes allowed) and host overrides of every
  minigame's definition (`chimera_sim::minigame::overrides`: rung extras
  by part, demote on suicide, the knife finale, the map rotation,
  spawn protection), which apply from the next match, as the page says.
- **Vote to skip the map** is one of the engine's votes (`net::vote`,
  issue `skipmap`): server-decided, a majority of the people (bots never
  vote), 20 s, one at a time, a wait per caller; passed, the server
  goes to the rotation's next map. Everyone sees a small vote panel
  (F1 yes, F2 no).
- **Disconnect** (apps with the main menu; its entries are data,
  data/main_menu.toml `[[escape]]`): back to the main menu. A host with
  people connected is asked first ("End the game for 2 players?"); its
  clients land on their menu with "The host left the game."
  (`net::client::HOST_LEFT`).
- No scoreboard entry (Tab).

### The party apps' main menu

Ultimate Gun Game starts at a menu (`party_kit::main_menu`; Lucker Party,
shelved, still starts in its minigame) unless the command line plays
(`--map`, `+minigame`, `+connect`, `--screenshot` and the other tool
runs skip it, so scripts and tests are unchanged). Its first iteration
copies CS:S's main menu (user, 2026-10-31): the same VGUI widgets
(`chimera_client::client::widgets`) in the GameUI look of CS:S's install,
the entries at the bottom left under the app's name in text (no logo),
each page a movable frame; the code is the party's, not CS:S's game
menu, which is that game's front end (its Create Server, options and
server browser are CS:S's). Entries and the Play page's rows are data
(data/main_menu.toml).

- **Play** hosts: the rows' cvars (map or the minigame's rotation, bots,
  max. players, server name, password, port, UPnP), then the map, a
  listen server when `maxplayers` > 1 (joinable by default), the app's
  minigame once the map is in. The Game tab is the host settings'
  next-match rows.
- **Join** is direct connect (address, password, recent addresses); the
  LAN browser is M2.
- **Options**: the Escape menu's tabs and Binds.
- **Behind it** a live map (`[background]`) with a slow camera orbit;
  the HUD is hidden by drawing the menu's UI on a camera of its own
  while the shell's UI camera (the HUD's) is off.
- **Loading**: CS:S's loading dialog's parts (stage, bar, Cancel), for
  joins, Play's load and a map change in a game; Combat Arms' own
  loading screen instead for Combat Arms' maps (user decision 5).
- **Failures** (a refused join, a drop, a failed load) land on the menu
  in CS:S's "Disconnected" box with why.
- The shell tells front ends what happens by triggered events
  (`front_end::EnteredGame`, `LeftGame`, `Joining`, `JoinFailed`,
  `MapLoadFailed`, `StartGame`), which CS:S's game menu, Combat Arms'
  loading screen and this menu each observe.

## Multiplayer

Same principles as the main game: the server owns the loadout, rules and
outcomes; clients submit intent. Every player must have each game the loadout
uses mounted; the server checks this before a match. Plan:
[docs/plans/active/multiplayer.md](plans/active/multiplayer.md).

## Assets, code and legal

- Never ship other games' assets. Load them at runtime from the player's own
  install (the Garry's Mod model). Check each publisher's terms before a
  public release, especially Nexon/Valofe titles.
- Reverse engineering formats for interoperability is generally lawful; check
  each game's EULA.
- Facts and ideas (file layouts, values, formulas, how a mechanic behaves) are
  free to reuse. The specific code expressing them is not, and a line-by-line
  translation into Rust is still a copy.
- Workflow: a spec session works from reference material and writes down
  what the game does as a plain spec in our own words (formats, values,
  rules, edge cases); a separate session implements from the spec in this
  project's ECS architecture rather than mirroring the original's structure.
- Never commit third-party code, assets, extracted files or keys. The goal
  is a faithful recreation built without distributing anything we don't
  own; players load assets from their own install.
- Two ways to get assets (user decision, 2026-10-29). By default every
  player loads from their own install. A server may instead name a
  private asset service, behind a CDN and run by whoever hosts the
  server, that serves assets in a packed form only after checking,
  through the player's Steam account, that they own the game. The
  project never ships that service or any assets. We keep an internal
  instance for testing. Plan in the backlog's roadmap.

### Per-game notes

- **Counter-Strike: Source.** Steam app 240, Linux build. Tested build
  25338318: `de_dust2.bsp` is BSP version 20 (loose in `cstrike/maps/`); VPK
  directories are version 2 (`cstrike/cstrike_pak_dir.vpk`, and
  `hl2/hl2_textures_dir.vpk`, `hl2/hl2_misc_dir.vpk` for shared content).
  Source 1 formats (VPK, BSP, MDL/VVD/VTX,
  VTF/VMT) are well documented. Valve's public Source SDK 2013 includes the
  shared movement code. Weapon stats live in `weapon_*.txt` scripts. Faithful
  rendering needs BSP lightmaps and Source material rules.
- **Combat Arms.** LithTech Jupiter (`.rez` archives, LithTech model and
  world formats). Specs and the `games/combat_arms` library are in the
  repository since 2026-10-18 (clean room, no keys or assets); the plan
  is [plans/active/combat-arms.md](plans/active/combat-arms.md).

## Specs

Behavior worked out from reference material reaches this repository only as
specs in `specs/`, written in our own words: constants with units, formulas
as math, per-tick order of operations, edge cases, deliberate quirks, and
test cases with expected numbers. Specs are written in sessions that read the
reference material; implementation happens in separate sessions that see only
the specs. The test cases become automated tests. See
[specs/README.md](../specs/README.md).

## First steps (the original plan, 2026-10; kept for the record)

MVP: a Combat Arms slice. Run, sprint, shoot and fight bots on one map, built
through the slots so another game's controller is a loadout change. Gameplay
never waits on asset formats.

1. ~~Create a Bevy project in this repository.~~
2. Foundation: core components, `Intent`, loadout resource with one plugin per
   slot, a greybox map built in code, collision queries, debug tools (inspector,
   free-fly camera), placeholder movement.
3. Combat Arms movement from its spec, as a kinematic character controller.
4. One Combat Arms weapon from its spec as trigger/cost/delivery/effect parts,
   plus hitscan, hit zones, health, crosshair and ammo HUD.
5. Bots (intent from a brain on a waypoint graph, navmesh later) and a simple
   deathmatch ruleset with respawns.
6. Real maps, in parallel with 2-5: `de_dust2` from a local CS:S install (BSP
   geometry, VPK textures, lightmaps), then a Combat Arms map once `.rez` and
   the world format are specced.
7. Prove the swap: Source movement (specced from Source SDK 2013) as a second
   Movement implementation, switchable live against Combat Arms movement on
   the same map. MW2 follows the same path.

## Open questions

- Which third game tests the interfaces best: MW2 or Minecraft?
- How do players pick loadouts: fixed per minigame, chosen in a lobby, rolled
  by dice, or a mix?
