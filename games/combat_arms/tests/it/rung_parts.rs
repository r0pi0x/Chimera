//! A gun game rung's fitted parts (`game::rungs`, docs/backlog.md
//! "Rung attachments may not fit their gun"): the draw takes only parts
//! the gun lists for a slot, in their own `Kinds` slot, never in a slot
//! its record closes (`AttachParams::slot_options` 0 or 2) or one with a
//! default part (it stays), keeps the stock part most of the time at the data's odds (sights most often,
//! power parts rarely, at most two: data/attachments/rung_odds.toml),
//! which a definition tunes (`rung_extras = { attachments = { ... } }`);
//! the rung line names the parts by display name, shortened and capped;
//! the match's asset offer names each rung gun's parts set
//! (`rungs::parts_set`) when its view model shows parts.

use chimera_sim::{
    minigame::{
        auto::Rng,
        ladder::{Ladder, RungExtras, RungOptions},
    },
    weapon::data::WeaponCatalog,
};
use combat_arms::game::{
    attachments::{ATTACHMENTS, AttachParams, BULLET, CaModTable, MAGAZINE, MECH_SIGHT, MUZZLE, SIGHT, SLOTS},
    characters::preload,
    data::CaData,
    rungs::{self, CaPartNames, LABEL_MAX, RungOdds},
};

fn m4a1() -> AttachParams {
    CaData::builtin().weapon_set().get("combat_arms:m4a1").unwrap().hook::<AttachParams>(ATTACHMENTS).unwrap()
}

/// Per slot, how often (of `n` seeds) a part is drawn there; and the most
/// parts one draw gave.
fn tally(params: &AttachParams, odds: &RungOdds, n: u64) -> ([f64; SLOTS], usize) {
    let table = CaModTable::built_in();
    let mut counts = [0usize; SLOTS];
    let mut most = 0;
    for seed in 0..n {
        let parts = rungs::draw_parts_with(table, params, odds, &mut Rng::new(seed));
        most = most.max(parts.len());
        let mut slots = Vec::new();
        for p in &parts {
            assert!(params.variant_of(p).is_some(), "{p} is in the M4A1's list");
            let slot = table.get(table.find(p).unwrap()).unwrap().slot().unwrap();
            assert!(!slots.contains(&slot), "one part per slot: {parts:?}");
            slots.push(slot);
            counts[slot] += 1;
        }
    }
    (counts.map(|c| c as f64 / n as f64), most)
}

#[test]
fn most_slots_keep_their_stock_part_and_power_parts_are_rare() {
    let params = m4a1();
    let odds = RungOdds::default();
    assert_eq!(odds, RungOdds::built_in());
    assert_eq!(odds.max_parts, 2);
    let (share, most) = tally(&params, &odds, 4000);
    eprintln!("non-stock share by slot (muzzle, sight, magazine, bullet, mech sight): {share:.3?}");
    let stock = 1.0 - share.iter().sum::<f64>() / SLOTS as f64;
    assert!((0.70..=0.85).contains(&stock), "stock share {stock}");
    assert!(most <= 2, "at most two non-stock parts ({most})");
    // Sights most often, then silencers, power parts least.
    assert!(share[SIGHT] > share[MUZZLE] && share[MUZZLE] > share[MAGAZINE], "{share:?}");
    for slot in [MAGAZINE, BULLET] {
        assert!(share[slot] < 0.13, "power parts rare: slot {slot} {}", share[slot]);
    }
    assert!(share[MECH_SIGHT] > share[MAGAZINE]);
}

#[test]
fn a_definition_tunes_the_odds() {
    let mut options = RungOptions::default();
    let table: toml::Table = toml::from_str("sight = 1\nmuzzle = 0\nmagazine = 0\nbullet = 0\nmech_sight = 0\nmax_parts = 1").unwrap();
    options.tables.insert(rungs::ATTACHMENTS_KEY.into(), table);
    let odds = RungOdds::of(&options);
    assert_eq!((odds.sight, odds.muzzle, odds.max_parts), (1.0, 0.0, 1));
    let (share, most) = tally(&m4a1(), &odds, 200);
    assert_eq!(share[SIGHT], 1.0, "always a sight");
    assert_eq!(most, 1);
    // A bad value: the built-in odds.
    let mut bad = RungOptions::default();
    bad.tables.insert(rungs::ATTACHMENTS_KEY.into(), toml::from_str("sight = \"often\"").unwrap());
    assert_eq!(RungOdds::of(&bad), RungOdds::default());
}

