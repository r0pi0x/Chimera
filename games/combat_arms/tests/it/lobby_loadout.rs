//! specs/combat_arms/lobby.md's loadout and item cases (48 to 57, E and
//! P): a new player's sets, what may go where, the 1st Arms check at READY
//! and what the game server keeps of the loadout, item lifetimes and the
//! fallbacks when an item expires; and how a set becomes the gear's
//! `Selection` and `CaLoadout`.

use std::sync::Arc;

use combat_arms::game::lobby::{
    data::LobbyData,
    items::{Catalog, Consume, Kind, Owned, expires_at},
    loadout::{self, Loadout, SetKind, Slot, text},
    room::*,
    service::Lobby,
};

fn catalog() -> Arc<Catalog> {
    Catalog::built_in()
}

fn data() -> Arc<LobbyData> {
    LobbyData::built_in()
}

fn fresh() -> Loadout {
    Loadout::new_player(&catalog())
}

fn equip(l: &mut Loadout, slot: Slot, item: i32) -> Result<(), &'static str> {
    l.equip(&catalog(), &Owned::all(), SetKind::Normal, slot, item)
}

const M16A3: i32 = 20;
const M92FS: i32 = 24;
const M9: i32 = 0;
const M67: i32 = 7;
const DESERT_EAGLE: i32 = 920;
const BOWIE: i32 = 901;
const M4A1: i32 = 910;
const AK47: i32 = 911;
const MP5A4: i32 = 912;
const L96A1: i32 = 915;

#[test]
fn case_48_a_new_players_normal_set() {
    let c = catalog();
    let l = fresh();
    let s = &l.normal;
    assert_eq!(c.get(s.primary).unwrap().name, "M16A3");
    assert_eq!(c.get(s.secondary).unwrap().name, "Beretta M92FS");
    assert_eq!(c.get(s.melee).unwrap().name, "M9");
    assert_eq!(c.get(s.support).unwrap().name, "M67");
    assert_eq!((s.primary, s.secondary, s.melee, s.support), (20, 24, 0, 7));
    assert_eq!(s.head, Some(4007), "Black Cap");
    assert_eq!(s.backpack, Some(7001), "Standard Backpack");
    assert_eq!(s.usable_bag_slots(&c), 1, "backpack A usable, B to D not");
    let mut l = l;
    assert_eq!(equip(&mut l, Slot::Backpack(0), DESERT_EAGLE), Ok(()));
    for i in 1..4 {
        assert_eq!(equip(&mut l, Slot::Backpack(i), AK47), Err(text::NO_SLOT), "slot {i}");
    }
    // The Pure Combat set has no backpack (open question 9).
    assert_eq!(l.pure_combat.backpack, None);
    assert_eq!(l.pure_combat.usable_bag_slots(&c), 0);
}

#[test]
fn case_49_a_weapon_slot_can_never_be_emptied() {
    let c = catalog();
    let mut l = fresh();
    for slot in [Slot::Primary, Slot::Secondary, Slot::Melee, Slot::Support] {
        assert_eq!(l.release(&c, SetKind::Normal, slot), Err(text::NEVER_EMPTY));
    }
    assert_eq!(l.normal.primary, M16A3);
    // A base item is not released; gear that is not base is.
    assert_eq!(l.release(&c, SetKind::Normal, Slot::Head), Err(text::BASE_ITEM));
    equip(&mut l, Slot::Face, 5000).unwrap();
    assert_eq!(l.release(&c, SetKind::Normal, Slot::Face), Ok(()));
    assert_eq!(l.normal.face, None);
    // Releasing the backpack releases the backpack-slot weapons.
    equip(&mut l, Slot::Backpack(0), DESERT_EAGLE).unwrap();
    assert_eq!(l.release(&c, SetKind::Normal, Slot::BackpackGear), Ok(()));
    assert_eq!(l.normal.bag, [None; 4]);
    assert_eq!(l.normal.usable_bag_slots(&c), 0);
}

