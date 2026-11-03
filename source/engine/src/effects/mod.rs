//! What Source's impacts look and sound like, shared by the Source games
//! (lift S10 and the rest of S5 of docs/plans/active/hl2dm.md, moved from
//! `games/cs_source`): `impact_effects` (the surface's debris, dust and
//! sparks, blood, water splashes, glass; on `map::particles`), `impacts`
//! (bullet impact sounds and decals, physics impact sounds), `scrapes`
//! (looping friction sounds), `explosions` (the explosion temp entity's
//! smoke, fire, sparks and flecks, lifted from CS:S's HE grenade for
//! HL2:DM's). Each game adds the plugins with its
//! `ImpactStyle` (its id and the few numbers its engine differs in: the
//! ricochet, the splash's scale, the grouping of pellets' sounds); its
//! tracers, flashes and hurt sounds stay its own.

use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponEvent,
    impact::{ImpactBy, ImpactRoute},
};

pub mod explosions;
pub mod impact_effects;
pub mod impacts;
pub mod scrapes;

/// How one Source game's impacts differ (specs/cs_source/impact_effects.md,
/// specs/hl2dm/effects_sounds.md 6, 7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpactStyle {
    /// The game's id: whose guns' impacts (and whose maps' when no gun's
    /// game draws them) these are (`weapon::impact::ImpactRoute`).
    pub game: &'static str,
    /// The ricochet sound's entry and the share of bullet impacts that
    /// play it (CS:S `Bounce.Shrapnel` 0.3; HL2:DM none).
    pub ricochet: Option<(&'static str, f32)>,
    /// The scale of a bullet's water splash, drawn uniformly (CS:S
    /// 8 to 12, HL2:DM 4 to 8).
    pub splash: (f32, f32),
    /// Pellets of one shot skip an identical impact sound within this
    /// many units (CS:S 300; HL2:DM plays each pellet's).
    pub group_radius: Option<f32>,
    /// A damage amount's unit for the wall blood's count (CS:S's weapon
    /// data's 0.01 hit points; HL2:DM's 1).
    pub damage_unit: f32,
    /// The surface a character's hitbox is: its bullet impact is a body
    /// hit's sound where the game's hurt sounds don't play one.
    pub flesh: &'static str,
}

/// The styles of the games in this app (`impact_effects::ImpactEffectsPlugin`,
/// `impacts::BulletImpactsPlugin` add theirs).
#[derive(Resource, Default, Clone, Debug)]
pub struct ImpactStyles(pub Vec<ImpactStyle>);

impl ImpactStyles {
    /// The style of the game that draws `e`'s impacts, if one does.
    pub fn drawing(&self, route: &ImpactRoute, e: &WeaponEvent) -> Option<&ImpactStyle> {
        let ImpactBy::Game(g) = route.by(e) else { return None };
        self.of(g)
    }

    /// `game`'s style.
    pub fn of(&self, game: &str) -> Option<&ImpactStyle> {
        self.0.iter().find(|s| s.game.eq_ignore_ascii_case(game))
    }

    /// Add `style` (once per game).
    pub fn add(&mut self, style: ImpactStyle) {
        if self.of(style.game).is_none() {
            self.0.push(style);
        }
    }
}
