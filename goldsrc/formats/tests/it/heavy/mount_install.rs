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

/// CS 1.6's lookup on the install's file (specs/cs16/footsteps.md, test
/// cases and open question 3): 1024 entries read (all 701), the doubly
/// listed `generic015v` metal, prefixed entries dead, `cudumpsterc` (past
/// Half-Life's 512th entry) metal.
#[test]
fn cs16_materials_lookup_on_the_install() {
    use goldsrc_formats::materials::Rule;
    let Some(m) = mount() else { return };
    let text = String::from_utf8_lossy(&m.read("sound/materials.txt").unwrap()).into_owned();
    let mat = Materials::parse_with(&text, Rule::Cs16);
    assert_eq!(mat.entries.len(), 701);
    assert_eq!(mat.surface("generic015v"), 'M', "the sorted search lands on the M entry");
    assert_eq!(mat.surface("GENERIC015V"), 'M');
    assert_eq!(mat.surface("-0cstrike_me1pl"), 'C');
    assert_eq!(mat.surface("cstrike_me1pl"), 'C');
    assert_eq!(mat.surface("a_texture_in_no_entry"), 'C');
    // Every unprefixed entry finds a letter (its own, or its twin's).
    for (letter, name) in &mat.entries {
        if !name.starts_with(['-', '+']) {
            let got = mat.surface(name);
            assert!(got == *letter || mat.entries.iter().any(|(l, n)| n == name && *l == got), "{name}");
        }
    }
    let at = |n: &str| text.lines().filter_map(|l| l.split_whitespace().nth(1).map(|x| (l, x))).find(|(_, x)| x.eq_ignore_ascii_case(n)).map(|(l, _)| l.trim().chars().next().unwrap());
    assert_eq!(mat.surface("cudumpsterc"), at("cudumpsterc").unwrap_or('C'));
    // Half-Life's 12 characters still apply to its own rule.
    let hl = Materials::parse_with(&text, Rule::HalfLife);
    assert_eq!(hl.entries.len(), 701);
}
