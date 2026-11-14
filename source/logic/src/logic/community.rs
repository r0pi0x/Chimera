//! Classes community maps use that no spec covers yet (docs/plans/
//! active/community-maps.md, "Map logic audit"), from the public entity
//! documentation; our choices where it is silent are in docs/tech-debt.md:
//! logic_measure_movement, point_teleport, logic_multicompare, env_shake,
//! env_shooter, point_push, env_texturetoggle,
//! env_screenoverlay, point_camera (func_monitor is a func_brush),
//! env_viewpunch, env_muzzleflash.
//!
//! Covers: classes community maps use with no spec yet: logic_measure_movement,
//! point_teleport, logic_multicompare, env_shake (`MapShake`, shaken by
//! `client::senses`), env_shooter (Shoot: its model as gibs, `Effect::Gibs`),
//! point_push (physics bodies and players pushed each tick while enabled),
//! env_texturetoggle (its target brushes' `texture_frame`, drawn through
//! `map::BrushTint`), env_screenoverlay (its overlays in turn on every
//! screen, `HudShow::Overlay`), env_viewpunch (`Effect::ViewPunch`: the weapon
//! layer adds it to the view punch through `map::ViewKick`), env_muzzleflash
//! (`Effect::MuzzleFlash`), point_camera (on/off, field of view;
//! `LogicWorld::monitor_camera`, the camera a shown func_monitor names, written
//! to `map::MonitorCamera`); momentary_rot_button and func_rot_button
//! are in `movers.rs`, func_tanktrain is a func_tracktrain, func_monitor a
//! func_brush.
//!
//! Known shortcuts (docs/tech-debt.md): Community-map behaviour the specs leave
//! open (docs/plans/active/community-maps.md, "Map logic audit"): physics
//! bodies (physics props, func_physbox) touch triggers with spawnflag 8 or 64
//! by their collider's world box as the host last reported it (a tick late);
//! trigger_push on them adds speed × 100 × dt / mass to their velocity each
//! tick (the spec's force read as an impulse in kg·units/s; once-only: the push
//! itself) and trigger_teleport moves their box's centre to the destination; a
//! player's classname set by AddOutput (kz_ stage marks) lasts until changed,
//! across deaths and round restarts, and stops it matching "player" as a
//! target; AddOutput classname on any other entity renames it for filters and
//! targets only; func_rotating's AddOutput maxspeed takes effect at its next
//! Start or SetSpeed; a mover given speed 0 holds with its goal kept (the
//! move's time is infinite). Classes with no spec (`logic::community`,
//! `movers::Momentary`, from the public entity docs): logic_measure_movement
//! measures from activation every tick until Disabled (MeasureType 1: the
//! player's eye and view, else its feet and yaw), placing a mover's pusher (its
//! track along, translation only), a physics body's centre, a player or a point
//! entity; point_teleport places its target the same way at its own origin and
//! angles; logic_multicompare compares the values given since the last
//! CompareValues (none: equal) and forgets them; env_shake shakes the
//! view (`client::senses`, the grenade's shake shape; the server sends it to
//! network clients as `MapFx::Shake`), GlobalShake as an infinite radius, StopShake ending every map
//! shake; func_rot_button turns like a rotating door (flags 2/64/128,
//! "distance") with func_button's other rules, its flag 1 is not-solid;
//! momentary_rot_button turns by SetPosition at its speed and fires
//! Position/OnReachedPosition/OnFullyOpen/OnFullyClosed on arrival,
//! SetPositionImmediately fires nothing, nothing at spawn (so surf_boreas's
//! decorative spinners, which only restart themselves from OnFullyClosed, stay
//! still), and players can't turn it with +use. Render inputs
//! (`map::RenderLook`): Color, Alpha and AddOutput
//! rendercolor/renderamt/rendermode draw on brush entities with a node and on
//! players' bodies and held models; baked brush entities draw their spawn
//! render mode only (no colour); renderfx (pulses, flicker, hologram) is kept
//! and not drawn; sprites, beams and props keep their spawn colour and alpha
//! (props' and placed weapons' render inputs aren't drawn); a player keeps its
//! look across deaths and round restarts (its entity persists; not measured in
//! CS:S); a player's view model doesn't fade with it. Parenting at run time:
//! SetParent/ClearParent on any entity and on players; SetParentAttachment
//! snaps to the attachment's place in the parent model's reference pose (the
//! loader's `$attachment <name>` keys on prop entities something is parented
//! to; animated attachments don't move; none known: the parent's origin), the
//! MaintainOffset form keeps the offset; a parented player is placed at its
//! parent's pose each logic phase with its own movement, physics shadow and
//! push-away off (`core::MapControls::parented`; a physics parent passes it no
//! velocity), let go when it dies. Why it's OK for now: What the maps need
//! (mg_boatrace_scramble's boats, kz_bhop_izanami's stages,
//! mg_swag_multigames_v1's lifts); a probe-server check would settle the
//! timings.
//!
//! Known shortcuts (docs/tech-debt.md): Classes from the public entity docs
//! only (`logic::community`, `render::monitor`, `render::emitters`):
//! func_water_analog is a func_movelinear (Open, Close, SetPosition) whose swim
//! volume and surface follow its node, as water parented to a mover does
//! (`map::water::move_water`; WaveHeight unused; a moving surface is drawn
//! whole, not split by the map's visibility, and isn't replicated beyond its
//! node's motion); func_tanktrain is a func_tracktrain (no health, gun or
//! damage); env_shooter throws its gibs all at once ("delay", shoot sounds,
//! skin and gib angles unused); point_push accelerates by its magnitude in
//! units/s², falling off linearly to its radius (inner radius and line-of-sight
//! unused; not measured); env_texturetoggle frames show on movers' own meshes
//! only (a looked node leaves the merged chunks; a colour set with a frame
//! shows the frame's material untinted until the colour changes);
//! env_screenoverlay shows on every screen (not per player), each overlay until
//! its time, the client drawing it stretched over the view; point_camera
//! renders at 512² for every monitor (one camera at a time: the first on camera
//! a shown monitor names), culled for the player's own view, without FOV
//! changes over time, with the camera's own fog (fogEnable, fogColor, fogStart,
//! fogEnd, fogMaxDensity; no SetFog inputs) except on water surfaces and prop
//! shadows in it, which still fade with the scene's, and no water fog;
//! point_tesla arcs trace the world only (not props) and are redrawn jagged
//! each frame; color_correction's SetFadeInDuration/SetFadeOutDuration are
//! accepted and not kept, and the table is applied after the tone map to the
//! whole view (view models included; under water the screen warp and it read
//! the same frame, the later one wins); env_embers use `particle/fire` 2 units
//! wide (the game's ember sprite and size aren't in the entity docs), drift at
//! 50-100 % of their speed, pulled ones speed up along it; env_muzzleflash
//! draws three `effects/muzzleflash1` sprites for 0.05 s (no light);
//! env_wind, env_detail_controller,
//! info_ladder_dismount, propper_model, logic_script and ai_changetarget are
//! noted once per map and do nothing (detail props aren't drawn:
//! kz_ancient_ruins has some); player SetFogController isn't switched per
//! player (surf_halloween_tf2's names no controller); func_fish_pool's fish
//! (props the loader spreads around its origin) swim to random places in its
//! range and in water, without flocking, fleeing players or their own animation
//! (`map::fish`); func_reflective_glass is a func_brush (no reflection); props'
//! EnableDamageForces/DisableDamageForces are accepted and not read;
//! env_soundscape_triggerable is an env_soundscape. Why it's OK for now: What
//! the maps use (mg_lt_galaxy_v5's screens, black hole and towers, mg_kommando,
//! mg_escape_prison_beta, mg_crazykart_v1_1, mg_swag_multigames_v1,
//! surf_halloween_tf2).

