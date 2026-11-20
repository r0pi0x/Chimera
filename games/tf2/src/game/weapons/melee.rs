//! TF2's nine stock melee weapons (specs/tf2/weapons_stock.md): the Bat,
//! Shovel, Fire Axe, Bottle, Fists, Wrench, Bonesaw, Kukri and Knife, as
//! data (data/weapons, base `tf2:melee`) on Chimera's melee swing with its
//! delayed hit (`Swing::delay`: the smack 0.2 s after the swing starts,
//! the Knife's 0.1 s, traced along the view then) and its rule for a hit
//! from behind (`Backstab::Behind`: the Knife's three angle conditions, 6
//! times the victim's current health, the kill credited to `BACKSTAB`, its
//! kill icon), its contact point (data's `box_centre`, `refine_box`:
//! a box hit on a player at the box's centre, one on the world refined
//! onto its surface; effects.md 10.1) and its impact ray
//! (`looks::STYLE`'s `melee_ray`). The hook `tf2:melee` (`MeleeParams`, the part `Tf2Melee`)
//! adds what data can't say:
//!
//! - **Crits** (3): one roll per swing, before it starts (`prepare`,
//!   before the weapon frame): a crit swing deals 3 times its damage
//!   (`CRIT_MULT`), starts with the crit swing sound (`crit_swing`, the
//!   script's `burst`) and its crit sequence (`SWING_CRIT`), and its hit
//!   plays `TFPlayer.CritHit` (`CRIT_HIT`, under TF2's name prefix: `maps::own_sound`). The chance is
//!   `crit_chance(D)` (15 % to 60 % with D the damage dealt in the last
//!   20 s); random crits stay off (`MeleeCrits`, the party's choice, as
//!   for the guns: weapons.md 2.6), so a melee swing crits only when a
//!   test forces it (`Tf2Melee::force_crit`). The Knife never rolls
//!   (`crits = false`); its backstab is always a crit (its sound).
//! - **The view-model actions** (`after`, after the frame): each swing's
//!   start as `SWING` (attack), `SWING_ALT` (attack2: the Fists' right
//!   fist) or `SWING_CRIT`, for `game::view` to play the class's sequence.
//! - **The raised knife** (4.4, `backstab_ready`): while the Knife is out
//!   and not swinging, whether the player in front within reach would be
//!   backstabbed now; on a change `BACKSTAB_UP` or `BACKSTAB_DOWN` and the
//!   weapon's `ViewPose` (1 raised), so it idles raised and a swing stabs
//!   with `knife_backstab`. The backstab itself is decided at the smack,
//!   from the positions then (open question 4).
//!
//! Known shortcuts:
//! - No damage history: with random crits on, the chance is the base 15 %
//!   (`crit_chance(0)`).
//! - The Bottle doesn't break on a crit hit (cosmetic, 5.5); the Wrench
//!   hits no buildings (there are none).
//! - The Fists with both buttons held punch right every time (Chimera's
//!   attack2 has priority), not alternately (open question 5).
//! - Other players see a swing's third-person gesture and hear its hit at
//!   the smack (the swing's start isn't sent to them); its swing sound
//!   they hear at the start.

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Backstab, Inventory, Melee, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState, random, view_anim::ViewPose,
};

use crate::{
    core::{FirstTimePredicted, Health, Intent, SimClock, Team},
    map::{GameSound, PlaySound},
};

/// The kill-icon id a backstab kill is credited with (data/weapons/
/// knife.toml's `credit`; `hud::kill_icon_pictures` cuts its cell).
pub const BACKSTAB: &str = "tf2:backstab";
/// Its kill icon's name in `mod_textures.txt` (8.4).
pub const BACKSTAB_ICON: &str = "backstab";
/// A crit's damage factor (3).
pub const CRIT_MULT: f32 = 3.0;
/// The crit hit's sound (8.3).
pub const CRIT_HIT: &str = "tf2:TFPlayer.CritHit";

/// The view-model actions (`WeaponEventKind::Action`).
pub const SWING: &str = "melee_swing";
pub const SWING_ALT: &str = "melee_swing_alt";
pub const SWING_CRIT: &str = "melee_swing_crit";
pub const BACKSTAB_UP: &str = "backstab_up";
pub const BACKSTAB_DOWN: &str = "backstab_down";

