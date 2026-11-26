//! Counter-Strike 1.6's spent cases (specs/cs16/shells.md), data/shells.toml
//! per gun: each shot's event (`WeaponEventKind::Fired`, every machine that
//! shows it: the shooter's own predicted one and the others' replicated
//! ones) throws one case at the gun's first-person offset from the eye
//! (the local player's) or the third-person one (others'), flying out
//! sideways, up and forward at random speeds with half the shooter's
//! velocity, for 2.5 s (`chimera_world::map::ThrowShell`, drawn and
//! bounced by Chimera's shells, `render::shells`). The M3, the Scout and
//! the AWP throw theirs late (0.45, 0.56, 0.55 s after the shot), at the
//! view of that moment, 16 units ahead of the eye and 9 down.
//!
//! The pure parts (`event_throw`, `late_throw`: the spec's start and
//! velocity from the view) are unit tested on its test cases.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The late shells are made on every machine from the shot's event, not
//!   by the server's shell message; the view at the eject time is the
//!   owner's `Intent`, its eye the shot's.
//! - The sideways direction of the event shells follows the client
//!   reference (a right-handed case starts right and flies left) and the
//!   late shells' 9 units and 120 units/s go to the left of the view:
//!   shells.md's open questions 1 and 2.
//! - How the cases fly and bounce (gravity, damping, spin, which bounce
//!   sound, the fade before the 2.5 s end) is ours (open question 3):
//!   `physics`.
//! - The Elites' shot alternates by this machine's count of that
//!   shooter's shots, not by the gun's clip.
//! - The shooter's eye is the event's origin (the shooter's own view
//!   height, also for others' guns: the spec's 17 or 12 ducked).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_core::core::{Intent, LocalPlayer, Velocity};
use chimera_movement::{to_engine, to_units};
use chimera_sim::weapon::{WeaponEvent, WeaponEventKind, aim_of};
use chimera_world::map::{
    ViewModelSettings,
    shells::{MapShell, MapShellPhysics, ThrowShell},
};
use serde::Deserialize;

/// data/shells.toml.
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/shells.toml");

/// A case's life, seconds.
pub const LIFE: f32 = 2.5;
/// The start's lead by the shooter's velocity, seconds; the share of that
/// velocity a case keeps.
pub const LEAD: f32 = 0.1;
pub const CARRY: f32 = 0.5;
/// Third-person start (forward, up, right), units.
pub const THIRD_PERSON: (f32, f32, f32) = (20.0, -12.0, 4.0);
/// A late case's start (forward, up), units, and its speeds.
pub const LATE_START: (f32, f32) = (16.0, -9.0);
pub const LATE_FORWARD: f32 = 25.0;
/// A late case's client shift and push (sideways, units and units/s).
pub const LATE_SHIFT: f32 = 9.0;
pub const LATE_PUSH: f32 = 120.0;

/// One gun's document line.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Gun {
    pub model: String,
    /// The bounce sound's type: `brass` (default) or `shotgun`.
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub f: f32,
    #[serde(default)]
    pub f_left: Option<f32>,
    #[serde(default)]
    pub up: f32,
    #[serde(default)]
    pub r: f32,
    /// Two guns (the Elites): the offset's side alternates by shot.
    #[serde(default)]
    pub pair: bool,
    /// Thrown this many seconds after the shot.
    #[serde(default)]
    pub late: Option<f32>,
}

impl Gun {
    /// The kit's shell type key: the model's name, `_shotgun` after it
    /// for the M3's shotgun clinks.
    pub fn key(&self) -> String {
        match self.sound.as_deref() {
            Some("shotgun") => format!("{}_shotgun", self.model),
            _ => self.model.clone(),
        }
    }

    /// Its bounce sound's entry.
    pub fn bounce(&self) -> &'static str {
        match self.sound.as_deref() {
            Some("shotgun") => "weapons/sshell1.wav;weapons/sshell2.wav;weapons/sshell3.wav",
            _ => "player/pl_shell1.wav;player/pl_shell2.wav;player/pl_shell3.wav",
        }
    }
}

/// The guns by id (without `cs16:weapon_`).
pub fn guns() -> BTreeMap<String, Gun> {
    toml::from_str(DATA.text()).unwrap_or_else(|e| panic!("{}: {e}", DATA.path()))
}

