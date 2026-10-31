//! specs/combat_arms/effects_sounds.md's test cases (`t01_` is test case
//! 1) that need no install: the selection, attenuation and effect rules
//! (`game::effects::rules`, the tables of data/effects.toml and the
//! weapons' `combat_arms:effects` hooks), and the effects in play on the
//! greybox (sounds as `CaSound` messages, decals, tracers, casings,
//! effect requests). The numbers the install's effect file holds (lights,
//! stains, the smoke cloud: cases 21, 24, 32, 34, 35) are checked against
//! it in heavy::effects_sounds.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, LocalPlayer};
use chimera_sim::weapon::{self, Inventory, WeaponEvent, WeaponEventKind};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{MapData, MapMesh, sound::SurfaceGrid},
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        attachments::{CaMods, CaModTable, Part},
        data::CaData,
        effects::{
            self, CaFxRequest, CaFxSettings, CaShell, CaTracers, FxKind,
            data::{AmmoKind, CaFxTables, CaWeaponFx, EFFECTS},
            fx::FxJitter,
            rules::{
                self, CaMarks, Casing, CasingHit, CasingStep, DingQueue, Foot, Gait, HurtKind, HurtLimiter, Mark,
                MarkAdded, OwnSteps, StepInputs, TracerCounter, TracerPath,
            },
            sounds::CaSound,
        },
        inventory::UNIT,
        movement,
    },
    harness::CaSim,
};

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a}, expected {b}");
}

fn t() -> &'static CaFxTables {
    CaFxTables::built_in()
}

/// Combat Arms' built-in weapons, as the scenario tests play them.
fn weapon_set() -> &'static chimera_sim::weapon::data::WeaponSet {
    CaData::builtin_static().weapon_set()
}

/// A weapon's effects hook.
fn wfx(id: &str) -> CaWeaponFx {
    weapon_set().get(id).unwrap().hook(EFFECTS).unwrap()
}

/// A weapon's fitted parts: `names` by their slots, with the weapon's
/// sound radii.
fn mods(names: &[&str], fire: i32, silenced: i32) -> CaMods {
    let table = CaModTable::built_in();
    let mut m = CaMods {
        fire_sound_radius: fire,
        silenced_radius: vec![silenced; 2],
        ..Default::default()
    };
    for n in names {
        let id = table.find(n).unwrap_or_else(|| panic!("no part {n}"));
        let record = table.get(id).unwrap().clone();
        let slot = record.slot().unwrap();
        m.parts[slot] = Some(Part {
            id,
            variant: Some(0),
            record,
            default: false,
        });
    }
    m
}

/// g(d) with the default inner radius.
fn g(d: f32, big_r: f32) -> f32 {
    rules::gain(d, rules::inner_radius(big_r), big_r, 100.0)
}

// ---------------------------------------------------------------------------
// 10. Gain and low-pass

#[test]
fn t01_gain_m4a1_shot() {
    close(rules::inner_radius(2000.0), 140.0, 1e-3, "r");
    for (d, want) in [(100.0, 1.0), (500.0, 0.9334), (1000.0, 0.7166), (1500.0, 0.3553), (1800.0, 0.0691), (1900.0, 0.0)] {
        close(g(d, 2000.0), want, 1e-4, &format!("d = {d}"));
    }
    close(rules::zero_point(140.0, 2000.0), 1865.26, 0.01, "zero point");
    // The others' fire sound of a stock M4A1: FireSnd at FireSndRadius.
    let m4 = wfx("combat_arms:m4a1");
    let s = effects::fire_sound(&m4, Some(&mods(&[], 2000, 1400)), false).unwrap();
    assert_eq!(s, ("guns_snd_ar/m4a1/fire.wav", 2000.0));
}

#[test]
fn t02_gain_silenced_m4a1() {
    let m4 = wfx("combat_arms:m4a1");
    let m = mods(&["Silencer_A_AR"], 2000, 1400);
    let (file, big_r) = effects::fire_sound(&m4, Some(&m), false).unwrap();
    assert_eq!((file, big_r), ("guns_snd_option/ar/silenced1.wav", 1400.0));
    close(rules::inner_radius(1400.0), 98.0, 1e-3, "r");
    for (d, want) in [(500.0, 0.8582), (1000.0, 0.4158), (1300.0, 0.0087)] {
        close(g(d, 1400.0), want, 1e-4, &format!("d = {d}"));
    }
    close(rules::zero_point(98.0, 1400.0), 1305.68, 0.01, "zero point");
    // Your own shot plays the same file, local.
    assert_eq!(effects::fire_sound(&m4, Some(&m), true).unwrap().0, file);
}

#[test]
fn t03_t04_footstep_gain_and_ghillie() {
    let metal = t().surface(20);
    assert_eq!(metal.right, ["chars_snd/footsteps/metal1a.wav", "chars_snd/footsteps/metal2a.wav"]);
    assert_eq!(rules::draw_slot(&metal.right, 2, 0.2), Some("chars_snd/footsteps/metal1a.wav"));
    assert_eq!(rules::draw_slot(&metal.right, 2, 0.7), Some("chars_snd/footsteps/metal2a.wav"));
    close(rules::inner_radius(rules::FOOTSTEP_RADIUS), 84.0, 1e-3, "r");
    for (d, want) in [(500.0, 0.8049), (1000.0, 0.2027), (1150.0, 0.0)] {
        close(g(d, 1200.0), want, 1e-4, &format!("d = {d}"));
    }
    // 4: the ghillie's volume 50.
    close(rules::gain(500.0, 84.0, 1200.0, 50.0), 0.4025, 1e-4, "ghillie");
}

#[test]
fn t05_server_hurt_sound() {
    let (r, big_r) = rules::server_radii(rules::inner_radius(500.0), 500.0);
    close(r, 17.0 / 255.0 * 500.0, 1e-4, "quantised r");
    for (d, want) in [(100.0, 0.9592), (300.0, 0.5918), (460.0, 0.0335)] {
        close(rules::gain(d, r, big_r, 100.0), want, 1e-4, &format!("d = {d}"));
    }
    assert!(!rules::server_sends(1001.0, 500.0), "beyond 2R: not sent");
    assert!(rules::server_sends(999.0, 500.0));
}

#[test]
fn t06_bullet_impact_on_metal() {
    let record = t().impact("BulletFmj").unwrap();
    let metal = t().surface(20);
    let files: Vec<&str> = (0..4)
        .map(|i| effects::impact_sound(record, metal, AmmoKind::Bullet, (i as f32 + 0.5) / 4.0).unwrap())
        .collect();
    assert_eq!(files, (1..=4).map(|i| format!("guns_snd_impacts/metal_bg/impact{i}.wav")).collect::<Vec<_>>());
    assert_eq!(record.sound_radius, 900.0);
    for (d, want) in [(200.0, 0.9486), (500.0, 0.6488), (800.0, 0.0921)] {
        close(g(d, 900.0), want, 1e-4, &format!("d = {d}"));
    }
}

#[test]
fn t07_frag_detonation_gain() {
    let record = t().impact("handgrenade").unwrap();
    assert_eq!((record.sound.as_str(), record.sound_radius), ("guns_snd_impacts/explosions/gren.wav", 1500.0));
    for (d, want) in [(500.0, 0.8772), (1000.0, 0.4918), (1400.0, 0.0)] {
        close(g(d, 1500.0), want, 1e-4, &format!("d = {d}"));
    }
}

#[test]
fn t08_low_pass() {
    for (d, want) in [(1000.0, 1.0), (1200.0, 1.0), (1350.0, 0.6173), (1500.0, 0.2929), (1650.0, 0.1), (1800.0, 0.1)] {
        close(rules::low_pass(d, 140.0, 2000.0), want, 1e-4, &format!("d = {d}"));
    }
    // Only while d < R.
    assert_eq!(rules::low_pass(2100.0, 140.0, 2000.0), 1.0);
    // Chimera's playback reads the same from the level.
    let level = rules::sound_level(2000.0, UNIT);
    let metres = 1350.0 * UNIT;
    close(chimera_world::map::sound::distance_cutoff(level, metres), 0.6173, 1e-3, "playback");
    close(chimera_world::map::sound::distance_gain(level, metres), g(1350.0, 2000.0), 1e-4, "gain");
}

#[test]
fn t09_impact_silence() {
    let record = t().impact("BulletFmj").unwrap();
    let mut rolls = effects::Rolls::default();
    let silent = |id: u16, rolls: &mut effects::Rolls| {
        (0..10_000)
            .filter(|_| effects::impact_sound(record, t().surface(id), AmmoKind::Bullet, rolls.next()).is_none())
            .count()
    };
    let unknown = silent(0, &mut rolls);
    assert!((2300..2700).contains(&unknown), "{unknown}");
    let snow = silent(71, &mut rolls);
    assert!((4800..5200).contains(&snow), "{snow}");
    assert_eq!(silent(20, &mut rolls), 0);
}

// ---------------------------------------------------------------------------
// 4. Tracers

fn cadence(f: u32, shots: u32) -> Vec<u32> {
    let mut c = TracerCounter::default();
    (1..=shots).filter(|_| c.ray(f)).collect()
}

#[test]
fn t10_tracer_cadence() {
    let m4 = wfx("combat_arms:m4a1");
    let f = effects::own_tracer_frequency(t(), &m4, Some(&mods(&[], 2000, 1400))).unwrap();
    assert_eq!(f, 4);
    assert_eq!(cadence(f, 8), [4, 8]);
    let look = effects::tracer_look(t(), &m4, None).unwrap();
    assert_eq!((look.width, look.length, look.velocity), (15.0, 600.0, 6000.0));
    // AK-47 (AR01) and M24 (SR01, F = 1).
    assert_eq!(cadence(t().tracer("AR01").unwrap().frequency, 9), [3, 6, 9]);
    assert_eq!(cadence(t().tracer("SR01").unwrap().frequency, 4), [1, 2, 3, 4]);
    assert_eq!(cadence(0, 3), [1, 2, 3], "F = 0: every shot");
}

#[test]
fn t11_tracer_timing() {
    let p = TracerPath::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1500.0), 6000.0, 600.0).unwrap();
    close(p.life(), 0.25, 1e-6, "life");
    let (tail, head) = p.ends(0.1).unwrap();
    close(tail, 600.0, 1e-3, "tail at 0.1");
    close(head, 1200.0, 1e-3, "head at 0.1");
    let (tail, head) = p.ends(0.2).unwrap();
    close(tail, 1200.0, 1e-3, "tail at 0.2");
    close(head, 1500.0, 1e-3, "head clamped");
    assert!(p.ends(0.26).is_none());
    assert!(TracerPath::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -90.0), 6000.0, 600.0).is_none(), "90 u: none");
}

#[test]
fn t12_tracer_with_a_dot_sight() {
    let m4 = wfx("combat_arms:m4a1");
    let m = mods(&["DotSight_A_AR"], 2000, 1400);
    let own = effects::own_tracer_frequency(t(), &m4, Some(&m)).unwrap();
    assert_eq!(cadence(own, 9), [3, 6, 9], "the sight's AR01 cadence");
    assert_eq!(effects::tracer_look(t(), &m4, Some(&m)).unwrap(), t().tracer("AR02").unwrap(), "the ammo's look");
    // Others: the server's counter with the ammo's F.
    assert_eq!(cadence(t().tracer(&m4.tracer).unwrap().frequency, 8), [4, 8]);
}

#[test]
fn t13_tracer_with_an_a_silencer() {
    let m4 = wfx("combat_arms:m4a1");
    let m = mods(&["Silencer_A_AR"], 2000, 1400);
    assert_eq!(cadence(effects::own_tracer_frequency(t(), &m4, Some(&m)).unwrap(), 9), [3, 6, 9]);
    let look = effects::tracer_look(t(), &m4, Some(&m)).unwrap();
    assert_eq!(look, t().tracer("AR01").unwrap());
    assert_eq!((look.width, look.length), (25.0, 300.0));
    assert_eq!(cadence(t().tracer(&m4.tracer).unwrap().frequency, 8), [4, 8], "others: the ammo's F");
}

