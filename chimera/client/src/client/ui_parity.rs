//! The game side of the UI parity check (docs/OBSERVABILITY.md, "UI
//! parity against the look book"; `tools/ui-parity/`): stage a screen
//! with a fixture's data over a flat ground, then write where every named
//! node (`chimera_ui::UiParityId`) was laid out.
//!
//! - `ui_stage <board|chip|chip_lead|feed|off> [fixture.json]` (or
//!   `--fixture <path>` typed in the console): the race chip and board
//!   (`race_chip::RaceStage`, the board held open for `board`) or the kill
//!   feed (its entries replaced, kept from fading) show the fixture's
//!   players, map and kills, over the theme's ink at `STAGE_Z`. The
//!   fixture is the look book's (`tools/ui-parity/fixtures/hud.json`):
//!   `map`, `map_game`, `rungs`, `rows` (name, rung counted from 1,
//!   kills, deaths, gun, game, class, bot, host, you, wins) and `feed`
//!   (killer, class, victim, you_killer, you_victim).
//! - `ui_dump <path> [frames]`: after `frames` more frames (default 30,
//!   so the board's open has settled), writes every named node as JSON:
//!   `{"screen": {"w","h"}, "elements": {id: {x, y, w, h, visible, text?:
//!   {text, font_size, letter_spacing, weight, color}}}}`, physical px
//!   from the top left, the shape `tools/ui-parity/mockup.mjs` writes.
//!
//! On the command line: `ultimate_gun_game --offscreen 1920x1080 --frames
//! 90 --screenshot <png> +ui_stage board <fixture> +ui_dump <json> 60`
//! (`tools/ui-parity/run.sh` does it).
//!
//! Known shortcuts: a fixture's guns get made-up ids (`parity:<class>`),
//! so the board and chip draw the kit's fallback icons, never a game's;
//! the feed's guns are text (no game icon for a made-up id).

use std::path::PathBuf;

use bevy::prelude::*;
use chimera_core::console::ConsoleAppExt;
use chimera_ui::{Theme, Tone, parity};

use crate::core::DamageKind;
use serde::Deserialize;

use super::{
    kill_feed::{FeedName, KillEntry, KillFeed, SharedKillFeed},
    race_chip::{RaceRow, RaceStage, RaceView},
};

/// The flat ground under a staged screen: over the game's HUD and menus
/// under the kill feed (45) and the race chip (`SCOREBOARD_Z`).
pub const STAGE_Z: i32 = 44;

/// The screens `ui_stage` stages.
pub const SCREENS: &[&str] = &["board", "chip", "chip_lead", "feed"];

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
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FixtureKill {
    pub killer: String,
    pub class: String,
    pub victim: String,
    pub you_killer: bool,
    pub you_victim: bool,
}

/// The fallback icons' names, as `'static` for `RaceRow::class`.
const CLASSES: &[&str] = &[
    "pistol", "revolver", "smg", "shotgun", "rifle", "lmg", "sniper", "energy", "launcher", "grenade", "knife", "crossbow",
    "melee",
];

fn class(name: &str) -> Option<&'static str> {
    CLASSES.iter().copied().find(|c| *c == name)
}

/// A made-up weapon id for a class (leaked once per class).
fn gun_id(class: &str) -> Option<&'static str> {
    use std::{collections::HashMap, sync::Mutex};
    static IDS: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
    if class.is_empty() {
        return None;
    }
    let mut ids = IDS.lock().ok()?;
    let ids = ids.get_or_insert_with(HashMap::new);
    Some(*ids.entry(class.to_string()).or_insert_with(|| Box::leak(format!("parity:{class}").into_boxed_str())))
}

impl Fixture {
    pub fn load(path: &str) -> Result<Fixture, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("ui_stage: {path}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("ui_stage: {path}: {e}"))
    }

    /// The chip's and board's view: `lead` the rows where you lead.
    pub fn view(&self, lead: bool) -> RaceView {
        let rows = if lead && !self.rows_lead.is_empty() { &self.rows_lead } else { &self.rows };
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
                })
                .collect(),
            rungs: self.rungs,
            map: self.map.clone(),
            map_game: self.map_game.clone(),
            size: (0, 0),
        }
    }
}

