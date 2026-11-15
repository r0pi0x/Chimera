//! Halo's HUD in a party app (its party layout, `halo_ce::ui::party_app`,
//! `ui::hud::layout::HudLayout::Party`) against everything the party
//! draws over play, from the disc image: a Halo-rung holder's HUD is
//! Halo's in the party layout, and at 1280x720, 1920x1080, 1280x960 and
//! 1024x768 its blocks (the ammunition block and the grenade plate top
//! left, the shield and health block bottom left: `draw::Blocks`) meet
//! none of the shell's overlay nodes laid out by Bevy's UI (the kill
//! feed's rows top right, the minigame panel and its callout:
//! `overlay_guard`'s every-HUD setup), nor the Signal look's places from
//! Ultimate Gun Game's theme (its kill feed top right, the race chip and
//! the rank-up toast top centre, the chat's lines above the bottom-left
//! status band), with the party's grenade switch off (no grenade plate
//! drawn) and on. And a Halo map in the party has no Halo equipment (no
//! grenade to pick up). Passes with a note without the disc image.

use std::sync::Arc;

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_client::client::kill_feed::{FeedName, KillEntry, KillFeed};
use chimera_core::{core::DamageKind, gameplay::Gameplay};
use chimera_world::harness::Sim;
use halo_ce::game::weapons::{HaloHands, grenades::PartyGrenades};
use halo_ce::ui::hud::{
    HaloHudLayout,
    data::HudData,
    layout::{Canvas, HudLayout},
    nodes::{HaloHud, HudLoad, read_donor},
};

const SIZES: [(u32, u32); 4] = [(1280, 720), (1920, 1080), (1280, 960), (1024, 768)];

fn hud_data() -> Option<HudData> {
    if halo_ce::game::INFO.mount().is_none() {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return None;
    }
    Some(read_donor().expect("the donor map's HUD"))
}