#[test]
fn case_50_what_may_go_in_a_backpack_slot_and_no_duplicates() {
    let mut l = fresh();
    assert_eq!(
        equip(&mut l, Slot::Backpack(0), BOWIE),
        Err(text::WRONG_SLOT),
        "no knives in the backpack"
    );
    assert_eq!(equip(&mut l, Slot::Backpack(0), DESERT_EAGLE), Ok(()));
    assert_eq!(
        equip(&mut l, Slot::Secondary, DESERT_EAGLE),
        Err("You cannot equip two of the same item at the same time.")
    );
    assert_eq!(l.normal.secondary, M92FS);
    // Special items go only in the backpack; primaries only in the primary.
    assert_eq!(equip(&mut l, Slot::Primary, 932), Err(text::WRONG_SLOT));
    assert_eq!(equip(&mut l, Slot::Secondary, M4A1), Err(text::WRONG_SLOT));
    assert_eq!(equip(&mut l, Slot::Melee, M4A1), Err(text::WRONG_SLOT));
    assert_eq!(equip(&mut l, Slot::Primary, 914), Ok(()), "a primary shotgun");
    assert_eq!(
        equip(&mut l, Slot::Secondary, 921),
        Ok(()),
        "type 5 goes in the secondary"
    );
}

#[test]
fn an_items_cant_with_list_refuses_the_pair() {
    let mut c = (*catalog()).clone();
    c.items.get_mut(&M4A1).unwrap().cant_with = vec![AK47];
    let owned = Owned::all();
    let mut l = fresh();
    l.equip(&c, &owned, SetKind::Normal, Slot::Primary, M4A1).unwrap();
    l.equip(&c, &owned, SetKind::Normal, Slot::Backpack(0), DESERT_EAGLE)
        .unwrap();
    // The AK-47 cannot go with the M4A1 (it replaces the M16A3 only when
    // the M4A1 is not in the way): the primary slot holds the M4A1, so
    // swapping is fine, but the backpack cannot hold the AK-47 beside it.
    let r = l.equip(&c, &owned, SetKind::Normal, Slot::Backpack(0), AK47);
    assert_eq!(r, Err(text::CANT_TOGETHER));
    assert_eq!(
        text::CANT_TOGETHER,
        "There is an item already equipped that can't be equipped together. Cannot equip this item."
    );
}

#[test]
fn only_owned_items_equip_and_the_pure_combat_set_has_a_list() {
    let c = catalog();
    let owned = Owned::base_only(&c);
    let mut l = fresh();
    assert_eq!(
        l.equip(&c, &owned, SetKind::Normal, Slot::Primary, M4A1),
        Err(text::NOT_OWNED),
        "an account without the M4A1"
    );
    let mut owned = owned;
    owned.grant(M4A1, None);
    assert_eq!(l.equip(&c, &owned, SetKind::Normal, Slot::Primary, M4A1), Ok(()));
    // Grade 3 weapons are off the Pure Combat list.
    assert_eq!(
        l.equip(&c, &Owned::all(), SetKind::PureCombat, Slot::Primary, L96A1),
        Err(text::NOT_PURE)
    );
}

fn ready(arms: [u8; 2], l: &Loadout) -> Result<(), String> {
    loadout::ready_check(&data(), &catalog(), &l.normal, arms)
}

fn kit(arms: [u8; 2], l: &Loadout) -> Vec<i32> {
    loadout::match_weapons(&data(), &catalog(), &l.normal, arms)
}

#[test]
fn case_51_pistols_only() {
    let l = fresh();
    assert_eq!(ready([1, 0], &l), Ok(()), "a pistol is equipped");
    assert_eq!(kit([1, 0], &l), [M92FS, M9], "the pistol and the knife");
}

#[test]
fn case_52_sniper_only_with_the_base_loadout_cannot_ready() {
    let l = fresh();
    let e = ready([6, 0], &l).unwrap_err();
    assert!(e.contains("sniper rifle") && e.contains("Sniper Only"), "{e}");
    // With a sniper rifle it can; the match keeps it and the knife.
    let mut l = l;
    equip(&mut l, Slot::Primary, L96A1).unwrap();
    assert_eq!(ready([6, 0], &l), Ok(()));
    assert_eq!(kit([6, 0], &l), [L96A1, M9]);
    // The pistol's message is the spec's.
    let mut nopistol = fresh();
    nopistol.normal.secondary = 921; // the type-5 shotgun is no pistol
    nopistol.normal.bag = [None; 4];
    assert_eq!(
        loadout::ready_check(&data(), &catalog(), &nopistol.normal, [1, 0]).unwrap_err(),
        "You do not have a pistol equipped for 'Pistols Only' mode."
    );
}

