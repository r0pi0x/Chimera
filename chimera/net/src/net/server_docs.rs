//! Server data documents over the network (docs/plans/active/
//! multiplayer.md, "Server data documents"; net33; chimera_sim's
//! `server_docs`): a client plays with the server's derived data, not its
//! own, wherever the two differ.
//!
//! - **The offer**: the match manifest names each registered set
//!   (`DocsOffer`: its name, the SHA-256 of its documents as sent, their
//!   count and size) and the weapon registry (`RegistryOffer`: the SHA-256
//!   of every weapon id in registry order, every game's, and their count).
//! - **The client** (`check`, from `manifest::accept`): a set whose hash
//!   differs from its own (no install data, another install version) is
//!   asked for (`DocsRequest`, once per set and offer) and the client
//!   waits for it before it enters the game (`pending`: `client::check_map`
//!   holds `Loaded` back). With nothing to ask for, or once everything
//!   asked for is in, its weapon registry must hash as the server's: else
//!   it leaves with the reason (another build's weapons) rather than play
//!   with other weapons at the same indices. A set this build doesn't have
//!   is skipped (the registry check decides).
//! - **The server** packs each set once, in a thread started with the
//!   first manifest (`Packed`: the documents as postcard, bzip2'd; Combat
//!   Arms' install set, ~2,300 documents: ~14 MB of text, ~280 KB packed,
//!   ~2 s; a big set's packing is kept in the converted cache by the
//!   documents' hash and read back at the next start, `pack_kept`), and sends a client what it asked for in `DocsChunk`s of
//!   `CHUNK` bytes, `PER_FRAME` a frame, logging each set's sizes. A set
//!   packed over `DocsLimit` (`MAX_PACKED`, 4 MB) isn't sent: the server refuses with the
//!   reason (`DocsDenied`), and the client leaves with it.
//! - **On arrival** the client unpacks a set (at most `MAX_UNPACKED`),
//!   checks its hash against the offer, and puts it in (chimera_sim's
//!   `server_docs::apply`: the weapon registry rebuilt from the server's
//!   documents, a game's resources replaced). `DocsHeard` keeps what came
//!   (sets, packed and unpacked bytes) for tests and logs. Leaving the
//!   server puts this machine's own data back (`reset`).
//!
//! Known shortcuts: the documents are sent whole (every weapon of the set,
//! not only those the match can give: packed they are smaller than most
//! maps); a set the server's data changes mid-match (it doesn't today) is
//! put in while playing, and weapons already built keep their old parts
//! until rebuilt.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    sync::{Arc, OnceLock},
};

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use super::{
    client::{JoinFailure, fail},
    manifest::MatchManifest,
};
use crate::{core::NetRole, weapon::WeaponRegistry};
use chimera_core::mount::assets::map_cache::{self, Key, MapCache, Outcome};
use chimera_sim::server_docs::{self as docs, Docs, ServerDocs};

/// The bytes of a set a server sends per message.
pub const CHUNK: usize = 16 * 1024;
/// At most this many bytes of sets per client per frame.
const PER_FRAME: usize = 4 * CHUNK;
/// A set packed bigger than this isn't sent (the join is refused):
/// `DocsLimit`'s default.
pub const MAX_PACKED: usize = 4 * 1024 * 1024;

/// On a server: the most a set may pack to and still be sent (tests set
/// less).
#[derive(Resource, Clone, Copy, Debug)]
pub struct DocsLimit(pub usize);

impl Default for DocsLimit {
    fn default() -> Self {
        Self(MAX_PACKED)
    }
}
/// A set unpacks to at most this many bytes.
pub const MAX_UNPACKED: u64 = 128 * 1024 * 1024;

/// How the server's and client's refusals over data start
/// (`client::classify`: a `JoinFailure::Mounts`).
pub const DATA: &str = "The server's game data ";

/// A set of documents the server plays with (`MatchManifest::docs`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DocsOffer {
    /// e.g. `combat_arms:weapons`.
    pub name: String,
    /// SHA-256 of the documents as sent (postcard), before packing.
    pub hash: [u8; 32],
    pub count: u32,
    /// Their size as sent, unpacked.
    pub bytes: u64,
}

/// The server's weapon registry (`MatchManifest::registry`): the SHA-256
/// of its ids in order and their count. Zero: not offered (an older
/// manifest).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct RegistryOffer {
    pub hash: [u8; 32],
    pub count: u32,
}

/// Client -> server: send me these sets.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DocsRequest {
    pub names: Vec<String>,
}

