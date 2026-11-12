//! Announcers for any game (docs/DESIGN.md, "Announcers";
//! specs/combat_arms/kill_streaks.md, Part A the rules, Part B a pack):
//! the vocabulary the server's counting (`chimera_sim::announce`), the
//! network (`net::Killed` carries `KillFacts`) and the client shell's
//! banner and voice (`chimera_client::client::announcer`) share.
//!
//! - **A pack** (`AnnouncerPack`) is data, a TOML document a game
//!   contributes (`game_parts::GameParts::announcer`,
//!   `AnnouncerAppExt::register_announcer`): its multi-kill window, the
//!   streak steps by count, the special kills (first kill, headshot, groin
//!   shot, revenge), the plain-kill banner (`PlainKill`: "KILL" / "KILLS"
//!   with per-digit images in fixed slots; optional), each callout's banner art, top image, voice and
//!   priority level, the banner's layout on its reference screen, the
//!   animation's timings, and weapon skins (a weapon's own art and
//!   voices; a game may add skins it reads at run time,
//!   `AnnouncerAppExt::announcer_skins`: Combat Arms' from its install).
//!   Its files are paths in the install of the pack's `game`, loaded at
//!   run time.
//! - **Which pack** is the server's setting (`sv_announcer`, replicated:
//!   `default`, the first registered pack; `none`; `random`, one of them
//!   per map; or a pack's id), resolved by the server
//!   (`AnnouncerSetting::resolve`) into `ActiveAnnouncer`.
//! - **What a kill was** (`KillFacts`): the killer's multi-kill count,
//!   first kill, a head or groin shot, revenge, the killer's match kill
//!   total (the plain-kill banner's number), the killer's body: worked
//!   out by the server for an enemy kill and sent with the kill
//!   (`KillAnnounced`); every client decides from it what to show.

use std::{collections::BTreeMap, sync::Arc};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One callout: a banner (with an optional image drawn over it), its
/// voice, and its priority level (a banner with a higher level showing
/// drops a request with a lower one; kill_streaks.md A4).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Callout {
    /// The name a weapon skin overrides it by (`Skin::images`, `voices`).
    pub key: String,
    /// What it says ("DOUBLE KILL").
    pub name: String,
    /// The banner's image file.
    pub banner: String,
    /// An image over the banner (the skull over HEADSHOT).
    #[serde(default)]
    pub top: Option<String>,
    /// The voice's sound file (None: silent).
    #[serde(default)]
    pub voice: Option<String>,
    /// The voice when the killer's body is a woman's (`core::FemaleBodies`;
    /// None: `voice`).
    #[serde(default)]
    pub voice_female: Option<String>,
    pub level: i32,
}

/// A streak step: the callout of a multi-kill count from `count` up to
/// the next step's.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Step {
    pub count: u8,
    #[serde(flatten)]
    pub callout: Callout,
}

/// A rectangle (x, y, width, height) on the pack's reference screen.
pub type Rect = [f32; 4];

/// Where the banner is drawn: rectangles on a reference screen, each
/// axis stretched to the real one (kill_streaks.md, "Units").
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub reference: [f32; 2],
    pub banner: Rect,
    pub top: Rect,
}

/// The plain-kill banner (kill_streaks.md B2a): an enemy kill by the local
/// player that is no step, first kill, revenge, headshot or groin shot
/// shows "KILL" (the killer's match kill total N <= 1) or "KILLS" (N > 1)
/// with N in digits, one image per digit in three fixed slots. A pack
/// without one announces no plain kills.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlainKill {
    /// "KILL" (its `voice`, if the pack has one, plays; B2a).
    pub one: Callout,
    /// "KILLS" (no voice).
    pub many: Callout,
    /// A digit's image is this followed by the digit and `.dtx`.
    pub digit_prefix: String,
    /// The ones, tens and hundreds digit slots on the layout's reference
    /// screen, each showing its whole digit image.
    pub digits: [Rect; 3],
}

impl PlainKill {
    /// The image file of a digit.
    pub fn digit_file(&self, digit: u8) -> String {
        format!("{}{digit}.dtx", self.digit_prefix)
    }
}

