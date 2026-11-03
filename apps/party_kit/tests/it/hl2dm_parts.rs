//! HL2:DM's own parts in the party apps follow the gun in hand (its
//! bundle, `hl2dm::game::BUNDLE`): a gun game whose ladder has HL2:DM's
//! guns makes their holder HL2:DM's (movement, identity, the crowbar as
//! its knife), each HL2:DM rung draws a player model (citizens and the
//! Combine) whose kind gives the rung's melee weapon, HL2:DM's damage
//! shove moves only an HL2:DM victim, its punch is HL2's spring only
//! while its gun is in hand, and its view models play on Chimera's driver
//! with HL2:DM's rules, motion and field of view.

use bevy::prelude::*;
use chimera_core::{
    core::Velocity,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, MinigameAppExt, ladder::Ladder},
    testkit,
    weapon::{Inventory, Weapon},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::GameViewModelFovs};
use combat_arms::harness::CaSim;
use hl2dm::game::{
    movement as hl2_movement,
    rungs::RungModel,
    weapons::{self as hl2, inventory::Hl2Model},
};
use party_kit::PartyPartsPlugin;

use crate::gun_game::{held, line_up, no_finale, shoot_until_dead, soldier};

/// The ladder of `MIXED`: HL2:DM's pistol first, then CS:S's, Combat
/// Arms' and HL2:DM's again.
pub(crate) const LADDER: [&str; 5] =
    [hl2::PISTOL, "cs_source:weapon_glock", hl2::SMG1, "combat_arms:beretta_m92fs", hl2::SHOTGUN];

/// The party's gun game with `LADDER` and every game's knife.
pub(crate) fn mixed_text(rung_extras: bool) -> String {
    let text = originals::MINIGAMES.iter().find(|(f, _)| f.contains("gun_game_dust2")).unwrap().1;
    let start = text.find("ladder = [").unwrap();
    let end = start + text[start..].find(']').unwrap() + 1;
    let ladder = format!("ladder = [{}]", LADDER.iter().map(|g| format!("\"{g}\"")).collect::<Vec<_>>().join(", "));
    let text = format!("{}{}{}", &text[..start], ladder, &text[end..]);
    text.replace("originals:gun_game_dust2", "test:gun_game_hl2dm")
        .replace("weapons = [\"cs_source\", \"combat_arms\"]", "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]")
        .replace(
            "kit = { cs_source = [\"cs_source:weapon_knife\"], combat_arms = [\"combat_arms:m9_knife\"] }",
            "kit = { cs_source = [\"cs_source:weapon_knife\"], hl2dm = [\"hl2dm:weapon_crowbar\"], combat_arms = [\"combat_arms:m9_knife\"] }",
        )
        .replace("rung_extras = true", &format!("rung_extras = {rung_extras}"))
}

/// The party on the greybox playing `mixed_text`.
fn party(rung_extras: bool) -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    no_finale(sim.app.world_mut());
    sim.app.add_minigames(&[("hl2dm.toml", &mixed_text(rung_extras))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim
}

fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    minigame::ladder::set_rung(w, e, r, &l);
}

fn identity(w: &World, e: Entity) -> Option<String> {
    w.get::<GameIdentity>(e).map(|g| g.0.clone())
}

fn weapons_of(w: &World, e: Entity) -> Vec<&'static str> {
    w.get::<Inventory>(e).map_or(Vec::new(), |i| i.weapons.iter().filter_map(|x| w.get::<Weapon>(*x).map(|x| x.id)).collect())
}

#[test]
fn an_hl2dm_rung_makes_its_holder_hl2dms_with_the_crowbar() {
    let mut sim = party(false);
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let w = sim.app.world();
    assert_eq!(held(&sim, a), Some(hl2::PISTOL));
    assert_eq!(identity(w, a).as_deref(), Some("hl2dm"));
    assert_eq!(w.get::<MovementSlot>(a).map(|m| m.0), Some(hl2_movement::ID));
    assert!(weapons_of(w, a).contains(&hl2::CROWBAR), "{:?}", weapons_of(w, a));
    // CS:S's Glock next: CS:S's again, its knife.
    set_rung(&mut sim, a, 1);
    sim.seconds(1.2);
    let w = sim.app.world();
    assert_eq!(identity(w, a).as_deref(), Some("cs_source"));
    assert!(weapons_of(w, a).contains(&"cs_source:weapon_knife") && !weapons_of(w, a).contains(&hl2::CROWBAR));
    // HL2:DM's SMG1 kills as HL2:DM's.
    set_rung(&mut sim, a, 2);
    sim.seconds(1.2);
    assert_eq!(held(&sim, a), Some(hl2::SMG1));
    assert_eq!(identity(sim.app.world(), a).as_deref(), Some("hl2dm"));
}

