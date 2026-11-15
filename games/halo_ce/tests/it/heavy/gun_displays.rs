//! The displays on Halo's guns from the disc (`game::view::displays`,
//! specs/halo_ce/weapons.md 18 and its disc-backed cases): the assault
//! rifle's first-person model has its two digit parts on the numeric
//! `numbers` shader (positions 0 and 1, limit 60) with the 10-image
//! `numbers_plate` and its compass on the 8-image `compass_plate` (limit
//! 8), and a shot of the rifle's own tick changes the units image from 0
//! to 9 through the weapon tag's own exports and functions; the plasma
//! rifle's and plasma pistol's gauges are meters with their baked cuts;
//! the memo made after the map cache (`load::displays_of`) is the same,
//! with bloodgulch's local north (pi).
//! Passes with a note without an install.

use blam_formats::cache::MapFile;
use halo_ce::game::{
    body::load::DONOR_MAP,
    maps,
    view::{
        displays::{self, Display, Functions, METER_LEVELS},
        load,
    },
    weapons::{
        ASSAULT_RIFLE, PLASMA_PISTOL, PLASMA_RIFLE, gun,
        tick::{GunEvent, HaloGun, Inputs},
    },
};

use super::maps::mount;

fn bloodgulch() -> Option<MapFile> {
    let bytes = mount()?.read(&maps::map_path(DONOR_MAP)).unwrap();
    Some(MapFile::load(&bytes).unwrap())
}

#[test]
fn the_assault_rifles_counter_counts_its_rounds() {
    let Some(file) = bloodgulch() else { return };
    let looks = load::looks(&file, false);
    let model = looks.view_models.iter().find(|v| v.key == ASSAULT_RIFLE).expect("the rifle's view model");
    let program = looks.displays.guns.get(ASSAULT_RIFLE).expect("the rifle's displays");
    let numeric: Vec<(usize, u8, u16, u16)> = program
        .parts
        .iter()
        .filter_map(|&(i, d)| match d {
            Display::Numeric { limit, position, images } => Some((i, limit, position, images)),
            Display::Meter { .. } => None,
        })
        .collect();
    let digits: Vec<_> = numeric.iter().filter(|p| p.3 == 10).collect();
    assert_eq!(digits.len(), 2, "{numeric:?}");
    let mut positions: Vec<u16> = digits.iter().map(|p| p.2).collect();
    positions.sort();
    assert_eq!(positions, [0, 1], "units and tens");
    for &&(i, limit, _, _) in &digits {
        assert_eq!(limit, 60);
        let mesh = &model.model.meshes[i];
        assert!(mesh.material.ends_with("fp/shaders/numbers"), "{}", mesh.material);
        assert_eq!(mesh.frames.len(), 10);
        let first = &looks.textures[mesh.frames[0]];
        assert!(first.name.contains("numbers_plate"), "{}", first.name);
        assert_eq!((first.width, first.height), (64, 64));
    }
    let dial: Vec<_> = numeric.iter().filter(|p| p.3 == 8).collect();
    assert_eq!(dial.len(), 1, "{numeric:?}");
    assert_eq!(dial[0].1, 8);
    assert!(model.model.meshes[dial[0].0].material.ends_with("fp/shaders/compass"));
    assert_eq!(program.inputs[3], 19, "object input D: compass");
    assert_eq!(program.exports[1], 2, "weapon input B: primary ammunition");

    // A shot on the rifle's own tick: 60, then 59.
    let g = gun(ASSAULT_RIFLE);
    let mut state = HaloGun::new(&g);
    let mut f = Functions::default();
    let units = digits.iter().find(|p| p.2 == 0).unwrap().0;
    let mut shown = |state: &HaloGun| {
        displays::step(program, &|c| displays::export(c, &g, state), None, displays::tables(), &mut f);
        displays::frames(program, &f.outgoing).into_iter().find(|p| p.0 == units).unwrap().1
    };
    assert_eq!(shown(&state), 0, "60 rounds");
    let inputs = Inputs { fire: true, held: true, aim: bevy::math::Vec3::X, ..Inputs::default() };
    let fired = (0..120).any(|_| state.tick(&g, &inputs).iter().any(|e| matches!(e, GunEvent::Fired { .. })));
    assert!(fired, "the rifle fired");
    assert_eq!(state.loaded, 59);
    assert_eq!(shown(&state), 9, "59 rounds on the tick of the shot");
}

#[test]
fn the_plasma_gauges_are_meters() {
    let Some(file) = bloodgulch() else { return };
    let looks = load::looks(&file, false);
    for (id, source) in [(PLASMA_RIFLE, 2), (PLASMA_PISTOL, 4)] {
        let program = looks.displays.guns.get(id).unwrap_or_else(|| panic!("{id}: no displays"));
        let model = looks.view_models.iter().find(|v| v.key == id).unwrap();
        let meters: Vec<_> = program.parts.iter().filter(|(_, d)| *d == Display::Meter { source }).collect();
        assert_eq!(meters.len(), 1, "{id}: {:?}", program.parts);
        assert_eq!(model.model.meshes[meters[0].0].frames.len(), METER_LEVELS + 1, "{id}");
    }
}

#[test]
fn the_memo_after_the_cache_matches_the_looks() {
    let Some(file) = bloodgulch() else { return };
    let looks = load::looks(&file, false);
    let memo = load::displays_of(&file);
    assert_eq!(memo.guns, looks.displays.guns);
    assert!((memo.local_north - std::f32::consts::PI).abs() < 1e-3, "{}", memo.local_north);
}
