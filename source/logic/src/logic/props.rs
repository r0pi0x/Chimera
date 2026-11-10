//! Prop entities in the logic: model doors (specs/source/doors_buttons.md,
//! "prop_door_rotating") turned like rotating brush doors. Props that take
//! damage and fire outputs (prop_dynamic, prop_physics*) are in
//! `prop_damage`.
//!
//! Covers: prop_door_rotating: a pusher turning about its hinge, opening away
//! from the user (the other way when players, loose physics props or the static
//! world block its swing), return delay, slaves, blocked stop/resume (also by a
//! physics prop pinned against the world, which forceclosed pushes through and
//! crushes), the hardware's handle sounds, Open/Close firing twice.
//!
//! Known shortcuts (docs/tech-debt.md): Model doors (prop_door_rotating,
//! source/logic/src/logic/props.rs) beyond the spec: the leaf side decides forward/back (a
//! leaf on the hinge's right swaps them: our reading of "hinge on the left");
//! spawnpos 1/2/3 = open forward/back/ajar and flag 1 opens forward;
//! OnOpen/OnClose/OnFullyOpen/OnFullyClosed use the door as activator (like
//! func_door); an Open input with no user opens forward; the swing-side check
//! and the auto-close retry look at living players, loose physics props (their
//! world box shrunk 0.5 units sideways, 2 up and down) and the static world
//! (the leaf's corners, edge midpoints and centre pulled 20 % toward its
//! centre, as small boxes), sampled every 15°, and a player's opening takes the
//! other way only when that one is clear; a loose physics prop blocks a turning
//! door when the door would push its box into the world or another mover (its
//! world box, as for players), and with forceclosed it is pushed through
//! instead and a damageable one takes its health as crush damage (each tick it
//! is pinned); sounds are the model's `door_options` move/open/close for its
//! skin and locked/unlocked from its `hardwareN` block for the door's
//! `hardware` (measured, spec Q8: a model without that block plays
//! `DoorSound.Null`, ours nothing) or the map's sound overrides, the unlocked
//! one on an unlocked +use (the move 0.2 s later) and on an Open input; collision is the `.phy` pieces
//! as pusher brushes. Skin/SetBodyGroup/SetAnimation follow
//! specs/source/prop_damage.md 8-9 except: a skin the model lacks is ignored,
//! OnAnimationBegun fires for any SetAnimation, even a sequence the model lacks,
//! and not at spawn for DefaultAnim; OnAnimationDone comes from the client's animation
//! (`map::PropAnimationDone`, when a non-looping sequence ends: a server without
//! the models' skeletons never fires it, and it is checked per frame, not at 0.1 s
//! thinks), the cycle advances every frame (not 0.1 s
//! thinks), sequences pick up no animation events (sounds), skinned props keep
//! their spawn-time per-vertex light and draw without frustum culling; a skin
//! swap drops a prop's nearest-cubemap reflection variant; only prop_dynamic
//! entities with DefaultAnim or a SetAnimation output aimed at them load their
//! skeleton. Why it's OK for now: The spec's cs_assault cases pass; de_nuke's
//! steam and cs_office's projector work; see the prop damage row.

use super::classes::kinds;
use bevy::prelude::*;

use super::classes::Class;
use super::movers::{Done, DoorState, Pusher};
use super::triggers::forward;
use super::value::Value;
use super::world::{Effect, EntId, LogicWorld, SOLID_SKIN, Who, place_hull};
use crate::map::entities::{
    DOOR_CLOSE_KEY, DOOR_LOCKED_KEY, DOOR_MOVE_KEY, DOOR_OPEN_KEY, DOOR_UNLOCKED_KEY, entity_rotation,
};

pub use super::prop_damage::{Prop, PropState, is_prop_class};
pub(super) use super::prop_damage::prop_input;

/// prop_door_rotating defaults (doors_buttons.md constants).
pub const PROPDOOR_DEFAULT_DISTANCE: f32 = 90.0;
pub const PROPDOOR_DEFAULT_SPEED: f32 = 100.0;
/// A door tries to close this long after its return delay.
pub const PROPDOOR_RETURN_PAD: f32 = 0.1;

/// prop_door_rotating spawnflags.
pub const SF_DOOR_START_OPEN: u32 = 1;
pub const SF_DOOR_LOCKED: u32 = 2048;
pub const SF_DOOR_SILENT: u32 = 4096;
pub const SF_DOOR_USE_CLOSES: u32 = 8192;
pub const SF_DOOR_IGNORE_USE: u32 = 32768;

