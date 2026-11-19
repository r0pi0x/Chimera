//! specs/combat_arms/effects_sounds.md, Follow-up (2026-11-14) 1 to 6:
//! the damage screen effects (`game::damage_fx`), cases 97 to 105 and
//! 107 to 110. The records are a fixture of ours in the attribute file's
//! syntax (`FIXTURE`), never the install's.

use bevy::prelude::*;
use chimera_core::core::{Dead, Health, LocalPlayer, SimClock, Team};
use chimera_sim::weapon::{Armor, ArmorPieces, hits};
use chimera_world::harness::Sim;
use combat_arms::game::{
    damage_fx::{self, CaDamageFxTable, CaDamageFxView, CaProgress, CaScreenFx, FxSound, ProgressPass},
    grenades::{CaGrenadeEffects, OverTime, OverTimeKind},
    movement,
    weapons::{BURN, POISON, class_of},
};

/// Our records: case 99's test record, burn, poison, glue, land trap and
/// freezing with the values item 6 gives, a record that leaves out the
/// movement and input keys, and one with a HUD picture and loop sound.
const FIXTURE: &str = r#"
[DamageFX0]
Name = "Test"
DamageType = 50
TintColor = <255, 128, 0>
TintRampUpTime = 0.5
TintOnTime = 1.0
TintRampDownTime = 0.5
TintOffTime = 1.0
FadeTime = 2.0
LoopSound = "test\loop.wav"
HudIcon = "test\icon.dtx"
LightScaleColor = <128, 128, 128>
LightScaleSpeed = 1.0
MinFXPercent = 0.25
AllowMovement = 1
AllowInput = 1

[DamageFX1]
Name = "Burning"
DamageType = 3
TintColor = <128, 32, 0>
TintRampUpTime = 0.3
TintOnTime = 0.7
TintRampDownTime = 0.3
TintOffTime = 0.7
FadeTime = 1.0
LightScaleColor = <212, 212, 212>
LightScaleSpeed = 0.3
MinFXPercent = 1.0
AllowMovement = 1
AllowInput = 1

[DamageFX2]
Name = "Poisoned"
DamageType = 9
TintColor = <128, 32, 128>
TintRampUpTime = 0.3
TintOnTime = 0.7
TintRampDownTime = 0.3
TintOffTime = 0.7
FadeTime = 1.0
LightScaleColor = <212, 212, 212>
LightScaleSpeed = 0.3
MinFXPercent = 0.1
AllowMovement = 1
AllowInput = 1
NumJumpsToEscape = 1

[DamageFX3]
Name = "Glue"
DamageType = 15
FadeTime = 1.0
LightScaleColor = <196, 196, 196>
LightScaleSpeed = 0.2
MinFXPercent = 1.0
AllowMovement = 0
AllowInput = 1
NumJumpsToEscape = 10

[DamageFX4]
Name = "LandTrap"
DamageType = 16
FadeTime = 1.0
MinFXPercent = 1.0
AllowMovement = 0
AllowInput = 1
NumJumpsToEscape = 15

[DamageFX5]
Name = "Freezing"
DamageType = 8
TintColor = <56, 89, 105>
TintRampUpTime = 0.5
TintOnTime = 0.8
TintRampDownTime = 0.5
TintOffTime = 1.5
FadeTime = 0.5
LightScaleColor = <120, 120, 120>
LightScaleSpeed = 0.3
RotationMax = 0.005
RotationSpeed = 0.5
RotationRandomRange = <0.9, 1.0, 0.0>
MinFXPercent = 1.0
AllowMovement = 1
AllowInput = 1

[DamageFX6]
Name = "Silent"
DamageType = 51
"#;

const TEST: usize = 0;
const BURNING: usize = 1;
const POISONED: usize = 2;
const GLUE: usize = 3;
const LAND_TRAP: usize = 4;
const FREEZING: usize = 5;

fn table() -> CaDamageFxTable {
    CaDamageFxTable::parse(FIXTURE).expect("the fixture parses")
}

fn close3(a: Vec3, b: Vec3, tol: f32, what: &str) {
    assert!((a - b).abs().max_element() <= tol, "{what}: {a} vs {b}");
}

