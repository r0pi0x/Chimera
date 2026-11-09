//! Everything that needs a window: local input, the local player's camera,
//! debug tools and agent-facing tools (screenshots, remote inspection).
//! Simulation code must never depend on this module.
//!
//! Local input (mouse look as CS:S: `sensitivity` x `m_yaw`/`m_pitch` degrees
//! per count; Left Alt / `+freelook` turns only the camera; E or `+use` sets
//! `Intent::use_key`; a click grabs the mouse (not while the console, a menu or
//! the debug UI is open, nor on an egui window), Esc opens the game menu, which
//! frees it), first-person camera (FOV from `Zoomed`), debug UI, `--screenshot`
//! and the `screenshot <file>` and `jpeg` (CS:S's F5: the screenshots folder,
//! `<map>NNNN.png`) console commands (`capture.rs`), remote protocol.

pub mod aim;
pub mod announcer;
pub mod callouts;
pub mod hit_marker;
pub mod canvas;
pub mod app;
pub mod audio;
pub mod binds;
pub mod capture;
pub mod chat;
pub mod console;
pub mod debug;
pub mod debug_ui;
pub mod debug_views;
pub mod dialog;
pub mod effects;
pub mod clip_quad;
pub mod fonts;
pub mod front_end;
pub mod color_correction;
pub mod gamma;
pub mod hdr;
pub mod hud;
pub mod hud_minimal;
pub mod minigame_panel;
pub mod minigame_scoreboard;
pub mod hud_text;
pub mod input;
pub mod kill_feed;
pub mod interp;
pub mod map_screen;
pub mod match_end;
pub mod net;
pub mod net_graph;
pub mod night_vision;
pub mod frame_metrics;
pub mod perf;
pub mod respawn_countdown;
#[cfg(feature = "trace")]
pub mod trace_capture;
pub mod game_text;
pub mod senses;
pub mod spectate;
pub mod swap_effect;
pub mod vgui;
pub mod video;
pub mod view;
pub mod voice;
pub mod weapon_icons;
pub mod weapon_menu;
pub mod ui_write;
pub mod widgets;


use std::path::PathBuf;

use bevy::prelude::*;

use crate::{
    character::spawn_character,
    core::{Intent, LocalPlayer, MovementState, SpawnPoint, Team},
    slots::{Loadout, MovementRegistry},
};

/// Command-line options. Run with `--help` for the list.
#[derive(Clone, Default, Debug)]
pub struct Args {
    /// Save a screenshot here after `frames` frames, then exit.
    pub screenshot: Option<PathBuf>,
    pub frames: Option<u32>,
    /// Movement implementation ID for the local player (overrides the loadout).
    pub movement: Option<String>,
    /// Spawn position in meters, overriding the map's spawn points.
    pub spawn: Option<Vec3>,
    /// Initial look direction: yaw and pitch in degrees.
    pub look: Option<Vec2>,
    /// Map ID such as `cs_source:de_dust2`, or `greybox`. Without it the
    /// greybox is loaded behind the main menu (see `starts_in_game`).
    pub map: Option<String>,
    /// Debug view: white surfaces, only baked lighting.
    pub lightmap_only: bool,
    /// Debug render for comparisons: `lighting` (x0.25) or `albedo`, with no
    /// tonemapping and a magenta background.
    pub debug_view: Option<String>,
    /// Capture these views (JSON, see `capture::View`), one PNG each.
    pub views: Option<PathBuf>,
    pub capture_dir: Option<PathBuf>,
    /// With `views`: time each view instead of capturing it.
    pub bench: bool,
    /// Console variables to set (`name=value`), in order, after any
    /// `exec` files.
    pub cvars: Vec<(String, String)>,
    /// Source-style config files (`name value` per line) to apply.
    pub exec: Vec<PathBuf>,
    /// Console commands from `+name args...` (Source style), run at startup.
    pub console: Vec<String>,
    /// Start with the console open.
    pub console_open: bool,
    /// Window size in physical pixels (screenshots at a known size).
    pub window: Option<UVec2>,
    /// The window's scale factor in place of the display's (with
    /// `--window`: 1), e.g. 1.25 as Windows at 125 %.
    pub scale: Option<f32>,
    /// Size of `--views` captures and `--bench` frames (default 1280x720).
    pub view_size: Option<UVec2>,
    /// `--join`: how the local player comes into games, in place of
    /// `join_choice`'s pick.
    pub join: Option<crate::core::JoinChoice>,
}

