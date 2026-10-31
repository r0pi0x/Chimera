//! NPC and boss bodies on the user's install (specs/combat_arms/
//! characters.md 12 and 13, test cases 62-86 and 89; 87 and 88 run here
//! too, on the install's libraries): the weapon, model, item, effect, AI
//! and mission files, every world's NPCs, the body files and their
//! libraries, and our mapping and loaders on them (every NPC gun's
//! holders are bodies the worlds give it; one body per family loads,
//! dresses and poses). Needs the V1 key (attributes, missions) and the V2
//! key (bodies, keyed worlds); skips with a note without the install.
//! Nothing read here is written anywhere.

use std::collections::{BTreeMap, BTreeSet};

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{
    GAME, INFO,
    characters::{
        CaAnim,
        driver::{Inputs, Status},
        models,
        records::{self, CharacterData},
        skeleton::Rig,
    },
    data::CaData,
    model, rungs,
    weapons::hit_table,
};
use lithtech_formats::{
    attributes::{Document, Section},
    ltb::Model,
    world::World,
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

fn read_doc(m: &chimera_core::mount::Mount, path: &str) -> Option<Document> {
    match m.read(path).ok().map(|b| Document::parse(&b)) {
        Some(Ok(d)) => Some(d),
        _ => {
            eprintln!("skipping: {path} unreadable (the V1 key)");
            None
        }
    }
}

fn text(s: &Section, key: &str) -> String {
    s.string(key).map(|b| b.iter().map(|&c| c as char).collect()).unwrap_or_default()
}

fn truthy(s: &Section, key: &str) -> bool {
    use lithtech_formats::attributes::Value;
    match s.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Int(i)) => *i != 0,
        Some(Value::Byte(i)) => *i != 0,
        Some(Value::Dword(i)) => *i != 0,
        Some(Value::Str(v)) => {
            let t: String = v.iter().map(|&c| c as char).collect();
            t.eq_ignore_ascii_case("true") || t == "1"
        }
        _ => false,
    }
}

/// The install's character data (the models file needs the V1 key).
fn install_characters() -> Option<&'static CharacterData> {
    let ca = CaData::install_static();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's models file isn't readable (the V1 key)");
        return None;
    }
    Some(ca.characters())
}

/// The 48 NPC and boss weapon candidates of 12.4 (by index).
const CANDIDATES: [usize; 48] = [
    62, 63, 64, 65, 66, 67, 68, 69, 283, 286, 287, 288, 366, 416, 417, 418, 419, 420, 421, 457, 458, 459, 460, 461, 462, 463, 526, 527,
    528, 529, 530, 531, 532, 533, 534, 535, 537, 538, 560, 561, 562, 676, 858, 863, 864, 1142, 1143, 1144,
];

/// 62, 63, 74: the weapon file's candidates and their fields; our mapping
/// names a holder for every linked one (and the spec's guesses), the
/// unlinked ones without a guess keep the derived rule.
#[test]
fn t62_63_74_weapon_file() {
    let Some(m) = mount() else { return };
    let Some(w) = read_doc(&m, "attributes/weapons.txt") else { return };
    let weapons = w.records("Weapon");
    assert_eq!(weapons.len(), 2386);
    let names: Vec<String> = weapons.iter().map(|s| text(s, "Name")).collect();
    let found: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            let n = n.to_lowercase();
            ["npc", "boss", "hqraid", "powered_suit", "assassin"].iter().any(|k| n.contains(k))
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(found, CANDIDATES, "the 48 candidates");
    // 63.
    let legs: Vec<usize> = (0..weapons.len()).filter(|&i| matches!(weapons[i].int("LowerAniType", 0), 1 | 2)).collect();
    assert_eq!(legs.len(), 40);
    assert_eq!(legs.iter().filter(|&&i| weapons[i].int("LowerAniType", 0) == 2).collect::<Vec<_>>(), [&366]);
    let hidden: Vec<usize> = (0..weapons.len()).filter(|&i| truthy(weapons[i], "Hidden")).collect();
    assert_eq!(hidden.len(), 21);
    assert!(hidden.iter().all(|i| legs.contains(i)), "every hidden one an infected hand");
    let no_select: Vec<usize> = (0..weapons.len()).filter(|&i| weapons[i].int("NoSelect", 0) == 1).collect();
    assert_eq!(no_select, [173, 447, 535]);
    let primary: Vec<usize> = (0..weapons.len()).filter(|&i| truthy(weapons[i], "AIPrimaryWeapon")).collect();
    assert_eq!(primary, [457, 538]);
    // 74.
    if let Some(items) = read_doc(&m, "attributes/items.txt") {
        let ids: BTreeSet<usize> = items.records("WeaponItem").iter().filter_map(|s| usize::try_from(s.int("ItemID", -1)).ok()).collect();
        let with: Vec<usize> = CANDIDATES.iter().copied().filter(|i| ids.contains(i)).collect();
        assert_eq!(with, [416, 417, 418, 419, 420, 526, 527, 528, 529, 530, 531, 532, 533, 534]);
        assert!(ids.contains(&344));
    }
    // Our mapping: every candidate but the unlinked ones without a guess
    // and the crossbows has its own holders.
    let h = rungs::holders();
    let ca = CaData::install_static();
    for &i in &CANDIDATES {
        let Some(id) = ca.weapon_for_index(i as i32) else { continue };
        let own = h.override_for(id).is_some();
        let keeps = [67, 421, 533, 534, 535, 561, 864].contains(&i);
        assert_eq!(own, !keeps, "{i} {id}");
    }
}