/// A model door (prop_door_rotating): turns about its origin (the hinge)
/// around Z.
#[derive(Clone, Debug)]
pub struct PropDoor {
    pub push: Pusher,
    pub state: DoorState,
    /// Closed angles, and the open ones each way (forward: away from
    /// someone behind the door; back: away from someone in front).
    pub closed: Vec3,
    pub open_forward: Vec3,
    pub open_back: Vec3,
    pub speed: f32,
    /// Seconds open before closing again; negative: never.
    pub return_delay: f32,
    /// 0 both ways, 1 forward only, 2 back only.
    pub opendir: i32,
    pub locked: bool,
    /// Doors (by name) that move with this one.
    pub slave: String,
    pub move_sound: String,
    pub open_sound: String,
    pub close_sound: String,
    pub locked_sound: String,
    pub unlocked_sound: String,
    /// A +use waiting out `PROPDOOR_USE_DELAY` after its handle sound:
    /// open (true, with the user) or close.
    pub pending_use: Option<(bool, Option<Who>)>,
}

/// An unlocked +use plays the handle sound, and the door starts moving
/// this long after (CS:S, measured 2026-10-17: 14 ticks later; an Open
/// input moves at once; doors_buttons.md Q8).
pub const PROPDOOR_USE_DELAY: f32 = 0.2;

impl PropDoor {
    pub(super) fn spawn(w: &mut LogicWorld, id: EntId) -> PropDoor {
        let e = w.get(id).unwrap();
        let distance = match e.kv_f("distance") {
            d if d == 0.0 => PROPDOOR_DEFAULT_DISTANCE,
            d => d,
        };
        let speed = match e.kv_f("speed") {
            s if s <= 0.0 => PROPDOOR_DEFAULT_SPEED,
            s => s,
        };
        // Which way the leaf lies from the hinge: with the leaf on the
        // door's left (+Y), a positive turn swings it backward; a model
        // hinged on the left has its leaf on the right, which swaps the
        // two (spec "Open forward/back ... swapped if the hinge is on the
        // left").
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in e.hulls.iter().flat_map(|h| &h.points) {
            lo = lo.min(p.y);
            hi = hi.max(p.y);
        }
        let leaf_right = lo < hi && lo + hi < 0.0;
        let turn = |d: f32| Vec3::new(0.0, d, 0.0);
        let (forward_turn, back_turn) = if leaf_right {
            (turn(distance), turn(-distance))
        } else {
            (turn(-distance), turn(distance))
        };
        let sound = |keys: &[&str]| {
            keys.iter()
                .filter_map(|k| e.kv(k).filter(|s| !s.trim().is_empty()))
                .map(str::to_string)
                .next()
                .unwrap_or_default()
        };
        let mut door = PropDoor {
            push: Pusher::at(e.origin, e.angles),
            state: DoorState::Closed,
            closed: e.angles,
            open_forward: e.angles + forward_turn,
            open_back: e.angles + back_turn,
            speed,
            return_delay: e.kv("returndelay").map_or(-1.0, super::value::atof),
            opendir: e.kv_i("opendir"),
            locked: e.has_flag(SF_DOOR_LOCKED),
            slave: e.kv("slavename").unwrap_or("").trim().to_string(),
            move_sound: sound(&["soundmoveoverride", DOOR_MOVE_KEY]),
            open_sound: sound(&["soundopenoverride", DOOR_OPEN_KEY]),
            close_sound: sound(&["soundcloseoverride", DOOR_CLOSE_KEY]),
            locked_sound: sound(&["soundlockedoverride", DOOR_LOCKED_KEY]),
            unlocked_sound: sound(&["soundunlockedoverride", DOOR_UNLOCKED_KEY]),
            pending_use: None,
        };
        // Spawn position: 1 open forward, 2 open back, 3 ajar; the "starts
        // open" flag opens it forward.
        let open_at = match e.kv_i("spawnpos") {
            1 => Some(door.open_forward),
            2 => Some(door.open_back),
            3 => Some(e.vector_kv("ajarangles")),
            _ if e.has_flag(SF_DOOR_START_OPEN) => Some(door.open_forward),
            _ => None,
        };
        if let Some(a) = open_at {
            door.state = DoorState::Open;
            door.push.angles = a;
        }
        door
    }
}

fn door(w: &mut LogicWorld, id: EntId) -> Option<&mut PropDoor> {
    match w.get_mut(id).map(|e| &mut e.class) {
        Some(Class::PropDoor(d)) => Some(d),
        _ => None,
    }
}