/// Step `view` by `dt` until `t` seconds have passed.
fn run(view: &mut CaDamageFxView, t: &CaDamageFxTable, seconds: f32, dt: f32) {
    let n = (seconds / dt).round() as usize;
    for _ in 0..n {
        view.step(t, dt, false);
    }
}

fn running(view: &CaDamageFxView, record: usize) -> &damage_fx::Running {
    view.running.iter().find(|r| r.record == record).expect("running")
}

/// Item 2: the keys read, `AllowMovement` and `AllowInput` 0 (blocking)
/// when left out, the range's default.
#[test]
fn item2_records_read() {
    let t = table();
    assert_eq!(t.0.len(), 7);
    let b = &t.0[BURNING];
    assert_eq!((b.name.as_str(), b.damage_type), ("Burning", BURN));
    close3(b.tint, Vec3::new(128.0, 32.0, 0.0) / 255.0, 1e-6, "tint /255");
    assert_eq!(b.tint_times, [0.3, 0.7, 0.3, 0.7]);
    assert!(b.allow_movement && b.allow_input);
    assert_eq!(t.0[POISONED].jumps_to_escape, 1);
    let silent = &t.0[6];
    assert!(!silent.allow_movement && !silent.allow_input, "left out: blocked");
    assert_eq!(silent.roll_range, Vec2::new(0.5, 1.0));
    assert_eq!(t.0[FREEZING].roll_range, Vec2::new(0.9, 1.0));
}

/// 99. The tint cycle.
#[test]
fn c99_tint_cycle() {
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, TEST);
    let c = Vec3::new(1.0, 128.0 / 255.0, 0.0);
    let mut at = 0.0;
    for (time, want) in [
        (0.25, c * 0.5),
        (1.0, c),
        (1.75, c * 0.5),
        (2.5, Vec3::ZERO),
        (3.25, c * 0.5),
    ] {
        run(&mut v, &t, time - at, 0.05);
        at = time;
        close3(running(&v, TEST).tint, want, 1e-4, &format!("at {time}"));
    }
}

/// 100. The fade: stopped at 1.0 s, half 1 s later, none after 2 s; the
/// loop sound stops at the stop.
#[test]
fn c100_fade() {
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, TEST);
    assert!(v.sounds.contains(&FxSound::Loop {
        record: TEST,
        file: "test\\loop.wav".into()
    }));
    run(&mut v, &t, 1.0, 0.05);
    v.sounds.clear();
    v.stop(&t, TEST);
    assert_eq!(
        v.sounds,
        vec![FxSound::StopLoop { record: TEST }],
        "the loop ends at the stop"
    );
    let c = Vec3::new(1.0, 128.0 / 255.0, 0.0);
    run(&mut v, &t, 1.0, 0.05);
    close3(running(&v, TEST).tint, c * 0.5, 1e-4, "1 s into the fade");
    run(&mut v, &t, 1.0, 0.05);
    assert!(v.running.is_empty(), "gone after 2 s");
    assert_eq!(v.screen(&t, 0.0), CaScreenFx::default());
}

/// 101. Two records' tints by maximum; a flash's on top, clamped.
#[test]
fn c101_two_records() {
    let burn = Vec3::new(0.4, 0.2, 0.0);
    let poison = Vec3::new(0.0, 0.3, 0.0);
    close3(
        damage_fx::combine_tint(&[burn, poison]),
        Vec3::new(0.4, 0.3, 0.0),
        0.0,
        "damage slot",
    );
    let slot = damage_fx::combine_tint(&[burn, poison]);
    close3(
        damage_fx::combine_tint(&[slot, Vec3::ONE]),
        Vec3::ONE,
        0.0,
        "with a flash",
    );
    // Through the view: burning and poisoned together, on phase.
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, BURNING);
    v.start(&t, POISONED);
    run(&mut v, &t, 0.5, 0.05);
    let s = v.screen(&t, 0.0);
    close3(s.tint, Vec3::new(128.0, 32.0, 128.0) / 255.0, 1e-4, "max per channel");
}

/// 102. Darkening: 0.75 after 0.25 s, the colour floor 0.502 from
/// 0.498 s.
#[test]
fn c102_darkening() {
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, TEST);
    run(&mut v, &t, 0.25, 0.05);
    close3(v.screen(&t, 0.0).light_scale, Vec3::splat(0.75), 1e-4, "0.25 s");
    run(&mut v, &t, 0.25, 0.05);
    close3(
        v.screen(&t, 0.0).light_scale,
        Vec3::splat(128.0 / 255.0),
        1e-4,
        "the floor",
    );
}

