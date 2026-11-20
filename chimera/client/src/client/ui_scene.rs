//! `ui_scene <name>` (console): stages a screen of the UI kit
//! (`chimera_ui`) with made-up data for screenshots
//! (docs/plans/active/ugg-ui.md, section 5); `ui_scene off` clears it.
//! Scenes: `chip`, `lead` (the race chip, chasing or leading), `board`,
//! `board_lead` (the Tab board open), `feed` (five kills in the feed),
//! `rankup` (the rank-up toast's loop on the chip: a roll, then a chain;
//! `rankup <t>` frozen `t` s into the loop, the UI clock stopped until
//! `ui_scene off`), `airstrike` (the effects' loop: moon gravity's chip,
//! an air strike's banner, edge pulses and fold into the chip stack;
//! `airstrike <t>` frozen likewise):
//! the HUD's own screens with made-up players (`race_chip::stage`,
//! `kill_feed::signal_feed::stage`), and `matchend [fixture.json] [chat|busy]` (the match end, with chat lines, with
//! eight players, the climb and the vote: `match_end_signal::stage`),
//! `runghint` (the rung hint: G, THROW; `rung_hint`),
//! `lobby` (the pre-match lobby with made-up players, names as typed:
//! a `NetLobby` of its own, `StagedLobby`), under the Signal theme; given a
//! fixture (`ui_scene board <fixture.json>`), its data over a flat ground
//! instead (the UI parity check, `ui_parity`). `proof` (the kit itself: the type ramp of every role, tick
//! bars, slants, the gun icons and marks, the logo pass frozen mid-way,
//! the ambient and the match end's lines behind). `controls` (the kit's
//! form controls in their states: `chimera_ui::controls`), `fields` (the
//! text inputs and the value controls in every state: rest, hover, focus,
//! typing with the caret mid-text, a selection, masked, error, disabled:
//! the gallery to check a change to the kit against). An app adds
//! its own screens' scenes (`UiSceneHooks`: the party's `menu`, `esc`,
//! `settings <tab>`, `play [game]` and `join`), staged by the app's code and cleared at `off`. The screens' milestones add theirs
//! (`lobby`, `chip`, `board`, `toast`, `airstrike`, `matchend`, `menu`)
//! to `SCENES`. An app's own screens register theirs in `UiSceneHooks`
//! (`menu`: the party menu's first page, `party_kit::signal_menu::stage`).
//! The shell's chat registers `chat [open|notice]` and `chat_lobby` the
//! same way (`signal_chat`).
//!
//! A scene is a root of its own over everything (`UI_SCENE_Z`), on the
//! default UI camera, laid out for the window's size at the time it is
//! staged, again whenever that changes (`chimera_ui::UiUnits`), from the app's `Theme` and faces
//! (`KitFonts`; the shell's default theme without them).
//!
//! `ui_scene_name <name>` calls every made-up player of the HUD scenes
//! `name` (`StagedName`: a 20-character name's fit, screenshots).
//!
//! `ui_gallery [fields|controls|picker|off]` opens the kit's gallery in the
//! real apps (Esc closes it; `picker`: the shared picker in both modes and
//! both frames, made-up maps and guns); the Escape menu's Developer entry runs it while
//! Options' Developer mode (`DeveloperMode`, `developer_mode`) is on.
//!
//! `+ui_scene proof` on the command line stages it at start, e.g.
//! `ultimate_gun_game --offscreen 1920x1080 --frames 30 --screenshot
//! <out>.png +ui_scene proof`.

use bevy::prelude::*;
use chimera_core::console::ConsoleAppExt;
use chimera_ui::{
    Ease, IconImages, KitFonts, LinesMaterial, PassMaterial, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout,
    Tone, UiUnits, fonts, icons,
    widgets::{self, PingLevel, TagKind},
};

/// Above every overlay and menu.
pub const UI_SCENE_Z: i32 = 900;

/// The scenes `ui_scene` stages.
pub const SCENES: &[&str] =
    &["proof", "controls", "fields", "picker", "chip", "lead", "board", "board_lead", "feed", "rankup", "airstrike", "matchend", "lobby", "runghint"];

/// `ui_scene lobby`'s made-up lobby (despawned at `off`).
#[derive(Component)]
pub struct StagedLobby;

/// `ui_scene lobby`: a lobby waiting for players, its names as their
/// players typed them; `ui_scene lobby host`: the host's view of a first
/// lobby waiting for its Start, with two parties (you and two friends,
/// another pair), a stranger and a bot, 8 slots; `ui_scene lobby
/// between`: the host's lobby after a match, the next map coming in on
/// every machine, each row at its own stage (downloading, loading the
/// map, building models, physics); `ui_scene lobby join`: a joiner's view
/// (its own row second, downloading; no Start, a Leave button) of a lobby
/// with the match's settings.
fn stage_lobby(w: &mut World, how: Option<&str>) {
    use chimera_net::prematch::{LobbyRow, NetLobby, status};
    let row = |id: Option<u64>, name: &str, st: u8, ping: Option<u16>, wins: u32| LobbyRow {
        id,
        name: name.into(),
        status: st,
        done: 0,
        total: 0,
        step: chimera_net::prematch::NO_STEP,
        permille: 0,
        ping_ms: ping,
        wins,
        avatar: if id.is_some() { (name.len() as u8 * 7) % 32 } else { chimera_net::prematch::NO_AVATAR },
        party: 0,
    };
    if matches!(how, Some("host" | "between" | "join")) {
        use chimera_net::net::HOST_ID;
        let between = how == Some("between");
        let join = how == Some("join");
        let mut dl = row(Some(2), "AI_alpha_9", status::DOWNLOADING, Some(31), 0);
        (dl.done, dl.total, dl.party, dl.avatar) = (42, 100, 1, 6 * 1 + 8 * 2);
        (dl.step, dl.permille) = (0, 420);
        let mut ld = row(Some(3), "AI_alpha_3", status::LOADING, Some(28), 0);
        (ld.party, ld.avatar) = (1, 8 * 3 + 3);
        (ld.step, ld.permille) = (2, 620);
        let mut me = row(Some(HOST_ID), "Player", if between { status::LOADING } else { status::READY }, None, 0);
        if between {
            (me.step, me.permille) = (3, 810);
        }
        (me.party, me.avatar) = (1, 8 * 1);
        let mut nj = row(Some(4), "Nightjar", status::READY, Some(44), 0);
        (nj.party, nj.avatar) = (2, 8 * 3 + 1);
        let mut wr = row(Some(5), "wren_77", if between { status::LOADING } else { status::CONNECTING }, Some(95), 0);
        (wr.party, wr.avatar) = (2, 8 * 1 + 5);
        if between {
            (wr.step, wr.permille) = (1, 300);
        }
        let mut st = row(Some(6), "Stranger", status::READY, Some(140), 0);
        st.avatar = 8 * 2 + 7;
        w.insert_resource(super::prematch_lobby::StagedLobbyHost { slots: 8, joiner: how == Some("join") });
        w.spawn((
            StagedLobby,
            NetLobby {
                map: "cs_source:de_dust2".into(),
                mode: "Ultimate Gun Game".into(),
                rows: vec![me, dl, ld, nj, wr, st, row(None, "Bot_Kilo", status::READY, None, 0)],
                waiting: if between { 4 } else { 3 },
                wait_left: (between || join).then_some(75),
                countdown: None,
                host_start: !between && !join,
                settings: vec![
                ("Length".into(), "10 min".into()),
                ("Bots".into(), "3".into()),
                ("Cross-game balance".into(), "On".into()),
                ("Knife finale".into(), "On".into()),
            ],
            },
        ));
        return;
    }
    w.spawn((
        StagedLobby,
        NetLobby {
            map: "cs_source:de_dust2".into(),
            mode: "Ultimate Gun Game".into(),
            rows: vec![
                row(Some(1), "Zoë the Kestrel", status::READY, Some(24), 2),
                row(Some(2), "nadia.exe", status::NOT_READY, Some(61), 0),
                row(Some(3), "Björn_Quigley", status::LOADING, Some(130), 1),
                row(None, "Rook", status::READY, None, 0),
            ],
            waiting: 2,
            wait_left: Some(72),
            countdown: None,
            host_start: true,
            settings: vec![
                ("Length".into(), "10 min".into()),
                ("Bots".into(), "3".into()),
                ("Cross-game balance".into(), "On".into()),
                ("Knife finale".into(), "On".into()),
            ],
        },
    ));
}

