//! Others' weapons on a network client (docs/plans/active/multiplayer.md,
//! slice 4): what a client sees of characters it doesn't play.
//!
//! - **What they hold** (`show_held`): a character drawn from snapshots
//!   carries one weapon, built from the registry like any weapon, so its
//!   body holds the right model and plays the right animations. It never
//!   runs the weapon frame (the weapon systems leave out
//!   `map::interp::NetDrawn` characters).
//! - **Their shots** (`RemoteShot`): the server sends each round someone
//!   else fires with its origin, angles, spread seed and spread
//!   (`WeaponEventKind::Fired`); the client traces the same pellets
//!   (`pellet_dirs`) through its own world with no damage (a client deals
//!   none), so their tracers, impacts and decals show and the fire sound
//!   plays, with the same spread pattern the server traced. For weapons
//!   with `Hitscan::tracer_every` the server flags the round
//!   (`shot_flags`: a tracer on each bullet, any impact at all): a
//!   `WeaponEventKind::Redrawn` goes before its `Shot` events, and with
//!   `Hitscan::redrawn_through_characters` the trace passes every
//!   character (HL2:DM, specs/hl2dm/effects_sounds.md 1). A round of a
//!   weapon's `AltShot` (mode 1 on a weapon without `AltModes`) is drawn
//!   with that shot's pellets (the shotgun's double shot: 12).
//! - **What else they do** (`RemoteAction`): reloads, swings and throws,
//!   for their bodies' animations.
//!
//! Covers: what a network client draws of others' weapons (`show_held`: one
//! weapon built from the registry in their hands; `RemoteShot`: their rounds
//! traced again with no damage for tracers, impacts and the fire sound, as
//! the server flagged them (`shot_flags`, `WeaponEventKind::Redrawn`);
//! `RemoteAction`: reloads, swings, throws for their bodies).

use bevy::prelude::*;

use super::{
    AltModes, AltShot, DamageEffect, Hitscan, Inventory, Penetration, SpreadShape, Weapon, WeaponEvent, WeaponEventKind,
    WeaponSounds, deliver, grenade::Throwable,
};
use crate::core::SimClock;

/// A round someone else fired, as the server traced it: draw it here.
#[derive(Message, Clone, Debug)]
pub struct RemoteShot {
    /// The character (this client's copy).
    pub shooter: Entity,
    pub weapon: &'static str,
    pub origin: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub seed: u32,
    pub spread: SpreadShape,
    pub mode: u8,
    /// What the server says of the round (`shot_flags`; 0: nothing, the
    /// games draw it by their own counts).
    pub flags: u8,
}

/// What a server says of a round it sends other clients, for weapons with
/// `Hitscan::tracer_every` (Source's fire-bullets message, specs/hl2dm/
/// effects_sounds.md 1).
pub mod shot_flags {
    /// The weapon's tracers follow the server's count: the other bits
    /// mean something.
    pub const FLAGGED: u8 = 1;
    /// The server's count picked one of the round's bullets for a tracer:
    /// every bullet draws one here.
    pub const TRACERS: u8 = 2;
    /// One of its bullets hit something (else no impact is drawn here).
    pub const IMPACTS: u8 = 4;
}

/// Something else someone else's weapon did (for its body's animation).
#[derive(Message, Clone, Debug)]
pub struct RemoteAction {
    pub shooter: Entity,
    pub kind: WeaponEventKind,
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<RemoteShot>()
        .add_message::<RemoteAction>()
        .init_resource::<FireSounds>()
        .add_systems(
            Update,
            (remember_fire_sounds, remote_shots, remote_actions)
                .chain()
                .run_if(crate::core::shows_others),
        );
}

