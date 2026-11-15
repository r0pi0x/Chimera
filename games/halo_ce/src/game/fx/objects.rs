//! Halo's objects seen on every machine (specs/halo_ce/projectiles.md 15 to
//! 17, effects.md "The rest of the arsenal", "Grenades",
//! multiplayer_rules.md 5; objects.md "Equipment"): thrown grenades, the
//! projectiles' looks in flight, detonations, a plasma grenade's countdown,
//! items lying on the map and their pickup sound.
//!
//! `halo_ce_blast <projectile> <x> <y> <z>` (console) shows a projectile's
//! detonation at a point on this machine alone, without damage (pictures
//! of the effects: docs/OBSERVABILITY.md's offscreen frame dumps).
//!
//! - **Grenades** (`show_grenades`, the authority): the server's own
//!   grenade projectiles (`weapons::systems::HaloProjectile`, frag and
//!   plasma) once out of the hand get a `Transform` from their flight each
//!   tick, their model as a shown item (`view::load::object_key`) and the
//!   engine's `ShownBody`, so the network's items (`NetItem`) carry them
//!   to every client as they bounce, roll, rest or stick (a plasma grenade
//!   stuck in a character moves with it on the server): the server's
//!   entities are replicated, not flown again from the throw, so every
//!   machine sees the one bounce the server computed (a bouncing object
//!   flown again would part from it at the first bounce off a moving body).
//! - **In flight** (`trails`, every machine, each Halo tick): every shown
//!   Halo object of a projectile's key (the grenades above as the server or
//!   a client draws them, the rockets and needles `fx::shots` flies again)
//!   makes its projectile's light (the grenades' glow) and its effect
//!   attachments where it is (the rocket's exhaust, the frag's smoke, the
//!   plasma grenade's armed glow).
//! - **Detonations** (`blasts`): the server's `HaloDetonation`s, to its own
//!   presentation and to every client (`HaloBlastNet`), become `HaloBlast`
//!   on each machine: the projectile's detonation effect (the supercombine's
//!   for a needler's super detonation) at the point, its "" location
//!   along the flight, "normal" against it (up for a fall), "gravity" down
//!   (the rocket's and grenades' sound, light, char decal and smoke; the
//!   authority places the decals); a needle shown stuck there (`fx::shots`)
//!   goes with it.
//! - **The countdown** (`HaloStuck` of a projectile with a "detonation
//!   started" effect: the plasma grenade, `HaloStuckNet` to the clients):
//!   that effect where it stuck.
//! - **Items** (`show_items`, the authority): each `pickups::HaloItem`
//!   gets a look entity of its own (`HaloItemLook`: its item's model,
//!   `view::load::item_key`, as a shown item with `ShownBody`, replicated
//!   as the grenades are), resting on the level below it unless its entry
//!   levitates, gone when the item is (taken or expired); a taken item's
//!   pickup sound plays for its taker (`HaloTaken`, `HaloTakenNet`).
//!
//! The network part `halo_ce_objects`: `HaloBlastNet`, `HaloStuckNet` and
//! `HaloTakenNet` from the server (the grenades and items ride the engine's
//! `NetItem`).
//!
//! Known shortcuts: a grenade in the thrower's hand (the six ticks between
//! its creation and release) is not drawn; a grenade's spin is not shown
//! (it faces its velocity, or keeps its last facing at rest); an item
//! "falls" at once to the level below its entry (no flight); the
//! countdown's effect stays where the grenade stuck (it does not follow a
//! moving body); a needle shown stuck goes with a detonation of its tag
//! within 1.5 m (or after `shots::STUCK_TICKS`), not with its own; particle
//! systems of the effects (the explosions' fireballs, the rocket's exhaust
//! plume) are not drawn (`load`'s shortcut); the shock waves' camera shake
//! is not applied.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use chimera_sim::weapon::projectile::ShownBody;
use serde::{Deserialize, Serialize};

