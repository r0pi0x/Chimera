//! What a bullet does that every game shares, whoever's gun fired it on
//! whoever's map (docs/DESIGN.md, "Impacts across games"):
//!
//! - **Who draws an impact** (`ImpactRoute`): each game whose effects draw
//!   impacts registers as a provider (`ImpactAppExt::register_impact_provider`);
//!   an impact is drawn by the gun's game when it is a provider and its
//!   looks are on the map (`map::impact::ActiveImpacts`), else by the map's
//!   game on the same terms, else by any provider whose looks the map
//!   carries (an app's gameplay on another game's map), else by Chimera's
//!   generic impact (`ImpactBy::
//!   Generic`, the client's `effects::GenericImpactsPlugin`). Every game's
//!   impact systems ask the route and draw only their own, so one impact
//!   is drawn once. The hit surface goes between the games by class
//!   (`map::impact::SurfaceClasses`), and the drawing game takes its own
//!   looks on any map (`map::impact::own_name`). A hit on a character
//!   sounds as the gun's game's too (`dealt_by`: another game's effects
//!   play no flesh impact for it).
//! - **The push on physics objects** (`BulletImpulse`): every game's
//!   bullets tell the impulse they give a body at the hit point, in N·s
//!   (each game converts its own numbers: CS:S's damage force by ammo
//!   type from its weapon data's `impulse`, Combat Arms' from its damage,
//!   `combat_arms::game::weapons::IMPULSE_PER_HIT_POINT`), and
//!   `apply_bullet_impulses` gives it to dynamic bodies, times
//!   `phys_pushscale` (`PushScale`), after the weapon frame in the
//!   predicted weapon tick (`ApplyImpulses`: a game telling pushes after
//!   the frame orders itself before it). The server's
//!   alone: a client's own shots push nothing.
//! - **Spawn protection hides hits** (`core::SpawnProtected`): a hit on
//!   a protected character shows nothing on any machine, whichever game's
//!   gun or effects: Chimera's deliveries tell its trace with no `hit`
//!   (`deliver`), and a game with its own (Combat Arms') passes its
//!   events through `hide_protected`; so no game draws blood, a puff, a
//!   decal or an impact sound there, nor a hit indicator or name-tag
//!   flash (those follow the `Hit`s and traces), and the damage the rules
//!   block (`core::God`) never shows as landed.
//! - **Whose recoil the view punch is** (`RecoilGame`): each game kicks
//!   and returns the shared `ViewPunch` by its own rules; a character
//!   that switches to another game's weapon (the gun game's swap on a
//!   kill) ends the old game's punch: the punch is zeroed and the
//!   component names the new game, whose rules alone touch it from then
//!   (each game's recoil code checks `RecoilGame::is`). Set in the
//!   predicted selection (`core::Predict::Select`), from the predicted
//!   inventory, so the server and the client predicting the player agree.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Inventory, PushScale, ViewPunch, Weapon, WeaponEvent, WeaponEventKind};
use crate::{
    core::{Predict, PredictedAppExt},
    map::{interp::NetDrawn, impact::ActiveImpacts},
};

