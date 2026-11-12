//! The gravity gun, `+use` and physics damage on `Hl2Sim`
//! (specs/hl2dm/gravity_gun.md, H5's implementer cases: "case N"), on a
//! small map in Source units: a floor at z = 0 and box props ("crates",
//! ±10 units, 30 kg unless said) placed relative to the player's eye,
//! which looks along +X (Source and engine). Ticks at 0.015 s. The pure
//! numbers (cases 1-3, 12, 18-20, 23, 27, 28) are `physcannon`'s unit
//! tests.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Died, Health, Hitgroup, MovementState, SimClock};
use chimera_sim::weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind, WeaponState};
use chimera_world::{
    harness::Sim,
    map::{
        MapBrush, MapCollision, MapConvex, MapData, MapEntity, MapModel, MapPhysics, MapProp, MapSimPlugin, PlaySound,
        PropEntity, PropSolid, PushAway,
        held::{Held, PhysicsInfluence},
    },
};
use hl2dm::{
    game::{
        movement,
        weapons::{
            self,
            frag::Hl2Grenade,
            launch::{U, to_engine, to_source},
            physcannon::{self, Hl2Physcannon},
        },
    },
    harness::Hl2Sim,
};

/// Looking east: engine and Source +X.
pub(super) const EAST: f32 = -90.0;
/// The player's feet (Source).
pub(super) const FEET: Vec3 = Vec3::new(0.0, 0.0, 0.0);

/// A prop to place: its centre relative to the player's eye (Source), its
/// half size, mass, spawnflags and whether it starts pinned (flag 8).
#[derive(Clone, Copy)]
pub(super) struct Crate {
    at: Vec3,
    half: Vec3,
    mass: f32,
    flags: u32,
    /// Its rotational inertia (engine axes, kg m²); None: a solid box's.
    inertia: Option<Vec3>,
    /// A `prop_physics_multiplayer` (mode 1, solid) instead.
    multiplayer: bool,
    /// More keyvalues (the model's data, `PROP_*_KEY`).
    keys: &'static [(&'static str, &'static str)],
}

impl Crate {
    pub(super) fn ahead(x: f32) -> Self {
        Crate {
            at: Vec3::new(x, 0.0, 0.0),
            half: Vec3::splat(10.0),
            mass: 30.0,
            flags: 0,
            inertia: None,
            multiplayer: false,
            keys: &[],
        }
    }
    fn mass(self, mass: f32) -> Self {
        Crate { mass, ..self }
    }
    fn flags(self, flags: u32) -> Self {
        Crate { flags, ..self }
    }
    fn at(self, at: Vec3) -> Self {
        Crate { at, ..self }
    }
    fn keys(self, keys: &'static [(&'static str, &'static str)]) -> Self {
        Crate { keys, ..self }
    }
}

/// The eye's height above the feet (Source).
const EYE: f32 = 64.0;

pub(super) fn solid(data: &mut MapData, lo: Vec3, hi: Vec3) {
    let (a, b) = (to_engine(lo), to_engine(hi));
    let (a, b) = (a.min(b), a.max(b));
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
}