use super::{
    ActiveFx, HaloLight,
    place::Frame,
    play::{Effects, Instance, Made, start},
    shots::{HaloShown, facing, instances_from},
};
use super::super::{
    METERS_PER_UNIT,
    view::load::{OBJECT_PREFIX, item_key, object_key},
    vitality::Spartan,
    weapons::{
        grenades::GrenadeData,
        halo_ticks,
        pickups::{HaloItem, HaloTaken, NetgameEntry, item_tag},
        systems::{FRAG_GRENADE, HaloDetonation, HaloProjectile, HaloStuck, Holder, PLASMA_GRENADE},
    },
};
use crate::{
    core::{Intent, LocalPlayer, NetRole, SOLID_LAYERS, SimClock, SimSet},
    map::{PlaySound, loose::ShownItem, particles::Particles},
};

/// A detonation as every machine shows it: the projectile's tag, where
/// (metres), its flight's direction, whether a supercombine.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloBlast {
    pub tag: String,
    pub at: Vec3,
    pub forward: Vec3,
    pub supercombine: bool,
}

/// A countdown begun (a plasma grenade stuck or landed): its tag, where.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloCountdown {
    pub tag: String,
    pub at: Vec3,
}

/// An item taken, as its taker's machine hears it.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloPickedUp {
    pub unit: Entity,
    pub tag: String,
}

/// Server -> clients: a detonation (`HaloBlast`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HaloBlastNet {
    pub tag: String,
    pub at: [f32; 3],
    pub forward: [f32; 3],
    pub supercombine: bool,
}

/// Server -> clients: a countdown begun (`HaloCountdown`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HaloStuckNet {
    pub tag: String,
    pub at: [f32; 3],
}

/// Server -> clients: an item taken (`HaloPickedUp`; its taker's client
/// plays the sound).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct HaloTakenNet {
    #[entities]
    pub unit: Entity,
    pub tag: String,
}

/// An item's look (module doc): the item it shows.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HaloItemLook {
    pub item: Entity,
}

fn authority(role: Option<&NetRole>) -> bool {
    role.is_none_or(|r| *r != NetRole::Client)
}

/// The server's grenades out of the hand, drawn and replicated (module
/// doc).
#[allow(clippy::type_complexity)]
pub fn show_grenades(projectiles: Query<(Entity, &HaloProjectile, Option<&mut Transform>, Has<ShownItem>)>, mut commands: Commands) {
    let mut projectiles = projectiles;
    for (e, p, at, shown) in &mut projectiles {
        if (p.id != FRAG_GRENADE && p.id != PLASMA_GRENADE) || matches!(p.holder, Holder::Hand { .. }) {
            continue;
        }
        let position = p.flight.position * METERS_PER_UNIT;
        let moving = p.flight.velocity.length_squared() > 1e-8;
        match at {
            Some(mut t) => {
                t.translation = position;
                if moving {
                    t.rotation = facing(position, p.flight.velocity).rotation;
                }
            }
            None => {
                commands.entity(e).insert(facing(position, p.flight.velocity));
            }
        }
        if !shown {
            commands.entity(e).insert((ShownItem(object_key(&p.proj.tag)), ShownBody { owner: Some(p.owner) }));
        }
    }
}