/// 103. The strength factor.
#[test]
fn c103_strength() {
    assert!((damage_fx::strength(0.25, 0.0) - 0.25).abs() < 1e-6);
    assert!((damage_fx::strength(0.25, 0.5) - 0.625).abs() < 1e-6);
    assert!((damage_fx::strength(0.25, 1.0) - 1.0).abs() < 1e-6);
}

/// 105. The HUD picture: none in a mode whose HUD has no `DamageFX`
/// control (every one we have: TDM); with one (Fireteam) it covers the
/// screen, its alpha rising by 255 / `FadeTime` per second.
#[test]
fn c105_hud_picture_by_mode() {
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, TEST);
    run(&mut v, &t, 0.5, 0.05);
    assert_eq!(v.screen(&t, 0.0).hud, None, "TDM");
    v.hud_layout = true;
    let (icon, alpha) = v.screen(&t, 0.0).hud.expect("Fireteam");
    assert_eq!(icon, "test\\icon.dtx");
    assert!((alpha - 255.0 / 2.0 * 0.5).abs() < 1e-3, "{alpha}");
}

/// 107. Stock burn (item 6's values): the tint cycle and the darkening;
/// no sound, no picture, movement free.
#[test]
fn c107_stock_burn() {
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, BURNING);
    assert!(v.sounds.is_empty(), "no sound");
    let c = Vec3::new(128.0, 32.0, 0.0) / 255.0;
    let mut at = 0.0;
    for (time, want, light) in [
        (0.15, c * 0.5, None),
        (0.3, c, Some(0.91)),
        (0.6, c, Some(212.0 / 255.0)),
        (1.0, c, None),
        (1.3, Vec3::ZERO, None),
        (2.0, Vec3::ZERO, None),
        (2.15, c * 0.5, None),
    ] {
        run(&mut v, &t, time - at, 0.01);
        at = time;
        let s = v.screen(&t, 0.0);
        close3(s.tint, want, 2e-3, &format!("tint at {time}"));
        if let Some(l) = light {
            close3(s.light_scale, Vec3::splat(l), 2e-3, &format!("light at {time}"));
        }
        assert!(!s.move_locked && !s.input_locked && s.hud.is_none());
    }
}

/// 109. Glue: movement blocked, the scene darkening at 0.2/s to its
/// floor; ten jump presses free the player. The land trap: fifteen, no
/// tint.
#[test]
fn c109_glue_and_trap() {
    let t = table();
    for (record, presses) in [(GLUE, 10), (LAND_TRAP, 15)] {
        let mut v = CaDamageFxView::default();
        v.start(&t, record);
        v.step(&t, 0.05, false);
        assert!(v.screen(&t, 0.0).move_locked);
        for i in 1..=presses {
            v.step(&t, 0.05, true);
            v.step(&t, 0.05, true);
            v.step(&t, 0.05, false);
            assert_eq!(v.is_running(record), i < presses, "press {i}");
        }
        assert_eq!(v.escaped, vec![t.0[record].damage_type]);
        assert!(v.screen(&t, 0.0).move_locked, "still locked while it fades");
        assert_eq!(v.screen(&t, 0.0).tint, Vec3::ZERO);
    }
    let mut v = CaDamageFxView::default();
    v.start(&t, GLUE);
    run(&mut v, &t, 1.0, 0.05);
    close3(v.screen(&t, 0.0).light_scale, Vec3::splat(0.8), 1e-4, "0.2/s");
    run(&mut v, &t, 1.0, 0.05);
    close3(
        v.screen(&t, 0.0).light_scale,
        Vec3::splat(196.0 / 255.0),
        1e-4,
        "the floor",
    );
}

