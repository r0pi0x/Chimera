//! The set pieces' logic (specs/hl2dm/maps_entities.md 5, 10, 16, and the
//! game-handled classes' inputs): respawnable props, the angular velocity
//! sensor, the microphone. dt = 0.015 s.

use bevy::prelude::*;

use super::Value;
use super::prop_damage::Hit;
use crate::map::entities::{PROP_HEALTH_KEY, PROP_PIECES_KEY};
use super::tests::{fired, run_to, spawn, world};
use super::world::*;
use crate::core::DamageKind;
use crate::map::entities::AiSoundKind;

fn hit(amount: f32) -> Hit {
    Hit {
        amount,
        kind: DamageKind::Bullet,
        attacker: None,
        point: Vec3::ZERO,
        dir: Vec3::X,
        force: 0.0,
        direct: false,
    }
}

#[test]
fn a_respawnable_prop_keeps_its_name_and_comes_back() {
    let mut w = world();
    let p = spawn(
        &mut w,
        &[
            ("classname", "prop_physics_respawnable"),
            ("targetname", "crates"),
            ("RespawnTime", "5"),
            (PROP_HEALTH_KEY, "30"),
            (PROP_PIECES_KEY, "7"),
            ("OnBreak", "log,Trigger,,0,-1"),
        ],
    );
    w.activate();
    run_to(&mut w, 1);
    w.prop_hit(p, hit(100.0));
    assert!(w.prop(p).unwrap().broken);
    assert_eq!(w.get(p).unwrap().targetname, "crates", "same entity, name kept");
    let state = w.prop_states().into_iter().find(|s| s.id == p).unwrap();
    assert!(state.gone && !state.visible && !state.solid);
    // The respawn is asked of the host after 5 s (334 ticks, compared timer).
    w.effects.clear();
    run_to(&mut w, 1 + 334);
    let asked = w.effects.iter().filter(|e| matches!(e, Effect::PropRespawn { .. })).count();
    assert_eq!(asked, 1);
    // Blocked: asked again after a second (67 ticks).
    w.effects.clear();
    w.prop_respawn_blocked(p);
    run_to(&mut w, 1 + 334 + 68);
    assert_eq!(w.effects.iter().filter(|e| matches!(e, Effect::PropRespawn { .. })).count(), 1);
    // Free: revived with its health, and breakable again (OnBreak twice).
    w.prop_revive(p);
    assert!(!w.prop(p).unwrap().broken);
    assert_eq!(w.prop_health(p), Some((30, 30)));
    let t = w.tick;
    run_to(&mut w, t + 2);
    w.prop_hit(p, hit(100.0));
    assert_eq!(fired(&w, p, "OnBreak").len(), 2);
}

#[test]
fn the_sensor_fires_after_its_interval_once_the_spin_passes_the_threshold() {
    let mut w = world();
    let fan = spawn(&mut w, &[("classname", "func_physbox"), ("targetname", "fan"), ("health", "0")]);
    let s = spawn(
        &mut w,
        &[
            ("classname", "point_angularvelocitysensor"),
            ("target", "fan"),
            ("threshold", "1400"),
            ("fireinterval", "0.2"),
            ("OnGreaterThan", "x,Enable,,0,-1"),
            ("OnLessThan", "x,Disable,,0,-1"),
        ],
    );
    w.activate();
    w.set_prop_spin(fan, Vec3::new(0.0, 0.0, 0.0));
    run_to(&mut w, 5);
    // The very first measurement fires nothing.
    assert!(fired(&w, s, "OnLessThan").is_empty());
    let t0 = w.tick;
    w.set_prop_spin(fan, Vec3::new(1500.0, 0.0, 0.0));
    run_to(&mut w, t0 + 30);
    let ticks = fired(&w, s, "OnGreaterThan");
    assert_eq!(ticks.len(), 1, "once");
    // 0.2 s = 13.3 ticks after the change is seen.
    assert!((ticks[0] - t0 - 14).abs() <= 2, "fired at {} for a change at {t0}", ticks[0]);
    assert_eq!(fired(&w, s, "OnGreaterThanOrEqualTo").len(), 1);
    // Slowing again fires OnLessThan.
    let t1 = w.tick;
    w.set_prop_spin(fan, Vec3::new(100.0, 0.0, 0.0));
    run_to(&mut w, t1 + 30);
    assert_eq!(fired(&w, s, "OnLessThan").len(), 1);
}

#[test]
fn the_microphone_hears_bullets_in_range_and_not_beyond() {
    let mut w = world();
    let m = spawn(
        &mut w,
        &[
            ("classname", "env_microphone"),
            ("spawnflags", "40"),
            ("MaxRange", "256"),
            ("Sensitivity", "1"),
            ("StartDisabled", "1"),
            ("origin", "0 0 0"),
        ],
    );
    w.activate();
    // Disabled: hears nothing.
    w.add_ai_sound(Vec3::new(50.0, 0.0, 0.0), 200.0, AiSoundKind::BulletImpact, 0.5);
    run_to(&mut w, 60);
    assert!(fired(&w, m, "OnHeardSound").is_empty());
    w.deliver(Who::Ent(m), "Enable", Value::Void, None, None);
    let t = w.tick;
    // Beyond volume (200 units): not heard; a footstep kind is masked out.
    w.add_ai_sound(Vec3::new(260.0, 0.0, 0.0), 300.0, AiSoundKind::BulletImpact, 0.5);
    w.add_ai_sound(Vec3::new(10.0, 0.0, 0.0), 300.0, AiSoundKind::World, 0.5);
    run_to(&mut w, t + 20);
    assert!(fired(&w, m, "OnHeardSound").is_empty());
    w.add_ai_sound(Vec3::new(150.0, 0.0, 0.0), 200.0, AiSoundKind::BulletImpact, 0.5);
    let t = w.tick;
    run_to(&mut w, t + 10);
    assert!(!fired(&w, m, "OnHeardSound").is_empty());
}

#[test]
fn inputs_to_a_game_class_go_out_as_effects() {
    let mut w = world();
    w.game_classes.push("item_suitcharger".into());
    let c = spawn(&mut w, &[("classname", "item_suitcharger"), ("targetname", "megacharger")]);
    w.get_mut(c).unwrap().map_index = Some(7);
    w.activate();
    w.deliver(Who::Ent(c), "SetCharge", Value::Str("50".into()), None, None);
    assert!(w.effects.iter().any(|e| matches!(
        e,
        Effect::EntityInput { map_index: Some(7), input, value, .. } if input == "setcharge" && value == "50"
    )));
    assert!(!w.log.iter().any(|l| l.contains("unhandled")));
}
