//! A Halo player's death on synthetic data (`game::body::death`,
//! health_shields.md "Death" and "Stun and pings", animations.md "Damage
//! and death" and "Playback", multiplayer_rules.md 5.4): which death the
//! killing hit picks (airborne, hard or soft, the direction's set, the
//! head's hard kill on the left set, the part by hitgroup, no direction
//! as the front), the table's slot numbering, the body's play (frame 0
//! first, one frame a tick, the sound at its frame before the advance, the
//! last frame held, the 3-tick blend in), its fall onto the ground (then
//! the landing death), and
//! its removal after more than 900 idle ticks.

use bevy::math::{Quat, Vec3};
use blam_formats::animation::{BodyPart, DamageDirection, DamageKind, Slot, UnitDamageSlot};
use chimera_core::core::Hitgroup;
use chimera_world::map::anim::{AnimEvent, AnimSet, Animation, Sequence, Track};
use halo_ce::game::{
    body::{
        anims::{SOUND_EVENT, damage_key, seat_death_key},
        death::{CORPSE_LIFE, CorpseState, DeathInputs, HaloCorpse, choose, direction_of, kill_slot, part_of},
    },
    vitality::{PingDirection, Spartan},
};

/// A set with every kill slot and the airborne death: each a 5-frame
/// animation turning bone 0 about Z by 0.1 a frame; the front gut's soft
/// kill has a sound at frame 2.
fn set() -> AnimSet {
    let mut s = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        bases: vec![vec![(Quat::IDENTITY, Vec3::ZERO)]],
        parents: vec![None],
        ..Default::default()
    };
    s.animations.push(Animation {
        name: "death".into(),
        fps: 30.0,
        frames: 5,
        tracks: vec![Track {
            bone: 0,
            rotation: Some((0..5).map(|f| Quat::from_rotation_z(0.1 * f as f32)).collect()),
            position: None,
        }],
        ..Default::default()
    });
    let mut names: Vec<String> = [DamageKind::SoftKill, DamageKind::HardKill]
        .iter()
        .flat_map(|k| {
            DamageDirection::ALL
                .iter()
                .flat_map(move |d| BodyPart::ALL.iter().map(move |p| damage_key(UnitDamageSlot::new(*k, *d, *p))))
        })
        .collect();
    names.push(seat_death_key("stand", "airborne-dead"));
    names.push(seat_death_key("stand", "landing-dead"));
    for name in names {
        let mut seq = Sequence {
            name: name.clone(),
            anims: vec![0],
            grid: (1, 1),
            ..Default::default()
        };
        if name == "damage s-kill front gut" {
            seq.events.push(AnimEvent { cycle: 0.0, event: 2, name: SOUND_EVENT.into(), options: "halo_ce:death".into() });
        }
        s.sequences.push(seq);
        s.names.insert(name, s.sequences.len() - 1);
    }
    s
}

fn inputs(airborne: bool, hard: bool, direction: PingDirection, part: BodyPart) -> DeathInputs {
    DeathInputs { airborne, hard, direction, part }
}

#[test]
fn the_table_numbers_its_slots_as_the_spec_says() {
    // slot = (type x 4 + direction) x 11 + part.
    let s = UnitDamageSlot::new(DamageKind::SoftKill, DamageDirection::Back, BodyPart::Head);
    assert_eq!(s.index(), (2 * 4 + 3) * 11 + 2);
    assert_eq!(UnitDamageSlot::from_index(s.index()), Some(s));
    assert_eq!(damage_key(s), "damage s-kill back head");
}

#[test]
fn the_killing_hit_picks_the_death() {
    let set = set();
    let pick = |i: DeathInputs| choose(&set, &i).unwrap();
    assert_eq!(pick(inputs(false, false, PingDirection::Front, BodyPart::Gut)), "damage s-kill front gut");
    assert_eq!(pick(inputs(false, false, PingDirection::Back, BodyPart::Chest)), "damage s-kill back chest");
    assert_eq!(pick(inputs(false, true, PingDirection::Right, BodyPart::Gut)), "damage h-kill right gut");
    // A hard kill on the head always uses the left set (multiplayer).
    assert_eq!(pick(inputs(false, true, PingDirection::Back, BodyPart::Head)), "damage h-kill left head");
    // A soft one on the head keeps its direction.
    assert_eq!(pick(inputs(false, false, PingDirection::Back, BodyPart::Head)), "damage s-kill back head");
    // In the air: the seat's airborne death, whatever the hit.
    assert_eq!(pick(inputs(true, true, PingDirection::Front, BodyPart::Head)), "stand airborne-dead");
    // A set without the slot: none (the state is entered with no animation).
    assert_eq!(choose(&AnimSet::default(), &inputs(false, false, PingDirection::Front, BodyPart::Gut)), None);
    assert_eq!(
        kill_slot(&inputs(false, false, PingDirection::Left, BodyPart::RightFoot)),
        UnitDamageSlot::new(DamageKind::SoftKill, DamageDirection::Left, BodyPart::RightFoot)
    );
}

