//! The RPG's laser on the headless harness (specs/hl2dm/effects_sounds.md
//! 9 and its "Projectile visuals" table, test 47; projectiles.md 6.1):
//! the laser turns on as the draw ends, with its sounds, and off when the
//! RPG is put away; the beam and glow on the view model's attachments 2
//! and 3, on the world model's 4 and 5 (none on a model without them);
//! the pistol's and .357's first-person smoke; the SMG1 grenade's scorch
//! along its flight (projectiles.md 4.3).

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_sim::{
    testkit,
    weapon::{
        Inventory, give,
        grenade::{Detonated, GrenadeKind},
        projectile::Flyer,
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        DecalGroup, GameSound, MapAttachment, MapBone, MapModel, MapViewModel, PlaceDecal, ViewAnimator,
        ViewModelAnchor, ViewModelEvent, ViewModelEventKind, ViewModelOffset, ViewModels,
        attached,
        particles::{MapParticles, ParticleMaterial, ParticleMaterials, ParticleSet, Particles, Shape},
        view_model::{HeldAttachment, placement},
    },
};
use hl2dm::{
    game::{
        effects, flashes, movement, projectile_fx, rpg_fx,
        weapons::{
            self,
            launch::{Hl2Rounds, U},
        },
    },
    harness::Hl2Sim,
};

fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    let materials = effects::MATERIALS
        .iter()
        .chain(projectile_fx::MATERIALS)
        .chain(rpg_fx::MATERIALS)
        .map(|n| ParticleMaterial { name: n.to_string(), ..default() })
        .collect();
    sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
    // A map's (the greybox registers none): sprites and beams on weapons.
    sim.app.add_systems(
        Update,
        attached::draw_attached.in_set(attached::AttachedSet).after(ParticleSet::Step).before(ParticleSet::Draw),
    );
    sim
}

/// The particles drawn this frame with `material`: their centres and
/// whether on the view model's layer.
fn drawn(sim: &Sim, material: &str) -> Vec<(Vec3, bool)> {
    let m = sim.app.world().resource::<ParticleMaterials>().0.find(material).expect(material);
    let ps = sim.app.world().resource::<Particles>();
    ps.groups
        .iter()
        .flat_map(|g| g.particles.iter().map(move |p| (g, p)))
        .filter(|(_, p)| p.material == m)
        .map(|(g, p)| match p.shape {
            Shape::Quad { corners, .. } => ((corners[0] + corners[3]) / 2.0, g.view_model),
            _ => (p.position, g.view_model),
        })
        .collect()
}

fn clear(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<Particles>().groups.clear();
}

/// The game sounds of the next `ticks` ticks.
fn game_sounds(sim: &mut Sim, ticks: u64) -> Vec<String> {
    let mut cursor = sim.app.world().resource::<Messages<GameSound>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..ticks {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<GameSound>>()).map(|s| s.0.entry.clone()));
    }
    out
}

/// A local player holding the RPG with 3 rockets, looking at the north
/// wall; the RPG drawn on tick 0 of the returned state.
fn holding_rpg(sim: &mut Sim) -> (Entity, Entity) {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.seconds(1.5);
    let w = give(sim.app.world_mut(), p, weapons::RPG).unwrap();
    sim.app.world_mut().get_mut::<Hl2Rounds>(w).unwrap().count = 3;
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    testkit::aim_at(sim, p, Vec3::new(0.0, 1.5, greybox::NORTH_WALL_Z));
    (p, w)
}

