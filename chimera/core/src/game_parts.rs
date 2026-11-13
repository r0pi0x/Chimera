//! What each game can lend a game made of several (docs/plans/active/
//! ultimate-gun-game.md, "The game parts contract"): a game's own claim
//! of its parts (`GameParts`: guns with their class and its knife, its
//! movement, its character kit, its HUD, its maps, its announcer pack, its minigame cue pack, its match-end
//! win and lose sounds, whether it lends a music director), registered by the app
//! that composes the game (`GamePartsAppExt::register_parts`) into
//! `PartsRegistry`, in order. Plain data, no game named: a game fills its
//! own `GameParts` (`cs_source::game::parts::PARTS`,
//! `combat_arms::game::parts::PARTS`), an app reads them all.
//!
//! The claim is checked against what the app really composed by
//! `chimera_sim::minigame::auto::report` (each gun in the weapon
//! catalogue, the movement registered): that report decides what a game
//! may take part in. Required for a game to be in an automatic ladder:
//! guns and movement; optional, with fallbacks (`PartsRegistry::
//! fallback_hud`, the shell's capsule body, other games' maps): its
//! characters, HUD and maps. A game's guns are its claimed ones plus every
//! weapon of its own in the catalogue whose data names a `class`
//! (`GunClass`; its install's weapons too), its maps the claimed ones plus
//! its installed maps that its pool rules allow (its data/map_pool.toml,
//! `chimera_sim::minigame::map_fit`).

use bevy::prelude::*;

/// A gun's class: which band of an automatic ladder's power curve it
/// plays in (`chimera_sim::minigame::power`, its data maps each class to
/// a band: pistols warm up, SMGs and shotguns rise, rifles and carbines
/// peak, machine guns, marksman rifles, snipers and heavy pistols are
/// heavy, launchers and flamethrowers special), and `Grenade`, a thrown
/// weapon that kills (the grenade band). A weapon's data can name it
/// (`class = "rifle"`, `chimera_sim::weapon::data`), so a game's weapons
/// from its install join a ladder without a hand list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GunClass {
    Pistol,
    Smg,
    Shotgun,
    Carbine,
    Rifle,
    /// A machine gun.
    Heavy,
    Marksman,
    Sniper,
    HeavyPistol,
    Launcher,
    Flamethrower,
    Grenade,
}

impl GunClass {
    /// Every gun class, roughly weakest first (not `Grenade`: not a gun).
    pub const ALL: [GunClass; 11] = [
        GunClass::Pistol,
        GunClass::Smg,
        GunClass::Shotgun,
        GunClass::Carbine,
        GunClass::Rifle,
        GunClass::Heavy,
        GunClass::Marksman,
        GunClass::Sniper,
        GunClass::HeavyPistol,
        GunClass::Launcher,
        GunClass::Flamethrower,
    ];

    /// Its name in data (`heavy_pistol`).
    pub fn name(self) -> &'static str {
        match self {
            GunClass::Pistol => "pistol",
            GunClass::Smg => "smg",
            GunClass::Shotgun => "shotgun",
            GunClass::Carbine => "carbine",
            GunClass::Rifle => "rifle",
            GunClass::Heavy => "heavy",
            GunClass::Marksman => "marksman",
            GunClass::Sniper => "sniper",
            GunClass::HeavyPistol => "heavy_pistol",
            GunClass::Launcher => "launcher",
            GunClass::Flamethrower => "flamethrower",
            GunClass::Grenade => "grenade",
        }
    }
}

/// One gun a game lends: its weapon id (`game:name`, in the weapon
/// catalogue) and its class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GunPart {
    pub id: &'static str,
    pub class: GunClass,
}