#[test]
fn t14_pistol_with_silencer() {
    let pistol = wfx("combat_arms:beretta_m92fs");
    let m = mods(&["Silencer_A_Pistol"], 1600, 1200);
    assert!(effects::tracer_look(t(), &pistol, Some(&m)).is_none(), "pistol ammo has no tracer");
    assert_eq!(effects::own_tracer_frequency(t(), &pistol, Some(&m)), Some(0));
}

// ---------------------------------------------------------------------------
// 6. Decals

fn settings() -> CaFxSettings {
    CaFxSettings::default()
}

fn hit(id: u16, u: f32) -> effects::Impact {
    effects::impact(
        t().impact("BulletFmj").unwrap(),
        t().surface(id),
        AmmoKind::Bullet,
        true,
        true,
        false,
        &settings(),
        [0.0, u],
    )
}

#[test]
fn t15_decal_size() {
    let w = |id: u16, u: f32| hit(id, u).mark.unwrap().1;
    let metal = hit(20, 0.0).mark.unwrap();
    assert_eq!(metal.0, "fx/i_mark/im_metal.spr");
    close(w(20, 0.0), 2.88, 1e-4, "metal min");
    close(w(20, 1.0), 3.36, 1e-4, "metal max");
    close(w(30, 0.0), 3.84, 1e-4, "wood min");
    close(w(30, 1.0), 4.32, 1e-4, "wood max");
    close(w(11, 0.0), 2.88, 1e-4, "cement min");
    close(w(11, 1.0), 3.84, 1e-4, "cement max");
    close(w(40, 0.0), 2.88, 1e-4, "glass min");
    close(w(40, 1.0), 3.36, 1e-4, "glass max");
    assert!(hit(40, 0.0).burst.is_none(), "glass: no burst");
    assert_eq!(hit(20, 0.0).burst.as_deref(), Some("Impact_Metal"));
}

#[test]
fn t16_decal_life() {
    assert_eq!(rules::mark_alpha(2.9), Some(1.0));
    close(rules::mark_alpha(4.5).unwrap(), 0.5, 1e-6, "4.5 s");
    assert_eq!(rules::mark_alpha(6.01), None);
}

fn mark(at: Vec3, age: f32) -> Mark {
    Mark {
        at,
        normal: Vec3::Y,
        angle: 0.0,
        width: 3.0,
        sprite: String::new(),
        grey: 0.5,
        age,
        serial: 0,
    }
}

#[test]
fn t17_decal_crowding() {
    let mut m = CaMarks::default();
    for i in 0..10 {
        m.add(mark(Vec3::new(i as f32 * 0.5, 0.0, 0.0), i as f32 * 0.1));
    }
    assert_eq!(m.add(mark(Vec3::ZERO, 0.0)), MarkAdded::Added);
    assert_eq!(m.marks.len(), 11);
    let target = Vec3::new(4.4, 0.0, 0.0);
    match m.add(mark(target, 0.0)) {
        MarkAdded::Moved(i) => {
            assert_eq!(m.marks[i].at, target);
            // The closest one (at 4.5 u), with its old age.
            close(m.marks[i].age, 0.9, 1e-6, "kept age");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(m.marks.len(), 11);
}

#[test]
fn t18_decal_cap() {
    let mut m = CaMarks::default();
    for i in 0..101 {
        m.add(mark(Vec3::new(i as f32 * 100.0, 0.0, 0.0), 0.0));
    }
    let first = m.marks[0].at;
    assert_eq!(m.add(mark(Vec3::new(0.0, 500.0, 0.0), 0.0)), MarkAdded::Replaced);
    assert_eq!(m.marks.len(), 101);
    assert!(m.marks.iter().all(|x| x.at != first), "the oldest went");
}

#[test]
fn t19_no_decal_on_doors() {
    let door = hit(100, 0.5);
    assert!(door.mark.is_none(), "ShowsMark 0");
    assert_eq!(door.burst.as_deref(), Some("Impact_Wood"));
    assert!(door.sound.unwrap().0.starts_with("guns_snd_impacts/wood/"));
}

#[test]
fn t20_no_decal_on_characters() {
    let body = effects::impact(
        t().impact("BulletFmj").unwrap(),
        t().surface(6),
        AmmoKind::Bullet,
        true,
        false,
        false,
        &settings(),
        [0.1, 0.5],
    );
    assert!(body.mark.is_none());
    assert_eq!(body.burst.as_deref(), Some("Impact_Body"));
    let (file, r) = body.sound.unwrap();
    assert!(file.starts_with("guns_snd_impacts/cloth/"), "{file}");
    assert_eq!(r, 900.0);
    // Two blobs splatting 50 % each: 1.0 expected.
    let mut rolls = effects::Rolls::default();
    let n = 20_000;
    let total: usize = (0..n).map(|_| rules::splats(&[50.0, 50.0], &[rolls.next(), rolls.next()])).sum();
    close(total as f32 / n as f32, 1.0, 0.03, "expected stains");
    // A head hit's group (case 21; its light and stains: heavy).
    let head = effects::impact(
        t().impact("BulletFmj").unwrap(),
        t().surface(4),
        AmmoKind::Bullet,
        true,
        false,
        false,
        &settings(),
        [0.1, 0.5],
    );
    assert_eq!(head.burst.as_deref(), Some("Impact_Head"));
}

// ---------------------------------------------------------------------------
// 2, 3. Muzzle flashes

#[test]
fn t22_others_muzzle_flash() {
    let cam = Vec3::ZERO;
    let fwd = Vec3::NEG_Z;
    let ahead = rules::gates(cam, fwd, Vec3::new(0.0, 0.0, -500.0), Vec3::new(0.0, 0.0, -10.0), true);
    assert!(ahead.fire_seen && !ahead.fire_far);
    let m4 = wfx("combat_arms:m4a1");
    assert_eq!(effects::hh_muzzle(t(), &m4, None), "M4_HH_Muzz");
    let behind = rules::gates(cam, fwd, Vec3::new(0.0, 0.0, 500.0), Vec3::new(0.0, 0.0, 600.0), true);
    assert!(!behind.fire_seen, "no flash behind you");
    close(g(500.0, 2000.0), 0.9334, 1e-4, "its sound");
    // A point at the camera counts as straight up.
    assert!(!rules::seen(cam, fwd, cam));
    assert!(rules::seen(cam, Vec3::Y, cam));
}

#[test]
fn t23_silencer_flash() {
    let m4 = wfx("combat_arms:m4a1");
    let c = mods(&["Silencer_C_AR"], 2000, 1400);
    let a = mods(&["Silencer_A_AR"], 2000, 1400);
    assert_eq!(effects::hh_muzzle(t(), &m4, Some(&c)), "Silenced01_HH_Muzz", "a group the file lacks: nothing");
    assert_eq!(effects::pv_muzzle(t(), "AK47_pv_muzz", &[], Some(&c)), "Silenced01_PV_Muzz");
    assert_eq!(effects::hh_muzzle(t(), &m4, Some(&a)), "WEAPON_HH");
    assert_eq!(effects::hh_muzzle(t(), &m4, Some(&mods(&["Silencer_A_AR_Red"], 2000, 1400))), "Silenced01_HH_Muzz");
    for m in [&c, &a] {
        assert_eq!(
            effects::fire_sound(&m4, Some(m), false).unwrap(),
            ("guns_snd_option/ar/silenced1.wav", 1400.0)
        );
    }
}

#[test]
fn t24_own_muzzle() {
    let params: combat_arms::game::view_model::CaViewParams =
        weapon_set().get("combat_arms:m4a1").unwrap().hook(combat_arms::game::view_model::VIEW).unwrap();
    assert_eq!(effects::pv_muzzle(t(), &params.muzzle_fx, &[], None), "AK47_pv_muzz");
    assert_eq!(rules::FLASH_TIME, 0.05);
    assert_eq!(t().fire(&wfx("combat_arms:m4a1").fire).unwrap().fx, "test", "the smoke puff's group");
}

// ---------------------------------------------------------------------------
// 8. Casings

/// A floor at height 0 (u), metal.
fn floor(from: Vec3, to: Vec3) -> Option<CasingHit> {
    (from.y > 0.0 && to.y <= 0.0).then(|| {
        let f = from.y / (from.y - to.y);
        CasingHit {
            at: from.lerp(to, f),
            normal: Vec3::Y,
            removes: false,
        }
    })
}

#[test]
fn t25_casing() {
    let v = rules::casing_velocity([0.0, 0.0, 0.0], false);
    assert_eq!(v, Vec3::new(50.0, 30.0, 20.0));
    let v = rules::casing_velocity([1.0, 1.0, 1.0], false);
    assert_eq!(v, Vec3::new(75.0, 75.0, 50.0));
    assert_eq!(rules::casing_velocity([0.5, 0.5, 0.5], true).x, -62.5, "left hand: to the left");
    let fire = t().fire("5.56mm").unwrap();
    assert!(fire.shells && fire.shell_model.ends_with("5.56mm.ltb"));
    let mut c = Casing {
        at: Vec3::new(0.0, 60.0, 0.0),
        velocity: Vec3::new(70.0, 50.0, 30.0),
        spin: Vec2::new(5.0, 5.0),
        angles: Vec2::ZERO,
        orientation: Quat::IDENTITY,
        bounces: 0,
        age: 0.0,
        resting: false,
        in_water: false,
        first_person: false,
    };
    let dt = 1.0 / 60.0;
    let mut steps = Vec::new();
    while c.age < 3.0 {
        if c.age >= 0.5 - 1e-4 && c.age < 0.5 + dt {
            close(c.scale(), 2.0, 0.05, "grown to 2x at 0.5 s");
        }
        let s = c.step(dt, Vec3::ZERO, floor);
        steps.push(s);
        if s == CasingStep::Removed {
            break;
        }
    }
    assert_eq!(steps.iter().filter(|s| **s == CasingStep::FirstBounce).count(), 1, "one sound");
    assert_eq!(*steps.last().unwrap(), CasingStep::Removed);
    close(c.age, 3.0, dt, "removed at 3 s");
    // Its sound: the surface's, R 200.
    let metal = t().surface(20);
    assert_eq!((metal.shell.as_str(), metal.shell_radius), ("guns_snd_shells/metal.wav", 200.0));
    // Seen from 1100 u: no casing.
    let far = rules::gates(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, -1100.0), Vec3::ZERO, true);
    assert!(far.fire_seen && far.fire_far);
    let near = rules::gates(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, -300.0), Vec3::ZERO, true);
    assert!(near.fire_seen && !near.fire_far);
}

#[test]
fn t26_casing_rest() {
    let down = Vec3::new(0.0, -100.0, 0.0);
    assert!(rules::rests(down, 1.0 / 60.0));
    assert!(rules::rests(down, 1.0 / 144.0));
    close((225.0f32 * 144.0).sqrt(), 180.0, 1e-3, "144 fps threshold");
    close((225.0f32 * 30.0).sqrt(), 82.16, 0.01, "30 fps threshold");
    assert!(!rules::rests(down, 1.0 / 30.0));
    // At 30 fps the casing bounces, at most three times.
    let mut c = Casing {
        at: Vec3::new(0.0, 1.0, 0.0),
        velocity: down,
        spin: Vec2::ZERO,
        angles: Vec2::ZERO,
        orientation: Quat::IDENTITY,
        bounces: 0,
        age: 0.0,
        resting: false,
        in_water: false,
        first_person: false,
    };
    assert_eq!(c.step(1.0 / 30.0, Vec3::ZERO, floor), CasingStep::FirstBounce);
    assert!(!c.resting);
    for _ in 0..200 {
        c.step(1.0 / 30.0, Vec3::ZERO, floor);
    }
    assert!(c.bounces <= 3);
    // At 60 fps it rests at once.
    let mut c2 = Casing {
        at: Vec3::new(0.0, 1.0, 0.0),
        velocity: down,
        ..c.clone()
    };
    c2.bounces = 0;
    c2.resting = false;
    c2.age = 0.0;
    c2.step(1.0 / 60.0, Vec3::ZERO, floor);
    assert!(c2.resting);
}