/// The fire sounds of every weapon seen here, by weapon ID: a shooter whose
/// gun changed since the round left it (the gun game swaps it on the kill)
/// still has that round's sound played.
#[derive(Resource, Default)]
struct FireSounds(Vec<(&'static str, Option<String>, Option<String>)>);

fn remember_fire_sounds(new: Query<(&Weapon, &WeaponSounds), Added<WeaponSounds>>, mut known: ResMut<FireSounds>) {
    for (w, s) in &new {
        if !known.0.iter().any(|(id, ..)| *id == w.id) {
            known.0.push((w.id, s.fire.clone(), s.fire_alt.clone()));
        }
    }
}

/// Make `character` hold weapon `id` (None: nothing) in mode `mode`, its
/// grenade's pin out when `primed`: what a client draws of a character
/// it doesn't play.
pub fn show_held(world: &mut World, character: Entity, id: Option<&str>, mode: u8, primed: bool) {
    if world.get_entity(character).is_err() {
        return;
    }
    let held = world
        .get::<Inventory>(character)
        .and_then(|i| i.active)
        .filter(|w| world.get::<Weapon>(*w).map(|x| x.id) == id.and_then(|id| world.resource::<super::WeaponRegistry>().find(id).map(|d| d.id)));
    let weapon = match held {
        Some(w) => Some(w),
        None => {
            let old = world
                .get::<Inventory>(character)
                .map(|i| i.weapons.clone())
                .unwrap_or_default();
            for w in old {
                if let Ok(e) = world.get_entity_mut(w) {
                    e.despawn();
                }
            }
            let w = id.and_then(|id| super::spawn_weapon(world, id, character));
            world.entity_mut(character).insert(Inventory {
                weapons: w.into_iter().collect(),
                active: w,
                ..default()
            });
            w
        }
    };
    let Some(w) = weapon else { return };
    if let Some(mut m) = world.get_mut::<AltModes>(w)
        && m.current != mode
    {
        m.current = mode;
    }
    if let Some(mut t) = world.get_mut::<Throwable>(w)
        && t.pin != primed
    {
        t.pin = primed;
    }
}

/// Others' rounds traced again here: tracers, impacts and decals from the
/// `Shot` events, and the fire sound.
fn remote_shots(
    mut shots: MessageReader<RemoteShot>,
    owners: Query<&Inventory>,
    weapons: Query<(
        &Weapon,
        Option<&Hitscan>,
        Option<&DamageEffect>,
        Option<&Penetration>,
        Option<&WeaponSounds>,
        (Option<&AltShot>, Has<AltModes>),
    )>,
    mut world: deliver::World,
    clock: Res<SimClock>,
    known: Res<FireSounds>,
) {
    for s in shots.read() {
        let held = owners
            .get(s.shooter)
            .ok()
            .and_then(|i| i.active)
            .and_then(|a| weapons.get(a).ok().map(|w| (a, w)));
        let Some((active, (_, Some(scan), Some(effect), pen, sounds, (alt_shot, modes)))) = held.filter(|(_, (w, ..))| w.id == s.weapon)
        else {
            // The gun in hand is another now: no trace to draw, but the
            // shot's sound still plays.
            let fire = known.0.iter().find(|(id, ..)| *id == s.weapon);
            let entry = fire.and_then(|(_, f, alt)| if s.mode > 0 && alt.is_some() { alt.clone() } else { f.clone() });
            if let Some(entry) = entry {
                world.sound(super::owner_sound(entry, s.shooter, s.origin, super::CHAN_WEAPON));
            }
            continue;
        };
        // An `AltShot`'s round (mode 1 on a weapon with no modes of its
        // own: the shotgun's double shot) has its pellets.
        let pellets = match alt_shot {
            Some(a) if s.mode > 0 && !modes => a.pellets,
            _ => scan.pellets,
        };
        let scan = Hitscan {
            spread: s.spread,
            pellets,
            ..scan.clone()
        };
        let flagged = s.flags & shot_flags::FLAGGED != 0;
        let (tracers, impacts) = (s.flags & shot_flags::TRACERS != 0, s.flags & shot_flags::IMPACTS != 0);
        if flagged {
            world.event(s.shooter, active, WeaponEventKind::Redrawn { tracers });
        }
        let mut ctx = deliver::Shot {
            owner: s.shooter,
            weapon: active,
            eye: s.origin,
            aim: deliver::aim_of(s.yaw, s.pitch),
            yaw: s.yaw,
            pitch: s.pitch,
            seed: s.seed,
            now: clock.now,
            rewound: Vec::new(),
            through_characters: scan.redrawn_through_characters,
            show_hits: !flagged || impacts,
            round: (0, 1),
            traced: false,
            w: &mut world,
        };
        if !flagged || tracers || impacts {
            ctx.fire(&scan, effect, pen);
        }
        let sound = sounds.and_then(|x| match (s.mode > 0, &x.fire_alt) {
            (true, Some(alt)) => Some(alt.clone()),
            _ => x.fire.clone(),
        });
        if let Some(entry) = sound {
            world.sound(super::owner_sound(entry, s.shooter, s.origin, super::CHAN_WEAPON));
        }
    }
}

/// Others' reloads, swings and throws as weapon events of what they hold.
fn remote_actions(
    mut actions: MessageReader<RemoteAction>,
    owners: Query<&Inventory>,
    mut events: MessageWriter<WeaponEvent>,
) {
    for a in actions.read() {
        let Some(weapon) = owners.get(a.shooter).ok().and_then(|i| i.active) else {
            continue;
        };
        events.write(WeaponEvent {
            owner: a.shooter,
            weapon,
            kind: a.kind.clone(),
            replay: false,
        });
    }
}
