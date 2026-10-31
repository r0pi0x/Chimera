//! NPC and boss bodies worn by players (specs/combat_arms/characters.md
//! 12 and 13; the logic cases, and the data paths on hand-written
//! attribute documents and stand-in libraries). The install's numbers
//! (cases 62-86, 89) are `heavy::npc_bodies`.
//!
//! Stand-in body files here have names no install body has, and no
//! document here has `Skeleton#` records: the hit tables and rigs are
//! process-wide (kept per index and per file), shared with the heavy
//! tests of this binary.

use std::{collections::BTreeSet, sync::Arc};

use bevy::prelude::*;
use combat_arms::game::{
    characters::{
        driver::{self, Direction, Driver, Inputs, Movement, Posture, Status},
        families::{self, Families},
        records::{self, CharacterData},
        skeleton::Rig,
    },
    data::{
        CaData, InstallAttributes,
        attributes::{AttachmentsFile, ModelsFile, SkeletonNode},
    },
    rungs,
};
use lithtech_formats::{
    anim::AnimList,
    attributes::Document,
    ltb::{self, Model, Node, WeightSet},
};

const TICK: u32 = 15;

fn doc(text: &str) -> Document {
    Document::parse(text.as_bytes()).unwrap()
}

/// A models file and an attachments file by hand: two bodies, two faces
/// (the second named as an `AIFace` is), head pieces, goggles, and four
/// NPC records.
fn npc_data() -> CaData {
    let models = doc(
        r#"[Model0] Name="Normal" ModelFile="CHARS_M_BODY\CM_VST_NM_DF_Nuts_JS.ltb" Skeleton=0 UpperSkin0="a.dtx" LowerSkin0="b.dtx"
        [Model1] Name="Light" ModelFile="CHARS_M_BODY\Light_vest.ltb" Skeleton=0 UpperSkin0="a.dtx" LowerSkin0="b.dtx"
        [Face0] Name="Face_A" ModelFile="CHARS_M_FACE\Face_A.ltb"
        [Face1] Name="Dup" ModelFile="CHARS_M_FACE\Dup_player.ltb"
        [Hair0] Name="HBasic"
        [Hair1] Name="Combat_Helmet" ModelFile0="CHARS_M_HEAD\Combat_Helmet.ltb" ModelFile1="CHARS_M_HEAD\W_Combat_Helmet.ltb"
        [Hair2] Name="Cap" ModelFile0="CHARS_M_HEAD\CAP.ltb" ModelFile1="CHARS_M_HEAD\W_CAP.ltb"
        [AIFace0] Name="MT_RF_FACE" ModelFile="CHARS_M_FACE\MT_NO_FACE.ltb"
        [AIFace1] Name="Dup" ModelFile="CHARS_M_FACE\Dup_ai.ltb"
        [AIModel0] Name="Squad_1" ModelFile="TEST\squad.ltb" Skeleton=0 Skin0="TEST\s0.dtx" Skin1="TEST\s1.dtx" Skin2="TEST\s2.dtx"
            AIFace="mt_rf_face" AIHair="Cap" AIGoggle="Chemical_Gassmask" ClientFX0="Eye_Glow"
        [AIModel1] Name="Heavy_Infecter_Gas_A" ModelFile="TEST\tanker.ltb" Skeleton=2 Skin0="TEST\t.dtx" AttachFace=0 AIFace="Dup"
            UseSpecialDeathAni="VDIE"
        [AIModel2] Name="Uses_Dup" ModelFile="TEST\m.ltb" Skeleton=0 AIFace="DUP"
        [AIModel3] Name="SH_Sandhog" ModelFile="TEST\boss_model.ltb" Skeleton=1
        [AIModel4] Name="Powered_Suit_Npc_Smg" ModelFile="TEST\suit.ltb" Skeleton=5 AIOnly=0 HumanModel=0"#,
    );
    let attachments = doc(
        r#"[Goggle0] Name="Glass" Socket="Eyes" Model0="ATTACH_M\Glass.ltb"
        [Goggle1] Name="Chemical_Gassmask" Socket="Eyes" Model0="ATTACH_M\Gas.ltb""#,
    );
    CaData::with_attributes(InstallAttributes {
        models: Some(ModelsFile::read(&models)),
        attachments: Some(AttachmentsFile::read(&attachments)),
        ..default()
    })
}

