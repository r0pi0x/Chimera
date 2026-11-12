//! What stops an HL2:DM explosion and a missile (specs/hl2dm/projectiles.md
//! 2 steps 2-4, 6.5), on a small map in Source units: a floor at z = 0, a
//! water tank, a door, sky faces.

use avian3d::prelude::RigidBody;
use bevy::prelude::*;
use chimera_core::core::{Health, Velocity};
use chimera_sim::{
    testkit,
    weapon::{
        Inventory, give,
        grenade::BlastAt,
        projectile::{Flyer, FlyerGone, SkyEnd, Touch, spawn_flyer},
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        MapBrush, MapCollision, MapConvex, MapData, MapEntity, MapHull, MapModel, MapPhysics, MapProp, MapSimPlugin,
        MapWaterVolume, PropSolid, PushAway,
    },
};
use hl2dm::{
    game::{
        movement,
        weapons::{
            self,
            launch::{self, Hl2Rounds, U, to_engine},
        },
    },
    harness::Hl2Sim,
};

/// Engine box from a Source box.
fn engine_box(lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    let (a, b) = (to_engine(lo), to_engine(hi));
    (a.min(b), a.max(b))
}

fn solid(data: &mut MapData, lo: Vec3, hi: Vec3) {
    let (a, b) = engine_box(lo, hi);
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
}

fn floor() -> MapData {
    let mut data = MapData {
        name: "test:blast_rules".into(),
        entity_scale: U,
        gravity: Some(600.0 * U),
        ..default()
    };
    solid(&mut data, Vec3::new(-4096.0, -4096.0, -256.0), Vec3::new(4096.0, 4096.0, 0.0));
    data
}

/// Water over the Source box.
fn water(data: &mut MapData, lo: Vec3, hi: Vec3) {
    let (a, b) = engine_box(lo, hi);
    data.water.push(MapWaterVolume {
        brush: MapBrush::from_box(a, b),
        slime: false,
    });
}

/// A `class` brush entity over the Source box, its origin at the centre.
fn door(data: &mut MapData, class: &str, lo: Vec3, hi: Vec3) {
    let c = (lo + hi) / 2.0;
    // A hull is in the entity's own units, about its origin.
    let (a, b) = (lo - c, hi - c);
    let brush = MapBrush::from_box(a, b);
    let points = (0..8)
        .map(|k| Vec3::new([a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]))
        .collect();
    data.entities.push(MapEntity {
        keyvalues: vec![
            ("classname".into(), class.into()),
            ("origin".into(), format!("{} {} {}", c.x, c.y, c.z)),
        ],
        hulls: vec![MapHull { planes: brush.planes, points }],
        mover: true,
        physics: None,
    });
}

fn start(data: MapData) -> Sim {
    let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim
}

/// A standing player with his feet at `feet`, 5000 health.
fn player(sim: &mut Sim, feet: Vec3) -> Entity {
    let p = sim.spawn_character(to_engine(feet + Vec3::Z * 36.0), movement::ID);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 50.0;
    p
}

/// Put `p` back with its feet at `feet`, at rest.
fn place(sim: &mut Sim, p: Entity, feet: Vec3) {
    let world = sim.app.world_mut();
    world.get_mut::<Transform>(p).unwrap().translation = to_engine(feet + Vec3::Z * 36.0);
    world.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
}

/// Damage `p` took (points) from `at`'s 150 in 250 blast, the point in
/// Source units.
fn blast_on(sim: &mut Sim, at: Vec3, players: &[Entity]) -> Vec<f32> {
    let before: Vec<f32> = players.iter().map(|p| sim.app.world().get::<Health>(*p).unwrap().current).collect();
    sim.app.world_mut().write_message(BlastAt {
        at: to_engine(at),
        blast: launch::blast(150.0, 250.0, None, None),
        thrower: None,
        weapon: weapons::FRAG,
        inflictor: None,
    });
    sim.ticks(3);
    players
        .iter()
        .zip(before)
        .map(|(p, h)| (h - sim.app.world().get::<Health>(*p).unwrap().current) * 100.0)
        .collect()
}