// ---------------------------------------------------------------------------
// 11. Fly-by, dings, hurt, footsteps

#[test]
fn t27_flyby() {
    let cam = Vec3::ZERO;
    // A line 400 u to the side, its impact 2000 u away.
    let from = Vec3::new(400.0, 0.0, 1000.0);
    let to = Vec3::new(400.0, 0.0, -1960.0);
    let near = rules::flyby(cam, from, to, 900.0).expect("fly-by");
    close(near.distance(cam), 400.0, 1e-3, "nearest point");
    let to_near = Vec3::new(400.0, 0.0, -450.0);
    assert!(rules::flyby(cam, from, to_near, 900.0).is_none(), "impact 600 u away: inside 0.75 x 900");
    // Quirk 6: the original's position near the origin.
    let q = rules::flyby_quirk_position(Vec3::new(5000.0, 0.0, 0.0), Vec3::new(5000.0, 0.0, 300.0));
    close(q.length(), 300.0 + 0.0, 1.01, "about 300 u from the origin");
}

#[test]
fn t28_hit_confirm_queue() {
    let mut q = DingQueue::default();
    let queued = (0..8).filter(|_| q.push()).count();
    assert_eq!(queued, 5);
    assert_eq!(q.drain(), 5);
    assert_eq!(q.drain(), 0);
    assert_eq!(t().sounds.ding, ["guns_snd_impacts/multiding1.wav", "guns_snd_impacts/multiding2.wav"]);
}

#[test]
fn t29_hurt_limit() {
    let mut l = HurtLimiter::default();
    let kinds = effects::hurt_kinds(DamageKind::Bullet, Hitgroup::Head, false);
    assert_eq!(kinds, [Some(HurtKind::Guns), Some(HurtKind::Head)]);
    assert!(l.try_start(HurtKind::Guns, 0.0, 1.0));
    assert!(!l.try_start(HurtKind::Guns, 0.0, 1.0), "the second player's: dropped");
    assert!(l.try_start(HurtKind::Head, 0.0, 1.0), "a separate kind");
    assert!(l.try_start(HurtKind::Guns, 1.0, 1.0), "free again once it ended");
    assert!(effects::hurt_files(t(), HurtKind::Guns)[0].contains("hurtpain_a_male"));
    assert_eq!(effects::hurt_files(t(), HurtKind::Head), ["chars_snd/male/headshot.wav"]);
    assert_eq!(effects::hurt_kinds(DamageKind::Bullet, Hitgroup::Chest, false), [Some(HurtKind::Guns), None]);
    assert_eq!(effects::hurt_kinds(DamageKind::Blast, Hitgroup::Stomach, false)[1], Some(HurtKind::Groin));
}

#[test]
fn t30_others_footsteps() {
    // Running: 50 and 350 ms of each 600 ms loop.
    assert_eq!(rules::footsteps_between(Gait::Run, 0.0, 0.049), 0);
    assert_eq!(rules::footsteps_between(Gait::Run, 0.0, 0.05), 1);
    assert_eq!(rules::footsteps_between(Gait::Run, 0.05, 0.35), 1);
    assert_eq!(rules::footsteps_between(Gait::Run, 0.0, 0.6), 2);
    assert_eq!(rules::footsteps_between(Gait::Run, 0.0, 6.0), 20, "3.33 steps a second");
    assert_eq!(rules::footsteps_between(Gait::Walk, 0.0, 6.0), 0);
    assert_eq!(rules::footsteps_between(Gait::Crouch, 0.0, 6.0), 0);
}

#[test]
fn t31_own_footsteps() {
    let dt = 1.0 / 600.0;
    let run = StepInputs {
        dt,
        moving: true,
        can_step: true,
        run: true,
        jumped: false,
        landed: false,
        alive: true,
    };
    let mut s = OwnSteps::default();
    let mut times = Vec::new();
    for i in 0..1200 {
        for f in s.step(&run) {
            times.push((i as f32 * dt, f));
        }
    }
    assert_eq!(times[0], (0.0, Foot::Right), "one step at once");
    let gaps: Vec<f32> = times[1..].windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert!(gaps.iter().all(|g| (g - 0.357).abs() < 0.003), "{gaps:?}");
    close(1.0 / (2.0 * rules::STEP_PACE_RUN), 0.357, 0.001, "2.8 steps/s");
    // Holding the walk key: none.
    let mut s = OwnSteps::default();
    let walk = StepInputs { run: false, ..run };
    assert!((0..1200).all(|_| s.step(&walk).is_empty()));
}

// ---------------------------------------------------------------------------
// 9. Grenades

#[test]
fn t32_frag_on_stone() {
    let j = FxJitter {
        at: Vec3::ZERO,
        age: 0.0,
        life: 0.6,
        scale: vec![(0.0, 60.0), (1.0, 0.0)],
        inner: 200.0,
        outer: 800.0,
    };
    close(j.amplitude(500.0), 39.0, 1e-3, "60 x 0.65");
    assert_eq!(j.amplitude(800.0), 0.0);
    assert_eq!(j.amplitude(150.0), 60.0);
    let frag = t().impact("handgrenade").unwrap();
    let on = |id: u16, seen: bool| {
        effects::impact(frag, t().surface(id), AmmoKind::Thrown, seen, true, false, &settings(), [0.0, 0.0])
    };
    let stone = on(10, true);
    assert_eq!(stone.burst.as_deref(), Some("Grenade_Stone"));
    assert_eq!(stone.sound.as_ref().unwrap().0, "guns_snd_impacts/explosions/gren.wav");
    assert!(stone.mark.is_none(), "Mark empty: no scorch");
    // Air has no grenade group, but a thrown grenade never has Air's
    // surface: mid-air it is its own Metal's (Corrections 2026-10-22, 1;
    // case 58).
    let air = on(1, true);
    assert!(air.burst.is_none() && air.sound.is_some());
    assert_eq!(on(effects::GRENADE_OWN_SURFACE, true).burst.as_deref(), Some("Grenade_Stone"));
    assert_eq!(on(0, true).burst.as_deref(), Some("Grenade_Dirt"));
}

#[test]
fn t33_frag_behind_you() {
    let gates = rules::gates(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, 500.0), Vec3::new(0.0, 0.0, 500.0), true);
    assert!(!gates.impact_seen);
    let out = effects::impact(
        t().impact("handgrenade").unwrap(),
        t().surface(10),
        AmmoKind::Thrown,
        gates.impact_seen,
        true,
        false,
        &settings(),
        [0.0, 0.0],
    );
    assert!(out.burst.is_none(), "no explosion look");
    assert!(out.sound.is_some());
    close(g(500.0, 1500.0), 0.8772, 1e-4, "its sound");
}

#[test]
fn t34_smoke_cloud_drift() {
    // Friction 0.98 per second: v x 0.02^dt; a particle travels at most
    // v0 / ln 50.
    let (mut x, mut v) = (0.0f32, 1000.0f32);
    let dt = 1.0 / 1000.0;
    for _ in 0..20_000 {
        x += v * dt;
        v *= rules::friction_factor(0.98, dt);
    }
    close(x, 1000.0 / 50f32.ln(), 0.5, "horizontal travel");
    close(1.0 / 50f32.ln(), 0.2556, 1e-4, "factor");
    // Smoke and flash: their own groups always (area ammo).
    let smoke = t().impact("Smoke_Grenade").unwrap();
    let unseen = effects::impact(smoke, t().surface(1), AmmoKind::Thrown, false, true, false, &settings(), [0.0; 2]);
    assert_eq!(unseen.ammo_fx.as_deref(), Some("Smoke_Grenade_01"));
    let flash = t().impact("flashbang_exp").unwrap();
    let unseen = effects::impact(flash, t().surface(1), AmmoKind::Thrown, false, true, false, &settings(), [0.0; 2]);
    assert_eq!(unseen.ammo_fx.as_deref(), Some("Flashbang_Ini"));
    assert_eq!(unseen.sound.unwrap(), ("guns_snd_impacts/explosions/flash.wav".into(), 1500.0));
}

#[test]
fn t36_dry_fire_heard_far() {
    let m4 = wfx("combat_arms:m4a1");
    assert_eq!(m4.dry_fire_sound, "guns_snd_ar/m4a1/dryfire.wav");
    // Others hear it with the fire sound's radius (Quirk 13).
    close(g(1500.0, 2000.0), 0.3553, 1e-4, "gain");
}

#[test]
fn t37_reload_keys() {
    let m4 = wfx("combat_arms:m4a1");
    assert_eq!(m4.key_sound(1), Some(("guns_snd_ar/m4a1/reload1.wav", 300.0)));
    assert_eq!(m4.key_sound(2), Some(("guns_snd_ar/m4a1/reload2.wav", 300.0)));
    assert_eq!(m4.key_sound(3), Some(("guns_snd_ar/m4a1/reload3.wav", 300.0)), "also the select's key 3");
    assert_eq!(m4.key_sound(4), Some(("guns_snd_ar/m4a1/select.wav", 300.0)));
    assert_eq!(m4.key_sound(5), Some(("guns_snd_ar/m4a1/deselect.wav", 300.0)));
    close(rules::zero_point(21.0, 300.0), 280.0, 0.5, "heard to about 280 u");
}

#[test]
fn t38_grenade_bounce_sounds() {
    assert!(t().surface(0).grenade.is_empty(), "unknown: silent");
    let cement = t().surface(11);
    assert_eq!(
        (cement.grenade.as_str(), cement.grenade_radius),
        ("guns_snd_impacts/stone/impactgrenade.wav", 1000.0)
    );
}

#[test]
fn t39_melee_impact() {
    let melee = t().impact("Melee").unwrap();
    assert_eq!(melee.sound_radius, 153.0);
    let wood = t().surface(30);
    let mut rolls = effects::Rolls::default();
    let mut silent = 0;
    for _ in 0..10_000 {
        match effects::impact_sound(melee, wood, AmmoKind::Melee, rolls.next()) {
            None => silent += 1,
            Some(f) => assert!(f == "guns_snd_impacts/melee/wood/impact1.wav" || f == "guns_snd_impacts/melee/wood/impact2.wav"),
        }
    }
    assert!((4800..5200).contains(&silent), "{silent}");
    let knife = wfx("combat_arms:m9_knife");
    assert_eq!((knife.ammo_kind, knife.impact.as_str()), (AmmoKind::Melee, "Melee"));
    let hit = effects::impact(melee, wood, AmmoKind::Melee, true, true, false, &settings(), [0.9, 0.0]);
    assert!(hit.mark.is_none() && hit.burst.is_none(), "no decal, no surface effects");
}

// ---------------------------------------------------------------------------
// In play on the greybox

/// Where characters' feet stand on the greybox (m).
const GREYBOX_FLOOR: f32 = 0.0;

#[derive(Resource, Default)]
struct Heard(Vec<CaSound>);

#[derive(Resource, Default)]
struct Requested(Vec<CaFxRequest>);

fn sim() -> Sim {
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.init_resource::<Heard>().init_resource::<Requested>();
    sim.app.add_systems(
        Update,
        (
            |mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| h.0.extend(r.read().cloned()),
            |mut r: MessageReader<CaFxRequest>, mut h: ResMut<Requested>| h.0.extend(r.read().cloned()),
        )
            .after(effects::CaEffectsSet),
    );
    sim
}

fn player(sim: &mut Sim, at: Vec3, yaw: f32) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert(Health { current: 1.0, max: 1.0 });
    p
}

fn arm(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).unwrap();
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    w
}

fn heard(sim: &mut Sim) -> Vec<CaSound> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0)
}

fn fire_once(sim: &mut Sim, p: Entity) {
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    // The round on the 3rd update after the press (weapon_firing.md
    // Corrections 10), then one for its effects.
    sim.ticks(4);
}

