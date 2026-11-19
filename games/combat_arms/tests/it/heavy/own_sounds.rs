//! Every Combat Arms weapon plays its own fire sound, from the user's
//! install: for each weapon of the merged set (ours and the install's
//! copies) whose record names a `FireSnd`, the `combat_arms:effects`
//! table's `fire_sound` is that file. Prints how many do, and the first
//! few that don't. Also checks a few of ours the audit found wrong: the
//! M60's casing, the Remington 870's.
//!
//! Needs the V1 key (the weapons file) in chimera.local.toml. Skips with a
//! note without it. `--nocapture` for the counts.

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{
    GAME,
    data::CaData,
    effects::data::{CaWeaponFx, EFFECTS},
    model,
};

#[test]
fn every_weapon_plays_its_records_fire_sound() {
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's weapons file isn't readable (the V1 key)");
        return;
    }
    let (mut named, mut own, mut silent, mut other) = (0, 0, 0, Vec::new());
    for w in &ca.weapon_set().weapons {
        let Some(record) = ca.install_weapon(&w.id) else {
            continue;
        };
        let Some(fx) = w.hook::<CaWeaponFx>(EFFECTS) else {
            if !record.fire_snd.is_empty() {
                other.push(format!("{}: no effects table", record.name));
            }
            continue;
        };
        if record.fire_snd.is_empty() {
            silent += 1;
            continue;
        }
        named += 1;
        if fx.fire_sound == model::normalize(&record.fire_snd) {
            own += 1;
        } else {
            other.push(format!(
                "{}: plays {:?}, record {:?}",
                record.name, fx.fire_sound, record.fire_snd
            ));
        }
    }
    println!(
        "{own} of {named} weapons naming a FireSnd play it ({silent} name none); {} don't",
        other.len()
    );
    for o in other.iter().take(20) {
        println!("  {o}");
    }
    assert!(
        other.is_empty(),
        "{} weapons play another file than their record's",
        other.len()
    );

    // Ours, from the install: the shells the audit found wrong.
    let fire = |id: &str| w_fx(&ca, id).fire;
    assert!(fire("combat_arms:m60").eq_ignore_ascii_case("5.56mm"));
    assert!(fire("combat_arms:remington_870").eq_ignore_ascii_case("12Gauge_R"));
}

fn w_fx(ca: &CaData, id: &str) -> CaWeaponFx {
    ca.weapon_set()
        .get(id)
        .and_then(|w| w.hook(EFFECTS))
        .unwrap_or_else(|| panic!("{id} has no effects"))
}
