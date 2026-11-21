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

/// A gun shown ahead of its kit draws before the sound table has its
/// sounds (or while the map still loads): the owner's draw sound waits for
/// them and plays once, instead of being asked of an audio that drops it.
#[test]
fn an_owners_view_sound_waits_for_its_sound_table() {
    use chimera_world::map::{
        LocalMapLoad, MapSoundEntry, MapSounds, PlaySound, SoundLevel, ViewModelEvent, ViewModelEventKind,
        sound::Interval,
    };
    use chimera_core::core::LocalPlayer;
    use chimera_sim::weapon::view_anim::{DRAW_SOUND_WAIT, play_view_sounds};
    use chimera_world::map::sound::SoundBank;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<ViewModelEvent>()
        .add_message::<PlaySound>()
        .add_systems(Update, play_view_sounds);
    let owner = app.world_mut().spawn((Transform::default(), LocalPlayer)).id();
    let draw = || ViewModelEvent {
        owner,
        kind: ViewModelEventKind::Sound { entry: "Weapon_Glock.Draw".into() },
        others: None,
        reload: false,
    };
    let heard = |app: &mut App| app.world_mut().resource_mut::<Messages<PlaySound>>().drain().count();
    let step = |app: &mut App, secs: f64| {
        // Frames of at most 0.1 s (virtual time clamps a longer one).
        let frames = (secs / 0.1).ceil().max(1.0) as u32;
        let each = std::time::Duration::from_secs_f64(secs / frames as f64);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(each));
        for _ in 0..frames {
            app.update();
        }
    };
    // No sound table yet: nothing is asked.
    app.world_mut().write_message(draw());
    step(&mut app, 0.1);
    assert_eq!(heard(&mut app), 0, "waits, no table");
    // The table comes in, but the map still loads.
    let mut sounds = MapSounds::default();
    sounds.entries.insert(
        "weapon_glock.draw".into(),
        MapSoundEntry {
            waves: vec![0],
            volume: Interval::fixed(1.0),
            pitch: Interval::fixed(100.0),
            level: SoundLevel::Db(75.0),
            channel: 0,
            dry: false,
        },
    );
    app.insert_resource(SoundBank(std::sync::Arc::new(sounds)));
    app.insert_resource(LocalMapLoad::default());
    step(&mut app, 0.1);
    assert_eq!(heard(&mut app), 0, "waits, map loading");
    // Loaded: it plays, once.
    app.world_mut().remove_resource::<LocalMapLoad>();
    step(&mut app, 0.1);
    assert_eq!(heard(&mut app), 1, "plays when it can");
    step(&mut app, 0.1);
    assert_eq!(heard(&mut app), 0, "once");
    // Never audible: asked once after its wait, not kept.
    app.world_mut().insert_resource(LocalMapLoad::default());
    app.world_mut().write_message(draw());
    step(&mut app, 0.1);
    assert_eq!(heard(&mut app), 0);
    step(&mut app, DRAW_SOUND_WAIT);
    assert_eq!(heard(&mut app), 1, "past its wait it is asked (the audio says why it is silent)");
    step(&mut app, 0.1);
    assert_eq!(heard(&mut app), 0);
}