/// Server -> client: a piece of set `name`, packed, at `offset` of
/// `total` bytes. Reliable and in order.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DocsChunk {
    pub name: String,
    pub offset: u64,
    pub total: u64,
    pub data: Vec<u8>,
}

/// Server -> client: it won't send a set, and why.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DocsDenied {
    pub reason: String,
}

/// A set's documents as sent: postcard of (name, text) pairs.
pub fn encode(docs: &[(String, String)]) -> Vec<u8> {
    postcard::to_allocvec(docs).expect("strings serialize")
}

/// What is offered of one set (`DocsOffer`), computed once per documents.
fn offer_of(name: &str, docs: &[(String, String)]) -> DocsOffer {
    let bytes = encode(docs);
    DocsOffer {
        name: name.to_string(),
        hash: sha2::Sha256::digest(&bytes).into(),
        count: docs.len() as u32,
        bytes: bytes.len() as u64,
    }
}

/// The registry's offer: every weapon id in order.
pub fn registry_offer(registry: &WeaponRegistry) -> RegistryOffer {
    let mut h = sha2::Sha256::new();
    for d in &registry.0 {
        h.update(d.id.as_bytes());
        h.update([0]);
    }
    RegistryOffer {
        hash: h.finalize().into(),
        count: registry.0.len() as u32,
    }
}

/// A set's documents as sent (`encode`), packed: bzip2 at its best level.
fn pack(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut z = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::best());
    z.write_all(raw).and_then(|()| z.finish()).map_err(|e| e.to_string())
}

/// The packing's version in the converted cache: bump it when `pack`
/// packs otherwise (a client only needs the bytes to unpack to the
/// documents: `unpack`).
const PACKED: &str = "bzip2 best";

/// The least encoded size (bytes) a set's packing is kept from: a
/// game's own documents pack in milliseconds (no entry each).
pub const KEEP_PACKED_FROM: usize = 256 * 1024;

/// The cache a set of `bytes` encoded bytes is kept packed in: the
/// converted cache (chimera_assets' `map_cache`, off with
/// `CHIMERA_MAP_CACHE=0`) for a big set (`KEEP_PACKED_FROM`).
fn pack_disk(bytes: usize) -> Option<MapCache> {
    (bytes >= KEEP_PACKED_FROM).then(MapCache::from_env).flatten()
}

/// `pack(raw)`, read back from `disk` when documents of this `hash` (the
/// offer's: SHA-256 of `raw`) were packed before, and how it went:
/// packing Combat Arms' install set (14 MB) took 20 G instructions of a
/// thread at every server's and host's start (docs/performance.md,
/// "Start cost, fifth pass"). The entry is the packed bytes; it reads no
/// file, so it has no inputs.
pub fn pack_kept(raw: &[u8], hash: &[u8; 32], disk: Option<&MapCache>) -> (Result<Vec<u8>, String>, Outcome) {
    let key = Key::new("server_docs:packed", PACKED).part("sha256", hash);
    match map_cache::cached(disk, &chimera_core::mount::Mount::default(), key, || pack(raw).map(PackedBytes)) {
        Ok((bytes, outcome)) => (Ok(bytes.0), outcome),
        Err(e) => (Err(e), Outcome::Off),
    }
}

/// Packed bytes as an entry: one byte string, not a sequence of bytes.
#[derive(Serialize, Deserialize)]
struct PackedBytes(#[serde(with = "packed_bytes")] Vec<u8>);

mod packed_bytes {
    pub fn serialize<S: serde::Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(v)
    }

    struct Bytes;

    impl serde::de::Visitor<'_> for Bytes {
        type Value = Vec<u8>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("packed bytes")
        }

        fn visit_bytes<E: serde::de::Error>(self, b: &[u8]) -> Result<Vec<u8>, E> {
            Ok(b.to_vec())
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        d.deserialize_bytes(Bytes)
    }
}

/// A set packed for sending (filled by a thread).
#[derive(Default)]
struct Packed(OnceLock<Result<Vec<u8>, String>>);

/// Offers and packs by set, keyed by the documents' allocation (a set put
/// in anew is offered anew).
#[derive(Resource, Default)]
struct DocsCache(BTreeMap<String, (usize, DocsOffer, Arc<Packed>)>);

fn key(d: &Docs) -> usize {
    Arc::as_ptr(d) as usize
}

