//! A Source weapon script (`scripts/weapon_<name>.txt`, KeyValues with
//! one `WeaponData` block): the keys every Source game's shared weapon
//! base reads (specs/hl2dm/weapons.md 1, source_deltas.md "Weapon
//! scripts"), with their defaults when missing: `default_clip` the clip
//! size, `autoswitchto` and `autoswitchfrom` 1, `damage` 0, `item_flags`
//! 8 ("limit in world"); clip sizes of -1 mean none. Only the plain text
//! scripts: an encrypted `.ctx` (CS:S's, and HL2:DM's beside its `.txt`)
//! is never read.

use source_formats::keyvalues::{self, Escapes, Kv};

use crate::mount::Mount;

/// A weapon script's keys.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponScript {
    /// `printname`: a string token (`#HL2_Pistol`).
    pub printname: String,
    pub viewmodel: String,
    pub playermodel: String,
    pub anim_prefix: String,
    pub bucket: u8,
    pub bucket_position: u8,
    /// None: no clip (`-1`).
    pub clip_size: Option<u32>,
    pub clip2_size: Option<u32>,
    /// Missing: the clip size.
    pub default_clip: Option<u32>,
    pub default_clip2: Option<u32>,
    /// None: `None`.
    pub primary_ammo: Option<String>,
    pub secondary_ammo: Option<String>,
    pub weight: i32,
    pub item_flags: u32,
    pub autoswitchto: bool,
    pub autoswitchfrom: bool,
    /// `damage` (HL2:DM's: a bullet's against every target).
    pub damage: i32,
    /// `SoundData`: kind (`single_shot`, `empty`, `reload`, ...) to entry.
    pub sounds: Vec<(String, String)>,
}

/// `item_flags` bits.
pub mod item_flags {
    pub const SELECT_ON_EMPTY: u32 = 1;
    pub const NO_AUTO_RELOAD: u32 = 2;
    pub const NO_AUTO_SWITCH_EMPTY: u32 = 4;
    pub const LIMIT_IN_WORLD: u32 = 8;
    pub const EXHAUSTIBLE: u32 = 16;
}

fn clip(v: Option<&str>) -> Option<u32> {
    v.and_then(|s| s.trim().parse::<i64>().ok()).filter(|n| *n >= 0).map(|n| n as u32)
}

fn ammo(v: Option<&str>) -> Option<String> {
    v.map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none")).map(str::to_string)
}

impl WeaponScript {
    /// The script in `text` (None: no `WeaponData` block).
    pub fn parse(text: &str) -> Option<WeaponScript> {
        let root = keyvalues::parse(text, Escapes::Off);
        let d: &Kv = root.get("WeaponData")?;
        let s = |k: &str| d.str(k).map(|v| v.trim().to_string()).unwrap_or_default();
        let int = |k: &str, default: i64| d.str(k).and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(default);
        let clip_size = clip(d.str("clip_size"));
        let clip2_size = clip(d.str("clip2_size"));
        Some(WeaponScript {
            printname: s("printname"),
            viewmodel: s("viewmodel").replace('\\', "/").to_lowercase(),
            playermodel: s("playermodel").replace('\\', "/").to_lowercase(),
            anim_prefix: s("anim_prefix"),
            bucket: int("bucket", 0) as u8,
            bucket_position: int("bucket_position", 0) as u8,
            clip_size,
            clip2_size,
            default_clip: match d.str("default_clip") {
                Some(v) => clip(Some(v)),
                None => clip_size,
            },
            default_clip2: match d.str("default_clip2") {
                Some(v) => clip(Some(v)),
                None => clip2_size,
            },
            primary_ammo: ammo(d.str("primary_ammo")),
            secondary_ammo: ammo(d.str("secondary_ammo")),
            weight: int("weight", 0) as i32,
            item_flags: int("item_flags", item_flags::LIMIT_IN_WORLD as i64) as u32,
            autoswitchto: int("autoswitchto", 1) != 0,
            autoswitchfrom: int("autoswitchfrom", 1) != 0,
            damage: int("damage", 0) as i32,
            sounds: d
                .get("SoundData")
                .map(|b| {
                    b.items()
                        .iter()
                        .filter_map(|(k, v)| Some((k.to_lowercase(), v.str_value()?.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// The `SoundData` entry of `kind` (`single_shot`).
    pub fn sound(&self, kind: &str) -> Option<&str> {
        self.sounds.iter().find(|(k, _)| k == kind).map(|(_, v)| v.as_str())
    }

    /// `scripts/<class>.txt` on `mount` (`weapon_pistol`).
    pub fn read(mount: &Mount, class: &str) -> Result<WeaponScript, String> {
        let path = format!("scripts/{}.txt", class.to_lowercase());
        let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
        WeaponScript::parse(&String::from_utf8_lossy(&bytes)).ok_or_else(|| format!("{path}: no WeaponData"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_keys_and_their_defaults() {
        let text = r##"// a script
WeaponData
{
	"printname"	"#HL2_Pistol"
	"viewmodel"			"models/weapons/v_pistol.mdl"
	"bucket"			"1"
	"bucket_position"	"0"
	"clip_size"			"18"
	"primary_ammo"		"Pistol"
	"secondary_ammo"	"None"
	"weight"			"2"
	"item_flags"			"0"
	"damage"			"8"
	SoundData
	{
		"single_shot"	"Weapon_Pistol.Single"
	}
}"##;
        let s = WeaponScript::parse(text).unwrap();
        assert_eq!((s.bucket, s.bucket_position, s.weight, s.damage, s.item_flags), (1, 0, 2, 8, 0));
        assert_eq!((s.clip_size, s.default_clip, s.clip2_size), (Some(18), Some(18), None));
        assert_eq!((s.primary_ammo.as_deref(), s.secondary_ammo.as_deref()), (Some("Pistol"), None));
        assert!(s.autoswitchto && s.autoswitchfrom);
        assert_eq!(s.sound("single_shot"), Some("Weapon_Pistol.Single"));
        let melee = WeaponScript::parse("WeaponData { \"clip_size\" \"-1\" }").unwrap();
        assert_eq!((melee.clip_size, melee.default_clip, melee.item_flags), (None, None, 8));
    }
}