const USAGE: &str = "\
usage: cs_source [options]
  --screenshot <file.png>   save a screenshot after --frames frames, then exit
  --frames <n>              frames to run before the screenshot or exit (default 60)
  --movement <id>           movement implementation for the local player
  --spawn <x,y,z>           spawn position in meters
  --look <yaw,pitch>        initial look angles in degrees (yaw 0 = -Z)
  --map <game:name>         load a game's map, e.g. cs_source:de_dust2, or greybox (Chimera's
                            test map). Without it the game starts at the main menu, unless
                            --spawn, --look, --movement or --views is given (then: greybox)
  --cvar <name=value>       set a movement console variable, e.g. sv_airaccelerate=150
                            (repeatable; cs_source: sv_accelerate, sv_airaccelerate,
                            sv_friction, sv_stopspeed, sv_gravity, sv_maxspeed,
                            sv_stepsize, sv_maxvelocity, sv_bounce,
                            sv_enablebunnyhopping, sv_autobunnyhopping, cl_forwardspeed)
  --exec <file.cfg>         apply a Source-style config file (\"name value\" lines)
  --lightmap-only           debug view: white surfaces, only baked lighting
  --debug-view <kind>       lighting (x0.25) | albedo, untonemapped, magenta background
  --views <file.json>       capture each view (name, position, yaw, pitch; engine space)
                            off-screen at 1280x720, then exit (use with --movement chimera:noclip)
  --view-size <WxH>         size of --views captures and --bench frames (default 1280x720),
                            e.g. 3840x2160 to time 4K
  --capture-dir <dir>       where --views writes <name>.png (default: current directory)
  --bench                   with --views: time frames at each view (no vsync) and print
                            avg/p95/max frame ms and drawn meshes instead of capturing
  --console                 start with the console open (~ toggles it)
  --window <WxH>            window size in pixels, e.g. 1920x1080 (screenshots at a known size;
                            a tiling window manager may still resize it)
  --scale <f>               the window's scale factor (physical pixels per logical one) in place of
                            the display's, e.g. 1.25 as Windows at 125 % (with --window: 1)
  --join <how>              how you come into a game you start or join: menus (CS:S's message of
                            the day, team and class menus), auto (the smaller team) or a jointeam
                            number (1 spectators, 2 terrorists, 3 counter-terrorists). Default:
                            menus, but auto in --screenshot/--views runs, with --spawn or --look,
                            and with +jointeam/+spectate (that team)
  --playdemo <file.dem>     watch a demo (the game's playdemo command: its map loads, then it plays)
  -port <n>                 UDP port to host on (hostport; default 27015; tool runs: --screenshot,
                            --views, --frames, --bench host on 127.0.0.1 on a port the system picks)
  +<command> [args...]      run a console command at startup, Source style
                            (e.g. +sv_airaccelerate 150 +cl_showpos 1 +bind f noclip;
                            ++attack holds an action, e.g. to fire in a --screenshot run)

network play (docs/OBSERVABILITY.md, \"Network play\"):
  host:    cs_source -port 27016 +maxplayers 4 +map greybox
  join:    cs_source +connect 127.0.0.1:27016
  server:  cargo run --bin cs_source_server -- -port 27016 +map greybox";

impl Args {
    /// Whether the run starts playing (a map given, or the player placed
    /// or its movement or views chosen: the greybox then) rather than at
    /// the main menu.
    pub fn starts_in_game(&self) -> bool {
        self.map.is_some() || self.spawn.is_some() || self.look.is_some() || self.movement.is_some() || self.views.is_some()
    }

    /// How the local player comes into the games it starts or joins
    /// (`rules::JoinChoice`): `--join`, else CS:S's join menus unless the
    /// run is scripted: a capture (`--screenshot`, `--views`) or the
    /// player placed (`--spawn`, `--look`) comes in on the smaller team,
    /// and a team asked for on the command line (`+jointeam <n>`,
    /// `+spectate`) on that one.
    pub fn join_choice(&self) -> crate::core::JoinChoice {
        use crate::core::JoinChoice;
        if let Some(j) = self.join {
            return j;
        }
        for words in self.console.iter().flat_map(|l| crate::console::parse(l)) {
            match words.first().map(|w| w.to_lowercase()).as_deref() {
                Some("jointeam") => {
                    return words.get(1).and_then(|n| JoinChoice::from_key(n)).unwrap_or(JoinChoice::Auto);
                }
                Some("spectate") => return JoinChoice::Team(crate::core::Team(0)),
                _ => {}
            }
        }
        let scripted = self.screenshot.is_some() || self.views.is_some() || self.spawn.is_some() || self.look.is_some();
        if scripted { JoinChoice::Auto } else { JoinChoice::Menus }
    }

    /// The game map to load at startup: `--map` unless it names the
    /// greybox (`greybox`, `chimera:greybox` or the old `mashup:greybox`), which is there anyway.
    pub fn game_map(&self) -> Option<&str> {
        self.map
            .as_deref()
            .filter(|m| !matches!(m.to_lowercase().as_str(), "greybox" | "chimera:greybox" | "mashup:greybox"))
    }

    pub fn parse() -> Self {
        Self::parse_from(std::env::args().skip(1)).unwrap_or_else(|e| {
            eprintln!("{e}\n\n{USAGE}");
            std::process::exit(2);
        })
    }

    pub fn parse_from(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        fn floats<const N: usize>(flag: &str, v: &str) -> Result<[f32; N], String> {
            let parts: Vec<f32> = v
                .split(',')
                .map(|p| p.trim().parse::<f32>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("{flag}: expected {N} comma-separated numbers, got {v:?}"))?;
            parts
                .try_into()
                .map_err(|_| format!("{flag}: expected {N} numbers, got {v:?}"))
        }

        let mut out = Self::default();
        let mut it = args.into_iter().peekable();
        while let Some(flag) = it.next() {
            // `+command args...`: words up to the next option or +command.
            if let Some(name) = flag.strip_prefix('+').filter(|n| !n.is_empty()) {
                let mut line = vec![crate::console::quote(name)];
                while let Some(next) = it.peek() {
                    if next.starts_with('+') && next.len() > 1 || next.starts_with("--") || next == "-port" {
                        break;
                    }
                    line.push(crate::console::quote(&it.next().unwrap()));
                }
                out.console.push(line.join(" "));
                continue;
            }
            // Source's `-port <n>`: the port a server listens on (before
            // any +map that hosts).
            if flag == "-port" {
                let port = it.next().ok_or("-port: missing value")?;
                port.parse::<u16>().map_err(|_| format!("-port: not a port: {port}"))?;
                out.console.insert(0, format!("hostport {port}"));
                continue;
            }
            if flag == "--console" {
                out.console_open = true;
                continue;
            }
            if flag == "--help" || flag == "-h" {
                println!("{USAGE}");
                std::process::exit(0);
            }
            if flag == "--bench" {
                out.bench = true;
                continue;
            }
            if flag == "--lightmap-only" {
                out.lightmap_only = true;
                continue;
            }
            let value = it.next().ok_or_else(|| format!("{flag}: missing value"))?;
            match flag.as_str() {
                "--screenshot" => out.screenshot = Some(value.into()),
                "--frames" => out.frames = Some(value.parse().map_err(|_| format!("--frames: not a number: {value}"))?),
                "--movement" => out.movement = Some(value),
                "--map" => out.map = Some(value),
                "--views" => out.views = Some(value.into()),
                "--debug-view" => out.debug_view = Some(value),
                "--capture-dir" => out.capture_dir = Some(value.into()),
                "--spawn" => out.spawn = Some(Vec3::from_array(floats::<3>(&flag, &value)?)),
                "--look" => out.look = Some(Vec2::from_array(floats::<2>(&flag, &value)?)),
                "--join" => {
                    out.join = Some(
                        crate::core::JoinChoice::from_key(&value)
                            .ok_or_else(|| format!("--join: expected menus, auto, 1, 2 or 3, got {value}"))?,
                    )
                }
                "--cvar" => {
                    let (name, v) = value
                        .split_once('=')
                        .ok_or_else(|| format!("--cvar: expected name=value, got {value}"))?;
                    out.cvars.push((name.to_string(), v.to_string()));
                }
                "--exec" => out.exec.push(value.into()),
                // The game's `playdemo` (CS:S: `games::cs_source::ui::demo`).
                "--playdemo" => out.console.push(format!("playdemo {}", crate::console::quote(&value))),
                "--scale" => {
                    let s: f32 = value.parse().map_err(|_| format!("--scale: not a number: {value}"))?;
                    if !(0.5..=4.0).contains(&s) {
                        return Err(format!("--scale: expected 0.5-4, got {value}"));
                    }
                    out.scale = Some(s);
                }
                "--window" | "--view-size" => {
                    let size = value
                        .split_once(['x', 'X'])
                        .and_then(|(w, h)| Some(UVec2::new(w.trim().parse().ok()?, h.trim().parse().ok()?)))
                        .filter(|s| s.x > 0 && s.y > 0)
                        .ok_or_else(|| format!("{flag}: expected WxH, got {value}"))?;
                    if flag == "--window" {
                        out.window = Some(size);
                    } else {
                        out.view_size = Some(size);
                    }
                }
                _ => return Err(format!("unknown option {flag}")),
            }
        }
        // A tool run hosts (if `maxplayers` asks it to) on loopback, on a
        // port the system picks unless `-port` names one: several at once
        // on one machine, or beside a real CS:S, don't fight over 27015.
        if out.tool_run() {
            let explicit_port = out.console.first().is_some_and(|l| l.starts_with("hostport "));
            if !explicit_port {
                out.console.insert(0, "hostport 0".into());
            }
            out.console.insert(0, "ip 127.0.0.1".into());
        }
        Ok(out)
    }

    /// Whether this is a scripted run that captures or exits by itself
    /// (`--screenshot`, `--views`, `--frames`, `--bench`): the tools'
    /// (refcmp, mapsweep, agents' screenshots). It hosts on loopback on a
    /// port the system picks (see `parse_from`), and with `dev` serves
    /// the remote protocol on one too unless `CHIMERA_REMOTE_PORT` says.
    pub fn tool_run(&self) -> bool {
        self.screenshot.is_some() || self.views.is_some() || self.frames.is_some() || self.bench
    }
}

#[derive(Resource, Clone)]
pub struct ClientArgs(pub Args);

/// What the HUD (kill feed, chat, radio, scoreboard) calls a character:
/// the server's name in a network game (`net::NetCharacter`, ours too);
/// in single player our own player by the `name` cvar ("Player" unless
/// set), others by their `Name`, else their id.
pub fn shown_name(
    e: Entity,
    local: bool,
    net: Option<&crate::net::NetCharacter>,
    name: Option<&Name>,
    settings: Option<&crate::net::NetSettings>,
) -> String {
    match (net, local, name) {
        (Some(c), ..) => c.name.clone(),
        (None, true, _) => settings.map_or_else(|| "Player".to_string(), |s| s.name.clone()),
        (None, false, Some(n)) => n.to_string(),
        (None, false, None) => e.to_string(),
    }
}

/// What the app tells the shell about itself: its build's commit, how it
/// loads a game's map (by id, or from a downloaded file) and reads a
/// map's file, and its built-in test map (the greybox). The app fills it
/// (the CS:S app: `games::load_map_level`, `greybox`), so the shell names
/// neither a game's loaders nor the greybox. Without it (tests), maps
/// don't load and the greybox isn't put back.
#[derive(Resource, Clone, Copy)]
pub struct AppHooks {
    /// The build's commit (`git describe`), for bench files.
    pub build: &'static str,
    /// Map `id` (`game:name`) with `options` (`hdr_level`: the
    /// `mat_hdr_level`; `progress`: where it reports its stages).
    pub load_map: fn(&str, &crate::map::MapLoadOptions) -> Result<crate::map::MapData, String>,
    /// Map `id` from a copy of its file (a server's map downloaded).
    pub load_map_file: fn(&str, &std::path::Path, &crate::map::MapLoadOptions) -> Result<crate::map::MapData, String>,
    /// The bytes of map `id`'s file, for the network's map offer.
    pub map_file_bytes: fn(&str) -> Option<Vec<u8>>,
    /// Take the greybox out (a map loads in its place).
    pub unload_greybox: fn(&mut World),
    /// Put the greybox back and everyone on it.
    pub respawn_greybox: fn(&mut World),
}

/// The client shell's plugins, in order: its setup (the command line,
/// `AppHooks`), then input, debug tools, captures, the console, the HUD
/// base, cameras, chat, senses, spectating, video, network glue, voice,
/// sound and picture settings, and last the local player's camera
/// (`ShellCameraPlugin`). A game's front end puts its HUD among them at
/// its place (`PluginGroupBuilder::add_after`, CS:S's
/// `games::cs_source::ui::with_hud`), so the systems keep their order.
pub struct ClientPlugins {
    pub args: Args,
    pub hooks: AppHooks,
}

impl PluginGroup for ClientPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        bevy::app::PluginGroupBuilder::start::<Self>()
            .add(ShellSetupPlugin {
                args: self.args,
                hooks: self.hooks,
            })
            .add(fonts::FontsPlugin)
            .add(input::LocalInputPlugin)
            .add(debug::DebugPlugin)
            .add(debug_views::DebugViewsPlugin)
            .add(capture::CapturePlugin)
            .add(perf::PerfPlugin)
            .add(console::ConsoleUiPlugin)
            .add(net_graph::NetGraphPlugin)
            .add(hud::HudPlugin)
            .add(announcer::AnnouncerPlugin)
            .add(game_text::GameTextPlugin)
            .add(effects::ShotEffectsPlugin)
            .add(effects::GenericImpactsPlugin)
            .add(view::ViewPlugin)
            .add(vgui::VguiPlugin)
            .add(chat::ChatPlugin)
            .add(senses::SensesPlugin)
            .add(spectate::SpectatePlugin)
            .add(respawn_countdown::RespawnCountdownPlugin)
            .add(match_end::MatchEndPlugin)
            .add(kill_feed::KillFeedPlugin)
            .add(callouts::CalloutsPlugin)
            .add(hit_marker::HitMarkerPlugin)
            .add(hdr::HdrPlugin)
            .add(video::VideoPlugin)
            .add(hud_text::HudTextPlugin)
            .add(clip_quad::ClipQuadPlugin)
            .add(interp::ClientInterpPlugin)
            .add(color_correction::ColorCorrectionPlugin)
            .add(net::ClientNetPlugin)
            .add(voice::VoicePlugin)
            .add(map_screen::MapScreenPlugin)
            .add(widgets::WidgetsPlugin)
            .add(audio::SoundOptionsPlugin)
            .add(gamma::GammaPlugin)
            .add(night_vision::NightVisionPlugin)
            .add(ShellCameraPlugin)
    }
}

/// The shell's resources from the command line and the app: the map it
/// starts on, `ClientArgs`, `AppHooks`, how the local player joins.
struct ShellSetupPlugin {
    args: Args,
    hooks: AppHooks,
}

impl Plugin for ShellSetupPlugin {
    fn build(&self, app: &mut App) {
        if let Some(map) = &self.args.map {
            app.insert_resource(crate::map::LoadedMapName(map.clone()));
        }
        app.insert_resource(ClientArgs(self.args.clone()))
            .insert_resource(self.hooks)
            .init_resource::<crate::map::loading::LoadReporter>()
            .insert_resource(self.args.join_choice());
    }
}

/// The local player and its first-person camera (last of the shell's
/// plugins), and the remote protocol with `dev`.
struct ShellCameraPlugin;

impl Plugin for ShellCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, (spawn_local_player, join_at_start).chain())
            .add_systems(Update, camera_for_local_player)
            .add_systems(Update, (follow_eye, zoom_camera).after(spectate::SpectateSet));

        // Bevy Remote Protocol: query and edit the live ECS over HTTP
        // (JSON-RPC on localhost:15702, or CHIMERA_REMOTE_PORT: two games at
        // once, e.g. a host and a client). See docs/OBSERVABILITY.md.
        #[cfg(feature = "dev")]
        {
            use bevy::remote::http::{DEFAULT_PORT, DEFAULT_RENDER_PORT, HostPort};
            let tool_run = app.world().get_resource::<ClientArgs>().is_some_and(|a| a.0.tool_run());
            let port = chimera_core::env::var("CHIMERA_REMOTE_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                // A tool run: a free port (printed), so runs at once don't
                // answer each other's queries on 15702.
                .or_else(|| tool_run.then(free_loopback_port).flatten())
                .unwrap_or(DEFAULT_PORT);
            if port != DEFAULT_PORT {
                info!("remote protocol on 127.0.0.1:{port}");
            }
            app.add_plugins((
                // `mashup/console`: its old name, for scripts written before.
                bevy::remote::RemotePlugin::default()
                    .with_method_main("chimera/console", console::remote_exec)
                    .with_method_main("mashup/console", console::remote_exec),
                bevy::remote::http::RemoteHttpPlugin::default().with_port(port),
            ));
            // The render world's server (15703, no setter): beside the
            // default only; elsewhere a port the system picks.
            if port != DEFAULT_PORT
                && let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp)
                && render.world().get_resource::<HostPort>().is_some_and(|p| p.0 == DEFAULT_RENDER_PORT)
            {
                render.insert_resource(HostPort(0));
            }
        }
    }
}

