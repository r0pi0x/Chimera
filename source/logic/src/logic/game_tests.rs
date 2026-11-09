//! Test cases from specs/source/game_entities.md (player_speedmod,
//! game_ui, env_fade, game_score, env_hudhint, env_explosion,
//! func_wall_toggle, func_conveyor) on the logic world, and the client
//! fade arithmetic.

use super::*;
use crate::core::buttons;
use crate::logic::hud::{HudShow, ScreenFade, ScreenFades};

fn player(w: &mut LogicWorld) -> Entity {
    player_at(w, 1, Vec3::ZERO)
}

fn effects(w: &mut LogicWorld) -> Vec<Effect> {
    std::mem::take(&mut w.effects)
}

/// Values an output fired, in order.
fn values(w: &LogicWorld, target: EntId, input: &str) -> Vec<Value> {
    w.deliveries
        .iter()
        .filter(|d| d.target == Who::Ent(target) && d.input.eq_ignore_ascii_case(input))
        .map(|d| d.value.clone())
        .collect()
}

// ------------------------------------------------------- player_speedmod

#[test]
fn speedmod_needs_a_player_activator() {
    let mut w = world();
    let p = player(&mut w);
    spawn(&mut w, &[("classname", "player_speedmod"), ("targetname", "speed"), ("spawnflags", "4")]);
    w.activate();
    w.queue_input("speed", "ModifySpeed", Value::Str("2".into()), 0.0, Some(Who::Player(p)));
    run_to(&mut w, 0);
    assert!(effects(&mut w).contains(&Effect::SpeedMod {
        target: p,
        key: 0,
        scale: 2.0,
        flags: 4
    }));
    // Fired by logic_auto (no player): nobody.
    w.queue_input("speed", "ModifySpeed", Value::Str("2".into()), 0.0, None);
    run_to(&mut w, 1);
    assert!(!effects(&mut w).iter().any(|e| matches!(e, Effect::SpeedMod { .. })));
}

#[test]
fn speedmod_flags_take_buttons() {
    use super::super::game::speedmod_buttons;
    assert_eq!(speedmod_buttons(4), buttons::JUMP);
    assert_eq!(speedmod_buttons(64), buttons::ATTACK | buttons::ATTACK2);
    assert_eq!(speedmod_buttons(8 | 16 | 32 | 128), buttons::DUCK | buttons::USE | buttons::SPEED | buttons::ZOOM);
}

// --------------------------------------------------------------- game_ui

/// A game_ui "ui" with `flags` and the given FieldOfView, its outputs
/// into relays, and a player in front of it.
fn ui_world(flags: &str, fov: &str) -> (LogicWorld, EntId, Entity) {
    let mut w = world();
    let p = player(&mut w);
    let mut pairs = vec![
        ("classname", "game_ui"),
        ("targetname", "ui"),
        ("spawnflags", flags),
        ("FieldOfView", fov),
        ("origin", "100 0 31"),
    ];
    let outs = [
        ("PlayerOn", "on,Trigger,,0,-1"),
        ("PlayerOff", "off,Trigger,,0,-1"),
        ("PressedForward", "fwd,Trigger,,0,-1"),
        ("UnpressedForward", "unfwd,Trigger,,0,-1"),
        ("XAxis", "x,InValue,,0,-1"),
        ("YAxis", "y,InValue,,0,-1"),
        ("AttackAxis", "a,InValue,,0,-1"),
        ("Attack2Axis", "a2,InValue,,0,-1"),
    ];
    pairs.extend(outs);
    let ui = spawn(&mut w, &pairs);
    for name in ["on", "off", "fwd", "unfwd"] {
        relay(&mut w, name, &[]);
    }
    for name in ["x", "y", "a", "a2"] {
        spawn(&mut w, &[("classname", "logic_case"), ("targetname", name)]);
    }
    w.activate();
    w.queue_input("ui", "Activate", Value::Void, 0.0, Some(Who::Player(p)));
    run_to(&mut w, 0);
    (w, ui, p)
}

