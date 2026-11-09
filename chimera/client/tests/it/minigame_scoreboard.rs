//! The minigame scoreboard (`client::minigame_scoreboard`): hidden until
//! `+showscores` is held while a minigame plays, then a row for every
//! player and bot with name, kills, deaths, the rung's gun, ping and
//! matches won, best rung first; down again on release, and never up
//! without a minigame (plain CS:S keeps its own scoreboard).

use bevy::prelude::*;
use chimera_client::client::{
    console::HeldActions,
    minigame_scoreboard::{MinigameScoreboardPlugin, ScoreboardRoot, ScoreboardRow},
};
use chimera_client::client::minigame_scoreboard::GunNames;
use chimera_net::net::{NetScore, score_flags};
use chimera_core::core::Score;
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        hud::MinigameHud,
        ladder::{Ladder, Rung},
        runtime::InMinigame,
        wins::MatchesWon,
    },
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

fn rows(sim: &mut Sim) -> Vec<ScoreboardRow> {
    let w = sim.app.world_mut();
    w.query::<&ScoreboardRow>().iter(w).cloned().collect()
}

fn shown(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    let v = w.query_filtered::<&Visibility, With<ScoreboardRoot>>().single(w).unwrap();
    *v != Visibility::Hidden
}

fn player(sim: &mut Sim, name: &'static str, rung: usize, kills: i32, deaths: u32, won: u32, net: Option<NetScore>) {
    let mut e = sim.app.world_mut().spawn((
        InMinigame,
        Name::new(name),
        Rung(rung),
        Score { kills, deaths },
        MatchesWon(won),
    ));
    if let Some(n) = net {
        e.insert(n);
    }
}

#[test]
fn holding_the_key_lists_every_player_and_bot_best_first() {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, MinigameScoreboardPlugin));
    sim.app.init_resource::<HeldActions>();
    sim.app.insert_resource(Ladder { rungs: vec!["kit:pistol", "kit:weapon_rifle", "kit:weapon_shotgun"], ..default() });
    sim.app.world_mut().insert_resource(GunNames([("kit:weapon_rifle".to_string(), "K&M Rifle".to_string())].into()));
    player(&mut sim, "Alpha", 1, 4, 2, 3, Some(NetScore { ping: 42, ..default() }));
    player(&mut sim, "Botty", 2, 1, 5, 0, Some(NetScore { flags: score_flags::BOT, ..default() }));
    player(&mut sim, "Cee", 1, 6, 1, 1, None);

    // Not while no minigame plays, whatever the key does.
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    sim.ticks(3);
    assert!(!shown(&mut sim) && rows(&mut sim).is_empty(), "plain games keep their own scoreboard");

    // A minigame plays but the key is up.
    sim.app.world_mut().insert_resource(MinigameHud { title: "Kit gun game".into(), ..default() });
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = false;
    sim.ticks(3);
    assert!(!shown(&mut sim) && rows(&mut sim).is_empty());

    // Held: a row each, rung then kills decide the order.
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    sim.ticks(3);
    assert!(shown(&mut sim));
    let got: Vec<(String, i32, u32, String, u16, bool, u32)> =
        rows(&mut sim).into_iter().map(|r| (r.name, r.kills, r.deaths, r.gun, r.ping, r.bot, r.wins)).collect();
    let want = [
        ("Botty", 1, 5, "SHOTGUN", 0, true, 0),
        ("Cee", 6, 1, "K&M Rifle", 0, false, 1),
        ("Alpha", 4, 2, "K&M Rifle", 42, false, 3),
    ]
    .map(|(n, k, d, g, p, b, w)| (n.to_string(), k, d, g.to_string(), p, b, w));
    assert_eq!(got, want);

    // The columns are fixed, non-shrinking widths that fit the board, so
    // no header or value overlaps its neighbour.
    {
        use chimera_client::client::minigame_scoreboard::{ScoreboardHeader, WIDTHS};
        let w = sim.app.world_mut();
        let header = w.query_filtered::<&Children, With<ScoreboardHeader>>().single(w).unwrap().to_vec();
        let widths: Vec<f32> = header
            .iter()
            .map(|c| {
                let n = w.get::<Node>(*c).unwrap();
                assert_eq!(n.flex_shrink, 0.0);
                match n.width {
                    Val::Px(v) => v,
                    other => panic!("column width {other:?}"),
                }
            })
            .collect();
        assert_eq!(widths, WIDTHS);
        let root = w.query_filtered::<&Node, With<ScoreboardRoot>>().single(w).unwrap();
        let Val::Px(board) = root.width else { panic!("board width") };
        assert!(widths.iter().sum::<f32>() < board, "columns fit the board");
        let bg = w.query_filtered::<&BackgroundColor, With<ScoreboardRoot>>().single(w).unwrap();
        assert_eq!(bg.0.alpha(), 1.0, "opaque over the minigame panel");
    }

    // Released: gone.
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = false;
    sim.ticks(3);
    assert!(!shown(&mut sim) && rows(&mut sim).is_empty());
}