/// 13.2: the `AIModel#` records join the bodies after the `Model#` ones
/// (id = the `Model#` count + the index), their skins in slot order, the
/// face found among the `Face#` records first, then the `AIFace#` ones
/// (case ignored; `AttachFace` 0: none), the head piece and goggles by
/// name, the defaults (`AIFace0`... matching nothing: none); the
/// `AIFace#` records join the faces the same way (cases 75, 77 on
/// hand-written data).
#[test]
fn t75_77_npc_records() {
    let ca = npc_data();
    let d = ca.characters();
    let squad = d.model(2).unwrap();
    assert_eq!((squad.name.as_str(), squad.skeleton), ("Squad_1", 0));
    let n = squad.npc.as_ref().unwrap();
    assert_eq!(n.skins, ["TEST\\s0.dtx", "TEST\\s1.dtx", "TEST\\s2.dtx"]);
    // AIFace0 is face 2 (two `Face#` records before it).
    assert_eq!((n.face, n.head, n.goggle, n.backpack), (2, 2, 1, -1));
    assert_eq!(d.face(2).unwrap().file.as_deref(), Some("CHARS_M_FACE\\MT_NO_FACE.ltb"));
    assert!(n.ai_only && n.human);
    assert_eq!(n.client_fx, ["Eye_Glow"]);
    let tanker = d.model(3).unwrap().npc.as_ref().unwrap();
    assert_eq!((tanker.face, tanker.head, tanker.goggle), (-1, -1, -1), "AttachFace 0: no face");
    assert_eq!(tanker.special_death.as_deref(), Some("VDIE"));
    // A name in both lists: the `Face#` record wins.
    assert_eq!(d.model(4).unwrap().npc.as_ref().unwrap().face, 1);
    let suit = d.model(6).unwrap().npc.as_ref().unwrap();
    assert!(!suit.ai_only && !suit.human);
    // The look a rung gives: the record's own pieces.
    let look = d.default_look(2);
    assert_eq!((look.model, look.face, look.head, look.goggle, look.backpack), (2, 2, 2, 1, -1));
    assert_eq!(d.infected_look(3), d.default_look(3), "an NPC body is infected as it is");
    // Never a player body; wearable unless its family isn't (the
    // gunship's body).
    assert!(d.rung_bodies().iter().all(|&b| d.model(b).unwrap().npc.is_none()));
    assert_eq!(d.npc_bodies(), [2, 3, 4, 6]);
}

/// An NPC body's head pieces by its player tree; its skins have no team
/// mark, camouflage or colour: slot i is `Skin{i}` whatever the family
/// (13.2, 13.8).
#[test]
fn npc_pieces_by_tree() {
    let ca = npc_data();
    let d = ca.characters();
    let squad = d.model(2).unwrap();
    assert_eq!(d.piece_skeleton(squad), 0);
    assert_eq!(d.head_file(2, d.piece_skeleton(squad)), Some("CHARS_M_HEAD\\CAP.ltb"));
    assert_eq!(d.family(d.model(3).unwrap()).name, "tanker");
}