#[test]
fn hitgroups_become_the_spartans_parts() {
    assert_eq!(part_of(Hitgroup::Generic), BodyPart::Gut, "a hit without a node: part 0");
    assert_eq!(part_of(Hitgroup::Stomach), BodyPart::Gut);
    assert_eq!(part_of(Hitgroup::Chest), BodyPart::Chest);
    assert_eq!(part_of(Hitgroup::Head), BodyPart::Head);
    assert_eq!(part_of(Hitgroup::LeftArm), BodyPart::LeftArm);
    assert_eq!(part_of(Hitgroup::RightArm), BodyPart::RightArm);
    // Thigh, calf and foot are the foot's part (5 and 9 unused).
    assert_eq!(part_of(Hitgroup::LeftLeg), BodyPart::LeftFoot);
    assert_eq!(part_of(Hitgroup::RightLeg), BodyPart::RightFoot);
}

#[test]
fn the_hits_direction_by_the_cones() {
    let spartan = Spartan::load();
    // Facing -Z (yaw 0): a shot travelling +Z came from the front.
    assert_eq!(direction_of(&spartan, Vec3::Z, 0.0), PingDirection::Front);
    assert_eq!(direction_of(&spartan, Vec3::NEG_Z, 0.0), PingDirection::Back);
    assert_eq!(direction_of(&spartan, Vec3::ZERO, 0.0), PingDirection::Front, "no direction: the front");
    // Turned a quarter: the same shot comes from the side.
    let side = direction_of(&spartan, Vec3::Z, std::f32::consts::FRAC_PI_2);
    assert!(matches!(side, PingDirection::Left | PingDirection::Right), "{side:?}");
    let other = direction_of(&spartan, Vec3::Z, -std::f32::consts::FRAC_PI_2);
    assert_ne!(side, other);
}

fn corpse(animation: &str, velocity: Vec3) -> HaloCorpse {
    HaloCorpse {
        owner: bevy::ecs::entity::Entity::PLACEHOLDER,
        skin: 0,
        at: Vec3::new(0.0, 5.0, 0.0),
        yaw: 0.0,
        velocity,
        animation: animation.into(),
        body: Default::default(),
    }
}

fn angle(q: Quat) -> f32 {
    let (axis, a) = q.to_axis_angle();
    a * axis.z.signum()
}

#[test]
fn the_body_plays_its_death_once_and_holds() {
    let set = set();
    let mut s = CorpseState::new(&set, &corpse("damage s-kill front gut", Vec3::ZERO), true, None);
    let mut frames = Vec::new();
    let mut sounds = Vec::new();
    for t in 0..8 {
        let (pose, sound) = s.tick(&set);
        frames.push((angle(pose[0].0) / 0.1).round() as i32);
        if let Some(n) = sound {
            sounds.push((t, n));
        }
    }
    assert_eq!(frames, [0, 1, 2, 3, 4, 4, 4, 4], "frame 0 first, then one a tick, the last held");
    // "Playback": the sound starts on the advance off its frame (frame 2
    // is shown on tick 2, the sound on tick 3).
    assert_eq!(sounds, [(3, "halo_ce:death".to_string())]);
}

#[test]
fn the_death_blends_in_over_three_ticks() {
    let set = set();
    let from = vec![(Quat::from_rotation_z(-0.9), Vec3::ZERO)];
    let mut s = CorpseState::new(&set, &corpse("damage s-kill back gut", Vec3::ZERO), true, Some(from));
    let a: Vec<f32> = (0..4).map(|_| angle(s.tick(&set).0[0].0)).collect();
    // u = 1/3, 2/3, 1 of the way from -0.9 to the frame's 0, 0.1, 0.2.
    assert!((a[0] - (-0.6)).abs() < 0.02, "{a:?}");
    assert!((a[1] - (-0.3 + 0.1 * 2.0 / 3.0)).abs() < 0.02, "{a:?}");
    assert!((a[2] - 0.2).abs() < 0.02, "{a:?}");
    assert!((a[3] - 0.3).abs() < 0.02, "{a:?}");
}