/// How the cases fly (ours: shells.md open question 3): gravity as the
/// game's, a damped bounce, tumbling, a fade at the end of its 2.5 s.
pub fn physics() -> MapShellPhysics {
    MapShellPhysics {
        gravity: 800.0 * super::super::METERS_PER_UNIT,
        damping: 0.45,
        rest_normal: 0.7,
        rest_steps: 2.0,
        angle_damping: 0.5,
        unit: super::super::METERS_PER_UNIT,
        speed_factor: (1.0, 1.0),
        up_jitter: 0.0,
        right_jitter: 0.0,
        spin: 8.0,
        life: LIFE - 0.2,
        fade: 0.2,
        sound_chance: 0.6,
        sound_full_speed: 3.0,
        sound_sure_speed: None,
        world_jitter: (Vec3::ZERO, Vec3::ZERO),
        roll_spin: 8.0,
        life_jitter: 0.0,
        owner_velocity: false,
    }
}

/// The kit's shell types for the guns `wants` (a model once per sound
/// type), their models read by `load`.
pub fn kit(
    wants: &dyn Fn(&str) -> bool,
    mut load: impl FnMut(&str) -> Option<chimera_world::map::MapModel>,
    sounds: &mut Vec<String>,
    shells: &mut Vec<MapShell>,
) {
    for (id, gun) in guns() {
        if !wants(&format!("cs16:weapon_{id}")) || shells.iter().any(|s| s.is(super::super::GAME, &gun.key())) {
            continue;
        }
        let Some(model) = load(&gun.model) else { continue };
        sounds.push(gun.bounce().to_string());
        shells.push(MapShell {
            game: super::super::GAME.to_string(),
            key: gun.key(),
            model,
            bounce: Some(gun.bounce().to_string()),
            physics: physics(),
        });
    }
}

/// The shooter's view (units, Z up; engine directions turned to units).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// The shooter's velocity, units/s.
    pub velocity: Vec3,
}

impl View {
    /// From the eye (engine metres), the look (yaw, pitch) and the
    /// velocity (engine).
    pub fn of(eye: Vec3, yaw: f32, pitch: f32, velocity: Vec3) -> View {
        let q = aim_of(yaw, pitch);
        let dir = |v: Vec3| to_units(q * v * 1000.0).normalize_or_zero();
        View {
            eye: to_units(eye),
            forward: dir(Vec3::NEG_Z),
            right: dir(Vec3::X),
            up: dir(Vec3::Y),
            velocity: to_units(velocity),
        }
    }
}

/// A case thrown: start (units) and velocity (units/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throw {
    pub start: Vec3,
    pub velocity: Vec3,
}

/// The offsets (forward, up, right) an event shell starts at: the gun's
/// for the local player (`right_handed`; the Elites by `side`: -1 the
/// right gun, +1 the left), the third-person ones for others.
pub fn offsets(gun: &Gun, local: bool, right_handed: bool, side: f32) -> (f32, f32, f32) {
    if !local {
        return THIRD_PERSON;
    }
    if gun.pair {
        return (gun.f, gun.up, gun.r * -side);
    }
    if right_handed {
        (gun.f, gun.up, gun.r)
    } else {
        (gun.f_left.unwrap_or(gun.f), gun.up, -gun.r)
    }
}

/// An event shell (shells.md "Event shells"); `roll(lo, hi)` a uniform
/// draw.
pub fn event_throw(view: &View, (f, up, r): (f32, f32, f32), roll: &mut dyn FnMut(f32, f32) -> f32) -> Throw {
    let start = view.eye + view.velocity * LEAD + view.forward * f + view.up * up + view.right * r;
    // The reference flips the sideways direction against the side the
    // case starts on.
    let side = if r > 0.0 { -1.0 } else { 1.0 };
    let velocity = view.velocity * CARRY
        + view.right * side * roll(50.0, 70.0)
        + view.up * roll(75.0, 175.0)
        + view.forward * roll(25.0, 250.0);
    Throw { start, velocity }
}

/// A late shell (shells.md "Late shells"), as the server makes it.
pub fn late_throw(view: &View, roll: &mut dyn FnMut(f32, f32) -> f32) -> Throw {
    let start = view.eye + view.forward * LATE_START.0 + view.up * LATE_START.1;
    let velocity =
        view.velocity + view.right * roll(50.0, 70.0) + view.up * roll(100.0, 150.0) + view.forward * LATE_FORWARD;
    Throw { start, velocity }
}