fn named(w: &LogicWorld, name: &str) -> EntId {
    w.find(name).unwrap()
}

fn floats(w: &LogicWorld, name: &str) -> Vec<f32> {
    values(w, named(w, name), "InValue").iter().filter_map(|v| v.to_float()).collect()
}

#[test]
fn game_ui_fires_axes_once_on_activation() {
    let (mut w, _, p) = ui_world("32", "-1");
    assert_eq!(got(&w, named(&w, "on"), "Trigger").len(), 1);
    assert!(effects(&mut w).contains(&Effect::GameUi {
        target: p,
        on: true,
        freeze: true,
        hide_weapon: false
    }));
    run_to(&mut w, 1);
    for axis in ["x", "y", "a", "a2"] {
        assert_eq!(floats(&w, axis), vec![0.0], "{axis}");
    }
    run_to(&mut w, 5);
    assert_eq!(floats(&w, "y"), vec![0.0], "no repeats");
}

#[test]
fn game_ui_reports_keys() {
    let (mut w, _, _) = ui_world("32", "-1");
    run_to(&mut w, 1);
    w.players[0].buttons = buttons::FORWARD;
    run_to(&mut w, 2);
    assert_eq!(got(&w, named(&w, "fwd"), "Trigger"), vec![2]);
    assert_eq!(floats(&w, "y"), vec![0.0, 1.0]);
    // Forward wins over back.
    w.players[0].buttons = buttons::FORWARD | buttons::BACK;
    run_to(&mut w, 3);
    assert_eq!(floats(&w, "y"), vec![0.0, 1.0]);
    w.players[0].buttons = 0;
    run_to(&mut w, 4);
    assert_eq!(got(&w, named(&w, "unfwd"), "Trigger"), vec![4]);
    assert_eq!(floats(&w, "y"), vec![0.0, 1.0, 0.0]);
    // Left, then right too: right wins while both are held.
    w.players[0].buttons = buttons::MOVELEFT;
    run_to(&mut w, 5);
    w.players[0].buttons = buttons::MOVELEFT | buttons::MOVERIGHT;
    run_to(&mut w, 6);
    assert_eq!(floats(&w, "x"), vec![0.0, -1.0, 1.0]);
}

#[test]
fn keys_held_at_activation_are_not_pressed() {
    let mut w = world();
    let p = player(&mut w);
    w.players[0].buttons = buttons::FORWARD;
    spawn(&mut w, &[("classname", "game_ui"), ("targetname", "ui"), ("PressedForward", "fwd,Trigger,,0,-1")]);
    relay(&mut w, "fwd", &[]);
    w.activate();
    w.queue_input("ui", "Activate", Value::Void, 0.0, Some(Who::Player(p)));
    run_to(&mut w, 5);
    assert!(got(&w, named(&w, "fwd"), "Trigger").is_empty());
}

#[test]
fn game_ui_jump_deactivates_and_zeroes_the_axes() {
    let (mut w, _, p) = ui_world("288", "-1");
    run_to(&mut w, 1);
    effects(&mut w);
    w.players[0].buttons = buttons::JUMP;
    run_to(&mut w, 2);
    assert_eq!(got(&w, named(&w, "off"), "Trigger"), vec![2]);
    for axis in ["x", "y", "a", "a2"] {
        assert_eq!(floats(&w, axis), vec![0.0, 0.0], "{axis}: first tick, then Deactivate");
    }
    assert!(effects(&mut w).contains(&Effect::GameUi {
        target: p,
        on: false,
        freeze: true,
        hide_weapon: false
    }));
}

#[test]
fn game_ui_has_one_driver() {
    let (mut w, ui, p) = ui_world("0", "-1");
    let q = player_at(&mut w, 2, Vec3::ZERO);
    w.queue_input("ui", "Activate", Value::Void, 0.0, Some(Who::Player(q)));
    run_to(&mut w, 3);
    let Class::GameUi(u) = &w.get(ui).unwrap().class else { panic!() };
    assert_eq!(u.player, Some(p));
}

