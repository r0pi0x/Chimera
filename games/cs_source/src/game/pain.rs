//! What a player sounds like when hit and when killed: the install's
//! `game_sounds.txt` entries, played from the victim (its own sounds are
//! centred for the victim, `audio::sound::Ear`; positional for everyone
//! else). Measured on a CS:S server (specs/cs_source/sounds.md "Hurt and
//! death sounds"): each bullet hit on a player makes one hit sound, a
//! killing hit too, then the death sound.
//!
//! - A bullet in a bare head: `Player.DamageHeadShot` (CHAN_VOICE), the
//!   killing one included. In a helmet: `Player.DamageHelmet` (CHAN_BODY;
//!   unmeasured, below).
//! - A bullet or an HE blast where armour covers (the vest takes part:
//!   chest, stomach, arms, generic; a blast is generic):
//!   `Player.DamageKevlar` (CHAN_BODY) only.
//! - Any other bullet or blast hit (bare body, legs with or without
//!   armour): the `flesh` surface's bullet impact (`Flesh.BulletImpact`) at
//!   the hit (an HE blast on a bare player: measured 2026-10-17, as its
//!   kevlar sound on an armoured one). Knife hits are bullet damage
//!   and sound the same (measured 2026-10-17, weapons.md M11: kevlar on an
//!   armoured player, whatever was aimed at, as knife hits are generic;
//!   the flesh impact on a bare one; both besides the knife's own hit
//!   sound). It is the only impact sound
//!   a hit on a player makes: `impacts.rs` leaves players' hits to this
//!   module (hostages keep theirs there).
//! - A landing that takes health: `Player.FallDamage` (CHAN_BODY;
//!   sounds.md, "Landing").
//! - A death: `Player.DeathHeadShot` for a bullet in the head, else
//!   `Player.Death` (both CHAN_VOICE).
//!
//! Only damage that is taken sounds (`core::DamageRules::lands`): a hit
//! refused makes none of these, not even the flesh impact (measured
//! 2026-10-17, weapons.md "Refused hits": a teammate's bullet, knife or HE
//! with friendly fire off, a bullet on a noclipping player; the knife's
//! own hit sound still plays). Hit sounds are the server's
//! (`map::GameSound`: every client hears them once, the victim's and the
//! shooter's own included: a client knows no one else's armour, so it
//! can't predict which plays); death sounds play wherever the death is
//! seen (`core::Died`, which a network client gets from the server's
//! kill), so they are not sent again.
//!
//! Only CS:S characters sound so: one whose `slots::GameIdentity` is
//! another game's (the party apps, `chimera_sim::identity`) gets that
//! game's hurt and death sounds, none of these; and the flesh impact is
//! only a CS:S weapon's (or map damage's): another game's gun makes its
//! own impact on characters.
//!
//! Known shortcuts (docs/tech-debt.md): Fall damage feedback beyond the specs:
//! the `Player.FallDamage` sound (the server's, on any landing whose damage
//! is taken: not under god mode or a map's fall-damage filter,
//! `games/cs_source/pain.rs`; measured 2026-10-17, fall_damage.md "Landing
//! sounds") is as CS:S's; the landing roll (0.013° per unit/s, the shared
//! movement code's) is specced but not measured in CS:S; the HUD's part
//! is a guess: the health panel plays the install's `HealthTookDamage`
//! animation (red, 4 pulses over 1 s; VGUI's pulse curve assumed to be 0.5 −
//! 0.5·cos, blur left out) on any health drop, and a fall lights both damage
//! indicator bars (specs/cs_source/fall_damage.md open question 4). Why it's OK
//! for now: Reads right in play; the coordinator can measure it on the probe
//! server.
//!
//! Known shortcuts (docs/tech-debt.md): Hurt and death sounds
//! (`games/cs_source/pain.rs`): the per-hit choice for USP and AK-47 shots on
//! heads, chests, stomachs and legs is measured (sounds.md "Hurt and death
//! sounds"); the rest is ours from the install's game_sounds.txt entries: a
//! helmeted head plays `Player.DamageHelmet` (measured), arms and
//! generic hits count as covered like the chest (the damage rule's), and a
//! hit under god mode or refused by a map's filter is silent like the
//! measured refused hits (`god` couldn't be given to the probe's bots).
//! Fire and crushing play no hurt sound; `Player.PlasmaDamage`/`SonicDamage` (null waves) are unused;
//! drowning plays no hurt sound (measured: the server emits none; the client's
//! own `Player.DrownStart`/`DrownContinue`, if any, aren't played). Played
//! from the victim's origin (its body centre; the flesh impact at the hit),
//! the script's channel and level, to everyone, the shooter included (Source
//! may leave the shooter's own client out of effects made during its command;
//! the server trace doesn't show it), so the shooter hears its own hit one
//! round trip late; armour is what it was when the tick's shots began; no rate
//! limit but the channel (a new CHAN_BODY/CHAN_VOICE sound on the victim
//! replaces the last, so a killing headshot's `Player.DamageHeadShot` is cut
//! by `Player.DeathHeadShot`); shotgun pellets on one player are one hit, and
//! two players hit by one shotgun shot each make a flesh impact (no 300-unit
//! grouping). Deaths play wherever `core::Died` is seen (a client's from the
//! server's kill), not sent as a sound. Why it's OK for now: Matches every
//! measured case; the helmet and the rest are reference checks in
//! docs/plans/active/sound.md ("Damage and death").

