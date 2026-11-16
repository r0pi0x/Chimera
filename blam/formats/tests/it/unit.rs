//! `blam_formats::unit` on a synthetic map (specs/halo_ce/characters.md,
//! section 8): references found by their shape (a look-alike with the
//! wrong path address is not one), the closure with the spec's exclusions
//! and a dialogue left out for the voice, the spans measured, and the cut
//! map file loading with the unit's bytes kept, the rest zero, the same
//! bytes every time.

use blam_formats::{
    cache::{MapFile, TAG_BASE, class},
    unit::{self, character_excluded},
};

use super::map_fixture::{CacheFile, Struct, TagData, tag_ref_bytes};

/// Tags: 0 scenario, 1 biped, 2 effect, 3 weapon, 4 dialogue, 5 damage
/// effect, 6 actor variant, 7 decal (only a look-alike points at it).
fn map() -> MapFile {
    let mut td = TagData::new(&[
        ("scnr", "levels\\x"),
        ("bipd", "characters\\elite\\elite"),
        ("effe", "effects\\blood"),
        ("weap", "weapons\\plasma rifle"),
        ("udlg", "sound\\dialog\\elite"),
        ("jpt!", "characters\\elite\\melee"),
        ("actv", "characters\\elite\\minor"),
        ("deca", "decals\\stray"),
    ]);
    let (r2, r3, r4, r5, r6) = (td.tag_ref(2), td.tag_ref(3), td.tag_ref(4), td.tag_ref(5), td.tag_ref(6));
    let r1 = td.tag_ref(1);
    let decoy = tag_ref_bytes(super::map_fixture::c("deca"), TAG_BASE + 4, td.id(7));
    td.set_struct(0, &Struct::new(0x20).0);
    td.set_struct(1, &Struct::new(0x60).u32(0, 0xB1B1).put(0x10, &r2).put(0x24, &r3).put(0x34, &r4).put(0x48, &decoy).0);
    td.set_struct(2, &Struct::new(0x30).put(0x00, &r5).put(0x10, &r1).0);
    td.set_struct(3, &Struct::new(0x20).put(0x00, &r6).0);
    td.set_struct(4, &Struct::new(0x20).put(0x08, &r5).0);
    td.set_struct(5, &Struct::new(0x10).u32(0, 0x55).0);
    td.set_struct(6, &Struct::new(0x10).u32(0, 0x66).0);
    td.set_struct(7, &Struct::new(0x10).u32(0, 0x77).0);
    MapFile::load(&CacheFile::new("x", 0, td.finish()).file()).unwrap()
}

#[test]
fn references_are_found_by_shape() {
    let m = map();
    let spans = unit::Spans::of(&m);
    assert_eq!(unit::references(&m, &spans, 1), vec![2, 3, 4]);
    assert_eq!(unit::references(&m, &spans, 2), vec![5, 1]);
}

#[test]
fn the_closure_leaves_out_actors_weapons_and_what_only_they_reach() {
    let m = map();
    let all = unit::closure(&m, &[1], |t| !character_excluded(t));
    assert_eq!(all.into_iter().collect::<Vec<_>>(), vec![1, 2, 4, 5]);
    let body = unit::closure(&m, &[1], |t| !character_excluded(t) && t.class != class::DIALOGUE);
    assert_eq!(body.iter().copied().collect::<Vec<_>>(), vec![1, 2, 5]);
    let u = unit::measure(&m, body).unwrap();
    let spans = unit::Spans::of(&m);
    let sum: u64 = [1u16, 2, 5].iter().map(|&i| spans.span(&m, i).map_or(0, |(a, b)| u64::from(b - a))).sum();
    assert_eq!(u.tag_bytes, sum);
    assert_eq!(u.raw_bytes, 0);
}

#[test]
fn a_cut_keeps_the_unit_and_zeroes_the_rest() {
    let m = map();
    let body = unit::closure(&m, &[1], |t| !character_excluded(t) && t.class != class::DIALOGUE);
    let u = unit::measure(&m, body).unwrap();
    let file = unit::cut(&m, &u).unwrap();
    assert_eq!(file, unit::cut(&m, &u).unwrap(), "deterministic");
    let cut = MapFile::load(&file).unwrap();
    assert_eq!(cut.tags().len(), m.tags().len());
    for (a, b) in cut.tags().iter().zip(m.tags()) {
        assert_eq!((a.id, &a.path, a.address), (b.id, &b.path, b.address));
    }
    let word = |map: &MapFile, i: usize| map.tag_view(&map.tags()[i], 4).unwrap().u32(0).unwrap();
    assert_eq!(word(&cut, 1), 0xB1B1);
    assert_eq!(word(&cut, 5), 0x55);
    assert_eq!(word(&cut, 6), 0, "the actor variant is not in the unit");
    assert_eq!(word(&cut, 7), 0);
    // The index is whole: the biped still refers to its effect (and to the
    // weapon and dialogue, whose bytes are zero).
    let spans = unit::Spans::of(&cut);
    assert_eq!(unit::references(&cut, &spans, 1), vec![2, 3, 4]);
}
