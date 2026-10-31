//! The death screen bound by role to the kill-cam layout
//! (`ui::hud::death`; specs/combat_arms/death_killcam.md 8) on a small
//! layout written here with the controls the binding looks up (the
//! install's own: `heavy::install_ui`), and the name tags of a dead
//! player (case 28).

use bevy::math::IVec2;
use chimera_client::client::canvas::{HAlign, Scale, VAlign, WidgetId};
use combat_arms::{
    game::death_cam::screen::{
        DamageView, DeathScreen, FastView, KillerView, RespawnView, TargetView, TipView, WeaponView, Zone,
    },
    ui::{
        CaUi,
        hud::{
            death::KillCamPanel,
            model::{TAG_DEAD, TAG_ENEMY, TAG_MATE, TagInputs, name_tag},
        },
        layout::{GaugeLook, ImageLook, Kind, Layout, TextLook},
    },
};

fn text(element: &str) -> Kind {
    Kind::Text(TextLook {
        font: "bold 12".into(),
        element: element.into(),
        h: HAlign::Default,
        v: VAlign::Default,
        colour: [255; 4],
        ..Default::default()
    })
}

fn image() -> Kind {
    Kind::Image(ImageLook {
        texture: "sample".into(),
        ..Default::default()
    })
}

fn window() -> Kind {
    Kind::Window(ImageLook {
        texture: "UI_SCR_Common\\blank.dtx".into(),
        ..Default::default()
    })
}

/// A kill-cam layout with the controls of the spec's table.
fn layout() -> Layout {
    let mut l = Layout::new();
    let at = IVec2::new;
    let root = l.add(0, "KillCam", at(0, 0), at(1024, 768), window());
    let info = l.add(root, "KillerInfo", at(10, 10), at(300, 60), window());
    let on = l.add(info, "CamOn", at(0, 0), at(300, 60), window());
    l.add(on, "name", at(5, 5), IVec2::ZERO, text("KILLER"));
    l.add(on, "Rank", at(100, 5), at(32, 32), image());
    l.add(on, "hp", at(5, 30), IVec2::ZERO, text("000"));
    l.add(on, "ap", at(60, 30), IVec2::ZERO, text("000"));
    let off = l.add(info, "CamOff", at(0, 0), at(300, 60), window());
    l.add(off, "name", at(5, 5), IVec2::ZERO, text("KILLER"));
    l.add(off, "Rank", at(100, 5), at(32, 32), image());
    let weapon = l.add(root, "KillerWeapon", at(10, 80), at(200, 80), window());
    l.add(weapon, "name", at(5, 5), IVec2::ZERO, text("WEAPON"));
    l.add(weapon, "weapon_img", at(5, 20), at(128, 48), image());
    let hit = l.add(root, "DamageWnd", at(10, 170), at(150, 50), window());
    l.add(hit, "Damage", at(5, 5), IVec2::ZERO, text("000"));
    l.add(hit, "Damage_img", at(60, 5), at(32, 32), image());
    let target = l.add(root, "Target", at(400, 600), at(300, 60), window());
    l.add(target, "name", at(5, 5), IVec2::ZERO, text("TARGET"));
    l.add(target, "KillCount", at(5, 25), IVec2::ZERO, text("$DeadCamKillCount"));
    l.add(target, "FreeCam_info", at(5, 40), IVec2::ZERO, text("$FreeCam_info"));
    let banner = l.add(root, "Banner", at(300, 680), at(400, 60), window());
    l.add(banner, "Banner_icon", at(5, 5), at(32, 32), image());
    l.add(banner, "banner_description", at(40, 5), IVec2::ZERO, text("tip"));
    let gauge = || {
        Kind::Gauge(GaugeLook {
            frame: "frame".into(),
            bar: "bar".into(),
            bar_at: IVec2::ONE,
            max_range: 200,
            fill: 0.0,
            bar_height: 8,
        })
    };
    let plain = l.add(root, "Respawn", at(700, 650), at(300, 100), window());
    l.add(plain, "respawn_gauge", at(5, 5), at(200, 10), gauge());
    l.add(plain, "timer", at(5, 20), IVec2::ZERO, text("00:00"));
    l.add(plain, "des_FastRespawn_2", at(5, 40), IVec2::ZERO, text("sample"));
    l.add(plain, "des_FastRespawn", at(5, 60), IVec2::ZERO, text("sample"));
    l.add(plain, "des_FastRespawn_3", at(5, 80), IVec2::ZERO, text("$des_FastRespawn_default_3"));
    let express = l.add(root, "Respawn_EXPass", at(700, 650), at(300, 100), window());
    l.add(express, "respawn_gauge", at(5, 5), at(200, 10), gauge());
    l.add(express, "timer", at(5, 20), IVec2::ZERO, text("00:00"));
    l
}