use std::collections::HashMap;

use bevy::prelude::*;

use crate::{
    core::{Damage, DamageKind, DamageRules, Died, Health, Hitgroup, Intent, MapWater, SimSet, Team},
    map::{GameSound, PlaySound, sound::SoundBank},
    objectives::hostages::Hostage,
    slots::GameIdentity,
    weapon::{Armor, Inventory, Weapon, impact::weapon_game},
};

pub const DAMAGE_KEVLAR: &str = "Player.DamageKevlar";
pub const DAMAGE_HELMET: &str = "Player.DamageHelmet";
pub const DAMAGE_HEADSHOT: &str = "Player.DamageHeadShot";
pub const FALL_DAMAGE: &str = "Player.FallDamage";
pub const DEATH: &str = "Player.Death";
pub const DEATH_HEADSHOT: &str = "Player.DeathHeadShot";
/// The surface players' hitboxes are (impact_effects.md 3): its bullet
/// impact is a body hit's sound when no hurt sound plays.
pub const FLESH: &str = "flesh";

/// The entries this module plays (precached with every map).
pub const SOUNDS: &[&str] = &[
    DAMAGE_KEVLAR,
    DAMAGE_HELMET,
    DAMAGE_HEADSHOT,
    FALL_DAMAGE,
    DEATH,
    DEATH_HEADSHOT,
];

pub struct PainSoundsPlugin;

impl Plugin for PainSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArmorBeforeShots>()
            .add_message::<Damage>()
            .add_message::<crate::weapon::BulletHit>()
            .add_message::<Died>()
            .add_message::<GameSound>()
            .add_message::<PlaySound>()
            .add_systems(
                FixedUpdate,
                (
                    remember_armor.after(SimSet::Movement).before(SimSet::Weapons),
                    hurt_sounds.after(SimSet::Weapons).before(crate::core::apply_damage),
                )
                    .run_if(crate::core::authoritative),
            )
            .add_systems(Update, death_sounds);
    }
}

/// Every character's armour as the tick's shots start: whether armour
/// covered a hit is decided by what it had before the hit took its share.
#[derive(Resource, Default)]
struct ArmorBeforeShots(HashMap<Entity, Armor>);

fn remember_armor(armor: Query<(Entity, &Armor)>, mut before: ResMut<ArmorBeforeShots>) {
    before.0.clear();
    before.0.extend(armor.iter().map(|(e, a)| (e, *a)));
}

