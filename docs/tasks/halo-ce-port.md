# Halo: Combat Evolved into Chimera and Ultimate Gun Game (long-running brief)

A brief for a long, mostly unattended orchestration session (an orchestrator
model that spawns its own sub-agents). It runs on the Windows PC's checkout
(`git clone https://github.com/r0pi0x/Chimera`), works in milestones from a
playable slice to a comprehensive port, and keeps going until the milestones
run out or the user stops it. Paste everything below the line as the
session's first prompt.

---

You are the orchestrator for porting **Halo: Combat Evolved (original Xbox)**
into **Chimera**, the engine of this repository, as a new game that plugs
into **Ultimate Gun Game** (UGG) the way Counter-Strike: Source, Combat Arms
and Half-Life 2: Deathmatch already do. You run for a long time with little
supervision: plan, delegate to sub-agents, verify, merge, push, and move on
to the next milestone. Work until every milestone below is done or the user
stops you; when one milestone lands, start the next without waiting.

## Read first (all of it, before planning)

- `CLAUDE.md` and `AGENTS.md` (the contributor contract and hard rules),
  `docs/ARCHITECTURE.md`, `docs/DESIGN.md`, `docs/new-game.md` (the checklist
  for porting a new game and the crate's shape), `docs/OBSERVABILITY.md`,
  `specs/README.md` (what a spec is), `docs/assets-for-developers.md`,
  `docs/playtest-hosting.md`, and `docs/plans/active/` (in particular the UGG
  UI plan and the platform plan, so you don't collide with them).
- How the last port was done: `games/hl2dm/` and `specs/hl2dm/` (the most
  recent and smallest), plus `specs/combat_arms/` for a non-Source engine.

## The two hard rules this port lives or dies by

1. **Clean room.** The reference implementation is **OpenCE**
   (https://github.com/OpenCommunityEdition/OpenCE), a **decompilation** of
   Halo CE licensed **AGPL-3.0**. Under CLAUDE.md's hard rules, **no code
   from it may enter this repository**, neither copied nor translated line by
   line, and nothing decompiled ever enters. Its behaviour reaches us **only
   as specs in our own words**:
   - **Spec agents** may read OpenCE (cloned *outside* the repository, e.g.
     `%USERPROFILE%\ref\OpenCE`) and the game's data, and write **only** under
     `specs/halo_ce/`: behaviour, rules, numbers, formulas and file/data
     layouts in prose and tables, with no code blocks derived from it. Short
     identifiers (tag class names, field names, file names) are fine.
   - **Implementation agents never open OpenCE or anything derived from it.**
     They read `specs/halo_ce/` and the repository only. Say so in every
     implementation brief, and never give an implementation agent a path to
     the reference.
   - Never put OpenCE source, snippets, or decoded/derived copies of it
     anywhere inside the repository or in a scratch folder implementation
     agents read. Spec agents read reference files in place, by absolute
     path.
   - The orchestrator (you) stays on the implementation side: read specs,
     not OpenCE. If you need a reference question answered, spawn a spec
     agent.
   - `cargo test` enforces the spec and third-party rules; its messages say
     how to fix things.
2. **Game data comes from the player's own copy.** Assets load at runtime
   from the user's own Halo CE Xbox disc image (`.iso`/`.xiso`), the way the
   other games load from the user's own installs. Use the disc image the user
   provides from **their own disc**; ask for its path once if it isn't
   given. Never commit maps, extracted files, tags or anything derived from
   the disc (`cargo test` checks this too). Converted or extracted data goes
   in the per-user cache, as for the other games.

## Working rules

- **Identity and history:** commits by `r0pi0x
  <339403489+r0pi0x@users.noreply.github.com>` with UTC dates (`TZ=UTC`);
  set that identity in this clone's local git config if it isn't. Enable
  the hooks once: `git config core.hooksPath .githooks`. **Never skip or
  bypass hooks** (no `--no-verify`, ever). No real names, personal emails,
  machine paths (`C:\Users\<name>`, `/home/<name>`) or IPs in any file,
  test fixtures included (build such strings with `concat!` if a test needs
  one).
- **Branches and pushing:** work on feature branches, merge each finished,
  tested milestone step into `main` and push (another session on a Linux box
  pushes to `main` too: `git pull --rebase` or merge `origin/main` before
  pushing; never force-push `main`). Push first, test after:
  `scripts\affected-tests.ps1` for local checks, and fix any failure on
  `main` at once.
- **Stay in your lane** to avoid conflicts with the other session:
  - new code goes in `games/halo_ce/` (the game crate) and a loader crate for
    Halo's formats under `source/` (or wherever `docs/new-game.md` says
    loaders live);
  - shared crates (`chimera/*`, `apps/party_kit`, `apps/ultimate_gun_game`)
    change only through small, generic extension points, the way
    `docs/new-game.md` describes ("lift into Chimera when a second game needs
    it; never copy or depend game-on-game");
  - don't restyle UI; the other session owns the UGG UI and the platform
    services.
- **Network version:** if a network message changes, bump `NET_VERSION`
  past whatever `main` has and update `docs/playtest-hosting.md`.
- **No releases:** never create GitHub releases or tags and never run
  GitHub Actions. The user cuts releases.
- **Never publish** to the asset service's live store; asset sets for Halo
  are prepared and documented, and the user publishes.
- **Verify** with tests, offscreen screenshots and the observability tools
  (`docs/OBSERVABILITY.md`) before calling anything done. New behaviour gets
  a scenario test. Judge performance by deterministic counts in seeded
  scenarios, not wall clock.
- **Keep docs true:** `//!` docs, `cargo run -q -p repo_checks --bin
  docs_index`, a plan file `docs/plans/active/halo-ce.md` kept current (the
  milestones below with their status), and backlog entries for what's left.
- **Delegation:** spawn sub-agents for spec work and for implementation;
  choose models per task (stronger models for spec sessions, debugging and
  seams; lighter ones for mechanical work). Keep about three building agents
  at once so the PC doesn't run out of memory. Give each a self-contained
  brief with these rules.
- **Report as you go:** after each milestone, write its summary (what landed,
  screenshots, open questions) in the plan file and commit it, so the user
  can catch up from the repository alone.

## Milestones

Each milestone: spec first (spec agents, from OpenCE and the data), then
implementation (from the specs), tests, screenshots, merge, push, plan
update. Smallest playable value first.

**M0: groundwork (spec only)**
- The Xbox disc image layout and how to read its files (XISO), the `maps/`
  cache-file format (header, compression, the tag index, tag classes and the
  ones a multiplayer match needs), bitmaps, sounds, models (gbxmodel /
  model), collision BSP, the structure BSP (render geometry, lightmaps,
  portals), and animations.
- The core rules for multiplayer play: player movement (speeds, gravity,
  jump, crouch, air control), health and shields (recharge delay and rate,
  overshield), damage (body parts, headshots, melee, falloff), weapons
  (hitscan versus projectile, spread and error, heat and plasma overcharge,
  magazine/reload, zoom), grenades (frag, plasma sticking), pickups and
  respawns, and the multiplayer game types' rules relevant to UGG.
- Output: `specs/halo_ce/` files, each with confidence notes and open
  questions; plan the crates per `docs/new-game.md`.

**M1: a map you can walk on**
- Load one multiplayer map (Blood Gulch or Hang 'Em High) from the user's
  disc image: render geometry with textures and lightmaps, the sky, and
  collision; walk it as an existing UGG character. Screenshots against the
  real game's look (from the user, or the spec's descriptions).

**M2: Halo guns as UGG rungs (the vertical slice)**
- 3–4 weapons: assault rifle, pistol, plasma rifle, shotgun. First-person
  models and animations, sounds, muzzle effects, hitscan and projectiles,
  spread/heat/overcharge per the specs. Registered as data-driven weapons
  (see the `add-weapon-from-data` skill) so UGG ladders can deal them.
- A Halo rung makes you a Halo player: the Spartan body (third person) with
  basic animations, Halo movement feel, shields plus health.
- UGG can run a match where a ladder mixes Halo rungs with the other games'.
- Asset sets for Halo, prepared for the asset service (documented; not
  published).

**M3: the full multiplayer arsenal**
- Every multiplayer weapon: sniper rifle, rocket launcher, needler (homing,
  supercombine), plasma pistol (charge, EMP), fuel rod, flamethrower if in
  the data; frag and plasma grenades; melee; pickups; the HUD elements Halo
  shows for a rung (shields, ammo, motion tracker as the game draws it,
  crosshairs), using UGG's HUD hooks.

**M4: maps**
- All multiplayer maps from the disc in the UGG pool, with spawn points and
  bot navigation; map-specific objects (teleporters, lifts, team bases'
  doors if any); per-map tests that every pool map loads with spawns, as
  the other games have.

**M5: feel and parity pass**
- Measured parity against the real game where the user can capture:
  movement curves, weapon timings, damage numbers, shield timings, sounds'
  falloff. A probe or capture tool if one is needed (as CS:S's probe). Fix
  differences.

**M6: vehicles (only if the earlier milestones are solid)**
- Warthog, Ghost, Banshee and Scorpion as a UGG mode or rung variant: drive,
  passengers, turrets; physics through the engine's existing physics.

**M7 and on: comprehensive**
- Effects and decals, ragdoll and death animations, the Anniversary-free
  classic look throughout, Custom Edition maps if the user wants them, and
  anything left in the plan's backlog. Keep extending the plan with the next
  most valuable item and keep going.

## Start now

1. Read the files above.
2. Ask the user once for the path to their Halo CE disc image if it isn't
   already in this conversation, and clone OpenCE outside the repository.
3. Write `docs/plans/active/halo-ce.md` with these milestones, commit, push.
4. Begin M0 with spec agents.