/// Every world's `CAIHuman` objects (the worlds that read).
fn world_npcs(m: &chimera_core::mount::Mount) -> Vec<(String, BTreeMap<String, String>)> {
    let mut paths: Vec<String> = m.entries().into_iter().map(|(e, _)| e.path).filter(|p| p.starts_with("worlds") && p.ends_with(".dat")).collect();
    paths.sort();
    paths.dedup();
    let mut out = Vec::new();
    for p in paths {
        let Ok(bytes) = m.read(&p) else { continue };
        let Ok(w) = World::parse(&bytes) else { continue };
        for (_, o) in w.objects_of("CAIHuman") {
            let mut props = BTreeMap::new();
            for k in ["ModelTemplate", "RightHand", "LeftHand", "RightHand2", "LeftHand2", "NPCType", "Brain"] {
                props.insert(k.to_string(), o.string(k).unwrap_or_default().to_string());
            }
            let world = p.rsplit('/').next().unwrap_or(&p).trim_end_matches(".dat").to_string();
            out.push((world, props));
        }
    }
    out
}

/// 64, 65, 76: the worlds' NPCs; and our mapping against them: each NPC
/// or boss gun, and each infected hand, a world puts in an NPC's hands
/// may be drawn on that NPC's body.
#[test]
fn t64_65_76_world_npcs() {
    let Some(m) = mount() else { return };
    let Some(ch) = install_characters() else { return };
    let npcs = world_npcs(&m);
    let worlds: BTreeSet<&str> = npcs.iter().map(|(w, _)| w.as_str()).collect();
    if npcs.len() != 947 {
        eprintln!("worlds read: {} ({} NPCs); some keyed worlds need the V2 key", worlds.len(), npcs.len());
    }
    assert_eq!((npcs.len(), worlds.len()), (947, 23));
    let base = ch.model.iter().filter(|r| r.npc.is_none()).map(|r| r.id).max().unwrap() + 1;
    let by_name = |n: &str| ch.model.iter().find(|r| r.npc.is_some() && r.name.eq_ignore_ascii_case(n));
    let mut placed: BTreeSet<i32> = BTreeSet::new();
    let mut by_table: BTreeMap<usize, usize> = BTreeMap::new();
    for (_, p) in &npcs {
        let r = by_name(&p["ModelTemplate"]).unwrap_or_else(|| panic!("{} names no AIModel", p["ModelTemplate"]));
        assert!(r.id >= base);
        placed.insert(r.id);
        *by_table.entry(r.skeleton).or_default() += 1;
        assert_eq!((p["RightHand2"].as_str(), p["LeftHand2"].as_str()), ("<none>", "<none>"));
    }
    assert_eq!(placed.len(), 86);
    let want: BTreeMap<usize, usize> =
        [(0, 768), (1, 46), (2, 67), (3, 11), (5, 18), (7, 7), (9, 1), (10, 1), (11, 2), (12, 4), (13, 4), (14, 4), (15, 3), (16, 5), (18, 6)].into();
    assert_eq!(by_table, want);
    // 65.
    let holders_of = |hand: &str| -> BTreeSet<String> {
        npcs.iter().filter(|(_, p)| p["RightHand"].eq_ignore_ascii_case(hand) || p["LeftHand"].eq_ignore_ascii_case(hand)).map(|(_, p)| p["ModelTemplate"].clone()).collect()
    };
    let in_worlds = |hand: &str| -> BTreeSet<String> { npcs.iter().filter(|(_, p)| p["RightHand"].eq_ignore_ascii_case(hand)).map(|(w, _)| w.to_lowercase()).collect() };
    assert_eq!(holders_of("VH_Normal_NPC"), ["Normal_Infecter_Common_D".to_string()].into());
    assert_eq!(in_worlds("VH_Normal_NPC"), ["blacklung", "cabinfever", "deathroom_cp", "railwaydefence"].iter().map(|s| s.to_string()).collect());
    assert_eq!(
        holders_of("HQRAIDNPC_2_SMG"),
        ["HQRAIDNPC_2", "Powered_Suit_Npc_Melee", "Powered_Suit_Npc_Smg", "SubMission_Z_1", "SubMission_Z_2"].iter().map(|s| s.to_string()).collect()
    );
    assert!(holders_of("NPC_Powered_Suit_Basic_SMG").is_empty() && holders_of("NPC_Powered_Suit_Basic_Melee").is_empty());
    // Our mapping covers every (NPC body, NPC weapon) pair the worlds give.
    let ca = CaData::install_static();
    let h = rungs::holders();
    let catalog_infected = |id: &str| ca.weapon_set().get(id).is_some_and(|w| w.has_hook(rungs::INFECTED));
    let mut checked = 0;
    for (_, p) in &npcs {
        for hand in [&p["RightHand"], &p["LeftHand"]] {
            let Some(id) = ca.weapon_for_name(hand) else { continue };
            let npcish = CANDIDATES.iter().any(|&i| ca.weapon_for_index(i as i32) == Some(id)) || catalog_infected(id);
            if !npcish || h.override_for(id).is_none() {
                continue;
            }
            let derived = catalog_infected(id).then_some(rungs::INFECTED_VALUE);
            let bodies = rungs::bodies_for(&h, None, Some(ca), id);
            let body = by_name(&p["ModelTemplate"]).unwrap();
            assert!(bodies.contains(&body.id), "{hand} on {}: not among its holders ({:?})", body.name, h.of(id, derived));
            checked += 1;
        }
    }
    assert!(checked > 500, "{checked}");
}

/// 66: the HQ raid boss's weapons (the effect file's pattern record, the
/// weapon file's fire patterns).
#[test]
fn t66_raid_boss() {
    let Some(m) = mount() else { return };
    let (Some(fx), Some(w)) = (read_doc(&m, "attributes/fx.txt"), read_doc(&m, "attributes/weapons.txt")) else { return };
    let p = fx.section("PatternFX0").unwrap();
    assert_eq!(text(p, "Name"), "HQRaid_Boss");
    let got: Vec<(i32, String, i32)> = (0..3).map(|i| (p.int(&format!("Weapon{i}"), -1), text(p, &format!("WeaponSoket{i}")), p.int(&format!("WeaponProbabilty{i}"), -1))).collect();
    assert_eq!(got, [(460, "highgun".into(), 100), (462, "Missile_".into(), 50), (463, "bomb_".into(), 50)]);
    let ids: Vec<i32> = w.records("PatternWeapon").iter().map(|s| s.int("WeaponId", -1)).collect();
    assert_eq!(ids, [460, 461, 462, 463]);
}