/// 13.3, 13.4: a body's family by its file, else its hit table: the
/// female body on the male table is the female's (Quirk 12), `GXL-4209`
/// on table 7 is its own boss (not the EOD boss's), the gunship and the
/// dummy aren't wearable.
#[test]
fn families_by_file_then_table() {
    let f = families::families();
    let of = |file: &str, table: usize| f.of(file, table).name.clone();
    assert_eq!(of("CHARS_M_BODY\\MT_AR_BODY.ltb", 0), "male");
    assert_eq!(of("CHARS_M_BODY\\CW_Body_HV_NX_YK.ltb", 1), "female");
    assert_eq!(of("CHARS_M_BODY\\Light_W_vest.ltb", 0), "female");
    assert_eq!(of("CHARS_M_BODY\\CM_Body_Infected_Ship_Captain_KE.ltb", 2), "tanker");
    assert_eq!(of("CHARS_M_BODY\\CW_VST_TheShining.ltb", 3), "assassin");
    assert_eq!(of("CHARS_M_BODY\\Suit_small_Base.ltb", 5), "small_suit");
    assert_eq!(of("CHARS_M_BODY\\Suit_BIG_Base.ltb", 6), "big_suit");
    assert_eq!(of("CHARS_M_BODY\\PV_CabinFever2_EOD_Boss_KE.ltb", 7), "eod_boss");
    assert_eq!(of("CHARS_M_BODY\\GXL-4209.ltb", 7), "gxl_4209");
    assert_eq!(of("CHARS_M_BODY\\boss_model.ltb", 1), "pattern_boss");
    assert!(!f.of("CHARS_M_BODY\\boss_model.ltb", 1).wearable);
    assert!(!f.of("CHARS_M_BODY\\Body_Tutorial_Dummy_KI.ltb", 8).wearable);
    // The player trees pose on the stock rigs; the others on their own.
    assert_eq!(f.of("x", 0).stock_rig, Some(0));
    assert_eq!(f.of("x", 1).stock_rig, Some(1));
    assert_eq!(f.of("x", 2).stock_rig, None);
}

/// A library of stand-in entries.
fn library(names: &[&str]) -> Arc<AnimList> {
    let mut list = AnimList::default();
    list.offsets.push(Some(0));
    for n in names {
        list.entries.push(driver::stand_in(n, 600, 0));
    }
    Arc::new(list)
}

fn on(family: &str, names: &[&str]) -> Driver {
    let mut d = Driver::new(library(names));
    d.family = Some(families::families().families.get(family).unwrap());
    d
}

fn at(d: &Driver) -> (String, String) {
    (d.name_of(&d.upper).into(), d.name_of(&d.lower).into())
}

fn ticks(d: &mut Driver, i: &Inputs, n: usize) {
    for _ in 0..n {
        d.update(TICK, i);
    }
}

/// 87: the robot classes. SMG suit aiming while strafing right plays the
/// left name (Quirk 15); the pistol suit aims on the SMG suit's names and
/// fires its own; the big suit with the launcher (class 64) aiming still
/// starts nothing new (its aim names are the SMG suit's, absent); moving
/// forward-left the legs keep their animation (no diagonals).
#[test]
fn t87_powered_suits() {
    assert_eq!(driver::upper_names(58, Posture::Aim, Direction::R).0, "USMGRAmL");
    assert_eq!(driver::upper_names(58, Posture::Aim, Direction::R).1.as_deref(), Some("USMGWAmL"));
    assert_eq!(driver::upper_names(59, Posture::Aim, Direction::None).0, "USMGRAmS");
    assert_eq!(driver::upper_names(59, Posture::Fire, Direction::None).0, "UPRFiS");
    assert_eq!(driver::upper_names(59, Posture::Reload, Direction::F), ("UPRReL".into(), Some("UPWReS".into()), Some("UPRRReS".into())));
    assert_eq!(driver::upper_names(61, Posture::AltFire, Direction::L).0, "UMERFiL1");
    assert_eq!(driver::upper_names(62, Posture::AltFire, Direction::None).0, "UMERAmS", "the medic's alt-fire: the melee suit's aim");
    assert_eq!(driver::upper_names(64, Posture::Aim, Direction::None).0, "USMGRAmS");
    assert_eq!(driver::upper_names(63, Posture::Hit, Direction::None).0, "D_HIT");

    let small = ["Base", "LST", "LRF", "LRL", "USMGRAmS", "USMGRAmL", "UPRFiS", "D_HIT"];
    let mut d = on("small_suit", &small);
    let pistol = Inputs { class: 59, ..default() };
    ticks(&mut d, &pistol, 2);
    assert_eq!(at(&d), ("USMGRAmS".into(), "LST".into()));
    ticks(&mut d, &Inputs { fire: true, ..pistol.clone() }, 2);
    assert_eq!(at(&d).0, "UPRFiS");
    let mut d = on("small_suit", &small);
    ticks(&mut d, &Inputs { class: 58, right: true, ..default() }, 2);
    assert_eq!(at(&d).0, "USMGRAmL", "right on the left name");
    // Forward-left: no `LRFL`; the legs keep the strafe's `LRL`.
    ticks(&mut d, &Inputs { class: 58, left: true, ..default() }, 2);
    assert_eq!(at(&d).1, "LRL");
    ticks(&mut d, &Inputs { class: 58, left: true, forward: true, ..default() }, 2);
    assert_eq!(at(&d).1, "LRL", "no diagonal on a suit: the legs keep theirs");

    let mut d = on("big_suit", &["Base", "LST", "UFGRFiS", "UMGRAmS"]);
    ticks(&mut d, &Inputs { class: 64, fire: true, ..default() }, 2);
    assert_eq!(at(&d).0, "UFGRFiS");
    ticks(&mut d, &Inputs { class: 64, ..default() }, 3);
    assert_eq!(at(&d).0, "UFGRFiS", "no `USMGRAmS` on the big suit: no new upper animation");
}