/// The banner's animation (kill_streaks.md A5; seconds, sizes as scales).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    /// T: hidden on the first frame at or past it.
    pub total: f32,
    /// t0, c: the slam-in's start and length.
    pub start: f32,
    pub creating: f32,
    /// b: the echo's length, from the slam-in's end.
    pub blowing: f32,
    /// te, d: the blow-out's start and length.
    pub end: f32,
    pub destroying: f32,
    /// S0, B, L: the main copy's first scale, the echo's last, the main
    /// copy's last.
    pub start_size: f32,
    pub blow_size: f32,
    pub last_size: f32,
    /// The shake during the echo: whole screen pixels, each axis in
    /// -shake..=shake.
    pub shake: i32,
}

/// A weapon skin: its weapons (ids, `game:name`) draw `images` and play
/// `voices` in place of the callouts' (by `Callout::key`; a top image by
/// `<key>_top`, a woman killer's voice by `<key>_female`). A key it lacks
/// falls back to the pack's own (kill_streaks.md A8, Open question 7).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Skin {
    pub name: String,
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub images: BTreeMap<String, String>,
    #[serde(default)]
    pub voices: BTreeMap<String, String>,
}

/// An announcer pack (module doc).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnnouncerPack {
    /// Its id (`sv_announcer <id>`).
    pub id: String,
    pub name: String,
    /// The game whose install holds its files (`<game>://<path>`).
    pub game: String,
    /// Enemy kills each less than this after the killer's previous one
    /// raise the multi-kill count (ms; kill_streaks.md A1).
    pub window_ms: u32,
    /// The victim had killed the killer this many times since the killer
    /// last killed them: a revenge kill.
    pub revenge_after: u32,
    pub layout: Layout,
    pub timing: Timing,
    /// By `count`, rising.
    pub steps: Vec<Step>,
    #[serde(default)]
    pub first_kill: Option<Callout>,
    #[serde(default)]
    pub headshot: Option<Callout>,
    #[serde(default)]
    pub groin: Option<Callout>,
    #[serde(default)]
    pub revenge: Option<Callout>,
    #[serde(default)]
    pub plain_kill: Option<PlainKill>,
    #[serde(default)]
    pub skins: Vec<Skin>,
}

impl AnnouncerPack {
    /// A pack from its TOML document.
    pub fn parse(toml: &str) -> Result<Self, String> {
        let mut pack: Self = toml::from_str(toml).map_err(|e| e.to_string())?;
        pack.steps.sort_by_key(|s| s.count);
        Ok(pack)
    }

    /// The step of a multi-kill count (None: below the first).
    pub fn step(&self, count: u8) -> Option<&Callout> {
        self.steps.iter().rev().find(|s| count >= s.count).map(|s| &s.callout)
    }

    /// The skin of a weapon (by id), if one names it.
    pub fn skin(&self, weapon: &str) -> Option<&Skin> {
        self.skins.iter().find(|s| s.weapons.iter().any(|w| w.eq_ignore_ascii_case(weapon)))
    }

    /// Every callout.
    pub fn callouts(&self) -> impl Iterator<Item = &Callout> {
        self.steps
            .iter()
            .map(|s| &s.callout)
            .chain([&self.first_kill, &self.headshot, &self.groin, &self.revenge].into_iter().flatten())
    }

    /// Every image and every sound file it names (skins' too), sorted,
    /// once each: what a client loads ahead.
    pub fn files(&self) -> (Vec<String>, Vec<String>) {
        let mut images: Vec<String> = Vec::new();
        let mut sounds: Vec<String> = Vec::new();
        for c in self.callouts() {
            images.push(c.banner.clone());
            images.extend(c.top.clone());
            sounds.extend(c.voice.clone());
            sounds.extend(c.voice_female.clone());
        }
        if let Some(p) = &self.plain_kill {
            for c in [&p.one, &p.many] {
                images.push(c.banner.clone());
                sounds.extend(c.voice.clone());
            }
            images.extend((0..10).map(|d| p.digit_file(d)));
        }
        for s in &self.skins {
            images.extend(s.images.values().cloned());
            sounds.extend(s.voices.values().cloned());
        }
        for list in [&mut images, &mut sounds] {
            list.sort();
            list.dedup();
        }
        (images, sounds)
    }
}

/// The packs the app's games contributed, in the order registered, and
/// the weapon skins read for packs at run time (`add_skins`).
#[derive(Resource, Clone, Debug, Default)]
pub struct AnnouncerPacks(pub Vec<Arc<AnnouncerPack>>, BTreeMap<String, Vec<Skin>>);

