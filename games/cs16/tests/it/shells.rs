//! CS 1.6's spent cases (specs/cs16/shells.md, test cases): the start and
//! the velocity of an event shell for the local right- and left-handed
//! shooter, another player's gun, the Deagle and the Elites, a running
//! shooter; the late shell of the M3 and its place and speeds; every gun
//! has its line; a shot's event throws its case at once (the AK-47) or
//! 0.45 s later (the M3), 2.5 s of life, the model of its table.

use bevy::prelude::*;
use chimera_movement::{to_engine, to_units};
use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::shells::ThrowShell};
use cs16::{game::weapons::shells as cs, harness::Cs16Sim};

fn view(velocity: Vec3) -> cs::View {
    cs::View::of(
        to_engine(Vec3::new(0.0, 0.0, 17.0)),
        -std::f32::consts::FRAC_PI_2,
        0.0,
        to_engine(velocity),
    )
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-3
}

/// The lowest draws, then the highest.
fn throws(gun: &str, local: bool, right_handed: bool, side: f32, v: Vec3) -> (cs::Throw, cs::Throw) {
    let guns = cs::guns();
    let g = &guns[gun];
    let off = cs::offsets(g, local, right_handed, side);
    let view = view(v);
    (
        cs::event_throw(&view, off, &mut |lo, _| lo),
        cs::event_throw(&view, off, &mut |_, hi| hi),
    )
}

#[test]
fn the_local_ak47s_case() {
    let (lo, hi) = throws("ak47", true, true, 0.0, Vec3::ZERO);
    assert!(close(lo.start, Vec3::new(20.0, -10.0, 9.0)), "{:?}", lo.start);
    // x in [25, 250], y in [50, 70] (the flip: s = -1 along r), z in [75, 175].
    assert!(close(lo.velocity, Vec3::new(25.0, 50.0, 75.0)), "{:?}", lo.velocity);
    assert!(close(hi.velocity, Vec3::new(250.0, 70.0, 175.0)), "{:?}", hi.velocity);
}

#[test]
fn left_handed_mirrors_the_side() {
    let (lo, hi) = throws("ak47", true, false, 0.0, Vec3::ZERO);
    assert!(close(lo.start, Vec3::new(20.0, 10.0, 9.0)), "{:?}", lo.start);
    assert!(close(lo.velocity, Vec3::new(25.0, -50.0, 75.0)) && close(hi.velocity, Vec3::new(250.0, -70.0, 175.0)));
    // The SG 550's left-handed forward offset is 20, not 17.
    let (a, _) = throws("sg550", true, true, 0.0, Vec3::ZERO);
    let (b, _) = throws("sg550", true, false, 0.0, Vec3::ZERO);
    assert_eq!((a.start.x, b.start.x), (17.0, 20.0));
}

#[test]
fn another_players_case_and_the_pistols() {
    let (lo, _) = throws("ak47", false, true, 0.0, Vec3::ZERO);
    assert!(close(lo.start, Vec3::new(20.0, -4.0, 5.0)), "{:?}", lo.start);
    let (d, _) = throws("deagle", true, true, 0.0, Vec3::ZERO);
    assert!(close(d.start, Vec3::new(35.0, -16.0, 6.0)), "{:?}", d.start);
    // The Elites: the right gun's shot starts left of the eye's right, +16.
    let (e, e_hi) = throws("elite", true, true, -1.0, Vec3::ZERO);
    assert!(close(e.start, Vec3::new(35.0, 16.0, 6.0)), "{:?}", e.start);
    assert!(close(e.velocity, Vec3::new(25.0, -50.0, 75.0)) && e_hi.velocity.y == -70.0);
    let (l, _) = throws("elite", true, true, 1.0, Vec3::ZERO);
    assert!(close(l.start, Vec3::new(35.0, -16.0, 6.0)), "{:?}", l.start);
}

