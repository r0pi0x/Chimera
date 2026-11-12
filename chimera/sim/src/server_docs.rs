//! Server data documents (docs/plans/active/multiplayer.md, "Server data
//! documents"): data a game derives from what this machine has (Combat
//! Arms' ~2,300 weapon documents merged from the install's attribute file,
//! which needs a key; its attachment records; its movement numbers) and
//! which a client without that data, or with another install version,
//! would derive differently: other weapons at other registry indices (the
//! network names weapons by index), other clips, cycles and spreads.
//!
//! - **Sets** (`DocsAppExt::server_docs`, `server_weapon_docs`): a game
//!   registers each of its derived data as a named set of documents
//!   (name, text: the data-driven TOML the game builds from), in the
//!   order that matters (a weapon set's is the registry's), with how a
//!   client puts another set in place (`DocApply`). Kept sorted by name
//!   (`ServerDocs`).
//! - **The network** (chimera_net's `net::server_docs`) offers each set's
//!   hash in the match manifest; a client whose own set differs (or has
//!   none) asks for the server's and puts it in (`apply`) before it
//!   enters the game, so its registry indices and numbers are the
//!   server's. Only numbers and names travel: models, textures and sounds
//!   are still the client's own install's (a missing one draws the
//!   game's fallback).
//! - A weapon set (`DocApply::Weapons`) is put in by
//!   `weapon::data::replace_weapon_set`: the set's weapons leave the
//!   registry and the server's take their place, at the same position
//!   (the games registered after keep theirs, at the server's indices as
//!   long as the set's length is the server's). Other sets name a
//!   function (`DocApply::With`) that replaces the game's resource, and
//!   the type it reads them into, whose format (`data_format`) a client
//!   and server compare at the join.
//! - `restore_own` puts every replaced set back to this machine's own
//!   (the client left the server).
//!
//! Known shortcuts: each weapon set put in is a new leaked `WeaponSet`
//! (weapon ids are `&'static str`), once per join to a server whose data
//! differs (~15 MB for Combat Arms' install set); a game's code that read
//! its own data at build (preloads, icons, model lookups by its own
//! documents) keeps what it read.

use std::sync::Arc;

use bevy::prelude::*;

use crate::weapon::data::{WeaponSet, replace_weapon_set, swap_weapon_set};

/// A set's documents: (name, text), in order.
pub type Docs = Arc<Vec<(String, String)>>;

/// How a client puts the server's documents of a set in place of its own.
#[derive(Clone, Copy)]
pub enum DocApply {
    /// A weapon set: this world's current set of it (replaced by
    /// `replace_weapon_set`).
    Weapons(&'static WeaponSet),
    /// Anything else: parse the documents and replace the game's
    /// resources (Err: why they can't be used), and the documents' format
    /// (`data_format::describe` of the type they're read into), which the
    /// join compares with the server's.
    With(fn(&mut World, &[(String, String)]) -> Result<(), String>, fn() -> String),
}

/// One named set (`DocsAppExt`).
#[derive(Clone)]
pub struct DocSet {
    /// Namespaced, e.g. `combat_arms:weapons`.
    pub name: &'static str,
    /// The documents this world plays with now.
    pub docs: Docs,
    pub apply: DocApply,
    /// This machine's own documents and apply, while the server's are in
    /// (`restore_own`).
    own: Option<(Docs, DocApply)>,
}

impl DocSet {
    /// Whether the server's documents are in place of this machine's.
    pub fn replaced(&self) -> bool {
        self.own.is_some()
    }

    /// The documents' total text length, bytes.
    pub fn text_len(&self) -> usize {
        self.docs.iter().map(|(n, t)| n.len() + t.len()).sum()
    }
}

/// The registered sets, sorted by name.
#[derive(Resource, Default, Clone)]
pub struct ServerDocs(Vec<DocSet>);

impl ServerDocs {
    pub fn sets(&self) -> &[DocSet] {
        &self.0
    }

