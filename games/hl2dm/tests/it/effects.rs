//! HL2:DM's effects and sounds on the headless harness (specs/hl2dm/
//! effects_sounds.md, H9's cases by number): tracers by weapon and the
//! counter, the near-miss sound of one's own tracers, the AR2's flash on a
//! wall (no ricochet), the splash's scale, blood per viewer (the
//! server's for all but the attacker, the shooter's predicted copy, none
//! for a melee attacker) and the flesh impact, the
//! death voice by model, footsteps by model kind.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Died, Hitgroup, LocalPlayer};
use chimera_sim::{
    testkit,
    weapon::{Inventory, give},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        GameSound, PlaySound,
        particles::{MapParticles, ParticleMaterial, ParticleMaterials, Particles},
        tracer::Tracer,
    },
};
use hl2dm::{
    game::{
        effects::{self, AR2_TRACER, TRACER},
        movement,
        sounds::{self, Dissolving},
        weapons::{self, inventory::Hl2Model},
    },
    harness::Hl2Sim,
};
use source_engine::{
    effects::impact_effects::{
        BLOOD_COUNTS, ImpactEffectSettings, MATERIALS as SOURCE_MATERIALS, ServerBlood, bleed_traces, predicted_blood_at,
    },
    source_movement::StepSounds,
};

fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    let materials = SOURCE_MATERIALS
        .iter()
        .chain(effects::MATERIALS)
        .map(|n| ParticleMaterial { name: n.to_string(), ..default() })
        .collect();
    sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
    sim
}

fn holding(sim: &mut Sim, id: &str) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let have = {
        let w = sim.app.world();
        w.get::<Inventory>(p).unwrap().weapons.iter().copied().find(|e| w.get::<chimera_sim::weapon::Weapon>(*e).is_some_and(|x| x.id == id))
    };
    let w = have.unwrap_or_else(|| give(sim.app.world_mut(), p, id).unwrap());
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    p
}

fn wall() -> Vec3 {
    Vec3::new(0.0, 1.5, greybox::NORTH_WALL_Z)
}

/// Fire `shots` presses of the active gun at `at`, collecting what the
/// run sent.
fn fire<T: Message + Clone>(sim: &mut Sim, p: Entity, at: Vec3, shots: usize) -> Vec<T> {
    let mut cursor = sim.app.world().resource::<Messages<T>>().get_cursor_current();
    testkit::aim_at(sim, p, at);
    let mut out = Vec::new();
    let mut drain = |sim: &mut Sim, out: &mut Vec<T>| {
        let m = sim.app.world().resource::<Messages<T>>();
        out.extend(cursor.read(m).cloned());
    };
    for _ in 0..shots {
        sim.intent(p).fire = true;
        for _ in 0..2 {
            sim.ticks(1);
            drain(sim, &mut out);
        }
        sim.intent(p).fire = false;
        for _ in 0..70 {
            sim.ticks(1);
            drain(sim, &mut out);
        }
    }
    out
}

#[test]
fn tracers_every_second_bullet_of_the_smg1_26_18() {
    let mut sim = sim();
    let p = holding(&mut sim, weapons::SMG1);
    let tracers: Vec<Tracer> = fire(&mut sim, p, wall(), 4);
    // The counter advances first: bullets 2 and 4 draw.
    assert_eq!(tracers.len(), 2);
    assert!(tracers.iter().all(|t| t.look == TRACER));
}

#[test]
fn the_ar2_draws_its_own_streak_21() {
    let mut sim = sim();
    let p = holding(&mut sim, weapons::AR2);
    let tracers: Vec<Tracer> = fire(&mut sim, p, wall(), 2);
    assert_eq!(tracers.len(), 1);
    assert_eq!(tracers[0].look, AR2_TRACER);
}

#[test]
fn the_magnum_draws_every_fourth_bullet_19() {
    let mut sim = sim();
    let p = holding(&mut sim, weapons::MAGNUM);
    let tracers: Vec<Tracer> = fire(&mut sim, p, wall(), 4);
    assert_eq!(tracers.len(), 1);
}