/// 67, 72: the suit sets and the infected loadouts of the mission files.
#[test]
fn t67_72_mission_files() {
    let Some(m) = mount() else { return };
    let Some(lab) = read_doc(&m, "gms/nemexislab_cp.gms") else { return };
    let set = |d: &Document, n: i32| -> (i32, Vec<i32>) {
        let info = d.section(&format!("ChangeCharInfo_{n}")).unwrap();
        let model = d.section(&format!("ChangeCharModel_{}", info.int("ModelIndex", -1))).unwrap().int("ModelId", -1);
        let weapon = d.section(&format!("ChangeCharWeapon_{}", info.int("WeaponIndex", -1))).unwrap();
        let mut w: Vec<i32> = (0..4).map(|i| weapon.int(&format!("Primary{i}"), -1)).filter(|&x| x >= 0).collect();
        w.extend((0..4).map(|i| weapon.int(&format!("Secondary{i}"), -1)).filter(|&x| x >= 0));
        (model, w)
    };
    assert_eq!(set(&lab, 2), (58, vec![527]));
    assert_eq!(set(&lab, 0), (62, vec![528, 531]));
    let hole = read_doc(&m, "gms/deserthole_rtdm.gms").unwrap();
    assert_eq!((0..).take_while(|n| hole.section(&format!("ChangeCharInfo_{n}")).is_some()).count(), 10);
    assert_eq!(set(&hole, 5), (588, vec![528, 531]));
    // Our suit sets: each set's weapons may be drawn on its body.
    let ca = CaData::install_static();
    let h = rungs::holders();
    for d in [&lab, &hole] {
        for n in 0.. {
            if d.section(&format!("ChangeCharInfo_{n}")).is_none() {
                break;
            }
            let (body, weapons) = set(d, n);
            for w in weapons {
                let id = ca.weapon_for_index(w).unwrap();
                assert!(rungs::bodies_for(&h, None, Some(ca), id).contains(&body), "set {n}: {id} on {body}");
            }
        }
    }
    // 72.
    let mut qrt = 0;
    let mut paths: Vec<String> = m.entries().into_iter().map(|(e, _)| e.path).filter(|p| p.starts_with("gms/") && p.contains("_qrt") && p.ends_with(".gms")).collect();
    paths.sort();
    paths.dedup();
    for p in &paths {
        let d = read_doc(&m, p).unwrap();
        assert_eq!(d.section("InfectedModelSetting").map(|s| s.int("MeleeWeapon0", -1)), Some(191), "{p}");
        assert_eq!(d.section("InfectedHostModelSetting").map(|s| s.int("MeleeWeapon0", -1)), Some(190), "{p}");
        qrt += 1;
    }
    assert_eq!(qrt, 33);
    let llhg = read_doc(&m, "gms/noxiousdamnation_llhg.gms").unwrap();
    assert_eq!(llhg.section("InfectedModelSetting").map(|s| s.int("MeleeWeapon0", -1)), Some(1346));
}

/// 68, 69, 70: the item file's infected mercenaries, characters and
/// slot-typed weapons.
#[test]
fn t68_69_70_item_file() {
    let Some(m) = mount() else { return };
    let Some(items) = read_doc(&m, "attributes/items.txt") else { return };
    // Use items by their `ItemID`.
    let uses = items.records("UseItem");
    let item = |id: i32| *uses.iter().find(|u| u.int("ItemID", -1) == id).unwrap_or_else(|| panic!("use item {id}"));
    let merc = |id: i32| {
        let u = item(id);
        (u.int("SubType", -1), u.int("InfectedModelNo", -1), u.int("InfectedWeaponNo", -1))
    };
    assert_eq!(merc(54), (20, 56, 521));
    assert_eq!(merc(55), (20, 57, 522));
    assert_eq!(merc(148), (20, 513, 1734));
    assert_eq!((merc(150).1, merc(150).2), (513, 1734));
    let chars = items.records("Character");
    assert_eq!(chars.len(), 303);
    // The spec reads all 303 as mercenaries (22); this install has 113
    // of type 38. Neither is a specialist's or an assassin's (30, 37).
    assert!(chars.iter().all(|c| matches!(c.int("ItemType", -1), 22 | 38)));
    assert!(items.sections().iter().all(|s| !matches!(s.int("ItemType", -1), 30 | 36 | 37)));
    let slots: Vec<&&Section> = chars.iter().filter(|c| c.get("AddSlot").is_some()).collect();
    assert_eq!(slots.len(), 116);
    assert!(slots.iter().all(|c| c.int("SlotType", 0) == 0));
    assert_eq!(chars[14].int("ItemID", -1), 22);
    if let Some(ch) = install_characters() {
        assert_eq!(ch.model(22).unwrap().name, "Specialist01_M");
    }
    let typed: Vec<(i32, i32, i32)> = items
        .records("WeaponItem")
        .iter()
        .filter(|s| s.get("EquipBackpackType").is_some())
        .map(|s| (s.int("ItemID", -1), s.int("EquipBackpackType", -1), s.int("EquipOnlyBackpack", -1)))
        .collect();
    assert_eq!(typed.len(), 8);
    assert!(typed.iter().all(|t| t.2 == 1));
    let special: BTreeSet<i32> = typed.iter().filter(|t| t.1 == 1).map(|t| t.0).collect();
    assert_eq!(special, [173, 174, 290, 291, 293, 447, 558].into());
    assert_eq!(typed.iter().filter(|t| t.1 == 3).map(|t| t.0).collect::<Vec<_>>(), [536]);
    // The spec's four extend items (this install also has item 44 with
    // `SubType` 18: not in 12.3's list).
    let extend: BTreeSet<i32> = uses.iter().filter(|u| u.int("SubType", -1) == 18).map(|u| u.int("ItemID", -1)).collect();
    assert!([36, 52, 76, 77].iter().all(|i| extend.contains(i)), "{extend:?}");
}