/// The floor typed `ty` (the greybox has no surface types): a plane at
/// the greybox floor's height.
fn type_everything(sim: &mut Sim, ty: u16) {
    name_everything(sim, &ty.to_string());
}

/// The floor named `name`, as another game's map names its surfaces.
fn name_everything(sim: &mut Sim, name: &str) {
    let world = sim.app.world_mut();
    let y = GREYBOX_FLOOR;
    let data = MapData {
        meshes: vec![MapMesh {
            positions: vec![[-500.0, y, -500.0], [-500.0, y, 500.0], [500.0, y, 500.0], [500.0, y, -500.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            surface: Some(name.to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    world.insert_resource(SurfaceGrid::new(&data));
}

#[test]
fn shots_sound_local_for_you_and_3d_for_others() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(24.0, 1.0, 20.0), 0.0);
    arm(&mut sim, me, "combat_arms:m4a1");
    let gun = arm(&mut sim, other, "combat_arms:m4a1");
    heard(&mut sim);
    fire_once(&mut sim, me);
    let mine = heard(&mut sim);
    assert!(mine.iter().any(|s| s.file == "guns_snd_ar/m4a1/fire.wav" && s.at.is_none()), "{mine:?}");
    fire_once(&mut sim, other);
    let theirs = heard(&mut sim);
    let fire = theirs
        .iter()
        .find(|s| s.file == "guns_snd_ar/m4a1/fire.wav")
        .expect("their fire sound");
    assert!(fire.at.is_some());
    assert_eq!(fire.radii, (140.0, 2000.0));
    // The fire voice is the shooter's weapon's (one per attacker and weapon).
    assert_eq!((fire.source, fire.channel), (Some(gun), Some(effects::sounds::CHANNEL_FIRE)));
}

/// A round whose gun is gone when its effects are drawn (the gun game
/// swaps the gun on the kill's tick: playtest 2026-10-28) still sounds as
/// that gun: your own (the event names the despawned gun) and others' on a
/// network client (a `RemoteShot` naming a gun no longer in their hands).
#[test]
fn a_round_whose_gun_was_swapped_still_sounds() {
    use chimera_sim::{minigame::runtime::take_weapon, weapon::{SpreadShape, remote::RemoteShot}};
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(24.0, 1.0, 20.0), 0.0);
    let mine = arm(&mut sim, me, "combat_arms:m4a1");
    arm(&mut sim, other, "combat_arms:m4a1");
    heard(&mut sim);
    let fired = |owner, weapon| WeaponEvent {
        owner,
        weapon,
        kind: WeaponEventKind::Fired {
            id: "combat_arms:m4a1",
            origin: Vec3::new(20.0, 1.6, 20.0),
            yaw: 0.0,
            pitch: 0.0,
            seed: 0,
            spread: SpreadShape::Template(0.0),
            mode: 0,
        },
        replay: false,
    };
    // Yours: the round's event, then the gun despawned in the same tick.
    let w = sim.app.world_mut();
    w.write_message(fired(me, mine));
    take_weapon(w, me, mine);
    weapon::give(w, me, "combat_arms:mp5a4").unwrap();
    sim.ticks(1);
    let s = heard(&mut sim);
    assert!(s.iter().any(|s| s.file == "guns_snd_ar/m4a1/fire.wav" && s.at.is_none()), "{s:?}");
    // Others': their gun swapped before their round arrived.
    let theirs = sim.app.world().get::<Inventory>(other).unwrap().active.unwrap();
    let w = sim.app.world_mut();
    take_weapon(w, other, theirs);
    weapon::give(w, other, "combat_arms:mp5a4").unwrap();
    sim.ticks(1);
    heard(&mut sim);
    sim.app.world_mut().write_message(RemoteShot {
        shooter: other,
        weapon: "combat_arms:m4a1",
        origin: Vec3::new(24.0, 1.6, 20.0),
        yaw: 0.0,
        pitch: 0.0,
        seed: 0,
        spread: SpreadShape::Template(0.0),
        mode: 0,
    });
    sim.ticks(1);
    let s = heard(&mut sim);
    let fire = s.iter().find(|s| s.file == "guns_snd_ar/m4a1/fire.wav").unwrap_or_else(|| panic!("{s:?}"));
    assert_eq!(fire.source, Some(other));
    assert!(!s.iter().any(|s| s.file.contains("mp5") && s.file.contains("fire")), "not the new gun's: {s:?}");
}

#[test]
fn impacts_make_decals_bursts_and_sounds() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, "combat_arms:m4a1");
    type_everything(&mut sim, 20);
    // Look down at the floor ahead.
    sim.intent(me).pitch = -0.6;
    sim.ticks(2);
    heard(&mut sim);
    sim.app.world_mut().resource_mut::<Requested>().0.clear();
    fire_once(&mut sim, me);
    let sounds = heard(&mut sim);
    assert!(
        sounds.iter().any(|s| s.file.starts_with("guns_snd_impacts/metal_bg/impact") && s.radii.1 == 900.0),
        "{sounds:?}"
    );
    let marks = sim.app.world().resource::<CaMarks>();
    assert_eq!(marks.marks.len(), 1);
    assert_eq!(marks.marks[0].sprite, "fx/i_mark/im_metal.spr");
    let req = std::mem::take(&mut sim.app.world_mut().resource_mut::<Requested>().0);
    assert!(req.iter().any(|r| r.group == "Impact_Metal" && r.kind == FxKind::Surface), "{req:?}");
    assert!(req.iter().any(|r| r.group == "test" && r.kind == FxKind::OwnFire));
    // Decals fade and go after 6 s.
    sim.seconds(6.5);
    assert!(sim.app.world().resource::<CaMarks>().marks.is_empty());
}

#[test]
fn a_surface_of_another_games_map_without_a_type_still_leaves_one_bullet_hole() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, "combat_arms:m4a1");
    // Not one of Combat Arms' types or names: its class gives the type.
    name_everything(&mut sim, "concrete");
    sim.intent(me).pitch = -0.6;
    sim.ticks(2);
    fire_once(&mut sim, me);
    let marks = sim.app.world().resource::<CaMarks>();
    assert_eq!(marks.marks.len(), 1);
    assert_eq!(marks.marks[0].sprite, "fx/test/imark_ceramic.spr");
}

#[test]
fn tracers_every_fourth_m4a1_round_and_casings() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, "combat_arms:m4a1");
    // Level, along the floor: impacts beyond 100 u.
    sim.intent(me).pitch = 0.0;
    let mut drawn = 0;
    let mut casings_seen = 0;
    for _ in 0..8 {
        let before = sim.app.world().resource::<CaTracers>().0.len();
        fire_once(&mut sim, me);
        let after = sim.app.world().resource::<CaTracers>().0.len();
        drawn += after.saturating_sub(before).min(1);
        casings_seen = casings_seen.max(sim.app.world_mut().query::<&CaShell>().iter(sim.app.world()).count());
        sim.seconds(0.4);
    }
    assert_eq!(drawn, 2, "shots 4 and 8");
    assert!(casings_seen >= 1);
    sim.seconds(3.5);
    assert_eq!(sim.app.world_mut().query::<&CaShell>().iter(sim.app.world()).count(), 0, "gone after 3 s");
}

#[test]
fn hits_ding_on_the_next_frame() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let target = player(&mut sim, Vec3::new(30.0, 1.0, 20.0), 0.0);
    let w = arm(&mut sim, me, "combat_arms:m4a1");
    heard(&mut sim);
    let hit = |n| {
        (0..n)
            .map(|_| WeaponEvent {
                owner: me,
                weapon: w,
                kind: WeaponEventKind::Hit {
                    target,
                    amount: 0.1,
                    hitgroup: Hitgroup::Chest,
                },
                replay: false,
            })
            .collect::<Vec<_>>()
    };
    sim.app.world_mut().write_message_batch(hit(8));
    sim.app.update();
    let now: Vec<CaSound> = heard(&mut sim);
    assert!(now.iter().all(|s| !s.file.contains("multiding")), "not the same frame");
    sim.app.update();
    let dings = heard(&mut sim).into_iter().filter(|s| s.file.contains("multiding")).count();
    assert_eq!(dings, 5, "five slots");
}

#[test]
fn hurt_sounds_one_per_kind_across_the_server() {
    let mut sim = sim();
    let a = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    let b = player(&mut sim, Vec3::new(26.0, 1.0, 20.0), 0.0);
    heard(&mut sim);
    let hurt = |target, hitgroup| Damage {
        target,
        attacker: None,
        amount: 0.1,
        point: Vec3::new(20.0, 1.5, 20.0),
        dir: Vec3::Z,
        hitgroup,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    };
    sim.app.world_mut().write_message_batch([hurt(a, Hitgroup::Chest), hurt(b, Hitgroup::Head)]);
    sim.app.update();
    let s = heard(&mut sim);
    let pains = s.iter().filter(|s| s.file.contains("hurtpain_a")).count();
    assert_eq!(pains, 1, "{s:?}");
    assert!(s.iter().any(|s| s.file.ends_with("headshot.wav")), "a separate kind");
    assert!(s.iter().all(|s| s.server && s.radii.1 == 500.0));
}

#[test]
fn footsteps_by_surface_and_stance() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(28.0, 1.0, 20.0), 0.0);
    type_everything(&mut sim, 20);
    heard(&mut sim);
    sim.intent(me).move_axis = Vec2::Y;
    sim.intent(other).move_axis = Vec2::Y;
    sim.seconds(1.5);
    let s = heard(&mut sim);
    let mine = s.iter().filter(|s| s.at.is_none() && s.file.contains("footsteps/metal")).count();
    let theirs = s.iter().filter(|s| s.at.is_some() && s.file.contains("footsteps/metal")).count();
    assert!(mine >= 3, "{s:?}");
    assert!(theirs >= 3, "{s:?}");
    assert!(s.iter().filter(|s| s.at.is_some()).all(|s| s.radii.1 == 1200.0));
    // Walking: silent for everyone.
    sim.intent(me).walk = true;
    sim.intent(other).walk = true;
    sim.seconds(0.3);
    heard(&mut sim);
    sim.seconds(1.5);
    assert!(heard(&mut sim).iter().all(|s| !s.file.contains("footsteps")));
}

#[test]
fn reload_keys_play_the_weapons_sounds() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, "combat_arms:m4a1");
    heard(&mut sim);
    sim.app.world_mut().write_message(combat_arms::game::view_model::CaViewKey {
        owner: me,
        key: combat_arms::game::view_model::machine::ViewKey::Sound(2),
    });
    sim.app.update();
    let s = heard(&mut sim);
    assert!(s.iter().any(|s| s.file == "guns_snd_ar/m4a1/reload2.wav" && s.at.is_none()), "{s:?}");
}

// ---------------------------------------------------------------------------
// Liquids (sections 5, 8, 11; Per-frame order 6)

/// A liquid volume over the whole greybox.
fn flood(sim: &mut Sim) {
    sim.app.world_mut().spawn(combat_arms::game::volumes::CaVolume::liquid_box(
        Vec3::splat(-1000.0),
        Vec3::splat(1000.0),
        0.0,
        Vec3::ZERO,
    ));
}

#[test]
fn under_water_impacts_are_half_volume_without_bursts_or_tracers() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, "combat_arms:m4a1");
    type_everything(&mut sim, 20);
    flood(&mut sim);
    // Level: eight rounds, shots 4 and 8 would carry tracers.
    sim.intent(me).pitch = 0.0;
    for _ in 0..8 {
        fire_once(&mut sim, me);
        assert!(sim.app.world().resource::<CaTracers>().0.is_empty(), "no tracer into liquid");
        sim.seconds(0.4);
    }
    // Down at the metal floor: its sound at half volume, no burst (Metal
    // has no under-water group), the decal still made.
    sim.intent(me).pitch = -0.6;
    sim.ticks(2);
    heard(&mut sim);
    sim.app.world_mut().resource_mut::<Requested>().0.clear();
    sim.app.world_mut().resource_mut::<CaMarks>().marks.clear();
    fire_once(&mut sim, me);
    let sounds = heard(&mut sim);
    let hit = sounds
        .iter()
        .find(|s| s.file.starts_with("guns_snd_impacts/metal_bg/impact"))
        .expect("the impact sound");
    assert_eq!(hit.volume, 50.0);
    let fire = sounds.iter().find(|s| s.file == "guns_snd_ar/m4a1/fire.wav").unwrap();
    assert_eq!(fire.volume, 100.0, "only impacts are muffled");
    let req = std::mem::take(&mut sim.app.world_mut().resource_mut::<Requested>().0);
    assert!(!req.iter().any(|r| r.kind == FxKind::Surface), "{req:?}");
    assert_eq!(sim.app.world().resource::<CaMarks>().marks.len(), 1);
    // The casings fall as in water.
    let wet: Vec<bool> = sim
        .app
        .world_mut()
        .query::<&CaShell>()
        .iter(sim.app.world())
        .map(|s| s.casing.in_water)
        .collect();
    assert!(!wet.is_empty() && wet.iter().all(|w| *w));
}