/// A TCP port free on loopback now (for the remote protocol of a tool
/// run; another process could take it before the game binds it, which
/// costs only remote queries).
#[cfg(feature = "dev")]
fn free_loopback_port() -> Option<u16> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).ok()?;
    listener.local_addr().ok().map(|a| a.port())
}

#[derive(Component)]
pub struct FirstPersonCamera;

fn spawn_local_player(
    mut commands: Commands,
    spawns: Query<&Transform, With<SpawnPoint>>,
    loadout: Res<Loadout>,
    registry: Res<MovementRegistry>,
    args: Res<ClientArgs>,
) {
    let args = &args.0;
    let at = match args.spawn {
        Some(pos) => Transform::from_translation(pos),
        None => spawns.iter().next().copied().unwrap_or_default(),
    };
    // Face the way the spawn point does unless --look says otherwise. The
    // character itself is never rotated: the look angles turn the camera.
    let spawn_yaw = at.rotation.to_euler(EulerRot::YXZ).0.to_degrees();
    let at = Transform::from_translation(at.translation);
    let movement = match &args.movement {
        Some(id) => match registry.get(id) {
            Some(m) => m.id,
            None => {
                let known: Vec<_> = registry.0.iter().map(|m| m.id).collect();
                error!("--movement {id}: unknown. Registered: {known:?}");
                loadout.movement
            }
        },
        None => loadout.movement,
    };

    // Counter-terrorists by default; a game started on a map shows the
    // join menus or joins as asked (`join_at_start`).
    let player = spawn_character(&mut commands, at, Team(2), movement);
    let look = args.look.unwrap_or(Vec2::new(spawn_yaw, 0.0));
    commands.entity(player).insert((
        LocalPlayer,
        Intent {
            yaw: look.x.to_radians(),
            pitch: look.y.to_radians(),
            ..default()
        },
    ));
    attach_camera(&mut commands, player, args.debug_view.is_some());
}