struct Built {
    l: Layout,
}

impl Built {
    fn id(&self, from: &str, name: &str) -> WidgetId {
        let f = self.l.find(from).unwrap_or_else(|| panic!("no {from}"));
        self.l.tree.find_from(f, name).unwrap_or_else(|| panic!("no {name} in {from}"))
    }

    fn shown(&self, name: &str) -> bool {
        self.l.tree.get(self.l.find(name).unwrap()).show
    }

    fn text(&self, from: &str, name: &str) -> String {
        let t = self.l.widget(self.id(from, name)).text().unwrap().clone();
        t.set.unwrap_or(t.element)
    }

    fn texture(&self, from: &str, name: &str) -> String {
        match &self.l.widget(self.id(from, name)).kind {
            Kind::Image(i) | Kind::Window(i) => i.texture.clone(),
            k => panic!("{k:?}"),
        }
    }

    fn fill(&self, from: &str) -> f64 {
        match &self.l.widget(self.id(from, "respawn_gauge")).kind {
            Kind::Gauge(g) => g.fill,
            k => panic!("{k:?}"),
        }
    }
}

fn build(screen: &DeathScreen) -> Built {
    let ui = CaUi::default();
    let panel = KillCamPanel::new(&layout()).expect("KillCam");
    let textures: std::collections::HashMap<String, String> = [
        ("damagepointiconheadtex", "ui/hit_head.dtx"),
        ("damagepointiconbodytex", "ui/hit_body.dtx"),
        ("damagepointiconarmstex", "ui/hit_arms.dtx"),
        ("damagepointiconlegtex", "ui/hit_leg.dtx"),
        ("damagepointiconnuttex", "ui/hit_nut.dtx"),
        ("respawngauge_normal", "ui/gauge_normal.dtx"),
        ("respawngauge_perk", "ui/gauge_perk.dtx"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    Built {
        l: panel.build(
            screen,
            &ui.strings,
            &textures,
            Scale::of(bevy::math::Vec2::new(1024.0, 768.0), bevy::math::Vec2::new(1024.0, 768.0)),
        ),
    }
}

fn killer(cam_on: bool) -> KillerView {
    KillerView {
        name: "Bravo".into(),
        cam_on,
        hp: "042".into(),
        ap: "000".into(),
    }
}

/// 4. With the enemy view on `KillerInfo` uses `CamOn` with `042` / `000`.
#[test]
fn case_04_killer_info_cam_on() {
    let b = build(&DeathScreen {
        killer: Some(killer(true)),
        ..Default::default()
    });
    assert!(b.shown("KillerInfo") && b.shown("CamOn") && !b.shown("CamOff"));
    assert_eq!(b.text("CamOn", "name"), "Bravo");
    assert_eq!((b.text("CamOn", "hp").as_str(), b.text("CamOn", "ap").as_str()), ("042", "000"));
}

/// 5. With it off `CamOff` (name and rank only).
#[test]
fn case_05_killer_info_cam_off() {
    let b = build(&DeathScreen {
        killer: Some(killer(false)),
        ..Default::default()
    });
    assert!(b.shown("KillerInfo") && b.shown("CamOff") && !b.shown("CamOn"));
    assert_eq!(b.text("CamOff", "name"), "Bravo");
}

/// 10, 11. A suicide or a fall: `KillerInfo` hidden (the damage and the
/// target panels still show).
#[test]
fn case_10_11_no_killer_info() {
    let b = build(&DeathScreen {
        damage: Some(DamageView {
            text: "12".into(),
            zone: Zone::Body,
        }),
        ..Default::default()
    });
    assert!(!b.shown("KillerInfo") && b.shown("DamageWnd") && !b.shown("KillerWeapon"));
}

/// 22. The damage figure and the zone icon from the layout's strings.
#[test]
fn case_22_damage_window() {
    for (zone, tex) in [
        (Zone::Head, "ui/hit_head.dtx"),
        (Zone::Body, "ui/hit_body.dtx"),
        (Zone::Arms, "ui/hit_arms.dtx"),
        (Zone::Leg, "ui/hit_leg.dtx"),
        (Zone::Groin, "ui/hit_nut.dtx"),
    ] {
        let b = build(&DeathScreen {
            damage: Some(DamageView { text: "75".into(), zone }),
            ..Default::default()
        });
        assert_eq!(b.text("DamageWnd", "Damage"), "75");
        assert_eq!(b.texture("DamageWnd", "Damage_img"), tex);
    }
}

/// The killing weapon: its name and large icon.
#[test]
fn weapon_panel() {
    let b = build(&DeathScreen {
        weapon: Some(WeaponView {
            name: "M4A1".into(),
            id: "combat_arms:m4a1".into(),
        }),
        ..Default::default()
    });
    assert!(b.shown("KillerWeapon"));
    assert_eq!(b.text("KillerWeapon", "name"), "M4A1");
    assert_eq!(b.texture("KillerWeapon", "weapon_img"), combat_arms::ui::install::kill_icon("combat_arms:m4a1"));
}

/// 23. Watching B, who killed you three times: `$DeadCamKillCount` with 3;
/// watching C, who never did: no count.
#[test]
fn case_23_kill_me_count() {
    let b = build(&DeathScreen {
        target: Some(TargetView {
            name: "B".into(),
            kills_you: Some(3),
            free_cam_info: false,
        }),
        ..Default::default()
    });
    assert_eq!(b.text("Target", "KillCount"), "Killed you 3 times");
    assert!(!b.l.tree.get(b.id("Target", "FreeCam_info")).show);
    let b = build(&DeathScreen {
        target: Some(TargetView {
            name: "C".into(),
            kills_you: None,
            free_cam_info: false,
        }),
        ..Default::default()
    });
    assert_eq!(b.text("Target", "KillCount"), "");
    assert_eq!(b.text("Target", "name"), "C");
}

/// 24. Hunted: the free cameras' key help shows in the target panel.
#[test]
fn hunted_free_cam_info() {
    let b = build(&DeathScreen {
        target: Some(TargetView {
            name: "C".into(),
            kills_you: None,
            free_cam_info: true,
        }),
        ..Default::default()
    });
    assert!(b.l.tree.get(b.id("Target", "FreeCam_info")).show);
}

/// 6-8. The countdown: the gauge in whole percent, the `SS:hh` timer, the
/// bar art by the perk; the fast-respawn counts, or the premium pass's
/// panel.
#[test]
fn case_06_08_respawn_panel() {
    let respawn = |fast, perk| DeathScreen {
        respawn: Some(RespawnView {
            gauge: 6,
            timer: "06:53".into(),
            perk,
            fast,
        }),
        banner: Some(TipView {
            icon: "ui\\tip.dtx".into(),
            text: "Use cover".into(),
        }),
        ..Default::default()
    };
    let b = build(&respawn(None, false));
    assert!(b.shown("Respawn") && !b.shown("Respawn_EXPass"));
    assert_eq!(b.text("Respawn", "timer"), "06:53");
    assert_eq!(b.fill("Respawn"), 6.0);
    assert!(b.shown("Banner"));
    assert_eq!(b.text("Banner", "banner_description"), "Use cover");
    assert_eq!(b.texture("Banner", "Banner_icon"), "ui/tip.dtx");
    assert!(!b.l.tree.get(b.id("Respawn", "des_FastRespawn")).show);
    // The bar's art: Relentless's, else the normal one.
    let bar = |b: &Built| match &b.l.widget(b.id("Respawn", "respawn_gauge")).kind {
        Kind::Gauge(g) => g.bar.clone(),
        _ => unreachable!(),
    };
    assert_eq!(bar(&b), "ui/gauge_normal.dtx");
    assert_eq!(bar(&build(&respawn(None, true))), "ui/gauge_perk.dtx");
    // Case 8: 2 left of 3.
    let b = build(&respawn(Some(FastView::Free { left: 2, max: 3 }), false));
    assert_eq!(b.text("Respawn", "des_FastRespawn_2"), "Press F7 to respawn now (2 left)");
    assert_eq!(b.text("Respawn", "des_FastRespawn"), "Count : [2/3]");
    // The premium pass: its own panel.
    let b = build(&respawn(Some(FastView::Premium), false));
    assert!(!b.shown("Respawn") && b.shown("Respawn_EXPass"));
    // Nothing: neither.
    let b = build(&DeathScreen::default());
    assert!(!b.shown("Respawn") && !b.shown("Respawn_EXPass") && !b.shown("Banner") && !b.shown("Target"));
}

/// 6. Space, Ctrl and Shift (either side), F7, the wheel and the movement
/// keys are the physical keys and wheel of the dead player's input.
#[test]
fn the_keys_into_the_death_input() {
    use bevy::{
        input::{
            ButtonInput,
            mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
        },
        prelude::*,
    };
    use combat_arms::{game::death_cam::CaDeathInput, ui::hud::death::read_input};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<CaDeathInput>()
        .add_systems(Update, read_input);
    app.update();
    let input = |app: &App| app.world().resource::<CaDeathInput>().0;
    assert_eq!((input(&app).space, input(&app).ctrl, input(&app).shift, input(&app).fast), (false, false, false, false));
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::Space);
        keys.press(KeyCode::ControlRight);
        keys.press(KeyCode::F7);
    }
    *app.world_mut().resource_mut::<AccumulatedMouseScroll>() = AccumulatedMouseScroll {
        unit: MouseScrollUnit::Line,
        delta: Vec2::new(0.0, 2.0),
    };
    app.update();
    let i = input(&app);
    assert!(i.space && i.ctrl && i.fast && !i.shift);
    assert_eq!(i.wheel, 2.0);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    app.update();
    assert!(input(&app).shift && !input(&app).space);
}