/// A late shell as the receiving client places it: 9 units to the
/// left of the view and, for the local right-handed shooter, 120
/// units/s the same way (shells.md open question 2).
pub fn late_client(t: Throw, view: &View, local_right_handed: bool) -> Throw {
    Throw {
        start: t.start - view.right * LATE_SHIFT,
        velocity: t.velocity
            - if local_right_handed {
                view.right * LATE_PUSH
            } else {
                Vec3::ZERO
            },
    }
}

/// Shots whose cases are still to be thrown.
#[derive(Resource, Default)]
pub struct Late(Vec<(f64, Entity, String, Vec3)>);

/// A small xorshift, the draws' source.
#[derive(Default)]
pub struct Dice(u32);

impl Dice {
    fn roll(&mut self, lo: f32, hi: f32) -> f32 {
        let mut x = self.0.wrapping_add(0x9e37_79b9).max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        lo + (hi - lo) * ((x >> 8) as f32 / (1u32 << 24) as f32)
    }
}

/// The yaw a case lies at: the shooter's, its model's +X along the
/// shooter's view.
fn rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
}

fn write(out: &mut MessageWriter<ThrowShell>, gun: &Gun, t: Throw, yaw: f32) {
    out.write(ThrowShell {
        game: super::super::GAME.to_string(),
        key: gun.key(),
        position: to_engine(t.start),
        velocity: to_engine(t.velocity),
        rotation: rotation(yaw),
    });
}

/// Throw each shown shot's case, now or late.
#[allow(clippy::too_many_arguments)]
pub fn throw(
    mut events: MessageReader<WeaponEvent>,
    time: Res<Time>,
    settings: Option<Res<ViewModelSettings>>,
    table: Option<Res<ShellGuns>>,
    owners: Query<(Option<&Velocity>, Option<&LocalPlayer>, Option<&Intent>)>,
    mut late: ResMut<Late>,
    mut out: MessageWriter<ThrowShell>,
    mut dice: Local<Dice>,
    mut sides: Local<BTreeMap<Entity, bool>>,
) {
    let Some(table) = table.as_deref().map(|t| &t.0) else {
        events.clear();
        return;
    };
    let now = time.elapsed_secs_f64();
    let right_handed = settings.is_none_or(|s| s.right_hand != 0);
    for e in events.read() {
        let WeaponEventKind::Fired {
            id, origin, yaw, pitch, ..
        } = e.kind
        else {
            continue;
        };
        if e.replay {
            continue;
        }
        let Some(gun) = id.strip_prefix("cs16:weapon_").and_then(|g| table.get(g)) else {
            continue;
        };
        let Ok((velocity, local, _)) = owners.get(e.owner) else {
            continue;
        };
        if let Some(delay) = gun.late {
            late.0.push((now + f64::from(delay), e.owner, id.to_string(), origin));
            continue;
        }
        let view = View::of(origin, yaw, pitch, velocity.map_or(Vec3::ZERO, |v| v.0));
        let side = if gun.pair {
            let left = sides.entry(e.owner).or_default();
            *left = !*left;
            if *left { 1.0 } else { -1.0 }
        } else {
            0.0
        };
        let t = event_throw(&view, offsets(gun, local.is_some(), right_handed, side), &mut |a, b| {
            dice.roll(a, b)
        });
        write(&mut out, gun, t, yaw);
    }
    // The late ones now due, at the view of this moment.
    let due: Vec<_> = late.0.iter().filter(|l| l.0 <= now).cloned().collect();
    late.0.retain(|l| l.0 > now);
    for (_, owner, id, origin) in due {
        let Some(gun) = id.strip_prefix("cs16:weapon_").and_then(|g| table.get(g)) else {
            continue;
        };
        let Ok((velocity, local, intent)) = owners.get(owner) else {
            continue;
        };
        let (yaw, pitch) = intent.map_or((0.0, 0.0), |i| (i.yaw, i.pitch));
        let view = View::of(origin, yaw, pitch, velocity.map_or(Vec3::ZERO, |v| v.0));
        let t = late_client(
            late_throw(&view, &mut |a, b| dice.roll(a, b)),
            &view,
            local.is_some() && right_handed,
        );
        write(&mut out, gun, t, yaw);
    }
}

/// The guns' table, parsed once (a resource the plugin inserts).
#[derive(Resource, Clone, Debug)]
pub struct ShellGuns(pub BTreeMap<String, Gun>);

/// The shells' parts: the throw message and the system.
pub fn add(app: &mut App) {
    app.add_message::<ThrowShell>()
        .init_resource::<Late>()
        .insert_resource(ShellGuns(guns()))
        .add_systems(Update, throw);
}
