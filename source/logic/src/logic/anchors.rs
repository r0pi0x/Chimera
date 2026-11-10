//! Entities parented to something the ECS moves (`map::entities::
//! EntityAnchors`: placed weapons, which lie loose and are picked up):
//! each keeps its placement relative to its parent from map load, as
//! Source's parenting does (the child's local transform is taken at
//! spawn), and moves with it. Brushes (`Attached`) and triggers move
//! their volumes along; other entities their origin and angles (aim
//! points, game_ui's facing check, explosions).
//!
//! Covers: entities parented to something that moves follow it: a placed weapon
//! (its pose from `map::entities::EntityAnchors`), a physics brush or prop or a
//! mover, from map load or a SetParent at run time (ClearParent lets go,
//! SetParentAttachment snaps to the parent); triggers and brushes move their
//! volumes (a brush riding a physics body touches no physics body, so it
//! doesn't shove its carrier, but shots still hit it: `rides_body`, the
//! bridge's `sync_movers`), mover children (a door on a lift) are carried along with their own
//! motion kept, drawn parts go where their entity is (`part_poses`); players
//! SetParent to an entity ride it (`player_follows`; the bridge sets
//! `core::MapControls::parented`, which stops their movement, physics shadow
//! and push-away) until ClearParent or death.
//!
//! Known shortcuts (docs/tech-debt.md): Entities parented to a placed weapon
//! (no spec; public entity docs): the weapon's place in its owner's hands is
//! its owner's feet turned to its yaw; props riding it have no collision (as
//! other parented props); a network client draws the anchor in the past
//! (`net::anchors`, as other players), so the item its own player carries
//! trails it by the interpolation delay. Why it's OK for now: mg_item_battle's
//! items work.

use bevy::prelude::*;

use super::classes::Class;
use super::world::{EntId, LogicWorld, Who, place_hull};
use crate::map::entities::{anchor_class, entity_rotation, parent_name};

/// A child's place relative to its parent (entity space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    /// An entity, or a player (SetParent to `!activator`).
    pub parent: Who,
    pub offset: Vec3,
    pub rotation: Quat,
}

/// A class the physics moves (func_physbox, physics props): what is
/// parented to one follows its body.
pub fn is_physics_body(classname: &str) -> bool {
    super::prop_damage::is_physbox(classname) || classname.to_ascii_lowercase().starts_with("prop_physics")
}