/// 73: the ice-gas skill fires its weapon, but no brain lists it.
#[test]
fn t73_ai_skill() {
    let Some(m) = mount() else { return };
    let mut skill = false;
    for p in ["attributes/aibutes.txt", "attributes/aibutes_scp.txt", "attributes/aibutes_single.txt"] {
        let Some(d) = read_doc(&m, p) else { return };
        if let Some(s) = d.section("NpcSkill16") {
            assert_eq!((text(s, "Name").as_str(), text(s, "UseWeaponName").as_str()), ("P_Boss_IceGas", "Abnormal_IceGas_boss"));
            skill = true;
        }
        for b in d.records("Brain") {
            for i in 0..32 {
                assert!(!text(b, &format!("NpcSkillName{i}")).eq_ignore_ascii_case("P_Boss_IceGas"), "{p} {}", b.name());
            }
        }
    }
    assert!(skill);
}

/// 75, 77: the models file's NPC records, and our records from them.
#[test]
fn t75_77_npc_records() {
    let Some(m) = mount() else { return };
    let Some(d) = read_doc(&m, "attributes/modelbutes.txt") else { return };
    let ai = d.records("AIModel");
    assert_eq!((ai.len(), d.records("AIFace").len()), (101, 59));
    let robots: Vec<usize> = (0..ai.len()).filter(|&i| ai[i].int("AIOnly", 1) == 0 && ai[i].int("HumanModel", 1) == 0).collect();
    assert_eq!(robots, [35, 36]);
    assert!((0..ai.len()).all(|i| (ai[i].int("AIOnly", 1) == 0) == robots.contains(&i)));
    assert!(ai.iter().chain(d.records("Model").iter()).all(|s| s.get("ModelScale").is_none()), "no ModelScale anywhere");
    let deaths: Vec<(usize, String)> = (0..ai.len()).filter_map(|i| Some((i, text(ai[i], "UseSpecialDeathAni"))).filter(|(_, t)| !t.is_empty())).collect();
    let want: Vec<(usize, String)> =
        [8, 9, 40, 50, 60, 70, 100].iter().map(|&i| (i, "VDIE".to_string())).chain([87, 88].iter().map(|&i| (i, "death".to_string()))).collect();
    let mut deaths = deaths;
    deaths.sort();
    let mut want = want;
    want.sort();
    assert_eq!(deaths, want);
    // 77, on our records.
    let Some(ch) = install_characters() else { return };
    let base = d.records("Model").len() as i32;
    let faces = d.records("Face").len() as i32;
    let squad = ch.model(base + 13).unwrap();
    assert_eq!((squad.name.as_str(), squad.file.as_str()), ("Squad_1", "CHARS_M_BODY\\CM_Body_HV_NX_YK.ltb"));
    let n = squad.npc.as_ref().unwrap();
    assert_eq!(n.skins.len(), 5);
    assert_eq!(n.face, faces + 10);
    assert_eq!(ch.face(n.face).unwrap().file.as_deref().map(str::to_lowercase).as_deref(), Some("chars_m_face\\face_a.ltb"));
    assert_eq!((n.head, ch.head_file(n.head, 0).map(str::to_lowercase).as_deref()), (3, Some("chars_m_head\\cap.ltb")));
    assert_eq!(n.goggle, 18);
    assert_eq!(ch.goggle_file(18, 0).map(|g| g.0), Some("Eyes"));
    let rifle = ch.model(base).unwrap().npc.as_ref().unwrap();
    assert_eq!(rifle.face, faces + 6);
    assert_eq!(ch.face(rifle.face).unwrap().file.as_deref().map(str::to_lowercase).as_deref(), Some("chars_m_face\\mt_no_face.ltb"));
    assert_eq!((rifle.head, rifle.goggle), (-1, -1));
    assert_eq!(ch.model(base + 8).unwrap().npc.as_ref().unwrap().face, -1);
    assert_eq!(ch.model(base + 58).unwrap().npc.as_ref().unwrap().face, 55);
    // Every record's pieces resolve (13.2: "all records that set them").
    for r in ch.model.iter().filter(|r| r.npc.is_some()) {
        let n = r.npc.as_ref().unwrap();
        let i = (r.id - base) as usize;
        if text(ai[i], "AIHair").len() > 0 && !text(ai[i], "AIHair").eq_ignore_ascii_case("AIhair0") {
            assert!(n.head >= 0, "{}", r.name);
        }
        if ai[i].int("AttachFace", 1) != 0 && ai[i].get("AIFace").is_some() && !text(ai[i], "AIFace").eq_ignore_ascii_case("AIFace0") {
            assert!(n.face >= 0, "{}", r.name);
        }
    }
    assert_eq!(ch.npc_bodies().len(), 101 - 3, "all but the gunship's and the two dummies");
}

/// The body file of a record, read alone (no library).
fn own(m: &chimera_core::mount::Mount, path: &str) -> Option<Model> {
    model::load_own(path, &|p: &str| m.read(p).ok()).ok().map(|l| l.model)
}

fn shape(model: &Model) -> Vec<(String, Option<usize>)> {
    model.nodes.iter().map(|n| (n.name.to_lowercase(), n.parent)).collect()
}

fn has_socket(model: &Model, name: &str) -> bool {
    model.sockets.iter().any(|s| s.name.eq_ignore_ascii_case(name))
}

