//! A ladder that mixes Halo's rungs with the other games' (M2's acceptance
//! line, docs/plans/active/halo-ce.md: "UGG can run a match where a
//! ladder mixes Halo rungs with the other games'"), headless on the
//! party's simulation at its own tick (`CaSim`, 0.015 s) with stand-in
//! bodies for Combat Arms, CS:S and Halo (no mesh, no disc): a written
//! ladder alternates CS:S's and Combat Arms' guns with Halo's four, and a
//! player climbs it to the end by real fire at a player standing still.
//! On each Halo rung the climber is Halo's (its identity, its movement,
//! its vitality with a full shield, the Spartan's body, the gun in hand
//! and held); on each other rung it is that game's again with nothing of
//! Halo's left (no vitality, stun or shield armour, no Halo movement,
//! another body, no Halo gun held); each kill of a Halo gun advances the
//! ladder and is in the kill feed with the gun's display name; a Halo
//! player killed by another game's gun loses its shield first (the
//! cross-game rule: the shield's pieces cover every part, health
//! untouched while it lasts) and dies; the match ends at the last rung.
//! (It found Combat Arms' armour step taking the shield's pieces off a
//! Halo player every tick, so other games' rounds went through it as
//! through kevlar: `weapon::PiecesOf` now marks them Halo's.)

use bevy::prelude::*;
use chimera_client::client::{kill_feed::KillFeedStatePlugin, minigame_scoreboard::gun_name};
use chimera_core::{
    core::{Died, Health, Team},
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, MinigameAppExt, ladder::Ladder},
    mode::state::{Match, Phase},
    weapon::{Armor, ArmorPieces, ArmorRatio, PiecesOf, data::WeaponCatalog},
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{BodyModel, CharacterModels, Held, MapCharacterModel, MapSimPlugin},
};
use combat_arms::harness::CaSim;
use halo_ce::game::{
    movement::{self as halo_movement, HaloMovement},
    vitality::{Stun, Vitality, shield_armour},
    weapons,
};
use party_roster::PartyPartsPlugin;

use crate::gun_game::{alive, held, line_up, no_finale, phase, plain_rungs, rung, shoot_until_dead, soldier};

/// CS:S's and Combat Arms' guns between Halo's four.
const LADDER: [&str; 8] = [
    "cs_source:weapon_glock",
    weapons::ASSAULT_RIFLE,
    "combat_arms:mp5a4",
    weapons::PISTOL,
    "cs_source:weapon_deagle",
    weapons::PLASMA_RIFLE,
    "combat_arms:m4a1",
    weapons::SHOTGUN,
];

/// The stand-in bodies, by game.
const BODIES: [&str; 3] = ["combat_arms", "cs_source", "halo_ce"];

fn game_of(id: &str) -> &str {
    id.split(':').next().unwrap()
}

