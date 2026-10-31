//! The effect and sound tables as data (data/effects.toml; per weapon,
//! the `combat_arms:effects` hook): ImpactFX, FireFX and TracerFX records,
//! attachments' effects, grenades' records, the event sounds and the
//! surface table (specs/combat_arms/effects_sounds.md, "How effects are
//! described as data").

use std::{collections::HashMap, sync::OnceLock};

use bevy::prelude::*;
use serde::Deserialize;

/// An ImpactFX record.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ImpactFx {
    /// A file, or `surface` (by surface and ammo kind).
    pub sound: String,
    pub sound_radius: f32,
    pub create_mark: bool,
    /// `surface`: the surface's bullet hole; empty: none.
    pub mark: String,
    pub mark_scale: f32,
    pub mark_color_ratio: Option<f32>,
    /// Also play the surface's impact (or grenade) group.
    pub surface_fx: bool,
    /// A group always played at the impact (empty: none).
    pub fx: String,
    /// The ammo does area or lasting damage: its own group shows even
    /// when the impact isn't seen (section 2).
    pub area: bool,
}

impl ImpactFx {
    pub fn color_ratio(&self) -> f32 {
        self.mark_color_ratio.unwrap_or(0.5)
    }
}

/// A FireFX record.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct FireFx {
    /// The group the shooter alone sees at their muzzle.
    pub fx: String,
    pub shells: bool,
    pub shell_model: String,
    pub shell_skin: String,
    pub shell_scale: f32,
    /// Non-zero: replaces the random casing velocity (u/s).
    pub velocity: [f32; 3],
}

/// A TracerFX record.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TracerFx {
    pub texture: String,
    /// Every F-th ray (0 and 1: every ray).
    pub frequency: u32,
    /// u/s, u, u.
    pub velocity: f32,
    pub width: f32,
    pub length: f32,
    /// 0..255.
    pub color: [f32; 3],
    pub initial_alpha: f32,
    pub final_alpha: f32,
}

impl Default for TracerFx {
    fn default() -> Self {
        Self {
            texture: String::new(),
            frequency: 0,
            velocity: 6000.0,
            width: 20.0,
            length: 300.0,
            color: [255.0; 3],
            initial_alpha: 1.0,
            final_alpha: 1.0,
        }
    }
}

/// What an attachment names (muzzle groups, tracer).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AttachmentFx {
    /// Name patterns (`*` matches any text).
    pub names: Vec<String>,
    pub pv_muzzle: Option<String>,
    pub hh_muzzle: Option<String>,
    pub tracer: Option<String>,
}

/// A grenade's records.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GrenadeFx {
    pub impact: String,
}

/// The event sounds (section 11).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EventSounds {
    pub ding: Vec<String>,
    pub flyby: Vec<String>,
    pub flyby_radii: [f32; 2],
    pub jump: Vec<String>,
    pub hurt_radius: f32,
    pub guns: Vec<String>,
    pub explosions: Vec<String>,
    pub burning: Vec<String>,
    pub falling: Vec<String>,
    pub gas: Vec<String>,
    pub head: Vec<String>,
    pub groin: Vec<String>,
    pub enemy_down: Vec<String>,
    pub enemy_down_radius: f32,
    /// The team's "grenade throwing" call-out (the auto-cast record
    /// `GrenadeThrowing`, grenades.md Corrections (2026-10-27) C1): the
    /// male and female templates' two files each, and its outer radius.
    pub grenade_throw: Vec<String>,
    pub grenade_throw_female: Vec<String>,
    pub grenade_throw_radius: f32,
    /// The local auto-cast entries (Corrections 2026-10-22, 8), played in
    /// your head: the kill voice (not on a groin kill), the sprint
    /// (`dash`) sound, the idle sound and the low-health heartbeat's.
    pub kill: Vec<String>,
    pub dash: Vec<String>,
    pub idle: Vec<String>,
    pub heartbeat: Vec<String>,
    /// The Awareness perk's own step sounds (Corrections 2026-10-22, 7).
    pub awareness: Vec<String>,
    /// The infection's sound (game_modes.md M8 names none: ours, the
    /// infected voice sets' file; `rungs::infection_moment`), the female
    /// body's, and its outer radius.
    pub infection: Vec<String>,
    pub infection_female: Vec<String>,
    pub infection_radius: f32,
}

