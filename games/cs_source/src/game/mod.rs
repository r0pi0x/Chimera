//! Counter-Strike: Source (Steam app 240).
//!
//! The pure-data readers (VPK, pakfile, LZMA lumps, `.vhv`, `.phy`, WAV/MP3,
//! the detail lump, DMX/PCF, `.nav` records, KeyValues, localization files)
//! live in the `source_formats` crate and are re-exported here at their old
//! paths (`vpk`, `pak`, `lumps`, `vhv`, `phy`, `wav`); this module turns what
//! they read into engine types.
//!
//! VPK archives, LZMA-compressed BSP lumps decompressed before anything reads the
//! file (`lumps`; community maps repacked with bspzip), the CS:S search path
//! (plus the install's `download/` folder and Chimera's content cache, last;
//! `mount::import_map` copies `.bsp`/`.bsp.bz2` maps into the cache), BSP to
//! `MapData` (via `vbsp`), VMT/VTF materials (map pak first, then the mount;
//! VTF formats the `vtf` crate lacks, IA88 and the 16-bit ones, decoded in
//! `material.rs`, and half-float RGBA; what the VMT parser refuses is read
//! again leniently, the game's way (`material::lenient_vmt`: unbalanced braces,
//! DX-level blocks, repeated keys, oversized vectors, `{r g b}` colours,
//! WorldVertexTransition without `$basetexture2`), shaders it doesn't know as
//! stand-ins (WindowImposter, ShatteredGlass, LightmappedReflective, `_dx90`
//! names); pak entries the zip reader's LZMA decoder refuses through lzma-rs),
//! lightmap atlas from the lighting lump, brush entities (drawn and solid where
//! they spawn; movers and breakables on their own node, `bsp::MOVERS`,
//! `breakables::CLASSES`), gib lists from `scripts/propdata.txt`, the falling
//! pane piece and windows' broken looks (`$crackmaterial`, the glass edge
//! materials; `breakables.rs`), prop data (`propdata.rs`: a model's `prop_data`
//! over its propdata.txt template chain, interactions, `.phy` break pieces
//! (`phy::break_blocks`), chunk size limits, surface break sounds) handed to
//! the logic as `PROP_*_KEY` keyvalues and to the map as `MapModel::breaks`,
//! the entity lump as `MapData::entities` (`bsp::map_entities`),
//! env_spritetrail and env_smokestack (`trails.rs`), prop_ragdoll drawn in
//! Hammer's pose or lying (`props::ragdoll_pose`; `MapProp::pose`), prop
//! entities' model attachments as `$attachment` keys for SetParentAttachment
//! (`props::add_attachment_keys`), info_particle_system's `.pcf` effects
//! (`pcf.rs`: the install's particle manifest, then the map's own from its pak
//! or the install; binary DMX encoding 2; the named systems and their children
//! into `map::psys`), brush collision hulls, static props and prop entities
//! (`prop_physics*`, `prop_dynamic*`) via `vmdl`, with their lower levels of
//! detail (`props::lod_models`, `MapModel::lods`) (a static prop that collides
//! by its physics model blocks nothing without one), detail props (`detail.rs`:
//! the `dprp` game lump, compressed or not; sprites and their cross and tri
//! shapes as `MapDetailProps`, models as non-solid static props,
//! env_detail_controller's fade, env_wind's sway), ambient cubes and world
//! lights for prop light probes, static props' baked per-vertex light from the
//! pakfile's `sp_<n>.vhv`/`sp_hdr_<n>.vhv` (`vhv.rs`, mapped through the
//! model's LOD 0 `.vtx` vertex order, `props::lod0_vertex_order`; replaces the
//! probe in `MapProp::vertex_light`), infodecals clipped onto faces (the
//! world's, or a brush entity's when the decal's probe hits it), overlays (lump
//! 45) clipped onto their listed faces (brush entity faces in that entity's
//! space; DecalModulate decals and overlays multiply the surface,
//! `MapMesh::modulate`), 2D sky and 3D skybox, ropes (simulated to rest, their
//! nodes kept for the wind), Source player movement (`cs_source:movement`:
//! swept box, exact against brush planes, displacement triangles and moving
//! solids, physics shape casts for props; ladders, water, walking, fall damage,
//! base velocity from pushes and moving platforms), `.phy` collision models
//! (and player models' ragdolls, `phy::parse_ragdoll` → `props::ragdoll`),
//! surface properties, physics props (avian bodies) and the multiplayer
//! push-away between players and multiplayer props, the players' physics
//! shadows for `prop_physics` (`shadow.rs`: driven to the player after the
//! movement, the player follows it after the physics step), WAV decoding (PCM,
//! MS-ADPCM) and sound scripts (plus the map's ambient_generic sounds, raw
//! waves as entries named by path; model doors (`prop_door_rotating`) become
//! movers whose node carries the model, with their `.phy` as volumes and
//! `door_options` sounds (its skin's move/open/close, its hardware's
//! locked/unlocked); prop entities get their prop_data health as a keyvalue;
//! models keep every skin family's materials and their body parts
//! (`props::body_layout`), and prop_dynamic entities that play sequences
//! (DefaultAnim, or the target of SetAnimation outputs) load skinned with their
//! skeleton and `.mdl` sequences (`props::load_rig`); animated (1-31, with
//! their preset or the light's `pattern`, `lightmap::style_pattern`) and
//! switchable (32+) light styles are kept apart in the lightmap atlas
//! (`lightmap::face_extra_styles`); sprites, dust volumes and
//! trigger_soundscape zones know their entity; prop entities' nodes carry
//! `map::PropEntity` so a sound can follow them), footstep/jump/landing/water
//! sounds in movement, the knife and the guns (`weapons.rs`: all 24 CS:S guns,
//! the knife, grenades and C4 as data, games/cs_source/data/weapons, of script
//! values built into the weapon parts with CS:S's hooks (scopes,
//! silencers, bursts, pellets, shell-by-shell reloads), prices, team limits and
//! the buy menu (ammo boxes per ammo type, unmeasured), measured rules: recoil
//! sets, inaccuracy with alternate-mode keys, penetration materials and ammo;
//! unmeasured rules marked, borrowing the nearest measured gun's), bullet and
//! physics impact sounds (`impacts.rs`), hurt and death sounds (`pain.rs`: the
//! install's `Player.Damage*`/`Player.Death*`/`Player.FallDamage` entries from
//! the victim, hurt ones the server's `map::GameSound`, deaths from
//! `core::Died`), scrapes (`scrapes.rs`: looping friction sounds of props,
//! pieces and ragdolls sliding, from a stand-in for Source's friction energy,
//! `friction_energy`), steam jets (`steam.rs`: env_steam keys to
//! `map::steam::MapSteam`, colour from the light field along the jet), beams
//! and glows (`beams.rs`: point_spotlight traced to the first wall, env_laser
//! and persistent env_beam between their placed ends, env_lightglow, into
//! `map::beams`), func_physbox bodies (`bsp::brush_bodies`: the compiler's mass
//! from the physics collision lump, `brush_model_physics`, with massScale or an
//! overridescript mass, the surface's friction and elasticity; the brush node
//! becomes a body), impact particles by surface material, the ricochet sound,
//! blood (and its stains on walls behind) and water splashes (bullets and the
//! knife), glass shards of shattered panes and the glass impact where a hit
//! shattered one (`impact_effects.rs`: the spec's numbers on `map::particles`;
//! materials loaded with the map), the grenades (`grenades.rs`,
//! specs/cs_source/grenades.md: HE, flashbang and smoke as `Throwable` weapons
//! with the spec's numbers, the HE explosion's particles, the smoke cloud's 64
//! churning sprites, the flash model fit; `fire.rs`, specs/source/fire.md 7:
//! lit fires (`map::fire::MapFires`) as `env_fire_large_smoke` (smoke, flames,
//! embers) and burning entities as `burning_character` plus bubbles in water,
//! on `map::particles`; the third-person grenade gesture is in
//! `player_anim.rs`, the pin and throw sequences in `view_anim.rs`), `.nav`
//! meshes (`nav.rs`, versions 9 and 16, with hiding spots, visibility and
//! encounter spots), soundscapes (`soundscape.rs`), model animations
//! (`anim.rs`: `.mdl` v44–48 animations (in the file, in sections, or in
//! external `.ani` blocks, with the v48 zero-frame cache when a block is
//! missing), sequences, ground speeds and include models decoded from raw bytes
//! into a `map::anim::AnimSet`), the player animation state driving them
//! (`player_anim.rs`) and the hostages' (`hostage_anim.rs`: HL2 citizen
//! idle/walk/run by speed, the body turned toward its heading, a nod on follow
//! and stop; registered by `PlayerAnimPlugin`), Water materials (`water.rs`:
//! KeyValues with patch merging, `Water_DX90` keys,
//! TextureScroll/AnimatedTexture/WaterLOD proxies, `water_lod_control`,
//! animated normal-map frames, bottom materials, `$forceexpensive`, the
//! `$underwateroverlay` Refract material as a `MapScreenWarp`), view models
//! (`props.rs::load_view_model`, paths and handedness in the weapon data,
//! `weapons.rs::view_models`; `anim.rs` also reads animation events and
//! attachments), what they play from weapon events and the effects their
//! animation events ask for (`view_anim.rs`: draw, fire, knife slash/stab,
//! reload, idle by activity, silenced variants and silencer attach/detach, one
//! burst sequence per burst, last-round sequence, the Elites' left and right
//! hands, shotgun reload start/insert/finish, hidden behind a scope; muzzle
//! flash and shell data; the view-model cvars), bob and sway
//! (`view_motion.rs`), and the light field for moving models
//! (`props::probe_with` at run time), the HUD look from the install (`hud.rs`:
//! `hudlayout.res`, `clientscheme.res` (read as the Windows game reads it:
//! platform conditionals) with its text fonts and TTFs, `mod_textures.txt`
//! glyphs; sprites for the scope, radio icon, scoreboard status icons and the
//! bomb's marker and LED; the model screens of `scripts/vgui_screens.txt` with
//! the C4 panel scheme `c4panel.res`; `vgui.rs`: the VGUI menus: a `.res`
//! reader (KeyValues controls, `#base` merges, `#token` text and `&` hotkeys
//! from `cstrike_english.txt`) for the buy pages per team, each item's
//! `classes/<item>.res` panel and its `materials/vgui/` picture, `teammenu.res`
//! and `maps/<map>.txt`, `scoreboard.res` (its `../vgui/` picture path
//! resolved), `spectator.res` and `bottomspectator.res`, the spectator menu's
//! command menus `spectatormenu.res` and `spectatormodes.res`
//! (`vgui::command_menu`), strings from `valve_english.txt` under
//! `cstrike_english.txt`, and the scheme's fonts (`vgui::fonts`: sizes by
//! `yres`, weight, `antialias`); `gameui.rs`: the GameUI look as the Windows
//! game reads it (platform conditionals): `SourceScheme.res` over `platform/`'s
//! `SourceSchemeBase.res` (the mount ends with the install's `platform/`
//! folder), `GameMenu.res`, `OptionsSub*.res` (and the keyboard and video tabs'
//! `OptionsSubKeyboardAdvancedDlg.res`, `OptionsSubVideoAdvancedDlg.res`, the
//! brightness dialog's `OptionsSubVideoGammaDlg.res`; their ImagePanels'
//! pictures from `materials/vgui/`, `GameUi::option_images`), Create Server's
//! `CreateMultiplayerGame*Page.res` and `cfg/settings.scr` (`server_settings`),
//! `kb_act.lst`,
//! `gameui_english.txt`/`valve_english.txt`/`cstrike_english.txt`, the loading
//! dialog (`LoadingDialogNoBanner.res`), `sound/ui/button*.wav`, the server
//! browser's `servers/` layouts (`DialogServerBrowser.res`,
//! `InternetGamesPage*.res`, `DialogAddServer.res`,
//! `DialogServerPassword.res`), `serverbrowser_english.txt` and
//! `icon_password`/`icon_bots` TGAs; the BSP loader reports its stages to
//! `map::loading`; the main menu's `materials/console/background01*.vtf`
//! pictures, `gameinfo.txt` title lines and `clientscheme.res`'s
//! `ClientTitleFont` file), the radio (`radio.rs`: menus, call texts and the
//! chat format from `resource/cstrike_english.txt` (UTF-16 localization), each
//! call's `game_sounds_radio.txt` entry; the text chat formats `Cstrike_Chat_*`
//! and `HudChat.Message`, the team join lines `Cstrike_game_join_*`), the bomb
//! and hostages (`objectives.rs`: `weapon_c4` (slot 5, terrorists pick it up),
//! the planted bomb's and kit's models (items with no player-skeleton bone load
//! in model space, `props::load_held`), the `v_c4` press-button sequence on
//! `ArmingStarted`, the third-person `*_Shoot_C4` gesture, sound entries, the
//! $200 defusal kit for CTs, the four hostage models loaded on maps with
//! hostages). CS:S demos played back with their players as puppets (`demo.rs`);
//! a client demo's (or the probe's) recorded input played again through our
//! movement and weapons and compared (`resim.rs`, the `demosim` tool).