use super::classes::kinds;
use bevy::prelude::*;

use super::anchors::angles_of;
use super::classes::Class;
use super::value::Value;
use super::world::{Effect, EntId, LogicWorld, Who};
use crate::map::entities::entity_rotation;

/// One of this module's classes.
#[derive(Clone, Debug)]
pub enum Extra {
    Measure(Measure),
    Teleport,
    MultiCompare(MultiCompare),
    Shake(Shake),
    /// env_shooter: Shoot throws its model as gibs.
    Shooter,
    /// point_push: pushes physics bodies (and players with flag 8)
    /// within its radius while enabled.
    Push {
        enabled: bool,
    },
    /// env_texturetoggle: sets the texture frame of its target brushes.
    TextureToggle,
    /// env_screenoverlay: its overlays (1-10) on every player's screen,
    /// each for its time, in turn; the one showing (0: none).
    Overlay {
        at: usize,
    },
    /// env_viewpunch: ViewPunch kicks the view of players within its
    /// radius (flag 1: all; flag 2: those in the air too).
    ViewPunch,
    /// env_muzzleflash: Fire flashes at it (the map draws it).
    MuzzleFlash,
    /// point_camera: what monitors show while it is on; its field of view.
    Camera {
        on: bool,
        fov: f32,
    },
    /// point_angularvelocitysensor (`sensors`).
    Sensor(super::sensors::AngularSensor),
    /// env_microphone (`sensors`).
    Mic(super::sensors::Microphone),
}