/// The games whose effects draw impacts in this app.
#[derive(Resource, Clone, Debug, Default)]
pub struct ImpactProviders(pub Vec<&'static str>);

/// Register a game as an impact provider.
pub trait ImpactAppExt {
    fn register_impact_provider(&mut self, game: &'static str) -> &mut Self;
}

impl ImpactAppExt for App {
    fn register_impact_provider(&mut self, game: &'static str) -> &mut Self {
        let mut p = self.world_mut().get_resource_or_init::<ImpactProviders>();
        if !p.0.contains(&game) {
            p.0.push(game);
        }
        self
    }
}

/// Who draws an impact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactBy {
    /// That game's effects.
    Game(&'static str),
    /// Chimera's generic impact (no game can).
    Generic,
}

/// The game of a weapon id (`cs_source` of `cs_source:weapon_ak47`).
pub fn weapon_game(id: &'static str) -> Option<&'static str> {
    id.split_once(':').map(|(g, _)| g)
}

/// The game of the weapon that dealt `d`: its own (`Damage::weapon`),
/// else the one the attacker holds; None: no game's (map damage).
pub fn damage_game(
    d: &crate::core::Damage,
    holders: &Query<&Inventory>,
    weapons: &Query<&Weapon>,
) -> Option<&'static str> {
    let id = d.weapon.or_else(|| {
        let held = holders.get(d.attacker?).ok()?.active?;
        Some(weapons.get(held).ok()?.id)
    });
    id.and_then(weapon_game)
}

/// Whether `d` was dealt by a weapon of `game` (or of no game: map
/// damage): a hit's impact on a character (its flesh sound) is the gun's
/// game's, so another game's effects leave it alone.
pub fn dealt_by(
    game: &str,
    d: &crate::core::Damage,
    holders: &Query<&Inventory>,
    weapons: &Query<&Weapon>,
) -> bool {
    damage_game(d, holders, weapons).is_none_or(|g| g.eq_ignore_ascii_case(game))
}

/// Who draws an impact of a gun of `gun` (its game) on a map that carries
/// `impacts`: the gun's game if it provides impacts and its looks are on
/// the map, else the map's game on the same terms, else the first
/// provider whose looks are there, else the generic one.
pub fn impact_by(gun: Option<&str>, impacts: Option<&ActiveImpacts>, providers: &[&'static str]) -> ImpactBy {
    let ok = |g: &str| {
        providers
            .iter()
            .find(|p| p.eq_ignore_ascii_case(g))
            .filter(|_| impacts.is_none_or(|i| i.0.has(g)))
            .copied()
    };
    gun.and_then(ok)
        .or_else(|| impacts.and_then(|i| i.0.map_game()).and_then(ok))
        // Then any game whose looks the map carries (an app's gameplay
        // dressing another game's map).
        .or_else(|| providers.iter().find(|p| ok(p).is_some()).copied())
        .map_or(ImpactBy::Generic, ImpactBy::Game)
}

/// The route of each weapon event's impact (`impact_by`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct ImpactRoute<'w, 's> {
    providers: Option<Res<'w, ImpactProviders>>,
    impacts: Option<Res<'w, ActiveImpacts>>,
    weapons: Query<'w, 's, &'static Weapon>,
}

impl ImpactRoute<'_, '_> {
    /// Who draws `e`'s impacts.
    pub fn by(&self, e: &WeaponEvent) -> ImpactBy {
        self.by_weapon(e.weapon)
    }

    /// Who draws the impacts of `weapon`'s bullets.
    pub fn by_weapon(&self, weapon: Entity) -> ImpactBy {
        let providers = self.providers.as_deref().map_or(&[][..], |p| &p.0[..]);
        impact_by(self.gun_game(weapon), self.impacts.as_deref(), providers)
    }

    /// The game of `weapon`.
    pub fn gun_game(&self, weapon: Entity) -> Option<&'static str> {
        self.weapons.get(weapon).ok().and_then(|w| weapon_game(w.id))
    }

    /// Whether `game` draws `e`'s impacts.
    pub fn draws(&self, game: &str, e: &WeaponEvent) -> bool {
        matches!(self.by(e), ImpactBy::Game(g) if g.eq_ignore_ascii_case(game))
    }

    /// The map's own game, when known.
    pub fn map_game(&self) -> Option<&str> {
        self.impacts.as_deref().and_then(|i| i.0.map_game())
    }
}

/// A game's own delivery's event with spawn protection applied (module
/// doc): a trace whose `hit` `hides` loses its hit and normal, a swing
/// at one is a miss, a `Hit` on one is gone (None).
pub fn hide_protected(kind: WeaponEventKind, hides: impl Fn(Entity) -> bool) -> Option<WeaponEventKind> {
    use WeaponEventKind as K;
    Some(match kind {
        K::Shot { from, to, hit: Some(h), .. } if hides(h) => K::Shot {
            from,
            to,
            hit: None,
            normal: None,
        },
        K::ShotContinued { from, to, hit: Some(h), .. } if hides(h) => K::ShotContinued {
            from,
            to,
            hit: None,
            normal: None,
        },
        K::Swing { secondary, at: Some((_, _, h)), line, .. } if hides(h) => K::Swing {
            hit: false,
            secondary,
            at: None,
            line,
        },
        K::Hit { target, .. } if hides(target) => return None,
        k => k,
    })
}

/// A bullet's push on a body: `impulse` (N·s, world space) at `point`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct BulletImpulse {
    pub body: Entity,
    pub point: Vec3,
    pub impulse: Vec3,
}

/// Give each `BulletImpulse` to its body when it is dynamic, times
/// `phys_pushscale`.
pub fn apply_bullet_impulses(
    mut impulses: MessageReader<BulletImpulse>,
    mut bodies: Query<(&RigidBody, Forces)>,
    scale: Option<Res<PushScale>>,
) {
    let scale = scale.map_or(1.0, |s| s.0);
    for i in impulses.read() {
        if i.impulse == Vec3::ZERO {
            continue;
        }
        if let Ok((rb, mut forces)) = bodies.get_mut(i.body)
            && rb.is_dynamic()
        {
            forces.apply_linear_impulse_at_point(i.impulse * scale, i.point);
        }
    }
}