/// What `ui_stage` asked for.
#[derive(Resource, Clone, Debug, Default)]
pub struct UiStage {
    pub screen: Option<String>,
    pub fixture: Option<Fixture>,
    shown: Option<String>,
}

/// A pending `ui_dump`: where, and how many frames from now.
#[derive(Resource, Clone, Debug, Default)]
pub struct UiDump(pub Option<(PathBuf, u32)>);

/// The flat ground's node.
#[derive(Component)]
pub struct StageGround;

pub struct UiParityPlugin;

impl Plugin for UiParityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiStage>()
            .init_resource::<UiDump>()
            .console_command(
                "ui_stage",
                "ui_stage <board|chip|chip_lead|feed|off> [fixture.json]: stage a screen with a fixture's data over a flat ground (the UI parity check).",
                |w, a| {
                    let mut words = a.iter().map(String::as_str).filter(|s| *s != "--fixture");
                    let screen = words.next().unwrap_or("board");
                    if screen == "off" {
                        *w.resource_mut::<UiStage>() = UiStage { shown: w.resource::<UiStage>().shown.clone(), ..default() };
                        return Ok(None);
                    }
                    if !SCREENS.contains(&screen) {
                        return Err(format!("ui_stage: no screen `{screen}` (screens: {})", SCREENS.join(", ")));
                    }
                    let fixture = match words.next() {
                        Some(path) => Fixture::load(path)?,
                        None => return Err("ui_stage: a fixture is needed (tools/ui-parity/fixtures/hud.json)".into()),
                    };
                    let mut s = w.resource_mut::<UiStage>();
                    s.screen = Some(screen.to_string());
                    s.fixture = Some(fixture);
                    Ok(None)
                },
            )
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
            .add_systems(Update, (stage, keep_feed))
            .add_systems(Last, dump.run_if(|d: Res<UiDump>| d.0.is_some()));
    }
}

#[allow(clippy::too_many_arguments)]
fn stage(
    mut stage: ResMut<UiStage>,
    theme: Res<Theme>,
    race: Option<ResMut<RaceStage>>,
    feed: Option<ResMut<KillFeed>>,
    time: Res<Time>,
    ground: Query<Entity, With<StageGround>>,
    mut commands: Commands,
) {
    if stage.screen == stage.shown {
        return;
    }
    stage.shown = stage.screen.clone();
    for e in &ground {
        commands.entity(e).despawn();
    }
    let screen = stage.screen.clone();
    let fixture = stage.fixture.clone().unwrap_or_default();
    let race_on = matches!(screen.as_deref(), Some("board" | "chip" | "chip_lead"));
    let next = RaceStage {
        view: race_on.then(|| fixture.view(screen.as_deref() == Some("chip_lead"))),
        board: screen.as_deref() == Some("board"),
    };
    match race {
        Some(mut r) => *r = next,
        None => commands.insert_resource(next),
    }
    if let Some(mut feed) = feed {
        feed.entries.clear();
        if screen.as_deref() == Some("feed") {
            let now = time.elapsed_secs();
            let name = |n: &str, local: bool| FeedName { entity: Entity::PLACEHOLDER, name: n.to_uppercase(), team: None, local };
            feed.entries = fixture
                .feed
                .iter()
                .enumerate()
                .map(|(i, k)| KillEntry {
                    id: 1_000_000 + i as u64,
                    killer: Some(name(&k.killer, k.you_killer)),
                    victim: name(&k.victim, k.you_victim),
                    weapon: gun_id(&k.class),
                    kind: DamageKind::Generic,
                    headshot: false,
                    suicide: false,
                    team_kill: false,
                    at: now,
                })
                .collect();
            commands.insert_resource(SharedKillFeed);
        }
    }
    if screen.is_some() {
        commands.spawn((
            StageGround,
            GlobalZIndex(STAGE_Z),
            Pickable::IGNORE,
            Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
            BackgroundColor(theme.tone(Tone::Ink)),
        ));
    }
}

/// A staged feed's rows never fade.
fn keep_feed(stage: Res<UiStage>, time: Res<Time>, feed: Option<ResMut<KillFeed>>) {
    if stage.shown.as_deref() != Some("feed") {
        return;
    }
    if let Some(mut feed) = feed {
        let now = time.elapsed_secs();
        if feed.entries.first().is_some_and(|e| now - e.at > 1.0) {
            for e in &mut feed.entries {
                e.at = now;
            }
        }
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