/// The scene asked for (`None`: none).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct UiScene {
    pub want: Option<String>,
    shown: Option<String>,
    /// A scene froze the UI clock (`ui_scene <name> <t>`): it runs again
    /// at `off`.
    froze: bool,
}

impl UiScene {
    /// The kit's gallery is open (`ui_gallery`): it owns the mouse
    /// (`input::FreeCursorOwners`); the staged scenes don't.
    pub fn gallery_open(&self) -> bool {
        self.want.as_deref().is_some_and(|n| matches!(n, "fields" | "controls"))
    }
}

/// Where a scene's loop starts on the UI clock: now, or (`t` given) `t`
/// s ago with the clock frozen there (a screenshot of one moment).
fn scene_start(world: &mut World, t: Option<f64>) -> f64 {
    let mut clock = world.get_resource_or_init::<chimera_ui::UiClock>();
    match t {
        Some(t) => {
            clock.manual = true;
            let now = clock.now;
            world.resource_mut::<UiScene>().froze = true;
            now - t
        }
        None => clock.now,
    }
}

/// Every staged HUD scene cleared, the clock running again if a scene
/// froze it.
fn clear_staged(w: &mut World) {
    let lobbies: Vec<Entity> = w.query_filtered::<Entity, With<StagedLobby>>().iter(w).collect();
    for e in lobbies {
        w.despawn(e);
    }
    w.remove_resource::<super::prematch_lobby::StagedLobbyHost>();
    super::race_chip::stage(w, None);
    super::rank_toast::stage(w, None);
    super::match_effects::stage(w, None);
    super::match_end_signal::unstage(w);
    if let Some(mut h) = w.get_resource_mut::<super::rung_hint::RungHintView>() {
        h.staged = None;
    }
    if std::mem::take(&mut w.resource_mut::<UiScene>().froze)
        && let Some(mut c) = w.get_resource_mut::<chimera_ui::UiClock>()
    {
        c.manual = false;
    }
}

/// A staged scene's root.
#[derive(Component)]
pub struct UiSceneRoot;

/// An app's scene: stages it from the arguments after its name (`off`
/// when every scene is cleared).
pub type SceneHook = fn(&mut World, &[String]) -> Result<(), String>;

/// `ui_scene_name <name>`: every made-up player of the staged HUD scenes
/// (`chip`, `lead`, `board`, `board_lead`, `feed`, `matchend`) is called
/// `name` (a long name's fit: `chimera_ui::Ellipsize`); `ui_scene_name`
/// alone gives them their own again. Applies at the next `ui_scene`.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct StagedName(pub Option<String>);

/// The staged scenes' players renamed (`StagedName`).
pub fn rename_staged(w: &mut World) {
    let Some(name) = w.get_resource::<StagedName>().and_then(|n| n.0.clone()) else { return };
    if let Some(mut r) = w.get_resource_mut::<super::race_chip::RaceStaged>() {
        for row in &mut r.view.rows {
            row.name = name.clone();
        }
    }
    if w.contains_resource::<super::kill_feed::signal_feed::FeedStaged>()
        && let Some(mut f) = w.get_resource_mut::<super::kill_feed::KillFeed>()
    {
        for e in &mut f.entries {
            e.victim.name = name.clone();
            if let Some(k) = e.killer.as_mut() {
                k.name = name.clone();
            }
        }
    }
    if let Some(mut m) = w.get_resource_mut::<super::match_end::MatchEndStaged>() {
        for row in &mut m.view.rows {
            row.name = name.clone();
        }
        if m.view.winner.is_some() {
            m.view.winner = Some(name.clone());
        }
    }
}