/// One surface record.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Surface {
    pub id: u16,
    pub name: String,
    pub shows_mark: bool,
    /// The bullet hole's sprite (empty: none) and its scale range.
    pub mark: String,
    pub mark_scale: [f32; 2],
    pub impact_fx: String,
    pub uw_impact_fx: String,
    pub grenade_fx: String,
    pub uw_grenade_fx: String,
    /// Slots 1..4 (empty strings: undefined slots).
    pub bullet: Vec<String>,
    pub melee: Vec<String>,
    pub projectile: Vec<String>,
    pub shell: String,
    pub shell_radius: f32,
    pub grenade: String,
    pub grenade_radius: f32,
    /// Footsteps, slots 1 and 2.
    pub right: Vec<String>,
    pub left: Vec<String>,
    /// A character's (no decals, blood bursts).
    pub character: bool,
    pub sky: bool,
    pub liquid: bool,
    pub invisible: bool,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            shows_mark: true,
            mark: String::new(),
            mark_scale: [1.0, 1.0],
            impact_fx: String::new(),
            uw_impact_fx: String::new(),
            grenade_fx: String::new(),
            uw_grenade_fx: String::new(),
            bullet: Vec::new(),
            melee: Vec::new(),
            projectile: Vec::new(),
            shell: String::new(),
            shell_radius: 200.0,
            grenade: String::new(),
            grenade_radius: 1000.0,
            right: Vec::new(),
            left: Vec::new(),
            character: false,
            sky: false,
            liquid: false,
            invisible: false,
        }
    }
}

/// The tables (data/effects.toml).
#[derive(Clone, Debug, Default, Deserialize, Resource)]
#[serde(default, deny_unknown_fields)]
pub struct CaFxTables {
    pub impact: HashMap<String, ImpactFx>,
    pub fire: HashMap<String, FireFx>,
    pub tracer: HashMap<String, TracerFx>,
    pub attachment: Vec<AttachmentFx>,
    pub grenade: HashMap<String, GrenadeFx>,
    pub sounds: EventSounds,
    pub surface: Vec<Surface>,
}

