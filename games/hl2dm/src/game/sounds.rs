//! HL2:DM's sounds (specs/hl2dm/effects_sounds.md 15; S5's rest): the
//! sound-script entries a map loads for it (`entries`: the weapons' own
//! from their data, their hooks' parameters too, the suit's and pickups'
//! in `maps::SOUNDS`, these; an entry not loaded plays nothing, without a
//! word: `SoundBank` only has what the map asked for),
//! what a hit player sounds like (`hurt_sounds`: there is no pain voice;
//! a bullet makes `Flesh.BulletImpact` from the victim, fire `HL2Player.
//! BurnPain`), a death (`death_sounds`: `<type>.Die` by the model's kind,
//! the female citizen's variant, none for a body being dissolved) and the
//! footsteps of the model's kind (`model_steps`: citizen, Combine, metro
//! police entries, 15.6).
//!
//! Known shortcuts: a death plays on every machine that sees the kill (as
//! CS:S's), not sent as a sound; the server's flesh impact goes to all
//! with the entry's level, without the PAS filter; `Dissolving` (the
//! energy ball's kill, `dissolve`) is only a marker here. Why it's OK for now: the sounds
//! and their choice are the spec's; the delivery rules need the network
//! milestone's probe.

use std::collections::BTreeMap;

use bevy::prelude::*;
use source_engine::source_movement::StepSounds;

use super::{
    GAME,
    suit::Suit,
    weapons::inventory::{DEFAULT_MODEL, Hl2Model, ModelKind},
};
use crate::{
    core::{Damage, DamageKind, DamageRules, Died, Health, SimSet},
    map::{GameSound, PlaySound},
    slots::GameIdentity,
};

pub const FLESH_IMPACT: &str = "Flesh.BulletImpact";
pub const BURN_PAIN: &str = "HL2Player.BurnPain";

/// The entries HL2:DM's sounds play besides the weapons' (precached with
/// every map): the damage and death sounds, near-misses, splashes, shell
/// bounces.
pub const SOUNDS: &[&str] = &[
    FLESH_IMPACT,
    BURN_PAIN,
    "NPC_Citizen.die",
    "NPC_CombineS.Die",
    "NPC_MetroPolice.Die",
    "Bullets.DefaultNearmiss",
    "Bullets.GunshipNearmiss",
    "Physics.WaterSplash",
    "Bounce.Shell",
    "Bounce.ShotgunShell",
    // Named by the effects' code, not the weapons' data.
    super::projectile_fx::EXPLOSION_SOUND,
    "BaseGrenade.Explode",
    super::physcannon_fx::HOLD_SOUND,
    super::physcannon_fx::BALL_HOLD_SOUND,
    super::rpg_fx::LASER_ON,
    super::rpg_fx::LASER_OFF,
    super::rpg_fx::IGNITE_SOUND,
    "Weapon_Crossbow.BoltSkewer",
];

/// The strings of a hook's parameters whose key says they name a sound
/// (`launch_sound`, the launcher's `sound`), at any depth.
fn hook_sounds<'a>(table: &'a toml::Table, out: &mut Vec<&'a str>) {
    for (key, value) in table {
        match value {
            toml::Value::Table(t) => hook_sounds(t, out),
            toml::Value::String(s) if !s.is_empty() && (key.contains("sound") || matches!(key.as_str(), "empty" | "explode" | "blip")) => {
                out.push(s)
            }
            _ => {}
        }
    }
}

/// Every entry a map loads for HL2:DM's sounds: `SOUNDS` and the
/// weapons' (their data's sounds, melee hits and swings, what their hooks
/// play).
pub fn entries() -> Vec<String> {
    let mut out: Vec<String> = SOUNDS.iter().map(|s| s.to_string()).collect();
    for w in &super::weapons::hl2_weapons().weapons {
        let swings = w.melee.iter().flat_map(|m| [Some(&m.primary), m.secondary.as_ref()]).flatten();
        let swung = swings.flat_map(|s| [&s.sound_hit, &s.sound_hit_world, &s.sound_miss]).flatten();
        let mut hooked = Vec::new();
        hook_sounds(&w.hooks, &mut hooked);
        for s in w.sounds.iter().flat_map(|s| s.entries()).chain(swung.map(String::as_str)).chain(hooked) {
            if !out.iter().any(|o| o == s) {
                out.push(s.to_string());
            }
        }
    }
    out
}