/// Set `name`'s offer here (and, on a server, its packing started).
fn cached(world: &mut World, name: &str, d: &Docs, pack: bool) -> (DocsOffer, Arc<Packed>) {
    let k = key(d);
    let mut cache = world.get_resource_or_init::<DocsCache>();
    if let Some((at, offer, packed)) = cache.0.get(name)
        && *at == k
    {
        return (offer.clone(), packed.clone());
    }
    let offer = offer_of(name, d);
    let packed = Arc::new(Packed::default());
    if pack {
        let (d, p, n, hash) = (d.clone(), packed.clone(), name.to_string(), offer.hash);
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let raw = encode(&d);
            let (out, kept) = pack_kept(&raw, &hash, pack_disk(raw.len()).as_ref());
            if let Ok(b) = &out {
                info!(
                    "{n}: {} documents, {} bytes, packed {} bytes in {:.2} s{}",
                    d.len(),
                    raw.len(),
                    b.len(),
                    started.elapsed().as_secs_f64(),
                    if kept == Outcome::Hit { " (read back)" } else { "" }
                );
            }
            let _ = p.0.set(out);
        });
    }
    cache.0.insert(name.to_string(), (k, offer.clone(), packed.clone()));
    (offer, packed)
}

/// The server: its sets and registry into the manifest (`manifest::current`).
pub(super) fn offer(world: &mut World, m: &mut MatchManifest) {
    let sets: Vec<(&'static str, Docs)> = world
        .get_resource::<ServerDocs>()
        .map(|s| s.sets().iter().map(|s| (s.name, s.docs.clone())).collect())
        .unwrap_or_default();
    let pack = world.get_resource::<NetRole>() == Some(&NetRole::Server);
    m.docs = sets.iter().map(|(n, d)| cached(world, n, d, pack).0).collect();
    m.registry = world
        .get_resource::<WeaponRegistry>()
        .map(registry_offer)
        .unwrap_or_default();
}

/// On a client: the sets asked for and not yet in, with the registry
/// they must lead to. While it exists the client doesn't enter the game.
#[derive(Resource, Debug, Clone, Default)]
pub struct Pending {
    pub wanted: BTreeMap<String, DocsOffer>,
    pub registry: RegistryOffer,
    /// Packed bytes received so far, by set.
    received: BTreeMap<String, Vec<u8>>,
}

/// On a client: the sets the server sent (name, packed bytes, unpacked
/// bytes, documents), in arrival order.
#[derive(Resource, Debug, Clone, Default)]
pub struct DocsHeard(pub Vec<(String, u64, u64, u32)>);

/// Whether a client is still waiting for the server's data.
pub fn pending(world: &World) -> bool {
    world.contains_resource::<Pending>()
}

/// A client: the server's offer against this machine's data (from
/// `manifest::accept`): ask for each set that differs; with none to ask
/// for, the registries must match. Err: the reason to leave.
pub(super) fn check(world: &mut World, m: &MatchManifest) -> Result<(), String> {
    let mut wanted = BTreeMap::new();
    for o in &m.docs {
        let Some(own) = world
            .get_resource::<ServerDocs>()
            .and_then(|s| s.get(&o.name))
            .map(|s| s.docs.clone())
        else {
            warn!("the server plays with {} data this build doesn't have", o.name);
            continue;
        };
        if cached(world, &o.name, &own, false).0.hash == o.hash {
            continue;
        }
        if o.bytes > MAX_UNPACKED {
            return Err(format!(
                "{DATA}({}) differs from yours and is too big to take ({:.1} MB).",
                o.name,
                o.bytes as f64 / 1048576.0
            ));
        }
        wanted.insert(o.name.clone(), o.clone());
    }
    if let Some(p) = world.get_resource::<Pending>()
        && p.wanted.keys().eq(wanted.keys())
        && p.wanted.values().zip(wanted.values()).all(|(a, b)| a == b)
    {
        // Already asked for (the same offer heard again).
        return Ok(());
    }
    if wanted.is_empty() {
        world.remove_resource::<Pending>();
        return check_registry(world, &m.registry);
    }
    info!(
        "asking the server for its data: {}",
        wanted
            .values()
            .map(|o| format!("{} ({} documents)", o.name, o.count))
            .collect::<Vec<_>>()
            .join(", ")
    );
    world.write_message(DocsRequest {
        names: wanted.keys().cloned().collect(),
    });
    world.insert_resource(Pending {
        wanted,
        registry: m.registry.clone(),
        received: BTreeMap::new(),
    });
    Ok(())
}

/// This client's weapon registry against the server's offer.
fn check_registry(world: &World, offer: &RegistryOffer) -> Result<(), String> {
    if offer.count == 0 && offer.hash == [0; 32] {
        return Ok(());
    }
    let Some(own) = world.get_resource::<WeaponRegistry>().map(registry_offer) else {
        return Ok(());
    };
    if own == *offer {
        return Ok(());
    }
    Err(format!(
        "{DATA}differs from this build's: its weapon list has {} weapons, this build's {} (another build or other \
         games): update the game, or join a server running the same one.",
        offer.count, own.count
    ))
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PreUpdate,
        receive_requests
            .after(ServerSystems::Receive)
            .run_if(in_state(ServerState::Running)),
    )
    .add_systems(
        PostUpdate,
        send_uploads
            .before(ServerSystems::Send)
            .run_if(resource_equals(NetRole::Server)),
    )
    .add_systems(
        PreUpdate,
        receive_chunks
            .after(ClientSystems::Receive)
            .run_if(resource_equals(NetRole::Client)),
    );
}

