//! specs/combat_arms/view_models.md's test cases (`t04_` is test case
//! 4) that need no install, and the view model in play
//! (`game::view_model`): the state machine on stand-in animation tables
//! with the spec's stock lengths and keys (S: Steam, B: BR 1810.04),
//! stepped at the spec's 60 fps; the sway, placement, flash, field of
//! view and hand-choice maths; and on the greybox, stand-in view models
//! named as the install's (one bone, a `flash` socket) driven by the
//! game. Cases 1, 2 and 5-8 need the install (heavy::view_model).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{Health, LocalPlayer};
use chimera_sim::weapon::{self, Inventory, Magazine, data::WeaponDataAppExt, grenade::Throwable};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        MapAttachment, MapBone, MapMesh, MapModel, MapViewModel, ViewAnimator, ViewModelOffset, ViewModels,
        anim::{AnimEvent, AnimSet, Animation, Sequence},
        view_model::{aspect_fov, vertical_fov, view_model_fov},
    },
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        effects::CaShell,
        movement,
        view_model::{
            CaViewKey, CaViewModel, DEFAULT_FOV, VIEW_SCALE, VIEWMODEL_FOV,
            hands::{self, HandProfile},
            machine::{AnimInfo, Class, FireType, Inputs, Rates, Setup, Stage, ViewKey, ViewMachine},
            place::{self, CameraAxes, FlashTimer},
            settings,
            sway::{Sway, SwayInputs},
        },
    },
    harness::CaSim,
};

/// One of the spec's frames (60 fps).
const F: f64 = 1.0 / 60.0;

fn a(name: &str, length: u32, keys: &[(u32, &str)]) -> AnimInfo {
    AnimInfo::new(name, length, keys)
}

/// The M92FS view model (S).
fn m92fs() -> Arc<Vec<AnimInfo>> {
    Arc::new(vec![
        a("CM_HND_SP_DF_Beretta_M92F_FS_CH", 0, &[]),
        a("select", 650, &[(296, "sound_key 3")]),
        a("idle_0", 1566, &[]),
        a("idle_1", 1566, &[]),
        a("prefire", 0, &[]),
        a("fire_0", 180, &[(0, "FIRE_KEY")]),
        a("postfire", 150, &[]),
        a("dryFire", 300, &[(0, "dryfire_key")]),
        a("reload", 1500, &[(226, "sound_key 2"), (453, "sound_key 1"), (962, "sound_key 3")]),
        a("PreRush", 250, &[]),
        a("rush", 1500, &[]),
        a("PostRush", 400, &[]),
    ])
}

/// The M4A1 (B): its `idle_0` with a key at 500 ms for case 32.
fn m4a1() -> Arc<Vec<AnimInfo>> {
    Arc::new(vec![
        a("select", 1200, &[(666, "sound_key 3")]),
        a("idle_0", 1833, &[(500, "")]),
        a("idle_1", 1833, &[]),
        a("prefire", 0, &[]),
        a("fire_0", 105, &[(0, "FIRE_KEY")]),
        a("postfire", 0, &[]),
        a("reload", 2300, &[(403, "sound_key 1"), (834, "sound_key 2"), (1725, "sound_key 3")]),
        a("PreRush", 233, &[]),
        a("rush", 620, &[]),
        a("PostRush", 367, &[]),
    ])
}

/// The M9 knife (B).
fn m9() -> Arc<Vec<AnimInfo>> {
    Arc::new(vec![
        a("select", 450, &[(32, "sound_key 4")]),
        a("idle_0", 2000, &[]),
        a("idle_1", 2000, &[]),
        a("prefire", 1, &[]),
        a("fire_0", 650, &[(155, "FIRE_KEY"), (402, "FIRE_KEY")]),
        a("alt_fire_0", 1100, &[(367, "ALT_FIRE_KEY")]),
        a("postfire", 1, &[]),
        a("PreRush", 167, &[]),
        a("rush", 666, &[]),
        a("PostRush", 167, &[]),
    ])
}

/// The M67 (B).
fn m67() -> Arc<Vec<AnimInfo>> {
    Arc::new(vec![
        a("select", 400, &[]),
        a("idle_0", 933, &[]),
        a("prefire", 700, &[(513, "sound_key 10")]),
        a("fire_0", 67, &[(0, "THROW_KEY")]),
        a("postfire", 366, &[(133, "FIRE_KEY")]),
        a("reload", 400, &[]),
        a("PreRush", 167, &[]),
        a("rush", 666, &[]),
        a("PostRush", 267, &[]),
    ])
}

/// A shotgun shell by shell (the 870's lengths).
fn r870() -> Arc<Vec<AnimInfo>> {
    Arc::new(vec![
        a("select", 1000, &[]),
        a("idle_0", 2000, &[]),
        a("prefire", 0, &[]),
        a("fire_0", 600, &[(0, "FIRE_KEY")]),
        a("PreReload", 333, &[]),
        a("reload", 700, &[]),
        a("PostReload", 900, &[]),
    ])
}

fn setup(class: Class, per_press: u32, clip_size: u32) -> Setup {
    Setup {
        class,
        per_press,
        shells: false,
        clip_size,
        rates: Rates::default(),
        ..Setup::default()
    }
}

/// A weapon on the machine, stepped one 60 fps frame per `frame`: the
/// machine's shots take a round, its reloads fill the clip.
struct Run {
    m: ViewMachine,
    clip: u32,
    total: u32,
    /// Frames run (the next frame's update index).
    n: u32,
    /// (update, animation) for every start.
    starts: Vec<(u32, String)>,
    /// Updates that consumed a shot.
    shots: Vec<(u32, FireType)>,
    /// (update whose advance fired it, key).
    keys: Vec<(u32, ViewKey)>,
    reloads: Vec<u32>,
    infinite: bool,
}

impl Run {
    /// Drawn at update 0 (the draw is that update's).
    fn draw(anims: Arc<Vec<AnimInfo>>, s: Setup, clip: u32, total: u32) -> Self {
        Self {
            m: ViewMachine::draw(anims, s, 7),
            clip,
            total,
            n: 0,
            starts: vec![(0, "select".into())],
            shots: Vec::new(),
            keys: Vec::new(),
            reloads: Vec::new(),
            infinite: false,
        }
    }

    fn frame(&mut self, mut i: Inputs) {
        i.clip = self.clip;
        i.total = self.total;
        let before = (self.m.current(), self.m.time());
        let out = self.m.update(&i);
        if let Some(t) = out.shot {
            self.shots.push((self.n, t));
            if !self.infinite {
                self.clip -= 1;
                self.total -= 1;
            }
        }
        if out.reloaded {
            self.clip = self.total.min(self.m.setup().clip_size);
            self.reloads.push(self.n);
        }
        if out.shell_in {
            self.clip += 1;
        }
        let started = self.m.time() == 0.0 && (self.m.current() != before.0 || before.1 != 0.0);
        if started {
            self.starts.push((self.n, self.m.playing().unwrap_or("").to_string()));
        }
        for k in self.m.advance(F) {
            self.keys.push((self.n, k));
        }
        self.n += 1;
    }

    fn frames(&mut self, n: u32, i: Inputs) {
        for _ in 0..n {
            self.frame(i);
        }
    }

    /// Run until the subtle idle plays, then forget the history (the next
    /// update is update 0).
    fn settle(&mut self) {
        for _ in 0..600 {
            self.frame(Inputs::default());
            if self.m.playing() == Some("idle_0") {
                break;
            }
        }
        assert_eq!(self.m.playing(), Some("idle_0"));
        self.n = 0;
        self.starts.clear();
        self.shots.clear();
        self.keys.clear();
        self.reloads.clear();
    }

    fn started(&self, name: &str) -> Vec<u32> {
        self.starts
            .iter()
            .filter(|(_, n)| n.eq_ignore_ascii_case(name))
            .map(|(u, _)| *u)
            .collect()
    }
}

const FIRE: Inputs = Inputs {
    fire: true,
    alt_fire: false,
    reload: false,
    clip: 0,
    total: 0,
    sprinting: false,
    moving: false,
};
const NONE: Inputs = Inputs {
    fire: false,
    ..FIRE
};
const SPRINT: Inputs = Inputs {
    sprinting: true,
    moving: true,
    ..NONE
};
const RELOAD: Inputs = Inputs {
    reload: true,
    ..NONE
};

fn pistol() -> Setup {
    setup(Class::Secondary, 1, 15)
}

#[test]
fn t22_draw_with_fire_held() {
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.frames(60, FIRE);
    assert!(r.keys.contains(&(17, ViewKey::Sound(3))), "{:?}", r.keys);
    assert!(!r.keys.iter().any(|k| k.0 < 17 && k.1 == ViewKey::Sound(3)));
    assert_eq!(r.started("fire_0").first(), Some(&41), "{:?}", r.starts);
    assert_eq!(r.shots.first().map(|s| s.0), Some(42));
    assert!((42.0 * F * 1000.0 - 700.0).abs() < 1e-6);
    // Without fire: `idle_0` on update 41.
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.frames(60, NONE);
    assert_eq!(r.started("idle_0").first(), Some(&41), "{:?}", r.starts);
}

#[test]
fn t23_pistol_postfire_cancel() {
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.settle();
    r.frame(FIRE);
    r.frames(40, NONE);
    assert_eq!(r.started("fire_0"), vec![1]);
    assert_eq!(r.started("postfire"), vec![12], "{:?}", r.starts);
    assert_eq!(r.started("idle_0"), vec![23], "no press: idle after postfire");
    // A second press while `postfire` plays: `fire_0` at once.
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.settle();
    r.frame(FIRE);
    r.frames(14, NONE);
    assert_eq!(r.m.playing(), Some("postfire"));
    r.frame(FIRE);
    r.frames(20, NONE);
    assert_eq!(r.started("fire_0"), vec![1, 15], "{:?}", r.starts);
    assert_eq!(r.started("postfire"), vec![12, 26], "the rest of the first post-fire skipped");
    assert_eq!(r.shots.len(), 2);
}

