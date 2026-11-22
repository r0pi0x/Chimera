//! Each game's per-character sounds follow the character's identity (the
//! gun in hand, `chimera_sim::identity`): a player holding a CS:S gun
//! steps, jumps, groans and dies only in CS:S's sounds (Source movement's
//! steps, `cs_source::game::pain`'s `Player.*` entries), one holding a
//! Combat Arms gun only in Combat Arms' (its footsteps, jump and sprint
//! voices, hurt sounds: `CaSound` files). Headless on the party's
//! simulation, the greybox's floor given a CS:S surface with steps.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Dead, God, Hitgroup, LocalPlayer, PropSurface},
    slots::GameIdentity,
};
use chimera_world::{
    greybox::GreyboxPart,
    harness::Sim,
    map::{GameSound, MapSounds, MapSurface, PlaySound, sound::SoundBank},
};
use combat_arms::game::effects::sounds::CaSound;
use cs_source::game::weapons as cs_weapons;

use crate::gun_game::{held, party, soldier};

/// A Combat Arms gun (a pickup, `give`).
const M4: &str = "combat_arms:m4a1";

/// Every sound heard: CS:S's entries (`PlaySound`, `GameSound`) and
/// Combat Arms' files (`CaSound`).
#[derive(Resource, Default)]
struct Heard {
    cs: Vec<String>,
    ca: Vec<String>,
}

fn hear(
    mut play: MessageReader<PlaySound>,
    mut game: MessageReader<GameSound>,
    mut ca: MessageReader<CaSound>,
    mut heard: ResMut<Heard>,
) {
    heard.cs.extend(play.read().map(|s| s.entry.clone()));
    heard.cs.extend(game.read().map(|s| s.0.entry.clone()));
    heard.ca.extend(ca.read().map(|s| s.file.clone()));
}

/// The floor's surface and its steps (Source movement's).
const FLOOR: &str = "concrete";
const STEP: &str = "Concrete.Step";

/// The party with two soldiers (`a` the local player), both holding the
/// first rung's gun (CS:S's Glock), the greybox's solids all `FLOOR`.
fn sim() -> (Sim, Entity, Entity) {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.app.init_resource::<Heard>().add_systems(Last, hear);
    // Past the spawn protection (2 s).
    sim.seconds(2.2);
    let w = sim.app.world_mut();
    let parts: Vec<Entity> = w.query_filtered::<Entity, With<GreyboxPart>>().iter(w).collect();
    for e in parts {
        w.entity_mut(e).insert(PropSurface(FLOOR.into()));
    }
    let mut sounds = MapSounds::default();
    sounds.surfaces.insert(
        FLOOR.into(),
        MapSurface {
            step_left: Some(format!("{STEP}Left")),
            step_right: Some(format!("{STEP}Right")),
            game_material: 'C',
            ..default()
        },
    );
    sim.app.insert_resource(SoundBank(Arc::new(sounds)));
    assert_eq!(held(&sim, a), Some(cs_weapons::GLOCK));
    (sim, a, b)
}

fn identity(sim: &Sim, e: Entity) -> Option<String> {
    sim.app.world().get::<GameIdentity>(e).map(|i| i.0.clone())
}

fn clear(sim: &mut Sim) {
    *sim.app.world_mut().resource_mut::<Heard>() = Heard::default();
}

fn count(list: &[String], what: impl Fn(&str) -> bool) -> usize {
    list.iter().filter(|s| what(s)).count()
}

/// Combat Arms' per-character sounds: steps, voices (jump, sprint, kill),
/// hurt sounds.
fn ca_character(file: &str) -> bool {
    file.contains("footsteps") || file.starts_with("chars_snd/")
}

/// Run forward and back (sprinting), then jump and land.
fn walk_and_jump(sim: &mut Sim, e: Entity) {
    for axis in [Vec2::Y, Vec2::NEG_Y] {
        {
            let mut i = sim.intent(e);
            i.move_axis = axis;
            i.sprint = true;
        }
        sim.seconds(0.8);
    }
    {
        let mut i = sim.intent(e);
        i.move_axis = Vec2::ZERO;
        i.sprint = false;
        i.jump = true;
    }
    sim.ticks(2);
    sim.intent(e).jump = false;
    sim.seconds(1.2);
}

/// `e` holds `gun` (given and drawn).
fn hold(sim: &mut Sim, e: Entity, gun: &'static str) {
    chimera_sim::weapon::give(sim.app.world_mut(), e, gun);
    sim.seconds(1.0);
    assert_eq!(held(sim, e), Some(gun));
}