/// A run that starts on a map (`--map`): the local player comes in as
/// `rules::JoinChoice` says (CS:S's join menus, or on a team at once:
/// `join_menus::start_game`).
fn join_at_start(world: &mut World) {
    if world.resource::<ClientArgs>().0.map.is_some() {
        front_end::start_game(world);
    }
}

/// The first-person camera on a new local player (one the network sent:
/// the client's own character, `net::client`).
fn camera_for_local_player(
    players: Query<(Entity, Option<&Children>), Added<LocalPlayer>>,
    cameras: Query<(), With<FirstPersonCamera>>,
    args: Res<ClientArgs>,
    mut commands: Commands,
) {
    for (player, children) in &players {
        if children.is_some_and(|c| c.iter().any(|c| cameras.contains(c))) {
            continue;
        }
        attach_camera(&mut commands, player, args.0.debug_view.is_some());
    }
}

/// The local player's first-person camera, its view model anchor and
/// sound listener, following the map's presentation.
fn attach_camera(commands: &mut Commands, player: Entity, debug_view: bool) {
    commands.entity(player).with_child((
        FirstPersonCamera,
        // The local player's weapon is drawn here (map::view_model).
        crate::map::ViewModelAnchor,
        crate::audio::sound::SoundListener,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            // Source's fov 90 (horizontal at 4:3), kept vertically:
            // 2 atan(3/4) = 73.74 degrees.
            fov: 2.0 * 0.75f32.atan(),
            ..default()
        }),
    ));
    // Follow the map's presentation (e.g. no tonemapping for Source LDR).
    commands.queue(|world: &mut World| {
        let Some(look) = world.get_resource::<crate::map::ActiveMapLook>().cloned() else {
            return;
        };
        if look.0.tonemapping {
            return;
        }
        let mut q = world.query_filtered::<Entity, With<FirstPersonCamera>>();
        let cams: Vec<Entity> = q.iter(world).collect();
        for cam in cams {
            world
                .entity_mut(cam)
                .insert(bevy::core_pipeline::tonemapping::Tonemapping::Linear);
        }
    });
    if debug_view {
        // Raw values for analysis: no tonemapping, an unmistakable background.
        commands.queue(move |world: &mut World| {
            let mut q = world.query_filtered::<Entity, With<FirstPersonCamera>>();
            let cams: Vec<Entity> = q.iter(world).collect();
            for cam in cams {
                world.entity_mut(cam).insert((
                    bevy::core_pipeline::tonemapping::Tonemapping::Linear,
                    Camera {
                        clear_color: ClearColorConfig::Custom(Color::srgb(1.0, 0.0, 1.0)),
                        ..default()
                    },
                ));
            }
        });
    }
}