pub mod bsp;
pub mod demo;
pub mod drown;
pub mod fire;
pub mod gameui;
pub mod grenades;
pub mod hit_reaction;
pub mod hostage_anim;
pub mod hud;
pub mod impact_effects;
pub mod impacts;
pub mod mount;
pub mod movement;
pub mod net;
pub mod objectives;
pub mod pain;
pub mod parts;
pub mod player_anim;
pub mod pushaway;
pub mod radio;
pub mod resim;
pub mod rules;
pub mod rungs;
pub mod scrapes;
pub mod shadow;
pub mod sound;
pub mod soundscape;
pub mod view_anim;
pub mod vgui;
pub mod view_motion;
pub mod votes;
pub mod weapons;

// Pure-data readers moved to `source_formats`, at their old paths.
pub use source_formats::{lumps, pak, phy, vhv, vpk, wav};

// Source's content converters lifted into `source_engine` (docs/plans/
// active/hl2dm.md, section 3), at their old paths.
pub use source_engine::content::{
    ambient, anim, beams, breakables, decals, detail, dust, lightmap, material, nav, overlays, pcf, propdata, props,
    ropes, sky, sprites, steam, surfaceprops, trails, water,
};

/// The game's id (`cs_source`): defined with the gameplay's condition
/// for CS:S's match-wide systems (`objectives::plays`), the lowest of its
/// modules, so its rules and bots name it too.
pub const GAME: &str = crate::objectives::GAME;