/// Spec 2 steps 2-3: under water (the point and 24 above it in water) a
/// blast reaches only those with their feet in water; in shallow water it
/// counts as dry; dry, it doesn't reach one wholly under.
#[test]
fn the_water_rule() {
    let mut data = floor();
    // Deep water east of x = 100, 200 deep (the floor sunk under it by the
    // tank's box: it needs none here, the players float on the floor).
    water(&mut data, Vec3::new(100.0, -500.0, 0.0), Vec3::new(500.0, 500.0, 200.0));
    // Shallow beside it: 16 deep, west.
    water(&mut data, Vec3::new(-500.0, -500.0, 0.0), Vec3::new(-100.0, 500.0, 16.0));
    let mut sim = start(data);
    sim.seconds(0.5);
    let (dry_at, wet_at, wading_at) = (Vec3::new(60.0, 400.0, 0.0), Vec3::new(180.0, 400.0, 0.0), Vec3::new(-180.0, 400.0, 0.0));
    let (dry, wet, wading) = (player(&mut sim, dry_at), player(&mut sim, wet_at), player(&mut sim, wading_at));
    let settle = |sim: &mut Sim| {
        place(sim, dry, dry_at);
        place(sim, wet, wet_at);
        place(sim, wading, wading_at);
        sim.ticks(2);
    };
    settle(&mut sim);
    // Under water, deep: the wet player yes, the dry one no.
    let hit = blast_on(&mut sim, Vec3::new(120.0, 400.0, 100.0), &[dry, wet]);
    assert_eq!(hit[0], 0.0, "dry, a blast under water: {hit:?}");
    assert!(hit[1] > 0.0, "wet, a blast under water: {hit:?}");
    // In the shallows (the point + 24 is out): as dry: both.
    settle(&mut sim);
    let hit = blast_on(&mut sim, Vec3::new(-120.0, 400.0, 8.0), &[dry, wading]);
    assert!(hit[0] > 0.0 && hit[1] > 0.0, "shallow counts as dry: {hit:?}");
    // Dry, in the air: not the one wholly under (his eye is in the deep
    // water), but the dry one beside.
    settle(&mut sim);
    let hit = blast_on(&mut sim, Vec3::new(120.0, 400.0, 230.0), &[dry, wet]);
    assert!(hit[0] > 0.0, "dry: {hit:?}");
    assert_eq!(hit[1], 0.0, "wholly under, a dry blast: {hit:?}");
}

/// Spec 2 step 3.4: a door on the line blocks the blast whole, moving or
/// not, a `func_breakable` of the same size between only shields (0.25).
#[test]
fn doors_block_a_blast_whole() {
    for (class, blocks) in [("func_door", true), ("func_door_rotating", true), ("func_movelinear", false)] {
        let mut data = floor();
        door(&mut data, class, Vec3::new(50.0, -200.0, 0.0), Vec3::new(60.0, 200.0, 200.0));
        let mut sim = start(data);
        sim.seconds(0.5);
        let p = player(&mut sim, Vec3::new(150.0, 0.0, 0.0));
        sim.ticks(30);
        let world = sim.app.world_mut();
        let node = world
            .query_filtered::<Entity, With<chimera_world::map::MapBrushEntity>>()
            .iter(world)
            .next()
            .expect("the entity");
        assert!(world.get::<RigidBody>(node).is_some_and(|r| !r.is_static()), "{class} moves");
        let hit = blast_on(&mut sim, Vec3::new(0.0, 0.0, 40.0), &[p]);
        if blocks {
            assert_eq!(hit[0], 0.0, "{class} blocks: {hit:?}");
        } else {
            assert!(hit[0] > 0.0, "{class} doesn't: {hit:?}");
        }
    }
}

/// A sky wall and a plain one, 250 ahead of a player looking east, whose
/// missile flies into it: (blasts made, missile gone).
fn missile_into(sky: bool) -> (usize, usize) {
    let mut data = floor();
    let x = 500.0;
    solid(&mut data, Vec3::new(x, -512.0, 0.0), Vec3::new(x + 64.0, 512.0, 512.0));
    if sky {
        // Its near face.
        let (a, b, c, d) = (
            to_engine(Vec3::new(x, -512.0, 0.0)),
            to_engine(Vec3::new(x, 512.0, 0.0)),
            to_engine(Vec3::new(x, 512.0, 512.0)),
            to_engine(Vec3::new(x, -512.0, 512.0)),
        );
        for tri in [[a, b, c], [a, c, d]] {
            data.sky_surfaces.extend(tri.map(|p| p.to_array()));
        }
    }
    let mut sim = start(data);
    let p = sim.spawn_character(to_engine(Vec3::Z * 36.0), movement::ID);
    sim.seconds(1.0);
    {
        let mut i = sim.intent(p);
        i.yaw = (-90f32).to_radians();
        i.pitch = 0.0;
    }
    let w = give(sim.app.world_mut(), p, weapons::RPG).unwrap();
    sim.app.world_mut().get_mut::<Hl2Rounds>(w).unwrap().count = 3;
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::RPG));
    let mut blasts = sim.app.world().resource::<Messages<BlastAt>>().get_cursor_current();
    let mut gone = sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let (mut n_blasts, mut n_gone) = (0, 0);
    for k in 0..100 {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        let world = sim.app.world();
        n_blasts += blasts.read(world.resource::<Messages<BlastAt>>()).count();
        n_gone += gone.read(world.resource::<Messages<FlyerGone>>()).filter(|g| g.weapon == weapons::RPG).count();
    }
    (n_blasts, n_gone)
}

