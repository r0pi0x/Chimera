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
pub mod spray_pictures;

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
    /// Its bullets' hits on players sound here (`impacts::bullet_impacts`:
    /// `flesh`'s impact at the hit), the game having no hurt sounds that
    /// play one (TF2 so far); false: its hurt sounds do (CS:S's `pain`,
    /// HL2:DM's `sounds`). Another game's hurt sounds never play for its
    /// gun (`weapon::impact::dealt_by`).
    pub player_hits: bool,
    /// The decal group a melee hit places (CS:S's knife slash); None: the
    /// surface's, as a bullet's (HL2:DM's crowbar and stunstick,
    /// specs/hl2dm/effects_sounds.md 6).
    pub melee_decal: Option<&'static str>,
    /// Who sees a hit player's blood, and where.
    pub blood: BloodView,
    /// A melee hit's impact (decal, effect, sound) shows on the
    /// attacker's own machine only (TF2: made in the predicted swing,
    /// specs/tf2/effects.md 10); false: wherever the swing's hit is known.
    pub melee_own_only: bool,
    /// A projectile's mark (`WeaponEventKind::Marked`) plays the ricochet
    /// when its shooter is on an odd team (TF2's needle: RED's only, ours'
    /// odd teams, effects.md 12); false: never.
    pub mark_ricochet: bool,
    /// Impact sounds farther than this from this machine's player (units)
    /// are skipped (TF2's 1024, effects.md 7); None: none skipped.
    pub sound_reach: Option<f32>,
    /// A melee hit's impact follows its impact ray
    /// (`WeaponEventKind::Swing::ray`): the
    /// decal at the contact point and the debris there with the ray's
    /// normal only when the ray reaches the world, the sound at the ray's
    /// hit, else at the contact (TF2's, specs/tf2/effects.md 10.1). False:
    /// all at the swing's hit.
    pub melee_ray: bool,
    /// Whether its bullets and swings put their decal on a hit character's
    /// body (`map::PlaceBodyDecal`).
    pub body_decals: BodyDecals,
}

/// Whether a game's hits decal the bodies they hit (a model decal that
/// follows the skin, `render::body_decals`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyDecals {
    /// None (Combat Arms: characters take no decal).
    Never,
    /// Every hit (CS:S's and HL2:DM's impact on a player: the flesh
    /// surface's decal, or a melee style's own).
    Always,
    /// Every hit but in low violence (`violence_hblood 0`; TF2,
    /// specs/tf2/effects.md 8b).
    NotLowViolence,
}

/// Where this machine's player is (its origin, m; none: no player here),
/// noted each frame for the impact sounds' reach (`ImpactStyle::heard`)
/// so the tick's impact systems read no `Transform`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ImpactListener(pub Option<Vec3>);

/// `ImpactListener` and its system, once whichever impact plugin comes
/// first.
pub fn add_listener(app: &mut App) {
    if !app.world().contains_resource::<ImpactListener>() {
        app.init_resource::<ImpactListener>().add_systems(Update, note_listener);
    }
}

/// Notes `ImpactListener`.
fn note_listener(local: Query<&Transform, With<crate::core::LocalPlayer>>, mut listener: ResMut<ImpactListener>) {
    let at = local.iter().next().map(|t| t.translation);
    if listener.0 != at {
        listener.0 = at;
    }
}

impl ImpactStyle {
    /// Whether an impact sound at `at` (m) is heard by a player at
    /// `listener` (`sound_reach`; no player here: always).
    pub fn heard(&self, listener: Option<Vec3>, at: Vec3) -> bool {
        match (self.sound_reach, listener) {
            (Some(r), Some(l)) => l.distance(at) <= r * 0.0254,
            _ => true,
        }
    }

    /// Whether a projectile's mark by a shooter of `team` plays the
    /// ricochet (`mark_ricochet`).
    pub fn mark_ricochets(&self, team: u8) -> bool {
        self.mark_ricochet && team % 2 == 1
    }

    /// Whether this machine skips `e`'s impact: another player's swing
    /// with `melee_own_only` (`local`: the event's owner plays here).
    pub fn skips(&self, e: &WeaponEvent, local: bool) -> bool {
        self.melee_own_only && !local && matches!(e.kind, chimera_sim::weapon::WeaponEventKind::Swing { .. })
    }
}

/// Who sees a hit player's blood (`impact_effects::blood_effects`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BloodView {
    /// Every machine that sees the damage draws it at the hit point
    /// (CS:S: specs/cs_source/impact_effects.md 10).
    Damage,
    /// Per viewer (specs/hl2dm/effects_sounds.md 7): the server's copy at
    /// the hit point for everyone but the attacker (`ServerBlood`, sent to
    /// network clients by the game), the shooter's own predicted copy at
    /// the hit 4 in back along the shot from his own bullets' traces, a
    /// melee attacker none.
    PerViewer,
    /// The game draws its own (TF2's blood temp entity,
    /// specs/tf2/effects.md 8: `tf2::game::weapons::blood`), with
    /// `impact_effects::BloodPainter::stains` for the walls.
    Game,
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