/// What the client shell knows of CS:S (`game_registry`): its install
/// and folder, map cycle, tick, default binds and movement cvars;
/// registered by `CsSourcePlugins`.
pub static INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: GAME,
    short_name: "CS:S",
    content_dir: "cstrike",
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle,
    map_names,
    steam: Some(crate::mount::install::SteamGame {
        app_id: 240,
        name: "Counter-Strike: Source",
        required: &["cstrike/cstrike_pak_dir.vpk", "hl2/hl2_misc_dir.vpk"],
    }),
    map_file: |name| Some(format!("maps/{name}.bsp")),
    import_map: mount::import_map,
    default_binds_cfg: "cfg/config_default.cfg",
    default_binds: DEFAULT_BINDS,
    old_default_binds: OLD_DEFAULT_BINDS,
    movement_cvars: movement::CVARS,
    meters_per_unit: 0.0254,
    console_namespaces: &["cs_source", "css"],
};

// Source's map listings (`maps/<name>.bsp`, the map cycle files), at
// their old paths.
pub use source_engine::maps::{map_cycle, map_names, parse_map_cycle};

/// CS:S's maps from the install (`map::MapLoader`): what an app that
/// plays CS:S maps chooses, the faithful one or a mashup
/// (`map::MapSources`).
pub static MAP_LOADER: crate::map::MapLoader = crate::map::MapLoader {
    game: GAME,
    load: load_installed_map,
};