#[test]
fn case_53_no_backpack() {
    let mut l = fresh();
    equip(&mut l, Slot::Backpack(0), DESERT_EAGLE).unwrap();
    assert_eq!(ready([5, 0], &l), Ok(()));
    assert_eq!(
        kit([5, 0], &l),
        [M16A3, M92FS, M9, M67],
        "the backpack pistol is missing"
    );
    assert_eq!(kit([0, 0], &l), [M16A3, M92FS, M9, M67, DESERT_EAGLE]);
}

#[test]
fn case_54_no_explosives() {
    let l = fresh();
    assert_eq!(ready([4, 0], &l), Ok(()), "the M16A3 is a non-explosive primary");
    assert_eq!(kit([4, 0], &l), [M16A3, M92FS, M9], "the M67 is missing");
    let mut l = l;
    equip(&mut l, Slot::Primary, 930).unwrap();
    assert!(
        ready([4, 0], &l).is_err(),
        "a rocket launcher is not a non-explosive primary"
    );
    assert_eq!(kit([4, 0], &l), [M92FS, M9]);
}

#[test]
fn the_other_arms_options() {
    let mut l = fresh();
    // Melee Only: always ready; the knife is what stays.
    assert_eq!(ready([2, 0], &l), Ok(()));
    assert_eq!(kit([2, 0], &l), [M9]);
    // No Sniper Rifles: a non-sniper primary readies; the sniper is dropped.
    assert_eq!(ready([3, 0], &l), Ok(()));
    equip(&mut l, Slot::Primary, L96A1).unwrap();
    assert!(ready([3, 0], &l).is_err());
    assert_eq!(kit([3, 0], &l), [M92FS, M9, M67]);
    // Shotguns Only keeps both shotgun types and the knife; No Shotguns
    // drops them (but its readiness ignores a secondary shotgun, quirk 11).
    equip(&mut l, Slot::Primary, 914).unwrap();
    equip(&mut l, Slot::Secondary, 921).unwrap();
    assert_eq!(ready([7, 0], &l), Ok(()));
    assert_eq!(kit([7, 0], &l), [914, 921, M9]);
    assert!(ready([8, 0], &l).is_err());
    assert_eq!(kit([8, 0], &l), [M9, M67]);
    let mut other = fresh();
    equip(&mut other, Slot::Secondary, 921).unwrap();
    assert_eq!(
        ready([8, 0], &other),
        Ok(()),
        "a primary rifle: the secondary shotgun is ignored"
    );
    assert_eq!(kit([8, 0], &other), [M16A3, M9, M67], "while the server removes it");
    // With a 2nd Arms: "only" options keep what either allows, "except"
    // options need both and drop what either forbids.
    let l = fresh();
    assert_eq!(kit([1, 2], &l), [M92FS, M9]);
    assert_eq!(kit([3, 4], &l), [M16A3, M92FS, M9]);
    assert_eq!(ready([1, 6], &l), Ok(()), "either passes");
    // "Except" options need both to pass: a base loadout has no sniper
    // rifle for the second one's test.
    assert_eq!(ready([3, 4], &l), Ok(()));
}

#[test]
fn a_set_is_what_the_gear_takes() {
    let c = catalog();
    let mut l = fresh();
    l.extender = true;
    equip(&mut l, Slot::Head, 4001).ok();
    l.equip_ability(combat_arms::game::gear::gear_data(), SetKind::Normal, 16001)
        .unwrap();
    l.equip_ability(combat_arms::game::gear::gear_data(), SetKind::Normal, 16002)
        .unwrap();
    let base = combat_arms::game::characters::records::Selection::default();
    let (look, gear) = loadout::to_gear(&c, &l.normal, base, l.extender);
    assert_eq!(look.backpack, 1, "Standard Backpack is backpack record 1");
    assert_eq!(look.model, 0);
    assert_eq!(gear.perks, [16001, 16002]);
    assert!(gear.extender);
    assert!(gear.items.contains(&M16A3) && gear.items.contains(&7001));
    // Three ability slots, four with the extender.
    let mut l = fresh();
    let gd = combat_arms::game::gear::gear_data();
    l.equip_ability(gd, SetKind::Normal, 16001).unwrap();
    l.equip_ability(gd, SetKind::Normal, 16002).unwrap();
    l.equip_ability(gd, SetKind::Normal, 16003).unwrap();
    assert_eq!(l.equip_ability(gd, SetKind::Normal, 16004), Err(text::NO_ABILITY_SLOT));
    l.extender = true;
    assert_eq!(l.equip_ability(gd, SetKind::Normal, 16004), Ok(()));
    assert_eq!(l.equip_ability(gd, SetKind::Normal, 16004), Err(text::TWO_SAME));
}