#[test]
fn submerge_swaps_the_burst_for_the_under_water_group() {
    let record = t().impact("BulletFmj").unwrap();
    let metal = t().surface(20);
    let mut out = effects::impact(record, metal, AmmoKind::Bullet, true, true, false, &settings(), [0.0, 0.0]);
    assert!(out.burst.is_some());
    effects::submerge(&mut out, record, metal, AmmoKind::Bullet, true);
    assert_eq!((out.burst.clone(), out.under_water, out.sound_volume()), (None, true, 50.0));
    let mut custom = metal.clone();
    custom.uw_impact_fx = "UW_Test".into();
    let mut out = effects::impact(record, &custom, AmmoKind::Bullet, true, true, false, &settings(), [0.0, 0.0]);
    effects::submerge(&mut out, record, &custom, AmmoKind::Bullet, true);
    assert_eq!(out.burst.as_deref(), Some("UW_Test"));
}

#[test]
fn footsteps_on_an_unknown_surface_in_water_are_waters() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    type_everything(&mut sim, 0);
    flood(&mut sim);
    heard(&mut sim);
    sim.intent(me).move_axis = Vec2::Y;
    sim.seconds(1.5);
    let s = heard(&mut sim);
    let steps: Vec<&CaSound> = s.iter().filter(|s| s.file.contains("footsteps")).collect();
    assert!(steps.len() >= 3, "{s:?}");
    assert!(steps.iter().all(|s| s.file.starts_with("chars_snd/footsteps/water")), "{steps:?}");
}

#[test]
fn grenade_bounce_volume() {
    assert_eq!(effects::bounce_volume(false, 1.0), 100.0);
    assert_eq!(effects::bounce_volume(true, 1.0), 50.0, "half in water");
    assert_eq!(effects::bounce_volume(true, 0.5), 25.0);
    assert_eq!(effects::bounce_volume(false, 3.0), 100.0);
}

/// view_models.md case 43: a muzzle part with `FlashHiderLevel` 2 on a
/// weapon with `PVFlashHiderFXName2` = G takes G; without that key no
/// first-person flash at all.
#[test]
fn view_models_43_flash_hider() {
    let mut c = mods(&["Silencer_C_AR"], 2000, 1400);
    let slot = c.parts.iter().position(Option::is_some).unwrap();
    c.parts[slot].as_mut().unwrap().record.flash_hider_level = 2;
    let hider = vec![String::new(), String::new(), "G".to_string()];
    assert_eq!(effects::pv_muzzle(t(), "AK47_pv_muzz", &hider, Some(&c)), "G");
    assert_eq!(effects::pv_muzzle(t(), "AK47_pv_muzz", &[], Some(&c)), "", "no flash");
}

// ---------------------------------------------------------------------
// Follow-up cases (Open questions 1, 4, 6, 8, 17-25: cases 40-57).

/// 40. Placement: a socket-attached key (mode 4) follows the socket, its
/// offset along the socket's forward; a fixed one (mode 0) stays at its
/// creation point, only its offset turning.
#[test]
fn t40_placement_modes() {
    let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let socket = Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)).with_rotation(turn);
    let offset = Vec3::new(0.0, 0.0, 2.0);
    let attached = rules::place_key(4, socket, Vec3::ZERO, offset, turn);
    assert!(attached.distance(Vec3::new(10.0, 0.0, 0.0) + turn * offset) < 1e-5);
    assert!((attached.distance(socket.translation) - 2.0).abs() < 1e-5, "2 u along the socket's axis");
    let fixed = rules::place_key(0, socket, Vec3::new(1.0, 0.0, 0.0), offset, turn);
    assert!(fixed.distance(Vec3::new(1.0, 0.0, 0.0) + turn * offset) < 1e-5);
}

/// 41. A two-key alpha curve on a 2 s looping key: linear, held after its
/// last key, starting over.
#[test]
fn t41_key_curve() {
    let keys = [(0.0, 1.0), (0.5, 0.0)];
    let at = |t: f32| rules::key_curve(&keys, t, 2.0, true, 1.0);
    assert!((at(0.5) - 0.5).abs() < 1e-5);
    assert!(at(1.0).abs() < 1e-5);
    assert!(at(1.5).abs() < 1e-5, "held");
    assert!((at(2.5) - 0.5).abs() < 1e-5, "again");
}

/// 42. A cone emitter's start points lie in the cone, most of them in its
/// wide half.
#[test]
fn t42_cone_emitter() {
    let mut high = 0;
    let n = 1000;
    for i in 0..n {
        let u = [((i * 7919) % 1000) as f32 / 1000.0, ((i * 104729) % 997) as f32 / 997.0, ((i * 31) % 991) as f32 / 991.0];
        let p = rules::cone_start(10.0, 40.0, u);
        assert!(p.z >= 0.0 && p.z <= 40.0);
        assert!(p.truncate().length() <= 10.0 * p.z / 40.0 + 1e-4, "{p}");
        if p.z > 20.0 {
            high += 1;
        }
    }
    assert!(high > n / 2, "{high}");
}

/// 43. A marked particle bounces off its plane, an unmarked one passes.
#[test]
fn t43_bounce() {
    let (p, v) = rules::bounce_step(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -100.0, 0.0), 0.05, 0.0, true);
    assert_eq!(p, Vec3::new(0.0, 1.0, 0.0));
    assert!((v - Vec3::new(0.0, 75.0, 0.0)).length() < 1e-4);
    let (p, _) = rules::bounce_step(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -100.0, 0.0), 0.05, 0.0, false);
    assert!(p.y < 0.0);
}

/// 50. Kill call-outs: `EnemyDown` by the killer; `ManDown` by the
/// victim's nearest living teammate within 1000 u; none for a team kill.
#[test]
fn t50_callouts() {
    let e = |i: u32| Entity::from_raw_u32(i).unwrap();
    let alpha = (e(1), 1u8, Vec3::ZERO);
    let bravo = (e(2), 2u8, Vec3::new(0.0, 0.0, 100.0));
    let c = (e(3), 2u8, Vec3::new(0.0, 0.0, 900.0));
    let d = (e(4), 2u8, Vec3::new(0.0, 0.0, 1300.0));
    assert_eq!(rules::callouts(Some(alpha), bravo, &[alpha, c, d]), (Some(e(1)), Some(e(3))));
    assert_eq!(rules::callouts(Some(alpha), bravo, &[alpha, d]), (Some(e(1)), None), "C dead");
    let mate = (e(5), 1u8, Vec3::ZERO);
    assert_eq!(rules::callouts(Some(alpha), mate, &[alpha]).0, None, "a team kill");
}

/// 51. A map sound's messages.
#[test]
fn t51_map_sound_messages() {
    let mut s = rules::SoundFxState::created(false);
    assert!(!s.playing);
    s.message("TOGGLE");
    assert!(s.playing);
    s.message("TOGGLE");
    assert!(!s.playing);
    s.message("TRIGGER");
    s.message("TRIGGER");
    assert!(s.playing);
    assert_eq!(s.starts, 3, "restarted from the beginning, one copy");
}

/// 53. A playing sound's gain follows the listener inside r..R, and keeps
/// its last value beyond R.
#[test]
fn t53_gain_follows_motion() {
    let (r, big_r) = (100.0, 1000.0);
    let mut s = rules::PlayingSound::start(500.0, r, big_r, 100.0);
    assert!((s.gain - rules::gain(500.0, r, big_r, 100.0)).abs() < 1e-6);
    s.update(900.0);
    assert!((s.gain - rules::gain(900.0, r, big_r, 100.0)).abs() < 1e-6);
    let last = s.gain;
    s.update(1100.0);
    assert_eq!(s.gain, last, "kept beyond R");
    s.update(950.0);
    assert!((s.gain - rules::gain(950.0, r, big_r, 100.0)).abs() < 1e-6);
}

/// 54. The 256th live sound is refused; none is stopped.
#[test]
fn t54_instance_cap() {
    let mut live = rules::LiveSounds::default();
    for _ in 0..255 {
        assert!(live.start());
    }
    assert!(!live.start());
    assert_eq!(live.0, 255);
}

/// 56. The camera's rise: an 18 u step at 180 u/s (in 0.1 s), a 4 u one
/// at 140 u/s, a 1 u one at once.
#[test]
fn t56_camera_rise() {
    use combat_arms::game::movement::{CameraRise, movement_data};
    let d = movement_data();
    let climb = |step: f32| -> f32 {
        let mut c = CameraRise::default();
        c.step(0.0, 0.001, false, d);
        let mut t: f32 = 0.0;
        while c.step(step, 0.001, false, d) < step {
            t += 0.001;
        }
        t
    };
    // 18 u: the case's 180 u/s for 0.1 s; movement.md (its case 53)
    // recomputes the speed from the gap every frame: about 0.12 s.
    assert!((0.09..=0.125).contains(&climb(18.0)), "{}", climb(18.0));
    // 4 u at 140 u/s: about 29 ms in the case, 21 ms with movement.md's
    // snap within the last 1 u (ours).
    assert!((0.02..=0.03).contains(&climb(4.0)), "{}", climb(4.0));
    assert_eq!(climb(1.0), 0.0);
}

// ---------------------------------------------------------------------
// Corrections (2026-10-22): cases 58-66.

use chimera_core::core::{Died, Team};
use chimera_sim::weapon::grenade::{Detonated, GrenadeKind};
use combat_arms::game::{
    attachments::{MAGAZINE, ModRecord},
    characters::records::{CharacterData, Selection},
    effects::voice::{self, IdleAction, Voices},
    grenades::CaBlastSurface,
    perks::CaPerkEffects,
    view_model::{CaViewKey, machine::ViewKey},
};

fn fx_requests(sim: &mut Sim) -> Vec<CaFxRequest> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Requested>().0)
}

/// The camera of a local player (m) and its forward.
fn eye_of(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.state(p).eye_offset
}

/// 58. A grenade that goes off in mid-air shows the explosion of its last
/// bounce's surface (its own Metal when it never touched anything), 500 u
/// in front of you, with `gren.wav`; the frame's +Z is world up.
#[test]
fn c58_mid_air_frag_shows_its_explosion() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.ticks(2);
    let at = eye_of(&sim, me) + Vec3::NEG_Z * 500.0 * UNIT;
    let blast = |sim: &mut Sim, surface: Option<&str>, told: bool| -> (Vec<CaFxRequest>, Vec<CaSound>) {
        heard(sim);
        fx_requests(sim);
        let projectile = sim.app.world_mut().spawn_empty().id();
        sim.app.world_mut().write_message(Detonated {
            kind: GrenadeKind::Blast,
            weapon: "combat_arms:m67",
            thrower: None,
            at,
            ground: None,
            projectile,
            shake: None,
        });
        if told {
            sim.app.world_mut().write_message(CaBlastSurface {
                projectile,
                surface: surface.map(str::to_string),
            });
        }
        sim.app.update();
        (fx_requests(sim), heard(sim))
    };
    let look = |req: &[CaFxRequest]| -> Vec<String> {
        req.iter().filter(|r| r.kind == FxKind::Grenade).map(|r| r.group.clone()).collect()
    };
    // Never touched anything: its own metal.
    let (req, sounds) = blast(&mut sim, None, true);
    assert_eq!(look(&req), ["Grenade_Stone"], "{req:?}");
    assert_eq!(req.iter().find(|r| r.kind == FxKind::Grenade).unwrap().normal, Vec3::Y, "world up");
    let gren = sounds.iter().find(|s| s.file == "guns_snd_impacts/explosions/gren.wav").expect("its sound");
    assert_eq!(gren.radii.1, 1500.0);
    close(g(500.0, 1500.0), 0.8772, 1e-4, "gain");
    // Last bounced off a Wood wall, now far from it.
    assert_eq!(look(&blast(&mut sim, Some("30"), true).0), ["Grenade_Dirt"]);
    // Resting on Desert_Sand after bouncing there.
    assert_eq!(look(&blast(&mut sim, Some("60"), true).0), ["Grenade_Sand"]);
    // Without the server's word (a network client) and no map: metal, not Air.
    assert_eq!(look(&blast(&mut sim, None, false).0), ["Grenade_Stone"]);
}