fn sound(w: &mut LogicWorld, id: EntId, entry: &str) {
    if entry.is_empty() {
        return;
    }
    let Some(e) = w.get(id) else { return };
    if e.has_flag(SF_DOOR_SILENT) {
        return;
    }
    let at = match &e.class {
        Class::PropDoor(d) => d.push.origin,
        _ => e.origin,
    };
    w.effects.push(Effect::Sound {
        entry: entry.to_string(),
        at,
        volume: None,
        pitch: None,
    });
}

/// Where `who` stands (entity space).
fn origin_of(w: &LogicWorld, who: Who) -> Option<Vec3> {
    match who {
        Who::Player(p) => w.player(p).map(|p| p.origin),
        Who::Ent(e) => w.get(e).map(|e| match super::movers::pusher(&e.class) {
            Some(p) => p.origin,
            None => e.origin,
        }),
    }
}

/// Physics props' boxes are shrunk this much (units) sideways and up and
/// down before testing them against a door: their world box is a bound,
/// and a resting prop's box touches the floor.
pub const PROP_BOX_SHRINK: Vec3 = Vec3::new(0.5, 0.5, 2.0);
/// A door leaf's sample points are pulled this fraction toward its
/// centre before testing them against the world, so its frame and hinge
/// don't count.
const LEAF_SHRINK: f32 = 0.2;

/// Loose physics props a door can push or be blocked by: id, box centre
/// and (shrunk) half size, entity space.
pub(super) fn door_props(w: &LogicWorld) -> Vec<(EntId, Vec3, Vec3)> {
    w.ids_of(kinds::PROP)
        .into_iter()
        .filter_map(|id| {
            let p = w.prop(id)?;
            let (lo, hi) = p.bounds?;
            (p.physics && !p.frozen && !p.broken && p.solid).then(|| {
                let half = ((hi - lo) / 2.0 - PROP_BOX_SHRINK).max(Vec3::splat(0.25));
                (id, (lo + hi) / 2.0, half)
            })
        })
        .collect()
}

/// Whether a door's leaf (its hulls placed at `rot`, `origin`) runs into
/// the static world: its points, edge midpoints and centre, pulled
/// toward its centre, tested as small boxes.
fn leaf_in_world(w: &LogicWorld, hulls: &[crate::map::MapHull], rot: Quat, origin: Vec3) -> bool {
    let Some(col) = w.collision.as_deref() else {
        return false;
    };
    let tiny = Vec3::splat(0.5);
    hulls.iter().any(|h| {
        if h.points.is_empty() {
            return false;
        }
        let centre = h.points.iter().copied().sum::<Vec3>() / h.points.len() as f32;
        let mut samples = vec![centre];
        for (i, a) in h.points.iter().enumerate() {
            samples.push(*a);
            for b in &h.points[i + 1..] {
                samples.push((*a + *b) / 2.0);
            }
        }
        samples.into_iter().any(|p| {
            let p = centre + (p - centre) * (1.0 - LEAF_SHRINK);
            col.solid(-tiny, tiny, origin + rot * p)
        })
    })
}

/// Whether something is in the way of the door turning from where it is
/// to `goal` (sampled every 15 degrees): a living player, a loose physics
/// prop, or the static world.
fn swing_blocked(w: &LogicWorld, id: EntId, goal: Vec3) -> bool {
    let Some(e) = w.get(id) else { return false };
    let Class::PropDoor(d) = &e.class else { return false };
    let from = d.push.angles;
    let steps = ((goal - from).length() / 15.0).ceil().max(1.0) as usize;
    let props = door_props(w);
    (1..=steps).any(|k| {
        let a = from + (goal - from) * (k as f32 / steps as f32);
        let rot = entity_rotation(a);
        let brushes: Vec<_> = e.hulls.iter().map(|h| place_hull(h, rot, d.push.origin)).collect();
        let player = w.players.iter().filter(|p| p.alive).any(|p| {
            let (half, centre) = ((p.maxs - p.mins) / 2.0, p.origin + (p.mins + p.maxs) / 2.0);
            brushes.iter().any(|b| b.overlaps_box(centre, half, SOLID_SKIN))
        });
        let prop = || {
            props
                .iter()
                .any(|(_, c, half)| brushes.iter().any(|b| b.overlaps_box(*c, *half, SOLID_SKIN)))
        };
        player || prop() || leaf_in_world(w, &e.hulls, rot, d.push.origin)
    })
}