/// The lights and effect attachments of the shown projectiles (module doc).
pub fn trails(
    shown: Query<(&ShownItem, &Transform)>,
    fx: Res<ActiveFx>,
    clock: Res<SimClock>,
    mut effects: ResMut<Effects>,
    mut lights: MessageWriter<HaloLight>,
    mut particles: ResMut<Particles>,
) {
    let Some(lib) = fx.0.clone() else { return };
    if halo_ticks(&clock) == 0 {
        return;
    }
    for (item, t) in &shown {
        let Some(tag) = item.0.strip_prefix(OBJECT_PREFIX) else { continue };
        let Some(look) = lib.projectile(tag) else { continue };
        let forward = t.rotation * Vec3::NEG_Z;
        if let Some(l) = &look.light {
            lights.write(HaloLight { at: t.translation, color: Vec3::from(l.color), radius: l.radius * METERS_PER_UNIT, seconds: 1.5 / 30.0, falloff: 0, owner: None, follow: false });
        }
        for (effect, _marker) in &look.attached {
            let f = Frame::facing(t.translation, -forward);
            let inst = lib.effects[*effect].locations.iter().map(|_| vec![Instance::both(f)]).collect();
            start(&mut effects, &lib, *effect, inst, |_| {});
        }
        // Its attached particle systems: a Halo tick of their rate, at the
        // projectile (the marker's offset is not kept).
        for (system, _marker) in &look.attached_systems {
            let Some(sys) = lib.systems.get(*system) else { continue };
            let mut made = Made::default();
            super::play::emit(sys, &Frame::facing(t.translation, -forward), 1.0 / 30.0, &mut effects.rng, &mut made);
            for g in made.groups {
                particles.add(g);
            }
        }
    }
}

/// The server's detonations, countdowns and pickups to its own
/// presentation (module doc).
pub fn from_simulation(
    mut detonations: MessageReader<HaloDetonation>,
    mut stuck: MessageReader<HaloStuck>,
    mut taken: MessageReader<HaloTaken>,
    fx: Res<ActiveFx>,
    mut blasts: MessageWriter<HaloBlast>,
    mut countdowns: MessageWriter<HaloCountdown>,
    mut picked: MessageWriter<HaloPickedUp>,
) {
    for d in detonations.read() {
        blasts.write(HaloBlast { tag: d.tag.clone(), at: d.at, forward: d.forward, supercombine: d.supercombine });
    }
    for s in stuck.read() {
        if fx.0.as_ref().and_then(|l| l.projectile(&s.tag)).is_some_and(|p| p.detonation_started.is_some()) {
            countdowns.write(HaloCountdown { tag: s.tag.clone(), at: s.at });
        }
    }
    for t in taken.read() {
        picked.write(HaloPickedUp { unit: t.unit, tag: t.tag.clone() });
    }
}

/// The frames of a detonation's locations (module doc).
pub fn blast_frames(at: Vec3, forward: Vec3) -> std::collections::BTreeMap<&'static str, Frame> {
    let forward = forward.normalize_or(Vec3::NEG_Y);
    let normal = if forward.y < -0.7 { Vec3::Y } else { -forward };
    std::collections::BTreeMap::from([
        ("", Frame::facing(at, forward)),
        ("normal", Frame::facing(at, normal)),
        ("gravity", Frame::facing(at, Vec3::NEG_Y)),
    ])
}

/// Detonations and countdowns drawn and heard (module doc).
pub fn blasts(
    mut blasts: MessageReader<HaloBlast>,
    mut countdowns: MessageReader<HaloCountdown>,
    shown: Query<(Entity, &HaloShown, &Transform)>,
    fx: Res<ActiveFx>,
    role: Option<Res<NetRole>>,
    mut effects: ResMut<Effects>,
    mut commands: Commands,
) {
    let Some(lib) = fx.0.clone() else {
        blasts.clear();
        countdowns.clear();
        return;
    };
    let decals = authority(role.as_deref());
    for b in blasts.read() {
        // A needle shown stuck goes with its detonation.
        for (e, s, t) in &shown {
            if s.stuck.is_some() && s.proj.tag.eq_ignore_ascii_case(&b.tag) && t.translation.distance(b.at) < 1.5 {
                commands.entity(e).despawn();
            }
        }
        let Some(look) = lib.projectile(&b.tag) else { continue };
        let effect = if b.supercombine { look.super_detonation.or(look.detonation) } else { look.detonation };
        let Some(effect) = effect else { continue };
        let frames = blast_frames(b.at, b.forward);
        let inst = instances_from(&lib, effect, &frames, frames[""]);
        start(&mut effects, &lib, effect, inst, |r| r.decals = decals);
    }
    for c in countdowns.read() {
        let Some(effect) = lib.projectile(&c.tag).and_then(|p| p.detonation_started) else { continue };
        let frames = blast_frames(c.at, Vec3::NEG_Y);
        let inst = instances_from(&lib, effect, &frames, frames["normal"]);
        start(&mut effects, &lib, effect, inst, |r| r.decals = decals);
    }
}