#[test]
fn game_ui_field_of_view() {
    // The game_ui 60° off the view stays (dot 0.5 is not below 0.5); 61°
    // deactivates.
    for (yaw, stays) in [(59.99, true), (61.0, false)] {
        let (mut w, ui, _) = ui_world("0", "0.5");
        w.players[0].view = Vec3::new(0.0, yaw, 0.0);
        run_to(&mut w, 2);
        let Class::GameUi(u) = &w.get(ui).unwrap().class else { panic!() };
        assert_eq!(u.player.is_some(), stays, "{yaw}°");
    }
}

// ------------------------------------------------------------------ fade

#[test]
fn fade_out_ramps_holds_and_goes() {
    let f = ScreenFade {
        duration: 2.0,
        hold: 1.0,
        color: [0, 0, 0],
        alpha: 255,
        fade_in: false,
        modulate: false,
        stay_out: false,
        purge: true,
    };
    assert_eq!(f.alpha_at(1.0), Some(127));
    assert_eq!(f.alpha_at(2.5), Some(255));
    assert_eq!(f.alpha_at(3.01), None);
    let fade_in = ScreenFade { fade_in: true, ..f };
    assert_eq!(fade_in.alpha_at(0.5), Some(255));
    assert_eq!(fade_in.alpha_at(2.0), Some(127));
    assert_eq!(fade_in.alpha_at(3.01), None);
    let stay = ScreenFade {
        duration: 1.0,
        stay_out: true,
        ..f
    };
    assert_eq!(stay.alpha_at(100.0), Some(255));
    let instant = ScreenFade { duration: 0.0, ..stay };
    assert_eq!(instant.alpha_at(0.0), Some(255));
    assert_eq!(instant.alpha_at(1e4), Some(255));
    let nothing = ScreenFade { duration: 0.0, ..f };
    assert_eq!(nothing.alpha_at(0.0), None);
    assert!((ScreenFade::quantize(0.3) - 153.0 / 512.0).abs() < 1e-6);
}

#[test]
fn fades_mix() {
    let red = ScreenFade {
        duration: 0.0,
        hold: 0.0,
        color: [255, 0, 0],
        alpha: 100,
        fade_in: false,
        modulate: false,
        stay_out: true,
        purge: false,
    };
    let blue = ScreenFade {
        color: [0, 0, 255],
        alpha: 200,
        ..red
    };
    let mut fades = ScreenFades::default();
    fades.show(red, 0.0);
    fades.show(blue, 0.0);
    assert_eq!(fades.at(1.0), Some(([127, 0, 127], 200, false)));
    // A purging fade replaces both.
    fades.show(ScreenFade { purge: true, ..red }, 1.0);
    assert_eq!(fades.at(2.0), Some(([255, 0, 0], 100, false)));
}

#[test]
fn env_fade_targets() {
    let mut w = world();
    let p = player(&mut w);
    spawn(
        &mut w,
        &[
            ("classname", "env_fade"),
            ("targetname", "all"),
            ("duration", "0.3"),
            ("holdtime", "1"),
            ("renderamt", "255"),
            ("rendercolor", "10 20 30"),
        ],
    );
    spawn(&mut w, &[("classname", "env_fade"), ("targetname", "me"), ("spawnflags", "4"), ("renderamt", "200")]);
    w.activate();
    w.queue_input("all", "Fade", Value::Void, 0.0, None);
    w.queue_input("me", "Fade", Value::Void, 0.0, Some(Who::Player(p)));
    run_to(&mut w, 0);
    let fades: Vec<(Option<Entity>, ScreenFade)> = effects(&mut w)
        .into_iter()
        .filter_map(|e| match e {
            Effect::Hud {
                to,
                what: HudShow::Fade(f),
            } => Some((to, f)),
            _ => None,
        })
        .collect();
    assert_eq!(fades.len(), 2);
    let (to, all) = fades.iter().find(|(_, f)| f.alpha == 255).unwrap();
    assert_eq!(*to, None);
    assert!(all.purge && all.color == [10, 20, 30] && (all.duration - 153.0 / 512.0).abs() < 1e-6);
    let (to, mine) = fades.iter().find(|(_, f)| f.alpha == 200).unwrap();
    assert_eq!(*to, Some(p));
    assert!(!mine.purge);
}