/// Scenes of screens an app draws itself (the party menu's `menu`), by
/// name: `ui_scene <name> ...` calls the hook.
#[derive(Resource, Default, Clone)]
pub struct UiSceneHooks(pub Vec<(&'static str, SceneHook)>);

impl UiSceneHooks {
    /// Adds (or replaces) `name`'s hook.
    pub fn add(world: &mut World, name: &'static str, hook: SceneHook) {
        let mut h = world.get_resource_or_init::<UiSceneHooks>();
        h.0.retain(|(n, _)| *n != name);
        h.0.push((name, hook));
    }
}

pub struct UiScenePlugin;

impl Plugin for UiScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiScene>()
            .init_resource::<DeveloperMode>()
            .init_resource::<StagedName>()
            .console_command(
                "ui_scene_name",
                "ui_scene_name [name]: the staged HUD scenes' made-up players all called `name` (a long name's fit), from the \
                 next ui_scene; alone: their own names.",
                |w, a| {
                    let name = a.join(" ");
                    w.resource_mut::<StagedName>().0 = (!name.is_empty()).then_some(name);
                    Ok(None)
                },
            )
            .console_command(
                "ui_scene",
                "ui_scene <proof|controls|fields|chip|lead|board|board_lead|feed|rankup|airstrike|matchend|lobby|runghint|off> [fixture.json | t]: stage a screen of the UI \
                 kit with made-up data (screenshots), or a fixture's (the UI parity check); an animated scene (rankup, airstrike) \
                 with `t` frozen `t` s into its loop.",
                |w, a| {
                    let name = a.first().map(String::as_str).unwrap_or("proof");
                    // An animated scene's frozen time.
                    let t = || a.get(1).map(|t| t.parse::<f64>().map_err(|_| format!("ui_scene: not a time: {t}"))).transpose();
                    if name == "off" {
                        w.resource_mut::<UiScene>().want = None;
                        let hooks: Vec<SceneHook> = w.get_resource::<UiSceneHooks>().map(|h| h.0.iter().map(|(_, f)| *f).collect()).unwrap_or_default();
                        for hook in hooks {
                            hook(w, &["off".to_string()])?;
                        }
                        clear_staged(w);
                        super::ui_parity::clear_ground(w);
                        if w.remove_resource::<super::kill_feed::signal_feed::FeedStaged>().is_some()
                            && let Some(mut f) = w.get_resource_mut::<super::kill_feed::KillFeed>()
                        {
                            f.lifetime = super::kill_feed::LIFETIME;
                            f.entries.clear();
                        }
                        return Ok(None);
                    }
                    // An app's own screen (`UiSceneHooks`: the party menu's).
                    let hook = w.get_resource::<UiSceneHooks>().and_then(|h| h.0.iter().find(|(n, _)| *n == name).map(|(_, f)| *f));
                    if let Some(hook) = hook {
                        clear_staged(w);
                        hook(w, &a[1..])?;
                        return Ok(None);
                    }
                    if !SCENES.contains(&name) {
                        let apps: Vec<&str> = w.get_resource::<UiSceneHooks>().map(|h| h.0.iter().map(|(n, _)| *n).collect()).unwrap_or_default();
                        return Err(format!("ui_scene: no scene `{name}` (scenes: {}, {})", SCENES.join(", "), apps.join(", ")));
                    }
                    // The HUD's own screens, staged with made-up data where
                    // they draw (the race chip and board, the kill feed).
                    clear_staged(w);
                    match name {
                        "rankup" => {
                            let start = scene_start(w, t()?);
                            super::rank_toast::stage(w, Some(start));
                            return Ok(None);
                        }
                        "airstrike" => {
                            // On the chip, as in play.
                            let start = scene_start(w, t()?);
                            super::race_chip::stage(w, Some("chip"));
                            super::match_effects::stage(w, Some(start));
                            return Ok(None);
                        }
                        "chip" | "lead" | "board" | "board_lead" => {
                            super::race_chip::stage(w, Some(name));
                            rename_staged(w);
                            if let Some(path) = a.get(1) {
                                super::ui_parity::apply_fixture(w, name, path)?;
                            }
                            return Ok(None);
                        }
                        "matchend" => {
                            // `matchend [fixture.json] [chat|busy]`
                            let chat = a.iter().skip(1).find(|x| matches!(x.as_str(), "chat" | "busy")).cloned();
                            let fixture = a.get(1).filter(|x| !matches!(x.as_str(), "chat" | "busy"));
                            super::match_end_signal::stage(w, fixture.map(String::as_str))?;
                            rename_staged(w);
                            if let Some(c) = chat {
                                super::signal_chat::stage_end_chat(w, c == "busy");
                            }
                            return Ok(None);
                        }
                        "lobby" => {
                            stage_lobby(w, a.get(1).map(String::as_str));
                            return Ok(None);
                        }
                        "runghint" => {
                            if let Some(mut h) = w.get_resource_mut::<super::rung_hint::RungHintView>() {
                                h.staged = Some(("G".into(), "throw".into()));
                            }
                            return Ok(None);
                        }
                        "feed" => {
                            super::kill_feed::signal_feed::stage(w);
                            rename_staged(w);
                            if let Some(path) = a.get(1) {
                                super::ui_parity::apply_fixture(w, name, path)?;
                            }
                            return Ok(None);
                        }
                        _ => {}
                    }
                    w.resource_mut::<UiScene>().want = Some(name.to_string());
                    Ok(None)
                },
            )
            .console_command(
                "ui_gallery",
                "ui_gallery [fields|controls|picker|off]: open the UI kit's gallery (every control in every state; `fields` by default; \
                 Esc closes it).",
                |w, a| {
                    let name = a.first().map(String::as_str).unwrap_or("fields");
                    if name == "off" {
                        w.resource_mut::<UiScene>().want = None;
                        clear_staged(w);
                        return Ok(None);
                    }
                    if !["fields", "controls", "picker"].contains(&name) {
                        return Err(format!("ui_gallery: no page `{name}` (fields, controls, picker, off)"));
                    }
                    clear_staged(w);
                    w.resource_mut::<UiScene>().want = Some(name.to_string());
                    Ok(None)
                },
            )
            .add_systems(Update, (stage, close_gallery));
        developer_mode_cvar(app);
    }
}

/// Registers `developer_mode` (archived), when there is a console.
pub fn developer_mode_cvar(app: &mut App) {
    if app.world().get_resource::<crate::console::Console>().is_some() {
        app.init_resource::<DeveloperMode>();
        crate::console::resource_cvar::<DeveloperMode, u8>(
            app,
            "chimera:developer_mode",
            "1: the Escape menu shows a Developer entry (the UI kit's gallery, `ui_gallery`); 0: off (default).",
            |s| &mut s.on,
        );
        app.world_mut().resource_mut::<crate::console::Console>().archive("developer_mode");
    }
}

/// Options' "Developer mode" (`developer_mode`, archived, off by
/// default): shows the Escape menu's Developer entry, which opens the
/// gallery (`ui_gallery`).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeveloperMode {
    pub on: u8,
}

impl DeveloperMode {
    pub fn is_on(&self) -> bool {
        self.on != 0
    }
}