/// The party playing `LADDER` with a body of each game.
pub(crate) fn party() -> Sim {
    let text = crate::hl2dm_parts::ladder_text(&LADDER, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    party_with(&text)
}

/// The party playing the ladder minigame `text` (`test:gun_game_hl2dm`)
/// with a body of each game.
pub(crate) fn party_with(text: &str) -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin, KillFeedStatePlugin));
    });
    let body = |game| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    sim.app.insert_resource(CharacterModels(std::sync::Arc::new(BODIES.iter().map(|g| body(*g)).collect())));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    sim.app.add_minigames(&[("halo_ladder.toml", text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim
}

pub(crate) fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    minigame::ladder::set_rung(w, e, r, &l);
}

/// `e` is wholly the game of the gun it holds on rung `r` (module doc).
fn assert_is_its_guns_game(sim: &Sim, e: Entity, r: usize) {
    let w = sim.app.world();
    let gun = LADDER[r];
    let game = game_of(gun);
    let what = format!("rung {r} ({gun})");
    assert_eq!(held(sim, e), Some(gun), "{what}: the rung's gun in hand");
    assert_eq!(w.get::<GameIdentity>(e).map(|g| g.0.as_str()), Some(game), "{what}: identity");
    let body = w.get::<BodyModel>(e).map(|b| BODIES[b.0]);
    assert_eq!(body, Some(game), "{what}: its game's body");
    let movement = w.get::<MovementSlot>(e).map(|m| m.0);
    if game == halo_ce::game::GAME {
        assert_eq!(movement, Some(halo_movement::ID), "{what}: Halo's movement");
        assert!(w.get::<HaloMovement>(e).is_some(), "{what}: Halo's movement state");
        let v = w.get::<Vitality>(e).unwrap_or_else(|| panic!("{what}: Halo's vitality"));
        assert_eq!(v.shield, 1.0, "{what}: a full shield");
        // In the party table's incoming unit (`cross_game`).
        let unit = halo_ce::game::cross_game::incoming_in(w, "halo_ce:spartan");
        assert_eq!(w.get::<Armor>(e).map(|a| a.amount), Some(1.0 / unit), "{what}: the shield as its armour");
        assert_eq!(w.get::<ArmorPieces>(e), Some(&shield_armour()), "{what}: covered by the shield's pieces");
        assert_eq!(w.get::<PiecesOf>(e).map(|p| p.0), Some(halo_ce::game::GAME), "{what}: Halo's pieces");
        assert_eq!(w.get::<Held>(e).and_then(|h| h.0.as_deref()), Some(gun), "{what}: the gun held in the hand");
    } else {
        assert!(movement.is_some_and(|m| m != halo_movement::ID), "{what}: {movement:?}");
        assert!(w.get::<HaloMovement>(e).is_none(), "{what}: no Halo movement left");
        assert!(w.get::<Vitality>(e).is_none() && w.get::<Stun>(e).is_none(), "{what}: no Halo vitality left");
        // The armour value is the character's across games (docs/DESIGN.md
        // "Hits across games"); what covers it is no longer the shield.
        assert!(
            w.get::<ArmorRatio>(e).is_none()
                && w.get::<PiecesOf>(e).is_none()
                && w.get::<ArmorPieces>(e).is_none_or(|p| *p != shield_armour()),
            "{what}: the shield's pieces left on"
        );
        assert!(
            !w.get::<Held>(e).and_then(|h| h.0.as_deref()).is_some_and(|k| k.starts_with("halo_ce:")),
            "{what}: no Halo gun held: {:?}",
            w.get::<Held>(e).map(|h| h.0.clone())
        );
    }
}

/// The feed's newest entry: (victim, weapon).
fn last_kill(sim: &Sim) -> Option<(Entity, Option<&'static str>)> {
    let feed = sim.app.world().resource::<chimera_client::client::kill_feed::KillFeed>();
    feed.entries.last().map(|e| (e.victim.entity, e.weapon))
}

#[test]
fn a_ladder_mixing_halos_rungs_with_the_other_games_runs_to_its_end() {
    let mut sim = party();
    assert!((sim.tick_hz() - 1.0 / 0.015).abs() < 1e-6, "the party's tick");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(phase(&sim), Phase::Live);

    // A Halo player (b, on the assault rifle's rung) shot with CS:S's
    // Glock: the shield first, its health untouched while the shield
    // lasts, then dead by the cross-game rule.
    set_rung(&mut sim, b, 1);
    sim.seconds(1.2);
    assert_is_its_guns_game(&sim, b, 1);
    line_up(&mut sim, a, b, 3.0);
    let mut died = sim.app.world().resource::<Messages<Died>>().get_cursor_current();
    let mut shield_hit_first = false;
    for _ in 0..(8.0 * sim.tick_hz()) as u32 {
        let at = sim.position(b);
        chimera_sim::testkit::aim_at(&mut sim, a, at);
        sim.intent(a).fire = !sim.intent(a).fire;
        sim.ticks(1);
        let w = sim.app.world();
        if let (Some(v), Some(h)) = (w.get::<Vitality>(b), w.get::<Health>(b))
            && v.shield < 1.0
            && v.shield > 0.0
        {
            assert_eq!(h.current, 1.0, "the shield takes another game's bullet first");
            shield_hit_first = true;
        }
        if !alive(&sim, b) {
            break;
        }
    }
    sim.intent(a).fire = false;
    assert!(shield_hit_first, "a Glock round met the shield");
    assert!(!alive(&sim, b), "the Halo player died to CS:S's Glock");
    let deaths: Vec<Died> = died.read(sim.app.world().resource::<Messages<Died>>()).cloned().collect();
    assert!(deaths.iter().any(|d| d.entity == b && d.attacker == Some(a)), "{deaths:?}");
    // b back on the first rung for the rest, a on the assault rifle's.
    set_rung(&mut sim, b, 0);
    sim.seconds(1.2);
    assert_eq!(rung(&sim, a), 1, "the Glock's kill advanced the ladder");

    // a climbs the rest by real fire at b standing still.
    for r in 1..LADDER.len() {
        line_up(&mut sim, a, b, 3.0);
        assert_eq!(rung(&sim, a), r);
        assert_is_its_guns_game(&sim, a, r);
        assert!(shoot_until_dead(&mut sim, a, b, 12.0), "rung {r} ({}): b dies", LADDER[r]);
        sim.ticks(2);
        let (victim, weapon) = last_kill(&sim).expect("the kill is in the feed");
        assert_eq!((victim, weapon), (b, Some(LADDER[r])), "rung {r}: the feed names the gun");
        if game_of(LADDER[r]) == halo_ce::game::GAME {
            let catalog = sim.app.world().resource::<WeaponCatalog>();
            let name = gun_name(LADDER[r], None, Some(catalog));
            let want = match LADDER[r] {
                weapons::ASSAULT_RIFLE => "Assault Rifle",
                weapons::PISTOL => "Pistol",
                weapons::PLASMA_RIFLE => "Plasma Rifle",
                _ => "Shotgun",
            };
            assert_eq!(name, want, "the feed's display name");
        }
        if r + 1 < LADDER.len() {
            sim.seconds(1.2);
            assert_eq!(rung(&sim, a), r + 1, "rung {r}'s kill advanced the ladder");
        }
    }
    let team_a = sim.app.world().get::<Team>(a).unwrap().0;
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team_a), "the last rung's kill wins");
    sim.seconds(9.0);
    assert_eq!(phase(&sim), Phase::MatchOver, "the match ends at the last rung");
    assert_eq!(sim.app.world().resource::<Match>().result, Some(Some(team_a)));
}

/// Halo's guns keep their view and held models on Halo's own maps in the
/// party (its bundle's `kept`, in the party's `MAP_SOURCES`): without it a
/// Halo rung on Hang 'Em High showed no gun in hand (found in a tool run).
#[test]
fn halos_guns_stay_in_hand_on_its_own_maps() {
    let kit = chimera_world::map::KitLoad::Players(party_roster::MAP_SOURCES.kept);
    for gun in weapons::GUNS {
        assert!(kit.wants(gun), "{gun}");
    }
    assert!(kit.any_models());
}