    pub fn get(&self, name: &str) -> Option<&DocSet> {
        self.0.iter().find(|s| s.name == name)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn insert(&mut self, set: DocSet) {
        match self.0.binary_search_by_key(&set.name, |s| s.name) {
            Ok(i) => self.0[i] = set,
            Err(i) => self.0.insert(i, set),
        }
    }
}

/// Registering a game's derived data as server documents.
pub trait DocsAppExt {
    /// Set `name`'s documents and how a client puts the server's in. A
    /// name registered again replaces the first.
    fn server_docs(&mut self, name: &'static str, docs: Vec<(String, String)>, apply: DocApply) -> &mut Self;

    /// A weapon set (already added: `add_weapon_set`) as the documents it
    /// was parsed from, in its order.
    fn server_weapon_docs(
        &mut self,
        name: &'static str,
        set: &'static WeaponSet,
        docs: Vec<(String, String)>,
    ) -> &mut Self {
        self.server_docs(name, docs, DocApply::Weapons(set))
    }
}

impl DocsAppExt for App {
    fn server_docs(&mut self, name: &'static str, docs: Vec<(String, String)>, apply: DocApply) -> &mut Self {
        if let DocApply::With(_, format) = apply {
            let format = format();
            self.world_mut()
                .get_resource_or_init::<crate::data_format::DataFormats>()
                .0
                .insert(name.to_string(), format);
        }
        self.world_mut().get_resource_or_init::<ServerDocs>().insert(DocSet {
            name,
            docs: Arc::new(docs),
            apply,
            own: None,
        });
        self
    }
}

/// Put the server's documents of set `name` in place of this world's
/// (a client, before it enters the game). Err: no such set here, or the
/// documents don't load (this world's stay).
pub fn apply(world: &mut World, name: &str, docs: Vec<(String, String)>) -> Result<(), String> {
    let set = world
        .get_resource::<ServerDocs>()
        .and_then(|s| s.get(name))
        .cloned()
        .ok_or_else(|| format!("this build has no {name} data"))?;
    let apply = put(world, set.apply, &docs)?;
    let mut all = world.resource_mut::<ServerDocs>();
    let s = all.0.iter_mut().find(|s| s.name == name).expect("found above");
    if s.own.is_none() {
        s.own = Some((s.docs.clone(), s.apply));
    }
    s.docs = Arc::new(docs);
    s.apply = apply;
    Ok(())
}

fn put(world: &mut World, how: DocApply, docs: &[(String, String)]) -> Result<DocApply, String> {
    match how {
        DocApply::Weapons(old) => replace_weapon_set(world, old, docs).map(DocApply::Weapons),
        DocApply::With(f, _) => f(world, docs).map(|()| how),
    }
}

/// Every set the server's documents replaced back to this machine's own
/// (the client left).
pub fn restore_own(world: &mut World) {
    let Some(sets) = world.get_resource::<ServerDocs>().map(|s| s.0.clone()) else {
        return;
    };
    for s in sets {
        let Some((docs, own)) = s.own else { continue };
        let back = match (s.apply, own) {
            // The own set is still parsed: swapped back, not parsed again.
            (DocApply::Weapons(now), DocApply::Weapons(mine)) => swap_weapon_set(world, now, mine),
            (_, DocApply::With(f, _)) => f(world, &docs),
            _ => Ok(()),
        };
        if let Err(e) = back {
            warn!("{}: this machine's own data doesn't load again: {e}", s.name);
        }
        let restored = own;
        let mut all = world.resource_mut::<ServerDocs>();
        if let Some(x) = all.0.iter_mut().find(|x| x.name == s.name) {
            x.docs = docs;
            x.apply = restored;
            x.own = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::{WeaponRegistry, data::WeaponDataAppExt};

    fn doc(id: &str) -> (String, String) {
        (format!("{id}.toml"), format!("id = \"{id}\"\nslot = 1\ndraw = 1.0\n"))
    }

    fn ids(w: &World) -> Vec<&'static str> {
        w.resource::<WeaponRegistry>().0.iter().map(|d| d.id).collect()
    }

    /// The server's set takes the place of this machine's in the registry
    /// (the sets after it shift as on the server), and leaving puts this
    /// machine's back.
    #[test]
    fn a_set_is_replaced_in_place_and_put_back() {
        let mut app = App::new();
        app.add_weapon_data(&[("a.toml", "id = \"x:first\"\nslot = 1\ndraw = 1.0\n")])
            .unwrap();
        let own_docs = vec![doc("g:a"), doc("g:b")];
        let refs: Vec<(&str, &str)> = own_docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let set = app.add_weapon_data(&refs).unwrap();
        app.add_weapon_data(&[("z.toml", "id = \"x:last\"\nslot = 1\ndraw = 1.0\n")])
            .unwrap();
        app.server_weapon_docs("g:weapons", set, own_docs);
        let world = app.world_mut();
        assert_eq!(ids(world), ["x:first", "g:a", "g:b", "x:last"]);
        let theirs = vec![doc("g:b"), doc("g:c"), doc("g:d")];
        apply(world, "g:weapons", theirs.clone()).unwrap();
        assert_eq!(ids(world), ["x:first", "g:b", "g:c", "g:d", "x:last"]);
        let s = world.resource::<ServerDocs>().get("g:weapons").unwrap();
        assert!(s.replaced());
        assert_eq!(*s.docs, theirs);
        // Bad documents change nothing.
        assert!(apply(world, "g:weapons", vec![("bad.toml".into(), "id = 3".into())]).is_err());
        assert_eq!(ids(world), ["x:first", "g:b", "g:c", "g:d", "x:last"]);
        restore_own(world);
        assert_eq!(ids(world), ["x:first", "g:a", "g:b", "x:last"]);
        assert!(!world.resource::<ServerDocs>().get("g:weapons").unwrap().replaced());
    }
}