/// `tf2:melee`'s parameters.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MeleeParams {
    /// The class whose arms hold it (`c_<arms>_arms.mdl`).
    pub arms: String,
    /// Its script (`scripts/<script>.ctx`).
    pub script: String,
    /// Whether it rolls random crits (the Knife doesn't).
    #[serde(default = "yes")]
    pub crits: bool,
    /// The crit swing's sound (the script's `burst`).
    #[serde(default)]
    pub crit_swing: Option<String>,
}

fn yes() -> bool {
    true
}

/// Whether TF2's melee swings roll random crits (`tf_weapon_criticals`
/// and `tf_weapon_criticals_melee`): off, the party's choice, as the guns'
/// (module doc).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeleeCrits {
    pub random: bool,
}

/// The random melee crit chance with `recent` points of damage dealt in
/// the last 20 s (3): 0.15 x (1 + 3 min(D / 800, 1)).
pub fn crit_chance(recent: f32) -> f32 {
    0.15 * (1.0 + 3.0 * (recent / 800.0).clamp(0.0, 1.0))
}

/// A melee weapon's state (a predicted part).
#[derive(Component, Clone, Debug)]
pub struct Tf2Melee {
    pub params: MeleeParams,
    /// Its swings' damage and swing sounds as its data gives them
    /// (primary, secondary), taken from its `Melee` the first time.
    base: Option<[(f32, Option<String>); 2]>,
    /// The swing about to start (or under way) is a crit.
    pub crit: bool,
    /// A test's choice for every swing (None: the roll).
    pub force_crit: Option<bool>,
    /// The raised knife (4.4).
    pub ready: bool,
}

impl NetPart for Tf2Melee {
    type Net = (bool, bool);
    fn net(&self) -> Self::Net {
        (self.crit, self.ready)
    }
    fn set_net(&mut self, (crit, ready): Self::Net) {
        self.crit = crit;
        self.ready = ready;
    }
}

/// `tf2:melee` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &MeleeParams) {
    e.insert((
        Tf2Melee {
            params: p.clone(),
            base: None,
            crit: false,
            force_crit: None,
            ready: false,
        },
        ViewPose(0),
    ));
}

/// Each held melee weapon's next swing, before the frame: between swings
/// its crit rolled from the command's seed (the same on every machine)
/// and its swings' damage and sound set by it (module doc).
pub fn prepare(
    owners: Query<(&Intent, &Inventory)>,
    mut weapons: Query<(Entity, &Weapon, &mut Tf2Melee, &mut Melee, &WeaponState)>,
    crits: Option<Res<MeleeCrits>>,
) {
    let random_on = crits.is_some_and(|c| c.random);
    // By the melee weapons, not the characters (a query over every
    // character's archetypes costs each tick: party_kit's archetypes test).
    for (we, weapon, mut m, mut melee, state) in &mut weapons {
        let Some(Ok((intent, inv))) = weapon.owner.map(|o| owners.get(o)) else { continue };
        if inv.active != Some(we) || state.pending_swing.is_some() {
            continue;
        }
        let base = m.base.get_or_insert_with(|| {
            let of = |s: &chimera_sim::weapon::Swing| (s.damage, s.sound_swing.clone());
            [of(&melee.primary), melee.secondary.as_ref().map_or((0.0, None), of)]
        });
        let base = base.clone();
        let roll = || {
            let mut rng = random::Rng::new(random::shot_seed(intent.command) ^ 0x6d65_6c65);
            rng.next() < crit_chance(0.0)
        };
        let crit = m.force_crit.unwrap_or_else(|| random_on && m.params.crits && roll());
        if m.crit != crit {
            m.crit = crit;
        }
        let mult = if crit { CRIT_MULT } else { 1.0 };
        let sound = |normal: &Option<String>| if crit { m.params.crit_swing.clone().or(normal.clone()) } else { normal.clone() };
        let (primary, secondary) = ((base[0].0 * mult, sound(&base[0].1)), (base[1].0 * mult, sound(&base[1].1)));
        if melee.primary.damage != primary.0 || melee.primary.sound_swing != primary.1 {
            melee.primary.damage = primary.0;
            melee.primary.sound_swing = primary.1;
        }
        if let Some(s) = melee.secondary.as_mut()
            && (s.damage != secondary.0 || s.sound_swing != secondary.1)
        {
            s.damage = secondary.0;
            s.sound_swing = secondary.1;
        }
    }
}