/// point_camera's "Start Off" spawnflag.
pub const SF_CAMERA_START_OFF: u32 = 1;

/// point_push spawnflags (public entity docs).
pub const SF_PUSH_DIRECTIONAL: u32 = 2;
pub const SF_PUSH_NO_FALLOFF: u32 = 4;
pub const SF_PUSH_PLAYERS: u32 = 8;
pub const SF_PUSH_PHYSICS: u32 = 16;

/// A point_push's acceleration at `dist` from it: magnitude (units/s²),
/// falling off linearly to 0 at the radius unless "No falloff".
pub fn push_strength(magnitude: f32, radius: f32, dist: f32, falloff: bool) -> f32 {
    if dist > radius || radius <= 0.0 {
        return 0.0;
    }
    if falloff {
        magnitude * (1.0 - dist / radius)
    } else {
        magnitude
    }
}

/// logic_measure_movement: each tick, the target's pose relative to its
/// reference is the measured entity's pose relative to its own reference,
/// its position scaled ("TargetScale"). "MeasureType" 1 measures a
/// player's eye (position and view angles).
#[derive(Clone, Debug)]
pub struct Measure {
    pub enabled: bool,
    pub measure: String,
    pub measure_ref: String,
    pub target: String,
    pub target_ref: String,
    pub scale: f32,
    pub eye: bool,
}

/// logic_multicompare: the values given since the last compare.
#[derive(Clone, Debug, Default)]
pub struct MultiCompare {
    pub values: Vec<i32>,
}

/// env_shake's numbers (units, Hz, seconds).
#[derive(Clone, Debug)]
pub struct Shake {
    pub amplitude: f32,
    pub frequency: f32,
    pub duration: f32,
    pub radius: f32,
}

/// env_shake's "GlobalShake" flag.
pub const SF_SHAKE_GLOBAL: u32 = 1;

pub(super) fn spawn(w: &LogicWorld, id: EntId, class: &str) -> Option<Class> {
    let e = w.get(id)?;
    let s = |k: &str| e.kv(k).unwrap_or("").trim().to_string();
    if let Some(c) = super::sensors::spawn(w, id, class) {
        return Some(c);
    }
    let extra = match class {
        "logic_measure_movement" => Extra::Measure(Measure {
            // Measures from activation (the Disable input stops it).
            enabled: true,
            measure: s("MeasureTarget"),
            measure_ref: s("MeasureReference"),
            target: s("Target"),
            target_ref: s("TargetReference"),
            scale: e.kv("TargetScale").map_or(1.0, super::value::atof),
            eye: e.kv_i("MeasureType") == 1,
        }),
        "point_teleport" => Extra::Teleport,
        "env_texturetoggle" => Extra::TextureToggle,
        "env_screenoverlay" => Extra::Overlay { at: 0 },
        "point_camera" => Extra::Camera {
            on: !e.has_flag(SF_CAMERA_START_OFF),
            fov: e.kv("FOV").map_or(90.0, super::value::atof),
        },
        "env_shooter" | "env_rotorshooter" => Extra::Shooter,
        "env_viewpunch" => Extra::ViewPunch,
        "env_muzzleflash" => Extra::MuzzleFlash,
        "point_push" => Extra::Push {
            enabled: e.kv("enabled").is_none_or(|v| super::value::atoi(v) != 0),
        },
        "logic_multicompare" => Extra::MultiCompare(MultiCompare::default()),
        "env_shake" => Extra::Shake(Shake {
            amplitude: e.kv_f("amplitude"),
            frequency: e.kv("frequency").map_or(40.0, super::value::atof),
            duration: e.kv("duration").map_or(1.0, super::value::atof),
            radius: e.kv_f("radius"),
        }),
        _ => return None,
    };
    Some(Class::Extra(Box::new(extra)))
}