/// The hurt sound for one hit that is taken, a killing one included
/// (measured: `Player.DamageHeadShot`, then `Player.DeathHeadShot`). A
/// bullet that gets none makes the flesh impact instead (`hurt_sounds`).
pub fn hurt_sound(kind: DamageKind, group: Hitgroup, armor: Option<Armor>) -> Option<&'static str> {
    let covered = armor.is_some_and(|a| a.covers(group));
    match kind {
        DamageKind::Fall => Some(FALL_DAMAGE),
        DamageKind::Bullet if group == Hitgroup::Head => Some(if covered { DAMAGE_HELMET } else { DAMAGE_HEADSHOT }),
        // An HE blast too (measured 2026-10-17: kevlar on an armoured
        // player, the flesh impact on a bare one).
        DamageKind::Bullet | DamageKind::Blast | DamageKind::BlastSurface if covered => Some(DAMAGE_KEVLAR),
        _ => None,
    }
}

/// The death sound for the hit that killed.
pub fn death_sound(kind: DamageKind, group: Hitgroup) -> &'static str {
    if kind == DamageKind::Bullet && group == Hitgroup::Head {
        DEATH_HEADSHOT
    } else {
        DEATH
    }
}

#[allow(clippy::type_complexity)]
fn hurt_sounds(
    mut damage: MessageReader<Damage>,
    before: Res<ArmorBeforeShots>,
    victims: Query<(&Health, &Transform, Has<Team>, Option<&GameIdentity>), (With<Intent>, Without<Hostage>)>,
    holders: Query<&Inventory>,
    weapons: Query<&Weapon>,
    rules: DamageRules,
    bank: Option<Res<SoundBank>>,
    water: Option<Res<MapWater>>,
    mut out: MessageWriter<GameSound>,
) {
    let flesh = bank
        .as_ref()
        .and_then(|b| super::impacts::surface(&b.0, FLESH))
        .and_then(|s| s.bullet_impact.clone());
    // Health left as this tick's hits land (several can hit one victim).
    let mut left: HashMap<Entity, f32> = HashMap::new();
    for d in damage.read() {
        let Ok((health, at, team, identity)) = victims.get(d.target) else {
            continue;
        };
        let h = left.entry(d.target).or_insert(health.current);
        // A hit not taken makes no sound at all: a teammate's with
        // friendly fire off, a noclipping player's (measured 2026-10-17,
        // weapons.md "Refused hits"), and ours: god mode, a map's filter.
        if *h <= 0.0 || !rules.lands(d) {
            continue;
        }
        let hurt = if team {
            *h -= d.amount;
            // Only a CS:S character's voice and body: another game's
            // identity (the party apps' `identity = "weapon"`) sounds as
            // that game's.
            hurt_sound(d.kind, d.hitgroup, before.0.get(&d.target).copied())
                .filter(|_| GameIdentity::allows(identity, super::GAME))
        } else {
            None
        };
        if let Some(entry) = hurt {
            out.write(GameSound(PlaySound {
                source: Some(d.target),
                ..PlaySound::at(entry, at.translation)
            }));
        } else if matches!(d.kind, DamageKind::Bullet | DamageKind::Blast | DamageKind::BlastSurface)
            && dealt_by_ours(d, &holders, &weapons)
            && let Some(entry) = &flesh
            && !under_water(d.point, water.as_deref())
        {
            // A bullet or blast no hurt sound covers: the hit surface's
            // impact (players' hitboxes are `flesh`), where it hit. A
            // knife's hit too (bullet damage; measured 2026-10-17: an
            // unarmoured player knifed sounds `flesh_impact_bullet*`
            // besides the knife's own hit sound), and an HE blast's on a
            // bare player (measured 2026-10-17).
            out.write(GameSound(PlaySound::at(entry.clone(), d.point)));
        }
    }
}