/// Source-style angles (pitch, yaw, roll; degrees) of a rotation in
/// entity space (the inverse of `entity_rotation`).
pub fn angles_of(q: Quat) -> Vec3 {
    let (yaw, pitch, roll) = q.to_euler(EulerRot::ZYX);
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

impl LogicWorld {
    /// Link every entity parented (directly, or through other children)
    /// to something that moves (an anchor entity, `map::entities::
    /// anchor_class`; a physics brush; a mover) to its parent: its place
    /// relative to the parent now. Run after activation.
    pub fn link_anchored(&mut self) {
        let ids = self.ids();
        // The first entity of each name.
        let mut by_name: std::collections::HashMap<String, EntId> = std::collections::HashMap::new();
        for &id in &ids {
            let e = self.get(id).unwrap();
            if !e.targetname.is_empty() {
                by_name.entry(e.targetname.to_ascii_lowercase()).or_insert(id);
            }
        }
        let parent_of = |w: &LogicWorld, id: EntId| -> Option<EntId> {
            let p = w.get(id)?.kv("parentname").map(parent_name)?;
            by_name.get(&p.to_ascii_lowercase()).copied().filter(|p| *p != id)
        };
        // Entities something else moves: placed weapons, physics
        // brushes and physics props (the physics), movers (their own
        // logic).
        let moves = |w: &LogicWorld, id: EntId| {
            w.get(id).is_some_and(|e| {
                anchor_class(&e.classname) || is_physics_body(&e.classname) || super::movers::pusher(&e.class).is_some()
            })
        };
        // How many moving ancestors each entity has (0: stays put).
        let mut depth: std::collections::HashMap<EntId, u32> = std::collections::HashMap::new();
        fn depth_of(
            w: &LogicWorld,
            id: EntId,
            parent_of: &dyn Fn(&LogicWorld, EntId) -> Option<EntId>,
            moves: &dyn Fn(&LogicWorld, EntId) -> bool,
            memo: &mut std::collections::HashMap<EntId, u32>,
            guard: u32,
        ) -> u32 {
            if let Some(d) = memo.get(&id) {
                return *d;
            }
            let d = match parent_of(w, id) {
                Some(p) if guard < 32 => {
                    let pd = depth_of(w, p, parent_of, moves, memo, guard + 1);
                    if pd > 0 || moves(w, p) { pd + 1 } else { 0 }
                }
                _ => 0,
            };
            memo.insert(id, d);
            d
        }
        let mut children: Vec<(u32, EntId, EntId)> = Vec::new();
        for &id in &ids {
            let d = depth_of(self, id, &parent_of, &moves, &mut depth, 0);
            if d == 0 {
                continue;
            }
            let parent = parent_of(self, id).unwrap();
            let e = self.get(id).unwrap();
            let parent_moves_itself = self
                .get(parent)
                .is_some_and(|p| super::movers::pusher(&p.class).is_some());
            // Parented brushes and breakables follow a mover their own
            // way (`movers::follow_parents`), and props ride the mover's
            // node (the bridge).
            let own_way = super::movers::attached(&e.class).is_some_and(|a| a.parent.is_some())
                || (parent_moves_itself && matches!(e.class, Class::Prop(_) | Class::PropDoor(_)));
            if !own_way {
                children.push((d, id, parent));
            }
        }
        // Parents before children.
        children.sort_by_key(|(d, ..)| *d);
        for (_, id, parent) in children {
            self.attach(id, Who::Ent(parent));
        }
    }

    /// Parent `id` to `parent` where both are now (keeping its place
    /// relative to the parent), replacing any parent it had.
    pub fn attach(&mut self, id: EntId, parent: Who) {
        let (po, pa) = self.parent_pose(parent);
        let prot = entity_rotation(pa);
        let Some(e) = self.get(id) else { return };
        let follow = Follow {
            parent,
            offset: prot.inverse() * (e.origin - po),
            rotation: prot.inverse() * entity_rotation(e.angles),
        };
        self.follows.retain(|(c, _)| *c != id);
        self.follows.push((id, follow));
    }

    /// Unparent `id` (ClearParent): it stays where it is.
    pub fn detach(&mut self, id: EntId) {
        self.follows.retain(|(c, _)| *c != id);
    }

    /// The SetParent, SetParentAttachment, SetParentAttachmentMaintainOffset
    /// and ClearParent inputs (entity_io.md base inputs). Attachments are
    /// model points the logic doesn't know: the child keeps its offset
    /// (tech-debt). True when handled.
    pub(super) fn parent_input(
        &mut self,
        id: EntId,
        input: &str,
        value: &super::value::Value,
        activator: Option<Who>,
        caller: Option<Who>,
    ) -> bool {
        match input {
            "setparent" => {
                let name = value.to_str(|w| self.name_of(w)).unwrap_or_default();
                if name.trim().is_empty() {
                    self.detach(id);
                    return true;
                }
                let found = self
                    .resolve(name.trim(), activator, caller)
                    .into_iter()
                    .find(|w| *w != Who::Ent(id));
                match found {
                    Some(p) => self.attach(id, p),
                    None => self.log.push(format!("SetParent: no entity named '{}'", name.trim())),
                }
                true
            }
            "clearparent" => {
                self.detach(id);
                true
            }
            // Snaps to the attachment (the parent's origin when its model's
            // attachments aren't known).
            "setparentattachment" => {
                if let Some(i) = self.follows.iter().position(|(c, _)| *c == id) {
                    let name = value.to_str(|w| self.name_of(w)).unwrap_or_default();
                    self.follows[i].1.offset = self.attachment(self.follows[i].1.parent, &name);
                    self.follows[i].1.rotation = Quat::IDENTITY;
                }
                true
            }
            "setparentattachmentmaintainoffset" => true,
            _ => false,
        }
    }

    /// Where a parent is now: a mover's pusher pose, else its origin and
    /// angles; a player's origin and yaw.
    pub fn parent_pose(&self, parent: Who) -> (Vec3, Vec3) {
        match parent {
            Who::Ent(id) => match self.get(id) {
                Some(e) => match super::movers::pusher(&e.class) {
                    Some(p) => (p.origin, p.angles),
                    None => (e.origin, e.angles),
                },
                None => (Vec3::ZERO, Vec3::ZERO),
            },
            Who::Player(p) => match self.player(p) {
                Some(p) => (p.origin, Vec3::new(0.0, p.view.y, 0.0)),
                None => (Vec3::ZERO, Vec3::ZERO),
            },
        }
    }

    /// An anchor entity is here now (entity space; the host's `EntityAnchors`).
    pub fn set_anchor(&mut self, id: EntId, origin: Vec3, angles: Vec3) {
        if let Some(e) = self.get_mut(id) {
            e.origin = origin;
            e.angles = angles;
        }
    }

    /// Move every linked child to its parent's pose (parents before
    /// children: `link_anchored` keeps them in that order). A mover child
    /// (a door on a lift) is carried along by the parent's move, its own
    /// motion kept: translation only (tech-debt: a turning parent doesn't
    /// turn a mover child).
    pub fn follow_anchors(&mut self) {
        self.follow_players();
        self.move_followers(false);
    }

    /// `follow_anchors` for what rides physics bodies only (directly, or
    /// through other riders), after the physics step has moved the
    /// bodies (`set_anchor`): without it they would trail their body by
    /// a tick.
    pub fn follow_bodies(&mut self) {
        self.move_followers(true);
    }

    /// Whether anything rides a physics body (`follow_bodies` has work).
    pub fn has_body_riders(&self) -> bool {
        self.follows.iter().any(|(_, f)| match f.parent {
            Who::Ent(p) => self.get(p).is_some_and(|p| is_physics_body(&p.classname)),
            Who::Player(_) => false,
        })
    }

    /// Move linked children to their parents' poses; with `bodies_only`,
    /// only those whose parent is a physics body or was moved here.
    fn move_followers(&mut self, bodies_only: bool) {
        let follows = self.follows.clone();
        let mut moved: Vec<EntId> = Vec::new();
        for (id, f) in follows {
            if !self.exists(f.parent) {
                continue;
            }
            if bodies_only {
                let Who::Ent(p) = f.parent else { continue };
                if !moved.contains(&p) && !self.get(p).is_some_and(|p| is_physics_body(&p.classname)) {
                    continue;
                }
                moved.push(id);
            }
            let (po, pa) = self.parent_pose(f.parent);
            let parent_velocity = match f.parent {
                Who::Ent(p) => self
                    .get(p)
                    .and_then(|p| super::movers::pusher(&p.class))
                    .map_or(Vec3::ZERO, |p| p.velocity + p.carry),
                Who::Player(p) => self.player(p).map_or(Vec3::ZERO, |p| p.velocity),
            };
            let prot = entity_rotation(pa);
            let origin = po + prot * f.offset;
            let rotation = prot * f.rotation;
            let Some(e) = self.get(id) else { continue };
            if e.origin.distance_squared(origin) < 1e-6 && (entity_rotation(e.angles).dot(rotation)).abs() > 1.0 - 1e-7
            {
                // A still parent carries nothing.
                if let Some(e) = self.get_mut(id) {
                    super::movers::shift(&mut e.class, Vec3::ZERO, parent_velocity);
                }
                continue;
            }
            let angles = angles_of(rotation);
            let hulls = e.hulls.clone();
            let delta = origin - e.origin;
            let e = self.get_mut(id).unwrap();
            e.origin = origin;
            if super::movers::shift(&mut e.class, delta, parent_velocity) {
                self.refresh_solid(id);
                continue;
            }
            e.angles = angles;
            match &mut e.class {
                Class::Trigger(t) => {
                    t.brushes = hulls.iter().map(|h| place_hull(h, rotation, origin)).collect();
                }
                Class::Attached(a) => {
                    a.push.origin = origin;
                    a.push.angles = angles;
                }
                Class::Breakable(b) => {
                    b.attach.push.origin = origin;
                    b.attach.push.angles = angles;
                }
                _ => {}
            }
            self.refresh_solid(id);
        }
    }

    /// Players parented to an entity (SetParent on a player: karts,
    /// seats) stay at their place relative to it, carried with its
    /// velocity when it is a mover; dead players are let go.
    fn follow_players(&mut self) {
        let follows = self.player_follows.clone();
        for (p, f) in follows {
            let alive = self.player(p).is_some_and(|p| p.alive);
            if !alive || !self.exists(f.parent) {
                if !alive || self.player(p).is_some() {
                    self.player_follows.retain(|(q, _)| *q != p);
                }
                continue;
            }
            let (po, pa) = self.parent_pose(f.parent);
            let origin = po + entity_rotation(pa) * f.offset;
            let velocity = match f.parent {
                Who::Ent(e) => self
                    .get(e)
                    .and_then(|e| super::movers::pusher(&e.class))
                    .map_or(Vec3::ZERO, |m| m.velocity + m.carry),
                Who::Player(_) => Vec3::ZERO,
            };
            if let Some(pl) = self.player_mut(p) {
                pl.origin = origin;
                pl.velocity = velocity;
                pl.moved = true;
            }
        }
    }

    /// Player inputs that parent it (SetParent, SetParentAttachment,
    /// SetParentAttachmentMaintainOffset, ClearParent); true when handled.
    pub(super) fn player_parent_input(
        &mut self,
        p: bevy::ecs::entity::Entity,
        input: &str,
        value: &super::value::Value,
        activator: Option<Who>,
        caller: Option<Who>,
    ) -> bool {
        match input {
            "setparent" => {
                let name = value.to_str(|w| self.name_of(w)).unwrap_or_default();
                self.player_follows.retain(|(q, _)| *q != p);
                if name.trim().is_empty() {
                    return true;
                }
                let parent = self
                    .resolve(name.trim(), activator, caller)
                    .into_iter()
                    .find(|w| *w != Who::Player(p));
                let Some(parent) = parent else {
                    self.log.push(format!("SetParent: no entity named '{}'", name.trim()));
                    return true;
                };
                let (po, pa) = self.parent_pose(parent);
                let Some(origin) = self.player(p).map(|pl| pl.origin) else {
                    return true;
                };
                let rotation = entity_rotation(pa);
                self.player_follows.push((
                    p,
                    Follow {
                        parent,
                        offset: rotation.inverse() * (origin - po),
                        rotation: Quat::IDENTITY,
                    },
                ));
                true
            }
            "setparentattachment" => {
                if let Some(i) = self.player_follows.iter().position(|(q, _)| *q == p) {
                    let name = value.to_str(|w| self.name_of(w)).unwrap_or_default();
                    self.player_follows[i].1.offset = self.attachment(self.player_follows[i].1.parent, &name);
                }
                true
            }
            "setparentattachmentmaintainoffset" => true,
            "clearparent" => {
                self.player_follows.retain(|(q, _)| *q != p);
                true
            }
            _ => false,
        }
    }

    /// Where a parent's model attachment is in its frame (the loader's
    /// `$attachment <name>` keys; none known: its origin).
    pub fn attachment(&self, parent: Who, name: &str) -> Vec3 {
        let Who::Ent(id) = parent else { return Vec3::ZERO };
        self.get(id)
            .and_then(|e| e.kv(&format!("$attachment {}", name.trim().to_ascii_lowercase())))
            .map_or(Vec3::ZERO, crate::map::entities::parse_vector)
    }

    /// The entity a player is parented to.
    pub fn player_parent(&self, p: bevy::ecs::entity::Entity) -> Option<Who> {
        self.player_follows.iter().find(|(q, _)| *q == p).map(|(_, f)| f.parent)
    }

    /// Whether `id` follows an anchored parent.
    pub fn follows_anchor(&self, id: EntId) -> bool {
        self.follows.iter().any(|(c, _)| *c == id)
    }

    /// Whether `id` is carried by a physics brush or prop (through its parents):
    /// its own collider must not collide with that body.
    pub fn rides_body(&self, id: EntId) -> bool {
        self.rides_body_by(id, |at| self.follows.iter().find(|(c, _)| *c == at).map(|(_, f)| f))
    }

    /// `rides_body` with the search for an entity's link given.
    fn rides_body_by<'a>(&'a self, id: EntId, link: impl Fn(EntId) -> Option<&'a Follow>) -> bool {
        let mut at = id;
        for _ in 0..16 {
            let Some(f) = link(at) else {
                return false;
            };
            let Who::Ent(parent) = f.parent else { return false };
            if self.get(parent).is_some_and(|p| is_physics_body(&p.classname)) {
                return true;
            }
            at = parent;
        }
        false
    }

    /// `rides_body` for many entities (the bridge asks for every mover,
    /// every phase): the links looked up by slot, not by a search of the
    /// whole list for each (docs/performance.md, "Performance pass").
    pub fn body_riders(&self) -> BodyRiders<'_> {
        // Each slot's first link, as 1 + its place in `follows` (0: none).
        let mut first: Vec<u32> = Vec::new();
        for (i, (child, _)) in self.follows.iter().enumerate() {
            let slot = child.index as usize;
            if first.len() <= slot {
                first.resize(slot + 1, 0);
            }
            if first[slot] == 0 {
                first[slot] = i as u32 + 1;
            }
        }
        BodyRiders { world: self, first }
    }
}

/// `LogicWorld::rides_body` for many entities (`LogicWorld::body_riders`).
pub struct BodyRiders<'a> {
    world: &'a LogicWorld,
    first: Vec<u32>,
}

impl BodyRiders<'_> {
    /// `LogicWorld::rides_body(id)`.
    pub fn rides(&self, id: EntId) -> bool {
        let follows = &self.world.follows;
        self.world.rides_body_by(id, |at| {
            let place = *self.first.get(at.index as usize)?;
            let (child, f) = follows.get(place.checked_sub(1)? as usize)?;
            if *child == at {
                return Some(f);
            }
            // The slot's first link is an older entity's: the search.
            follows.iter().find(|(c, _)| *c == at).map(|(_, f)| f)
        })
    }
}