/// Projectiles 6.1: the dot (and the laser) comes on as the 1.1 s draw
/// ends, with `Weapon_RPG.LaserOn`; put away, it goes with
/// `Weapon_RPG.LaserOff`.
#[test]
fn the_laser_comes_on_after_the_draw_and_goes_when_put_away() {
    let mut sim = sim();
    let (p, _) = holding_rpg(&mut sim);
    let sounds = game_sounds(&mut sim, 60);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::RPG));
    assert!(sim.app.world().get::<rpg_fx::Hl2LaserOn>(p).is_none(), "not during the draw");
    assert!(!sounds.iter().any(|s| s == rpg_fx::LASER_ON));
    let sounds = game_sounds(&mut sim, 30);
    assert!(sim.app.world().get::<rpg_fx::Hl2LaserOn>(p).is_some(), "on after the draw");
    assert_eq!(sounds.iter().filter(|s| *s == rpg_fx::LASER_ON).count(), 1);
    clear(&mut sim);
    sim.ticks(1);
    assert_eq!(drawn(&sim, rpg_fx::GLOW).len(), 1, "the dot");

    let pistol = give(sim.app.world_mut(), p, weapons::PISTOL).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(pistol);
    let sounds = game_sounds(&mut sim, 30);
    assert!(sim.app.world().get::<rpg_fx::Hl2LaserOn>(p).is_none());
    assert_eq!(sounds.iter().filter(|s| *s == rpg_fx::LASER_OFF).count(), 1);
    clear(&mut sim);
    sim.ticks(1);
    assert!(drawn(&sim, rpg_fx::GLOW).is_empty(), "no dot");
}

/// A test view model with the RPG's attachments: 1 `muzzle`, 2 `laser`,
/// 3 `laser_end` (meters, the model's frame), drawn at an anchor.
fn rpg_view_model(sim: &mut Sim, p: Entity, key: &str) -> Vec<MapAttachment> {
    let attachments: Vec<MapAttachment> = [
        ("muzzle", Vec3::new(0.05, -0.05, -0.6)),
        ("laser", Vec3::new(0.08, -0.04, -0.5)),
        ("laser_end", Vec3::new(0.08, -0.04, -0.5 - 100.0 * U)),
    ]
    .into_iter()
    .map(|(name, at)| MapAttachment { name: name.into(), bone: 0, local: Transform::from_translation(at) })
    .collect();
    let model = MapViewModel {
        key: key.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: attachments.clone(),
        light_origin: Vec3::ZERO,
    };
    sim.app.insert_resource(ViewModels(std::sync::Arc::new(vec![model])));
    let eye = GlobalTransform::from(Transform::from_xyz(1.0, 2.0, 3.0));
    sim.app.world_mut().spawn((ViewModelAnchor, eye, ChildOf(p)));
    sim.app.world_mut().entity_mut(p).insert(ViewAnimator { key: Some(key.into()), ..default() });
    attachments
}

/// Where attachment `name` of the test view model is drawn.
fn view_point(sim: &mut Sim, p: Entity, attachments: &[MapAttachment], name: &str) -> Vec3 {
    let world = sim.app.world_mut();
    let anchor = world.query_filtered::<&GlobalTransform, With<ViewModelAnchor>>().single(world).unwrap().compute_transform();
    let offset = sim.app.world().get::<ViewModelOffset>(p).copied().unwrap_or_default();
    let local = attachments.iter().find(|a| a.name == name).unwrap().local.translation;
    anchor.transform_point(placement(&offset, false).transform_point(local))
}