#[test]
fn t24_knife_without_prefire() {
    let mut r = Run::draw(m9(), setup(Class::Melee, 0, 0), 1, 1);
    r.infinite = true;
    r.settle();
    r.frame(FIRE);
    r.frames(40, NONE);
    assert_eq!(r.started("fire_0"), vec![1], "{:?}", r.starts);
    assert!(r.started("prefire").is_empty());
    let hits: Vec<u32> = r.shots.iter().map(|s| s.0).collect();
    assert_eq!(hits, vec![11, 26]);
    // The stab: `alt_fire_0`, its key an alternate hit.
    let mut r = Run::draw(m9(), setup(Class::Melee, 0, 0), 1, 1);
    r.infinite = true;
    r.settle();
    r.frame(Inputs {
        alt_fire: true,
        ..NONE
    });
    r.frames(40, NONE);
    assert_eq!(r.started("alt_fire_0"), vec![1]);
    assert_eq!(r.shots, vec![(24, FireType::Alternate)]);
}

fn grenade() -> Setup {
    setup(Class::Throw, 0, 1)
}

#[test]
fn t25_grenade_release() {
    let mut r = Run::draw(m67(), grenade(), 1, 2);
    r.settle();
    r.frames(30, FIRE);
    r.frames(60, NONE);
    // Held again during the re-ready (ignored until it ends).
    r.frames(20, FIRE);
    assert_eq!(r.started("prefire"), vec![1, 100], "{:?}", r.starts);
    assert_eq!(r.started("fire_0"), vec![44]);
    assert_eq!(r.started("postfire"), vec![49]);
    assert_eq!(r.shots.first().map(|s| s.0), Some(57));
    assert!((57.0 * F * 1000.0 - 950.0).abs() < 1e-6);
    assert_eq!(r.started("reload"), vec![73]);
    assert_eq!(r.reloads, vec![98]);
    // The pin sound and the throw notice.
    assert!(r.keys.iter().any(|k| k.1 == ViewKey::Sound(10)));
    assert_eq!(r.keys.iter().filter(|k| k.1 == ViewKey::Throw).count(), 1);
}

#[test]
fn t26_grenade_held() {
    let mut r = Run::draw(m67(), grenade(), 1, 2);
    r.settle();
    r.frames(120, FIRE);
    r.frames(20, NONE);
    let loops: Vec<u32> = (0..=14).map(|k| 49 + 5 * k).collect();
    let mut fire0 = r.started("fire_0");
    assert_eq!(fire0.remove(0), 44);
    assert_eq!(fire0, loops, "{:?}", r.starts);
    assert_eq!(r.started("postfire"), vec![124]);
    assert_eq!(r.shots.first().map(|s| s.0), Some(132));
    assert!((132.0 * F * 1000.0 - 2200.0).abs() < 1e-6);
    assert_eq!(r.keys.iter().filter(|k| k.1 == ViewKey::Throw).count(), 16, "a notice per loop");
}

#[test]
fn t27_sprint_cycle() {
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.settle();
    r.frames(60, SPRINT);
    r.frames(40, FIRE);
    assert_eq!(r.started("PreRush"), vec![0], "{:?}", r.starts);
    assert_eq!(r.started("rush"), vec![16]);
    assert_eq!(r.started("PostRush"), vec![60]);
    assert_eq!(r.started("idle_1"), vec![85]);
    assert!(!r.started("idle_0").contains(&85));
    assert_eq!(r.started("fire_0"), vec![87]);
    assert!(matches!(r.m.stage, Stage::Firing(_)) || r.m.playing() == Some("postfire"));
}

#[test]
fn t28_sprint_mid_reload() {
    let mut r = Run::draw(m4a1(), setup(Class::Primary, 0, 30), 10, 100);
    r.settle();
    r.frame(RELOAD);
    r.frames(59, NONE);
    r.frames(100, SPRINT);
    assert_eq!(r.started("reload"), vec![0]);
    assert_eq!(r.reloads, vec![139]);
    assert_eq!(r.started("PreRush"), vec![139], "{:?}", r.starts);
    assert_eq!(r.clip, 30);
}

#[test]
fn t29_last_round() {
    let mut r = Run::draw(m92fs(), pistol(), 1, 16);
    r.settle();
    r.frames(115, FIRE);
    assert_eq!(r.shots.len(), 1);
    assert_eq!(r.started("fire_0"), vec![1]);
    assert_eq!(r.started("postfire"), vec![12]);
    assert_eq!(r.started("reload"), vec![23], "{:?}", r.starts);
    assert_eq!(r.reloads, vec![114]);
    assert_eq!(r.clip, 15);
}

#[test]
fn t30_dry_fire_not_reached() {
    let mut r = Run::draw(m92fs(), pistol(), 0, 0);
    r.settle();
    for _ in 0..1200 {
        r.frame(FIRE);
        let p = r.m.playing().unwrap();
        assert!(p == "idle_0" || p == "idle_1", "{p}");
    }
    assert!(r.started("dryFire").is_empty() && r.shots.is_empty());
}

#[test]
fn t32_one_frame_pop() {
    let anims = m4a1();
    let mut r = Run::draw(anims.clone(), setup(Class::Primary, 0, 30), 10, 100);
    r.settle();
    r.frames(10, NONE);
    let idle = r.m.find("idle_0").unwrap();
    assert!(r.m.time() > 0.0 && r.m.time() < 500.0);
    let i = Inputs {
        reload: true,
        clip: 10,
        total: 100,
        ..NONE
    };
    r.m.update(&i);
    // Frame n's image: `idle_0`'s key 1 (500 ms).
    assert_eq!(r.m.playing(), Some("reload"));
    assert_eq!(r.m.shown(), Some((idle, 500.0)));
    r.m.advance(F);
    r.m.update(&Inputs {
        reload: false,
        ..i
    });
    let reload = r.m.find("reload").unwrap();
    let (s, t) = r.m.shown().unwrap();
    assert_eq!(s, reload);
    assert!((t - 1000.0 * F).abs() < 1e-9, "{t}");
}

#[test]
fn full_idle_after_a_still_wait() {
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.settle();
    r.frames(60 * 20, NONE);
    let first = *r.started("idle_1").first().expect("a full idle within 20 s");
    let t = first as f64 * F;
    // The wait (5-15 s from entering idle) ends at an idle_0's end.
    assert!((4.9..=15.0 + 1.6).contains(&t), "{t}");
    // Moving: no full idle.
    let mut r = Run::draw(m92fs(), pistol(), 15, 45);
    r.settle();
    r.frames(60 * 30, Inputs {
        moving: true,
        ..NONE
    });
    assert!(r.started("idle_1").is_empty());
}

#[test]
fn rates_scale_playback() {
    // Draw rate 2: the 650 ms select ends after 326 ms (update 20).
    let fast = Setup {
        rates: Rates {
            draw: 2.0,
            ..Rates::default()
        },
        ..pistol()
    };
    let mut r = Run::draw(m92fs(), fast, 15, 45);
    r.frames(30, NONE);
    assert_eq!(r.started("idle_0"), vec![21], "{:?}", r.starts);
    // Above 2.0: 1.0.
    let capped = Setup {
        rates: Rates {
            draw: 2.5,
            ..Rates::default()
        },
        ..pistol()
    };
    let mut r = Run::draw(m92fs(), capped, 15, 45);
    r.frames(60, NONE);
    assert_eq!(r.started("idle_0"), vec![41]);
    // Reload rate 1.5: 1500 ms in 1000 ms.
    let quick = Setup {
        rates: Rates {
            reload: 1.5,
            ..Rates::default()
        },
        ..pistol()
    };
    let mut r = Run::draw(m92fs(), quick, 10, 45);
    r.settle();
    r.frame(RELOAD);
    r.frames(80, NONE);
    assert_eq!(r.reloads, vec![61], "1001 ms at 1.5: 61 frames");
}

#[test]
fn shell_reload_and_fire_stopping_it() {
    let shotgun = Setup {
        shells: true,
        ..setup(Class::Shotgun, 1, 8)
    };
    let mut r = Run::draw(r870(), shotgun, 5, 20);
    r.settle();
    r.frame(RELOAD);
    r.frames(400, NONE);
    let order: Vec<&str> = r.starts.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(order[..6], ["PreReload", "reload", "reload", "reload", "PostReload", "idle_0"], "{order:?}");
    assert_eq!(r.clip, 8);
    // Fire mid-reload stops it at once (no pre-fire for a shotgun).
    let mut r = Run::draw(r870(), shotgun, 5, 20);
    r.settle();
    r.frame(RELOAD);
    r.frames(30, NONE);
    assert_eq!(r.m.playing(), Some("reload"));
    r.frames(3, FIRE);
    assert_eq!(r.m.playing(), Some("fire_0"));
    assert!(r.started("PostReload").is_empty());
}

#[test]
fn automatic_waits_for_postfire_and_reload_after_postfire() {
    let anims = Arc::new(vec![
        a("select", 100, &[]),
        a("idle_0", 1000, &[]),
        a("prefire", 0, &[]),
        a("fire_0", 100, &[(0, "FIRE_KEY")]),
        a("postfire", 200, &[]),
        a("reload", 500, &[]),
    ]);
    let mut r = Run::draw(anims.clone(), setup(Class::Primary, 0, 30), 30, 90);
    r.settle();
    r.frame(FIRE);
    r.frames(9, NONE);
    assert_eq!(r.started("postfire"), vec![9], "{:?}", r.starts);
    // An automatic's new press waits for the post-fire.
    r.frames(3, FIRE);
    assert_eq!(r.m.playing(), Some("postfire"));
    // A reload asked for mid-fire plays after the post-fire.
    let mut r = Run::draw(anims, setup(Class::Primary, 0, 30), 20, 90);
    r.settle();
    r.frame(FIRE);
    r.frame(FIRE);
    r.frame(Inputs {
        reload: true,
        ..FIRE
    });
    r.frames(40, FIRE);
    let order: Vec<&str> = r.starts.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(order[..4], ["prefire", "fire_0", "postfire", "reload"], "{order:?}");
}