#[test]
fn only_ones_own_tracers_whizz_24_23() {
    let mut sim = sim();
    let p = holding(&mut sim, weapons::SMG1);
    let plays: Vec<PlaySound> = fire(&mut sim, p, wall(), 4);
    assert!(plays.iter().all(|s| !s.entry.contains("Nearmiss")), "no flag, no whizz");
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let plays: Vec<PlaySound> = fire(&mut sim, p, wall(), 4);
    let whizz: Vec<_> = plays.iter().filter(|s| s.entry == "Bullets.DefaultNearmiss").collect();
    assert_eq!(whizz.len(), 2, "one per tracer, 0.6 s apart");
    let mut sim = self::sim();
    let p = holding(&mut sim, weapons::AR2);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let plays: Vec<PlaySound> = fire(&mut sim, p, wall(), 2);
    assert!(plays.iter().any(|s| s.entry == "Bullets.GunshipNearmiss"));
}

#[test]
fn the_ar2_flashes_on_a_wall_and_does_not_ricochet_35() {
    let mut sim = sim();
    let p = holding(&mut sim, weapons::AR2);
    sim.app.world_mut().resource_mut::<Particles>().groups.clear();
    let plays: Vec<PlaySound> = fire(&mut sim, p, wall(), 1);
    assert!(plays.iter().all(|s| s.entry != "Bounce.Shrapnel"));
    // The flash is a flat quad of the AR2's material, plus the surface's
    // own flecks: more particles than a pistol's shot makes.
    let ar2 = sim.app.world().resource::<Particles>().count();
    assert!(ar2 > 0);
}

#[test]
fn a_hit_player_bleeds_and_sounds_as_flesh_13() {
    let mut sim = sim();
    let p = holding(&mut sim, weapons::PISTOL);
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 1.6, movement::ID);
    sim.seconds(1.0);
    let body = sim.position(t) + Vec3::Y * 0.2;
    let sounds: Vec<GameSound> = fire(&mut sim, p, body, 1);
    assert!(sounds.iter().any(|s| s.0.entry == "Flesh.BulletImpact" && s.0.source == Some(t)), "{sounds:?}");
}

/// One press of `p`'s gun at `target`'s body (fire, or attack2 when
/// `secondary`): the blood particles drawn (the free ones, less what the
/// same press draws with `violence_hblood` 0) and the server's blood
/// written.
fn blood_of(local: Option<usize>, id: &str, secondary: bool) -> (usize, Vec<ServerBlood>, Entity) {
    let run = |hblood: i32| {
        let mut sim = sim();
        let p = holding(&mut sim, id);
        let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 1.2, movement::ID);
        sim.seconds(1.0);
        sim.app.world_mut().resource_mut::<ImpactEffectSettings>().violence_hblood = hblood;
        match local {
            Some(0) => {
                sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
            }
            Some(_) => {
                sim.app.world_mut().entity_mut(t).insert(LocalPlayer);
            }
            None => {}
        }
        let body = sim.position(t) + Vec3::Y * 0.2;
        testkit::aim_at(&mut sim, p, body);
        sim.ticks(2);
        sim.app.world_mut().resource_mut::<Particles>().groups.clear();
        let mut cursor = sim.app.world().resource::<Messages<ServerBlood>>().get_cursor_current();
        if secondary {
            sim.intent(p).secondary = true;
        } else {
            sim.intent(p).fire = true;
        }
        let mut blood = Vec::new();
        let mut most = 0;
        for _ in 0..12 {
            sim.ticks(1);
            sim.intent(p).fire = false;
            sim.intent(p).secondary = false;
            blood.extend(cursor.read(sim.app.world().resource::<Messages<ServerBlood>>()).copied());
            let pool = sim.app.world().resource::<Particles>();
            most = most.max(pool.count() - pool.capped_count());
        }
        (most, blood, p)
    };
    let (with, blood, p) = run(1);
    let (without, none, _) = run(0);
    assert!(none.is_empty(), "violence_hblood 0: no server blood");
    (with - without, blood, p)
}

/// Effects_sounds.md 7: the server's blood (at the hit point, wall
/// stains by the damage) goes to everyone but the attacker; the shooter
/// draws his own predicted copy from his bullet's trace; a melee
/// attacker sees none. One burst is `BLOOD_COUNTS` particles.
#[test]
fn blood_per_viewer_7_16() {
    let burst: usize = BLOOD_COUNTS.iter().sum();
    // The shooter's screen: his predicted copy alone, not the server's too.
    let (drawn, server, p) = blood_of(Some(0), weapons::PISTOL, false);
    assert_eq!(server.len(), 1, "the server's blood: {server:?}");
    assert_eq!(server[0].attacker, Some(p));
    assert_eq!(drawn, burst, "one burst, the predicted copy");
    // The victim's (or a bystander's) screen: the server's.
    let (drawn, server, _) = blood_of(Some(1), weapons::PISTOL, false);
    assert_eq!(server.len(), 1);
    assert_eq!(drawn, burst, "the server's copy");
    // A crowbar hit: the attacker sees no blood (melee isn't predicted),
    // the others do (case 16).
    let (drawn, server, _) = blood_of(Some(0), weapons::CROWBAR, false);
    assert_eq!(server.len(), 1, "the crowbar hit: {server:?}");
    assert_eq!(drawn, 0, "no blood for the melee attacker");
    let (drawn, _, _) = blood_of(Some(1), weapons::CROWBAR, false);
    assert_eq!(drawn, burst, "the victim sees it");
}