#[test]
fn slots_the_record_closes_take_no_part() {
    // The FMG-9's record: muzzle free, sight and magazine closed (0),
    // bullet and mechanical sight free; a sniper's built-in scope (2).
    let mut params = m4a1();
    params.slot_options = vec![1, 0, 2, 1, 1];
    let all = RungOdds {
        muzzle: 1.0,
        sight: 1.0,
        magazine: 1.0,
        bullet: 1.0,
        mech_sight: 1.0,
        max_parts: 5,
    };
    let (share, _) = tally(&params, &all, 100);
    assert_eq!(share, [1.0, 0.0, 0.0, 1.0, 1.0]);
    // A slot with a default part (a sniper's scope, an event weapon's
    // built-in parts) keeps it: nothing drawn there, the rest as before.
    let mut params = m4a1();
    params.defaults.sight = Some("DotSight_A_AR".into());
    params.defaults.magazine = Some("Magazine_A_AR".into());
    let table = CaModTable::built_in();
    assert!(rungs::slot_candidates(table, &params, SIGHT).is_empty());
    let (share, _) = tally(&params, &all, 100);
    assert_eq!(share, [1.0, 0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn the_rung_line_names_parts_by_display_name() {
    let names = CaPartNames::new([
        ("Silencer_A_SMG_Gold".to_string(), "S1 Suppressor I (SMG) ".to_string()),
        ("DotSight_Holosight_SMG_Gold".to_string(), "EOTech Holo Sight II (SMG)".to_string()),
    ]);
    let parts = ["Silencer_A_SMG_Gold".to_string(), "DotSight_Holosight_SMG_Gold".to_string()];
    assert_eq!(rungs::label(Some(&names), &parts), "S1 Suppressor I, EOTech Holo Sight II");
    // Without the install's strings: the record names made readable.
    assert_eq!(rungs::label(None, &parts), "Silencer A Gold, Holosight Gold");
    assert_eq!(rungs::readable("Bullet_PVP_Stun_I_SMG"), "Stun I bullet");
    assert_eq!(rungs::readable("MechSight_Aim_III_SMG"), "Aim III");
    assert_eq!(rungs::readable("DotSight_A_Pistol_Red"), "Red dot A Red");
    // Capped.
    let many: Vec<String> = (0..5).map(|_| "Bullet_PVP_Damage_III_AR".to_string()).collect();
    let l = rungs::label(None, &many);
    assert!(l.chars().count() <= LABEL_MAX && l.ends_with('…'), "{l}");
}

/// The offer's parts sets (`rungs::ladder_parts_sets`, what
/// `rungs::asset_names` names): one per Combat Arms rung gun whose view
/// model shows parts (drawn on its rung, or its own defaults), as
/// `parts/<id>` without the game, once each; none for a gun shown bare
/// or another game's.
#[test]
fn the_offer_names_each_rung_guns_parts_set() {
    let set = CaData::builtin().weapon_set();
    let catalog = WeaponCatalog { sets: vec![set] };
    let params = m4a1();
    // The built-in records name no models: one of the M4A1's muzzle parts
    // gets one (the install's have them).
    let mut table = CaModTable::built_in().clone();
    let part = rungs::slot_candidates(&table, &params, MUZZLE)[0].to_string();
    let i = table.find(&part).unwrap();
    let record = &mut table.records[usize::from(i)];
    record.socket = Some("Muzzle".into());
    record.attach_models = vec!["guns_m_pv_option/test.ltb".into(); 9];
    let table = &table;
    assert_ne!(preload::fitted_key(table, &params, "combat_arms:m4a1", &[&part]), "combat_arms:m4a1");
    // A gun shown bare: no part drawn, no default shown.
    let bare = |id: &str| {
        let p = catalog.get(id).unwrap().hook::<AttachParams>(ATTACHMENTS).unwrap();
        preload::fitted_key(table, &p, id, &[]) == id
    };
    assert!(bare("combat_arms:m4a1"));
    let other = set
        .weapons
        .iter()
        .find(|w| w.id != "combat_arms:m4a1" && w.hook::<AttachParams>(ATTACHMENTS).is_some() && bare(&w.id))
        .map(|w| w.id.as_str())
        .expect("a gun shown bare");
    let drawn = |parts: &str| {
        Some(RungExtras {
            game: "combat_arms".into(),
            values: vec![(rungs::PARTS.into(), parts.into())],
            ..Default::default()
        })
    };
    let mut ladder = Ladder {
        rungs: vec![other, "combat_arms:m4a1", "cs_source:weapon_ak47", "combat_arms:m4a1"],
        ..Default::default()
    };
    ladder.extras = vec![drawn(""), drawn(&part), drawn(&part), drawn(&part)];
    assert_eq!(rungs::parts_set("combat_arms:m4a1"), "parts/m4a1");
    assert_eq!(rungs::ladder_parts_sets(&catalog, table, &ladder), ["parts/m4a1"]);
    // Nothing drawn: the guns shown bare, nothing named.
    ladder.extras = vec![None; 4];
    assert!(rungs::ladder_parts_sets(&catalog, table, &ladder).is_empty());
}