/// The table's `rpg_beam_1st` and `rpg_glow`, first person: a red beam
/// of 4 segments from `laser` to `laser_end` and the glow at `laser`, on
/// the view model's layer; nothing while the laser is off (the draw).
#[test]
fn the_view_model_shows_the_beam_and_glow() {
    let mut sim = sim();
    let (p, _) = holding_rpg(&mut sim);
    let attachments = rpg_view_model(&mut sim, p, weapons::RPG);
    sim.ticks(10);
    assert!(drawn(&sim, rpg_fx::VIEW_BEAM).is_empty(), "off during the draw");
    sim.ticks(80);
    clear(&mut sim);
    sim.ticks(1);
    let beam = drawn(&sim, rpg_fx::VIEW_BEAM);
    assert_eq!(beam.len(), 4, "4 segments");
    assert!(beam.iter().all(|(_, vm)| *vm), "on the view model's layer");
    let (from, to) = (view_point(&mut sim, p, &attachments, "laser"), view_point(&mut sim, p, &attachments, "laser_end"));
    let on_line = |q: Vec3| (q - from).cross((to - from).normalize()).length() < 1e-3;
    assert!(beam.iter().all(|(q, _)| on_line(*q)), "along the laser: {beam:?}");
    let m = sim.app.world().resource::<ParticleMaterials>().0.find(rpg_fx::VIEW_BEAM).unwrap();
    let ps = sim.app.world().resource::<Particles>();
    let bright: Vec<_> = ps.groups.iter().flat_map(|g| g.particles.iter()).filter(|q| q.material == m).collect();
    assert!(bright.iter().all(|q| q.color == Vec3::new(1.0, 0.0, 0.0) && (128.0..=200.0).contains(&(q.alpha * 255.0))));
    let glows = drawn(&sim, rpg_fx::GLOW);
    assert!(glows.iter().any(|(q, vm)| *vm && q.distance(from) < 1e-3), "the glow at the beam's start: {glows:?}");
    assert!(drawn(&sim, rpg_fx::WORLD_BEAM).is_empty(), "not the world model's");
}

/// The playtest's lag: as the view turns, the laser and its glow are on
/// the attachment in the same frame, not where it was a frame ago. The
/// view model is the camera's child, so it turns this frame; the camera's
/// `GlobalTransform` is only updated after `Update`.
#[test]
fn the_beam_and_glow_keep_up_with_the_view_this_frame() {
    #[derive(Resource)]
    struct Anchor(Entity);
    /// What the client's `follow_eye` does: the eye's `Transform`.
    fn turn(anchor: Res<Anchor>, mut t: Query<&mut Transform>) {
        t.get_mut(anchor.0).unwrap().rotate_y(0.1);
    }
    let mut sim = sim();
    let (p, _) = holding_rpg(&mut sim);
    let attachments = rpg_view_model(&mut sim, p, weapons::RPG);
    sim.ticks(90);
    let anchor = sim.app.world_mut().query_filtered::<Entity, With<ViewModelAnchor>>().single(sim.app.world()).unwrap();
    sim.app.world_mut().entity_mut(anchor).insert(Transform::from_xyz(0.0, 0.6, 0.0));
    sim.app.insert_resource(Anchor(anchor));
    attached::order_sets(&mut sim.app);
    sim.app.add_systems(Update, turn.in_set(attached::EyePlaced));
    sim.ticks(2);
    for _ in 0..3 {
        clear(&mut sim);
        sim.ticks(1);
        let w = sim.app.world();
        let eye = *w.get::<Transform>(p).unwrap() * *w.get::<Transform>(anchor).unwrap();
        let offset = w.get::<ViewModelOffset>(p).copied().unwrap_or_default();
        let at = |name: &str| {
            let local = attachments.iter().find(|a| a.name == name).unwrap().local.translation;
            eye.transform_point(placement(&offset, false).transform_point(local))
        };
        let (from, to) = (at("laser"), at("laser_end"));
        let glows = drawn(&sim, rpg_fx::GLOW);
        assert!(glows.iter().any(|(q, vm)| *vm && q.distance(from) < 1e-3), "the glow on `laser` {from}: {glows:?}");
        let beam = drawn(&sim, rpg_fx::VIEW_BEAM);
        assert_eq!(beam.len(), 4);
        let on_line = |q: Vec3| (q - from).cross((to - from).normalize()).length() < 1e-3;
        assert!(beam.iter().all(|(q, _)| on_line(*q)), "the beam on the attachments {from} {to}: {beam:?}");
    }
}