#[test]
fn an_airborne_body_falls_to_the_ground_and_lies_there() {
    let set = set();
    let mut s = CorpseState::new(&set, &corpse("stand airborne-dead", Vec3::new(3.0, 0.0, 0.0)), false, None);
    // The ground at y 0; `ground(from, drop)` is the distance down to it.
    let ground = |from: Vec3, drop: f32| (from.y <= drop).then_some(from.y.max(0.0));
    let g = 0.0035651792 * 3.048;
    let mut ticks = 0;
    while !s.resting && ticks < 1000 {
        s.fall(g, ground);
        s.tick(&set);
        ticks += 1;
    }
    assert!(s.resting, "landed");
    assert!(s.at.y.abs() < 1e-5, "on the ground: {}", s.at);
    assert!(s.at.x > 0.1, "carried along: {}", s.at);
    // 5 m under Halo's gravity: sqrt(2 h / g) ticks.
    let want = (2.0 * 5.0 / g).sqrt();
    assert!((ticks as f32 - want).abs() <= 2.0, "{ticks} ticks, {want}");
    let x = s.at.x;
    // The touchdown tick still shows the airborne death; the landing death
    // starts on the next tick, from its frame 0.
    assert_eq!(s.sequence, Some(set.names["stand airborne-dead"]));
    s.fall(g, ground);
    assert_eq!(s.at.x, x, "lying still");
    s.tick(&set);
    assert_eq!(s.sequence, Some(set.names["stand landing-dead"]));
    assert_eq!(s.frame, 0);
}

#[test]
fn the_airborne_death_repeats_while_the_body_falls() {
    let set = set();
    let mut s = CorpseState::new(&set, &corpse("stand airborne-dead", Vec3::ZERO), false, None);
    let frames: Vec<usize> = (0..12).map(|_| { s.tick(&set); s.frame }).collect();
    assert_eq!(frames, [0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1], "loop frame 0");
}

#[test]
fn a_body_at_rest_more_than_900_ticks_goes() {
    let set = set();
    let mut s = CorpseState::new(&set, &corpse("damage s-kill front gut", Vec3::ZERO), true, None);
    // At rest from its first tick, its animation running or not.
    let mut n = 0;
    while !s.expired() {
        s.tick(&set);
        n += 1;
    }
    assert_eq!(n, CORPSE_LIFE + 1, "deleted on the 901st tick at rest");
    // A falling body does not count.
    let mut f = CorpseState::new(&set, &corpse("stand airborne-dead", Vec3::ZERO), false, None);
    for _ in 0..1000 {
        f.tick(&set);
    }
    assert_eq!(f.idle, 0);
}

#[test]
fn the_held_weapon_drops_at_once_for_a_push_or_in_the_air() {
    use chimera_core::core::DamageKind;
    use halo_ce::game::body::death::{DeathPush, drops_at_once, violent_death};
    let ground = corpse("damage s-kill front gut", Vec3::ZERO);
    assert!(!drops_at_once(&ground, None), "a bullet: after the drawn delay");
    assert!(!drops_at_once(&ground, Some(&DeathPush(0.0))));
    assert!(drops_at_once(&ground, Some(&DeathPush(2.0))), "the needler's supercombine, 2");
    assert!(drops_at_once(&ground, Some(&DeathPush(4.0))), "a grenade, 4");
    assert!(drops_at_once(&corpse("stand airborne-dead", Vec3::ZERO), None), "in the air");
    assert!(drops_at_once(&corpse("", Vec3::ZERO), None), "no death animation");
    // The death line: violent for falls, flame and pushes of 2 or more.
    assert!(!violent_death(DamageKind::Bullet, 0.0));
    assert!(!violent_death(DamageKind::Melee, 0.0));
    assert!(violent_death(DamageKind::Fall, 0.0));
    assert!(violent_death(DamageKind::Burn, 0.0));
    assert!(violent_death(DamageKind::Bullet, 2.0));
    assert!(violent_death(DamageKind::Blast, 6.0));
}

#[test]
fn a_body_has_hit_volumes_on_its_pose() {
    use chimera_world::map::{BoneBox, MapBone, MapCharacterModel};
    use halo_ce::game::body::death::corpse_hitboxes;
    let m = MapCharacterModel {
        game: Some(halo_ce::game::GAME),
        team: None,
        name: None,
        class: None,
        model: Default::default(),
        hitboxes: vec![chimera_core::core::Hitbox::cuboid(Vec3::ZERO, Vec3::splat(0.1), Quat::IDENTITY, chimera_core::core::BodyRegion::Chest)],
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: bevy::prelude::Transform::IDENTITY,
        animations: None,
        boxes: vec![BoneBox { bone: 0, center: Vec3::new(0.0, 1.0, 0.0), half: Vec3::splat(0.1), group: Hitgroup::Chest }],
        ragdoll: None,
    };
    // The bone turned a quarter about Z: the box above it lies to its side.
    let pose = vec![(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::ZERO)];
    let h = corpse_hitboxes(&m, &pose, 0.0);
    assert_eq!(h.0.len(), 1);
    assert!((h.0[0].center - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-4, "{}", h.0[0].center);
    assert_eq!(h.0[0].region, chimera_core::core::BodyRegion::Chest);
    // Turned by the body's yaw about its feet.
    let h = corpse_hitboxes(&m, &[(Quat::IDENTITY, Vec3::ZERO)], std::f32::consts::FRAC_PI_2);
    assert!((h.0[0].center - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-4);
    let ray = h.0[0].center + Vec3::Z * 5.0;
    assert!(h.nearest(ray, Vec3::NEG_Z, 10.0).is_some(), "a shot meets it");
}
