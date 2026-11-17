//! Each game's guest dressing as a converted asset
//! (docs/plans/active/converted-cache.md): converted twice from the same
//! files it serializes (postcard, as the converted-map cache writes it) to
//! the same bytes, the generated-assets rule's "same input, same output"
//! (step 1: the `HashMap`s a map holds are written in key order,
//! chimera_world's `pod_serde::sorted`).
//!
//! Each game passes with a note without its install (CS:S, HL2:DM, TF2,
//! Combat Arms with its keys).

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::MapData;

/// The guests whose install is here, each with its dressing of an empty
/// map (textures decoded, its kept guns), as its kept dressing converts
/// it.
fn guests() -> Vec<(&'static str, Box<dyn Fn() -> MapData>)> {
    let mut out: Vec<(&'static str, Box<dyn Fn() -> MapData>)> = Vec::new();
    let empty = || MapData::default();
    if cs_source::game::INFO.install().is_some() {
        out.push((
            cs_source::game::GAME,
            Box::new(move || {
                let mut m = empty();
                cs_source::game::dress_guest_with(&mut m, cs_source::game::BUNDLE.kept, false).unwrap();
                m
            }),
        ));
    }
    if hl2dm::game::INFO.install().is_some() {
        out.push((
            hl2dm::game::GAME,
            Box::new(move || {
                let mut m = empty();
                hl2dm::game::maps::dress_guest_with(&mut m, hl2dm::game::BUNDLE.kept, false).unwrap();
                m
            }),
        ));
    }
    if tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf")) {
        out.push((
            tf2::game::GAME,
            Box::new(move || {
                let mut m = empty();
                tf2::game::maps::dress_guest_with(&mut m, tf2::game::BUNDLE.kept, false).unwrap();
                m
            }),
        ));
    }
    let ca = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(combat_arms::game::GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if ca {
        out.push((
            combat_arms::game::GAME,
            Box::new(move || {
                let data = combat_arms::game::data::CaData::install();
                let mount = data.mount().expect("Combat Arms' install mounts");
                let mut m = empty();
                combat_arms::game::world::dress(&data, &mount, &mut m);
                m
            }),
        ));
    }
    out
}

/// The parts of a map's bytes, in the order they're written, for a
/// failure that says where two encodings part.
fn parts(map: &MapData) -> Vec<(&'static str, Vec<u8>)> {
    let enc = |v: &dyn erased::Ser| v.bytes();
    vec![
        ("textures", enc(&map.textures)),
        ("sounds", enc(&*map.sounds)),
        ("kit", enc(&map.kit)),
        ("particles", enc(&map.particles)),
        ("impacts", enc(&map.impacts)),
        ("surface_names", enc(&map.surface_names)),
    ]
}

#[test]
fn a_guest_dressing_converted_twice_writes_the_same_bytes() {
    let guests = guests();
    if guests.is_empty() {
        eprintln!("skipping: needs an install of CS:S, HL2:DM, TF2 or Combat Arms");
        return;
    }
    for (game, dress) in guests {
        let (a, b) = (dress(), dress());
        for ((name, x), (_, y)) in parts(&a).iter().zip(parts(&b)) {
            let first = x.iter().zip(&y).position(|(p, q)| p != q);
            assert!(
                x.len() == y.len() && first.is_none(),
                "{game}: its {name} encode differently ({} and {} bytes, first difference at {first:?})",
                x.len(),
                y.len()
            );
        }
        let (x, y) = (postcard::to_allocvec(&a).unwrap(), postcard::to_allocvec(&b).unwrap());
        assert!(x == y, "{game}: the whole dressing encodes differently ({} and {} bytes)", x.len(), y.len());
        eprintln!("{game}: {} bytes, the same twice", x.len());
    }
}

mod erased {
    /// A value's postcard bytes.
    pub trait Ser {
        fn bytes(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn bytes(&self) -> Vec<u8> {
            postcard::to_allocvec(self).unwrap()
        }
    }
}