/// A game's parts, as the game itself claims them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameParts {
    /// The game's id (`GameInfo::id`; a stand-in may have no `GameInfo`).
    pub game: &'static str,
    /// Its guns (weapon set, held and view models, sounds), any order.
    pub guns: &'static [GunPart],
    /// Its melee weapon, given beside a gun of its game.
    pub knife: Option<&'static str>,
    /// Its movement's id (`slots::MovementRegistry`).
    pub movement: Option<&'static str>,
    /// Its character kit (bodies a map of any game can wear: its own maps'
    /// and, as a guest, others': `MapSources::players`/`guests`).
    pub characters: Option<&'static str>,
    /// Its HUD's id for the gameplay's `hud` slot (its game id).
    pub hud: Option<&'static str>,
    /// Its maps by id (`game:map`) that load in a composed app, first in
    /// an automatic pool; its other installed maps join by its pool rules
    /// (data/map_pool.toml, `chimera_sim::minigame::map_fit`).
    pub maps: &'static [&'static str],
    /// Its announcer pack, a TOML document (`announcer::AnnouncerPack`),
    /// registered with its parts.
    pub announcer: Option<&'static str>,
    /// A sound file in its install (a path its mount reads, loaded at run
    /// time) that a party match's end plays when someone won (the shell's
    /// match-end screen, `chimera_client::client::match_end`: the HUD's
    /// game's first, else another game's; none if missing; a draw plays
    /// none): to the winner, and to everyone else too unless the game has
    /// a `match_lost_sound` (`PartsRegistry::match_end_sounds`).
    pub match_won_sound: Option<&'static str>,
    /// The sound the same screen plays to a player who lost instead
    /// (Combat Arms' `ResultLoseSound`); none: its `match_won_sound`.
    pub match_lost_sound: Option<&'static str>,
    /// It lends a music director that plays a party match's moments
    /// (`chimera_sim::minigame::moments`: the match starting, the last
    /// rung, the respawn countdown, the end); a minigame's music is the
    /// first such game's unless its definition names one (`music`).
    pub music: bool,
    /// Its minigame cue pack, a TOML document (`cues::CuePack`: the
    /// sounds of a gun game's rung ups, last rung, lead and win),
    /// registered with its parts.
    pub cues: Option<&'static str>,
}

impl GameParts {
    /// Guns and movement: what a game needs to be in a ladder.
    pub fn playable(&self) -> bool {
        !self.guns.is_empty() && self.movement.is_some()
    }
}

/// The HUD of a game that has none (the shell's own: crosshair, health,
/// ammo; no game's HUD drawn).
pub const MINIMAL_HUD: &str = "chimera";

/// An app that never draws a stand-in for a model it hasn't got (a
/// player's capsule, a first-person box gun or knife: Ultimate Gun Game):
/// what is showing stays until the real model is ready, or nothing shows
/// for a moment. Without it (the greybox app, Combat Arms without its
/// keys, tests) the stand-ins draw.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoStandIns;

/// The registered games' parts, in the order registered, and the HUD a
/// game without one falls back to.
#[derive(Resource, Clone, Debug)]
pub struct PartsRegistry {
    parts: Vec<&'static GameParts>,
    /// For a game whose parts have no HUD: another game's (an app's
    /// choice) or `MINIMAL_HUD`.
    pub fallback_hud: &'static str,
}

impl Default for PartsRegistry {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            fallback_hud: MINIMAL_HUD,
        }
    }
}