/// 78: the bodies the `AIModel`s name, by tree and library.
#[test]
fn t78_body_files() {
    let Some(m) = mount() else { return };
    let Some(ch) = install_characters() else { return };
    let Some(male) = own(&m, "CHARS_M_BODY\\CM_VST_NM_DF_Nuts_JS.ltb") else {
        eprintln!("skipping: the body files need the V2 key");
        return;
    };
    let female = own(&m, "CHARS_M_BODY\\Nomal_W_vest.ltb").unwrap();
    let files: BTreeSet<String> = ch.model.iter().filter(|r| r.npc.is_some()).map(|r| r.file.to_lowercase()).collect();
    assert_eq!(files.len(), 64);
    let (mut males, mut females, mut others, mut missing) = (0, 0, 0, Vec::new());
    for f in &files {
        let Some(b) = own(&m, f) else {
            missing.push(f.clone());
            continue;
        };
        let child = b.children.first().map(|c| c.to_lowercase()).unwrap_or_default();
        if shape(&b) == shape(&male) && child.ends_with("st_m_child.ltb") {
            males += 1;
        } else if shape(&b) == shape(&female) && child.ends_with("st_w_child.ltb") {
            females += 1;
        } else {
            others += 1;
        }
    }
    assert_eq!(missing, ["chars_m_body\\body_tutorial_dummy_ki.ltb"]);
    assert_eq!((males, females, others), (33, 11, 19));
    let gg = own(&m, "CHARS_M_BODY\\MT_GG_BODY.ltb").unwrap();
    assert!(!has_socket(&gg, "Eyes") && gg.animations.is_empty());
    for f in ["MT_AR_BODY", "MT_NO_BODY", "MT_GG_BODY", "MT_SP_BODY", "MT_RPG_BODY", "MT_BM_BODY", "MT_ELT_BODY"] {
        assert!(!has_socket(&own(&m, &format!("CHARS_M_BODY\\{f}.ltb")).unwrap(), "Nuts"), "{f}");
    }
    // Our families agree: the male and female bodies are those families.
    for r in ch.model.iter().filter(|r| r.npc.is_some() && r.file.to_lowercase() != "chars_m_body\\body_tutorial_dummy_ki.ltb") {
        let b = own(&m, &r.file).unwrap();
        let fam = ch.family(r);
        if shape(&b) == shape(&male) {
            assert_eq!(fam.name, "male", "{}", r.name);
        } else if shape(&b) == shape(&female) {
            assert_eq!(fam.name, "female", "{}", r.name);
        } else {
            assert!(fam.stock_rig.is_none(), "{} ({}): {}", r.name, r.file, fam.name);
        }
    }
}

/// A body and its library.
fn full(m: &chimera_core::mount::Mount, path: &str) -> Option<model::LoadedModel> {
    model::load(path, &|p: &str| m.read(p).ok()).ok()
}

fn length(l: &model::LoadedModel, name: &str) -> Option<u32> {
    l.list.find(name).map(|e| l.list.entries[e].length())
}

fn weight_sets(model: &Model) -> Vec<String> {
    model.weight_sets.iter().map(|w| w.name.clone()).collect()
}

/// 79, 80: the infected bodies.
#[test]
fn t79_80_infected_bodies() {
    let Some(m) = mount() else { return };
    let Some(t) = full(&m, "CHARS_M_BODY\\VIM_F_NM_DF_Tanker_SH.ltb") else {
        eprintln!("skipping: the body files need the V2 key");
        return;
    };
    assert_eq!((t.model.nodes.len(), t.model.sockets.len()), (43, 17));
    assert_eq!(weight_sets(&t.model).iter().map(|w| w.to_lowercase()).collect::<BTreeSet<_>>(), ["blend", "lower", "upper"].iter().map(|s| s.to_string()).collect());
    assert!(t.model.children[0].to_lowercase().ends_with("ani_vi_tanker_sh.ltb"));
    assert_eq!(t.list.entries.len(), 103);
    assert_eq!(
        ["VLST", "VLRF", "VLRRF", "VUMRFIS", "VDIE"].map(|n| length(&t, n)),
        [Some(1333), Some(800), Some(667), Some(666), Some(3733)]
    );
    assert_eq!(t.list.entries[t.list.find("VUMRFIS").unwrap()].animation.interp_ms, 0);
    for absent in ["VLSJ", "VLRRL", "VLRRR", "VUMRSSeS", "D_HIT", "VD_HITT"] {
        assert!(t.list.find(absent).is_none(), "{absent}");
    }
    assert!(t.list.find("VLJS").is_some());
    let c = own(&m, "CHARS_M_BODY\\CM_Body_Infected_Ship_Captain_KE.ltb").unwrap();
    assert_eq!(c.nodes.iter().map(|n| n.parent).collect::<Vec<_>>(), t.model.nodes.iter().map(|n| n.parent).collect::<Vec<_>>());
    assert_eq!([7, 9, 37, 42].map(|i| c.nodes[i].name.as_str()), ["Bip01 Neck", "Bip01 L Clavicle", "Bip01 L Foot", "Left_foot"]);

    let a = full(&m, "CHARS_M_BODY\\VIW_F_NM_DF_Assassin_CH.ltb").unwrap();
    assert_eq!((a.model.nodes.len(), a.model.sockets.len()), (71, 19));
    let eyes = a.model.sockets.iter().find(|s| s.name == "Eyes").unwrap();
    assert_eq!(a.model.nodes[eyes.node as usize].name, "Bip01 Head");
    assert!(a.model.children[0].to_lowercase().ends_with("ani_vi_assassin_ch.ltb"));
    assert_eq!(a.list.entries.len(), 149);
    assert_eq!(["VLST", "VLC", "VD_HITT", "VDIE1"].map(|n| length(&a, n)), [Some(5666), Some(5700), Some(533), Some(3933)]);
    let vlc = &a.list.entries[a.list.find("VLC").unwrap()];
    assert_eq!((vlc.dims, vlc.translation), ([15.0, 40.0, 15.0], [0.0, -40.0, 0.0]));
    let s = own(&m, "CHARS_M_BODY\\CW_VST_TheShining.ltb").unwrap();
    assert_eq!(shape(&s), shape(&a.model));
    assert!(s.weight_sets.is_empty());
}