/// Others see the beam on the world model's attachments 4 and 5; a model
/// with fewer draws none.
#[test]
fn the_world_model_beam_needs_its_fifth_attachment() {
    let mut sim = sim();
    let (p, _) = holding_rpg(&mut sim);
    for index in 0..3 {
        let g = GlobalTransform::from_translation(Vec3::new(0.1 * index as f32, 1.4, 0.0));
        sim.app.world_mut().spawn((HeldAttachment { owner: p, index, name: format!("a{index}") }, g));
    }
    sim.ticks(90);
    assert!(sim.app.world().get::<rpg_fx::Hl2LaserOn>(p).is_some());
    assert!(drawn(&sim, rpg_fx::WORLD_BEAM).is_empty(), "3 attachments: nothing");
    let (a, b) = (Vec3::new(0.0, 1.5, -0.3), Vec3::new(0.0, 1.5, -0.8));
    sim.app.world_mut().spawn((HeldAttachment { owner: p, index: 3, name: "laser".into() }, GlobalTransform::from_translation(a)));
    sim.app.world_mut().spawn((HeldAttachment { owner: p, index: 4, name: "laser_end".into() }, GlobalTransform::from_translation(b)));
    sim.ticks(1);
    clear(&mut sim);
    sim.ticks(1);
    let beam = drawn(&sim, rpg_fx::WORLD_BEAM);
    assert_eq!(beam.len(), 4);
    assert!(beam.iter().all(|(q, vm)| !vm && (q.y - 1.5).abs() < 1e-3 && q.z < -0.29 && q.z > -0.81), "{beam:?}");
    assert!(drawn(&sim, rpg_fx::GLOW).iter().any(|(q, _)| q.distance(a) < 1e-4), "the glow at attachment 4");
}

/// The table's `pistol_smoke` and `magnum_smoke`: a first-person `357`
/// flash always puts a puff 8 units ahead of the muzzle; an SMG1 flash
/// none; a pistol flash 3 times in 4 (of 200 flashes, 120 to 180).
#[test]
fn pistol_and_magnum_flashes_smoke() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.ticks(5);
    // The spawn's gun (the gravity gun) is the one drawn.
    let attachments = rpg_view_model(&mut sim, p, weapons::PHYSCANNON);
    sim.ticks(5);
    let muzzle = view_point(&mut sim, p, &attachments, "muzzle");
    let flash = |sim: &mut Sim, kind: &str| {
        sim.app.world_mut().write_message(ViewModelEvent {
            owner: p,
            kind: ViewModelEventKind::MuzzleFlashOf { attachment: 0, kind: kind.into() },
        });
    };
    let puffs = |sim: &Sim| drawn(sim, flashes::SMOKE);
    clear(&mut sim);
    flash(&mut sim, "357");
    sim.ticks(1);
    let after = puffs(&sim);
    assert_eq!(after.len(), 1);
    // Headless, no cameras: no re-projection; ahead along the model's
    // forward (+X of the skeleton), placed by the view model's offset.
    let d = after[0].0 - muzzle;
    assert!(d.length() / U > 6.0 && d.length() / U < 12.0, "{}", d.length() / U);
    clear(&mut sim);
    flash(&mut sim, "SMG1");
    sim.ticks(1);
    assert!(puffs(&sim).is_empty(), "the SMG1 doesn't smoke");
    let mut n = 0;
    for _ in 0..200 {
        clear(&mut sim);
        flash(&mut sim, "PISTOL");
        sim.app.update();
        n += puffs(&sim).len();
    }
    assert!((120..=180).contains(&n), "{n} of 200");
}