fn extra(w: &LogicWorld, id: EntId) -> Option<&Extra> {
    match w.get(id).map(|e| &e.class) {
        Some(Class::Extra(x)) => Some(x),
        _ => None,
    }
}

fn extra_mut(w: &mut LogicWorld, id: EntId) -> Option<&mut Extra> {
    match w.get_mut(id).map(|e| &mut e.class) {
        Some(Class::Extra(x)) => Some(x),
        _ => None,
    }
}

pub(super) fn activate(w: &mut LogicWorld, id: EntId) {
    super::sensors::activate(w, id);
    if matches!(extra(w, id), Some(Extra::Measure(_) | Extra::Push { enabled: true })) {
        w.think_in(id, w.dt);
    }
}

pub(super) fn think(w: &mut LogicWorld, id: EntId) {
    if super::sensors::think(w, id) {
        return;
    }
    match extra(w, id).cloned() {
        Some(Extra::Measure(m)) if m.enabled => {
            measure(w, id, &m);
            w.think_in(id, w.dt);
        }
        Some(Extra::Push { enabled: true }) => {
            push(w, id);
            w.think_in(id, w.dt);
        }
        // The overlay's time ran out: the next one, else none.
        Some(Extra::Overlay { at }) if at > 0 => {
            let next = at + 1;
            if overlay_name(w, id, next).is_some() {
                show_overlay(w, id, next);
            } else {
                stop_overlays(w, id);
            }
        }
        _ => {}
    }
}

impl LogicWorld {
    /// The point_camera monitors show (public entity docs): the first one
    /// that is on and named by a shown func_monitor's target, with its
    /// origin, angles and field of view (entity space, degrees).
    pub fn monitor_camera(&self) -> Option<(Vec3, Vec3, f32)> {
        let cam = self.active_monitor_camera()?;
        let Some(Extra::Camera { fov, .. }) = extra(self, cam) else {
            return None;
        };
        let (o, a) = self.parent_pose(Who::Ent(cam));
        Some((o, a, *fov))
    }

    /// The fog of the camera monitors show (`monitor_camera`): its own
    /// settings (public point_camera docs: fogEnable, fogColor, fogStart,
    /// fogEnd, fogMaxDensity; defaults off, black, 2048, 4096, 1), not the
    /// map's. Distances in entity units.
    pub fn monitor_fog(&self) -> Option<crate::map::MonitorFog> {
        let c = self.get(self.active_monitor_camera()?)?;
        let num = |k: &str, d: f32| c.kv(k).map_or(d, super::value::atof);
        let mut color = [0.0f32; 3];
        for (slot, v) in color
            .iter_mut()
            .zip(c.kv("fogColor").unwrap_or("0 0 0").split_whitespace())
        {
            *slot = (super::value::atof(v) / 255.0).clamp(0.0, 1.0);
        }
        Some(crate::map::MonitorFog {
            enabled: c.kv("fogEnable").is_some_and(|v| super::value::atoi(v) != 0),
            color,
            start: num("fogStart", 2048.0),
            end: num("fogEnd", 4096.0),
            max_density: num("fogMaxDensity", 1.0),
        })
    }