impl AnnouncerPacks {
    pub fn get(&self, id: &str) -> Option<&Arc<AnnouncerPack>> {
        self.0.iter().find(|p| p.id.eq_ignore_ascii_case(id))
    }

    /// Add (or replace, by id) a pack, with the skins added for it.
    pub fn register(&mut self, mut pack: AnnouncerPack) {
        if let Some(skins) = self.1.get(&pack.id) {
            merge_skins(&mut pack.skins, skins);
        }
        match self.0.iter_mut().find(|p| p.id == pack.id) {
            Some(p) => *p = Arc::new(pack),
            None => self.0.push(Arc::new(pack)),
        }
    }

    /// Weapon skins for pack `id` that a game reads at run time (Combat
    /// Arms' from its install, kill_streaks.md A8): over the pack's own
    /// by name, now and whenever the pack is registered again or later.
    pub fn add_skins(&mut self, id: &str, skins: Vec<Skin>) {
        merge_skins(self.1.entry(id.to_string()).or_default(), &skins);
        if let Some(p) = self.0.iter_mut().find(|p| p.id == id) {
            merge_skins(&mut Arc::make_mut(p).skins, &skins);
        }
    }
}

/// `skins` into `into`, replacing those of the same name.
fn merge_skins(into: &mut Vec<Skin>, skins: &[Skin]) {
    for s in skins {
        match into.iter_mut().find(|x| x.name == s.name) {
            Some(x) => *x = s.clone(),
            None => into.push(s.clone()),
        }
    }
}

/// The server's choice of pack (`sv_announcer`): `default` (the first
/// registered: the party apps' and Combat Arms', Combat Arms'; none where
/// no game has one, plain CS:S), `none`, `random` (one per map) or a
/// pack's id.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct AnnouncerSetting(pub String);

impl Default for AnnouncerSetting {
    fn default() -> Self {
        Self("default".into())
    }
}

impl AnnouncerSetting {
    /// The pack the setting picks from `packs`; `seed` picks the random
    /// one (the map's).
    pub fn resolve(&self, packs: &AnnouncerPacks, seed: u64) -> Option<Arc<AnnouncerPack>> {
        match self.0.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "0" => None,
            "default" => packs.0.first().cloned(),
            "random" if !packs.0.is_empty() => packs.0.get((seed % packs.0.len() as u64) as usize).cloned(),
            "random" => None,
            id => packs.get(id).cloned(),
        }
    }
}

/// The pack the server announces with now (None: no announcer).
#[derive(Resource, Clone, Debug, Default)]
pub struct ActiveAnnouncer(pub Option<Arc<AnnouncerPack>>);

/// A killing hit's special place (kill_streaks.md A1: head, groin; never
/// for damage that can't headshot: blasts, burns).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialShot {
    #[default]
    None,
    Head,
    Groin,
}

/// What an enemy kill was, as the server counted it (kill_streaks.md A1):
/// sent with the kill to every client.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KillFacts {
    /// The pack the server announces with (`AnnouncerPack::id`).
    pub pack: String,
    /// The killer's multi-kill count, this kill included (1: none before
    /// within the window).
    pub multi: u8,
    /// The first kill of the round (or map).
    pub first: bool,
    pub shot: SpecialShot,
    /// The victim had killed the killer `revenge_after` times since.
    pub revenge: bool,
    /// The killer's match kill total, this kill included (enemy kills
    /// since the map began, not reset by a round): the plain-kill
    /// banner's number (B2a).
    pub total: u32,
    /// The killer's body is a woman's (`core::FemaleBodies`): the groin
    /// shot's voice goes by it (kill_streaks.md Corrections 6).
    pub killer_female: bool,
}

/// An enemy kill with its facts (on the server as counted, on a client as
/// received): what announcers read.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct KillAnnounced {
    pub victim: Entity,
    pub killer: Entity,
    /// The killing weapon's id (its skin), when known.
    pub weapon: Option<&'static str>,
    pub facts: KillFacts,
}

/// The server's counting (`chimera_sim::announce`), before what sends
/// kills to clients.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnnounceSet;

pub trait AnnouncerAppExt {
    /// Register a pack from its TOML document (`AnnouncerPacks`).
    /// Panics on a pack that doesn't parse (it is the game's own data).
    fn register_announcer(&mut self, toml: &str) -> &mut Self;
    /// Weapon skins for pack `id`, read at run time
    /// (`AnnouncerPacks::add_skins`), before or after the pack registers.
    fn announcer_skins(&mut self, id: &str, skins: Vec<Skin>) -> &mut Self;
}