/// 110. Freezing: the roll swings to 0.005 U(0.9, 1.0) rad at 0.5 rad/s,
/// starting downwards; the tint cycle 0.5 / 0.8 / 0.5 / 1.5 s; fade 0.5 s.
#[test]
fn c110_freezing_wobble() {
    let t = table();
    let mut v = CaDamageFxView::default();
    v.start(&t, FREEZING);
    let dt = 0.001;
    v.step(&t, dt, false);
    let mut low: f32 = 0.0;
    let mut high: f32 = 0.0;
    let mut prev = 0.0;
    let mut max_rate: f32 = 0.0;
    for _ in 0..200 {
        v.step(&t, dt, false);
        let r = v.screen(&t, 0.0).roll;
        max_rate = max_rate.max((r - prev).abs() / dt);
        prev = r;
        low = low.min(r);
        high = high.max(r);
    }
    assert!(low < 0.0, "downwards first");
    assert!((-0.005..=-0.0045).contains(&low), "{low}");
    assert!((0.0045..=0.005).contains(&high), "{high}");
    assert!(max_rate <= 0.5 + 1e-3, "{max_rate}");
    let r = &t.0[FREEZING];
    assert_eq!((r.tint_times, r.fade), ([0.5, 0.8, 0.5, 1.5], 0.5));
}

// ---------------------------------------------------------------------------
// On the simulation: the server's pass, death, escape

fn sim() -> Sim {
    let mut sim = crate::common::sim();
    sim.app.insert_resource(table());
    sim
}

/// The local player, Alpha, 100 HP.
fn local_player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert((
        Health { current: 1.0, max: 1.0 },
        Team(1),
        LocalPlayer,
        combat_arms::game::weapons::gear_armour(0.0, &Vec::new()),
    ));
    sim.ticks(1);
    p
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

fn next_pass(sim: &Sim) -> f64 {
    sim.app.world().resource::<ProgressPass>().next.expect("passes run")
}

fn progress(sim: &Sim, p: Entity) -> CaProgress {
    sim.app.world().get::<CaProgress>(p).copied().unwrap_or_default()
}

/// An over-time entry of `kind` on `p` for `seconds`, dealing almost
/// nothing.
fn entry(sim: &mut Sim, p: Entity, kind: OverTimeKind, seconds: f32) {
    sim.app
        .world_mut()
        .resource_mut::<CaGrenadeEffects>()
        .over_time
        .push(OverTime {
            victim: p,
            source: None,
            kind,
            rate: 1e-6,
            remaining: seconds,
            attacker: None,
            weapon: "test",
        });
}

/// Tick until the clock reaches `t` (s).
fn until(sim: &mut Sim, t: f64) {
    while now(sim) + 1e-9 < t {
        sim.ticks(1);
    }
}

/// 97. A 5 s burn entry 0.1 s after a pass: active from the next pass,
/// through the pass 4.9 s after it was given, gone at the one after.
#[test]
fn c97_progressive_flags() {
    let mut sim = sim();
    let p = local_player(&mut sim);
    let pass = next_pass(&sim);
    until(&mut sim, pass + 0.1);
    entry(&mut sim, p, OverTimeKind::Burn, 5.0);
    assert!(!progress(&sim, p).is_active(BURN));
    until(&mut sim, pass + 0.5);
    assert!(progress(&sim, p).is_active(BURN), "after the next pass");
    assert!(progress(&sim, p).dealing & damage_fx::bit(BURN) != 0, "dealing");
    until(&mut sim, pass + 5.0);
    assert!(progress(&sim, p).is_active(BURN), "the pass at 5.0");
    until(&mut sim, pass + 5.5);
    assert!(!progress(&sim, p).is_active(BURN), "the pass at 5.5");
    // The local player's records followed.
    let v = sim.app.world().resource::<CaDamageFxView>();
    assert!(!v.is_running(BURNING));
}

/// 98. Gear with burn protection 1.0 hides the burn.
#[test]
fn c98_gear_hides_burn() {
    let mut sim = sim();
    let p = local_player(&mut sim);
    let cover = hits::Coverage {
        class: Some(class_of(BURN)),
        head: 0.0,
        torso: 1.0,
        arm: 0.0,
        leg: 0.0,
        groin: 0.0,
        wear: 0.0,
        except: 0,
    };
    sim.app.world_mut().entity_mut(p).insert((
        Armor {
            amount: 0.0,
            helmet: false,
        },
        ArmorPieces {
            pieces: vec![hits::ArmorPiece { entries: vec![cover] }],
            depletion: Default::default(),
            armour_quantum: None,
        },
    ));
    entry(&mut sim, p, OverTimeKind::Burn, 5.0);
    for _ in 0..60 {
        sim.ticks(1);
        assert!(!progress(&sim, p).is_active(BURN));
    }
    assert!(!sim.app.world().resource::<CaDamageFxView>().is_running(BURNING));
}