/// The SMG1 grenade's flight scorch (projectiles.md 4.3): going off just
/// above the floor while flying down and forward, a `Scorch` on the floor
/// where the 60-unit line along its flight hits; stopped dead, none.
#[test]
fn the_contact_grenade_scorches_along_its_flight() {
    let mut sim = sim();
    sim.seconds(0.5);
    let v = Vec3::new(0.0, -600.0, -300.0) * U;
    let mut f = Flyer::new(weapons::SMG1, None, v, &chimera_core::core::SimClock::default());
    f.gravity = 0.0;
    // It passes through everything: only its velocity matters here.
    f.solid_after = 1e9;
    let at = Vec3::new(0.0, 4.0 * U, 0.0);
    let g = sim.app.world_mut().spawn((f, Transform::from_translation(at))).id();
    sim.ticks(1);
    let mut cursor = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    sim.app.world_mut().write_message(Detonated {
        kind: GrenadeKind::Blast,
        weapon: weapons::SMG1,
        thrower: None,
        at,
        ground: None,
        projectile: g,
        shake: None,
        flags: Default::default(),
    });
    sim.app.world_mut().despawn(g);
    sim.ticks(1);
    let marks: Vec<_> = cursor.read(sim.app.world().resource::<Messages<PlaceDecal>>()).cloned().collect();
    assert_eq!(marks.len(), 1, "{marks:?}");
    assert!(matches!(&marks[0].group, DecalGroup::Named(n) if n == "Scorch"));
    assert!(marks[0].target.is_none() && marks[0].normal.y > 0.99);
    // 4 units down along (0, -2, -1): 2 units ahead.
    assert!(marks[0].point.y.abs() < 1e-3 && (marks[0].point.z / U + 2.0).abs() < 0.05, "{:?}", marks[0].point / U);

    let stopped = sim.app.world_mut().spawn((Flyer::new(weapons::SMG1, None, Vec3::ZERO, &chimera_core::core::SimClock::default()), Transform::from_translation(at))).id();
    sim.ticks(1);
    let mut cursor = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    sim.app.world_mut().write_message(Detonated {
        kind: GrenadeKind::Blast,
        weapon: weapons::SMG1,
        thrower: None,
        at,
        ground: None,
        projectile: stopped,
        shake: None,
        flags: Default::default(),
    });
    sim.ticks(1);
    assert_eq!(cursor.read(sim.app.world().resource::<Messages<PlaceDecal>>()).count(), 0, "no line, no scorch");
}