/// 13.8, Open question 36: a gun of no suit class on a suit plays the SMG
/// suit's class (the melee record the melee suit's); the big suit's own
/// fallback is its machine gun's (ours); suit classes play themselves.
#[test]
fn suits_with_other_guns() {
    let f = families::families();
    let small = f.families.get("small_suit").unwrap();
    assert_eq!(small.played("Powered_Suit_Npc_Smg", 2, 0), (58, 0));
    assert_eq!(small.played("powered_suit_npc_melee", 2, 0), (61, 0));
    assert_eq!(small.played("Powered_Suit_SR", 60, 0), (60, 0));
    let big = f.families.get("big_suit").unwrap();
    assert_eq!(big.played("Powered_Suit_Boss", 28, 0), (63, 0));
    assert_eq!(big.played("Powered_Suit_Boss", 64, 0), (64, 0));
    let male = f.families.get("male").unwrap();
    assert_eq!(male.played("SH_Rifle", 0, 0), (0, 0));
}

/// 88: a player-driven tanker holding an M4A1 (class 0): the hand class
/// 39 and the `V` legs: standing `VUMRAmS`, firing `VUMRFIS` (names
/// found without case), running forward `VLRF`; a standing jump starts
/// no new legs (`VLSJ` absent); hits `VD_HIT` (13.8).
#[test]
fn t88_tanker_with_a_rifle() {
    let tanker = families::families().families.get("tanker").unwrap();
    let (class, legs) = tanker.played("Heavy_Infecter_Gas_A", 0, 0);
    assert_eq!((class, legs), (39, 1));
    assert_eq!(driver::upper_names(39, Posture::Aim, Direction::None).0, "VUMRAmS");
    assert_eq!(driver::upper_names(39, Posture::Fire, Direction::F).0, "VUMRFiS", "a hand's fire is the standing one");
    assert_eq!(driver::upper_names(39, Posture::AltFire, Direction::None).0, "VUMRFiS2");
    assert_eq!(driver::upper_names(47, Posture::Aim, Direction::None).0, "VGUMRAmS");
    assert_eq!(driver::leg_names(1, Movement::Run, Direction::F).0.as_deref(), Some("VLRF"));

    let names = ["BaseAnim", "VLST", "VLRF", "VLJS", "VUMRAmS", "VUMRFIS", "VD_hit", "VDIE"];
    let mut d = on("tanker", &names);
    let i = Inputs { class, legs, ..default() };
    ticks(&mut d, &i, 2);
    assert_eq!(at(&d), ("VUMRAmS".into(), "VLST".into()));
    ticks(&mut d, &Inputs { fire: true, ..i.clone() }, 2);
    assert_eq!(at(&d).0, "VUMRFIS");
    ticks(&mut d, &Inputs { forward: true, ..i.clone() }, 2);
    assert_eq!(at(&d).1, "VLRF");
    let mut d = on("tanker", &names);
    ticks(&mut d, &i, 2);
    ticks(&mut d, &Inputs { jumped: true, ..i.clone() }, 1);
    assert_eq!(at(&d).1, "VLST", "no `VLSJ`: no new leg animation");
    let mut d = on("tanker", &names);
    ticks(&mut d, &i, 2);
    ticks(&mut d, &Inputs { status: Some(Status::Hit), damaged: true, ..i.clone() }, 1);
    assert_eq!(at(&d).0, "VD_hit");
}