/// 104 (and 97's start): burning, the local player dies → the tint, loop
/// and picture gone that tick, no fade.
#[test]
fn c104_death_clears() {
    let mut sim = sim();
    let p = local_player(&mut sim);
    entry(&mut sim, p, OverTimeKind::Burn, 30.0);
    sim.seconds(1.0);
    assert!(sim.app.world().resource::<CaDamageFxView>().is_running(BURNING));
    assert!(
        sim.app.world().resource::<CaScreenFx>().tint != Vec3::ZERO
            || sim.app.world().resource::<CaScreenFx>().light_scale != Vec3::ONE
    );
    let at = now(&sim);
    sim.app.world_mut().entity_mut(p).insert(Dead { since: at });
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.0;
    sim.ticks(1);
    assert!(
        sim.app.world().resource::<CaDamageFxView>().running.is_empty(),
        "no fade"
    );
    assert_eq!(*sim.app.world().resource::<CaScreenFx>(), CaScreenFx::default());
}

/// 108. Poisoned: one jump press stops the effect (it fades over 1 s)
/// and the server clears the poison entries.
#[test]
fn c108_poison_one_jump() {
    let mut sim = sim();
    let p = local_player(&mut sim);
    entry(&mut sim, p, OverTimeKind::Poison, 30.0);
    entry(&mut sim, p, OverTimeKind::Burn, 30.0);
    sim.seconds(1.0);
    assert!(sim.app.world().resource::<CaDamageFxView>().is_running(POISONED));
    sim.intent(p).jump = true;
    sim.ticks(1);
    sim.intent(p).jump = false;
    sim.ticks(1);
    let v = sim.app.world().resource::<CaDamageFxView>();
    assert!(!v.is_running(POISONED), "stopped");
    assert!(
        v.running.iter().any(|r| r.record == POISONED && r.fading.is_some()),
        "fading"
    );
    let left: Vec<OverTimeKind> = sim
        .app
        .world()
        .resource::<CaGrenadeEffects>()
        .over_time
        .iter()
        .filter(|o| o.victim == p)
        .map(|o| o.kind)
        .collect();
    assert_eq!(
        left,
        vec![OverTimeKind::Burn],
        "the poison entries cleared, the burn kept"
    );
    sim.seconds(1.1);
    assert!(!progress(&sim, p).is_active(POISON));
    let v = sim.app.world().resource::<CaDamageFxView>();
    assert!(v.running.iter().all(|r| r.record != POISONED), "faded out");
    assert!(v.is_running(BURNING));
}

/// 109 on the simulation: glued, the local player's movement keys do
/// nothing.
#[test]
fn c109_glue_blocks_movement() {
    let mut sim = sim();
    let p = local_player(&mut sim);
    let t = table();
    sim.app.world_mut().resource_mut::<CaDamageFxView>().start(&t, GLUE);
    sim.ticks(2);
    let from = sim.app.world().get::<Transform>(p).unwrap().translation;
    for _ in 0..30 {
        sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
        sim.ticks(1);
    }
    let to = sim.app.world().get::<Transform>(p).unwrap().translation;
    assert!((to - from).length() < 0.01, "moved {}", (to - from).length());
}

/// Item 4, the spread: a poison (or stun) record's type active and
/// dealing forces mu S to 1; active alone, or no record of the type, not.
#[test]
fn item4_poison_forces_the_spread() {
    let t = table();
    let p = |active: u64, dealing: u64| CaProgress { active, dealing };
    let poison = damage_fx::bit(POISON);
    assert!(damage_fx::forces_spread(&t, &p(poison, poison)));
    assert!(!damage_fx::forces_spread(&t, &p(poison, 0)), "not dealing");
    let burn = damage_fx::bit(BURN);
    assert!(!damage_fx::forces_spread(&t, &p(burn, burn)), "burn: no");
    assert!(
        !damage_fx::forces_spread(&CaDamageFxTable::default(), &p(poison, poison)),
        "no record"
    );
}
