//! Halo's sounds on the engine's (specs/halo_ce/sounds.md "Distance",
//! "Gain"; the four weapons' table): the distance law as a `SoundLevel`
//! (`level`) and which first-person graph sound a weapon event starts,
//! and when (`event_sound`).
//!
//! **The distance mapping.** Halo attenuates a positional mono sound as
//! 1 within its minimum distance m and m / d beyond, with the maximum
//! distance M an audibility cut-off (silent past it). The engine's Source
//! level L gives min(1, 10^((L - 60) / 20) x 0.9144 m / d): the same
//! inverse law. With L = 60 + 20 log10(m / 0.9144 m) the two agree for
//! every d up to M; past M Halo is silent and the engine still gives m / d.
//! The largest deviation is there, just past M: m / M (weapon fire 4 / 70 =
//! 0.057, the empty click and the animation sounds 1 / 9 = 0.111, the
//! melee 0.5 / 3 = 0.167 of full gain).
//!
//! Known shortcuts: the sounds of the first-person graph are heard by the
//! one who holds the gun only (the spec: "only in first-person
//! perspective"), at the holder; a sound's gain is its permutations' mean
//! times its gain modifier at scale 1 (the melee sound's strength scale is
//! not applied).

use crate::map::sound::SoundLevel;

/// The engine's reference distance of its levels (36 Source units), m.
const REFERENCE: f32 = 36.0 * 0.0254;

/// The engine level whose fall-off is Halo's for minimum distance `min`
/// (WU; module doc).
pub fn level(min: f32) -> SoundLevel {
    let m = (min.max(0.01)) * super::super::METERS_PER_UNIT;
    SoundLevel::Db(60.0 + 20.0 * (m / REFERENCE).log10())
}

/// Halo's gain at `d` WU of a sound of distances `min`, `max` (WU):
/// sounds.md's mono law with its cut-off.
pub fn halo_gain(min: f32, max: f32, d: f32) -> f32 {
    if d > max {
        0.0
    } else if d <= min {
        1.0
    } else {
        min / d
    }
}

/// The first-person graph slot a weapon event plays (animations.md, the
/// view's states) and so the sound it starts: `ready` on a draw,
/// `reload-empty` / `reload-full` on a reload (the shotgun's first shell:
/// `enter`; each further shell `reload-empty`), `melee`, `overheating` on
/// an overheat, `o-h-exit` when it clears, `fire-1` on a shot (the
/// shotgun's pump), `exit-empty` / `exit-full` after the shotgun's last
/// shell.
pub fn event_slot(kind: &chimera_sim::weapon::WeaponEventKind, loaded: i16, shotgun: bool) -> Option<&'static str> {
    use chimera_sim::weapon::WeaponEventKind as K;
    Some(match kind {
        K::Deployed => "ready",
        K::ReloadStarted if shotgun => "enter",
        K::ReloadStarted if loaded <= 0 => "reload-empty",
        K::ReloadStarted => "reload-full",
        K::ShellInserting => "reload-empty",
        K::Action("melee") => "melee",
        K::Action("overheated") => "overheating",
        K::Action("overheat_cleared") => "o-h-exit",
        K::Action("fire") | K::Action("fire_last") => "fire-1",
        _ => return None,
    })
}

/// The sound a weapon event starts and the ticks until it does (its
/// animation's sound frame), from the gun's graph sounds.
pub fn event_sound<'a>(gun: &'a super::load::FxGun, slot: &str) -> Option<(&'a str, u32)> {
    gun.anim_sounds.get(slot).map(|(e, f)| (e.as_str(), (*f).max(0) as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::sound::distance_gain;

    /// The spec's worked values for m = 4, M = 70 WU at its sample
    /// distances: the engine's gain equals Halo's within the range; past it
    /// the deviation is m / d.
    #[test]
    fn the_distance_law_matches_halos() {
        let l = level(4.0);
        for (d, want) in [(0.0, 1.0), (4.0, 1.0), (5.0, 0.8), (8.0, 0.5), (16.0, 0.25), (40.0, 0.1), (70.0, 0.0571429)] {
            let got = distance_gain(l, d * super::super::super::METERS_PER_UNIT);
            assert!((got - want).abs() < 1e-4, "{d} WU: {got} vs {want}");
            assert!((halo_gain(4.0, 70.0, d) - want).abs() < 1e-4);
        }
        assert_eq!(halo_gain(4.0, 70.0, 71.0), 0.0);
        let past = distance_gain(l, 71.0 * super::super::super::METERS_PER_UNIT);
        assert!((past - 4.0 / 71.0).abs() < 1e-4);
        // The largest deviation of the four weapons' classes: the melee's.
        let melee = distance_gain(level(0.5), 3.0001 * super::super::super::METERS_PER_UNIT);
        assert!((melee - 0.5 / 3.0).abs() < 1e-3);
    }
}