/// 13.8: a boss body on its minimal mapping, one name on both trackers:
/// standing the idle, moving the walk, fire held its start, loop and
/// end, alt-fire the melee, a hit the hit; the death its own.
#[test]
fn boss_mapping() {
    let names = ["ilde", "Walk", "Attack_L_ST", "Attack_L_LP", "Attack_L_ED", "Foot_Attack", "Damage_ST", "Death", "AL"];
    let mut d = on("eod_boss", &names);
    let i = Inputs::default();
    ticks(&mut d, &i, 2);
    assert_eq!(at(&d), ("ilde".into(), "ilde".into()));
    ticks(&mut d, &Inputs { back: true, ..i.clone() }, 2);
    assert_eq!(at(&d), ("Walk".into(), "Walk".into()));
    ticks(&mut d, &Inputs { fire: true, ..i.clone() }, 1);
    assert_eq!(at(&d).0, "Attack_L_ST");
    // 600 ms each (the stand-ins').
    ticks(&mut d, &Inputs { fire: true, ..i.clone() }, 45);
    assert_eq!(at(&d), ("Attack_L_LP".into(), "Attack_L_LP".into()));
    assert!(d.upper.looping);
    ticks(&mut d, &Inputs { fire: true, ..i.clone() }, 60);
    assert_eq!(at(&d).0, "Attack_L_LP", "looping while held");
    ticks(&mut d, &i, 1);
    assert_eq!(at(&d).0, "Attack_L_ED");
    ticks(&mut d, &i, 45);
    assert_eq!(at(&d).0, "ilde");
    ticks(&mut d, &Inputs { alt_fire: true, ..i.clone() }, 1);
    assert_eq!(at(&d).0, "Foot_Attack");
    ticks(&mut d, &i, 45);
    ticks(&mut d, &Inputs { status: Some(Status::Hit), damaged: true, ..i.clone() }, 1);
    assert_eq!(at(&d).0, "Damage_ST");
    let eod = families::families().families.get("eod_boss").unwrap();
    assert_eq!(eod.boss.as_ref().unwrap().death.as_deref(), Some("Death"));

    // One fire name loops while held; a boss without a walk stays idle.
    let mut d = on("callus", &["Idle", "Breed", "Hit", "Die"]);
    ticks(&mut d, &Inputs { forward: true, ..i.clone() }, 2);
    assert_eq!(at(&d).0, "Idle");
    ticks(&mut d, &Inputs { fire: true, ..i.clone() }, 2);
    assert_eq!(at(&d).0, "Breed");
    assert!(d.upper.looping);
}

/// A node-only model for `Rig::new`.
fn tiny_model() -> Model {
    Model {
        version: 25,
        counts: Default::default(),
        command: String::new(),
        radius: 50.0,
        pieces: Vec::new(),
        nodes: vec![Node {
            name: "Scene Root".into(),
            stored_index: 0,
            flags: 0,
            bind: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            parent: None,
        }],
        weight_sets: vec![WeightSet {
            name: "Upper".into(),
            weights: vec![1.0],
        }],
        children: Vec::new(),
        animations: Vec::new(),
        sockets: Vec::new(),
        bindings: Vec::new(),
    }
}