#[test]
fn grenade_and_sprint() {
    // Sprint during the pin pull: `prefire` plays out and holds; the rest
    // waits for the sprint to end.
    let mut r = Run::draw(m67(), grenade(), 1, 2);
    r.settle();
    r.frames(10, FIRE);
    r.frames(80, Inputs {
        sprinting: true,
        ..FIRE
    });
    assert_eq!(r.m.playing(), Some("prefire"));
    assert!(r.started("PreRush").is_empty() && r.started("fire_0").is_empty());
    r.frames(3, FIRE);
    assert_eq!(r.m.playing(), Some("fire_0"));
}

// Sway (6)

fn running(n: usize, dt: f64, f: impl Fn(usize) -> bool) -> Vec<Sway> {
    let mut s = Sway::default();
    (1..=n)
        .map(|i| {
            s.step(&SwayInputs {
                dt,
                clock: i as f64 * dt,
                active: true,
                run: true,
                moving: f(i),
                speed: 100.0,
                spectating: false,
                weapon: true,
                zoomed: false,
                scale: 1.0,
            });
            s
        })
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 2e-6
}

#[test]
fn t13_sway_ramp() {
    let s = running(12, F, |_| true);
    let want = [
        (1, 0.0, 0.0, 0.0, 0.0),
        (2, 0.66987, 0.0, -0.00058, 0.0),
        (3, 2.5, 0.146608, -0.0019592, 0.0010839),
        (4, 5.0, 0.293215, -0.0034227, 0.0041504),
        (5, 6.66667, 0.439823, -0.0038048, 0.0077051),
        (6, 8.33333, 0.586431, -0.0037053, 0.0115233),
        (7, 10.0, 0.733038, -0.0030902, 0.0149178),
        (12, 10.0, 1.466077, 0.0040674, 0.0031187),
    ];
    for (f, amp, phase, w, h) in want {
        let x = s[f - 1];
        assert!((x.amp - amp).abs() < 1e-5 && (x.phase - phase).abs() < 1e-6, "frame {f}: {x:?}");
        assert!((x.w - w).abs() < 1e-6 && (x.h - h).abs() < 1e-6, "frame {f}: {x:?}");
    }
    // The M4A1's origin on frame 7.
    let o = place::model_origin(Vec3::new(0.3, -0.6, 1.15), (s[6].w as f32, s[6].h as f32), 0.0);
    assert!((o - Vec3::new(0.2969098, -0.5850822, 1.15)).length() < 1e-6, "{o}");
}

#[test]
fn t14_sway_steady_state() {
    let s = running(200, F, |_| true);
    let (a, b) = (s[29], s[59]);
    assert!(near(a.phase, 4.105014) && near(a.w, 0.0008368) && near(a.h, 0.0140592), "{a:?}");
    assert!(near(b.phase, 2.220059) && near(b.w, 0.0092186) && near(b.h, -0.0144474), "{b:?}");
    assert!(s.iter().all(|x| x.w.abs() <= 0.010 + 1e-9 && x.h.abs() <= 0.015 + 1e-9));
}

#[test]
fn t15_sway_decay() {
    let s = running(70, F, |i| i <= 60);
    let amps: Vec<f64> = (61..=67).map(|f| s[f - 1].amp).collect();
    for (got, want) in amps.iter().zip([10.0, 8.33333, 6.66667, 5.0, 2.5, 0.66987, 0.0]) {
        assert!((got - want).abs() < 1e-5, "{amps:?}");
    }
    assert!(near(s[66].phase, 3.246312), "{:?}", s[66]);
    assert_eq!(s[67].phase, 0.0);
    assert!(near(s[63].w, 0.0049114) && near(s[63].h, -0.0046586), "{:?}", s[63]);
    assert!(s[66..].iter().all(|x| x.w == 0.0 && x.h == 0.0));
}

#[test]
fn t16_walk_and_crouch_pace() {
    let mut s = Sway::default();
    let mut out = Vec::new();
    for i in 1..=7 {
        s.step(&SwayInputs {
            dt: F,
            clock: i as f64 * F,
            active: true,
            run: false,
            moving: true,
            speed: 100.0,
            weapon: true,
            scale: 1.0,
            ..default()
        });
        out.push(s);
    }
    assert!(near(out[3].phase - out[2].phase, 0.0628319));
    assert!(near(out[2].w, -0.0020823) && near(out[2].h, 0.00047), "{:?}", out[2]);
    assert!(near(out[6].amp, 10.0) && near(out[6].phase, 0.314159));
    assert!(near(out[6].w, -0.0066913) && near(out[6].h, 0.0088168), "{:?}", out[6]);
}