/// 81, 82: the powered suits, and the hull and eye the player rule gives
/// them (`Rig::new`).
#[test]
fn t81_82_suits() {
    let Some(m) = mount() else { return };
    let Some(s) = full(&m, "CHARS_M_BODY\\Suit_small_Base.ltb") else {
        eprintln!("skipping: the body files need the V2 key");
        return;
    };
    assert_eq!((s.model.nodes.len(), s.model.sockets.len()), (44, 13));
    assert!(!has_socket(&s.model, "Eyes") && !s.model.nodes.iter().any(|n| n.name == "Upper_torso"));
    assert!(s.model.children[0].to_lowercase().ends_with("small_anibase.ltb"));
    assert_eq!(s.list.entries.len(), 272);
    let e = |l: &model::LoadedModel, n: &str| l.list.entries[l.list.find(n).unwrap()].clone();
    assert_eq!((e(&s, "LST").dims, e(&s, "LST").translation), ([80.0, 100.0, 50.0], [0.0, -100.0, 0.0]));
    assert_eq!((e(&s, "LC").dims, e(&s, "LC").translation), ([80.0, 70.0, 50.0], [0.0, -70.0, 0.0]));
    for absent in ["LWFL", "LRFL", "LRRL"] {
        assert!(s.list.find(absent).is_none(), "{absent}");
    }
    assert_eq!(["USMGRFiS", "USMGRSReS", "D_HIT", "DT"].map(|n| length(&s, n)), [Some(200), Some(1933), Some(833), Some(2666)]);
    // Every library animation (not the body's own, not the `---x---`
    // separators) blends in 50 ms.
    assert!(s.list.entries.iter().filter(|e| e.child == 1 && !e.name().starts_with("---")).all(|e| e.animation.interp_ms == 50));
    let rig = Rig::new(s.model.clone(), s.list.clone());
    assert_eq!(rig.half_heights, (100.0, 70.0));
    assert!((0.66 * rig.half_heights.0 - 66.0).abs() < 1e-3);

    let b = full(&m, "CHARS_M_BODY\\Suit_BIG_Base.ltb").unwrap();
    assert_eq!((b.model.nodes.len(), b.model.sockets.len()), (43, 16));
    assert!(b.model.children[0].to_lowercase().ends_with("big_anibase.ltb"));
    assert_eq!(b.list.entries.len(), 122);
    assert_eq!((e(&b, "LST").dims, e(&b, "LST").translation), ([128.0, 150.0, 70.0], [0.0, -150.0, -40.0]));
    assert_eq!((e(&b, "LC").dims, e(&b, "LC").translation), ([128.0, 120.0, 80.0], [0.0, -120.0, -20.0]));
    let hand = b.model.sockets.iter().find(|s| s.name == "RightHand").unwrap();
    assert_eq!(b.model.nodes[hand.node as usize].name, "Right_hand");
    assert!((hand.position[0] + 12.109).abs() < 1e-3 && hand.position[1] == 0.0 && hand.position[2] == 0.0);
    let eyes = b.model.sockets.iter().find(|s| s.name == "Eyes").unwrap();
    assert_eq!((b.model.nodes[eyes.node as usize].name.as_str(), hand.position.len()), ("Head", 3));
    assert!((eyes.position[0] - 8.281).abs() < 1e-3 && (eyes.position[2] - 7.891).abs() < 1e-3);
    assert!(b.list.find("USMGRAmS").is_none());
    let rig = Rig::new(b.model.clone(), b.list.clone());
    assert!((0.66 * rig.half_heights.0 - 99.0).abs() < 1e-3);
}

/// 83, 84, 86: the bosses' bodies.
#[test]
fn t83_84_86_bosses() {
    let Some(m) = mount() else { return };
    let Some(eod) = full(&m, "CHARS_M_BODY\\PV_CabinFever2_EOD_Boss_KE.ltb") else {
        eprintln!("skipping: the body files need the V2 key");
        return;
    };
    assert_eq!((eod.model.nodes.len(), eod.model.sockets.len()), (35, 8));
    let sets: BTreeSet<String> = weight_sets(&eod.model).into_iter().collect();
    assert_eq!(sets, ["AL", "BODY", "Lower", "blend", "upper"].iter().map(|s| s.to_string()).collect());
    assert_eq!(eod.list.entries.len(), 20);
    assert!(eod.list.entries.iter().all(|e| e.dims == [70.0, 100.0, 55.0] && e.translation == [0.0, -100.0, 0.0]));
    assert_eq!(["ilde", "Walk", "All_Attack", "Death"].map(|n| length(&eod, n)), [Some(1667), Some(1367), Some(7033), Some(4667)]);
    let hand = eod.model.sockets.iter().find(|s| s.name == "RightHand").unwrap();
    assert_eq!(eod.model.nodes[hand.node as usize].name, "Dummy003");
    assert!(hand.position.iter().zip([37.0, 6.031, 2.031]).all(|(a, b)| (a - b).abs() < 1e-3), "{:?}", hand.position);
    let left = eod.model.sockets.iter().find(|s| s.name == "LeftHand").unwrap();
    assert_eq!(eod.model.nodes[left.node as usize].name, "Dummy001");
    // Our feet for it (Open question 35): where its animations put them.
    assert_eq!(Rig::new(eod.model.clone(), eod.list.clone()).half_heights, (100.0, 100.0));
    // Every name our mapping plays exists.
    let ch = install_characters();
    let fams = combat_arms::game::characters::families::families();
    let check = |l: &model::LoadedModel, family: &str| {
        let b = fams.families.get(family).unwrap().boss.as_ref().unwrap();
        for n in std::iter::once(&b.idle).chain(&b.walk).chain(&b.fire).chain(&b.melee).chain(&b.hit).chain(&b.death) {
            assert!(l.list.find(n).is_some(), "{family}: {n}");
        }
    };
    check(&eod, "eod_boss");

    let counts = [
        ("GXL-4209", 12, "gxl_4209"),
        ("GXL-4209B", 15, "gxl_4209b"),
        ("GXL-X", 4, "gxl_x"),
        ("Boss_Fortress", 18, "fortress"),
        ("Mob_Robot", 8, "mob_robot"),
        ("Beetle", 12, "beetle"),
        ("Callus", 4, "callus"),
        ("Scorpion", 10, "scorpion"),
        ("Thorn", 5, "thorn"),
        ("Bugs_Explosive", 4, "bugs"),
        ("Bugs_MeleeHeavy", 4, "bugs"),
    ];
    for (file, n, family) in counts {
        let l = full(&m, &format!("CHARS_M_BODY\\{file}.ltb")).unwrap();
        assert_eq!(l.list.entries.len(), n, "{file}");
        let upper = l.model.weight_sets.iter().find(|w| w.name == "Upper").unwrap();
        let lower = l.model.weight_sets.iter().find(|w| w.name == "Lower").unwrap();
        assert!(upper.weights.iter().all(|&w| w == 1.0) && lower.weights.iter().all(|&w| w == 0.0), "{file}");
        check(&l, family);
        if file == "Boss_Fortress" {
            assert_eq!(length(&l, "Gatling_LP"), Some(2567));
            let left = l.model.sockets.iter().find(|s| s.name == "LeftHand").unwrap();
            assert_eq!((l.model.nodes[left.node as usize].name.as_str(), left.position), ("Gun_L", [100.0, 0.0, 0.0]));
        }
    }
    // 86: the gunship's body (no walking body of ours).
    let g = full(&m, "CHARS_M_BODY\\boss_model.ltb").unwrap();
    assert_eq!((g.model.nodes.len(), g.model.sockets.len(), g.model.weight_sets.len()), (66, 24, 0));
    assert!(g.model.children[0].to_lowercase().ends_with("boss_anibase.ltb"));
    assert_eq!(g.list.entries.len(), 14);
    assert_eq!(g.list.entries[0].dims, [450.0, 200.0, 500.0]);
    assert_eq!(["idle", "left", "left_start", "left_end"].map(|n| length(&g, n)), [Some(333), Some(133), Some(333), Some(500)]);
    for i in 1..=6 {
        assert!(has_socket(&g.model, &format!("Missile_{i}")));
    }
    for i in 1..=7 {
        assert!(has_socket(&g.model, &format!("bomb_{i}")));
    }
    assert!(!has_socket(&g.model, "Missile_") && !has_socket(&g.model, "bomb_"));
    let high = g.model.sockets.iter().find(|s| s.name == "highgun").unwrap();
    assert_eq!(g.model.nodes[high.node as usize].name, "du_minigun_01");
    if let Some(ch) = ch {
        let sandhog = ch.model.iter().find(|r| r.name == "SH_Sandhog").unwrap();
        assert!(!ch.family(sandhog).wearable);
    }
}

