//! The animation graph reader and sampler on the user's Halo disc image,
//! against specs/halo_ce/animations.md's "Measured facts", "Verification"
//! and "Disc-backed" tables: the first-person graphs' slots with frame,
//! key and loop frames, the Spartan's labels and animation tables, its
//! first poses, and the self-checks (the Spartan's idle and run composed
//! with its model's skeleton keep every segment length and put the head
//! above the pelvis; every animation of bloodgulch samples to unit
//! quaternions; the first-person graphs' nodes match their models' by
//! name; a10's compressed crewman animations decode). Each test passes
//! with a note when there is no image.

// The spec states its angles as 1.5708: the tests use them as written.
#![allow(clippy::approx_constant)]

use std::sync::OnceLock;

use blam_formats::animation::{
    Animation, AnimationGraph, AnimationType, BodyPart, DamageDirection, DamageKind, FirstPersonSlot, FrameInfoType,
    Pose, UnitDamageSlot, UnitSelection, WeaponClassSlot, WeaponTypeSlot, decode_compressed_rotation, quat_length,
    quat_normalize,
};
use blam_formats::cache::{MapFile, class};
use blam_formats::model::Model;

use super::disc_install::{SKIP, image_path};
use super::maps::{MULTIPLAYER, load};

fn bloodgulch() -> Option<&'static MapFile> {
    static MAP: OnceLock<Option<MapFile>> = OnceLock::new();
    MAP.get_or_init(|| load("bloodgulch")).as_ref()
}

fn graph(map: &MapFile, path: &str) -> AnimationGraph {
    let tag = map
        .find(class::MODEL_ANIMATIONS, path)
        .unwrap_or_else(|| panic!("no graph `{path}`"));
    AnimationGraph::read(map, tag).unwrap()
}

fn model(map: &MapFile, path: &str) -> Model {
    Model::read(
        map,
        map.find(class::MODEL, path)
            .unwrap_or_else(|| panic!("no model `{path}`")),
    )
    .unwrap()
}

const SPARTAN: &str = "characters\\cyborg\\cyborg";
const ARMS: &str = "characters\\cyborg\\fp\\fp";

fn spartan() -> Option<(&'static MapFile, &'static AnimationGraph)> {
    static G: OnceLock<Option<AnimationGraph>> = OnceLock::new();
    let map = bloodgulch()?;
    Some((map, G.get_or_init(|| Some(graph(map, SPARTAN))).as_ref()?))
}

fn close(a: f32, b: f32, eps: f32, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: {a} vs {b}");
}

/// Frames, key frame, loop frame.
fn assert_fkl(a: &Animation, want: (i16, i16, i16), what: &str) {
    assert_eq!(
        (a.frame_count, a.key_frame, a.loop_frame),
        want,
        "{what} (`{}`)",
        a.name
    );
}

#[test]
fn inventory() {
    let Some(map) = bloodgulch() else { return };
    // (path, nodes, animations)
    let rows: [(&str, usize, usize); 20] = [
        ("characters\\cyborg\\cyborg", 19, 195),
        ("weapons\\assault rifle\\fp\\fp", 42, 13),
        ("weapons\\pistol\\fp\\fp", 44, 10),
        ("weapons\\shotgun\\fp\\fp", 41, 12),
        ("weapons\\plasma rifle\\fp\\fp", 42, 14),
        ("weapons\\rocket launcher\\fp\\fp", 42, 10),
        ("weapons\\sniper rifle\\fp\\fp", 41, 10),
        ("weapons\\plasma pistol\\fp\\fp", 41, 18),
        ("weapons\\flamethrower\\fp\\fp", 40, 8),
        ("weapons\\needler\\fp\\fp", 54, 10),
        ("weapons\\ball\\fp\\fp", 38, 6),
        ("weapons\\flag\\fp\\fp", 38, 6),
        ("weapons\\pistol\\pistol", 7, 1),
        ("weapons\\shotgun\\shotgun", 2, 2),
        ("weapons\\sniper rifle\\sniper rifle", 4, 1),
        ("vehicles\\warthog\\warthog", 0, 11),
        ("vehicles\\scorpion\\scorpion", 11, 14),
        ("vehicles\\ghost\\ghost", 0, 4),
        ("scenery\\plants\\plant_broadleaf_short\\plant_broadleaf_short", 0, 1),
        ("weapons\\flag\\fp\\fp", 38, 6),
    ];
    for (path, nodes, anims) in rows {
        let g = graph(map, path);
        assert_eq!((g.nodes.len(), g.animations.len()), (nodes, anims), "{path}");
    }
    let g = graph(map, SPARTAN);
    assert_eq!(g.units.len(), 24);
    assert_eq!(g.weapons.len(), 1);
    assert_eq!(g.unit_damage.len(), 176);
    assert_eq!(g.sounds.len(), 23);
    // First-person tables: the number of slots and sounds.
    for (path, slots, sounds) in [
        ("weapons\\assault rifle\\fp\\fp", 17, 3),
        ("weapons\\pistol\\fp\\fp", 17, 4),
        ("weapons\\shotgun\\fp\\fp", 26, 8),
        ("weapons\\plasma rifle\\fp\\fp", 27, 5),
        ("weapons\\rocket launcher\\fp\\fp", 17, 6),
        ("weapons\\sniper rifle\\fp\\fp", 17, 5),
        ("weapons\\plasma pistol\\fp\\fp", 28, 5),
        ("weapons\\flamethrower\\fp\\fp", 17, 4),
        ("weapons\\needler\\fp\\fp", 18, 4),
        ("weapons\\ball\\fp\\fp", 14, 3),
        ("weapons\\flag\\fp\\fp", 14, 3),
    ] {
        let g = graph(map, path);
        assert_eq!(g.first_person_weapons.len(), 1, "{path}");
        assert_eq!(g.first_person_weapons[0].animations.len(), slots, "{path} slots");
        assert_eq!(g.sounds.len(), sounds, "{path} sounds");
    }
    // The weapons' own graphs hold a weapons table.
    for path in [
        "weapons\\pistol\\pistol",
        "weapons\\shotgun\\shotgun",
        "weapons\\sniper rifle\\sniper rifle",
    ] {
        assert_eq!(graph(map, path).weapons.len(), 1, "{path}");
    }
    // The vehicles' graphs hold vehicle overlays.
    assert_eq!(graph(map, "vehicles\\warthog\\warthog").vehicles.len(), 1);
}

/// One first-person slot's expectation.
enum Slot {
    None,
    /// Frames, key frame, loop frame.
    Is(i16, i16, i16),
    /// The slot holds the same animation as another slot.
    Same(FirstPersonSlot),
    /// Frames (key and loop frames 0 unless stated by `Is`).
    Frames(i16),
}

use FirstPersonSlot as F;
use Slot::{Frames, Is, None as No, Same};