#[test]
fn t17_ramp_at_100_fps() {
    let s = running(11, 0.01, |_| true);
    let want = [0.24472, 0.95492, 2.06107, 3.45492, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    for (f, w) in (2..=11).zip(want) {
        assert!((s[f - 1].amp - w).abs() < 1e-4, "frame {f}: {}", s[f - 1].amp);
    }
}

#[test]
fn t18_frame_rate_gate() {
    let run = |dt: f64, n: usize, s: &mut Sway| {
        for i in 0..n {
            s.step(&SwayInputs {
                dt,
                clock: i as f64 * dt,
                active: true,
                run: true,
                moving: true,
                speed: 20.0,
                weapon: true,
                scale: 1.0,
                ..default()
            });
        }
    };
    let mut s = Sway::default();
    run(F, 30, &mut s);
    assert_eq!(s.amp, 10.0, "60 fps: 0.333 u a frame");
    // 250 fps: 0.08 u a frame, decays within 0.1 s and stays at 0.
    let mut slow = s;
    let mut t = 0.0;
    let dt = 1.0 / 250.0;
    let mut zero_at = None;
    for i in 0..100 {
        t += dt;
        slow.step(&SwayInputs {
            dt,
            clock: 10.0 + t,
            active: true,
            run: true,
            moving: true,
            speed: 20.0,
            weapon: true,
            scale: 1.0,
            ..default()
        });
        if slow.amp == 0.0 && zero_at.is_none() {
            zero_at = Some(i);
        }
        if zero_at.is_some() {
            assert_eq!(slow.amp, 0.0);
        }
    }
    // Counted from the first frame not moving (d = 0 there).
    assert!(zero_at.is_some_and(|i| i as f64 * dt <= 0.1 + 1e-9), "{zero_at:?}");
}

#[test]
fn t19_airborne_freeze() {
    let mut s = running(60, F, |_| true)[59];
    let held = s;
    for i in 0..20 {
        s.step(&SwayInputs {
            dt: F,
            clock: 2.0 + i as f64 * F,
            active: false,
            run: true,
            moving: true,
            speed: 100.0,
            weapon: true,
            scale: 1.0,
            ..default()
        });
        assert_eq!(s, held);
    }
    s.step(&SwayInputs {
        dt: F,
        clock: 3.0,
        active: true,
        run: true,
        moving: true,
        speed: 100.0,
        weapon: true,
        scale: 1.0,
        ..default()
    });
    assert!(near(s.phase, held.phase + std::f64::consts::TAU * 1.4 * F));
    assert_eq!(s.amp, 10.0);
}

#[test]
fn t20_sway_scale() {
    let at = |scale: f64| {
        let mut s = Sway::default();
        for i in 1..=30 {
            s.step(&SwayInputs {
                dt: F,
                clock: i as f64 * F,
                active: true,
                run: true,
                moving: true,
                speed: 100.0,
                weapon: true,
                scale,
                ..default()
            });
        }
        s
    };
    let (full, half, none) = (at(1.0), at(0.5), at(0.0));
    assert!(near(half.w, full.w / 2.0) && near(half.h, full.h / 2.0));
    assert_eq!((none.w, none.h), (0.0, 0.0));
}

// Placement, flash, FOV (4, 5, 9, 10)

#[test]
fn t04_rest_placement() {
    let pos = Vec3::new(-0.1, -0.2, 0.7);
    assert_eq!(place::model_origin(pos, (0.0, 0.0), 0.0), pos);
}

fn facing_z() -> CameraAxes {
    CameraAxes {
        right: Vec3::X,
        up: Vec3::Y,
        forward: Vec3::Z,
    }
}

#[test]
fn t09_world_flash_point() {
    let o = place::flash_offset(
        Vec3::new(-0.1, -0.2, 0.7),
        Vec3::new(2.84, -2.4, 8.559921),
        (0.0, 0.0),
        Vec3::ZERO,
    );
    let f = place::flash_point(Vec3::ZERO, &facing_z(), o);
    assert!((f - Vec3::new(2.74, -2.6, 9.259921)).length() < 1e-4, "{f}");
    // Yawed 90 degrees right: forward +X, right -Z.
    let yawed = CameraAxes {
        right: Vec3::NEG_Z,
        up: Vec3::Y,
        forward: Vec3::X,
    };
    let f = place::flash_point(Vec3::ZERO, &yawed, o);
    assert!((f - Vec3::new(9.259921, -2.6, -2.74)).length() < 1e-4, "{f}");
}

#[test]
fn t10_shell_start() {
    let pos = Vec3::new(0.3, -0.6, 1.15);
    let muzzle = Vec3::new(4.4, -3.55, 7.75);
    let f = place::flash_point(Vec3::ZERO, &facing_z(), place::flash_offset(pos, muzzle, (0.0, 0.0), Vec3::ZERO));
    let m16 = place::breach_point(f, &facing_z(), Vec3::ONE);
    let m4 = place::breach_point(f, &facing_z(), Vec3::ZERO);
    assert!((m16 - Vec3::new(5.7, -3.15, 9.9)).length() < 1e-4, "{m16}");
    assert!((m4 - Vec3::new(4.7, -4.15, 8.9)).length() < 1e-4, "{m4}");
}

#[test]
fn t11_world_and_view_model_fov() {
    for (w, h, world, pv) in [
        (1024.0, 768.0, 91.309, 63.742),
        (1920.0, 1080.0, 107.512, 79.317),
        (1680.0, 1050.0, 101.673, 73.453),
        (1280.0, 1024.0, 87.612, 60.474),
    ] {
        let a = w / h;
        assert!((place::fov_x(75.0, a) - world).abs() < 1e-3, "{w}x{h}");
        assert!((place::fov_x(50.0, a) - pv).abs() < 1e-3, "{w}x{h}");
        // Chimera's view-model camera from our 4:3 settings agrees.
        assert!((aspect_fov(VIEWMODEL_FOV, a) - pv).abs() < 0.01, "{w}x{h}");
    }
    assert!((vertical_fov(VIEWMODEL_FOV) - 50.0).abs() < 0.01);
}

#[test]
fn t21_shot_jiggle() {
    let (pos, muzzle) = (Vec3::new(-0.1, -0.2, 0.7), Vec3::new(2.84, -2.4, 8.559921));
    let recoil = Vec3::new(0.1, 0.2, 0.3);
    // Rolls giving the draws (0.05, -0.1, 0.2).
    let d = place::jiggle_draws(recoil, [0.75, 0.25, 0.833_333_3]);
    assert!((d - Vec3::new(0.05, -0.1, 0.2)).length() < 1e-6, "{d}");
    let j = d.x + d.y + d.z;
    assert!((place::model_origin(pos, (0.0, 0.0), j) - (pos + Vec3::splat(0.15))).length() < 1e-6);
    assert!((place::flash_offset(pos, muzzle, (0.0, 0.0), d) - (pos + muzzle + d)).length() < 1e-6);
    // The next frame, no shot: back.
    assert_eq!(place::model_origin(pos, (0.0, 0.0), 0.0), pos);
    assert_eq!(place::flash_offset(pos, muzzle, (0.0, 0.0), Vec3::ZERO), pos + muzzle);
}

#[test]
fn t31_muzzle_flash_visibility() {
    let mut t = FlashTimer::default();
    let shot = 10.0;
    t.shot(shot);
    let shown: Vec<bool> = (0..6)
        .map(|i| t.update(shot + i as f64 * F, place::FLASH_TIME, true, true, false))
        .collect();
    assert_eq!(shown, [true, true, true, false, false, false]);
    // Zoomed (the model hidden): never.
    let mut t = FlashTimer::default();
    t.shot(shot);
    assert!((0..3).all(|i| !t.update(shot + i as f64 * F, place::FLASH_TIME, false, true, false)));
}

// Hands (1, 2)

#[test]
fn t03_sleeve_texture() {
    let table = hands::builtin();
    let pick = |face, uniform| {
        hands::choose(&table, &HandProfile {
            face,
            uniform,
            character: true,
            ..default()
        })
        .unwrap()
    };
    let c = pick(0, 0);
    assert_eq!(c.model, "chars_m_hand/normal_m_hand.ltb");
    assert_eq!(c.skins[0], "chars_t_hand/hand_00_00.dtx");
    assert_eq!(pick(2, 3).skins[0], "chars_t_hand/hand_02_03.dtx");
    // No character yet: the record's own skins.
    let c = hands::choose(&table, &HandProfile::default()).unwrap();
    assert_eq!(c.skins, vec!["chars_t_hand/hand_00_00.dtx".to_string()]);
    // Hand index -1: no arms; tattoo slots blanked.
    let none = hands::HandTable {
        models: vec![hands::CharacterHands {
            hand_index: -1,
            ..default()
        }],
        ..table.clone()
    };
    assert!(hands::choose(&none, &HandProfile::default()).is_none());
    let tattoo = hands::HandTable {
        hands: vec![hands::HandRecord {
            model: "h.ltb".into(),
            skins: vec!["a.dtx".into(), "PV_L_ARM".into(), "PV_R_ARM".into()],
            ..default()
        }],
        ..table
    };
    let c = hands::choose(&tattoo, &HandProfile::default()).unwrap();
    assert_eq!(c.skins[1..], [hands::NO_TATTOO.to_string(), hands::NO_TATTOO.to_string()]);
}

fn bone(name: &str, parent: Option<usize>, position: Vec3) -> MapBone {
    MapBone {
        name: name.into(),
        parent,
        position,
        rotation: Quat::IDENTITY,
    }
}

/// Case 1 on stand-ins: the hand's bones find the weapon's nodes by name
/// (case ignored), skipping the gun's own nodes between the two arms; its
/// vertices follow into the weapon's bind pose.
#[test]
fn t01_hand_to_weapon_bone_match() {
    let mut weapon = view_model("w", &[]);
    weapon.bones = vec![
        bone("Scene Root", None, Vec3::ZERO),
        bone("Bip01 R Hand", Some(0), Vec3::new(1.0, 0.0, 0.0)),
        bone("Box01", Some(1), Vec3::ZERO),
        bone("Bip01 L Hand", Some(0), Vec3::new(-1.0, 0.0, 0.0)),
    ];
    weapon.model.meshes.push(MapMesh::default());
    let hand_bones = vec![
        bone("scene root", None, Vec3::ZERO),
        bone("BIP01 R HAND", Some(0), Vec3::new(1.0, 0.0, 0.0)),
        // The left hand's bind differs: its vertices move with it.
        bone("Bip01 L Hand", Some(0), Vec3::new(-2.0, 0.0, 0.0)),
    ];
    let mesh = MapMesh {
        positions: vec![[0.1, 0.0, 0.0], [-0.2, 0.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 2],
        joints: vec![[1, 0, 0, 0], [2, 0, 0, 0]],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 2],
        ..default()
    };
    let map = hands::attach(&mut weapon, vec![mesh], &hand_bones, 0.1, Vec3::new(0.0, 0.0, -1.0)).unwrap();
    assert_eq!(map, vec![0, 1, 3]);
    let m = weapon.model.meshes.last().unwrap();
    assert_eq!(weapon.model.meshes.len(), 2, "the weapon's piece and the hand's");
    assert_eq!(m.joints, vec![[1, 0, 0, 0], [3, 0, 0, 0]]);
    let p: Vec<Vec3> = m.positions.iter().map(|p| Vec3::from(*p)).collect();
    assert!((p[0] - Vec3::new(0.1, 0.0, -1.0)).length() < 1e-6, "{p:?}");
    assert!((p[1] - Vec3::new(-0.1, 0.0, -1.0)).length() < 1e-6, "{p:?}");
    // A hand bone the weapon lacks.
    let mut bad = hand_bones;
    bad.push(bone("Bip01 Tail", Some(0), Vec3::ZERO));
    assert!(hands::attach(&mut weapon, Vec::new(), &bad, 0.1, Vec3::ZERO).is_err());
}

// In play, on the greybox

const M92FS: &str = "combat_arms:beretta_m92fs";
const R870: &str = "combat_arms:remington_870";
const ZOOMING: &str = "test:m92fs_zoom";
const M67: &str = "combat_arms:m67";

/// The M92FS with a sight (no such stock weapon is unkeyed).
const ZOOM_DOC: &str = r#"
id = "test:m92fs_zoom"
base = "combat_arms:beretta_m92fs"
[hooks."combat_arms:zoom"]
zoom_time = 0.13
[hooks."combat_arms:attachments"]
accepts = ["DotSight_A_Pistol"]
defaults = { sight = "DotSight_A_Pistol" }
"#;

/// A stand-in view model: one bone with a `flash` socket 1 unit ahead,
/// each animation `AnimInfo` played exactly (two frames over its length),
/// its keys as events.
pub(super) fn view_model(key: &str, anims: &[AnimInfo]) -> MapViewModel {
    let defaults = vec![(Quat::IDENTITY, Vec3::ZERO)];
    let mut set = AnimSet {
        bases: vec![defaults.clone()],
        defaults,
        ..default()
    };
    for (i, info) in anims.iter().enumerate() {
        let secs = (info.length as f32 / 1000.0).max(1e-3);
        set.animations.push(Animation {
            name: info.name.clone(),
            fps: 1.0 / secs,
            frames: 2,
            ..default()
        });
        set.sequences.push(Sequence {
            name: info.name.clone(),
            snap: true,
            grid: (1, 1),
            anims: vec![i],
            bone_weights: vec![1.0],
            events: info
                .keys
                .iter()
                .map(|(t, s)| AnimEvent {
                    cycle: if info.length > 0 { *t as f32 / info.length as f32 } else { 0.0 },
                    name: s.clone(),
                    ..default()
                })
                .collect(),
            ..default()
        });
    }
    MapViewModel {
        key: key.to_string(),
        model: MapModel::default(),
        bones: vec![bone("Scene Root", None, Vec3::ZERO)],
        root: Transform::from_scale(Vec3::splat(VIEW_SCALE)).with_translation(Vec3::new(-0.01, -0.02, -0.07)),
        animations: Some(Arc::new(set)),
        right_handed: true,
        allow_flipping: false,
        attachments: vec![MapAttachment {
            name: "flash".into(),
            bone: 0,
            local: Transform::from_xyz(0.0, 0.0, -1.0),
        }],
        light_origin: Vec3::ZERO,
    }
}

fn sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.add_weapon_data(&[("zoom", ZOOM_DOC)]).unwrap();
    let pistol: Vec<AnimInfo> = m92fs().iter().cloned().collect();
    let shotgun: Vec<AnimInfo> = r870().iter().cloned().collect();
    let grenade: Vec<AnimInfo> = m67().iter().cloned().collect();
    sim.app.insert_resource(ViewModels(Arc::new(vec![
        view_model(M92FS, &pistol),
        view_model(ZOOMING, &pistol),
        view_model(R870, &shotgun),
        view_model(M67, &grenade),
    ])));
    sim
}

fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(Vec3::new(30.0, 1.0, 30.0), movement::ID);
    sim.seconds(0.5);
    sim.app
        .world_mut()
        .entity_mut(p)
        .insert(Health { current: 1.0, max: 1.0 });
    p
}

fn playing(sim: &Sim, p: Entity) -> Option<String> {
    let vm = sim.app.world().get::<CaViewModel>(p)?;
    vm.machine.as_ref()?.playing().map(str::to_string)
}