fn entry(name: &str, dims: ltb::Vec3, root: f32) -> lithtech_formats::anim::Entry {
    let mut e = driver::stand_in(name, 1000, 0);
    e.dims = dims;
    e.translation = [0.0, root, 0.0];
    e
}

/// 13.6 and Open question 35 (ours): a body with the player's `LST` and
/// `LC` stands on their boxes (a small suit: 100 and 70 u); one without
/// them whose first animation sinks its root deeper than the fallback's
/// 50 u (a boss) has its feet there, standing and crouched; else the
/// fallback (the infected bodies: 50 and 40).
#[test]
fn feet_of_big_bodies() {
    let rig = |entries: Vec<lithtech_formats::anim::Entry>| {
        let mut list = AnimList::default();
        list.offsets.push(Some(0));
        list.entries = entries;
        Rig::new(tiny_model(), list)
    };
    let suit = rig(vec![entry("Base", [80.0, 100.0, 50.0], -100.0), entry("LST", [80.0, 100.0, 50.0], -100.0), entry("LC", [80.0, 70.0, 50.0], -70.0)]);
    assert_eq!(suit.half_heights, (100.0, 70.0));
    let eod = rig(vec![entry("ilde", [70.0, 100.0, 55.0], -100.0), entry("Walk", [70.0, 100.0, 55.0], -100.0)]);
    assert_eq!(eod.half_heights, (100.0, 100.0));
    let tanker = rig(vec![entry("BaseAnim", [15.0, 50.0, 15.0], -50.0), entry("VLST", [15.0, 50.0, 15.0], -50.0)]);
    assert_eq!(tanker.half_heights, (50.0, 40.0));
}

/// An install hit table's spheres (13.6, 11): by name, no measured place;
/// a sphere with radius 0 is kept but never hit; the location and factor
/// as written (L0 counts as the torso).
#[test]
fn install_hit_tables() {
    let nodes = SkeletonNode::read_all(doc(
        r#"[Skeleton0] Node0Name="Head" Node0Radius=5.5 Node0Location=1 Node0DamageFactor=3.0
        Node1Name="_zN_LowerBody" Node1Radius=8.0 Node1Location=0 Node1DamageFactor=1.0
        Node2Name="Left_hand" Node2Radius=0.0 Node2Location=3 Node2DamageFactor=1.0"#,
    )
    .records("Skeleton")[0]);
    let t = records::install_table(2, &nodes);
    assert_eq!(t.spheres.len(), 3);
    assert_eq!((t.spheres[0].location, t.spheres[0].factor, t.spheres[0].radius), (1, 3.0, 5.5));
    assert!(t.spheres.iter().all(|s| s.at.is_none() && !s.absent));
    assert!(t.aim.is_empty(), "the aim is the family's");
}