/// How a hit table's entries with radius > 0 resolve on a body: (node,
/// socket, absent) (11).
fn resolves(model: &Model, table: usize) -> (usize, usize, usize) {
    let t = records::table(table);
    let mut out = (0, 0, 0);
    for s in t.spheres.iter().filter(|s| s.radius > 0.0) {
        if model.nodes.iter().any(|n| n.name.eq_ignore_ascii_case(&s.name)) {
            out.0 += 1;
        } else if has_socket(model, &s.name) {
            out.1 += 1;
        } else {
            out.2 += 1;
        }
    }
    out
}

/// 85: the hit tables on their bodies, as our tables (the install's past
/// ours) resolve them.
#[test]
fn t85_hit_tables() {
    let Some(m) = mount() else { return };
    if install_characters().is_none() {
        return;
    }
    let Some(tanker) = own(&m, "CHARS_M_BODY\\VIM_F_NM_DF_Tanker_SH.ltb") else {
        eprintln!("skipping: the body files need the V2 key");
        return;
    };
    let body = |f: &str| own(&m, &format!("CHARS_M_BODY\\{f}.ltb")).unwrap();
    assert_eq!(resolves(&tanker, 2), (41, 4, 0));
    assert_eq!(resolves(&body("CM_Body_Infected_Ship_Captain_KE"), 2), (38, 4, 3));
    assert_eq!(resolves(&body("VIW_F_NM_DF_Assassin_CH"), 3), (57, 8, 1));
    assert_eq!(resolves(&body("Suit_small_Base"), 5), (22, 8, 3));
    assert_eq!(resolves(&body("Suit_BIG_Base"), 6), (22, 9, 2));
    assert_eq!(resolves(&body("PV_CabinFever2_EOD_Boss_KE"), 7), (30, 0, 2));
    assert_eq!(resolves(&body("GXL-4209"), 7), (19, 0, 13));
    assert_eq!(resolves(&body("Bugs_Explosive"), 18), (1, 0, 19));
    assert_eq!(resolves(&body("boss_model"), 4), (41, 0, 0));
    // Table 0 on the female body (Quirk 12): ours, the measured male
    // table, by name.
    let light = body("Light_W_vest");
    let ours = records::table(0);
    let (mut n, mut s, mut a) = (0, 0, 0);
    for sp in ours.spheres.iter().filter(|sp| sp.radius > 0.0) {
        if light.nodes.iter().any(|x| x.name.eq_ignore_ascii_case(&sp.name)) {
            n += 1;
        } else if has_socket(&light, &sp.name) {
            s += 1;
        } else {
            a += 1;
        }
    }
    assert_eq!((n, s, a), (19, 9, 7));
    // Our hit spheres of an install table keep their radius (a body
    // model poses them); table 2's head is worth three.
    let t2 = hit_table(2);
    let head = t2.iter().find(|s| s.name == "Head").unwrap();
    assert_eq!((head.radius, head.factor), (5.5, 3.0));
}

