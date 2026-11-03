//! The install's weapon scripts and view models against our weapon data
//! (`game::scripts`, specs/hl2dm/weapons.md 1-2, 7): each weapon's script
//! (`scripts/weapon_<name>.txt`, plain text) agrees with its data file on
//! bucket, position, weight, clip, `default_clip`, ammo, damage, models,
//! name and sounds; each view model's draw sequence lasts the data's
//! `draw` (the switch time) and its reload the data's reload (the
//! shotgun's start, insert and finish its shell times). Skips without an
//! install.

use chimera_sim::weapon::data::WeaponData;
use hl2dm::game::{scripts, weapons};
use source_engine::content::anim;

use super::map_stock::install_mount;

/// The length of `activity`'s sequence in view model `path`, seconds:
/// (frames - 1) / fps of its first animation.
fn length(mount: &chimera_core::mount::Mount, path: &str, activity: &str) -> Option<f32> {
    let read = |p: &str| mount.read(&p.to_lowercase().replace('\\', "/")).ok();
    let set = anim::load(&read, path).ok()?;
    let s = set.pick_activity(activity, 0.0)?;
    let a = &set.animations[set.sequences[s].anims[0]];
    Some(if a.frames > 1 { (a.frames - 1) as f32 / a.fps } else { 0.0 })
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

#[test]
fn every_weapon_matches_its_script() {
    let Some(mount) = install_mount() else { return };
    let mut wrong = Vec::new();
    for data in &weapons::hl2_weapons().weapons {
        let script = scripts::read(&mount, &data.id).unwrap_or_else(|e| panic!("{}: {e}", data.id));
        wrong.extend(scripts::differences(data, &script));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn draw_and_reload_times_are_the_view_models() {
    let Some(mount) = install_mount() else { return };
    let mut wrong = Vec::new();
    let mut check = |data: &WeaponData, act: &str, ours: f32| {
        let view = data.models.view.as_deref().unwrap();
        match length(&mount, view, act) {
            Some(theirs) if near(ours, theirs) => {}
            other => wrong.push(format!("{} {act}: ours {ours}, the model's {other:?}", data.id)),
        }
    };
    for data in &weapons::hl2_weapons().weapons {
        check(data, "ACT_VM_DRAW", data.draw);
        let Some(m) = &data.magazine else { continue };
        match m.shells {
            Some(s) => {
                check(data, "ACT_SHOTGUN_RELOAD_START", s.start);
                check(data, "ACT_VM_RELOAD", s.insert);
                check(data, "ACT_SHOTGUN_RELOAD_FINISH", s.finish);
            }
            None => check(data, "ACT_VM_RELOAD", m.reload),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