/// Esc closes the gallery (`ui_gallery`).
fn close_gallery(keys: Option<Res<ButtonInput<KeyCode>>>, mut scene: ResMut<UiScene>) {
    if scene.want.as_deref().is_some_and(|n| matches!(n, "fields" | "controls" | "picker")) && keys.is_some_and(|k| k.just_pressed(KeyCode::Escape)) {
        scene.want = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn stage(
    mut scene: ResMut<UiScene>,
    roots: Query<Entity, With<UiSceneRoot>>,
    theme: Res<Theme>,
    kit: Option<Res<KitFonts>>,
    units: Res<UiUnits>,
    mut images: ResMut<Assets<Image>>,
    mut icon_images: ResMut<IconImages>,
    mut slants: ResMut<Assets<SlantMaterial>>,
    mut ticks: ResMut<Assets<TickBarMaterial>>,
    mut passes: ResMut<Assets<PassMaterial>>,
    mut lines: ResMut<Assets<LinesMaterial>>,
    mut ambients: ResMut<Assets<chimera_ui::AmbientMaterial>>,
    mut lists: ResMut<chimera_ui::VirtualLists>,
    mut commands: Commands,
) {
    // Again when the window's size or scale changes (and on the frame
    // the window is first measured).
    if scene.want == scene.shown && !(units.is_changed() && scene.shown.is_some()) {
        return;
    }
    for r in &roots {
        commands.entity(r).despawn();
    }
    scene.shown = scene.want.clone();
    if scene.want.as_deref() == Some("proof") {
        let mut m = Makers {
            theme: &theme,
            kit: kit.as_deref(),
            units: *units,
            images: &mut images,
            icon_images: &mut icon_images,
            slants: &mut slants,
            ticks: &mut ticks,
        };
        proof(&mut m, &mut passes, &mut lines, &mut ambients, &mut commands);
    }
    if scene.want.as_deref() == Some("controls") {
        controls(&theme, kit.as_deref(), *units, &mut ambients, &mut commands);
    }
    if scene.want.as_deref() == Some("fields") {
        fields(&theme, kit.as_deref(), *units, &mut ambients, &mut commands);
    }
    if scene.want.as_deref() == Some("picker") {
        picker_page(&theme, kit.as_deref(), *units, &mut ambients, &mut lists, &mut commands);
    }
}

/// The kit's picker (`chimera_ui::picker`) with made-up maps and guns:
/// multi-select rows and cards at a dialog's width (a header's box, an
/// item off, a game some on), and the single pick's cards in a wider
/// frame with the search and the chips (more columns, larger cards; the
/// current gun outlined).
fn picker_page(
    t: &Theme,
    kit: Option<&KitFonts>,
    u: UiUnits,
    ambients: &mut Assets<chimera_ui::AmbientMaterial>,
    lists: &mut chimera_ui::VirtualLists,
    c: &mut Commands,
) {
    use chimera_ui::{
        UiStyle,
        controls::Ui,
        picker::{self, PickerFrame, PickerGame, PickerGroup, PickerItem, PickerLook, PickerMode, PickerState},
    };
    let ui = Ui::new(t, kit, u);
    let root = c
        .spawn((
            UiSceneRoot,
            Name::new("ui_scene picker"),
            GlobalZIndex(UI_SCENE_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                padding: UiRect::axes(u.val(2.0), u.val(2.4)),
                flex_direction: FlexDirection::Row,
                column_gap: u.val(1.6),
                ..default()
            },
            BackgroundColor(t.tone(Tone::Ink)),
        ))
        .id();
    c.spawn((
        MaterialNode(ambients.add(chimera_ui::AmbientMaterial::new(t).freeze(Some(6.0)))),
        Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: percent(100.0), height: percent(100.0), ..default() },
        ChildOf(root),
    ));
    let gun = |id: &str, name: &str, icon: &str, class: &str| PickerItem {
        icon: Some(icon.into()),
        tags: vec![format!("class:{class}")],
        ..PickerItem::new(id, name)
    };
    let guns = vec![
        PickerGame {
            id: "cs_source".into(),
            name: "Counter-Strike: Source".into(),
            groups: vec![PickerGroup {
                name: String::new(),
                items: vec![
                    gun("cs:ak47", "AK-47", "rifle", "rifle"),
                    gun("cs:awp", "AWP", "sniper", "sniper"),
                    gun("cs:mp5", "MP5 Navy", "smg", "smg"),
                    gun("cs:deagle", "Desert Eagle", "pistol", "pistol"),
                    gun("cs:m3", "M3 Super 90", "shotgun", "shotgun"),
                    gun("cs:m249", "M249", "lmg", "lmg"),
                ],
            }],
        },
        PickerGame {
            id: "tf2".into(),
            name: "Team Fortress 2".into(),
            groups: vec![PickerGroup {
                name: String::new(),
                items: vec![
                    gun("tf2:rocket", "Rocket Launcher", "launcher", "launcher"),
                    gun("tf2:minigun", "Minigun", "lmg", "lmg"),
                    gun("tf2:huntsman", "Huntsman", "crossbow", "sniper"),
                ],
            }],
        },
    ];
    let map = |id: &str, parent: Option<&str>| PickerItem {
        parent: parent.map(str::to_string),
        ..PickerItem::new(id, id.rsplit([':', '@']).next().unwrap_or(id))
    };
    let maps = vec![
        PickerGame {
            id: "cs_source".into(),
            name: "Counter-Strike: Source".into(),
            groups: vec![PickerGroup {
                name: "Official maps".into(),
                items: vec![
                    map("cs_source:de_dust2", None),
                    map("cs_source:de_dust2@long", Some("cs_source:de_dust2")),
                    map("cs_source:de_dust2@mid", Some("cs_source:de_dust2")),
                    map("cs_source:cs_office", None),
                    map("cs_source:de_nuke", None),
                ],
            }],
        },
        PickerGame {
            id: "hl2dm".into(),
            name: "Half-Life 2: Deathmatch".into(),
            groups: vec![PickerGroup { name: "Official maps".into(), items: vec![map("hl2dm:dm_lockdown", None), map("hl2dm:dm_overwatch", None)] }],
        },
    ];
    let col = |c: &mut Commands, w: f32, title: &str, note: &str| {
        let k = ui.boxed(c, root, UiStyle { w: Some(w), h: Some(51.0), shrink: Some(0.0), gap: Some(0.55), ..UiStyle::column() });
        ui.section(c, k, title, Some(note));
        k
    };
    let list = || UiStyle { grow: Some(1.0), ..UiStyle::row() };
    // Multi rows (the Maps dialog's): a slice off, a game folded.
    let a = col(c, 26.0, "Multi, rows", "dialog width; box toggles, header folds");
    let mut st = PickerState { off: ["cs_source:de_dust2@mid".to_string(), "cs_source:de_nuke".to_string()].into(), cursor: 3, ..default() };
    st.fold(&maps, &picker::PickerRow::Game(1), false);
    let frame = PickerFrame { w: 26.0, look: PickerLook::List, mode: PickerMode::Multi, ..default() };
    picker::spawn(c, &ui, a, "gallery.picker.rows", &maps, &st, frame, lists, list());
    // Multi cards at a dialog's width: fewer columns, smaller cards.
    let b = col(c, 26.0, "Multi, cards", "dialog width: fewer, smaller cards");
    let st = PickerState { off: ["cs:awp".to_string(), "cs:m3".to_string()].into(), ..default() };
    let frame = PickerFrame { w: 26.0, look: PickerLook::Cards, mode: PickerMode::Multi, ..default() };
    picker::spawn(c, &ui, b, "gallery.picker.cards", &guns, &st, frame, lists, list());
    // Single pick, wider: the search, the chips, the current gun.
    let d = col(c, 40.0, "Single pick, cards", "full screen: more columns; search typed, chips, badges, a family, a pin, a download");
    let st = PickerState { current: Some("cs:ak47".into()), chips: ["class:rifle".to_string(), "class:lmg".to_string(), "class:launcher".to_string()].into(), cursor: 2, ..default() };
    let frame = PickerFrame {
        w: 40.0,
        look: PickerLook::Cards,
        mode: PickerMode::Single,
        search: true,
        facets: vec!["class".into(), "game".into()],
        placeholder: "Find a gun or a tag".into(),
        ..default()
    };
    // The Sandbox's cards: a badge, a family's count, a pinned mark, a
    // download's bar; the search typed into.
    let mut guns = guns;
    for it in guns.iter_mut().flat_map(|g| g.groups.iter_mut()).flat_map(|k| k.items.iter_mut()) {
        it.badge = Some(if it.id.starts_with("tf2") { "TF2" } else { "CS:S" }.into());
        match it.id.as_str() {
            "cs:ak47" => (it.family, it.mark) = (3, Some("1".into())),
            "tf2:rocket" => it.progress = Some(400),
            _ => {}
        }
    }
    let st = PickerState { search_focus: true, search: "r".into(), caret: chimera_ui::text_edit::Caret { at: 1, anchor: 1 }, ..st };
    picker::spawn(c, &ui, d, "gallery.picker.single", &guns, &st, frame, lists, list());
}

/// The kit's form controls in their states (`chimera_ui::controls`), over
/// the frozen ambient.
fn controls(t: &Theme, kit: Option<&KitFonts>, u: UiUnits, ambients: &mut Assets<chimera_ui::AmbientMaterial>, c: &mut Commands) {
    use chimera_ui::controls::{Kind, State, StepperRange, Ui};
    let ui = Ui::new(t, kit, u);
    let root = c
        .spawn((
            UiSceneRoot,
            Name::new("ui_scene controls"),
            GlobalZIndex(UI_SCENE_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                padding: UiRect::axes(u.val(4.6), u.val(3.4)),
                flex_direction: FlexDirection::Row,
                column_gap: u.val(3.0),
                ..default()
            },
            BackgroundColor(t.tone(Tone::Ink)),
        ))
        .id();
    c.spawn((
        MaterialNode(ambients.add(chimera_ui::AmbientMaterial::new(t).freeze(Some(6.0)))),
        Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: percent(100.0), height: percent(100.0), ..default() },
        ChildOf(root),
    ));
    let col = |c: &mut Commands, w: f32| ui.boxed(c, root, chimera_ui::UiStyle { w: Some(w), gap: Some(0.8), ..chimera_ui::UiStyle::column() });
    let left = col(c, 40.0);
    ui.section(c, left, "Buttons", Some("default • focus • disabled • danger"));
    for (k, st) in [(Kind::Primary, State::NONE), (Kind::Primary, State::focused(true)), (Kind::Secondary, State::NONE), (Kind::Secondary, State::focused(true))] {
        let r = ui.boxed(c, left, chimera_ui::UiStyle::row());
        ui.button(c, r, k, "Apply", Some("Ctrl S"), 2.4, st);
    }
    let r = ui.boxed(c, left, chimera_ui::UiStyle { gap: Some(0.5), ..chimera_ui::UiStyle::row() });
    ui.button(c, r, Kind::Danger, "End the game", None, 2.4, State::NONE);
    ui.button(c, r, Kind::Ghost, "Back", Some("Esc"), 2.4, State::NONE);
    ui.button(c, r, Kind::Secondary, "Revert", None, 2.4, State { disabled: true, ..State::NONE });
    ui.section(c, left, "Tabs", None);
    ui.tabs(c, left, &["Video", "Audio", "Controls", "Party"], 1, false);
    ui.section(c, left, "Key binding", None);
    let r = ui.boxed(c, left, chimera_ui::UiStyle { gap: Some(0.5), ..chimera_ui::UiStyle::row() });
    ui.key_field(c, r, &["SPACE".into(), "MWHEELDOWN".into()], false, 8.5, State::NONE);
    ui.key_field(c, r, &[], true, 8.5, State::NONE);
    let right = col(c, 46.0);
    ui.section(c, right, "Rows", Some("focus • changed • error"));
    let (_, v) = ui.row(c, right, "Vertical sync", None, 2.9, State::NONE);
    ui.toggle(c, v, true, State::NONE);
    let (_, v) = ui.row(c, right, "Mouse filter", None, 2.9, State { focus: true, changed: true, ..State::NONE });
    ui.toggle(c, v, false, State::focused(true));
    let (_, v) = ui.row(c, right, "Frame limit", None, 2.9, State::NONE);
    ui.segmented(c, v, &["None", "60", "144", "240", "300"], Some(2), State::NONE);
    let (_, v) = ui.row(c, right, "Field of view", None, 2.9, State::focused(true));
    ui.slider(c, v, 0.6, Some(1.0), "84", 15.0, State::focused(true));
    let (_, v) = ui.row(c, right, "Brightness (gamma)", None, 2.9, State { error: true, ..State::NONE });
    ui.slider(c, v, 0.6, Some(0.6), "2.2", 15.0, State { error: true, ..State::NONE });
    let (_, v) = ui.row(c, right, "Max. players", Some("next match"), 2.9, State::NONE);
    ui.stepper(c, v, "8", StepperRange::new().range(2.0, 16.0).step(1.0).reached(false, false), State::NONE);
    let (_, v) = ui.row(c, right, "Music", None, 2.9, State::NONE);
    ui.dropdown(c, v, &["Party playlist", "The game's loop", "None"], 0, Some(1), 12.0, State::NONE);
}