#[test]
fn a_loadout_from_a_client_is_replayed_through_the_rules() {
    let c = catalog();
    let mut forged = fresh();
    forged.normal.primary = 7; // a grenade as the primary
    forged.normal.melee = M4A1; // a rifle as the knife
    forged.normal.bag = [Some(BOWIE), Some(DESERT_EAGLE), None, None];
    forged.normal.secondary = DESERT_EAGLE;
    let clean = forged.sanitized(&c, &Owned::all());
    assert_eq!(clean.normal.primary, M16A3);
    assert_eq!(clean.normal.melee, M9);
    assert_eq!(clean.normal.secondary, DESERT_EAGLE);
    assert_eq!(
        clean.normal.bag, [None; 4],
        "a knife and a duplicate pistol are refused, one slot only"
    );
}

// --- P: item lifetimes

#[test]
fn case_55_item_periods() {
    assert_eq!(expires_at(0.0, Consume::from_code(0).unwrap(), 7), Some(7.0));
    assert_eq!(expires_at(0.0, Consume::from_code(0).unwrap(), 999), None, "never");
    assert_eq!(
        expires_at(0.0, Consume::from_code(5).unwrap(), 3),
        Some(3.0 / 24.0),
        "3 hours"
    );
    assert_eq!(
        expires_at(2.0, Consume::from_code(7).unwrap(), 90),
        Some(2.0 + 90.0 / 1440.0),
        "minutes"
    );
    assert_eq!(
        expires_at(0.0, Consume::from_code(3).unwrap(), 10),
        None,
        "a use count, not a time"
    );
    // Boxes, GP and coupons are not equipment durations.
    assert_eq!(Consume::from_code(9), None);
    // The expiry clock removes only what has passed.
    let c = catalog();
    let mut owned = Owned::base_only(&c);
    owned.grant(M4A1, expires_at(0.0, Consume::Days, 7));
    owned.grant(AK47, expires_at(0.0, Consume::Days, 999));
    assert!(owned.expire(6.9).is_empty());
    assert_eq!(owned.expire(7.0), [M4A1]);
    assert!(owned.owns(&c, AK47) && !owned.owns(&c, M4A1));
}

#[test]
fn case_56_an_expired_primary_falls_back_then_the_best_owned_replaces_it() {
    let c = catalog();
    let mut owned = Owned::base_only(&c);
    for i in [AK47, M4A1, MP5A4] {
        owned.grant(i, None);
    }
    let mut l = fresh();
    l.equip(&c, &owned, SetKind::Normal, Slot::Primary, AK47).unwrap();
    owned.grant(AK47, Some(30.0));
    let gone = owned.expire(30.0);
    assert_eq!(gone, [AK47]);
    let fell = l.expired(&c, &gone);
    assert_eq!(l.normal.primary, M16A3, "at once the base item");
    assert!(fell.contains(&(SetKind::Normal, Slot::Primary)));
    // The next inventory visit: same weapon type (assault rifle) beats the
    // SMG.
    let expired: Vec<_> = gone.iter().filter_map(|n| c.get(*n).cloned()).collect();
    l.auto_equip(&c, &owned, &expired, &fell);
    assert_eq!(l.normal.primary, M4A1);
    // The player's own chosen default wins over the base item.
    let mut l = fresh();
    l.defaults[0] = Some(MP5A4);
    l.equip(&c, &Owned::all(), SetKind::Normal, Slot::Primary, AK47)
        .unwrap();
    l.expired(&c, &[AK47]);
    assert_eq!(l.normal.primary, MP5A4);
}