    /// The point_camera monitors show: the first one that is on and named
    /// by a shown func_monitor's target.
    fn active_monitor_camera(&self) -> Option<EntId> {
        for id in self.ids_of(kinds::BRUSH) {
            let Some(e) = self.get(id) else { continue };
            if !e.classname.eq_ignore_ascii_case("func_monitor") {
                continue;
            }
            let shown = matches!(&e.class, Class::Brush(t) if t.enabled);
            let target = e.kv("target").unwrap_or("").trim();
            if !shown || target.is_empty() {
                continue;
            }
            for cam in self.ids_of(kinds::EXTRA) {
                let Some(c) = self.get(cam) else { continue };
                if let Class::Extra(x) = &c.class
                    && let Extra::Camera { on: true, .. } = **x
                    && !c.targetname.is_empty()
                    && super::world::name_matches(target, &c.targetname)
                {
                    return Some(cam);
                }
            }
        }
        None
    }
}

/// env_screenoverlay's overlay `n` (1-10): its material, when named.
fn overlay_name(w: &LogicWorld, id: EntId, n: usize) -> Option<String> {
    let name = w.get(id)?.kv(&format!("OverlayName{n}"))?.trim().to_string();
    (!name.is_empty() && (1..=10).contains(&n)).then_some(name)
}

/// Show overlay `n` on every screen for its time ("OverlayTimeN"; -1:
/// until stopped), then the next (public entity docs).
fn show_overlay(w: &mut LogicWorld, id: EntId, n: usize) {
    let Some(material) = overlay_name(w, id, n) else { return };
    let secs = w.get(id).map_or(-1.0, |e| {
        e.kv(&format!("OverlayTime{n}")).map_or(-1.0, super::value::atof)
    });
    if let Some(Extra::Overlay { at }) = extra_mut(w, id) {
        *at = n;
    }
    w.effects.push(Effect::Hud {
        to: None,
        what: super::hud::HudShow::Overlay { material, secs: -1.0 },
    });
    if secs > 0.0 {
        w.think_in(id, secs);
    } else if let Some(e) = w.get_mut(id) {
        e.next_think = None;
    }
}

fn stop_overlays(w: &mut LogicWorld, id: EntId) {
    if let Some(Extra::Overlay { at }) = extra_mut(w, id) {
        *at = 0;
    }
    if let Some(e) = w.get_mut(id) {
        e.next_think = None;
    }
    w.effects.push(Effect::Hud {
        to: None,
        what: super::hud::HudShow::Overlay {
            material: String::new(),
            secs: 0.0,
        },
    });
}

/// One tick of a point_push: physics bodies (flag 16) and players (flag
/// 8) within its radius are sped up away from it (along its forward with
/// "Use angles for push direction"), less the farther they are.
fn push(w: &mut LogicWorld, id: EntId) {
    let Some(e) = w.get(id) else { return };
    let (origin, flags) = (e.origin, e.spawnflags);
    let (magnitude, radius) = (e.kv_f("magnitude"), e.kv_f("radius"));
    let along = (flags & SF_PUSH_DIRECTIONAL != 0).then(|| super::triggers::forward(e.angles));
    let falloff = flags & SF_PUSH_NO_FALLOFF == 0;
    let dt = w.dt;
    let dir = |at: Vec3| along.unwrap_or_else(|| (at - origin).normalize_or_zero());
    if flags & SF_PUSH_PHYSICS != 0 {
        for body in w.ids_of(kinds::PROP) {
            let Some((lo, hi)) = super::triggers::physics_body(w, body) else {
                continue;
            };
            let at = (lo + hi) * 0.5;
            let a = push_strength(magnitude, radius, at.distance(origin), falloff);
            if a != 0.0 {
                w.effects.push(Effect::BodyVelocity {
                    id: body,
                    velocity: dir(at) * a * dt,
                });
            }
        }
    }
    if flags & SF_PUSH_PLAYERS != 0 {
        for p in w.players.iter_mut().filter(|p| p.alive) {
            let at = p.origin + (p.mins + p.maxs) * 0.5;
            let a = push_strength(magnitude, radius, at.distance(origin), falloff);
            if a != 0.0 {
                p.velocity += dir(at) * a * dt;
                p.moved = true;
            }
        }
    }
}