#[test]
fn a_running_shooters_case_leads_and_carries() {
    let (lo, _) = throws("ak47", true, true, 0.0, Vec3::new(250.0, 0.0, 0.0));
    assert!((lo.start.x - 45.0).abs() < 1e-3, "{:?}", lo.start);
    assert!((lo.velocity.x - (25.0 + 125.0)).abs() < 1e-3, "{:?}", lo.velocity);
}

#[test]
fn the_m3s_late_case() {
    let v = view(Vec3::ZERO);
    let lo = cs::late_throw(&v, &mut |lo, _| lo);
    let hi = cs::late_throw(&v, &mut |_, hi| hi);
    assert!(close(lo.start, Vec3::new(16.0, 0.0, 8.0)), "{:?}", lo.start);
    // x 25, y in [-70, -50] (+U r with r = -y), z in [100, 150].
    assert!(close(lo.velocity, Vec3::new(25.0, -50.0, 100.0)) && close(hi.velocity, Vec3::new(25.0, -70.0, 150.0)));
}

#[test]
fn every_gun_has_its_line_and_the_late_ones_their_delays() {
    let guns = cs::guns();
    for w in cs16::game::weapons::weapons().weapons {
        let Some(id) = w.id.strip_prefix("cs16:weapon_") else {
            continue;
        };
        if matches!(id, "knife" | "hegrenade" | "flashbang" | "smokegrenade" | "standin") || w.id.contains("ammo") {
            continue;
        }
        assert!(guns.contains_key(id), "{id}");
    }
    assert_eq!(
        (guns["m3"].late, guns["scout"].late, guns["awp"].late),
        (Some(0.45), Some(0.56), Some(0.55))
    );
    assert_eq!(guns["m3"].key(), "shotgunshell_shotgun");
    assert_eq!(guns["xm1014"].key(), "shotgunshell");
    assert_eq!(guns["awp"].model, "rshell_big");
    assert_eq!(guns["glock18"].model, "pshell");
}

#[derive(Resource, Default)]
struct Thrown(Vec<(f64, ThrowShell)>);

fn collect(mut thrown: ResMut<Thrown>, mut shells: MessageReader<ThrowShell>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    thrown.0.extend(shells.read().map(|s| (now, s.clone())));
}

fn fire(sim: &mut Sim, owner: Entity, id: &'static str) {
    let w = sim.app.world_mut().spawn_empty().id();
    sim.app.world_mut().write_message(WeaponEvent {
        owner,
        weapon: w,
        kind: WeaponEventKind::Fired {
            id,
            origin: to_engine(Vec3::new(0.0, 0.0, 17.0)),
            yaw: 0.0,
            pitch: 0.0,
            seed: 0,
            spread: chimera_sim::weapon::SpreadShape::Template(0.0),
            mode: 0,
        },
        replay: false,
    });
}

#[test]
fn a_shot_throws_its_case_now_or_late() {
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.app.init_resource::<Thrown>().add_systems(Last, collect);
    let p = sim.spawn_character(chimera_world::greybox::SPAWNS[0], cs16::game::movement::ID);
    sim.ticks(5);
    fire(&mut sim, p, "cs16:weapon_ak47");
    sim.ticks(2);
    {
        let t = &sim.app.world().resource::<Thrown>().0;
        assert_eq!(t.len(), 1, "one case at once");
        assert_eq!((t[0].1.game.as_str(), t[0].1.key.as_str()), ("cs16", "rshell"));
    }
    sim.app.world_mut().resource_mut::<Thrown>().0.clear();
    let start = sim.app.world().resource::<Time>().elapsed_secs_f64();
    fire(&mut sim, p, "cs16:weapon_m3");
    sim.ticks(30);
    assert!(
        sim.app.world().resource::<Thrown>().0.is_empty(),
        "no case at the shot (0.3 s)"
    );
    sim.ticks(25);
    let t = &sim.app.world().resource::<Thrown>().0;
    assert_eq!(t.len(), 1, "one case 0.45 s on");
    assert_eq!(t[0].1.key, "shotgunshell_shotgun");
    let late = t[0].0 - start;
    assert!((0.45..0.6).contains(&late), "{late}");
    let _ = to_units(Vec3::ZERO);
}
