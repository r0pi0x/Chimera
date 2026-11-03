//! HL2:DM's weapon scripts in the install (`scripts/weapon_<name>.txt`,
//! plain text beside the encrypted `.ctx`: specs/hl2dm/source_deltas.md
//! "Weapon scripts"; read through `source_engine::weapon_script`), against
//! our weapon data (`game::weapons`, data/weapons): `read` reads a weapon's
//! script, `differences` names each key the data disagrees on (the bucket
//! as the slot, its position, weight, clip and `default_clip`, ammo type,
//! damage (a weapon without a magazine counts its rounds itself, in its
//! projectile hook: where the script names its ammo or clip, those are
//! compared with the hook's; a clip of 0 with no ammo, the gravity gun's,
//! is no clip), models, the HUD's name and the fire, empty and double-shot
//! sounds). The data holds the spec's numbers so the game plays without
//! the install's files; tests/it/heavy/scripts.rs checks they still match
//! the install. Nothing is read from the scripts at run time.

use chimera_sim::weapon::data::WeaponData;
use source_engine::weapon_script::WeaponScript;

use crate::mount::Mount;

/// The script class of weapon `id` (`hl2dm:weapon_pistol`: `weapon_pistol`).
pub fn class_of(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

/// The install's script of weapon `id`.
pub fn read(mount: &Mount, id: &str) -> Result<WeaponScript, String> {
    WeaponScript::read(mount, class_of(id))
}

/// Our ammo id of a script's ammo name (`Pistol`: `hl2dm:pistol`).
pub fn ammo_id(name: &str) -> String {
    format!("{}:{}", super::GAME, name.to_lowercase())
}

/// The ammo and count of the weapon's own rounds (`launch::Hl2Rounds`,
/// named by its projectile hook): the rockets, grenades and S.L.A.M.s
/// that the data holds beside, not in, a magazine.
fn own_rounds(data: &WeaponData) -> Option<(String, u32)> {
    data.hooks.values().find_map(|h| {
        let ammo = h.get("ammo")?.as_str()?.to_string();
        let count = h.get("count")?.as_integer()? as u32;
        Some((ammo, count))
    })
}

/// Where `data` disagrees with `script`, one line each.
pub fn differences(data: &WeaponData, script: &WeaponScript) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |what: &str, ours: String, theirs: String| {
        if ours != theirs {
            out.push(format!("{}: {what}: ours {ours}, the script's {theirs}", data.id));
        }
    };
    check("bucket", data.slot.to_string(), script.bucket.to_string());
    let position = data
        .hooks
        .get(super::weapons::INVENTORY)
        .and_then(|h| h.get("position"))
        .and_then(|p| p.as_integer());
    check("bucket_position", format!("{position:?}"), format!("{:?}", Some(script.bucket_position as i64)));
    check("weight", format!("{:?}", data.weight), format!("{:?}", Some(script.weight)));
    // A weapon without a magazine counts its rounds itself (`own_rounds`):
    // where the script names a clip or ammo for it (the RPG's rocket, the
    // frag's grenade, the S.L.A.M.'s three), those are what it carries.
    let own = own_rounds(data).filter(|_| data.magazine.is_none());
    let own_ammo = own.as_ref().map(|(a, _)| a.clone());
    // A clip of 0 with no ammo (the gravity gun's script) is no clip.
    let no_clip = script.clip_size == Some(0) && script.primary_ammo.is_none();
    let (script_clip, script_default) =
        if no_clip { (None, None) } else { (script.clip_size, script.default_clip) };
    let own_count = own.as_ref().map(|(_, c)| *c);
    check(
        "clip_size",
        format!("{:?}", data.magazine.as_ref().map(|m| m.clip).or(own_count.filter(|_| script_clip.is_some()))),
        format!("{script_clip:?}"),
    );
    let default_clip = data
        .hooks
        .get(super::weapons::INVENTORY)
        .and_then(|h| h.get("default_clip"))
        .and_then(|p| p.as_integer())
        .map(|n| n as u32)
        .or(data.magazine.as_ref().map(|m| m.clip))
        .or(own_count.filter(|_| script_default.is_some()));
    check("default_clip", format!("{default_clip:?}"), format!("{script_default:?}"));
    check(
        "primary_ammo",
        format!("{:?}", data.magazine.as_ref().map(|m| m.ammo.clone()).or(own_ammo.filter(|_| script.primary_ammo.is_some()))),
        format!("{:?}", script.primary_ammo.as_deref().map(ammo_id)),
    );
    if let Some(d) = &data.damage {
        check("damage", format!("{}", d.amount as i32), script.damage.to_string());
    }
    check("viewmodel", format!("{:?}", data.models.view), format!("{:?}", Some(&script.viewmodel)));
    check("playermodel", format!("{:?}", data.models.world), format!("{:?}", Some(&script.playermodel)));
    check("printname", format!("{:?}", data.hud.name), format!("{:?}", Some(&script.printname)));
    if let Some(s) = &data.sounds {
        if data.melee.is_none() {
            check("single_shot", format!("{:?}", s.fire), format!("{:?}", script.sound("single_shot")));
        }
        if s.empty.is_some() {
            check("empty", format!("{:?}", s.empty), format!("{:?}", script.sound("empty")));
        }
        if s.fire_alt.is_some() {
            check("double_shot", format!("{:?}", s.fire_alt), format!("{:?}", script.sound("double_shot")));
        }
    }
    if let Some(m) = &data.melee {
        // A swing's sound, hit or miss: the miss's.
        let swing = m.primary.sound_miss.as_deref();
        check("single_shot", format!("{swing:?}"), format!("{:?}", script.sound("single_shot")));
    }
    out
}