#[test]
fn expiry_releases_backpack_weapons_that_no_longer_fit() {
    let c = catalog();
    let mut l = fresh();
    // A three-slot backpack with weapons in it.
    equip(&mut l, Slot::BackpackGear, 7003).unwrap();
    equip(&mut l, Slot::Backpack(0), DESERT_EAGLE).unwrap();
    equip(&mut l, Slot::Backpack(1), 933).unwrap();
    equip(&mut l, Slot::Backpack(2), 932).unwrap();
    l.normal.attachments.insert(M16A3, vec![4242]);
    l.normal.abilities = vec![16001];
    l.expired(&c, &[7003, 4242, 16001]);
    assert_eq!(l.normal.backpack, Some(7001), "the Standard Backpack in the normal set");
    assert_eq!(l.normal.bag, [Some(DESERT_EAGLE), None, None, None], "one slot left");
    assert!(l.normal.attachments[&M16A3].is_empty());
    assert!(l.normal.abilities.is_empty());
    // The Pure Combat set falls back to none.
    equip_pure(&mut l);
    l.expired(&c, &[7003]);
    assert_eq!(l.pure_combat.backpack, None);
    assert_eq!(l.pure_combat.bag, [None; 4]);
}

fn equip_pure(l: &mut Loadout) {
    l.equip(&catalog(), &Owned::all(), SetKind::PureCombat, Slot::BackpackGear, 7003)
        .unwrap();
    l.equip(
        &catalog(),
        &Owned::all(),
        SetKind::PureCombat,
        Slot::Backpack(0),
        DESERT_EAGLE,
    )
    .unwrap();
}

#[test]
fn case_57_an_item_expiring_during_a_match_changes_nothing_until_it_is_over() {
    let mut lobby = Lobby::builtin();
    for p in 1..=2 {
        lobby.request(p, Request::Hello { name: format!("P{p}") }).unwrap();
    }
    let m = lobby.data.mode_by_key("TDM").unwrap();
    lobby
        .request(
            1,
            Request::Create(CreateSpec {
                name: "r".into(),
                password: String::new(),
                mode: m.id,
                pure_combat: false,
                map: None,
                goal: 100,
                players: 16,
                arms: [0, 0],
            }),
        )
        .unwrap();
    let room = lobby.room_of(1).unwrap();
    lobby
        .request(
            2,
            Request::Enter {
                room,
                password: String::new(),
            },
        )
        .unwrap();
    let mut l = fresh();
    l.normal.primary = AK47;
    lobby.request(2, Request::Loadout(Box::new(l))).unwrap();
    lobby.request(2, Request::Ready(true)).unwrap();
    lobby.request(1, Request::Start).unwrap();
    lobby.server_ready(room);
    lobby.take_events();
    for _ in 0..6 {
        lobby.tick();
    }
    let sent = lobby.take_events().into_iter().find_map(|e| match e {
        LobbyEvent::Expect { player: 2, weapons, .. } => Some(weapons),
        _ => None,
    });
    assert_eq!(sent, Some(vec![AK47, M92FS, M9, M67]), "the copy sent at entry");
    // The item expires in the match: the stored loadout changes, the
    // match's kit was already sent.
    lobby.items_expired(2, &[AK47]);
    assert_eq!(lobby.weapons_for(room, 2), [M16A3, M92FS, M9, M67], "the next match's");
    // After the result the notice reaches the client, shown on the next
    // visit to the waiting room.
    lobby.match_over(room);
    let notice = lobby.take_notices().into_iter().find_map(|(a, n)| match (a, n) {
        (Audience::Player(2), Notice::ItemsExpired(items)) => Some(items),
        _ => None,
    });
    assert_eq!(notice, Some(vec![AK47]));
    let mut client = combat_arms::game::lobby::client::ClientLobby::new(data(), 2, "P2");
    client.apply(Notice::ItemsExpired(vec![AK47]), 0.0);
    assert_eq!(client.expired, [AK47]);
}

#[test]
fn every_item_stands_on_one_of_our_weapons() {
    let c = catalog();
    let set = combat_arms::game::data::CaData::builtin_static().weapon_set();
    let ids: Vec<String> = set.weapons.iter().map(|d| d.id.clone()).collect();
    for i in c.items.values().filter(|i| i.kind.weapon()) {
        let w = i.weapon.as_ref().unwrap_or_else(|| panic!("{} has a weapon", i.name));
        assert!(ids.contains(w), "{}: {w} is one of our weapon documents", i.name);
    }
    // The four base items are the spec's.
    for (no, kind) in [
        (20, Kind::Assault),
        (24, Kind::Pistol),
        (0, Kind::Knife),
        (7, Kind::Throwable),
    ] {
        let i = c.get(no).unwrap();
        assert!(i.base && i.kind == kind);
    }
}