/// The open angles for an opening by `user` (spec "Direction").
fn open_angles(w: &LogicWorld, id: EntId, user: Option<Who>) -> Vec3 {
    let Some(Class::PropDoor(d)) = w.get(id).map(|e| &e.class) else {
        return Vec3::ZERO;
    };
    match d.opendir {
        1 => return d.open_forward,
        2 => return d.open_back,
        _ => {}
    }
    let Some(at) = user.and_then(|u| origin_of(w, u)) else {
        return d.open_forward;
    };
    let f = forward(Vec3::new(0.0, d.closed.y, 0.0));
    let (first, other) = if f.dot(at) > f.dot(d.push.origin) {
        (d.open_back, d.open_forward)
    } else {
        (d.open_forward, d.open_back)
    };
    // A player opening it into someone: the other way, if that is clear.
    if matches!(user, Some(Who::Player(_))) && swing_blocked(w, id, first) && !swing_blocked(w, id, other) {
        other
    } else {
        first
    }
}

/// The doors that move with this one (`slavename`).
fn slaves(w: &LogicWorld, id: EntId) -> Vec<EntId> {
    let Some(Class::PropDoor(d)) = w.get(id).map(|e| &e.class) else {
        return Vec::new();
    };
    if d.slave.is_empty() {
        return Vec::new();
    }
    w.resolve(&d.slave, None, Some(Who::Ent(id)))
        .into_iter()
        .filter_map(|x| match x {
            Who::Ent(e) if e != id && matches!(w.get(e).map(|e| &e.class), Some(Class::PropDoor(_))) => Some(e),
            _ => None,
        })
        .collect()
}

/// Start turning open (away from `user`) and fire OnOpen.
fn start_open(w: &mut LogicWorld, id: EntId, user: Option<Who>) {
    let target = open_angles(w, id, user);
    let Some(d) = door(w, id) else { return };
    let quiet = d.push.moving();
    let snd = d.move_sound.clone();
    d.state = DoorState::Opening;
    let speed = d.speed;
    d.push.rotate_to(target, speed, Done::PropDoorOpened);
    if let Some(e) = w.get_mut(id) {
        e.next_think = None;
    }
    if !quiet {
        sound(w, id, &snd);
    }
    w.fire_output(id, "OnOpen", Some(Who::Ent(id)), Value::Void);
}

/// Start turning shut and fire OnClose.
fn start_close(w: &mut LogicWorld, id: EntId) {
    let Some(d) = door(w, id) else { return };
    let quiet = d.push.moving();
    let snd = d.move_sound.clone();
    d.state = DoorState::Closing;
    let (closed, speed) = (d.closed, d.speed);
    d.push.rotate_to(closed, speed, Done::PropDoorClosed);
    if let Some(e) = w.get_mut(id) {
        e.next_think = None;
    }
    if !quiet {
        sound(w, id, &snd);
    }
    w.fire_output(id, "OnClose", Some(Who::Ent(id)), Value::Void);
}

/// Open (with its slaves), unless open or opening.
fn open(w: &mut LogicWorld, id: EntId, user: Option<Who>) {
    for d in std::iter::once(id).chain(slaves(w, id)) {
        if door(w, d).is_some_and(|x| !matches!(x.state, DoorState::Open | DoorState::Opening)) {
            start_open(w, d, user);
        }
    }
}

/// Close (with its slaves), unless closed or closing.
fn close(w: &mut LogicWorld, id: EntId) {
    for d in std::iter::once(id).chain(slaves(w, id)) {
        if door(w, d).is_some_and(|x| !matches!(x.state, DoorState::Closed | DoorState::Closing)) {
            start_close(w, d);
        }
    }
}

/// +use or the Use input (spec "Use").
pub(super) fn door_use(w: &mut LogicWorld, id: EntId, user: Option<Who>) {
    let flags = w.get(id).map_or(0, |e| e.spawnflags);
    if flags & SF_DOOR_IGNORE_USE != 0 && matches!(user, Some(Who::Player(_))) {
        return;
    }
    let Some(d) = door(w, id).cloned() else { return };
    let use_closes = flags & SF_DOOR_USE_CLOSES != 0;
    match d.state {
        DoorState::Closed | DoorState::Open if d.state == DoorState::Closed || use_closes => {
            if d.locked {
                sound(w, id, &d.locked_sound);
                w.fire_output(id, "OnLockedUse", user, Value::Void);
            } else {
                // The handle, then the move a moment later (measured: as
                // much for a use that closes it).
                sound(w, id, &d.unlocked_sound);
                let opening = d.state == DoorState::Closed;
                door(w, id).unwrap().pending_use = Some((opening, user));
                let at = w.tick + w.delay_ticks(PROPDOOR_USE_DELAY);
                if let Some(e) = w.get_mut(id) {
                    e.next_think = Some(at);
                }
            }
        }
        DoorState::Opening if use_closes => close(w, id),
        DoorState::Closing => open(w, id, user),
        _ => {}
    }
}