impl CaFxTables {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The built-in tables (data/effects.toml).
    pub fn built_in() -> &'static Self {
        static SET: OnceLock<CaFxTables> = OnceLock::new();
        SET.get_or_init(|| {
            Self::parse(include_str!("../../../data/effects.toml"))
                .unwrap_or_else(|e| panic!("Combat Arms effects.toml: {e}"))
        })
    }

    /// The record of surface type `id`; a type without one uses Unknown's
    /// (id 0; section 5).
    pub fn surface(&self, id: u16) -> &Surface {
        static NONE: OnceLock<Surface> = OnceLock::new();
        self.surface
            .iter()
            .find(|s| s.id == id)
            .or_else(|| self.surface.iter().find(|s| s.id == 0))
            .unwrap_or_else(|| NONE.get_or_init(Surface::default))
    }

    /// The surface type named `name` (without regard to case).
    pub fn surface_id(&self, name: &str) -> Option<u16> {
        self.surface.iter().find(|s| s.name.eq_ignore_ascii_case(name)).map(|s| s.id)
    }

    /// An ImpactFX record by name (case kept as the data writes it, then
    /// without regard to case).
    pub fn impact(&self, name: &str) -> Option<&ImpactFx> {
        self.impact
            .get(name)
            .or_else(|| self.impact.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v))
    }

    pub fn fire(&self, name: &str) -> Option<&FireFx> {
        self.fire
            .get(name)
            .or_else(|| self.fire.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v))
    }

    pub fn tracer(&self, name: &str) -> Option<&TracerFx> {
        self.tracer
            .get(name)
            .or_else(|| self.tracer.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v))
    }

    /// The first attachment row whose patterns match `part`.
    pub fn attachment(&self, part: &str) -> Option<&AttachmentFx> {
        self.attachment
            .iter()
            .find(|a| a.names.iter().any(|p| glob(p, part)))
    }

    /// Every file the tables name (sounds and textures), for loading.
    pub fn sound_files(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for i in self.impact.values() {
            if i.sound != "surface" && !i.sound.is_empty() {
                out.push(&i.sound);
            }
        }
        let s = &self.sounds;
        for list in [
            &s.ding,
            &s.flyby,
            &s.jump,
            &s.guns,
            &s.explosions,
            &s.burning,
            &s.falling,
            &s.gas,
            &s.head,
            &s.groin,
            &s.enemy_down,
            &s.kill,
            &s.dash,
            &s.idle,
            &s.heartbeat,
            &s.awareness,
        ] {
            out.extend(list.iter().map(String::as_str));
        }
        for f in &self.surface {
            for list in [&f.bullet, &f.melee, &f.projectile, &f.right, &f.left] {
                out.extend(list.iter().map(String::as_str));
            }
            out.push(&f.shell);
            out.push(&f.grenade);
        }
        out.retain(|f| !f.is_empty());
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// `*`-pattern match without regard to case.
pub fn glob(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.to_ascii_lowercase(), text.to_ascii_lowercase());
    let parts: Vec<&str> = p.split('*').collect();
    if parts.len() == 1 {
        return p == t;
    }
    let mut rest = t.as_str();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            let Some(r) = rest.strip_prefix(part) else { return false };
            rest = r;
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            let Some(at) = rest.find(part) else { return false };
            rest = &rest[at + part.len()..];
        }
    }
    true
}

/// What kind of ammo a weapon fires (the impact sound list, section 11).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AmmoKind {
    #[default]
    Bullet,
    Melee,
    Thrown,
}

/// `combat_arms:effects`: a weapon's effects and sounds (its record's
/// sound fields and muzzle groups, its ammo's effect records).
#[derive(Component, Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CaWeaponFx {
    pub ammo_kind: AmmoKind,
    /// The ammo's ImpactFX, FireFX and TracerFX (empty: none).
    pub impact: String,
    pub fire: String,
    pub tracer: String,
    /// `HHMuzzleFXName` (the first person's is the view model's
    /// `muzzle_fx`).
    pub hh_muzzle: String,
    /// The third-person flash-hider groups (`HHFlashHiderFXName`): a muzzle
    /// part of `FlashHiderLevel` h shows entry h-1 to others when it names
    /// no third-person group of its own (Corrections 2026-10-22, 12).
    pub hh_flash_hider: Vec<String>,
    /// `FireSnd` and `FireSndRadius` (when the weapon has no attachment
    /// data: else the attachments' `fire_sound_radius`).
    pub fire_sound: String,
    pub fire_radius: f32,
    /// `Alt_FireSnd`: the attacker's own client plays it for a shot or
    /// swing started with the secondary button (empty: `fire_sound`;
    /// effects_sounds.md Corrections (2026-10-30) 2).
    pub alt_fire_sound: String,
    /// `SilencedFireSnd1..`.
    pub silenced_sounds: Vec<String>,
    pub dry_fire_sound: String,
    /// `ReloadSnd1..3`, `SelectSnd`, `DeselectSnd`, and the misc keys
    /// 10..14.
    pub reload_sounds: Vec<String>,
    pub select_sound: String,
    pub deselect_sound: String,
    pub misc_sounds: Vec<String>,
    pub reload_radius: f32,
    pub select_radius: f32,
    /// The install's record of `impact` (the ammo's `ImpactFXName` in its
    /// effect file), used when the tables have none of that name: the
    /// launchers' explosions.
    pub impact_record: Option<ImpactFx>,
    /// The install's record of `fire` (the ammo's `FireFXName`), used
    /// when the tables have none of that name: the launchers' own-muzzle
    /// groups (special_weapons.md Follow-up 7).
    pub fire_record: Option<FireFx>,
    /// A launched round (an impact projectile whose ammo `Type` isn't
    /// grenade's, 2): its blast's surface group is the surface's impact
    /// group, not its grenade group (special_weapons.md Follow-up 8).
    pub launched: bool,
}