/// Whether `d` was dealt by a CS:S weapon (or one of no game: map
/// damage): another game's gun makes its own impact on characters.
fn dealt_by_ours(d: &Damage, holders: &Query<&Inventory>, weapons: &Query<&Weapon>) -> bool {
    let id = d.weapon.or_else(|| {
        let held = holders.get(d.attacker?).ok()?.active?;
        Some(weapons.get(held).ok()?.id)
    });
    id.and_then(weapon_game).is_none_or(|g| g.eq_ignore_ascii_case(super::GAME))
}

/// A hit in water makes no impact sound (as `impacts::bullet_impacts`).
fn under_water(point: Vec3, water: Option<&MapWater>) -> bool {
    water
        .is_some_and(|w| super::impact_effects::water_shot(point, point, &w.0) != super::impact_effects::WaterShot::Dry)
}

fn death_sounds(
    mut died: MessageReader<Died>,
    victims: Query<(&GlobalTransform, Option<&GameIdentity>), (With<Team>, Without<Hostage>)>,
    mut play: MessageWriter<PlaySound>,
) {
    for d in died.read() {
        let Ok((at, identity)) = victims.get(d.entity) else { continue };
        if !GameIdentity::allows(identity, super::GAME) {
            continue;
        }
        play.write(PlaySound {
            source: Some(d.entity),
            ..PlaySound::at(death_sound(d.damage.kind, d.damage.hitgroup), at.translation())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEVLAR: Armor = Armor {
        amount: 1.0,
        helmet: false,
    };
    const HELMET: Armor = Armor {
        amount: 1.0,
        helmet: true,
    };

    #[test]
    fn bullets_by_armour_and_hitgroup() {
        let b = DamageKind::Bullet;
        assert_eq!(hurt_sound(b, Hitgroup::Chest, Some(KEVLAR)), Some(DAMAGE_KEVLAR));
        assert_eq!(hurt_sound(b, Hitgroup::LeftArm, Some(KEVLAR)), Some(DAMAGE_KEVLAR));
        assert_eq!(hurt_sound(b, Hitgroup::Chest, None), None);
        // Legs aren't covered.
        assert_eq!(hurt_sound(b, Hitgroup::RightLeg, Some(HELMET)), None);
        // Spent armour covers nothing.
        let spent = Armor {
            amount: 0.0,
            helmet: true,
        };
        assert_eq!(hurt_sound(b, Hitgroup::Stomach, Some(spent)), None);
        assert_eq!(hurt_sound(b, Hitgroup::Head, Some(HELMET)), Some(DAMAGE_HELMET));
        assert_eq!(hurt_sound(b, Hitgroup::Head, Some(KEVLAR)), Some(DAMAGE_HEADSHOT));
        // A bare head, the killing hit too (the death sound follows).
        assert_eq!(hurt_sound(b, Hitgroup::Head, None), Some(DAMAGE_HEADSHOT));
    }

    #[test]
    fn other_kinds() {
        assert_eq!(
            hurt_sound(DamageKind::Fall, Hitgroup::Generic, Some(HELMET)),
            Some(FALL_DAMAGE)
        );
        assert_eq!(hurt_sound(DamageKind::Melee, Hitgroup::Chest, Some(HELMET)), None);
        // An HE blast on armour sounds kevlar; on a bare body, the flesh
        // impact (`hurt_sounds`).
        assert_eq!(
            hurt_sound(DamageKind::Blast, Hitgroup::Generic, Some(HELMET)),
            Some(DAMAGE_KEVLAR)
        );
        assert_eq!(hurt_sound(DamageKind::Blast, Hitgroup::Generic, None), None);
        assert_eq!(hurt_sound(DamageKind::Burn, Hitgroup::Generic, None), None);
        assert_eq!(death_sound(DamageKind::Bullet, Hitgroup::Head), DEATH_HEADSHOT);
        assert_eq!(death_sound(DamageKind::Melee, Hitgroup::Head), DEATH);
        assert_eq!(death_sound(DamageKind::Fall, Hitgroup::Generic), DEATH);
    }
}
