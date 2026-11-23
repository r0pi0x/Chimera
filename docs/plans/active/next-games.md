# Next games: ranked by ease

Research, 2026-11-22. Which first-person shooter to port after CS:S,
HL2:DM, TF2 (Source), Combat Arms (LithTech) and Halo CE (blam). Ranked
by ease of implementation, weighed by fit for the party gun game and the
legal rules ([DESIGN.md](../../DESIGN.md), "Assets, code and legal";
[THIRD_PARTY.md](../../../THIRD_PARTY.md)). Nothing here is decided: a
port still starts with its own plan ([new-game.md](../../new-game.md)).

Effort: S = data and a game crate on existing loaders (days); M = a new
small loader family or large game-specific systems (one to two weeks of
agent work); L = a new engine family with rich formats; XL = a new family
with encryption, little documentation or no lawful way to get the files.

## Ranking

| # | Game | Engine / reuse | Formats and parsers | How players get it | Fit | Effort |
|---|---|---|---|---|---|---|
| 1 | Day of Defeat: Source | Source (Orange Box branch), ~same as CS:S | BSP 20, VPK, MDL, VTF: all ours | Steam (app 300), Linux build, no DRM on files | WW2 guns, classes, sprint/prone | S |
| 2 | GoldSrc family: Counter-Strike 1.6, Half-Life DM, DoD 1.3, TFC, Opposing Force | GoldSrc: new loader, movement and gameplay close to Source | BSP 30, WAD3, MDL v10, SPR, plain files; `qbsp` (MIT OR Apache-2.0) reads BSP 30 | Steam (apps 10, 70, 30, 20, 50), Linux builds, no encryption | Iconic guns and maps; Quake-lineage movement | M (family), then S per game |
| 3 | Insurgency (2014), Left 4 Dead 2, Black Mesa | Source branches (BSP 21, newer VPK/MDL revisions) | Mostly ours; per-branch revisions | Steam (222880, 550, 362890) | Realistic guns (INS), co-op zombie guns (L4D2), HL1 guns in HD (BM) | S-M each |
| 4 | Quake III Arena / Wolfenstein: Enemy Territory / Urban Terror | id Tech 3: new family | PK3 = zip, BSP 46, MD3, shader scripts: very well documented; engine source is GPL (spec-only) | Q3A on Steam (2200); ET and UrT are free downloads | Arena guns and movement (Q3), objective WW2 (ET), realistic (UrT) | M |
| 5 | F.E.A.R. (+ expansions) | LithTech Jupiter EX: same family as Combat Arms, later revision | `.Arch00` archives, `.World00p`, `.Model00p`, game database; some overlap with our LTB/DTX work, unencrypted | Steam (21090) and GOG (preservation program) | Strong guns, slow-mo is optional; MP maps exist | M-L |
| 6 | Halo 2 (MCC) | blam, same family as Halo CE | Tag-based maps; zlib-compressed in MCC; official H2 mod tools (MCC) and community docs | Steam MCC (976730); EAC is off in offline/modded mode | Halo guns, dual wield; fits next to Halo CE | L |
| 7 | Unreal Tournament 99 / 2004 | Unreal Engine 1/2: new family | Unreal packages (`.unr`/`.ut2`, `.utx`, `.ukx`...), documented by the community; UE Viewer is under its author's own licence (spec-only) | Free with Epic's permission via OldUnreal (UT2004 since Feb 2026) | Huge gun roster; dodge/double-jump movement differs | L |
| 8 | CrossFire (2007 original) | LithTech Jupiter EX (CA's sibling) | Rez archives, likely encrypted per region; little public documentation | Live free-to-play with anti-cheat | Same genre as CA | L-XL |
| 9 | Battlefield: Bad Company 2 | Frostbite 1.5: new family | `.fbrb` archives (compressed), DICE formats; community tools (Venice Unleashed / Project Rome) with unclear licences | Delisted 2023, EA servers closed; owners keep it in EA app/Steam; Project Rome keeps MP alive | Guns fit; vehicles and destruction do not | XL |
| 10 | WolfTeam (Reboot / Classic) | Softnyx in-house engine | Undocumented, presumably packed/encrypted; no public tools found | Live free-to-play on Steam | Human form is FPS; werewolf form is third-person melee | XL |
| 11 | GoldenEye 007 (N64) | Rare's N64 engine | ROM data; complete decompilation exists (n64decomp/007) but it is derived from Rare's code: spec-only, never in the repo | Cartridge only on PC (a dumped ROM); the 2023 re-release is Xbox / Switch Online, not extractable; no Steam ownership check | Classic guns, but N64 controls and scale | XL, high risk |
| 12 | Battlefield Heroes | Refractor 2 (BF2 lineage) | Documented by BF2 modders | Shut down 2015; never sold; EA stopped revival projects distributing the client: no lawful way to get it | **Third-person**: out of scope | Not recommended |

Garry's Mod was considered and left out: it is Source we already read,
but its own content is mostly HL2's, which HL2:DM already gives us.

## Top five

**1. Day of Defeat: Source.** The cheapest new game: same engine branch,
BSP version and VPK layout as CS:S, so `source_formats`, the world, models,
materials and CS:S-style movement all carry over; new work is its weapon
scripts (bolt-action, deploy for MGs, rifle grenades), stamina sprint and
prone, class loadouts and its HUD. It is on Steam with a Linux build, so
the asset service's Steam ownership check works as for CS:S, and the
Source modding wiki documents it like CS:S. Unknowns: prone and deploy
need camera/hull work that CS:S never needed; its player models use more
animation layers. Effort S; mostly a spec session for its weapons and
movement deltas.

**2. GoldSrc family (CS 1.6 first).** One new loader family unlocks about
six Steam games, all with Linux builds and unencrypted files, and CS 1.6
is the most recognisable shooter we could add. Formats are small and very
well documented (BSP 30 with embedded or WAD3 textures, MDL v10 with one
file per model, SPR, plain `.txt`/`.cfg` data); the `qbsp` crate (Noxmore,
MIT OR Apache-2.0) already parses BSP 30 and could be adopted with the
user's OK. Valve's Half-Life SDK is under a non-permissive licence, so
gameplay arrives by specs only, as with the Source SDK. Movement is the
Quake lineage our Source movement already follows, so specs are mostly
deltas. Unknowns: software-style lightmaps and palette textures in our
renderer, MDL v10 skeletal animation, CS 1.6's weapon values (the SDK does
not hold CS). Effort M for the family, then S per game.

