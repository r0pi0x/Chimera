//! The data formats check at the join (docs/plans/active/multiplayer.md,
//! "Server data documents"; chimera_sim's `data_format`): a client says
//! in its userinfo (`FORMATS_KEY`) the format of each data it reads from a
//! server (the weapon document, each weapon hook's parameters, each other
//! document set: `data_format::formats`, as names and `fingerprint`s), and
//! the server refuses it at once, before the map, the files or the data
//! travel, when a format both have differs, or the client has data of a
//! game (a name's namespace) but not one of the server's for it: the
//! server's documents wouldn't load there (a newer server's
//! `combat_arms:view_model` with a field an older client rejects, the
//! 2026-11-12 rehearsal). The reason starts with `FORMAT` and names the
//! data (`client::classify`: `JoinFailure::Protocol`).
//!
//! A client that sends no formats (an older build) isn't checked here
//! (its version is: `NET_VERSION`).
//!
//! Known shortcuts: the list travels as one userinfo value (~30 names,
//! ~1 KB in a party app), every join.

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_sim::data_format;

/// The userinfo key a client's formats travel under.
pub const FORMATS_KEY: &str = "_formats";

/// How a refusal over formats starts.
pub const FORMAT: &str = "This game reads the server's game data in another format";

/// This world's formats, by name: their fingerprints.
pub fn fingerprints(world: &World) -> BTreeMap<String, u32> {
    data_format::formats(world)
        .into_iter()
        .map(|(name, f)| (name, data_format::fingerprint(&f)))
        .collect()
}

/// `name=xxxxxxxx,...` (hex fingerprints).
pub fn encode(prints: &BTreeMap<String, u32>) -> String {
    prints
        .iter()
        .map(|(n, p)| format!("{n}={p:08x}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// `encode`'s text back (bad entries skipped).
pub fn decode(text: &str) -> BTreeMap<String, u32> {
    text.split(',')
        .filter_map(|e| {
            let (n, p) = e.split_once('=')?;
            Some((n.to_string(), u32::from_str_radix(p, 16).ok()?))
        })
        .collect()
}

/// A client's formats from its userinfo (None: it sent none).
pub fn from_userinfo<'a>(mut info: impl Iterator<Item = (&'a str, &'a str)>) -> Option<BTreeMap<String, u32>> {
    info.find(|(k, _)| k.eq_ignore_ascii_case(FORMATS_KEY))
        .map(|(_, v)| decode(v))
}

fn namespace(name: &str) -> &str {
    name.split_once(':').map_or("", |(ns, _)| ns)
}

/// Err: why a client with `client`'s formats can't read the data of a
/// server with `server`'s.
pub fn check(server: &BTreeMap<String, u32>, client: &BTreeMap<String, u32>) -> Result<(), String> {
    let differ: Vec<&str> = server
        .iter()
        .filter(|(name, print)| match client.get(*name) {
            Some(theirs) => theirs != *print,
            // A game whose data the client reads, without this of it.
            None => client.keys().any(|k| namespace(k) == namespace(name)),
        })
        .map(|(name, _)| name.as_str())
        .collect();
    if differ.is_empty() {
        return Ok(());
    }
    const SHOWN: usize = 4;
    let more = if differ.len() > SHOWN {
        format!(" and {} more", differ.len() - SHOWN)
    } else {
        String::new()
    };
    Err(format!(
        "{FORMAT} ({}{more}): the server and this game are different builds of the same version. Run the server's build.",
        differ[..differ.len().min(SHOWN)].join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(items: &[(&str, u32)]) -> BTreeMap<String, u32> {
        items.iter().map(|(n, p)| (n.to_string(), *p)).collect()
    }

    #[test]
    fn formats_travel_and_compare() {
        let server = m(&[
            ("chimera:weapon", 1),
            ("combat_arms:view_model", 2),
            ("cs_source:recoil", 3),
        ]);
        assert_eq!(decode(&encode(&server)), server);
        assert!(check(&server, &server).is_ok());
        // Another view-model format: refused, by name.
        let older = m(&[
            ("chimera:weapon", 1),
            ("combat_arms:view_model", 9),
            ("cs_source:recoil", 3),
        ]);
        let why = check(&server, &older).unwrap_err();
        assert!(
            why.starts_with(FORMAT) && why.contains("combat_arms:view_model"),
            "{why}"
        );
        // A client without Combat Arms at all: the manifest decides.
        assert!(check(&server, &m(&[("chimera:weapon", 1), ("cs_source:recoil", 3)])).is_ok());
        // With Combat Arms but without one of its hooks: refused.
        let why = check(
            &server,
            &m(&[("chimera:weapon", 1), ("combat_arms:other", 5), ("cs_source:recoil", 3)]),
        );
        assert!(why.unwrap_err().contains("combat_arms:view_model"));
        // The client's extra data: the server's documents don't use it.
        let mut newer = server.clone();
        newer.insert("combat_arms:new".into(), 7);
        assert!(check(&server, &newer).is_ok());
    }
}
