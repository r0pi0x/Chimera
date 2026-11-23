//! The item converter on the user's install (specs/tf2/item_attributes.md
//! 5, its measured numbers): the schema read through the install's mount,
//! the item classes' scripts decrypted with the user's key when it is set
//! (never printed), the pack made in memory (`items::generated`), then
//! every batch-1 item of data/items.toml found with its attributes'
//! effective numbers as measured in the game, and the stock weapons with
//! every item weapon built over them. Without the install it passes with a
//! note; without the key the scripts' base numbers are left out (noted).

use tf2::game::{
    items::{self, ItemPack, ItemsData},
    mount, scripts,
    weapons::script_numbers::GunNumbers,
};

use super::maps::install;

#[test]
fn the_install_converts_into_an_item_pack_with_the_spec_numbers() {
    let Some(install) = install() else { return };
    let key = scripts::key().expect("the key's config");
    if key.is_none() {
        eprintln!("note: no .ctx key ({}): the item classes' scripts aren't read", scripts::KEY_ENV);
    }
    let plain = match mount::open_install(&install) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let source = scripts::decrypting(plain, key.clone());
    let sets = items::generated(&source, items::SET);
    assert_eq!(sets.len(), 1, "one set");
    let (set, files) = &sets[0];
    assert_eq!(set, items::SET);
    assert_eq!(files[0].0, items::PACK_PATH);
    let pack = ItemPack::parse(std::str::from_utf8(&files[0].1).unwrap()).unwrap();
    // Deterministic: a second conversion gives the same bytes.
    assert_eq!(items::generated(&source, items::SET)[0].1[0].1, files[0].1);
    assert!(pack.items.len() > 100, "{} weapon items", pack.items.len());

    let data = ItemsData::builtin();
    for (name, def) in &data.items {
        let r = pack.get(def.index).unwrap_or_else(|| panic!("{name} ({}) not in the pack", def.index));
        assert!(def.wearable || data.bases.contains_key(&r.class), "{name}: no base for {}", r.class);
    }
    let r = |name: &str| pack.get(data.items[name].index).unwrap();
    let a = |name: &str, class: &str| r(name).attr(class, 1.0);
    let close = |x: f64, y: f64| (x - y).abs() < 1e-3;
    // The attributes, folded (§5).
    assert!(close(a("winger", "mult_clipsize"), 0.4));
    assert!(close(a("winger", "mod_jump_height_from_weapon"), 1.25));
    assert!(close(r("rocket_jumper").attr("mult_dmg", 1.0), 0.0));
    assert!(close(a("brass_beast", "mult_player_aiming_movespeed"), 0.4));
    assert!(close(a("scotsmans_skullcutter", "mult_player_movespeed"), 0.85));
    assert!(r("scotsmans_skullcutter").flag("is_a_sword"));
    assert!(close(a("warriors_spirit", "mult_dmgtaken"), 1.3));
    assert!(close(r("bread_bite").attr("mod_maxhealth_drain_rate", 0.0), 10.0));
    // The effective numbers measured in the game (4.1-4.6).
    let e = |name: &str, k: &str| *r(name).effective.get(k).unwrap_or_else(|| panic!("{name} {k}: {:?}", r(name).effective));
    assert!(close(e("liberty_launcher", "projectile_speed"), 1540.0));
    assert!(close(e("direct_hit", "projectile_speed"), 1980.0));
    assert!(close(e("direct_hit", "blast_radius"), 43.8));
    assert!(close(e("iron_bomber", "fuse"), 1.4));
    assert!(close(e("quickiebomb_launcher", "charge"), 1.2));
    assert!(close(e("quickiebomb_launcher", "arm"), 0.6));
    assert!(close(e("sticky_jumper", "max_stickies"), 2.0));
    assert!(close(e("brass_beast", "spinup"), 1.125));
    assert!(close(e("tomislav", "spinup"), 0.6));
    assert!(close(e("brass_beast", "spun_speed"), 44.0));
    assert!(close(e("winger", "jump"), 361.25));
    if key.is_some() {
        for (name, clip) in [("winger", 5.0), ("pretty_boys_pocket_pistol", 9.0), ("liberty_launcher", 5.0), ("loch_n_load", 3.0), ("quickiebomb_launcher", 4.0), ("family_business", 8.0)] {
            assert_eq!(e(name, "clip"), clip, "{name}");
        }
    }

    // Every item weapon builds over the stock documents and parses.
    let numbers = GunNumbers::builtin().with_pack(&pack);
    let weapons = data.items.values().filter(|d| !d.wearable).count();
    assert_eq!(numbers.items.len(), weapons, "{:?}", numbers.sources.iter().filter(|s| s.gun == "items").collect::<Vec<_>>());
    numbers.parse().unwrap();
}