// ----------------------------------------------- score, hint, explosion

#[test]
fn game_score_applies_to_the_activator() {
    let mut w = world();
    let p = player(&mut w);
    spawn(&mut w, &[("classname", "game_score"), ("targetname", "s"), ("points", "-10"), ("spawnflags", "1")]);
    spawn(&mut w, &[("classname", "game_score"), ("targetname", "zero"), ("points", "0")]);
    w.activate();
    w.queue_input("s", "ApplyScore", Value::Void, 0.0, Some(Who::Player(p)));
    w.queue_input("s", "ApplyScore", Value::Void, 0.0, None);
    w.queue_input("zero", "ApplyScore", Value::Void, 0.0, Some(Who::Player(p)));
    run_to(&mut w, 0);
    let scores: Vec<Effect> = effects(&mut w).into_iter().filter(|e| matches!(e, Effect::Score { .. })).collect();
    assert_eq!(
        scores,
        vec![Effect::Score {
            target: p,
            points: -10,
            team: false,
            allow_negative: true
        }]
    );
}

#[test]
fn hud_hint_to_the_activator_or_everyone() {
    let mut w = world();
    let p = player(&mut w);
    spawn(&mut w, &[("classname", "env_hudhint"), ("targetname", "h"), ("message", "Press %+use% to drive")]);
    spawn(&mut w, &[("classname", "env_hudhint"), ("targetname", "all"), ("spawnflags", "1"), ("message", "hi")]);
    w.activate();
    w.queue_input("h", "ShowHudHint", Value::Void, 0.0, Some(Who::Player(p)));
    w.queue_input("h", "ShowHudHint", Value::Void, 0.0, None);
    w.queue_input("all", "HideHudHint", Value::Void, 0.0, None);
    run_to(&mut w, 0);
    let hints: Vec<Effect> = effects(&mut w).into_iter().filter(|e| matches!(e, Effect::Hud { .. })).collect();
    assert_eq!(
        hints,
        vec![
            Effect::Hud {
                to: Some(p),
                what: HudShow::Hint("Press %+use% to drive".into())
            },
            Effect::Hud {
                to: None,
                what: HudShow::Hint(String::new())
            }
        ]
    );
}

#[test]
fn explosions_radius_scale_and_removal() {
    let mut w = world();
    let x = spawn(&mut w, &[("classname", "env_explosion"), ("targetname", "boom"), ("iMagnitude", "100")]);
    let big = spawn(&mut w, &[("classname", "env_explosion"), ("iMagnitude", "300"), ("spawnflags", "4096")]);
    let small = spawn(&mut w, &[("classname", "env_explosion"), ("iMagnitude", "60")]);
    let again = spawn(&mut w, &[("classname", "env_explosion"), ("targetname", "rep"), ("iMagnitude", "10"), ("spawnflags", "3")]);
    w.activate();
    let scale = |w: &LogicWorld, id| match &w.get(id).unwrap().class {
        Class::Explosion(e) => (e.radius, e.sprite_scale),
        _ => panic!(),
    };
    assert_eq!(scale(&w, x), (250.0, 30));
    assert_eq!(scale(&w, big).1, 150);
    assert_eq!(scale(&w, small).1, 10);
    w.queue_input("boom", "Explode", Value::Void, 0.0, None);
    w.queue_input("rep", "Explode", Value::Void, 0.0, None);
    w.queue_input("rep", "Explode", Value::Void, 0.0, None);
    run_to(&mut w, 0);
    let blasts: Vec<(f32, f32)> = effects(&mut w)
        .into_iter()
        .filter_map(|e| match e {
            Effect::Explosion { damage, radius, .. } => Some((damage, radius)),
            _ => None,
        })
        .collect();
    // "No Damage" (flag 1) still explodes, for nothing.
    assert_eq!(blasts, vec![(100.0, 250.0), (0.0, 25.0), (0.0, 25.0)]);
    // 0.3 s later the non-repeatable one is gone.
    run_to(&mut w, 21);
    assert!(w.get(x).is_none());
    assert!(w.get(again).is_some());
}

