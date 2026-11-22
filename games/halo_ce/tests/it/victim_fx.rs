//! What a hit shows its victim (`game::fx::victim`, specs/halo_ce/
//! effects.md "Damage effect presentation", low_health.md 5): the flash's
//! intensity, fade and replacement, the impulse's and shake's rules, the
//! permanent impulse's turn, on synthetic tag numbers; then in the game: a
//! hit with a known damage effect starts them on its local victim only,
//! offline and over the network (the client hit, its server none); and
//! the overshield's drain is no shield hit (`vitality::mark`).

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use bevy::prelude::*;
use blam_formats::{object::TransitionFunction, shader::PeriodicFunction};
use chimera_core::core::{Intent, LocalPlayer};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        damage::HaloHit,
        fx::{
            ActiveFx, HaloFx,
            victim::{DamageLook, FlashLook, FlashRun, HaloVictim, ImpulseLook, ShakeLook, ShakeRun, VictimFx, VictimView, permanent_turn, ticks_of},
        },
        movement,
        vitality::{DamageEffects, Spartan, ids, mark::{DRAIN_ALLOWANCE, ShieldMark}},
    },
    harness::HaloSim,
};

/// A lighten flash of 1 s, linear, maximum 0.5, zero scale 0.2, green.
fn flash() -> FlashLook {
    FlashLook { kind: 1, priority: 1, duration: 1.0, fade: TransitionFunction::Linear, maximum: 0.5, zero: 0.2, color: [0.0, 1.0, 0.0, 1.0] }
}

fn impulse() -> ImpulseLook {
    ImpulseLook { duration: 0.2, fade: TransitionFunction::Early, rotation: 0.0262, pushback: 0.1, jitter: [0.0, 0.0], zero: 0.0 }
}

/// The frag's shock wave's shake (effects.md): 1.5 s late, 0.1 WU, no
/// rotation, wobble weight 0.4 on a cosine of 6 s.
fn shake() -> ShakeLook {
    ShakeLook {
        duration: 1.5,
        falloff: TransitionFunction::Late,
        translation: 0.1,
        rotation: 0.0,
        wobble: PeriodicFunction::Cosine,
        wobble_period: 6.0,
        wobble_weight: 0.4,
        zero: 0.0,
    }
}

fn look() -> DamageLook {
    DamageLook { flash: Some(flash()), impulse: Some(impulse()), permanent: Some((0.0698, 0.0)), shake: Some(shake()) }
}

#[test]
fn durations_truncate_to_ticks() {
    assert_eq!(ticks_of(1.0), 30.0);
    assert_eq!(ticks_of(0.05), 1.0);
    assert_eq!(ticks_of(0.099), 2.0);
}

/// I = clamp(z + (1 - z) s, 0, max); shown = fade(I left / duration),
/// I on the first frame, then counting down; gone after its ticks.
#[test]
fn a_flash_fades_from_its_clamped_intensity() {
    let mut f = FlashRun::new(&flash(), 0.25);
    assert!((f.intensity - 0.4).abs() < 1e-6, "0.2 + 0.8 x 0.25");
    assert_eq!(FlashRun::new(&flash(), 1.0).intensity, 0.5, "clamped to the maximum");
    assert!((f.frame(1.0).unwrap() - 0.4).abs() < 1e-6);
    assert!((f.frame(1.0).unwrap() - 0.4 * 29.0 / 30.0).abs() < 1e-6);
    for _ in 0..28 {
        f.frame(1.0).unwrap();
    }
    assert_eq!(f.frame(1.0), None, "30 ticks");
    // A late fade squares the fraction.
    let mut late = FlashRun::new(&FlashLook { fade: TransitionFunction::Late, ..flash() }, 1.0);
    assert!((late.frame(15.0).unwrap() - 0.25).abs() < 1e-6, "late(0.5)");
}

/// A new flash replaces the current one unless the current has a higher
/// priority and more ticks left than the new one's duration.
#[test]
fn a_higher_priority_flash_keeps_its_place() {
    let high = FlashRun::new(&FlashLook { priority: 2, duration: 2.0, ..flash() }, 1.0);
    let low = FlashRun::new(&FlashLook { priority: 0, duration: 1.0, ..flash() }, 1.0);
    assert!(!high.replaced_by(&low));
    assert!(low.replaced_by(&high));
    let long_low = FlashRun::new(&FlashLook { priority: 0, duration: 3.0, ..flash() }, 1.0);
    assert!(high.replaced_by(&long_low), "the new one outlasts it");
}