#[test]
fn first_person_tables() {
    let Some(map) = bloodgulch() else { return };
    // Per weapon: the slots the spec's table states (every other slot of
    // the table is none unless it is named in the "others" column).
    let weapons: Vec<(&str, Vec<(FirstPersonSlot, Slot)>)> = vec![
        (
            "weapons\\assault rifle\\fp\\fp",
            vec![
                (F::Ready, Is(29, 0, 0)),
                (F::PutAway, Is(11, 0, 0)),
                (F::Fire1, No),
                (F::ReloadEmpty, Is(87, 0, 0)),
                (F::ReloadFull, Same(F::ReloadEmpty)),
                (F::Melee, Is(37, 3, 0)),
                (F::ThrowGrenade, Is(37, 0, 0)),
                (F::Idle, Frames(80)),
                (F::Posing, Frames(118)),
                (F::Moving, Frames(26)),
                (F::Overlays, Frames(9)),
                (F::LightOff, Frames(27)),
                (F::LightOn, Same(F::LightOff)),
            ],
        ),
        (
            "weapons\\pistol\\fp\\fp",
            vec![
                (F::Ready, Is(35, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(15, 0, 0)),
                (F::ReloadEmpty, Is(67, 0, 0)),
                (F::ReloadFull, Is(65, 0, 0)),
                (F::Melee, Is(45, 4, 0)),
                (F::ThrowGrenade, Is(38, 0, 0)),
                (F::Idle, Frames(60)),
                (F::Posing, Frames(133)),
                (F::Moving, Frames(25)),
                (F::Overlays, Frames(9)),
            ],
        ),
        (
            "weapons\\plasma rifle\\fp\\fp",
            vec![
                (F::Ready, Is(29, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(22, 0, 0)),
                (F::ReloadEmpty, No),
                (F::ReloadFull, No),
                (F::Melee, Is(36, 6, 0)),
                (F::ThrowGrenade, Is(37, 0, 0)),
                (F::Overheated, Is(50, 0, 58)),
                (F::Overheating, Frames(58)),
                (F::OverheatingAgain, Frames(50)),
                (F::ThrowOverheated, Frames(35)),
                (F::Misfire1, Frames(18)),
                (F::OverheatedExit, Frames(35)),
                (F::Idle, Frames(60)),
                (F::Posing, Frames(105)),
            ],
        ),
        (
            "weapons\\shotgun\\fp\\fp",
            vec![
                (F::Ready, Is(23, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(26, 0, 0)),
                (F::ReloadEmpty, Is(12, 0, 0)),
                (F::ReloadFull, Same(F::ReloadEmpty)),
                (F::Melee, Is(36, 4, 0)),
                (F::ThrowGrenade, Is(37, 0, 0)),
                (F::Enter, Frames(15)),
                (F::ExitEmpty, Frames(42)),
                (F::ExitFull, Frames(24)),
                (F::Idle, Frames(62)),
                (F::Posing, Frames(66)),
            ],
        ),
        (
            "weapons\\sniper rifle\\fp\\fp",
            vec![
                (F::Ready, Is(29, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(19, 0, 0)),
                (F::ReloadEmpty, Is(94, 0, 0)),
                (F::ReloadFull, Is(83, 0, 0)),
                (F::Melee, Is(36, 6, 0)),
                (F::ThrowGrenade, Is(37, 0, 0)),
                (F::Idle, Frames(80)),
                (F::Posing, Frames(166)),
            ],
        ),
        (
            "weapons\\rocket launcher\\fp\\fp",
            vec![
                (F::Ready, Is(22, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(28, 0, 0)),
                (F::ReloadEmpty, Is(125, 0, 0)),
                (F::ReloadFull, Is(111, 0, 0)),
                (F::Melee, Is(52, 4, 0)),
                (F::ThrowGrenade, Is(43, 0, 0)),
                (F::Idle, Frames(60)),
                (F::Posing, Frames(118)),
            ],
        ),
        (
            "weapons\\needler\\fp\\fp",
            vec![
                (F::Ready, Is(23, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(8, 0, 0)),
                (F::ReloadEmpty, Is(70, 0, 0)),
                (F::ReloadFull, Same(F::ReloadEmpty)),
                (F::Melee, Is(48, 6, 0)),
                (F::ThrowGrenade, Is(37, 0, 0)),
                (F::Ammunition, Frames(21)),
                (F::Idle, Frames(60)),
                (F::Posing, Frames(88)),
            ],
        ),
        (
            "weapons\\plasma pistol\\fp\\fp",
            vec![
                (F::Ready, Is(14, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, Is(13, 0, 0)),
                (F::ReloadEmpty, Is(9, 0, 34)),
                (F::ReloadFull, Same(F::ReloadEmpty)),
                (F::Melee, Is(33, 4, 0)),
                (F::ThrowGrenade, Is(37, 0, 0)),
                (F::Fire2, Frames(24)),
                (F::Overcharged, Is(20, 0, 20)),
                (F::OverchargedJitter, Frames(16)),
                (F::Overheated, Is(9, 0, 34)),
                (F::Overheating, Frames(34)),
                (F::OverheatingAgain, Frames(25)),
                (F::Misfire1, Frames(19)),
                (F::ThrowOverheated, Frames(33)),
                (F::OverheatedExit, Frames(24)),
                (F::OverheatedSuperchargeEnter, Frames(24)),
            ],
        ),
        (
            "weapons\\flamethrower\\fp\\fp",
            vec![
                (F::Ready, Is(30, 0, 0)),
                (F::PutAway, No),
                (F::Fire1, No),
                (F::ReloadEmpty, Is(104, 0, 0)),
                (F::ReloadFull, Same(F::ReloadEmpty)),
                (F::Melee, Is(51, 8, 0)),
                (F::ThrowGrenade, Is(39, 0, 0)),
                (F::Idle, Frames(60)),
                (F::Posing, Frames(113)),
            ],
        ),
    ];
    for (path, slots) in weapons {
        let g = graph(map, path);
        for (slot, want) in slots {
            let what = format!("{path} {}", slot.label());
            let a = g.first_person_animation(slot);
            match want {
                No => assert!(a.is_none(), "{what}"),
                Is(f, k, l) => assert_fkl(a.unwrap_or_else(|| panic!("{what} is empty")), (f, k, l), &what),
                Frames(f) => assert_eq!(a.unwrap_or_else(|| panic!("{what} is empty")).frame_count, f, "{what}"),
                Same(other) => {
                    let table = &g.first_person_weapons[0].animations;
                    use blam_formats::animation::Slot as _;
                    assert!(
                        table.get(slot).is_some() && table.get(slot) == table.get(other),
                        "{what}"
                    );
                    let _ = slot.index();
                }
            }
        }
        // Every first-person animation is uncompressed; the second key
        // frame is 0 everywhere, and only melee has a key frame among the
        // slotted ones.
        for a in &g.animations {
            assert!(!a.is_compressed(), "{path} {}", a.name);
            assert_eq!(a.second_key_frame, 0, "{path} {}", a.name);
        }
        let table = &g.first_person_weapons[0].animations;
        for (i, e) in table.entries().iter().enumerate() {
            let Some(e) = e else { continue };
            let slot = FirstPersonSlot::from_index(i).unwrap();
            let a = g.animation(*e).unwrap();
            if slot != F::Melee {
                assert_eq!(a.key_frame, 0, "{path} {}", slot.label());
            }
            // Moving and overlays are overlays: the others base animations
            // (but for the ammunition and jitter overlays).
            let overlay = matches!(slot, F::Moving | F::Overlays | F::OverchargedJitter | F::Ammunition);
            assert_eq!(a.kind == AnimationType::Overlay, overlay, "{path} {}", slot.label());
        }
        // Moving is 25 or 26 frames, overlays 9.
        let moving = g.first_person_animation(F::Moving).unwrap().frame_count;
        assert!(moving == 25 || moving == 26, "{path} moving {moving}");
        assert_eq!(g.first_person_animation(F::Overlays).unwrap().frame_count, 9, "{path}");
    }
    // The ball and the flag.
    for path in ["weapons\\ball\\fp\\fp", "weapons\\flag\\fp\\fp"] {
        let g = graph(map, path);
        assert_eq!(g.first_person_weapons[0].animations.len(), 14);
        assert_eq!(g.first_person_animation(F::Moving).unwrap().frame_count / 5, 5);
        assert_eq!(g.first_person_animation(F::Overlays).unwrap().frame_count, 9);
    }
}

#[test]
fn assault_rifle_table_and_sounds() {
    let Some(map) = bloodgulch() else { return };
    let g = graph(map, "weapons\\assault rifle\\fp\\fp");
    let t = &g.first_person_weapons[0].animations;
    assert_eq!(t.len(), 17);
    // The spec's slot list: 0 idle 80, 1 posing 118, 2 none, 3 moving 26,
    // 4 overlays 9, 5 and 6 light-off 27, 7 and 8 reload-full 87, 9 none,
    // 10 ready 29, 11 put-away 11, 12 none, 13 melee 37 key 3, 14 none,
    // 15 none, 16 throw-grenade 37.
    let want: [Option<(&str, i16)>; 17] = [
        Some(("first-person idle", 80)),
        Some(("first-person posing", 118)),
        None,
        Some(("first-person moving", 26)),
        Some(("first-person overlays", 9)),
        Some(("first-person light-off", 27)),
        Some(("first-person light-off", 27)),
        Some(("first-person reload-full", 87)),
        Some(("first-person reload-full", 87)),
        None,
        Some(("first-person ready", 29)),
        Some(("first-person put-away", 11)),
        None,
        Some(("first-person melee", 37)),
        None,
        None,
        Some(("first-person throw-grenade", 37)),
    ];
    for (i, w) in want.iter().enumerate() {
        let a = t.entries()[i].and_then(|e| g.animation(e));
        match (a, w) {
            (None, None) => {}
            (Some(a), Some((name, frames))) => {
                assert_eq!(a.name, *name, "slot {i}");
                assert_eq!(a.frame_count, *frames, "slot {i}");
                assert_eq!(a.loop_frame, 0, "slot {i}: no loop frames");
            }
            (a, w) => panic!("slot {i}: {:?} vs {w:?}", a.map(|a| &a.name)),
        }
    }
    assert_eq!(g.first_person_animation(F::Melee).unwrap().key_frame, 3);
    // Sound indices: reload 0, melee 1, ready 2, all at sound frame 0.
    for (slot, sound) in [(F::ReloadEmpty, 0), (F::Melee, 1), (F::Ready, 2)] {
        let a = g.first_person_animation(slot).unwrap();
        assert_eq!((a.sound, a.sound_frame), (Some(sound), 0), "{}", slot.label());
    }
    // The three animations no slot uses.
    let used: Vec<usize> = t.entries().iter().flatten().copied().collect();
    let unused: Vec<&Animation> = g
        .animations
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(i))
        .map(|(_, a)| a)
        .collect();
    let names: Vec<&str> = unused.iter().map(|a| a.name.as_str()).collect();
    for n in [
        "first-person firing",
        "first-person reload-full2",
        "first-person stealth-melee",
    ] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    let find = |n: &str| unused.iter().find(|a| a.name == n).unwrap();
    assert_eq!(find("first-person firing").frame_count, 21);
    assert_eq!(find("first-person reload-full2").frame_count, 87);
    assert_eq!(
        (
            find("first-person stealth-melee").frame_count,
            find("first-person stealth-melee").key_frame
        ),
        (61, 24)
    );
}

#[test]
fn spartan_labels() {
    let Some((_, g)) = spartan() else { return };
    let seats: Vec<&str> = g.units.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(
        seats,
        [
            "W-driver",
            "W-gunner",
            "W-passenger",
            "crouch",
            "stand",
            "G-driver",
            "B-driver",
            "S-driver",
            "S-gunner",
            "S-rider",
            "R-driver",
            "P-driver",
            "alert",
            "scorpion-driver",
            "driver lifepod",
            "W-rider",
            "P-riderLF",
            "GT-gunner",
            "CT-driver",
            "P-riderRF",
            "scorpionLF",
            "scorpionLB",
            "scorpionRB",
            "scorpionRF",
        ]
    );
    let classes = |seat: &str| -> Vec<(String, Vec<String>)> {
        let s = g.units.iter().find(|s| s.label == seat).unwrap();
        s.weapon_classes
            .iter()
            .map(|c| {
                (
                    c.label.clone(),
                    c.weapon_types.iter().map(|t| t.label.clone()).collect(),
                )
            })
            .collect()
    };
    let strs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let stand = classes("stand");
    assert_eq!(
        stand,
        vec![
            ("rifle".to_string(), strs(&["ar", "sg", "sr", "pr", "pb", "f", "ne"])),
            ("pistol".to_string(), strs(&["hp", "pp", "ne", "b"])),
            ("missile".to_string(), strs(&["rl"])),
            ("support".to_string(), strs(&["ft"])),
            ("unarmed".to_string(), strs(&[""])),
        ]
    );
    // Crouch: the same without `pb` and `rifle ne`.
    let crouch = classes("crouch");
    assert_eq!(crouch[0].1, strs(&["ar", "sg", "sr", "pr", "f"]));
    assert_eq!(crouch[1..], stand[1..]);
    // The bounds of every seat's looking screen and the stand screens.
    for s in &g.units {
        let b = s.looking_bounds;
        close(b.right_yaw_per_frame, 1.2217, 1e-3, "look right");
        close(b.left_yaw_per_frame, 1.2217, 1e-3, "look left");
        close(b.down_pitch_per_frame, 0.7854, 1e-3, "look down");
        close(b.up_pitch_per_frame, 0.7854, 1e-3, "look up");
        assert_eq!(
            (
                b.right_frame_count,
                b.left_frame_count,
                b.down_frame_count,
                b.up_frame_count
            ),
            (1, 1, 1, 1)
        );
        assert_eq!((b.columns(), b.rows()), (3, 3));
    }
    let stand_seat = g.units.iter().find(|s| s.label == "stand").unwrap();
    for c in &stand_seat.weapon_classes[..2] {
        let b = c.aiming_bounds;
        for v in [
            b.right_yaw_per_frame,
            b.left_yaw_per_frame,
            b.down_pitch_per_frame,
            b.up_pitch_per_frame,
        ] {
            close(v, 1.5708, 1e-3, "aim bounds");
        }
        assert_eq!(
            (
                b.right_frame_count,
                b.left_frame_count,
                b.down_frame_count,
                b.up_frame_count
            ),
            (1, 1, 1, 1)
        );
    }
    // The labels the selection finds: stand with ar, hp, ne, unarmed.
    for (label, class) in [
        ("ar", "rifle"),
        ("hp", "pistol"),
        ("ne", "pistol"),
        ("unarmed", "unarmed"),
    ] {
        let s = g
            .select_unit("stand", label)
            .unwrap_or_else(|| panic!("no selection for {label}"));
        assert_eq!(g.seat(s).unwrap().label, "stand");
        assert_eq!(g.weapon_class(s).unwrap().label, class, "{label}");
    }
}

/// The standing selection for a class label and a type label.
fn stand_selection(g: &AnimationGraph, seat: &str, class: &str, ty: &str) -> UnitSelection {
    let s = g.units.iter().position(|s| s.label == seat).unwrap();
    let c = g.units[s].weapon_classes.iter().position(|c| c.label == class).unwrap();
    let t = g.units[s].weapon_classes[c]
        .weapon_types
        .iter()
        .position(|t| t.label == ty)
        .unwrap();
    UnitSelection {
        seat: s,
        weapon_class: c,
        weapon_type: t,
    }
}

fn frame_info_sums(a: &Animation) -> (f32, f32, f32, f32) {
    a.frame_info.iter().fold((0.0, 0.0, 0.0, 0.0), |s, f| {
        (s.0 + f.dx, s.1 + f.dy, s.2 + f.dz, s.3 + f.dyaw)
    })
}

#[test]
fn spartan_standing_animations() {
    let Some((_, g)) = spartan() else { return };
    use WeaponClassSlot as C;
    let rifle = stand_selection(g, "stand", "rifle", "ar");
    let pistol = stand_selection(g, "stand", "pistol", "hp");
    let class_row = |sel: UnitSelection, slot: C| {
        g.class_animation(sel, slot)
            .unwrap_or_else(|| panic!("{} empty", slot.label()))
    };

    // (slot, rifle (name, f, k, l), pistol (name, f, k, l))
    type Row = (&'static str, i16, i16, i16);
    let rows: [(C, Row, Row); 13] = [
        (C::Idle, ("stand rifle idle", 59, 0, 0), ("stand pistol idle", 49, 0, 0)),
        (
            C::TurnLeft,
            ("stand rifle turn-left", 7, 0, 0),
            ("stand pistol turn-left", 7, 0, 0),
        ),
        (
            C::TurnRight,
            ("stand rifle turn-right", 7, 0, 0),
            ("stand pistol turn-right", 7, 0, 0),
        ),
        (
            C::MoveFront,
            ("stand rifle move-front", 20, 0, 0),
            ("stand pistol move-front", 20, 0, 0),
        ),
        (
            C::MoveBack,
            ("stand rifle move-back", 20, 0, 0),
            ("stand pistol move-back", 20, 0, 0),
        ),
        (
            C::MoveLeft,
            ("stand rifle move-left", 20, 0, 0),
            ("stand pistol move-left", 20, 0, 0),
        ),
        (
            C::MoveRight,
            ("stand rifle move-right", 20, 0, 0),
            ("stand pistol move-right", 20, 0, 0),
        ),
        (
            C::Airborne,
            ("stand rifle airborne", 79, 0, 40),
            ("stand pistol airborne", 80, 0, 40),
        ),
        (
            C::LandSoft,
            ("stand rifle land-soft", 29, 0, 0),
            ("stand pistol land-soft", 29, 0, 30),
        ),
        (
            C::LandHard,
            ("stand rifle land-hard", 39, 0, 30),
            ("stand pistol land-hard", 40, 0, 30),
        ),
        (
            C::ThrowGrenade,
            ("stand rifle throw-grenade", 38, 8, 0),
            ("stand rifle throw-grenade", 38, 8, 0),
        ),
        (
            C::AimStill,
            ("stand rifle aim-still", 9, 0, 0),
            ("stand pistol aim-still", 9, 0, 0),
        ),
        (
            C::AimMove,
            ("stand rifle aim-move", 9, 0, 0),
            ("stand pistol aim-move", 9, 0, 0),
        ),
    ];
    for (slot, r, p) in rows {
        for (sel, (name, f, k, l)) in [(rifle, r), (pistol, p)] {
            let a = class_row(sel, slot);
            assert_eq!(a.name, name, "{}", slot.label());
            assert_fkl(a, (f, k, l), slot.label());
        }
    }
    for slot in [C::AimStill, C::AimMove] {
        for sel in [rifle, pistol] {
            assert_eq!(class_row(sel, slot).kind, AnimationType::Overlay);
        }
    }
    // Stunned: 19, 26, 19, 19 frames.
    for (slot, n) in [
        (C::StunnedFront, 19),
        (C::StunnedBack, 26),
        (C::StunnedLeft, 19),
        (C::StunnedRight, 19),
    ] {
        assert_eq!(class_row(rifle, slot).frame_count, n, "{}", slot.label());
        assert_eq!(class_row(pistol, slot).frame_count, n, "{}", slot.label());
    }
    // Ready: the rifle class has a 15-frame replacement, the pistol none.
    let ready = class_row(rifle, C::Ready);
    assert_eq!(
        (ready.name.as_str(), ready.frame_count, ready.kind),
        ("stand rifle ready", 15, AnimationType::Replacement)
    );
    assert!(g.class_animation(pistol, C::Ready).is_none());

    // Frame info types, foot frames and the sums of dx, dy and dyaw.
    for (sel, tag) in [(rifle, "rifle"), (pistol, "pistol")] {
        let feet_turn = if tag == "rifle" {
            [(7, 14), (14, 7)]
        } else {
            [(4, 8), (8, 4)]
        };
        for ((slot, sign), feet) in [(C::TurnLeft, 1.0f32), (C::TurnRight, -1.0)].into_iter().zip(feet_turn) {
            let a = class_row(sel, slot);
            assert_eq!(a.frame_info_type, FrameInfoType::DxDyDyaw);
            close(frame_info_sums(a).3, sign * 1.5708, 1e-3, "turn dyaw");
            assert_eq!(
                (a.left_foot_frame, a.right_foot_frame),
                feet,
                "{tag} {} feet",
                slot.label()
            );
        }
        for (slot, feet) in [
            (C::MoveFront, (0, 10)),
            (C::MoveBack, (9, 19)),
            (C::MoveLeft, (17, 8)),
            (C::MoveRight, (8, 17)),
        ] {
            let a = class_row(sel, slot);
            assert_eq!(a.frame_info_type, FrameInfoType::DxDy);
            assert_eq!((a.left_foot_frame, a.right_foot_frame), feet, "{tag} {}", slot.label());
        }
        assert_eq!(class_row(sel, C::Idle).frame_info_type, FrameInfoType::DxDy);
    }
    // The rifle's sums: dx 1.50, -1.34, dy +1.34, -1.34.
    for (slot, dx, dy) in [
        (C::MoveFront, 1.50, 0.0),
        (C::MoveBack, -1.34, 0.0),
        (C::MoveLeft, 0.0, 1.34),
        (C::MoveRight, 0.0, -1.34),
    ] {
        let s = frame_info_sums(class_row(rifle, slot));
        close(s.0, dx, 0.02, &format!("{} dx", slot.label()));
        close(s.1, dy, 0.02, &format!("{} dy", slot.label()));
    }
    close(
        class_row(rifle, C::MoveFront).frame_info[0].dx,
        0.075,
        0.002,
        "dx per tick",
    );
    // The turn-left frame info: 7 records of (0, 0, 0.2244).
    let tl = class_row(rifle, C::TurnLeft);
    assert_eq!(tl.frame_info.len(), 7);
    for f in &tl.frame_info {
        close(f.dx, 0.0, 1e-6, "turn dx");
        close(f.dy, 0.0, 1e-6, "turn dy");
        close(f.dyaw, 0.2244, 1e-4, "turn dyaw");
    }
    // The pistol's moves: the spec states dx 0 (see the report).
    let s = frame_info_sums(class_row(pistol, C::MoveFront));
    close(s.0, 0.0, 0.02, "pistol move-front dx (spec: 0)");

    // Weapon types: reload-1, fire-1, fire-2, chamber-1, melee.
    use WeaponTypeSlot as T;
    let ty = |class: &str, label: &str, slot: T| g.type_animation(stand_selection(g, "stand", class, label), slot);
    for (class, label, n) in [
        ("rifle", "ar", 87),
        ("rifle", "sg", 11),
        ("rifle", "sr", 64),
        ("pistol", "hp", 64),
        ("pistol", "ne", 64),
    ] {
        assert_eq!(ty(class, label, T::Reload1).unwrap().frame_count, n, "{label} reload-1");
    }
    assert_eq!(ty("rifle", "sr", T::Reload1).unwrap().kind, AnimationType::Replacement);
    for (class, label, n) in [("rifle", "ar", 4), ("rifle", "sg", 13), ("pistol", "hp", 9)] {
        let a = ty(class, label, T::Fire1).unwrap();
        assert_eq!(a.frame_count, n, "{label} fire-1");
        assert_eq!(a.kind, AnimationType::Overlay);
    }
    // `pr` uses `ar`'s fire-1 and `pp` uses `hp`'s: the same animation.
    let table = |class: &str, label: &str| {
        let sel = stand_selection(g, "stand", class, label);
        g.weapon_type(sel).unwrap().animations.get(T::Fire1)
    };
    assert_eq!(table("rifle", "pr"), table("rifle", "ar"));
    assert_eq!(table("pistol", "pp"), table("pistol", "hp"));
    assert_eq!(ty("rifle", "ar", T::Fire2).unwrap().frame_count, 15);
    assert!(ty("pistol", "hp", T::Fire2).is_none());
    assert_eq!(ty("rifle", "sg", T::Chamber1).unwrap().frame_count, 12);
    assert!(ty("pistol", "hp", T::Chamber1).is_none());
    for (class, label, n) in [
        ("rifle", "ar", 36),
        ("rifle", "sg", 38),
        ("rifle", "sr", 35),
        ("rifle", "pr", 40),
        ("rifle", "f", 32),
        ("pistol", "hp", 46),
        ("pistol", "pp", 40),
        ("pistol", "ne", 46),
        ("pistol", "b", 28),
    ] {
        let a = ty(class, label, T::Melee).unwrap();
        assert_eq!(a.frame_count, n, "{class} {label} melee");
        assert_eq!(a.kind, AnimationType::Replacement, "{label} melee is a replacement");
    }
}

#[test]
fn spartan_crouching_and_deaths() {
    let Some((_, g)) = spartan() else { return };
    use WeaponClassSlot as C;
    let rifle = stand_selection(g, "crouch", "rifle", "ar");
    let pistol = stand_selection(g, "crouch", "pistol", "hp");
    let c = |sel, slot: C| {
        g.class_animation(sel, slot)
            .unwrap_or_else(|| panic!("crouch {} empty", slot.label()))
    };
    assert_eq!(c(rifle, C::Idle).frame_count, 60);
    assert_eq!(c(pistol, C::Idle).frame_count, 31);
    for sel in [rifle, pistol] {
        for slot in [C::TurnLeft, C::TurnRight] {
            let a = c(sel, slot);
            assert_eq!(a.frame_count, 7);
            close(frame_info_sums(a).3.abs(), 1.5708, 1e-3, "crouch turn");
        }
        for slot in [C::MoveFront, C::MoveBack, C::MoveLeft, C::MoveRight] {
            assert_eq!(c(sel, slot).frame_count, 30, "{}", slot.label());
        }
        let front = c(sel, C::MoveFront);
        assert_eq!((front.left_foot_frame, front.right_foot_frame), (29, 15));
        close(frame_info_sums(front).0, 0.90, 0.02, "crouch front dx");
        close(frame_info_sums(c(sel, C::MoveBack)).0, -0.65, 0.02, "crouch back dx");
        close(
            frame_info_sums(c(sel, C::MoveLeft)).1.abs(),
            0.60,
            0.02,
            "crouch left dy",
        );
        close(
            frame_info_sums(c(sel, C::MoveRight)).1.abs(),
            0.60,
            0.02,
            "crouch right dy",
        );
    }
    let a = c(rifle, C::Airborne);
    assert_eq!(
        (a.name.as_str(), a.frame_count, a.key_frame, a.loop_frame),
        ("crouch rifle airborne", 40, 0, 20)
    );
    let a = c(pistol, C::Airborne);
    assert_eq!(
        (a.name.as_str(), a.frame_count, a.key_frame, a.loop_frame),
        ("stand unarmed airborne", 79, 0, 39)
    );
    assert_eq!(c(rifle, C::LandSoft).frame_count, 29);
    assert_eq!(c(pistol, C::LandSoft).frame_count, 30);
    assert_fkl(c(rifle, C::LandHard), (59, 0, 0), "crouch rifle land-hard");
    assert_fkl(c(pistol, C::LandHard), (40, 0, 30), "crouch pistol land-hard");
    assert_fkl(c(rifle, C::ThrowGrenade), (38, 8, 0), "crouch throw");
    // The crouching `sg` and `pr` melee slots use `stand rifle sr melee`.
    for label in ["sg", "pr"] {
        let sel = stand_selection(g, "crouch", "rifle", label);
        let a = g.type_animation(sel, WeaponTypeSlot::Melee).unwrap();
        assert_eq!(
            (a.name.as_str(), a.frame_count),
            ("stand rifle sr melee", 35),
            "crouch {label}"
        );
    }

    // Deaths: every seat's airborne-dead and landing-dead.
    use blam_formats::animation::UnitSlot as U;
    for (i, seat) in g.units.iter().enumerate() {
        let sel = UnitSelection {
            seat: i,
            weapon_class: 0,
            weapon_type: 0,
        };
        let a = g
            .resolve(&seat.animations, U::AirborneDead)
            .unwrap_or_else(|| panic!("{} airborne-dead", seat.label));
        assert_eq!(
            (a.name.as_str(), a.frame_count),
            ("stand airborne-dead", 37),
            "{}",
            seat.label
        );
        let l = g.resolve(&seat.animations, U::LandingDead).unwrap();
        assert_eq!(
            (l.name.as_str(), l.frame_count, l.sound_frame),
            ("stand landing-dead", 42, 2),
            "{}",
            seat.label
        );
        assert!(l.sound.is_some());
        let _ = sel;
    }

    // The damage table: s-ping overlays of 9 frames for the 44 slots, kill
    // animations of the stated lengths, no h-ping, no key frames.
    let mut pings = 0;
    for i in 0..blam_formats::animation::UNIT_DAMAGE_SLOTS {
        let slot = UnitDamageSlot::from_index(i).unwrap();
        let a = g.unit_damage_animation(slot);
        match slot.kind {
            DamageKind::SoftPing => {
                let a = a.unwrap_or_else(|| panic!("s-ping {:?} {:?} empty", slot.direction, slot.part));
                assert_eq!((a.kind, a.frame_count), (AnimationType::Overlay, 9));
                pings += 1;
            }
            DamageKind::HardPing => assert!(a.is_none(), "no h-ping"),
            _ => {
                assert!(a.is_some(), "{:?} {:?} {:?}", slot.kind, slot.direction, slot.part);
            }
        }
        if let Some(a) = a {
            assert_eq!(a.key_frame, 0, "{}", a.name);
        }
    }
    assert_eq!(pings, 44);
    let k = |kind, dir, part| {
        g.unit_damage_animation(UnitDamageSlot::new(kind, dir, part))
            .unwrap()
            .frame_count
    };
    use BodyPart as P;
    use DamageDirection as D;
    use DamageKind as K;
    assert_eq!(k(K::SoftKill, D::Front, P::Gut), 69);
    assert_eq!(k(K::SoftKill, D::Front, P::Chest), 44);
    assert_eq!(k(K::SoftKill, D::Left, P::Gut), 30);
    assert_eq!(k(K::SoftKill, D::Right, P::Gut), 30);
    assert_eq!(k(K::SoftKill, D::Back, P::Gut), 43);
    assert_eq!(k(K::HardKill, D::Front, P::Gut), 46);
    assert_eq!(k(K::HardKill, D::Front, P::Head), 60);
    assert_eq!(k(K::HardKill, D::Left, P::Gut), 41);
    assert_eq!(k(K::HardKill, D::Left, P::Head), 44);
    assert_eq!(k(K::HardKill, D::Right, P::Gut), 41);
    assert_eq!(k(K::HardKill, D::Right, P::Head), 44);
    assert_eq!(k(K::HardKill, D::Back, P::Gut), 40);
    // No Spartan animation is compressed; all carry the model's checksum.
    for a in &g.animations {
        assert!(!a.is_compressed(), "{}", a.name);
        assert_eq!(a.node_list_checksum, 0x6CFA_6498, "{}", a.name);
    }
}

#[test]
fn spartan_first_frames() {
    let Some((map, g)) = spartan() else { return };
    let sel = stand_selection(g, "stand", "rifle", "ar");
    let idle = g.class_animation(sel, WeaponClassSlot::Idle).unwrap();
    assert_eq!(idle.name, "stand rifle idle");
    assert_eq!(idle.node_count(), 19);
    assert_eq!(idle.rotation_flags, 0x0002_FD7E);
    assert_eq!((idle.translation_flags, idle.scale_flags), (0, 0));
    assert_eq!(idle.computed_frame_size(), 112);
    assert_eq!(idle.frame_size, 112);
    assert_eq!(idle.computed_default_size(), 344);
    assert_eq!(idle.default_data.len(), 344);
    assert_eq!(idle.frame_data.len(), 112 * 59);
    let raw = |q: [i16; 4]| q.map(|c| f32::from(c) / 32767.0);
    let p = idle.sample(0).unwrap();
    assert_eq!(g.nodes[0].name, "bip01 pelvis");
    assert_eq!(p.nodes[0].rotation, raw([-20504, -10788, -20504, -10788]));
    for k in 0..3 {
        close(
            p.nodes[0].translation[k],
            [0.0, 0.0, 0.36][k],
            1e-6,
            "pelvis translation",
        );
    }
    assert_eq!(g.nodes[3].name, "bip01 spine");
    assert_eq!(p.nodes[3].rotation, raw([2810, -484, -5546, 32167]));
    assert_eq!(g.nodes[12].name, "bip01 head");
    assert_eq!(p.nodes[12].rotation, raw([-10323, 1667, 1366, 31023]));

    // Posed with the model's tree.
    let m = model(map, SPARTAN);
    assert_eq!(m.nodes.len(), 19);
    let parents: Vec<Option<usize>> = m.nodes.iter().map(|n| n.parent).collect();
    let w = p.world_transforms(&parents).unwrap();
    let pos = |name: &str| {
        let i = m.nodes.iter().position(|n| n.name == name).unwrap();
        w[i].position
    };
    for (name, want) in [
        ("bip01 pelvis", [0.0, 0.0, 0.360]),
        ("bip01 head", [0.049, -0.051, 0.560]),
        ("bip01 l foot", [0.121, 0.072, 0.068]),
        ("bip01 r foot", [-0.096, -0.068, 0.068]),
    ] {
        let got = pos(name);
        for k in 0..3 {
            close(got[k], want[k], 1e-3, &format!("{name}[{k}]"));
        }
    }
}

/// The distance from each node to its parent in a pose against the bind
/// pose's (the spec: translation lengths match within 5e-5).
fn check_segments(m: &Model, a: &Animation, what: &str) {
    let parents: Vec<Option<usize>> = m.nodes.iter().map(|n| n.parent).collect();
    let (pelvis, head) = (
        m.nodes.iter().position(|n| n.name == "bip01 pelvis").unwrap(),
        m.nodes.iter().position(|n| n.name == "bip01 head").unwrap(),
    );
    for f in 0..a.frame_count as usize {
        let w = a.sample(f).unwrap().world_transforms(&parents).unwrap();
        for (n, node) in m.nodes.iter().enumerate() {
            let Some(p) = node.parent else { continue };
            let d = (0..3)
                .map(|k| (w[n].position[k] - w[p].position[k]).powi(2))
                .sum::<f32>()
                .sqrt();
            close(
                d,
                node.distance_from_parent,
                2e-4,
                &format!("{what} frame {f} node {n} segment"),
            );
        }
        assert!(
            w[head].position[2] > w[pelvis].position[2],
            "{what} frame {f}: head above pelvis"
        );
    }
}

#[test]
fn spartan_pose_self_checks() {
    let Some((map, g)) = spartan() else { return };
    let m = model(map, SPARTAN);
    // The graph's node names equal the model's, in order; its checksum is
    // the model's.
    let names: Vec<&str> = g.nodes.iter().map(|n| n.name.as_str()).collect();
    let model_names: Vec<&str> = m.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, model_names);
    assert_eq!(m.node_list_checksum, 0x6CFA_6498);
    let nm = g.match_model(&m);
    assert!(nm.is_complete());
    assert_eq!(nm.graph_nodes, (0..19).map(Some).collect::<Vec<_>>());
    assert_eq!(
        g.parents()[1..],
        m.nodes[1..].iter().map(|n| n.parent).collect::<Vec<_>>()[..]
    );
    assert_eq!(g.parents()[0], None);
    for a in &g.animations {
        assert_eq!(
            a.fits_model(m.nodes.len(), m.node_list_checksum),
            a.kind == AnimationType::Base,
            "{}",
            a.name
        );
    }
    let sel = stand_selection(g, "stand", "rifle", "ar");
    for slot in [WeaponClassSlot::Idle, WeaponClassSlot::MoveFront] {
        let a = g.class_animation(sel, slot).unwrap();
        check_segments(&m, a, a.name.as_str());
    }
    // The run cycle alternates the feet and lifts them (spec: left foot x
    // 0.067, -0.217, -0.194, -0.093, 0.069 over frames 0, 4, 8, 12, 16).
    let run = g.class_animation(sel, WeaponClassSlot::MoveFront).unwrap();
    let parents: Vec<Option<usize>> = m.nodes.iter().map(|n| n.parent).collect();
    let lf = m.nodes.iter().position(|n| n.name == "bip01 l foot").unwrap();
    let rf = m.nodes.iter().position(|n| n.name == "bip01 r foot").unwrap();
    let pelvis = 0;
    for (f, lx, rx) in [
        (0, 0.067, -0.161),
        (4, -0.217, -0.011),
        (8, -0.194, 0.139),
        (12, -0.093, -0.081),
        (16, 0.069, -0.208),
    ] {
        let w = run.sample(f).unwrap().world_transforms(&parents).unwrap();
        close(w[lf].position[0], lx, 2e-3, &format!("left foot x at {f}"));
        close(w[rf].position[0], rx, 2e-3, &format!("right foot x at {f}"));
        close(w[pelvis].position[0], -0.007, 2e-3, "pelvis x");
    }
    // The aim grid: frame 4 leaves idle frame 0 unchanged; frames 3 and 5
    // swing the hands about 90 degrees (the pose differs).
    let idle = g.class_animation(sel, WeaponClassSlot::Idle).unwrap();
    let aim = g.class_animation(sel, WeaponClassSlot::AimStill).unwrap();
    let base = idle.sample(0).unwrap();
    let mut p = base.clone();
    p.apply_overlay(aim, 4).unwrap();
    let wb = base.world_transforms(&parents).unwrap();
    let wp = p.world_transforms(&parents).unwrap();
    // The torso and the hands (the legs carry small rotations in frame 4).
    for name in [
        "bip01 spine",
        "bip01 head",
        "bip01 l hand",
        "bip01 r hand",
        "bip01 l forearm",
        "bip01 r forearm",
    ] {
        let n = m.nodes.iter().position(|n| n.name == name).unwrap();
        for k in 0..3 {
            close(
                wp[n].position[k],
                wb[n].position[k],
                1e-3,
                &format!("centre aim frame leaves {name}"),
            );
        }
    }
    let bounds = g.weapon_class(sel).unwrap().aiming_bounds;
    let mut p = base.clone();
    assert!(p.apply_screen(aim, &bounds, 1.5708, 0.0).unwrap());
    let wl = p.world_transforms(&parents).unwrap();
    let hand = m.nodes.iter().position(|n| n.name == "bip01 r hand").unwrap();
    let moved: f32 = (0..3)
        .map(|k| (wl[hand].position[k] - wb[hand].position[k]).powi(2))
        .sum::<f32>()
        .sqrt();
    assert!(moved > 0.05, "aiming left moves the hand ({moved})");
}

#[test]
fn spartan_selection_by_label() {
    let Some((_, g)) = spartan() else { return };
    // The pistol class, later in block order, wins `ne`.
    let ne = g.select_unit("stand", "ne").unwrap();
    assert_eq!(g.weapon_class(ne).unwrap().label, "pistol");
    let ne_crouch = g.select_unit("crouch", "ne").unwrap();
    assert_eq!(g.weapon_class(ne_crouch).unwrap().label, "pistol");
    assert!(g.select_unit("stand", "nothing").is_none());
    let alert = g.select_unit("alert", "ar").unwrap();
    assert_eq!(g.seat(alert).unwrap().label, "alert");
    // The type animations resolve through the selection.
    let ar = g.select_unit("stand", "ar").unwrap();
    assert_eq!(g.type_animation(ar, WeaponTypeSlot::Melee).unwrap().frame_count, 36);
}

#[test]
fn every_animation_of_bloodgulch_samples() {
    let Some(map) = bloodgulch() else { return };
    let mut total = 0;
    for tag in map.of_class(class::MODEL_ANIMATIONS) {
        let g = AnimationGraph::read(map, tag).unwrap();
        for a in &g.animations {
            assert!(!a.is_compressed(), "{} `{}`", tag.path, a.name);
            let n = a.frame_count as usize;
            assert!(n >= 1, "{} `{}`", tag.path, a.name);
            for f in [0, n / 2, n - 1] {
                let p = a
                    .sample(f)
                    .unwrap_or_else(|e| panic!("{} `{}` frame {f}: {e}", tag.path, a.name));
                assert_eq!(p.len(), a.node_count());
                for (i, node) in p.nodes.iter().enumerate() {
                    close(
                        quat_length(node.rotation),
                        1.0,
                        2e-3,
                        &format!("{} `{}` frame {f} node {i}", tag.path, a.name),
                    );
                    assert!(node.translation.iter().all(|v| v.is_finite()));
                    assert!(node.scale.is_finite());
                }
            }
            // Frame info has a record per frame; sizes are as the spec's
            // formulas say.
            assert_eq!(
                a.frame_info.len(),
                if a.frame_info_type == FrameInfoType::None { 0 } else { n }
            );
            assert_eq!(
                a.frame_size as usize,
                a.computed_frame_size(),
                "{} `{}`",
                tag.path,
                a.name
            );
            assert_eq!(
                a.default_data.len(),
                a.computed_default_size(),
                "{} `{}`",
                tag.path,
                a.name
            );
            assert_eq!(
                a.frame_data.len(),
                n * a.computed_frame_size(),
                "{} `{}`",
                tag.path,
                a.name
            );
            total += 1;
        }
    }
    assert!(total > 300, "{total} animations");
}

#[test]
fn every_multiplayer_graph_reads() {
    if image_path().is_none() {
        eprintln!("{SKIP}");
        return;
    }
    for name in MULTIPLAYER {
        let map = load(name).unwrap();
        let mut graphs = 0;
        for tag in map.of_class(class::MODEL_ANIMATIONS) {
            let g = AnimationGraph::read(&map, tag).unwrap_or_else(|e| panic!("{name} `{}`: {e}", tag.path));
            graphs += 1;
            for a in &g.animations {
                assert!(
                    !a.is_compressed(),
                    "{name} {} `{}`: no multiplayer map holds compressed data",
                    tag.path,
                    a.name
                );
                assert_eq!(
                    a.frame_size as usize,
                    a.computed_frame_size(),
                    "{name} {} `{}`",
                    tag.path,
                    a.name
                );
                assert_eq!(a.frame_data.len(), a.frame_count as usize * a.computed_frame_size());
                // Every table entry points inside the animations (or none).
                for s in 0..a.frame_count.max(1) as usize {
                    if s == 0 || s + 1 == a.frame_count as usize {
                        a.sample(s).unwrap();
                    }
                }
            }
            for e in g.unit_damage.entries().iter().flatten() {
                assert!(*e < g.animations.len(), "{name} {} damage index", tag.path);
            }
        }
        assert!(graphs >= 15, "{name}: {graphs} graphs");
    }
}

#[test]
fn first_person_nodes_match_models_by_name() {
    let Some(map) = bloodgulch() else { return };
    let arms = model(map, ARMS);
    assert_eq!(arms.nodes.len(), 37);
    for weapon in [
        "assault rifle",
        "pistol",
        "shotgun",
        "plasma rifle",
        "rocket launcher",
        "sniper rifle",
        "plasma pistol",
        "flamethrower",
        "needler",
        "ball",
        "flag",
    ] {
        let path = format!("weapons\\{weapon}\\fp\\fp");
        let g = graph(map, &path);
        let wm = model(map, &path);
        let arm_match = g.match_model(&arms);
        assert!(
            arm_match.is_complete(),
            "{path}: arms nodes missing from the graph: {:?}",
            arm_match.missing()
        );
        let weapon_match = g.match_model(&wm);
        assert!(
            weapon_match.is_complete(),
            "{path}: weapon nodes missing from the graph: {:?}",
            weapon_match.missing()
        );
        // The graph's node list is the union of the two models' nodes.
        let mut union: Vec<&str> = arms.nodes.iter().map(|n| n.name.as_str()).collect();
        for n in &wm.nodes {
            if !union.contains(&n.name.as_str()) {
                union.push(n.name.as_str());
            }
        }
        assert_eq!(g.nodes.len(), union.len(), "{path}: graph nodes vs the union");
        assert_eq!(g.nodes[0].name, "frame bone24", "{path} root");
        // Poses of the graph give a matrix per model node by name.
        let a = g.first_person_animation(FirstPersonSlot::Idle).unwrap();
        assert!(a.node_count() <= g.nodes.len(), "{path}");
        let w = a.sample(0).unwrap().world_transforms(&g.parents()).unwrap();
        let arm_world = arm_match.select(&w).unwrap();
        assert_eq!(arm_world.len(), 37);
        assert!(weapon_match.select(&w).is_some());
        // Every animation fits the graph.
        for a in &g.animations {
            assert!(a.node_count() <= g.nodes.len(), "{path} `{}`", a.name);
        }
    }
    // The assault rifle: 42 nodes = 37 + 5; `frame gun` is the wrist's child.
    let g = graph(map, "weapons\\assault rifle\\fp\\fp");
    assert_eq!(g.nodes.len(), 42);
    let gun = g.nodes.iter().position(|n| n.name == "frame gun").unwrap();
    let wrist = g.nodes.iter().position(|n| n.name == "frame r wriste").unwrap();
    assert_eq!(g.parents()[gun], Some(wrist));
    // Idle: `frame gun` sits near the camera: 0.067 ahead, 0.040 right
    // (-y), 0.089 below; the wrist 0.039 from it.
    let idle = g.first_person_animation(FirstPersonSlot::Idle).unwrap();
    let w = idle.sample(0).unwrap().world_transforms(&g.parents()).unwrap();
    let p = w[gun].position;
    close(p[0], 0.067, 2e-3, "gun x");
    close(p[1], -0.040, 2e-3, "gun y");
    close(p[2], -0.089, 2e-3, "gun z");
    let d: f32 = (0..3)
        .map(|k| (w[gun].position[k] - w[wrist].position[k]).powi(2))
        .sum::<f32>()
        .sqrt();
    close(d, 0.039, 2e-3, "gun to wrist");
    // The distance stays constant over the idle loop.
    for f in 0..idle.frame_count as usize {
        let w = idle.sample(f).unwrap().world_transforms(&g.parents()).unwrap();
        let df: f32 = (0..3)
            .map(|k| (w[gun].position[k] - w[wrist].position[k]).powi(2))
            .sum::<f32>()
            .sqrt();
        close(df, d, 1e-4, "gun to wrist over the loop");
    }
}

#[test]
fn compressed_crewman_animations() {
    let Some(path) = image_path() else {
        eprintln!("{SKIP}");
        return;
    };
    let disc = blam_formats::disc::DiscImage::open(&path).unwrap();
    let map = MapFile::load(&disc.read_file("maps/a10.map").unwrap()).unwrap();
    let g = graph(&map, "cinematics\\animations\\crewman\\x10\\x10");
    let a = g
        .animations
        .iter()
        .find(|a| a.name == "crew_panic1")
        .expect("crew_panic1");
    assert!(a.is_compressed());
    assert_eq!(a.compressed_data_offset, 0);
    assert!(a.default_data.is_empty());
    assert_eq!((0..a.node_count()).filter(|&n| a.has_rotation(n)).count(), 17);
    assert_eq!((0..a.node_count()).filter(|&n| a.has_translation(n)).count(), 2);
    assert_eq!((0..a.node_count()).filter(|&n| a.has_scale(n)).count(), 0);
    // The stream's headers: node 0's header is first 0, count 51; the
    // counts of the 17 rotation headers sum to 1421.
    let word = |at: usize| u32::from_le_bytes(a.frame_data[at..at + 4].try_into().unwrap());
    assert_eq!(word(0x2C), 0x0000_0033);
    let keys: u32 = (0..17).map(|i| word(0x2C + 4 * i) & 0xFFF).sum();
    assert_eq!(keys, 1421);
    // The default rotation of node 0 is the worked example.
    let defaults = word(0x04) as usize;
    let d: [u8; 6] = a.frame_data[defaults..defaults + 6].try_into().unwrap();
    assert_eq!(d, [0xD3, 0x44, 0x3F, 0x38, 0x64, 0xA4]);
    let q = quat_normalize(decode_compressed_rotation(d));
    let p0 = a.sample(0).unwrap();
    for (got, want) in p0.nodes[0].rotation.iter().zip(q) {
        close(*got, want, 1e-6, "frame 0 is the default");
    }
    // Every frame of every compressed animation of a10's graphs gives unit
    // quaternions and finite translations.
    let mut n = 0;
    for tag in map.of_class(class::MODEL_ANIMATIONS) {
        let g = AnimationGraph::read(&map, tag).unwrap();
        for a in g.animations.iter().filter(|a| a.is_compressed()) {
            for f in 0..a.frame_count as usize {
                let p = a
                    .sample(f)
                    .unwrap_or_else(|e| panic!("{} `{}` frame {f}: {e}", tag.path, a.name));
                for node in &p.nodes {
                    close(
                        quat_length(node.rotation),
                        1.0,
                        1e-4,
                        &format!("{} `{}` frame {f}", tag.path, a.name),
                    );
                    assert!(node.translation.iter().all(|v| v.is_finite()));
                }
            }
            n += 1;
        }
    }
    assert_eq!(n, 66, "a10's compressed animations");
    let _ = Pose::identity(0);
}