/// env_viewpunch's ViewPunch (public entity docs): "punchangle" (pitch,
/// yaw, roll degrees) added to the view punch of each player within
/// "radius" (flag 1: every player), on the ground unless flag 2.
fn view_punch(w: &mut LogicWorld, id: EntId) {
    let Some(e) = w.get(id) else { return };
    let angles = e.kv("punchangle").map_or(Vec3::ZERO, crate::map::entities::parse_vector);
    let (all, air) = (e.has_flag(SF_VIEWPUNCH_ALL), e.has_flag(SF_VIEWPUNCH_AIR));
    let (origin, radius) = (e.origin, e.kv_f("radius"));
    let hit: Vec<Entity> = w
        .players
        .iter()
        .filter(|p| p.alive && (air || p.on_ground) && (all || p.origin.distance(origin) <= radius))
        .map(|p| p.entity)
        .collect();
    for player in hit {
        w.effects.push(Effect::ViewPunch { player, angles });
    }
}

/// env_viewpunch spawnflags (public entity docs).
pub const SF_VIEWPUNCH_ALL: u32 = 1;
pub const SF_VIEWPUNCH_AIR: u32 = 2;

/// env_shooter's Shoot (public entity docs): its "m_iGibs" gibs of
/// "shootmodel" leave its origin along its forward at "m_flVelocity",
/// spread by "m_flVariance", spinning at "gibanglevelocity", for
/// "m_flGibLife" seconds.
fn shoot(w: &mut LogicWorld, id: EntId) {
    let Some(e) = w.get(id) else { return };
    let model = e
        .kv("shootmodel")
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .replace('\\', "/");
    if model.is_empty() {
        return;
    }
    let count = e.kv_i("m_iGibs").clamp(1, 64);
    let (speed, variance) = (e.kv_f("m_flVelocity"), e.kv_f("m_flVariance"));
    let life = e.kv("m_flGibLife").map_or(4.0, super::value::atof).max(0.1);
    let spin = e.kv_f("gibanglevelocity");
    let (origin, forward) = (e.origin, super::triggers::forward(e.angles));
    let mut pieces = Vec::new();
    for _ in 0..count {
        let r = Vec3::new(w.random(), w.random(), w.random()) * 2.0 - Vec3::ONE;
        let dir = (forward + r * variance).normalize_or_zero();
        let s = Vec3::new(w.random(), w.random(), w.random()) * 2.0 - Vec3::ONE;
        pieces.push(super::breakables::Gib {
            position: origin,
            velocity: dir * speed,
            spin: s * spin,
            life,
        });
    }
    w.effects.push(Effect::Gibs {
        set: model,
        glass: false,
        pieces,
        bounce: None,
    });
}

/// The first entity or player a name reaches.
fn named(w: &LogicWorld, name: &str, me: EntId) -> Option<Who> {
    if name.is_empty() {
        return None;
    }
    w.resolve(name, None, Some(Who::Ent(me))).into_iter().next()
}

/// Where something is: a player's feet (or eye) and view, a mover's
/// pusher, else an entity's origin and angles.
fn pose(w: &LogicWorld, who: Who, eye: bool) -> Option<(Vec3, Quat)> {
    match who {
        Who::Player(p) => {
            let p = w.player(p)?;
            let (at, angles) = if eye {
                (p.origin + p.eye, p.view)
            } else {
                (p.origin, Vec3::new(0.0, p.view.y, 0.0))
            };
            Some((at, entity_rotation(angles)))
        }
        Who::Ent(id) => {
            let (o, a) = w.parent_pose(Who::Ent(id));
            w.get(id)?;
            Some((o, entity_rotation(a)))
        }
    }
}

fn measure(w: &mut LogicWorld, id: EntId, m: &Measure) {
    let (Some(src), Some(src_ref), Some(target), Some(target_ref)) = (
        named(w, &m.measure, id),
        named(w, &m.measure_ref, id),
        named(w, &m.target, id),
        named(w, &m.target_ref, id),
    ) else {
        return;
    };
    let (Some((so, sr)), Some((ro, rr)), Some((to, tr))) =
        (pose(w, src, m.eye), pose(w, src_ref, false), pose(w, target_ref, false))
    else {
        return;
    };
    let local = rr.inverse() * (so - ro) * m.scale;
    let local_rot = rr.inverse() * sr;
    place(w, target, to + tr * local, angles_of(tr * local_rot));
}