/// Spec 6 step 3: the spawnflags that change an explosion's look and sound
/// go with its effect; dm_steamlab's 50 (repeatable, no decal, no sparks) say
/// no decal.
#[test]
fn explosion_flags_reach_its_effect() {
    use crate::core::BlastFlags;
    let mut w = world();
    spawn(&mut w, &[("classname", "env_explosion"), ("targetname", "lab"), ("iMagnitude", "125"), ("spawnflags", "50")]);
    spawn(&mut w, &[("classname", "env_explosion"), ("targetname", "all"), ("iMagnitude", "125"), ("spawnflags", "836")]);
    w.activate();
    w.queue_input("lab", "Explode", Value::Void, 0.0, None);
    w.queue_input("all", "Explode", Value::Void, 0.0, None);
    run_to(&mut w, 0);
    let flags: Vec<BlastFlags> = effects(&mut w)
        .into_iter()
        .filter_map(|e| match e {
            Effect::Explosion { flags, .. } => Some(flags),
            _ => None,
        })
        .collect();
    assert_eq!(flags[0], BlastFlags(BlastFlags::NO_DECAL | BlastFlags::NO_SPARKS));
    assert_eq!(
        flags[1],
        BlastFlags(BlastFlags::NO_FIREBALL | BlastFlags::NO_SOUND | BlastFlags::NO_FIREBALL_SMOKE | BlastFlags::NO_PARTICLES)
    );
}