/// Screen rectangles (left, top, right, bottom, px) overlap.
fn meet(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// A canvas rectangle in screen px (left, top, right, bottom).
fn screen(c: &Canvas, r: [f32; 4]) -> [f32; 4] {
    r.map(|v| v * c.scale)
}

/// Halo's blocks on screen at the sim's window: (name, rect).
fn blocks(sim: &Sim) -> Vec<(&'static str, [f32; 4])> {
    let hud = sim.app.world().resource::<HaloHud>();
    let c = hud.canvas.expect("drawn");
    assert_eq!(c.layout, HudLayout::Party, "the party's layout");
    let b = hud.blocks;
    [("ammunition", b.weapon), ("grenades", b.grenades), ("shield and health", b.unit)]
        .into_iter()
        .filter_map(|(n, r)| Some((n, screen(&c, r?))))
        .collect()
}

/// The Signal look's places at a `w` x `h` window (px), from Ultimate Gun
/// Game's theme: the feed (top right, as many rows as `rows`), the chip and
/// the toast (top centre), the chat (its six lines at most, growing upward
/// from its bottom edge).
fn signal_places(w: f32, h: f32, rows: f32) -> Vec<(&'static str, [f32; 4])> {
    let theme = chimera_ui::Theme::from_toml(include_str!("../../../../ultimate_gun_game/data/theme.toml")).unwrap();
    let cq = chimera_ui::units::cq(1.0, chimera_ui::units::layout_height(w, h));
    let l = |s: &str, k: &str| theme.layout(s, k) * cq;
    let feed_w = l("feed", "w");
    let feed_top = 2.2 * cq;
    let feed = [w - 2.2 * cq - feed_w, feed_top, w - 2.2 * cq, feed_top + rows * l("feed", "pitch")];
    let chip_w = l("chip", "w");
    let chip = [w / 2.0 - chip_w / 2.0, l("chip", "top"), w / 2.0 + chip_w / 2.0, l("chip", "top") + l("chip", "h")];
    let toast_w = chimera_client::client::rank_toast::TOAST_W * cq;
    let toast = [w / 2.0 - toast_w / 2.0, l("toast", "top"), w / 2.0 + toast_w / 2.0, l("toast", "top") + l("toast", "h")];
    let chat_left = theme.layout_opt("chat", "left").unwrap_or(2.2) * cq;
    let chat_bottom = h - theme.layout_opt("chat", "bottom").unwrap_or(8.4) * cq;
    // Six lines at most (`signal_chat`), each under 2.4 cq with its gap.
    let chat = [chat_left, chat_bottom - 6.0 * 2.4 * cq, chat_left + theme.layout_opt("chat", "w").unwrap_or(30.0) * cq, chat_bottom];
    vec![("Signal feed", feed), ("race chip", chip), ("rank-up toast", toast), ("Signal chat", chat)]
}

#[test]
fn a_halo_rung_holders_hud_leaves_the_partys_overlay_free() {
    let Some(data) = hud_data() else { return };
    let (mut sim, me) = crate::overlay_guard::laid_out();
    // The party's front end (what `party_kit::client` runs for each game's
    // `GameClient::app`) sets the party layout.
    sim.app.insert_resource(HaloHudLayout(HudLayout::Party));
    sim.app.insert_resource(HudLoad::Ready(Arc::new(data)));
    // Six rows in the feed (`overlay_guard::layout` keeps them fresh).
    let w = sim.app.world_mut();
    let at = w.resource::<Time>().elapsed_secs();
    let mut feed = w.resource_mut::<KillFeed>();
    for i in 0..6 {
        feed.entries.push(KillEntry {
            id: 2_000 + i,
            killer: Some(FeedName { entity: Entity::PLACEHOLDER, name: "AI_Alpha_9".into(), local: false, team: Some(1), bot: true }),
            victim: FeedName { entity: Entity::PLACEHOLDER, name: format!("AI_Bravo_{i}"), local: false, team: Some(2), bot: true },
            weapon: None,
            kind: DamageKind::Bullet,
            headshot: false,
            suicide: false,
            team_kill: false,
            at,
        });
    }
    // The party's grenade switch off (its default) and on: no grenade
    // plate, then the plate (a new life's frags).
    for (grenades, (w, h)) in [false, true].into_iter().flat_map(|g| SIZES.map(|s| (g, s))) {
        sim.app.insert_resource(PartyGrenades(grenades));
        if let Some(mut hands) = sim.app.world_mut().get_mut::<HaloHands>(me) {
            hands.armed = false;
        }
        let laid = crate::overlay_guard::layout(&mut sim, me, halo_ce::game::GAME, (w, h));
        assert_eq!(sim.app.world().resource::<Gameplay>().hud, halo_ce::game::GAME);
        let win = {
            let world = sim.app.world_mut();
            world.query_filtered::<&Window, With<PrimaryWindow>>().single(world).map(|w| (w.width(), w.height())).unwrap()
        };
        assert_eq!(win, (w as f32, h as f32));
        let ours = blocks(&sim);
        let names: Vec<&str> = ours.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"ammunition") && names.contains(&"shield and health"), "{names:?} at {w}x{h}");
        assert_eq!(names.contains(&"grenades"), grenades, "the grenade plate with the switch {grenades}: {names:?} at {w}x{h}");
        // The shell's overlay as laid out: every node with words in it
        // (the feed's rows and their names, the panel's lines, the callout).
        let rows: Vec<&crate::overlay_guard::Laid> =
            laid.iter().filter(|l| l.text.contains("AI_Alpha_9") && l.size.x < w as f32 * 0.6).collect();
        assert!(!rows.is_empty(), "the feed's rows at {w}x{h}");
        let rect = |l: &crate::overlay_guard::Laid| [l.centre.x - l.size.x / 2.0, l.centre.y - l.size.y / 2.0, l.centre.x + l.size.x / 2.0, l.centre.y + l.size.y / 2.0];
        let feed = rows.iter().map(|l| rect(l)).reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]).unwrap();
        assert!(feed[0] > w as f32 / 2.0 && feed[1] < h as f32 / 4.0, "the feed top right: {feed:?}");
        for (name, r) in &ours {
            assert!(!meet(*r, feed), "Halo's {name} block {r:?} and the kill feed {feed:?} at {w}x{h}");
            for l in laid.iter().filter(|l| l.font.is_some() && !l.text.trim().is_empty() && l.size.x < w as f32 * 0.6) {
                assert!(!meet(*r, rect(l)), "Halo's {name} block {r:?} and the overlay's {:?} ({:?}) at {w}x{h}", l.text, rect(l));
            }
            for (place, p) in signal_places(w as f32, h as f32, 6.0) {
                assert!(!meet(*r, p), "Halo's {name} block {r:?} and the {place} {p:?} at {w}x{h}");
            }
        }
        eprintln!("{w}x{h}: Halo {ours:?}; feed {feed:?}; Signal {:?}", signal_places(w as f32, h as f32, 6.0));
    }
    // Each of the eight rung guns in hand draws its crosshair in the party
    // layout, as a party crosshair (`draw::Fill::Reticle`: it also hides the
    // frame behind it by its brightness, so it shows on a bright map).
    use halo_ce::game::weapons as hw;
    for id in [hw::ASSAULT_RIFLE, hw::PISTOL, hw::PLASMA_RIFLE, hw::SHOTGUN, hw::SNIPER_RIFLE, hw::ROCKET_LAUNCHER, hw::NEEDLER, hw::PLASMA_PISTOL] {
        let w = sim.app.world_mut();
        let gun = chimera_sim::weapon::give(w, me, id).unwrap_or_else(|| panic!("{id} given"));
        w.get_mut::<chimera_sim::weapon::Inventory>(me).unwrap().active = Some(gun);
        sim.seconds(0.5);
        for _ in 0..2 {
            sim.app.update();
        }
        let hud = sim.app.world().resource::<HaloHud>();
        let c = hud.canvas.expect("drawn");
        let (cx, cy) = (c.width as f32 / 2.0, 240.0);
        let reticles = hud
            .quads
            .iter()
            .filter(|q| matches!(q.fill, halo_ce::ui::hud::draw::Fill::Reticle(_)) && q.rect[0] <= cx && q.rect[2] >= cx && q.rect[1] <= cy && q.rect[3] >= cy)
            .count();
        assert!(reticles > 0, "{id}: no crosshair in the party layout ({:?})", hud.unit.as_ref().and_then(|u| u.weapon.as_ref()).map(|w| &w.tag));
    }
}