/// Put an entity (or player) at a pose: a player's feet and view, a
/// physics body (its centre), a mover (its track along), else its origin.
pub(super) fn place(w: &mut LogicWorld, who: Who, origin: Vec3, angles: Vec3) {
    match who {
        Who::Player(p) => {
            if let Some(pl) = w.player_mut(p) {
                pl.origin = origin;
                pl.view = angles;
                pl.on_ground = false;
                pl.unground = true;
                pl.teleported = true;
            }
        }
        Who::Ent(id) => {
            if super::triggers::physics_body(w, id).is_some() {
                w.effects.push(Effect::BodyTeleport {
                    id,
                    origin,
                    angles: Some(angles),
                });
                return;
            }
            let (o, _) = w.parent_pose(Who::Ent(id));
            let Some(e) = w.get_mut(id) else { return };
            let delta = origin - o;
            if super::movers::shift(&mut e.class, delta, Vec3::ZERO) {
                if let Some(p) = super::movers::pusher_mut(&mut e.class) {
                    p.angles = angles;
                }
                w.refresh_solid(id);
                return;
            }
            e.origin = origin;
            e.angles = angles;
            let hulls = e.hulls.clone();
            if let Class::Trigger(t) = &mut e.class {
                let rot = entity_rotation(angles);
                t.brushes = hulls.iter().map(|h| super::world::place_hull(h, rot, origin)).collect();
            }
        }
    }
}

