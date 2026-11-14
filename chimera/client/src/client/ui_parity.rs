//! The game side of the UI parity check (docs/OBSERVABILITY.md, "UI
//! parity against the look book"; `tools/ui-parity/`): stage a screen
//! with a fixture's data over a flat ground, then write where every named
//! node (`chimera_ui::UiParityId`) was laid out.
//!
//! - `ui_scene <chip|lead|board|board_lead|feed> <fixture.json>`
//!   (`ui_scene`'s scenes, given a fixture): the race chip and board, or
//!   the kill feed, show the fixture's players, map and kills instead of
//!   the scene's made-up ones, over the theme's ink (`STAGE_Z`). The
//!   fixture is the look book's data (`tools/ui-parity/fixtures/hud.json`):
//!   `map`, `map_game`, `rungs`, `rows` and `rows_lead` (name, rung
//!   counted from 1, kills, deaths, gun, game, class, bot, host, you,
//!   wins, ping, loss) and `feed` (killer, class, victim, you_killer,
//!   you_victim, killer_bot, victim_bot), oldest first.
//! - `ui_dump <path> [frames]`: after `frames` more frames (default 30,
//!   so the board's open has settled), writes every named node as JSON:
//!   `{"screen": {"w","h"}, "elements": {id: {x, y, w, h, visible, text?:
//!   {text, font_size, letter_spacing, weight, color}}}}`, physical px
//!   from the top left, the shape `tools/ui-parity/mockup.mjs` writes.
//!
//! On the command line: `ultimate_gun_game --offscreen 1920x1080 --frames
//! 150 --screenshot <png> +ui_scene board <fixture> +ui_dump <json> 90`
//! (`tools/ui-parity/run.sh` does it).
//!
//! Known shortcuts: a fixture's guns get made-up ids (`parity:<class>`,
//! `made_up_class`), so the board, chip and feed draw the kit's icons by
//! class, never a game's.

use std::path::PathBuf;

use bevy::prelude::*;
use chimera_core::console::ConsoleAppExt;
use chimera_ui::{Theme, Tone, parity};
use serde::Deserialize;

use super::{
    kill_feed::{FeedName, KillEntry, KillFeed},
    race_chip::{RaceRow, RaceStaged, RaceView},
};
use crate::core::DamageKind;

/// The flat ground under a staged screen: over the game's HUD and menus,
/// under the kill feed (45) and the race chip (`SCOREBOARD_Z`).
pub const STAGE_Z: i32 = 44;

/// A fixture (module doc).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Fixture {
    pub map: String,
    pub map_game: String,
    pub rungs: usize,
    pub rows: Vec<FixtureRow>,
    pub rows_lead: Vec<FixtureRow>,
    pub feed: Vec<FixtureKill>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FixtureRow {
    pub name: String,
    /// Counted from 1, as shown.
    pub rung: usize,
    pub kills: i32,
    pub deaths: u32,
    pub gun: String,
    pub game: String,
    pub class: String,
    pub bot: bool,
    pub host: bool,
    pub you: bool,
    pub wins: u32,
    pub ping: u16,
    /// Losing packets (the loss mark at the worst ping).
    pub loss: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FixtureKill {
    pub killer: String,
    pub class: String,
    pub victim: String,
    pub you_killer: bool,
    pub you_victim: bool,
    pub killer_bot: bool,
    pub victim_bot: bool,
}

/// The kit's gun and mark names a fixture's class can be.
const CLASSES: &[&str] = &[
    "pistol", "revolver", "smg", "shotgun", "rifle", "lmg", "sniper", "energy", "launcher", "grenade", "knife",
    "crossbow", "melee",
];

fn class(name: &str) -> Option<&'static str> {
    CLASSES.iter().copied().find(|c| *c == name)
}

/// The prefix of a fixture gun's made-up id.
const MADE_UP: &str = "parity:";

/// A made-up weapon id for a class (`parity:<class>`).
fn gun_id(class_name: &str) -> Option<&'static str> {
    let c = class(class_name)?;
    // One 'static id a class, built once.
    static IDS: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    let ids = IDS.get_or_init(|| {
        CLASSES
            .iter()
            .map(|c| (*c, &*Box::leak(format!("{MADE_UP}{c}").into_boxed_str())))
            .collect()
    });
    ids.iter().find(|(k, _)| *k == c).map(|(_, id)| *id)
}

/// A made-up fixture gun's class (`parity:smg` is `smg`): the feed draws
/// its icon by it, as it would a catalogued gun's.
pub fn made_up_class(id: &str) -> Option<&'static str> {
    id.strip_prefix(MADE_UP).and_then(class)
}

impl Fixture {
    pub fn load(path: &str) -> Result<Fixture, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("ui_scene: {path}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("ui_scene: {path}: {e}"))
    }

