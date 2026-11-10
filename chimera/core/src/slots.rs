//! Slot registries. A game plugin registers its implementations here; the
//! loadout picks one per slot by ID. Nothing outside a game plugin names that
//! plugin's types.
//!
//! `MovementRegistry`, `Loadout`, `set_movement` (swap an entity's movement:
//! the marker goes on with its `MovementSlot` in one move, and two
//! movements on one marker, CS:S's and HL2:DM's, switch by a new value in
//! place: each move alone is an archetype for every combination of the
//! character's other parts, docs/DESIGN.md "ECS style").
//! A movement with a hull can say where its character's feet are and how
//! it stands (`Footing`, `MovementImpl::footing`) and put a character
//! there with its own hull (`MovementImpl::seat`; `RegisterSlots::
//! seat_movement`): `set_movement` hands the old movement's footing to the
//! new one, so a switch between hulls of other heights keeps the feet on
//! the same floor (each movement then frees the hull from solids and puts
//! it down on the floor in its next step, on the server and on the
//! predicting client alike).
//!
//! Per-character identity: a character with `FollowsWeapon` takes its game
//! slots from the game of the weapon it holds (a minigame's `identity =
//! "weapon"`): its movement (the weapon's game's own,
//! `MovementRegistry::of_game`; `chimera_sim::identity` switches it in the
//! predicted tick, on the server and on the client predicting it alike),
//! and its `GameIdentity` (the server's, replicated: whose body and look
//! it wears, whose HUD its player sees).

use bevy::prelude::*;

/// One Movement implementation. Each implementation is driven by a marker
/// component: its systems only act on entities that have it, so swapping
/// movement is "remove one marker, insert another".
pub struct MovementImpl {
    /// Namespaced ID, e.g. `combat_arms:movement`.
    pub id: &'static str,
    /// Puts the marker on with the slot (`MovementSlot`), in one move.
    pub insert: fn(&mut EntityWorldMut, MovementSlot),
    pub remove: fn(&mut EntityWorldMut),
    /// The marker component's type: two movements on the same marker (CS:S's
    /// and HL2:DM's, both `SourceMovement`) switch by a new value in place,
    /// not a removal and an insert (each a new archetype for every
    /// combination of the character's other parts).
    pub marker: std::any::TypeId,
    /// Where a character on this movement stands (None: no hull, as
    /// noclip's).
    pub footing: Option<fn(&World, Entity) -> Option<Footing>>,
    /// Put a character just switched to this movement where `Footing`
    /// says, with this movement's hull (None: it stays where it is).
    pub seat: Option<fn(&mut World, Entity, Footing)>,
}

/// Where a character's feet are and how it stands, as its movement keeps
/// it: what a switch of movement keeps (`set_movement`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footing {
    /// The bottom centre of its hull, engine space (m).
    pub feet: Vec3,
    /// In its crouched hull (or going down into it).
    pub crouched: bool,
    /// On the ground (not falling or jumping).
    pub on_ground: bool,
}

#[derive(Resource, Default)]
pub struct MovementRegistry(pub Vec<MovementImpl>);

impl MovementRegistry {
    /// The implementation registered as `id` (Chimera's own also by their
    /// old namespace: `mashup:noclip` is `chimera:noclip`).
    pub fn get(&self, id: &str) -> Option<&MovementImpl> {
        let renamed = id.strip_prefix("mashup:").map(|rest| format!("chimera:{rest}"));
        let id = renamed.as_deref().unwrap_or(id);
        self.0.iter().find(|m| m.id == id)
    }

    /// Game `game`'s own movement: the first registered whose id is
    /// `game:...` (`cs_source:movement` for `cs_source`).
    pub fn of_game(&self, game: &str) -> Option<&MovementImpl> {
        self.0
            .iter()
            .find(|m| m.id.split_once(':').is_some_and(|(g, _)| g.eq_ignore_ascii_case(game)))
    }

    /// The implementation registered after `id`, wrapping around.
    pub fn next_after(&self, id: &str) -> Option<&MovementImpl> {
        let i = self.0.iter().position(|m| m.id == id)?;
        self.0.get((i + 1) % self.0.len())
    }
}