impl PartsRegistry {
    pub fn get(&self, game: &str) -> Option<&'static GameParts> {
        self.parts.iter().copied().find(|p| p.game.eq_ignore_ascii_case(game))
    }

    pub fn all(&self) -> impl Iterator<Item = &'static GameParts> + '_ {
        self.parts.iter().copied()
    }

    /// Add (or replace, by game) `parts`.
    pub fn register(&mut self, parts: &'static GameParts) {
        match self.parts.iter_mut().find(|p| p.game == parts.game) {
            Some(p) => *p = parts,
            None => self.parts.push(parts),
        }
    }

    /// The match-end sound's candidates `(game, file)` for a player who
    /// `won` (or lost), `first` (the HUD's game) before the others: each
    /// game's `match_won_sound`, or for a loser its `match_lost_sound`
    /// where it has one (else its won sound).
    pub fn match_end_sounds(&self, first: Option<&str>, won: bool) -> Vec<(String, String)> {
        let mut all: Vec<_> = self.all().collect();
        all.sort_by_key(|p| first.is_none_or(|f| !p.game.eq_ignore_ascii_case(f)));
        all.into_iter()
            .filter_map(|p| {
                let file = if won { p.match_won_sound } else { p.match_lost_sound.or(p.match_won_sound) };
                file.map(|f| (p.game.to_string(), f.to_string()))
            })
            .collect()
    }

    /// The first game whose parts lend a music director (`music`).
    pub fn music(&self) -> Option<&'static str> {
        self.parts.iter().find(|p| p.music).map(|p| p.game)
    }

    /// The HUD a player of `game` sees: its own, else `fallback_hud`.
    /// None for a game not registered here.
    pub fn hud_of(&self, game: &str) -> Option<&'static str> {
        self.get(game).map(|p| p.hud.unwrap_or(self.fallback_hud))
    }
}

pub trait GamePartsAppExt {
    /// Register a game's parts (`PartsRegistry`).
    fn register_parts(&mut self, parts: &'static GameParts) -> &mut Self;
}

impl GamePartsAppExt for App {
    fn register_parts(&mut self, parts: &'static GameParts) -> &mut Self {
        self.world_mut().get_resource_or_init::<PartsRegistry>().register(parts);
        if let Some(pack) = parts.announcer {
            crate::announcer::AnnouncerAppExt::register_announcer(self, pack);
        }
        if let Some(pack) = parts.cues {
            crate::cues::CueAppExt::register_cues(self, pack);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static GUNS_ONLY: GameParts = GameParts {
        game: "stand_in",
        guns: &[GunPart {
            id: "stand_in:gun",
            class: GunClass::Rifle,
        }],
        knife: None,
        movement: Some("stand_in:movement"),
        characters: None,
        hud: None,
        maps: &[],
        announcer: None,
        match_won_sound: None,
        match_lost_sound: None,
        music: false,
        cues: None,
    };

    #[test]
    fn a_game_without_a_hud_gets_the_fallback() {
        let mut r = PartsRegistry::default();
        r.register(&GUNS_ONLY);
        assert!(GUNS_ONLY.playable());
        assert_eq!(r.hud_of("stand_in"), Some(MINIMAL_HUD));
        r.fallback_hud = "other";
        assert_eq!(r.hud_of("STAND_IN"), Some("other"));
        assert_eq!(r.hud_of("nobody"), None);
        r.register(&GUNS_ONLY);
        assert_eq!(r.all().count(), 1, "registered again: replaced");
    }

    #[test]
    fn the_match_end_sound_is_the_huds_games_win_or_lose_and_music_the_first_lender() {
        static CHEER: GameParts = GameParts {
            game: "cheer",
            match_won_sound: Some("cheer.wav"),
            ..GUNS_ONLY
        };
        static RESULT: GameParts = GameParts {
            game: "result",
            match_won_sound: Some("win.wav"),
            match_lost_sound: Some("lose.wav"),
            music: true,
            ..GUNS_ONLY
        };
        let mut r = PartsRegistry::default();
        r.register(&CHEER);
        r.register(&RESULT);
        let first = |hud, won| r.match_end_sounds(Some(hud), won)[0].1.clone();
        assert_eq!(first("result", true), "win.wav");
        assert_eq!(first("result", false), "lose.wav", "a loser hears the lose sound");
        assert_eq!(first("cheer", false), "cheer.wav", "no lose sound: the win sound to everyone");
        assert_eq!(r.match_end_sounds(None, true)[0].1, "cheer.wav", "else the order registered");
        assert_eq!(r.music(), Some("result"));
    }
}