/// The kit's text inputs and value controls in every state, labelled: rest,
/// hover, focus, typing (caret mid-text), a selection, masked, error,
/// disabled (`chimera_ui::controls`), over the frozen ambient. The gallery
/// a change to the kit is checked against.
fn fields(t: &Theme, kit: Option<&KitFonts>, u: UiUnits, ambients: &mut Assets<chimera_ui::AmbientMaterial>, c: &mut Commands) {
    use chimera_ui::{
        UiStyle,
        controls::{Hover, State, StepperRange, TextCursor, Typing, Ui},
    };
    let ui = Ui::new(t, kit, u);
    let root = c
        .spawn((
            UiSceneRoot,
            Name::new("ui_scene fields"),
            GlobalZIndex(UI_SCENE_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                padding: UiRect::axes(u.val(2.0), u.val(2.4)),
                flex_direction: FlexDirection::Row,
                column_gap: u.val(1.6),
                ..default()
            },
            BackgroundColor(t.tone(Tone::Ink)),
        ))
        .id();
    c.spawn((
        MaterialNode(ambients.add(chimera_ui::AmbientMaterial::new(t).freeze(Some(6.0)))),
        Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: percent(100.0), height: percent(100.0), ..default() },
        ChildOf(root),
    ));
    let col = |c: &mut Commands, w: f32| ui.boxed(c, root, UiStyle { w: Some(w), gap: Some(0.55), ..UiStyle::column() });
    // A labelled cell: the state's name over the control.
    let cell = |c: &mut Commands, col: Entity, label: &str| {
        let r = ui.boxed(c, col, UiStyle { gap: Some(0.15), ..UiStyle::column() });
        ui.text(c, r, Role::Mono, 0.42, Tone::Fg3, label);
        r
    };
    // The look of a hovered control: its own `Hover` tint set at rest.
    let hovered = |c: &mut Commands, e: Entity, over: Color| {
        c.entity(e).insert(BackgroundColor(over));
    };
    let hov = ui.color(Tone::Fg, 0.07);
    let cur = |at: usize, anchor: usize| Some(TextCursor { at, anchor });
    let long = "Counter-Strike: Source";

    let a = col(c, 19.0);
    ui.section(c, a, "Text field", Some("one node a character, caret overlaid"));
    for (label, value, caret, st) in [
        ("Rest, empty", "", None, State::NONE),
        ("Rest, filled", "de_dust2", None, State::NONE),
        ("Focus, no keys", "de_dust2", None, State::focused(true)),
        ("Typing, caret at the end", "de_dust2", cur(8, 8), State::focused(true)),
        ("Typing, caret mid-text", "de_dust2", cur(3, 3), State::focused(true)),
        ("Typing, empty", "", cur(0, 0), State::focused(true)),
        ("Selection", "de_dust2", cur(5, 2), State::focused(true)),
        ("Selected all", long, cur(long.len(), 0), State::focused(true)),
        ("Error", "27x15", None, State { error: true, ..State::NONE }),
        ("Disabled", "de_dust2", None, State { disabled: true, ..State::NONE }),
    ] {
        let r = cell(c, a, label);
        ui.text_field(c, r, value, "Port", caret, false, 17.0, st);
    }
    let r = cell(c, a, "Hover");
    let f = ui.text_field(c, r, "de_dust2", "Port", None, false, 17.0, State::NONE);
    hovered(c, f, hov);
    let r = cell(c, a, "Masked, typing");
    ui.text_field(c, r, "hunter22", "Password", cur(8, 8), true, 17.0, State::focused(true));

    let b = col(c, 24.0);
    ui.section(c, b, "Search field", Some("glass, clear key once typed"));
    for (label, value, caret, st) in [
        ("Rest, empty", "", None, State::NONE),
        ("Rest, filled", "awp", None, State::NONE),
        ("Focus, empty (no caret yet)", "", None, State::focused(true)),
        ("Typing, caret at the end", "ak47", cur(4, 4), State::focused(true)),
        ("Typing, caret mid-text", "ak47", cur(2, 2), State::focused(true)),
        ("Selection", "ak47 rifle", cur(10, 5), State::focused(true)),
        ("Disabled", "awp", None, State { disabled: true, ..State::NONE }),
    ] {
        let r = cell(c, b, label);
        ui.search_field(c, r, value, "Find a gun or a tag", caret, 19.0, st);
    }
    let r = cell(c, b, "Hover");
    let f = ui.search_field(c, r, "awp", "Find a gun", None, 19.0, State::NONE);
    hovered(c, f.field, hov);
    ui.section(c, b, "Text area", Some("wraps, caret overlaid"));
    let r = cell(c, b, "Rest, empty");
    ui.text_area(c, r, "", "What happened? What did you expect?", None, 24.0, 5.0, State::NONE);
    let r = cell(c, b, "Typing, three lines");
    ui.text_area(c, r, "The grenade went through the wall.\n\nIt happened twice on de_dust2 in the second round.", "", cur(34, 34), 24.0, 6.5, State::focused(true));
    let r = cell(c, b, "Selected all, error");
    let text = "Not enough said";
    ui.text_area(c, r, text, "", cur(text.len(), 0), 24.0, 3.2, State { focus: true, error: true, ..State::NONE });

    let d = col(c, 27.0);
    ui.section(c, d, "Toggle", Some("off, on, yes/no, focus, disabled"));
    let r = ui.boxed(c, d, UiStyle { gap: Some(0.5), ..UiStyle::row() });
    ui.toggle(c, r, false, State::NONE);
    ui.toggle(c, r, true, State::NONE);
    ui.toggle_words(c, r, false, State::NONE, chimera_ui::controls::TOGGLE_YES_NO);
    ui.toggle_words(c, r, true, State::NONE, chimera_ui::controls::TOGGLE_YES_NO);
    ui.toggle(c, r, true, State::focused(true));
    ui.toggle(c, r, false, State::focused(true));
    ui.toggle(c, r, true, State { disabled: true, ..State::NONE });
    ui.section(c, d, "Slider", Some("rest, focus, typing the value, disabled"));
    let r = cell(c, d, "Rest");
    ui.slider(c, r, 0.45, Some(0.6), "84", 17.0, State::NONE);
    let r = cell(c, d, "Focus");
    ui.slider(c, r, 0.45, Some(0.6), "84", 17.0, State::focused(true));
    let r = cell(c, d, "Typing the value, caret mid-text");
    ui.slider_ex(c, r, 0.45, Some(0.6), "84", 17.0, State::focused(true), Some(Typing { text: "1234", caret: 2, anchor: 2, error: false, suffix: None }));
    let r = cell(c, d, "Typing, refused");
    ui.slider_ex(c, r, 0.45, Some(0.6), "84", 17.0, State::focused(true), Some(Typing { text: "abc", caret: 3, anchor: 3, error: true, suffix: None }));
    let r = cell(c, d, "Disabled");
    ui.slider(c, r, 0.45, None, "84", 17.0, State { disabled: true, ..State::NONE });
    ui.section(c, d, "Stepper", Some("one border; the box follows the range"));
    let small = StepperRange::new().range(0.0, 10.0).step(1.0);
    let wide = StepperRange::new().range(1.0, 65535.0).step(1.0);
    let unit = StepperRange::new().range(0.5, 4.0).step(0.5).unit("s");
    let r = cell(c, d, "Rest: 0-10, 1-65535, 0.5-4 s");
    let r = ui.boxed(c, r, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    ui.stepper(c, r, "8", small, State::NONE);
    ui.stepper(c, r, "27015", wide, State::NONE);
    ui.stepper(c, r, "1.5 s", unit, State::NONE);
    let r = cell(c, d, "Hover on minus, then plus");
    let r = ui.boxed(c, r, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    for key in [0, 1] {
        let (m, p) = ui.stepper(c, r, "8", small, State::NONE);
        // The tint a hovered key shows (its `Tick`, kept shown).
        let k = if key == 0 { m } else { p };
        c.queue(move |w: &mut World| {
            let kids: Vec<Entity> = w.get::<Children>(k).map(|c| c.iter().collect()).unwrap_or_default();
            for e in kids {
                if w.get::<chimera_ui::controls::Tick>(e).is_some() {
                    w.entity_mut(e).insert((chimera_ui::controls::Tick { always: true }, Visibility::Inherited));
                }
            }
        });
    }
    let r = cell(c, d, "Focus");
    ui.stepper(c, r, "8", small, State::focused(true));
    let r = cell(c, d, "Typing with a selection, refused");
    let r = ui.boxed(c, r, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    ui.stepper_ex(c, r, "10", small, State::focused(true), Some(Typing { text: "10", caret: 2, anchor: 0, error: false, suffix: None }));
    ui.stepper_ex(c, r, "10", small, State::focused(true), Some(Typing { text: "abc", caret: 3, anchor: 3, error: true, suffix: None }));
    let r = cell(c, d, "At its minimum, at its maximum");
    let r = ui.boxed(c, r, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    ui.stepper(c, r, "0", small.reached(true, false), State::NONE);
    ui.stepper(c, r, "10", small.reached(false, true), State::NONE);
    let r = cell(c, d, "Disabled");
    ui.stepper(c, r, "8", small, State { disabled: true, ..State::NONE });

    let e = col(c, 17.0);
    ui.section(c, e, "Buttons", Some("rest, focus, disabled"));
    for (k, st) in [
        (chimera_ui::controls::Kind::Primary, State::NONE),
        (chimera_ui::controls::Kind::Primary, State { disabled: true, ..State::NONE }),
        (chimera_ui::controls::Kind::Secondary, State::NONE),
        (chimera_ui::controls::Kind::Secondary, State::focused(true)),
        (chimera_ui::controls::Kind::Secondary, State { disabled: true, ..State::NONE }),
        (chimera_ui::controls::Kind::Ghost, State::NONE),
        (chimera_ui::controls::Kind::Danger, State::NONE),
    ] {
        let r = ui.boxed(c, e, UiStyle::row());
        ui.button(c, r, k, "Apply", None, 2.4, st);
    }
    ui.section(c, e, "Dropdown", Some("rest, open, focus, disabled"));
    let r = cell(c, e, "Rest");
    ui.dropdown(c, r, &["Party playlist", "The game's loop", "None"], 0, None, 12.0, State::NONE);
    let r = cell(c, e, "Disabled");
    ui.dropdown(c, r, &["Party playlist", "None"], 0, None, 12.0, State { disabled: true, ..State::NONE });
    let r = cell(c, e, "Open");
    ui.dropdown(c, r, &["Party playlist", "The game's loop", "None"], 0, Some(1), 12.0, State::focused(true));
    let _ = Hover { base: Color::NONE, over: Color::NONE };
}

struct Makers<'a> {
    theme: &'a Theme,
    kit: Option<&'a KitFonts>,
    units: UiUnits,
    images: &'a mut Assets<Image>,
    icon_images: &'a mut IconImages,
    slants: &'a mut Assets<SlantMaterial>,
    ticks: &'a mut Assets<TickBarMaterial>,
}

impl Makers<'_> {
    fn text(&self, role: Role, size_cq: f32, text: &str, tone: Tone) -> impl Bundle + use<> {
        let s = fonts::text(self.theme, self.kit, role, self.units.text_px(size_cq));
        (Text::new(s.case(text)), s.bundle(), TextColor(self.theme.tone(tone)))
    }

    fn icon(&mut self, key: &str, h_cq: f32, tone: Tone) -> impl Bundle + use<> {
        let px = self.units.phys(self.units.cq(h_cq)).round().max(1.0) as u32;
        let aspect = self.icon_images.icons().get(key).map_or(1.0, |i| i.aspect());
        let image = self.icon_images.get(self.images, key, px).unwrap_or_default();
        let (h, w) = (px as f32 / self.units.scale_factor, (aspect * px as f32).ceil() / self.units.scale_factor);
        (
            ImageNode { image, color: self.theme.tone(tone), ..default() },
            Node { width: px_val(w), height: px_val(h), ..default() },
        )
    }

    fn slant(&mut self, mat: SlantMaterial, w_cq: f32, h_cq: f32) -> impl Bundle + use<> {
        (
            MaterialNode(self.slants.add(mat)),
            Node { width: self.units.val(w_cq), height: self.units.val(h_cq), ..default() },
        )
    }

    fn tick_bar(&mut self, n: u32, w_cq: f32, h_cq: f32, lit: u32, hot: u32, skew: bool) -> impl Bundle + use<> {
        let u = self.units;
        let layout = TickLayout::new(u.phys(u.cq(w_cq)) as u32, u.phys(u.cq(h_cq)) as u32, n);
        let mut mat = TickBarMaterial::new(self.theme, n, layout);
        if skew {
            mat = mat.skewed(self.theme);
        }
        mat.set(lit, hot);
        (
            MaterialNode(self.ticks.add(mat)),
            Node {
                width: px_val((layout.w as f32 + layout.h as f32 * self.theme.tan()).ceil() / u.scale_factor),
                height: px_val(u.snap(u.cq(h_cq))),
                ..default()
            },
        )
    }
}

fn px_val(v: f32) -> Val {
    Val::Px(v)
}

fn row(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(gap), ..default() }
}