/// Which Movement implementation an entity currently uses.
#[derive(Component, Debug, Clone, Copy)]
pub struct MovementSlot(pub &'static str);

/// A character whose game slots follow the game of the weapon it holds
/// (a minigame's `identity = "weapon"`): its movement and its
/// `GameIdentity`. Predicted with the player (its client has it too), so
/// both sides switch its movement on the same tick.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FollowsWeapon;

/// The game a character is now (a game id, `cs_source`): whose body and
/// look it wears and whose HUD its player sees, where that follows the
/// character (`FollowsWeapon`). Set by the server, replicated to every
/// client. Without it: the app's (the gameplay's) everywhere.
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GameIdentity(pub String);

impl GameIdentity {
    /// Whether `e`'s identity (None: none) lets game `game`'s per-character
    /// parts act on it: no identity, or `game`'s.
    pub fn allows(identity: Option<&GameIdentity>, game: &str) -> bool {
        identity.is_none_or(|i| i.0.eq_ignore_ascii_case(game))
    }
}

/// The match loadout: one implementation ID per slot. Owned by the server in
/// multiplayer (README, "Multiplayer").
#[derive(Resource, Debug, Clone)]
pub struct Loadout {
    pub movement: &'static str,
}

pub trait RegisterSlots {
    /// Register a Movement implementation driven by marker component `M`.
    fn register_movement<M: Component + Default>(&mut self, id: &'static str) -> &mut Self;
    /// Movement `id`'s `footing` and `seat` (registered before).
    fn seat_movement(
        &mut self,
        id: &'static str,
        footing: fn(&World, Entity) -> Option<Footing>,
        seat: fn(&mut World, Entity, Footing),
    ) -> &mut Self;
}

impl RegisterSlots for App {
    fn register_movement<M: Component + Default>(&mut self, id: &'static str) -> &mut Self {
        fn insert<M: Component + Default>(e: &mut EntityWorldMut, slot: MovementSlot) {
            e.insert((M::default(), slot));
        }
        fn remove<M: Component>(e: &mut EntityWorldMut) {
            e.remove::<M>();
        }
        self.init_resource::<MovementRegistry>()
            .world_mut()
            .resource_mut::<MovementRegistry>()
            .0
            .push(MovementImpl {
                id,
                insert: insert::<M>,
                remove: remove::<M>,
                marker: std::any::TypeId::of::<M>(),
                footing: None,
                seat: None,
            });
        self
    }

    fn seat_movement(
        &mut self,
        id: &'static str,
        footing: fn(&World, Entity) -> Option<Footing>,
        seat: fn(&mut World, Entity, Footing),
    ) -> &mut Self {
        let mut registry = self.world_mut().resource_mut::<MovementRegistry>();
        let m = registry.0.iter_mut().find(|m| m.id == id).expect("seat_movement: movement registered first");
        m.footing = Some(footing);
        m.seat = Some(seat);
        self
    }
}

/// Switch `entity` to the Movement implementation `id`. Velocity carries
/// over; where both movements have a hull (`MovementImpl::footing`,
/// `seat`), the new one is seated on the old one's feet, crouched as it
/// was; the new implementation takes it from there.
pub fn set_movement(entity: Entity, id: &'static str) -> impl Command {
    move |world: &mut World| {
        let registry = world.resource::<MovementRegistry>();
        let Some(new) = registry.get(id) else {
            warn!("unknown movement implementation {id}");
            return;
        };
        let (insert, seat, marker) = (new.insert, new.seat, new.marker);
        let old = world.get::<MovementSlot>(entity).and_then(|s| registry.get(s.0));
        let footing = old.filter(|o| o.id != id).and_then(|o| o.footing);
        // The other markers go; the new one's own is replaced in place.
        let removes: Vec<_> = registry.0.iter().filter(|m| m.marker != marker).map(|m| m.remove).collect();
        let footing = footing.and_then(|f| f(world, entity));
        let Ok(mut e) = world.get_entity_mut(entity) else {
            return;
        };
        for remove in removes {
            remove(&mut e);
        }
        insert(&mut e, MovementSlot(id));
        if let (Some(seat), Some(footing)) = (seat, footing) {
            seat(world, entity, footing);
        }
        info!("{entity}: movement -> {id}");
    }
}