/// 59. A shot that hits nothing still flies by (and one into a wall far
/// behind you too).
#[test]
fn c59_flyby_of_a_miss() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(40.0, 1.0, 60.0), 0.0);
    let w = arm(&mut sim, other, "combat_arms:m4a1");
    let cam = eye_of(&sim, me);
    heard(&mut sim);
    let shot = |sim: &mut Sim, from: Vec3, to: Vec3, hit: Option<Entity>| {
        sim.app.world_mut().write_message(WeaponEvent {
            owner: other,
            weapon: w,
            kind: WeaponEventKind::Shot {
                from,
                to,
                hit,
                normal: hit.map(|_| Vec3::Y),
            },
            replay: false,
        });
        sim.app.update();
        heard(sim).into_iter().filter(|s| s.file == "guns_snd_etc/flyby.wav").count()
    };
    // The line passes 200 u from the camera; its end is 3000 u away.
    let from = cam + Vec3::new(200.0 * UNIT, 0.0, -1500.0 * UNIT);
    let to = cam + Vec3::new(200.0 * UNIT, 0.0, 3000.0 * UNIT);
    assert_eq!(shot(&mut sim, from, to, None), 1, "a miss");
    // Into a wall 2000 u behind you.
    let wall = sim.app.world_mut().spawn_empty().id();
    let to = cam + Vec3::new(200.0 * UNIT, 0.0, 2000.0 * UNIT);
    assert_eq!(shot(&mut sim, from, to, Some(wall)), 1, "a hit, as before");
    // Not inside the guard (impact 600 u from you), nor for your own shot.
    let near = cam + Vec3::new(200.0 * UNIT, 0.0, 300.0 * UNIT);
    assert_eq!(shot(&mut sim, from, near, None), 0);
}

/// 60. The shooter's own tracer cadence comes from the last fitted part,
/// whether it names a tracer or not.
#[test]
fn c60_own_tracer_cadence() {
    let m4 = wfx("combat_arms:m4a1");
    let mut m = mods(&["Silencer_A_AR"], 2000, 1400);
    // A magazine part that names no tracer, after the muzzle part in slot
    // order.
    m.parts[MAGAZINE] = Some(Part {
        id: 0,
        variant: Some(0),
        record: ModRecord {
            name: "Test_Magazine".into(),
            kinds: Some(MAGAZINE as i32),
            ..Default::default()
        },
        default: false,
    });
    assert!(t().attachment("Test_Magazine").is_none());
    let f = effects::own_tracer_frequency(t(), &m4, Some(&m)).unwrap();
    assert_eq!(f, 4, "the ammo's: the later part resets the choice");
    assert_eq!(cadence(f, 8), [4, 8]);
    assert_eq!(effects::tracer_look(t(), &m4, Some(&m)).unwrap(), t().tracer("AR01").unwrap(), "the muzzle part's look");
    // The silencer alone: its own F 3.
    let a = mods(&["Silencer_A_AR"], 2000, 1400);
    assert_eq!(cadence(effects::own_tracer_frequency(t(), &m4, Some(&a)).unwrap(), 9), [3, 6, 9]);
    // A sight, then a bullet part that names none: the ammo's.
    let mut s = mods(&["DotSight_A_AR"], 2000, 1400);
    s.parts[combat_arms::game::attachments::BULLET] = m.parts[MAGAZINE].clone();
    assert_eq!(cadence(effects::own_tracer_frequency(t(), &m4, Some(&s)).unwrap(), 8), [4, 8]);
}

/// 61. Others hear a weapon's misc key sounds to 2000 u (a reload key's
/// stop at 300); no key sound comes from a dead player.
#[test]
fn c61_misc_key_radius() {
    let mut m4 = wfx("combat_arms:m4a1");
    m4.misc_sounds = vec!["guns_snd_ar/m4a1/misc1.wav".into()];
    assert_eq!(m4.key_sound(10), Some(("guns_snd_ar/m4a1/misc1.wav", 2000.0)));
    close(g(1000.0, 2000.0), 0.7166, 1e-4, "misc at 1000 u");
    assert_eq!(g(1000.0, m4.key_sound(1).unwrap().1), 0.0, "a reload key at 1000 u: silent");
    // A dead player sends none.
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(24.0, 1.0, 20.0), 0.0);
    arm(&mut sim, other, "combat_arms:m4a1");
    heard(&mut sim);
    let key = |sim: &mut Sim| {
        sim.app.world_mut().write_message(CaViewKey {
            owner: other,
            key: ViewKey::Sound(1),
        });
        sim.app.update();
        heard(sim).iter().filter(|s| s.file.ends_with("reload1.wav")).count()
    };
    assert_eq!(key(&mut sim), 1);
    sim.app.world_mut().get_mut::<Health>(other).unwrap().current = 0.0;
    assert_eq!(key(&mut sim), 0, "dead: none");
}

/// 62. Your own steps restart after you stop for longer than the bob's
/// decay: a step at once, the next about 0.21 s later, then every 0.357 s;
/// a pause of 0.05 s keeps the phase.
#[test]
fn c62_own_step_restart() {
    let dt = 1.0 / 60.0;
    let run = StepInputs {
        dt,
        moving: true,
        can_step: true,
        run: true,
        jumped: false,
        landed: false,
        alive: true,
    };
    let stop = StepInputs { moving: false, ..run };
    let mut s = OwnSteps::default();
    let frame = |s: &mut OwnSteps, i: &StepInputs| s.step(i).len();
    for _ in 0..120 {
        frame(&mut s, &run);
    }
    for _ in 0..30 {
        assert_eq!(frame(&mut s, &stop), 0, "no step while stopped");
    }
    assert_eq!(s.phase, 0.0, "reset once the amplitude reached 0");
    let mut times = Vec::new();
    for i in 0..60 {
        for _ in 0..frame(&mut s, &run) {
            times.push(i as f32 * dt);
        }
    }
    assert_eq!(times[0], 0.0, "a step on the key press");
    close(times[1], 0.212, 0.02, "the next step");
    close(times[2], 0.569, 0.02, "and the one after");
    // A short pause: the phase carries on.
    let mut s = OwnSteps::default();
    for _ in 0..40 {
        frame(&mut s, &run);
    }
    let before = s.phase;
    assert!(before > 0.0);
    for _ in 0..3 {
        frame(&mut s, &stop);
    }
    assert!(s.phase > 0.0 && s.ramp > 0.0, "0.05 s: no reset");
    frame(&mut s, &run);
    assert!(s.phase > 0.0);
    // The key press itself makes the start step (nothing about
    // displacement in the inputs): pressing again after a pause too.
    let mut s = OwnSteps::default();
    assert_eq!(frame(&mut s, &run), 1);
    assert_eq!(frame(&mut s, &StepInputs { run: false, ..run }), 0, "walking: silent");
}

/// 63. Ghillie-type models' steps are at half volume, and sound from the
/// body's centre.
#[test]
fn c63_ghillie_step() {
    let (volume, radius, extra) = effects::footstep_sound(None, 1.0, -0.5);
    assert_eq!((volume, radius, extra), (50.0, 1200.0, None));
    close(rules::gain(500.0, 84.0, radius, volume), 0.4025, 1e-4, "at 500 u on Metal");
    // The share comes from the model record (the install's models file).
    let mut data = CharacterData::clone(combat_arms::game::characters::records::builtin());
    assert_eq!(data.effective(&Selection::default()).0.footprint_volume, 0.0, "stock bodies: none");
    data.model[0].footprint_volume = -0.5;
    assert_eq!(data.effective(&Selection::default()).0.footprint_volume, -0.5);
    // Others' steps sound at the character's centre, not its feet.
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(28.0, 1.0, 20.0), 0.0);
    type_everything(&mut sim, 20);
    heard(&mut sim);
    sim.intent(other).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let centre_y = sim.position(other).y;
    let steps: Vec<CaSound> = heard(&mut sim).into_iter().filter(|s| s.at.is_some() && s.file.contains("footsteps/metal")).collect();
    assert!(steps.len() >= 2, "{steps:?}");
    for s in &steps {
        close(s.at.unwrap().y, centre_y, 0.01, "the body's centre");
    }
}

