//! The bots' data (data/bots.toml): each attribute template's values
//! (specs/combat_arms/bots.md, "What a level changes"), the per-weapon AI
//! rows by gun type, which gun type each weapon is, the profile counts and
//! the item sets. A template is a level and an archetype (`Template`).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::Deserialize;

/// The built-in data file.
pub const BOT_DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/bots.toml");

/// A difficulty level (the room's `BotLevel` 0-2; training's own).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Level {
    /// Shown as Easy (the data's "Normal").
    Easy,
    /// Shown as Normal (the data's "Hard").
    #[default]
    Normal,
    /// Shown as Veteran (the data's "Extreme").
    Veteran,
    /// Bot Training's "Tutorial".
    Tutorial,
}

impl Level {
    /// `BotLevel`: 0 Easy, 1 Normal, 2 Veteran (3: training).
    pub fn of(n: u8) -> Self {
        match n {
            0 => Level::Easy,
            1 => Level::Normal,
            2 => Level::Veteran,
            _ => Level::Tutorial,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn key(self) -> &'static str {
        ["easy", "normal", "veteran", "tutorial"][self.index()]
    }
}

/// An attribute template: a level and an archetype (0 N, 1 H, 2 E).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Template {
    pub level: Level,
    pub archetype: u8,
}

impl Template {
    pub fn new(level: Level, archetype: u8) -> Self {
        Self { level, archetype }
    }

    /// The column in the data's arrays (NN NH NE HN ... TE).
    pub fn column(self) -> usize {
        self.level.index() * 3 + self.archetype.min(2) as usize
    }

    /// The data's name of the template (`NN`, `XE`, ...).
    pub fn short(self) -> String {
        let l = ["N", "H", "X", "T"][self.level.index()];
        let a = ["N", "H", "E"][self.archetype.min(2) as usize];
        format!("{l}{a}")
    }
}

/// A number per template, or one for all.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum PerTemplate {
    One(f32),
    Each(Vec<f32>),
}

impl PerTemplate {
    pub fn at(&self, t: Template) -> f32 {
        match self {
            PerTemplate::One(v) => *v,
            PerTemplate::Each(v) => v.get(t.column()).copied().unwrap_or(v[0]),
        }
    }
}

/// The template table (`[template]`).
#[derive(Clone, Debug, Deserialize)]
pub struct TemplateTable {
    pub reaction_min: PerTemplate,
    pub reaction_max: PerTemplate,
    pub run_speed: PerTemplate,
    pub jump_speed: PerTemplate,
    pub notice_near: PerTemplate,
    pub notice_far: PerTemplate,
    pub notice_far_percent: PerTemplate,
    pub sight_cooldown_min: PerTemplate,
    pub camp_sight_step: PerTemplate,
    pub grenade_shots: PerTemplate,
    pub grenade_shots_flash: PerTemplate,
    pub grenade_shots_smoke: PerTemplate,
    pub attack_move_min: PerTemplate,
    pub attack_move_max: PerTemplate,
    pub shuffle: PerTemplate,
    pub chase_endurance: PerTemplate,
    pub dodge_delay_min: PerTemplate,
    pub dodge_delay_max: PerTemplate,
    pub safe_device: PerTemplate,
    pub mine_min: PerTemplate,
    pub mine_max: PerTemplate,
    pub stop_look_around_percent: PerTemplate,
}

/// One gun type's AI row (`[guns.<type>]`).
#[derive(Clone, Debug, Deserialize)]
pub struct GunTable {
    pub a: PerTemplate,
    pub c_max: PerTemplate,
    pub decay: PerTemplate,
    pub burst_min: PerTemplate,
    pub burst_max: PerTemplate,
    pub pause_min: PerTemplate,
    pub pause_max: PerTemplate,
    pub range_min: PerTemplate,
    pub range_max: PerTemplate,
    pub full_accuracy: PerTemplate,
    pub miss_offset: PerTemplate,
    pub ammo: PerTemplate,
    pub fire_frame: PerTemplate,
}

/// A gun type's row for one template.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GunAi {
    pub a: f32,
    pub c_max: f32,
    pub decay: f32,
    pub burst: (u32, u32),
    pub pause: (f32, f32),
    pub range: (f32, f32),
    pub full_accuracy: f32,
    pub miss_offset: f32,
    pub ammo: f32,
    pub fire_frame: f32,
}

impl GunTable {
    pub fn at(&self, t: Template) -> GunAi {
        GunAi {
            a: self.a.at(t),
            c_max: self.c_max.at(t),
            decay: self.decay.at(t),
            burst: (self.burst_min.at(t) as u32, self.burst_max.at(t) as u32),
            pause: (self.pause_min.at(t), self.pause_max.at(t)),
            range: (self.range_min.at(t), self.range_max.at(t)),
            full_accuracy: self.full_accuracy.at(t),
            miss_offset: self.miss_offset.at(t),
            ammo: self.ammo.at(t),
            fire_frame: self.fire_frame.at(t),
        }
    }
}

/// An item-set slot: its use-percent and list.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SlotList {
    pub percent: f32,
    pub items: Vec<String>,
}