/// Spec 6.5: a missile on a wall explodes; on the sky it is removed
/// silently (the RPG told, no blast).
#[test]
fn a_missile_on_the_sky_is_gone_silently() {
    assert_eq!(missile_into(false), (1, 1), "a wall");
    assert_eq!(missile_into(true), (0, 1), "the sky");
}

/// Spec 5.2: a ball ends at once on a sky surface, as its time running out.
#[test]
fn a_ball_on_the_sky_ends() {
    let mut data = floor();
    let x = 200.0;
    solid(&mut data, Vec3::new(x, -512.0, 0.0), Vec3::new(x + 64.0, 512.0, 512.0));
    let (a, b, c, d) = (
        to_engine(Vec3::new(x, -512.0, 0.0)),
        to_engine(Vec3::new(x, 512.0, 0.0)),
        to_engine(Vec3::new(x, 512.0, 512.0)),
        to_engine(Vec3::new(x, -512.0, 512.0)),
    );
    for tri in [[a, b, c], [a, c, d]] {
        data.sky_surfaces.extend(tri.map(|p| p.to_array()));
    }
    let mut sim = start(data);
    sim.seconds(0.5);
    let world = sim.app.world_mut();
    let clock = *world.resource::<chimera_core::core::SimClock>();
    let mut f = Flyer::new(weapons::AR2, None, to_engine(Vec3::new(1000.0, 0.0, 0.0)), &clock);
    f.half = Vec3::splat(10.0 * U);
    f.sphere = true;
    f.keep_speed = Some(1000.0 * U);
    f.touch = Touch::Bounce;
    f.sky_end = Some(SkyEnd { probe: 20.0 * U, expired: true });
    let mut commands = world.commands();
    let e = spawn_flyer(&mut commands, f, to_engine(Vec3::new(0.0, 0.0, 100.0)), weapons::launch::BALL_MODEL.into(), &clock);
    world.flush();
    let mut gone = sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let mut ended = None;
    for _ in 0..40 {
        sim.ticks(1);
        if let Some(g) = gone.read(sim.app.world().resource::<Messages<FlyerGone>>()).find(|g| g.flyer == e) {
            ended = Some(g.expired);
            break;
        }
    }
    assert_eq!(ended, Some(true), "ended as a ball's time running out");
}

/// A resting explosive drum's keys, as the install's prop data gives them
/// (health 20, 120 damage in 256 units: `oildrum001_explosive`).
fn drum(data: &mut MapData, at: Vec3) {
    let index = data.entities.len();
    data.entities.push(MapEntity {
        keyvalues: vec![
            ("classname".into(), "prop_physics_respawnable".into()),
            ("origin".into(), format!("{} {} {}", at.x, at.y, at.z)),
            ("chimera_prop_health".into(), "20".into()),
            ("chimera_prop_pieces".into(), "0".into()),
            ("chimera_prop_explode".into(), "120 256".into()),
        ],
        hulls: Vec::new(),
        mover: false,
        physics: None,
    });
    let (lo, hi) = (to_engine(Vec3::splat(-12.0)), to_engine(Vec3::splat(12.0)));
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    data.models.push(MapModel {
        bounds: (lo, hi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex {
                points: (0..8)
                    .map(|k| Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]))
                    .collect(),
                planes: Vec::new(),
            }],
            mass: 30.0,
            ..default()
        }),
        ..default()
    });
    data.props.push(MapProp {
        pose: None,
        ragdoll: None,
        model: data.models.len() - 1,
        translation: to_engine(at),
        rotation: Quat::IDENTITY,
        solid: PropSolid::Mesh,
        skybox: false,
        lighting: None,
        vertex_light: None,
        casts_shadow: false,
        physics: Some(MapPhysics {
            mass: 30.0,
            friction: 0.8,
            elasticity: 0.0,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: None,
            push: PushAway::Collide,
            frozen: false,
            asleep: false,
        }),
        fade: None,
        parent: None,
        entity: Some(index),
        skin: 0,
        body: 0,
        detail: false,
    });
}