pub(super) fn input(w: &mut LogicWorld, id: EntId, input: &str, value: &Value, activator: Option<Who>) -> bool {
    if super::sensors::input(w, id, input, value, activator) {
        return true;
    }
    let Some(x) = extra(w, id).cloned() else { return false };
    match (x, input) {
        (Extra::Measure(_), "enable" | "disable") => {
            let on = input == "enable";
            if let Some(Extra::Measure(m)) = extra_mut(w, id) {
                m.enabled = on;
            }
            if on {
                w.think_in(id, w.dt);
            }
        }
        (Extra::Measure(_), "setmeasuretarget" | "setmeasurereference" | "settarget" | "settargetreference") => {
            let Some(name) = w.need_str(value, input) else { return true };
            if let Some(Extra::Measure(m)) = extra_mut(w, id) {
                *match input {
                    "setmeasuretarget" => &mut m.measure,
                    "setmeasurereference" => &mut m.measure_ref,
                    "settarget" => &mut m.target,
                    _ => &mut m.target_ref,
                } = name.trim().to_string();
            }
        }
        (Extra::Measure(_), "settargetscale") => {
            let Some(v) = w.need_float(value, input) else { return true };
            if let Some(Extra::Measure(m)) = extra_mut(w, id) {
                m.scale = v;
            }
        }
        (Extra::Shooter, "shoot") => shoot(w, id),
        (Extra::ViewPunch, "viewpunch") => view_punch(w, id),
        (Extra::MuzzleFlash, "fire") => {
            if let Some(entity) = w.get(id).and_then(|e| e.map_index) {
                w.effects.push(Effect::MuzzleFlash { entity });
            }
        }
        (Extra::Camera { .. }, "seton" | "setoff" | "setonandturnothersoff") => {
            if input == "setonandturnothersoff" {
                for other in w.ids_of(kinds::EXTRA) {
                    if other != id
                        && let Some(Extra::Camera { on, .. }) = extra_mut(w, other)
                    {
                        *on = false;
                    }
                }
            }
            if let Some(Extra::Camera { on, .. }) = extra_mut(w, id) {
                *on = input != "setoff";
            }
        }
        (Extra::Camera { .. }, "changefov") => {
            // "fov seconds": taken at once.
            let text = w.need_str(value, input).unwrap_or_default();
            if let Some(v) = text.split_whitespace().next().and_then(|v| v.parse::<f32>().ok())
                && let Some(Extra::Camera { fov, .. }) = extra_mut(w, id)
            {
                *fov = v;
            }
        }
        (Extra::Camera { .. }, "forceactive" | "forceinactive") => {}
        (Extra::Overlay { .. }, "startoverlays") => show_overlay(w, id, 1),
        (Extra::Overlay { .. }, "stopoverlays") => stop_overlays(w, id),
        (Extra::Overlay { .. }, "switchoverlay") => {
            let Some(n) = w.need_int(value, input) else { return true };
            show_overlay(w, id, n.max(1) as usize);
        }
        // env_texturetoggle (public entity docs): its target's brushes show
        // texture frame n (SetTextureIndex) or the next one.
        (Extra::TextureToggle, "settextureindex" | "incrementtextureindex") => {
            let n = match input {
                "settextureindex" => match w.need_int(value, input) {
                    Some(n) => Some(n.max(0) as u32),
                    None => return true,
                },
                _ => None,
            };
            let target = w.get(id).and_then(|e| e.kv("target")).unwrap_or("").trim().to_string();
            for who in w.resolve(&target, activator, Some(Who::Ent(id))) {
                if let Who::Ent(t) = who
                    && let Some(e) = w.get_mut(t)
                {
                    e.texture_frame = n.unwrap_or(e.texture_frame + 1);
                }
            }
        }
        (Extra::Shooter, "setgibangles" | "setgibanglevelocity" | "setgiblife" | "setgibvelocity") => {}
        (Extra::Push { .. }, "enable" | "disable") => {
            let on = input == "enable";
            if let Some(Extra::Push { enabled }) = extra_mut(w, id) {
                *enabled = on;
            }
            if on {
                w.think_in(id, w.dt);
            } else if let Some(e) = w.get_mut(id) {
                e.next_think = None;
            }
        }
        (Extra::Teleport, "teleport") => {
            let Some(e) = w.get(id) else { return true };
            let (origin, angles) = (e.origin, e.angles);
            let target = e.kv("target").unwrap_or("").trim().to_string();
            for who in w.resolve(&target, activator, Some(Who::Ent(id))) {
                place(w, who, origin, angles);
            }
        }
        (Extra::MultiCompare(_), "updatevalue") => {
            let Some(v) = w.need_int(value, input) else { return true };
            if let Some(Extra::MultiCompare(m)) = extra_mut(w, id) {
                m.values.push(v);
            }
        }
        (Extra::MultiCompare(m), "comparevalues") => {
            let (to_value, value) = w
                .get(id)
                .map_or((false, 0), |e| (e.kv_i("ShouldComparetoValue") != 0, e.kv_i("IntegerValue")));
            // Every value given since the last compare equal (to each
            // other, or to IntegerValue when asked); none given: equal.
            let first = if to_value { Some(value) } else { m.values.first().copied() };
            let equal = first.is_none_or(|f| m.values.iter().all(|v| *v == f));
            if let Some(Extra::MultiCompare(m)) = extra_mut(w, id) {
                m.values.clear();
            }
            w.fire_output(id, if equal { "OnEqual" } else { "OnNotEqual" }, activator, Value::Void);
        }
        (Extra::Shake(s), "startshake" | "stopshake") => {
            let Some(e) = w.get(id) else { return true };
            let global = e.has_flag(SF_SHAKE_GLOBAL);
            let at = e.origin;
            // Every screen shake's amplitude is capped to 16 when sent
            // (hl2dm/maps_entities.md 15).
            let amplitude = if input == "startshake" { s.amplitude.min(16.0) } else { 0.0 };
            w.effects.push(Effect::Shake {
                at,
                amplitude,
                frequency: s.frequency,
                duration: if input == "startshake" { s.duration } else { 0.0 },
                radius: if global { f32::INFINITY } else { s.radius },
            });
        }
        (Extra::Shake(_), "amplitude" | "frequency") => {
            let Some(v) = w.need_float(value, input) else { return true };
            if let Some(Extra::Shake(s)) = extra_mut(w, id) {
                if input == "amplitude" {
                    s.amplitude = v;
                } else {
                    s.frequency = v;
                }
            }
        }
        _ => return false,
    }
    true
}