/// The shake: magnitude 1 on its first frame, then falloff(1 - e/d) x
/// intensity x (wobble x weight + 1 - weight); a longer or stronger one
/// replaces it.
#[test]
fn a_shake_falls_off_with_its_wobble() {
    let tables = blam_formats::shader::PeriodicTables::new();
    let mut s = ShakeRun::new(&shake(), 1.0);
    assert_eq!(s.duration, 45.0);
    assert_eq!(s.magnitude(1.0, &tables), Some(1.0));
    let m = s.magnitude(1.0, &tables).unwrap();
    let w = tables.value(PeriodicFunction::Cosine, 1.0 / 180.0);
    assert!((m - (1.0 - 1.0 / 45.0f32).powi(2) * (w * 0.4 + 0.6)).abs() < 1e-5, "{m}");
    let weaker = ShakeRun::new(&ShakeLook { duration: 0.5, ..shake() }, 0.5);
    assert!(!s.replaced_by(&weaker));
    assert!(s.replaced_by(&ShakeRun::new(&ShakeLook { duration: 3.0, ..shake() }, 0.5)));
}

/// The impulse turns the view about up x direction and pushes it along
/// the direction; k is 1 on its first frame, then fades.
#[test]
fn an_impulse_pushes_along_the_hit() {
    let mut fx = VictimFx::default();
    // Hit from behind: the damage travels forward (-z).
    fx.hit(&DamageLook { impulse: Some(impulse()), ..Default::default() }, 1.0, Vec3::NEG_Z);
    let v = fx.frame(1.0);
    assert!(v.offset.z < 0.0 && v.offset.x.abs() < 1e-6, "{v:?}");
    assert!((v.offset.length() - 0.1 * halo_ce::game::METERS_PER_UNIT).abs() < 1e-4);
    assert!((v.rotation - Vec3::NEG_X * 0.0262).length() < 1e-6, "{v:?}");
    // 6 ticks; the next frame fades by early(1 - 1/6).
    let v = fx.frame(1.0);
    assert!((v.rotation.length() - 0.0262 * (5.0f32 / 6.0).sqrt()).abs() < 1e-6);
    for _ in 0..5 {
        fx.frame(1.0);
    }
    assert_eq!(fx.frame(1.0).rotation, Vec3::ZERO, "over");
}

/// The permanent impulse: yaw by (left . d), pitch by (forward . d).
#[test]
fn the_permanent_impulse_turns_the_aim() {
    // Looking along -z; pushed toward the left (-x): a yaw to the left.
    let (yaw, pitch) = permanent_turn(0.0698, 0.0, 1.0, Vec3::NEG_X, 0.0, 0.0);
    assert!((yaw - 0.0698).abs() < 1e-6 && pitch.abs() < 1e-6);
    let (yaw, pitch) = permanent_turn(0.0698, 0.5, 0.0, Vec3::NEG_Z, 0.0, 0.0);
    assert!(yaw.abs() < 1e-6 && (pitch - 0.0349).abs() < 1e-6, "i = z at scale 0");
}

/// The overshield's drain lowers the HUD's memory with it: no mark, no
/// `shield_hit`; a hit on an overshield still marks.
#[test]
fn an_overshield_draining_is_no_hit() {
    let drain = Spartan::load().constants.overshield_drain;
    assert!((DRAIN_ALLOWANCE - 4.0 * drain).abs() < 1e-7, "four ticks of the data's drain");
    let mut m = ShieldMark::default();
    let mut s = 2.0;
    m.update(s, 0);
    for t in 1..100 {
        s -= drain;
        m.update(s, t);
        assert_eq!(m.last, s, "tick {t}");
        assert!(m.fade < 0.0);
    }
    // Two ticks' drain at once (a client's replication).
    s -= 2.0 * drain;
    m.update(s, 100);
    assert_eq!(m.last, s);
    // To 1.0 exactly, clamped.
    let mut m = ShieldMark::default();
    m.update(1.0002, 0);
    m.update(1.0, 1);
    assert_eq!((m.last, m.fade), (1.0, -1.0));
    // A hit on the overshield marks.
    let mut m = ShieldMark::default();
    m.update(2.0, 0);
    m.update(1.6, 1);
    assert_eq!((m.last, m.fade), (2.0, 0.0));
}

