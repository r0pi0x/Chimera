//! The view-model driver's rules per game (`weapon::view_anim::
//! GameViewAnimRules`): two games' rules side by side, each weapon played
//! by its own game's, whichever registered first.

use std::sync::Mutex;

use bevy::prelude::*;
use chimera_sim::{
    SimBasePlugins,
    testkit::{self, PISTOL},
    weapon::{
        WeaponEventKind,
        data::WeaponDataAppExt,
        view_anim::{GameViewAnimRules, Pick, ViewAnimAppExt, ViewAnimPlugin, ViewAnimRules, ViewFrame, ViewState},
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};

/// A second game's pistol, on the kit's base.
const OTHER_PISTOL: &str = "other:pistol";
const OTHER_DOC: &str = r#"
id = "other:pistol"
base = "kit:gun"
slot = 1
draw = 0.5

[trigger]
automatic = false
cycle = 0.5

[magazine]
clip = 8
ammo = "kit:round"
reload = 1.5

[hitscan]
range = 100.0

[damage]
amount = 20.0
"#;

/// Which game's rules were asked about which weapon (draws and idles).
static ASKED: Mutex<Vec<(&str, &str)>> = Mutex::new(Vec::new());

fn ask(game: &'static str, s: &ViewState) -> Pick {
    ASKED.lock().unwrap().push((game, s.weapon));
    Pick::Names(Vec::new())
}

fn no_event(_: &mut ViewFrame, _: &WeaponEventKind) {}

fn rules_of(game: &'static str) -> ViewAnimRules {
    match game {
        "kit" => ViewAnimRules {
            draw: |s| ask("kit", s),
            idle: |s| ask("kit", s),
            event: no_event,
            effect: |_| None,
            draw_rate: 1.0,
            others_reload: None,
        },
        _ => ViewAnimRules {
            draw: |s| ask("other", s),
            idle: |s| ask("other", s),
            event: no_event,
            effect: |_| None,
            draw_rate: 1.0,
            others_reload: None,
        },
    }
}

#[test]
fn two_games_rules_each_play_their_own_weapons_in_either_order() {
    for order in [["kit", "other"], ["other", "kit"]] {
        ASKED.lock().unwrap().clear();
        let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, ViewAnimPlugin));
        testkit::add_weapons(&mut sim.app);
        sim.app.add_weapon_data(&[("other_pistol", OTHER_DOC)]).expect("the other game's pistol loads");
        for game in order {
            sim.app.view_anim_rules(game, rules_of(game));
        }
        // Registering again replaces the game's own entry.
        sim.app.view_anim_rules(order[0], rules_of(order[0]));
        let games: Vec<_> = sim.app.world().resource::<GameViewAnimRules>().games().collect();
        assert_eq!(games, order, "one entry per game, in registration order");
        let kit = testkit::with_player(&mut sim, None, PISTOL);
        let other = testkit::with_player(&mut sim, Some(greybox::SPAWNS[1]), OTHER_PISTOL);
        sim.seconds(0.5);
        assert_eq!(testkit::active_id(sim.app.world(), kit), Some(PISTOL));
        assert_eq!(testkit::active_id(sim.app.world(), other), Some(OTHER_PISTOL));
        let asked = ASKED.lock().unwrap().clone();
        for (game, weapon) in &asked {
            assert!(weapon.starts_with(&format!("{game}:")), "{order:?}: {game}'s rules asked about {weapon}");
        }
        for (game, weapon) in [("kit", PISTOL), ("other", OTHER_PISTOL)] {
            assert!(
                asked.contains(&(game, weapon)),
                "{order:?}: {game}'s rules drew {weapon}: {asked:?}"
            );
        }
    }
}

#[test]
fn a_game_without_rules_leaves_its_weapons_alone() {
    let mut rules = GameViewAnimRules::default();
    rules.set("kit", rules_of("kit"));
    assert!(rules.for_weapon(PISTOL).is_some());
    assert!(rules.for_weapon("KIT:pistol").is_some(), "game ids ignore case");
    assert!(rules.for_weapon(OTHER_PISTOL).is_none());
    assert!(rules.for_weapon("pistol").is_none(), "an id without a game");
}