#[test]
fn each_hl2dm_rung_draws_a_model_whose_kind_gives_its_melee() {
    let mut sim = party(true);
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let mut kinds = Vec::new();
    for r in [0usize, 2, 4] {
        set_rung(&mut sim, a, r);
        sim.seconds(1.2);
        let w = sim.app.world();
        let model = w.get::<Hl2Model>(a).map(|m| m.0.clone()).expect("an HL2:DM rung's model");
        assert!(w.get::<RungModel>(a).is_some());
        let kind = hl2::inventory::ModelKind::of(&model).expect("one of HL2:DM's player models");
        let melee = if kind == hl2::inventory::ModelKind::Citizen { hl2::CROWBAR } else { hl2::STUNSTICK };
        assert!(weapons_of(w, a).contains(&melee), "{model}: {:?}", weapons_of(w, a));
        let label = ladder.extras[r].as_ref().and_then(|x| x.label.clone());
        assert!(label.is_some(), "the rung's line names the model");
        kinds.push(kind);
    }
    // A rung of another game's gun takes the model off.
    set_rung(&mut sim, a, 1);
    sim.seconds(1.2);
    assert!(sim.app.world().get::<Hl2Model>(a).is_none());
    assert_eq!(kinds.len(), 3);
}

#[test]
fn hl2dms_shove_moves_an_hl2dm_victim_only() {
    // The victim on rung 2 (HL2:DM's SMG1) or 1 (CS:S's Glock); the
    // shooter on rung 0, HL2:DM's pistol.
    let shoved = |victim_rung: usize| -> f32 {
        let mut sim = party(false);
        let a = soldier(&mut sim);
        let b = soldier(&mut sim);
        sim.seconds(1.5);
        set_rung(&mut sim, b, victim_rung);
        sim.seconds(1.2);
        line_up(&mut sim, a, b, 4.0);
        let mut most = 0.0_f32;
        let w0 = sim.app.world().get::<Velocity>(b).unwrap().0.xz().length();
        for t in 0..60u64 {
            let at = sim.position(b) + Vec3::Y * 0.2;
            testkit::aim_at(&mut sim, a, at);
            sim.intent(a).fire = t % 10 == 0;
            sim.ticks(1);
            most = most.max(sim.app.world().get::<Velocity>(b).unwrap().0.xz().length() - w0);
        }
        most / 0.0254
    };
    let hl2 = shoved(2);
    assert!(hl2 > 30.0, "an HL2:DM victim is shoved about 40 units/s a pistol hit: {hl2}");
    let cs = shoved(1);
    assert!(cs < 5.0, "a CS:S victim isn't: {cs}");
}

#[test]
fn hl2dms_view_models_play_with_its_rules_motion_and_field_of_view() {
    let sim = party(false);
    let w = sim.app.world();
    assert!(w.resource::<chimera_sim::weapon::view_anim::GameViewAnimRules>().get("hl2dm").is_some());
    assert!(w.resource::<source_engine::view_motion::GameMotionRules>().get("hl2dm").is_some());
    let fov = w.resource::<GameViewModelFovs>().0.iter().find(|f| f.game == "hl2dm").cloned().unwrap();
    assert_eq!((fov.fov, fov.default_fov), (54.0, 75.0));
}

#[test]
fn an_hl2dm_gun_kills_in_the_party() {
    let mut sim = party(false);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    line_up(&mut sim, a, b, 4.0);
    assert!(shoot_until_dead(&mut sim, a, b, 12.0), "HL2:DM's pistol kills");
    sim.ticks(3);
    assert_eq!(held(&sim, a), Some("cs_source:weapon_glock"), "the next rung's gun");
}