/// A taken item's pickup sound for its taker (module doc).
pub fn pickup_sounds(
    mut picked: MessageReader<HaloPickedUp>,
    fx: Res<ActiveFx>,
    locals: Query<(), With<LocalPlayer>>,
    role: Option<Res<NetRole>>,
    mut play: MessageWriter<PlaySound>,
) {
    for p in picked.read() {
        // Without a local player (a test, not a server) every pickup is heard.
        let mine = locals.contains(p.unit) || (locals.is_empty() && role.as_deref().is_none_or(|r| *r != NetRole::Server));
        if !mine {
            continue;
        }
        if let Some(sound) = fx.0.as_ref().and_then(|l| l.pickups.get(&p.tag.to_ascii_lowercase())) {
            play.write(PlaySound { at: None, source: Some(p.unit), ..PlaySound::at(sound.clone(), Vec3::ZERO) });
        }
    }
}

/// How far an item looks for the level below it (metres).
const REST_REACH: f32 = 6.0;

/// The items' looks (module doc), on the authority.
#[allow(clippy::too_many_arguments)]
pub fn show_items(
    items: Query<(Entity, &HaloItem, &Transform)>,
    entries: Query<&NetgameEntry>,
    looks: Query<(Entity, &HaloItemLook)>,
    characters: Query<Entity, With<Intent>>,
    spatial: SpatialQuery,
    loose: Option<Res<crate::map::loose::LooseAssets>>,
    grenades: Res<GrenadeData>,
    spartan: Option<Res<Spartan>>,
    mut commands: Commands,
) {
    let shown: Vec<Entity> = looks.iter().map(|(_, l)| l.item).collect();
    for (e, l) in &looks {
        if !items.contains(l.item) {
            commands.entity(e).despawn();
        }
    }
    let Some(spartan) = spartan else { return };
    for (e, item, t) in &items {
        if shown.contains(&e) {
            continue;
        }
        let tag = item_tag(&item.item, &grenades, &spartan);
        let key = item_key(&tag);
        let levitate = item.entry.is_some_and(|i| entries.iter().any(|n| n.index == i && n.levitate));
        let half = loose.as_ref().and_then(|l| l.0.get(&key)).map_or(0.05, |a| a.half.y);
        let mut at = t.translation;
        if !levitate {
            let filter = SpatialQueryFilter::from_excluded_entities(characters.iter()).with_mask(SOLID_LAYERS);
            let from = at + Vec3::Y * 0.1;
            if let Some(hit) = spatial.cast_ray(from, Dir3::NEG_Y, REST_REACH, true, &filter) {
                at = from - Vec3::Y * hit.distance + Vec3::Y * half;
            }
        }
        commands.spawn((
            Name::new("halo_ce item look"),
            HaloItemLook { item: e },
            ShownItem(key),
            ShownBody { owner: None },
            Transform::from_translation(at),
        ));
    }
}