/// The death entry of a model kind (15.8).
pub fn death_entry(kind: ModelKind) -> &'static str {
    match kind {
        ModelKind::Citizen => "NPC_Citizen.die",
        ModelKind::Combine => "NPC_CombineS.Die",
        ModelKind::MetroPolice => "NPC_MetroPolice.Die",
    }
}

/// The footstep entries of a model kind (15.6).
pub fn steps(kind: ModelKind) -> StepSounds {
    let (l, r) = match kind {
        ModelKind::Citizen => ("NPC_Citizen.RunFootstepLeft", "NPC_Citizen.RunFootstepRight"),
        ModelKind::Combine => ("NPC_CombineS.RunFootstepLeft", "NPC_CombineS.RunFootstepRight"),
        ModelKind::MetroPolice => ("NPC_MetroPolice.RunFootstepLeft", "NPC_MetroPolice.RunFootstepRight"),
    };
    StepSounds { left: l.into(), right: r.into() }
}

/// Whether a model path is a female citizen's (the script's `$gender`).
pub fn female(path: &str) -> bool {
    path.to_ascii_lowercase().contains("female")
}

/// A body the energy ball is dissolving (M5 puts it on): it has no death
/// voice (the ragdoll is dissolving when the voice is decided, 15.8).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Dissolving;

pub struct Hl2SoundsPlugin;

impl Plugin for Hl2SoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Damage>()
            .add_message::<Died>()
            .add_message::<GameSound>()
            .add_message::<PlaySound>()
            .add_observer(
                |added: On<Add<Suit>>, mut commands: Commands| {
                    // A bullet hit on an HL2:DM character sounds here, the
                    // flesh impact at the hit (the shooter's own copy).
                    commands.entity(added.entity).insert(source_engine::effects::impacts::HitSoundHere);
                },
            )
            .add_systems(FixedUpdate, hurt_sounds.after(SimSet::Weapons).before(crate::core::apply_damage).run_if(crate::core::authoritative))
            .add_systems(FixedUpdate, model_steps.before(SimSet::Movement))
            .add_systems(Update, death_sounds);
    }
}

/// A hit that is taken, sounded from the victim to everyone (15.7): a
/// bullet's flesh impact, fire's burn voice. Falls and drowning have
/// theirs elsewhere (movement, the suit); club, blast and crush none.
#[allow(clippy::type_complexity)]
fn hurt_sounds(
    mut damage: MessageReader<Damage>,
    victims: Query<(&Health, &Transform, Option<&GameIdentity>), (With<Suit>, With<crate::core::Intent>)>,
    rules: DamageRules,
    mut out: MessageWriter<GameSound>,
) {
    let mut left: BTreeMap<Entity, f32> = BTreeMap::new();
    for d in damage.read() {
        let Ok((health, at, identity)) = victims.get(d.target) else { continue };
        let h = left.entry(d.target).or_insert(health.current);
        if *h <= 0.0 || !rules.lands(d) || !GameIdentity::allows(identity, GAME) {
            continue;
        }
        *h -= d.amount;
        let entry = match d.kind {
            DamageKind::Bullet => FLESH_IMPACT,
            DamageKind::Burn => BURN_PAIN,
            _ => continue,
        };
        out.write(GameSound(PlaySound { source: Some(d.target), ..PlaySound::at(entry, at.translation) }));
    }
}