/// The death camera's pose is the shell's camera while dead or waiting,
/// and not otherwise.
#[test]
fn the_death_camera_on_the_shell() {
    use bevy::prelude::*;
    use chimera_client::client::spectate::SpecView;
    use combat_arms::{game::death_cam::CaDeathCam, ui::hud::death::place_death_camera};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SpecView>()
        .init_resource::<CaDeathCam>()
        .add_systems(Update, place_death_camera);
    let pose = (Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_y(0.5));
    app.world_mut().resource_mut::<CaDeathCam>().pose = Some(pose);
    app.update();
    assert_eq!(app.world().resource::<SpecView>().pose, None, "not dead");
    app.world_mut().resource_mut::<CaDeathCam>().waiting = true;
    app.update();
    assert_eq!(app.world().resource::<SpecView>().pose, Some(pose));
}

/// 28. Dead in TDM watching an enemy: that enemy's tag shows, every
/// teammate's, the other enemies' only under the aim ray; in Hunted every
/// player's.
#[test]
fn case_28_name_tags_while_dead() {
    let dead_me = TagInputs {
        local_alive: false,
        alive: true,
        seen: true,
        ..Default::default()
    };
    let watched_enemy = name_tag(TagInputs { chased: true, ..dead_me }).expect("the watched player");
    assert_eq!(watched_enemy.name, TAG_ENEMY);
    assert_eq!(name_tag(TagInputs { teammate: true, ..dead_me }).expect("a teammate").name, TAG_MATE);
    assert_eq!(name_tag(dead_me), None, "another enemy");
    assert!(name_tag(TagInputs { aimed: true, ..dead_me }).is_some(), "under the aim ray");
    // Occluded tags are drawn at alpha 125.
    assert_eq!(name_tag(TagInputs { chased: true, seen: false, ..dead_me }).unwrap().opacity, 125);
    // Hunted: everyone.
    assert!(name_tag(TagInputs { everyone: true, ..dead_me }).is_some());
    // Dead players' tags are the dead colour.
    assert_eq!(name_tag(TagInputs { alive: false, body_visible: true, ..dead_me }).unwrap().name, TAG_DEAD);
}

