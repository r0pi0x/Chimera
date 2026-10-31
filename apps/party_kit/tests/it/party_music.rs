//! The party's music and spawn protection (docs/backlog.md "Gaps vs the
//! originals, second pass" 5 and 10) on the party's simulation: the gun
//! game's music is Combat Arms' (the first game whose parts lend a music
//! director), whose director plays Arms Race's cues from the minigame's
//! moments (music_ambience.md Section 10); its callouts sound Combat
//! Arms' cue pack while Combat Arms' HUD is up; a freshly spawned player
//! is protected for the spec's windows and a hit on them, a CS:S gun's or
//! a Combat Arms gun's, shows and deals nothing.

use bevy::prelude::*;
use chimera_core::{
    core::{Health, LocalPlayer, SpawnProtected},
    cues::CuePacks,
    game_parts::PartsRegistry,
};
use chimera_sim::{
    minigame::moments::{MinigameMoments, MomentPhase},
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::harness::Sim;
use combat_arms::game::music::director::CaMusic;

use super::gun_game::{held, party, place, soldier};

#[test]
fn the_gun_games_music_is_combat_arms_and_its_start_of_play_cues_play() {
    let mut sim = party();
    let a = soldier(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    let _b = soldier(&mut sim);
    sim.seconds(1.0);
    let m = sim.app.world().resource::<MinigameMoments>().clone();
    assert_eq!((m.music, m.phase, m.alive), (Some(combat_arms::game::GAME), MomentPhase::Live, true));
    let music = sim.app.world().resource::<CaMusic>();
    assert!(music.heard.contains(&"RoundStartSound".to_string()), "{:?}", music.heard);
    let start = music.data.mode_track("VWRM", None).unwrap().0;
    assert_eq!(music.current, start, "Arms Race's start track asked for");
    // The cue pack and the match-end sounds of the HUD's game.
    let packs = sim.app.world().resource::<CuePacks>();
    assert_eq!(packs.choose_for(None, Some(combat_arms::game::GAME)).unwrap().id, "combat_arms");
    assert_eq!(packs.choose_for(None, Some(cs_source::game::GAME)).unwrap().id, "cs_source");
    let parts = sim.app.world().resource::<PartsRegistry>();
    assert_eq!(parts.match_end_sounds(Some(combat_arms::game::GAME), false)[0].1, "UI_SND/LOSE.WAV");
    assert_eq!(parts.match_end_sounds(Some(cs_source::game::GAME), true)[0].0, cs_source::game::GAME);
}

/// `shooter` fires at `target` for half a second: the
/// traces that hit it, the damage it took.
fn fire_at(sim: &mut Sim, shooter: Entity, target: Entity) -> (usize, f32) {
    let before = sim.app.world().get::<Health>(target).unwrap().current;
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut hits = 0;
    for t in 0..(0.5 * sim.tick_hz()) as u64 {
        let at = sim.position(target);
        chimera_sim::testkit::aim_at(sim, shooter, at);
        sim.intent(shooter).fire = t % 2 == 0;
        sim.ticks(1);
        hits += cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .filter(|e| match e.kind {
                WeaponEventKind::Shot { hit, .. } | WeaponEventKind::ShotContinued { hit, .. } => hit == Some(target),
                WeaponEventKind::Hit { target: h, .. } => h == target,
                _ => false,
            })
            .count();
    }
    sim.intent(shooter).fire = false;
    (hits, before - sim.app.world().get::<Health>(target).unwrap().current)
}

/// Case 105 for both games' guns: a CS:S gun's (the first rung's Glock)
/// and a Combat Arms gun's (the second rung's Beretta) hits on a player
/// inside its protection show nothing (no trace hit: no blood, impact or
/// hit indicator) and deal nothing; after it they show and land.
#[test]
fn a_protected_player_shows_and_takes_no_hit_from_either_games_guns() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    let full = sim.app.world().get::<Health>(b).unwrap().current;
    for gun in [cs_source::game::weapons::GLOCK, "combat_arms:beretta_m92fs"] {
        if held(&sim, a) != Some(gun) {
            let w = chimera_sim::testkit::active(sim.app.world(), a).unwrap();
            chimera_sim::minigame::runtime::take_weapon(sim.app.world_mut(), a, w);
            chimera_sim::weapon::give(sim.app.world_mut(), a, gun);
            sim.seconds(1.2);
        }
        let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
        sim.app.world_mut().entity_mut(b).insert((
            SpawnProtected {
                damage_until: now + 4.0,
                bullets_until: now + 3.0,
            },
            chimera_core::core::God,
        ));
        // On the greybox's open middle (a spawn spot may be near a ledge).
        let from = crate::gun_game::OPEN;
        place(&mut sim, a, from);
        place(&mut sim, b, from + Vec3::new(0.0, 0.0, -4.0));
        sim.ticks(3);
        assert_eq!(fire_at(&mut sim, a, b), (0, 0.0), "{gun}: protected");
        sim.app.world_mut().entity_mut(b).remove::<(SpawnProtected, chimera_core::core::God)>();
        // Let the first burst's view kick return: Combat Arms' rounds leave
        // along the look plus the kick, so straight after it they fly over
        // the shoulders, and whether they clip the body then depends on
        // which way the target faces (its spawn facing).
        sim.seconds(1.0);
        let (hits, lost) = fire_at(&mut sim, a, b);
        assert!(hits > 0 && lost > 0.0, "{gun}: after it ({hits} hits, {lost} lost)");
        sim.app.world_mut().get_mut::<Health>(b).unwrap().current = full;
    }
}

/// The bullets window lets rounds through (user, 2026-10-29): with
/// either game's gun (CS:S's delivery and Combat Arms' own), a round at
/// a protected player hits the player behind it, never the protected one.
#[test]
fn rounds_pass_through_a_protected_player_into_the_one_behind_with_either_games_guns() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    let c = soldier(&mut sim);
    sim.seconds(1.5);
    for gun in [cs_source::game::weapons::GLOCK, "combat_arms:beretta_m92fs"] {
        if held(&sim, a) != Some(gun) {
            let w = chimera_sim::testkit::active(sim.app.world(), a).unwrap();
            chimera_sim::minigame::runtime::take_weapon(sim.app.world_mut(), a, w);
            chimera_sim::weapon::give(sim.app.world_mut(), a, gun);
            sim.seconds(1.2);
        }
        let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
        sim.app.world_mut().entity_mut(b).insert((
            SpawnProtected {
                damage_until: now + 4.0,
                bullets_until: now + 4.0,
            },
            chimera_core::core::God,
        ));
        sim.app.world_mut().entity_mut(c).remove::<(SpawnProtected, chimera_core::core::God)>();
        let from = sim.position(a);
        place(&mut sim, b, from + Vec3::new(0.0, 0.0, -4.0));
        place(&mut sim, c, from + Vec3::new(0.0, 0.0, -6.0));
        sim.ticks(3);
        for e in [b, c] {
            sim.app.world_mut().get_mut::<Health>(e).unwrap().current = 1000.0;
        }
        let before = sim.app.world().get::<Health>(c).unwrap().current;
        assert_eq!(fire_at(&mut sim, a, b), (0, 0.0), "{gun}: nothing on the protected player");
        let lost = before - sim.app.world().get::<Health>(c).unwrap().current;
        assert!(lost > 0.0, "{gun}: the one behind takes the rounds");
    }
}

/// The `party_music` cvar (0 off, 1 playlist, 2 the original's loop) is
/// on the party's console, defaults to the playlist and sets the music.
#[test]
fn the_party_music_cvar_defaults_to_the_playlist_and_sets_the_director() {
    let mut sim = party();
    let console = sim.app.world().resource::<chimera_core::console::Console>();
    let cvar = console.cvar("party_music").expect("party_music").clone();
    assert_eq!(cvar.default, "1");
    assert_eq!((cvar.get)(sim.app.world_mut()).as_deref(), Some("1"));
    (cvar.set)(sim.app.world_mut(), "2").unwrap();
    assert_eq!(sim.app.world().resource::<CaMusic>().party_music, 2);
    assert!((cvar.set)(sim.app.world_mut(), "3").is_err());
}