/// A rung that makes its players a character shows that character's weapon
/// (Combat Arms' infected hands), not the ladder's entry beneath it.
#[test]
fn a_rung_character_shows_its_own_weapon() {
    use chimera_sim::minigame::ladder::{RungCharacter, RungExtras};
    use chimera_sim::weapon::data::WeaponDataAppExt;
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigameScoreboardPlugin));
        app.add_weapon_data(&[("hands.toml", "id = \"kit:hands\"\nslot = 2\ndraw = 0.5\n")]).expect("the data reads");
    });
    sim.app.init_resource::<HeldActions>();
    let knife = "kit:hands";
    sim.app.world_mut().insert_resource(GunNames([(knife.to_string(), "Hands".to_string())].into()));
    sim.app.insert_resource(Ladder {
        rungs: vec!["kit:pistol", "kit:weapon_knife"],
        extras: vec![
            None,
            Some(RungExtras {
                game: "kit".into(),
                character: Some(RungCharacter { id: "kit:zombie".into(), weapon: knife.into() }),
                ..default()
            }),
        ],
        ..default()
    });
    sim.app.world_mut().insert_resource(MinigameHud { title: "Kit".into(), ..default() });
    player(&mut sim, "Zed", 1, 0, 0, 0, None);
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    sim.ticks(3);
    assert_eq!(rows(&mut sim)[0].gun, "Hands");
}

/// A Sim with the real layout (bevy_ui's own plugin on a dummy camera): the
/// headless app otherwise has none.
fn laid_out_sim(extra: impl FnOnce(&mut App) + Send + Sync + 'static) -> Sim {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigameScoreboardPlugin));
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<bevy::picking::events::PointerState>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::picking::backend::PointerHits>()
            .init_asset::<Font>();
        app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin));
        extra(app);
    });
    sim.app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo { physical_size: UVec2::new(1280, 720), scale_factor: 1.0 }),
                ..default()
            },
            viewport: Some(Viewport { physical_size: UVec2::new(1280, 720), ..default() }),
            ..default()
        },
    ));
    sim
}

/// Where a node is drawn: its centre's y and its height.
fn drawn_y(w: &World, e: Entity) -> (f32, f32) {
    let t = w.get::<bevy::ui::UiGlobalTransform>(e).expect("laid out");
    (t.translation.y, w.get::<ComputedNode>(e).expect("laid out").size.y)
}

/// Every cell of a row is drawn at the row's middle.
#[test]
fn the_rows_cells_are_drawn_centred() {
    let mut sim = laid_out_sim(|_| {});
    sim.app.init_resource::<HeldActions>();
    sim.app.insert_resource(Ladder { rungs: vec!["kit:pistol"], ..default() });
    sim.app.world_mut().insert_resource(MinigameHud { title: "Kit".into(), ..default() });
    player(&mut sim, "Alpha", 0, 1, 0, 0, None);
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    sim.ticks(3);
    sim.app.update();
    let w = sim.app.world_mut();
    let row = w.query_filtered::<Entity, With<ScoreboardRow>>().single(w).unwrap();
    let w = sim.app.world();
    let (mid, tall) = drawn_y(w, row);
    assert!(tall >= 20.0, "the row has its height: {tall}");
    for c in w.get::<Children>(row).unwrap().iter() {
        let (at, _) = drawn_y(w, c);
        assert!((at - mid).abs() < 0.5, "a cell at {at} in a row centred at {mid}");
    }
}

/// A kill icon painted in the top rows of its texture (Combat Arms': 20 of
/// 32) is drawn with its art at the row's middle, at the scale the whole
/// texture would have.
#[test]
fn a_kill_icons_art_is_drawn_centred_in_its_row() {
    use chimera_client::client::{
        kill_feed::{KillFeed, KillFeedPlugin, KillFeedRow, KillFeedStatePlugin, SharedKillFeed},
        weapon_icons::{WeaponIcon, WeaponIcons, ink_rows},
    };
    use chimera_core::core::{Damage, DamageKind, Died, Hitgroup};
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut sim = laid_out_sim(|app| {
        app.add_plugins((KillFeedStatePlugin, KillFeedPlugin));
    });
    sim.app.insert_resource(SharedKillFeed);
    let (w, h) = (64usize, 32usize);
    let mut data = vec![0u8; w * h * 4];
    for y in 0..20 {
        for x in 0..w {
            data[(y * w + x) * 4 + 3] = 255;
        }
    }
    let image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    assert_eq!(ink_rows(&image), Some((0.0, 20.0)));
    let handle = sim.app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let mut icons = WeaponIcons::default();
    let art = handle.clone();
    icons.register("kit", move |_| Some(WeaponIcon::Image { image: art.clone(), size: Vec2::new(64.0, 32.0) }));
    sim.app.insert_resource(icons);
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), chimera_sim::movement::placeholder::ID);
    let b = sim.spawn_character(Vec3::new(5.0, 1.0, 0.0), chimera_sim::movement::placeholder::ID);
    sim.app.world_mut().write_message(Died {
        entity: b,
        attacker: Some(a),
        damage: Damage {
            target: b,
            attacker: Some(a),
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: Some("kit:gun"),
            force: Vec3::ZERO,
        },
    });
    sim.ticks(5);
    sim.app.update();
    assert_eq!(sim.app.world().resource::<KillFeed>().entries.len(), 1);
    let world = sim.app.world_mut();
    let row = world.query_filtered::<Entity, With<KillFeedRow>>().single(world).unwrap();
    let mut q = world.query_filtered::<(Entity, &ImageNode), ()>();
    let (icon, node) = q.iter(world).find(|(_, n)| n.image == handle).expect("the icon is drawn");
    assert_eq!(node.rect, Some(Rect::new(0.0, 0.0, 64.0, 20.0)), "its art's rows");
    let world = sim.app.world();
    let (mid, row_tall) = drawn_y(world, row);
    let (at, tall) = drawn_y(world, icon);
    assert!((at - mid).abs() < 0.5, "the art at {at} in a row centred at {mid}");
    assert!(tall > 0.0 && tall < row_tall, "{tall} in {row_tall}");
}