/// Sets a client asked for, the one being sent first (on its server-side
/// entity).
#[derive(Component, Default)]
struct Upload {
    queue: Vec<(String, Arc<Packed>)>,
    sent: usize,
}

fn receive_requests(world: &mut World) {
    let reqs: Vec<(Entity, DocsRequest)> = world
        .resource_mut::<Messages<FromClient<DocsRequest>>>()
        .drain()
        .filter_map(|r| Some((r.client_id.entity()?, r.message)))
        .collect();
    for (client, r) in reqs {
        let mut queue = Vec::new();
        for name in r.names {
            let Some(d) = world
                .get_resource::<ServerDocs>()
                .and_then(|s| s.get(&name))
                .map(|s| s.docs.clone())
            else {
                warn!("a client asked for {name} data, which this server hasn't");
                continue;
            };
            let (_, packed) = cached(world, &name, &d, true);
            queue.push((name, packed));
        }
        if let Ok(mut e) = world.get_entity_mut(client) {
            e.insert(Upload { queue, sent: 0 });
        }
    }
}

fn send_uploads(world: &mut World) {
    let mut chunks = Vec::new();
    let mut denied = Vec::new();
    let mut done = Vec::new();
    let limit = world.get_resource::<DocsLimit>().copied().unwrap_or_default().0;
    for (client, mut u) in world.query::<(Entity, &mut Upload)>().iter_mut(world) {
        let mut budget = PER_FRAME;
        while budget > 0 {
            let Some((name, packed)) = u.queue.first().cloned() else {
                done.push(client);
                break;
            };
            // Still packing: next frame.
            let Some(bytes) = packed.0.get() else { break };
            let bytes = match bytes {
                Ok(b) if b.len() <= limit => b,
                Ok(b) => {
                    denied.push((
                        client,
                        format!(
                            "{DATA}({name}) differs from yours and is too big to send ({} KB packed, at most {} KB).",
                            b.len().div_ceil(1024),
                            limit / 1024
                        ),
                    ));
                    done.push(client);
                    break;
                }
                Err(e) => {
                    denied.push((client, format!("{DATA}({name}) couldn't be packed on the server: {e}")));
                    done.push(client);
                    break;
                }
            };
            if u.sent == 0 {
                info!("sending {name} to a client: {} bytes packed", bytes.len());
            }
            let end = (u.sent + CHUNK).min(bytes.len());
            chunks.push(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: DocsChunk {
                    name: name.clone(),
                    offset: u.sent as u64,
                    total: bytes.len() as u64,
                    data: bytes[u.sent..end].to_vec(),
                },
            });
            budget = budget.saturating_sub(end - u.sent);
            u.sent = end;
            if u.sent >= bytes.len() {
                u.queue.remove(0);
                u.sent = 0;
            }
        }
    }
    for c in done {
        world.entity_mut(c).remove::<Upload>();
    }
    for (client, reason) in denied {
        warn!("{reason}");
        world.write_message(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: DocsDenied { reason },
        });
    }
    for c in chunks {
        world.write_message(c);
    }
}

