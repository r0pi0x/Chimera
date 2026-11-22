//! Source's sound mixer as data (specs/source/sound_hearing.md 6): the
//! active mixer in TF2, HL2:DM and CS:S is `Default_Mix`, whose groups
//! match a sound by a substring of its wave path, whether a player makes
//! it and a level range; the sound takes the value of the first matched
//! group that has one. `DEFAULT_MIX` holds the groups that decide a value
//! (in file order, groups with no value left out), so one table gives
//! TF2's 94 dB guns 0.72, HL2:DM's 140 dB guns 1.0 and its explosions
//! 0.90. `hearing` turns an entry's first wave and level into its
//! `EntryHearing` (the mixer values, the wall loss, distance-variant
//! waves).
//!
//! Known shortcuts (docs/tech-debt.md): the table is the spec's, not read
//! from the install's mixer script (its format isn't specified); the
//! groups' class-name and channel columns and ducking aren't kept (Q6); a
//! CS:S compatibility entry is matched at its encoded level below 256
//! (Q7). Why it's OK for now: the spec lists every value that matters in
//! the three games, and they share the script.

use crate::map::{
    SoundLevel,
    hearing::{EntryHearing, OBSCURED_DB, level_of, obscured_gain},
};

/// One mixer group that sets a value: matched by a wave-path substring
/// (empty: any), who makes the sound (None: anyone, Some(true): a
/// player) and the lowest level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixGroup {
    pub name: &'static str,
    pub wave: &'static str,
    pub player: Option<bool>,
    pub min_level: f32,
    pub value: f32,
}

const fn group(name: &'static str, wave: &'static str, player: Option<bool>, min_level: f32, value: f32) -> MixGroup {
    MixGroup { name, wave, player, min_level, value }
}

/// `Default_Mix`'s groups with a value, in order (sound_hearing.md 6).
pub const DEFAULT_MIX: [MixGroup; 7] = [
    group("Player_Weapons_Loud", "weapons/", Some(true), 140.0, 1.0),
    group("Explosions", "explo", None, 120.0, 0.90),
    group("Combat", "weapon", None, 120.0, 0.79),
    group("bullethit", "impact_bullet", None, 0.0, 0.67),
    group("Player_Suit", "fvox/", Some(true), 0.0, 0.56),
    group("Music", "music/", None, 0.0, 0.81),
    group("All", "", None, 0.0, 0.72),
];

/// The mixer value of a sound with wave `wave` (its path, prefix
/// characters and all) at `level`, made by a player or not.
pub fn mix_value(groups: &[MixGroup], wave: &str, level: f32, player: bool) -> f32 {
    let wave = wave.replace('\\', "/").to_ascii_lowercase();
    groups
        .iter()
        .find(|g| g.player.is_none_or(|p| p == player) && level >= g.min_level && wave.contains(g.wave))
        .map_or(1.0, |g| g.value)
}

/// How a Source entry whose first wave is `wave` at `level` is heard:
/// `Default_Mix`'s values for a player and for the world, the wall loss
/// of `snd_obscured_gain_dB`'s default, distance-variant when `near_far`.
pub fn hearing(wave: &str, level: SoundLevel, near_far: bool) -> EntryHearing {
    let l = level_of(level);
    EntryHearing {
        mix_player: mix_value(&DEFAULT_MIX, wave, l, true),
        mix_world: mix_value(&DEFAULT_MIX, wave, l, false),
        obscured: obscured_gain(OBSCURED_DB),
        near_far,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's table: TF2's guns 0.72, HL2:DM's 140 dB guns from a
    /// player 1.0, its explosions 0.90, impacts 0.67, the rest 0.72.
    #[test]
    fn default_mix_values() {
        let tf2 = hearing(")weapons/rocket_shoot.wav", SoundLevel::Db(94.0), false);
        assert_eq!((tf2.mix_player, tf2.mix_world), (0.72, 0.72));
        let magnum = hearing(")weapons/357/357_fire2.wav", SoundLevel::Db(140.0), false);
        assert_eq!(magnum.mix_player, 1.0);
        assert_eq!(magnum.mix_world, 0.79);
        let blast = hearing("^weapons/explode3.wav", SoundLevel::Db(140.0), true);
        assert_eq!(blast.mix_world, 0.90);
        assert!(blast.near_far);
        assert_eq!(hearing("physics/concrete/concrete_impact_bullet1.wav", SoundLevel::Db(75.0), false).mix_world, 0.67);
        assert_eq!(hearing("ambient/wind.wav", SoundLevel::Db(75.0), false).mix_world, 0.72);
        assert_eq!(hearing("fvox/bell.wav", SoundLevel::Db(75.0), false).mix_player, 0.56);
        assert!((tf2.obscured - 0.393).abs() < 1e-3);
    }
}
