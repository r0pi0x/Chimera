//! CS:S's own parts in the party apps follow the gun in hand (parity
//! audit 2026-10-27): the hit
//! reaction (slowdown, view flinch) of a CS:S-identity player, the target
//! ID and radar while CS:S's HUD is up (hidden with a Combat Arms gun),
//! prop collision sounds, and the kill feed's icons by the full weapon id.

use std::{collections::HashMap, sync::Arc};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_sim::{testkit, weapon::ViewPunch};
use chimera_world::{
    harness::Sim,
    map::{
        MapSoundEntry, MapSounds, MapSurface, PhysicsProp, PlaySound, PushAway,
        hud::{ActiveOverview, MapOverview}, sound::{Interval, SoundBank, SoundLevel},
    },
};
use cs_source::game::movement::SourceMovement;
use cs_source::ui::{radar::RadarFrame, target_id::TargetIdText};

use crate::gun_game::{held, line_up, party, place, soldier};
use crate::hud::{hud, party_with_huds_and, swap_to};

fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    use chimera_sim::minigame::ladder::{self, Ladder};
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    ladder::set_rung(w, e, r, &l);
}

/// Two soldiers 10 m apart, the shooter on rung 0 (CS:S's Glock), the
/// victim on `victim_rung`; the shooter fires for `secs` and the lowest
/// velocity modifier and the strongest view punch the victim had are
/// returned.
fn fight(victim_rung: usize) -> (f32, f32) {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, b, victim_rung);
    sim.seconds(1.0);
    place(&mut sim, a, Vec3::new(0.0, 1.0, 0.0));
    line_up(&mut sim, a, b, 10.0);
    assert_eq!(held(&sim, a), Some(cs_source::game::weapons::GLOCK));
    let (mut low, mut punch) = (1.0_f32, 0.0_f32);
    for t in 0..(2.0 * sim.tick_hz()) as u64 {
        let at = sim.position(b);
        testkit::aim_at(&mut sim, a, at);
        sim.intent(a).fire = t % 2 == 0;
        sim.ticks(1);
        if let Some(m) = sim.app.world().get::<SourceMovement>(b) {
            low = low.min(m.velocity_modifier);
        }
        punch = punch.max(sim.app.world().get::<ViewPunch>(b).map_or(0.0, |p| p.0.length()));
    }
    (low, punch)
}

#[test]
fn a_cs_source_gun_slows_and_flinches_a_cs_source_victim_only() {
    // A CS:S victim (the Glock in hand): slowed and flinched (M18).
    let (low, punch) = fight(0);
    assert!(low < 0.99, "the hit slows the victim: {low}");
    assert!(punch > 0.001, "and kicks its view: {punch}");
    // A Combat Arms victim (the Beretta): neither reaction of CS:S's.
    let (low, punch) = fight(1);
    assert!(low >= 1.0, "no CS:S slowdown on a Combat Arms player: {low}");
    assert!(punch < 1e-6, "nor CS:S's flinch: {punch}");
}

fn overview() -> ActiveOverview {
    ActiveOverview(
        MapOverview {
            texture: 0,
            origin: Vec2::new(-50.0, -50.0),
            meters_per_pixel: 0.1,
            rotate: false,
            size: Vec2::splat(1000.0),
        },
        Handle::default(),
    )
}

fn visible(sim: &mut Sim, which: &str) -> Vec<Visibility> {
    let w = sim.app.world_mut();
    match which {
        "target" => w.query_filtered::<&Visibility, With<TargetIdText>>().iter(w).copied().collect(),
        _ => w.query_filtered::<&Visibility, With<RadarFrame>>().iter(w).copied().collect(),
    }
}