/// Map `name` (`de_dust2`) from the install (`mount::install`:
/// chimera.local.toml, saved, or Steam) with `options`.
pub fn load_installed_map(name: &str, options: &crate::map::MapLoadOptions) -> Result<crate::map::MapData, String> {
    let install = INFO
        .install()
        .ok_or_else(|| format!("no `{GAME}` install found (chimera_install <folder>, or chimera.local.toml)"))?;
    let mount = mount::open(&install).map_err(|e| e.to_string())?;
    bsp::load_level_with(&mount, name, options)
}

/// CS:S's players and the guns `kept` names on a map of another game
/// (`map::MapSources::guests`), from the install; the map gets no CS:S
/// parts when there is no install.
pub fn dress_guest(map: &mut crate::map::MapData, kept: &'static [&'static str]) -> Result<(), String> {
    let install = INFO
        .install()
        .ok_or_else(|| format!("no `{GAME}` install found (chimera_install <folder>, or chimera.local.toml)"))?;
    let mount = mount::open(&install).map_err(|e| e.to_string())?;
    bsp::dress_guest(&mount, map, kept);
    Ok(())
}

/// CS:S's default binds as its `cfg/config_default.cfg` lists them (what a
/// fresh CS:S config gets), in that file's order: the fallback when the
/// install's file can't be read (the shell's `binds::read_install_defaults`
/// reads it at start). Esc (`cancelselect`) and ` (`toggleconsole`) are
/// not binds. Some have nothing to run yet (`impulse 100`, `commandmenu`,
/// `showbriefing`, `cheer`, `save`, `load`, `pause`): the
/// keys are CS:S's all the same.
pub const DEFAULT_BINDS: &[(&str, &str)] = &[
    ("tab", "+showscores"),
    ("space", "+jump"),
    (",", "buyammo1"),
    (".", "buyammo2"),
    ("0", "slot10"),
    ("1", "slot1"),
    ("2", "slot2"),
    ("3", "slot3"),
    ("4", "slot4"),
    ("5", "slot5"),
    ("6", "slot6"),
    ("7", "slot7"),
    ("8", "slot8"),
    ("9", "slot9"),
    ("a", "+moveleft"),
    ("b", "buymenu"),
    ("c", "radio3"),
    ("d", "+moveright"),
    ("e", "+use"),
    ("f", "impulse 100"),
    ("g", "drop"),
    ("h", "commandmenu"),
    ("i", "showbriefing"),
    ("j", "cheer"),
    ("k", "+voicerecord"),
    ("m", "chooseteam"),
    ("n", "nightvision"),
    ("o", "buyequip"),
    ("q", "lastinv"),
    ("r", "+reload"),
    ("s", "+back"),
    ("t", "impulse 201"),
    ("u", "messagemode2"),
    ("w", "+forward"),
    ("x", "radio2"),
    ("y", "messagemode"),
    ("z", "radio1"),
    ("ctrl", "+duck"),
    ("shift", "+speed"),
    ("f1", "autobuy"),
    ("f2", "rebuy"),
    ("f4", "bug"),
    ("f5", "jpeg"),
    ("f6", "save quick"),
    ("f7", "load quick"),
    ("f10", "quit prompt"),
    ("mwheeldown", "invnext"),
    ("mwheelup", "invprev"),
    ("mouse1", "+attack"),
    ("mouse2", "+attack2"),
    ("pause", "pause"),
];