/// 12.4, 12.6 (the user's decisions of 2026-10-31): who may hold the NPC
/// and boss guns: the `AIModel`s the worlds give them, the suit weapons
/// their suits, the HQ raid boss's guns our stand-in (the big suit), the
/// infected hands their own type and every other one the worlds give
/// them; guns with no link keep the derived rule unless the spec guesses.
#[test]
fn t62_74_holders() {
    let h = rungs::holders();
    let of = |w: &str| -> BTreeSet<String> { h.of(&format!("combat_arms:{w}"), None).into_iter().collect() };
    let set = |names: &[&str]| -> BTreeSet<String> { names.iter().map(|s| s.to_string()).collect() };
    let sandhog = ["SH_Rifle", "SH_Recon", "SH_MGunner", "SH_Sniper", "SH_HeavyGunner", "SH_RoyalGuard"];
    let mut ak = sandhog.to_vec();
    ak.extend(["Agent_Z", "SubMission_Z_1", "SubMission_Z_2"]);
    // 12.4, L1 (by record name; Squad_n is `AIModel` 13 + n...).
    let l1: &[(&str, Vec<&str>)] = &[
        ("ak-47_npc", ak),
        ("imi_uzi_npc", sandhog.to_vec()),
        ("m24_npc", vec!["SH_Recon", "SH_Sniper", "SH_RoyalGuard"]),
        ("m60_npc", vec!["SH_Recon", "SH_MGunner", "SH_RoyalGuard"]),
        ("law_npc", vec!["SH_HeavyGunner"]),
        ("dynamite_npc", vec!["SH_Recon", "SH_MGunner", "SH_Sniper", "SH_HeavyGunner", "SH_RoyalGuard"]),
        ("vh_normal_npc", vec!["Normal_Infecter_Common_D"]),
        ("vh_explode_npc", vec!["Normal_Infecter_Explode_C"]),
        ("vh_heavy_npc", vec!["Heavy_Infecter_Gas_A"]),
        ("vh_light_npc", vec!["Light_Infecter_Speed_B"]),
        ("quarantine_stick_npc", vec!["Normal_Infecter_Common_D"]),
        ("mp5a4_npc", vec!["Squad_3", "Squad_4", "Squad_12"]),
        ("m4a1_npc", vec!["Squad_1", "Squad_2", "Squad_11"]),
        ("p90_npc", vec!["Squad_5", "Squad_6", "Squad_10"]),
        ("m416_npc", vec!["Squad_7", "Squad_8"]),
        ("mg21e_npc", vec!["Squad_9", "Squad_0"]),
        ("hqraidnpc_1_melee", vec!["HQRAIDNPC_1"]),
        ("hqraidnpc_2_smg", vec!["HQRAIDNPC_2", "Powered_Suit_Npc_Melee", "Powered_Suit_Npc_Smg", "SubMission_Z_1", "SubMission_Z_2"]),
        ("hqraidnpc_3_special", vec!["HQRAIDNPC_3"]),
        ("infecteds_captain_hand_boss", vec!["InfectedS_Captain_Boss", "InfectedS_W_Captain_Boss", "InfectedS_Captain"]),
        (
            "abnormal_boss_minigun",
            vec!["Abnormal_Boss", "Abnormal_Du_Boss", "Abnormal_Du_Boss_A", "Abnormal_Boss_Fortress", "Abnormal_Boss_Beetle", "Abnormal_Boss_GroundThorn"],
        ),
        ("vh_normal_npc_cfs", vec!["Normal_Infect_CFS", "Normal_Infect_Carnival_CFS"]),
        ("vh_explode_npc_cfs", vec!["Explode_Infect_CFS", "Explode_Infect_Carnival_CFS"]),
        ("vh_heavy_npc_cfs", vec!["Hauser_Infect_CFS"]),
    ];
    for (w, holders) in l1 {
        assert_eq!(of(w), set(holders), "{w}");
    }
    let mut m67: Vec<&str> = sandhog.to_vec();
    m67.extend(["Agent_Z", "HQRAIDNPC_1", "HQRAIDNPC_2", "HQRAIDNPC_3", "Powered_Suit_Npc_Melee", "Powered_Suit_Npc_Smg"]);
    m67.extend(["Squad_0", "Squad_1", "Squad_2", "Squad_3", "Squad_4", "Squad_5", "Squad_6", "Squad_7", "Squad_8", "Squad_9", "Squad_11", "Squad_12"]);
    assert_eq!(of("m67_npc"), set(&m67));
    // L2: our stand-in. L3: the suit sets.
    for w in ["hqraid_boss_minigun", "hqraid_boss_flame", "hqraid_boss_missile", "hqraid_boss_poisonbomb", "powered_suit_boss_mg", "powered_suit_boss_launcher"] {
        assert_eq!(of(w), set(&["62", "583", "588"]), "{w}");
    }
    assert_eq!(of("powered_suit_basic_smg"), set(&["58", "579", "584"]));
    assert_eq!(of("powered_suit_basic_sr"), set(&["59", "580", "585"]));
    assert_eq!(of("powered_suit_basic_pistol"), set(&["60", "581", "586", "61", "582", "587"]));
    assert_eq!(of("powered_suit_basic_melee"), set(&["60", "581", "586"]));
    assert_eq!(of("powered_suit_basic_medic"), set(&["61", "582", "587"]));
    // L5: the infected mercenaries.
    assert_eq!(of("quarantine_subtypes_hand"), set(&["56"]));
    assert_eq!(of("quarantine_subtypep_hand"), set(&["57"]));
    assert_eq!(of("the_shining_hand"), set(&["513"]));
    // 12.6: own type and the "also" column.
    assert_eq!(
        of("quarantine_host_hand"),
        set(&["@infected", "Heavy_Infecter_Gas_A", "Light_Infecter_Speed_B", "Normal_Infecter_Explode_C", "Normal_Infecter_Common_D"])
    );
    assert_eq!(
        of("infecteds_captain_hand"),
        set(&[
            "InfectedS_Captain",
            "InfectedS_W_Captain",
            "InfectedS_Man01_Host",
            "InfectedS_Man02_Host",
            "InfectedS_W_Man01_Host",
            "InfectedS_Man01_Virus",
            "SubMission_Z_1",
            "Infecter_Abnormal_Suicide"
        ])
    );
    assert_eq!(of("infecteds_new_woman_hand").len(), 4);
    assert_eq!(of("infecteds_man02_host_b_hand"), set(&["InfectedS_Man02_Host_B", "InfectedS_New_Man02_B"]));
    assert!(of("abnormal_normal").contains("Abnormal_Boss_Callus"));
    // The spec's guesses (Open question 26), ours.
    assert_eq!(of("npc_powered_suit_basic_smg"), set(&["Powered_Suit_Npc_Smg"]));
    assert_eq!(of("npc_powered_suit_basic_melee"), set(&["Powered_Suit_Npc_Melee"]));
    assert_eq!(of("abnormal_boss_minigun_left"), of("abnormal_boss_minigun"));
    // No link and no guess: the derived rule (any player body; an
    // infected's hand the infected).
    for w in ["minigun_npc", "watergun_npc", "souzas_parafal_npc", "abnormal_icegas_boss", "assassin_bowgun_nor"] {
        assert_eq!(of(w), set(&["@players"]), "{w}");
    }
    assert_eq!(h.of("combat_arms:mutation_quarantine_virus_hand", Some("infected")), ["@infected"]);
}