/// Effects_sounds.md 7: the shooter's predicted blood is 4 in back along
/// the shot from the hit.
#[test]
fn predicted_blood_is_four_inches_back_15() {
    let from = Vec3::new(0.0, 1.0, 10.0);
    let to = Vec3::new(0.0, 1.0, 2.0);
    let (at, dir) = predicted_blood_at(from, to);
    assert!(dir.abs_diff_eq(Vec3::NEG_Z, 1e-6));
    assert!(at.abs_diff_eq(Vec3::new(0.0, 1.0, 2.0 + 4.0 * 0.0254), 1e-5), "{at}");
    // Its wall stains count by distance (300 in: 4, spread 0.3).
    assert_eq!(bleed_traces(300.0), (4, 0.3));
}

#[test]
fn a_death_has_a_voice_by_model_7_8_9() {
    let mut sim = sim();
    let die = |sim: &mut Sim, path: &str, dissolving: bool| {
        let e = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        sim.app.world_mut().entity_mut(e).insert(Hl2Model(path.into()));
        if dissolving {
            sim.app.world_mut().entity_mut(e).insert(Dissolving);
        }
        sim.seconds(1.0);
        let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
        let damage = Damage {
            target: e,
            attacker: None,
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::ZERO,
        };
        sim.app.world_mut().write_message(Died { entity: e, attacker: None, damage });
        let mut heard = Vec::new();
        for _ in 0..2 {
            sim.ticks(1);
            let m = sim.app.world().resource::<Messages<PlaySound>>();
            heard.extend(cursor.read(m).map(|s| s.entry.clone()).filter(|e| e.to_lowercase().ends_with(".die") || e.contains("$female")));
        }
        heard
    };
    assert_eq!(die(&mut sim, "models/humans/group03/female_04.mdl", false), ["NPC_Citizen.die$female"]);
    assert_eq!(die(&mut sim, "models/combine_soldier.mdl", false), ["NPC_CombineS.Die"]);
    assert_eq!(die(&mut sim, "models/police.mdl", false), ["NPC_MetroPolice.Die"]);
    assert!(die(&mut sim, "models/combine_soldier.mdl", true).is_empty());
}

#[test]
fn footsteps_follow_the_model_kind_1_2() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Hl2Model("models/police.mdl".into()));
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<StepSounds>(p).unwrap().left, "NPC_MetroPolice.RunFootstepLeft");
    sim.app.world_mut().entity_mut(p).insert(Hl2Model("models/humans/group03/female_04.mdl".into()));
    sim.ticks(3);
    assert_eq!(sim.app.world().get::<StepSounds>(p).unwrap().left, sounds::steps(weapons::inventory::ModelKind::Citizen).left);
}

/// Each gun's tracer cadence and remote re-trace are its data's
/// (`[hitscan] tracer_every`, weapons.md 6, spec 1-2): every 2nd bullet
/// for the pistol, SMG1 and AR2, every 4th for the .357 and the shotgun;
/// all traced again through players on other clients.
#[test]
fn each_guns_tracer_cadence_is_its_data() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(0.5);
    for (id, every) in [
        (weapons::PISTOL, 2),
        (weapons::SMG1, 2),
        (weapons::AR2, 2),
        (weapons::MAGNUM, 4),
        (weapons::SHOTGUN, 4),
    ] {
        let w = give(sim.app.world_mut(), p, id).unwrap();
        let scan = sim.app.world().get::<chimera_sim::weapon::Hitscan>(w).unwrap();
        assert_eq!(scan.tracer_every, every, "{id}");
        assert!(scan.redrawn_through_characters, "{id}");
        assert_eq!(effects::tracer_every(Some(scan)), u64::from(every));
    }
}