/// The killing gun shows by its display name, as the HUD names it (the
/// item table's; another game's gun by the name an app gave it), not
/// its id (a party death screen read `m136_at4allstar`); with nothing
/// naming it, the id's last part in capitals.
#[test]
fn the_killing_gun_shows_by_its_display_name() {
    use combat_arms::ui::hud::{ItemNames, death::name_weapon};
    let screen = |id: &str| DeathScreen {
        weapon: Some(WeaponView { name: id.rsplit(':').next().unwrap().into(), id: id.into() }),
        ..Default::default()
    };
    let mut items = ItemNames::default();
    items.0.insert("combat_arms:m136_at4allstar".into(), "AT4 All-Star".into());
    items.0.insert("cs_source:weapon_ak47".into(), "AK-47".into());
    let catalog = chimera_sim::weapon::data::WeaponCatalog::default();
    let named = |id: &str| {
        let mut s = screen(id);
        name_weapon(&mut s, &items, &catalog);
        s.weapon.unwrap().name
    };
    assert_eq!(named("combat_arms:m136_at4allstar"), "AT4 All-Star");
    assert_eq!(named("cs_source:weapon_ak47"), "AK-47");
    assert_eq!(named("combat_arms:unnamed_gun"), "UNNAMED GUN");
    let mut none = DeathScreen::default();
    name_weapon(&mut none, &items, &catalog);
    assert!(none.weapon.is_none(), "a suicide names no gun");
}