#[derive(Resource, Default)]
struct Blasts {
    detonated: Vec<&'static str>,
    damage: Vec<(f32, Option<&'static str>)>,
    sounds: Vec<String>,
    decals: usize,
}

fn watch_blasts(
    mut detonated: MessageReader<chimera_sim::weapon::grenade::Detonated>,
    mut damage: MessageReader<chimera_core::core::Damage>,
    mut sounds: MessageReader<chimera_world::map::PlaySound>,
    mut decals: MessageReader<chimera_world::map::PlaceDecal>,
    mut log: ResMut<Blasts>,
) {
    log.detonated.extend(detonated.read().map(|d| d.weapon));
    log.damage.extend(damage.read().map(|d| (d.amount, d.weapon)));
    log.sounds.extend(sounds.read().map(|s| s.entry.clone()));
    log.decals += decals.read().count();
}

/// The playtest's report (prop_damage.md 7.5, effects_sounds.md 8): an
/// explosive drum that breaks hurts a player by every explosion's rule,
/// explodes where it can be seen and heard, leaves its scorch, and its kill
/// names the prop. HL2:DM had no rule for map blasts, so they did nothing.
#[test]
fn an_explosive_drum_goes_off() {
    let mut data = floor();
    let at = Vec3::new(60.0, 0.0, 12.0);
    drum(&mut data, at);
    let mut sim = start(data);
    sim.app.init_resource::<Blasts>().add_systems(FixedLast, watch_blasts);
    let p = player(&mut sim, Vec3::ZERO);
    sim.seconds(0.6);
    {
        let mut logic = sim.app.world_mut().resource_mut::<source_logic::logic::Logic>();
        let id = logic.world.by_map_index(0).expect("the drum");
        logic.world.queue_direct(id, "Break", source_logic::logic::Value::Void, 0.0, None);
    }
    sim.ticks(4);
    let log = sim.app.world().resource::<Blasts>();
    assert_eq!(log.detonated, ["prop_physics"], "it went off");
    let &(amount, weapon) = log.damage.iter().find(|(a, _)| *a > 0.0).expect("the player was hurt");
    assert_eq!(weapon, Some("prop_physics"), "the death notice's");
    // The line from the drum to his eye enters his box 44 units along x
    // of 60: about 54 units, 120 × (1 − 54 / 256).
    let points = amount * 100.0;
    assert!((points - 94.7).abs() < 3.0, "{points} points");
    assert!(log.decals >= 1, "its scorch");
    assert!(sim.app.world().get::<Health>(p).unwrap().current < 50.0);
}

/// Spec 2.2: a blast's hearing. A health part of 30 shocks; below it, a
/// player whose origin is within 240 units of the blast rings; farther
/// (hurt or not), nothing. Each stands on a side of the blast so none
/// shields another.
#[test]
fn a_blast_rings_or_shocks_the_ears_near_it() {
    let mut sim = start(floor());
    sim.seconds(0.5);
    let centre = Vec3::new(0.0, 400.0, 0.0);
    let (shocked, rung, hurt_far, clear) = (
        centre + Vec3::new(50.0, 0.0, 0.0),
        centre + Vec3::new(-200.0, 0.0, 0.0),
        centre + Vec3::new(0.0, 245.0, 0.0),
        centre + Vec3::new(0.0, -400.0, 0.0),
    );
    let ids: Vec<Entity> = [shocked, rung, hurt_far, clear].iter().map(|&at| player(&mut sim, at)).collect();
    for (p, at) in ids.iter().zip([shocked, rung, hurt_far, clear]) {
        place(&mut sim, *p, at);
    }
    sim.ticks(2);
    deaf_log(&mut sim);
    // 100 damage over 250 units: 80 at the first (shock), 20 at 200
    // (inside 240, below 30: rings), 2 at 245 (hurt, too far to ring), none
    // past the radius.
    sim.app.world_mut().write_message(BlastAt {
        at: to_engine(centre),
        blast: launch::blast(100.0, 250.0, None, None),
        thrower: None,
        weapon: weapons::FRAG,
        inflictor: None,
    });
    sim.ticks(3);
    let got = sim.app.world().resource::<DeafLog>().0.clone();
    let of = |p: Entity| got.iter().find(|d| d.target == p).map(|d| d.effect);
    assert_eq!(of(ids[0]), Some(chimera_sim::weapon::grenade::SHOCK_MUFFLE), "a health part of 30 or more shocks");
    assert_eq!(of(ids[1]), Some(chimera_sim::weapon::grenade::EXPLOSION_RING), "within 240, below 30: rings");
    assert_eq!(of(ids[2]), None, "hurt but past 240: nothing");
    assert_eq!(of(ids[3]), None, "outside the radius: nothing");
}

/// What `Deafened` messages the blast sent.
#[derive(Resource, Default)]
struct DeafLog(Vec<chimera_core::core::Deafened>);

/// Start collecting `Deafened`.
fn deaf_log(sim: &mut Sim) {
    fn collect(mut m: MessageReader<chimera_core::core::Deafened>, mut log: ResMut<DeafLog>) {
        log.0.extend(m.read().copied());
    }
    sim.app.init_resource::<DeafLog>().add_systems(Update, collect);
}