/// A crate, on a thin pillar when it floats (so it stays where it is
/// put: the cases place crates at eye height), unless motion-disabled
/// (flag 8: it hangs).
pub(super) fn add_crate(data: &mut MapData, c: Crate) {
    let index = data.entities.len();
    let centre = FEET + Vec3::Z * EYE + c.at;
    let bottom = centre.z - c.half.z;
    if bottom > 1.0 && c.flags & 8 == 0 {
        solid(data, Vec3::new(centre.x - 4.0, centre.y - 4.0, 0.0), Vec3::new(centre.x + 4.0, centre.y + 4.0, bottom - 0.05));
    }
    let mut keyvalues: Vec<(String, String)> = vec![
        (
            "classname".into(),
            if c.multiplayer { "prop_physics_multiplayer" } else { "prop_physics" }.into(),
        ),
        ("origin".into(), format!("{} {} {}", centre.x, centre.y, centre.z)),
        ("spawnflags".into(), c.flags.to_string()),
    ];
    keyvalues.extend(c.keys.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    data.entities.push(MapEntity {
        keyvalues,
        hulls: Vec::new(),
        mover: false,
        physics: None,
    });
    let (lo, hi) = (to_engine(-c.half), to_engine(c.half));
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let corners = (0..8)
        .map(|k| Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]))
        .collect();
    data.models.push(MapModel {
        bounds: (lo, hi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex { points: corners, planes: Vec::new() }],
            mass: c.mass,
            ..default()
        }),
        ..default()
    });
    data.props.push(MapProp {
        pose: None,
        ragdoll: None,
        model: data.models.len() - 1,
        translation: to_engine(centre),
        rotation: Quat::IDENTITY,
        solid: PropSolid::Mesh,
        skybox: false,
        lighting: None,
        vertex_light: None,
        casts_shadow: false,
        physics: Some(MapPhysics {
            mass: c.mass,
            friction: 0.8,
            elasticity: 0.0,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: c.inertia,
            push: if c.multiplayer { PushAway::Solid } else { PushAway::Collide },
            frozen: c.flags & 8 != 0,
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

/// What happened, by tick.
#[derive(Resource, Default)]
struct Log {
    sounds: Vec<(u64, String)>,
    events: Vec<(u64, Entity, WeaponEventKind)>,
    died: Vec<(u64, Entity, Option<Entity>)>,
}

fn listen(
    clock: Res<SimClock>,
    mut log: ResMut<Log>,
    mut sounds: MessageReader<PlaySound>,
    mut events: MessageReader<WeaponEvent>,
    mut died: MessageReader<Died>,
) {
    let t = clock.tick;
    log.sounds.extend(sounds.read().map(|s| (t, s.entry.clone())));
    log.events.extend(events.read().filter(|e| !e.replay).map(|e| (t, e.owner, e.kind.clone())));
    log.died.extend(died.read().map(|d| (t, d.entity, d.attacker)));
}

pub(super) struct W {
    pub(super) sim: Sim,
    pub(super) p: Entity,
}

/// The map with `crates` (and walls), the player at `FEET` looking east
/// with the gravity gun in hand (the spawn's).
fn world_with(crates: &[Crate], walls: &[(Vec3, Vec3)]) -> W {
    world_on(crates, walls, (Vec3::new(-4096.0, -4096.0, -64.0), Vec3::new(4096.0, 4096.0, 0.0)))
}

/// `world_with` on the floor `floor` (lowest and highest corners).
fn world_on(crates: &[Crate], walls: &[(Vec3, Vec3)], floor: (Vec3, Vec3)) -> W {
    world_of(map_data(crates, walls, floor))
}

/// The map: `floor`, `walls`, `crates`.
pub(super) fn map_data(crates: &[Crate], walls: &[(Vec3, Vec3)], floor: (Vec3, Vec3)) -> MapData {
    let mut data = MapData {
        name: "test:gravity_gun".into(),
        entity_scale: U,
        // HL2:DM's physics gravity, as its maps load (sv_gravity 600).
        gravity: Some(600.0 * U),
        ..default()
    };
    solid(&mut data, floor.0, floor.1);
    for (lo, hi) in walls {
        solid(&mut data, *lo, *hi);
    }
    for c in crates {
        add_crate(&mut data, *c);
    }
    data
}

/// `data` loaded, the player at `FEET` looking east with the gravity gun
/// in hand (the spawn's), settled.
pub(super) fn world_of(data: MapData) -> W {
    let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim.app.init_resource::<Log>().add_systems(FixedLast, listen);
    let p = sim.spawn_character(to_engine(FEET + Vec3::Z * 36.0), movement::ID);
    look(&mut sim, p, EAST, 0.0);
    sim.seconds(1.0);
    let mut w = W { sim, p };
    w.settle_eye();
    w
}

fn world(crates: &[Crate]) -> W {
    world_with(crates, &[])
}

pub(super) fn look(sim: &mut Sim, p: Entity, yaw: f32, pitch: f32) {
    let mut i = sim.intent(p);
    i.yaw = yaw.to_radians();
    i.pitch = pitch.to_radians();
}

impl W {
    pub(super) fn tick(&self) -> u64 {
        self.sim.app.world().resource::<SimClock>().tick
    }

    pub(super) fn eye(&self) -> Vec3 {
        let w = self.sim.app.world();
        to_source(self.sim.position(self.p) + w.get::<MovementState>(self.p).unwrap().eye_offset)
    }

    /// The crates were placed from the nominal eye; put them where the
    /// player's real eye is (its height after settling).
    fn settle_eye(&mut self) {
        let dz = self.eye().z - (FEET.z + EYE);
        let world = self.sim.app.world_mut();
        let nodes: Vec<Entity> = world.query_filtered::<Entity, With<PropEntity>>().iter(world).collect();
        for n in nodes {
            let Some(mut pos) = world.get_mut::<Position>(n) else { continue };
            let up = to_engine(Vec3::Z * dz);
            pos.0 += up;
            if let Some(mut t) = world.get_mut::<Transform>(n) {
                t.translation += up;
            }
            if let Some(mut v) = world.get_mut::<LinearVelocity>(n) {
                v.0 = Vec3::ZERO;
            }
        }
    }

    /// Map prop `i` (in placing order).
    pub(super) fn node(&mut self, i: usize) -> Entity {
        let world = self.sim.app.world_mut();
        world
            .query::<(Entity, &PropEntity)>()
            .iter(world)
            .find(|(_, p)| p.0 == i)
            .unwrap()
            .0
    }

    /// Where prop `i`'s centre is (Source).
    pub(super) fn at(&mut self, i: usize) -> Vec3 {
        let n = self.node(i);
        to_source(self.sim.position(n))
    }

    /// Prop `i`'s velocity (Source units/s).
    pub(super) fn velocity(&mut self, i: usize) -> Vec3 {
        let n = self.node(i);
        self.sim
            .app
            .world()
            .get::<LinearVelocity>(n)
            .map_or(Vec3::ZERO, |v| to_source(v.0))
    }

    fn gun(&self) -> Entity {
        let w = self.sim.app.world();
        w.get::<Inventory>(self.p)
            .unwrap()
            .weapons
            .iter()
            .copied()
            .find(|e| w.get::<Hl2Physcannon>(*e).is_some())
            .unwrap()
    }

    fn state(&self) -> WeaponState {
        self.sim.app.world().get::<WeaponState>(self.gun()).unwrap().clone()
    }

    pub(super) fn holding(&self) -> bool {
        self.sim.app.world().get::<Hl2Physcannon>(self.gun()).unwrap().holding
    }

    fn now(&self) -> f64 {
        self.sim.app.world().resource::<SimClock>().now
    }

    /// Press (`fire`, `secondary`) for `hold` ticks, then let go; run
    /// `n` ticks in all.
    pub(super) fn press(&mut self, fire: bool, secondary: bool, hold: u64, n: u64) {
        for k in 0..n {
            {
                let mut i = self.sim.intent(self.p);
                i.fire = fire && k < hold;
                i.secondary = secondary && k < hold;
            }
            self.sim.ticks(1);
        }
        let mut i = self.sim.intent(self.p);
        i.fire = false;
        i.secondary = false;
    }

    fn sounds(&self, entry: &str, since: u64) -> Vec<u64> {
        let log = self.sim.app.world().resource::<Log>();
        log.sounds.iter().filter(|(t, s)| *t >= since && s == entry).map(|(t, _)| *t).collect()
    }

    fn dry_fires(&self, since: u64) -> usize {
        let log = self.sim.app.world().resource::<Log>();
        log.events
            .iter()
            .filter(|(t, o, k)| *t >= since && *o == self.p && matches!(k, WeaponEventKind::DryFire))
            .count()
    }

    /// Grab prop `i` with a secondary click (and let go of the button).
    pub(super) fn grab(&mut self, i: usize) -> Entity {
        self.press(false, true, 1, 2);
        let n = self.node(i);
        assert!(self.sim.app.world().get::<Held>(n).is_some(), "grabbed");
        n
    }
}

/// The spawn (weapons.md 3, test 1; gravity_gun.md 1): the gravity gun in
/// hand, the SMG1 the last weapon, two grenades.
#[test]
fn the_spawn_holds_the_gravity_gun() {
    let w = world(&[]);
    let world = w.sim.app.world();
    assert_eq!(chimera_sim::testkit::active_id(world, w.p), Some(weapons::PHYSCANNON));
    let inv = world.get::<Inventory>(w.p).unwrap();
    let last = inv.last.and_then(|l| world.get::<Weapon>(l)).map(|x| x.id);
    assert_eq!(last, Some(weapons::SMG1));
    let frag = inv
        .weapons
        .iter()
        .find(|e| world.get::<Weapon>(**e).is_some_and(|x| x.id == weapons::FRAG))
        .unwrap();
    assert_eq!(world.get::<weapons::launch::Hl2Rounds>(*frag).unwrap().count, 2);
}

/// Case 1: a punt of a 30 kg crate at 100 units sends it at 1100 in/s
/// along +X; the next primary and secondary 34 ticks later.
#[test]
fn a_punt_sends_a_crate_at_1100() {
    let mut w = world(&[Crate::ahead(100.0)]);
    let t0 = w.tick();
    w.press(true, false, 1, 1);
    let v = w.velocity(0);
    assert!((v.x - 1100.0).abs() < 15.0 && v.y.abs() < 5.0, "{v}");
    let st = w.state();
    let ticks = ((st.next_primary - w.now()) / 0.015).round() as i64;
    assert_eq!(ticks, 33, "next primary 34 ticks after the punt's");
    assert_eq!(((st.next_secondary - w.now()) / 0.015).round() as i64, 33);
    assert_eq!(w.sounds("Weapon_PhysCannon.Launch", t0).len(), 1);
    let n = w.node(0);
    let i = w.sim.app.world().get::<PhysicsInfluence>(n).copied().unwrap();
    assert_eq!(i.by, w.p);
}

/// Case 2: 10 kg leaves at the 2000 cap, 250 kg at 660.
#[test]
fn light_crates_leave_at_the_cap_heavy_slowly() {
    for (mass, want) in [(10.0, 2000.0), (250.0, 660.0)] {
        let mut w = world(&[Crate::ahead(100.0).mass(mass)]);
        w.press(true, false, 1, 1);
        let v = w.velocity(0);
        assert!((v.x - want).abs() < want * 0.015, "{mass} kg: {v}");
    }
}

/// Case 3: looking 45 degrees down at a crate, the punt goes up.
#[test]
fn a_downward_punt_goes_up() {
    let mut w = world(&[Crate::ahead(70.0).at(Vec3::new(70.0, 0.0, -54.0))]);
    look(&mut w.sim, w.p, EAST, -45.0);
    w.sim.ticks(2);
    w.press(true, false, 1, 1);
    let v = w.velocity(0);
    assert!(v.z > 450.0 && (v.x - 777.8).abs() < 40.0, "{v}");
}

/// Case 3, measured on the real server (the plan's Progress 2026-11-03
/// "probe 2"): a 30 kg wood crate (40 units, its inertia 0.714 of a solid
/// box's, measured) hanging motion-disabled (flags 8 + 64) 100 ahead, its
/// centre 40 below the eye; looking 21.8 degrees down at it: 1011 forward,
/// 252 up a tick after (the spec's 1100 × (0.928, 0.65 × 0.371) less a
/// tick of drag and gravity) and pitching at 2964 degrees/s, the point
/// impulse's turn about the box trace's end (its centre where it stopped,
/// 8 units short of the face), not the contact point (2100/s). The
/// player stands on a pad: nothing under the crate.
#[test]
fn a_hanging_crate_punted_from_above() {
    let s = 40.4 * 0.0254;
    let mut c = Crate::ahead(100.0).at(Vec3::new(100.0, 0.0, -40.0)).flags(72);
    c.half = Vec3::splat(20.2);
    c.inertia = Some(Vec3::splat(0.714 * 30.0 * s * s / 6.0));
    let pad = (Vec3::new(-32.0, -32.0, -64.0), Vec3::new(32.0, 32.0, 0.0));
    let mut w = world_on(&[c], &[], pad);
    look(&mut w.sim, w.p, EAST, -21.8);
    w.sim.ticks(2);
    w.press(true, false, 1, 1);
    let v = w.velocity(0);
    assert!((v.x - 1011.0).abs() < 15.0 && (v.z - 252.0).abs() < 10.0 && v.y.abs() < 5.0, "{v}");
    let n = w.node(0);
    let spin = to_source(w.sim.app.world().get::<AngularVelocity>(n).unwrap().0) * 0.0254;
    let pitch = spin.y.to_degrees();
    assert!((pitch - 2964.0).abs() < 150.0, "pitching at {pitch} degrees/s ({spin})");
}

/// Case 3 on the floor, measured on the real server (2026-11-04,
/// `cases.py floor_punts`): the 30 kg wood crate resting on the floor
/// (centre 20.4 up), 40 ahead, punted looking 45 degrees down, leaves at
/// (695, 871) on its second tick (1114 in/s, 51.4 degrees up; its first
/// tick's step (773, 898) while its leading edge digs into the floor) and
/// pitching at about 1490 degrees/s. Ours leaves at (761, 792) (1098 in/s,
/// 46 degrees up) pitching at 1670: the floor contact (avian's, not
/// vphysics') turns less of the spin into lift. Pinned: the speed within
/// 3 %, the climb within 6 degrees (gravity_gun.md 11, the plan's open
/// item).
#[test]
fn a_crate_on_the_floor_punted_from_above() {
    let s = 40.4 * 0.0254;
    let mut c = Crate::ahead(40.0).at(Vec3::new(40.0, 0.0, 20.4 - EYE));
    c.half = Vec3::splat(20.2);
    c.inertia = Some(Vec3::splat(0.714 * 30.0 * s * s / 6.0));
    let mut w = world(&[c]);
    look(&mut w.sim, w.p, EAST, -45.0);
    w.sim.seconds(1.0);
    w.press(true, false, 1, 1);
    let first = w.at(0);
    w.sim.ticks(1);
    let v = (w.at(0) - first) / 0.015;
    let (speed, climb) = (v.length(), v.z.atan2(v.x).to_degrees());
    assert!((speed - 1114.0).abs() < 1114.0 * 0.03 && (climb - 51.4).abs() < 6.0, "{v}: {speed} in/s, {climb} up");
}

/// Case 4: a crate at 270 is out of reach (the spec's 260 puts its face
/// at 250, which the ±8 box reaches at 242: the plan's Progress notes it): a dry fire, unchanged; next
/// primary + 34, secondary unchanged.
#[test]
fn out_of_reach_is_a_dry_fire() {
    let mut w = world(&[Crate::ahead(270.0)]);
    let before = w.at(0);
    let secondary = w.state().next_secondary;
    let t0 = w.tick();
    w.press(true, false, 1, 1);
    assert_eq!(w.dry_fires(t0), 1);
    assert_eq!(w.sounds("Weapon_PhysCannon.DryFire", t0).len(), 1);
    assert!(w.at(0).distance(before) < 0.5);
    assert_eq!(w.state().next_secondary, secondary);
    assert_eq!(((w.state().next_primary - w.now()) / 0.015).round() as i64, 33);
}

/// Cases 5, 6: a player in front is a dry fire, his velocity and health
/// unchanged, and shields a crate behind him.
#[test]
fn players_are_never_punted() {
    let mut w = world(&[Crate::ahead(150.0)]);
    let b = w.sim.spawn_character(to_engine(FEET + Vec3::new(80.0, 0.0, 36.0)), movement::ID);
    w.sim.ticks(30);
    let t0 = w.tick();
    w.press(true, false, 1, 3);
    assert_eq!(w.dry_fires(t0), 1);
    assert_eq!(w.sim.app.world().get::<Health>(b).unwrap().current, 1.0);
    assert!(w.velocity(0).length() < 5.0);
}

/// Case 8: primary held punts on ticks 0, 34, 68, ...
#[test]
fn held_primary_punts_every_34_ticks() {
    let mut w = world(&[]);
    let t0 = w.tick();
    w.press(true, false, 120, 120);
    let at: Vec<u64> = w
        .sim
        .app
        .world()
        .resource::<Log>()
        .events
        .iter()
        .filter(|(t, ..)| *t >= t0)
        .filter(|(_, _, k)| matches!(k, WeaponEventKind::DryFire))
        .map(|(t, ..)| *t)
        .collect();
    let first = at[0];
    assert_eq!(at.iter().map(|t| t - first).collect::<Vec<_>>(), vec![0, 34, 68, 102]);
}

/// Case 10: a crate at 200 is grabbed: held, `.Pickup`, 1 kg, damping
/// 10, next secondary + 34.
#[test]
fn a_crate_at_200_is_grabbed() {
    let mut w = world(&[Crate::ahead(200.0)]);
    let t0 = w.tick();
    w.press(false, true, 1, 1);
    let n = w.node(0);
    let world = w.sim.app.world();
    assert!(w.holding());
    assert_eq!(world.get::<Held>(n).map(|h| h.holder), Some(w.p));
    assert_eq!(world.get::<Mass>(n).unwrap().0, 1.0);
    assert_eq!(world.get::<AngularDamping>(n).unwrap().0, 10.0);
    assert_eq!(w.sounds("Weapon_PhysCannon.Pickup", t0).len(), 1);
    assert_eq!(((w.state().next_secondary - w.now()) / 0.015).round() as i64, 33);
    // It comes to the hold point: 24 + 22.627 + 10 ahead of the eye.
    w.sim.seconds(1.0);
    let rel = w.at(0) - w.eye();
    assert!((rel.x - 56.627).abs() < 2.0 && rel.y.abs() < 1.0 && rel.z.abs() < 2.0, "{rel}");
}

/// Case 11: a crate at 400 is pulled every 7 ticks, 81.3 in/s a pull
/// toward the eye; TooHeavy never plays.
#[test]
fn a_far_crate_is_pulled() {
    // On the floor 400 ahead, looked at.
    let mut w = world(&[Crate::ahead(400.0).at(Vec3::new(400.0, 0.0, -54.0))]);
    look(&mut w.sim, w.p, EAST, -(54f32 / 400.0).atan().to_degrees());
    w.sim.ticks(2);
    let t0 = w.tick();
    let mut jumps = Vec::new();
    let mut last = w.velocity(0);
    for k in 0..22 {
        w.sim.intent(w.p).secondary = true;
        w.sim.ticks(1);
        let v = w.velocity(0);
        if (v - last).length() > 40.0 {
            jumps.push((k, (v - last).length()));
        }
        last = v;
    }
    let ticks: Vec<u64> = jumps.iter().map(|j| j.0).collect();
    assert_eq!(ticks, vec![0, 7, 14, 21]);
    assert!((jumps[0].1 - 81.33).abs() < 3.0, "{jumps:?}");
    assert!(w.sounds("Weapon_PhysCannon.TooHeavy", t0).is_empty());
    assert!(w.velocity(0).x < -200.0, "toward the player");
}

/// P8 (probe, 2026-11-04, `cases.py pulls`): secondary held at the 30 kg
/// wood crate (40 units) resting on the floor 600 ahead (looking 4.2
/// down) tugs it every 7 ticks; its floor speed by tick (the steps) on the
/// real server, from the first tug: 75.7, 72.0, 66.2, 62.4, 57.3, 51.9,
/// 46.6, then 121.8 ... 92.5, 167.5 ... 137.3, 212.0: each tug + 81.3 less
/// a tick of the floor's friction (about 5.5 in/s a tick, 0.6 g).
#[test]
fn a_crate_pulled_from_600_as_the_probe_measured() {
    let s = 40.4 * 0.0254;
    let mut c = Crate::ahead(600.0).at(Vec3::new(600.0, 0.0, 20.4 - EYE));
    c.half = Vec3::splat(20.2);
    c.inertia = Some(Vec3::splat(0.714 * 30.0 * s * s / 6.0));
    let mut w = world(&[c]);
    look(&mut w.sim, w.p, EAST, -4.2);
    w.sim.seconds(0.5);
    let mut steps = Vec::new();
    let mut last = w.at(0);
    for _ in 0..23 {
        w.sim.intent(w.p).secondary = true;
        w.sim.ticks(1);
        let at = w.at(0);
        steps.push(((at - last).truncate().length() / 0.015 * 10.0).round() / 10.0);
        last = at;
    }
    eprintln!("pulled: {steps:?}");
    let want = [
        75.7, 72.0, 66.2, 62.4, 57.3, 51.9, 46.6, 121.8, 118.9, 114.6, 109.0, 103.5, 98.0, 92.5, 167.5, 164.4, 160.0,
        154.2, 148.6, 142.9, 137.3, 212.0, 208.6,
    ];
    let off: Vec<(usize, f32, f32)> =
        steps.iter().zip(want).enumerate().filter(|(_, (a, b))| (*a - b).abs() > 6.0).map(|(k, (a, b))| (k, *a, b)).collect();
    assert!(off.is_empty(), "(tick, ours, real) off by more than 6 in/s: {off:?}");
}

/// Case 13: a 300 kg crate: TooHeavy once a press, nothing moved.
#[test]
fn too_heavy_once_a_press() {
    let mut w = world(&[Crate::ahead(100.0).mass(300.0)]);
    let t0 = w.tick();
    w.press(false, true, 30, 40);
    w.press(false, true, 30, 40);
    assert_eq!(w.sounds("Weapon_PhysCannon.TooHeavy", t0).len(), 2);
    assert!(w.velocity(0).length() < 1.0);
    assert!(!w.holding());
}

/// Case 14: a wall at 50, nothing else: TooHeavy once; next secondary 7
/// ticks on; the claws closed.
#[test]
fn a_wall_is_too_heavy() {
    let mut w = world_with(&[], &[(Vec3::new(50.0, -500.0, 0.0), Vec3::new(60.0, 500.0, 500.0))]);
    let t0 = w.tick();
    w.press(false, true, 1, 1);
    assert_eq!(w.sounds("Weapon_PhysCannon.TooHeavy", t0).len(), 1);
    assert_eq!(((w.state().next_secondary - w.now()) / 0.015).round() as i64, 7);
    assert!(!w.sim.app.world().get::<Hl2Physcannon>(w.gun()).unwrap().claws);
}

/// Cases 16, 17: a pinned crate is refused, with 64 it is grabbed and
/// moves; 512 is refused but punted.
#[test]
fn spawnflags_decide() {
    let mut w = world(&[Crate::ahead(100.0).flags(8)]);
    let t0 = w.tick();
    w.press(false, true, 1, 2);
    assert!(!w.holding());
    assert_eq!(w.sounds("Weapon_PhysCannon.TooHeavy", t0).len(), 1);

    let mut w = world(&[Crate::ahead(100.0).flags(8 | 64)]);
    w.press(false, true, 1, 2);
    assert!(w.holding());
    w.sim.seconds(0.5);
    assert!((w.at(0) - w.eye()).x < 70.0, "pulled in to the hold point");

    let mut w = world(&[Crate::ahead(100.0).flags(512)]);
    w.press(false, true, 1, 2);
    assert!(!w.holding());
    w.sim.seconds(0.6);
    w.press(true, false, 1, 1);
    assert!(w.velocity(0).x > 1000.0, "punted");
}

/// Case 21 in the scene: the view turns 90 degrees left, the crate comes
/// round to the new forward and turns with it.
#[test]
fn the_held_crate_follows_the_view() {
    let mut w = world(&[Crate::ahead(150.0)]);
    let n = w.grab(0);
    w.sim.seconds(1.0);
    look(&mut w.sim, w.p, 0.0, 0.0);
    w.sim.ticks(1);
    // At most 1000 in/s on the way.
    let v = w.velocity(0);
    assert!(v.length() <= 1000.0 + 1.0, "{v}");
    w.sim.seconds(0.5);
    let rel = w.at(0) - w.eye();
    assert!((rel.y - 56.627).abs() < 3.0 && rel.x.abs() < 3.0, "{rel}");
    let turn = w.sim.app.world().get::<Rotation>(n).unwrap().0;
    let (axis, angle) = turn.to_axis_angle();
    assert!((angle.abs() - std::f32::consts::FRAC_PI_2).abs() < 0.1 && axis.y.abs() > 0.9, "{axis} {angle}");
}

/// Case 22: a launch (possible the tick after the grab): 1500 along the view, spin
/// within 600 deg/s a axis, the mass back, next primary + 34, `.Launch`.
#[test]
fn a_launch_at_1500() {
    let mut w = world(&[Crate::ahead(150.0)]);
    w.press(false, true, 1, 1);
    // A launch could go the next tick (the grab leaves next primary).
    assert!(w.state().next_primary <= w.now() + 0.015);
    // (Held off its pillar first, so its spin doesn't catch it.)
    w.sim.seconds(1.0);
    let t0 = w.tick();
    w.press(true, false, 1, 1);
    let n = w.node(0);
    let v = w.velocity(0);
    assert!((v.x - 1500.0).abs() < 10.0 && v.y.abs() < 5.0, "{v}");
    let world = w.sim.app.world();
    assert!(world.get::<Held>(n).is_none());
    assert_eq!(world.get::<Mass>(n).unwrap().0, 30.0);
    let spin = world.get::<AngularVelocity>(n).unwrap().0;
    assert!(spin.max_element() <= 600f32.to_radians() + 1e-3 && spin.min_element() >= -600f32.to_radians() - 1e-3);
    assert_eq!(w.sounds("Weapon_PhysCannon.Launch", t0).len(), 1);
    assert_eq!(((w.state().next_primary - w.now()) / 0.015).round() as i64, 33);
}

/// Case 23: 250 kg leaves at 1327.2.
#[test]
fn a_heavy_launch_is_slower() {
    let mut w = world(&[Crate::ahead(150.0).mass(250.0)]);
    w.press(false, true, 1, 1);
    w.sim.seconds(1.0);
    w.press(true, false, 1, 1);
    let v = w.velocity(0);
    assert!((v.x - 1327.2).abs() < 10.0, "{v}");
}

/// Case 25: a drop of a crate swung fast keeps at most 285 in/s; `.Drop`,
/// next + 34, the influence the holder's.
#[test]
fn a_drop_caps_the_speed() {
    let mut w = world(&[Crate::ahead(150.0)]);
    let n = w.grab(0);
    w.sim.seconds(1.0);
    look(&mut w.sim, w.p, 0.0, 0.0);
    w.sim.ticks(2);
    let t0 = w.tick();
    w.press(false, true, 1, 1);
    assert!(!w.holding());
    assert!(w.velocity(0).length() <= 285.0 + 1.0, "{}", w.velocity(0));
    assert_eq!(w.sounds("Weapon_PhysCannon.Drop", t0).len(), 1);
    assert_eq!(((w.state().next_primary - w.now()) / 0.015).round() as i64, 33);
    let i = w.sim.app.world().get::<PhysicsInfluence>(n).copied().unwrap();
    assert_eq!(i.by, w.p);
    assert!((w.now() - i.at).abs() < 0.02);
}

/// Case 26: a second click 20 ticks after the grab is lost; held on,
/// TooHeavy at tick 34 once, the claws closing; still held.
#[test]
fn an_early_second_click_is_lost() {
    let mut w = world(&[Crate::ahead(150.0)]);
    let t0 = w.tick();
    w.press(false, true, 1, 20);
    assert!(w.holding());
    w.press(false, true, 40, 40);
    assert!(w.holding(), "still held");
    // (The grab's tick is t0 + 1.)
    let too = w.sounds("Weapon_PhysCannon.TooHeavy", t0);
    assert_eq!(too.iter().map(|t| t - t0 - 1).collect::<Vec<_>>(), vec![34]);
}

/// Case 31: the holder walks through what he holds.
#[test]
fn the_holder_walks_through_his_crate() {
    let mut w = world(&[Crate::ahead(150.0)]);
    w.grab(0);
    w.sim.seconds(1.0);
    let start = to_source(w.sim.position(w.p));
    w.sim.intent(w.p).move_axis = Vec2::new(0.0, 1.0);
    w.sim.seconds(1.0);
    w.sim.intent(w.p).move_axis = Vec2::ZERO;
    let gone = to_source(w.sim.position(w.p)) - start;
    // Unblocked (the crate is 57 ahead; it moves on with the view too).
    assert!(gone.x > 100.0, "walked {gone}");
}

/// Case 33: holding, the gun can't be put away.
#[test]
fn holding_the_gun_stays_in_hand() {
    let mut w = world(&[Crate::ahead(150.0)]);
    w.grab(0);
    let pistol = {
        let world = w.sim.app.world();
        let inv = world.get::<Inventory>(w.p).unwrap();
        inv.weapons.iter().copied().find(|e| world.get::<Weapon>(*e).is_some_and(|x| x.id == weapons::PISTOL)).unwrap()
    };
    w.sim.app.world_mut().get_mut::<Inventory>(w.p).unwrap().wanted = Some(pistol);
    w.sim.ticks(5);
    assert_eq!(chimera_sim::testkit::active_id(w.sim.app.world(), w.p), Some(weapons::PHYSCANNON));
}

/// Case 34: `phys_swap` drops the crate and draws the last weapon.
#[test]
fn phys_swap_drops_and_switches() {
    let mut w = world(&[Crate::ahead(150.0)]);
    let n = w.grab(0);
    let t0 = w.tick();
    let p = w.p;
    physcannon::phys_swap(w.sim.app.world_mut(), p);
    w.sim.ticks(3);
    assert!(w.sim.app.world().get::<Held>(n).is_none());
    assert_eq!(w.sounds("Weapon_PhysCannon.Drop", t0).len(), 1);
    assert_eq!(chimera_sim::testkit::active_id(w.sim.app.world(), w.p), Some(weapons::SMG1));
}

/// Case 35: the gun works the tick it is drawn.
#[test]
fn the_gun_works_the_tick_it_is_drawn() {
    let mut w = world(&[Crate::ahead(100.0)]);
    let (pistol, gun) = {
        let world = w.sim.app.world();
        let inv = world.get::<Inventory>(w.p).unwrap();
        let pistol = inv.weapons.iter().copied().find(|e| world.get::<Weapon>(*e).is_some_and(|x| x.id == weapons::PISTOL)).unwrap();
        (pistol, w.gun())
    };
    w.sim.app.world_mut().get_mut::<Inventory>(w.p).unwrap().wanted = Some(pistol);
    w.sim.seconds(1.0);
    w.sim.app.world_mut().get_mut::<Inventory>(w.p).unwrap().wanted = Some(gun);
    w.press(true, false, 1, 1);
    assert!(w.velocity(0).x > 1000.0, "punted on the draw's tick: {}", w.velocity(0));
}

/// Case 36: the holder killed: the crate released at 285 at most, its
/// influence the dead holder's; the gun gone.
#[test]
fn the_holders_death_drops_the_crate() {
    let mut w = world(&[Crate::ahead(150.0)]);
    let n = w.grab(0);
    w.sim.seconds(1.0);
    look(&mut w.sim, w.p, 0.0, 0.0);
    w.sim.ticks(2);
    let gun = w.gun();
    let at = w.sim.position(w.p);
    let p = w.p;
    w.sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    w.sim.ticks(3);
    let world = w.sim.app.world();
    assert!(world.get::<Held>(n).is_none());
    assert!(world.get::<LinearVelocity>(n).unwrap().0.length() / U <= 290.0);
    assert_eq!(world.get::<PhysicsInfluence>(n).map(|i| i.by), Some(w.p));
    w.sim.seconds(0.5);
    // Never dropped (weapons.md 9).
    assert!(w.sim.app.world().get::<chimera_sim::weapon::drop::Loose>(gun).is_none());
}

/// Kill `victim` with a prop's impact: the bridge's crush damage, named
/// for the prop.
fn prop_hit(w: &mut W, prop: Entity, victim: Entity, amount: f32) {
    let at = w.sim.position(victim);
    w.sim.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(prop),
        amount,
        point: at,
        dir: Vec3::X,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Crush,
        weapon: None,
        force: Vec3::ZERO,
    });
}

/// Case 41: a crate punted by A kills B 0.9 s later: A's kill; 1.1 s
/// later: nobody's (the prop's, a world kill).
#[test]
fn a_prop_kill_is_its_punters_within_a_second() {
    for (after, credited) in [(0.9, true), (1.1, false)] {
        let mut w = world(&[Crate::ahead(100.0)]);
        let b = w.sim.spawn_character(to_engine(FEET + Vec3::new(-300.0, 300.0, 36.0)), movement::ID);
        w.sim.ticks(10);
        w.press(true, false, 1, 1);
        w.sim.seconds(after - 0.015);
        let n = w.node(0);
        let t0 = w.tick();
        prop_hit(&mut w, n, b, 5.0);
        w.sim.ticks(2);
        let log = w.sim.app.world().resource::<Log>();
        let d = log.died.iter().find(|(t, e, _)| *t >= t0 && *e == b).copied().unwrap();
        assert_eq!(d.2, if credited { Some(w.p) } else { Some(n) }, "after {after} s");
    }
}

/// Case 42: the punter's own crate rebounding into him later hurts him,
/// credited to himself.
#[test]
fn ones_own_prop_hurts_later() {
    let mut w = world(&[Crate::ahead(100.0)]);
    w.press(true, false, 1, 1);
    w.sim.seconds(0.3);
    let n = w.node(0);
    let t0 = w.tick();
    let p = w.p;
    prop_hit(&mut w, n, p, 5.0);
    w.sim.ticks(2);
    let log = w.sim.app.world().resource::<Log>();
    let d = log.died.iter().find(|(t, e, _)| *t >= t0 && *e == p).copied().unwrap();
    assert_eq!(d.2, Some(p));
}

/// A crate falling onto a standing player's head: the crush hits it
/// takes on the landing's tick (health points, in order) and how many come
/// later within 0.9 s, the crate placed with its bottom `gap` above the
/// head moving down at `speed` (Source units/s), and its bottom's height
/// over the head at the end.
fn drop_on_head(mass: f32, speed: f32, gap: f32) -> (Vec<i32>, usize, f32) {
    // Out of the way on its pillar until it is dropped.
    let mut w = world(&[Crate::ahead(300.0).mass(mass)]);
    w.sim.app.world_mut().get_mut::<Health>(w.p).unwrap().current = 50.0;
    let n = w.node(0);
    let head = to_source(w.sim.position(w.p)).z + 36.0;
    {
        let world = w.sim.app.world_mut();
        let mut e = world.entity_mut(n);
        e.get_mut::<Position>().unwrap().0 = to_engine(Vec3::new(FEET.x, FEET.y, head + gap + 10.0));
        e.get_mut::<LinearVelocity>().unwrap().0 = to_engine(Vec3::new(0.0, 0.0, -speed));
        e.get_mut::<AngularVelocity>().unwrap().0 = Vec3::ZERO;
    }
    let mut cursor = w.sim.app.world().resource::<Messages<Damage>>().get_cursor_current();
    let mut hits = Vec::new();
    for _ in 0..60 {
        w.sim.ticks(1);
        let world = w.sim.app.world();
        for d in cursor.read(world.resource::<Messages<Damage>>()) {
            if d.target == w.p && d.kind == DamageKind::Crush {
                hits.push((w.tick(), (d.amount * 100.0).round() as i32));
            }
        }
    }
    eprintln!("{mass} kg at {speed}: {hits:?}");
    let first = hits.first().map(|h| h.0);
    let later = hits.iter().filter(|h| Some(h.0) != first).count();
    let bottom = to_source(w.sim.position(n)).z - 10.0 - head;
    (hits.into_iter().filter(|h| Some(h.0) == first).map(|h| h.1).collect(), later, bottom)
}

/// P10 (probe, 2026-11-04, `cases.py drops`): crates let fall onto a
/// standing bot's head (5000 health, the roof of dm_lockdown) hurt it as
/// two contacts on one tick, by the crate's mass and its speed at the
/// head (its last free tick's step; its bottom that far above the head
/// then): 30 kg at 284, 389, 524 and 663: 5 + 10, 5 + 10, 10 + 20,
/// 10 + 50; 10 kg at 420: one 10; 100 kg at 572 and 780: 20 + 100,
/// 100 + 500; 250 kg at 582: 100 + 500. The heavy crates stay on the head
/// (round 3: no second landing within 1.5 s; ours, before the shadow was
/// put back up instead of driven, threw them 30 units up to land again
/// 0.7 s on for 5 + 10).
#[test]
fn a_crate_falling_on_a_head_hurts_as_the_probe_measured() {
    let mut wrong = Vec::new();
    for (mass, speed, gap, want) in [
        (30.0, 284.0, 4.4, vec![5, 10]),
        (30.0, 389.0, 9.3, vec![5, 10]),
        (30.0, 524.0, 8.7, vec![10, 20]),
        (30.0, 663.0, 12.9, vec![10, 50]),
        (10.0, 420.0, 8.7, vec![10]),
        (100.0, 572.0, 8.4, vec![20, 100]),
        (100.0, 780.0, 14.6, vec![100, 500]),
        (250.0, 582.0, 12.6, vec![100, 500]),
    ] {
        let (hits, later, bottom) = drop_on_head(mass, speed, gap);
        if hits != want {
            wrong.push(format!("{mass} kg at {speed} in/s: {hits:?}, not {want:?}"));
        }
        if mass >= 100.0 && (later > 0 || !(-12.0..1.0).contains(&bottom)) {
            wrong.push(format!("{mass} kg at {speed} in/s: {later} later hits, its bottom {bottom} over the head"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A wood crate (40 units, `mass` kg) hanging motion-disabled at eye
/// height 50 ahead, punted level into a standing player `dist` ahead (5000
/// health): the crush hits that player takes on the hit's tick (health
/// points, in order), and all of them (tick, amount).
fn punt_into(mass: f32, dist: f32, multiplayer: bool) -> (Vec<i32>, Vec<(u64, i32)>) {
    let s = 40.4 * 0.0254;
    let mut c = Crate::ahead(50.0).mass(mass).flags(72);
    c.multiplayer = multiplayer;
    c.half = Vec3::splat(20.2);
    c.inertia = Some(Vec3::splat(0.714 * mass * s * s / 6.0));
    let mut w = world(&[c]);
    let b = w.sim.spawn_character(to_engine(FEET + Vec3::new(dist, 0.0, 36.0)), movement::ID);
    w.sim.app.world_mut().get_mut::<Health>(b).unwrap().current = 50.0;
    w.sim.ticks(30);
    let mut cursor = w.sim.app.world().resource::<Messages<Damage>>().get_cursor_current();
    let mut hits = Vec::new();
    for k in 0..60 {
        w.sim.intent(w.p).fire = k == 0;
        w.sim.ticks(1);
        let world = w.sim.app.world();
        for d in cursor.read(world.resource::<Messages<Damage>>()) {
            if d.target == b && d.kind == DamageKind::Crush {
                hits.push((w.tick(), (d.amount * 100.0).round() as i32));
            }
        }
    }
    eprintln!("{mass} kg punted into a player: {hits:?}");
    let first = hits.first().map(|h| h.0);
    (hits.iter().filter(|h| Some(h.0) == first).map(|h| h.1).collect(), hits)
}

/// P10's punts (probe, 2026-11-04, `cases.py crate_hits`): a 40-unit
/// crate hanging at eye height punted level into a standing bot 150
/// ahead of it hurts it as two or three contacts on the hit's tick: 10 kg
/// (at 1500 in/s): 20 + 10; 30 kg (1000): 20 + 10 + 5; 100 kg (730):
/// 50 + 10 + 10; 250 kg (650): 100 + 5 + 10; and the 30 kg crate with the
/// bot 50 ahead of it (at 1100): 50 + 10 + 5. The speeds come from the
/// punt (`a_punt_sends_a_crate_at_1100`) less the air's drag. P14: a
/// `prop_physics_multiplayer` crate hurts the same (30 kg: 20 + 10 + 5,
/// 100 kg: 50 + 10 + 10; Corrections 1).
#[test]
fn a_crate_punted_into_a_player_hurts_as_the_probe_measured() {
    let mut wrong = Vec::new();
    for (mass, dist, multiplayer, want) in [
        (10.0, 200.0, false, vec![20, 10]),
        (30.0, 200.0, false, vec![20, 10, 5]),
        (100.0, 200.0, false, vec![50, 10, 10]),
        (250.0, 200.0, false, vec![100, 5, 10]),
        (30.0, 100.0, false, vec![50, 10, 5]),
        (30.0, 200.0, true, vec![20, 10, 5]),
        (100.0, 200.0, true, vec![50, 10, 10]),
    ] {
        let (hits, _) = punt_into(mass, dist, multiplayer);
        if hits != want {
            wrong.push(format!("{mass} kg (multiplayer {multiplayer}) into a player {dist} ahead: {hits:?}, not {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// As `punt_into`, what the crate and the player do: (the player's forward
/// speed the first tick he moves and his fastest within 8 ticks of the hit,
/// the ticks from the hit until he moves, the crate's forward speed 4 ticks
/// after the hit).
fn punt_shove(mass: f32, dist: f32) -> (f32, f32, i64, f32) {
    let s = 40.4 * 0.0254;
    let mut c = Crate::ahead(50.0).mass(mass).flags(72);
    c.half = Vec3::splat(20.2);
    c.inertia = Some(Vec3::splat(0.714 * mass * s * s / 6.0));
    let mut w = world(&[c]);
    let b = w.sim.spawn_character(to_engine(FEET + Vec3::new(dist, 0.0, 36.0)), movement::ID);
    w.sim.app.world_mut().get_mut::<Health>(b).unwrap().current = 50.0;
    w.sim.ticks(30);
    let mut cursor = w.sim.app.world().resource::<Messages<Damage>>().get_cursor_current();
    let mut hit = None;
    let (mut first, mut fastest, mut moved, mut crate_after) = (0.0f32, 0.0f32, None, 0.0);
    for k in 0..60i64 {
        w.sim.intent(w.p).fire = k == 0;
        w.sim.ticks(1);
        let world = w.sim.app.world();
        if hit.is_none() && cursor.read(world.resource::<Messages<Damage>>()).any(|d| d.target == b && d.kind == DamageKind::Crush) {
            hit = Some(k);
        }
        let v = to_source(world.get::<chimera_core::core::Velocity>(b).unwrap().0).x;
        if let Some(h) = hit.or(if v > 20.0 { Some(k) } else { None }) {
            if v > 20.0 && moved.is_none() {
                moved = Some(k - h);
                first = v;
            }
            if k - h <= 8 {
                fastest = fastest.max(v);
            }
            if k - h == 4 {
                crate_after = w.velocity(0).x;
            }
        }
    }
    (first, fastest, moved.unwrap_or(99), crate_after)
}

/// Open questions 14 (probe, `cases.py phys_rules`): a crate punted into a
/// standing bot knocks it at once and goes on forward. The bot's fastest
/// (with the damage's shove): 10 kg 340, 30 kg 454, 100 kg 795, 250 kg 1094,
/// 30 kg at 1100 from 50 ahead 568; the crate after: 8, 165, 337, 445, 192.
/// (Ours before: the crate back off the shadow, the bot only by the shove,
/// 4 ticks late.)
#[test]
fn a_punted_crate_knocks_a_player_and_goes_on() {
    let mut wrong = Vec::new();
    for (mass, dist, bot, keeps) in [
        (10.0, 200.0, 340.0, 8.0),
        (30.0, 200.0, 454.0, 165.0),
        (100.0, 200.0, 795.0, 337.0),
        (250.0, 200.0, 1094.0, 445.0),
        (30.0, 100.0, 568.0, 192.0),
    ] {
        let (first, fastest, moved, after) = punt_shove(mass, dist);
        eprintln!("{mass} kg at {dist}: moved {moved} ticks from the hit at {first}, fastest {fastest}, crate {after}");
        if moved > 1 {
            wrong.push(format!("{mass} kg: moved {moved} ticks after the hit"));
        }
        if (fastest - bot).abs() > 0.15 * bot {
            wrong.push(format!("{mass} kg: fastest {fastest}, not {bot}"));
        }
        if (after - keeps).abs() > 60.0 {
            wrong.push(format!("{mass} kg: the crate {after}, not {keeps}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `hl2mp_allow_pickup` 1 (section 8; the probe, 2026-11-04,
/// `cases.py use_carry`): `+use` at a 20 kg crate on the floor 45 ahead,
/// looking 40 down, carries it 66.5 ahead at eye height looking level,
/// (63.9, 37.0) looking 30 up and (63.8, −36.8) down, the crate level (the yaw-only frame);
/// sprint is off; primary throws it at 199 a tick after (10 kg: 276).
/// With 0 it stays.
#[test]
fn use_carries_with_allow_pickup_as_the_probe_measured() {
    fn setup(mass: f32, allow: u8) -> W {
        let s = 40.4 * 0.0254;
        let mut c = Crate::ahead(45.0).mass(mass).at(Vec3::new(45.0, 0.0, 20.4 - EYE));
        c.half = Vec3::splat(20.2);
        c.inertia = Some(Vec3::splat(0.714 * mass * s * s / 6.0));
        let mut w = world(&[c]);
        w.sim.app.world_mut().resource_mut::<hl2dm::game::carry::CarryRules>().allow_pickup = allow;
        look(&mut w.sim, w.p, EAST, -40.0);
        w.sim.ticks(5);
        w.sim.intent(w.p).use_key = true;
        w.sim.ticks(1);
        w.sim.intent(w.p).use_key = false;
        look(&mut w.sim, w.p, EAST, 0.0);
        w.sim.ticks(40);
        w
    }
    let rel = |w: &mut W| w.at(0) - w.eye();
    let mut w = setup(20.0, 0);
    let r = rel(&mut w);
    assert!((r - Vec3::new(45.0, 0.0, -43.6)).length() < 1.5, "allow 0: {r}");

    let mut w = setup(20.0, 1);
    let r = rel(&mut w);
    assert!((r - Vec3::new(66.5, 0.0, 0.1)).length() < 1.5, "carried level: {r}");
    look(&mut w.sim, w.p, EAST, 30.0);
    w.sim.ticks(30);
    let r = rel(&mut w);
    assert!((r - Vec3::new(63.9, 0.0, 37.0)).length() < 1.5, "carried looking up: {r}");
    look(&mut w.sim, w.p, EAST, -30.0);
    w.sim.ticks(30);
    let r = rel(&mut w);
    assert!((r - Vec3::new(63.8, 0.0, -36.8)).length() < 1.5, "carried looking down: {r}");
    let n = w.node(0);
    let rot = w.sim.app.world().get::<Transform>(n).unwrap().rotation;
    assert!(rot.angle_between(Quat::IDENTITY) < 0.02, "the crate stays level: {rot}");
    look(&mut w.sim, w.p, EAST, 0.0);
    w.sim.ticks(30);
    // Sprint held while carrying: the run speed.
    {
        let mut i = w.sim.intent(w.p);
        i.sprint = true;
        i.move_axis = Vec2::Y;
    }
    w.sim.seconds(1.0);
    let speed = to_source(w.sim.velocity(w.p)).truncate().length();
    assert!((speed - 190.0).abs() < 2.0, "sprinting while carrying: {speed}");
    {
        let mut i = w.sim.intent(w.p);
        i.sprint = false;
        i.move_axis = Vec2::ZERO;
    }
    w.sim.seconds(1.0);
    // A press of primary throws it; the gun doesn't punt.
    let t0 = w.tick();
    w.press(true, false, 1, 3);
    let v = w.velocity(0);
    assert!(w.sim.app.world().get::<Held>(n).is_none(), "let go");
    assert!((v.x - 199.0).abs() < 6.0 && v.y.abs() < 2.0, "thrown: {v}");
    assert!(w.sounds("Weapon_PhysCannon.Launch", t0).is_empty() && w.dry_fires(t0) == 0, "no punt");

    let mut w = setup(10.0, 1);
    w.press(true, false, 1, 3);
    let v = w.velocity(0);
    assert!((v.x - 276.0).abs() < 8.0, "10 kg thrown: {v}");

    // `+use` again drops it from rest.
    let mut w = setup(20.0, 1);
    w.sim.intent(w.p).use_key = true;
    w.sim.ticks(2);
    w.sim.intent(w.p).use_key = false;
    let n = w.node(0);
    assert!(w.sim.app.world().get::<Held>(n).is_none(), "dropped");
    let v = w.velocity(0);
    assert!(v.truncate().length() < 5.0 && v.z <= 0.0, "from rest: {v}");
}

/// Section 8 step 4: a carried crate dropped while it touches something
/// has its velocity zeroed, in the air it is only cut (285, 720).
#[test]
fn a_carried_crate_dropped_while_touching_stops_dead() {
    let drop_at = |pitch: f32| {
        let s = 40.4 * 0.0254;
        let mut c = Crate::ahead(45.0).mass(20.0).at(Vec3::new(45.0, 0.0, 20.4 - EYE));
        c.half = Vec3::splat(20.2);
        c.inertia = Some(Vec3::splat(0.714 * 20.0 * s * s / 6.0));
        let mut w = world(&[c]);
        w.sim.app.world_mut().resource_mut::<hl2dm::game::carry::CarryRules>().allow_pickup = 1;
        look(&mut w.sim, w.p, EAST, -40.0);
        w.sim.ticks(5);
        w.sim.intent(w.p).use_key = true;
        w.sim.ticks(1);
        w.sim.intent(w.p).use_key = false;
        look(&mut w.sim, w.p, EAST, pitch);
        w.sim.ticks(40);
        let n = w.node(0);
        // Swung round, as if the player had turned fast.
        for k in 1..=8 {
            look(&mut w.sim, w.p, EAST + 4.0 * k as f32, pitch);
            w.sim.ticks(1);
        }
        w.sim.intent(w.p).use_key = true;
        w.sim.ticks(1);
        w.sim.intent(w.p).use_key = false;
        assert!(w.sim.app.world().get::<Held>(n).is_none(), "dropped");
        w.velocity(0)
    };
    // Pressed into the floor (looking far down), then well clear of it.
    let touching = drop_at(-75.0);
    assert!(touching.length() < 20.0, "touching the floor: {touching}");
    let clear = drop_at(0.0);
    assert!(clear.length() > 60.0, "in the air: {clear}");
}

/// P15 (probe, 2026-11-04, `cases.py break_input`): the `Break` input on a
/// `prop_physics` (a 30-health wood crate) leaves it where it was, drawn,
/// non-solid and taking no damage (Corrections 2: only lethal damage
/// removes a prop in HL2:DM).
#[test]
fn the_break_input_leaves_the_prop() {
    let mut w = world(&[Crate::ahead(100.0).at(Vec3::new(100.0, 0.0, 10.0 - EYE))]);
    let n = w.node(0);
    let before = w.at(0);
    {
        let mut logic = w.sim.app.world_mut().resource_mut::<source_logic::logic::Logic>();
        let id = logic.world.by_map_index(0).expect("the crate's logic entity");
        logic.world.queue_direct(id, "Break", source_logic::logic::Value::Void, 0.0, None);
    }
    w.sim.ticks(10);
    let world = w.sim.app.world();
    let logic = world.resource::<source_logic::logic::Logic>();
    let id = logic.world.by_map_index(0).expect("still there");
    let p = logic.world.prop(id).expect("still a prop");
    assert!(p.broken && !p.solid, "broken, non-solid");
    assert!(world.get_entity(n).is_ok(), "its body stays");
    assert!(w.at(0).distance(before) < 1.0, "where it was");
}

/// Case 44: a held crate pushed into another player hurts him not.
#[test]
fn held_crates_never_hurt() {
    let mut w = world(&[Crate::ahead(200.0)]);
    let b = w.sim.spawn_character(to_engine(FEET + Vec3::new(60.0, 0.0, 36.0)), movement::ID);
    w.sim.ticks(10);
    w.press(false, true, 1, 1);
    w.sim.seconds(1.5);
    assert_eq!(w.sim.app.world().get::<Health>(b).unwrap().current, 1.0);
}

/// Cases 47, 48: `+use` at a 20 kg crate clicks and moves nothing; at a
/// 100 kg one, the deny.
#[test]
fn use_clicks_at_small_props_and_denies_big_ones() {
    for (mass, entry) in [(20.0, "HL2Player.Use"), (100.0, "HL2Player.UseDeny")] {
        let mut w = world(&[Crate::ahead(50.0).mass(mass)]);
        let before = w.at(0);
        let t0 = w.tick();
        w.sim.intent(w.p).use_key = true;
        w.sim.ticks(2);
        w.sim.intent(w.p).use_key = false;
        w.sim.ticks(2);
        assert_eq!(w.sounds(entry, t0).len(), 1, "{mass} kg");
        assert!(w.at(0).distance(before) < 0.5);
        assert_eq!(chimera_sim::testkit::active_id(w.sim.app.world(), w.p), Some(weapons::PHYSCANNON));
    }
}

/// Case 54: a live frag grabbed becomes the grabber's, 1.5 s from then.
#[test]
fn a_caught_frag_is_the_catchers() {
    let mut w = world(&[]);
    let eye = to_engine(w.eye() + Vec3::new(100.0, 0.0, 0.0));
    let thrower = w.sim.spawn_character(to_engine(FEET + Vec3::new(-300.0, 300.0, 36.0)), movement::ID);
    let p = weapons::launch::hook_params::<weapons::frag::FragParams>(weapons::FRAG, weapons::FRAG_HOOK);
    let g = weapons::frag::spawn_grenade(w.sim.app.world_mut(), &p, Some(thrower), eye, Vec3::ZERO, Vec3::ZERO);
    w.sim.app.world_mut().entity_mut(g).insert(GravityScale(0.0));
    w.sim.ticks(1);
    w.press(false, true, 1, 1);
    let now = w.now();
    let gr = w.sim.app.world().get::<Hl2Grenade>(g).unwrap();
    assert_eq!(gr.thrower, Some(w.p));
    assert!((gr.explode_at - (now + 1.5)).abs() < 0.02, "{} {now}", gr.explode_at);
}

/// Case 58: the claws open at a crate in reach and close 34 ticks after
/// looking away.
#[test]
fn the_claws_open_and_close() {
    let mut w = world(&[Crate::ahead(200.0)]);
    w.sim.ticks(1);
    assert!(w.sim.app.world().get::<Hl2Physcannon>(w.gun()).unwrap().claws);
    look(&mut w.sim, w.p, 90.0, 0.0);
    let t0 = w.tick();
    w.sim.ticks(40);
    // Looked away from tick t0 + 1: closed 34 ticks later.
    let close = w.sounds("Weapon_PhysCannon.CloseClaws", t0);
    assert_eq!(close.iter().map(|t| t - t0 - 1).collect::<Vec<_>>(), vec![34]);
}

/// Case 60: a multiplayer crate blocks a player as a `prop_physics`
/// (HL2:DM's rule).
#[test]
fn multiplayer_props_are_solid() {
    let mut p = chimera_world::map::PhysicsProp {
        push: PushAway::Solid,
        mass: 20.0,
        bounds: (Vec3::ZERO, Vec3::ONE),
    };
    let mut app = App::new();
    let e = app.world_mut().spawn(p).id();
    app.add_systems(Update, hl2dm::game::physics::solid_multiplayer);
    app.update();
    p = *app.world().get::<chimera_world::map::PhysicsProp>(e).unwrap();
    assert_eq!(p.push, PushAway::Collide);
}


/// A combine ball (B's) floating `ahead` units in front of the eye.
fn ball(w: &mut W, ahead: f32) -> (Entity, Entity) {
    use chimera_sim::weapon::projectile::{Flyer, Touch, spawn_flyer};
    let b = w.sim.spawn_character(to_engine(FEET + Vec3::new(-300.0, 300.0, 36.0)), movement::ID);
    w.sim.ticks(10);
    let start = to_engine(w.eye() + Vec3::new(ahead, 0.0, 0.0));
    let world = w.sim.app.world_mut();
    let clock = *world.resource::<SimClock>();
    let mut f = Flyer::new(weapons::AR2, Some(b), Vec3::ZERO, &clock);
    f.half = Vec3::splat(10.0 * U);
    f.sphere = true;
    f.keep_speed = Some(1000.0 * U);
    f.expires = Some(clock.now + 4.0);
    f.touch = Touch::Bounce;
    // As the AR2's (`alt_fire`): 1000 crush.
    f.direct = Some(chimera_sim::weapon::projectile::Direct {
        damage: 10.0,
        kind: DamageKind::Crush,
        head: 1.0,
        armor_ratio: Some(1.0),
        quantum: 0.0,
        force: 0.0,
    });
    let mut commands = world.commands();
    let e = spawn_flyer(&mut commands, f, start, weapons::launch::BALL_MODEL.into(), &clock);
    world.flush();
    (e, b)
}

/// Cases 55, 56: a ball caught becomes the catcher's and stays in the
/// gun; launched it leaves at 959.2 along the view with 4 s of life, and
/// at 1500 from its first bounce (P12, probe 2026-11-04, `cases.py
/// ball_catch`: an AR2 ball caught as it left the catcher's own gun flew
/// at 959 until it hit a wall, then 1500).
#[test]
fn a_caught_ball_is_the_catchers_and_leaves_at_959() {
    use chimera_sim::weapon::projectile::Flyer;
    let mut w = world(&[]);
    let (e, _) = ball(&mut w, 150.0);
    w.press(false, true, 1, 2);
    assert!(w.holding(), "caught");
    let f = w.sim.app.world().get::<Flyer>(e).unwrap();
    assert_eq!(f.owner, Some(w.p));
    assert!(f.expires.is_none(), "its life stops while held");
    w.sim.seconds(1.0);
    let rel = to_source(w.sim.position(e)) - w.eye();
    assert!(rel.x > 30.0 && rel.x < 80.0 && rel.y.abs() < 3.0, "held at {rel}");
    w.press(true, false, 1, 1);
    let now = w.now();
    let f = w.sim.app.world().get::<Flyer>(e).unwrap();
    let v = to_source(f.velocity);
    assert!((v.x - 959.2).abs() < 1.0 && v.y.abs() < 1.0, "{v}");
    assert_eq!(f.keep_speed, Some(1500.0 * U), "1500 after its first bounce");
    assert!((f.expires.unwrap() - (now + 4.0)).abs() < 0.02);
}

/// Case 55's end: a ball kept in the gun goes off 8 s after the catch.
#[test]
fn a_ball_kept_goes_off_after_8_s() {
    let mut w = world(&[]);
    let (e, _) = ball(&mut w, 150.0);
    w.press(false, true, 1, 2);
    w.sim.seconds(7.8);
    assert!(w.sim.app.world().get_entity(e).is_ok(), "still held at 7.8 s");
    w.sim.seconds(0.3);
    assert!(w.sim.app.world().get_entity(e).is_err(), "gone at 8 s");
    assert!(!w.holding());
}

/// Case 57: dropped, it leaves at 1500 within 55 degrees of the view.
#[test]
fn a_dropped_ball_leaves_at_1500() {
    use chimera_sim::weapon::projectile::Flyer;
    let mut w = world(&[]);
    let (e, _) = ball(&mut w, 150.0);
    w.press(false, true, 1, 2);
    w.sim.seconds(1.0);
    w.press(false, true, 1, 1);
    let f = w.sim.app.world().get::<Flyer>(e).unwrap();
    let v = to_source(f.velocity);
    assert!((v.length() - 1500.0).abs() < 1.0, "{v}");
    let off = v.normalize().dot(Vec3::X).acos().to_degrees();
    assert!(off <= 55.0 * std::f32::consts::SQRT_2 + 0.5, "{off} degrees off the view");
}

// ---------------------------------------------------------------------------
// Launches: first impacts, spin and carry angles (9, 4, 7)

/// A logic prop's health, breakable: 1000 health, in pieces when it breaks.
const TOUGH: &[(&str, &str)] = &[("chimera_prop_health", "1000"), ("chimera_prop_pieces", "1")];

fn prop_health(w: &W, i: usize) -> i32 {
    let logic = w.sim.app.world().resource::<source_logic::logic::Logic>();
    logic.world.by_map_index(i).and_then(|id| logic.world.prop(id)).map_or(-1, |p| p.health)
}

fn prop_broken(w: &W, i: usize) -> bool {
    let logic = w.sim.app.world().resource::<source_logic::logic::Logic>();
    logic.world.by_map_index(i).is_none_or(|id| logic.world.prop(id).is_none_or(|p| p.broken))
}

/// A wall across the line of fire, `x` ahead of the feet.
fn wall_at(x: f32) -> (Vec3, Vec3) {
    (Vec3::new(x, -512.0, 0.0), Vec3::new(x + 16.0, 512.0, 512.0))
}

/// Grab prop 0 and launch it after settling; the launch's tick.
fn launch_it(w: &mut W) {
    w.grab(0);
    w.sim.ticks(20);
    w.press(true, false, 1, 1);
}

/// Spec 9: a crate launched from the hold takes 10× its computed impact
/// damage on its first collision, and only then; the same launch with the
/// mark taken off (as a throw, or a punt) takes one.
#[test]
fn a_launchs_first_impact_counts_ten_times() {
    let lost = |marked: bool| {
        let mut w = world_with(&[Crate::ahead(60.0).keys(TOUGH)], &[wall_at(300.0)]);
        launch_it(&mut w);
        let n = w.node(0);
        assert_eq!(w.sim.app.world().get::<chimera_world::map::held::LaunchedFromHold>(n).is_some(), true, "marked");
        if !marked {
            w.sim.app.world_mut().entity_mut(n).remove::<chimera_world::map::held::LaunchedFromHold>();
        }
        w.sim.seconds(0.4);
        assert!(w.sim.app.world().get::<chimera_world::map::held::LaunchedFromHold>(n).is_none(), "gone with the contact");
        1000 - prop_health(&w, 0)
    };
    let (once, ten) = (lost(false), lost(true));
    assert!(once > 0, "the wall hurts it at all: {once}");
    assert!((ten - 10 * once).abs() <= once, "{ten} after a launch, {once} without");
}

/// Case 46's first half, and P3's numbers beside it (`cases.py
/// phys_rules`: an oil drum launched at a wall breaks on it, punted it
/// doesn't): a prop with `onfirstimpact` "break" breaks on its first
/// collision after a launch whatever the speed, not after a punt.
#[test]
fn first_impact_props_break_only_after_a_launch() {
    const KEYS: &[(&str, &str)] = &[
        ("chimera_prop_health", "1000"),
        ("chimera_prop_pieces", "1"),
        ("chimera_prop_interactions", "firstimpact_break"),
    ];
    let mut w = world_with(&[Crate::ahead(60.0).keys(KEYS)], &[wall_at(300.0)]);
    launch_it(&mut w);
    w.sim.seconds(0.4);
    assert!(prop_broken(&w, 0), "launched: broke on the wall");
    let mut w = world_with(&[Crate::ahead(60.0).keys(KEYS)], &[wall_at(300.0)]);
    w.press(true, false, 1, 1);
    w.sim.seconds(0.6);
    assert!(!prop_broken(&w, 0), "punted: stays whole");
}

/// The explosions `n` ticks wrote: their attackers.
fn explosion_attackers(w: &mut W, n: u64) -> Vec<Option<Entity>> {
    let mut cursor = w.sim.app.world().resource::<Messages<chimera_core::core::Explosion>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..n {
        w.sim.ticks(1);
        out.extend(cursor.read(w.sim.app.world().resource::<Messages<chimera_core::core::Explosion>>()).map(|e| e.attacker));
    }
    out
}

/// Spec 9 and cases 45, 46: an explosive prop's blast names its last
/// attacker (a punt counts, for good); else its influencer while the
/// influence is at most 2 s old; else the break's own attacker.
#[test]
fn an_explosive_props_blast_credit() {
    const KEYS: &[(&str, &str)] = &[
        ("chimera_prop_health", "20"),
        ("chimera_prop_pieces", "1"),
        ("chimera_prop_explode", "100 200"),
    ];
    let break_it = |w: &mut W| {
        let mut logic = w.sim.app.world_mut().resource_mut::<source_logic::logic::Logic>();
        let id = logic.world.by_map_index(0).expect("the prop");
        logic.world.queue_direct(id, "Break", source_logic::logic::Value::Void, 0.0, None);
    };
    let spawn = || world(&[Crate::ahead(60.0).keys(KEYS)]);
    // Grabbed and dropped (an influence, no hit), broken 1.9 s and 2.1 s on.
    for (after, credited) in [(1.9, true), (2.1, false)] {
        let mut w = spawn();
        let n = w.grab(0);
        w.sim.seconds(0.6);
        w.press(false, true, 1, 1);
        let at = w.sim.app.world().get::<PhysicsInfluence>(n).unwrap().at;
        w.sim.seconds(after - (w.now() - at) - 0.03);
        break_it(&mut w);
        let got = explosion_attackers(&mut w, 4);
        assert_eq!(got.len(), 1, "one blast after {after} s: {got:?}");
        assert_eq!(got[0] == Some(w.p), credited, "after {after} s: {got:?}");
    }
    // Punted: its last attacker, however long ago (case 45).
    let mut w = spawn();
    w.press(true, false, 1, 1);
    w.sim.seconds(3.0);
    break_it(&mut w);
    let got = explosion_attackers(&mut w, 4);
    assert_eq!(got, vec![Some(w.p)], "the punter, 3 s on");
}

/// Source's angles (pitch, yaw, roll) of an engine rotation's forward and
/// up: forward (Source) and up (Source).
fn axes(rot: Quat) -> (Vec3, Vec3) {
    (to_source(rot * Vec3::X * U), to_source(rot * Vec3::Y * U))
}

/// Spec 4 step 4: `preferred_carryangles` hold the prop in the yaw-only
/// frame, whatever the pitch; measured (`cases.py phys_rules2`) the propane
/// tank ("-90 0 0") held by a bot looking at yaw 180: (−90, 0, 180), the
/// same rotation as (−90, 180, 0).
#[test]
fn preferred_carry_angles_hold_in_the_yaw_frame() {
    const KEYS: &[(&str, &str)] = &[("chimera_prop_carry_angles", "-90 0 0")];
    let mut w = world(&[Crate::ahead(60.0).keys(KEYS)]);
    let n = w.grab(0);
    w.sim.seconds(0.5);
    let check = |w: &W, what: &str| {
        let rot = w.sim.app.world().get::<Transform>(n).unwrap().rotation;
        let (fwd, up) = axes(rot);
        // Pitch −90: its +X up, its +Z back along the view.
        assert!((fwd - Vec3::Z).length() < 0.05, "{what}: forward {fwd}");
        assert!((up - Vec3::NEG_X).length() < 0.05, "{what}: up {up}");
    };
    check(&w, "level");
    look(&mut w.sim, w.p, EAST, -40.0);
    w.sim.seconds(0.3);
    check(&w, "looking down: the frame is the yaw's");
}

/// Spec 4 step 4, 7: a "spin_zaxis" prop (the sawblade) is held at
/// (−5, 0, 0) in the view frame and spins about its own z when launched:
/// measured (`cases.py phys_rules2`) a sawblade held by a bot looking level
/// at yaw 180: (−5, −180, 0), then 48.7 degrees a tick of yaw (3250 deg/s;
/// the spec's 5000, under Source's 3600 cap). "spin_none" doesn't spin.
#[test]
fn launch_spin_by_the_props_interactions() {
    const ZAXIS: &[(&str, &str)] = &[("chimera_prop_interactions", "launch_spin_z")];
    const NONE: &[(&str, &str)] = &[("chimera_prop_interactions", "launch_spin_none")];
    let mut w = world(&[Crate::ahead(60.0).keys(ZAXIS)]);
    let n = w.grab(0);
    w.sim.seconds(0.5);
    let rot = w.sim.app.world().get::<Transform>(n).unwrap().rotation;
    let (fwd, up) = axes(rot);
    let want = Vec3::new(5f32.to_radians().cos(), 0.0, 5f32.to_radians().sin());
    assert!((fwd - want).length() < 0.05, "held at pitch −5 in the view frame: {fwd}");
    assert!(up.z > 0.9, "{up}");
    w.press(true, false, 1, 1);
    w.sim.ticks(1);
    let world = w.sim.app.world();
    let spin = world.get::<AngularVelocity>(n).unwrap().0;
    let axis = world.get::<Rotation>(n).unwrap().0 * Vec3::Y;
    // Source's cap, less a tick of the air's drag on it (measured 3250).
    assert!((spin.length().to_degrees() - 3250.0).abs() < 100.0, "{} deg/s", spin.length().to_degrees());
    assert!(spin.normalize().dot(axis) > 0.9, "about its own z: {spin} {axis}");

    let mut w = world_(NONE);
    let n = w.grab(0);
    w.sim.seconds(0.5);
    w.press(true, false, 1, 1);
    w.sim.ticks(1);
    let spin = w.sim.app.world().get::<AngularVelocity>(n).unwrap().0;
    assert!(spin.length() < 0.5, "none: {spin}");
}

fn world_(keys: &'static [(&'static str, &'static str)]) -> W {
    world(&[Crate::ahead(60.0).keys(keys)])
}

/// Two bodies of a metal ragdoll (a map's prop_ragdoll, none in the stock
/// maps), floating at eye height: 20 kg `ahead` on the line, 10 kg 60
/// to the side.
fn two_part_ragdoll(w: &mut W, ahead: f32, surface: &str) -> (Entity, Entity) {
    use chimera_core::core::PropSurface;
    use chimera_world::map::{MapPart, Ragdoll, RagdollBody};
    let eye = w.eye();
    let world = w.sim.app.world_mut();
    let now = world.resource::<SimClock>().now;
    let sim = world.spawn((Name::new("test ragdoll"), MapPart, Transform::default())).id();
    let mut body = |at: Vec3, mass: f32, i: usize| {
        world
            .spawn((
                RagdollBody { ragdoll: sim, index: i, collides: 0 },
                MapPart,
                Transform::from_translation(to_engine(at)),
                RigidBody::Dynamic,
                Collider::cuboid(20.0 * U, 20.0 * U, 20.0 * U),
                Mass(mass),
                LinearVelocity::ZERO,
                AngularVelocity::ZERO,
                GravityScale(0.0),
                PropSurface(surface.to_string()),
            ))
            .id()
    };
    let a = body(eye + Vec3::new(ahead, 0.0, 0.0), 20.0, 0);
    let b = body(eye + Vec3::new(ahead, 60.0, 0.0), 10.0, 1);
    let placed = chimera_world::map::placed_ragdoll::PlacedRagdoll {
        ragdoll: std::sync::Arc::new(chimera_world::map::MapRagdoll::default()),
        bones: std::sync::Arc::new(Vec::new()),
        root: Transform::default(),
        pose: None,
        joints: Vec::new(),
        flags: 0,
        home: Transform::default(),
        sim: Some(sim),
        follow: Transform::default(),
        pinned: false,
        asleep: false,
        gone: false,
        solid: true,
    };
    let node = world.spawn((Name::new("test ragdoll node"), MapPart, Transform::default(), placed)).id();
    world.entity_mut(sim).insert(Ragdoll::placed(node, vec![a, b], Vec::new(), vec![None, None], U, Vec3::ZERO, now));
    w.sim.ticks(2);
    (a, b)
}

/// Section 2 step 5, 4 step 3: an object of several bodies shares the
/// punt by mass (M 30: the hit one 1, 1650 in/s; the other 1/6, 550), is
/// held whole at the summed mass (the hit one 1 kg, the other 1 / max(2 /
/// 7.5, 1)) and takes its masses back. A ragdoll of flesh isn't there for
/// the gun.
#[test]
fn an_object_of_several_bodies() {
    let speed = |w: &W, e: Entity| to_source(w.sim.app.world().get::<LinearVelocity>(e).unwrap().0);
    let mut w = world(&[]);
    let (a, b) = two_part_ragdoll(&mut w, 100.0, "metal");
    w.press(true, false, 1, 1);
    w.sim.ticks(1);
    let (va, vb) = (speed(&w, a), speed(&w, b));
    assert!((va.x - 1650.0).abs() < 30.0, "the hit body: {va}");
    assert!((vb.x - 550.0).abs() < 15.0, "the other: {vb}");

    let mut w = world(&[]);
    let (a, b) = two_part_ragdoll(&mut w, 100.0, "metal");
    w.press(false, true, 1, 2);
    assert!(w.holding(), "held");
    let mass = |w: &W, e: Entity| w.sim.app.world().get::<Mass>(e).unwrap().0;
    assert_eq!((mass(&w, a), mass(&w, b)), (1.0, 1.0));
    assert!(w.sim.app.world().get::<Held>(b).is_some());
    w.sim.seconds(0.6);
    w.press(false, true, 1, 1);
    assert!(!w.holding(), "dropped");
    assert_eq!((mass(&w, a), mass(&w, b)), (20.0, 10.0));
    assert!(w.sim.app.world().get::<Held>(b).is_none());

    let mut w = world(&[]);
    let (a, _) = two_part_ragdoll(&mut w, 100.0, "flesh");
    let t = w.tick();
    w.press(true, false, 1, 2);
    assert_eq!(w.dry_fires(t), 1, "flesh: a dry fire");
    assert!(speed(&w, a).length() < 1.0);
}

// ---------------------------------------------------------------------------
// Items in the gun (11; Open questions 6, 14)

/// A world with one `item_battery` on the floor `ahead` of the player, who
/// looks down at it; its entity index.
fn with_battery(ahead: f32) -> (W, Entity) {
    let mut data = MapData {
        name: "test:gravity_gun_items".into(),
        entity_scale: U,
        gravity: Some(600.0 * U),
        ..default()
    };
    solid(&mut data, Vec3::new(-4096.0, -4096.0, -64.0), Vec3::new(4096.0, 4096.0, 0.0));
    data.entities.push(MapEntity {
        keyvalues: vec![
            ("classname".into(), "item_battery".into()),
            ("origin".into(), format!("{ahead} 0 0")),
        ],
        hulls: Vec::new(),
        mover: false,
        physics: None,
    });
    let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim.app.init_resource::<Log>().add_systems(FixedLast, listen);
    let p = sim.spawn_character(to_engine(FEET + Vec3::Z * 36.0), movement::ID);
    let pitch = -(EYE - 8.0).atan2(ahead).to_degrees();
    look(&mut sim, p, EAST, pitch);
    sim.seconds(1.0);
    let world = sim.app.world_mut();
    let item = world.query_filtered::<Entity, With<hl2dm::game::pickups::Pickup>>().iter(world).next().expect("the battery");
    world.entity_mut(p).insert(hl2dm::game::suit::Suit { worn: true, ..default() });
    (W { sim, p }, item)
}

/// Open questions 6, measured (`cases.py phys_rules`): a battery grabbed
/// from 100 ahead is taken on its way in, 4 ticks after the grab, and 130
/// ahead 6 ticks after, armour + 15, the gun's drop sound on that tick
/// (its touch box grown 48: 72 from the player's axis; the object would
/// stop 51 away, short of the 48 reach of a 24-unit box). Not held, the
/// same battery 60 ahead isn't touched.
#[test]
fn a_held_battery_is_taken_on_its_way_in() {
    for (ahead, ticks) in [(100.0, 4i64), (130.0, 6)] {
        let (mut w, _) = with_battery(ahead);
        let t0 = w.tick();
        let armour = |w: &W| w.sim.app.world().get::<chimera_sim::weapon::Armor>(w.p).unwrap().amount;
        assert_eq!(armour(&w), 0.0);
        w.press(false, true, 1, 1);
        assert!(w.holding(), "grabbed from {ahead}");
        let mut taken = None;
        for k in 1..30i64 {
            w.sim.ticks(1);
            if armour(&w) > 0.0 && taken.is_none() {
                taken = Some(k);
                break;
            }
        }
        let taken = taken.expect("taken");
        assert!((taken - ticks).abs() <= 2, "{ahead} ahead: taken {taken} ticks after the grab, the probe's {ticks}");
        w.sim.ticks(3);
        assert!(!w.holding(), "its body gone, the hold ends");
        assert_eq!(w.sounds("Weapon_PhysCannon.Drop", t0).len(), 1);
    }
    let (mut w, _) = with_battery(60.0);
    w.sim.seconds(0.5);
    assert_eq!(w.sim.app.world().get::<chimera_sim::weapon::Armor>(w.p).unwrap().amount, 0.0, "24 from 60 is out of reach");
}

// ---------------------------------------------------------------------------
// The ball (12; projectiles.md 5)

/// Section 12, measured (`cases.py phys_rules`): a free ball (1000 in/s)
/// punted leaves at 1959 (959.2 added along the view) with the punter's
/// credit and a fresh 4 s; it keeps 1959 through its bounces (5.2's rule,
/// not 12's "restores").
#[test]
fn a_free_ball_can_be_punted() {
    use chimera_sim::weapon::projectile::Flyer;
    let mut w = world_with(&[], &[wall_at(1200.0)]);
    let (e, b) = ball(&mut w, 150.0);
    w.sim.app.world_mut().get_mut::<Flyer>(e).unwrap().velocity = to_engine(Vec3::new(1000.0, 0.0, 0.0));
    w.press(true, false, 1, 1);
    let now = w.now();
    let f = w.sim.app.world().get::<Flyer>(e).unwrap();
    let v = to_source(f.velocity);
    assert!((v.x - 1959.2).abs() < 1.0 && v.y.abs() < 1.0 && v.z.abs() < 1.0, "{v}");
    assert_eq!(f.owner, Some(w.p), "was {b}");
    assert!((f.expires.unwrap() - (now + 4.0)).abs() < 0.02);
    assert!((f.keep_speed.unwrap() / U - 1000.0).abs() < 1.0, "its stored speed");
    w.sim.seconds(1.0);
    let f = w.sim.app.world().get::<Flyer>(e).unwrap();
    assert!((to_source(f.velocity).length() - 1959.2).abs() < 1.0, "after its bounce: {}", to_source(f.velocity));
    assert!(to_source(f.velocity).x < 0.0, "it bounced");
}

/// Case 55's end, projectiles.md 5.6: a ball kept in the gun ends as any
/// ball: `FlyerGone` (expired: its rings and shake), its explosion sound,
/// no damage.
#[test]
fn a_held_balls_end_is_a_balls_end() {
    use chimera_sim::weapon::projectile::FlyerGone;
    use chimera_world::map::logic_view::MapShake;
    let mut w = world(&[]);
    let (e, _) = ball(&mut w, 150.0);
    let t0 = w.tick();
    w.press(false, true, 1, 2);
    w.sim.app.world_mut().get_mut::<Health>(w.p).unwrap().current = 1.0;
    let mut cursor = w.sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let mut shakes = w.sim.app.world().resource::<Messages<MapShake>>().get_cursor_current();
    let mut shaken = Vec::new();
    let mut ended = None;
    for _ in 0..600 {
        w.sim.ticks(1);
        shaken.extend(shakes.read(w.sim.app.world().resource::<Messages<MapShake>>()).copied());
        if let Some(g) = cursor.read(w.sim.app.world().resource::<Messages<FlyerGone>>()).find(|g| g.flyer == e) {
            ended = Some((g.expired, w.tick() - t0));
            break;
        }
    }
    let (expired, ticks) = ended.expect("it ended");
    assert!(expired, "as a ball's time running out");
    assert!((530..=536).contains(&ticks), "8 s after the catch: {ticks}");
    assert_eq!(w.sounds("NPC_CombineBall.Explosion", t0).len(), 1, "its sound");
    // 5.6: the screen shake (20, 150 Hz, 1 s, radius 1250 units).
    assert_eq!(shaken.len(), 1, "its shake");
    let s = shaken[0];
    assert!((s.amplitude - 20.0 * 0.0254).abs() < 1e-4 && s.frequency == 150.0 && s.duration == 1.0);
    assert!((s.radius - 1250.0 * 0.0254).abs() < 1e-3, "{}", s.radius);
    assert!(w.sim.app.world().get_entity(e).is_err());
    assert!(w.sim.app.world().get::<Health>(w.p).unwrap().current > 0.0, "harmless");
    assert!(!w.holding());
}

/// Projectiles.md 5.3: a ball into a breakable prop: 1000 crush (it
/// breaks); into an unbreakable one: knocked (150 kg at 1000 in/s), the
/// ball flying on at its speed.
#[test]
fn the_ball_crushes_and_knocks_props() {
    use chimera_sim::weapon::projectile::Flyer;
    let tough: &'static [(&'static str, &'static str)] = &[("chimera_prop_health", "900"), ("chimera_prop_pieces", "1")];
    let mut w = world(&[Crate::ahead(300.0).keys(tough)]);
    let (e, b) = ball(&mut w, 150.0);
    w.sim.app.world_mut().get_mut::<Flyer>(e).unwrap().velocity = to_engine(Vec3::new(1000.0, 0.0, 0.0));
    w.sim.seconds(0.3);
    assert!(prop_broken(&w, 0), "{} health left", prop_health(&w, 0));
    assert!(w.sim.app.world().get_entity(e).is_ok() && b != w.p);

    let mut w = world(&[Crate::ahead(300.0)]);
    let (e, _) = ball(&mut w, 150.0);
    w.sim.app.world_mut().get_mut::<Flyer>(e).unwrap().velocity = to_engine(Vec3::new(1000.0, 0.0, 0.0));
    w.sim.seconds(0.15);
    let v = w.velocity(0);
    // 2 · 150 / (150 + 30) of 1000 (1667), a whole elastic knock, less a
    // little drag.
    assert!(v.x > 1400.0, "knocked: {v}");
    let f = w.sim.app.world().get::<Flyer>(e).unwrap();
    assert!((to_source(f.velocity).length() - 1000.0).abs() < 1.0);
}

// ---------------------------------------------------------------------------
// The gun's effects (effects_sounds.md 11; `physcannon_fx`)

fn with_effects(w: &mut W) {
    use chimera_core::core::LocalPlayer;
    use chimera_world::map::{
        logic_view::ScreenFades,
        particles::{MapParticles, ParticleMaterial, ParticleMaterials},
    };
    let materials = hl2dm::game::projectile_fx::MATERIALS
        .iter()
        .chain(hl2dm::game::physcannon_fx::MATERIALS)
        .chain(source_engine::effects::impact_effects::MATERIALS)
        .map(|n| ParticleMaterial { name: n.to_string(), ..default() })
        .collect();
    w.sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
    w.sim.app.init_resource::<ScreenFades>();
    w.sim.app.world_mut().entity_mut(w.p).insert(LocalPlayer);
}

/// H9 11: a punt's spark burst (16 to 32 sparks), the punter's beam of
/// `orangelight1` and the white fire fade (245, 245, 255, 32 over 0.1 s).
#[test]
fn a_punt_bursts_beams_and_fades_white() {
    use chimera_world::map::{
        logic_view::ScreenFades,
        particles::{ParticleMaterials, Particles},
    };
    let mut w = world(&[Crate::ahead(100.0)]);
    with_effects(&mut w);
    w.sim.app.world_mut().resource_mut::<Particles>().groups.clear();
    w.press(true, false, 1, 2);
    let (spark, beam) = {
        let m = &w.sim.app.world().resource::<ParticleMaterials>().0;
        (m.find("effects/spark").unwrap(), m.find("sprites/orangelight1").unwrap())
    };
    let ps = w.sim.app.world().resource::<Particles>();
    let sparks = ps.groups.iter().flat_map(|g| g.particles.iter()).filter(|p| p.material == spark).count();
    assert!(sparks >= 16, "{sparks}");
    let beams = ps.groups.iter().flat_map(|g| g.particles.iter()).filter(|p| p.material == beam).count();
    assert_eq!(beams, 1);
    let now = w.sim.app.world().resource::<bevy::prelude::Time>().elapsed_secs_f64();
    let mut fades = w.sim.app.world_mut().resource_mut::<ScreenFades>();
    let (color, alpha, _) = fades.at(now).expect("a fade");
    assert_eq!(color, [245, 245, 255]);
    assert!(alpha > 0 && alpha <= 32);
}

/// H9 11: the hold loop starts silent at pitch 50 and reaches 0.8 and 100
/// after 0.5 s; letting go fades it out over a second and stops it.
#[test]
fn the_hold_loop_ramps_up_and_down() {
    use chimera_world::map::SoundControl;
    let mut w = world(&[Crate::ahead(200.0)]);
    with_effects(&mut w);
    let mut cursor = w.sim.app.world().resource::<Messages<SoundControl>>().get_cursor_current();
    let mut msgs: Vec<SoundControl> = Vec::new();
    let mut run = |w: &mut W, ticks: u64, msgs: &mut Vec<SoundControl>| {
        for _ in 0..ticks {
            w.sim.ticks(1);
            msgs.extend(cursor.read(w.sim.app.world().resource::<Messages<SoundControl>>()).cloned());
        }
    };
    w.press(false, true, 1, 1);
    run(&mut w, 50, &mut msgs);
    let start = msgs.iter().find_map(|m| match m {
        SoundControl::Start(s) => Some(s.clone()),
        _ => None,
    });
    let start = start.expect("the loop starts");
    assert_eq!(start.entry, hl2dm::game::physcannon_fx::HOLD_SOUND);
    assert!(start.looping && start.volume == Some(0.0) && start.pitch == Some(50.0));
    let last = msgs.iter().rev().find_map(|m| match m {
        SoundControl::Change { volume, pitch, .. } => Some((*volume, *pitch)),
        _ => None,
    });
    assert_eq!(last, Some((Some(0.8), Some(100.0))));
    // Let go: a secondary press after 0.5 s drops it.
    w.press(false, true, 1, 1);
    run(&mut w, 100, &mut msgs);
    assert!(msgs.iter().any(|m| matches!(m, SoundControl::Stop(_))), "stopped after the fade");
}

/// The gun's attachments in a test view model and world model: attachment
/// 1 (`core`) and the forks, each at its own point (meters, the model's
/// frame).
fn claw_attachments() -> Vec<(String, Vec3)> {
    let mut list = vec![("core".to_string(), Vec3::new(0.0, -0.05, -0.5))];
    for (i, n) in ["fork1b", "fork1m", "fork1t", "fork2b", "fork2m", "fork2t", "fork3m", "fork3t"].iter().enumerate() {
        list.push((n.to_string(), Vec3::new(0.02 * i as f32 - 0.08, 0.03, -0.45)));
    }
    list
}

/// The particles drawn this frame with `material` (by name): their
/// positions and whether on the view model's layer.
fn drawn(w: &W, material: &str) -> Vec<(Vec3, bool)> {
    use chimera_world::map::particles::{ParticleMaterials, Particles, Shape};
    let m = w.sim.app.world().resource::<ParticleMaterials>().0.find(material).expect(material);
    let ps = w.sim.app.world().resource::<Particles>();
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

/// The playtest's lag: as the view turns, the claws' core and glows are on
/// their attachments in the same frame (the view model is the camera's
/// child and turns this frame; the camera's `GlobalTransform` waits for
/// `PostUpdate`).
#[test]
fn the_claws_keep_up_with_the_view_this_frame() {
    use chimera_world::map::{
        MapAttachment, MapBone, MapViewModel, ViewAnimator, ViewModelAnchor, ViewModelOffset, ViewModels, attached,
        particles::Particles, view_model::placement,
    };
    use hl2dm::game::physcannon_fx::{CORE, GLOW, VIEW_GLOWS};
    #[derive(Resource)]
    struct Anchor(Entity);
    fn turn(anchor: Res<Anchor>, mut t: Query<&mut Transform>) {
        t.get_mut(anchor.0).unwrap().rotate_y(0.1);
    }
    let mut w = world(&[Crate::ahead(200.0)]);
    with_effects(&mut w);
    let attachments: Vec<MapAttachment> = claw_attachments()
        .into_iter()
        .map(|(name, at)| MapAttachment { name, bone: 0, local: Transform::from_translation(at) })
        .collect();
    let model = MapViewModel {
        key: weapons::PHYSCANNON.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: attachments.clone(),
        light_origin: Vec3::ZERO,
    };
    w.sim.app.insert_resource(ViewModels::of(vec![model]));
    let p = w.p;
    let anchor = w
        .sim
        .app
        .world_mut()
        .spawn((ViewModelAnchor, Transform::from_xyz(0.0, 0.6, 0.0), GlobalTransform::default(), ChildOf(p)))
        .id();
    if w.sim.app.world().get::<ViewAnimator>(p).is_none() {
        w.sim.app.world_mut().entity_mut(p).insert(ViewAnimator { key: Some(weapons::PHYSCANNON.into()), ..default() });
    }
    w.sim.app.insert_resource(Anchor(anchor));
    attached::order_sets(&mut w.sim.app);
    w.sim.app.add_systems(
        Update,
        (
            turn.in_set(attached::EyePlaced),
            attached::draw_attached.in_set(attached::AttachedSet).after(chimera_world::map::particles::ParticleSet::Step).before(chimera_world::map::particles::ParticleSet::Draw),
        ),
    );
    w.sim.ticks(30);
    for _ in 0..3 {
        w.sim.app.world_mut().resource_mut::<Particles>().groups.clear();
        w.sim.ticks(1);
        let world = w.sim.app.world();
        let eye = *world.get::<Transform>(p).unwrap() * *world.get::<Transform>(anchor).unwrap();
        let offset = world.get::<ViewModelOffset>(p).copied().unwrap_or_default();
        let at = |name: &str| {
            let local = attachments.iter().find(|a| a.name == name).unwrap().local.translation;
            eye.transform_point(placement(&offset, false).transform_point(local))
        };
        let core = drawn(&w, CORE);
        assert!(core.iter().any(|(q, vm)| *vm && q.distance(at("core")) < 1e-3), "core {:?} not {}", core, at("core"));
        let glows = drawn(&w, GLOW);
        for name in VIEW_GLOWS {
            assert!(glows.iter().any(|(q, _)| q.distance(at(name)) < 1e-3), "{name}: {glows:?}");
        }
    }
}

/// H9 48 (effects_sounds.md 11, "Gravity gun sprites and beams"), first
/// person: ready, the core and six glows sit on the view model's
/// attachments (drawn on its layer); holding, the end caps on the two
/// fork tips and two beams from them to attachment 1.
#[test]
fn the_claws_glow_on_the_view_model_and_beam_while_holding() {
    use chimera_world::map::{
        MapAttachment, MapBone, MapViewModel, ViewAnimator, ViewModelAnchor, ViewModelOffset, ViewModels,
        view_model::placement,
    };
    use hl2dm::game::physcannon_fx::{CORE, END_CAP, GLOW, VIEW_BEAM};
    let mut w = world(&[Crate::ahead(200.0)]);
    with_effects(&mut w);
    let attachments: Vec<MapAttachment> = claw_attachments()
        .into_iter()
        .map(|(name, at)| MapAttachment { name, bone: 0, local: Transform::from_translation(at) })
        .collect();
    let model = MapViewModel {
        key: weapons::PHYSCANNON.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: attachments.clone(),
        light_origin: Vec3::ZERO,
    };
    w.sim.app.insert_resource(ViewModels::of(vec![model]));
    let eye = GlobalTransform::from(Transform::from_xyz(1.0, 2.0, 3.0));
    let p = w.p;
    w.sim.app.world_mut().spawn((ViewModelAnchor, eye, ChildOf(p)));
    if w.sim.app.world().get::<ViewAnimator>(p).is_none() {
        w.sim.app.world_mut().entity_mut(p).insert(ViewAnimator { key: Some(weapons::PHYSCANNON.into()), ..default() });
    }
    w.sim.ticks(30);
    let anchor_at = |w: &mut W| {
        let world = w.sim.app.world_mut();
        world.query_filtered::<&GlobalTransform, With<ViewModelAnchor>>().single(world).unwrap().compute_transform()
    };
    let at = |w: &mut W, name: &str| {
        let offset = w.sim.app.world().get::<ViewModelOffset>(w.p).copied().unwrap_or_default();
        let local = attachments.iter().find(|a| a.name == name).unwrap().local.translation;
        anchor_at(w).transform_point(placement(&offset, false).transform_point(local))
    };
    let core = drawn(&w, CORE);
    assert_eq!(core.len(), 1, "one core");
    assert!(core[0].1, "on the view model's layer");
    let want = at(&mut w, "core");
    assert!(core[0].0.distance(want) < 1e-3, "core at {} not {want}", core[0].0);
    let glows = drawn(&w, GLOW);
    assert_eq!(glows.len(), 6);
    for name in hl2dm::game::physcannon_fx::VIEW_GLOWS {
        let want = at(&mut w, name);
        assert!(glows.iter().any(|(p, vm)| *vm && p.distance(want) < 1e-3), "a glow at {name}");
    }
    assert!(drawn(&w, END_CAP).is_empty() && drawn(&w, VIEW_BEAM).is_empty(), "no caps or beams when ready");

    w.grab(0);
    w.sim.ticks(5);
    let caps = drawn(&w, END_CAP);
    assert_eq!(caps.len(), 2);
    for name in hl2dm::game::physcannon_fx::VIEW_TIPS {
        let want = at(&mut w, name);
        assert!(caps.iter().any(|(p, _)| p.distance(want) < 1e-3), "an end cap at {name}");
    }
    // Two beams of 8 segments, on the view model's layer.
    let beams = drawn(&w, VIEW_BEAM);
    assert_eq!(beams.len(), 16);
    assert!(beams.iter().all(|(_, vm)| *vm));
}

/// Others see the world model's set at its attachments (`HeldAttachment`):
/// the glows on `fork1m` to `fork3t`; holding, three end caps and three
/// beams; put away, nothing.
#[test]
fn the_claws_glow_on_the_world_model_for_others() {
    use chimera_world::map::view_model::HeldAttachment;
    use hl2dm::game::physcannon_fx::{END_CAP, GLOW, WORLD_BEAM, WORLD_GLOWS};
    let mut w = world(&[Crate::ahead(200.0)]);
    with_effects(&mut w);
    let p = w.p;
    let points = claw_attachments();
    for (index, (name, at)) in points.iter().enumerate() {
        let g = GlobalTransform::from(Transform::from_translation(*at + Vec3::Y));
        w.sim.app.world_mut().spawn((HeldAttachment { owner: p, index, name: name.clone() }, g));
    }
    w.sim.ticks(30);
    let glows = drawn(&w, GLOW);
    assert_eq!(glows.len(), 6);
    for name in WORLD_GLOWS {
        let want = points.iter().find(|(n, _)| n == name).unwrap().1 + Vec3::Y;
        assert!(glows.iter().any(|(p, vm)| !vm && p.distance(want) < 1e-4), "a glow at {name}");
    }
    w.grab(0);
    w.sim.ticks(5);
    assert_eq!(drawn(&w, END_CAP).len(), 3);
    assert_eq!(drawn(&w, WORLD_BEAM).len(), 24);
}

/// H9 49: a punt shows the blast sprite at attachment 1, 8 to 64 wide.
#[test]
fn a_punt_flashes_the_blast_sprite_at_the_core() {
    use chimera_world::map::view_model::HeldAttachment;
    use hl2dm::game::physcannon_fx::BLAST;
    let mut w = world(&[Crate::ahead(100.0)]);
    with_effects(&mut w);
    let p = w.p;
    let core = Vec3::new(0.3, 1.5, 0.2);
    w.sim.app.world_mut().spawn((HeldAttachment { owner: p, index: 0, name: "core".into() }, GlobalTransform::from_translation(core)));
    w.sim.ticks(10);
    assert!(drawn(&w, BLAST).is_empty());
    w.press(true, false, 1, 2);
    let blast = drawn(&w, BLAST);
    assert_eq!(blast.len(), 1);
    assert!(blast[0].0.distance(core) < 1e-4);
    w.sim.ticks(20);
    assert!(drawn(&w, BLAST).is_empty(), "gone after 0.2 s");
}

/// H9 11: a punt's effect ends at the punted object's centre (not at the
/// end of the eye's trace): the punter's beam reaches the crate's centre,
/// 100 units ahead at eye height.
#[test]
fn a_punts_beam_ends_at_the_objects_centre() {
    use chimera_world::map::particles::{ParticleMaterials, Particles, Shape};
    let mut w = world(&[Crate::ahead(100.0)]);
    with_effects(&mut w);
    w.sim.app.world_mut().resource_mut::<Particles>().groups.clear();
    w.press(true, false, 1, 2);
    let beam = w.sim.app.world().resource::<ParticleMaterials>().0.find("sprites/orangelight1").unwrap();
    let ends: Vec<Vec3> = w
        .sim
        .app
        .world()
        .resource::<Particles>()
        .groups
        .iter()
        .flat_map(|g| g.particles.iter())
        .filter(|p| p.material == beam)
        .filter_map(|p| match p.shape {
            Shape::Ribbon { to, .. } => Some(to),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 1);
    let centre = to_engine(FEET + Vec3::Z * EYE + Vec3::new(100.0, 0.0, 0.0));
    let off = ends[0].distance(centre) / U;
    assert!(off < 3.0, "the beam ends {off} units from the crate's centre ({:?})", to_source(ends[0]));
}