/// prop_damage.md (answered 2026-11-09): an env_explosion's damage is
/// blast alone, blast with blast-surface with "surface only" (8192), and
/// generic with "generic damage" (16384).
#[test]
fn explosion_damage_kind_follows_its_flags() {
    use crate::core::DamageKind;
    let mut w = world();
    for (name, flags) in [("plain", "0"), ("surface", "8192"), ("generic", "16384")] {
        spawn(&mut w, &[("classname", "env_explosion"), ("targetname", name), ("iMagnitude", "100"), ("spawnflags", flags)]);
    }
    w.activate();
    for name in ["plain", "surface", "generic"] {
        w.queue_input(name, "Explode", Value::Void, 0.0, None);
    }
    run_to(&mut w, 0);
    let kinds: Vec<DamageKind> = effects(&mut w)
        .into_iter()
        .filter_map(|e| match e {
            Effect::Explosion { kind, .. } => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, [DamageKind::Blast, DamageKind::BlastSurface, DamageKind::Generic]);
}

// ------------------------------------------------ wall toggle, conveyor

#[test]
fn wall_toggle_starts_invisible_and_toggles() {
    let mut w = world();
    let b = spawn_brush(
        &mut w,
        &[("classname", "func_wall_toggle"), ("targetname", "wall"), ("spawnflags", "1")],
        Vec3::splat(-4.0),
        Vec3::splat(4.0),
    );
    w.activate();
    assert!(w.mover_solid(b).is_none());
    assert!(!w.mover_poses()[0].4);
    w.queue_input("wall", "Toggle", Value::Void, 0.0, None);
    run_to(&mut w, 0);
    assert!(w.mover_solid(b).is_some());
    assert!(w.mover_poses()[0].4);
}

#[test]
fn conveyor_sets_base_velocity_on_riders() {
    let mut w = world();
    let belt = spawn_brush(
        &mut w,
        &[("classname", "func_conveyor"), ("targetname", "belt"), ("movedir", "0 90 0"), ("speed", "100")],
        Vec3::new(-64.0, -64.0, -8.0),
        Vec3::new(64.0, 64.0, 0.0),
    );
    w.activate();
    player(&mut w);
    w.players[0].ground = Some(belt);
    run_to(&mut w, 0);
    let v = w.players[0].base_velocity;
    assert!((v - Vec3::new(0.0, 100.0, 0.0)).length() < 1e-3, "{v}");
    assert!(w.players[0].base_touched);
    w.queue_input("belt", "ToggleDirection", Value::Void, 0.0, None);
    run_to(&mut w, 1);
    w.players[0].base_touched = false;
    run_to(&mut w, 2);
    let v = w.players[0].base_velocity;
    assert!((v - Vec3::new(0.0, -100.0, 0.0)).length() < 1e-3, "{v}");
}

// ------------------------------------------- spotlights, lasers, sparks

fn part_on(w: &LogicWorld, id: EntId) -> bool {
    match &w.get(id).unwrap().class {
        Class::Part(p) => p.on,
        _ => panic!("not a part"),
    }
}

#[test]
fn spotlight_switches_and_says_so() {
    let mut w = world();
    let unnamed = spawn(&mut w, &[("classname", "point_spotlight"), ("spawnflags", "1"), ("OnLightOn", "on,Trigger,,0,-1")]);
    let s = spawn(&mut w, &[("classname", "point_spotlight"), ("targetname", "s1"), ("OnLightOff", "off,Trigger,,0,-1")]);
    relay(&mut w, "on", &[]);
    relay(&mut w, "off", &[]);
    w.activate();
    run_to(&mut w, 0);
    assert!(part_on(&w, unnamed));
    assert_eq!(got(&w, w.find("on").unwrap(), "Trigger"), vec![0], "OnLightOn once");
    assert!(!part_on(&w, s), "named without Start On: off");
    w.queue_input("s1", "LightOn", Value::Void, 0.0, None);
    run_to(&mut w, 1);
    assert!(part_on(&w, s));
    w.queue_input("s1", "LightOff", Value::Void, 0.0, None);
    w.queue_input("s1", "LightOff", Value::Void, 0.0, None);
    run_to(&mut w, 2);
    assert!(!part_on(&w, s));
    assert_eq!(got(&w, w.find("off").unwrap(), "Trigger"), vec![2], "OnLightOff once");
}

#[test]
fn laser_hurts_every_seven_ticks() {
    let mut w = world();
    spawn(
        &mut w,
        &[
            ("classname", "env_laser"),
            ("texture", "sprites/laserbeam.vmt"),
            ("LaserTarget", "t"),
            ("damage", "100"),
            ("origin", "0 0 32"),
        ],
    );
    spawn(&mut w, &[("classname", "info_target"), ("targetname", "t"), ("origin", "500 0 32")]);
    player_at(&mut w, 1, Vec3::new(200.0, 0.0, 0.0));
    w.activate();
    let mut hits = Vec::new();
    while w.tick <= 66 {
        w.frame(&NoCollision);
        for e in std::mem::take(&mut w.effects) {
            if let Effect::Hurt { amount, armour, .. } = e {
                assert!(armour, "armour takes its part");
                hits.push((w.tick - 1, amount));
            }
        }
    }
    assert_eq!(hits.first().map(|h| h.0), Some(7), "first hit at tick 7");
    assert!(hits.iter().all(|(_, a)| (a - 10.5).abs() < 1e-3), "{hits:?}");
    assert_eq!(hits.len(), 9);
}

#[test]
fn named_laser_waits_for_turn_on() {
    let mut w = world();
    let l = spawn(&mut w, &[("classname", "env_laser"), ("targetname", "l"), ("texture", "x"), ("LaserTarget", "t")]);
    w.activate();
    assert!(!part_on(&w, l));
    w.queue_input("l", "Toggle", Value::Void, 0.0, None);
    run_to(&mut w, 0);
    assert!(part_on(&w, l));
}

#[test]
fn beam_touch_fires_once() {
    let mut w = world();
    spawn(
        &mut w,
        &[
            ("classname", "env_beam"),
            ("texture", "x"),
            ("LightningStart", "a"),
            ("LightningEnd", "b"),
            ("TouchType", "1"),
            ("OnTouchedByEntity", "zap,Trigger,,0,-1"),
        ],
    );
    spawn(&mut w, &[("classname", "info_target"), ("targetname", "a"), ("origin", "0 -100 32")]);
    spawn(&mut w, &[("classname", "info_target"), ("targetname", "b"), ("origin", "0 100 32")]);
    relay(&mut w, "zap", &[]);
    w.activate();
    run_to(&mut w, 5);
    assert!(got(&w, w.find("zap").unwrap(), "Trigger").is_empty());
    player_at(&mut w, 1, Vec3::ZERO);
    run_to(&mut w, 20);
    assert_eq!(got(&w, w.find("zap").unwrap(), "Trigger").len(), 1, "once, then it stops");
}

#[test]
fn spark_once_stops_sparking() {
    let mut w = world();
    let s = spawn(
        &mut w,
        &[
            ("classname", "env_spark"),
            ("targetname", "sp"),
            ("spawnflags", "64"),
            ("MaxDelay", "0"),
            ("OnSpark", "sparked,Trigger,,0,-1"),
        ],
    );
    relay(&mut w, "sparked", &[]);
    w.activate();
    run_to(&mut w, 200);
    let n = got(&w, w.find("sparked").unwrap(), "Trigger").len();
    assert!(n > 10, "sparks every 0.1 s: {n}");
    w.queue_input("sp", "SparkOnce", Value::Void, 0.0, None);
    run_to(&mut w, 201);
    let after = got(&w, w.find("sparked").unwrap(), "Trigger").len();
    run_to(&mut w, 300);
    assert_eq!(got(&w, w.find("sparked").unwrap(), "Trigger").len(), after);
    assert!(!part_on(&w, s));
}

// ------------------------------------------------------ env_beam strikes

fn strike_beam(w: &mut LogicWorld, extra: &[(&str, &str)]) -> EntId {
    let mut pairs = vec![
        ("classname", "env_beam"),
        ("texture", "x"),
        ("LightningStart", "a"),
        ("LightningEnd", "b"),
        ("life", "0.5"),
        ("StrikeTime", "1"),
        ("damage", "20"),
    ];
    pairs.extend_from_slice(extra);
    let id = spawn(w, &pairs);
    w.get_mut(id).unwrap().map_index = Some(9);
    spawn(w, &[("classname", "info_target"), ("targetname", "a"), ("origin", "0 -100 32")]);
    spawn(w, &[("classname", "info_target"), ("targetname", "b"), ("origin", "0 100 32")]);
    id
}

type Strikes = Vec<(i64, Vec3, Vec3, f32)>;

/// (tick, start, end, life) of each strike up to tick `until`, and the
/// damage dealt.
fn strikes(w: &mut LogicWorld, until: i64) -> (Strikes, f32) {
    let (mut out, mut damage) = (Vec::new(), 0.0);
    while w.tick <= until {
        w.frame(&NoCollision);
        for e in effects(w) {
            match e {
                Effect::BeamStrike {
                    entity: 9,
                    start,
                    end,
                    life,
                } => out.push((w.tick - 1, start, end, life)),
                Effect::Hurt { amount, .. } => damage += amount,
                _ => {}
            }
        }
    }
    (out, damage)
}

#[test]
fn strike_generator_strikes_after_its_life_and_wait() {
    // Spec test: life 0.5, StrikeTime 1, unnamed, damage 20, a player in
    // line: strikes at t = 1, 2.5, 4; 20 damage each.
    let mut w = world();
    strike_beam(&mut w, &[]);
    player_at(&mut w, 1, Vec3::ZERO);
    w.activate();
    let (s, damage) = strikes(&mut w, 280);
    let times: Vec<f32> = s.iter().map(|(t, ..)| *t as f32 * DT).collect();
    assert_eq!(times.len(), 3, "{times:?}");
    for (t, want) in times.iter().zip([1.0, 2.5, 4.0]) {
        assert!((t - want).abs() < 0.03, "{times:?}");
    }
    assert_eq!(s[0].1, Vec3::new(0.0, -100.0, 32.0));
    assert_eq!(s[0].2, Vec3::new(0.0, 100.0, 32.0));
    assert_eq!(s[0].3, 0.5);
    assert!((damage - 60.0).abs() < 1e-3, "{damage}");
}

#[test]
fn named_strike_generator_waits_for_turn_on_and_strike_once_needs_none() {
    let mut w = world();
    strike_beam(&mut w, &[("targetname", "beam"), ("spawnflags", "2"), ("damage", "0")]);
    w.activate();
    let (s, _) = strikes(&mut w, 200);
    assert!(s.is_empty(), "off until TurnOn");
    w.queue_input("beam", "StrikeOnce", Value::Void, 0.0, None);
    let (s, _) = strikes(&mut w, 202);
    assert_eq!(s.len(), 1, "StrikeOnce while off");
    w.queue_input("beam", "TurnOn", Value::Void, 0.0, None);
    let (s, _) = strikes(&mut w, 205);
    assert_eq!(s.len(), 1, "TurnOn strikes on the next tick");
    w.queue_input("beam", "TurnOff", Value::Void, 0.0, None);
    let (s, _) = strikes(&mut w, 400);
    assert!(s.is_empty(), "TurnOff stops the strikes");
}

#[test]
fn random_strike_waits_up_to_its_strike_time() {
    let mut w = world();
    strike_beam(&mut w, &[("spawnflags", "4"), ("damage", "0")]);
    w.activate();
    let (s, _) = strikes(&mut w, 2000);
    let gaps: Vec<f32> = s.windows(2).map(|p| (p[1].0 - p[0].0) as f32 * DT).collect();
    assert!(gaps.len() > 10, "{gaps:?}");
    assert!(gaps.iter().all(|g| (0.5 - 0.02..=1.5 + 0.02).contains(g)), "{gaps:?}");
    assert!(gaps.iter().any(|g| (g - gaps[0]).abs() > 0.05), "random: {gaps:?}");
}

#[test]
fn strike_without_an_end_runs_to_a_wall_within_its_radius() {
    // A closed box 200 across around the start: each strike runs from the
    // start to a wall.
    let mut w = world();
    let id = spawn(
        &mut w,
        &[
            ("classname", "env_beam"),
            ("texture", "x"),
            ("LightningStart", "a"),
            ("life", "0.2"),
            ("StrikeTime", "0"),
            ("Radius", "256"),
        ],
    );
    w.get_mut(id).unwrap().map_index = Some(9);
    spawn(&mut w, &[("classname", "info_target"), ("targetname", "a"), ("origin", "0 0 0")]);
    let slab = |lo: [f32; 3], hi: [f32; 3]| crate::map::MapBrush::from_box(Vec3::from(lo), Vec3::from(hi));
    w.collision = Some(std::sync::Arc::new(BrushCollision(vec![
        slab([100.0, -500.0, -500.0], [120.0, 500.0, 500.0]),
        slab([-120.0, -500.0, -500.0], [-100.0, 500.0, 500.0]),
        slab([-500.0, 100.0, -500.0], [500.0, 120.0, 500.0]),
        slab([-500.0, -120.0, -500.0], [500.0, -100.0, 500.0]),
        slab([-500.0, -500.0, 100.0], [500.0, 500.0, 120.0]),
        slab([-500.0, -500.0, -120.0], [500.0, 500.0, -100.0]),
    ])));
    w.activate();
    let (s, _) = strikes(&mut w, 400);
    assert!(s.len() > 5, "{}", s.len());
    for (_, start, end, _) in s {
        assert_eq!(start, Vec3::ZERO);
        let m = end.abs().max_element();
        assert!((m - 100.0).abs() < 0.5, "on a wall: {end}");
    }
}