/// The sequence the drawn view model shows.
fn shown(sim: &Sim, p: Entity) -> Option<String> {
    sim.app.world().get::<ViewAnimator>(p)?.sequence().map(str::to_string)
}

fn vm(sim: &Sim, p: Entity) -> &CaViewModel {
    sim.app.world().get::<CaViewModel>(p).unwrap()
}

/// Ticks until `f`, at most `n`; whether it happened.
fn until(sim: &mut Sim, n: u32, f: impl Fn(&Sim) -> bool) -> bool {
    for _ in 0..n {
        if f(sim) {
            return true;
        }
        sim.ticks(1);
    }
    f(sim)
}

fn drawn(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).unwrap();
    sim.ticks(3);
    assert_eq!(shown(sim, p).as_deref(), Some("select"));
    assert!(until(sim, 100, |s| shown(s, p).as_deref() == Some("idle_0")));
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    w
}

/// A view model that arrives after its weapon was drawn (loaded on
/// demand: `view_model::on_demand`, through `map::add_view_model`) plays
/// from its arrival: drawn afresh, `select` then idle.
#[test]
fn a_view_model_loaded_later_plays_from_its_arrival() {
    let mut sim = sim();
    sim.app.world_mut().remove_resource::<ViewModels>();
    let p = player(&mut sim);
    weapon::give(sim.app.world_mut(), p, M92FS).unwrap();
    sim.ticks(5);
    assert_eq!(shown(&sim, p), None, "nothing to draw yet");
    let pistol: Vec<AnimInfo> = m92fs().iter().cloned().collect();
    chimera_world::map::add_view_model(sim.app.world_mut(), view_model(M92FS, &pistol), Arc::new(Vec::new()));
    sim.ticks(1);
    assert_eq!(shown(&sim, p).as_deref(), Some("select"));
    assert!(until(&mut sim, 100, |s| shown(s, p).as_deref() == Some("idle_0")));
    assert!(sim.app.world().resource::<chimera_world::map::LateViewModelTextures>().0.contains_key(M92FS));
}

#[test]
fn plays_the_machine_in_game() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = drawn(&mut sim, p, M92FS);
    // A tap: `fire_0`, `postfire`, then idle.
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    assert!(until(&mut sim, 5, |s| shown(s, p).as_deref() == Some("fire_0")));
    assert!(until(&mut sim, 40, |s| shown(s, p).as_deref() == Some("postfire")));
    assert!(until(&mut sim, 40, |s| shown(s, p).as_deref() == Some("idle_0")));
    // An empty clip reloads once the fire sequence is over.
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 1;
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    assert!(until(&mut sim, 60, |s| playing(s, p).as_deref() == Some("reload")));
    assert!(until(&mut sim, 200, |s| playing(s, p).is_some_and(|n| n.starts_with("idle_"))));
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 15);
    // Its rest placement: identity rotation, no sway standing still.
    let o = sim.app.world().get::<ViewModelOffset>(p).unwrap();
    assert_eq!((o.translation, o.rotation), (Vec3::ZERO, Quat::IDENTITY));
}

#[test]
fn key_events_and_flash_and_shells() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    #[derive(Resource, Default)]
    struct Keys(Vec<ViewKey>);
    sim.app.init_resource::<Keys>();
    sim.app.add_systems(Update, |mut r: MessageReader<CaViewKey>, mut k: ResMut<Keys>| {
        k.0.extend(r.read().map(|e| e.key.clone()));
    });
    drawn(&mut sim, p, M92FS);
    assert!(sim.app.world().resource::<Keys>().0.contains(&ViewKey::Sound(3)), "the select's sound key");
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    assert!(until(&mut sim, 3, |s| vm(s, p).flash.shown));
    let v = vm(&sim, p);
    // The flash at the stand-in's socket + 0.5 u ahead (view-model space).
    let at = v.flash_at.expect("flash place");
    assert!(at.z < -0.07, "{at}");
    let flash_point = v.flash_point;
    assert!(until(&mut sim, 8, |s| !vm(s, p).flash.shown), "50 ms");
    let shells: Vec<(CaShell, Transform)> = sim
        .app
        .world_mut()
        .query::<(&CaShell, &Transform)>()
        .iter(sim.app.world())
        .map(|(s, t)| (s.clone(), *t))
        .collect();
    assert_eq!(shells.len(), 1);
    assert!(shells[0].1.translation.distance(flash_point) < 1.0);
    assert!(sim.app.world().resource::<Keys>().0.contains(&ViewKey::Fire));
    sim.seconds(3.5);
    assert_eq!(sim.app.world_mut().query::<&CaShell>().iter(sim.app.world()).count(), 0, "gone after 3 s");
}

#[test]
fn sprint_pose_in_game() {
    let mut sim = sim();
    let p = player(&mut sim);
    drawn(&mut sim, p, M92FS);
    {
        let mut i = sim.intent(p);
        i.sprint = true;
        i.move_axis = Vec2::Y;
    }
    assert!(until(&mut sim, 40, |s| shown(s, p).as_deref() == Some("PreRush")));
    assert!(until(&mut sim, 40, |s| shown(s, p).as_deref() == Some("rush")));
    // Running: the sway moves the model, within its small bounds.
    sim.seconds(0.5);
    let o = sim.app.world().get::<ViewModelOffset>(p).unwrap().translation / VIEW_SCALE;
    assert!(o.length() > 0.0 && o.x.abs() <= 0.0101 && o.y.abs() <= 0.0151, "{o}");
    {
        let mut i = sim.intent(p);
        i.sprint = false;
        i.move_axis = Vec2::ZERO;
    }
    assert!(until(&mut sim, 40, |s| shown(s, p).as_deref() == Some("PostRush")));
    assert!(until(&mut sim, 60, |s| shown(s, p).as_deref() == Some("idle_1")));
}

#[test]
fn shotgun_reloads_shell_by_shell() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = drawn(&mut sim, p, R870);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 5;
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..400 {
        sim.ticks(1);
        if let Some(s) = playing(&sim, p)
            && seen.last() != Some(&s)
        {
            seen.push(s);
        }
    }
    let at = |n: &str| seen.iter().position(|s| s == n);
    let (start, lp, end) = (at("PreReload"), at("reload"), at("PostReload"));
    assert!(start < lp && lp < end && start.is_some(), "{seen:?}");
    assert_eq!(seen.last().map(String::as_str), Some("idle_0"));
}

#[test]
fn t12_zoom_hides_and_leaves_the_fov() {
    let mut sim = sim();
    let p = player(&mut sim);
    drawn(&mut sim, p, ZOOMING);
    let hidden = |s: &Sim| s.app.world().get::<ViewAnimator>(p).unwrap().hidden;
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    assert!(hidden(&sim), "hidden from the press");
    sim.seconds(0.5);
    assert!(hidden(&sim));
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    assert!(hidden(&sim), "until the zoom-out ends");
    assert!(until(&mut sim, 60, |s| !hidden(s)));
    // Shown again unzoomed: the view model's vertical FOV is 50 degrees.
    let fov = view_model_fov(&settings(), DEFAULT_FOV);
    assert!((vertical_fov(fov) - 50.0).abs() < 0.01);
}

#[test]
fn one_frame_pop_in_game() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = drawn(&mut sim, p, M92FS);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 10;
    sim.ticks(2);
    sim.intent(p).reload = true;
    // The frame whose update starts `reload` still shows `idle_0`.
    assert!(until(&mut sim, 5, |s| playing(s, p).as_deref() == Some("reload")));
    sim.intent(p).reload = false;
    assert_eq!(shown(&sim, p).as_deref(), Some("idle_0"));
    sim.ticks(1);
    assert_eq!(shown(&sim, p).as_deref(), Some("reload"));
}

/// Grenade timings Q5: with grenades left, the throw's `postfire` is
/// followed by the 400 ms re-ready (`reload`), whose end fills the clip.
#[test]
fn grenade_re_ready_after_a_throw() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = drawn(&mut sim, p, M67);
    {
        let mut t = sim.app.world_mut().get_mut::<Throwable>(w).unwrap();
        t.count = 2;
        t.max = 2;
    }
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    assert!(until(&mut sim, 100, |s| playing(s, p).as_deref() == Some("postfire")));
    assert!(until(&mut sim, 60, |s| vm(s, p).thrown), "the grenade left the hand");
    assert!(until(&mut sim, 60, |s| playing(s, p).as_deref() == Some("reload")), "the re-ready");
    assert!(until(&mut sim, 60, |s| playing(s, p).is_some_and(|n| n.starts_with("idle_"))));
    assert!(!vm(&sim, p).thrown, "the clip filled");
    assert_eq!(sim.app.world().get::<Throwable>(w).unwrap().count, 1);
}

/// The keys a throw's view model sends.
#[derive(Resource, Default)]
struct Keys(Vec<(String, ViewKey)>);

/// A throw in play plays the grenade's view-model sequence, the pin pull
/// (`prefire`, its `sound_key 10` pin sound), the throw (`fire_0`, its
/// `THROW_KEY`) and the release (`postfire`, its `FIRE_KEY`), in that
/// order, each key once for a tap (grenades.md 3; view_models.md 8).
#[test]
fn a_throw_plays_pin_pull_throw_and_release() {
    let mut sim = sim();
    sim.app.init_resource::<Keys>().add_systems(
        Update,
        |mut r: MessageReader<CaViewKey>, mut k: ResMut<Keys>, vms: Query<&CaViewModel>| {
            for m in r.read() {
                let at = vms.get(m.owner).ok().and_then(|v| v.machine.as_ref()?.playing().map(str::to_string));
                k.0.push((at.unwrap_or_default(), m.key.clone()));
            }
        },
    );
    let p = player(&mut sim);
    drawn(&mut sim, p, M67);
    sim.app.world_mut().resource_mut::<Keys>().0.clear();
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..150 {
        if let Some(n) = playing(&sim, p)
            && seen.last() != Some(&n)
        {
            seen.push(n);
        }
        if seen.iter().any(|n| n == "postfire") && seen.last().is_some_and(|n| n != "postfire") {
            break;
        }
        sim.ticks(1);
    }
    let at = |name: &str| seen.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} in {seen:?}"));
    assert!(at("prefire") < at("fire_0") && at("fire_0") < at("postfire"), "{seen:?}");
    let keys: Vec<ViewKey> = sim.app.world().resource::<Keys>().0.iter().map(|(_, k)| k.clone()).collect();
    let pos = |k: ViewKey| keys.iter().position(|x| *x == k).unwrap_or_else(|| panic!("{k:?} in {keys:?}"));
    assert!(pos(ViewKey::Sound(10)) < pos(ViewKey::Throw) && pos(ViewKey::Throw) < pos(ViewKey::Fire), "{keys:?}");
    for k in [ViewKey::Sound(10), ViewKey::Throw, ViewKey::Fire] {
        assert_eq!(keys.iter().filter(|x| **x == k).count(), 1, "{k:?} once: {keys:?}");
    }
}