/// After the frame: each swing's start as its view-model action, and a
/// crit swing's or a backstab's hit with the crit hit sound (module doc).
pub fn after(
    mut events: ParamSet<(MessageReader<WeaponEvent>, MessageWriter<WeaponEvent>)>,
    mut damage: MessageReader<crate::core::Damage>,
    weapons: Query<&Tf2Melee>,
    places: Query<&Transform>,
    mut sounds: MessageWriter<GameSound>,
) {
    let mut out = Vec::new();
    for e in events.p0().read() {
        let Ok(m) = weapons.get(e.weapon) else { continue };
        match e.kind {
            WeaponEventKind::SwingStarted { secondary } => {
                let action = if m.crit {
                    SWING_CRIT
                } else if secondary {
                    SWING_ALT
                } else {
                    SWING
                };
                out.push(WeaponEvent { owner: e.owner, weapon: e.weapon, kind: WeaponEventKind::Action(action), replay: e.replay });
            }
            // The hit's own (the server's: `Hit` is told where damage is
            // dealt).
            WeaponEventKind::Hit { target, .. } if m.crit && !e.replay => {
                let at = places.get(target).map_or(Vec3::ZERO, |t| t.translation);
                sounds.write(GameSound(PlaySound { source: Some(target), ..PlaySound::at(CRIT_HIT, at) }));
            }
            _ => {}
        }
    }
    let mut writer = events.p1();
    for e in out {
        writer.write(e);
    }
    for d in damage.read().filter(|d| d.weapon == Some(BACKSTAB)) {
        sounds.write(GameSound(PlaySound::at(CRIT_HIT, d.point)));
    }
}

/// The raised knife (4.4): for each player holding a weapon whose swing
/// backstabs (`Backstab::Behind`) and not swinging, whether another
/// living character (not a teammate) within its reach would be backstabbed
/// now; on a change its `ViewPose` and `BACKSTAB_UP` / `BACKSTAB_DOWN`.
#[allow(clippy::type_complexity)]
pub fn backstab_ready(
    owners: Query<(&Intent, &Inventory, &Transform, Option<&Health>, Option<&Team>)>,
    others: Query<(Entity, &Intent, &Transform, Option<&Health>, Option<&Team>)>,
    mut weapons: Query<(Entity, &mut Tf2Melee, &Melee, &WeaponState, &mut ViewPose, &Weapon)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut events: MessageWriter<WeaponEvent>,
) {
    // By the melee weapons (others only for a knife held between swings).
    for (w, mut m, melee, state, mut pose, weapon) in &mut weapons {
        let Some(owner) = weapon.owner else { continue };
        let Ok((intent, inv, at, health, team)) = owners.get(owner) else { continue };
        if inv.active != Some(w) {
            continue;
        }
        let Some(rule) = melee.primary.backstab.filter(|b| matches!(b, Backstab::Behind { .. })) else { continue };
        let alive = health.is_none_or(|h| h.current > 0.0);
        let swinging = state.pending_swing.is_some() || state.next_primary > clock.now;
        let ready = if !alive {
            false
        } else if swinging {
            m.ready
        } else {
            // In reach: the swing's line and box (centre to centre, the
            // box's half width beyond the range).
            let reach = melee.primary.range + melee.primary.hull.unwrap_or(0.0) + 24.0 * super::damage::U;
            let forward = intent.yaw_rotation() * Vec3::NEG_Z;
            others.iter().any(|(e, theirs, t, h, their_team)| {
                let friend = matches!((team, their_team), (Some(a), Some(b)) if a.0 != 0 && a.0 == b.0);
                let to = t.translation - at.translation;
                e != owner
                    && !friend
                    && h.is_some_and(|h| h.current > 0.0)
                    && to.with_y(0.0).length() <= reach
                    && rule.behind(forward, theirs.yaw_rotation() * Vec3::NEG_Z, to)
            })
        };
        if ready != m.ready {
            m.ready = ready;
            pose.0 = u8::from(ready);
            let kind = WeaponEventKind::Action(if ready { BACKSTAB_UP } else { BACKSTAB_DOWN });
            events.write(WeaponEvent { owner, weapon: w, kind, replay: !first.0 });
        }
    }
}