#[test]
fn target_id_and_radar_follow_the_gun_in_hand() {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        // (The radar's material needs the renderer's asset types.)
        app.init_asset::<bevy::shader::Shader>().init_asset::<Image>();
        app.init_resource::<chimera_client::client::spectate::Spectator>();
        app.add_plugins((
            cs_source::ui::target_id::TargetIdPlugin,
            cs_source::ui::radar::RadarPlugin,
            cs_source::ui::freeze_cam::FreezeCamPlugin,
            cs_source::ui::voice_hud::VoiceHudPlugin,
        ))
        .insert_resource(overview());
        app.world_mut().spawn(Window::default());
    });
    sim.app.world_mut().entity_mut(other).insert(Name::new("Mate"));
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    place(&mut sim, me, Vec3::new(0.0, 1.0, 0.0));
    place(&mut sim, other, Vec3::new(0.0, 1.0, -5.0));
    sim.ticks(10);
    let aim = |sim: &mut Sim| {
        let at = sim.position(other) + Vec3::Y * 0.9;
        testkit::aim_at(sim, me, at);
        sim.seconds(0.3);
    };
    // The Glock: CS:S's HUD, the name of the player aimed at, the radar.
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::GLOCK));
    aim(&mut sim);
    let id = sim.app.world().resource::<cs_source::ui::target_id::TargetId>().clone();
    assert_eq!(id.target, Some(other), "{id:?}");
    assert!(id.text.contains("Mate"), "{id:?}");
    assert_eq!(visible(&mut sim, "target"), [Visibility::Inherited], "the name shows");
    assert_eq!(visible(&mut sim, "radar"), [Visibility::Inherited], "the radar shows");
    // The Beretta: Combat Arms' HUD; CS:S's pieces put away.
    swap_to(&mut sim, me, 1);
    assert_eq!(held(&sim, me), Some("combat_arms:beretta_m92fs"));
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    aim(&mut sim);
    assert_eq!(visible(&mut sim, "target"), [Visibility::Hidden], "no CS:S names with a Combat Arms gun");
    assert_eq!(visible(&mut sim, "radar"), [Visibility::Hidden], "no CS:S radar with it");
    // And back with the Desert Eagle.
    swap_to(&mut sim, me, 2);
    aim(&mut sim);
    assert_eq!(visible(&mut sim, "target"), [Visibility::Inherited]);
    assert_eq!(visible(&mut sim, "radar"), [Visibility::Inherited]);
}

#[test]
fn a_prop_knocked_about_makes_a_collision_sound() {
    let mut sim = party();
    let mut sounds = MapSounds::default();
    sounds.surfaces.insert(
        "default".into(),
        MapSurface {
            impact_hard: Some("Test.ImpactHard".into()),
            hardness: 1.0,
            ..default()
        },
    );
    sounds.entries.insert("test.impacthard".into(), MapSoundEntry {
            waves: vec![],
            volume: Interval { start: 1.0, range: 0.0 },
            pitch: Interval { start: 100.0, range: 0.0 },
            level: SoundLevel::Db(60.0),
            channel: 0,
            dry: false,
            hearing: None,
        });
    sim.app.insert_resource(SoundBank(Arc::new(sounds)));
    let heard = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = heard.clone();
    sim.app.add_systems(Last, move |mut s: MessageReader<PlaySound>| {
        sink.lock().unwrap().extend(s.read().map(|p| p.entry.clone()));
    });
    sim.ticks(2);
    // A 10 kg box dropped from 3 m onto the floor.
    sim.app.world_mut().spawn((
        RigidBody::Dynamic,
        Collider::cuboid(0.5, 0.5, 0.5),
        Mass(10.0),
        CollisionEventsEnabled,
        PhysicsProp {
            push: PushAway::Collide,
            mass: 10.0,
            bounds: (Vec3::splat(-0.25), Vec3::splat(0.25)),
        },
        Transform::from_xyz(5.0, 3.0, 5.0),
    ));
    sim.seconds(2.0);
    let heard = heard.lock().unwrap();
    assert!(heard.iter().any(|e| e == "Test.ImpactHard"), "CS:S's prop impact sound: {heard:?}");
}

#[test]
fn kill_feed_icons_are_by_the_full_weapon_id() {
    let icons: HashMap<String, u8> = [("d_m4a1".to_string(), 1), ("d_glock".to_string(), 2)].into();
    let icon = |id| cs_source::ui::game_hud::kill_icon(id, &icons);
    assert_eq!(icon("cs_source:weapon_m4a1"), Some(1));
    assert_eq!(icon("cs_source:weapon_glock"), Some(2));
    assert_eq!(icon("combat_arms:m4a1"), None, "a Combat Arms M4A1 isn't CS:S's");
    assert_eq!(icon("combat_arms:glock"), None);
}