/// 9: no first-person flash with the camera in a liquid volume (the
/// M92FS has no `UseUWMuzzleFX`).
#[test]
fn no_muzzle_flash_under_water() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    drawn(&mut sim, p, M92FS);
    sim.app.world_mut().spawn(combat_arms::game::volumes::CaVolume::liquid_box(
        Vec3::splat(-1000.0),
        Vec3::splat(1000.0),
        0.0,
        Vec3::ZERO,
    ));
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    assert!(until(&mut sim, 5, |s| vm(s, p).flash.shot_at.is_some()), "it fired");
    for _ in 0..6 {
        assert!(!vm(&sim, p).flash.shown && vm(&sim, p).flash_at.is_none());
        sim.ticks(1);
    }
}

// ---------------------------------------------------------------------
// Follow-up cases (Open questions 9-16: cases 33-44).

/// 33. Full-automatic cadence: `fire_0` (105 ms) seen done after 7
/// advances at 60 fps (116.7 ms a round), 11 at 100 fps (110 ms).
#[test]
fn t33_full_automatic_cadence() {
    let mut r = Run::draw(m4a1(), setup(Class::Primary, 0, 30), 30, 120);
    r.settle();
    r.frames(40, FIRE);
    let at: Vec<u32> = r.shots.iter().map(|s| s.0).collect();
    assert!(at.len() >= 4, "{at:?}");
    assert!(at.windows(2).all(|w| w[1] - w[0] == 7), "every 7 updates: {at:?}");
    // 100 fps.
    let mut m = ViewMachine::draw(m4a1(), setup(Class::Primary, 0, 30), 7);
    let mut shots = Vec::new();
    let (mut clip, mut total) = (30, 120);
    for n in 0..400u32 {
        let fire = n >= 150;
        let out = m.update(&Inputs { clip, total, ..if fire { FIRE } else { NONE } });
        if out.shot.is_some() {
            shots.push(n);
            clip -= 1;
            total -= 1;
        }
        m.advance(0.01);
    }
    assert!(shots.windows(2).all(|w| w[1] - w[0] == 11), "every 11 updates at 100 fps: {shots:?}");
}

/// 34. Dry fire is unreachable: clip 0 with rounds left reloads; with
/// none, idles; never `dryFire`.
#[test]
fn t34_dry_fire_unreachable() {
    let mut r = Run::draw(m4a1(), setup(Class::Primary, 0, 30), 0, 30);
    r.frames(120, FIRE);
    assert!(!r.started("reload").is_empty(), "the automatic reload");
    assert!(r.started("dryFire").is_empty());
    let mut r = Run::draw(m92fs(), pistol(), 0, 0);
    r.settle();
    r.frames(120, FIRE);
    assert!(r.started("dryFire").is_empty() && r.shots.is_empty());
}

/// 35. Holster: no `deselect` in the stock files (an instant switch); a
/// file with a 400 ms one plays it at 1.0 whatever the draw rate, the
/// switch waiting for it: done after 401 ms of advance (update 25 at
/// 60 fps).
#[test]
fn t35_holster() {
    let mut m = ViewMachine::draw(m4a1(), setup(Class::Primary, 0, 30), 7);
    assert!(!m.holster(), "none in the stock files");
    let mut anims = (*m4a1()).clone();
    anims.push(a("deselect", 400, &[]));
    let mut s = setup(Class::Primary, 0, 30);
    s.rates.draw = 1.5;
    let mut m = ViewMachine::draw(Arc::new(anims), s, 7);
    assert!(m.holster());
    let mut n = 0;
    while !m.holstered() {
        m.update(&Inputs { clip: 30, total: 120, ..NONE });
        m.advance(F);
        n += 1;
        assert!(n < 100);
    }
    assert_eq!(n, 25, "seen done on update 25 (416.7 ms)");
}

/// 36. The double barrel's `reload_1` with a round left; `reload` empty;
/// other guns always `reload`.
#[test]
fn t36_reload_one() {
    let anims = || {
        Arc::new(vec![
            a("select", 500, &[]),
            a("idle_0", 2000, &[]),
            a("fire_0", 300, &[(0, "FIRE_KEY")]),
            a("reload", 2000, &[]),
            a("reload_1", 1200, &[]),
        ])
    };
    let db = || Setup {
        double_barrel: true,
        ..setup(Class::Shotgun, 1, 2)
    };
    for (s, clip, want) in [(db(), 1, "reload_1"), (db(), 0, "reload"), (setup(Class::Shotgun, 1, 2), 1, "reload")] {
        let mut r = Run::draw(anims(), s, clip, 10);
        r.frames(60, NONE);
        r.frames(3, RELOAD);
        assert_eq!(r.started("reload_1").is_empty(), want != "reload_1", "clip {clip}: {:?}", r.starts);
        assert!(!r.started(want).is_empty(), "clip {clip}: {:?}", r.starts);
    }
}

/// 37. The projectile piece: shown at a draw with a round, hidden on the
/// shot; the hidden pieces hide at the fire sequence's end with nothing
/// left; a draw empty shows neither.
#[test]
fn t37_projectile_piece() {
    let launcher = || {
        Arc::new(vec![
            a("select", 500, &[]),
            a("idle_0", 2000, &[]),
            a("fire_0", 300, &[(0, "FIRE_KEY")]),
            a("reload", 2000, &[]),
        ])
    };
    let s = Setup {
        ammo_max: 1,
        ..setup(Class::Special, 1, 1)
    };
    let mut r = Run::draw(launcher(), s, 1, 1);
    r.m.draw_ammo(1, 1);
    assert!(r.m.pieces.projectile && !r.m.pieces.hidden_list);
    r.settle();
    r.frames(2, FIRE);
    r.frames(60, NONE);
    assert_eq!(r.shots.len(), 1);
    assert!(!r.m.pieces.projectile, "hidden on the shot");
    assert!(r.m.pieces.hidden_list, "total 0 at the fire sequence's end");
    let mut again = ViewMachine::draw(launcher(), s, 7);
    again.draw_ammo(0, 0);
    assert!(!again.pieces.projectile && again.pieces.hidden_list);
}

/// 38. A hand record's colour variant replaces its whole skin list (skin
/// 5 of the ghillie hands for model colour 1); colour 0: the plain list.
#[test]
fn t38_hand_colour() {
    let mut table = hands::builtin();
    let mut ghillie = table.hands[0].clone();
    let mut coloured = ghillie.skins.clone();
    coloured.resize(12, String::new());
    coloured[5] = "CHARS_T_BODY\\CM_Fur_Ghillie_Suit_DE_SD.DTX".into();
    ghillie.colour_skins.insert(1, coloured);
    table.hands[0] = ghillie;
    let pick = |colour: u32| {
        hands::choose(&table, &HandProfile {
            colour,
            ..HandProfile::default()
        })
        .unwrap()
    };
    assert_eq!(pick(1).skins.get(5).map(String::as_str), Some("CHARS_T_BODY\\CM_Fur_Ghillie_Suit_DE_SD.DTX"));
    assert_ne!(pick(0).skins.get(5).map(String::as_str), Some("CHARS_T_BODY\\CM_Fur_Ghillie_Suit_DE_SD.DTX"));
}

/// 40. Projection: the world's horizontal FOV from 75° vertical at the
/// aspect of the last camera reset; the view model's from 50° at every
/// draw's aspect.
#[test]
fn t40_projection() {
    let deg = |a: f32, fov: f32| place::fov_x(fov, a);
    assert!((deg(16.0 / 9.0, 75.0) - 107.51).abs() < 0.01);
    assert!((deg(16.0 / 9.0, 50.0) - 79.32).abs() < 0.01);
    assert!((deg(1.25, 50.0) - 60.47).abs() < 0.01);
    assert!((deg(1.25, 75.0) - 87.61).abs() < 0.01);
    let mut fov = place::WorldFov::reset(16.0 / 9.0);
    fov.resize(1.25);
    assert!((fov.fov_x - 107.51).abs() < 0.01, "kept until the next world start");
    fov = place::WorldFov::reset(1.25);
    assert!((fov.fov_x - 87.61).abs() < 0.01);
}

/// 41. The footing ray: 140 u above a floor can step, 160 u can't.
#[test]
fn t41_footing_ray() {
    use combat_arms::game::view_model::sway::can_step;
    assert!(can_step(false, false, false, false, Some(140.0)));
    assert!(!can_step(false, false, false, false, Some(160.0)));
    assert!(!can_step(false, false, false, false, None));
    assert!(can_step(false, false, true, true, None), "on the ground");
    assert!(!can_step(false, false, false, true, Some(10.0)), "jumped");
    assert!(!can_step(true, true, true, false, Some(1.0)), "on sky");
}