fn column(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(gap), ..default() }
}

/// The kit on one screen (module doc).
fn proof(m: &mut Makers, passes: &mut Assets<PassMaterial>, lines: &mut Assets<LinesMaterial>, ambients: &mut Assets<chimera_ui::AmbientMaterial>, commands: &mut Commands) {
    let u = m.units;
    let t = m.theme;
    let gap = u.cq(1.2);
    let root = commands
        .spawn((
            UiSceneRoot,
            Name::new("ui_scene proof"),
            GlobalZIndex(UI_SCENE_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                padding: UiRect::axes(u.val(4.6), u.val(3.4)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(gap),
                ..default()
            },
            BackgroundColor(t.tone(Tone::Ink)),
        ))
        .id();
    // The ambient and the match end's lines behind, frozen (screenshots
    // compare alike).
    let amb = commands
        .spawn((
            MaterialNode(ambients.add(chimera_ui::AmbientMaterial::new(t).freeze(Some(6.0)))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        ))
        .id();
    commands.entity(root).add_child(amb);
    let bg = commands
        .spawn((
            MaterialNode(lines.add(LinesMaterial::new(t).freeze(Some(6.0)))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        ))
        .id();
    commands.entity(root).add_child(bg);

    // The lockup: the mark and the wordmark.
    let mark = m.icon("mark", 2.05 * 3.3, Tone::Fg);
    let lockup = commands.spawn(row(u.cq(1.8))).id();
    let mark_e = commands.spawn(mark).id();
    let words = commands.spawn(column(u.cq(1.0))).id();
    let ult = commands.spawn(m.text(Role::DisplayRegular, 3.3 * 0.62, "Ultimate", Tone::Fg2)).id();
    let gg = commands.spawn(m.text(Role::Wordmark, 3.3, "Gun Game", Tone::Fg)).id();
    commands.entity(words).add_children(&[ult, gg]);
    let tag = commands.spawn(m.text(Role::Mono, 0.62, "UI kit proof - Signal - M0", Tone::AccText)).id();
    commands.entity(lockup).add_children(&[mark_e, words, tag]);
    for (kind, label) in [(TagKind::You, "You"), (TagKind::Host, "Host"), (TagKind::Accent, "Rung 7")] {
        let e = widgets::spawn_pair(commands, widgets::tag(t, m.kit, &u, kind, label, 0.62));
        commands.entity(lockup).add_child(e);
    }
    let loss = commands.spawn(m.icon("loss", 0.72, Tone::Threat)).id();
    commands.entity(lockup).add_child(loss);
    let fused = widgets::spawn_you_host(commands, m.slants, t, m.kit, &u, 0.62);
    let bot = commands.spawn(widgets::bot_slot(m.images, m.icon_images, t, &u, true, 1.1, Tone::Fg2)).id();
    let k = widgets::spawn_pair(commands, widgets::kbd(t, m.kit, &u, "Tab", 0.62));
    commands.entity(lockup).add_children(&[fused, bot, k]);
    for ms in [12, 72, 140, 240] {
        let e = widgets::spawn_ping_meter(commands, m.slants, t, &u, 0.72 * 0.92, PingLevel::from_ms(t, ms));
        commands.entity(lockup).add_child(e);
    }
    commands.entity(root).add_child(lockup);

    let body = commands.spawn(row(u.cq(4.0))).id();
    commands.entity(body).insert(Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(u.cq(4.0)),
        align_items: AlignItems::FlexStart,
        ..default()
    });
    commands.entity(root).add_child(body);

    // Left: the type ramp.
    let ramp = commands.spawn(column(u.cq(0.7))).id();
    let lines_of = [
        (Role::DisplayBlack, 3.0, "Rank 07 / 14", Tone::Fg),
        (Role::DisplayBold, 1.36, "Play  Join  Options  Quit", Tone::Fg),
        (Role::DisplayBold, 0.94, "AK-47 - Combat Arms", Tone::Fg),
        (Role::DisplayRegular, 0.94, "Desert Eagle .357 Magnum", Tone::Fg2),
        (Role::Mono, 0.66, "K 12  D 04  +2 over lowgrav_larry", Tone::Fg2),
        (Role::Mono, 0.56, "Impact in 3 - stay down", Tone::Threat),
        (Role::Mono, 0.48, "Rifle - Counter-Strike: Source  0123456789", Tone::Fg3),
    ];
    for (role, size, text, tone) in lines_of {
        let label = format!("{} {:.2}cq", role.key(), size);
        let r = commands.spawn(row(u.cq(1.0))).id();
        let a = commands.spawn(m.text(Role::Mono, 0.42, &label, Tone::Fg3)).id();
        let b = commands.spawn(m.text(role, size, text, tone)).id();
        commands.entity(r).add_children(&[a, b]);
        commands.entity(ramp).add_child(r);
    }
    for (you, name) in [(true, "kestrel"), (false, "lowgrav_larry")] {
        let r = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(u.cq(0.6)),
                    width: u.val(24.0),
                    height: u.val(2.15),
                    padding: UiRect::horizontal(u.val(0.6)),
                    ..default()
                },
            ))
            .id();
        if you {
            commands.entity(r).insert((widgets::row_you(t, &u), bevy::picking::hover::Hovered::default()));
        }
        let slot = commands.spawn(widgets::bot_slot(m.images, m.icon_images, t, &u, !you, 0.9, Tone::Fg3)).id();
        let n = commands.spawn(m.text(Role::DisplayBold, 0.78, name, Tone::Fg)).id();
        commands.entity(r).add_children(&[slot, n]);
        commands.entity(ramp).add_child(r);
    }
    commands.entity(body).add_child(ramp);

    // Right: bars, slants, the pass.
    let right = commands.spawn(column(u.cq(1.0))).id();
    let countdown = m.tick_bar(48, 20.0, 1.0, 30, 26, false);
    let chip = m.tick_bar(14, 9.24, 0.6, 9, 14, true);
    let c1 = commands.spawn(countdown).id();
    let c2 = commands.spawn(chip).id();
    let highlight = m.slant(
        SlantMaterial::new(t, t.tone(Tone::Fg5)).edges(false, true).accent(t.tone(Tone::Acc), u.phys(u.cq(0.32))),
        24.0,
        3.5,
    );
    let wipe_k = Ease::OUT.sample(0.6);
    let wiped = m.slant(SlantMaterial::new(t, t.tone(Tone::Acc)).clip(0.0, 1.0 - wipe_k), 21.0, 3.1);
    let banner = m.slant(SlantMaterial::new(t, t.tone(Tone::Threat)).ramp(0.15, 0.9), 24.0, 1.4);
    let h1 = commands.spawn(highlight).id();
    let h2 = commands.spawn(wiped).id();
    let h3 = commands.spawn(banner).id();
    let pips = commands.spawn(row(u.cq(0.3))).id();
    for i in 0..6 {
        let tone = if i < 4 { Tone::Acc } else { Tone::Fg4 };
        let p = m.slant(SlantMaterial::new(t, t.tone(tone)), 0.8, 0.5);
        let e = commands.spawn(p).id();
        commands.entity(pips).add_child(e);
    }
    let mark_px = (u.phys(u.cq(9.33 * 0.75))).round().max(1.0) as u32;
    let mark_img = m.icon_images.get(m.images, "mark", mark_px);
    let passes_row = commands.spawn(row(u.cq(1.0))).id();
    for p in [0.2, 0.42] {
        let mut pass = PassMaterial::new(t, mark_img.clone());
        pass.set_progress(p);
        let e = commands
            .spawn((
                MaterialNode(passes.add(pass)),
                Node { width: u.val(16.0 * 1.5), height: u.val(9.0 * 1.5), ..default() },
                Outline::new(Val::Px(u.hairline()), Val::ZERO, t.tone(Tone::Fg4)),
            ))
            .id();
        commands.entity(passes_row).add_child(e);
    }
    let pass_e = passes_row;
    commands.entity(right).add_children(&[c1, c2, h1, h2, h3, pips, pass_e]);
    commands.entity(body).add_child(right);

    // The gun icons at the kill feed's size and twice it, then the marks.
    for (h, tone) in [(1.25, Tone::Fg), (2.5, Tone::Fg2)] {
        let r = commands.spawn(row(u.cq(1.2))).id();
        for key in icons::GUNS {
            let e = commands.spawn(m.icon(key, h, tone)).id();
            commands.entity(r).add_child(e);
        }
        commands.entity(root).add_child(r);
    }
    let r = commands.spawn(row(u.cq(1.2))).id();
    for (key, tone) in [("crown", Tone::Acc), ("trophy", Tone::Acc), ("kill", Tone::Fg), ("death", Tone::Fg2), ("warn", Tone::Threat)] {
        let e = commands.spawn(m.icon(key, 1.25, tone)).id();
        commands.entity(r).add_child(e);
    }
    commands.entity(root).add_child(r);
}
