//! The mount's text files on the install (specs/goldsrc/mount.md):
//! `liblist.gam`'s chain, case-insensitive lookups through it, de_dust2's
//! WADs resolved, and `materials.txt`.

use goldsrc_formats::bsp::{Bsp, wad_names};
use goldsrc_formats::liblist::LibList;
use goldsrc_formats::materials::Materials;

use super::install::{mount, root};

#[test]
fn liblist_chain_and_lookups() {
    let (Some(root), Some(m)) = (root(), mount()) else {
        return;
    };
    let l = LibList::parse(&std::fs::read_to_string(root.join("cstrike/liblist.gam")).unwrap());
    assert_eq!(l.game(), Some("Counter-Strike"));
    assert_eq!(l.get("fallback_dir"), None);
    assert_eq!(l.folder_chain("cstrike", None), ["cstrike", "valve"]);
    assert_eq!(l.get("nohimodel"), Some("1"));
    let layer = |p: &str| m.layer_of(&chimera_vfs::normalize(p)).map(|(i, _)| i);
    assert_eq!(layer("gfx/env/desup.tga"), Some(0));
    assert_eq!(layer("decals.wad"), Some(0));
    assert_eq!(layer("halflife.wad"), Some(1));
    assert_eq!(layer("\\models\\barney01.mdl"), Some(1));
    let b = Bsp::parse(&m.read("maps/de_dust2.bsp").unwrap()).unwrap();
    let wads: Vec<_> = wad_names(&b.entities()[0]).iter().map(|w| layer(w)).collect();
    assert_eq!(wads, [Some(1), Some(0), Some(0)]);
}

#[test]
fn materials_from_cstrike() {
    let Some(m) = mount() else { return };
    let mat = Materials::parse(&String::from_utf8_lossy(&m.read("sound/materials.txt").unwrap()));
    assert_eq!(mat.entries.len(), 701);
    assert_eq!(mat.surface("some_unlisted_texture"), 'C');
}
