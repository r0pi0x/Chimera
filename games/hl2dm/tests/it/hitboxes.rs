//! HL2:DM's hitboxes (specs/hl2dm/player_models.md 9; its test cases
//! 54-56): shots are traced against the body's hitbox set and only the
//! head counts (x3, `sk_player_head`): a male citizen's head takes 24 of
//! the pistol's 8, its pelvis (stomach, group 3) 8; a Combine's pelvis and
//! upper chest are generic (group 0), 8; a woman's `Spine1` is stomach. On
//! stand-in bodies with the spec's boxes (no install), on the harness.

use bevy::prelude::*;
use chimera_core::core::{Damage, Health, Hitbox, Hitgroup};
use chimera_sim::{testkit, weapon::Inventory};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{MapCharacterModel, MapData, MapModel, MapSimPlugin},
};
use hl2dm::{
    game::{movement, weapons},
    harness::Hl2Sim,
};

const U: f32 = 0.0254;

/// A box of `group` centred `up` units above the feet, `half` units.
fn box_at(up: f32, half: Vec3, group: Hitgroup, bone: &str) -> Hitbox {
    Hitbox::cuboid(Vec3::Y * up * U, half * U, Quat::IDENTITY, source_engine::content::props::body_region(group, bone))
}

/// A stand-in body: a head, the chest (`Spine2`), the hips (`Pelvis`) in
/// `hips`' group and a `Spine1` between them in `spine1` (none: no box).
fn body(hips: Hitgroup, chest: Hitgroup, spine1: Option<Hitgroup>) -> MapCharacterModel {
    let mut hitboxes = vec![
        box_at(64.0, Vec3::new(4.5, 5.0, 3.5), Hitgroup::Head, "ValveBiped.Bip01_Head1"),
        box_at(54.0, Vec3::new(8.0, 5.0, 5.0), chest, "ValveBiped.Bip01_Spine2"),
        box_at(38.0, Vec3::new(7.5, 6.0, 5.0), hips, "ValveBiped.Bip01_Pelvis"),
    ];
    if let Some(g) = spine1 {
        hitboxes.push(box_at(45.5, Vec3::new(7.0, 2.5, 5.0), g, "ValveBiped.Bip01_Spine1"));
    }
    MapCharacterModel {
        game: Some(hl2dm::game::GAME),
        team: None,
        name: None,
        class: None,
        model: MapModel::default(),
        hitboxes,
        bones: Vec::new(),
        root: Transform::default(),
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    }
}

/// A pistol shot at a target wearing `body`, aimed `up` units above its
/// feet: the target's health after it and the damage's hitgroup.
fn shot(body: MapCharacterModel, up: f32) -> (f32, Option<Hitgroup>) {
    let mut map = MapData::default();
    map.kit.characters.push(body);
    let mut sim = Sim::new(Hl2Sim, (GreyboxMapPlugin, MapSimPlugin::new(map)));
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let pistol = sim.app.world().get::<Inventory>(p).unwrap().weapons.iter().copied().find(|w| {
        sim.app.world().get::<chimera_sim::weapon::Weapon>(*w).is_some_and(|x| x.id == weapons::PISTOL)
    });
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = pistol;
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 2.0, movement::ID);
    sim.seconds(1.5);
    let feet = sim.app.world().get::<avian3d::prelude::ColliderAabb>(t).unwrap().min.y;
    let at = sim.position(t).with_y(feet + up * U);
    testkit::aim_at(&mut sim, p, at);
    let mut cursor = sim.app.world().resource::<Messages<Damage>>().get_cursor_current();
    let mut group = None;
    for k in 0..4 {
        sim.intent(p).fire = k < 2;
        sim.ticks(1);
        if let Some(d) = cursor.read(sim.app.world().resource::<Messages<Damage>>()).find(|d| d.target == t) {
            group = Some(d.hitgroup);
        }
    }
    (sim.app.world().get::<Health>(t).unwrap().current, group)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn a_head_shot_is_three_times_the_rest() {
    // Case 54: male citizen, head 24, pelvis (stomach) 8.
    let citizen = || body(Hitgroup::Stomach, Hitgroup::Chest, None);
    let (h, g) = shot(citizen(), 64.0);
    assert!(close(h, 0.76) && g == Some(Hitgroup::Head), "{h} {g:?}");
    let (h, g) = shot(citizen(), 38.0);
    assert!(close(h, 0.92) && g == Some(Hitgroup::Stomach), "{h} {g:?}");
    let (h, g) = shot(citizen(), 54.0);
    assert!(close(h, 0.92) && g == Some(Hitgroup::Chest), "{h} {g:?}");
}

#[test]
fn the_combines_hips_and_chest_are_generic() {
    // Case 55.
    let combine = || body(Hitgroup::Generic, Hitgroup::Generic, None);
    for up in [38.0, 54.0] {
        let (h, g) = shot(combine(), up);
        assert!(close(h, 0.92) && g == Some(Hitgroup::Generic), "{up}: {h} {g:?}");
    }
}

#[test]
fn a_womans_spine1_is_stomach() {
    // Case 56.
    let (h, g) = shot(body(Hitgroup::Stomach, Hitgroup::Chest, Some(Hitgroup::Stomach)), 45.5);
    assert!(close(h, 0.92) && g == Some(Hitgroup::Stomach), "{h} {g:?}");
}