/// 64. Awareness plays an enemy's step at the perk's volume and radius
/// without Specter, plus one of its own four sounds.
#[test]
fn c64_awareness() {
    // Specter v = 0.4 on the walker.
    let (volume, radius, extra) = effects::footstep_sound(Some((1.0, 0.9)), 0.4, 0.0);
    assert_eq!((volume, extra), (100.0, Some(100.0)));
    close(radius, 1080.0, 1e-3, "radius");
    assert_eq!(effects::footstep_sound(None, 0.4, 0.0), (40.0, 1200.0, None), "without Awareness: Specter");
    assert_eq!(t().sounds.awareness.len(), 4);
    // In play: a running Specter enemy heard by an Awareness user.
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert((
        LocalPlayer,
        Team(1),
        CaPerkEffects {
            awareness: true,
            aware_volume: 1.0,
            aware_radius: 0.9,
            ..Default::default()
        },
    ));
    let enemy = player(&mut sim, Vec3::new(28.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(enemy).insert((
        Team(2),
        CaPerkEffects {
            step_volume: 0.4,
            ..Default::default()
        },
    ));
    type_everything(&mut sim, 20);
    heard(&mut sim);
    sim.intent(enemy).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let s = heard(&mut sim);
    let steps: Vec<&CaSound> = s.iter().filter(|s| s.at.is_some() && s.file.contains("footsteps/metal")).collect();
    let perk: Vec<&CaSound> = s.iter().filter(|s| s.file.starts_with("ui_snd/perk/awareness/")).collect();
    assert!(steps.len() >= 2, "{s:?}");
    assert_eq!(steps.len(), perk.len(), "one perk sound per step");
    assert!(steps.iter().all(|s| s.volume == 100.0 && (s.radii.1 - 1080.0).abs() < 1e-3), "{steps:?}");
    assert!(perk.iter().all(|s| s.volume == 100.0 && (s.radii.1 - 1080.0).abs() < 1e-3), "{perk:?}");
}

/// 65. Your local voices: the kill voice (not on a groin kill, not while
/// the last still plays), the dash sound back to back while sprinting, the
/// idle sound after 5 s with no keys, stopped by the next key.
#[test]
fn c65_local_voices() {
    // The state machine.
    let mut v = Voices::default();
    assert!(v.kill(Hitgroup::Chest, 0.0, 1.0), "a body shot");
    assert!(!v.kill(Hitgroup::Chest, 0.2, 1.0), "the last still plays");
    assert!(!v.kill(Hitgroup::Stomach, 2.0, 1.0), "a groin kill: none");
    assert!(v.kill(Hitgroup::Head, 2.0, 1.0));
    let mut v = Voices::default();
    let plays: Vec<f64> = (0..180).map(|i| i as f64 / 60.0).filter(|&t| v.dash(true, t, 1.0)).collect();
    assert_eq!(plays.len(), 3, "back to back for 3 s: {plays:?}");
    let mut v = Voices::default();
    let mut first = None;
    for i in 0..400 {
        let now = i as f64 / 60.0;
        if v.idle(true, true, 1.0 / 60.0, now, 1.0) == IdleAction::Play && first.is_none() {
            first = Some(now);
        }
    }
    close(first.unwrap() as f32, 5.0, 0.03, "idle starts at 5 s");
    assert_eq!(v.idle(false, true, 0.016, 7.0, 1.0), IdleAction::Stop, "the next key kills it");
    assert_eq!(v.idle(false, true, 0.016, 7.1, 1.0), IdleAction::Nothing);
    assert_eq!(v.idle(true, false, 0.016, 8.0, 1.0), IdleAction::Nothing, "an empty entry: no timer");
    assert!(voice::flags_idle(&Default::default()));
    assert!(!voice::flags_idle(&chimera_core::core::Intent {
        crouch: true,
        ..Default::default()
    }));
    // The stock files (male template; its idle entry is empty).
    assert_eq!(t().sounds.kill, ["chars_snd/male/enemykill.wav"]);
    assert_eq!(t().sounds.dash.len(), 3);
    assert_eq!(t().sounds.heartbeat, ["ui_snd/heartbeat.wav"]);
    assert!(t().sounds.idle.is_empty());
    // In play.
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let foe = player(&mut sim, Vec3::new(30.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().resource_mut::<CaFxTables>().sounds.idle = vec!["chars_snd/q_male/idle_male.wav".into()];
    sim.ticks(2);
    heard(&mut sim);
    let kill = |sim: &mut Sim, zone: Hitgroup| {
        sim.app.world_mut().write_message(Died {
            entity: foe,
            attacker: Some(me),
            damage: Damage {
                target: foe,
                attacker: Some(me),
                amount: 1.0,
                point: Vec3::ZERO,
                dir: Vec3::Z,
                hitgroup: zone,
                kind: DamageKind::Bullet,
                weapon: None,
                force: Vec3::ZERO,
            },
        });
        sim.ticks(1);
        heard(sim).iter().filter(|s| s.file.ends_with("enemykill.wav")).count()
    };
    assert_eq!(kill(&mut sim, Hitgroup::Chest), 1, "one kill voice, local");
    assert_eq!(kill(&mut sim, Hitgroup::Chest), 0, "while it still plays");
    sim.seconds(1.5);
    heard(&mut sim);
    assert_eq!(kill(&mut sim, Hitgroup::Stomach), 0, "a groin kill");
    // Idle: nothing for 4.5 s, the sound by 5.1 s, a key stops it.
    sim.seconds(1.0);
    heard(&mut sim);
    let idle = |sim: &mut Sim| heard(sim).iter().filter(|s| s.file.ends_with("idle_male.wav")).cloned().collect::<Vec<_>>();
    sim.seconds(2.0);
    assert!(idle(&mut sim).is_empty(), "before 5 s");
    sim.seconds(1.0);
    let started = idle(&mut sim);
    assert!(started.iter().any(|s| s.at.is_none() && s.volume > 0.0), "{started:?}");
    sim.intent(me).move_axis = Vec2::Y;
    sim.ticks(2);
    let stopped = idle(&mut sim);
    assert!(stopped.iter().any(|s| s.volume == 0.0), "killed by a silent copy: {stopped:?}");
}

/// 66. A hurt sound plays at the victim's centre (not the hit point), and
/// burning to the head makes no head sound.
#[test]
fn c66_hurt_sound_place() {
    let mut sim = sim();
    let victim = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.ticks(2);
    heard(&mut sim);
    let centre = sim.position(victim);
    let head = centre + Vec3::new(0.0, 0.6, 0.0);
    let hurt = |kind, hitgroup| Damage {
        target: victim,
        attacker: None,
        amount: 0.1,
        point: head,
        dir: Vec3::Z,
        hitgroup,
        kind,
        weapon: None,
        force: Vec3::ZERO,
    };
    sim.app.world_mut().write_message(hurt(DamageKind::Bullet, Hitgroup::Head));
    sim.app.update();
    let s = heard(&mut sim);
    let pain = s.iter().find(|s| s.file.contains("hurtpain_a")).expect("the gun pain");
    let shout = s.iter().find(|s| s.file.ends_with("headshot.wav")).expect("the head sound");
    assert_eq!(pain.at, Some(centre));
    assert_eq!(shout.at, Some(centre));
    assert_eq!(effects::hurt_radius(t(), HurtKind::Head), 500.0);
    // Burning damage to the head: only the burn sound (after the first
    // sounds ended).
    sim.seconds(3.0);
    heard(&mut sim);
    sim.app.world_mut().write_message(hurt(DamageKind::Burn, Hitgroup::Head));
    sim.app.update();
    let s = heard(&mut sim);
    assert!(s.iter().any(|s| s.file.contains("hurtfire")), "{s:?}");
    assert!(s.iter().all(|s| !s.file.ends_with("headshot.wav")), "{s:?}");
    assert_eq!(effects::hurt_kinds(DamageKind::Burn, Hitgroup::Stomach, false), [Some(HurtKind::Burning), None]);
    assert_eq!(effects::hurt_kinds(DamageKind::Generic, Hitgroup::Head, true)[1], None, "poison too");
}

/// Corrections 10: every further pellet makes the muzzle group again; the
/// fire sound and the casing come once.
#[test]
fn corrections_10_pellets_repeat_the_muzzle_group() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let w = arm(&mut sim, me, "combat_arms:m4a1");
    sim.ticks(4);
    let cam = eye_of(&sim, me);
    heard(&mut sim);
    fx_requests(&mut sim);
    let fired = WeaponEvent {
        owner: me,
        weapon: w,
        kind: WeaponEventKind::Fired {
            id: "combat_arms:m4a1",
            origin: cam,
            yaw: 0.0,
            pitch: 0.0,
            seed: 1,
            spread: chimera_sim::weapon::SpreadShape::Template(0.0),
            mode: 0,
        },
        replay: false,
    };
    let pellet = WeaponEvent {
        kind: WeaponEventKind::Shot {
            from: cam,
            to: cam + Vec3::NEG_Z * 200.0 * UNIT,
            hit: None,
            normal: None,
        },
        ..fired.clone()
    };
    let mut events = vec![fired];
    // The round's traces are re-made from its `Fired` (a client's re-trace
    // of a shot: one `Shot` for this single-pellet weapon); 7 more here.
    events.extend((0..7).map(|_| pellet.clone()));
    sim.app.world_mut().write_message_batch(events);
    sim.app.update();
    let reqs = fx_requests(&mut sim);
    let own = reqs.iter().filter(|r| r.kind == FxKind::OwnFire).count();
    assert_eq!(own, 8, "one group per pellet (the first with the round)");
    let fire = heard(&mut sim).iter().filter(|s| s.file == "guns_snd_ar/m4a1/fire.wav").count();
    assert_eq!(fire, 1, "one fire sound");
}

/// Corrections 11: a casing's turn compounds with its age.
#[test]
fn corrections_11_casings_tumble_faster() {
    let mut c = Casing {
        at: Vec3::new(0.0, 1000.0, 0.0),
        velocity: Vec3::ZERO,
        spin: Vec2::new(6.0, 0.0),
        angles: Vec2::ZERO,
        orientation: Quat::IDENTITY,
        bounces: 0,
        age: 0.0,
        resting: false,
        in_water: false,
        first_person: false,
    };
    let dt = 1.0 / 60.0;
    let mut turns = Vec::new();
    for _ in 0..13 {
        let before = c.orientation;
        c.step(dt, Vec3::ZERO, |_, _| None);
        turns.push((before.inverse() * c.orientation).to_axis_angle().1);
    }
    // After 0.1 s about 0.6 rad per frame; after 0.2 s about 1.2.
    close(turns[5], 6.0 * 6.0 * dt, 0.01, "frame 6");
    assert!(turns[11] > turns[5] * 1.9, "{turns:?}");
}

/// Corrections 12: others see the weapon's flash-hider group for a muzzle
/// part of that level that names none.
#[test]
fn corrections_12_third_person_flash_hider() {
    let mut m4 = wfx("combat_arms:m4a1");
    m4.hh_flash_hider = vec!["FH_A".into(), "FH_B".into()];
    let mut c = mods(&[], 2000, 1400);
    c.parts[combat_arms::game::attachments::MUZZLE] = Some(Part {
        id: 0,
        variant: Some(0),
        record: ModRecord {
            name: "Test_Hider".into(),
            kinds: Some(0),
            flash_hider_level: 1,
            ..Default::default()
        },
        default: false,
    });
    assert_eq!(effects::hh_muzzle(t(), &m4, Some(&c)), "FH_A");
    c.parts[combat_arms::game::attachments::MUZZLE].as_mut().unwrap().record.flash_hider_level = 2;
    assert_eq!(effects::hh_muzzle(t(), &m4, Some(&c)), "FH_B");
    c.parts[combat_arms::game::attachments::MUZZLE].as_mut().unwrap().record.flash_hider_level = 0;
    assert_eq!(effects::hh_muzzle(t(), &m4, Some(&c)), "M4_HH_Muzz");
}

/// A key drawn through an effect shader (`EffectShaderID` > 0: the
/// blasts' refraction ring, a distortion map) draws nothing; it once
/// showed as a square of its blue and pink texture in every frag's
/// explosion.
#[test]
fn shaded_keys_draw_nothing() {
    use chimera_world::map::particles::{MapParticles, NoWorld, ParticleMaterial, Particles};
    use combat_arms::game::effects::{
        fx::CaFx,
        library::{FxLibrary, material_name, shaded},
    };
    use lithtech_formats::fxf::{FxFile, FxGroup, FxKey, FxProp, FxValue};
    const TEX: &str = "fx/explosion/ring.dtx";
    let prop = |name: &str, value: FxValue| FxProp { name: name.into(), value };
    let key = |shader: i64| FxKey {
        kind: "ParticleSystem".into(),
        end: 0.5,
        props: vec![
            prop("Texture", FxValue::Path { ext: "dtx".into(), path: TEX.into() }),
            prop("EmissionInterval", FxValue::Float(-1.0)),
            prop("ParticlesPerEmission", FxValue::Int(1)),
            prop("MinParticleLifeSpan", FxValue::Float(0.3)),
            prop("MaxParticleLifeSpan", FxValue::Float(0.4)),
            prop("EffectShaderID", FxValue::Int(shader)),
        ],
        ..default()
    };
    assert!(shaded(&key(100)));
    assert!(!shaded(&key(0)));
    let lib = FxLibrary {
        file: FxFile {
            groups: vec![
                FxGroup { name: "Plain".into(), phase_ms: 500, keys: vec![key(0)] },
                FxGroup { name: "Refract".into(), phase_ms: 500, keys: vec![key(100)] },
            ],
        },
        ..default()
    };
    let materials = MapParticles {
        materials: vec![ParticleMaterial { name: material_name(TEX, false), ..default() }],
    };
    let count = |group: &str| {
        let (mut fx, mut particles) = (CaFx::default(), Particles::default());
        assert!(fx.spawn(&lib, group, Vec3::ZERO, Mat3::IDENTITY));
        fx.step(0.05, &lib, &mut particles, &materials, &NoWorld);
        particles.count()
    };
    assert_eq!(count("Plain"), 1, "an ordinary key emits");
    assert_eq!(count("Refract"), 0, "a shaded key draws nothing");
}

// ---------------------------------------------------------------------------
// The freeze reaches the view (the match's end screen, a round's freeze)

/// The raw trigger as the input writes it every frame (`Update`), then
/// the hold the client applies to it (`predict::hold_view`).
fn trigger_held_down(sim: &mut Sim) {
    sim.app.add_systems(
        Update,
        (
            (|mut i: Single<&mut chimera_core::core::Intent, With<LocalPlayer>>| {
                i.fire = true;
                i.secondary = true;
                i.move_axis = Vec2::Y;
            }),
            chimera_net::net::predict::hold_view,
        )
            .chain()
            .before(chimera_world::map::DriveAnimation),
    );
}

/// Once the match has ended (or in a freeze time) holding fire plays
/// nothing: no round, no shot sound, no muzzle flash or tracer, no fire
/// animation; the same trigger fires when nothing holds the player.
#[test]
fn a_held_player_holding_fire_shows_and_sounds_nothing() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let w = arm(&mut sim, me, "combat_arms:m4a1");
    type_everything(&mut sim, 20);
    trigger_held_down(&mut sim);
    let clip = |sim: &Sim| sim.app.world().get::<weapon::Magazine>(w).unwrap().clip;
    let before = clip(&sim);
    let spot = sim.position(me);
    heard(&mut sim);
    fx_requests(&mut sim);
    sim.app.insert_resource(chimera_core::core::FreezeTime(true));
    sim.seconds(1.0);
    assert_eq!(clip(&sim), before, "no round");
    assert!(heard(&mut sim).is_empty(), "no sound, no footsteps");
    assert!(sim.position(me).distance(spot) < 0.01, "held where it stood");
    assert!(fx_requests(&mut sim).is_empty(), "no flash, tracer or brass");
    let view = sim.app.world().get::<chimera_world::map::ViewAnimator>(me);
    assert!(
        view.is_none_or(|v| !v.activity().is_some_and(|a| a.contains("ATTACK") || a.contains("FIRE"))),
        "no fire animation"
    );
    // Control: lifted, the same keys shoot and walk.
    sim.app.insert_resource(chimera_core::core::FreezeTime(false));
    sim.seconds(0.5);
    assert!(clip(&sim) < before, "fires when free");
    assert!(sim.position(me).distance(spot) > 0.1, "walks when free");
    assert!(!heard(&mut sim).is_empty());
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-30) 3: how a tracer strip is drawn.

/// 73. The M4A1's AR02 (`Width` 15) seen from the side is 7.5 u across
/// in the world, whatever the zoom (a world width: no floor or cap).
#[test]
fn t73_tracer_width() {
    let look = effects::tracer_look(t(), &wfx("combat_arms:m4a1"), None).unwrap();
    close(rules::tracer_across(look.width), 7.5, 1e-6, "AR02 across");
    // Seen from the side, its middle 1000 u ahead: the corners 7.5 u apart.
    let cam = Transform::IDENTITY;
    let (a, b) = (Vec3::new(-300.0, 0.0, -1000.0), Vec3::new(300.0, 0.0, -1000.0));
    let [ta, _, _, tb] = rules::tracer_corners(a, b, &cam, look.width).unwrap();
    close((ta - tb).length(), 7.5, 1e-4, "world width");
    close((ta - tb).x, 0.0, 1e-4, "across the strip");
}

/// 74. One width direction for both ends: Δ = (−10, 10) → n = (0.7071,
/// 0.7071, 0); corners at each end ± 3.75·n (AR02). Our camera space looks
/// along −Z (x right, y up as the spec's), so the spec's z 50 is −50.
#[test]
fn t74_tracer_facing() {
    let (a, b) = (Vec3::new(10.0, -10.0, -50.0), Vec3::new(0.0, 0.0, -1050.0));
    let n = rules::tracer_side(a, b).unwrap();
    assert!((n - Vec3::new(0.7071, 0.7071, 0.0)).length() < 1e-4, "{n}");
    let [ah, bh, bl, al] = rules::tracer_corners(a, b, &Transform::IDENTITY, 15.0).unwrap();
    for (c, want) in [(ah, a + 3.75 * n), (bh, b + 3.75 * n), (bl, b - 3.75 * n), (al, a - 3.75 * n)] {
        assert!((c - want).length() < 1e-4, "{c} vs {want}");
    }
    // A turned and moved camera: the same strip in its frame, the same n.
    let cam = Transform::from_xyz(100.0, 20.0, -5.0).with_rotation(Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-0.2));
    let [ah2, bh2, ..] = rules::tracer_corners(cam * a, cam * b, &cam, 15.0).unwrap();
    assert!((ah2 - cam * ah).length() < 1e-3 && (bh2 - cam * bh).length() < 1e-3);
    // Pointing straight at the eye: not seen.
    assert!(rules::tracer_side(Vec3::new(0.0, 0.0, -10.0), Vec3::new(0.0, 0.0, -500.0)).is_none());
}

/// 76. A zoomed sniper's own tracer is made, its tail at the data flash
/// point F (camera + `Pos` + `MuzzlePos` in camera axes, as unzoomed); the
/// record's colour and alpha play no part in it (the draw is the texture's).
#[test]
fn t76_scoped_own_tracer() {
    use combat_arms::game::{view_model::CaViewModel, weapons::CaZoom};
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let w = arm(&mut sim, me, "combat_arms:l96a1");
    sim.intent(me).pitch = 0.0;
    sim.intent(me).secondary = true;
    sim.ticks(1);
    sim.intent(me).secondary = false;
    sim.seconds(0.5);
    let z = sim.app.world().get::<CaZoom>(w).unwrap();
    assert!(z.level > 0 && z.transition.is_none(), "zoomed in");
    let f = sim.app.world().get::<CaViewModel>(me).unwrap().flash_point;
    fire_once(&mut sim, me);
    let list = &sim.app.world().resource::<CaTracers>().0;
    assert_eq!(list.len(), 1, "SR01: every shot, scoped too");
    let from = list[0].path.from * UNIT;
    assert!((from - f).length() < 0.02, "tail at F: {from} vs {f}");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-30) 2 and special_weapons.md 5.1: swing sounds.

const M9: &str = "combat_arms:m9_knife";
const M9_FIRE: &str = "guns_snd_melee/m9/fire.wav";
const M9_ALT: &str = "test_snd/m9/alt_fire.wav";

/// The M9 in hand with an `Alt_FireSnd` of its own (data test: the stock
/// M9 has none), the swing aimed level at nothing.
fn knife_with_alt(sim: &mut Sim, p: Entity) -> Entity {
    let w = arm(sim, p, M9);
    sim.app.world_mut().get_mut::<CaWeaponFx>(w).unwrap().alt_fire_sound = M9_ALT.into();
    sim.intent(p).pitch = 0.6;
    w
}

/// One swing with the button `secondary` (held for one tick), then
/// time for its hits.
fn swing(sim: &mut Sim, p: Entity, secondary: bool) {
    if secondary {
        sim.intent(p).secondary = true;
    } else {
        sim.intent(p).fire = true;
    }
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.intent(p).secondary = false;
    sim.seconds(1.3);
}

/// effects_sounds 71 / special_weapons 44: the attacker's own client plays
/// `Alt_FireSnd` for a secondary swing; others hear `FireSnd` for both.
#[test]
fn t71_own_alt_fire_sound() {
    let wfx_ = |alt: &str| CaWeaponFx {
        fire_sound: "a.wav".into(),
        alt_fire_sound: alt.into(),
        ..wfx(M9)
    };
    let w = wfx_("b.wav");
    assert_eq!(effects::fire_sound_of(&w, None, true, false).unwrap().0, "a.wav");
    assert_eq!(effects::fire_sound_of(&w, None, true, true).unwrap().0, "b.wav");
    assert_eq!(effects::fire_sound_of(&w, None, false, false).unwrap().0, "a.wav", "others: plain");
    assert_eq!(effects::fire_sound_of(&wfx_(""), None, true, true).unwrap().0, "a.wav", "none named");

    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, Vec3::new(24.0, 1.0, 20.0), 0.0);
    knife_with_alt(&mut sim, me);
    knife_with_alt(&mut sim, other);
    heard(&mut sim);
    let fire = |h: &[CaSound], f: &str| h.iter().filter(|s| s.file == f).count();
    swing(&mut sim, me, false);
    let h = heard(&mut sim);
    assert_eq!((fire(&h, M9_FIRE), fire(&h, M9_ALT)), (2, 0), "primary: two hit keys, FireSnd: {h:?}");
    swing(&mut sim, me, true);
    let h = heard(&mut sim);
    assert_eq!((fire(&h, M9_FIRE), fire(&h, M9_ALT)), (0, 1), "secondary: Alt_FireSnd: {h:?}");
    assert!(h.iter().filter(|s| s.file == M9_ALT).all(|s| s.at.is_none()), "local");
    swing(&mut sim, other, true);
    let h = heard(&mut sim);
    assert_eq!((fire(&h, M9_FIRE), fire(&h, M9_ALT)), (1, 0), "others hear FireSnd (Quirk 10): {h:?}");
    assert!(h.iter().filter(|s| s.file == M9_FIRE).all(|s| s.at.is_some()), "3D");
}