/// The first-person camera's field of view: `fov_desired` (Source's 90,
/// horizontal at 4:3; `video::PlayerFov`), or the zoom the local player looks through (`weapon::Zoomed`).
/// Kept vertically, so it is the same on every screen shape; the view
/// model and sky cameras follow it.
pub fn zoom_camera(
    player: Option<Single<Option<&crate::weapon::Zoomed>, With<LocalPlayer>>>,
    spectating: Res<spectate::SpecView>,
    zoomed: Query<&crate::weapon::Zoomed>,
    mut cameras: Query<&mut Projection, With<FirstPersonCamera>>,
    player_fov: Option<Res<video::PlayerFov>>,
) {
    // Spectating in first person: the target's zoom; otherwise none.
    let zoom = match spectate::target_zoom(&spectating, &zoomed) {
        Some(z) => z,
        None if spectating.pose.is_some() => None,
        None => player.and_then(|z| z.map(|z| z.fov)),
    };
    let fov_43 = zoom.unwrap_or_else(|| player_fov.map_or(90.0, |f| f.0));
    let fov = crate::map::view_model::vertical_fov(fov_43).to_radians();
    for mut projection in &mut cameras {
        if let Projection::Perspective(p) = projection.as_ref()
            && (p.fov - fov).abs() > 1e-6
            && let Projection::Perspective(p) = projection.as_mut()
        {
            p.fov = fov;
        }
    }
}