/// The defaults before they followed CS:S's `config_default.cfg`
/// (2026-10-13): a config written before then (without the shell's
/// `binds::DEFAULTS_MARK`) gets every default not among these added once,
/// on free keys (`binds::added_defaults`). Alt, V and F9 are Chimera's
/// own (`binds::CHIMERA_BINDS`), defaults then as now.
pub const OLD_DEFAULT_BINDS: &[(&str, &str)] = &[
    ("w", "+forward"),
    ("s", "+back"),
    ("a", "+moveleft"),
    ("d", "+moveright"),
    ("space", "+jump"),
    ("ctrl", "+duck"),
    ("shift", "+speed"),
    ("e", "+use"),
    ("r", "+reload"),
    ("mouse1", "+attack"),
    ("mouse2", "+attack2"),
    ("1", "slot1"),
    ("2", "slot2"),
    ("3", "slot3"),
    ("4", "slot4"),
    ("5", "slot5"),
    ("q", "lastinv"),
    ("g", "drop"),
    ("b", "buymenu"),
    (",", "buyammo1"),
    (".", "buyammo2"),
    ("m", "chooseteam"),
    ("tab", "+showscores"),
    ("z", "radio1"),
    ("x", "radio2"),
    ("c", "radio3"),
    ("y", "messagemode"),
    ("u", "messagemode2"),
    ("alt", "+freelook"),
    ("v", "noclip"),
    ("f9", "bug"),
];

/// CS:S's server tick: 0.015 s (66.67 Hz). Measured with the movement probe
/// (tools/css_probe): a dedicated server started with -tickrate 64 still
/// accelerates and applies friction in 0.015 s steps, so CS:S fixes it.
pub const TICK_INTERVAL: f64 = 0.015;

/// The faithful game's parts, after `SimPlugins`: the game's registration
/// (`INFO`, the default game when it's the first), Source movement, CS:S's
/// weapons (with the bomb and the hostages), its rule modules, and the
/// player and view model animation. The dedicated server leaves the
/// animation out (`.build().disable::<..>()`), as it always has; a windowed game adds the
/// front end (`ui::CsSourceUiPlugin`) after the client shell.
pub struct CsSourcePlugins;

impl bevy::app::PluginGroup for CsSourcePlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        bevy::app::PluginGroupBuilder::start::<Self>()
            .add(crate::game_registry::RegisterGame(&INFO))
            .add(movement::SourceMovementPlugin)
            .add(weapons::CsWeaponsPlugin)
            .add(rules::CsRulesPlugin)
            .add(player_anim::PlayerAnimPlugin)
            .add(view_anim::ViewAnimPlugin)
            .add(demo::DemoPlugin)
    }
}
