//! The install's guns that a drawn gun game ladder leaves out
//! (`chimera_sim::minigame::auto::drawable`; the user's decision,
//! 2026-11-12): those whose data does no damage (`weapon::harm`: the
//! RMS12's burn and the M136_AT4's blast are 0) and those
//! data/ladder_exclusions.toml names. (The M136_AT4 and the M32_Stun are
//! our documents without a class, never in a drawn ladder anyway.) Needs
//! the V1 key; skips with a note without. `--nocapture` prints the list by
//! reason.

use chimera_core::mount::config::LocalConfig;
use chimera_sim::{
    minigame::auto::{self, Capabilities},
    weapon::{
        data::WeaponCatalog,
        harm::{HarmReaders, can_damage_player},
    },
};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{GAME, data::CaData, parts::PARTS},
    harness::CaSimWith,
};

#[test]
fn the_installs_guns_without_damage_stay_off_drawn_ladders() {
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let sim = Sim::with(CaSimWith(ca), |_| {});
    let w = sim.app.world();
    let catalog = w.resource::<WeaponCatalog>();
    let readers = w.resource::<HarmReaders>();
    let harm = |id: &str| can_damage_player(catalog.get(id).unwrap_or_else(|| panic!("{id}")), readers);
    for id in ["combat_arms:rms12", "combat_arms:m136_at4", "combat_arms:optical_buster_apollo", "combat_arms:m4devgru"] {
        assert_eq!(harm(id), Some(false), "{id}");
    }
    for id in [
        "combat_arms:flamethrower",
        "combat_arms:law",
        "combat_arms:m32",
        "combat_arms:m4a1",
        "combat_arms:rms12_bounty",
        "combat_arms:m136_at4_bounty",
    ] {
        assert_eq!(harm(id), Some(true), "{id}");
    }
    let mut caps = vec![Capabilities::of(&PARTS, catalog, None)];
    let before = caps[0].guns.len() + caps[0].grenades.len();
    let skipped = auto::drawable(w, &mut caps);
    eprintln!("{} of {before}\n{}", skipped.len(), auto::skipped_lines(&skipped));
    let ids: Vec<&str> = skipped.iter().map(|s| s.id).collect();
    for id in [
        "combat_arms:rms12",
        "combat_arms:optical_buster_apollo",
        "combat_arms:m4devgru",
        "combat_arms:infecteds_stunup",
        "combat_arms:asw338lmspunk",
        "combat_arms:auga3championslighting",
    ] {
        assert!(ids.contains(&id), "{id} skipped");
    }
    assert!(!ids.contains(&"combat_arms:rms12_bounty"), "its burn is 30 HP/s");
    for c in &caps {
        for id in c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()) {
            assert_ne!(can_damage_player(catalog.get(id).unwrap(), readers), Some(false), "{id} left in");
        }
    }
}