fn receive_chunks(world: &mut World) {
    let denied: Vec<DocsDenied> = world.resource_mut::<Messages<DocsDenied>>().drain().collect();
    if let Some(d) = denied.into_iter().next() {
        fail(world, JoinFailure::Mounts, &d.reason);
        return;
    }
    let chunks: Vec<DocsChunk> = world.resource_mut::<Messages<DocsChunk>>().drain().collect();
    for c in chunks {
        let Some(mut p) = world.get_resource_mut::<Pending>() else {
            continue;
        };
        let Some(offer) = p.wanted.get(&c.name).cloned() else {
            continue;
        };
        if c.total as usize > MAX_PACKED {
            continue;
        }
        let buf = p.received.entry(c.name.clone()).or_default();
        if c.offset as usize != buf.len() {
            warn!("{}: a piece at {} where {} was expected", c.name, c.offset, buf.len());
            continue;
        }
        buf.extend_from_slice(&c.data);
        if (buf.len() as u64) < c.total {
            continue;
        }
        let packed = p.received.remove(&c.name).unwrap_or_default();
        p.wanted.remove(&c.name);
        if let Err(reason) = put_in(world, &offer, &packed) {
            fail(world, JoinFailure::Mounts, &reason);
            return;
        }
    }
    let Some(p) = world.get_resource::<Pending>() else {
        return;
    };
    if !p.wanted.is_empty() {
        return;
    }
    let registry = p.registry.clone();
    world.remove_resource::<Pending>();
    if let Err(reason) = check_registry(world, &registry) {
        fail(world, JoinFailure::Mounts, &reason);
    }
}

/// A set that arrived whole: unpacked, checked, put in.
fn put_in(world: &mut World, offer: &DocsOffer, packed: &[u8]) -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut raw = Vec::new();
    bzip2::read::BzDecoder::new(packed)
        .take(MAX_UNPACKED)
        .read_to_end(&mut raw)
        .map_err(|e| format!("{DATA}({}) arrived broken: {e}", offer.name))?;
    let hash: [u8; 32] = sha2::Sha256::digest(&raw).into();
    if hash != offer.hash {
        return Err(format!(
            "{DATA}({}) arrived, but not as offered (its hash differs).",
            offer.name
        ));
    }
    let d: Vec<(String, String)> =
        postcard::from_bytes(&raw).map_err(|e| format!("{DATA}({}) arrived broken: {e}", offer.name))?;
    let count = d.len() as u32;
    docs::apply(world, &offer.name, d).map_err(|e| format!("{DATA}({}) doesn't load here: {e}", offer.name))?;
    info!(
        "{}: the server's {count} documents in ({} bytes packed, {} unpacked, {:.2} s)",
        offer.name,
        packed.len(),
        raw.len(),
        started.elapsed().as_secs_f64()
    );
    world.get_resource_or_init::<DocsHeard>().0.push((
        offer.name.clone(),
        packed.len() as u64,
        raw.len() as u64,
        count,
    ));
    Ok(())
}

/// The client left: its own data back, what came forgotten.
pub(super) fn reset(world: &mut World) {
    world.remove_resource::<Pending>();
    world.remove_resource::<DocsHeard>();
    docs::restore_own(world);
}

/// The sets this client asked for (tests).
pub fn wanted(world: &World) -> BTreeSet<String> {
    world
        .get_resource::<Pending>()
        .map(|p| p.wanted.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A set's packing read back from the converted cache is the packing
    /// made (`pack_kept`): the first call packs and writes the entry, the
    /// second reads it; what a client unpacks is the documents as sent;
    /// other documents are another entry; without a cache it packs.
    #[test]
    fn a_sets_packing_reads_back_from_the_cache_as_packed() {
        let dir = std::env::temp_dir().join(format!("chimera-packed-docs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = MapCache::at(&dir, 1 << 30);
        let docs: Vec<(String, String)> = (0..40).map(|i| (format!("w{i}.toml"), format!("id = \"t:w{i}\"\nslot = {i}\n"))).collect();
        let raw = encode(&docs);
        let hash: [u8; 32] = sha2::Sha256::digest(&raw).into();
        let plain = pack(&raw).unwrap();
        let (first, how) = pack_kept(&raw, &hash, Some(&cache));
        assert_eq!((first.as_ref(), how), (Ok(&plain), Outcome::Miss));
        let (again, how) = pack_kept(&raw, &hash, Some(&cache));
        assert_eq!((again.as_ref(), how), (Ok(&plain), Outcome::Hit), "read back the same bytes");
        let mut unpacked = Vec::new();
        bzip2::read::BzDecoder::new(again.unwrap().as_slice()).read_to_end(&mut unpacked).unwrap();
        assert_eq!(unpacked, raw, "a client unpacks the documents as sent");
        // Other documents: their own entry.
        let other = encode(&docs[..3]);
        let other_hash: [u8; 32] = sha2::Sha256::digest(&other).into();
        assert_eq!(pack_kept(&other, &other_hash, Some(&cache)), (pack(&other), Outcome::Miss));
        assert_eq!(pack_kept(&raw, &hash, None), (Ok(plain), Outcome::Off));
        // Small sets aren't kept at all.
        assert!(pack_disk(KEEP_PACKED_FROM - 1).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