/// An item set (`[[item_sets]]`): slots in filling order.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ItemSet {
    pub level: String,
    #[serde(default)]
    pub model: SlotList,
    #[serde(default)]
    pub backpack: SlotList,
    #[serde(default)]
    pub head: SlotList,
    #[serde(default)]
    pub goggle: SlotList,
    #[serde(default)]
    pub mid: SlotList,
    #[serde(default)]
    pub long: SlotList,
    #[serde(default)]
    pub secondary: SlotList,
    #[serde(default)]
    pub melee: SlotList,
    #[serde(default)]
    pub grenade: SlotList,
    #[serde(default)]
    pub mine: SlotList,
}

impl ItemSet {
    /// The slots in filling order: model, backpack, head, goggle, then the
    /// weapon slots 0-5.
    pub fn slots(&self) -> [&SlotList; 10] {
        [
            &self.model,
            &self.backpack,
            &self.head,
            &self.goggle,
            &self.mid,
            &self.long,
            &self.secondary,
            &self.melee,
            &self.grenade,
            &self.mine,
        ]
    }

    fn slots_mut(&mut self) -> [&mut SlotList; 10] {
        [
            &mut self.model,
            &mut self.backpack,
            &mut self.head,
            &mut self.goggle,
            &mut self.mid,
            &mut self.long,
            &mut self.secondary,
            &mut self.melee,
            &mut self.grenade,
            &mut self.mine,
        ]
    }

    /// The load-time padding ("Choosing the bots"): a list as long as an
    /// earlier one in the set gets copies of its own first entries until
    /// every non-empty list's length differs.
    pub fn pad(&mut self) {
        let mut seen: Vec<usize> = Vec::new();
        for s in self.slots_mut() {
            if s.items.is_empty() {
                continue;
            }
            let mut k = 0;
            while seen.contains(&s.items.len()) {
                let e = s.items[k].clone();
                s.items.push(e);
                k += 1;
            }
            seen.push(s.items.len());
        }
    }
}

/// The profile counts per level.
#[derive(Clone, Debug, Deserialize)]
pub struct Profiles {
    pub easy: usize,
    pub normal: usize,
    pub veteran: usize,
    pub tutorial: usize,
}

impl Profiles {
    pub fn count(&self, level: Level) -> usize {
        match level {
            Level::Easy => self.easy,
            Level::Normal => self.normal,
            Level::Veteran => self.veteran,
            Level::Tutorial => self.tutorial,
        }
    }
}

/// The alarm levels (`[alarm]`, 5.3 Corrections 2 and 3).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Alarm {
    /// The running total at which a bot is majorly alarmed.
    pub major: f32,
    pub gunfire: f32,
    pub footstep: f32,
    pub ally_call: f32,
}

impl Default for Alarm {
    fn default() -> Self {
        Self {
            major: 50.0,
            gunfire: 75.0,
            footstep: 20.0,
            ally_call: 1.0,
        }
    }
}

/// A profile's own chat cooldown (`NextChatTime`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProfileChat {
    pub level: String,
    pub profile: usize,
    pub cooldown: f64,
}

/// The chat table (`[chat]`, 10 Correction 6).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatTable {
    pub default_cooldown: f64,
    #[serde(default)]
    pub profiles: Vec<ProfileChat>,
}

impl Default for ChatTable {
    fn default() -> Self {
        Self {
            default_cooldown: 10.0,
            profiles: Vec::new(),
        }
    }
}

/// The bots' data.
#[derive(Resource, Clone, Debug, Deserialize)]
pub struct BotData {
    pub template: TemplateTable,
    pub guns: HashMap<String, GunTable>,
    pub gun_types: HashMap<String, String>,
    pub profiles: Profiles,
    pub item_sets: Vec<ItemSet>,
    #[serde(default)]
    pub alarm: Alarm,
    #[serde(default)]
    pub chat: ChatTable,
}

impl BotData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d: BotData = toml::from_str(text).map_err(|e| e.to_string())?;
        for s in &mut d.item_sets {
            s.pad();
        }
        Ok(d)
    }

    pub fn built_in() -> Self {
        Self::parse(&BOT_DATA).expect("data/bots.toml")
    }

    /// The gun type of weapon `id` (None: no AI row).
    pub fn gun_type(&self, id: &str) -> Option<&str> {
        self.gun_types.get(id).map(String::as_str)
    }

    /// Weapon `id`'s AI row for template `t`.
    pub fn gun(&self, id: &str, t: Template) -> Option<GunAi> {
        Some(self.guns.get(self.gun_type(id)?)?.at(t))
    }

    /// Profile `profile` of `level`'s chat cooldown (s): its own
    /// `NextChatTime`, else the default.
    pub fn chat_cooldown(&self, level: Level, profile: usize) -> f64 {
        self.chat
            .profiles
            .iter()
            .find(|p| p.level == level.key() && p.profile == profile)
            .map_or(self.chat.default_cooldown, |p| p.cooldown)
    }

    /// The item sets of `level`.
    pub fn sets(&self, level: Level) -> Vec<&ItemSet> {
        self.item_sets.iter().filter(|s| s.level == level.key()).collect()
    }
}

impl Default for BotData {
    fn default() -> Self {
        Self::built_in()
    }
}