**3. Source branches: Insurgency, Left 4 Dead 2, Black Mesa.** Same
engine family; each adds a format revision (BSP 21, newer MDL and VPK
versions; any ICE-encrypted `.ctx` scripts are already handled by our
TF2 port) rather than a new loader. Insurgency brings realistic
guns, L4D2 a large co-op arsenal, Black Mesa HL1's guns in modern art.
All on Steam. Unknowns: per-branch shader and material differences,
Insurgency's custom weapon attachment system. Effort S-M each; a good
second step after DoD:S once the branch differences are mapped.

**4. id Tech 3 (Quake III Arena, Enemy Territory, Urban Terror).** A new
but small and thoroughly documented family: PK3 is zip, BSP 46 with
Bézier patches, MD3 vertex-animated models, text shader scripts. The
engine source is GPL (ioquake3, ET:Legacy), so it is reference for specs
only, never code. ET and Urban Terror are free downloads (ownership check
not needed; check each data licence before the asset service serves
them); Q3A is on Steam. Gameplay fits a gun game directly. Unknowns: the
shader-script renderer (multi-pass, animated stages) is the bulk of the
work; strafe-jumping physics differ from Source. Effort M.

**5. F.E.A.R.** The only candidate that reuses the LithTech side: same
company and engine line as Combat Arms (Jupiter EX), files unencrypted,
sold on Steam and GOG, and its guns are among the best liked in the
genre. Some of our LTB/DTX/attributes knowledge may transfer, but its
world (`.World00p`) and model (`.Model00p`) formats are a later revision
and need their own specs; community tools exist but their licences are
unverified (treat as reference). Unknowns: how much of CA's format work
actually overlaps, its game database format. Effort M-L.

## The user's named games

- **Battlefield Heroes**: third-person, and there is no lawful way for a
  player to obtain the client today. Not recommended.
- **Battlefield: Bad Company 2**: possible for players who bought it, but
  Frostbite is a large undocumented family and vehicles/destruction are
  outside the party game's needs. XL; later, if ever.
- **WolfTeam**: live service, proprietary engine, no public format work
  found, and its signature werewolf form is third-person. XL; not soon.
- **GoldenEye 007**: the only PC-accessible version is the N64 cartridge
  ROM (the 2023 re-release runs on Xbox and Switch Online only). Ownership
  cannot be checked through Steam, so the asset service model does not
  apply; the decompilation can inform specs only. XL, high legal risk;
  revisit only if the user accepts "bring your own ROM, local load only".
- **Halo CE** is already ported; Halo 2 (MCC) is its natural follow-up
  (rank 6).

## Recommendation

Port **Day of Defeat: Source** next (smallest effort, high reuse, clean
legal path), then start the **GoldSrc family** with CS 1.6 as the next new
engine, since one loader yields six games.

Started 2026-11-23 (user's go): the plan is [dods.md](dods.md), the
first specs are in `specs/dods/`; `qbsp` (MIT OR Apache-2.0) is
approved in THIRD_PARTY.md for the GoldSrc plan, not yet a dependency.