/// 42. A spectated gun sways by its keys alone (the distance test always
/// passes): full amplitude 0.1 s after the key, 1.4 cycles a second;
/// without a key it decays to 0 in 0.1 s.
#[test]
fn t42_spectated_sway() {
    let mut s = Sway::default();
    let step = |s: &mut Sway, i: usize, moving: bool| {
        s.step(&SwayInputs {
            dt: F,
            clock: i as f64 * F,
            active: true,
            run: true,
            moving,
            speed: 0.0,
            spectating: true,
            weapon: true,
            zoomed: false,
            scale: 1.0,
        });
    };
    for i in 1..=7 {
        step(&mut s, i, true);
    }
    assert_eq!(s.amp, 10.0, "full after 0.1 s against a wall");
    for i in 8..=14 {
        step(&mut s, i, false);
    }
    assert_eq!(s.amp, 0.0);
}

/// 44. Attached effects: created hidden, shown with the model; a key
/// hides effect 1 and it stays hidden through a scope until a show key.
#[test]
fn t44_attached_effects() {
    let anims = Arc::new(vec![
        a("select", 500, &[]),
        a("idle_0", 2000, &[]),
        a("fire_0", 300, &[(0, "HIDE_PVATTACHFX 1"), (100, "FIRE_KEY")]),
        a("postfire", 200, &[(50, "SHOW_PVATTACHFX 1")]),
    ]);
    let s = Setup {
        attach_fx: 5,
        ..setup(Class::Special, 1, 1)
    };
    let mut r = Run::draw(anims, s, 1, 1);
    assert_eq!(r.m.pieces.fx_shown, vec![false; 5], "created hidden");
    r.m.set_visible(true);
    assert_eq!(r.m.pieces.fx_shown, vec![true; 5]);
    r.settle();
    r.frames(2, FIRE);
    r.frames(6, NONE);
    assert!(!r.m.pieces.fx_shown[1], "hidden by the key");
    r.m.set_visible(false);
    r.m.set_visible(true);
    assert!(!r.m.pieces.fx_shown[1], "still hidden after a scope");
    r.frames(30, NONE);
    assert!(r.m.pieces.fx_shown[1], "shown by the key");
}

/// Emptying the magazine and reloading plays the reload animation once
/// (the weapon's auto-reload; the view model must not ask again before
/// the clip is filled), fire released or held: one reload start in the
/// weapon, one `reload` in the view model, a full clip, idle.
fn empty_then_reload_once(held: bool, lag: f32) {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = drawn(&mut sim, p, M92FS);
    let full = sim.app.world().get::<Magazine>(w).unwrap().clip;
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 1;
        m.reserve = 60;
        // The weapon's clip fills later than the animation ends (a tick's
        // rounding, or a rate the animation does not follow).
        m.reload_time += lag;
    }
    sim.intent(p).fire = true;
    let (mut plays, mut starts) = (0, 0);
    let (mut last, mut last_end): (Option<bool>, Option<f64>) = (None, None);
    for n in 0..600 {
        if n == 3 && !held {
            sim.intent(p).fire = false;
        }
        sim.ticks(1);
        // The reload stage entered (the animation `reload` can still be the
        // one playing on the update that asks for the second).
        let now = vm(&sim, p).machine.as_ref().map(|m| matches!(m.stage, Stage::Reloading(_)));
        if now == Some(true) && last != Some(true) {
            plays += 1;
        }
        last = now;
        let end = sim.app.world().get::<weapon::WeaponState>(w).unwrap().reload_end;
        if end.is_some() && last_end.is_none() {
            starts += 1;
        }
        last_end = end;
        if held && n == 300 {
            sim.intent(p).fire = false;
        }
    }
    assert_eq!((plays, starts), (1, 1), "held {held}: view reload plays, weapon reload starts");
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    assert!(m.clip > 0, "refilled");
    let _ = full;
    assert!(playing(&sim, p).is_some_and(|n| n.starts_with("idle_")), "back to idle: {:?}", playing(&sim, p));
}

#[test]
fn an_emptied_magazine_reloads_once_fire_released() {
    empty_then_reload_once(false, 0.0);
}

#[test]
fn an_emptied_magazine_reloads_once_fire_held() {
    empty_then_reload_once(true, 0.0);
}

/// The clip fills after the animation ends (the weapon's reload is longer
/// than the animation): no second reload animation for the stale empty clip.
#[test]
fn an_emptied_magazine_reloads_once_when_the_clip_fills_late() {
    for lag in [0.05, 0.3] {
        empty_then_reload_once(false, lag);
        empty_then_reload_once(true, lag);
    }
}

// ---------------------------------------------------------------------
// Corrections (2026-10-22): view_models.md, one test each (`v01_` is
// correction 1).

/// An M4A1-like table with a 150 ms `postfire` and a `prefire` of
/// `pre` ms.
fn rifle_table(pre: u32) -> Arc<Vec<AnimInfo>> {
    Arc::new(vec![
        a("select", 1200, &[]),
        a("idle_0", 1833, &[]),
        a("idle_1", 1833, &[]),
        a("prefire", pre, &[]),
        a("fire_0", 105, &[(0, "FIRE_KEY")]),
        a("postfire", 150, &[]),
        a("reload", 2300, &[]),
        a("PreRush", 233, &[]),
        a("rush", 620, &[]),
        a("PostRush", 367, &[]),
    ])
}

/// Fire once and release; run until `postfire` plays.
fn into_postfire(r: &mut Run) {
    r.frame(FIRE);
    for _ in 0..80 {
        if r.m.playing() == Some("postfire") {
            return;
        }
        r.frame(NONE);
    }
    panic!("no postfire: {:?}", r.starts);
}

/// 1. A new press 50 ms into `postfire` of a full-automatic gun refires on
/// the update that sees `postfire` done, with no idle between.
#[test]
fn v01_press_during_postfire_refires_on_the_same_update() {
    let mut r = Run::draw(rifle_table(0), setup(Class::Primary, 0, 30), 30, 90);
    r.settle();
    into_postfire(&mut r);
    let post = r.n - 1;
    r.frames(2, NONE); // 50 ms in with the one before
    r.frames(60, FIRE);
    // 150 ms + 1 over 16.67 ms frames: seen done on the 10th update.
    let again = r.started("prefire");
    assert_eq!(again.len() >= 2, true, "{:?}", r.starts);
    assert_eq!(again[1], post + 10, "{:?}", r.starts);
    assert!(r.started("idle_0").is_empty() || r.started("idle_0")[0] > post + 10, "{:?}", r.starts);
    // The full-automatic gun still waits while `postfire` plays: no
    // restart before it is done.
    assert!(!r.starts.iter().any(|(u, n)| n == "prefire" && *u > post && *u < post + 10));
}

/// 1 and 2. A semi-automatic pistol (no pre-fire) refires with `fire_0`
/// on the pressing update; a semi-automatic rifle with a `prefire` goes
/// through `prefire` first.
#[test]
fn v02_semi_automatic_cancel_goes_through_prefire() {
    let mut r = Run::draw(rifle_table(100), setup(Class::Primary, 1, 30), 30, 90);
    r.settle();
    into_postfire(&mut r);
    r.frames(2, NONE);
    assert_eq!(r.m.playing(), Some("postfire"));
    let press = r.n;
    r.frame(FIRE);
    assert_eq!(r.m.playing(), Some("prefire"), "on the pressing update: {:?}", r.starts);
    assert!(r.started("prefire").contains(&press), "{:?}", r.starts);
    r.frames(12, NONE);
    let fire_0 = r.started("fire_0");
    assert!(fire_0.last().is_some_and(|u| *u > press), "{:?}", r.starts);
    assert_eq!(r.shots.len(), 2, "{:?}", r.shots);
}

/// 3. Sprinting cuts a knife slash and stab at once: `PreRush` from that
/// update, no hit after the cut.
#[test]
fn v03_sprint_cuts_a_knife_swing() {
    let mut r = Run::draw(m9(), setup(Class::Melee, 0, 0), 1, 1);
    r.infinite = true;
    r.settle();
    r.frame(FIRE);
    r.frames(5, NONE); // 100 ms into the slash
    let at = r.n;
    r.frames(1, SPRINT);
    assert_eq!(r.started("PreRush"), vec![at], "{:?}", r.starts);
    r.frames(40, SPRINT);
    assert!(r.shots.is_empty(), "a cut slash hits nothing: {:?}", r.shots);
    // The stab, cut at 300 ms.
    let mut r = Run::draw(m9(), setup(Class::Melee, 0, 0), 1, 1);
    r.infinite = true;
    r.settle();
    r.frame(Inputs { alt_fire: true, ..NONE });
    r.frames(17, NONE);
    let at = r.n;
    r.frames(40, SPRINT);
    assert_eq!(r.started("PreRush").first(), Some(&at), "{:?}", r.starts);
    assert!(r.shots.is_empty(), "{:?}", r.shots);
    // A rifle's `fire_0` plays out under sprint, then `PreRush`.
    let mut r = Run::draw(rifle_table(0), setup(Class::Primary, 0, 30), 30, 90);
    r.settle();
    r.frames(3, FIRE);
    let fire = *r.started("fire_0").first().expect("fired");
    r.frames(30, SPRINT);
    let rush = *r.started("PreRush").first().expect("sprinted");
    assert!(rush >= fire + 7, "fire {fire}, PreRush {rush}: {:?}", r.starts);
}

/// 3. A double barrel caught in `fire_0` is cut too.
#[test]
fn v03_sprint_cuts_a_double_barrel() {
    let s = Setup { double_barrel: true, ..setup(Class::Shotgun, 1, 2) };
    let mut r = Run::draw(rifle_table(0), s, 2, 20);
    r.settle();
    r.frame(FIRE);
    r.frames(1, NONE);
    assert_eq!(r.m.playing(), Some("fire_0"));
    let at = r.n;
    r.frames(3, SPRINT);
    assert_eq!(r.started("PreRush").first(), Some(&at), "{:?}", r.starts);
}