impl Default for CaWeaponFx {
    fn default() -> Self {
        Self {
            ammo_kind: AmmoKind::Bullet,
            impact: String::new(),
            fire: String::new(),
            tracer: String::new(),
            hh_muzzle: String::new(),
            hh_flash_hider: Vec::new(),
            fire_sound: String::new(),
            fire_radius: 2000.0,
            alt_fire_sound: String::new(),
            silenced_sounds: Vec::new(),
            dry_fire_sound: String::new(),
            reload_sounds: Vec::new(),
            select_sound: String::new(),
            deselect_sound: String::new(),
            misc_sounds: Vec::new(),
            reload_radius: 300.0,
            select_radius: 300.0,
            impact_record: None,
            fire_record: None,
            launched: false,
        }
    }
}

impl CaWeaponFx {
    /// Its ImpactFX record: the tables' of its name, else the install's
    /// carried with it (`impact_record`).
    pub fn impact_of<'a>(&'a self, tables: &'a CaFxTables) -> Option<&'a ImpactFx> {
        if self.impact.is_empty() {
            return None;
        }
        tables.impact(&self.impact).or(self.impact_record.as_ref())
    }

    /// Its FireFX record: the tables' of its name, else the install's
    /// carried with it (`fire_record`).
    pub fn fire_of<'a>(&'a self, tables: &'a CaFxTables) -> Option<&'a FireFx> {
        if self.fire.is_empty() {
            return None;
        }
        tables.fire(&self.fire).or(self.fire_record.as_ref())
    }

    /// The files it names.
    pub fn sound_files(&self) -> impl Iterator<Item = &str> {
        [&self.fire_sound, &self.alt_fire_sound, &self.dry_fire_sound, &self.select_sound, &self.deselect_sound]
            .into_iter()
            .chain(&self.silenced_sounds)
            .chain(&self.reload_sounds)
            .chain(&self.misc_sounds)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    /// The sound of view-model `sound_key n` (1-3 reload, 4 select, 5
    /// deselect, 10-14 misc) and its radius.
    pub fn key_sound(&self, n: u32) -> Option<(&str, f32)> {
        let s = match n {
            1..=3 => self.reload_sounds.get(n as usize - 1),
            4 => Some(&self.select_sound),
            5 => Some(&self.deselect_sound),
            10..=14 => self.misc_sounds.get(n as usize - 10),
            _ => None,
        }?;
        // Misc keys set no radius of their own: the weapon default
        // (Corrections 2026-10-22, 4).
        let radius = match n {
            4 | 5 => self.select_radius,
            10..=14 => MISC_KEY_RADIUS,
            _ => self.reload_radius,
        };
        (!s.is_empty()).then_some((s.as_str(), radius))
    }
}

/// The radius (u) of a weapon's misc key sounds (10-14): the weapon
/// default.
pub const MISC_KEY_RADIUS: f32 = 2000.0;

/// The weapon data's hook name.
pub const EFFECTS: &str = "combat_arms:effects";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_parse() {
        let t = CaFxTables::built_in();
        assert_eq!(t.surface(20).name, "Metal");
        assert_eq!(t.surface(9999).id, 0, "unknown types use Unknown's record");
        assert_eq!(t.impact("BulletFmj").unwrap().sound_radius, 900.0);
        assert_eq!(t.tracer("AR02").unwrap().frequency, 4);
        assert!(glob("Silencer_*_Red", "Silencer_A_AR_Red"));
        assert!(!glob("Silencer_*_Red", "Silencer_A_AR"));
        assert_eq!(t.attachment("Silencer_C_AR").unwrap().pv_muzzle.as_deref(), Some("Silenced01_PV_Muzz"));
        assert!(!t.sound_files().is_empty());
    }
}