/// Place the camera at the movement implementation's eye position (or
/// behind it in third person) and aim it along the look angles.
#[allow(clippy::type_complexity)]
fn follow_eye(
    players: Query<
        (
            &Transform,
            &Intent,
            &MovementState,
            &Children,
            Option<&crate::weapon::ViewPunch>,
            Option<&crate::map::interp::RenderedView>,
            Option<&crate::core::MapView>,
            Option<&avian3d::prelude::Collider>,
        ),
        (With<LocalPlayer>, Without<FirstPersonCamera>),
    >,
    mut cameras: Query<&mut Transform, With<FirstPersonCamera>>,
    shake: Res<senses::ViewShake>,
    mode: Res<view::CameraMode>,
    free: Res<input::FreeLook>,
    mut freecam: ResMut<view::FreeCam>,
    spatial: avian3d::prelude::SpatialQuery,
    characters: Query<Entity, With<Intent>>,
    watch: Res<view::Watch>,
    spectating: Res<spectate::SpecView>,
    others: Query<
        (&Name, &Transform, &Intent, &MovementState, Option<&crate::map::interp::RenderedView>),
        (Without<LocalPlayer>, Without<FirstPersonCamera>),
    >,
) {
    // Positions (`Transform`), eyes and other characters' looks are the
    // ones drawn this frame, eased between ticks (`map::interp`); the
    // local player's look is this frame's mouse.
    // The watched character, if any: a chase camera behind it (world space).
    let watched = (watch.0 > 0)
        .then(|| format!("Bot {}", watch.0))
        .and_then(|name| others.iter().find(|(n, ..)| n.as_str() == name))
        .map(|(_, t, i, s, v)| {
            let chase = view::CameraMode {
                third_person: true,
                ..*mode
            };
            let eye = crate::map::interp::eye_view(v, i, s);
            let look = view::camera_look(&chase, Quat::from_euler(EulerRot::YXZ, eye.yaw, eye.pitch, 0.0));
            let offset = view::camera_offset(&chase, t.translation, eye.eye_offset, look, &spatial, &characters);
            (t.translation + offset, look)
        });
    for (at, intent, state, children, punch, view, map_view, hull) in &players {
        let eye = match view {
            Some(v) => v.now,
            None => crate::map::interp::EyeView {
                punch: punch.map_or(Vec3::ZERO, |p| p.0),
                ..crate::map::interp::EyeView::of(intent, state)
            },
        };
        // Recoil kicks the view (pitch up, yaw left).
        let p = eye.punch;
        // A hard landing or a head hit rolls it (Source's roll turns it
        // clockwise).
        let look = Quat::from_euler(
            EulerRot::YXZ,
            intent.yaw + p.y + free.yaw,
            intent.pitch + p.x + free.pitch,
            -(eye.view_roll + p.z),
        );
        let look = view::camera_look(&mode, look);
        // The ceiling probe is no wider than the player's own hull.
        let hull_radius = hull.map_or(f32::INFINITY, view::hull_radius);
        let offset =
            view::camera_offset_for_hull(&mode, at.translation, eye.eye_offset, look, &spatial, &characters, hull_radius);
        // Detached: starts where the camera is; placed in the world (the
        // camera is the player's child, so undo the player's transform).
        // Spectating while dead comes first; then the debug cameras.
        // A map camera (point_viewcontrol) the player views through.
        let camera = map_view.map(|v| (v.origin, v.rotation));
        let (offset, look) = if let Some((p, q)) = spectating.pose.or(watched).or(camera) {
            let inv = at.compute_affine().inverse();
            (inv.transform_point3(p), at.rotation.inverse() * q)
        } else if freecam.mode == 0 {
            freecam.bypass_change_detection().at = None;
            (offset, look)
        } else {
            if freecam.at.is_none() {
                let (yaw, pitch, _) = look.to_euler(EulerRot::YXZ);
                freecam.at = Some((*at * offset, yaw, pitch));
            }
            let (p, q) = freecam.rotation().unwrap();
            let inv = at.compute_affine().inverse();
            (inv.transform_point3(p), at.rotation.inverse() * q)
        };
        let mut cams = cameras.iter_many_mut(children).matched();
        // Blasts shake the eye's view (not a detached camera's).
        let (offset, look) = if watched.is_none() && freecam.mode == 0 && spectating.pose.is_none() && camera.is_none() {
            (offset + shake.offset, look * Quat::from_rotation_z(shake.roll))
        } else {
            (offset, look)
        };
        while let Some(mut cam) = cams.fetch_next() {
            cam.translation = offset;
            cam.rotation = look;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Args {
        Args::parse_from(line.split_whitespace().map(String::from)).unwrap()
    }

    #[test]
    fn the_main_menu_unless_the_run_starts_playing() {
        assert!(!args("").starts_in_game());
        assert!(!args("--screenshot a.png --window 1280x720 +menu options").starts_in_game());
        assert!(args("--map cs_source:de_dust2").starts_in_game());
        assert!(args("--spawn 0,1,12").starts_in_game());
        assert!(args("--movement chimera:noclip").starts_in_game());
        // The greybox is a map name; no game map to load for it.
        let a = args("--map greybox");
        assert!(a.starts_in_game() && a.game_map().is_none());
        assert_eq!(args("--map cs_source:de_nuke").game_map(), Some("cs_source:de_nuke"));
    }

    #[test]
    fn the_join_menus_unless_scripted_or_asked_for_a_team() {
        use crate::core::JoinChoice;
        assert_eq!(args("").join_choice(), JoinChoice::Menus);
        assert_eq!(args("--map cs_source:de_dust2").join_choice(), JoinChoice::Menus);
        assert_eq!(args("--map cs_source:de_dust2 --screenshot a.png").join_choice(), JoinChoice::Auto);
        assert_eq!(args("--spawn 0,1,12").join_choice(), JoinChoice::Auto);
        assert_eq!(args("+map de_dust2 +jointeam 2").join_choice(), JoinChoice::Team(Team(1)));
        assert_eq!(args("+jointeam 0").join_choice(), JoinChoice::Auto);
        assert_eq!(args("+spectate").join_choice(), JoinChoice::Team(Team(0)));
        assert_eq!(args("--screenshot a.png --join menus").join_choice(), JoinChoice::Menus);
        assert_eq!(args("--join 3").join_choice(), JoinChoice::Team(Team(2)));
        assert!(Args::parse_from(["--join".to_string(), "x".to_string()]).is_err());
    }

    #[test]
    fn tool_runs_host_on_loopback_on_a_port_the_system_picks() {
        // Normal play: Source's 27015 on every interface (the defaults).
        assert!(args("+maxplayers 4 +map de_dust2").console.iter().all(|l| !l.starts_with("ip ")));
        assert_eq!(args("-port 27016 +map de_dust2").console[0], "hostport 27016");
        for run in ["--screenshot a.png", "--views v.json", "--frames 100", "--bench --views v.json"] {
            let a = args(&format!("{run} +maxplayers 4 +map de_dust2"));
            assert!(a.tool_run(), "{run}");
            assert_eq!(a.console[..2], ["ip 127.0.0.1", "hostport 0"], "{run}");
        }
        // `-port` still names one (on loopback).
        let a = args("--screenshot a.png -port 27031 +maxplayers 4");
        assert_eq!(a.console[..2], ["ip 127.0.0.1", "hostport 27031"]);
    }
}