/// 4. A grenade asked to fire but not yet in its pin pull goes to the
/// sprint pose and the request is dropped.
#[test]
fn v04_grenade_request_under_sprint_is_dropped() {
    let mut r = Run::draw(m67(), grenade(), 1, 2);
    r.settle();
    r.frame(FIRE); // state "firing", the idle still showing
    r.frame(Inputs { fire: true, ..SPRINT });
    assert_eq!(r.m.playing(), Some("PreRush"), "{:?}", r.starts);
    assert!(r.started("prefire").is_empty(), "{:?}", r.starts);
    r.frames(10, SPRINT);
    r.frames(1, NONE);
    assert_eq!(r.m.playing(), Some("PostRush"));
    r.frames(40, NONE);
    assert!(r.started("prefire").is_empty(), "no pin pull, ever");
    assert!(r.started("idle_1").len() + r.started("idle_0").len() >= 1);
}

/// 5. The full idle after `PostRush` starts from 0 (no pop).
#[test]
fn v05_post_sprint_full_idle_starts_from_zero() {
    let mut r = Run::draw(rifle_table(0), setup(Class::Primary, 0, 30), 30, 90);
    r.settle();
    r.frames(30, SPRINT);
    for _ in 0..60 {
        let before = r.m.playing().map(str::to_string);
        r.clip_total_update(NONE);
        if before.as_deref() == Some("PostRush") && r.m.playing() != Some("PostRush") {
            assert_eq!(r.m.playing(), Some("idle_1"));
            assert_eq!(r.m.shown().map(|(_, t)| t), Some(0.0), "idle_1 at t = 0, no pop");
            return;
        }
        r.m.advance(F);
    }
    panic!("PostRush never ended");
}

impl Run {
    /// One update only (no advance), for what is seen between the two.
    fn clip_total_update(&mut self, mut i: Inputs) {
        i.clip = self.clip;
        i.total = self.total;
        self.m.update(&i);
    }
}

/// 6. The fire rate includes the weapon's `FireAniRate`: 1.25 with 0.25,
/// a 105 ms `fire_0` taking 84 ms, one shot per 6 frames instead of 7;
/// the draw and reload sums add the character model's bonuses.
#[test]
fn v06_fire_animation_rate_includes_the_weapons_own() {
    use combat_arms::game::attachments::{animation_rate, fire_animation_rate};
    assert_eq!(fire_animation_rate(0.25, 0.0), animation_rate(1.25));
    assert!((fire_animation_rate(0.25, 0.0) - 1.25).abs() < 1e-6);
    assert!((fire_animation_rate(0.25, 0.25) - 1.5).abs() < 1e-6);
    assert!((fire_animation_rate(0.0, 0.0) - 1.0).abs() < 1e-6);
    assert_eq!(fire_animation_rate(1.5, 0.75), 1.0, "above 2.0 plays at 1.0");
    let cadence = |rate: f32| {
        let s = Setup {
            rates: Rates { fire: rate, ..Rates::default() },
            ..setup(Class::Primary, 0, 30)
        };
        let mut r = Run::draw(rifle_table(0), s, 30, 90);
        r.settle();
        r.frames(80, FIRE);
        let fires = r.started("fire_0");
        fires[2] - fires[1]
    };
    assert_eq!(cadence(1.0), 7);
    assert_eq!(cadence(fire_animation_rate(0.25, 0.0)), 6);
    // The vest's reload and select bonuses reach the machine's rates
    // through the same sums.
    let reload = animation_rate(1.0 + 0.2 + 0.1);
    let draw = animation_rate(1.0 + 0.0 + 0.3 + 0.1);
    assert!((reload - 1.3).abs() < 1e-6 && (draw - 1.4).abs() < 1e-6);
}

const M4A1: &str = "combat_arms:m4a1";

/// Frames of `frame` seconds (the fixed ticks keep the game's rate),
/// sampling the view model after each: emptying the clip of `id` with
/// fire held until the clip is full again or let go as it empties, starts one weapon reload and enters the view model's reload once.
fn one_reload_at_frame_rate(id: &str, anims: Arc<Vec<AnimInfo>>, frame: f64, held: bool) {
    let mut sim = sim();
    let pistol: Vec<AnimInfo> = m92fs().iter().cloned().collect();
    let rifle: Vec<AnimInfo> = anims.iter().cloned().collect();
    sim.app.insert_resource(ViewModels(Arc::new(vec![view_model(M92FS, &pistol), view_model(M4A1, &rifle)])));
    let p = player(&mut sim);
    let w = drawn(&mut sim, p, id);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 1;
        m.reserve = 60;
    }
    sim.app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(frame)));
    sim.intent(p).fire = true;
    let (mut plays, mut starts) = (0, 0);
    let (mut last, mut last_end) = (false, false);
    let (mut t, mut emptied) = (0.0, false);
    while t < 6.0 {
        let clip = sim.app.world().get::<Magazine>(w).unwrap().clip;
        emptied |= clip == 0;
        if emptied && (!held || clip > 0) {
            sim.intent(p).fire = false;
        }
        sim.app.update();
        t += frame;
        let now = vm(&sim, p).machine.as_ref().is_some_and(|m| matches!(m.stage, Stage::Reloading(_)));
        if now && !last {
            plays += 1;
        }
        last = now;
        let end = sim.app.world().get::<weapon::WeaponState>(w).unwrap().reloading();
        if end && !last_end {
            starts += 1;
        }
        last_end = end;
    }
    assert_eq!(
        (plays, starts),
        (1, 1),
        "{id} at {:.0} fps, held {held}: view reloads, weapon reloads",
        1.0 / frame
    );
    assert!(sim.app.world().get::<Magazine>(w).unwrap().clip > 0, "refilled");
}

/// The view model's own automatic reload can start a frame before the
/// weapon's (the M4A1's fire animation is as long as its cycle): the
/// weapon's reload starting meanwhile must not queue a second animation
/// for the still empty clip. At several frame rates (the race is the
/// frames' phase against the ticks), fire held and released.
#[test]
fn an_emptied_magazine_reloads_once_at_any_frame_rate() {
    for frame in [1.0 / 60.0, 1.0 / 75.0, 1.0 / 100.0, 1.0 / 144.0, 1.0 / 240.0, 1.0 / 37.0] {
        for held in [false, true] {
            one_reload_at_frame_rate(M4A1, m4a1(), frame, held);
            one_reload_at_frame_rate(M92FS, m92fs(), frame, held);
        }
    }
}

// ---------------------------------------------------------------------------
// Follow-up (2026-10-31b): view-model lighting and the player-view point.

/// 48. A dynamic light (radius 200) 20 u from a view-model point lights it
/// with 1 / (1 + 19·400/40000) ≈ 0.84 of its colour; 210 u away: none.
#[test]
fn vm48_light_on_the_gun() {
    let s = place::model_light_share(20.0, 200.0);
    assert!((s - 1.0 / 1.19).abs() < 1e-5, "{s}");
    assert_eq!(place::model_light_share(210.0, 200.0), 0.0);
    assert_eq!(place::model_light_share(200.0, 200.0), 0.0, "cut at the radius");
}

/// 49. The camera turned 90° with the light fixed in the world: the light
/// seen from the camera turns the other way (it isn't stuck to the camera).
#[test]
fn vm49_light_follows_the_camera_frame() {
    let light = Vec3::new(0.0, 0.0, -10.0);
    let ahead = place::light_in_camera(light, Vec3::ZERO, Quat::IDENTITY);
    let turned = place::light_in_camera(light, Vec3::ZERO, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    assert!(ahead.abs_diff_eq(Vec3::new(0.0, 0.0, -10.0), 1e-4));
    assert!(turned.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-4), "now to the right: {turned}");
}

/// 50 and 51. In the muzzle flash group a player-view key (mode 2) sits at
/// the flash point, its own offset not added: a light at the world flash
/// point F, a camera-relative particle at the camera-space offset o (e.g.
/// (6, -5, 30)); a socket-mode key is placed by its mode.
#[test]
fn vm50_vm51_player_view_keys_at_the_flash() {
    use combat_arms::game::view_model::attached::{FlashKeyPoint, PLAYER_VIEW, flash_key_point};
    let (o, f) = (Vec3::new(6.0, -5.0, 30.0), Vec3::new(2.74, -2.6, 9.259921));
    assert_eq!(flash_key_point(Some(PLAYER_VIEW), false, o, f), Some(FlashKeyPoint::World(f)), "the light");
    assert_eq!(flash_key_point(Some(PLAYER_VIEW), true, o, f), Some(FlashKeyPoint::Camera(o)), "the particle");
    assert_eq!(flash_key_point(Some(4), true, o, f), None, "a socket key: its socket");
    assert_eq!(flash_key_point(None, false, o, f), None);
}

/// 52. A player-view key in a `PVAttachFXName0` group stays at the view
/// model's place when the group was made + its offset, whatever the sway
/// does after; a fixed key follows the model.
#[test]
fn vm52_player_view_key_in_an_attached_group() {
    use combat_arms::game::view_model::attached::{PLAYER_VIEW, key_place};
    let model = view_model("t", &[]);
    let made = ViewModelOffset {
        translation: Vec3::new(0.01, 0.0, 0.0),
        rotation: Quat::IDENTITY,
    };
    let swayed = ViewModelOffset {
        translation: Vec3::new(0.05, 0.02, 0.0),
        rotation: Quat::from_rotation_z(0.1),
    };
    let off = Vec3::new(0.0, 0.0, 1.0);
    let at = |m: Mat4| m.transform_point3(Vec3::ZERO);
    let a = at(key_place(&model, &[], &made, None, off, Some(PLAYER_VIEW), &made));
    let b = at(key_place(&model, &[], &swayed, None, off, Some(PLAYER_VIEW), &made));
    assert!(a.abs_diff_eq(b, 1e-6), "it doesn't follow the sway: {a} {b}");
    let origin = made.translation + model.root.translation;
    assert!(a.abs_diff_eq(origin + Vec3::new(0.0, 0.0, -1.0), 1e-5), "the made place + its unrotated offset: {a}");
    let fixed_made = at(key_place(&model, &[], &made, None, off, Some(0), &made));
    let fixed_swayed = at(key_place(&model, &[], &swayed, None, off, Some(0), &made));
    assert!(!fixed_made.abs_diff_eq(fixed_swayed, 1e-4), "a fixed key follows the model");
}
