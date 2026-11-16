//! The install's encrypted scripts with the user's key (specs/tf2/ice.md
//! case 18): every `scripts/tf_weapon_*.ctx` and
//! `scripts/playerclasses/*.ctx` decrypts into KeyValues with a block at
//! its top, and encrypting it again gives the file byte for byte; and
//! which levels decrypt the Rocket Launcher's script. The key comes from
//! `CHIMERA_TF2_CTX_KEY` or `[games.tf2.keys.ctx]` at run time; each test passes with a note
//! without it or without the install. Decrypted text is never written
//! anywhere and never printed.

use chimera_core::mount::Mount;
use tf2::game::{
    ice::{self, CtxCipher},
    scripts::{self, ScriptSource},
};

use super::maps::install_mount;

fn key_and_mount() -> Option<(scripts::CtxKey, Mount)> {
    let key = match scripts::key() {
        Ok(Some(k)) => k,
        Ok(None) => {
            eprintln!("skipping: no .ctx key ({} or [games.tf2.keys.ctx])", scripts::KEY_ENV);
            return None;
        }
        Err(e) => panic!("{e}"),
    };
    Some((key, install_mount()?))
}

/// The scripts the test reads: the weapons' and the classes'.
fn encrypted_scripts(mount: &Mount) -> Vec<String> {
    let weapons = mount.files_in("scripts").into_iter().filter(|p| {
        p.rsplit('/').next().is_some_and(|f| f.starts_with("tf_weapon_")) && p.ends_with(".ctx")
    });
    let classes = mount.files_in("scripts/playerclasses").into_iter().filter(|p| p.ends_with(".ctx"));
    weapons.chain(classes).collect()
}

#[test]
fn every_weapon_and_class_script_decrypts_to_keyvalues() {
    let Some((key, mount)) = key_and_mount() else { return };
    let cipher = CtxCipher::configured();
    let ice = cipher.ice(&key.0);
    let files = encrypted_scripts(&mount);
    assert!(files.len() > 20, "{} scripts", files.len());
    let mut failed = Vec::new();
    for path in &files {
        let bytes = mount.read(path).unwrap();
        let name = path.trim_start_matches("scripts/").trim_end_matches(".ctx");
        let s = scripts::from_bytes(name, &bytes, Some(&key), cipher);
        if s.source != ScriptSource::Decrypted || s.data().is_none() {
            failed.push(path.clone());
            continue;
        }
        assert_eq!(ice::encrypt_ctx(&ice, &ice::decrypt_ctx(&ice, &bytes)), bytes, "{path}: round trip");
    }
    eprintln!("{} scripts, cipher {}: {} failed", files.len(), cipher.name(), failed.len());
    assert!(failed.is_empty(), "with cipher {} (try `dump tf2 --ice-probe`): {failed:?}", cipher.name());
}

#[test]
fn the_probe_names_the_cipher_of_the_rocket_launchers_script() {
    let Some((key, mount)) = key_and_mount() else { return };
    let bytes = mount.read("scripts/tf_weapon_rocketlauncher.ctx").expect("the Rocket Launcher's script");
    let found = ice::probe_ctx(&key.0, &bytes);
    eprintln!("ciphers that decrypt it: {:?}", found.iter().map(CtxCipher::name).collect::<Vec<_>>());
    assert!(!found.is_empty(), "no level decrypts it with the key");
    assert!(
        found.contains(&CtxCipher::configured()),
        "set {} or CtxCipher::DEFAULT to one of them",
        ice::LEVEL_ENV
    );
}

/// Every gun's numbers come from its script (specs/tf2/weapon_scripts.md),
/// the documents they make load as weapon data, and the mapped numbers
/// agree with the committed data's (the specs' public numbers) except
/// where the spec names a difference: the SMG's, Minigun's and Syringe
/// Gun's interval (not tick-rounded; equal under `set` timing), the view
/// kick (`PunchAngle`, non-zero on the Scattergun, the Shotgun and the
/// three launchers; the data's 0 is the wiki's "no recoil"). The
/// bullets' `Range` isn't compared (the data's is the wiki's reach). No
/// number is printed.
#[test]
fn every_guns_numbers_come_from_its_script_and_agree_with_the_data() {
    use tf2::game::weapons::{self as w, script_numbers::GunNumbers};
    let Some((key, mount)) = key_and_mount() else { return };
    let numbers = GunNumbers::from_mount(Some(&mount), Some(&key));
    let not: Vec<&str> = numbers.sources.iter().filter(|s| !s.from_script).map(|s| s.gun.as_str()).collect();
    assert!(not.is_empty(), "spec fallback for {not:?}");
    let differ = numbers.differences().unwrap_or_else(|e| panic!("the mapped documents: {e}"));
    let known = [
        format!("{} cycle", w::SMG),
        format!("{} cycle", w::MINIGUN),
        format!("{} cycle", w::SYRINGE_GUN),
        format!("{} punch", w::SCATTERGUN),
        format!("{} punch", w::SHOTGUN),
        format!("{} punch", w::ROCKET_LAUNCHER),
        format!("{} punch", w::GRENADE_LAUNCHER),
        format!("{} punch", w::STICKY_LAUNCHER),
    ];
    let new: Vec<&String> = differ.iter().filter(|d| !known.contains(d)).collect();
    assert!(new.is_empty(), "the scripts differ from the data beyond the spec's known cases: {new:?} (all: {differ:?})");
}