#[test]
fn steps_and_voices_are_the_held_guns_games() {
    let (mut sim, a, _b) = sim();
    assert_eq!(identity(&sim, a).as_deref(), Some("cs_source"));
    clear(&mut sim);
    walk_and_jump(&mut sim, a);
    let heard = sim.app.world().resource::<Heard>();
    assert!(count(&heard.cs, |s| s.starts_with(STEP)) > 0, "CS:S steps with the Glock: {:?}", heard.cs);
    assert_eq!(
        count(&heard.ca, ca_character),
        0,
        "no Combat Arms steps or voices with the Glock: {:?}",
        heard.ca
    );

    hold(&mut sim, a, M4);
    assert_eq!(identity(&sim, a).as_deref(), Some("combat_arms"));
    clear(&mut sim);
    walk_and_jump(&mut sim, a);
    let heard = sim.app.world().resource::<Heard>();
    assert!(count(&heard.ca, |f| f.contains("footsteps")) > 0, "Combat Arms steps with the M4: {:?}", heard.ca);
    assert!(count(&heard.ca, |f| f.starts_with("chars_snd/jump")) > 0, "its jump voice: {:?}", heard.ca);
    assert!(count(&heard.ca, |f| f.starts_with("chars_snd/male/breath")) > 0, "its sprint voice: {:?}", heard.ca);
    assert_eq!(count(&heard.cs, |s| s.starts_with(STEP)), 0, "no CS:S steps with the M4: {:?}", heard.cs);
}

/// A bullet in `e`'s head for `amount` (normalized health), no attacker.
fn headshot(sim: &mut Sim, e: Entity, amount: f32) {
    let at = sim.position(e);
    sim.app.world_mut().write_message(Damage {
        target: e,
        attacker: None,
        amount,
        point: at + Vec3::Y * 0.7,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.seconds(0.3);
}

fn dead(sim: &Sim, e: Entity) -> bool {
    sim.app.world().get::<Dead>(e).is_some()
}

#[test]
fn hurt_and_death_sounds_are_the_victims_games() {
    let (mut sim, _a, b) = sim();
    let cs_voice = |s: &str| s.starts_with("Player.");
    let ca_hurt = |f: &str| f.starts_with("chars_snd/male/hurt");

    // CS:S's Glock: CS:S's groan and death cry only.
    assert_eq!(identity(&sim, b).as_deref(), Some("cs_source"));
    clear(&mut sim);
    headshot(&mut sim, b, 0.2);
    let heard = sim.app.world().resource::<Heard>();
    assert!(heard.cs.iter().any(|s| s == "Player.DamageHeadShot"), "CS:S's hurt sound: {:?}", heard.cs);
    assert_eq!(count(&heard.ca, ca_hurt), 0, "no Combat Arms hurt sound: {:?}", heard.ca);

    // Combat Arms' M4: its hurt sounds only, and no death cry (it has none).
    hold(&mut sim, b, M4);
    assert_eq!(identity(&sim, b).as_deref(), Some("combat_arms"));
    clear(&mut sim);
    headshot(&mut sim, b, 0.2);
    let heard = sim.app.world().resource::<Heard>();
    assert!(count(&heard.ca, ca_hurt) > 0, "Combat Arms' hurt sounds: {:?}", heard.ca);
    assert_eq!(count(&heard.cs, cs_voice), 0, "no CS:S hurt sound: {:?}", heard.cs);
    sim.seconds(1.2);
    clear(&mut sim);
    headshot(&mut sim, b, 5.0);
    assert!(dead(&sim, b), "the headshot kills");
    let heard = sim.app.world().resource::<Heard>();
    assert!(count(&heard.ca, ca_hurt) > 0, "Combat Arms' hurt sounds on the kill: {:?}", heard.ca);
    assert_eq!(count(&heard.cs, cs_voice), 0, "no CS:S hurt or death sound: {:?}", heard.cs);

    // Back with the Glock after the respawn: CS:S's death cry.
    for _ in 0..(8.0 * sim.tick_hz()) as u32 {
        if !dead(&sim, b) && sim.app.world().get::<God>(b).is_none() {
            break;
        }
        sim.ticks(1);
    }
    assert!(!dead(&sim, b), "respawned");
    sim.seconds(1.0);
    assert_eq!(held(&sim, b), Some(cs_weapons::GLOCK));
    assert_eq!(identity(&sim, b).as_deref(), Some("cs_source"));
    clear(&mut sim);
    headshot(&mut sim, b, 5.0);
    assert!(dead(&sim, b), "the headshot kills");
    let heard = sim.app.world().resource::<Heard>();
    assert!(heard.cs.iter().any(|s| s == "Player.DeathHeadShot"), "CS:S's death cry: {:?}", heard.cs);
    assert_eq!(count(&heard.ca, ca_hurt), 0, "no Combat Arms hurt sound: {:?}", heard.ca);
}