/// Where the pushes are given, in the predicted weapon tick
/// (`core::Predict::Weapons`) after the frame: games whose systems after the
/// frame tell `BulletImpulse`s run before it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApplyImpulses;

/// The game whose recoil rules own a character's `ViewPunch` (the game of
/// the weapon it holds); predicted with the player.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoilGame(pub String);

impl RecoilGame {
    /// Whether `game`'s recoil may touch the punch of a character with
    /// `this` (none yet: anyone's).
    pub fn is(this: Option<&RecoilGame>, game: &str) -> bool {
        this.is_none_or(|r| r.0.eq_ignore_ascii_case(game))
    }
}

/// Each character's `RecoilGame` from what it holds (the weapon it
/// switches to this tick, else the one in hand); a change of game zeroes
/// the punch (the old game's recoil ends there).
#[allow(clippy::type_complexity)]
pub fn recoil_follows_weapon(
    mut characters: Query<(Entity, &Inventory, Option<&mut RecoilGame>, Option<&mut ViewPunch>), Without<NetDrawn>>,
    weapons: Query<&Weapon>,
    mut commands: Commands,
) {
    for (e, inv, game, punch) in &mut characters {
        // What it switches to this tick, else what's in hand (as
        // `identity::held_game`).
        let Some(held) = inv
            .wanted
            .or(inv.active)
            .and_then(|w| weapons.get(w).ok())
            .and_then(|w| weapon_game(w.id))
        else {
            continue;
        };
        match game {
            None => {
                commands.entity(e).insert(RecoilGame(held.to_string()));
            }
            Some(mut g) if !g.0.eq_ignore_ascii_case(held) => {
                g.0 = held.to_string();
                if let Some(mut p) = punch
                    && p.0 != Vec3::ZERO
                {
                    p.0 = Vec3::ZERO;
                }
            }
            Some(_) => {}
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ImpactProviders>()
        .add_message::<BulletImpulse>()
        .predicted_net::<RecoilGame>()
        .add_systems(
            Predict::Select,
            recoil_follows_weapon.in_set(super::SelectWeapons).after(super::select_weapons),
        )
        .add_systems(
            Predict::Weapons,
            apply_bullet_impulses.in_set(ApplyImpulses).after(super::WeaponFrame),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::impact::MapImpacts;

    fn on(game: &str, looks: &[&str]) -> ActiveImpacts {
        ActiveImpacts(MapImpacts {
            game: Some(game.into()),
            looks: looks.iter().map(|s| s.to_string()).collect(),
        })
    }

    #[test]
    fn the_guns_game_then_the_maps_then_the_generic_impact() {
        let both = ["a", "b"];
        // The gun's game's looks on the map: its own.
        assert_eq!(impact_by(Some("a"), Some(&on("b", &["b", "a"])), &both), ImpactBy::Game("a"));
        // Not there: the map's game's.
        assert_eq!(impact_by(Some("a"), Some(&on("b", &["b"])), &both), ImpactBy::Game("b"));
        // A gun of a game that draws no impacts: the map's.
        assert_eq!(impact_by(Some("c"), Some(&on("b", &["b"])), &both), ImpactBy::Game("b"));
        // Neither, but another's looks are on the map: that game's.
        assert_eq!(impact_by(Some("c"), Some(&on("d", &["d", "b"])), &both), ImpactBy::Game("b"));
        // None: Chimera's.
        assert_eq!(impact_by(Some("c"), Some(&on("d", &["d"])), &both), ImpactBy::Generic);
        assert_eq!(impact_by(Some("a"), Some(&on("b", &["b"])), &["a"]), ImpactBy::Generic);
        // A map that tells nothing (hand-made, tests): the gun's game's.
        assert_eq!(impact_by(Some("a"), None, &both), ImpactBy::Game("a"));
        assert_eq!(impact_by(Some("b"), Some(&ActiveImpacts::default()), &both), ImpactBy::Game("b"));
    }

    #[test]
    fn recoil_belongs_to_the_held_weapons_game() {
        assert!(RecoilGame::is(None, "a"));
        let g = RecoilGame("a".into());
        assert!(RecoilGame::is(Some(&g), "A") && !RecoilGame::is(Some(&g), "b"));
    }
}