impl AnnouncerAppExt for App {
    fn register_announcer(&mut self, toml: &str) -> &mut Self {
        let pack = AnnouncerPack::parse(toml).unwrap_or_else(|e| panic!("announcer pack: {e}"));
        self.world_mut().get_resource_or_init::<AnnouncerPacks>().register(pack);
        self
    }

    fn announcer_skins(&mut self, id: &str, skins: Vec<Skin>) -> &mut Self {
        if !skins.is_empty() {
            self.world_mut().get_resource_or_init::<AnnouncerPacks>().add_skins(id, skins);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACK: &str = r#"
id = "stand_in"
name = "Stand-in"
game = "stand_in"
window_ms = 5000
revenge_after = 2
steps = [
    { count = 3, key = "three", name = "THREE", banner = "three.png", level = 4 },
    { count = 2, key = "two", name = "TWO", banner = "two.png", voice = "two.wav", level = 4 },
]
headshot = { key = "head", name = "HEAD", banner = "head.png", top = "skull.png", voice = "head.wav", level = 5 }
[layout]
reference = [1024.0, 768.0]
banner = [256.0, 76.0, 512.0, 256.0]
top = [256.0, 93.0, 512.0, 256.0]
[timing]
total = 1.4
start = 0.0
creating = 0.1
blowing = 0.15
end = 1.2
destroying = 0.2
start_size = 5.0
blow_size = 3.0
last_size = 5.0
shake = 5
[[skins]]
name = "shiny"
weapons = ["stand_in:gun"]
images = { two = "two_shiny.png" }
"#;

    #[test]
    fn steps_by_count_and_files() {
        let p = AnnouncerPack::parse(PACK).unwrap();
        assert_eq!(p.step(1), None);
        assert_eq!(p.step(2).unwrap().name, "TWO");
        assert_eq!(p.step(3).unwrap().name, "THREE");
        assert_eq!(p.step(200).unwrap().name, "THREE", "the last step from its count up");
        assert_eq!(p.skin("STAND_IN:GUN").unwrap().name, "shiny");
        let (images, sounds) = p.files();
        assert_eq!(images, ["head.png", "skull.png", "three.png", "two.png", "two_shiny.png"]);
        assert_eq!(sounds, ["head.wav", "two.wav"]);
    }

    /// Skins a game reads at run time join its pack, before or after it
    /// registers, over the pack's own of the same name.
    #[test]
    fn skins_added_at_run_time() {
        let skin = |name: &str, weapon: &str| Skin {
            name: name.into(),
            weapons: vec![weapon.into()],
            ..Default::default()
        };
        let mut packs = AnnouncerPacks::default();
        packs.add_skins("stand_in", vec![skin("early", "stand_in:a")]);
        packs.register(AnnouncerPack::parse(PACK).unwrap());
        packs.add_skins("stand_in", vec![skin("shiny", "stand_in:b"), skin("late", "stand_in:c")]);
        let p = packs.get("stand_in").unwrap();
        let names: Vec<&str> = p.skins.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["shiny", "early", "late"]);
        assert_eq!(p.skin("stand_in:b").unwrap().name, "shiny", "replaced by name");
        assert!(p.skin("stand_in:gun").is_none());
        packs.register(AnnouncerPack::parse(PACK).unwrap());
        assert_eq!(packs.get("stand_in").unwrap().skins.len(), 3, "kept on a new registration");
    }

    #[test]
    fn the_setting_picks_a_pack() {
        let mut packs = AnnouncerPacks::default();
        let s = |v: &str| AnnouncerSetting(v.into());
        assert!(s("default").resolve(&packs, 0).is_none(), "no packs: none");
        packs.register(AnnouncerPack::parse(PACK).unwrap());
        let mut other = AnnouncerPack::parse(PACK).unwrap();
        other.id = "other".into();
        packs.register(other);
        assert_eq!(s("default").resolve(&packs, 0).unwrap().id, "stand_in");
        assert_eq!(s("Other").resolve(&packs, 0).unwrap().id, "other");
        assert!(s("none").resolve(&packs, 0).is_none());
        assert!(s("nobody").resolve(&packs, 0).is_none());
        assert_eq!(s("random").resolve(&packs, 1).unwrap().id, "other");
        assert_eq!(s("random").resolve(&packs, 2).unwrap().id, "stand_in");
    }
}
