//! The kill feed shows the weapon whatever game's gun killed and whatever
//! HUD is up: on CS:S's HUD a Combat Arms gun is drawn with Combat Arms'
//! icon (from the shared registry, `client::weapon_icons::WeaponIcons`),
//! else by its name; CS:S's own gun keeps its glyph path. The other way
//! round (Combat Arms' feed with a CS:S gun) is `combat_arms_ui::hud`'s
//! own tests. Headless, on the party with a stand-in CS:S HUD look that
//! has a notices panel.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
use chimera_core::core::{Damage, DamageKind, Died, Hitgroup};
use chimera_world::{harness::Sim, map::hud::{ActiveHud, HudCoord, HudPanel}};
use cs_source_ui::game_hud::Part;

use crate::hud::party_with_huds;

/// CS:S's HUD up with a notices panel and a window to lay it out on.
fn feed_sim() -> (Sim, Entity, Entity) {
    let (mut sim, me, other) = party_with_huds();
    let w = sim.app.world_mut();
    w.spawn(Window::default());
    let mut look = w.resource_mut::<ActiveHud>();
    let hud = Arc::make_mut(&mut look.0);
    hud.panels.insert(
        "HudDeathNotice".into(),
        HudPanel {
            x: HudCoord::Start(0.0),
            y: HudCoord::Start(0.0),
            wide: 300.0,
            tall: 100.0,
            background: None,
            icon: Vec2::ZERO,
            digit: Vec2::ZERO,
            digit2: Vec2::ZERO,
            keys: Default::default(),
        },
    );
    sim.seconds(0.2);
    (sim, me, other)
}

fn kill_with(sim: &mut Sim, victim: Entity, killer: Entity, weapon: &'static str) {
    sim.app.world_mut().write_message(Died {
        entity: victim,
        attacker: Some(killer),
        damage: Damage {
            target: victim,
            attacker: Some(killer),
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: Some(weapon),
            force: Vec3::ZERO,
        },
    });
    sim.seconds(0.3);
}

/// The feed's lines: (the words, the images) of its first row.
fn feed(sim: &mut Sim) -> (Vec<String>, usize) {
    let w = sim.app.world_mut();
    let parts: Vec<Part> = w.query::<&Part>().iter(w).copied().collect();
    let root = w.query::<(Entity, &Part)>().iter(w).find(|(_, p)| **p == Part::Notices).unwrap_or_else(|| panic!("the notices in {parts:?}, look {}", w.contains_resource::<ActiveHud>())).0;
    let rows = w.get::<Children>(root).map(|c| c.to_vec()).unwrap_or_default();
    assert_eq!(rows.len(), 1, "one kill, one row");
    let mut words = vec![];
    let mut images = 0;
    for part in w.get::<Children>(rows[0]).expect("the row's parts").to_vec() {
        if let Some(t) = w.get::<Text>(part) {
            words.push(t.0.clone());
        }
        images += w.get::<ImageNode>(part).is_some() as usize;
    }
    (words, images)
}

#[test]
fn another_games_gun_is_drawn_with_its_icon() {
    let (mut sim, me, other) = feed_sim();
    sim.app.world_mut().resource_mut::<WeaponIcons>().register("combat_arms", |_| {
        Some(WeaponIcon::Image { image: Handle::default(), size: Vec2::new(64.0, 20.0) })
    });
    kill_with(&mut sim, me, other, "combat_arms:m4a1");
    let (words, images) = feed(&mut sim);
    assert_eq!(images, 1, "Combat Arms' icon in CS:S's feed: {words:?}");
    assert_eq!(words.len(), 2, "the two names only: {words:?}");
}

#[test]
fn another_games_gun_without_an_icon_is_its_name() {
    let (mut sim, me, other) = feed_sim();
    kill_with(&mut sim, me, other, "combat_arms:m4a1");
    let (words, images) = feed(&mut sim);
    assert_eq!(images, 0);
    assert_eq!(words.len(), 3, "attacker, the gun's name, victim: {words:?}");
    assert_eq!(words[1], "M4A1");
}