/// No Halo equipment on a Halo map in the party: its netgame equipment
/// (the grenades among it) is the kit of a map played by Halo's own rules
/// (`halo_ce::game::weapons::pickups::ENTRY_CLASS` entities, read only
/// when the map's game is the gameplay's, `map::KitLoad::pickups`), so the
/// party's load of Hang 'Em High and Blood Gulch has none, while Halo's
/// own load of the same map has its grenades.
#[test]
fn no_halo_grenade_pickups_on_a_party_halo_map() {
    use chimera_world::map::MapLoadOptions;
    use halo_ce::game::weapons::pickups::ENTRY_CLASS;
    if halo_ce::game::INFO.mount().is_none() {
        eprintln!("skipping: no Halo CE disc image");
        return;
    }
    let entries = |m: &chimera_world::map::MapData| m.entities.iter().filter(|e| e.classname() == ENTRY_CLASS).count();
    for id in ["halo_ce:hangemhigh", "halo_ce:bloodgulch"] {
        let party = party_kit::load_map_with(id, &MapLoadOptions::lean()).unwrap();
        assert_eq!(entries(&party), 0, "{id}: no equipment in the party");
        let own = (halo_ce::game::MAP_LOADER.load)(id.split_once(':').unwrap().1, &MapLoadOptions::lean()).unwrap();
        assert!(entries(&own) > 0, "{id}: Halo's own load has its equipment");
    }
}