/// 89: the Steam libraries' names.
#[test]
fn t89_libraries() {
    let Some(m) = mount() else { return };
    let Some(male) = own(&m, "CHARS_M_BODY\\ST_M_child.ltb") else {
        eprintln!("skipping: the libraries need the V2 key");
        return;
    };
    let female = own(&m, "CHARS_M_BODY\\ST_W_Child.ltb").unwrap();
    let has = |l: &Model, n: &str| l.animations.iter().any(|a| a.name.eq_ignore_ascii_case(n));
    assert!(has(&male, "Insult") && has(&male, "Salute"));
    for n in ["DHB_1", "NS1", "NS2"] {
        assert!(!has(&female, n), "{n}");
    }
    for n in ["URHRAmS", "UBKRAmS", "USGRFiS1", "UGGRAmS", "URWRAmS", "UFSRAmS", "UP9RAmS"] {
        assert!(has(&male, n) && has(&female, n), "{n}");
    }
}

fn inputs(class: i32, legs: i32) -> Inputs {
    Inputs { class, legs, ..Default::default() }
}

/// 87, 88 on the install's libraries, through our families and the
/// driver as a player's character runs them (`CaAnim`); and one body per
/// family loads as a character model (`models::load_body`: its pieces
/// and skins) and poses its hit spheres.
#[test]
fn t87_88_bodies_play_and_load() {
    let Some(m) = mount() else { return };
    let Some(ch) = install_characters() else { return };
    let file = |p: &str| m.read(p).ok();
    if model::load_own("CHARS_M_BODY\\VIM_F_NM_DF_Tanker_SH.ltb", &file).is_err() {
        eprintln!("skipping: the body files need the V2 key");
        return;
    }
    let npc = |name: &str| ch.model.iter().find(|r| r.npc.is_some() && r.name == name).unwrap();
    let anim_of = |r: &records::ModelRecord| {
        let l = model::load(&r.file, &file).unwrap();
        let rig = std::sync::Arc::new(Rig::new(l.model, l.list));
        CaAnim::with_family(r.id, r.skeleton, Some(rig), ch.family(r), &r.name)
    };
    let play = |a: &mut CaAnim, i: &Inputs, n: usize| {
        for _ in 0..n {
            let (class, legs) = a.family.played(&a.record, i.class, i.legs);
            a.driver.update(15, &Inputs { class, legs, ..i.clone() });
        }
    };
    // 88: a tanker with an M4A1 (class 0).
    let mut t = anim_of(npc("Heavy_Infecter_Gas_A"));
    play(&mut t, &inputs(0, 0), 3);
    assert_eq!((t.playing().0, t.playing().1), ("VUMRAmS", "VLST"));
    play(&mut t, &Inputs { fire: true, ..inputs(0, 0) }, 2);
    assert!(t.playing().0.eq_ignore_ascii_case("VUMRFIS"));
    play(&mut t, &Inputs { forward: true, ..inputs(0, 0) }, 2);
    assert_eq!(t.playing().1, "VLRF");
    play(&mut t, &inputs(0, 0), 2);
    play(&mut t, &Inputs { jumped: true, ..inputs(0, 0) }, 1);
    assert_eq!(t.playing().1, "VLST", "no VLSJ");
    play(&mut t, &Inputs { status: Some(Status::Hit), damaged: true, ..inputs(0, 0) }, 1);
    assert!(t.playing().0.eq_ignore_ascii_case("VD_HIT"));
    // 87: the NPC SMG suit holding `HQRAIDNPC_2_SMG` (class 2) plays the
    // SMG suit's class; strafing right, the left name.
    let mut s = anim_of(npc("Powered_Suit_Npc_Smg"));
    play(&mut s, &Inputs { right: true, ..inputs(2, 0) }, 3);
    assert_eq!(s.playing().0, "USMGRAmL");
    play(&mut s, &Inputs { forward: true, left: true, ..inputs(2, 0) }, 3);
    // The legs keep the strafe's animation forward-left (no `LRFL`).
    assert_eq!(s.playing().1, "LRR");
    let mut big = anim_of(ch.model(62).unwrap());
    play(&mut big, &inputs(64, 0), 3);
    assert_ne!(big.playing().0, "USMGRAmS");
    // A boss: idle, then its fire.
    let mut f = anim_of(npc("Abnormal_Boss_Fortress"));
    play(&mut f, &inputs(0, 0), 2);
    assert_eq!(f.playing().0, "Idle");
    play(&mut f, &Inputs { fire: true, ..inputs(0, 0) }, 2);
    assert_eq!(f.playing().0, "Gatling_ST");

    // One body per family loads, with its pieces' skins.
    for name in ["Squad_1", "Squad_2", "Heavy_Infecter_Gas_A", "Light_Infecter_Speed_B", "Powered_Suit_Npc_Smg", "Abnormal_Boss", "Abnormal_Boss_Fortress"] {
        let r = npc(name);
        let (c, textures) = models::load_body(r.id, &file).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!c.model.meshes.is_empty() && !textures.is_empty(), "{name}");
        let untextured = c.model.meshes.iter().filter(|m| m.texture.is_none()).count();
        assert!(untextured * 4 <= c.model.meshes.len(), "{name}: {untextured} of {} meshes untextured", c.model.meshes.len());
        let look = ch.default_look(r.id);
        let (want, _) = models::drawn(CaData::install_static(), &look).unwrap_or_else(|| panic!("{name}: not drawn"));
        assert_eq!(want, models::body_name(r.id));
    }
    // The squad's face, cap and gas mask are its group's choices.
    let squad = npc("Squad_1");
    let g = models::loaded_groups().into_iter().find(|g| g.records == [squad.id]).unwrap();
    let n = squad.npc.as_ref().unwrap();
    assert_eq!((g.faces.clone(), g.heads.clone(), g.goggles.clone()), (vec![n.face], vec![-1, n.head], vec![-1, n.goggle]));
    // The tanker poses its table-2 spheres: 45 of its 45 (41 on nodes, 4
    // on sockets).
    let t = anim_of(npc("Heavy_Infecter_Gas_A"));
    let posed = t.posed(0.0, 1.0, false);
    let pose = combat_arms::game::characters::pose_of(&t, &posed, 0.0, 1.0, None, None);
    let table = hit_table(2);
    let live = table.iter().zip(&pose.spheres).filter(|(s, c)| s.radius > 0.0 && c.is_some()).count();
    assert_eq!(live, 45);
}


