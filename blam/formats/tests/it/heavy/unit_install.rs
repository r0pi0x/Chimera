//! Units cut from the user's Halo disc image (specs/halo_ce/characters.md,
//! section 8 and test case (b) 8): the Elite's closure on d20 has the
//! spec's tag count and sizes, and the map file `unit::cut` writes loads
//! as a map whose biped, model and graph read as on the campaign map.
//! Skipped (with a note) without an image.

use blam_formats::{
    animation::AnimationGraph,
    cache::{MapFile, class},
    model::Model,
    object::Biped,
    unit::{self, character_excluded},
};

use super::maps::load;

const ELITE: &str = "characters\\elite\\elite";

fn elite_unit(map: &MapFile) -> unit::Unit {
    let biped = map.find(class::BIPED, ELITE).expect("the Elite on d20");
    let tags = unit::closure(map, &[biped.index()], |t| !character_excluded(t));
    unit::measure(map, tags).unwrap()
}

#[test]
fn elite_closure_matches_the_spec() {
    let Some(map) = load("d20") else { return };
    let u = elite_unit(&map);
    eprintln!(
        "elite: {} tags, tag data {}, raw {}, total {}, sound {}; {:?}",
        u.tags.len(),
        u.tag_bytes,
        u.raw_bytes,
        u.total(),
        u.sound_bytes,
        u.classes
    );
    assert_eq!(u.tags.len(), 161);
    assert_eq!(u.tag_bytes, 1_013_828);
    assert_eq!(u.raw_bytes, 5_668_994);
    assert_eq!(u.total(), 6_682_822);
    assert_eq!(u.sound_bytes, 4_503_360);
}

/// The body without its voice (the dialogue and what only it reaches), and
/// the voice: together the whole closure.
#[test]
fn body_and_voice_split_the_closure() {
    let Some(map) = load("d20") else { return };
    let whole = elite_unit(&map);
    let biped = map.find(class::BIPED, ELITE).unwrap().index();
    let body = unit::closure(&map, &[biped], |t| !character_excluded(t) && t.class != class::DIALOGUE);
    let dialogue: Vec<u16> = whole.tags.iter().copied().filter(|&i| map.tags()[i as usize].class == class::DIALOGUE).collect();
    assert_eq!(dialogue.len(), 1);
    let voice: std::collections::BTreeSet<u16> =
        unit::closure(&map, &dialogue, |t| !character_excluded(t)).difference(&body).copied().collect();
    let (b, v) = (unit::measure(&map, body).unwrap(), unit::measure(&map, voice).unwrap());
    eprintln!("body: {} tags, {} bytes; voice: {} tags, {} bytes", b.tags.len(), b.total(), v.tags.len(), v.total());
    assert_eq!(b.tags.len() + v.tags.len(), 161);
    assert_eq!(b.total() + v.total(), 6_682_822);
}

#[test]
fn a_cut_unit_loads_as_a_map() {
    let Some(map) = load("d20") else { return };
    let u = elite_unit(&map);
    let file = unit::cut(&map, &u).unwrap();
    let again = unit::cut(&map, &u).unwrap();
    assert_eq!(file, again, "the cut is deterministic");
    let cut = MapFile::load(&file).unwrap();
    eprintln!("cut: {} bytes on disk, image {} bytes", file.len(), cut.image().len());
    let b = cut.find(class::BIPED, ELITE).unwrap();
    let biped = Biped::read(&cut, b).unwrap();
    let model = Model::read(&cut, cut.tag(biped.object.model.id).unwrap()).unwrap();
    let original = Model::read(&map, map.tag(biped.object.model.id).unwrap()).unwrap();
    assert_eq!(model, original);
    let graph = AnimationGraph::read(&cut, cut.tag(biped.object.animation_graph.id).unwrap()).unwrap();
    assert!(!graph.animations.is_empty());
}