    /// The chip's and board's view: `lead` the rows where you lead.
    pub fn view(&self, lead: bool) -> RaceView {
        let rows = if lead && !self.rows_lead.is_empty() {
            &self.rows_lead
        } else {
            &self.rows
        };
        RaceView {
            active: true,
            rows: rows
                .iter()
                .map(|r| RaceRow {
                    name: r.name.clone(),
                    rung: r.rung.saturating_sub(1),
                    kills: r.kills,
                    deaths: r.deaths,
                    gun: gun_id(&r.class),
                    gun_name: r.gun.clone(),
                    game: r.game.clone(),
                    class: class(&r.class),
                    bot: r.bot,
                    host: r.host,
                    local: r.you,
                    wins: r.wins,
                    ping: r.ping,
                    loss: r.loss,
                })
                .collect(),
            rungs: self.rungs,
            map: self.map.clone(),
            map_game: self.map_game.clone(),
            size: (0, 0),
        }
    }

    /// The feed's entries, oldest first, a second apart up to `now`.
    pub fn kills(&self, now: f32) -> Vec<KillEntry> {
        let n = self.feed.len();
        let side = |name: &str, local: bool, bot: bool| FeedName {
            entity: Entity::PLACEHOLDER,
            name: name.into(),
            team: None,
            local,
            bot,
        };
        self.feed
            .iter()
            .enumerate()
            .map(|(i, k)| KillEntry {
                id: 2_000_000 + i as u64,
                killer: Some(side(&k.killer, k.you_killer, k.killer_bot)),
                victim: side(&k.victim, k.you_victim, k.victim_bot),
                weapon: gun_id(&k.class),
                kind: DamageKind::Bullet,
                headshot: false,
                suicide: false,
                team_kill: false,
                at: now - (n - 1 - i) as f32 * 0.6,
            })
            .collect()
    }
}

/// The flat ground's node.
#[derive(Component)]
pub struct StageGround;

/// Gives staged scene `scene` (already staged by `ui_scene`) the
/// fixture at `path`'s data, over a flat ground.
pub fn apply_fixture(world: &mut World, scene: &str, path: &str) -> Result<(), String> {
    let fixture = Fixture::load(path)?;
    match scene {
        "chip" | "lead" | "board" | "board_lead" => {
            world.insert_resource(RaceStaged {
                view: fixture.view(scene.contains("lead")),
                open: scene.starts_with("board"),
            });
        }
        "feed" => {
            let now = world.get_resource::<Time>().map_or(0.0, |t| t.elapsed_secs());
            let mut feed = world.get_resource_or_init::<KillFeed>();
            feed.entries = fixture.kills(now);
        }
        _ => return Err(format!("ui_scene: `{scene}` takes no fixture")),
    }
    clear_ground(world);
    let ink = world
        .get_resource::<Theme>()
        .map_or(Color::BLACK, |t| t.tone(Tone::Ink));
    world.spawn((
        StageGround,
        GlobalZIndex(STAGE_Z),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(ink),
    ));
    Ok(())
}

/// Takes the flat ground away (`ui_scene off`).
pub fn clear_ground(world: &mut World) {
    let grounds: Vec<Entity> = world
        .query_filtered::<Entity, With<StageGround>>()
        .iter(world)
        .collect();
    for e in grounds {
        world.despawn(e);
    }
}

/// A pending `ui_dump`: where, and how many frames from now.
#[derive(Resource, Clone, Debug, Default)]
pub struct UiDump(pub Option<(PathBuf, u32)>);

pub struct UiParityPlugin;

impl Plugin for UiParityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDump>()
            .console_command(
                "ui_dump",
                "ui_dump <path> [frames]: after frames (30), write every UiParityId node's box and text as JSON (the UI parity check).",
                |w, a| {
                    let path = a.first().ok_or("ui_dump <path> [frames]")?;
                    let frames = a.get(1).map_or(Ok(30), |f| f.parse::<u32>().map_err(|_| format!("ui_dump: not a number: {f}")))?;
                    w.resource_mut::<UiDump>().0 = Some((path.into(), frames));
                    Ok(None)
                },
            )
            .add_systems(Last, dump.run_if(|d: Res<UiDump>| d.0.is_some()));
    }
}

fn dump(
    mut dump: ResMut<UiDump>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    nodes: Query<parity::ParityQueryData>,
) {
    let Some((path, left)) = dump.0.clone() else { return };
    if left > 0 {
        dump.0 = Some((path, left - 1));
        return;
    }
    dump.0 = None;
    let window = windows.iter().next();
    let size = window.map_or(UVec2::ZERO, |w| w.physical_size());
    let rects = parity::collect(&nodes, window.map_or(1.0, |w| w.scale_factor()));
    let mut elements = serde_json::Map::new();
    for r in rects {
        let mut o = serde_json::json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h, "visible": r.visible });
        if let Some(t) = r.text {
            o["text"] = serde_json::json!({
                "text": t.text,
                "font_size": t.font_size,
                "letter_spacing": t.letter_spacing,
                "weight": t.weight,
                "color": t.color,
            });
        }
        elements.insert(r.id, o);
    }
    let out = serde_json::json!({ "source": "game", "screen": { "w": size.x, "h": size.y }, "elements": elements });
    match std::fs::write(&path, serde_json::to_string_pretty(&out).unwrap_or_default()) {
        Ok(()) => info!("ui_dump: wrote {}", path.display()),
        Err(e) => error!("ui_dump: {}: {e}", path.display()),
    }
}