/// The server's messages to the clients (module doc).
fn send(
    mut blasts: MessageReader<HaloDetonation>,
    mut stuck: MessageReader<HaloStuck>,
    mut taken: MessageReader<HaloTaken>,
    mut out_blasts: MessageWriter<bevy_replicon::prelude::ToClients<HaloBlastNet>>,
    mut out_stuck: MessageWriter<bevy_replicon::prelude::ToClients<HaloStuckNet>>,
    mut out_taken: MessageWriter<bevy_replicon::prelude::ToClients<HaloTakenNet>>,
) {
    use bevy_replicon::prelude::{SendTargets, ToClients};
    for d in blasts.read() {
        let message = HaloBlastNet { tag: d.tag.clone(), at: d.at.to_array(), forward: d.forward.to_array(), supercombine: d.supercombine };
        out_blasts.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message });
    }
    for s in stuck.read() {
        out_stuck.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: HaloStuckNet { tag: s.tag.clone(), at: s.at.to_array() } });
    }
    for t in taken.read() {
        out_taken.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: HaloTakenNet { unit: t.unit, tag: t.tag.clone() } });
    }
}

/// A client's copies of the server's messages.
fn receive(
    mut blasts: MessageReader<HaloBlastNet>,
    mut stuck: MessageReader<HaloStuckNet>,
    mut taken: MessageReader<HaloTakenNet>,
    fx: Res<ActiveFx>,
    mut out_blasts: MessageWriter<HaloBlast>,
    mut out_countdowns: MessageWriter<HaloCountdown>,
    mut out_picked: MessageWriter<HaloPickedUp>,
) {
    for b in blasts.read() {
        out_blasts.write(HaloBlast { tag: b.tag.clone(), at: Vec3::from(b.at), forward: Vec3::from(b.forward), supercombine: b.supercombine });
    }
    for s in stuck.read() {
        if fx.0.as_ref().and_then(|l| l.projectile(&s.tag)).is_some_and(|p| p.detonation_started.is_some()) {
            out_countdowns.write(HaloCountdown { tag: s.tag.clone(), at: Vec3::from(s.at) });
        }
    }
    for t in taken.read() {
        out_picked.write(HaloPickedUp { unit: t.unit, tag: t.tag.clone() });
    }
}

fn register_net(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.add_server_message::<HaloBlastNet>(Channel::Ordered)
        .add_server_message::<HaloStuckNet>(Channel::Ordered)
        .add_mapped_server_message::<HaloTakenNet>(Channel::Ordered)
        .add_systems(PostUpdate, send.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

pub(super) fn plugin(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    app.add_message::<HaloBlast>()
        .add_message::<HaloCountdown>()
        .add_message::<HaloPickedUp>()
        .add_message::<HaloDetonation>()
        .add_message::<HaloStuck>()
        .add_message::<HaloTaken>()
        .add_systems(
            FixedUpdate,
            (
                (show_grenades, show_items, from_simulation)
                    .after(SimSet::Weapons)
                    .after(super::super::weapons::systems::settle_new)
                    .run_if(crate::core::authoritative),
                (trails, blasts, pickup_sounds).after(SimSet::Weapons),
            )
                .chain(),
        )
        .net_part("halo_ce_objects", register_net);
    use chimera_core::console::ConsoleAppExt;
    app.console_command(
        "halo_ce_blast",
        "halo_ce_blast <projectile> <x> <y> <z>: show the detonation of the first projectile whose tag path holds <projectile> (`_` for spaces, e.g. frag_grenade, rocket, needle) at that point (engine metres) on this machine (pictures of the effects)",
        |w, args| {
            let [name, x, y, z] = args else { return Err("expected <projectile> <x> <y> <z>".into()) };
            let at: Vec<f32> = [x, y, z].iter().map(|a| a.parse::<f32>()).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
            let name = name.replace('_', " ").to_ascii_lowercase();
            let tag = w
                .resource::<super::ActiveFx>()
                .0
                .as_ref()
                .and_then(|lib| lib.projectiles.keys().find(|k| k.contains(&name)).cloned())
                .ok_or_else(|| format!("no projectile like {name}"))?;
            w.write_message(HaloBlast { tag: tag.clone(), at: Vec3::new(at[0], at[1], at[2]), forward: Vec3::NEG_Y, supercombine: false });
            Ok(Some(format!("{tag} detonates")))
        },
    );
}