/// The voice of a death (15.8), from the body's origin.
fn death_sounds(
    mut died: MessageReader<Died>,
    victims: Query<(&GlobalTransform, Option<&Hl2Model>, Option<&GameIdentity>, Has<Dissolving>), With<Suit>>,
    mut play: MessageWriter<PlaySound>,
) {
    for d in died.read() {
        let Ok((at, model, identity, dissolving)) = victims.get(d.entity) else { continue };
        if dissolving || !GameIdentity::allows(identity, GAME) {
            continue;
        }
        let path = model.map_or(DEFAULT_MODEL, |m| m.0.as_str());
        let Some(kind) = ModelKind::of(path) else { continue };
        let mut entry = death_entry(kind).to_string();
        if kind == ModelKind::Citizen && female(path) {
            entry.push_str(source_engine::content::sound::FEMALE_SUFFIX);
        }
        play.write(PlaySound { source: Some(d.entity), ..PlaySound::at(entry, at.translation()) });
    }
}

/// A player's footsteps follow its model's kind (15.6); a path of no kind
/// keeps the last.
#[allow(clippy::type_complexity)]
fn model_steps(
    mut players: Query<(&Hl2Model, &mut StepSounds), Or<(Changed<Hl2Model>, Added<StepSounds>)>>,
) {
    // A player with no model keeps the initial citizen's steps.
    for (model, mut steps_now) in &mut players {
        if let Some(kind) = ModelKind::of(&model.0) {
            let want = steps(kind);
            if steps_now.left != want.left {
                *steps_now = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_their_entries() {
        // Spec tests 1-3: case-insensitive paths, by kind.
        let kind = |p: &str| ModelKind::of(p).unwrap();
        assert_eq!(steps(kind("models/humans/group03/female_04.mdl")).left, "NPC_Citizen.RunFootstepLeft");
        assert_eq!(steps(kind("models/police.mdl")).right, "NPC_MetroPolice.RunFootstepRight");
        assert_eq!(steps(kind("models/combine_super_soldier.mdl")).left, "NPC_CombineS.RunFootstepLeft");
        assert_eq!(steps(kind("models/Humans/Group03/male_01.mdl")).left, "NPC_Citizen.RunFootstepLeft");
        assert_eq!(death_entry(ModelKind::Combine), "NPC_CombineS.Die");
        assert!(female("models/humans/group03/female_04.mdl") && !female("models/humans/group03/male_04.mdl"));
    }

    #[test]
    fn the_entries_include_the_weapons() {
        let all = entries();
        for want in ["Weapon_Pistol.Single", "Weapon_SMG1.Single", "Weapon_Shotgun.Double", FLESH_IMPACT] {
            assert!(all.iter().any(|e| e == want), "{want}");
        }
    }

    /// The HL2:DM playtest's silent sounds: what hooks and effects play
    /// must be loaded with the map (a name that isn't plays nothing, with
    /// no error). The install's check of these: `heavy::sound_stock`.
    #[test]
    fn the_entries_include_what_hooks_and_effects_play() {
        let all = entries();
        for want in [
            "Weapon_PhysCannon.Pickup",
            "Weapon_PhysCannon.Launch",
            "Weapon_PhysCannon.Drop",
            "Weapon_PhysCannon.DryFire",
            "Weapon_PhysCannon.TooHeavy",
            "Weapon_PhysCannon.OpenClaws",
            "Weapon_PhysCannon.CloseClaws",
            "Weapon_PhysCannon.HoldSound",
            "NPC_CombineBall.HoldingInPhysCannon",
            "Weapon_SMG1.Double",
            "Weapon_RPG.Single",
            "Weapon_IRifle.Single",
            "Weapon_CombineGuard.Special1",
            "NPC_CombineBall.Launch",
            "NPC_CombineBall.Impact",
            "NPC_CombineBall.Explosion",
            "Weapon_Shotgun.Special1",
            "WeaponFrag.Roll",
            "Grenade.Blip",
            "TripmineGrenade.Place",
            "Weapon_SLAM.SatchelThrow",
            "BaseExplosionEffect.Sound",
            "BaseGrenade.Explode",
            "Weapon_RPG.LaserOn",
            "Missile.Ignite",
            "Weapon_Crossbow.BoltSkewer",
        ] {
            assert!(all.iter().any(|e| e == want), "{want}");
        }
    }
}
