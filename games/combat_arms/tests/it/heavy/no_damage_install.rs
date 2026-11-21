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
    minigame::{
        auto::{self, Capabilities},
        power::PowerReaders,
    },
    weapon::{data::WeaponCatalog, harm::can_damage_player},
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
    let readers = &w.resource::<PowerReaders>().harm;
    let harm = |id: &str| can_damage_player(catalog.get(id).unwrap_or_else(|| panic!("{id}")), readers);
    for id in [
        "combat_arms:rms12",
        "combat_arms:m136_at4",
        "combat_arms:optical_buster_apollo",
        "combat_arms:m4devgru",
    ] {
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
            assert_ne!(
                can_damage_player(catalog.get(id).unwrap(), readers),
                Some(false),
                "{id} left in"
            );
        }
    }
}

/// The install's weapon records no shop item sells (NPC, boss, turret and
/// test records) are never dealt by a drawn ladder, nor listed by the
/// Host's Guns picker (`auto::drawable_all`). `--nocapture` prints them.
#[test]
fn the_installs_guns_without_a_shop_item_stay_off_drawn_ladders() {
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    let Some(attributes) = ca.attributes().filter(|a| a.weapons.is_some() && a.shop_weapons.is_some()) else {
        eprintln!("skipping: the install's weapon and item files aren't readable (the V1 key)");
        return;
    };
    let unshopped = attributes.unshopped_weapons();
    eprintln!("{} weapon records have no shop item:\n{}", unshopped.len(), unshopped.join("\n"));
    assert!(!unshopped.is_empty(), "the install has NPC and test records");
    let sim = Sim::with(CaSimWith(ca.clone()), |_| {});
    let w = sim.app.world();
    let catalog = w.resource::<WeaponCatalog>();
    // Hands, emotes and the like are records, not guns of the catalogue.
    let unshopped: Vec<String> = unshopped.into_iter().filter(|id| catalog.get(id).is_some()).collect();
    for mut caps in [vec![Capabilities::of(&PARTS, catalog, None)]] {
        let listed: Vec<&str> = caps[0].guns.iter().map(|g| g.id).chain(caps[0].grenades.iter().copied()).collect();
        let skipped = auto::drawable_all(w, &mut caps);
        // Each one the ladder could have dealt is skipped, with the reason.
        let mut hit = 0;
        for id in unshopped.iter().filter(|u| listed.iter().any(|l| l == *u)) {
            assert!(skipped.iter().any(|s| s.id == id), "{id} skipped");
            hit += 1;
        }
        assert!(hit > 20, "the shop rule skipped only {hit}");
        eprintln!("{hit} unshopped records were drawable");
        // The explicit non-player entries (data/ladder_exclusions.toml) too.
        for id in ["combat_arms:test_gun_mg42", "combat_arms:m4a1-nsm", "combat_arms:mp5a4_npc", "combat_arms:powered_suit_boss_mg"] {
            assert!(skipped.iter().any(|s| s.id == id), "{id} skipped by an entry");
        }
        // The borderline ones are dealt (the user, 2026-11-21): they are
        // in the picker's list unless the install lacks them.
        for id in ["combat_arms:hg2_auto_turret", "combat_arms:minimi_para_camo_infecteds_super", "combat_arms:g36e_ugly"] {
            assert!(skipped.iter().all(|s| s.id != id), "{id} dealt");
        }
        // The sandbox's list loses them as well, but keeps a bad skin.
        let mut sandbox = vec![Capabilities::of(&PARTS, catalog, None)];
        auto::drop_not_players(w, &mut sandbox);
        let has = |id: &str| sandbox[0].guns.iter().any(|g| g.id == id);
        assert!(!has("combat_arms:test_gun_mg42") && !has("combat_arms:mp5a4_npc") && !has("combat_arms:abnormal_boss_minigun"));
        assert!(has("combat_arms:asw338lmspunk") && has("combat_arms:m4a1"), "the sandbox keeps real guns");
        let left: Vec<&str> = caps[0].guns.iter().map(|g| g.id).chain(caps[0].grenades.iter().copied()).collect();
        assert!(left.iter().all(|id| !unshopped.iter().any(|u| u == id)), "an unshopped gun is left in");
        eprintln!("{} of {} drawable after the shop rule", left.len(), listed.len());
    }
}