/// Effects_sounds.md 9 over the network (`NetSim`, two clients): A's
/// laser state reaches B (`Hl2LaserOn`), and B draws A's dot where A's
/// eye looks, on the wall the server's dot is on; A draws its own.
#[test]
fn others_see_the_dot_over_the_network() {
    use std::time::Duration;

    use chimera_core::core::Intent;
    use chimera_net::{
        harness::NetSim,
        net::{NetCharacter, memory::LinkConditions},
    };
    use hl2dm::game::weapons::rpg::Hl2LaserDot;
    let link = LinkConditions { latency: Duration::from_millis(40), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(Hl2Sim, link, 47, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.until_joined(600);
    sim.ticks(100);
    let materials: Vec<ParticleMaterial> =
        rpg_fx::MATERIALS.iter().map(|n| ParticleMaterial { name: n.to_string(), ..default() }).collect();
    for c in &mut sim.clients {
        c.app.insert_resource(ParticleMaterials(MapParticles { materials: materials.clone() }));
    }
    // A holds the RPG, looking down at the floor ahead.
    let theirs = sim.character_of(0).unwrap();
    let gun = {
        let w = sim.server.app.world_mut();
        let gun = give(w, theirs, weapons::RPG).unwrap();
        w.get_mut::<Hl2Rounds>(gun).unwrap().count = 3;
        w.get_mut::<Inventory>(theirs).unwrap().wanted = Some(gun);
        gun
    };
    let me = sim.local_player(0).unwrap();
    {
        let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
        i.pitch = -30f32.to_radians();
    }
    sim.ticks(150);
    let dot = *sim.server.app.world().get::<Hl2LaserDot>(gun).expect("the server's dot");
    let a_id = sim.client_id(0);
    let w1 = sim.clients[1].app.world_mut();
    let a = w1.query::<(Entity, &NetCharacter)>().iter(w1).find(|(_, c)| c.owner == Some(a_id)).map(|(e, _)| e).unwrap();
    assert!(w1.get::<rpg_fx::Hl2LaserOn>(a).is_some(), "B knows A's laser is on");
    let drawn_dots = |w: &World| {
        let m = w.resource::<ParticleMaterials>().0.find(rpg_fx::GLOW).unwrap();
        w.resource::<Particles>()
            .groups
            .iter()
            .flat_map(|g| g.particles.iter())
            .filter(|p| p.material == m)
            .map(|p| p.position)
            .collect::<Vec<_>>()
    };
    let want = dot.at + dot.normal * 4.0 * U;
    let on_b = drawn_dots(sim.clients[1].app.world());
    assert!(on_b.iter().any(|p| p.distance(want) < 0.15), "B draws A's dot near {want}: {on_b:?}");
    let on_a = drawn_dots(sim.clients[0].app.world());
    assert!(on_a.iter().any(|p| p.distance(want) < 0.15), "A draws its own near {want}: {on_a:?}");

    // Put away: off on B too, no dot.
    {
        let w = sim.server.app.world_mut();
        let pistol = give(w, theirs, weapons::PISTOL).unwrap();
        w.get_mut::<Inventory>(theirs).unwrap().wanted = Some(pistol);
    }
    sim.ticks(100);
    let w1 = sim.clients[1].app.world_mut();
    assert!(w1.get::<rpg_fx::Hl2LaserOn>(a).is_none(), "off on B");
    sim.clients[1].app.world_mut().resource_mut::<Particles>().groups.clear();
    sim.ticks(1);
    assert!(drawn_dots(sim.clients[1].app.world()).is_empty());
}

/// Projectiles.md 6.3: at its ignition (tick 20) the missile starts
/// `Missile.Ignite`, looping from it until it is gone, and its owner's
/// screen fades (255, 225, 205, alpha 64 over 0.1 s).
#[test]
fn the_missile_ignites_with_its_loop_and_the_owners_fade() {
    use chimera_world::map::{SoundControl, logic_view::ScreenFades};
    let mut sim = sim();
    sim.app.init_resource::<ScreenFades>();
    let (p, _) = holding_rpg(&mut sim);
    sim.seconds(1.5);
    let mut cursor = sim.app.world().resource::<Messages<SoundControl>>().get_cursor_current();
    let (mut started, mut stopped, mut faded) = (None, None, None);
    for k in 0..200u32 {
        sim.intent(p).fire = k < 2;
        sim.ticks(1);
        let msgs: Vec<SoundControl> = cursor.read(sim.app.world().resource::<Messages<SoundControl>>()).cloned().collect();
        for m in msgs {
            match m {
                SoundControl::Start(s) if s.entry == rpg_fx::IGNITE_SOUND => {
                    let w = sim.app.world_mut();
                    let missile = w.query_filtered::<Entity, With<Flyer>>().iter(w).next();
                    assert_eq!(s.follow, missile, "from the missile");
                    assert!(s.looping);
                    started = Some((k, s.key));
                    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
                    faded = sim.app.world_mut().resource_mut::<ScreenFades>().at(now);
                }
                SoundControl::Stop(key) if started.is_some_and(|(_, s)| s == key) => stopped = Some(k),
                _ => {}
            }
        }
    }
    let (k, _) = started.expect("the loop starts");
    assert!((19..=21).contains(&k), "at the ignition: tick {k}");
    let (color, alpha, _) = faded.expect("the owner's fade");
    assert_eq!(color, [255, 225, 205]);
    assert!(alpha > 0 && alpha <= 64, "{alpha}");
    let end = stopped.expect("stopped when the missile is gone");
    let w = sim.app.world_mut();
    assert_eq!(w.query_filtered::<(), With<Flyer>>().iter(w).count(), 0, "the missile is gone by {end}");
}