/// prop_door_rotating inputs; false when not one of them.
pub(super) fn door_input(w: &mut LogicWorld, id: EntId, input: &str, value: &Value, activator: Option<Who>) -> bool {
    let Some(d) = door(w, id).cloned() else { return false };
    let opening = matches!(d.state, DoorState::Open | DoorState::Opening);
    match input {
        // Open and Close fire their output here and again as the door
        // starts moving (spec quirk: twice).
        "open" | "openawayfrom" => {
            if d.locked || opening {
                return true;
            }
            let user = if input == "openawayfrom" {
                let Some(name) = w.need_str(value, input) else { return true };
                w.resolve(&name, activator, None).into_iter().next()
            } else {
                None
            };
            // The handle sound too (measured: an Open input plays the
            // unlocked one as it starts the move).
            sound(w, id, &d.unlocked_sound);
            w.fire_output(id, "OnOpen", Some(Who::Ent(id)), Value::Void);
            open(w, id, user);
        }
        "close" => {
            if matches!(d.state, DoorState::Closed | DoorState::Closing) {
                return true;
            }
            w.fire_output(id, "OnClose", Some(Who::Ent(id)), Value::Void);
            close(w, id);
        }
        "toggle" => {
            if opening {
                close(w, id);
            } else if !d.locked {
                open(w, id, None);
            }
        }
        "lock" => door(w, id).unwrap().locked = true,
        "unlock" => door(w, id).unwrap().locked = false,
        "setspeed" => {
            let Some(v) = w.need_float(value, input) else { return true };
            door(w, id).unwrap().speed = v.max(1e-3);
        }
        _ => return false,
    }
    true
}

/// Fully open or fully closed.
pub(super) fn door_arrived(w: &mut LogicWorld, id: EntId, opened: bool) {
    let Some(d) = door(w, id) else { return };
    if opened {
        d.state = DoorState::Open;
        let (snd, delay) = (d.open_sound.clone(), d.return_delay);
        sound(w, id, &snd);
        w.fire_output(id, "OnFullyOpen", Some(Who::Ent(id)), Value::Void);
        if delay >= 0.0 {
            w.think_in(id, delay + PROPDOOR_RETURN_PAD);
        }
    } else {
        d.state = DoorState::Closed;
        let snd = d.close_sound.clone();
        sound(w, id, &snd);
        w.fire_output(id, "OnFullyClosed", Some(Who::Ent(id)), Value::Void);
    }
}

/// A +use's delayed move (`PROPDOOR_USE_DELAY`), or the auto-close think:
/// close if nobody is in the way, else try again after the return delay.
pub(super) fn door_think(w: &mut LogicWorld, id: EntId) {
    let Some(d) = door(w, id).cloned() else { return };
    if let Some((opening, user)) = d.pending_use {
        door(w, id).unwrap().pending_use = None;
        if opening {
            open(w, id, user);
        } else {
            close(w, id);
        }
        return;
    }
    if d.state != DoorState::Open {
        return;
    }
    if swing_blocked(w, id, d.closed) {
        w.think_in(id, d.return_delay.max(0.0));
    } else {
        close(w, id);
    }
}

/// Blocked by a player: it holds still (the push step is undone) and
/// fires its blocked output when the blocking starts; no damage.
pub(super) fn door_blocked(w: &mut LogicWorld, id: EntId, blocker: Who, started: bool) {
    let Some(d) = door(w, id) else { return };
    if started {
        let out = if d.state == DoorState::Opening {
            "OnBlockedOpening"
        } else {
            "OnBlockedClosing"
        };
        w.fire_output(id, out, Some(blocker), Value::Void);
    }
}

impl LogicWorld {
    /// A model door's state.
    pub fn prop_door(&self, id: EntId) -> Option<&PropDoor> {
        match self.get(id).map(|e| &e.class) {
            Some(Class::PropDoor(d)) => Some(d),
            _ => None,
        }
    }
}
