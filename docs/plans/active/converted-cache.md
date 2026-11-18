# Converted assets on disk: a session's first load without conversion

Status: **steps 1-7 done** (2026-11-18; "Progress" below; Halo's part
written down, not moved): the disk layer on by default, converter
versions narrowed to what shapes the output with a guard test, served
converted dressings built and tested locally, **not yet on the live
service** (the owner's rollout: "Served entries", below); next: the
owner's rollout, Halo on the user's PC. The backlog entry is
docs/backlog.md §9, "Converted assets on disk and on the asset
service"; it builds on "One kept-dressing cache in Chimera for every
game" (the session cache, docs/performance.md, "Guest dressings kept
for the session"). The prototype is on branch `converted-cache`
(`apps/party_kit/tests/it/heavy/converted_cache.rs`); main has its
successors (`apps/party_kit/tests/it/heavy/converted.rs`,
`chimera_world::map::kept`).

## What was measured

Each game's guest parts: what its dressing of another game's map
converts (the Source guests' `dress_guest_in`, Combat Arms'
`world::dress_with`), dressed onto an empty map with textures and the
kept guns, its whole-DXT textures keeping their blocks alone, as
`source_engine::content::guest` kept it (now `chimera_world::map::kept`). That is the work a session's
first map load does for every guest today, and what the session cache
already skips for the second. Each run is its own process.

The prototype stores that `MapData` in the converted-map cache's
existing on-disk format (chimera_assets' `map_cache`, docs/plans/active/
asset-service.md, "Converted-map cache"): magic and format number, a
key that hashes the map format's version (chimera_world's sources),
the converter's version (the game crate's build.rs hash), the load
options and the mount's layers, then every file the conversion read
with its fingerprint (VPK entry CRC and place, a `.rez` entry's place,
a store record's id, else a BLAKE3), and the payload in postcard with
BLAKE3 per 16 MB. A read checks every input again and the checksums,
then decodes. No new format was needed: the map cache's already does
what the brief asks of one (versions, input hashes, migrations by
miss).

Box: the dev box, test profile (dependencies at opt-level 3, our
crates at 1: our decode and conversion code is slower than in the
playtest or release build; a playtest-profile run was cut short by a
full `/tmp`), load average 13-46 from other agents' builds.
**Instructions** (this thread and every thread it starts,
`InstructionCounter::current_with_children`) are the measure; they
repeat to within 0.1% across runs. Wall times are context only.
TF2 is the client install (`~/games/tf2`). Halo: CE is not on this
box (no disc image); its parts are not measured.

### Where a cold first dressing goes (instructions, M)

| | CS:S | HL2:DM | TF2 | Combat Arms |
|---|---|---|---|---|
| whole dressing | 10,049 | 13,077 | 41,047 | 22,516 |
| bodies (MDL/VVD/VTX parsed, meshes, skeletons, animations) | 2,534 | 7,735 | | character models 6,939 |
| guns (held and view models) | 666 | 701 | | view models 16 |
| HUD look | 217 | 112 | | |
| sounds (scripts, WAV/MP3 decoded) | 230 | 875 | | |
| looks (decals, effect materials, flashes, shells) | 46 | 583 | | effect looks 15,557 (with the install's weapon and attribute data, read on first use) |
| textures decoded (VTF to RGBA8 and mips, `finish_textures`) | **6,365** | 3,074 | | (in the models) |
| the files it read, read again (vfs, archives) | 190 (1,710 files, 108 MB) | 1,841 (1,430 files, 90 MB) | 54 (1,711 files, 454 MB) | 2,678 (361 files, 231 MB, `.rez`) |

TF2's steps are private to its dressing (one row). The file reads are
measured with the OS's cache warm, so they are the vfs and archive
work (Combat Arms' decryption), not the disk.

Two findings beside the cache:

- **CS:S spends 63% of its cold dressing (HL2:DM 24%) decoding
  textures to RGBA8 and mip levels that are then thrown away**: a kept
  guest texture of whole DXT blocks keeps the blocks alone
  (`keep_blocks_only`), but `material::decode_bytes` decodes the full
  image and every mip first. A load that knows it keeps blocks only
  could skip that decoding (check first what reads the texels during
  the dressing: alpha tests, `render::shadows`). An upper bound of
  6.4 G instructions for CS:S, with no disk cache at all.
- **Combat Arms' cold dressing is mostly its data, not its models**:
  15.6 G of 22.5 G is the effect looks with the install's weapon and
  attribute data parsed on first use (asset-service.md §10 named "CA
  weapon data parsed into one document" already). The gameplay needs
  that data anyway, so a cache of the dressing alone saves the 6.9 G of
  character models and the looks' own share, not the data's. The data
  is its own cache candidate (one document per install build).

### Converted vs read back (the prototype)

| | CS:S | HL2:DM | TF2 | Combat Arms |
|---|---|---|---|---|
| from the originals, OS cache warm | 10,049 M, 0.8-1.4 s | 13,077 M, 1.7 s | 41,047 M, 7.9 s | 22,516 M, 5.2 s |
| from the originals, OS cache dropped (`posix_fadvise` on the install) | 7.7 s | 5.5 s | 19.6 s | 15.0 s |
| written to the cache (a miss: converting plus writing) | +3,100 M, +0.6-1.0 s | +2,050 M, +0.7 s | +8,700 M, +3.5 s | +150 M, +0.1 s |
| **read from the cache, OS cache warm** | **3,103 M, 0.40 s** | **1,854 M, 0.21 s** | **8,960 M, 0.86-1.05 s** | **143 M, 0.11 s** |
| read from the cache, its file dropped from the OS cache | 1.6 s | 1.1 s | 6.4 s | 0.27 s |
| gain in instructions | 3.2x | 7.1x | 4.6x | 157x (but see the data above) |

A read's time (`CHIMERA_MAP_CACHE_LOG=1`): CS:S checksum 0.05 s,
1,751 inputs checked 0.12 s, decode 0.44 s; TF2 0.18 s, 0.07 s, 1.28 s.
Decoding the payload is most of it, and it runs in our crates
(`pod_serde`) at opt-level 1 here: an optimized build reads faster still.
Cold from disk, the cache is read at the disk's pace (TF2's 773 MB in
6.4 s on this encrypted disk under load), still 3x faster than the
originals cold (the originals' archives are bigger and read scattered).

### Sizes

| | CS:S | HL2:DM | TF2 | Combat Arms |
|---|---|---|---|---|
| originals the dressing read | 108 MB | 90 MB | 454 MB | 231 MB |
| cache entry, raw | 225 MB | 180 MB | 773 MB | 49 MB |
| zstd -3 (what the asset service would store and send) | 69 MB | 65 MB | 304 MB | 27 MB |
| zstd -19 | 61 MB | 52 MB | 265 MB | 20 MB |

What a raw entry holds (MB, postcard as the cache encodes it; test
`sizes`):

| | textures | sounds (samples) | meshes | animations | LODs | the rest |
|---|---|---|---|---|---|---|
| CS:S | 60 | 37 | 27 | 88 | 10 | 4 |
| HL2:DM | 32 | 37 | 31 | 37 | 33 | 10 |
| TF2 | 305 | 50 | 102 | 266 | 20 | 29 |
| Combat Arms | 43 | 0 | 5 | 0 | 0 | 0 |

Raw entries are about twice the originals: animations as Chimera
holds them in memory (the largest part for CS:S and TF2), meshes in
Chimera's layout, sounds decoded to samples; compressed they are
smaller than the originals for CS:S and HL2:DM. TF2's textures are
305 MB (many aren't whole DXT and keep RGBA8). The biggest cuts:
animations stored compressed (quantized, or kept as the original's
encoded data and decoded at load), textures as blocks only, sounds as
the original's encoded bytes.

### Determinism

The converted values are the same in every run (counts and sizes
equal), but **the entries' bytes are not**: two writes of CS:S, HL2:DM
and TF2 differ (Combat Arms' are byte-identical). The first difference
is just past the textures, where the sound table starts: most likely `MapSounds::entries` and
`surfaces` (and `MapData::surface_names`) are `HashMap`s, serialized in
their random order. The local cache doesn't care; a served,
content-addressed copy and the generated-assets rule (byte-identical
for the same input) do. Fix: serialize maps sorted (a `pod_serde`-style
helper, or `BTreeMap`/`IndexMap`), with a test that converts twice and
compares bytes.

## What each game would gain

From a session's first map load with guests (the sandbox's greybox,
a ladder match's first map); the session cache already covers later
ones:

- **CS:S:** 10.0 G to 3.1 G instructions (about 1 s to 0.4 s here).
  Skipping the thrown-away texture decoding alone would take it to
  about 3.7 G without any disk cache.
- **HL2:DM:** 13.1 G to 1.9 G (1.7 s to 0.2 s): its bodies' conversion
  is the cost, and the cache removes it.
- **TF2:** 41 G to 9 G (8 s to about 1 s): the biggest gain, and the
  biggest entry (773 MB raw, 265-304 MB compressed).
- **Combat Arms:** its dressing's own share (character models 6.9 G and
  the looks' own part) goes; the install data parse (most of the 15.6 G)
  stays unless cached too.
- **Halo: CE:** not measured (no disc image here); its donors
  (`body::load::donor`, `view::load::donor`, `audio::donor`) join the
  same API.
- Players served by the asset service: the first join would download
  converted parts instead of the original files these read (sets
  change), and a first load then reads, never converts. Sizes above.

## Proposed design (Chimera)

One API beside the kept-dressing cache, every game plugging in the
same way (the backlog's "One kept-dressing cache in Chimera"):

- **`chimera_world::map::kept`** (lifted from `source_engine::
  content::guest`): `dress_cached(game, map, options, Dressing)` where
  a game's `Dressing` gives `convert(&Mount, &mut MapData)`, its
  `converter` version, its `mount()` and `files_here`. Chimera keeps
  the session copy (`LoadContext`), appends it (`append`, texture
  shift), refreshes it on late files, logs its time and, new, puts a
  disk layer under the session copy: on the session's first ask it
  tries `map_cache::cached` with key `guest:<game>` + converter +
  options + layers, before converting. The same function serves Combat
  Arms' shared models and Halo's donors, so a new port gets it by
  giving its dressing.
- **One format, already there:** the map cache's entry (versioned
  magic, key, inputs with fingerprints, chunk checksums). A guest entry
  is a `MapData` like a map's. What must be serializable that isn't:
  Combat Arms' `CaModelIndex` (a memo; `append_shared` needs it), each
  game's per-session memos a dressing leaves.
- **Where, locally:** the map cache's folder and cap
  (`~/.cache/chimera/maps`, `%LOCALAPPDATA%\chimera\maps`,
  `CHIMERA_MAP_CACHE_GB`, LRU), its switch (`chimera:map_cache 0`,
  `CHIMERA_MAP_CACHE=0`) turning guests off too; guest entries are
  hundreds of MB, so the default cap (40 GB) holds them easily.
- **On the asset service:** a converted entry becomes a record like any
  other (asset-service.md §3, option C: "a transform is a record keyed
  by (transform version, source ids)"). `chimera_pack` gains a
  `convert` step that runs the same `Dressing::convert` on the packed
  files (the store's mount, so the inputs' fingerprints are record ids)
  and stores the entry zstd-compressed with its recipe in the manifest:
  converter id and version, map format version, input record ids. A
  client whose versions match takes the record; one whose don't
  converts from the original records as today. Anyone with the
  originals rebuilds the same records with the same `chimera_pack`
  (needs the determinism fix above).
- **Versioning and invalidation:** the key already carries the map
  format's version (hash of chimera_world's `src/map` + `FORMAT_BUMP`)
  and the converter's (hash of the game crate's sources and data); a
  changed install file is caught by the inputs' fingerprints. Any change
  is a miss and a new entry; old entries age out of the LRU. That is
  the migration policy for a local cache: regenerate, never migrate.
  For served records, keep old versions on the service while a released
  client may still ask for them; a cheap migration (a field added with a
  default) is a serde default, not a version bump; anything else
  converts again (the originals are always reachable: the service keeps
  the original records too, and developers' installs stay the input).
- **Bookkeeping:** one table, generated, of every converted format and
  its current version (map format, each game's converter, each served
  transform), in docs/ (beside docs/modules.md, by `docs_index`), and
  the manifest's recipe per record. A test fails when a version changes
  without the table.
- **Developing:** `CHIMERA_MAP_CACHE=0` (or `+chimera:map_cache 0`)
  converts from the originals every time; converter work changes the
  converter hash on each build (within its hashed paths; elsewhere,
  bump its number: "Converter versions"), so a developer never reads a
  stale entry. A tool command, `chimera_pack convert --check`, converts from
  the originals and compares with the stored record byte for byte.

## Risks

- **Disk:** guest entries add 0.5-1.3 GB per install set locally (CS:S,
  HL2:DM, TF2, CA: 1.2 GB raw); the cache's cap and free-space floor
  already bound it.
- **Our dev builds' converter hash changes on every edit of a game
  crate**, so developers miss often (the map cache has the same
  property); release builds are stable.
- **Nondeterministic bytes** (HashMap order) until fixed: harmless
  locally, breaks content addressing on the service.
- **Read cost is not zero:** decoding is 0.2-1 s for the Source guests
  here; textures and sounds decoded lazily (streamed textures, backlog
  §9) would cut both the entry and the read.
- **Late files:** an entry converted while store files were missing
  (`late_files`) must not be written, or must carry its late list (the
  session cache already re-dresses on them).
- **Correctness:** a cached entry must give the same parts a fresh
  conversion does; the guest-cache heavy test's comparison
  (`named_refs`, sound entries, bodies) is the check to reuse.

## Progress

- **Step 1, done (2026-11-17):** every `HashMap` a `MapData` serializes
  (sound entries and surfaces, the HUD look's and menus' tables,
  surface names, animation names) is written in key order
  (chimera_world's `pod_serde::sorted`); the decals' groups, materials
  and logos are `BTreeMap`s now. Each
  guest's dressing converted twice now encodes to the same bytes: CS:S
  225 MB, HL2:DM 180 MB, TF2 773 MB, Combat Arms 616 MB (party_kit
  `heavy::converted::a_guest_dressing_converted_twice_writes_the_same_bytes`).
- **Step 2, done (2026-11-17):** a kept dressing (`map::BlocksOnly`)
  no longer decodes its whole-DXT textures (`MaterialLoader::
  blocks_only`); the kept textures and the dressing's bytes are the
  same. A cold dressing: CS:S 10.4 G to 4.1 G instructions (61% fewer),
  HL2:DM 13.1 G to 10.0 G (24%), TF2 42.5 G to 18.6 G (56%)
  (docs/performance.md, "Guest textures kept as blocks without
  decoding"). Combat Arms' kept models already did; Halo throws nothing
  away.
- **Step 3, done for the Source guests (2026-11-17):** the session cache
  is Chimera's `map::kept` (`Dressing { game, converter, mount, convert
  }`, `dress_cached`, `append`, `add_decals`, `add_sounds`, the late
  files checked on the game's own mount, each conversion's time logged);
  CS:S, HL2:DM and TF2 dress through it; docs/new-game.md step 3. Combat
  Arms and Halo stay on their own caches for now: Combat Arms' dressing
  leaves memos (`CaModelIndex`, the effect library) a kept `MapData`
  doesn't carry (step 5's serializable index), and Halo's donors are
  kept as their own types; Halo needs the disc image (the user's PC).
- **Step 4, done (2026-11-17), off by default:** `map::kept`'s disk
  layer (`CHIMERA_GUEST_CACHE=1`; `kept_with` takes the cache): the
  map cache's format and folder, key `guest:<game>` + converter + map
  format + textures + layers, inputs fingerprinted (a late file's
  absence too, so its arrival misses). Read back, the same bytes as
  converting; instructions converted-and-written / read back: CS:S
  7.2 G / 3.4 G, HL2:DM 12.6 G / 1.9 G, TF2 27.3 G / 10.4 G
  (docs/performance.md, "Guest dressings on disk"). Unit test: a
  changed input file misses.
- **Step 5, done for Combat Arms (2026-11-17):** `world::DRESSING`
  dresses through `map::kept` (`dress_kept`). Two additions to the
  API, for every game: `Dressing::memos` (`KeptMemos`: how a game's
  memos are appended to each map, written beside the parts on disk,
  `map::KeptMemoBytes`, and read back) and `Dressing::variant` (an app's
  choice that changes the dressing: Combat Arms' view models, none when
  they load on demand; in the session's key and the disk's). Combat
  Arms' memos: the character models' index (`characters::kept`: each
  rig as its parsed body file and animation list, every animation once
  and shared again on the read, the install data's child models
  refilled so later bodies share them; `lithtech_formats::ltb` derives
  serde, its big arrays as byte strings) and the effect library's
  loaded parts (the effect file is the install data's, parsed once). The
  kept dressing gives the same parts and memos as `dress_with` (its
  textures as blocks, as every kept dressing's), and read back the same
  again (combat_arms `heavy::guest_dressing`). Test profile: converted
  and written 36.6 G, read back 8.9 G (4.1x fewer), 347 MB.
  **Combat Arms' install data as its own entry: the merged weapon
  documents.** Of a cold dressing's 34.8 G, the install data is 15.0 G:
  the attribute files 5.2 G (decrypting 0.7, parsing the documents 4.5)
  and the weapon set 9.8 G (merging our 63 documents with the install's
  into 2,363, 16.3 MB of TOML, 4.7; parsing those, 5.6). The game's
  weapons need the parsed attribute records and the parsed `WeaponSet`
  whatever the dressing does, and neither serializes (the records are
  dozens of types read from `Document`s; the set holds typed hooks parsed
  from TOML), so the entry is the merged TOML (`CaData::
  weapon_docs_merged`, under the same switch, keyed by a hash of the
  attribute files' bytes, our documents and the converter): merging
  4,671 M, read back 54 M, the same documents
  (`heavy::guest_dressing::the_merged_weapon_documents_read_back_the_same`).
  The other 10.3 G stay until the records or the set serialize.
  **Halo, not moved (no disc image here).** How its donors would sit
  behind the same API: one `Dressing` for `audio::GUEST`'s whole chain
  (game `halo_ce`, converter `maps::CONVERTER`, mount `maps::mount_for`
  the context); `convert` reads the donor map (`body::load::DONOR_MAP`)
  once through the given mount and adds, in today's order, the
  Spartan (`body::load::add`), the guns' looks (`view::load::add`, its
  `looks(&file, decode)` with `decode` = the map isn't `SizesOnly`), the
  effects (`fx::load::add`) and the kit's sounds (`audio`'s `kit` on a
  `Compiler`), each split from its `donor(context, ..)` into a function
  of the parsed file; `memos` keeps what they leave in the map's memos,
  `view::load::Displays` (`displays_of`) and `Arc<HaloFx>`: `append`
  inserts them as `add` does, `save`/`load` write them with serde if
  their types allow it, else `load` builds them again from the donor
  map (then the disk saves the models' conversion only). The four
  `Donor` memos in `LoadContext` then go. Measure first on the user's
  PC (the per-map append, the cold donor build, and the read back),
  with the same heavy test shape as Combat Arms'.
- **Step 6, done (2026-11-17): the disk layer is on by default**
  (`CHIMERA_GUEST_CACHE=0` turns it off). Playtest build, a cold first
  dressing per guest in a process of its own (docs/performance.md,
  "Guest dressings on disk"): converted / read back, instructions and
  ms: CS:S 2,949 M 475 ms / 1,697 M 143 ms; HL2:DM 9,006 M 963 ms /
  1,320 M 139 ms; TF2 13,147 M 2,083 ms / 4,951 M 451 ms; Combat Arms
  21,446 M 2,610 ms (its install data in it) / 4,066 M 578 ms plus its
  install data 8,716 M 1,015 ms. With the OS's cache dropped the reads
  take 193-736 ms, the conversions 628-2,874 ms. The first run of a
  converter version writes (+0.7-3.4 G). Same bytes and memos read back,
  so no visual change; heavy test: party_kit `heavy::converted::
  a_guest_dressing_reads_back_from_disk_by_default`.

## Converter versions (2026-11-18)

The user's goal: players and developers almost never download or convert
again. Before, each game's `CONVERTER` hashed its whole `src/game`, its
`data`, Source's engine and formats crates and the whole `Cargo.lock`, so
any gameplay edit (a gun's fire rate) or any dependency bump made every
kept dressing and converted map a miss, and would have needed a new served
copy.

- **Now** (each game's build.rs, `source_hash::converter`): `v<number>-
  <hash>`. The number (`VERSION`) is bumped by hand; the hash covers only
  what shapes the converted output (`CONVERTING`: the map converter and
  the kit it adds, the guest dressing and the modules it converts with:
  bodies, view models, HUD look, sounds, effect looks; Source's
  `content`, `hud/look.rs` and `effects`; the format crates; Chimera's
  `chimera_formats`; the data files those read) and the versions of the
  parsing crates in `Cargo.lock` (`PARSERS`: CS:S, HL2:DM and TF2 `vtf`,
  `vmdl`, `vbsp`, `vmt-parser`, `image`, `half`, `lzma-rs`, `bzip2`,
  `glam`, `symphonia*`, `postcard`; Combat Arms `aes`, `glam`,
  `symphonia*`, `postcard`; Halo `flate2`, `glam`, `postcard`), not the
  whole lockfile. The map format's own version (chimera_world's `src/map`
  and `FORMAT_BUMP`) is unchanged. docs/formats.md, generated by
  `docs_index`, lists every number and hashed path.
- **Policy:** bump `VERSION` when a change outside `CONVERTING` changes
  what a conversion gives: a gun's model path in its weapon data, a
  gameplay module a dressing calls (CS:S's `weapons.rs` lists its guns'
  models; `view_anim.rs` its shells and flash), a new kept gun. A change
  inside `CONVERTING` needs nothing (its hash moves). When in doubt,
  bump: a bump costs one conversion per player, a stale hit shows old
  parts.
- **The guard** (party_kit `heavy::converted_prints::converted_output_
  matches_its_version`): converts each game's fixtures from the install
  here, never from a cache (its guest dressing as kept; CS:S's
  `test_speakers`, HL2:DM's `halls3`, TF2's `itemtest`; Combat Arms'
  dressing), and compares the output's hash with
  `apps/party_kit/tests/converted-prints.txt` (hashes only, never game
  files), one line per game, fixture, system and the install files read.
  Same files, another output and the same converter version (number
  and hash) fails ("bump VERSION in games/<game>/build.rs, or add the
  changed file to CONVERTING, then record the prints":
  `CHIMERA_UPDATE_CONVERTED_PRINTS=1`); another output under a new version
  (an edit inside `CONVERTING`, or a bump) fails until the prints are
  recorded, so the next comparison is against the current version; other
  install files or another system pass with a note (the Windows PC
  records its own lines). Writing it found one more nondeterminism: a
  map's sound clips were numbered in a `HashSet`'s order
  (`source_engine::content::sound`, now name order), so the same map gave
  other bytes each run.
  It runs in the heavy tier (post-push), so a missed bump is caught on
  the next push with the installs, before a release.
- **Measured** (each non-merge commit's changed files against the old
  and the new hashed sets; a `Cargo.lock` change counts only when a
  parsing crate's version moved): of today's 78 commits on main
  (2026-11-17, most of them converter work: steps 1-6), 24 would have
  invalidated some game's caches before, 15 after; per game CS:S 12 to 9,
  HL2:DM 12 to 9, TF2 14 to 10, Combat Arms 14 to 7, Halo 11 to 3. Over
  the week since 2026-11-10 (744 commits): 318 to 163; CS:S 79 to 34,
  HL2:DM 88 to 33, TF2 85 to 42, Combat Arms 146 to 57, Halo 170 to 72.
  What still invalidates is converter work itself (Source's `content`,
  the dressing modules) and the map format.
- **Known shortcuts:** the hashed sets are chosen by hand from what each
  converter calls; a file they miss relies on the bump and the guard,
  whose fixtures cover the dressings and one small map per Source game
  (Combat Arms' worlds, cached only with `CHIMERA_MAP_CACHE_CA=1`, and
  Halo, with no disc image here, aren't fixtures yet).

## Served entries (step 7, 2026-11-18)

- **Format:** a served entry is the converted-map cache's entry (the same
  bytes a client's cache writes, its key the served one: the same parts
  but `layers` = `served`), stored as a record of the game's manifest at
  `_converted/guest/<game>/<converter>/<key>.map`, with its recipe at
  `<key>-recipe.txt` (`chimera_assets::map_cache::served`): the key's
  parts in words (game, converter, map format, textures, variant) and
  every input the conversion read with its fingerprint (`record <id>`).
  The manifest's format is unchanged (two `file` lines per entry, no
  set), so released clients read it as before and never look there.
- **The packer:** `chimera_pack convert <game|all> --store <server
  store> [--sign]` reads each game from the store alone (as a player's
  client mounts it; nothing fetched; a converted-map cache of its own)
  and runs its bundle's dressing with `map::kept::Export` in the app's
  context, for a client (textures; Combat Arms' view models on demand)
  and a dedicated server (no textures); each entry and recipe is stored
  (zstd 19) and entered in the game's newest manifest. The same files
  give the same bytes (step 1), so anyone with the originals rebuilds the
  same records; `--check` converts and compares byte for byte, writing
  nothing; `--list` shows the entries by converter; `--drop <converter>`
  removes one version's.
- **A client** (`map_cache::cached_served`, from `map::kept` on a local
  miss): its converter and format versions name the path; if its mount
  lists it, it reads the recipe (small), checks every input against its
  own mount (record ids from the manifest: nothing downloaded), then the
  entry (fetched, its SHA-256 checked), checks it as a local entry (key,
  chunk checksums, inputs), keeps it in its converted-map cache under
  its own key and uses it (`kept::served_reads`). Any difference (another
  version, an install layered over the store, another manifest's files)
  converts from the originals as before. `CHIMERA_CONVERTED_SERVED=0`
  never takes one; `CHIMERA_GUEST_CACHE=0` or `CHIMERA_MAP_CACHE=0`
  neither.
- **Retention:** the service keeps the entries of every converter
  version a released client still uses (`--list` names them; each
  release's commit has its versions in its build); `--drop <converter>`
  then `chimera_assets --store <server store> gc` once no release in use
  has it. The originals stay on the service, so a client without a
  matching entry always converts.
- **Tests:** `chimera_assets` unit tests (`map_cache::served`: packer to
  client, versions and files that differ, list and drop); its
  integration test `converted` (the service on 127.0.0.1 with a test
  store: the client downloads the recipe and the entry and none of the
  originals, converts nothing, gets the same value; another version
  converts and fetches the originals); `chimera_world` `map::kept` unit
  test (a dressing exported and taken); party_kit heavy
  `heavy::converted::a_served_dressing_is_the_same_as_converting`
  (CS:S and HL2:DM from the installs here, packed into a test store:
  taken, the same bytes as converting).
- **Sizes** (raw / stored at zstd 19, a client's entry; the originals the
  dressing reads, stored): CS:S 226 MB /
  62 MB (the 1,710 originals it reads: 56 MB stored), HL2:DM 180 MB /
  53 MB (1,459 originals, 55 MB stored), measured by the heavy test.
  TF2 (773 MB raw, about 265 MB at zstd 19 in the prototype) and Combat
  Arms (its memos now in the entry) aren't measured served yet: the
  owner's `convert --list` prints them. Served, a client trades a
  conversion (CS:S 2.9 G instructions, 475 ms in the playtest build;
  HL2:DM 9.0 G, 963 ms; TF2 13.1 G, 2.1 s; Combat Arms 21.4 G, 2.6 s)
  for a download of about the originals' size and a read (1.3-5 G).
  `chimera_pack convert hl2dm` on a copy of this box's asset store: a
  client's entry 180 MB / 53 MB stored, a server's 149 MB / 35 MB, a few
  seconds each in the test profile; `--check` again: the same bytes.
- **Known shortcuts:** a client keeps the downloaded record in its asset
  store beside its local entry (the store has no cap yet); TF2's dressing
  asks whether class bodies are here (`Access::Held`), so a client takes
  TF2's entry only when it holds what the packer held (every class);
  `base` still carries the originals the dressing reads (released
  clients need them), so a first join still downloads them: a lean
  `base` for clients that take served entries is a later step.

### The owner's rollout (not done: build and test only, 2026-11-18)

On the machine with the server store, from a checkout at the released
build's commit:

```
S=<server store>; B=<backup>/manifests-$(date -u +%Y%m%d-%H%M)
cp -a $S/manifests $B                      # snapshot every manifest first
cargo build --release --features tools --bin chimera_pack
target/release/chimera_pack convert all --store $S --check   # converts, writes nothing
target/release/chimera_pack convert all --store $S --sign    # enters and signs
target/release/chimera_pack convert all --store $S --list
```

The service reads a changed manifest within a second; clients take the
newer manifest at their next start or join. Undo: `rm -r $S/manifests
&& cp -a $B $S/manifests`. After a later `publish`, `sets` or `lean`,
run `convert` again (last). docs/playtest-hosting.md, "Converted guest
dressings", has the same.

## Steps

1. Determinism: sorted serialization of the `HashMap`s in `MapData`
   (sounds, surfaces, surface names); a heavy test that converts each
   guest twice and compares entry bytes. Also fixes CS:S map entries.
2. Skip decoding textures a load will keep as blocks only (measure the
   CS:S and HL2:DM gain; check nothing reads their texels).
3. Lift the session cache into `chimera_world::map::kept` (the existing
   backlog entry), Source guests first.
4. Disk layer under it, off by default (`CHIMERA_GUEST_CACHE=1`), Source
   guests; heavy test: hit equals a fresh conversion (the guest-cache
   test's checks), a changed input misses.
5. Combat Arms' shared models (serialize `CaModelIndex`) and Halo's
   donors onto it; measure Halo on the Windows PC.
6. On by default after a playtest-profile measurement.
7. The asset service: `chimera_pack convert`, recipes in the manifest,
   the clients' choice by version, and the format table (done
   2026-11-18, with the converter versions narrowed: "Converter
   versions", "Served entries").

## Decisions for the user

- Serve converted parts from the asset service, or only cache them
  locally? Served, a first join downloads 61-69 MB (CS:S), 52-65
  (HL2:DM), 265-304 (TF2), 20-27 (CA) compressed instead of the
  originals these read, and never converts; but the service must
  keep a copy per converter version a released client uses.
- For TF2, accept a 773 MB local entry, or first make its textures
  smaller (blocks, or decode lazily)?
- Combat Arms: also cache its install data (weapon and attribute
  documents, 15.6 G of its cold dressing), as its own entry?
- Order: the two cheap wins (deterministic bytes, skip the thrown-away
  texture decoding) before the disk layer?

### The user's answers (2026-11-17)

- **Served:** yes, converted parts are served from the asset service.
  Players get faster loads with no downside to them.
- **TF2's size:** our call. The rule is visual and experience parity:
  only lossless reductions, so the result looks and plays the same.
  Examples: the original DXT blocks kept as blocks, shared or
  deduplicated animation data, compression.
- **Combat Arms' install data:** cache it as its own entry if it
  improves the metrics (parity, performance, load time). Measure it.
- **Order:** the two cheap wins first, then the Chimera API, then the
  disk layer.