/// effects_sounds 72 / special_weapons 43: one fire voice per attacker
/// and weapon, both buttons sharing it; another weapon's is its own.
#[test]
fn t72_shared_fire_voice() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let knife = knife_with_alt(&mut sim, me);
    heard(&mut sim);
    swing(&mut sim, me, false);
    swing(&mut sim, me, true);
    let h = heard(&mut sim);
    let voices: Vec<_> = h.iter().filter(|s| s.file == M9_FIRE || s.file == M9_ALT).map(|s| (s.source, s.channel)).collect();
    assert_eq!(voices.len(), 3);
    assert!(voices.iter().all(|v| *v == (Some(knife), Some(effects::sounds::CHANNEL_FIRE))), "{voices:?}");
    let gun = arm(&mut sim, me, "combat_arms:m4a1");
    sim.intent(me).pitch = 0.0;
    heard(&mut sim);
    fire_once(&mut sim, me);
    let h = heard(&mut sim);
    let shot = h.iter().find(|s| s.file == "guns_snd_ar/m4a1/fire.wav").expect("the shot");
    assert_eq!(shot.source, Some(gun), "another weapon: its own voice");
}

/// special_weapons 45: a swing at nothing plays its fire sound per hit key
/// and no impact sound (a sky impact is silent).
#[test]
fn t45_a_miss_is_the_fire_sound_alone() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, M9);
    // Up at the open sky of the greybox.
    sim.intent(me).pitch = 1.4;
    heard(&mut sim);
    swing(&mut sim, me, false);
    let h = heard(&mut sim);
    assert_eq!(h.iter().filter(|s| s.file == M9_FIRE).count(), 2, "{h:?}");
    assert!(!h.iter().any(|s| s.file.contains("impact")), "no impact sound: {h:?}");
}

/// A round on Bullet-Proof Glass (surface 41, a pane with its own surface:
/// a glass set of a Combat Arms map, `map::MapCollisionSet::surface`)
/// takes that surface's impact (`Impact_Glass_BulletProof`) and decal,
/// not the world's around it (here metal).
#[test]
fn a_round_on_bullet_proof_glass_takes_its_impact() {
    use avian3d::prelude::{Collider, RigidBody};
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0), 0.0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    arm(&mut sim, me, "combat_arms:m4a1");
    type_everything(&mut sim, 20);
    let eye = sim.position(me) + sim.state(me).eye_offset;
    sim.app.world_mut().spawn((
        Transform::from_translation(Vec3::new(eye.x, eye.y, eye.z - 3.0)),
        RigidBody::Static,
        Collider::cuboid(2.0, 2.0, 0.02),
        chimera_world::map::PropSurface("Bullet-Proof Glass".into()),
    ));
    sim.intent(me).pitch = 0.0;
    sim.ticks(2);
    heard(&mut sim);
    sim.app.world_mut().resource_mut::<Requested>().0.clear();
    fire_once(&mut sim, me);
    let req = std::mem::take(&mut sim.app.world_mut().resource_mut::<Requested>().0);
    assert!(req.iter().any(|r| r.group == "Impact_Glass_BulletProof" && r.kind == FxKind::Surface), "{req:?}");
    assert!(!req.iter().any(|r| r.group == "Impact_Metal"), "{req:?}");
    let marks = sim.app.world().resource::<CaMarks>();
    assert_eq!(marks.marks.last().map(|m| m.sprite.as_str()), Some("fx/i_mark/im_glass_bp.spr"));
}