/// The library with `look` for the frag explosion's tag.
fn library(effects: &DamageEffects) -> ActiveFx {
    let tag = effects.get(ids::FRAG_EXPLOSION).unwrap().tag.to_ascii_lowercase();
    ActiveFx(Some(Arc::new(HaloFx { damage: BTreeMap::from([(tag, look())]), ..Default::default() })))
}

fn frag(effects: &DamageEffects, target: Entity, from: Option<Entity>) -> HaloHit {
    let amount = effects.get(ids::FRAG_EXPLOSION).unwrap().amount(0.5, 0.0);
    HaloHit::area(target, ids::FRAG_EXPLOSION, amount, 0.5, Vec3::ZERO, Vec3::NEG_X, from)
}

/// Offline: a hit on another character shows nothing; on the local
/// player its flash (I = 0.2 + 0.8 x 0.5 clamped to 0.5), shake and
/// impulse start and the aim turns; the frame's `HaloVictim` names the
/// effect and that someone else dealt it.
#[test]
fn a_hit_shows_on_its_local_victim_only() {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    let me = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let other = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let effects = sim.app.world().resource::<DamageEffects>().clone();
    sim.app.insert_resource(library(&effects));
    sim.ticks(2);
    sim.app.world_mut().write_message(frag(&effects, other, Some(me)));
    sim.ticks(2);
    assert_eq!(*sim.app.world().resource::<VictimView>(), VictimView::default(), "another's hit");
    assert!(sim.app.world().resource::<VictimFx>().flash.is_none());
    let yaw = sim.app.world().get::<Intent>(me).unwrap().yaw;
    let mut seen = sim.app.world().resource::<Messages<HaloVictim>>().get_cursor_current();
    sim.app.world_mut().write_message(frag(&effects, me, Some(other)));
    sim.ticks(1);
    let victims: Vec<HaloVictim> = seen.read(sim.app.world().resource::<Messages<HaloVictim>>()).copied().collect();
    assert_eq!(victims.len(), 1, "{victims:?}");
    assert_eq!(victims[0].effect, effects.index_of(ids::FRAG_EXPLOSION.0).unwrap());
    assert!(victims[0].directed && victims[0].scale == 0.5);
    let fx = sim.app.world().resource::<VictimFx>();
    let f = fx.flash.as_ref().expect("the flash");
    assert_eq!((f.kind, f.intensity, f.duration), (1, 0.5, 30.0));
    assert!(fx.shake.is_some() && fx.impulse.is_some());
    let view = *sim.app.world().resource::<VictimView>();
    assert_eq!(view.flash.map(|f| f.color), Some([0.0, 1.0, 0.0, 1.0]));
    assert!(view.offset != Vec3::ZERO);
    // Pushed toward -x while looking along -z: a turn to the left,
    // 0.0698 x i (i = 0.5 at scale 0.5).
    let turned = sim.app.world().get::<Intent>(me).unwrap().yaw - yaw;
    assert!((turned - 0.0349).abs() < 1e-4, "{turned}");
}

/// Over the network: the server's hit on a client's character reaches
/// that client (`HaloVictimNet`) and starts its flash; the server shows
/// nothing.
#[test]
fn a_clients_hit_flashes_its_own_screen() {
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 7, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    let effects = sim.server.app.world().resource::<DamageEffects>().clone();
    sim.server.app.insert_resource(library(&effects));
    sim.clients[0].app.insert_resource(library(&effects));
    let theirs = sim.character_of(0).expect("the client's player on the server");
    sim.server.app.world_mut().write_message(frag(&effects, theirs, None));
    let mut flashed = false;
    for _ in 0..20 {
        sim.ticks(1);
        flashed |= sim.clients[0].app.world().resource::<VictimFx>().flash.is_some();
    }
    assert!(flashed, "the client's flash");
    assert!(sim.server.app.world().resource::<VictimFx>().flash.is_none(), "not the server's");
}
