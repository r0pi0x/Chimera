# Converted assets on disk: a session's first load without conversion

Status: **measured, not built** (2026-11-17). The backlog entry is
docs/backlog.md §9, "Converted assets on disk and on the asset
service"; it builds on "One kept-dressing cache in Chimera for every
game" (the session cache, docs/performance.md, "Guest dressings kept
for the session"). The prototype is on branch `converted-cache`
(`apps/party_kit/tests/it/heavy/converted_cache.rs`); nothing of it is
on main.

## What was measured

Each game's guest parts: what its dressing of another game's map
converts (the Source guests' `dress_guest_in`, Combat Arms'
`world::dress_with`), dressed onto an empty map with textures and the
kept guns, its whole-DXT textures keeping their blocks alone, as
`source_engine::content::guest` keeps it. That is the work a session's
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
  converter hash on each build, so a developer never reads a stale
  entry. A tool command, `chimera_pack convert --check`, converts from
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
   the clients' choice by version, and the format table.

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