/// A rung's NPC look wears the record's pieces; the bodies a name or id
/// resolves to skip what no player may wear (`rungs::bodies_for`).
#[test]
fn rung_bodies_for_npc_names() {
    let ca = npc_data();
    let holders = chimera_sim::minigame::holders::Holders::parse(
        r#"default = "x"
        [groups]
        x = ["@players"]
        [weapons]
        "combat_arms:a" = ["Squad_1", "SH_Sandhog", "Heavy_*"]
        "combat_arms:b" = ["@npcs"]"#,
    )
    .unwrap();
    assert_eq!(rungs::bodies_for(&holders, None, Some(&ca), "combat_arms:a"), [2, 3]);
    assert_eq!(rungs::bodies_for(&holders, None, Some(&ca), "combat_arms:b"), [2, 3, 4, 6]);
}

/// Every name the robot classes' rows give parses into run and walk
/// forms; the data's families all read.
#[test]
fn family_data_reads() {
    let f = Families::parse(families::DATA).unwrap();
    assert!(f.families.len() >= 15);
    assert_eq!(families::forms("UPRReL|UPWReS"), ("UPRReL".into(), "UPWReS".into()));
    assert_eq!(families::forms("USMG{s}AmS"), ("USMGRAmS".into(), "USMGWAmS".into()));
    let _: &CharacterData = CaData::builtin_static().characters();
}
